//! The real agent: Claude Code, run headless (`claude -p`) inside the task's worktree.

use std::path::Path;

use serde_json::Value;
use tokio::process::Command;

use crate::domain::Task;

/// How to run Claude Code. Read once at startup from the environment (backend/.env).
#[derive(Clone, Debug)]
pub struct ClaudeConfig {
    /// The CLI to run (CLAUDE_BIN, default "claude")
    pub bin: String,
    /// The API key the agent bills to (KANBAN_ANTHROPIC_API_KEY)
    pub api_key: Option<String>,
    /// Where the CLI keeps its own settings and sessions (KANBAN_CLAUDE_CONFIG_DIR)
    pub config_dir: String,
    /// Optional model alias, e.g. "sonnet" (CLAUDE_MODEL)
    pub model: Option<String>,
    /// The most one run may spend, in dollars (CLAUDE_MAX_BUDGET_USD, default 1.00)
    pub max_budget_usd: String,
}

impl ClaudeConfig {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let home = var("HOME").unwrap_or_else(|| ".".into());
        Self {
            bin: var("CLAUDE_BIN").unwrap_or_else(|| "claude".into()),
            // A name of our own, NOT ANTHROPIC_API_KEY: a key that happens to be exported in
            // your shell for something else is never picked up by accident
            api_key: var("KANBAN_ANTHROPIC_API_KEY"),
            config_dir: var("KANBAN_CLAUDE_CONFIG_DIR")
                .unwrap_or_else(|| format!("{home}/.config/kanban-agents/claude")),
            model: var("CLAUDE_MODEL"),
            max_budget_usd: var("CLAUDE_MAX_BUDGET_USD").unwrap_or_else(|| "1.00".into()),
        }
    }

    /// Why Claude can't run right now, or None when it is ready
    pub fn problem(&self) -> Option<String> {
        if self.api_key.is_none() {
            return Some("KANBAN_ANTHROPIC_API_KEY is not set in backend/.env".into());
        }
        if !on_path(&self.bin) {
            return Some(format!("the `{}` command was not found", self.bin));
        }
        None
    }

    /// The command that runs one headless session. The caller sets the working directory.
    pub fn command(&self, prompt: &str) -> anyhow::Result<Command> {
        if let Some(problem) = self.problem() {
            anyhow::bail!("the Claude agent cannot run: {problem}");
        }
        let mut command = Command::new(&self.bin);
        command
            .arg("-p") // print mode: no interactive UI, exit when done
            .arg(prompt)
            .args(["--output-format", "stream-json", "--verbose"]) // one JSON event per line
            .args(["--permission-mode", "acceptEdits"]) // may edit files; may not run commands
            .args(["--max-budget-usd", &self.max_budget_usd])
            .arg("--no-session-persistence");
        if let Some(model) = &self.model {
            command.args(["--model", model]);
        }

        // The agent must only ever use the key from backend/.env. Its own config directory
        // means it never sees a login saved in ~/.claude, and we drop every variable that
        // could point it at another account or endpoint.
        for name in [
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
        ] {
            command.env_remove(name);
        }
        command
            .env("ANTHROPIC_API_KEY", self.api_key.as_deref().unwrap_or_default())
            .env("CLAUDE_CONFIG_DIR", &self.config_dir);
        Ok(command)
    }
}

/// Is this program a file we can find, either as a path or through $PATH?
fn on_path(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
}

/// What we ask the agent to do: the task, what happened before, and the journal rules
pub fn prompt(task: &Task, journal: &str, feedback: Option<&str>) -> String {
    let mut prompt = format!(
        "You are working on a task from a Kanban board. You are inside a git worktree of the \
         project, on a branch made for this task.\n\n# Task: {}\n\n{}\n",
        task.title, task.prompt
    );
    if !journal.is_empty() {
        prompt.push_str(&format!(
            "\n# What happened on this task so far (its journal)\n\n{journal}\n"
        ));
    }
    if let Some(feedback) = feedback {
        prompt.push_str(&format!(
            "\n# The reviewer asked for changes\n\nAddress this feedback:\n\n{feedback}\n"
        ));
    }
    prompt.push_str(
        "\n# Rules\n\n\
         - Make the changes directly in the files here. Do not run git: the board commits your \
         work when you finish.\n\
         - Keep a journal while you work. Write each entry on its own line, starting with \
         `JOURNAL:`. Write your plan first, then every important decision and why you made it.\n\
         - Finish with exactly one line starting with `JOURNAL: SUMMARY:` that tells the reviewer \
         what you changed.\n",
    );
    prompt
}

