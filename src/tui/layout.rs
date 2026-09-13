use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct PaneAreas {
    pub files: Rect,
    pub hunks: Rect,
    pub commits: Rect,
    pub help: Rect,
}

/// Files pane height adapts to the number of files (bounded), so it never
/// wastes vertical space that the hunk viewer needs far more. A full-width
/// help bar is reserved at the bottom, like the triage screen's.
pub fn compute_layout(area: Rect, file_count: usize) -> PaneAreas {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(rows[0]);

    let files_height = (file_count as u16 + 2).clamp(4, 12);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(files_height), Constraint::Min(0)])
        .split(outer[0]);

    PaneAreas {
        files: left[0],
        hunks: left[1],
        commits: outer[1],
        help: rows[1],
    }
}
