use serde_json::{json, Value};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::domain::{JournalActor, JournalEntry, JournalKind, Task};
use crate::events::{BoardEvent, Events};

/// A journal entry that hasn't been saved yet
pub struct NewEntry {
    pub task_id: Uuid,
    pub run_id: Option<Uuid>,
    pub actor: JournalActor,
    pub kind: JournalKind,
    pub message: String,
    pub details: Value,
}

impl NewEntry {
    pub fn new(task_id: Uuid, actor: JournalActor, kind: JournalKind, message: impl Into<String>) -> Self {
        Self { task_id, run_id: None, actor, kind, message: message.into(), details: json!({}) }
    }

    pub fn run(mut self, run_id: Uuid) -> Self {
        self.run_id = Some(run_id);
        self
    }

    pub fn details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }
}

/// Save an entry. Works with a pool or inside a transaction (both are a PgExecutor).
pub async fn insert<'e>(db: impl PgExecutor<'e>, entry: NewEntry) -> anyhow::Result<JournalEntry> {
    let row = sqlx::query_as::<_, JournalEntry>(
        "INSERT INTO journal_entries (task_id, run_id, actor, kind, message, details)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(entry.task_id)
    .bind(entry.run_id)
    .bind(entry.actor)
    .bind(entry.kind)
    .bind(entry.message)
    .bind(sqlx::types::Json(entry.details))
    .fetch_one(db)
    .await?;
    Ok(row)
}

/// Save an entry and tell every open browser: the task's timeline gets the entry,
/// and the board gets the task again so its card can show the newest line
pub async fn record(pool: &PgPool, events: &Events, entry: NewEntry) -> anyhow::Result<JournalEntry> {
    let entry = insert(pool, entry).await?;
    let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(entry.task_id)
        .fetch_one(pool)
        .await?;
    // send() only fails when nobody is listening, which is fine
    let _ = events.send(BoardEvent::Journal(entry.clone()));
    let _ = events.send(BoardEvent::TaskUpdated(task));
    Ok(entry)
}

/// The agent writes to the journal by printing a line like "JOURNAL: chose X because Y".
/// "JOURNAL: SUMMARY: ..." is its final report for the run.
pub fn parse_agent_line(line: &str) -> Option<(JournalKind, &str)> {
    let note = line.trim().strip_prefix("JOURNAL:")?.trim();
    let (kind, text) = match note.strip_prefix("SUMMARY:") {
        Some(summary) => (JournalKind::Summary, summary.trim()),
        None => (JournalKind::Note, note),
    };
    (!text.is_empty()).then_some((kind, text))
}

/// Earlier entries as plain text, so a re-run agent knows what already happened
pub fn as_context(entries: &[JournalEntry]) -> String {
    entries
        .iter()
        .map(|e| format!("[{:?}] {}", e.actor, e.message))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::parse_agent_line;
    use crate::domain::JournalKind::{Note, Summary};

    #[test]
    fn parses_journal_lines() {
        assert_eq!(parse_agent_line("JOURNAL: chose the header"), Some((Note, "chose the header")));
        assert_eq!(parse_agent_line("  JOURNAL:no space  "), Some((Note, "no space")));
        assert_eq!(parse_agent_line("JOURNAL:   "), None); // empty note
        assert_eq!(parse_agent_line("working... step 1/3"), None);
        assert_eq!(parse_agent_line("my JOURNAL: not at start"), None);
    }

    #[test]
    fn parses_summary_lines() {
        assert_eq!(
            parse_agent_line("JOURNAL: SUMMARY: added a greeting"),
            Some((Summary, "added a greeting"))
        );
        assert_eq!(parse_agent_line("JOURNAL: SUMMARY:  "), None); // empty summary
    }
}
