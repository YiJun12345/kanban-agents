-- The allowed values for a task's column and agent type
CREATE TYPE task_status AS ENUM ('todo', 'in_progress', 'in_review', 'done', 'failed');
CREATE TYPE agent_kind  AS ENUM ('claude', 'codex', 'fake');

-- A project = one git repo that agents will work on
CREATE TABLE projects (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name           TEXT NOT NULL,
    repo_path      TEXT NOT NULL,
    default_branch TEXT NOT NULL DEFAULT 'main',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A task = one card on the board
CREATE TABLE tasks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    prompt      TEXT NOT NULL,
    status      task_status NOT NULL DEFAULT 'todo',
    agent       agent_kind  NOT NULL DEFAULT 'fake',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX tasks_project_id_idx ON tasks (project_id);
