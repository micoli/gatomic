use crate::diff::FileDiff;

#[derive(Debug, Clone)]
pub struct HunkSelection {
    pub hunk_index: usize,
    pub selected: bool,
    /// Aligned with `Hunk::lines`; only meaningful for Added/Removed lines.
    pub line_selected: Vec<bool>,
}

#[derive(Debug, Clone)]
pub struct FileSelection {
    pub file: String,
    pub hunks: Vec<HunkSelection>,
}

impl FileSelection {
    /// Builds a selection with every hunk/line selected, the default state
    /// when a file is opened for the first time.
    pub fn all_selected(file: &str, diff: &FileDiff) -> Self {
        let hunks = diff
            .hunks
            .iter()
            .enumerate()
            .map(|(hunk_index, hunk)| HunkSelection {
                hunk_index,
                selected: true,
                line_selected: vec![true; hunk.lines.len()],
            })
            .collect();

        FileSelection {
            file: file.to_string(),
            hunks,
        }
    }

    pub fn toggle_hunk(&mut self, hunk_index: usize) {
        let Some(hunk) = self.hunks.get_mut(hunk_index) else {
            return;
        };
        hunk.selected = !hunk.selected;
        hunk.line_selected.fill(hunk.selected);
    }

    pub fn toggle_line(&mut self, hunk_index: usize, line_index: usize) {
        let Some(hunk) = self.hunks.get_mut(hunk_index) else {
            return;
        };
        let Some(line) = hunk.line_selected.get_mut(line_index) else {
            return;
        };
        *line = !*line;
        hunk.selected = hunk.line_selected.iter().all(|&selected| selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::parse_file_diff;

    fn sample_diff() -> FileDiff {
        let raw =
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-a\n+A\n b\n";
        parse_file_diff(raw).unwrap()
    }

    #[test]
    fn starts_fully_selected() {
        let diff = sample_diff();
        let sel = FileSelection::all_selected("f.txt", &diff);
        assert!(sel.hunks[0].selected);
        assert!(sel.hunks[0].line_selected.iter().all(|&s| s));
    }

    #[test]
    fn toggling_hunk_clears_all_lines() {
        let diff = sample_diff();
        let mut sel = FileSelection::all_selected("f.txt", &diff);
        sel.toggle_hunk(0);
        assert!(!sel.hunks[0].selected);
        assert!(sel.hunks[0].line_selected.iter().all(|&s| !s));
    }

    #[test]
    fn toggling_one_line_marks_hunk_as_partial() {
        let diff = sample_diff();
        let mut sel = FileSelection::all_selected("f.txt", &diff);
        sel.toggle_line(0, 0);
        assert!(!sel.hunks[0].selected);
        assert!(!sel.hunks[0].line_selected[0]);
        assert!(sel.hunks[0].line_selected[1]);
    }
}
