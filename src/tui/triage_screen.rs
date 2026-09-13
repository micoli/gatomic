use anyhow::Result;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::fixup::FileCommitMatch;
use crate::git::{CommitInfo, run_git};
use crate::i18n::Strings;

use super::text_pane::{colorize_line, render_text_pane};

pub struct TriageFileNode {
    pub file: String,
    pub checked: bool,
    pub added: u32,
    pub removed: u32,
}

pub struct TriageCommitGroup {
    pub commit: CommitInfo,
    pub files: Vec<TriageFileNode>,
}

#[derive(Debug, Clone, Copy)]
enum TriageRow {
    Commit(usize),
    File(usize, usize),
}

/// The screen has three panes: the commit/file tree, the diff of whichever
/// file is currently selected (below the tree), and the `git show` of
/// whichever commit is currently in scope (right column, always visible).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriageFocus {
    List,
    FileDiff,
    Show,
}

/// State for the startup triage screen: a tree of commits with the files
/// whose modifications evidently belong to them, so the user can accept
/// them in bulk before dropping into the detailed hunk-by-hunk review for
/// everything else.
pub struct TriageState {
    groups: Vec<TriageCommitGroup>,
    rows: Vec<TriageRow>,
    list_state: ListState,
    focus: TriageFocus,
    file_diff_scroll: u16,
    show_scroll: u16,
}

impl TriageState {
    pub fn new(matches: Vec<FileCommitMatch>) -> Self {
        let mut groups: Vec<TriageCommitGroup> = Vec::new();
        for m in matches {
            let commit = m.matching_commits[0].clone();
            let (added, removed) = diff_numstat(&m.file);
            let file = TriageFileNode {
                file: m.file,
                checked: true,
                added,
                removed,
            };
            match groups
                .iter_mut()
                .find(|g| g.commit.short_sha == commit.short_sha)
            {
                Some(group) => group.files.push(file),
                None => groups.push(TriageCommitGroup {
                    commit,
                    files: vec![file],
                }),
            }
        }

        let rows = build_rows(&groups);
        let mut list_state = ListState::default();
        if !rows.is_empty() {
            list_state.select(Some(0));
        }

        TriageState {
            groups,
            rows,
            list_state,
            focus: TriageFocus::List,
            file_diff_scroll: 0,
            show_scroll: 0,
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            TriageFocus::List => TriageFocus::FileDiff,
            TriageFocus::FileDiff => TriageFocus::Show,
            TriageFocus::Show => TriageFocus::List,
        };
    }

    /// Used by mouse click handling to focus whichever pane was clicked
    /// directly, rather than cycling through `toggle_focus`.
    pub fn set_focus(&mut self, focus: TriageFocus) {
        self.focus = focus;
    }

    pub fn list_offset(&mut self) -> usize {
        *self.list_state.offset_mut()
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Selects a row by absolute index, e.g. resolved from a mouse click —
    /// resets both preview scrolls like `move_up`/`move_down` do.
    pub fn select_row(&mut self, index: usize) {
        if index < self.rows.len() {
            self.list_state.select(Some(index));
            self.file_diff_scroll = 0;
            self.show_scroll = 0;
        }
    }

    pub fn move_down(&mut self) {
        let len = self.rows.len();
        if len == 0 {
            return;
        }
        let next = self
            .list_state
            .selected()
            .map_or(0, |i| (i + 1).min(len - 1));
        self.list_state.select(Some(next));
        self.file_diff_scroll = 0;
        self.show_scroll = 0;
    }

    pub fn move_up(&mut self) {
        let len = self.rows.len();
        if len == 0 {
            return;
        }
        let prev = self
            .list_state
            .selected()
            .map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(prev));
        self.file_diff_scroll = 0;
        self.show_scroll = 0;
    }

    pub fn scroll_focused_pane_down(&mut self) {
        match self.focus {
            TriageFocus::List => self.move_down(),
            TriageFocus::FileDiff => {
                self.file_diff_scroll = self.file_diff_scroll.saturating_add(1)
            }
            TriageFocus::Show => self.show_scroll = self.show_scroll.saturating_add(1),
        }
    }

    pub fn scroll_focused_pane_up(&mut self) {
        match self.focus {
            TriageFocus::List => self.move_up(),
            TriageFocus::FileDiff => {
                self.file_diff_scroll = self.file_diff_scroll.saturating_sub(1)
            }
            TriageFocus::Show => self.show_scroll = self.show_scroll.saturating_sub(1),
        }
    }

    pub fn toggle_current(&mut self) {
        let Some(row) = self
            .list_state
            .selected()
            .and_then(|i| self.rows.get(i).copied())
        else {
            return;
        };
        match row {
            TriageRow::File(group_index, file_index) => {
                if let Some(f) = self
                    .groups
                    .get_mut(group_index)
                    .and_then(|g| g.files.get_mut(file_index))
                {
                    f.checked = !f.checked;
                }
            }
            TriageRow::Commit(group_index) => {
                if let Some(group) = self.groups.get_mut(group_index) {
                    let all_checked = group.files.iter().all(|f| f.checked);
                    for f in &mut group.files {
                        f.checked = !all_checked;
                    }
                }
            }
        }
    }

    /// Paths still checked for bulk acceptance, grouped by target commit
    /// sha in the order commits were first encountered (stable,
    /// deterministic order for the resulting fixup commits).
    pub fn accepted_groups(&self) -> Vec<(String, Vec<String>)> {
        self.groups
            .iter()
            .filter_map(|g| {
                let paths: Vec<String> = g
                    .files
                    .iter()
                    .filter(|f| f.checked)
                    .map(|f| f.file.clone())
                    .collect();
                if paths.is_empty() {
                    None
                } else {
                    Some((g.commit.short_sha.clone(), paths))
                }
            })
            .collect()
    }

