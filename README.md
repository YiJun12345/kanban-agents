# Kanban Agents

A Kanban board where the "team members" are AI coding agents. You write a task, drag it to
**In Progress**, and an agent picks it up. The agent works in its own git worktree, streams its
output to the browser live, and commits its changes. The card then moves to **In Review**, where
you can read the diff before accepting the work.

![Board with a task in review showing the agent's log and diff](docs/board.png)

> **Status:** the agent runner, worktrees, live logs and diff review all work end to end with a
> scripted *fake* agent. The next milestone swaps it for a real coding agent (Claude Code).

## Features

- **Board rules on the server.** Cards follow a state machine, so you can't drag from To Do
  straight to Done, for example. The rule lives in one place (`TaskStatus::can_move_to`), and
  the database row is locked during a move so concurrent moves can't race.
- **Background agents.** Moving a card to In Progress starts an agent in a Tokio task. The
  mutation returns immediately.
- **Live updates over GraphQL subscriptions.** Agent output and card moves are pushed to every
  open browser over WebSocket. No polling.
- **One git worktree per task.** Each task gets its own branch (`kanban/task-<id>`) and folder,
  so agents can run in parallel without touching each other's files or your `main` checkout.
  Re-running a task reuses its worktree.
- **Diff review.** The task panel shows a coloured `git diff main...branch` of what the agent changed.
- **Failures are visible.** A non-zero exit or a git error moves the card to Failed, with the
  error in the log, instead of leaving it stuck in In Progress.

## Architecture

```mermaid
flowchart LR
    subgraph Browser["React (Vite)"]
        UI[Board + log panel]
    end
    subgraph Backend["Rust (axum + async-graphql)"]
        GQL[GraphQL<br/>queries / mutations]
        SUB[Subscriptions<br/>WebSocket]
        BUS((broadcast<br/>channel))
        AG[Agent runner<br/>tokio::spawn]
    end
    DB[(PostgreSQL)]
    GIT[(git repo +<br/>worktrees)]

    UI -- "POST /graphql" --> GQL
    UI <-- "ws /graphql/ws" --> SUB
    GQL --> DB
    GQL -- "moveTask → In Progress" --> AG
    AG -- "logs, status" --> DB
    AG -- "events" --> BUS --> SUB
    AG -- "worktree, commit, diff" --> GIT
```

### Task lifecycle

```mermaid
stateDiagram-v2
    [*] --> Todo
    Todo --> InProgress: drag (starts agent)
    InProgress --> InReview: agent exits 0
    InProgress --> Failed: agent fails
    InProgress --> Todo
    InReview --> Done: accept
    InReview --> InProgress: re-run
    Failed --> Todo
    Done --> [*]
```

## Tech stack

| Layer | Choice |
|---|---|
| API | GraphQL: [async-graphql](https://github.com/async-graphql/async-graphql) on [axum](https://github.com/tokio-rs/axum) |
| Backend | Rust 2024, Tokio, [sqlx](https://github.com/launchbadge/sqlx) (migrations run on startup) |
| Database | PostgreSQL 16 (Docker Compose) |
| Frontend | React 19, TypeScript, Vite, [urql](https://github.com/urql-graphql/urql), [dnd-kit](https://dndkit.com/) |
| Typed GraphQL | [graphql-codegen](https://the-guild.dev/graphql/codegen) client preset |
| Tests | `cargo test` (unit + `#[sqlx::test]` integration), Playwright (e2e) |

## GraphQL API

GraphiQL is served at `http://localhost:3000/graphql`.

```graphql
query {
  projects {
    name
    tasks { title status branch diff logs { seq content } }
  }
}

mutation { moveTask(id: "…", status: IN_PROGRESS) { status } }   # starts the agent

subscription { taskLogs(taskId: "…") { seq content } }            # live agent output
subscription { taskUpdated(projectId: "…") { id status } }       # live board changes
```

## Running locally

Requires Docker, Rust, Node 24+, `sqlx-cli` (`cargo install sqlx-cli --no-default-features --features postgres`).

```bash
docker compose up -d                       # Postgres on :5432

cd backend
echo 'DATABASE_URL=postgres://kanban:kanban@localhost:5432/kanban' > .env
cargo run                                  # API on :3000, applies migrations

cd ../web
npm install
npm run dev                                # UI on :5173
```

Create a project that points at any local git repo with a `main` branch (in GraphiQL):

```graphql
mutation { createProject(name: "My app", repoPath: "/path/to/repo") { id } }
```

Then add tasks in the UI and drag them to **In Progress**. Put the word `fail` in a description to
see the failure path.

## Tests

```bash
cd backend && cargo test     # 11 tests: board rules, git helpers, GraphQL + agent integration
cd web && npm run test:e2e   # 3 Playwright tests in a real browser
```

- **Unit tests** check every from/to pair of the state machine, and the git helpers (worktree
  creation, empty-commit skip, diff) against a temporary repo.
- **Integration tests** run real GraphQL operations against the schema. `#[sqlx::test]` gives
  each test its own freshly migrated database. They cover the full agent run, failure, a bad
  repo path, re-runs, and the log subscription stream.
- **End-to-end tests** start their own backend (port 3917, database `kanban_e2e`) and Vite
  (port 5917). They create a task in the UI, drag it with the mouse, watch the live logs, and
  check the diff appears.

## Project layout

```
backend/
  migrations/          SQL migrations (sqlx)
  src/
    main.rs            axum server, GraphiQL, WebSocket route
    graphql.rs         schema: queries, mutations, subscriptions
    domain.rs          Task / Project types, status state machine
    tasks.rs           transition() with row locking
    agent.rs           runs the agent, streams logs, commits work
    git.rs             worktree / commit / diff helpers
    events.rs          broadcast channel for subscriptions
web/
  src/App.tsx          board, drag and drop, live updates
  src/components/      Column, TaskCard, NewTaskForm, LogPanel (logs + diff)
  e2e/                 Playwright tests
```

## Roadmap

- [x] Board, GraphQL API, drag and drop
- [x] Background agent runner with live logs (subscriptions)
- [x] Per-task git worktrees, auto-commit, diff review
- [x] Unit, integration and end-to-end tests
- [ ] Real coding agent (Claude Code, headless) in the worktree
- [ ] Review flow: approve → merge into `main` and clean up the worktree; reject → re-run with feedback
