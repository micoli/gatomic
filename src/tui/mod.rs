mod commit_show_pane;
mod commits_pane;
mod files_pane;
mod hunk_pane;
mod layout;
mod text_pane;
mod theme;
mod triage_screen;

use std::collections::{HashMap, HashSet};
use std::io;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    KeyboardEnhancementFlags, MouseButton, MouseEvent, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
    supports_keyboard_enhancement,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::widgets::ListState;

use crate::cli::Cli;
use crate::diff::{FileDiff, apply_selection, count_hunks_per_file, load_file_diff, split_hunk};
use crate::fixup::{build_files_by_commit, match_files_to_commits};
use crate::git::{
    CommitInfo, FileEntry, commit_fixup, commit_template, commit_with_message,
    commits_of_current_branch, last_n_commits, list_file_entries, numstat_against_head, stage_path,
    staged_diff,
};
use crate::i18n::{Lang, Strings};
use crate::selection::FileSelection;
use hunk_pane::HunkRow;
use triage_screen::TriageState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Files,
    Hunks,
    Commits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Triage,
    Review,
}

pub struct App {
    context_lines: u32,
    last_commits: Option<usize>,

    screen: Screen,
    triage: Option<TriageState>,
    show_help: bool,

    files: Vec<FileEntry>,
    files_state: ListState,
    file_stats: HashMap<String, (u32, u32)>,
    hunk_counts: HashMap<String, usize>,

    commits: Vec<CommitInfo>,
    commits_state: ListState,
    commit_show_scroll: u16,
    files_by_commit: HashMap<String, HashSet<String>>,

    pane: Pane,

    current_file_path: Option<String>,
    current_file_diff: Option<FileDiff>,
    hunk_rows: Vec<HunkRow>,
    hunk_state: ListState,
    selections: HashMap<String, FileSelection>,

    commit_form: Option<CommitFormState>,

    lang: Lang,
    status: Option<String>,
    should_quit: bool,
}

struct CommitFormState {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    diff: String,
}

impl CommitFormState {
    fn new(template: &str, diff: String) -> Self {
        let lines = if template.is_empty() {
            vec![String::new()]
        } else {
            template.lines().map(str::to_string).collect()
        };
        CommitFormState {
            lines,
            cursor_line: 0,
            cursor_col: 0,
            diff,
        }
    }

    fn message(&self) -> String {
        self.lines.join("\n")
    }

    fn current_line_len(&self) -> usize {
        self.lines[self.cursor_line].chars().count()
    }

    fn insert_char(&mut self, c: char) {
        let byte_idx = char_to_byte_index(&self.lines[self.cursor_line], self.cursor_col);
        self.lines[self.cursor_line].insert(byte_idx, c);
        self.cursor_col += 1;
    }

    fn insert_newline(&mut self) {
        let byte_idx = char_to_byte_index(&self.lines[self.cursor_line], self.cursor_col);
        let tail = self.lines[self.cursor_line].split_off(byte_idx);
        self.lines.insert(self.cursor_line + 1, tail);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let start = char_to_byte_index(line, self.cursor_col - 1);
            let end = char_to_byte_index(line, self.cursor_col);
            line.replace_range(start..end, "");
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            let current = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col = self.current_line_len();
            self.lines[self.cursor_line].push_str(&current);
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.current_line_len();
        }
    }

    fn move_right(&mut self) {
        if self.cursor_col < self.current_line_len() {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.cursor_col.min(self.current_line_len());
        }
    }

    fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = self.cursor_col.min(self.current_line_len());
        }
    }

    fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    fn move_end(&mut self) {
        self.cursor_col = self.current_line_len();
    }
}

fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

impl App {
    pub fn new(cli: &Cli) -> Result<Self> {
        let mut app = App {
            context_lines: cli.context_lines,
            last_commits: cli.last_commits,
            screen: Screen::Review,
            triage: None,
            show_help: false,
            files: Vec::new(),
            files_state: ListState::default(),
            file_stats: HashMap::new(),
            hunk_counts: HashMap::new(),
            commits: Vec::new(),
            commits_state: ListState::default(),
            commit_show_scroll: 0,
            files_by_commit: HashMap::new(),
            lang: Lang::default(),
            pane: Pane::Files,
            current_file_path: None,
            current_file_diff: None,
            hunk_rows: Vec::new(),
            hunk_state: ListState::default(),
            selections: HashMap::new(),
            commit_form: None,
            status: None,
            should_quit: false,
        };
        app.refresh_files()?;
        app.refresh_commits()?;
        app.load_selected_file_diff()?;
        app.open_triage_if_evident_matches_exist()?;
        Ok(app)
    }

