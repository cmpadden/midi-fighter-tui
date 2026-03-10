use ratatui::style::{Color, Modifier, Style};

use crate::protocol::Confidence;

pub fn accent() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn border() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

#[allow(dead_code)]
pub fn confidence(confidence: Confidence) -> Style {
    match confidence {
        Confidence::ObservedOnly => Style::default().fg(Color::Yellow),
        Confidence::Tentative => Style::default().fg(Color::LightBlue),
        Confidence::Verified => Style::default().fg(Color::Green),
    }
}
