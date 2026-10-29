use std::process::Stdio;

use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use uuid::Uuid;

use crate::domain::{LogLine, Task, TaskStatus};
use crate::events::{BoardEvent, Events};
use crate::tasks;

// Pretends to work for a few seconds. Put "fail" in the prompt to make it fail.
const FAKE_AGENT: &str = r#"
echo "Agent starting: $PROMPT"
for i in 1 2 3 4 5; do
  echo "working... step $i/5"
  sleep 1
done
case "$PROMPT" in
  *fail*) echo "something went wrong"; exit 1 ;;
esac
echo "finished"
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

async fn run(pool: &PgPool, events: &Events, task: Task) -> anyhow::Result<()> {
    let run_id: Uuid = sqlx::query_scalar("INSERT INTO runs (task_id) VALUES ($1) RETURNING id")
        .bind(task.id)
        .fetch_one(pool)
        .await?;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(FAKE_AGENT)
        .env("PROMPT", &task.prompt) // pass user text as data, never paste it into the script
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    // Read the agent's output line by line as it is printed
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut lines = BufReader::new(stdout).lines();
    let mut seq = 0;
    while let Some(content) = lines.next_line().await? {
        seq += 1;
        let line = sqlx::query_as::<_, LogLine>(
            "INSERT INTO run_logs (run_id, seq, content) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(run_id)
        .bind(seq)
        .bind(content)
        .fetch_one(pool)
        .await?;
        // send() only fails when nobody is listening, which is fine
        let _ = events.send(BoardEvent::Log { task_id: task.id, line });
    }

    let exit = child.wait().await?;
    sqlx::query("UPDATE runs SET finished_at = now(), exit_code = $2 WHERE id = $1")
        .bind(run_id)
        .bind(exit.code())
        .execute(pool)
        .await?;

    let next = if exit.success() { TaskStatus::InReview } else { TaskStatus::Failed };
    let updated = tasks::transition(pool, task.id, next).await?;
    let _ = events.send(BoardEvent::TaskUpdated(updated));
    Ok(())
}