    fn refresh_files(&mut self) -> Result<()> {
        let previously_selected = self
            .files_state
            .selected()
            .and_then(|i| self.files.get(i))
            .map(|f| f.path.clone());

        self.files = list_file_entries()?;
        self.file_stats = numstat_against_head().unwrap_or_default();
        self.hunk_counts = count_hunks_per_file(self.context_lines).unwrap_or_default();

        let restored_index = previously_selected
            .and_then(|path| self.files.iter().position(|f| f.path == path))
            .or(if self.files.is_empty() { None } else { Some(0) });
        self.files_state.select(restored_index);
        Ok(())
    }

    fn refresh_commits(&mut self) -> Result<()> {
        self.commits = match self.last_commits {
            Some(n) => last_n_commits(n)?,
            None => commits_of_current_branch()?,
        };
        if !self.commits.is_empty() && self.commits_state.selected().is_none() {
            self.commits_state.select(Some(0));
        }
        self.files_by_commit = build_files_by_commit(&self.commits)?;
        Ok(())
    }

    pub(crate) fn strings(&self) -> &'static Strings {
        self.lang.strings()
    }

    fn selected_file(&self) -> Option<&FileEntry> {
        self.files_state.selected().and_then(|i| self.files.get(i))
    }

    fn load_selected_file_diff(&mut self) -> Result<()> {
        let Some(entry) = self.selected_file() else {
            self.current_file_path = None;
            self.current_file_diff = None;
            self.hunk_rows.clear();
            return Ok(());
        };

        let path = entry.path.clone();
        let diff = load_file_diff(&path, &entry.kind, self.context_lines)?;
        self.hunk_rows = hunk_pane::build_rows(&diff);
        self.selections
            .entry(path.clone())
            .or_insert_with(|| FileSelection::all_undecided(&path, &diff));
        self.current_file_diff = Some(diff);
        self.current_file_path = Some(path);
        self.hunk_state.select(if self.hunk_rows.is_empty() {
            None
        } else {
            Some(0)
        });
        Ok(())
    }

    // --- Triage screen -----------------------------------------------

    fn open_triage_if_evident_matches_exist(&mut self) -> Result<()> {
        let matches = match_files_to_commits(&self.files, &self.commits, &self.files_by_commit);
        let evident: Vec<_> = matches.into_iter().filter(|m| m.is_evident()).collect();
        if evident.is_empty() {
            self.screen = Screen::Review;
            self.triage = None;
        } else {
            self.triage = Some(TriageState::new(evident));
            self.screen = Screen::Triage;
        }
        Ok(())
    }

    fn triage_accept(&mut self) -> Result<()> {
        let Some(triage) = &self.triage else {
            return Ok(());
        };
        let groups = triage.accepted_groups();
        for (sha, paths) in &groups {
            for path in paths {
                stage_path(path)?;
            }
            commit_fixup(sha)?;
        }
        self.status = Some(
            self.strings()
                .status_fixup_created
                .replace("{n}", &groups.len().to_string()),
        );

        self.selections.clear();
        self.current_file_diff = None;
        self.current_file_path = None;
        self.hunk_rows.clear();

        self.refresh_files()?;
        self.refresh_commits()?;
        self.load_selected_file_diff()?;
        self.screen = Screen::Review;
        self.triage = None;
        Ok(())
    }

    fn triage_skip(&mut self) {
        self.screen = Screen::Review;
        self.triage = None;
    }

    fn handle_triage_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(t) = &mut self.triage {
                    t.toggle_focus();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(t) = &mut self.triage {
                    t.scroll_focused_pane_down();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(t) = &mut self.triage {
                    t.scroll_focused_pane_up();
                }
            }
            KeyCode::Char(' ') => {
                if let Some(t) = &mut self.triage {
                    t.toggle_current();
                }
            }
            KeyCode::Char('a') => self.triage_accept()?,
            KeyCode::Enter | KeyCode::Char('d') | KeyCode::Char('t') | KeyCode::Esc => {
                self.triage_skip()
            }
            _ => {}
        }
        Ok(())
    }

    // --- Review screen: pane navigation -------------------------------

    fn next_in(state: &mut ListState, len: usize) {
        if len == 0 {
            return;
        }
        let next = state.selected().map_or(0, |i| (i + 1).min(len - 1));
        state.select(Some(next));
    }

    fn prev_in(state: &mut ListState, len: usize) {
        if len == 0 {
            return;
        }
        let prev = state.selected().map_or(0, |i| i.saturating_sub(1));
        state.select(Some(prev));
    }

    fn move_down(&mut self) -> Result<()> {
        match self.pane {
            Pane::Files => {
                Self::next_in(&mut self.files_state, self.files.len());
                self.load_selected_file_diff()?;
            }
            Pane::Hunks => Self::next_in(&mut self.hunk_state, self.hunk_rows.len()),
            Pane::Commits => {
                Self::next_in(&mut self.commits_state, self.commits.len());
                self.commit_show_scroll = 0;
            }
        }
        Ok(())
    }

    fn move_up(&mut self) -> Result<()> {
        match self.pane {
            Pane::Files => {
                Self::prev_in(&mut self.files_state, self.files.len());
                self.load_selected_file_diff()?;
            }
            Pane::Hunks => Self::prev_in(&mut self.hunk_state, self.hunk_rows.len()),
            Pane::Commits => {
                Self::prev_in(&mut self.commits_state, self.commits.len());
                self.commit_show_scroll = 0;
            }
        }
        Ok(())
    }

    fn cycle_pane(&mut self, forward: bool) {
        self.pane = match (self.pane, forward) {
            (Pane::Files, true) => Pane::Hunks,
            (Pane::Hunks, true) => Pane::Commits,
            (Pane::Commits, true) => Pane::Files,
            (Pane::Files, false) => Pane::Commits,
            (Pane::Hunks, false) => Pane::Files,
            (Pane::Commits, false) => Pane::Hunks,
        };
    }

    // --- Hunks pane: free line-level toggle ---------------------------

    /// Toggles the hunk/line under the cursor and immediately re-stages the
    /// file to reflect the new selection (git add -p-style immediate apply).
    fn toggle_current_row(&mut self) -> Result<()> {
        if self.pane != Pane::Hunks {
            return Ok(());
        }
        let (Some(row_index), Some(path), Some(diff)) = (
            self.hunk_state.selected(),
            self.current_file_path.clone(),
            self.current_file_diff.as_ref(),
        ) else {
            return Ok(());
        };
        let Some(row) = self.hunk_rows.get(row_index).copied() else {
            return Ok(());
        };

        let selection = self
            .selections
            .entry(path.clone())
            .or_insert_with(|| FileSelection::all_undecided(&path, diff));

        let toggled_line = match row {
            HunkRow::Header(hunk_index) => {
                selection.toggle_hunk(hunk_index);
                false
            }
            HunkRow::Line(hunk_index, line_index) => {
                let is_context =
                    diff.hunks[hunk_index].lines[line_index].kind == crate::diff::LineKind::Context;
                if is_context {
                    return Ok(());
                }
                selection.toggle_line(hunk_index, line_index);
                true
            }
        };

        apply_selection(&path, diff, selection)?;
        self.refresh_files()?;
        self.status = Some(format!("Staged selection updated for {path}"));

        // Toggling a single line is a repeated action (review line by line),
        // so advance the cursor automatically; a hunk header toggle stays
        // put since it already covers the whole hunk.
        if toggled_line {
            Self::next_in(&mut self.hunk_state, self.hunk_rows.len());
        }
        Ok(())
    }

    // --- Hunks pane: guided `git add -p`-style review -----------------

    fn current_hunk_index(&self) -> Option<usize> {
        self.hunk_state
            .selected()
            .and_then(|i| self.hunk_rows.get(i))
            .map(|row| row.hunk_index())
    }

    fn select_hunk_header_row(&mut self, hunk_index: usize) {
        if let Some(row_index) = self
            .hunk_rows
            .iter()
            .position(|row| matches!(row, HunkRow::Header(h) if *h == hunk_index))
        {
            self.hunk_state.select(Some(row_index));
        }
    }

    /// Advances the cursor to the next hunk header (`j`/`k`, no decision
    /// made), clamped at the file's boundaries.
    fn move_hunk_cursor(&mut self, forward: bool) {
        let Some(current) = self.current_hunk_index() else {
            return;
        };
        let hunk_count = self.current_file_diff.as_ref().map_or(0, |d| d.hunks.len());
        if hunk_count == 0 {
            return;
        }
        let next = if forward {
            (current + 1).min(hunk_count - 1)
        } else {
            current.saturating_sub(1)
        };
        self.select_hunk_header_row(next);
    }

    fn jump_to_undecided_hunk(&mut self, forward: bool) {
        let (Some(current), Some(path)) =
            (self.current_hunk_index(), self.current_file_path.as_ref())
        else {
            return;
        };
        let Some(selection) = self.selections.get(path) else {
            return;
        };
        let target = if forward {
            selection.next_undecided_hunk(current)
        } else {
            selection.previous_undecided_hunk(current)
        };
        match target {
            Some(idx) => self.select_hunk_header_row(idx),
            None => self.status = Some(self.strings().status_all_hunks_decided.to_string()),
        }
    }

    /// After a decision, if every hunk of the file is now decided, jumps to
    /// the next file (git add -p's per-file batch flow); otherwise just
    /// moves the cursor to the next hunk to keep reviewing this file.
    fn advance_after_decision(&mut self, force_next_file: bool) -> Result<()> {
        let all_decided = self
            .current_file_path
            .as_ref()
            .and_then(|p| self.selections.get(p))
            .map(FileSelection::all_decided)
            .unwrap_or(false);

        if force_next_file || all_decided {
            let previous = self.files_state.selected();
            Self::next_in(&mut self.files_state, self.files.len());
            if self.files_state.selected() == previous {
                self.status = Some(self.strings().status_review_complete.to_string());
            } else {
                self.load_selected_file_diff()?;
            }
        } else {
            self.move_hunk_cursor(true);
        }
        Ok(())
    }

    fn decide_current_hunk(&mut self, accept: bool, extend_to_rest: bool) -> Result<()> {
        if self.pane != Pane::Hunks {
            return Ok(());
        }
        let (Some(hunk_index), Some(path), Some(diff)) = (
            self.current_hunk_index(),
            self.current_file_path.clone(),
            self.current_file_diff.as_ref(),
        ) else {
            return Ok(());
        };

        let selection = self
            .selections
            .entry(path.clone())
            .or_insert_with(|| FileSelection::all_undecided(&path, diff));

        match (accept, extend_to_rest) {
            (true, false) => selection.accept_hunk(hunk_index),
            (false, false) => selection.reject_hunk(hunk_index),
            (true, true) => selection.accept_hunk_and_rest(hunk_index),
            (false, true) => selection.reject_hunk_and_rest(hunk_index),
        }

        apply_selection(&path, diff, selection)?;
        self.refresh_files()?;
        self.advance_after_decision(extend_to_rest)?;
        Ok(())
    }

    fn split_current_hunk(&mut self) -> Result<()> {
        if self.pane != Pane::Hunks {
            return Ok(());
        }
        let (Some(hunk_index), Some(path)) =
            (self.current_hunk_index(), self.current_file_path.clone())
        else {
            return Ok(());
        };
        let Some(diff) = self.current_file_diff.as_mut() else {
            return Ok(());
        };

        let Some(sub_hunks) = split_hunk(&diff.hunks[hunk_index], self.context_lines) else {
            self.status = Some(self.strings().status_hunk_not_splittable.to_string());
            return Ok(());
        };
        let split_count = sub_hunks.len();
        diff.hunks.splice(hunk_index..=hunk_index, sub_hunks);

        self.hunk_rows = hunk_pane::build_rows(diff);
        self.selections
            .insert(path.clone(), FileSelection::all_undecided(&path, diff));
        self.select_hunk_header_row(hunk_index);
        self.status = Some(
            self.strings()
                .status_hunk_split
                .replace("{n}", &split_count.to_string()),
        );
        Ok(())
    }

    // --- Commits pane --------------------------------------------------

    fn fixup_selected_commit(&mut self) -> Result<()> {
        if self.pane != Pane::Commits {
            return Ok(());
        }
        let Some(commit) = self
            .commits_state
            .selected()
            .and_then(|i| self.commits.get(i))
            .cloned()
        else {
            return Ok(());
        };

        commit_fixup(&commit.short_sha)?;
        self.status = Some(format!("Created fixup! commit for {}", commit.short_sha));

        // HEAD moved: cached diffs/selections for open files are stale.
        self.selections.clear();
        self.current_file_diff = None;
        self.current_file_path = None;
        self.hunk_rows.clear();

        self.refresh_files()?;
        self.refresh_commits()?;
        self.load_selected_file_diff()?;
        Ok(())
    }

    // --- New commit form -------------------------------------------------

    fn open_commit_form(&mut self) -> Result<()> {
        let template = commit_template()?.unwrap_or_default();
        let diff = staged_diff().unwrap_or_default();
        self.commit_form = Some(CommitFormState::new(&template, diff));
        Ok(())
    }

    fn handle_commit_form_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if self.commit_form.is_none() {
            return Ok(());
        }

        if modifiers.contains(KeyModifiers::CONTROL) {
            return match code {
                KeyCode::Enter => self.submit_commit_form(),
                _ => Ok(()),
            };
        }

        if code == KeyCode::Esc {
            self.commit_form = None;
            return Ok(());
        }

        let form = self.commit_form.as_mut().expect("checked above");
        match code {
            KeyCode::Enter => form.insert_newline(),
            KeyCode::Backspace => form.backspace(),
            KeyCode::Left => form.move_left(),
            KeyCode::Right => form.move_right(),
            KeyCode::Up => form.move_up(),
            KeyCode::Down => form.move_down(),
            KeyCode::Home => form.move_home(),
            KeyCode::End => form.move_end(),
            KeyCode::Char(c) => form.insert_char(c),
            _ => {}
        }
        Ok(())
    }

    /// Creates a plain commit (not a fixup) from whatever hunks are
    /// currently staged, using the message typed in the form.
    fn submit_commit_form(&mut self) -> Result<()> {
        let Some(form) = self.commit_form.take() else {
            return Ok(());
        };
        let message = form.message();
        if message.trim().is_empty() {
            self.status = Some(self.strings().status_empty_commit_message.to_string());
            return Ok(());
        }

        commit_with_message(&message)?;
        self.status = Some(self.strings().status_new_commit_created.to_string());

        // HEAD moved: cached diffs/selections for open files are stale.
        self.selections.clear();
        self.current_file_diff = None;
        self.current_file_path = None;
        self.hunk_rows.clear();

        self.refresh_files()?;
        self.refresh_commits()?;
        self.load_selected_file_diff()?;
        Ok(())
    }

    fn handle_commit_form_mouse(&mut self, mouse: MouseEvent, terminal_area: Rect) -> Result<()> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Ok(());
        }
        let Some(form) = &self.commit_form else {
            return Ok(());
        };
        let areas = compute_commit_form_layout(terminal_area, form);
        if area_contains(areas.validate_button, mouse.column, mouse.row) {
            return self.submit_commit_form();
        }
        if area_contains(areas.cancel_button, mouse.column, mouse.row) {
            self.commit_form = None;
        }
        Ok(())
    }

    // --- Mouse dispatch ----------------------------------------------------

    fn handle_mouse(&mut self, mouse: MouseEvent, terminal_area: Rect) -> Result<()> {
        if self.show_help {
            return Ok(());
        }
        if self.screen == Screen::Triage {
            return self.handle_triage_mouse(mouse, terminal_area);
        }
        if self.commit_form.is_some() {
            return self.handle_commit_form_mouse(mouse, terminal_area);
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_review_click(mouse.column, mouse.row, terminal_area)?
            }
            MouseEventKind::ScrollDown => {
                self.handle_review_scroll(mouse.column, mouse.row, terminal_area, true)?
            }
            MouseEventKind::ScrollUp => {
                self.handle_review_scroll(mouse.column, mouse.row, terminal_area, false)?
            }
            _ => {}
        }
        Ok(())
    }

    /// Clicking a pane focuses it; clicking a row in Files/Commits selects
    /// it; clicking a row in Hunks selects *and* toggles it, like a
    /// checkbox click.
    fn handle_review_click(&mut self, x: u16, y: u16, terminal_area: Rect) -> Result<()> {
        let areas = layout::compute_layout(terminal_area, self.files.len());

        if area_contains(areas.files, x, y) {
            self.pane = Pane::Files;
            let offset = *self.files_state.offset_mut();
            if let Some(idx) = row_at(areas.files, offset, y, self.files.len()) {
                self.files_state.select(Some(idx));
                self.load_selected_file_diff()?;
            }
        } else if area_contains(areas.hunks, x, y) {
            self.pane = Pane::Hunks;
            let offset = *self.hunk_state.offset_mut();
            if let Some(idx) = row_at(areas.hunks, offset, y, self.hunk_rows.len()) {
                self.hunk_state.select(Some(idx));
                self.toggle_current_row()?;
            }
        } else if area_contains(areas.commits, x, y) {
            self.pane = Pane::Commits;
            let offset = *self.commits_state.offset_mut();
            if let Some(idx) = row_at(areas.commits, offset, y, self.commits.len()) {
                self.commits_state.select(Some(idx));
                self.commit_show_scroll = 0;
            }
        } else if area_contains(areas.commit_show, x, y) {
            self.pane = Pane::Commits;
        }
        Ok(())
    }

    fn handle_review_scroll(
        &mut self,
        x: u16,
        y: u16,
        terminal_area: Rect,
        down: bool,
    ) -> Result<()> {
        let areas = layout::compute_layout(terminal_area, self.files.len());
        if area_contains(areas.files, x, y) {
            self.pane = Pane::Files;
            if down {
                self.move_down()
            } else {
                self.move_up()
            }
        } else if area_contains(areas.hunks, x, y) {
            self.pane = Pane::Hunks;
            if down {
                self.move_down()
            } else {
                self.move_up()
            }
        } else if area_contains(areas.commits, x, y) {
            self.pane = Pane::Commits;
            if down {
                self.move_down()
            } else {
                self.move_up()
            }
        } else if area_contains(areas.commit_show, x, y) {
            self.pane = Pane::Commits;
            if down {
                self.commit_show_scroll = self.commit_show_scroll.saturating_add(1);
            } else {
                self.commit_show_scroll = self.commit_show_scroll.saturating_sub(1);
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    fn handle_triage_mouse(&mut self, mouse: MouseEvent, terminal_area: Rect) -> Result<()> {
        let areas = triage_screen::compute_layout(terminal_area);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(triage) = &mut self.triage else {
                    return Ok(());
                };
                if area_contains(areas.list, mouse.column, mouse.row) {
                    triage.set_focus(triage_screen::TriageFocus::List);
                    let offset = triage.list_offset();
                    if let Some(idx) = row_at(areas.list, offset, mouse.row, triage.row_count()) {
                        triage.select_row(idx);
                        triage.toggle_current();
                    }
                } else if area_contains(areas.file_diff, mouse.column, mouse.row) {
                    triage.set_focus(triage_screen::TriageFocus::FileDiff);
                } else if area_contains(areas.show, mouse.column, mouse.row) {
                    triage.set_focus(triage_screen::TriageFocus::Show);
                }
            }
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let down = mouse.kind == MouseEventKind::ScrollDown;
                let Some(triage) = &mut self.triage else {
                    return Ok(());
                };
                let focus = if area_contains(areas.list, mouse.column, mouse.row) {
                    Some(triage_screen::TriageFocus::List)
                } else if area_contains(areas.file_diff, mouse.column, mouse.row) {
                    Some(triage_screen::TriageFocus::FileDiff)
                } else if area_contains(areas.show, mouse.column, mouse.row) {
                    Some(triage_screen::TriageFocus::Show)
                } else {
                    None
                };
                if let Some(focus) = focus {
                    triage.set_focus(focus);
                    if down {
                        triage.scroll_focused_pane_down();
                    } else {
                        triage.scroll_focused_pane_up();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // --- Key dispatch ----------------------------------------------------

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        // Ctrl+C always quits, even mid-form. The commit form needs Ctrl+Enter
        // to submit, so it must see CONTROL-modified keys before the blanket
        // guard below discards every other Ctrl combination (Ctrl+D, Ctrl+A,
        // ...), which would otherwise collide with plain-key bindings sharing
        // the same `KeyCode::Char` values.
        if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        if self.commit_form.is_some() {
            return self.handle_commit_form_key(code, modifiers);
        }

        if modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(());
        }

        if self.show_help {
            self.show_help = false;
            return Ok(());
        }

        // Works from either screen, unlike pane-specific bindings below.
        if code == KeyCode::Char('l') {
            self.lang = self.lang.cycle();
            return Ok(());
        }

        if self.screen == Screen::Triage {
            return self.handle_triage_key(code);
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.cycle_pane(true),
            KeyCode::BackTab => self.cycle_pane(false),
            KeyCode::Down => self.move_down()?,
            KeyCode::Up => self.move_up()?,
            KeyCode::Char(' ') | KeyCode::Enter => self.toggle_current_row()?,
            KeyCode::Char('x') if self.pane == Pane::Commits => self.fixup_selected_commit()?,
            KeyCode::Char('c') => self.open_commit_form()?,
            KeyCode::Char('t') if self.pane == Pane::Files => {
                self.open_triage_if_evident_matches_exist()?
            }
            KeyCode::Char('?') => self.show_help = true,
            // git add -p-style guided review, active in the Hunks pane.
            KeyCode::Char('y') if self.pane == Pane::Hunks => {
                self.decide_current_hunk(true, false)?
            }
            KeyCode::Char('n') if self.pane == Pane::Hunks => {
                self.decide_current_hunk(false, false)?
            }
            KeyCode::Char('a') if self.pane == Pane::Hunks => {
                self.decide_current_hunk(true, true)?
            }
            KeyCode::Char('d') if self.pane == Pane::Hunks => {
                self.decide_current_hunk(false, true)?
            }
            KeyCode::Char('j') if self.pane == Pane::Hunks => self.move_hunk_cursor(true),
            KeyCode::Char('k') if self.pane == Pane::Hunks => self.move_hunk_cursor(false),
            KeyCode::Char('J') if self.pane == Pane::Hunks => self.jump_to_undecided_hunk(true),
            KeyCode::Char('K') if self.pane == Pane::Hunks => self.jump_to_undecided_hunk(false),
            KeyCode::Char('s') if self.pane == Pane::Hunks => self.split_current_hunk()?,
            KeyCode::Char('j') => self.move_down()?,
            KeyCode::Char('k') => self.move_up()?,
            KeyCode::PageDown if self.pane == Pane::Commits => {
                self.commit_show_scroll = self.commit_show_scroll.saturating_add(1)
            }
            KeyCode::PageUp if self.pane == Pane::Commits => {
                self.commit_show_scroll = self.commit_show_scroll.saturating_sub(1)
            }
            _ => {}
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        if self.screen == Screen::Triage {
            let strings = self.strings();
            if let Some(triage) = &mut self.triage {
                triage_screen::render(frame, frame.area(), triage, strings);
            }
            return;
        }

        let strings = self.strings();
        let areas = layout::compute_layout(frame.area(), self.files.len());
        files_pane::render(frame, areas.files, self);
        hunk_pane::render(frame, areas.hunks, self);
        commits_pane::render(frame, areas.commits, self);
        commit_show_pane::render(frame, areas.commit_show, self);
        render_help_bar(frame, areas.help, self.pane, strings);

        if let Some(form) = &self.commit_form {
            render_commit_form_popup(frame, form, strings);
        }

        if self.show_help {
            render_help_popup(frame, self.pane, strings);
        }
    }
}

struct CommitFormAreas {
    popup: Rect,
    text: Rect,
    diff: Rect,
    validate_button: Rect,
    cancel_button: Rect,
}

/// Shared between rendering and mouse-click handling so the two never drift
/// apart on where the buttons actually are.
fn compute_commit_form_layout(area: Rect, form: &CommitFormState) -> CommitFormAreas {
    use ratatui::layout::{Constraint, Flex, Layout};

    let popup_area = Layout::vertical([Constraint::Percentage(85)])
        .flex(Flex::Center)
        .split(area)[0];
    let popup_area = Layout::horizontal([Constraint::Percentage(85)])
        .flex(Flex::Center)
        .split(popup_area)[0];

    let text_height = (form.lines.len() as u16 + 2).clamp(5, 12);
    let sections = Layout::vertical([
        Constraint::Length(text_height),
        Constraint::Min(3),
        Constraint::Length(3),
    ])
    .split(popup_area);

    let button_row = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(13),
        Constraint::Length(2),
        Constraint::Length(13),
    ])
    .split(sections[2]);

    CommitFormAreas {
        popup: popup_area,
        text: sections[0],
        diff: sections[1],
        validate_button: button_row[1],
        cancel_button: button_row[3],
    }
}

fn render_commit_form_popup(
    frame: &mut ratatui::Frame,
    form: &CommitFormState,
    strings: &'static Strings,
) {
    use ratatui::layout::Alignment;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::Span;
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    let areas = compute_commit_form_layout(frame.area(), form);
    let red_border = Style::default().fg(Color::Red);

    frame.render_widget(Clear, areas.popup);

    let message = Paragraph::new(form.lines.join("\n")).block(
        Block::default()
            .title(Span::styled(
                strings.commit_form_title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(red_border),
    );
    frame.render_widget(message, areas.text);

    let cursor_x = (areas.text.x + 1 + form.cursor_col as u16)
        .min(areas.text.x + areas.text.width.saturating_sub(2));
    let cursor_y = (areas.text.y + 1 + form.cursor_line as u16)
        .min(areas.text.y + areas.text.height.saturating_sub(2));
    frame.set_cursor_position((cursor_x, cursor_y));

    let diff_view = Paragraph::new(form.diff.as_str()).block(
        Block::default()
            .title(strings.commit_form_diff_title)
            .borders(Borders::ALL)
            .border_style(red_border),
    );
    frame.render_widget(diff_view, areas.diff);

    let validate = Paragraph::new(strings.commit_form_validate)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        );
    frame.render_widget(validate, areas.validate_button);

    let cancel = Paragraph::new(strings.commit_form_cancel)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(red_border),
        );
    frame.render_widget(cancel, areas.cancel_button);
}

fn render_help_bar(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    pane: Pane,
    strings: &'static Strings,
) {
    use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

    let text = help_lines_short(pane, strings).join("   |   ");
    let bar = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(bar, area);
}

fn area_contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

/// Maps a clicked/scrolled screen row to a list index, accounting for the
/// list's current scroll `offset` and its bordered `Block` (one row of
/// border on each side). `None` if the row falls outside the list's actual
/// content (empty list, border, or past the last item).
fn row_at(area: Rect, offset: usize, y: u16, len: usize) -> Option<usize> {
    let content_top = area.y + 1;
    let content_bottom = area.y + area.height.saturating_sub(1);
    if y < content_top || y >= content_bottom {
        return None;
    }
    let idx = offset + (y - content_top) as usize;
    if idx < len { Some(idx) } else { None }
}

fn help_lines(pane: Pane, strings: &'static Strings) -> Vec<&'static str> {
    match pane {
        Pane::Files => vec![
            strings.help_switch_pane,
            strings.help_navigate,
            strings.help_reopen_triage,
            strings.help_new_commit,
            strings.help_language,
            strings.help_quit,
        ],
        Pane::Hunks => vec![
            strings.help_hunk_accept_reject_advance,
            strings.help_hunk_accept_reject_rest,
            strings.help_hunk_next_prev,
            strings.help_hunk_next_prev_undecided,
            strings.help_hunk_split,
            strings.help_hunk_toggle,
            strings.help_new_commit,
            strings.help_language,
            strings.help_quit,
        ],
        Pane::Commits => vec![
            strings.help_commit_fixup,
            strings.help_commit_green_check,
            strings.help_navigate,
            strings.help_new_commit,
            strings.help_language,
            strings.help_quit,
        ],
    }
}

/// Compact counterpart of [`help_lines`], for the always-visible bottom
/// help bar — same key order per pane, just the `short_*` strings.
fn help_lines_short(pane: Pane, strings: &'static Strings) -> Vec<&'static str> {
    match pane {
        Pane::Files => vec![
            strings.short_switch_pane,
            strings.short_navigate,
            strings.short_reopen_triage,
            strings.short_new_commit,
            strings.short_language,
            strings.short_quit,
        ],
        Pane::Hunks => vec![
            strings.short_hunk_accept_reject_advance,
            strings.short_hunk_accept_reject_rest,
            strings.short_hunk_next_prev,
            strings.short_hunk_next_prev_undecided,
            strings.short_hunk_split,
            strings.short_hunk_toggle,
            strings.short_new_commit,
            strings.short_language,
            strings.short_quit,
        ],
        Pane::Commits => vec![
            strings.short_commit_fixup,
            strings.short_commit_green_check,
            strings.short_navigate,
            strings.short_new_commit,
            strings.short_language,
            strings.short_quit,
        ],
    }
}

fn render_help_popup(frame: &mut ratatui::Frame, pane: Pane, strings: &'static Strings) {
    use ratatui::layout::{Constraint, Flex, Layout};
    use ratatui::style::{Modifier, Style};
    use ratatui::text::Line;
    use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

    let area = frame.area();
    // +2 for the border, +2 for the block's 1-char top/bottom padding.
    let popup_area = Layout::vertical([Constraint::Length(
        help_lines(pane, strings).len() as u16 + 4,
    )])
    .flex(Flex::Center)
    .split(area)[0];
    let popup_area = Layout::horizontal([Constraint::Percentage(60)])
        .flex(Flex::Center)
        .split(popup_area)[0];

    let lines: Vec<Line> = help_lines(pane, strings)
        .into_iter()
        .map(Line::from)
        .collect();
    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(ratatui::text::Span::styled(
                strings.help_popup_title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .padding(Padding::uniform(1))
            .borders(Borders::ALL),
    );

    frame.render_widget(Clear, popup_area);
    frame.render_widget(paragraph, popup_area);
}

pub fn run(cli: &Cli) -> Result<()> {
    let mut app = App::new(cli)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    // Needed to tell Ctrl+Enter apart from plain Enter (the commit form's
    // submit binding) — without it, terminals report both as the same key
    // event. Only some terminals (kitty, WezTerm, recent iTerm2...) support
    // this, hence the capability check.
    let keyboard_enhancement = supports_keyboard_enhancement().unwrap_or(false);
    if keyboard_enhancement {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app);

    if keyboard_enhancement {
        execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags)?;
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| app.draw(frame))?;

        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                app.handle_key(key.code, key.modifiers)?;
            }
            Event::Mouse(mouse) => {
                let size = terminal.size()?;
                let terminal_area = Rect::new(0, 0, size.width, size.height);
                app.handle_mouse(mouse, terminal_area)?;
            }
            _ => {}
        }
    }
    Ok(())
}
