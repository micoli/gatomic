mod commits_pane;
mod files_pane;
mod hunk_pane;
mod layout;

use std::collections::HashMap;
use std::io;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::ListState;

use crate::cli::Cli;
use crate::diff::{FileDiff, apply_selection, load_file_diff};
use crate::git::{
    CommitInfo, FileEntry, commit_fixup, commits_of_current_branch, last_n_commits,
    list_file_entries,
};
use crate::selection::FileSelection;
use hunk_pane::HunkRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Files,
    Hunks,
    Commits,
}

pub struct App {
    context_lines: u32,
    last_commits: Option<usize>,

    files: Vec<FileEntry>,
    files_state: ListState,

    commits: Vec<CommitInfo>,
    commits_state: ListState,

    pane: Pane,

    current_file_path: Option<String>,
    current_file_diff: Option<FileDiff>,
    hunk_rows: Vec<HunkRow>,
    hunk_state: ListState,
    selections: HashMap<String, FileSelection>,

    status: Option<String>,
    should_quit: bool,
}

impl App {
    pub fn new(cli: &Cli) -> Result<Self> {
        let mut app = App {
            context_lines: cli.context_lines,
            last_commits: cli.last_commits,
            files: Vec::new(),
            files_state: ListState::default(),
            commits: Vec::new(),
            commits_state: ListState::default(),
            pane: Pane::Files,
            current_file_path: None,
            current_file_diff: None,
            hunk_rows: Vec::new(),
            hunk_state: ListState::default(),
            selections: HashMap::new(),
            status: None,
            should_quit: false,
        };
        app.refresh_files()?;
        app.refresh_commits()?;
        app.load_selected_file_diff()?;
        Ok(app)
    }

    fn refresh_files(&mut self) -> Result<()> {
        let previously_selected = self
            .files_state
            .selected()
            .and_then(|i| self.files.get(i))
            .map(|f| f.path.clone());

        self.files = list_file_entries()?;

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
        Ok(())
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
            .or_insert_with(|| FileSelection::all_selected(&path, &diff));
        self.current_file_diff = Some(diff);
        self.current_file_path = Some(path);
        self.hunk_state.select(if self.hunk_rows.is_empty() {
            None
        } else {
            Some(0)
        });
        Ok(())
    }

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
            Pane::Commits => Self::next_in(&mut self.commits_state, self.commits.len()),
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
            Pane::Commits => Self::prev_in(&mut self.commits_state, self.commits.len()),
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
            .or_insert_with(|| FileSelection::all_selected(&path, diff));

        match row {
            HunkRow::Header(hunk_index) => selection.toggle_hunk(hunk_index),
            HunkRow::Line(hunk_index, line_index) => {
                let is_context =
                    diff.hunks[hunk_index].lines[line_index].kind == crate::diff::LineKind::Context;
                if is_context {
                    return Ok(());
                }
                selection.toggle_line(hunk_index, line_index);
            }
        }

        apply_selection(&path, diff, selection)?;
        self.refresh_files()?;
        self.status = Some(format!("Staged selection updated for {path}"));
        Ok(())
    }

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

    fn handle_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.cycle_pane(true),
            KeyCode::BackTab => self.cycle_pane(false),
            KeyCode::Down | KeyCode::Char('j') => self.move_down()?,
            KeyCode::Up | KeyCode::Char('k') => self.move_up()?,
            KeyCode::Char(' ') | KeyCode::Enter => self.toggle_current_row()?,
            KeyCode::Char('f') => self.fixup_selected_commit()?,
            _ => {}
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        let areas = layout::compute_layout(frame.area(), self.files.len());
        files_pane::render(frame, areas.files, self);
        hunk_pane::render(frame, areas.hunks, self);
        commits_pane::render(frame, areas.commits, self);
    }
}

pub fn run(cli: &Cli) -> Result<()> {
    let mut app = App::new(cli)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| app.draw(frame))?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key.code)?;
        }
    }
    Ok(())
}
