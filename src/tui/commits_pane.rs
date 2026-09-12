use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};

use super::{App, Pane};

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .commits
        .iter()
        .map(|commit| {
            ListItem::new(format!(
                "{} {} {}",
                commit.short_sha, commit.date, commit.message
            ))
        })
        .collect();

    let border_style = if app.pane == Pane::Commits {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title("Commits (f = fixup)")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, &mut app.commits_state);
}
