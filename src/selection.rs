use crate::diff::{FileDiff, LineKind};

/// Mirrors `git add -p`'s per-hunk decision. Purely informational for the
/// UI (checkbox glyph, `J`/`K` navigation) — staging truth always comes from
/// `line_selected`; `Undecided` and `Rejected` are equivalent there (both
/// mean "nothing from this hunk is staged").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkDecision {
    Undecided,
    Accepted,
    Rejected,
    Partial,
}

#[derive(Debug, Clone)]
pub struct HunkSelection {
    pub hunk_index: usize,
    pub decision: HunkDecision,
    /// Aligned with `Hunk::lines`; only meaningful for Added/Removed lines.
    pub line_selected: Vec<bool>,
    /// Aligned with `Hunk::lines`; `true` for Context lines, which are
    /// never toggled and always sit at `false` in `line_selected` — they
    /// must be excluded when `toggle_line` re-derives `decision`, otherwise
    /// a hunk with any context line could never reach `Accepted` (the
    /// context slot's permanent `false` would always break the `all(true)`
    /// check, stranding the header's checkbox on `Partial` forever).
    is_context: Vec<bool>,
}

#[derive(Debug, Clone)]
pub struct FileSelection {
    pub file: String,
    pub hunks: Vec<HunkSelection>,
}

impl FileSelection {
    /// Builds a selection with every hunk undecided, the default state when
    /// a file is opened for the first time — nothing is staged until the
    /// user explicitly accepts or rejects each hunk (git add -p-style).
    pub fn all_undecided(file: &str, diff: &FileDiff) -> Self {
        let hunks = diff
            .hunks
            .iter()
            .enumerate()
            .map(|(hunk_index, hunk)| HunkSelection {
                hunk_index,
                decision: HunkDecision::Undecided,
                line_selected: vec![false; hunk.lines.len()],
                is_context: hunk
                    .lines
                    .iter()
                    .map(|l| l.kind == LineKind::Context)
                    .collect(),
            })
            .collect();

        FileSelection {
            file: file.to_string(),
            hunks,
        }
    }

    pub fn accept_hunk(&mut self, hunk_index: usize) {
        let Some(hunk) = self.hunks.get_mut(hunk_index) else {
            return;
        };
        hunk.decision = HunkDecision::Accepted;
        hunk.line_selected.fill(true);
    }

    pub fn reject_hunk(&mut self, hunk_index: usize) {
        let Some(hunk) = self.hunks.get_mut(hunk_index) else {
            return;
        };
        hunk.decision = HunkDecision::Rejected;
        hunk.line_selected.fill(false);
    }

    pub fn accept_hunk_and_rest(&mut self, from_hunk_index: usize) {
        for idx in from_hunk_index..self.hunks.len() {
            self.accept_hunk(idx);
        }
    }

    pub fn reject_hunk_and_rest(&mut self, from_hunk_index: usize) {
        for idx in from_hunk_index..self.hunks.len() {
            self.reject_hunk(idx);
        }
    }

    /// Kept for the free line-level toggle (Space on a line row). Flips a
    /// single Added/Removed line and re-derives the hunk's `decision`.
    pub fn toggle_line(&mut self, hunk_index: usize, line_index: usize) {
        let Some(hunk) = self.hunks.get_mut(hunk_index) else {
            return;
        };
        let Some(line) = hunk.line_selected.get_mut(line_index) else {
            return;
        };
        *line = !*line;

        let (mut any_selected, mut any_unselected) = (false, false);
        for (&selected, &is_context) in hunk.line_selected.iter().zip(&hunk.is_context) {
            if is_context {
                continue;
            }
            if selected {
                any_selected = true;
            } else {
                any_unselected = true;
            }
        }
        hunk.decision = match (any_selected, any_unselected) {
            (true, false) => HunkDecision::Accepted,
            // Nothing selected: this line-level toggle undid its own work
            // back to the starting point, not an explicit "no" — leave it
            // Undecided rather than Rejected, so it still reads as "not
            // decided yet" (e.g. in the Files pane's decided/total count)
            // instead of looking permanently "reviewed". An explicit
            // reject via `n`/`d`/`reject_hunk` still sets Rejected.
            (false, _) => HunkDecision::Undecided,
            _ => HunkDecision::Partial,
        };
    }

    /// Toggles a whole hunk between Accepted and Rejected (Space/Enter on a
    /// hunk header row), for callers that don't need the explicit
    /// accept/reject-and-advance semantics of the guided `y`/`n` flow.
    pub fn toggle_hunk(&mut self, hunk_index: usize) {
        let Some(hunk) = self.hunks.get(hunk_index) else {
            return;
        };
        if hunk.decision == HunkDecision::Accepted {
            self.reject_hunk(hunk_index);
        } else {
            self.accept_hunk(hunk_index);
        }
    }

    /// Finds the next undecided hunk strictly after `from`, wrapping around.
    /// Returns `None` if every hunk is decided.
    pub fn next_undecided_hunk(&self, from: usize) -> Option<usize> {
        let len = self.hunks.len();
        if len == 0 {
            return None;
        }
        (1..=len)
            .map(|offset| (from + offset) % len)
            .find(|&idx| self.hunks[idx].decision == HunkDecision::Undecided)
    }

