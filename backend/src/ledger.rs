//! The ledger: how agents ("workers") take work from the board and report back.
//!
//! A worker claims a task, keeps the claim alive with heartbeats, writes notes while it
//! works, and ends by submitting, failing or releasing. The backend's own runners (the
//! fake agent, Claude Code) go through exactly these functions; outside agents reach the
//! same functions through GraphQL, usually with the `kanban` command-line tool.

use std::time::Duration;

use async_graphql::SimpleObject;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    AgentKind, Claim, JournalActor, JournalEntry, JournalKind, LogLine, NoteKind, Project, Task,
    TaskStatus, Worker,
};
use crate::events::{BoardEvent, Events};
use crate::journal::{self, NewEntry};
use crate::{git, tasks};

/// A claim is lost when its worker sends no heartbeat for this long
pub const LEASE: Duration = Duration::from_secs(60);

/// What a worker gets when it claims a task: everything it needs to start
#[derive(SimpleObject, Clone, Debug)]
pub struct Assignment {
    pub task: Task,
    pub run_id: Uuid,
    pub run_number: i64,
    pub branch: String,
    /// The folder to work in: the task's own git worktree
    pub worktree_path: String,
    /// What happened on this task before this run (empty on the first run)
    pub journal: String,
    /// What the reviewer asked for, when this run follows a "request changes"
    pub feedback: Option<String>,
}

// ---------- workers ----------

fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Add an outside worker. The token is returned once and never stored: we keep its hash.
pub async fn register(pool: &PgPool, name: &str) -> anyhow::Result<(Worker, String)> {
    let name = name.trim();
    let valid = !name.is_empty()
        && name.len() <= 40
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        anyhow::bail!("a worker name is 1 to 40 letters, digits, - or _");
    }
    if name.starts_with("builtin-") {
        anyhow::bail!("names starting with builtin- are reserved");
    }

    // Two random UUIDs: 244 random bits, far too many to guess
    let token = format!("kbt_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let worker = sqlx::query_as::<_, Worker>(
        "INSERT INTO workers (name, token_hash) VALUES ($1, $2)
         ON CONFLICT (name) DO NOTHING RETURNING *",
    )
    .bind(name)
    .bind(hash(&token))
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("a worker named {name} already exists"))?;
    Ok((worker, token))
}

/// Find the worker a token belongs to
pub async fn authenticate(pool: &PgPool, token: &str) -> anyhow::Result<Worker> {
    sqlx::query_as::<_, Worker>(
        "UPDATE workers SET last_seen_at = now() WHERE token_hash = $1 RETURNING *",
    )
    .bind(hash(token))
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("unknown worker token"))
}

/// The backend's own worker for an agent kind, created the first time it is needed
pub async fn builtin(pool: &PgPool, kind: AgentKind) -> anyhow::Result<Worker> {
    let name = format!("builtin-{kind:?}").to_lowercase();
    let worker = sqlx::query_as::<_, Worker>(
        "INSERT INTO workers (name) VALUES ($1)
         ON CONFLICT (name) DO UPDATE SET last_seen_at = now() RETURNING *",
    )
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(worker)
}

// ---------- claiming ----------

async fn get_task(pool: &PgPool, id: Uuid) -> anyhow::Result<Task> {
    sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task not found"))
}

/// The claim this worker holds on this task, or an error if it holds none
async fn held(pool: &PgPool, worker: &Worker, task_id: Uuid) -> anyhow::Result<Claim> {
    sqlx::query_as::<_, Claim>("SELECT * FROM claims WHERE task_id = $1 AND worker_id = $2")
        .bind(task_id)
        .bind(worker.id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("{} does not hold a claim on this task", worker.name))
}

