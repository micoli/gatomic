use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use super::{App, Pane};
use crate::diff::{FileDiff, LineKind};
use crate::selection::{FileSelection, HunkDecision};

#[derive(Debug, Clone, Copy)]
pub enum HunkRow {
    Header(usize),
    Line(usize, usize),
}

pub fn build_rows(diff: &FileDiff) -> Vec<HunkRow> {
    let mut rows = Vec::new();
    for (hunk_index, hunk) in diff.hunks.iter().enumerate() {
        rows.push(HunkRow::Header(hunk_index));
        for line_index in 0..hunk.lines.len() {
            rows.push(HunkRow::Line(hunk_index, line_index));
        }
    }
    rows
}

fn checkbox(selected: bool) -> &'static str {
    if selected { "[x]" } else { "[ ]" }
}

fn render_row(row: &HunkRow, diff: &FileDiff, selection: &FileSelection) -> ListItem<'static> {
    match *row {
        HunkRow::Header(hunk_index) => {
            let hunk = &diff.hunks[hunk_index];
            let selected = selection
                .hunks
                .get(hunk_index)
                .map(|h| h.decision == HunkDecision::Accepted)
                .unwrap_or(false);
            let text = format!(
                "{} @@ -{},{} +{},{} @@",
                checkbox(selected),
                hunk.old_start,
                hunk.old_lines,
                hunk.new_start,
                hunk.new_lines
            );
            ListItem::new(Line::from(Span::styled(
                text,
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )))
        }
        HunkRow::Line(hunk_index, line_index) => {
            let line = &diff.hunks[hunk_index].lines[line_index];
            let line_selected = selection
                .hunks
                .get(hunk_index)
                .and_then(|h| h.line_selected.get(line_index))
                .copied()
                .unwrap_or(false);

            let (prefix, color) = match line.kind {
                LineKind::Added => ("+", Color::Green),
                LineKind::Removed => ("-", Color::Red),
                LineKind::Context => (" ", Color::Gray),
            };

            let text = match line.kind {
                LineKind::Context => format!("     {prefix} {}", line.content),
                _ => format!(" {} {prefix} {}", checkbox(line_selected), line.content),
            };

            ListItem::new(Line::from(Span::styled(text, Style::default().fg(color))))
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let border_style = if app.pane == Pane::Hunks {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let title = app
        .current_file_path
        .as_deref()
        .map(|p| format!("Hunks — {p}"))
        .unwrap_or_else(|| "Hunks".to_string());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let (Some(diff), Some(selection)) = (
        app.current_file_diff.as_ref(),
        app.current_file_path
            .as_ref()
            .and_then(|p| app.selections.get(p)),
    ) else {
        frame.render_widget(block, area);
        return;
    };

    let items: Vec<ListItem> = app
        .hunk_rows
        .iter()
        .map(|row| render_row(row, diff, selection))
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, &mut app.hunk_state);
}
