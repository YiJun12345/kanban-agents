//! `kanban`: the command-line tool agents use to take work from the board and report back.
//!
//! Every command sends one GraphQL request to the backend and prints the answer as JSON,
//! so an agent (or a shell script) can read it. Settings come from two environment variables:
//!
//!   KANBAN_URL    where the board is (default http://localhost:3000/graphql)
//!   KANBAN_TOKEN  this worker's token, from `kanban register NAME`

use std::io::{BufRead, BufReader};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(name = "kanban", version, about = "Take work from the Kanban board and report back")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a worker and print its token (shown only this once)
    Register { name: String },
    /// Show which worker this token belongs to, and what it holds
    Whoami,
    /// List tasks
    Ls {
        /// Only tasks waiting for an outside worker to claim them
        #[arg(long)]
        waiting: bool,
    },
    /// Show one task with its journal
    Show { task: String },
    /// Claim the task that has waited longest; prints `null` when nothing is waiting
    Next {
        #[arg(long)]
        project: Option<String>,
    },
    /// Claim a task by id
    Claim { task: String },
    /// Heartbeat: "still working". Fails if the claim is gone, which means: stop
    Hb { task: String },
    /// Write a note in the task's journal
    Note {
        task: String,
        message: String,
        /// What the note is about
        #[arg(long, value_enum)]
        kind: Option<NoteKind>,
    },
    /// Record where the work stands, so another run could continue from here
    Checkpoint {
        task: String,
        #[arg(long)]
        summary: String,
        #[arg(long, default_value = "")]
        intent: String,
        #[arg(long, default_value = "")]
        next: String,
    },
    /// Add lines to the run's raw output (the given line, or everything on stdin)
    Log { task: String, line: Option<String> },
    /// Finished: the board commits the worktree and moves the task to In Review
    Submit {
        task: String,
        #[arg(long)]
        summary: Option<String>,
    },
    /// Could not do it: the task moves to Failed
    Fail {
        task: String,
        #[arg(long)]
        reason: String,
    },
    /// Give the task back unfinished
    Release { task: String },
    /// Claim the next task and run a command as its agent, handling everything else:
    /// heartbeats, passing its output to the board, then submit (exit 0) or fail.
    /// The command runs in the task's worktree with KANBAN_TASK_* variables set.
    Work {
        #[arg(long)]
        project: Option<String>,
        /// The agent command, after `--`
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum NoteKind {
    Plan,
    Progress,
    Blocker,
    Decision,
    Evidence,
}

impl NoteKind {
    /// The name GraphQL uses for this kind
    fn graphql(self) -> &'static str {
        match self {
            NoteKind::Plan => "PLAN",
            NoteKind::Progress => "PROGRESS",
            NoteKind::Blocker => "BLOCKER",
            NoteKind::Decision => "DECISION",
            NoteKind::Evidence => "EVIDENCE",
        }
    }
}

