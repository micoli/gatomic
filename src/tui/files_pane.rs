use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use super::theme::{active_pane_style, pane_title_style};
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

/// Renders the "NEW"/"DELETED"/`+N -M` suffix from data already computed
/// once in `App::refresh_files` — no git call happens here, since this runs
/// on every redraw and a per-file spawn here was the dominant cost of the
/// Files pane with many changed files.
fn stat_spans(kind: &FileStatusKind, stats: Option<(u32, u32)>) -> Vec<Span<'static>> {
    match kind {
        FileStatusKind::Untracked | FileStatusKind::Added => vec![Span::styled(
            "NEW",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )],
        FileStatusKind::Deleted => vec![Span::styled(
            "DELETED",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )],
        _ => {
            let (added, removed) = stats.unwrap_or((0, 0));
            vec![
                Span::styled(format!("+{added}"), Style::default().fg(Color::Green)),
                Span::raw(" "),
                Span::styled(format!("-{removed}"), Style::default().fg(Color::Red)),
            ]
        }
    }
}

/// "hunks 2/3" next to the file. Once it's been opened this session, the
/// real `FileSelection` gives an accurate decided/total count live; before
/// that, fall back to the total alone (from `App::hunk_counts`, one batched
/// `git diff HEAD` covering every file — see `diff::count_hunks_per_file`)
/// with 0 decided, so every changed file shows its hunk count immediately
/// rather than only the ones already clicked into.
fn hunk_progress_span(app: &App, path: &str) -> Option<Span<'static>> {
    let (decided, total) = match app.selections.get(path) {
        Some(selection) => selection.progress(),
        None => (0, *app.hunk_counts.get(path)?),
    };
    if total == 0 {
        return None;
    }
    let color = if decided == total {
        Color::Green
    } else {
        Color::Cyan
    };
    Some(Span::styled(
        format!("hunks {decided}/{total}"),
        Style::default().fg(color),
    ))
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
            let mut spans = vec![
                Span::styled(format!("{letter} "), Style::default().fg(color)),
                Span::raw(format!("{indicator} ")),
                Span::raw(entry.path.clone()),
                Span::raw("  "),
            ];
            spans.extend(stat_spans(
                &entry.kind,
                app.file_stats.get(&entry.path).copied(),
            ));
            if let Some(progress) = hunk_progress_span(app, &entry.path) {
                spans.push(Span::raw("  "));
                spans.push(progress);
            }
            Line::from(spans).into()
        })
        .collect();

    let is_active = app.pane == Pane::Files;
    let border_style = if is_active {
        active_pane_style()
    } else {
        Style::default()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(Span::styled("Files", pane_title_style(is_active)))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, &mut app.files_state);
}
