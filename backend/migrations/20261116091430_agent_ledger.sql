-- The ledger: agents ("workers") claim tasks from the board and report back.

-- A task can be given to any agent that talks to the board, not only the built-in ones
ALTER TYPE agent_kind ADD VALUE 'external';
ALTER TYPE journal_kind ADD VALUE 'checkpoint';

-- What a note is about, so the journal can be read and filtered by purpose
CREATE TYPE note_kind AS ENUM ('plan', 'progress', 'blocker', 'decision', 'evidence');

-- Anything that takes work from the board. External workers prove who they are with a token;
-- we only store the token's hash. The backend's own runners have no token (NULL).
CREATE TABLE workers (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT NOT NULL UNIQUE,
    token_hash   TEXT UNIQUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ
);

-- Who is working on a task right now. task_id is the primary key, so the database itself
-- guarantees a task has at most one claim, even if two workers ask at the same instant.
CREATE TABLE claims (
    task_id      UUID PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    worker_id    UUID NOT NULL REFERENCES workers(id),
    run_id       UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    claimed_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT now() -- the worker must keep refreshing this
);

ALTER TABLE runs ADD COLUMN worker_id UUID REFERENCES workers(id);

-- Journal entries say what kind of note they are and which worker signed them
ALTER TABLE journal_entries
    ADD COLUMN note_kind note_kind,
    ADD COLUMN author    TEXT;
