use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Path git accepts as "empty file" input for `--no-index` diffs, platform-specific.
#[cfg(windows)]
pub const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
pub const NULL_DEVICE: &str = "/dev/null";

/// Moves the process's current directory to the repository's top level.
///
/// Every path gatomic deals with (from `git status`, `git diff-tree`, ...)
/// is repo-root-relative, but plain `git <cmd> -- <path>` pathspecs are
/// resolved relative to the current directory. Without this, running
/// gatomic from a subdirectory silently produces empty diffs/stats for any
/// file whose root-relative path doesn't happen to also be a valid path
/// relative to that subdirectory.
pub fn cd_to_repo_root() -> Result<()> {
    let top_level = run_git(&["rev-parse", "--show-toplevel"])?;
    std::env::set_current_dir(top_level.trim())
        .with_context(|| format!("failed to switch to repo root {top_level:?}"))
}

/// Runs `git <args>` in the current directory and returns stdout as a String.
/// Fails on non-zero exit, except when `allow_exit_code_1` is set (used by
/// commands like `git diff --no-index` that exit 1 when there are differences).
pub fn run_git(args: &[&str]) -> Result<String> {
    run_git_allowing(args, &[0])
}

pub fn run_git_allowing(args: &[&str], allowed_exit_codes: &[i32]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn git {args:?}"))?;

    let code = output.status.code().unwrap_or(-1);
    if !allowed_exit_codes.contains(&code) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {args:?} failed (exit {code}): {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Stages a whole file, used for cases that are not subdivided into hunks
/// (deleted files, pure renames).
pub fn stage_path(path: &str) -> Result<()> {
    run_git(&["add", "--", path])?;
    Ok(())
}

/// Resets a path's index entry back to HEAD (or removes it from the index
/// entirely for a path with no HEAD entry), used before re-applying a hunk
/// selection so each toggle starts from a clean staged state for that file.
pub fn unstage_path(path: &str) -> Result<()> {
    run_git_allowing(&["reset", "-q", "--", path], &[0, 1])?;
    Ok(())
}

/// Runs `git <args>` feeding `input` on stdin, returning stdout as a String.
pub fn run_git_with_stdin(args: &[&str], input: &str) -> Result<String> {
    let mut child = Command::new("git")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn git {args:?}"))?;

    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(input.as_bytes())
        .with_context(|| format!("failed to write patch to git {args:?} stdin"))?;

    let output = child
        .wait_with_output()
        .with_context(|| format!("failed to wait for git {args:?}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {args:?} failed: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
