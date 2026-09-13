use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::git::{CommitInfo, FileEntry, FileStatusKind, files_changed_in_commits};

/// A file and the commits, among the currently displayed candidate list,
/// that touched it.
#[derive(Debug, Clone)]
pub struct FileCommitMatch {
    pub file: String,
    pub matching_commits: Vec<CommitInfo>,
}

impl FileCommitMatch {
    /// A match is "evident" when exactly one candidate commit touched the
    /// file — no ambiguity about which commit to fix up into.
    pub fn is_evident(&self) -> bool {
        self.matching_commits.len() == 1
    }
}

/// Maps each commit's short sha to the set of paths it touched, via a
/// single batched `git show` call for every commit at once (`-n 30` used to
/// mean 30 separate `git diff-tree` spawns here — this is now one process).
/// Shared by `match_files_to_commits` and by the Commits pane, which
/// highlights whether the currently selected file belongs to each listed
/// commit.
pub fn build_files_by_commit(commits: &[CommitInfo]) -> Result<HashMap<String, HashSet<String>>> {
    let shas: Vec<&str> = commits.iter().map(|c| c.short_sha.as_str()).collect();
    files_changed_in_commits(&shas)
}

/// For each modified/tracked file, finds which of `commits` touched it,
/// using an already-computed `files_by_commit` map (see
/// `build_files_by_commit`) — pure computation, no git calls, so callers
/// that already have the map (e.g. `App`, refreshed once per commit-list
/// change) don't pay for it twice. Untracked files are skipped entirely: a
/// brand new file has no history, so it can never be a fixup target.
pub fn match_files_to_commits(
    files: &[FileEntry],
    commits: &[CommitInfo],
    files_by_commit: &HashMap<String, HashSet<String>>,
) -> Vec<FileCommitMatch> {
    files
        .iter()
        .filter(|f| f.kind != FileStatusKind::Untracked)
        .filter_map(|f| {
            let matching_commits: Vec<CommitInfo> = commits
                .iter()
                .filter(|c| {
                    files_by_commit
                        .get(&c.short_sha)
                        .is_some_and(|paths| paths.contains(&f.path))
                })
                .cloned()
                .collect();
            if matching_commits.is_empty() {
                None
            } else {
                Some(FileCommitMatch {
                    file: f.path.clone(),
                    matching_commits,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, kind: FileStatusKind) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            old_path: None,
            kind,
            staged: false,
            has_unstaged_changes: true,
        }
    }

    fn commit(sha: &str) -> CommitInfo {
        CommitInfo {
            short_sha: sha.to_string(),
            date: "2026-01-01".to_string(),
            author: "tester".to_string(),
            message: "msg".to_string(),
        }
    }

    #[test]
    fn untracked_files_are_never_matched() {
        let files = vec![file("new.txt", FileStatusKind::Untracked)];
        let matches = match_files_to_commits(&files, &[], &HashMap::new());
        assert!(matches.is_empty());
    }

    #[test]
    fn is_evident_true_only_for_a_single_matching_commit() {
        let one = FileCommitMatch {
            file: "a.txt".to_string(),
            matching_commits: vec![commit("aaa")],
        };
        let two = FileCommitMatch {
            file: "b.txt".to_string(),
            matching_commits: vec![commit("aaa"), commit("bbb")],
        };
        assert!(one.is_evident());
        assert!(!two.is_evident());
    }
}
