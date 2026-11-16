use anyhow::{bail, Context};
use std::path::Path;
use tokio::process::Command;

/// Run `git -C <dir> <args...>` and return its stdout, or an error with git's message
async fn git(dir: &str, args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .await
        .context("could not run git")?;

    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Create a worktree on a new branch from `base`, unless it already exists (a re-run)
pub async fn ensure_worktree(repo: &str, base: &str, branch: &str, path: &str) -> anyhow::Result<()> {
    if Path::new(path).exists() {
        return Ok(());
    }
    git(repo, &["worktree", "add", "-b", branch, path, base]).await?;
    Ok(())
}

/// Commit everything in the worktree. Returns false if there was nothing to commit.
pub async fn commit_all(worktree: &str, message: &str) -> anyhow::Result<bool> {
    git(worktree, &["add", "-A"]).await?;
    if git(worktree, &["status", "--porcelain"]).await?.trim().is_empty() {
        return Ok(false);
    }
    git(
        worktree,
        &[
            "-c", "user.name=kanban-agent",
            "-c", "user.email=kanban-agent@localhost",
            "commit", "-m", message,
        ],
    )
    .await?;
    Ok(true)
}

/// Short hash and changed files of the latest commit
pub async fn last_commit(worktree: &str) -> anyhow::Result<(String, Vec<String>)> {
    let out = git(worktree, &["show", "--name-only", "--format=%h", "HEAD"]).await?;
    let mut lines = out.lines().filter(|l| !l.is_empty());
    let hash = lines.next().unwrap_or_default().to_string();
    Ok((hash, lines.map(String::from).collect()))
}

/// Everything the branch changed since it split off from `base`
pub async fn diff(repo: &str, base: &str, branch: &str) -> anyhow::Result<String> {
    git(repo, &["diff", &format!("{base}...{branch}")]).await
}

/// What a merge commit brought in (used after the task's branch is gone)
pub async fn diff_of_merge(repo: &str, commit: &str) -> anyhow::Result<String> {
    git(repo, &["diff", &format!("{commit}^1"), commit]).await
}

/// The branch that is checked out in this repo, e.g. "main"
pub async fn current_branch(repo: &str) -> anyhow::Result<String> {
    let out = git(repo, &["symbolic-ref", "--short", "HEAD"]).await;
    Ok(out.context("not a git repository with a branch checked out")?.trim().to_string())
}

pub async fn has_branch(repo: &str, branch: &str) -> bool {
    git(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).await.is_ok()
}

/// Merge the task's branch into `base` with a merge commit, and return its short hash.
/// Returns None when the branch has nothing new. A conflict is undone and reported as an error.
pub async fn merge(repo: &str, base: &str, branch: &str, message: &str) -> anyhow::Result<Option<String>> {
    let checked_out = current_branch(repo).await?;
    if checked_out != base {
        bail!("the repository has `{checked_out}` checked out; switch it to `{base}` to merge");
    }
    let ahead = git(repo, &["rev-list", "--count", &format!("{base}..{branch}")]).await?;
    if ahead.trim() == "0" {
        return Ok(None);
    }

    let merged = git(
        repo,
        &[
            "-c", "user.name=kanban-agent",
            "-c", "user.email=kanban-agent@localhost",
            "merge", "--no-ff", "-m", message, branch,
        ],
    )
    .await;
    if let Err(e) = merged {
        // Leave the repo as we found it instead of half-merged
        let _ = git(repo, &["merge", "--abort"]).await;
        return Err(e.context(format!("could not merge {branch} into {base}")));
    }
    Ok(Some(git(repo, &["rev-parse", "--short", "HEAD"]).await?.trim().to_string()))
}

/// Delete a task's worktree folder and its branch (after a merge, both are no longer needed)
pub async fn remove_worktree(repo: &str, path: &str, branch: &str) -> anyhow::Result<()> {
    if Path::new(path).exists() {
        git(repo, &["worktree", "remove", "--force", path]).await?;
    }
    if has_branch(repo, branch).await {
        git(repo, &["branch", "-D", branch]).await?;
    }
    Ok(())
}

#[cfg(test)]
pub mod testing {
    use super::git;
    use uuid::Uuid;

    /// A throwaway repo with one commit on `main`, in its own temp folder.
    /// Returns (folder to delete afterwards, repo path).
    pub async fn temp_repo() -> (std::path::PathBuf, String) {
        let root = std::env::temp_dir().join(format!("kanban-test-{}", Uuid::new_v4()));
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let repo = repo.to_str().unwrap().to_string();

        git(&repo, &["init", "-b", "main"]).await.unwrap();
        std::fs::write(format!("{repo}/README.md"), "# Test repo\n").unwrap();
        git(&repo, &["add", "-A"]).await.unwrap();
        git(&repo, &["-c", "user.name=test", "-c", "user.email=test@localhost", "commit", "-m", "init"])
            .await
            .unwrap();
        (root, repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn worktree_commit_and_diff() {
        let (root, repo) = testing::temp_repo().await;
        let wt = format!("{repo}-worktrees/task-1");

        ensure_worktree(&repo, "main", "kanban/task-1", &wt).await.unwrap();
        assert!(Path::new(&wt).join("README.md").exists(), "worktree has the repo's files");

        // Calling it again (a re-run) reuses the folder instead of failing
        ensure_worktree(&repo, "main", "kanban/task-1", &wt).await.unwrap();

        // Nothing changed yet, so nothing to commit
        assert!(!commit_all(&wt, "empty").await.unwrap());
        assert_eq!(diff(&repo, "main", "kanban/task-1").await.unwrap(), "");

        std::fs::write(format!("{wt}/NOTES.md"), "hello\n").unwrap();
        assert!(commit_all(&wt, "add notes").await.unwrap());
        let (hash, files) = last_commit(&wt).await.unwrap();
        assert_eq!(hash.len(), 7, "short hash: {hash}");
        assert_eq!(files, ["NOTES.md"]);

        let d = diff(&repo, "main", "kanban/task-1").await.unwrap();
        assert!(d.contains("+hello"), "diff shows the new line: {d}");
        // main itself is untouched
        assert!(!Path::new(&repo).join("NOTES.md").exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn merge_and_cleanup() {
        let (root, repo) = testing::temp_repo().await;
        let wt = format!("{repo}-worktrees/task-1");
        ensure_worktree(&repo, "main", "kanban/task-1", &wt).await.unwrap();
        assert_eq!(current_branch(&repo).await.unwrap(), "main");

        // A branch with nothing new is not merged
        assert_eq!(merge(&repo, "main", "kanban/task-1", "Merge").await.unwrap(), None);

        std::fs::write(format!("{wt}/NOTES.md"), "hello\n").unwrap();
        commit_all(&wt, "add notes").await.unwrap();
        let commit = merge(&repo, "main", "kanban/task-1", "Merge task: notes").await.unwrap().unwrap();
        assert!(Path::new(&repo).join("NOTES.md").exists(), "main now has the file");
        assert!(diff_of_merge(&repo, &commit).await.unwrap().contains("+hello"));

        remove_worktree(&repo, &wt, "kanban/task-1").await.unwrap();
        assert!(!Path::new(&wt).exists());
        assert!(!has_branch(&repo, "kanban/task-1").await);
        // The merge's diff is still there after the branch is deleted
        assert!(diff_of_merge(&repo, &commit).await.unwrap().contains("+hello"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn conflicting_merge_is_undone() {
        let (root, repo) = testing::temp_repo().await;
        let wt = format!("{repo}-worktrees/task-1");
        ensure_worktree(&repo, "main", "kanban/task-1", &wt).await.unwrap();

        // The same file changes on both sides
        std::fs::write(format!("{wt}/README.md"), "from the agent\n").unwrap();
        commit_all(&wt, "agent").await.unwrap();
        std::fs::write(format!("{repo}/README.md"), "from a person\n").unwrap();
        commit_all(&repo, "person").await.unwrap();

        let err = merge(&repo, "main", "kanban/task-1", "Merge").await.unwrap_err();
        assert!(format!("{err:#}").starts_with("could not merge kanban/task-1 into main"), "{err:#}");
        // No half-finished merge is left behind
        assert_eq!(git(&repo, &["status", "--porcelain"]).await.unwrap(), "");
        assert_eq!(std::fs::read_to_string(format!("{repo}/README.md")).unwrap(), "from a person\n");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn git_errors_include_gits_message() {
        let err = ensure_worktree("/nonexistent/repo", "main", "b", "/nonexistent/wt")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("git worktree add"), "{err}");
        assert!(err.contains("/nonexistent/repo"), "{err}");
    }
}