    /// Finds the previous undecided hunk strictly before `from`, wrapping
    /// around. Returns `None` if every hunk is decided.
    pub fn previous_undecided_hunk(&self, from: usize) -> Option<usize> {
        let len = self.hunks.len();
        if len == 0 {
            return None;
        }
        (1..=len)
            .map(|offset| (from + len - offset) % len)
            .find(|&idx| self.hunks[idx].decision == HunkDecision::Undecided)
    }

    /// True once every hunk has been explicitly decided (Accepted, Rejected
    /// or Partial) — drives the guided review's auto-advance to the next
    /// file.
    pub fn all_decided(&self) -> bool {
        self.hunks
            .iter()
            .all(|h| h.decision != HunkDecision::Undecided)
    }

    /// `(decided, total)` hunk counts, for the Files pane's progress
    /// indicator — a hunk is "decided" once it's Accepted, Rejected or
    /// Partial (anything but Undecided).
    pub fn progress(&self) -> (usize, usize) {
        let decided = self
            .hunks
            .iter()
            .filter(|h| h.decision != HunkDecision::Undecided)
            .count();
        (decided, self.hunks.len())
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

    fn two_hunk_diff() -> FileDiff {
        let raw = "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-a\n+A\n b\n@@ -10,2 +10,2 @@\n-x\n+X\n y\n";
        parse_file_diff(raw).unwrap()
    }

    #[test]
    fn starts_all_undecided() {
        let diff = sample_diff();
        let sel = FileSelection::all_undecided("f.txt", &diff);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Undecided);
        assert!(sel.hunks[0].line_selected.iter().all(|&s| !s));
        assert!(!sel.all_decided());
    }

    #[test]
    fn accept_hunk_selects_all_its_lines() {
        let diff = sample_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        sel.accept_hunk(0);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Accepted);
        assert!(sel.hunks[0].line_selected.iter().all(|&s| s));
        assert!(sel.all_decided());
    }

    #[test]
    fn reject_hunk_deselects_all_its_lines_but_counts_as_decided() {
        let diff = sample_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        sel.reject_hunk(0);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Rejected);
        assert!(sel.hunks[0].line_selected.iter().all(|&s| !s));
        assert!(sel.all_decided());
    }

    #[test]
    fn accept_hunk_and_rest_covers_all_following_hunks() {
        let diff = two_hunk_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        sel.accept_hunk_and_rest(0);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Accepted);
        assert_eq!(sel.hunks[1].decision, HunkDecision::Accepted);
        assert!(sel.all_decided());
    }

    #[test]
    fn progress_counts_decided_hunks_out_of_total() {
        let diff = two_hunk_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        assert_eq!(sel.progress(), (0, 2));
        sel.accept_hunk(0);
        assert_eq!(sel.progress(), (1, 2));
        sel.reject_hunk(1);
        assert_eq!(sel.progress(), (2, 2));
    }

    #[test]
    fn toggling_one_line_marks_hunk_as_partial() {
        let diff = sample_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        sel.toggle_line(0, 1); // select the "+A" line only
        assert_eq!(sel.hunks[0].decision, HunkDecision::Partial);
        assert!(sel.hunks[0].line_selected[1]);
        assert!(!sel.hunks[0].line_selected[0]);
    }

    #[test]
    fn toggling_every_non_context_line_on_reaches_accepted() {
        // Regression: the header checkbox used to be stuck on Partial
        // forever whenever a hunk had any context line, because its
        // permanently-false `line_selected` slot broke the `all(true)`
        // check that decided "Accepted".
        let diff = sample_diff(); // lines: 0 "-a", 1 "+A", 2 " b" (context)
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        sel.toggle_line(0, 0);
        sel.toggle_line(0, 1);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Accepted);
    }

    #[test]
    fn toggling_every_non_context_line_back_off_reaches_undecided_not_rejected() {
        // Unchecking every line via the free line-level toggle undoes the
        // hunk's own work, landing back at "not decided" — distinct from an
        // explicit `n`/`d` reject, which stays Rejected (see
        // `explicit_reject_hunk_is_not_undone_by_re_deciding_nothing`-style
        // callers like `reject_hunk`).
        let diff = sample_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        sel.toggle_line(0, 0);
        sel.toggle_line(0, 1);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Accepted);
        sel.toggle_line(0, 0);
        sel.toggle_line(0, 1);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Undecided);
    }

    #[test]
    fn explicit_reject_hunk_still_counts_as_decided() {
        let diff = sample_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        sel.reject_hunk(0);
        assert_eq!(sel.hunks[0].decision, HunkDecision::Rejected);
        assert_eq!(sel.progress(), (1, 1));
    }

    #[test]
    fn next_undecided_hunk_skips_decided_ones_and_wraps() {
        let diff = two_hunk_diff();
        let mut sel = FileSelection::all_undecided("f.txt", &diff);
        assert_eq!(sel.next_undecided_hunk(0), Some(1));
        sel.accept_hunk(1);
        assert_eq!(sel.next_undecided_hunk(1), Some(0));
        sel.accept_hunk(0);
        assert_eq!(sel.next_undecided_hunk(0), None);
    }

    #[test]
    fn previous_undecided_hunk_wraps_backward() {
        let diff = two_hunk_diff();
        let sel = FileSelection::all_undecided("f.txt", &diff);
        assert_eq!(sel.previous_undecided_hunk(0), Some(1));
        assert_eq!(sel.previous_undecided_hunk(1), Some(0));
    }
}