    fn current_row(&self) -> Option<TriageRow> {
        self.list_state
            .selected()
            .and_then(|i| self.rows.get(i))
            .copied()
    }

    /// The commit group the current selection belongs to, whether a commit
    /// row or one of its file rows is selected.
    fn current_group_index(&self) -> Option<usize> {
        match self.current_row()? {
            TriageRow::Commit(group_index) | TriageRow::File(group_index, _) => Some(group_index),
        }
    }

    fn current_commit_sha(&self) -> Option<String> {
        self.current_group_index()
            .map(|gi| self.groups[gi].commit.short_sha.clone())
    }

    fn current_selected_file_path(&self) -> Option<String> {
        match self.current_row()? {
            TriageRow::File(group_index, file_index) => {
                Some(self.groups[group_index].files[file_index].file.clone())
            }
            TriageRow::Commit(_) => None,
        }
    }
}

fn build_rows(groups: &[TriageCommitGroup]) -> Vec<TriageRow> {
    let mut rows = Vec::new();
    for (group_index, group) in groups.iter().enumerate() {
        rows.push(TriageRow::Commit(group_index));
        for file_index in 0..group.files.len() {
            rows.push(TriageRow::File(group_index, file_index));
        }
    }
    rows
}

fn commit_glyph(group: &TriageCommitGroup) -> &'static str {
    if group.files.iter().all(|f| f.checked) {
        "[x]"
    } else if group.files.iter().all(|f| !f.checked) {
        "[ ]"
    } else {
        "[~]"
    }
}

fn checkbox(checked: bool) -> &'static str {
    if checked { "[x]" } else { "[ ]" }
}

/// Insertion/deletion counts for a file's unstaged changes, via
/// `git diff --numstat`. Defaults to `(0, 0)` on error or for binary files
/// (which report `-`/`-` instead of numbers) — display-only, never fatal.
fn diff_numstat(path: &str) -> (u32, u32) {
    run_git(&["diff", "--no-color", "--numstat", "--", path])
        .ok()
        .and_then(|raw| raw.lines().next().and_then(parse_numstat_line))
        .unwrap_or((0, 0))
}

fn parse_numstat_line(line: &str) -> Option<(u32, u32)> {
    let mut parts = line.splitn(3, '\t');
    let added = parts.next()?.parse().ok()?;
    let removed = parts.next()?.parse().ok()?;
    Some((added, removed))
}

fn fetch_file_diff(path: &str) -> Result<String> {
    run_git(&["diff", "--no-color", "--", path])
}

fn fetch_commit_show(sha: &str) -> Result<String> {
    run_git(&["show", "--no-color", sha])
}

/// The screen's three clickable regions, computed once and shared by
/// `render` and by mouse-click hit-testing in `App` (which has no access to
/// the `Frame` at event-handling time, only the terminal's current size).
pub struct TriageAreas {
    pub list: Rect,
    pub file_diff: Rect,
    pub help: Rect,
    pub show: Rect,
}

pub fn compute_layout(area: Rect) -> TriageAreas {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(columns[0]);
    let left_top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(left[0]);

    TriageAreas {
        list: left_top[0],
        file_diff: left_top[1],
        help: left[1],
        show: columns[1],
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut TriageState, strings: &'static Strings) {
    use super::theme::active_pane_style;

    let border_style = |pane: TriageFocus| {
        if state.focus == pane {
            active_pane_style()
        } else {
            Style::default()
        }
    };

    let areas = compute_layout(area);

    let items: Vec<ListItem> = state
        .rows
        .iter()
        .map(|row| match *row {
            TriageRow::Commit(group_index) => {
                let group = &state.groups[group_index];
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{} ", commit_glyph(group))),
                    Span::styled(
                        group.commit.short_sha.clone(),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(format!(" {}", group.commit.message)),
                ]))
            }
            TriageRow::File(group_index, file_index) => {
                let file = &state.groups[group_index].files[file_index];
                ListItem::new(Line::from(vec![
                    Span::raw(format!("    {} ", checkbox(file.checked))),
                    Span::styled(file.file.clone(), Style::default().fg(Color::Blue)),
                    Span::raw(" "),
                    Span::styled(
                        format!("+{}", file.added),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!("-{}", file.removed),
                        Style::default().fg(Color::Red),
                    ),
                ]))
            }
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(Span::styled(
                    strings.triage_list_title,
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(border_style(TriageFocus::List)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, areas.list, &mut state.list_state);

    let file_diff_lines = match state.current_selected_file_path() {
        Some(path) => match fetch_file_diff(&path) {
            Ok(text) => text.lines().map(colorize_line).collect(),
            Err(err) => vec![Line::from(
                strings.show_error.replace("{err}", &err.to_string()),
            )],
        },
        None => vec![Line::from(strings.triage_select_file_prompt)],
    };
    render_text_pane(
        frame,
        areas.file_diff,
        strings.triage_file_diff_title,
        border_style(TriageFocus::FileDiff),
        file_diff_lines,
        &mut state.file_diff_scroll,
    );

    let help = Paragraph::new(strings.triage_help).block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, areas.help);

    let show_lines = match state.current_commit_sha() {
        Some(sha) => match fetch_commit_show(&sha) {
            Ok(text) => text.lines().map(colorize_line).collect(),
            Err(err) => vec![Line::from(
                strings.show_error.replace("{err}", &err.to_string()),
            )],
        },
        None => Vec::new(),
    };
    render_text_pane(
        frame,
        areas.show,
        strings.commit_show_title,
        border_style(TriageFocus::Show),
        show_lines,
        &mut state.show_scroll,
    );
}
