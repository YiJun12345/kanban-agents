use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Context as _;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::claude::{self, ClaudeConfig};
use crate::domain::{
    AgentKind, JournalActor, JournalEntry, JournalKind, LogLine, Project, Task, TaskStatus,
};
use crate::events::{BoardEvent, Events};
use crate::journal::{self, NewEntry};
use crate::{git, tasks};

// The fake agent: pretends to work for a few seconds, then edits a file in the worktree.
// Lines starting with "JOURNAL:" go into the task's journal; "JOURNAL: SUMMARY:" is its
// final report. Put "fail" in the prompt to make it fail.
const FAKE_AGENT: &str = r#"
echo "Agent starting: $PROMPT"
if [ -n "$JOURNAL" ]; then
  echo "JOURNAL: Read $(printf '%s\n' "$JOURNAL" | wc -l) earlier journal entries before starting"
fi
if [ -n "$FEEDBACK" ]; then
  echo "JOURNAL: Addressing review feedback: $FEEDBACK"
fi
echo "JOURNAL: Plan: $PROMPT"
for i in 1 2 3; do
  echo "working... step $i/3"
  sleep 1
done
case "$PROMPT" in
  *fail*) echo "something went wrong"; echo "JOURNAL: Could not finish: something went wrong"; exit 1 ;;
esac
echo "- $PROMPT" >> AGENT_NOTES.md
if [ -n "$FEEDBACK" ]; then
  echo "  - after review: $FEEDBACK" >> AGENT_NOTES.md
fi
echo "JOURNAL: Wrote the request to AGENT_NOTES.md"
echo "updated AGENT_NOTES.md"
echo "JOURNAL: SUMMARY: Added \"$PROMPT\" to AGENT_NOTES.md"
"#;

/// The agents that are running right now, so a run can be stopped from the board
pub struct Agents {
    pub claude: ClaudeConfig,
    // task id -> (which run, the sender that tells that run to stop)
    running: Mutex<HashMap<Uuid, (u64, oneshot::Sender<()>)>>,
    next_token: Mutex<u64>,
}

impl Agents {
    pub fn new(claude: ClaudeConfig) -> Arc<Self> {
        Arc::new(Self { claude, running: Mutex::default(), next_token: Mutex::default() })
    }

    /// Start an agent on a task in the background; returns immediately.
    /// `feedback` is what the reviewer asked for, when this run follows a review.
    pub fn start(self: &Arc<Self>, pool: PgPool, events: Events, task: Task, feedback: Option<String>) {
        let (stop_tx, stop_rx) = oneshot::channel();
        let token = {
            let mut next = self.next_token.lock().unwrap();
            *next += 1;
            *next
        };
        self.running.lock().unwrap().insert(task.id, (token, stop_tx));

        let agents = self.clone();
        tokio::spawn(async move {
            let task_id = task.id;
            if let Err(e) = run(&pool, &events, &agents.claude, task, feedback, stop_rx).await {
                tracing::error!("agent for task {task_id} failed: {e:#}");
            }
            // Forget this run, unless a newer run of the same task has replaced it already
            let mut running = agents.running.lock().unwrap();
            if running.get(&task_id).is_some_and(|(t, _)| *t == token) {
                running.remove(&task_id);
            }
        });
    }

    /// Ask the agent working on this task to stop. Returns false if none is running.
    pub fn stop(&self, task_id: Uuid) -> bool {
        match self.running.lock().unwrap().remove(&task_id) {
            Some((_, stop)) => stop.send(()).is_ok(),
            None => false,
        }
    }
}

/// After a restart no agent is running, so a task still marked In Progress would wait
/// forever. Mark those as Failed and say why in the journal.
pub async fn recover(pool: &PgPool) -> anyhow::Result<()> {
    let stuck: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM tasks WHERE status = 'in_progress'")
        .fetch_all(pool)
        .await?;
    for id in stuck {
        let message = "The backend restarted while the agent was running";
        journal::insert(pool, NewEntry::new(id, JournalActor::System, JournalKind::Error, message))
            .await?;
        tasks::transition(pool, id, TaskStatus::Failed, JournalActor::System).await?;
        tracing::warn!("task {id} was in progress at startup; marked as failed");
    }
    sqlx::query("UPDATE runs SET finished_at = now() WHERE finished_at IS NULL")
        .execute(pool)
        .await?;
    Ok(())
}

/// Saves output lines and journal entries for one run, and broadcasts them to browsers
struct RunLog<'a> {
    pool: &'a PgPool,
    events: &'a Events,
    task_id: Uuid,
    run_id: Uuid,
    seq: i32,
    // What the agent journaled this run, used for the commit message
    notes: Vec<String>,
    summary: Option<String>,
}

