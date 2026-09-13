use std::collections::HashMap;

use anyhow::Result;

use super::model::FileDiff;
use super::parser::parse_file_diff;
use crate::git::{FileStatusKind, NULL_DEVICE, run_git, run_git_allowing};

/// Loads the diff for one file against HEAD (staged and unstaged changes
/// combined), so the resulting hunk structure — and therefore the hunk
/// indices `FileSelection` tracks decisions against — stays stable no
/// matter which hunks have already been staged. Using the plain unstaged
/// `git diff` here instead would make already-accepted hunks vanish from
/// the diff as soon as they're staged, desyncing `FileSelection` from the
/// next `FileDiff` fetched for the same file (leaving already-decided
/// hunks stuck: their indices would now point at different hunks).
///
/// Untracked files have no HEAD entry at all, so `git diff HEAD` doesn't
/// see them until they've been partially staged at least once — until
/// then, go through `git diff --no-index` against the platform's null
/// device, which produces a standard "new file" diff that the parser and
/// patch builder handle exactly like any other new file.
pub fn load_file_diff(path: &str, kind: &FileStatusKind, context_lines: u32) -> Result<FileDiff> {
    let context_arg = format!("-U{context_lines}");

    let raw = if *kind == FileStatusKind::Untracked {
        run_git_allowing(
            &[
                "diff",
                "--no-color",
                "--no-index",
                &context_arg,
                "--",
                NULL_DEVICE,
                path,
            ],
            &[0, 1],
        )?
    } else {
        run_git(&["diff", "HEAD", "--no-color", &context_arg, "--", path])?
    };

    parse_file_diff(&raw)
}

/// Hunk counts for every tracked file changed vs HEAD, in a single `git
/// diff HEAD` call — lets the Files pane show a `hunks 0/N` total for every
/// modified file immediately, not only the ones already opened at least
/// once (which is when a real `FileSelection`, with an accurate "decided"
/// count, first gets created). Untracked files are excluded, same as
/// elsewhere: `git diff HEAD` can't see them at all.
///
/// Uses the same `context_lines` the app opens files with, since a wider
/// context can merge what would otherwise be separate hunks — the total
/// shown here must match what hunk_pane actually displays once opened.
pub fn count_hunks_per_file(context_lines: u32) -> Result<HashMap<String, usize>> {
    let context_arg = format!("-U{context_lines}");
    let raw = run_git(&["diff", "HEAD", "--no-color", &context_arg])?;

    let mut counts = HashMap::new();
    for chunk in split_into_file_diffs(&raw) {
        if let Ok(file_diff) = parse_file_diff(&chunk) {
            counts.insert(file_diff.display_path().to_string(), file_diff.hunks.len());
        }
    }
    Ok(counts)
}

/// Splits a multi-file `git diff` output back into one text blob per file,
/// each starting with its own `diff --git ...` header line, so the
/// existing single-file `parse_file_diff` can be reused as-is.
fn split_into_file_diffs(raw: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in raw.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_two_file_diffs_apart() {
        let raw = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-a\n+A\ndiff --git a/b.txt b/b.txt\n--- a/b.txt\n+++ b/b.txt\n@@ -1 +1 @@\n-b\n+B\n";
        let chunks = split_into_file_diffs(raw);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].starts_with("diff --git a/a.txt"));
        assert!(chunks[1].starts_with("diff --git a/b.txt"));
    }
}
