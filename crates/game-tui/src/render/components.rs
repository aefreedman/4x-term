use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use unicode_width::UnicodeWidthStr;

#[must_use]
pub fn panel(title: impl Into<String>, focused: bool) -> Block<'static> {
    let marker = if focused { "> " } else { "" };
    let style = if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default()
        .title(format!(" {marker}{} ", title.into()))
        .borders(Borders::ALL)
        .border_style(style)
}

#[must_use]
pub fn selected_line(selected: bool, text: impl Into<String>) -> Line<'static> {
    let marker = if selected { "> " } else { "  " };
    let style = if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    Line::from(Span::styled(format!("{marker}{}", text.into()), style))
}

#[must_use]
pub fn truncate_cells(value: &str, maximum: usize) -> String {
    if UnicodeWidthStr::width(value) <= maximum {
        return value.to_owned();
    }
    if maximum <= 3 {
        return ".".repeat(maximum);
    }
    let target = maximum - 3;
    let mut result = String::new();
    for character in value.chars() {
        let mut candidate = result.clone();
        candidate.push(character);
        if UnicodeWidthStr::width(candidate.as_str()) > target {
            break;
        }
        result.push(character);
    }
    result.push_str("...");
    result
}

#[must_use]
pub fn unavailable(reason: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "[UNAVAILABLE] ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(reason.to_owned()),
    ])
}