/// Send one GraphQL request and return its `data`, or the server's error message
fn call(query: &str, variables: Value) -> anyhow::Result<Value> {
    let url = std::env::var("KANBAN_URL").unwrap_or_else(|_| "http://localhost:3000/graphql".into());
    let mut request = ureq::post(&url);
    if let Ok(token) = std::env::var("KANBAN_TOKEN") {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let mut response = request
        .send_json(json!({ "query": query, "variables": variables }))
        .with_context(|| format!("could not reach the board at {url}"))?;
    let body: Value = response.body_mut().read_json().context("the board did not answer with JSON")?;

    if let Some(message) = body["errors"][0]["message"].as_str() {
        bail!("{message}");
    }
    Ok(body["data"].clone())
}

const ASSIGNMENT: &str = "runNumber branch worktreePath journal feedback task { id title prompt }";

fn run(cli: Cli) -> anyhow::Result<Value> {
    match cli.command {
        Cmd::Register { name } => {
            let data = call(
                "mutation($name: String!) { registerWorker(name: $name) { token worker { name } } }",
                json!({ "name": name }),
            )?;
            let token = data["registerWorker"]["token"].as_str().unwrap_or_default();
            eprintln!("Keep this token; it cannot be shown again. To use it:\n  export KANBAN_TOKEN={token}");
            Ok(json!({ "name": name, "token": token }))
        }
        Cmd::Whoami => {
            let data = call("{ me { name lastSeenAt claims { taskId quietSecs task { title } } } }", json!({}))?;
            Ok(data["me"].clone())
        }
        Cmd::Ls { waiting } => {
            let data = call(
                "{ projects { name tasks { id title status agent claim { worker { name } } } } }",
                json!({}),
            )?;
            let mut tasks = Vec::new();
            for project in data["projects"].as_array().into_iter().flatten() {
                for task in project["tasks"].as_array().into_iter().flatten() {
                    let is_waiting = task["status"] == "IN_PROGRESS"
                        && task["agent"] == "EXTERNAL"
                        && task["claim"].is_null();
                    if waiting && !is_waiting {
                        continue;
                    }
                    tasks.push(json!({
                        "id": task["id"],
                        "project": project["name"],
                        "title": task["title"],
                        "status": task["status"],
                        "agent": task["agent"],
                        "worker": task["claim"]["worker"]["name"],
                    }));
                }
            }
            Ok(json!(tasks))
        }
        Cmd::Show { task } => {
            let data = call(
                "query($id: UUID!) { task(id: $id) {
                    id title prompt status agent branch worktreePath
                    claim { worker { name } quietSecs }
                    journal { actor author kind noteKind message createdAt }
                } }",
                json!({ "id": task }),
            )?;
            if data["task"].is_null() {
                bail!("task not found");
            }
            Ok(data["task"].clone())
        }
        Cmd::Next { project } => {
            let data = call(
                &format!("mutation($p: UUID) {{ claimNextTask(projectId: $p) {{ {ASSIGNMENT} }} }}"),
                json!({ "p": project }),
            )?;
            Ok(data["claimNextTask"].clone())
        }
        Cmd::Claim { task } => {
            let data = call(
                &format!("mutation($id: UUID!) {{ claimTask(id: $id) {{ {ASSIGNMENT} }} }}"),
                json!({ "id": task }),
            )?;
            Ok(data["claimTask"].clone())
        }
        Cmd::Hb { task } => {
            let data = call(
                "mutation($id: UUID!) { heartbeat(taskId: $id) { taskId heartbeatAt } }",
                json!({ "id": task }),
            )?;
            Ok(data["heartbeat"].clone())
        }
        Cmd::Note { task, message, kind } => {
            let data = call(
                "mutation($id: UUID!, $kind: NoteKind, $message: String!) {
                    addWorkerNote(taskId: $id, kind: $kind, message: $message) { seq noteKind message }
                }",
                json!({ "id": task, "kind": kind.map(NoteKind::graphql), "message": message }),
            )?;
            Ok(data["addWorkerNote"].clone())
        }
        Cmd::Checkpoint { task, summary, intent, next } => {
            let data = call(
                "mutation($id: UUID!, $s: String!, $i: String!, $n: String!) {
                    checkpoint(taskId: $id, summary: $s, intent: $i, nextAction: $n) { seq message details }
                }",
                json!({ "id": task, "s": summary, "i": intent, "n": next }),
            )?;
            Ok(data["checkpoint"].clone())
        }
        Cmd::Log { task, line } => {
            let mut count = 0;
            match line {
                Some(line) => {
                    send_log(&task, &line)?;
                    count = 1;
                }
                None => {
                    for line in std::io::stdin().lock().lines() {
                        send_log(&task, &line?)?;
                        count += 1;
                    }
                }
            }
            Ok(json!({ "lines": count }))
        }
        Cmd::Submit { task, summary } => {
            let data = call(
                "mutation($id: UUID!, $s: String) { submitTask(taskId: $id, summary: $s) { id status } }",
                json!({ "id": task, "s": summary }),
            )?;
            Ok(data["submitTask"].clone())
        }
        Cmd::Fail { task, reason } => {
            let data = call(
                "mutation($id: UUID!, $r: String!) { failTask(taskId: $id, reason: $r) { id status } }",
                json!({ "id": task, "r": reason }),
            )?;
            Ok(data["failTask"].clone())
        }
        Cmd::Release { task } => {
            let data = call(
                "mutation($id: UUID!) { releaseTask(taskId: $id) { id status } }",
                json!({ "id": task }),
            )?;
            Ok(data["releaseTask"].clone())
        }
        Cmd::Work { project, command } => work(project, command),
    }
}

