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

/// Everything the branch changed since it split off from `base`
pub async fn diff(repo: &str, base: &str, branch: &str) -> anyhow::Result<String> {
    git(repo, &["diff", &format!("{base}...{branch}")]).await
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

        let d = diff(&repo, "main", "kanban/task-1").await.unwrap();
        assert!(d.contains("+hello"), "diff shows the new line: {d}");
        // main itself is untouched
        assert!(!Path::new(&repo).join("NOTES.md").exists());

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
