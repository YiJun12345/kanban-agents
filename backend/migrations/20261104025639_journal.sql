-- Who wrote a journal entry, and what kind of event it records
CREATE TYPE journal_actor AS ENUM ('agent', 'human', 'system');
CREATE TYPE journal_kind  AS ENUM ('status', 'run_started', 'run_finished', 'note', 'commit', 'error');

-- The journal: a clean, permanent record of what happened to each task
CREATE TABLE journal_entries (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    seq        BIGINT GENERATED ALWAYS AS IDENTITY, -- keeps entries in exact order
    task_id    UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    run_id     UUID REFERENCES runs(id) ON DELETE CASCADE,
    actor      journal_actor NOT NULL,
    kind       journal_kind  NOT NULL,
    message    TEXT NOT NULL,
    details    JSONB NOT NULL DEFAULT '{}', -- extra facts, e.g. {"commit": "a1b2c3d", "files": [...]}
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX journal_entries_task_idx ON journal_entries (task_id, seq);

-- Append-only: entries can never be edited, and only disappear together with their task
CREATE FUNCTION journal_append_only() RETURNS trigger AS $$
BEGIN
    -- pg_trigger_depth() > 1 means this DELETE comes from ON DELETE CASCADE (task deleted)
    IF TG_OP = 'DELETE' AND pg_trigger_depth() > 1 THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'journal entries are append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER journal_append_only
    BEFORE UPDATE OR DELETE ON journal_entries
    FOR EACH ROW EXECUTE FUNCTION journal_append_only();

