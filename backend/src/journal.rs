use serde_json::{json, Value};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::domain::{JournalActor, JournalEntry, JournalKind, NoteKind, Task};
use crate::events::{BoardEvent, Events};

/// A journal entry that hasn't been saved yet
pub struct NewEntry {
    pub task_id: Uuid,
    pub run_id: Option<Uuid>,
    pub actor: JournalActor,
    pub kind: JournalKind,
    pub note_kind: Option<NoteKind>,
    pub author: Option<String>,
    pub message: String,
    pub details: Value,
}

impl NewEntry {
    pub fn new(task_id: Uuid, actor: JournalActor, kind: JournalKind, message: impl Into<String>) -> Self {
        Self {
            task_id,
            run_id: None,
            actor,
            kind,
            note_kind: None,
            author: None,
            message: message.into(),
            details: json!({}),
        }
    }

    pub fn run(mut self, run_id: Uuid) -> Self {
        self.run_id = Some(run_id);
        self
    }

    pub fn note_kind(mut self, note_kind: Option<NoteKind>) -> Self {
        self.note_kind = note_kind;
        self
    }

    /// The worker that signs this entry
    pub fn author(mut self, name: &str) -> Self {
        self.author = Some(name.to_string());
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
        "INSERT INTO journal_entries (task_id, run_id, actor, kind, note_kind, author, message, details)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
    )
    .bind(entry.task_id)
    .bind(entry.run_id)
    .bind(entry.actor)
    .bind(entry.kind)
    .bind(entry.note_kind)
    .bind(entry.author)
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

/// An agent that only prints text writes to the journal with lines like these:
///
///   JOURNAL: looked at the header first          a plain note
///   JOURNAL: PLAN: add the toggle to the header  a note of a kind (PLAN, PROGRESS, BLOCKER,
///                                                DECISION or EVIDENCE)
///   JOURNAL: SUMMARY: added a dark mode toggle   its final report for the run
pub fn parse_agent_line(line: &str) -> Option<(JournalKind, Option<NoteKind>, &str)> {
    let note = line.trim().strip_prefix("JOURNAL:")?.trim();
    let kinds = [
        ("PLAN:", NoteKind::Plan),
        ("PROGRESS:", NoteKind::Progress),
        ("BLOCKER:", NoteKind::Blocker),
        ("DECISION:", NoteKind::Decision),
        ("EVIDENCE:", NoteKind::Evidence),
    ];
    let (kind, note_kind, text) = if let Some(summary) = note.strip_prefix("SUMMARY:") {
        (JournalKind::Summary, None, summary.trim())
    } else if let Some((prefix, note_kind)) = kinds.iter().find(|(p, _)| note.starts_with(p)) {
        (JournalKind::Note, Some(*note_kind), note[prefix.len()..].trim())
    } else {
        (JournalKind::Note, None, note)
    };
    (!text.is_empty()).then_some((kind, note_kind, text))
}

/// An entry's text with its kind in front, e.g. "Plan: add the toggle to the header"
pub fn labelled(entry: &JournalEntry) -> String {
    match entry.note_kind {
        Some(kind) => format!("{}: {}", kind.label(), entry.message),
        None => entry.message.clone(),
    }
}

/// Earlier entries as plain text, so a re-run agent knows what already happened
pub fn as_context(entries: &[JournalEntry]) -> String {
    entries
        .iter()
        .map(|e| format!("[{:?}] {}", e.actor, labelled(e)))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::parse_agent_line;
    use crate::domain::JournalKind::{Note, Summary};
    use crate::domain::NoteKind::{Decision, Plan};

    #[test]
    fn parses_journal_lines() {
        assert_eq!(parse_agent_line("JOURNAL: chose the header"), Some((Note, None, "chose the header")));
        assert_eq!(parse_agent_line("  JOURNAL:no space  "), Some((Note, None, "no space")));
        assert_eq!(parse_agent_line("JOURNAL:   "), None); // empty note
        assert_eq!(parse_agent_line("working... step 1/3"), None);
        assert_eq!(parse_agent_line("my JOURNAL: not at start"), None);
    }

    #[test]
    fn parses_note_kinds_and_summaries() {
        assert_eq!(parse_agent_line("JOURNAL: PLAN: add a toggle"), Some((Note, Some(Plan), "add a toggle")));
        assert_eq!(
            parse_agent_line("JOURNAL: DECISION: use localStorage"),
            Some((Note, Some(Decision), "use localStorage"))
        );
        // Only the exact upper-case words are kinds; anything else is a plain note
        assert_eq!(parse_agent_line("JOURNAL: Plan: x"), Some((Note, None, "Plan: x")));
        assert_eq!(
            parse_agent_line("JOURNAL: SUMMARY: added a greeting"),
            Some((Summary, None, "added a greeting"))
        );
        assert_eq!(parse_agent_line("JOURNAL: SUMMARY:  "), None); // empty summary
        assert_eq!(parse_agent_line("JOURNAL: PLAN:"), None);
    }
}
