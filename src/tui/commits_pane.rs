use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use super::theme::{active_pane_style, pane_title_style};
use super::{App, Pane};

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let selected_file = app.current_file_path.as_deref();

    let items: Vec<ListItem> = app
        .commits
        .iter()
        .map(|commit| {
            let contains_selected_file = selected_file.is_some_and(|path| {
                app.files_by_commit
                    .get(&commit.short_sha)
                    .is_some_and(|paths| paths.contains(path))
            });

            let marker = if contains_selected_file {
                Span::styled("✓ ", Style::default().fg(Color::Green))
            } else {
                Span::raw("  ")
            };

            Line::from(vec![
                marker,
                Span::raw(format!(
                    "{} {} {}",
                    commit.short_sha, commit.date, commit.message
                )),
            ])
            .into()
        })
        .collect();

    let is_active = app.pane == Pane::Commits;
    let border_style = if is_active {
        active_pane_style()
    } else {
        Style::default()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(Span::styled(
                    "Commits (x = fixup)",
                    pane_title_style(is_active),
                ))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, &mut app.commits_state);
}
