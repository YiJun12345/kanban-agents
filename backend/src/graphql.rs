use async_graphql::{ComplexObject, Context, Object, Result, Schema, Subscription};
use sqlx::PgPool;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

use crate::{agent, git};
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

        /// What the agent changed on its branch (empty until it has run)
    async fn diff(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let Some(branch) = &self.branch else {
            return Ok(None);
        };
        let pool = ctx.data::<PgPool>()?;
        let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
            .bind(self.project_id)
            .fetch_one(pool)
            .await?;
        Ok(Some(git::diff(&project.repo_path, &project.default_branch, branch).await?))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testing::temp_repo;
    use serde_json::{json, Value};
    use std::time::Duration;
    use tokio::sync::broadcast;

    fn schema(pool: PgPool) -> AppSchema {
        let (events, _) = broadcast::channel(256);
        build_schema(pool, events)
    }

    /// Run a GraphQL request; panics on errors so the test shows them
    async fn exec(schema: &AppSchema, query: &str, vars: Value) -> Value {
        let req = async_graphql::Request::new(query)
            .variables(async_graphql::Variables::from_json(vars));
        let res = schema.execute(req).await;
        assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);
        res.data.into_json().unwrap()
    }

    async fn create_project(schema: &AppSchema, repo_path: &str) -> String {
        let data = exec(
            schema,
            "mutation($repo: String!) { createProject(name: \"Test\", repoPath: $repo) { id } }",
            json!({ "repo": repo_path }),
        )
        .await;
        data["createProject"]["id"].as_str().unwrap().to_string()
    }

    async fn create_task(schema: &AppSchema, project_id: &str, prompt: &str) -> String {
        let data = exec(
            schema,
            "mutation($p: UUID!, $prompt: String!) {
                createTask(projectId: $p, title: \"T\", prompt: $prompt) { id }
            }",
            json!({ "p": project_id, "prompt": prompt }),
        )
        .await;
        data["createTask"]["id"].as_str().unwrap().to_string()
    }

    async fn move_task(schema: &AppSchema, id: &str, status: &str) {
        exec(
            schema,
            "mutation($id: UUID!, $s: TaskStatus!) { moveTask(id: $id, status: $s) { id } }",
            json!({ "id": id, "s": status }),
        )
        .await;
    }

    /// Wait for the background agent to finish, then return the task with its logs and diff
    async fn wait_for_agent(schema: &AppSchema, id: &str) -> Value {
        for _ in 0..100 {
            let data = exec(
                schema,
                "query($id: UUID!) { task(id: $id) { status branch diff logs { content } } }",
                json!({ "id": id }),
            )
            .await;
            if data["task"]["status"] != "IN_PROGRESS" {
                return data["task"].clone();
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        panic!("agent did not finish within 20s");
    }

    fn log_text(task: &Value) -> String {
        let lines = task["logs"].as_array().unwrap();
        lines.iter().map(|l| l["content"].as_str().unwrap()).collect::<Vec<_>>().join("\n")
    }

    // #[sqlx::test] gives each test its own fresh database with the migrations applied,
    // and deletes it afterwards, so tests never see your real data (or each other's)

    #[sqlx::test]
    async fn new_task_starts_in_todo(pool: PgPool) {
        let schema = schema(pool);
        let project = create_project(&schema, "/tmp/unused").await;
        create_task(&schema, &project, "do something").await;

        let data = exec(
            &schema,
            "query($id: UUID!) { project(id: $id) { tasks { status agent branch diff } } }",
            json!({ "id": project }),
        )
        .await;
        assert_eq!(
            data["project"]["tasks"],
            json!([{ "status": "TODO", "agent": "FAKE", "branch": null, "diff": null }])
        );
    }

    #[sqlx::test]
    async fn illegal_move_is_rejected(pool: PgPool) {
        let schema = schema(pool);
        let project = create_project(&schema, "/tmp/unused").await;
        let task = create_task(&schema, &project, "x").await;

        let res = schema
            .execute(
                async_graphql::Request::new(
                    "mutation($id: UUID!) { moveTask(id: $id, status: DONE) { id } }",
                )
                .variables(async_graphql::Variables::from_json(json!({ "id": task }))),
            )
            .await;
        assert_eq!(res.errors[0].message, "cannot move task from Todo to Done");
    }

    #[sqlx::test]
    async fn agent_success_goes_to_review_with_diff(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let project = create_project(&schema, &repo).await;
        let task = create_task(&schema, &project, "add a greeting").await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        let done = wait_for_agent(&schema, &task).await;

        assert_eq!(done["status"], "IN_REVIEW");
        assert!(done["branch"].as_str().unwrap().starts_with("kanban/task-"));
        let logs = log_text(&done);
        assert!(logs.contains("working in"), "{logs}");
        assert!(logs.ends_with("committed changes"), "{logs}");
        let diff = done["diff"].as_str().unwrap();
        assert!(diff.contains("+- add a greeting"), "{diff}");

        // Re-run: same worktree, a second change on top of the first
        move_task(&schema, &task, "IN_PROGRESS").await;
        let again = wait_for_agent(&schema, &task).await;
        assert_eq!(again["branch"], done["branch"]);
        assert_eq!(again["diff"].as_str().unwrap().matches("+- add a greeting").count(), 2);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn agent_failure_goes_to_failed(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let project = create_project(&schema, &repo).await;
        let task = create_task(&schema, &project, "this will fail").await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        let done = wait_for_agent(&schema, &task).await;

        assert_eq!(done["status"], "FAILED");
        assert!(log_text(&done).contains("something went wrong"));
        assert_eq!(done["diff"], "", "a failed run commits nothing");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn bad_repo_path_goes_to_failed_with_error(pool: PgPool) {
        let schema = schema(pool);
        let project = create_project(&schema, "/nonexistent/repo").await;
        let task = create_task(&schema, &project, "anything").await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        let done = wait_for_agent(&schema, &task).await;

        assert_eq!(done["status"], "FAILED");
        let logs = log_text(&done);
        assert!(logs.starts_with("error: git worktree add"), "{logs}");
    }

    #[sqlx::test]
    async fn subscription_streams_agent_logs(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let project = create_project(&schema, &repo).await;
        let task = create_task(&schema, &project, "hello").await;

        // Subscribe first, like the browser does, then start the agent
        let mut stream = schema.execute_stream(
            async_graphql::Request::new(
                "subscription($id: UUID!) { taskLogs(taskId: $id) { seq content } }",
            )
            .variables(async_graphql::Variables::from_json(json!({ "id": task }))),
        );
        move_task(&schema, &task, "IN_PROGRESS").await;

        let mut seen = Vec::new();
        while seen.last().map(String::as_str) != Some("committed changes") {
            let res = tokio::time::timeout(Duration::from_secs(10), stream.next())
                .await
                .expect("no log line within 10s")
                .unwrap();
            let line = res.data.into_json().unwrap();
            seen.push(line["taskLogs"]["content"].as_str().unwrap().to_string());
        }
        assert!(seen[0].starts_with("working in"));
        assert!(seen.contains(&"working... step 3/3".to_string()));

        wait_for_agent(&schema, &task).await; // let it finish before the test DB is dropped
        std::fs::remove_dir_all(root).unwrap();
    }
}
