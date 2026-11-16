use async_graphql::{Enum, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Enum, sqlx::Type, Copy, Clone, Debug, PartialEq, Eq)]
#[sqlx(type_name = "task_status", rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Failed,
}

impl TaskStatus {
    /// Name shown to people, e.g. "In Review"
    pub fn label(self) -> &'static str {
        match self {
            TaskStatus::Todo => "To Do",
            TaskStatus::InProgress => "In Progress",
            TaskStatus::InReview => "In Review",
            TaskStatus::Done => "Done",
            TaskStatus::Failed => "Failed",
        }
    }

    pub fn can_move_to(self, to: TaskStatus) -> bool {
        use TaskStatus::*;
        matches!(
            (self, to),
            (Todo, InProgress)
                | (InProgress, InReview)
                | (InProgress, Failed)
                | (InProgress, Todo)
                | (InReview, Done)
                | (InReview, InProgress)
                | (Failed, Todo)
                | (Failed, InProgress) // retry
        )
    }
}

#[derive(Enum, sqlx::Type, Copy, Clone, Debug, PartialEq, Eq)]
#[sqlx(type_name = "agent_kind", rename_all = "snake_case")]
pub enum AgentKind {
    Claude,
    Codex,
    Fake,
    /// No agent is started by the board: any worker with a token can claim the task
    External,
}

#[derive(SimpleObject, sqlx::FromRow, Clone, Debug)]
#[graphql(complex)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub repo_path: String,
    pub default_branch: String,
    pub created_at: DateTime<Utc>,
}

#[derive(SimpleObject, sqlx::FromRow, Clone, Debug)]
#[graphql(complex)] // adds Task.logs in graphql.rs
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub prompt: String,
    pub status: TaskStatus,
    pub agent: AgentKind,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
    /// Set when the task was approved and its branch merged
    pub merge_commit: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Enum, sqlx::Type, Copy, Clone, Debug, PartialEq, Eq)]
#[sqlx(type_name = "journal_actor", rename_all = "snake_case")]
pub enum JournalActor {
    Agent,
    Human,
    System,
}

#[derive(Enum, sqlx::Type, Copy, Clone, Debug, PartialEq, Eq)]
#[sqlx(type_name = "journal_kind", rename_all = "snake_case")]
pub enum JournalKind {
    Status,
    RunStarted,
    RunFinished,
    Note,
    Commit,
    Error,
    Summary, // the agent's own report at the end of a run
    Merge,   // the task's branch was merged when it was approved
    Checkpoint, // where the agent stands: what it did, what it intends, what comes next
}

/// What a note is about
#[derive(Enum, sqlx::Type, Copy, Clone, Debug, PartialEq, Eq)]
#[sqlx(type_name = "note_kind", rename_all = "snake_case")]
pub enum NoteKind {
    Plan,
    Progress,
    Blocker,
    Decision,
    Evidence,
}

impl NoteKind {
    pub fn label(self) -> &'static str {
        match self {
            NoteKind::Plan => "Plan",
            NoteKind::Progress => "Progress",
            NoteKind::Blocker => "Blocker",
            NoteKind::Decision => "Decision",
            NoteKind::Evidence => "Evidence",
        }
    }
}

#[derive(SimpleObject, sqlx::FromRow, Clone, Debug)]
#[graphql(complex)] // adds JournalEntry.details in graphql.rs
pub struct JournalEntry {
    pub id: Uuid,
    pub seq: i64,
    pub task_id: Uuid,
    pub run_id: Option<Uuid>,
    pub actor: JournalActor,
    pub kind: JournalKind,
    pub note_kind: Option<NoteKind>,
    /// The worker that signed this entry, e.g. "worker-1"
    pub author: Option<String>,
    pub message: String,
    #[graphql(skip)] // sqlx's Json type isn't a GraphQL type, so graphql.rs exposes it
    pub details: sqlx::types::Json<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Anything that takes work from the board: the backend's own runners, or an outside
/// agent that proves who it is with a token
#[derive(SimpleObject, sqlx::FromRow, Clone, Debug)]
#[graphql(complex)]
pub struct Worker {
    pub id: Uuid,
    pub name: String,
    #[graphql(skip)] // never leaves the backend
    pub token_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

impl Worker {
    /// The backend's own runners have no token
    pub fn is_builtin(&self) -> bool {
        self.token_hash.is_none()
    }
}

/// Who is working on a task right now. It only lasts while the worker keeps sending heartbeats.
#[derive(SimpleObject, sqlx::FromRow, Clone, Debug)]
#[graphql(complex)]
pub struct Claim {
    pub task_id: Uuid,
    pub worker_id: Uuid,
    pub run_id: Uuid,
    pub claimed_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
}

#[derive(SimpleObject, sqlx::FromRow, Clone, Debug)]
pub struct LogLine {
    pub run_id: Uuid,
    pub seq: i32,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::TaskStatus::{self, *};

    const ALL: [TaskStatus; 5] = [Todo, InProgress, InReview, Done, Failed];

    #[test]
    fn allowed_and_forbidden_moves() {
        assert!(Todo.can_move_to(InProgress));
        assert!(InReview.can_move_to(Done));
        assert!(!Todo.can_move_to(Done)); // can't skip the agent
        assert!(!Done.can_move_to(Todo)); // done is final
    }

    /// Check every (from, to) pair, so adding a rule by accident fails this test
    #[test]
    fn exactly_these_moves_are_allowed() {
        let allowed = [
            (Todo, InProgress),
            (InProgress, InReview),
            (InProgress, Failed),
            (InProgress, Todo),
            (InReview, Done),
            (InReview, InProgress),
            (Failed, Todo),
            (Failed, InProgress),
        ];
        for from in ALL {
            for to in ALL {
                assert_eq!(
                    from.can_move_to(to),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn done_is_final() {
        assert!(ALL.iter().all(|&to| !Done.can_move_to(to)));
    }
}
