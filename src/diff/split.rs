use super::model::{DiffLine, Hunk, LineKind};

/// Splits a hunk containing multiple change blocks into smaller hunks, one
/// per block, mirroring `git add -p`'s `s` key. Returns `None` when the
/// hunk has fewer than two change blocks, or when no two consecutive blocks
/// are separated by enough context to give each side up to `context_lines`
/// of its own without overlapping (gap >= 2 * context_lines).
pub fn split_hunk(hunk: &Hunk, context_lines: u32) -> Option<Vec<Hunk>> {
    let context_lines = context_lines as usize;
    let runs = change_runs(hunk);
    if runs.len() < 2 {
        return None;
    }

    let split_points: Vec<usize> = (0..runs.len() - 1)
        .filter(|&i| runs[i + 1].0 - runs[i].1 >= 2 * context_lines)
        .collect();
    if split_points.is_empty() {
        return None;
    }

    let mut segments = Vec::new();
    let mut segment_start = 0usize;
    for &i in &split_points {
        let segment_end = runs[i].1 + context_lines;
        segments.push((segment_start, segment_end));
        segment_start = runs[i + 1].0 - context_lines;
    }
    segments.push((segment_start, hunk.lines.len()));

    Some(
        segments
            .into_iter()
            .enumerate()
            .map(|(idx, (start, end))| build_sub_hunk(hunk, idx == 0, &hunk.lines[start..end]))
            .collect(),
    )
}

/// Maximal contiguous ranges of non-context lines, as `[start, end)` index
/// pairs into `hunk.lines`.
fn change_runs(hunk: &Hunk) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < hunk.lines.len() {
        if hunk.lines[i].kind == LineKind::Context {
            i += 1;
            continue;
        }
        let start = i;
        while i < hunk.lines.len() && hunk.lines[i].kind != LineKind::Context {
            i += 1;
        }
        runs.push((start, i));
    }
    runs
}

fn build_sub_hunk(original: &Hunk, is_first_segment: bool, lines: &[DiffLine]) -> Hunk {
    // Every segment but the first starts on a context line (the split point
    // sits inside a run of context lines), which always carries both line
    // numbers. The first segment starts wherever the original hunk did.
    let (old_start, new_start) = if is_first_segment {
        (original.old_start, original.new_start)
    } else {
        let first = &lines[0];
        (
            first
                .old_lineno
                .expect("split segment must start on a context line"),
            first
                .new_lineno
                .expect("split segment must start on a context line"),
        )
    };

    let old_lines = lines.iter().filter(|l| l.kind != LineKind::Added).count() as u32;
    let new_lines = lines.iter().filter(|l| l.kind != LineKind::Removed).count() as u32;

    Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines: lines.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parser::parse_file_diff;

    fn hunk_with_gap(gap_lines: usize) -> Hunk {
        let context: String = (0..gap_lines).map(|i| format!(" ctx{i}\n")).collect();
        let raw = format!(
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,{} +1,{} @@\n-a\n+A\n{context}-z\n+Z\n",
            gap_lines + 2,
            gap_lines + 2
        );
        parse_file_diff(&raw)
            .unwrap()
            .hunks
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn splits_when_gap_is_wide_enough() {
        let hunk = hunk_with_gap(10);
        let sub_hunks = split_hunk(&hunk, 2).expect("gap of 10 >= 2*2 should split");
        assert_eq!(sub_hunks.len(), 2);
        assert_eq!(sub_hunks[0].old_start, hunk.old_start);
        assert!(
            sub_hunks[0]
                .lines
                .iter()
                .any(|l| l.content == "a" || l.content == "A")
        );
        assert!(
            sub_hunks[1]
                .lines
                .iter()
                .any(|l| l.content == "z" || l.content == "Z")
        );
    }

    #[test]
    fn does_not_split_when_gap_is_too_narrow() {
        let hunk = hunk_with_gap(2);
        assert!(split_hunk(&hunk, 3).is_none());
    }

    #[test]
    fn single_change_block_is_not_splittable() {
        let raw =
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-a\n+A\n b\n";
        let hunk = parse_file_diff(raw)
            .unwrap()
            .hunks
            .into_iter()
            .next()
            .unwrap();
        assert!(split_hunk(&hunk, 1).is_none());
    }

    #[test]
    fn sub_hunk_line_counts_are_correct() {
        let hunk = hunk_with_gap(10);
        let sub_hunks = split_hunk(&hunk, 2).unwrap();
        // First sub-hunk: "-a", "+a", 2 lines of trailing context.
        assert_eq!(sub_hunks[0].old_lines, 3);
        assert_eq!(sub_hunks[0].new_lines, 3);
        // Second sub-hunk: 2 lines of leading context, "-z", "+Z".
        assert_eq!(sub_hunks[1].old_lines, 3);
        assert_eq!(sub_hunks[1].new_lines, 3);
    }
}