fn send_log(task: &str, line: &str) -> anyhow::Result<()> {
    call(
        "mutation($id: UUID!, $c: String!) { appendLog(taskId: $id, content: $c) { seq } }",
        json!({ "id": task, "c": line }),
    )?;
    Ok(())
}

/// How often `work` sends a heartbeat. The board's lease is 60 seconds.
const HEARTBEAT_EVERY: Duration = Duration::from_secs(15);

/// Claim the next task and run `command` as its agent
fn work(project: Option<String>, command: Vec<String>) -> anyhow::Result<Value> {
    let data = call(
        &format!("mutation($p: UUID) {{ claimNextTask(projectId: $p) {{ {ASSIGNMENT} }} }}"),
        json!({ "p": project }),
    )?;
    let job = &data["claimNextTask"];
    if job.is_null() {
        return Ok(Value::Null); // nothing is waiting
    }
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    let task = text(&job["task"]["id"]);

    let spawned = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(text(&job["worktreePath"]))
        .env("KANBAN_TASK_ID", &task)
        .env("KANBAN_TASK_TITLE", text(&job["task"]["title"]))
        .env("KANBAN_TASK_PROMPT", text(&job["task"]["prompt"]))
        .env("KANBAN_JOURNAL", text(&job["journal"]))
        .env("KANBAN_FEEDBACK", text(&job["feedback"]))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            // We hold the claim, so say why nothing will happen instead of going silent
            let reason = format!("could not start `{}`: {e}", command[0]);
            call(
                "mutation($id: UUID!, $r: String!) { failTask(taskId: $id, reason: $r) { id } }",
                json!({ "id": task, "r": reason }),
            )?;
            bail!("{reason}");
        }
    };
    let stdout = child.stdout.take().expect("stdout is piped");
    let child = Arc::new(Mutex::new(child));

    // A second thread sends heartbeats while the agent runs. A refused heartbeat means
    // the claim is gone (a person stopped the task, or it expired), so it kills the agent.
    let done = Arc::new(AtomicBool::new(false));
    let lost = Arc::new(AtomicBool::new(false));
    let heart = {
        let (task, child, done, lost) = (task.clone(), child.clone(), done.clone(), lost.clone());
        std::thread::spawn(move || {
            let mut waited = Duration::ZERO;
            while !done.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(200));
                waited += Duration::from_millis(200);
                if waited < HEARTBEAT_EVERY {
                    continue;
                }
                waited = Duration::ZERO;
                let beat = call("mutation($id: UUID!) { heartbeat(taskId: $id) { taskId } }", json!({ "id": task }));
                if beat.is_err() {
                    lost.store(true, Ordering::Relaxed);
                    let _ = child.lock().unwrap().kill();
                    return;
                }
            }
        })
    };

    // Pass on everything the agent prints. The board files "JOURNAL:" lines in the journal.
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if send_log(&task, &line).is_err() {
            break; // the claim is gone; the heartbeat thread deals with it
        }
    }
    let status = child.lock().unwrap().wait()?;
    done.store(true, Ordering::Relaxed);
    let _ = heart.join();

    if lost.load(Ordering::Relaxed) {
        return Ok(json!({ "task": task, "result": "stopped" }));
    }
    if status.success() {
        call("mutation($id: UUID!) { submitTask(taskId: $id) { id } }", json!({ "id": task }))?;
        Ok(json!({ "task": task, "result": "submitted" }))
    } else {
        let reason = match status.code() {
            Some(code) => format!("`{}` exited with code {code}", command[0]),
            None => format!("`{}` was killed by a signal", command[0]),
        };
        call(
            "mutation($id: UUID!, $r: String!) { failTask(taskId: $id, reason: $r) { id } }",
            json!({ "id": task, "r": reason }),
        )?;
        Ok(json!({ "task": task, "result": "failed", "reason": reason }))
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(answer) => {
            println!("{}", serde_json::to_string_pretty(&answer).expect("JSON values always print"));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("kanban: {e:#}");
            ExitCode::FAILURE
        }
    }
}
