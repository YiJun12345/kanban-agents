# Study guide: journal, review flow, real agents, the new design, and the ledger

This guide explains everything added on the `journal-review-agents` and `agent-ledger`
branches: what each feature does, where its code lives, why it is built that way, and how to
try it. Read it top to bottom once, with the app running next to you, then use it as a reference.

> **Two parts.** Sections 1 to 12 describe the first branch. [Section 13](#13-the-ledger-agents-take-work-themselves)
> describes the ledger, which came after and changed how agents are run. Where an earlier
> section is no longer exact, a note like this one points to section 13.

**Contents**

1. [What changed, at a glance](#1-what-changed-at-a-glance)
2. [Get it running](#2-get-it-running)
3. [A ten-minute tour](#3-a-ten-minute-tour)
4. [The design: "The Logbook"](#4-the-design-the-logbook)
5. [Backend, feature by feature](#5-backend-feature-by-feature)
6. [Frontend, feature by feature](#6-frontend-feature-by-feature)
7. [The tests and what each one proves](#7-the-tests-and-what-each-one-proves)
8. [New ideas you met, and where to see them](#8-new-ideas-you-met-and-where-to-see-them)
9. [What I fixed in your code](#9-what-i-fixed-in-your-code)
10. [Using the real Claude agent](#10-using-the-real-claude-agent)
11. [Questions George might ask](#11-questions-george-might-ask)
12. [Exercises](#12-exercises)
13. [The ledger: agents take work themselves](#13-the-ledger-agents-take-work-themselves)
14. [An accessibility and phone pass](#14-an-accessibility-and-phone-pass)

---

## 1. What changed, at a glance

| Feature | What you see | Main files |
|---|---|---|
| Journal summary and headline | The agent ends with a summary; each card shows its newest journal line | [journal.rs](../backend/src/journal.rs), [graphql.rs](../backend/src/graphql.rs) |
| Stop an agent | "Stop agent" button, or drag the card back to To Do | [agent.rs](../backend/src/agent.rs) |
| Retry and recovery | "Retry" on a failed task; tasks stuck by a restart become Failed | [domain.rs](../backend/src/domain.rs), [agent.rs](../backend/src/agent.rs) |
| Approve & merge | The branch is merged into `main`; worktree and branch are deleted | [review.rs](../backend/src/review.rs), [git.rs](../backend/src/git.rs) |
| Request changes | Your feedback goes to the agent and it runs again | [review.rs](../backend/src/review.rs) |
| Claude Code agent | A real agent option in the task form | [claude.rs](../backend/src/claude.rs) |
| Create a project in the UI | "+" next to the project menu | [ProjectSwitcher.tsx](../web/src/components/ProjectSwitcher.tsx) |
| Activity grouped by run | One box per agent run, with filters and raw output on demand | [Journal.tsx](../web/src/components/Journal.tsx) |
| Logbook page | The whole project's journal, by day | [Logbook.tsx](../web/src/components/Logbook.tsx) |
| New design | Paper, ink, tickets and stamps; light and dark | [index.css](../web/src/index.css) |

Numbers after the first branch: **36** backend tests (there were 18) and **10** browser tests
(there were 4). After the ledger: **42** and **11**.

The work is in three commits. Reading them in order is a good way to study:

1. `Journal, review flow, stoppable agents and a Claude Code runner` (backend)
2. `Frontend: Logbook design, review actions, logbook page, project switcher`
3. `Docs: README and study guide`

---

## 2. Get it running

You are on a new branch, so `main` is untouched until you merge.

```bash
cd ~/projects/kanban-agents
git status                    # should say: On branch agent-ledger
docker compose up -d          # Postgres (Docker Desktop must be running)

# Terminal 1
cd backend && cargo run       # applies 3 new migrations to your database

# Terminal 2
cd web && npm install         # new: the fonts are npm packages
npm run dev
```

✅ The backend should print `Claude agent is off: KANBAN_ANTHROPIC_API_KEY is not set in backend/.env`
and then the GraphiQL address. That first line is expected. It means only the fake agent can run.

Two things to know:

- **Your old projects.** Your database has about 20 projects with the same name. The board now
  opens the first project *that has tasks*, and the menu at the top shows each project's task
  count in brackets.
- **Running the tests** needs the scratch database from before:
  ```bash
  cd backend && DATABASE_URL=postgres://kanban:kanban@localhost:5432/kanban_scratch_master cargo test
  cd web && npm run test:e2e
  ```

---

## 3. A ten-minute tour

Do these in order at http://localhost:5173. Each step names what to look for.

1. **Add a task** called "Add greeting" with the description "add a greeting". Notice the
   ticket number (like `#6AB0`) on the card.
2. **Click the card**, then press **Start agent** in the panel. Watch three things at once:
   the card shows "working", the top bar says "1 agent running", and the panel's *Run #1* box
   fills with entries while "View raw output" shows the terminal.
3. When it lands in **In Review**, read the panel from the top: the **Agent summary**, the
   **Changes** (diff), then **Activity**. The card itself now shows the summary.
4. Press **Request changes**, type "make it shorter", and send. The agent runs again by
   itself. Open *Run #2*: its second note says "Addressing review feedback: make it shorter",
   and the diff has a new line that mentions your feedback.
5. Press **Approve & merge**. The card moves to Done and the panel says
   "Merged into main as `abc1234`". Now check the real repository:
   ```bash
   cd ~/projects/demo-repo
   git log -1            # "Merge task: Add greeting", with the journal in the message
   git branch            # the kanban/task-… branch is gone
   ls                    # AGENT_NOTES.md is on main now
   ```
6. Add a task with "this will fail" in the description and start it. It lands in **Failed**
   with the reason on the card. Press **Retry** to run it again from there.
7. Add another task, start it, and press **Stop agent** within three seconds. The card goes
   back to To Do and the run is marked *stopped*.
8. Click **Logbook** in the top bar. This is every task's journal in one list. Click any row
   to open that task.
9. Try the **All / Notes / History** filters. *Notes* is what people and agents wrote.
   *History* is what the system recorded.

---

## 4. The design: "The Logbook"

George asked for a journal so that everything the agents do is clear. So the design takes the
journal as its idea: **the board is a paper desk, tasks are tickets, and everything is written
down in ink.**

| Idea | How it shows up |
|---|---|
| Paper desk | Warm off-white background with a faint dot grid |
| Tickets | Cards have a ticket number, a coloured edge, and the newest journal line as a margin note |
| Rubber stamps | The status in the panel is a stamp, pressed on slightly crooked (`rotate: -2deg`) |
| Blue ink is the agent | Anything the agent wrote is blue: its notes, its summary, the "working" spinner |
| Sticky notes | A person's note has a yellow background |
| Typewriter | Labels, times and system entries use a monospace font; they are the "machine" voice |
| Night desk | Dark mode is the same desk at night, not a different design |

Three fonts carry this: **Fraunces** (a serif, for headings and the agent's summary),
**IBM Plex Sans** (for reading), and **IBM Plex Mono** (for labels and data). They are npm
packages imported in [main.tsx](../web/src/main.tsx), so the app needs no font website.

**How the CSS is organised.** Open [index.css](../web/src/index.css). The top of the file
defines *design tokens*: named values such as `--paper`, `--ink`, `--blue`. Everything below
uses the names, never raw colours. Dark mode only redefines the tokens:

```css
:root { --paper: #f3eee3; --ink: #211d18; --blue: #2748c9; /* … */ }

@media (prefers-color-scheme: dark) {
  :root { --paper: #15130f; --ink: #ece5d6; --blue: #91a7ff; /* … */ }
}
```

So to change the whole look, you change a dozen lines at the top.

**Status colours** use one trick. Any element with a `data-status` attribute gets a `--status`
variable, and columns, cards and stamps all just say `var(--status)`:

```css
[data-status='IN_REVIEW'] { --status: var(--review); }
.column { border-top: 3px solid var(--status); }
.stamp  { color: var(--status); border: 1.5px solid var(--status); }
```

**Icons** are small SVG drawings in [Icon.tsx](../web/src/components/Icon.tsx), not emoji or
symbol characters. You hit the reason earlier: the 🤖 emoji showed as an empty box in the test
browser, because characters depend on the fonts installed. SVG looks the same everywhere.

---

## 5. Backend, feature by feature

### 5.1 Summary, headline, and one place that saves and broadcasts

You typed most of this in Part 2a. In short:

- `parse_agent_line` in [journal.rs](../backend/src/journal.rs) returns the **kind** as well
  as the text: `JOURNAL: SUMMARY: …` becomes a `Summary`, any other `JOURNAL:` line a `Note`.
- `journal::record` saves an entry and then sends **two** events: `Journal` (for the open
  task panel) and `TaskUpdated` (so the board refetches and the card's headline changes).
- `Task.headline` in [graphql.rs](../backend/src/graphql.rs) is the newest entry whose kind is
  note, summary or error. Status moves are left out because the column already shows them.

### 5.2 Stopping an agent

> **Changed by the ledger.** `Agents::start` is now `Agents::dispatch`, and the loop waits for
> a third thing: a heartbeat being due. The oneshot channel and `select!` are still exactly
> as described here. See [13.5](#135-the-built-in-runners-became-workers).

**The problem.** The agent runs in a background task. The GraphQL mutation that wants to stop
it is a *different* task. They need a way to talk.

**The tool: a oneshot channel.** A `oneshot` channel carries exactly one message, once. We
create one per run. The agent keeps the receiving end; a shared map keeps the sending end.

```rust
// agent.rs
pub struct Agents {
    pub claude: ClaudeConfig,
    // task id -> (which run, the sender that tells that run to stop)
    running: Mutex<HashMap<Uuid, (u64, oneshot::Sender<()>)>>,
    next_token: Mutex<u64>,
}
```

`Agents::start` puts the sender in the map and spawns the run. `Agents::stop` takes the sender
out and sends `()`. The whole `Agents` value is shared as `Arc<Agents>` in the GraphQL schema
data, the same way the database pool is.

**The agent side: `tokio::select!`.** The agent used to wait only for the next line of output.
Now it waits for *whichever comes first*: a line, or the stop message.

```rust
loop {
    tokio::select! {
        line = lines.next_line() => {
            let Some(content) = line? else { break };   // the agent closed its output
            log.line(content).await?;
        }
        _ = &mut stop => {                              // someone asked us to stop
            child.kill().await?;
            log.line("stopped".into()).await?;
            return Ok(Outcome::Stopped);
        }
    }
}
```

**Why the `u64` token?** Imagine you stop a task and start it again straight away. The old
run is still finishing. When it ends, it removes "its" entry from the map, but by then the
entry belongs to the *new* run. The token is a number that is different for every run, and a
run only removes the entry if the token is still its own.

**Who triggers it.** `move_task` calls `agents.stop(id)` whenever a card lands in To Do. If no
agent is running, it does nothing.

**The outcome type.** A run can now end three ways, so `work()` returns an enum instead of
just an exit code:

```rust
enum Outcome {
    Exited(Option<i32>),   // the process ended (None: it never ran, or a signal killed it)
    Stopped,               // someone moved the card away
}
```

`run()` matches on it. A stopped run writes "Run stopped by you after 0.8s" and does **not**
move the card, because the person who stopped it has already put it in To Do.

### 5.3 Retry, and recovering after a restart

- **Retry** is one new rule in the state machine in [domain.rs](../backend/src/domain.rs):
  `(Failed, InProgress)`. The test `exactly_these_moves_are_allowed` lists every allowed pair,
  so the new rule had to be added there too. That test is there to stop rules being added by
  accident.
- **Recovery.** If you stop the backend while an agent is working, the task stays "In
  Progress" forever, because the agent died with the backend. `ledger::recover` (it was
  `agent::recover` before the ledger) runs once at startup (see [main.rs](../backend/src/main.rs)). It finds those tasks, writes "The backend
  restarted while the agent was running" in their journal, and moves them to Failed, where
  you can retry them.

### 5.4 Approve: merge, clean up, move to Done

All of this is `review::approve` in [review.rs](../backend/src/review.rs). The **order** of
the steps is the interesting part:

```
1. Check the task is In Review.                       (otherwise: the usual "cannot move" error)
2. Merge the branch into the project's branch.
     - if it fails: undo it (git merge --abort), write an ERROR journal entry,
       return the error. The task stays In Review. Nothing else changed.
3. Delete the worktree and the branch.                (a failure here is only logged)
4. Save the merge commit on the task; write a MERGE journal entry.
5. Move the task to Done (with its STATUS journal entry).
```

Why this order? **The step that can fail for outside reasons goes first.** A merge can
conflict. If we moved the card to Done first and the merge then failed, the board would say
Done about work that is not on `main`. By merging first, a failure leaves everything as it
was. And once the merge has worked, a problem while tidying up must not undo it, so step 3
only logs a warning.

`git::merge` in [git.rs](../backend/src/git.rs) has three details worth reading:

- It refuses to merge if the repository has a different branch checked out, and says which.
- If the branch has nothing new (`git rev-list --count main..branch` is 0), it returns `None`
  and no merge commit is made.
- It uses `--no-ff`, so there is always a real merge commit, and puts the task's story in its
  message: the title, the agent's summary, and the whole journal. `git log` on `main` then
  explains every change without the board.

**Dragging to Done is the same thing.** `move_task` checks for `Done` first and calls
`review::approve`, so the button and the drag cannot behave differently.

**The diff after a merge.** The branch is deleted, so `main...branch` no longer works. The
task now remembers `merge_commit`, and `Task.diff` uses `git diff <commit>^1 <commit>`: "what
did this merge bring in". `^1` means the merge's first parent, which is `main` just before.

### 5.5 Request changes

`review::request_changes` does four things: checks the task is In Review, saves your feedback
as a HUMAN NOTE (with `details: {"review": "changes_requested"}` so the UI can label it), moves
the task to In Progress, and starts the agent with the feedback.

The agent gets the feedback twice, on purpose. It is passed directly (the fake agent reads it
from the `FEEDBACK` environment variable; Claude gets a "The reviewer asked for changes"
section in its prompt), and it is also in the journal that every re-run receives.

### 5.6 The Claude Code agent

Everything about Claude is in [claude.rs](../backend/src/claude.rs), in three parts.

**1. The command.** `ClaudeConfig::command` builds this:

```
claude -p "<prompt>" --output-format stream-json --verbose
       --permission-mode acceptEdits --max-budget-usd 1.00 --no-session-persistence
```

`-p` is print mode: no interactive screen, exit when done. `acceptEdits` lets it edit files
but not run shell commands. `--max-budget-usd` caps what one run can cost.

**2. Keeping it on your own account.** This is the part that protects you:

```rust
// A name of our own, NOT ANTHROPIC_API_KEY: a key that happens to be exported in
// your shell for something else is never picked up by accident
api_key: var("KANBAN_ANTHROPIC_API_KEY"),
```

- The key is read from a variable with our own name, then handed to the CLI.
- The CLI is given its own config directory (`CLAUDE_CONFIG_DIR`), so it never sees a login
  saved in `~/.claude`.
- Variables that could point it at another account or endpoint are removed.
- With no key, `command()` returns an error. The task fails with a clear journal entry. It
  never falls back to anything else.

**3. Making the output readable.** With `stream-json`, the CLI prints one JSON object per
line. `claude::readable` turns each into zero or more plain lines:

| The CLI prints | The log shows |
|---|---|
| `{"type":"system","subtype":"init","model":"…"}` | `Claude Code session started (…)` |
| an assistant message with text | the text, line by line |
| an assistant message with a tool call | `⚙ Edit src/home.tsx` |
| a tool result | nothing (noise) |
| `{"type":"result", …}` | `Claude finished: 4 turns, $0.0312` |
| anything that is not JSON | the line unchanged |

Because the agent's text comes out as plain lines, the existing `JOURNAL:` handling works on
it with no changes. The prompt (`claude::prompt`) tells Claude the rules: write `JOURNAL:`
lines, finish with `JOURNAL: SUMMARY:`, and do not run git.

**How `agent.rs` chooses.** One `match` in `work()`:

```rust
let mut command = match task.agent {
    AgentKind::Fake => fake_command(&task.prompt, &context, feedback),
    AgentKind::Claude => claude.command(&claude::prompt(task, &context, feedback))?,
    AgentKind::Codex => anyhow::bail!("the Codex agent is not supported yet"),
};
```

Everything after that (worktree, reading lines, journal, commit) is shared by all agents.

**What is not proven yet.** The tests run a small shell script that pretends to be the
`claude` CLI. That proves our side: the flags, the key handling, the parsing, the commit. It
does not prove the real CLI prints exactly what the script prints. See
[section 10](#10-using-the-real-claude-agent).

### 5.7 Creating a project checks the repository

`createProject` now asks git which branch is checked out at the path you give. If git cannot
answer, you get "… is not a git repository with a branch checked out". If it can, that branch
becomes the project's `default_branch`, so repositories that use `master` or `trunk` work too.
Before this, a wrong path was only discovered when the first agent failed.

The `agents` query tells the UI which agents can run here, and why not when they can't.

---

## 6. Frontend, feature by feature

### 6.1 How the files fit together

```
App.tsx                 which project, which view, which task is open
├─ ProjectSwitcher      the project menu and the new-project form
├─ NewTaskForm          title, description, agent
├─ Board                the five columns and drag and drop
│   └─ Column → TaskCard
├─ Logbook              the project-wide journal
└─ TaskPanel            one task: details, summary, diff
    ├─ ReviewActions    the buttons for the next step
    └─ Activity         (in Journal.tsx) filters, timeline, runs, note form
```

Shared, non-visual code is outside `components/`: [journal.ts](../web/src/journal.ts) (types,
labels, filters), [useJournal.ts](../web/src/useJournal.ts) and
[useRefetchOnTick.ts](../web/src/useRefetchOnTick.ts).

### 6.2 Grouping the activity by run

The API returns a flat list of entries. Each has a `runId`, or none if a person did it. The
function `groupByRun` in [Journal.tsx](../web/src/components/Journal.tsx) turns the list into
*items*: a single entry, or a run that holds its entries.

```ts
type Run = { type: 'run'; id: string; number: number; entries: Entry[] }
type Item = { type: 'entry'; entry: Entry } | Run
```

This is a *discriminated union*: every item has a `type` field, and TypeScript uses it to know
which other fields exist. After `if (item.type === 'entry')`, TypeScript knows `item.entry`
is there.

`RunItem` works out the run's state from its own entries. No `RUN_FINISHED` entry yet means
*running*. Otherwise it reads that entry's `details`: `stopped`, or `exit_code === 0`.

No backend change was needed for grouping, because the `run_id` column was in the journal
table from the start.

### 6.3 A bug worth understanding: the card that stayed "In Progress"

While testing, a card sometimes stayed in In Progress although the agent had finished.

**What happened.** When a run ends, the backend sends several events within a few
milliseconds (summary, commit, run finished, moved to In Review). The board refetched on each
event. But while one fetch is on its way, a second identical fetch is dropped as a duplicate.
So the *first* fetch, started before the card moved, was the one that answered, and its
answer was already out of date.

**The fix** is [useRefetchOnTick.ts](../web/src/useRefetchOnTick.ts), about ten lines:

```ts
export function useRefetchOnTick(tick: number, busy: boolean, refetch: …) {
  const seen = useRef(tick)            // the newest tick we have fetched for
  useEffect(() => {
    if (tick !== seen.current && !busy) {
      seen.current = tick
      refetch({ requestPolicy: 'network-only' })
    }
  }, [tick, busy, refetch])
}
```

`tick` is a number that goes up with every live event. `App.tsx` counts the events:

```ts
const [live] = useSubscription({ query: TaskUpdatedSubscription, … }, (count = 0) => count + 1)
const tick = live.data ?? 0
```

The rule the hook follows: *if an event arrived since my last fetch started, fetch again, but
only once the current fetch has finished.* So however fast events arrive, there is always one
more fetch after the last event. The board, the task panel and the logbook all use it.

The general lesson: **"refetch on every event" is not enough when fetches can overlap. You
must make sure a fetch *starts after* the last event.**

### 6.4 The buttons for the next step

[ReviewActions.tsx](../web/src/components/ReviewActions.tsx) shows different buttons for each
status: Start agent, Stop agent, Approve & merge / Request changes, Retry / Back to To Do.
Every button calls the same mutations that dragging does, so there is one set of rules, on the
server. The buttons matter for two reasons: you cannot type feedback with a drag, and not
everyone can drag with a mouse.

The small `run()` helper shows the server's own message when it refuses, for example a merge
conflict. The client never invents its own error text.

### 6.5 The logbook

[Logbook.tsx](../web/src/components/Logbook.tsx) uses a query you already had:
`project { journal(limit: 200) { … task { id title } } }`. The entries come newest first, and
a short loop puts each under a day heading ("Today", "Yesterday", "14 November"). Each row is
a real `<button>`, so it works with the keyboard, and clicking it opens the same task panel
as the board does.

### 6.6 Projects

`App.tsx` picks the project in this order: the one you chose last (remembered in
`localStorage`), else the first that has tasks, else the first. With no project at all, the
page shows the new-project form instead of an empty board.

---

## 7. The tests and what each one proves

### Backend (`cargo test`, 42 tests; the ledger's are in [13.8](#138-the-tests))

| Test | What it proves |
|---|---|
| `journal_records_a_full_run` | One run writes exactly 8 entries in order, and the commit message carries the notes |
| `card_headline_is_the_newest_note_or_summary` | Status moves never become the headline |
| `notes_update_the_board_live` | Adding a note sends `taskUpdated`, so cards update without a reload |
| `moving_back_to_todo_stops_the_agent` | The run ends early, commits nothing, and leaves the card in To Do |
| `stuck_tasks_are_failed_after_a_restart` | `recover` un-sticks tasks and says why |
| `approving_merges_the_branch_and_cleans_up` | `main` gets the file; worktree and branch are gone; the diff still works |
| `dragging_to_done_is_an_approval` | `moveTask(DONE)` and `approveTask` are the same thing |
| `merge_conflict_keeps_the_task_in_review` | A conflict changes nothing and leaves no half-finished merge |
| `requesting_changes_reruns_the_agent_with_the_feedback` | The feedback is journaled, read by the agent, and visible in its work |
| `requesting_changes_needs_feedback_and_a_task_in_review` | A refused request leaves no note behind |
| `claude_agent_runs_headless_with_its_own_key` | The CLI is started with our key, our config directory and `-p`; its JSON becomes readable lines |
| `claude_agent_without_a_key_fails_with_a_clear_error` | No key means a clear failure, and the `agents` query says so |
| `create_project_checks_the_repo` | A bad path is refused; the repo's own branch becomes the default |
| `git::merge_and_cleanup`, `git::conflicting_merge_is_undone` | The git helpers on their own, against a temporary repo |
| `claude::stream_json_becomes_readable_lines` | Each kind of JSON event, including non-JSON lines |

**The stand-in CLI trick.** `claude_agent_runs_headless_with_its_own_key` writes a small shell
script to a temp folder and tells the backend "this is the `claude` program". The script
*checks its own inputs* and exits with an error if they are wrong:

```sh
[ "$ANTHROPIC_API_KEY" = "test-key" ] || { echo "wrong api key" >&2; exit 9; }
[ "$1" = "-p" ] || { echo "not headless" >&2; exit 9; }
```

So if the backend ever started the CLI with the wrong key or flags, the test would fail. This
only works because the settings are a value (`ClaudeConfig`) passed into the code, not read
from the environment deep inside it. Tests can then build their own. That is why
`ClaudeConfig::from_env()` is called once, in `main.rs`, and nowhere else.

### Browser (`npm run test:e2e`, 11 tests)

The tests in [board.spec.ts](../web/e2e/board.spec.ts) drive a real browser. Two details:

- `approving merges the work into main` does not trust the page. After clicking the button it
  runs `git log` on the real repository and checks the merge commit is there.
- [playwright.config.ts](../web/playwright.config.ts) starts the test backend with
  `KANBAN_ANTHROPIC_API_KEY: ''`. Even when you add a real key to `backend/.env` later, the
  tests can never start a paid agent.

---

## 8. New ideas you met, and where to see them

| Idea | In one sentence | Where |
|---|---|---|
| `oneshot` channel | A channel for exactly one message, here "please stop" | `Agents::start` / `stop` in agent.rs |
| `tokio::select!` | Wait for several things, act on whichever happens first | the read loop in `work()` |
| `Arc<Mutex<…>>` | Data shared between tasks (`Arc`), changed by one at a time (`Mutex`) | `Agents.running` |
| Enum for outcomes | Name each way something can end, then `match` on it | `enum Outcome` in agent.rs |
| Do the risky step first | Order steps so a failure leaves nothing half done | `review::approve` |
| Configuration as a value | Read the environment once, pass a struct around | `ClaudeConfig` |
| `r##"…"##` raw strings | Add `#`s when the text itself contains `"#` | `STUB_CLAUDE` in graphql.rs tests |
| `ALTER TYPE … ADD VALUE` | Add a value to a Postgres enum in a *new* migration | the two newest migrations |
| Discriminated union | A `type` field tells TypeScript which shape a value has | `Item` in Journal.tsx |
| Custom hook | A function that uses other hooks, to share behaviour | `useJournal`, `useRefetchOnTick` |
| `useRef` | A value that survives re-renders without causing one | `seen` in useRefetchOnTick.ts |
| Design tokens | Name your colours once, use the names everywhere | top of index.css |

---

## 9. What I fixed in your code

When I picked up your Part 2a, the backend did not compile. There were four small slips, all
from typing. They are worth a look because each has a typical error message:

| Slip | What the compiler said |
|---|---|
| `parseuse std::process::Stdio;` (a stray word on line 1 of agent.rs) | `expected one of '!' or '::', found 'std'` |
| `notes: Vec::new(), notes: Vec::new(), summary: None }; };` (pasted twice) | `unexpected closing delimiter: '}'`, pointing far below the real mistake |
| The old `commit_message(title, notes)` kept, while the call passed three arguments | `this function takes 2 arguments but 3 arguments were supplied` |
| The old two lines kept in `add_journal_note` above the new `journal::record` line | `expected 'NewEntry', found 'JournalEntry'` |

The second one is the most useful to remember. **When Rust complains about a closing bracket,
the mistake is usually earlier than the line it names.** The message even hints at it: "this
delimiter might not be properly closed… as it matches this but it has different indentation".

The last two have the same cause: when a step says "replace these lines", the old lines have
to go. If you are unsure, `git diff` shows exactly what you changed since the last commit.

---

## 10. Using the real Claude agent

Nothing here happens until you do it. It costs money on your own API key.

1. **Get a key** at https://console.anthropic.com with your *personal* account. Set a low
   spending limit there first.
2. **Add it to `backend/.env`**, in your editor, never in a chat:
   ```
   KANBAN_ANTHROPIC_API_KEY=sk-ant-…
   ```
   `git status` must not list `.env`. It is ignored, and it has to stay that way.
3. **Restart the backend.** The "Claude agent is off" line should be gone.
4. **Add a small task** to a throwaway repository, choose **Claude Code** in the agent menu,
   and start it. A good first task: "Add a file HELLO.md that says hello."
5. **Watch the raw output.** You should see "Claude Code session started", then `⚙` lines for
   each tool, then "Claude finished: N turns, $0.0…".

What might need adjusting, since this has only been tested against a stand-in:

- **If the log shows raw JSON**, the CLI's output shape differs from what `claude::readable`
  expects. Copy one such line and adjust the function. Its unit test shows the shapes it knows.
- **If there are no journal entries**, the model did not start its lines with `JOURNAL:`.
  Strengthen the rules at the end of `claude::prompt`.
- **If it stops with `error_max_budget_usd`**, raise `CLAUDE_MAX_BUDGET_USD` in `.env`.
- **It cannot run commands** (no tests, no `npm install`). That is the `acceptEdits` setting,
  chosen as the safe start.

---

## 11. Questions George might ask

**"Why is the journal a separate table and not just the logs?"**
Logs are everything the agent printed, for watching a run. The journal is the short record of
decisions and events, for reading afterwards. They have different readers, so they are stored
separately, and the journal is the one that cannot be edited.

**"What stops someone rewriting the journal?"**
A database trigger rejects every UPDATE and DELETE on `journal_entries`. Even a bug in our own
code cannot change an entry. Entries only disappear when their whole task is deleted.

**"What happens if two people approve the same task at once?"**
The status move is safe: `tasks::transition` locks the row, so only one request can move the
card to Done. The merge step before it is not locked, so two requests arriving in the same
instant could both try to merge. That is a known gap. The fix would be to take the row lock
before merging and hold it until the move.

**"What if the merge conflicts?"**
`git merge --abort` puts the repository back, an ERROR entry explains it in the journal, and
the task stays in review. You can then request changes so the agent redoes the work.

**"Why does the agent write to the journal by printing lines?"**
Any program can print. The fake agent is a shell script and Claude Code is a CLI, and neither
needs to know our database or API. Adding another agent means writing one function that
builds its command.

**"How do you know the real agent only uses your key?"**
The key has its own variable name, the CLI gets its own config directory, and variables that
select other accounts are removed. A test starts a stand-in CLI that fails unless it received
exactly that key and that directory.

**"Why GraphQL subscriptions and then refetching, instead of sending the data in the event?"**
The events say *that* something changed; the queries stay the single description of *what*
each view shows. It costs one extra request per event, which is fine at this size. The hook
in `useRefetchOnTick.ts` makes sure the last event is never missed.

---

## 12. Exercises

Small changes that make you read the code. Each has a hint.

1. **A new stamp colour.** Make the Done stamp and column use a different green. *Hint: one
   token at the top of index.css, in both the light and the dark block.*
2. **Show the cost.** For a Claude run, put the cost in the RUN_FINISHED entry's `details` and
   show it in the run's header. *Hint: `claude::readable` already sees `total_cost_usd`;
   `RunItem` already reads `details.duration_secs`.*
3. **Count the runs on the card.** Show "run 2" on a card whose task has run more than once.
   *Hint: add a field next to `headline` in `impl Task`, then ask for it in the Board query.*
4. **A filter for errors.** Add an "Errors" chip. *Hint: `FILTERS` in journal.ts is the only
   place to change, and both the panel and the logbook pick it up.*
5. **Delete a project.** Add a `deleteProject` mutation and a button. *Hint: `ON DELETE
   CASCADE` already removes its tasks, and the journal trigger allows deletes that come from a
   cascade. What should happen to worktrees that still exist?*
6. **Write the missing test.** Nothing tests that approving a task whose agent changed no
   files works. *Hint: copy `approving_merges_the_branch_and_cleans_up`. What should
   `mergeCommit` be? What does the journal say?*

---

## 13. The ledger: agents take work themselves

### 13.1 The idea

Until now the board was the boss of the agents: *it* started an agent for a task, and only the
two agents it knows about (fake and Claude Code) could exist.

The ledger turns that around. The board becomes a **work ledger**: a place where work is
offered, taken, reported on and handed in. An agent is now a **worker** that comes to the
board, says who it is, and takes a task. The board no longer needs to know what kind of
program the worker is.

| | Before | With the ledger |
|---|---|---|
| Who starts the work | The backend starts an agent | A worker **claims** a task |
| Which agents can exist | Fake and Claude Code | Anything that can run a command |
| How the board knows an agent is alive | It owns the process | The worker sends **heartbeats** |
| How an agent writes the journal | The backend reads its printed lines | The worker calls the API (printed lines still work) |
| Who is "the agent" in the journal | Just "Agent" | A named worker, e.g. `claude-laptop` |

Nothing was lost: you still drag cards, the fake agent still runs by itself, review and merge
are unchanged. The built-in agents simply became workers like any other.

### 13.2 The words

- **Worker**: anything that takes tasks. It has a name. An outside worker also has a token.
- **Claim**: "this worker is working on this task right now". One per task.
- **Heartbeat**: the worker saying "still working". A claim with no heartbeat for 60 seconds
  **expires**.
- **Run**: one attempt at a task, from claim to end. You already had runs.
- **External task**: a task whose agent is set to *External worker (CLI)*. The board starts
  nothing for it; it waits in In Progress until a worker claims it.

### 13.3 Life of a claim

```mermaid
stateDiagram-v2
    [*] --> Waiting: card moved to In Progress
    Waiting --> Claimed: worker claims (run + worktree are created)
    Claimed --> Claimed: heartbeat, note, checkpoint, log
    Claimed --> InReview: submit (board commits the worktree)
    Claimed --> Failed: fail, or no heartbeat for 60s
    Claimed --> Waiting: release (worker gives it back)
    Claimed --> ToDo: a person takes it back
```

"Waiting" and "Claimed" are both the In Progress column. The card tells them apart: it says
*waiting for a worker*, or shows the worker's name.

### 13.4 The database: two tables and one clever key

The migration `…_agent_ledger.sql` adds `workers` and `claims`.

```sql
CREATE TABLE claims (
    task_id      UUID PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    worker_id    UUID NOT NULL REFERENCES workers(id),
    run_id       UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    claimed_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

The important word is `PRIMARY KEY` on `task_id`. A primary key must be unique, so **the
database itself refuses a second claim on the same task.** We do not check "is it free?" and
then insert, because two workers could both see "free" in the same instant. We just try to
insert:

```rust
// ledger.rs, in claim()
"INSERT INTO claims (task_id, worker_id, run_id) VALUES ($1, $2, $3)
 ON CONFLICT (task_id) DO NOTHING RETURNING *"
```

If a row comes back, the task is ours. If nothing comes back, someone else has it. This is
the same lesson as the row lock in `tasks::transition`: **let the database decide races,
because it is the only one that sees every request.**

The run and the claim are inserted in one transaction. If the claim fails, the function
returns early, the transaction is dropped, and the run row disappears with it.

**Tokens.** `ledger::register` creates a random token (`kbt_` plus two random UUIDs) and
returns it once. The table stores only its SHA-256 hash. So even someone who reads the
database cannot use a worker's token. To check a request, we hash the token it sent and look
for that hash. The built-in workers have no token at all (`token_hash` is NULL), so nobody
can pretend to be them from outside.

### 13.5 The built-in runners became workers

All the ledger's rules are functions in [ledger.rs](../backend/src/ledger.rs):

| Function | What it does |
|---|---|
| `register`, `authenticate`, `builtin` | create a worker; find the worker for a token; the backend's own worker |
| `claim`, `claim_next` | take a task: creates the run, the claim and the worktree; returns an `Assignment` |
| `heartbeat` | refresh the claim; an error means the claim is gone |
| `note`, `checkpoint`, `log_line` | write in the journal or the raw output, as the worker |
| `submit` | commit the worktree, end the run, move to In Review |
| `fail`, `release` | end the run as failed, or give the task back |
| `revoke` | the board takes the task away (a person stopped it) |
| `expire` | end every claim that has been silent too long |
| `recover` | at startup, fail what the built-in runners were doing |

Every function that needs a claim starts with `held(pool, worker, task_id)`, which returns the
claim or the error "`w1` does not hold a claim on this task". That one check is the whole
permission system for workers.

[agent.rs](../backend/src/agent.rs) got much shorter, because it no longer saves anything
itself. Its `run` is now just: claim, run the program, report how it ended.

```rust
let worker = ledger::builtin(pool, task.agent).await?;
let job = ledger::claim(pool, events, &worker, task.id).await?;

match work(pool, events, claude, &worker, &job, stop).await {
    Ok(Outcome::Exited(Some(0))) => { ledger::submit(pool, events, &worker, task.id, None).await?; }
    Ok(Outcome::Exited(code))    => { ledger::fail(pool, events, &worker, task.id, None, code).await?; }
    // …
}
```

This matters more than it looks. The fake agent and an outside worker now go through **the
same code** for everything that counts (commit, journal, status). There is one set of rules,
and the tests for outside workers also protect the built-in path.

The read loop waits for three things now:

```rust
tokio::select! {
    line = lines.next_line() => { /* pass the line to ledger::log_line */ }
    _ = heartbeat.tick() => {
        // A refused heartbeat means the claim is gone: stop working
        if ledger::heartbeat(pool, events, worker, task.id).await.is_err() {
            child.kill().await?;
            return Ok(Outcome::Stopped);
        }
    }
    _ = &mut stop => { child.kill().await?; return Ok(Outcome::Stopped); }
}
```

**Ending a run exactly once.** A run can be ended by the worker (submit), by a person (stop)
or by the sweeper (expiry), possibly at the same moment. `ledger::end` handles that with one
statement: it deletes the claim first and checks that a row was really deleted.

```rust
let dropped = sqlx::query("DELETE FROM claims WHERE task_id = $1 AND run_id = $2") …
if dropped.rows_affected() == 0 {
    anyhow::bail!("this run has already ended");
}
```

Whoever deletes the claim "wins" and writes the ending. Everyone else is told the run already
ended. Again the database settles the race.

**The sweeper.** [main.rs](../backend/src/main.rs) starts a small loop that calls
`ledger::expire` every 10 seconds. A claim silent for longer than `LEASE` (60 seconds) gets an
ERROR entry "No heartbeat from w1 for 60s, so its claim expired" and its task moves to
Failed, where you can retry it.

**Restarts.** `ledger::recover` fails only the tasks of the built-in runners, because those
died with the backend. An outside worker is a separate program: its claim survives a backend
restart and it carries on.

### 13.6 How a worker talks to the board

**The API.** Workers use ordinary GraphQL mutations: `claimNextTask`, `heartbeat`,
`addWorkerNote`, `checkpoint`, `appendLog`, `submitTask`, `failTask`, `releaseTask`. They are
at the bottom of `MutationRoot` in [graphql.rs](../backend/src/graphql.rs). Each begins with
`let worker = caller(ctx).await?;`.

**The token travels in a header.** [main.rs](../backend/src/main.rs) now has its own small
handler instead of the ready-made one, so it can read the request's headers:

```rust
async fn run_graphql(State(schema): State<AppSchema>, headers: HeaderMap, request: GraphQLRequest)
    -> GraphQLResponse
{
    let token = headers.get("authorization") /* … strip "Bearer " … */;
    schema.execute(request.into_inner().data(Bearer(token))).await.into()
}
```

`.data(Bearer(token))` attaches the token to *this one request*. Resolvers that need a worker
call `caller(ctx)`, which reads it back with `ctx.data_opt::<Bearer>()`. Resolvers for people
never look at it. You have used schema-wide data before (the pool); this is the per-request
version of the same idea.

**The CLI.** [cli/src/main.rs](../cli/src/main.rs) is a second Rust program, about 300 lines.
Every command sends one GraphQL request and prints the answer as JSON, so an agent or a shell
script can read it. Three things to notice:

- `clap` builds the command-line parser from an enum. Each variant is a command, and the `///`
  comments become the `--help` text.
- `call()` is the only function that talks to the network. It adds the token from
  `KANBAN_TOKEN`, and turns a GraphQL error into a normal Rust error, so the server's own
  message is what the user sees.
- `kanban work -- <command>` wraps *any* program as an agent. It claims a task, runs the
  command in the worktree, and uses a second **thread** to send heartbeats while the main
  thread reads the program's output. If a heartbeat is refused, that thread kills the program.
  This is the same shape as the backend's `select!` loop, written with threads instead of
  async, because a small CLI does not need an async runtime.

**Agents that can only print.** `ledger::log_line` checks every line for the `JOURNAL:`
prefix and files it in the journal. The kind can now be given in the line:

```
JOURNAL: PLAN: add the toggle to the header
JOURNAL: DECISION: store the choice in localStorage, so it survives a reload
JOURNAL: SUMMARY: added a dark mode toggle
```

So there are two ways to keep a journal, and both end in the same function (`ledger::note`):
call the API (`kanban note --kind plan "…"`), or print a line.

### 13.7 Try it

Start the backend and frontend as usual, then in a third terminal:

```bash
cd ~/projects/kanban-agents
cargo build --manifest-path cli/Cargo.toml
export PATH="$PWD/cli/target/debug:$PATH"
```

1. In the browser, open **Workers**, type a name, press **Create token**, and copy the
   `export KANBAN_TOKEN=…` line into the terminal.
2. On the board, choose **External worker (CLI)** in the agent menu, add a task, and drag it
   to In Progress. The card says *waiting for a worker*.
3. In the terminal:
   ```bash
   kanban whoami
   kanban next                 # the card now shows your worker's name
   ```
   Copy the task `id` and the `worktreePath` from the answer.
4. Report and work:
   ```bash
   kanban note <id> --kind plan "write hello.txt"
   echo hello > <worktreePath>/hello.txt
   kanban submit <id> --summary "Added hello.txt"
   ```
   The card moves to In Review. Open it: your notes are signed with the worker's name.
5. See expiry: add another External task, drag it to In Progress, run `kanban next`, then do
   nothing. After about 30 seconds the card says *quiet*. After a little over a minute it
   moves to Failed with "No heartbeat…" in its journal.
6. See stopping: claim a task, then drag its card back to To Do. Now `kanban hb <id>` answers
   "… does not hold a claim on this task".
7. A whole worker as a script: add another External task, drag it to In Progress, and run
   `sh examples/external-agent.sh`. Read the script; it is the steps above in 20 lines.
8. Any program as an agent: add one more External task, drag it over, and run
   `kanban work -- sh -c 'echo "JOURNAL: PLAN: $KANBAN_TASK_PROMPT"; date > stamp.txt'`.
   `work` claims, runs the command in the worktree, sends heartbeats, and submits.

### 13.8 The tests

| Test | What it proves |
|---|---|
| `outside_worker_does_a_task_from_claim_to_review` | The whole path through the API: nothing starts by itself, claim, notes, checkpoint, log, submit, commit message, signed entries |
| `a_task_has_one_worker_at_a_time` | The second claim is refused; the loser can do nothing; after a release the task can be claimed again as run #2 |
| `worker_calls_need_a_token` | No token and a wrong token are refused; names are checked and unique |
| `a_silent_worker_loses_its_claim` | `expire` with a normal lease does nothing; with a lease of zero it fails the task and says why |
| `stopping_takes_the_task_away_from_an_outside_worker` | After a person's stop, the worker's heartbeat is refused |
| `review_feedback_reaches_the_next_outside_worker` | "Request changes" starts nothing; the next claim carries the feedback; a restart leaves outside tasks alone |
| `ledger::tokens_are_stored_as_sha256` | The hash function, against a known value |
| (browser) `an outside worker takes a task with the kanban CLI` | The real `kanban` binary and the real UI together |

A trick worth copying is in `a_silent_worker_loses_its_claim`. Waiting 60 seconds in a test
would be slow, so `expire` takes the lease as a parameter. Production passes 60 seconds; the
test passes zero. **If time makes something hard to test, make the time an input.**

### 13.9 The UI

- **Cards** in In Progress show the worker's name with a spinner, *quiet 45s* in amber when
  heartbeats have stopped for over 30 seconds, or *waiting for a worker*. This is the `Holder`
  component in [TaskCard.tsx](../web/src/components/TaskCard.tsx).
- **The timeline** shows the worker's name instead of "Agent", a small tag for the note's
  kind, and for a checkpoint its *Intent* and *Next* lines.
- **Workers page** ([Workers.tsx](../web/src/components/Workers.tsx)) lists workers and what
  each holds, and creates tokens. The token is kept only in the component's state: reload the
  page and it is gone, which matches "shown once".

### 13.10 What is still missing

Said plainly, so you can answer if asked:

- **People have no login.** Anyone who can open the board can move cards and create workers.
  Tokens identify workers only. That is fine for a tool on your own machine and not for a
  shared server.
- **A token does not expire** and cannot be revoked in the UI yet.
- **One claim, one task.** There is no limit on how many tasks a worker may hold, and no
  priority or dependency order in `claim_next`: it takes the task that has waited longest.
- **A worker on another computer** can claim a task, but the worktree path it gets is a folder
  on the backend's computer. Outside workers must run on the same machine for now.

### 13.11 More exercises

7. **Revoke a worker.** Add a `removeWorker` mutation and a button. *Hint: what should happen
   to a claim it holds? Look at `revoke`.*
8. **A limit per worker.** Refuse a claim when the worker already holds two tasks. *Hint: one
   `count(*)` at the top of `claim`, and one new test next to `a_task_has_one_worker_at_a_time`.*
9. **Priority.** Add a `priority` column to tasks and make `claim_next` take the highest
   first. *Hint: the `ORDER BY` in `claim_next`, a new migration, and a field in the task form.*
10. **`kanban watch`.** A CLI command that prints a task's journal entries as they arrive.
    *Hint: the simplest version polls `show` every two seconds and prints entries with a
    higher `seq` than the last one it printed.*

---

## 14. An accessibility and phone pass

We ran an outside design checker (Taste Skill's browser inspector) against the board. It is
made for marketing websites, so most of what it reported did not apply to an app ("the hero
must fill the first screen", "give the cards a photograph"). Seven findings were real, and
they are worth knowing because every UI gets them wrong at first.

| Problem | Why it matters | The fix |
|---|---|---|
| Faint grey text read at 3.2:1 | The accessibility minimum for text is **4.5:1**; below it, small text is hard to read in sunlight or with weaker eyesight | Darker `--muted`, `--faint` and status colours at the top of [index.css](../web/src/index.css) |
| "waiting for a worker" ran past its card on a phone | Text must never leave its box | The line is cut with an ellipsis, and columns are wider on a phone |
| The top bar was wider than a 320px screen | The whole page then scrolls sideways | The counts wrap onto their own lines |
| Buttons and tabs were 28 to 31px tall on a phone | A thumb needs about **44px** | `min-height: 44px` on controls in the phone media query |
| Fields were named only by placeholders | A placeholder disappears when you type, and some screen readers skip it | A small caption above each field, with the field inside its `<label>` |
| "Add task" was disabled until you typed | A disabled button does not say what is missing | The button always works; pressing it with no title shows "Give the task a title first." in a `role="status"` line |
| No skip link, no page description | Keyboard users had to tab through the top bar every time; shared links had no summary | A "Skip to the board" link that shows on focus, and a `<meta name="description">` |

**How to check contrast yourself.** Contrast is a number from 1:1 (same colour) to 21:1 (black
on white). Browser DevTools shows it: inspect a text element, click its colour swatch, and
read "Contrast ratio". Because all our colours are tokens, fixing a failing colour is one line.

**A testing surprise.** After the fields were wrapped in `<label>`, one browser test failed:
`expect(option).toBeDisabled()` said the option was enabled. The app was right and the test
was wrong. For an element inside a `<label>`, Playwright checks the label's *control* (here
the whole menu), not the element itself. The test now reads the option's own attribute:
`toHaveAttribute('disabled')`. When a test fails after a refactor, first ask which of the two
changed its meaning.

