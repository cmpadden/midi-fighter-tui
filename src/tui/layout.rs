use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
};

pub struct UiLayout {
    pub status: Rect,
    pub nav: Rect,
    pub main: Rect,
    pub bottom: Option<Rect>,
}

pub fn split(area: Rect, show_packet_pane: bool) -> UiLayout {
    let vertical = if show_packet_pane {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Length(9),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(10)])
            .split(area)
    };

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(10)])
        .split(vertical[1]);

    UiLayout {
        status: vertical[0],
        nav: body[0],
        main: body[1],
        bottom: if show_packet_pane {
            Some(vertical[2])
        } else {
            None
        },
    }
}
