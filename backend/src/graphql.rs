use async_graphql::{ComplexObject, Context, Object, Result, Schema, Subscription};
use sqlx::PgPool;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

use crate::agent;
use crate::domain::{AgentKind, LogLine, Project, Task, TaskStatus};
use crate::events::{BoardEvent, Events};
use crate::tasks;

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema(pool: PgPool, events: Events) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(events)
        .finish()
}


pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn projects(&self, ctx: &Context<'_>) -> Result<Vec<Project>> {
        let pool = ctx.data::<PgPool>()?;
        let rows = sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at")
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    async fn task(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Task>> {
        let pool = ctx.data::<PgPool>()?;
        let row = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }


    async fn project(&self, ctx: &Context<'_>, id: Uuid) -> Result<Option<Project>> {
        let pool = ctx.data::<PgPool>()?;
        let row = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }
}

#[ComplexObject]
impl Project {
    async fn tasks(&self, ctx: &Context<'_>) -> Result<Vec<Task>> {
        let pool = ctx.data::<PgPool>()?;
        let rows = sqlx::query_as::<_, Task>(
            "SELECT * FROM tasks WHERE project_id = $1 ORDER BY created_at",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

#[ComplexObject]
impl Task {
    /// Output of the most recent agent run
    async fn logs(&self, ctx: &Context<'_>) -> Result<Vec<LogLine>> {
        let pool = ctx.data::<PgPool>()?;
        let rows = sqlx::query_as::<_, LogLine>(
            "SELECT * FROM run_logs
             WHERE run_id = (SELECT id FROM runs WHERE task_id = $1 ORDER BY started_at DESC LIMIT 1)
             ORDER BY seq",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn create_project(
        &self,
        ctx: &Context<'_>,
        name: String,
        repo_path: String,
    ) -> Result<Project> {
        let pool = ctx.data::<PgPool>()?;
        let row = sqlx::query_as::<_, Project>(
            "INSERT INTO projects (name, repo_path) VALUES ($1, $2) RETURNING *",
        )
        .bind(name)
        .bind(repo_path)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    async fn create_task(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
        title: String,
        prompt: String,
        agent: Option<AgentKind>,
    ) -> Result<Task> {
        let pool = ctx.data::<PgPool>()?;
        let row = sqlx::query_as::<_, Task>(
            "INSERT INTO tasks (project_id, title, prompt, agent)
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(project_id)
        .bind(title)
        .bind(prompt)
        .bind(agent.unwrap_or(AgentKind::Fake))
        .fetch_one(pool)
        .await?;

        let _ = ctx.data::<Events>()?.send(BoardEvent::TaskUpdated(row.clone()));
        Ok(row)
    }


    async fn move_task(&self, ctx: &Context<'_>, id: Uuid, status: TaskStatus) -> Result<Task> {
        let pool = ctx.data::<PgPool>()?;
        let events = ctx.data::<Events>()?;

        let task = tasks::transition(pool, id, status).await?;
        let _ = events.send(BoardEvent::TaskUpdated(task.clone()));

        // Moving a card into In Progress starts an agent on it
        if task.status == TaskStatus::InProgress {
            agent::spawn(pool.clone(), events.clone(), task.clone());
        }
        Ok(task)
    }
}

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Fires whenever a task in this project is created or changes status
    async fn task_updated(
        &self,
        ctx: &Context<'_>,
        project_id: Uuid,
    ) -> Result<impl Stream<Item = Task>> {
        let rx = ctx.data::<Events>()?.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |event| match event {
            Ok(BoardEvent::TaskUpdated(task)) if task.project_id == project_id => Some(task),
            _ => None,
        }))
    }

    /// Live agent output for one task
    async fn task_logs(
        &self,
        ctx: &Context<'_>,
        task_id: Uuid,
    ) -> Result<impl Stream<Item = LogLine>> {
        let rx = ctx.data::<Events>()?.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |event| match event {
            Ok(BoardEvent::Log { task_id: id, line }) if id == task_id => Some(line),
            _ => None,
        }))
    }
}