/// Take a task. It must be In Progress, meant for this kind of worker, and not claimed.
/// The worker gets a run, a git worktree to work in, and what happened before.
pub async fn claim(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
) -> anyhow::Result<Assignment> {
    let task = get_task(pool, task_id).await?;
    if task.status != TaskStatus::InProgress {
        anyhow::bail!(
            "only a task in In Progress can be claimed (this one is in {})",
            task.status.label()
        );
    }
    // Outside workers take the tasks marked for them; the built-in runners take the rest
    if worker.is_builtin() == (task.agent == AgentKind::External) {
        anyhow::bail!("this task is for the {:?} agent, not for {}", task.agent, worker.name);
    }

    // The run and the claim are saved together. If someone else holds the claim, the
    // insert does nothing, we return early, and dropping `tx` undoes the run as well.
    let mut tx = pool.begin().await?;
    let run_id: Uuid =
        sqlx::query_scalar("INSERT INTO runs (task_id, worker_id) VALUES ($1, $2) RETURNING id")
            .bind(task_id)
            .bind(worker.id)
            .fetch_one(&mut *tx)
            .await?;
    let claim = sqlx::query_as::<_, Claim>(
        "INSERT INTO claims (task_id, worker_id, run_id) VALUES ($1, $2, $3)
         ON CONFLICT (task_id) DO NOTHING RETURNING *",
    )
    .bind(task_id)
    .bind(worker.id)
    .bind(run_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(claim) = claim else {
        anyhow::bail!("this task is already claimed by another worker");
    };
    tx.commit().await?;

    // A broken repo (moved, deleted...) fails the run instead of leaving the card stuck
    let (branch, worktree_path) = match prepare_worktree(pool, &task).await {
        Ok(ready) => ready,
        Err(e) => {
            append_log(pool, events, task_id, run_id, format!("error: {e:#}")).await?;
            end_as_failed(pool, events, &claim, Some(format!("{e:#}")), None).await?;
            return Err(e);
        }
    };

    // On a re-run, the worker gets the journal so far, so it knows what already happened
    let run_number: i64 = sqlx::query_scalar("SELECT count(*) FROM runs WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await?;
    let journal = if run_number > 1 {
        let earlier = sqlx::query_as::<_, JournalEntry>(
            "SELECT * FROM journal_entries WHERE task_id = $1 ORDER BY seq",
        )
        .bind(task_id)
        .fetch_all(pool)
        .await?;
        journal::as_context(&earlier)
    } else {
        String::new()
    };
    // Feedback counts if the reviewer gave it after the previous run ended
    let feedback: Option<String> = sqlx::query_scalar(
        "SELECT message FROM journal_entries
         WHERE task_id = $1 AND details->>'review' = 'changes_requested'
           AND seq > COALESCE((SELECT max(seq) FROM journal_entries
                               WHERE task_id = $1 AND kind = 'run_finished'), 0)
         ORDER BY seq DESC LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    let message = format!("Run #{run_number} started on branch {branch}");
    let details = json!({ "branch": branch, "worktree": worktree_path, "worker": worker.name });
    let entry = NewEntry::new(task_id, JournalActor::System, JournalKind::RunStarted, message)
        .run(run_id)
        .details(details);
    journal::record(pool, events, entry).await?;
    append_log(pool, events, task_id, run_id, format!("working in {worktree_path}")).await?;

    let task = get_task(pool, task_id).await?; // now with its branch and worktree
    Ok(Assignment { task, run_id, run_number, branch, worktree_path, journal, feedback })
}

/// Take the task that has waited longest for an outside worker, if there is one
pub async fn claim_next(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    project_id: Option<Uuid>,
) -> anyhow::Result<Option<Assignment>> {
    loop {
        let waiting: Option<Uuid> = sqlx::query_scalar(
            "SELECT t.id FROM tasks t
             WHERE t.status = 'in_progress' AND t.agent = 'external'
               AND ($1::uuid IS NULL OR t.project_id = $1)
               AND NOT EXISTS (SELECT 1 FROM claims c WHERE c.task_id = t.id)
             ORDER BY t.updated_at LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
        let Some(task_id) = waiting else { return Ok(None) };

        match claim(pool, events, worker, task_id).await {
            Ok(assignment) => return Ok(Some(assignment)),
            // Another worker was faster: look for the next waiting task
            Err(e) if e.to_string().contains("already claimed") => continue,
            Err(e) => return Err(e),
        }
    }
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

// ---------- while working ----------

/// "I am still working on this." Without it, the claim expires after LEASE.
pub async fn heartbeat(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
) -> anyhow::Result<Claim> {
    let claim = sqlx::query_as::<_, Claim>(
        "UPDATE claims SET heartbeat_at = now() WHERE task_id = $1 AND worker_id = $2 RETURNING *",
    )
    .bind(task_id)
    .bind(worker.id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("{} does not hold a claim on this task", worker.name))?;
    sqlx::query("UPDATE workers SET last_seen_at = now() WHERE id = $1")
        .bind(worker.id)
        .execute(pool)
        .await?;
    let _ = events.send(BoardEvent::TaskUpdated(get_task(pool, task_id).await?));
    Ok(claim)
}

/// Write a journal entry as the worker: a note (of a kind, or plain) or its summary
pub async fn note(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
    kind: JournalKind,
    note_kind: Option<NoteKind>,
    message: &str,
) -> anyhow::Result<JournalEntry> {
    let message = message.trim();
    if message.is_empty() {
        anyhow::bail!("note cannot be empty");
    }
    let claim = held(pool, worker, task_id).await?;
    let entry = NewEntry::new(task_id, JournalActor::Agent, kind, message)
        .run(claim.run_id)
        .note_kind(note_kind)
        .author(&worker.name);
    journal::record(pool, events, entry).await
}

/// Where the worker stands: what it has done, what it is trying to do, what comes next.
/// If the run dies after this, the next worker can pick up from here.
pub async fn checkpoint(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
    summary: &str,
    intent: &str,
    next_action: &str,
) -> anyhow::Result<JournalEntry> {
    if summary.trim().is_empty() {
        anyhow::bail!("a checkpoint needs a summary");
    }
    let claim = held(pool, worker, task_id).await?;
    let details = json!({ "intent": intent.trim(), "next_action": next_action.trim() });
    let entry = NewEntry::new(task_id, JournalActor::Agent, JournalKind::Checkpoint, summary.trim())
        .run(claim.run_id)
        .author(&worker.name)
        .details(details);
    journal::record(pool, events, entry).await
}

/// Add a line to the run's raw output, as the worker
pub async fn log_line(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
    content: String,
) -> anyhow::Result<LogLine> {
    let claim = held(pool, worker, task_id).await?;
    append_log(pool, events, task_id, claim.run_id, content).await
}

/// Add a line to a run's raw output and send it to open browsers
pub async fn append_log(
    pool: &PgPool,
    events: &Events,
    task_id: Uuid,
    run_id: Uuid,
    content: String,
) -> anyhow::Result<LogLine> {
    // The next line number is worked out by the database, in the same statement
    let line = sqlx::query_as::<_, LogLine>(
        "INSERT INTO run_logs (run_id, seq, content)
         SELECT $1, COALESCE(max(seq), 0) + 1, $2 FROM run_logs WHERE run_id = $1
         RETURNING *",
    )
    .bind(run_id)
    .bind(content)
    .fetch_one(pool)
    .await?;
    // send() only fails when nobody is listening, which is fine
    let _ = events.send(BoardEvent::Log { task_id, line: line.clone() });
    Ok(line)
}

// ---------- ending a run ----------

/// The work is finished: commit what is in the worktree and move the task to In Review
pub async fn submit(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
    summary: Option<&str>,
) -> anyhow::Result<Task> {
    let claim = held(pool, worker, task_id).await?;
    if let Some(summary) = summary.map(str::trim).filter(|s| !s.is_empty()) {
        note(pool, events, worker, task_id, JournalKind::Summary, None, summary).await?;
    }

    if let Err(e) = commit_work(pool, events, &claim).await {
        append_log(pool, events, task_id, claim.run_id, format!("error: {e:#}")).await?;
        end_as_failed(pool, events, &claim, Some(format!("{e:#}")), None).await?;
        return Err(e);
    }
    let details = json!({ "exit_code": 0 });
    end(pool, events, &claim, "Run finished in", details, Some(TaskStatus::InReview)).await
}

/// Commit everything in the task's worktree, with a message built from this run's journal
async fn commit_work(pool: &PgPool, events: &Events, claim: &Claim) -> anyhow::Result<()> {
    let task = get_task(pool, claim.task_id).await?;
    let worktree = task.worktree_path.as_deref().ok_or_else(|| anyhow::anyhow!("the task has no worktree"))?;
    let written = sqlx::query_as::<_, JournalEntry>(
        "SELECT * FROM journal_entries
         WHERE run_id = $1 AND actor = 'agent' AND kind IN ('note', 'summary') ORDER BY seq",
    )
    .bind(claim.run_id)
    .fetch_all(pool)
    .await?;

    let message = commit_message(&task.title, &written);
    if git::commit_all(worktree, &message).await? {
        let (hash, files) = git::last_commit(worktree).await?;
        append_log(pool, events, task.id, claim.run_id, "committed changes".into()).await?;
        let summary = format!(
            "Committed {hash}: {} file{} changed",
            files.len(),
            if files.len() == 1 { "" } else { "s" }
        );
        let entry = NewEntry::new(task.id, JournalActor::System, JournalKind::Commit, summary)
            .run(claim.run_id)
            .details(json!({ "commit": hash, "files": files }));
        journal::record(pool, events, entry).await?;
    } else {
        append_log(pool, events, task.id, claim.run_id, "no changes to commit".into()).await?;
    }
    Ok(())
}

/// "Agent: <title>", then the agent's summary and notes, so `git log` explains the work too
fn commit_message(title: &str, written: &[JournalEntry]) -> String {
    let mut message = format!("Agent: {title}");
    if let Some(summary) = written.iter().rfind(|e| e.kind == JournalKind::Summary) {
        message.push_str(&format!("\n\n{}", summary.message));
    }
    let notes: Vec<_> = written.iter().filter(|e| e.kind == JournalKind::Note).collect();
    if !notes.is_empty() {
        message.push_str("\n\n");
        for note in notes {
            message.push_str(&format!("- {}\n", journal::labelled(note)));
        }
    }
    message
}

/// The worker could not do the task: say why, and move it to Failed.
/// `exit_code` is the agent process's exit code, when there was a process.
pub async fn fail(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
    reason: Option<String>,
    exit_code: Option<i32>,
) -> anyhow::Result<Task> {
    let claim = held(pool, worker, task_id).await?;
    end_as_failed(pool, events, &claim, reason, exit_code).await
}

async fn end_as_failed(
    pool: &PgPool,
    events: &Events,
    claim: &Claim,
    reason: Option<String>,
    exit_code: Option<i32>,
) -> anyhow::Result<Task> {
    if let Some(reason) = reason.filter(|r| !r.trim().is_empty()) {
        let entry = NewEntry::new(claim.task_id, JournalActor::System, JournalKind::Error, reason)
            .run(claim.run_id);
        journal::record(pool, events, entry).await?;
    }
    let message = match exit_code {
        Some(code) => format!("Run failed with exit code {code} after"),
        None => "Run stopped by an error after".to_string(),
    };
    let details = json!({ "exit_code": exit_code });
    end(pool, events, claim, &message, details, Some(TaskStatus::Failed)).await
}

/// The worker gives the task back without finishing. It stays In Progress, waiting for a worker.
pub async fn release(
    pool: &PgPool,
    events: &Events,
    worker: &Worker,
    task_id: Uuid,
) -> anyhow::Result<Task> {
    let claim = held(pool, worker, task_id).await?;
    let message = format!("Run released by {} after", worker.name);
    end(pool, events, &claim, &message, json!({ "stopped": true }), None).await
}

/// The board takes the task away from whoever holds it (a person stopped the agent).
/// The worker finds out at its next heartbeat, which is refused.
pub async fn revoke(pool: &PgPool, events: &Events, task_id: Uuid) -> anyhow::Result<()> {
    let claim = sqlx::query_as::<_, Claim>("SELECT * FROM claims WHERE task_id = $1")
        .bind(task_id)
        .fetch_optional(pool)
        .await?;
    if let Some(claim) = claim {
        end(pool, events, &claim, "Run stopped by you after", json!({ "stopped": true }), None).await?;
    }
    Ok(())
}

/// End every claim whose worker has been silent for longer than `lease`: the task goes
/// to Failed, where a person can retry it. Returns how many claims expired.
pub async fn expire(pool: &PgPool, events: &Events, lease: Duration) -> anyhow::Result<usize> {
    let silent = sqlx::query_as::<_, Claim>(
        "SELECT * FROM claims WHERE heartbeat_at < now() - make_interval(secs => $1)",
    )
    .bind(lease.as_secs_f64())
    .fetch_all(pool)
    .await?;

    for claim in &silent {
        let name: String = sqlx::query_scalar("SELECT name FROM workers WHERE id = $1")
            .bind(claim.worker_id)
            .fetch_one(pool)
            .await?;
        let reason = format!("No heartbeat from {name} for {}s, so its claim expired", lease.as_secs());
        let entry = NewEntry::new(claim.task_id, JournalActor::System, JournalKind::Error, reason)
            .run(claim.run_id);
        journal::record(pool, events, entry).await?;
        let details = json!({ "expired": true });
        end(pool, events, claim, "Run abandoned after", details, Some(TaskStatus::Failed)).await?;
        tracing::warn!("claim on task {} by {name} expired", claim.task_id);
    }
    Ok(silent.len())
}

/// After a restart, none of the backend's own runners is alive. Fail the tasks they were
/// working on and say why. Outside workers are separate programs, so their claims stay.
pub async fn recover(pool: &PgPool, events: &Events) -> anyhow::Result<()> {
    let stuck: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM tasks WHERE status = 'in_progress' AND agent <> 'external'",
    )
    .fetch_all(pool)
    .await?;
    for task_id in stuck {
        let reason = "The backend restarted while the agent was running";
        let claim = sqlx::query_as::<_, Claim>("SELECT * FROM claims WHERE task_id = $1")
            .bind(task_id)
            .fetch_optional(pool)
            .await?;
        match claim {
            Some(claim) => {
                end_as_failed(pool, events, &claim, Some(reason.into()), None).await?;
            }
            None => {
                let entry = NewEntry::new(task_id, JournalActor::System, JournalKind::Error, reason);
                journal::insert(pool, entry).await?;
                tasks::transition(pool, task_id, TaskStatus::Failed, JournalActor::System).await?;
            }
        }
        tracing::warn!("task {task_id} was in progress at startup; marked as failed");
    }
    Ok(())
}

/// Close a run: record how it ended and how long it took, drop the claim, and move the
/// task if the ending calls for it. `message` is completed with the duration ("… 3.1s").
async fn end(
    pool: &PgPool,
    events: &Events,
    claim: &Claim,
    message: &str,
    mut details: serde_json::Value,
    next: Option<TaskStatus>,
) -> anyhow::Result<Task> {
    // Dropping the claim is the decisive step. If it is already gone, someone else ended
    // this run first (for example a person stopped it), and there is nothing left to do.
    let dropped = sqlx::query("DELETE FROM claims WHERE task_id = $1 AND run_id = $2")
        .bind(claim.task_id)
        .bind(claim.run_id)
        .execute(pool)
        .await?;
    if dropped.rows_affected() == 0 {
        anyhow::bail!("this run has already ended");
    }

    let secs: f64 = sqlx::query_scalar(
        "UPDATE runs SET finished_at = now(), exit_code = $2 WHERE id = $1
         RETURNING extract(epoch FROM finished_at - started_at)::float8",
    )
    .bind(claim.run_id)
    .bind(details["exit_code"].as_i64().map(|code| code as i32))
    .fetch_one(pool)
    .await?;
    details["duration_secs"] = json!(secs);

    let entry = NewEntry::new(
        claim.task_id,
        JournalActor::System,
        JournalKind::RunFinished,
        format!("{message} {secs:.1}s"),
    )
    .run(claim.run_id)
    .details(details);
    journal::record(pool, events, entry).await?;

    match next {
        Some(status) => {
            let (task, entry) =
                tasks::transition(pool, claim.task_id, status, JournalActor::System).await?;
            let _ = events.send(BoardEvent::TaskUpdated(task.clone()));
            let _ = events.send(BoardEvent::Journal(entry));
            Ok(task)
        }
        None => {
            let task = get_task(pool, claim.task_id).await?;
            let _ = events.send(BoardEvent::TaskUpdated(task.clone()));
            Ok(task)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::hash;

    #[test]
    fn tokens_are_stored_as_sha256() {
        assert_eq!(
            hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
