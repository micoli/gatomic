use anyhow::Result;

use super::model::{FileDiff, LineKind};
use crate::git::{run_git_with_stdin, unstage_path};
use crate::selection::FileSelection;

/// Builds a patch text containing only the selected hunks/lines, suitable
/// for `git apply --cached`. Returns `None` when nothing is selected.
///
/// `old_start` of a retained hunk never changes (it is an absolute position
/// in the current working file), but `new_start` must be shifted by the net
/// line-count delta introduced by every *retained* hunk before it, since
/// hunks that are skipped never change the resulting index content.
pub fn build_patch(diff: &FileDiff, selection: &FileSelection) -> Option<String> {
    let mut body = String::new();
    let mut new_offset: i64 = 0;
    let mut any_hunk_emitted = false;

    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        let hunk_selection = selection.hunks.get(hunk_index);
        let line_selected = |i: usize| {
            hunk_selection
                .and_then(|s| s.line_selected.get(i))
                .copied()
                .unwrap_or(false)
        };

        let mut emitted_lines = String::new();
        let mut old_count: u32 = 0;
        let mut new_count: u32 = 0;
        let mut net_delta: i64 = 0;
        let mut has_content = false;

        for (line_index, line) in hunk.lines.iter().enumerate() {
            match line.kind {
                LineKind::Context => {
                    emitted_lines.push(' ');
                    emitted_lines.push_str(&line.content);
                    push_no_newline_marker(&mut emitted_lines, line);
                    old_count += 1;
                    new_count += 1;
                }
                LineKind::Added => {
                    if line_selected(line_index) {
                        emitted_lines.push('+');
                        emitted_lines.push_str(&line.content);
                        push_no_newline_marker(&mut emitted_lines, line);
                        new_count += 1;
                        net_delta += 1;
                        has_content = true;
                    }
                }
                LineKind::Removed => {
                    if line_selected(line_index) {
                        emitted_lines.push('-');
                        emitted_lines.push_str(&line.content);
                        push_no_newline_marker(&mut emitted_lines, line);
                        old_count += 1;
                        net_delta -= 1;
                        has_content = true;
                    } else {
                        // Not retained: this removal does not happen in this
                        // commit, so the line stays as context on both sides.
                        emitted_lines.push(' ');
                        emitted_lines.push_str(&line.content);
                        push_no_newline_marker(&mut emitted_lines, line);
                        old_count += 1;
                        new_count += 1;
                    }
                }
            }
        }

        if !has_content {
            continue;
        }

        // Unified diff convention: a hunk with zero old-side lines (a pure
        // insertion, old_start == 0 for a brand new file) anchors new_start
        // one line after old_start rather than at the same position.
        let anchor = if old_count == 0 {
            hunk.old_start as i64 + 1
        } else {
            hunk.old_start as i64
        };
        let emitted_new_start = (anchor + new_offset).max(0) as u32;
        body.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, old_count, emitted_new_start, new_count
        ));
        body.push_str(&emitted_lines);
        new_offset += net_delta;
        any_hunk_emitted = true;
    }

    if !any_hunk_emitted {
        return None;
    }

    let mut patch = diff.raw_header_lines.join("\n");
    patch.push('\n');
    patch.push_str(&body);
    Some(patch)
}

fn push_no_newline_marker(buf: &mut String, line: &super::model::DiffLine) {
    buf.push('\n');
    if line.no_newline_at_eof {
        buf.push_str("\\ No newline at end of file\n");
    }
}

/// Resets the file's staged state to HEAD then re-applies the current
/// selection from scratch, so every toggle is idempotent regardless of the
/// toggle history (immediate staging, `git add -p`-style).
pub fn apply_selection(path: &str, diff: &FileDiff, selection: &FileSelection) -> Result<()> {
    unstage_path(path)?;

    let Some(patch) = build_patch(diff, selection) else {
        return Ok(());
    };

    run_git_with_stdin(&["apply", "--cached", "--recount", "-"], &patch)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse_file_diff;

    fn diff_with_three_hunks() -> FileDiff {
        let raw = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-a\n+A\n b\n@@ -10,2 +10,3 @@\n x\n+y\n z\n@@ -20,2 +21,2 @@\n-m\n+M\n n\n";
        parse_file_diff(raw).unwrap()
    }

    fn all_accepted(file: &str, diff: &FileDiff) -> FileSelection {
        let mut selection = FileSelection::all_undecided(file, diff);
        selection.accept_hunk_and_rest(0);
        selection
    }

    #[test]
    fn fully_selected_patch_matches_original_hunks() {
        let diff = diff_with_three_hunks();
        let selection = all_accepted("f.txt", &diff);
        let patch = build_patch(&diff, &selection).unwrap();
        assert!(patch.contains("@@ -1,2 +1,2 @@"));
        assert!(patch.contains("@@ -10,2 +10,3 @@"));
        assert!(patch.contains("@@ -20,2 +21,2 @@"));
    }

    #[test]
    fn dropping_a_net_zero_hunk_does_not_shift_others() {
        let diff = diff_with_three_hunks();
        let mut selection = all_accepted("f.txt", &diff);
        selection.reject_hunk(0); // hunk 1 has net delta 0 (one removal, one addition)
        let patch = build_patch(&diff, &selection).unwrap();
        // First hunk skipped entirely.
        assert!(!patch.contains("@@ -1,2"));
        // Hunks 2 and 3 keep their original offsets since hunk 1 contributed none.
        assert!(patch.contains("@@ -10,2 +10,3 @@"));
        assert!(patch.contains("@@ -20,2 +21,2 @@"));
    }

    #[test]
    fn skipping_middle_hunk_shifts_last_hunk_offset() {
        let diff = diff_with_three_hunks();
        let mut selection = all_accepted("f.txt", &diff);
        selection.reject_hunk(1); // drop the +1 net-delta hunk
        let patch = build_patch(&diff, &selection).unwrap();
        assert!(patch.contains("@@ -1,2 +1,2 @@"));
        assert!(!patch.contains("@@ -10,2"));
        // Without hunk 2's +1 contribution, hunk 3's new_start goes back to 20.
        assert!(patch.contains("@@ -20,2 +20,2 @@"));
    }

    #[test]
    fn deselecting_removed_line_retrogrades_it_to_context() {
        let raw =
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-a\n+A\n b\n";
        let diff = parse_file_diff(raw).unwrap();
        let mut selection = all_accepted("f.txt", &diff);
        selection.toggle_line(0, 0); // deselect the removal of "a"
        let patch = build_patch(&diff, &selection).unwrap();
        assert!(patch.contains(" a\n"));
        assert!(!patch.contains("-a\n"));
    }

    #[test]
    fn nothing_selected_produces_no_patch() {
        let diff = diff_with_three_hunks();
        let selection = FileSelection::all_undecided("f.txt", &diff);
        assert!(build_patch(&diff, &selection).is_none());
    }

    #[test]
    fn new_file_partial_selection_keeps_zero_old_start() {
        let raw = "diff --git a/new.txt b/new.txt\nnew file mode 100644\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+line1\n+line2\n";
        let diff = parse_file_diff(raw).unwrap();
        let mut selection = all_accepted("new.txt", &diff);
        selection.toggle_line(0, 1); // drop line2
        let patch = build_patch(&diff, &selection).unwrap();
        assert!(patch.contains("@@ -0,0 +1,1 @@"));
        assert!(patch.contains("+line1"));
        assert!(!patch.contains("+line2"));
    }
}
