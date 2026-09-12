use anyhow::{Result, bail};
use regex::Regex;
use std::sync::LazyLock;

use super::model::{DiffLine, FileDiff, Hunk, LineKind};

static HUNK_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap());

/// Strips a trailing `\r` so diffs from repositories with `core.autocrlf`
/// on Windows are parsed the same way as on Unix.
fn strip_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

fn strip_ab_prefix(path: &str) -> String {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_string()
}

/// Parses the unified diff for a single file, as produced by
/// `git diff -- <path>` or `git diff --no-index -- <null-device> <path>`.
pub fn parse_file_diff(raw: &str) -> Result<FileDiff> {
    let lines: Vec<&str> = raw.lines().map(strip_cr).collect();

    let mut old_path = String::new();
    let mut new_path = String::new();
    let mut is_new_file = false;
    let mut is_deleted_file = false;
    let mut is_binary = false;
    let mut raw_header_lines = Vec::new();
    let mut hunks: Vec<Hunk> = Vec::new();

    let mut i = 0;
    while i < lines.len() && !lines[i].starts_with("@@ -") {
        let line = lines[i];

        if line.starts_with("new file mode") {
            is_new_file = true;
        } else if line.starts_with("deleted file mode") {
            is_deleted_file = true;
        } else if let Some(rest) = line.strip_prefix("--- ") {
            old_path = strip_ab_prefix(rest);
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            new_path = strip_ab_prefix(rest);
        } else if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
            is_binary = true;
        }

        raw_header_lines.push(line.to_string());
        i += 1;
    }

    if is_binary {
        return Ok(FileDiff {
            old_path,
            new_path,
            is_new_file,
            is_deleted_file,
            is_binary,
            hunks: Vec::new(),
            raw_header_lines,
        });
    }

    while i < lines.len() {
        let header = lines[i];
        let Some(caps) = HUNK_HEADER.captures(header) else {
            bail!("expected hunk header, got: {header}");
        };
        let old_start: u32 = caps[1].parse()?;
        let old_lines: u32 = caps.get(2).map_or(Ok(1), |m| m.as_str().parse())?;
        let new_start: u32 = caps[3].parse()?;
        let new_lines: u32 = caps.get(4).map_or(Ok(1), |m| m.as_str().parse())?;
        i += 1;

        let mut old_cursor = old_start;
        let mut new_cursor = new_start;
        let mut diff_lines: Vec<DiffLine> = Vec::new();

        while i < lines.len() && !lines[i].starts_with("@@ -") {
            let raw_line = lines[i];

            if raw_line.starts_with('\\') {
                // "\ No newline at end of file" applies to the previous line.
                if let Some(last) = diff_lines.last_mut() {
                    last.no_newline_at_eof = true;
                }
                i += 1;
                continue;
            }

            let (kind, content) = match raw_line.split_at_checked(1) {
                Some(("+", rest)) => (LineKind::Added, rest),
                Some(("-", rest)) => (LineKind::Removed, rest),
                Some((" ", rest)) => (LineKind::Context, rest),
                _ if raw_line.is_empty() => (LineKind::Context, raw_line),
                _ => bail!("unexpected diff line: {raw_line}"),
            };

            let (old_lineno, new_lineno) = match kind {
                LineKind::Context => {
                    let l = (Some(old_cursor), Some(new_cursor));
                    old_cursor += 1;
                    new_cursor += 1;
                    l
                }
                LineKind::Removed => {
                    let l = (Some(old_cursor), None);
                    old_cursor += 1;
                    l
                }
                LineKind::Added => {
                    let l = (None, Some(new_cursor));
                    new_cursor += 1;
                    l
                }
            };

            diff_lines.push(DiffLine {
                kind,
                content: content.to_string(),
                old_lineno,
                new_lineno,
                no_newline_at_eof: false,
            });
            i += 1;
        }

        hunks.push(Hunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            lines: diff_lines,
        });
    }

    Ok(FileDiff {
        old_path,
        new_path,
        is_new_file,
        is_deleted_file,
        is_binary,
        hunks,
        raw_header_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_hunk() {
        let raw = "diff --git a/f.txt b/f.txt\nindex 111..222 100644\n--- a/f.txt\n+++ b/f.txt\n@@ -1,3 +1,4 @@\n line1\n-line2\n+line2 modified\n+line3\n line4\n";
        let diff = parse_file_diff(raw).unwrap();
        assert_eq!(diff.old_path, "f.txt");
        assert_eq!(diff.new_path, "f.txt");
        assert!(!diff.is_new_file);
        assert_eq!(diff.hunks.len(), 1);
        let hunk = &diff.hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.old_lines, 3);
        assert_eq!(hunk.new_start, 1);
        assert_eq!(hunk.new_lines, 4);
        assert_eq!(hunk.lines.len(), 5);
        assert_eq!(hunk.lines[0].kind, LineKind::Context);
        assert_eq!(hunk.lines[0].old_lineno, Some(1));
        assert_eq!(hunk.lines[0].new_lineno, Some(1));
        assert_eq!(hunk.lines[1].kind, LineKind::Removed);
        assert_eq!(hunk.lines[1].old_lineno, Some(2));
        assert_eq!(hunk.lines[1].new_lineno, None);
        assert_eq!(hunk.lines[2].kind, LineKind::Added);
        assert_eq!(hunk.lines[2].new_lineno, Some(2));
        assert_eq!(hunk.lines[3].kind, LineKind::Added);
        assert_eq!(hunk.lines[3].new_lineno, Some(3));
        assert_eq!(hunk.lines[4].kind, LineKind::Context);
        assert_eq!(hunk.lines[4].old_lineno, Some(3));
        assert_eq!(hunk.lines[4].new_lineno, Some(4));
    }

    #[test]
    fn parses_multiple_hunks() {
        let raw = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-a\n+A\n b\n@@ -10,2 +10,2 @@\n-x\n+X\n y\n";
        let diff = parse_file_diff(raw).unwrap();
        assert_eq!(diff.hunks.len(), 2);
        assert_eq!(diff.hunks[1].old_start, 10);
    }

    #[test]
    fn parses_new_file() {
        let raw = "diff --git a/new.txt b/new.txt\nnew file mode 100644\nindex 000..111\n--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+line1\n+line2\n";
        let diff = parse_file_diff(raw).unwrap();
        assert!(diff.is_new_file);
        assert_eq!(diff.old_path, "/dev/null");
        assert_eq!(diff.new_path, "new.txt");
        assert_eq!(diff.hunks[0].old_start, 0);
        assert_eq!(diff.hunks[0].old_lines, 0);
    }

    #[test]
    fn detects_binary_file() {
        let raw = "diff --git a/img.png b/img.png\nindex 111..222 100644\nBinary files a/img.png and b/img.png differ\n";
        let diff = parse_file_diff(raw).unwrap();
        assert!(diff.is_binary);
        assert!(diff.hunks.is_empty());
    }

    #[test]
    fn handles_no_newline_at_eof_marker() {
        let raw = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n";
        let diff = parse_file_diff(raw).unwrap();
        let hunk = &diff.hunks[0];
        assert_eq!(hunk.lines.len(), 2);
        assert!(hunk.lines[0].no_newline_at_eof);
        assert!(hunk.lines[1].no_newline_at_eof);
    }

    #[test]
    fn implicit_hunk_line_counts_default_to_one() {
        let raw =
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -5 +5,2 @@\n-x\n+y\n+z\n";
        let diff = parse_file_diff(raw).unwrap();
        assert_eq!(diff.hunks[0].old_lines, 1);
        assert_eq!(diff.hunks[0].new_lines, 2);
    }

    #[test]
    fn handles_crlf_line_endings() {
        let raw = "diff --git a/f.txt b/f.txt\r\n--- a/f.txt\r\n+++ b/f.txt\r\n@@ -1,1 +1,1 @@\r\n-old\r\n+new\r\n";
        let diff = parse_file_diff(raw).unwrap();
        assert_eq!(diff.hunks[0].lines.len(), 2);
        assert_eq!(diff.hunks[0].lines[1].content, "new");
    }

    #[test]
    fn empty_diff_has_no_hunks() {
        let diff = parse_file_diff("").unwrap();
        assert!(diff.hunks.is_empty());
        assert!(!diff.is_binary);
    }
}
