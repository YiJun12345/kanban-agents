-- Approving a task merges its branch: remember the merge commit, and journal the merge
ALTER TYPE journal_kind ADD VALUE 'merge';

ALTER TABLE tasks ADD COLUMN merge_commit TEXT;
