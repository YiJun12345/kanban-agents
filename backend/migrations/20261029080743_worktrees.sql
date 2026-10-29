-- Where the agent works on this task (empty until its first run)
ALTER TABLE tasks
    ADD COLUMN branch        TEXT,
    ADD COLUMN worktree_path TEXT;