/// Turn one line of `--output-format stream-json` into lines a person can read.
/// Text the agent wrote comes out as it is, so `JOURNAL:` lines still work.
pub fn readable(line: &str) -> Vec<String> {
    let Ok(event) = serde_json::from_str::<Value>(line) else {
        return vec![line.to_string()]; // not JSON (e.g. an error message): show it unchanged
    };
    match event["type"].as_str() {
        Some("system") if event["subtype"] == "init" => {
            let model = event["model"].as_str().unwrap_or("unknown model");
            vec![format!("Claude Code session started ({model})")]
        }
        Some("assistant") => {
            let blocks = event["message"]["content"].as_array().cloned().unwrap_or_default();
            blocks.iter().flat_map(readable_block).collect()
        }
        Some("result") => {
            let turns = event["num_turns"].as_u64().unwrap_or(0);
            let cost = event["total_cost_usd"].as_f64().unwrap_or(0.0);
            if event["is_error"] == true || event["subtype"] != "success" {
                let why = event["result"].as_str().or(event["subtype"].as_str()).unwrap_or("unknown");
                vec![format!("error: Claude stopped: {why}")]
            } else {
                vec![format!("Claude finished: {turns} turns, ${cost:.4}")]
            }
        }
        _ => Vec::new(), // tool results and other events are noise for the reader
    }
}

/// One block of an assistant message: its text, or a one-line note about the tool it used
fn readable_block(block: &Value) -> Vec<String> {
    match block["type"].as_str() {
        Some("text") => block["text"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(String::from)
            .collect(),
        Some("tool_use") => {
            let name = block["name"].as_str().unwrap_or("tool");
            // Show the one input that says what the tool is working on
            let input = &block["input"];
            let target = ["file_path", "path", "pattern", "command", "description"]
                .iter()
                .find_map(|key| input[*key].as_str())
                .unwrap_or_default();
            vec![format!("⚙ {name} {target}").trim_end().to_string()]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stream_json_becomes_readable_lines() {
        let init = json!({ "type": "system", "subtype": "init", "model": "claude-sonnet" });
        assert_eq!(readable(&init.to_string()), ["Claude Code session started (claude-sonnet)"]);

        let assistant = json!({ "type": "assistant", "message": { "content": [
            { "type": "text", "text": "JOURNAL: Plan: add a greeting\n\nLet me look around." },
            { "type": "tool_use", "name": "Edit", "input": { "file_path": "src/home.tsx" } },
            { "type": "tool_use", "name": "TodoWrite", "input": {} },
        ]}});
        assert_eq!(
            readable(&assistant.to_string()),
            ["JOURNAL: Plan: add a greeting", "Let me look around.", "⚙ Edit src/home.tsx", "⚙ TodoWrite"]
        );

        let tool_result = json!({ "type": "user", "message": { "content": [] } });
        assert!(readable(&tool_result.to_string()).is_empty());

        let done = json!({ "type": "result", "subtype": "success", "is_error": false,
                           "num_turns": 4, "total_cost_usd": 0.0312, "result": "Done." });
        assert_eq!(readable(&done.to_string()), ["Claude finished: 4 turns, $0.0312"]);

        let over = json!({ "type": "result", "subtype": "error_max_budget_usd", "is_error": true });
        assert_eq!(readable(&over.to_string()), ["error: Claude stopped: error_max_budget_usd"]);

        // Anything that is not JSON is passed through
        assert_eq!(readable("Credit balance is too low"), ["Credit balance is too low"]);
    }

    #[test]
    fn refuses_to_run_without_its_own_key() {
        let config = ClaudeConfig {
            bin: "sh".into(),
            api_key: None,
            config_dir: "/tmp/unused".into(),
            model: None,
            max_budget_usd: "1.00".into(),
        };
        let err = config.command("hello").unwrap_err().to_string();
        assert!(err.contains("KANBAN_ANTHROPIC_API_KEY"), "{err}");

        let config = ClaudeConfig { api_key: Some("k".into()), bin: "no-such-program".into(), ..config };
        assert_eq!(config.problem().unwrap(), "the `no-such-program` command was not found");
    }

    #[test]
    fn prompt_carries_the_journal_and_the_feedback() {
        let task = Task {
            id: uuid::Uuid::nil(),
            project_id: uuid::Uuid::nil(),
            title: "Add greeting".into(),
            prompt: "Say hello on the home page".into(),
            status: crate::domain::TaskStatus::InProgress,
            agent: crate::domain::AgentKind::Claude,
            branch: None,
            worktree_path: None,
            merge_commit: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let first = prompt(&task, "", None);
        assert!(first.contains("# Task: Add greeting"));
        assert!(first.contains("JOURNAL: SUMMARY:"));
        assert!(!first.contains("so far"));

        let rerun = prompt(&task, "[Agent] Plan: say hi", Some("Make it friendlier"));
        assert!(rerun.contains("[Agent] Plan: say hi"));
        assert!(rerun.contains("Address this feedback:\n\nMake it friendlier"));
    }
}
