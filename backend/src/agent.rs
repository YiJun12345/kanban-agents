use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;
use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::claude::{self, ClaudeConfig};
use crate::domain::{AgentKind, Task, Worker};
use crate::events::Events;
use crate::ledger::{self, Assignment};

// The fake agent: pretends to work for a few seconds, then edits a file in the worktree.
// It can only print, so it writes to the journal with "JOURNAL:" lines (see
// journal::parse_agent_line). Put "fail" in the prompt to make it fail.
const FAKE_AGENT: &str = r#"
echo "Agent starting: $PROMPT"
if [ -n "$JOURNAL" ]; then
  echo "JOURNAL: Read $(printf '%s\n' "$JOURNAL" | wc -l) earlier journal entries before starting"
fi
if [ -n "$FEEDBACK" ]; then
  echo "JOURNAL: Addressing review feedback: $FEEDBACK"
fi
echo "JOURNAL: PLAN: $PROMPT"
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
echo "JOURNAL: PROGRESS: Wrote the request to AGENT_NOTES.md"
echo "updated AGENT_NOTES.md"
echo "JOURNAL: SUMMARY: Added \"$PROMPT\" to AGENT_NOTES.md"
"#;

/// How often a built-in runner tells the ledger it is still working
const HEARTBEAT_EVERY: Duration = Duration::from_secs(10);

/// The backend's own runners: they start an agent program for a task and report to the
/// ledger for it, like any other worker. Kept here so a run can be stopped at once.
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

    /// A task has entered In Progress. If it is for a built-in agent, start that agent in
    /// the background. A task for an outside worker just waits until one claims it.
    pub fn dispatch(self: &Arc<Self>, pool: PgPool, events: Events, task: Task) {
        if task.agent == AgentKind::External {
            return;
        }
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
            if let Err(e) = run(&pool, &events, &agents.claude, task, stop_rx).await {
                tracing::error!("agent for task {task_id} failed: {e:#}");
            }
            // Forget this run, unless a newer run of the same task has replaced it already
            let mut running = agents.running.lock().unwrap();
            if running.get(&task_id).is_some_and(|(t, _)| *t == token) {
                running.remove(&task_id);
            }
        });
    }

    /// Tell the built-in runner working on this task to stop now. Returns false if none is.
    pub fn stop(&self, task_id: Uuid) -> bool {
        match self.running.lock().unwrap().remove(&task_id) {
            Some((_, stop)) => stop.send(()).is_ok(),
            None => false,
        }
    }
}

/// How the agent's process ended
enum Outcome {
    Exited(Option<i32>), // None: a signal killed it
    Stopped,             // a person stopped it, or the claim was lost
}

/// One run of a built-in agent: claim the task, run the program, report how it ended
async fn run(
    pool: &PgPool,
    events: &Events,
    claude: &ClaudeConfig,
    task: Task,
    stop: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let worker = ledger::builtin(pool, task.agent).await?;
    // If the worktree can't be prepared, claim() has already failed the task and said why
    let job = ledger::claim(pool, events, &worker, task.id).await?;

    match work(pool, events, claude, &worker, &job, stop).await {
        Ok(Outcome::Exited(Some(0))) => {
            ledger::submit(pool, events, &worker, task.id, None).await?;
        }
        Ok(Outcome::Exited(code)) => {
            ledger::fail(pool, events, &worker, task.id, None, code).await?;
        }
        Ok(Outcome::Stopped) => {
            ledger::append_log(pool, events, task.id, job.run_id, "stopped".into()).await?;
            // Usually the board has ended the run already; this covers the other cases
            ledger::revoke(pool, events, task.id).await?;
        }
        // Any error (no API key, the program is missing...) becomes a log line, a journal
        // entry and a Failed card, instead of leaving the card stuck in In Progress
        Err(e) => {
            ledger::append_log(pool, events, task.id, job.run_id, format!("error: {e:#}")).await?;
            ledger::fail(pool, events, &worker, task.id, Some(format!("{e:#}")), None).await?;
        }
    }
    Ok(())
}

/// Run the agent program in the task's worktree and pass on what it prints
async fn work(
    pool: &PgPool,
    events: &Events,
    claude: &ClaudeConfig,
    worker: &Worker,
    job: &Assignment,
    mut stop: oneshot::Receiver<()>,
) -> anyhow::Result<Outcome> {
    let task = &job.task;
    let feedback = job.feedback.as_deref();
    let mut command = match task.agent {
        AgentKind::Fake => fake_command(&task.prompt, &job.journal, feedback),
        AgentKind::Claude => claude.command(&claude::prompt(task, &job.journal, feedback))?,
        AgentKind::Codex => anyhow::bail!("the Codex agent is not supported yet"),
        AgentKind::External => anyhow::bail!("outside workers start their own agents"),
    };
    let mut child = command
        .current_dir(&job.worktree_path) // the agent only sees its own copy of the repo
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

    let stdout = child.stdout.take().expect("stdout is piped");
    let mut lines = BufReader::new(stdout).lines();
    let mut heartbeat = tokio::time::interval(HEARTBEAT_EVERY);

    // Wait for whichever comes first: a line of output, a heartbeat being due, or "stop"
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(content) = line? else { break };
                if task.agent == AgentKind::Claude {
                    // Claude prints JSON events; turn each into lines a person can read
                    for text in claude::readable(&content) {
                        ledger::log_line(pool, events, worker, task.id, text).await?;
                    }
                } else {
                    ledger::log_line(pool, events, worker, task.id, content).await?;
                }
            }
            _ = heartbeat.tick() => {
                // A refused heartbeat means the claim is gone: stop working
                if ledger::heartbeat(pool, events, worker, task.id).await.is_err() {
                    child.kill().await?;
                    return Ok(Outcome::Stopped);
                }
            }
            _ = &mut stop => {
                child.kill().await?;
                return Ok(Outcome::Stopped);
            }
        }
    }

    let exit = child.wait().await?;
    for line in errors.await?.lines().filter(|l| !l.trim().is_empty()) {
        ledger::append_log(pool, events, task.id, job.run_id, format!("stderr: {line}")).await?;
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
