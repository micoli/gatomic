use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem};

use super::{App, Pane};
use crate::git::FileStatusKind;

fn status_letter(kind: &FileStatusKind) -> (&'static str, Color) {
    match kind {
        FileStatusKind::Modified => ("M", Color::Yellow),
        FileStatusKind::Added => ("A", Color::Green),
        FileStatusKind::Deleted => ("D", Color::Red),
        FileStatusKind::Renamed => ("R", Color::Cyan),
        FileStatusKind::Untracked => ("?", Color::Magenta),
        FileStatusKind::Conflicted => ("U", Color::Red),
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .files
        .iter()
        .map(|entry| {
            let (letter, color) = status_letter(&entry.kind);
            let indicator = if entry.staged && !entry.has_unstaged_changes {
                "+"
            } else if entry.staged {
                "~"
            } else {
                " "
            };
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled(format!("{letter} "), Style::default().fg(color)),
                ratatui::text::Span::raw(format!("{indicator} ")),
                ratatui::text::Span::raw(entry.path.clone()),
            ])
            .into()
        })
        .collect();

    let border_style = if app.pane == Pane::Files {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title("Files")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, &mut app.files_state);
}
