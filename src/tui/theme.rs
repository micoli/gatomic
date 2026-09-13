use ratatui::style::{Color, Modifier, Style};

/// Dark orange, bold — used for the border of the currently active pane so
/// it stands out clearly against the other two panes.
const ACTIVE_PANE_COLOR: Color = Color::Rgb(204, 85, 0);

pub fn active_pane_style() -> Style {
    Style::default()
        .fg(ACTIVE_PANE_COLOR)
        .add_modifier(Modifier::BOLD)
}

pub fn pane_title_style(is_active: bool) -> Style {
    if is_active {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}