impl RunLog<'_> {
    /// One line of raw output
    async fn line(&mut self, content: String) -> anyhow::Result<()> {
        let note = journal::parse_agent_line(&content).map(|(kind, text)| (kind, text.to_owned()));

        self.seq += 1;
        let line = sqlx::query_as::<_, LogLine>(
            "INSERT INTO run_logs (run_id, seq, content) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(self.run_id)
        .bind(self.seq)
        .bind(content)
        .fetch_one(self.pool)
        .await?;
        // send() only fails when nobody is listening, which is fine
        let _ = self.events.send(BoardEvent::Log { task_id: self.task_id, line });

        if let Some((kind, text)) = note {
            if kind == JournalKind::Summary {
                self.summary = Some(text.clone());
            } else {
                self.notes.push(text.clone());
            }
            self.journal(JournalActor::Agent, kind, text, json!({})).await?;
        }
        Ok(())
    }

    /// One journal entry for this run
    async fn journal(
        &self,
        actor: JournalActor,
        kind: JournalKind,
        message: String,
        details: Value,
    ) -> anyhow::Result<()> {
        let entry = NewEntry::new(self.task_id, actor, kind, message)
            .run(self.run_id)
            .details(details);
        journal::record(self.pool, self.events, entry).await?;
        Ok(())
    }
}

/// How the agent's process ended
enum Outcome {
    Exited(Option<i32>), // None: it never ran, or was killed by a signal
    Stopped,             // someone moved the card away while it was working
}

async fn run(
    pool: &PgPool,
    events: &Events,
    claude: &ClaudeConfig,
    task: Task,
    feedback: Option<String>,
    stop: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let run_id: Uuid = sqlx::query_scalar("INSERT INTO runs (task_id) VALUES ($1) RETURNING id")
        .bind(task.id)
        .fetch_one(pool)
        .await?;
    let started = Instant::now();
    let mut log =
        RunLog { pool, events, task_id: task.id, run_id, seq: 0, notes: Vec::new(), summary: None };

    // Any error (bad repo path, git failure, no API key...) becomes a log line, a journal
    // entry and a Failed card, instead of leaving the card stuck in In Progress
    let outcome = match work(pool, &mut log, claude, &task, feedback.as_deref(), stop).await {
        Ok(outcome) => outcome,
        Err(e) => {
            log.line(format!("error: {e:#}")).await?;
            log.journal(JournalActor::System, JournalKind::Error, format!("{e:#}"), json!({})).await?;
            Outcome::Exited(None)
        }
    };

    let exit_code = match outcome {
        Outcome::Exited(code) => code,
        Outcome::Stopped => None,
    };
    sqlx::query("UPDATE runs SET finished_at = now(), exit_code = $2 WHERE id = $1")
        .bind(run_id)
        .bind(exit_code)
        .execute(pool)
        .await?;

    let secs = started.elapsed().as_secs_f32();
    let (summary, next) = match outcome {
        Outcome::Exited(Some(0)) => (format!("Run finished in {secs:.1}s"), Some(TaskStatus::InReview)),
        Outcome::Exited(Some(code)) => (
            format!("Run failed with exit code {code} after {secs:.1}s"),
            Some(TaskStatus::Failed),
        ),
        Outcome::Exited(None) => {
            (format!("Run stopped by an error after {secs:.1}s"), Some(TaskStatus::Failed))
        }
        // The person who stopped it has already moved the card, so the status stays as it is
        Outcome::Stopped => (format!("Run stopped by you after {secs:.1}s"), None),
    };
    let details = json!({
        "exit_code": exit_code,
        "duration_secs": secs,
        "stopped": matches!(outcome, Outcome::Stopped),
    });
    log.journal(JournalActor::System, JournalKind::RunFinished, summary, details).await?;

    if let Some(next) = next {
        let (updated, entry) = tasks::transition(pool, task.id, next, JournalActor::System).await?;
        let _ = events.send(BoardEvent::TaskUpdated(updated));
        let _ = events.send(BoardEvent::Journal(entry));
    }
    Ok(())
}

