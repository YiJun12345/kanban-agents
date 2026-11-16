//! The review step: a person approves the agent's work (merge it) or asks for changes (re-run).

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::agent::Agents;
use crate::domain::{JournalActor, JournalEntry, JournalKind, Project, Task, TaskStatus};
use crate::events::{BoardEvent, Events};
use crate::journal::{self, NewEntry};
use crate::{git, tasks};

async fn task_in_review(pool: &PgPool, id: Uuid) -> anyhow::Result<Task> {
    let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task not found"))?;
    if task.status != TaskStatus::InReview {
        anyhow::bail!("cannot move task from {:?} to Done", task.status);
    }
    Ok(task)
}

/// Approve a task: merge its branch into the project's main branch, delete the worktree
/// and the branch, and move the card to Done. If the merge fails, nothing changes.
pub async fn approve(pool: &PgPool, events: &Events, id: Uuid) -> anyhow::Result<Task> {
    let task = task_in_review(pool, id).await?;

    // A task whose agent has run has a branch and a worktree to merge and clean up
    if let (Some(branch), Some(worktree)) = (&task.branch, &task.worktree_path) {
        let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
            .bind(task.project_id)
            .fetch_one(pool)
            .await?;
        let base = &project.default_branch;
        let entries = sqlx::query_as::<_, JournalEntry>(
            "SELECT * FROM journal_entries WHERE task_id = $1 ORDER BY seq",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;

        let message = merge_message(&task.title, &entries);
        let commit = match git::merge(&project.repo_path, base, branch, &message).await {
            Ok(commit) => commit,
            Err(e) => {
                // Say why in the journal; the card stays in In Review
                let entry = NewEntry::new(id, JournalActor::System, JournalKind::Error, format!("Merge failed: {e:#}"));
                journal::record(pool, events, entry).await?;
                return Err(e);
            }
        };

        // The work is merged; failing to tidy up must not undo that
        if let Err(e) = git::remove_worktree(&project.repo_path, worktree, branch).await {
            tracing::warn!("could not remove worktree {worktree}: {e:#}");
        }
        sqlx::query("UPDATE tasks SET merge_commit = $2, worktree_path = NULL WHERE id = $1")
            .bind(id)
            .bind(&commit)
            .execute(pool)
            .await?;

        let message = match &commit {
            Some(commit) => format!("Merged {branch} into {base} as {commit}"),
            None => format!("Nothing to merge: {branch} has no changes"),
        };
        let details = json!({ "commit": commit, "branch": branch, "base": base });
        let entry = NewEntry::new(id, JournalActor::System, JournalKind::Merge, message).details(details);
        journal::record(pool, events, entry).await?;
    }

    let (task, entry) = tasks::transition(pool, id, TaskStatus::Done, JournalActor::Human).await?;
    let _ = events.send(BoardEvent::TaskUpdated(task.clone()));
    let _ = events.send(BoardEvent::Journal(entry));
    Ok(task)
}

/// Send a task back to the agent with the reviewer's feedback
pub async fn request_changes(
    pool: &PgPool,
    events: &Events,
    agents: &Arc<Agents>,
    id: Uuid,
    feedback: &str,
) -> anyhow::Result<Task> {
    let feedback = feedback.trim();
    if feedback.is_empty() {
        anyhow::bail!("say what should change");
    }
    let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("task not found"))?;
    if task.status != TaskStatus::InReview {
        anyhow::bail!("changes can only be requested for a task in review");
    }

    // The feedback is a journal note, so it stays on record and later runs can read it
    let entry = NewEntry::new(id, JournalActor::Human, JournalKind::Note, feedback)
        .details(json!({ "review": "changes_requested" }));
    journal::record(pool, events, entry).await?;

    let (task, entry) = tasks::transition(pool, id, TaskStatus::InProgress, JournalActor::Human).await?;
    let _ = events.send(BoardEvent::TaskUpdated(task.clone()));
    let _ = events.send(BoardEvent::Journal(entry));
    // A built-in agent starts again now; a task for an outside worker waits to be claimed.
    // Either way the worker is given this feedback when it claims the task.
    agents.dispatch(pool.clone(), events.clone(), task.clone());
    Ok(task)
}

/// The merge commit tells the task's whole story, so `git log` on main explains each change
fn merge_message(title: &str, entries: &[JournalEntry]) -> String {
    let mut message = format!("Merge task: {title}");
    if let Some(summary) = entries.iter().rfind(|e| e.kind == JournalKind::Summary) {
        message.push_str(&format!("\n\n{}", summary.message));
    }
    if !entries.is_empty() {
        message.push_str(&format!("\n\nJournal:\n{}\n", journal::as_context(entries)));
    }
    message
}
