use std::sync::Arc;

use async_graphql::{ComplexObject, Context, Object, Result, Schema, SimpleObject, Subscription};
use sqlx::PgPool;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

use crate::agent::Agents;
use crate::domain::{AgentKind, JournalActor, JournalEntry, JournalKind, LogLine, Project, Task, TaskStatus};
use crate::events::{BoardEvent, Events};
use crate::journal::{self, NewEntry};
use crate::{git, review, tasks};

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema(pool: PgPool, events: Events, agents: Arc<Agents>) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(events)
        .data(agents)
        .finish()
}

/// An agent you can give a task to, and whether it can run on this machine
#[derive(SimpleObject)]
pub struct AgentOption {
    kind: AgentKind,
    label: String,
    ready: bool,
    /// Why it is not ready (empty when it is)
    reason: Option<String>,
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

    /// The agents a task can be assigned to
    async fn agents(&self, ctx: &Context<'_>) -> Result<Vec<AgentOption>> {
        let claude = ctx.data::<Arc<Agents>>()?.claude.problem();
        Ok(vec![
            AgentOption { kind: AgentKind::Fake, label: "Fake agent".into(), ready: true, reason: None },
            AgentOption {
                kind: AgentKind::Claude,
                label: "Claude Code".into(),
                ready: claude.is_none(),
                reason: claude,
            },
            AgentOption {
                kind: AgentKind::Codex,
                label: "Codex".into(),
                ready: false,
                reason: Some("not supported yet".into()),
            },
        ])
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

    /// Latest journal entries across all tasks in this project, newest first
    async fn journal(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 50)] limit: i64,
    ) -> Result<Vec<JournalEntry>> {
        let pool = ctx.data::<PgPool>()?;
        let rows = sqlx::query_as::<_, JournalEntry>(
            "SELECT j.* FROM journal_entries j JOIN tasks t ON t.id = j.task_id
             WHERE t.project_id = $1 ORDER BY j.seq DESC LIMIT $2",
        )
        .bind(self.id)
        .bind(limit)
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

    /// What the agent changed: its branch compared with main, or, once the task is
    /// approved, what the merge brought in. Empty until the agent has run.
    async fn diff(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let pool = ctx.data::<PgPool>()?;
        let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = $1")
            .bind(self.project_id)
            .fetch_one(pool)
            .await?;
        let repo = &project.repo_path;
        match (&self.merge_commit, &self.branch, &self.worktree_path) {
            (Some(commit), _, _) => Ok(Some(git::diff_of_merge(repo, commit).await?)),
            // The worktree is only missing after an approval that had nothing to merge
            (None, Some(branch), Some(_)) => {
                Ok(Some(git::diff(repo, &project.default_branch, branch).await?))
            }
            _ => Ok(None),
        }
    }

    /// Everything that happened to this task, oldest first
    async fn journal(&self, ctx: &Context<'_>) -> Result<Vec<JournalEntry>> {
        let pool = ctx.data::<PgPool>()?;
        let rows = sqlx::query_as::<_, JournalEntry>(
            "SELECT * FROM journal_entries WHERE task_id = $1 ORDER BY seq",
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
    /// The one journal line shown on the card: the newest note, summary or error
    async fn headline(&self, ctx: &Context<'_>) -> Result<Option<JournalEntry>> {
        let pool = ctx.data::<PgPool>()?;
        let row = sqlx::query_as::<_, JournalEntry>(
            "SELECT * FROM journal_entries
                 WHERE task_id = $1 AND kind IN ('note', 'summary', 'error')
                 ORDER BY seq DESC LIMIT 1",
        )
        .bind(self.id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
    

}

#[ComplexObject]
impl JournalEntry {
    /// Extra facts as JSON, e.g. {"commit": "a1b2c3d", "files": ["README.md"]}
    async fn details(&self) -> async_graphql::Json<serde_json::Value> {
        async_graphql::Json(self.details.0.clone())
    }

    /// The task this entry belongs to (handy in the project-wide journal)
    async fn task(&self, ctx: &Context<'_>) -> Result<Task> {
        let pool = ctx.data::<PgPool>()?;
        let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
            .bind(self.task_id)
            .fetch_one(pool)
            .await?;
        Ok(task)
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// A project is a git repository on this machine. Its checked-out branch becomes the
    /// branch that agents start from and that approved work is merged into.
    async fn create_project(
        &self,
        ctx: &Context<'_>,
        name: String,
        repo_path: String,
    ) -> Result<Project> {
        let name = name.trim();
        let repo_path = repo_path.trim().trim_end_matches('/');
        if name.is_empty() {
            return Err("the project needs a name".into());
        }
        let Ok(branch) = git::current_branch(repo_path).await else {
            return Err(format!("{repo_path} is not a git repository with a branch checked out").into());
        };

        let pool = ctx.data::<PgPool>()?;
        let row = sqlx::query_as::<_, Project>(
            "INSERT INTO projects (name, repo_path, default_branch) VALUES ($1, $2, $3) RETURNING *",
        )
        .bind(name)
        .bind(repo_path)
        .bind(branch)
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


    /// Move a card. Some moves do work:
    /// into In Progress starts the agent, back to To Do stops it, into Done merges its branch.
    async fn move_task(&self, ctx: &Context<'_>, id: Uuid, status: TaskStatus) -> Result<Task> {
        let pool = ctx.data::<PgPool>()?;
        let events = ctx.data::<Events>()?;
        let agents = ctx.data::<Arc<Agents>>()?;

        // Done is special: the move only happens if the merge works
        if status == TaskStatus::Done {
            return Ok(review::approve(pool, events, id).await?);
        }

        let (task, entry) = tasks::transition(pool, id, status, JournalActor::Human).await?;
        let _ = events.send(BoardEvent::TaskUpdated(task.clone()));
        let _ = events.send(BoardEvent::Journal(entry));

        match task.status {
            TaskStatus::InProgress => agents.start(pool.clone(), events.clone(), task.clone(), None),
            TaskStatus::Todo => {
                agents.stop(id); // does nothing if no agent is running
            }
            _ => {}
        }
        Ok(task)
    }

    /// Accept the agent's work: merge its branch, clean up, move the card to Done
    async fn approve_task(&self, ctx: &Context<'_>, id: Uuid) -> Result<Task> {
        let pool = ctx.data::<PgPool>()?;
        Ok(review::approve(pool, ctx.data::<Events>()?, id).await?)
    }

    /// Send the task back to the agent with feedback; it runs again and reads the feedback
    async fn request_changes(&self, ctx: &Context<'_>, id: Uuid, feedback: String) -> Result<Task> {
        let pool = ctx.data::<PgPool>()?;
        let events = ctx.data::<Events>()?;
        let agents = ctx.data::<Arc<Agents>>()?;
        Ok(review::request_changes(pool, events, agents, id, &feedback).await?)
    }

    /// A person adds a note to a task's journal (e.g. review feedback)
    async fn add_journal_note(
        &self,
        ctx: &Context<'_>,
        task_id: Uuid,
        message: String,
    ) -> Result<JournalEntry> {
        let message = message.trim();
        if message.is_empty() {
            return Err("note cannot be empty".into());
        }
        let pool = ctx.data::<PgPool>()?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tasks WHERE id = $1)")
            .bind(task_id)
            .fetch_one(pool)
            .await?;
        if !exists {
            return Err("task not found".into());
        }

        let entry = NewEntry::new(task_id, JournalActor::Human, JournalKind::Note, message);
        Ok(journal::record(pool, ctx.data::<Events>()?, entry).await?)
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

    /// New journal entries for one task, as they are written
    async fn journal_added(
        &self,
        ctx: &Context<'_>,
        task_id: Uuid,
    ) -> Result<impl Stream<Item = JournalEntry>> {
        let rx = ctx.data::<Events>()?.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |event| match event {
            Ok(BoardEvent::Journal(entry)) if entry.task_id == task_id => Some(entry),
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
    use crate::claude::ClaudeConfig;
    use crate::git::testing::temp_repo;
    use serde_json::{json, Value};
    use std::path::Path;
    use std::time::Duration;
    use tokio::sync::broadcast;

    /// Claude settings for tests: no key, so the Claude agent is "off"
    fn no_claude() -> ClaudeConfig {
        ClaudeConfig {
            bin: "claude".into(),
            api_key: None,
            config_dir: "/tmp/unused".into(),
            model: None,
            max_budget_usd: "1.00".into(),
        }
    }

    fn schema(pool: PgPool) -> AppSchema {
        schema_with(pool, no_claude())
    }

    fn schema_with(pool: PgPool, claude: ClaudeConfig) -> AppSchema {
        let (events, _) = broadcast::channel(256);
        build_schema(pool, events, Agents::new(claude))
    }

    /// Run a GraphQL request; panics on errors so the test shows them
    async fn exec(schema: &AppSchema, query: &str, vars: Value) -> Value {
        let req = async_graphql::Request::new(query)
            .variables(async_graphql::Variables::from_json(vars));
        let res = schema.execute(req).await;
        assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);
        res.data.into_json().unwrap()
    }

    /// Run a GraphQL request that should fail, and return its error message
    async fn exec_err(schema: &AppSchema, query: &str, vars: Value) -> String {
        let req = async_graphql::Request::new(query)
            .variables(async_graphql::Variables::from_json(vars));
        let res = schema.execute(req).await;
        assert!(!res.errors.is_empty(), "expected an error, got {:?}", res.data);
        res.errors[0].message.clone()
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

    /// A project whose repo does not exist. createProject would refuse it, so insert it
    /// directly: it is what you get when a repo is moved or deleted later.
    async fn project_with_missing_repo(pool: &PgPool) -> String {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO projects (name, repo_path) VALUES ('Test', '/nonexistent/repo') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        id.to_string()
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

    async fn get_task(schema: &AppSchema, id: &str) -> Value {
        let data = exec(
            schema,
            "query($id: UUID!) {
                task(id: $id) { status branch worktreePath mergeCommit diff logs { content } }
            }",
            json!({ "id": id }),
        )
        .await;
        data["task"].clone()
    }

    /// Wait for the background agent to finish, then return the task with its logs and diff
    async fn wait_for_agent(schema: &AppSchema, id: &str) -> Value {
        for _ in 0..100 {
            let task = get_task(schema, id).await;
            if task["status"] != "IN_PROGRESS" {
                return task;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        panic!("agent did not finish within 20s");
    }

    fn log_text(task: &Value) -> String {
        let lines = task["logs"].as_array().unwrap();
        lines.iter().map(|l| l["content"].as_str().unwrap()).collect::<Vec<_>>().join("\n")
    }

    /// The task's journal as (actor, kind, message)
    async fn journal(schema: &AppSchema, task: &str) -> Vec<(String, String, String)> {
        let data = exec(
            schema,
            "query($id: UUID!) { task(id: $id) { journal { actor kind message } } }",
            json!({ "id": task }),
        )
        .await;
        data["task"]["journal"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                let s = |k: &str| e[k].as_str().unwrap().to_string();
                (s("actor"), s("kind"), s("message"))
            })
            .collect()
    }

    async fn git_out(dir: &str, args: &[&str]) -> String {
        let out = tokio::process::Command::new("git").arg("-C").arg(dir).args(args).output().await.unwrap();
        String::from_utf8(out.stdout).unwrap()
    }

    // #[sqlx::test] gives each test its own fresh database with the migrations applied,
    // and deletes it afterwards, so tests never see your real data (or each other's)

    // ---------- board basics ----------

    #[sqlx::test]
    async fn new_task_starts_in_todo(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool);
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
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool);
        let task = create_task(&schema, &project, "x").await;

        let err = exec_err(
            &schema,
            "mutation($id: UUID!) { moveTask(id: $id, status: DONE) { id } }",
            json!({ "id": task }),
        )
        .await;
        assert_eq!(err, "cannot move task from Todo to Done");
    }

    #[sqlx::test]
    async fn create_project_checks_the_repo(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);

        let err = exec_err(
            &schema,
            "mutation { createProject(name: \"X\", repoPath: \"/nonexistent/repo\") { id } }",
            json!({}),
        )
        .await;
        assert_eq!(err, "/nonexistent/repo is not a git repository with a branch checked out");

        // The repo's own branch becomes the project's default branch
        git_out(&repo, &["checkout", "-b", "trunk"]).await;
        let data = exec(
            &schema,
            "mutation($repo: String!) {
                createProject(name: \"  X  \", repoPath: $repo) { name repoPath defaultBranch }
            }",
            json!({ "repo": format!("{repo}/") }),
        )
        .await;
        assert_eq!(
            data["createProject"],
            json!({ "name": "X", "repoPath": repo, "defaultBranch": "trunk" })
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    // ---------- the agent ----------

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

        // A failed task can be retried straight away; it is run #2
        move_task(&schema, &task, "IN_PROGRESS").await;
        assert_eq!(wait_for_agent(&schema, &task).await["status"], "FAILED");
        let entries = journal(&schema, &task).await;
        assert!(entries.iter().any(|(_, _, m)| m.starts_with("Run #2 started")), "{entries:?}");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn bad_repo_path_goes_to_failed_with_error(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool);
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

    #[sqlx::test]
    async fn moving_back_to_todo_stops_the_agent(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let project = create_project(&schema, &repo).await;
        let task = create_task(&schema, &project, "add a greeting").await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        tokio::time::sleep(Duration::from_millis(800)).await; // let it get going
        move_task(&schema, &task, "TODO").await;

        // The run ends by itself, well before the fake agent's 3 seconds are over
        let mut entries = Vec::new();
        for _ in 0..20 {
            entries = journal(&schema, &task).await;
            if entries.iter().any(|(_, kind, _)| kind == "RUN_FINISHED") {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let finished = entries.iter().find(|(_, kind, _)| kind == "RUN_FINISHED").expect("run finished");
        assert!(finished.2.starts_with("Run stopped by you after"), "{}", finished.2);
        assert!(!entries.iter().any(|(_, kind, _)| kind == "COMMIT"), "nothing is committed");

        // The card stays where the person put it
        tokio::time::sleep(Duration::from_millis(300)).await;
        let task = get_task(&schema, &task).await;
        assert_eq!(task["status"], "TODO");
        assert!(log_text(&task).ends_with("stopped"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn stuck_tasks_are_failed_after_a_restart(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool.clone());
        let task = create_task(&schema, &project, "x").await;
        // As if the backend had been killed while the agent was working
        sqlx::query("UPDATE tasks SET status = 'in_progress'").execute(&pool).await.unwrap();

        crate::agent::recover(&pool).await.unwrap();

        assert_eq!(get_task(&schema, &task).await["status"], "FAILED");
        let messages: Vec<_> = journal(&schema, &task).await.into_iter().map(|e| e.2).collect();
        assert_eq!(
            messages,
            ["The backend restarted while the agent was running", "Moved from In Progress to Failed"]
        );
    }

    // ---------- the journal ----------

    #[sqlx::test]
    async fn journal_records_a_full_run(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let project = create_project(&schema, &repo).await;
        let task = create_task(&schema, &project, "add a greeting").await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        wait_for_agent(&schema, &task).await;

        let entries = journal(&schema, &task).await;
        let kinds: Vec<_> = entries.iter().map(|(actor, kind, _)| format!("{actor} {kind}")).collect();
        assert_eq!(
            kinds,
            [
                "HUMAN STATUS",        // you moved it to In Progress
                "SYSTEM RUN_STARTED",  // run #1 on its branch
                "AGENT NOTE",          // "Plan: ..."
                "AGENT NOTE",          // "Wrote the request to AGENT_NOTES.md"
                "AGENT SUMMARY",       // the agent's final report
                "SYSTEM COMMIT",       // hash + files
                "SYSTEM RUN_FINISHED", // duration
                "SYSTEM STATUS",       // moved to In Review
            ]
        );
        assert_eq!(entries[0].2, "Moved from To Do to In Progress");
        assert!(entries[1].2.starts_with("Run #1 started on branch kanban/task-"));
        assert_eq!(entries[2].2, "Plan: add a greeting");
        assert_eq!(entries[4].2, "Added \"add a greeting\" to AGENT_NOTES.md");
        assert!(entries[5].2.ends_with(": 1 file changed"), "{}", entries[5].2);
        assert_eq!(entries[7].2, "Moved from In Progress to In Review");

        // The commit's details list the files, and the commit message carries the agent's notes
        let data = exec(
            &schema,
            "query($id: UUID!) { task(id: $id) { worktreePath journal { kind details } } }",
            json!({ "id": task }),
        )
        .await;
        assert_eq!(data["task"]["journal"][5]["details"]["files"], json!(["AGENT_NOTES.md"]));
        let worktree = data["task"]["worktreePath"].as_str().unwrap();
        let log = git_out(worktree, &["log", "-1", "--format=%B"]).await;
        assert!(log.contains("Added \"add a greeting\" to AGENT_NOTES.md"), "{log}");
        assert!(log.contains("- Plan: add a greeting"), "{log}");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn rerun_agent_reads_earlier_journal(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let project = create_project(&schema, &repo).await;
        let task = create_task(&schema, &project, "add a greeting").await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        wait_for_agent(&schema, &task).await;
        let before = journal(&schema, &task).await.len() + 1; // + the move back to In Progress
        move_task(&schema, &task, "IN_PROGRESS").await;
        wait_for_agent(&schema, &task).await;

        let notes: Vec<_> = journal(&schema, &task)
            .await
            .into_iter()
            .filter(|(actor, _, _)| actor == "AGENT")
            .map(|(_, _, message)| message)
            .collect();
        assert!(
            notes.contains(&format!("Read {before} earlier journal entries before starting")),
            "{notes:?}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn failed_run_is_journaled(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool);
        let task = create_task(&schema, &project, "anything").await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        wait_for_agent(&schema, &task).await;

        let entries = journal(&schema, &task).await;
        let error = entries.iter().find(|(_, kind, _)| kind == "ERROR").expect("an ERROR entry");
        assert!(error.2.starts_with("git worktree add"), "{}", error.2);
        assert_eq!(entries.last().unwrap().2, "Moved from In Progress to Failed");
    }

    #[sqlx::test]
    async fn people_can_add_notes(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool);
        let task = create_task(&schema, &project, "x").await;

        exec(
            &schema,
            "mutation($id: UUID!) { addJournalNote(taskId: $id, message: \"  Looks good  \") { id } }",
            json!({ "id": task }),
        )
        .await;
        assert_eq!(
            journal(&schema, &task).await,
            [("HUMAN".into(), "NOTE".into(), "Looks good".into())]
        );

        // Empty notes are rejected
        let err = exec_err(
            &schema,
            "mutation($id: UUID!) { addJournalNote(taskId: $id, message: \"  \") { id } }",
            json!({ "id": task }),
        )
        .await;
        assert_eq!(err, "note cannot be empty");
    }

    async fn headline(schema: &AppSchema, task: &str) -> Value {
        let data = exec(
            schema,
            "query($id: UUID!) { task(id: $id) { headline { actor kind message } } }",
            json!({ "id": task }),
        )
        .await;
        data["task"]["headline"].clone()
    }

    #[sqlx::test]
    async fn card_headline_is_the_newest_note_or_summary(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let project = create_project(&schema, &repo).await;
        let task = create_task(&schema, &project, "add a greeting").await;
        assert_eq!(headline(&schema, &task).await, Value::Null, "nothing to show yet");

        // Status moves alone are not a headline: the column already shows the status
        move_task(&schema, &task, "IN_PROGRESS").await;
        wait_for_agent(&schema, &task).await;
        assert_eq!(
            headline(&schema, &task).await,
            json!({ "actor": "AGENT", "kind": "SUMMARY",
                    "message": "Added \"add a greeting\" to AGENT_NOTES.md" })
        );

        // A person's note is newer, so it becomes the headline
        exec(
            &schema,
            "mutation($id: UUID!) { addJournalNote(taskId: $id, message: \"Ship it\") { id } }",
            json!({ "id": task }),
        )
        .await;
        assert_eq!(headline(&schema, &task).await["message"], "Ship it");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn notes_update_the_board_live(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool);
        let task = create_task(&schema, &project, "x").await;

        // The board listens to taskUpdated; a new note must refresh the card's headline
        let mut stream = schema.execute_stream(
            async_graphql::Request::new(
                "subscription($p: UUID!) { taskUpdated(projectId: $p) { id headline { message } } }",
            )
            .variables(async_graphql::Variables::from_json(json!({ "p": project }))),
        );
        // A subscription starts listening when it is first polled, so wait for the event
        // and add the note at the same time (join! polls the first future first)
        let add_note = async {
            exec(
                &schema,
                "mutation($id: UUID!) { addJournalNote(taskId: $id, message: \"Hello\") { id } }",
                json!({ "id": task }),
            )
            .await
        };
        let (event, _) =
            tokio::join!(tokio::time::timeout(Duration::from_secs(5), stream.next()), add_note);
        let res = event.expect("no taskUpdated event within 5s").unwrap();
        let data = res.data.into_json().unwrap();
        assert_eq!(data["taskUpdated"], json!({ "id": task, "headline": { "message": "Hello" } }));
    }

    #[sqlx::test]
    async fn journal_is_append_only(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool.clone());
        let task = create_task(&schema, &project, "x").await;
        move_task(&schema, &task, "IN_PROGRESS").await; // writes entries (the agent fails fast)
        wait_for_agent(&schema, &task).await;

        let edit = sqlx::query("UPDATE journal_entries SET message = 'rewritten history'")
            .execute(&pool)
            .await;
        assert!(edit.unwrap_err().to_string().contains("append-only"));

        let delete = sqlx::query("DELETE FROM journal_entries").execute(&pool).await;
        assert!(delete.unwrap_err().to_string().contains("append-only"));

        // Deleting the whole task still works: its journal goes with it
        sqlx::query("DELETE FROM tasks WHERE id = $1::uuid")
            .bind(&task)
            .execute(&pool)
            .await
            .unwrap();
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM journal_entries")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(left, 0);
    }

    // ---------- review ----------

    /// A task whose fake agent has finished, waiting in In Review
    async fn task_in_review(schema: &AppSchema, repo: &str) -> String {
        let project = create_project(schema, repo).await;
        let task = create_task(schema, &project, "add a greeting").await;
        move_task(schema, &task, "IN_PROGRESS").await;
        assert_eq!(wait_for_agent(schema, &task).await["status"], "IN_REVIEW");
        task
    }

    #[sqlx::test]
    async fn approving_merges_the_branch_and_cleans_up(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let task = task_in_review(&schema, &repo).await;
        let before = get_task(&schema, &task).await;
        let worktree = before["worktreePath"].as_str().unwrap();
        assert!(!Path::new(&repo).join("AGENT_NOTES.md").exists(), "main is untouched before approval");

        let data = exec(
            &schema,
            "mutation($id: UUID!) { approveTask(id: $id) { status mergeCommit } }",
            json!({ "id": task }),
        )
        .await;
        assert_eq!(data["approveTask"]["status"], "DONE");
        let commit = data["approveTask"]["mergeCommit"].as_str().unwrap();

        // main has the agent's work, in a merge commit that tells the task's story
        assert!(Path::new(&repo).join("AGENT_NOTES.md").exists());
        let message = git_out(&repo, &["log", "-1", "--format=%h%n%B"]).await;
        assert!(message.starts_with(&format!("{commit}\nMerge task: T")), "{message}");
        assert!(message.contains("Added \"add a greeting\" to AGENT_NOTES.md"), "{message}");
        assert!(message.contains("[Agent] Plan: add a greeting"), "{message}");

        // The worktree and the branch are gone, but the diff can still be read
        assert!(!Path::new(worktree).exists());
        assert_eq!(git_out(&repo, &["branch", "--list", "kanban/*"]).await, "");
        let after = get_task(&schema, &task).await;
        assert_eq!(after["worktreePath"], Value::Null);
        assert!(after["diff"].as_str().unwrap().contains("+- add a greeting"));

        let entries = journal(&schema, &task).await;
        let last_two: Vec<_> = entries[entries.len() - 2..].iter().map(|e| e.2.as_str()).collect();
        assert_eq!(
            last_two,
            [
                format!("Merged {} into main as {commit}", before["branch"].as_str().unwrap()).as_str(),
                "Moved from In Review to Done"
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn dragging_to_done_is_an_approval(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let task = task_in_review(&schema, &repo).await;

        move_task(&schema, &task, "DONE").await;

        let task = get_task(&schema, &task).await;
        assert_eq!(task["status"], "DONE");
        assert!(task["mergeCommit"].is_string());
        assert!(Path::new(&repo).join("AGENT_NOTES.md").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn merge_conflict_keeps_the_task_in_review(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let task = task_in_review(&schema, &repo).await;

        // Meanwhile, someone adds a different AGENT_NOTES.md on main
        std::fs::write(format!("{repo}/AGENT_NOTES.md"), "written by a person\n").unwrap();
        crate::git::commit_all(&repo, "person").await.unwrap();

        let err = exec_err(
            &schema,
            "mutation($id: UUID!) { approveTask(id: $id) { id } }",
            json!({ "id": task }),
        )
        .await;
        assert!(err.starts_with("could not merge kanban/task-"), "{err}");

        let after = get_task(&schema, &task).await;
        assert_eq!(after["status"], "IN_REVIEW");
        assert_eq!(after["mergeCommit"], Value::Null);
        assert!(Path::new(after["worktreePath"].as_str().unwrap()).exists(), "worktree is kept");
        assert_eq!(git_out(&repo, &["status", "--porcelain"]).await, "", "no half-finished merge");
        let last = journal(&schema, &task).await.pop().unwrap();
        assert_eq!((last.0.as_str(), last.1.as_str()), ("SYSTEM", "ERROR"));
        assert!(last.2.starts_with("Merge failed: could not merge"), "{}", last.2);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn requesting_changes_reruns_the_agent_with_the_feedback(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool);
        let task = task_in_review(&schema, &repo).await;

        let data = exec(
            &schema,
            "mutation($id: UUID!) {
                requestChanges(id: $id, feedback: \"  make it friendlier  \") { status }
            }",
            json!({ "id": task }),
        )
        .await;
        assert_eq!(data["requestChanges"]["status"], "IN_PROGRESS");
        let done = wait_for_agent(&schema, &task).await;
        assert_eq!(done["status"], "IN_REVIEW");

        // The feedback is on record, the agent says it read it, and the work reflects it
        let entries = journal(&schema, &task).await;
        let has = |actor: &str, kind: &str, message: &str| {
            entries.contains(&(actor.to_string(), kind.to_string(), message.to_string()))
        };
        assert!(has("HUMAN", "NOTE", "make it friendlier"), "{entries:?}");
        assert!(has("HUMAN", "STATUS", "Moved from In Review to In Progress"));
        assert!(has("AGENT", "NOTE", "Addressing review feedback: make it friendlier"));
        assert!(done["diff"].as_str().unwrap().contains("+  - after review: make it friendlier"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn requesting_changes_needs_feedback_and_a_task_in_review(pool: PgPool) {
        let project = project_with_missing_repo(&pool).await;
        let schema = schema(pool);
        let task = create_task(&schema, &project, "x").await;
        let request = "mutation($id: UUID!, $f: String!) { requestChanges(id: $id, feedback: $f) { id } }";

        let err = exec_err(&schema, request, json!({ "id": task, "f": "  " })).await;
        assert_eq!(err, "say what should change");
        let err = exec_err(&schema, request, json!({ "id": task, "f": "more tests" })).await;
        assert_eq!(err, "changes can only be requested for a task in review");
        assert!(journal(&schema, &task).await.is_empty(), "a refused request leaves no note");
    }

    // ---------- the Claude agent ----------

    /// A stand-in for the `claude` CLI. It checks it was started the way we promise
    /// (own API key, own config directory, headless flags), edits a file, and prints
    /// the same kind of JSON events the real CLI prints.
    const STUB_CLAUDE: &str = r##"#!/bin/sh
[ "$ANTHROPIC_API_KEY" = "test-key" ] || { echo "wrong api key" >&2; exit 9; }
[ "$CLAUDE_CONFIG_DIR" = "/tmp/kanban-test-claude-config" ] || { echo "wrong config dir" >&2; exit 9; }
[ "$1" = "-p" ] || { echo "not headless" >&2; exit 9; }
case "$2" in *"# Task: T"*"say hello"*) ;; *) echo "prompt has no task" >&2; exit 9 ;; esac
echo "hello" > hello.txt
echo '{"type":"system","subtype":"init","model":"claude-test"}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"JOURNAL: Plan: create hello.txt"},{"type":"tool_use","name":"Write","input":{"file_path":"hello.txt"}}]}}'
echo '{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"JOURNAL: SUMMARY: Created hello.txt"}]}}'
echo '{"type":"result","subtype":"success","is_error":false,"num_turns":2,"total_cost_usd":0.01,"result":"done"}'
"##;

    async fn create_claude_task(schema: &AppSchema, project: &str) -> String {
        let data = exec(
            schema,
            "mutation($p: UUID!) {
                createTask(projectId: $p, title: \"T\", prompt: \"say hello\", agent: CLAUDE) { id }
            }",
            json!({ "p": project }),
        )
        .await;
        data["createTask"]["id"].as_str().unwrap().to_string()
    }

    #[sqlx::test]
    async fn claude_agent_runs_headless_with_its_own_key(pool: PgPool) {
        use std::os::unix::fs::PermissionsExt;
        let (root, repo) = temp_repo().await;
        let stub = root.join("claude-stub");
        std::fs::write(&stub, STUB_CLAUDE).unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        let claude = ClaudeConfig {
            bin: stub.to_str().unwrap().into(),
            api_key: Some("test-key".into()),
            config_dir: "/tmp/kanban-test-claude-config".into(),
            ..no_claude()
        };
        let schema = schema_with(pool, claude);
        let project = create_project(&schema, &repo).await;
        let task = create_claude_task(&schema, &project).await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        let done = wait_for_agent(&schema, &task).await;

        let logs = log_text(&done);
        assert_eq!(done["status"], "IN_REVIEW", "{logs}");
        // The JSON events were turned into readable lines
        assert!(logs.contains("Claude Code session started (claude-test)"), "{logs}");
        assert!(logs.contains("⚙ Write hello.txt"), "{logs}");
        assert!(logs.contains("Claude finished: 2 turns, $0.0100"), "{logs}");
        assert!(!logs.contains("tool_result"), "{logs}");
        // Its JOURNAL lines went into the journal, and its edit was committed
        let entries = journal(&schema, &task).await;
        assert!(entries.contains(&("AGENT".into(), "NOTE".into(), "Plan: create hello.txt".into())));
        assert!(entries.contains(&("AGENT".into(), "SUMMARY".into(), "Created hello.txt".into())));
        assert!(done["diff"].as_str().unwrap().contains("+hello"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[sqlx::test]
    async fn claude_agent_without_a_key_fails_with_a_clear_error(pool: PgPool) {
        let (root, repo) = temp_repo().await;
        let schema = schema(pool); // no key
        let project = create_project(&schema, &repo).await;
        let task = create_claude_task(&schema, &project).await;

        move_task(&schema, &task, "IN_PROGRESS").await;
        assert_eq!(wait_for_agent(&schema, &task).await["status"], "FAILED");

        let entries = journal(&schema, &task).await;
        let error = entries.iter().find(|(_, kind, _)| kind == "ERROR").expect("an ERROR entry");
        assert_eq!(
            error.2,
            "the Claude agent cannot run: KANBAN_ANTHROPIC_API_KEY is not set in backend/.env"
        );

        // The board can tell people before they try
        let data = exec(&schema, "{ agents { kind ready reason } }", json!({})).await;
        assert_eq!(
            data["agents"],
            json!([
                { "kind": "FAKE", "ready": true, "reason": null },
                { "kind": "CLAUDE", "ready": false,
                  "reason": "KANBAN_ANTHROPIC_API_KEY is not set in backend/.env" },
                { "kind": "CODEX", "ready": false, "reason": "not supported yet" },
            ])
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
