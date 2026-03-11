use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct UiLayout {
    pub tabs: Rect,
    pub status: Rect,
    pub main: Rect,
}

pub fn split(area: Rect) -> UiLayout {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    UiLayout {
        tabs: vertical[0],
        main: vertical[1],
        status: vertical[2],
    }
}
