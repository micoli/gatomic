use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::git::run_git;

use super::text_pane::{colorize_line, render_text_pane};
use super::theme::active_pane_style;
use super::{App, Pane};

fn fetch_commit_show(sha: &str) -> anyhow::Result<String> {
    run_git(&["show", "--no-color", sha])
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let selected_sha = app
        .commits_state
        .selected()
        .and_then(|i| app.commits.get(i))
        .map(|c| c.short_sha.clone());

    let strings = app.strings();
    let lines: Vec<Line<'static>> = match selected_sha {
        Some(sha) => match fetch_commit_show(&sha) {
            Ok(text) => text.lines().map(colorize_line).collect(),
            Err(err) => vec![Line::from(
                strings.show_error.replace("{err}", &err.to_string()),
            )],
        },
        None => Vec::new(),
    };

    let is_active = app.pane == Pane::Commits;
    let border_style = if is_active {
        active_pane_style()
    } else {
        ratatui::style::Style::default()
    };

    render_text_pane(
        frame,
        area,
        strings.commit_show_title,
        border_style,
        lines,
        &mut app.commit_show_scroll,
    );
}
