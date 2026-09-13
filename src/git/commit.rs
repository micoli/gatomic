use std::collections::{HashMap, HashSet};

use anyhow::Result;

use super::repo::{run_git, run_git_allowing};

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub short_sha: String,
    pub date: String,
    pub author: String,
    pub message: String,
}

const LOG_FORMAT: &str = "%h\x01%ad\x01%an\x01%s";

fn parse_log_output(raw: &str) -> Vec<CommitInfo> {
    raw.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\x01');
            Some(CommitInfo {
                short_sha: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                message: parts.next()?.to_string(),
            })
        })
        .collect()
}

pub fn commit_fixup(sha: &str) -> Result<()> {
    run_git(&["commit", "--fixup", sha])?;
    Ok(())
}

/// Files touched by each of `shas`, in a single `git show` process instead
/// of one per commit — with many commits (e.g. `-n 30`) the naive
/// one-spawn-per-commit approach was the dominant cost of opening the
/// triage screen and refreshing the Commits pane. Unlike `git diff-tree`,
/// plain `git show` already handles a repo's first (parentless) commit
/// correctly without needing a `--root` flag.
pub fn files_changed_in_commits(shas: &[&str]) -> Result<HashMap<String, HashSet<String>>> {
    if shas.is_empty() {
        return Ok(HashMap::new());
    }

    let mut args = vec!["show", "--no-color", "--name-only", "--format=\u{1}%h"];
    args.extend_from_slice(shas);
    let raw = run_git(&args)?;

    let mut files_by_commit = HashMap::new();
    for chunk in raw.split('\u{1}').filter(|c| !c.is_empty()) {
        let mut lines = chunk.lines();
        let Some(sha) = lines.next() else { continue };
        let files: HashSet<String> = lines
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        files_by_commit.insert(sha.to_string(), files);
    }
    Ok(files_by_commit)
}

pub fn last_n_commits(n: usize) -> Result<Vec<CommitInfo>> {
    let n_arg = n.to_string();
    let raw = run_git(&[
        "log",
        "-n",
        &n_arg,
        "--date=short",
        &format!("--format={LOG_FORMAT}"),
        "HEAD",
    ])?;
    Ok(parse_log_output(&raw))
}

/// Finds a base ref to diff the current branch against: the branch's upstream
/// tracking ref if configured, falling back to origin/HEAD then common
/// default branch names. Returns None if no suitable base can be found.
fn find_branch_base() -> Option<String> {
    let candidates = [
        vec!["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        vec!["rev-parse", "--abbrev-ref", "origin/HEAD"],
    ];

    for args in candidates {
        if let Ok(out) = run_git_allowing(&args, &[0]) {
            let trimmed = out.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    for fallback in ["origin/main", "origin/master"] {
        if run_git_allowing(&["rev-parse", "--verify", "--quiet", fallback], &[0]).is_ok() {
            return Some(fallback.to_string());
        }
    }

    None
}

/// Commits unique to the current branch, i.e. `git log <base>..HEAD`.
/// Falls back to the full HEAD history when no base ref can be determined.
pub fn commits_of_current_branch() -> Result<Vec<CommitInfo>> {
    let range = match find_branch_base() {
        Some(base) => format!("{base}..HEAD"),
        None => "HEAD".to_string(),
    };

    let raw = run_git(&[
        "log",
        "--date=short",
        &format!("--format={LOG_FORMAT}"),
        &range,
    ])?;
    Ok(parse_log_output(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_log_output() {
        let raw = "abc123\x012026-09-13\x01Alice\x01Fix bug\nd4e5f6\x012026-09-12\x01Bob\x01Add feature\n";
        let commits = parse_log_output(raw);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].short_sha, "abc123");
        assert_eq!(commits[0].author, "Alice");
        assert_eq!(commits[0].message, "Fix bug");
        assert_eq!(commits[1].message, "Add feature");
    }
}
