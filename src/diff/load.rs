use anyhow::Result;

use super::model::FileDiff;
use super::parser::parse_file_diff;
use crate::git::{FileStatusKind, NULL_DEVICE, run_git, run_git_allowing};

/// Loads the unstaged diff for one file. Untracked files go through
/// `git diff --no-index` against the platform's null device, which produces
/// a standard "new file" diff — it is then parsed and patched exactly like
/// any other new file, with no special-cased staging path.
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
        run_git(&["diff", "--no-color", &context_arg, "--", path])?
    };

    parse_file_diff(&raw)
}
