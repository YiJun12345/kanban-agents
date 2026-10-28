use async_graphql::{ComplexObject, Context, EmptySubscription, Object, Result, Schema};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{AgentKind, Project, Task, TaskStatus};

pub type AppSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn build_schema(pool: PgPool) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(pool)
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
        Ok(row)
    }

    async fn move_task(&self, ctx: &Context<'_>, id: Uuid, status: TaskStatus) -> Result<Task> {
        let pool = ctx.data::<PgPool>()?;
        let mut tx = pool.begin().await?;

        // Lock the row so two requests can't move the same task at once
        let current: TaskStatus =
            sqlx::query_scalar("SELECT status FROM tasks WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or("task not found")?;

        if !current.can_move_to(status) {
            return Err(format!("cannot move task from {current:?} to {status:?}").into());
        }

        let task = sqlx::query_as::<_, Task>(
            "UPDATE tasks SET status = $2, updated_at = now() WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(status)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(task)
    }
}
