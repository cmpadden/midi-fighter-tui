use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

pub struct UiLayout {
    pub tabs: Rect,
    pub separator: Rect,
    pub status: Rect,
    pub main: Rect,
}

pub fn split(area: Rect) -> UiLayout {
    let area = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    UiLayout {
        tabs: vertical[0],
        separator: vertical[1],
        main: vertical[2],
        status: vertical[3],
    }
}
