use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Colors a line of `git diff`/`git show` output the way `git` itself would
/// with `--color`, since we always fetch with `--no-color` to keep the raw
/// text parseable.
pub fn colorize_line(line: &str) -> Line<'static> {
    let style = if line.starts_with("commit ") {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if line.starts_with("+++") || line.starts_with("---") {
        Style::default().add_modifier(Modifier::BOLD)
    } else if line.starts_with('+') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::default().fg(Color::Red)
    } else if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    Line::from(Span::styled(line.to_string(), style))
}

/// Renders a scrollable diff-text pane, clamping `scroll` to the content's
/// actual length now that it's known, so scrolling past the end just stops.
pub fn render_text_pane(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    border_style: Style,
    lines: Vec<Line<'static>>,
    scroll: &mut u16,
) {
    let visible_height = area.height.saturating_sub(2); // minus borders
    let max_scroll = (lines.len() as u16).saturating_sub(visible_height);
    *scroll = (*scroll).min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((*scroll, 0));
    frame.render_widget(paragraph, area);
}
