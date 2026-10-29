use std::process::Stdio;

use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

use crate::domain::{LogLine, Project, Task, TaskStatus};
use crate::events::{BoardEvent, Events};
use crate::{git, tasks};

// Pretends to work for a few seconds, then edits a file in the worktree.
// Put "fail" in the prompt to make it fail.
const FAKE_AGENT: &str = r#"
echo "Agent starting: $PROMPT"
for i in 1 2 3; do
  echo "working... step $i/3"
  sleep 1
done
case "$PROMPT" in
  *fail*) echo "something went wrong"; exit 1 ;;
esac
echo "- $PROMPT" >> AGENT_NOTES.md
echo "updated AGENT_NOTES.md"
"#;

/// Start an agent in the background; returns immediately
pub fn spawn(pool: PgPool, events: Events, task: Task) {
    tokio::spawn(async move {
        let task_id = task.id;
        if let Err(e) = run(&pool, &events, task).await {
            tracing::error!("agent for task {task_id} failed: {e:#}");
        }
    });
}

/// Saves each output line to the database and broadcasts it to browsers
struct RunLog<'a> {
    pool: &'a PgPool,
    events: &'a Events,
    task_id: Uuid,
    run_id: Uuid,
    seq: i32,
}

impl RunLog<'_> {
    async fn line(&mut self, content: String) -> anyhow::Result<()> {
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
        Ok(())
    }
}

async fn run(pool: &PgPool, events: &Events, task: Task) -> anyhow::Result<()> {
    let run_id: Uuid = sqlx::query_scalar("INSERT INTO runs (task_id) VALUES ($1) RETURNING id")
        .bind(task.id)
        .fetch_one(pool)
        .await?;
    let mut log = RunLog { pool, events, task_id: task.id, run_id, seq: 0 };

    // Any error (bad repo path, git failure...) becomes a log line and a Failed card,
    // instead of leaving the card stuck in In Progress
    let exit_code = match work(pool, &mut log, &task).await {
        Ok(code) => code,
        Err(e) => {
            log.line(format!("error: {e:#}")).await?;
            None
        }
    };

    sqlx::query("UPDATE runs SET finished_at = now(), exit_code = $2 WHERE id = $1")
        .bind(run_id)
        .bind(exit_code)
        .execute(pool)
        .await?;

    let next = if exit_code == Some(0) { TaskStatus::InReview } else { TaskStatus::Failed };
    let updated = tasks::transition(pool, task.id, next).await?;
    let _ = events.send(BoardEvent::TaskUpdated(updated));
    Ok(())
}

/// Prepare the worktree, run the agent in it, commit its changes. Returns the exit code.
async fn work(pool: &PgPool, log: &mut RunLog<'_>, task: &Task) -> anyhow::Result<Option<i32>> {
    let worktree = prepare_worktree(pool, task).await?;
    log.line(format!("working in {worktree}")).await?;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(FAKE_AGENT)
        .current_dir(&worktree) // the agent only sees its own copy of the repo
        .env("PROMPT", &task.prompt) // pass user text as data, never paste it into the script
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    // Read the agent's output line by line as it is printed
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut lines = BufReader::new(stdout).lines();
    while let Some(content) = lines.next_line().await? {
        log.line(content).await?;
    }

    let exit = child.wait().await?;
    if exit.success() {
        let committed = git::commit_all(&worktree, &format!("Agent: {}", task.title)).await?;
        log.line(if committed { "committed changes" } else { "no changes to commit" }.into())
            .await?;
    }
    Ok(exit.code())
}

/// Give the task its own branch and worktree (reused on re-runs). Returns the worktree path.
async fn prepare_worktree(pool: &PgPool, task: &Task) -> anyhow::Result<String> {
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
    Ok(path)
}
