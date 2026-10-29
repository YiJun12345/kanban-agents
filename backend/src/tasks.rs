use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{Task, TaskStatus};

/// Move a task to a new status, enforcing the board rules.
/// Both users (moveTask) and agents (when they finish) go through here.
pub async fn transition(pool: &PgPool, id: Uuid, to: TaskStatus) -> anyhow::Result<Task> {
    let mut tx = pool.begin().await?;

    // Lock the row so two requests can't move the same task at once
    let current: TaskStatus =
        sqlx::query_scalar("SELECT status FROM tasks WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;

    if !current.can_move_to(to) {
        anyhow::bail!("cannot move task from {current:?} to {to:?}");
    }

    let task = sqlx::query_as::<_, Task>(
        "UPDATE tasks SET status = $2, updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(to)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(task)
}
