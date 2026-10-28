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
        )
    }
}

#[derive(Enum, sqlx::Type, Copy, Clone, Debug, PartialEq, Eq)]
#[sqlx(type_name = "agent_kind", rename_all = "snake_case")]
pub enum AgentKind {
    Claude,
    Codex,
    Fake,
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
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub prompt: String,
    pub status: TaskStatus,
    pub agent: AgentKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}


#[cfg(test)]
mod tests {
    use super::TaskStatus::*;

    #[test]
    fn allowed_and_forbidden_moves() {
        assert!(Todo.can_move_to(InProgress));
        assert!(InReview.can_move_to(Done));
        assert!(!Todo.can_move_to(Done));   // can't skip the agent
        assert!(!Done.can_move_to(Todo));   // done is final
    }
}
