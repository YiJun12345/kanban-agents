use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{JournalActor, JournalEntry, JournalKind, Task, TaskStatus};
use crate::journal::{self, NewEntry};

/// Move a task to a new status, enforcing the board rules, and record it in the journal.
/// Both users (moveTask) and agents (when they finish) go through here.
pub async fn transition(
    pool: &PgPool,
    id: Uuid,
    to: TaskStatus,
    actor: JournalActor,
) -> anyhow::Result<(Task, JournalEntry)> {
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

    // Same transaction: the move and its journal entry are saved together, or not at all
    let message = format!("Moved from {} to {}", current.label(), to.label());
    let entry = journal::insert(&mut *tx, NewEntry::new(id, actor, JournalKind::Status, message)).await?;

    tx.commit().await?;
    Ok((task, entry))
}