/// Prepare the worktree, run the agent in it, commit its changes
async fn work(
    pool: &PgPool,
    log: &mut RunLog<'_>,
    claude: &ClaudeConfig,
    task: &Task,
    feedback: Option<&str>,
    mut stop: oneshot::Receiver<()>,
) -> anyhow::Result<Outcome> {
    let (branch, worktree) = prepare_worktree(pool, task).await?;

    // On a re-run, the agent gets the journal so far, so it knows what already happened
    let run_number: i64 = sqlx::query_scalar("SELECT count(*) FROM runs WHERE task_id = $1")
        .bind(task.id)
        .fetch_one(pool)
        .await?;
    let context = if run_number > 1 {
        let earlier = sqlx::query_as::<_, JournalEntry>(
            "SELECT * FROM journal_entries WHERE task_id = $1 ORDER BY seq",
        )
        .bind(task.id)
        .fetch_all(pool)
        .await?;
        journal::as_context(&earlier)
    } else {
        String::new()
    };

    let message = format!("Run #{run_number} started on branch {branch}");
    let details = json!({ "branch": branch, "worktree": worktree, "agent": format!("{:?}", task.agent) });
    log.journal(JournalActor::System, JournalKind::RunStarted, message, details).await?;
    log.line(format!("working in {worktree}")).await?;

    let mut command = match task.agent {
        AgentKind::Fake => fake_command(&task.prompt, &context, feedback),
        AgentKind::Claude => claude.command(&claude::prompt(task, &context, feedback))?,
        AgentKind::Codex => anyhow::bail!("the Codex agent is not supported yet"),
    };
    let mut child = command
        .current_dir(&worktree) // the agent only sees its own copy of the repo
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("could not start the {:?} agent", task.agent))?;

    // Collect error output on the side; it is added to the log when the agent exits
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let errors = tokio::spawn(async move {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text).await;
        text
    });

    // Read the agent's output line by line as it is printed, until it ends or we are told to stop
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut lines = BufReader::new(stdout).lines();
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(content) = line? else { break };
                if task.agent == AgentKind::Claude {
                    // Claude prints JSON events; turn each into lines a person can read
                    for text in claude::readable(&content) {
                        log.line(text).await?;
                    }
                } else {
                    log.line(content).await?;
                }
            }
            _ = &mut stop => {
                child.kill().await?;
                log.line("stopped".into()).await?;
                return Ok(Outcome::Stopped);
            }
        }
    }

    let exit = child.wait().await?;
    for line in errors.await?.lines().filter(|l| !l.trim().is_empty()) {
        log.line(format!("stderr: {line}")).await?;
    }

    if exit.success() {
        let message = commit_message(&task.title, log.summary.as_deref(), &log.notes);
        if git::commit_all(&worktree, &message).await? {
            let (hash, files) = git::last_commit(&worktree).await?;
            log.line("committed changes".into()).await?;
            let summary = format!(
                "Committed {hash}: {} file{} changed",
                files.len(),
                if files.len() == 1 { "" } else { "s" }
            );
            let details = json!({ "commit": hash, "files": files });
            log.journal(JournalActor::System, JournalKind::Commit, summary, details).await?;
        } else {
            log.line("no changes to commit".into()).await?;
        }
    }
    Ok(Outcome::Exited(exit.code()))
}

fn fake_command(prompt: &str, journal: &str, feedback: Option<&str>) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(FAKE_AGENT)
        .env("PROMPT", prompt) // pass user text as data, never paste it into the script
        .env("JOURNAL", journal)
        .env("FEEDBACK", feedback.unwrap_or_default());
    command
}

/// "Agent: <title>", then the agent's summary and notes, so `git log` explains the work too
fn commit_message(title: &str, summary: Option<&str>, notes: &[String]) -> String {
    let mut message = format!("Agent: {title}");
    if let Some(summary) = summary {
        message.push_str(&format!("\n\n{summary}"));
    }
    if !notes.is_empty() {
        message.push_str("\n\n");
        for note in notes {
            message.push_str(&format!("- {note}\n"));
        }
    }
    message
}

/// Give the task its own branch and worktree (reused on re-runs). Returns (branch, path).
async fn prepare_worktree(pool: &PgPool, task: &Task) -> anyhow::Result<(String, String)> {
    let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
        .bind(task.project_id)
        .fetch_one(pool)
        .await?;

    // e.g. branch "kanban/task-1a2b3c4d" in "/home/you/demo-repo-worktrees/task-1a2b3c4d"
    let short = &task.id.simple().to_string()[..8];
    let branch = format!("kanban/task-{short}");
    let path = format!("{}-worktrees/task-{short}", project.repo_path.trim_end_matches('/'));

    git::ensure_worktree(&project.repo_path, &project.default_branch, &branch, &path).await?;

    sqlx::query("UPDATE tasks SET branch = $2, worktree_path = $3 WHERE id = $1")
        .bind(task.id)
        .bind(&branch)
        .bind(&path)
        .execute(pool)
        .await?;
    Ok((branch, path))
}

#[cfg(test)]
mod tests {
    use super::commit_message;

    #[test]
    fn commit_message_includes_journal_notes() {
        assert_eq!(commit_message("Add greeting", None, &[]), "Agent: Add greeting");
        let notes = ["Plan: add it".to_string(), "Done".to_string()];
        assert_eq!(
            commit_message("Add greeting", Some("Added it"), &notes),
            "Agent: Add greeting\n\nAdded it\n\n- Plan: add it\n- Done\n"
        );
    }
}
