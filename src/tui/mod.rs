mod layout;
mod theme;

use ratatui::{
    layout::{Constraint, Direction as LayoutDirection, Layout, Rect},
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::{
    app::{
        apply::build_apply_messages,
        pad_grid::{bank_slot_for_button, button_for_image_index},
        AppState, ColorTarget, Screen,
    },
    protocol::{DecodedField, FieldValue, SysexFrame},
};

pub fn render(frame: &mut Frame, state: &AppState) {
    let areas = layout::split(frame.area());
    render_tabs(frame, areas.tabs, state);
    render_separator(frame, areas.separator);
    render_status(frame, areas.status, state);
    render_main(frame, areas.main, state);

    if state.apply_modal_open {
        render_apply_modal(frame, state);
    } else if state.help_modal_open {
        render_help_modal(frame);
    }
}

fn render_status(frame: &mut Frame, area: Rect, state: &AppState) {
    let connected = state
        .connected_device()
        .map(|device| device.name.as_str())
        .unwrap_or("none");
    let key_hint = if matches!(state.screen, Screen::PadColors) {
        "<tab> sections <q> quit <r> scan <g> read <p> activity <?> help <arrows> move <shift+arrows> sweep <b> bank <space> mark <enter> layer <[ ]> palette <x> clear <u> undo <a> apply"
    } else {
        "<tab> sections <q> quit <r> scan <g> read <p> activity <?> help <up/down> move <left/right> edit <enter> select <a> apply"
    };

    let lines = vec![
        Line::from({
            let mut spans = vec![
                Span::styled("mode:", theme::footer_label()),
                Span::styled(state.app_mode.label(), theme::footer()),
                Span::styled("  device:", theme::footer_label()),
                Span::styled(connected, theme::footer()),
            ];
            if state.dirty_count() > 0 {
                spans.push(Span::styled("  Unapplied Changes", theme::footer_label()));
            }
            spans
        }),
        Line::from(Span::styled(key_hint, theme::footer())),
    ];

    frame.render_widget(Paragraph::new(lines).style(theme::footer()), area);
}

fn render_tabs(frame: &mut Frame, area: Rect, state: &AppState) {
    let titles = Screen::ALL
        .iter()
        .map(|screen| Line::from(format!(" {} ", screen.title())))
        .collect::<Vec<_>>();
    let selected = Screen::ALL
        .iter()
        .position(|screen| *screen == state.screen)
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(theme::border())
        .highlight_style(theme::selected())
        .divider("");
    frame.render_widget(tabs, area);
}

fn render_separator(frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(theme::border()),
        area,
    );
}

fn render_main(frame: &mut Frame, area: Rect, state: &AppState) {
    match state.screen {
        Screen::Devices => render_devices_screen(frame, area, state),
        Screen::Settings => render_settings_screen(frame, area, state),
        Screen::PadColors => render_pad_colors_screen(frame, area, state),
        Screen::PacketLog => render_packet_log_screen(frame, area, state),
    }
}

fn render_devices_screen(frame: &mut Frame, area: Rect, state: &AppState) {
    let device_items = if state.devices.is_empty() {
        vec![ListItem::new(
            "No Midi Fighter ports found. Press r to scan again.",
        )]
    } else {
        state
            .devices
            .iter()
            .enumerate()
            .map(|(index, device)| {
                let marker = if Some(&device.id) == state.connected_device_id.as_ref() {
                    "connected"
                } else {
                    "idle"
                };
                let style = if index == state.selected_device_idx {
                    theme::selected()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{} [{}]", device.name, marker),
                    style,
                )))
            })
            .collect()
    };

    let max_list_height = area.height.saturating_sub(8).max(3);
    let list_height = (device_items.len() as u16 + 2).max(3).min(max_list_height);
    let sections = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(list_height),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    render_list_section(frame, sections[0], "Detected Devices", device_items);

    let detail_lines = if let Some(device) = state.selected_device() {
        vec![
            Line::from(format!("Family: {}", device.family)),
            Line::from(format!(
                "Protocol confidence: {}",
                device.protocol_confidence
            )),
            Line::from(format!(
                "Input port: {}",
                device.input_port.as_deref().unwrap_or("missing")
            )),
            Line::from(format!(
                "Output port: {}",
                device.output_port.as_deref().unwrap_or("missing")
            )),
            Line::from(""),
            Line::from("Enter: connect"),
            Line::from("r: rescan ports"),
            Line::from("d: disconnect current device"),
        ]
    } else {
        vec![
            Line::from("No device selected."),
            Line::from("Press r to scan available MIDI ports."),
        ]
    };

    render_text_section(
        frame,
        sections[2],
        "Device Detail",
        detail_lines,
        true,
        false,
    );
}

fn render_settings_screen(frame: &mut Frame, area: Rect, state: &AppState) {
    let known_fields = state.known_fields();
    let items = if known_fields.is_empty() {
        vec![ListItem::new(
            "Connect a device to inspect supported settings.",
        )]
    } else {
        known_fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let current = state.current_value_for(field);
                let dirty = if state.staged_edits.contains_key(&field.id) {
                    " *"
                } else {
                    ""
                };
                let style = if index == state.selected_setting_idx {
                    theme::selected()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{}: {}{}", field.label, current, dirty),
                    style,
                )))
            })
            .collect()
    };

    let max_list_height = area.height.saturating_sub(8).max(3);
    let list_height = (items.len() as u16 + 2).max(3).min(max_list_height);
    let sections = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(list_height),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    render_list_section(frame, sections[0], "Known Settings", items);

    let detail = if let Some(field) = state.selected_known_field() {
        let current = state.current_value_for(field);
        field_detail_lines(field, &current, state.staged_edits.contains_key(&field.id))
    } else if matches!(state.app_mode, crate::app::AppMode::ReadingConfig) {
        vec![
            Line::from("Waiting for a tagged config reply from the device."),
            Line::from(""),
            Line::from("The TUI has already sent the vendor config-read request."),
            Line::from("If nothing appears, press g to retry and watch Activity Log for replies."),
        ]
    } else {
        vec![
            Line::from("No tagged settings decoded yet."),
            Line::from(""),
            Line::from("Connect a supported device and press g to request the current config."),
            Line::from(
                "Activity Log still shows the raw SysEx frames used to populate this screen.",
            ),
        ]
    };

    render_text_section(frame, sections[2], "Setting Detail", detail, true, false);
}

fn render_pad_colors_screen(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(device_pad_colors) = state.pad_colors.as_ref() else {
        let body = vec![
            Line::from("Pad colors have not been loaded yet."),
            Line::from(""),
            Line::from("MF64 color buffers are requested automatically after the base config snapshot completes."),
            Line::from("If needed, reconnect the device or press g to reread config."),
        ];
        render_text_section(frame, area, "Pad Colors", body, true, false);
        return;
    };

    let pad_colors = state.current_pad_colors().unwrap_or(device_pad_colors);
    let sections = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);

    let selected_bank = state.selected_pad_bank + 1;
    let selected_pad_in_bank = bank_slot_for_button(state.selected_pad_button_idx);
    let selected_rgb = match state.selected_color_target {
        ColorTarget::Inactive => rgb_for_button(
            pad_colors.inactive.as_deref(),
            state.selected_pad_button_idx,
        ),
        ColorTarget::Active => {
            rgb_for_button(pad_colors.active.as_deref(), state.selected_pad_button_idx)
        }
    };
    let summary = vec![
        Line::from(format!(
            "Inactive buffer: {}",
            if pad_colors.inactive.is_some() {
                "loaded"
            } else {
                "pending"
            }
        )),
        Line::from(format!(
            "Active buffer: {}",
            if pad_colors.active.is_some() {
                "loaded"
            } else {
                "pending"
            }
        )),
        Line::from(format!(
            "Bank: {}    Selected pad: {} (slot {})",
            selected_bank, state.selected_pad_button_idx, selected_pad_in_bank
        )),
        Line::from(format!(
            "Target: {}    Color: #{:02X}{:02X}{:02X}    Dirty pads: {}    Selection: {}",
            state.selected_color_target.label(),
            selected_rgb.0,
            selected_rgb.1,
            selected_rgb.2,
            state.dirty_pad_color_count(),
            state.selected_pad_buttons.len()
        )),
        Line::from(format!("Selection detail: {}", selected_pad_summary(state))),
    ];
    render_text_section(frame, sections[0], "Pad Color Status", summary, true, false);

    let grids = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(sections[1]);

    render_color_bank(
        frame,
        grids[0],
        &format!("Inactive Colors Bank {}", selected_bank),
        pad_colors.inactive.as_deref(),
        device_pad_colors.inactive.as_deref(),
        state.selected_pad_bank,
        ColorTarget::Inactive,
        state,
    );
    render_color_bank(
        frame,
        grids[1],
        &format!("Active Colors Bank {}", selected_bank),
        pad_colors.active.as_deref(),
        device_pad_colors.active.as_deref(),
        state.selected_pad_bank,
        ColorTarget::Active,
        state,
    );
}

fn render_packet_log_screen(frame: &mut Frame, area: Rect, state: &AppState) {
    let items = if state.packet_log.is_empty() {
        vec![ListItem::new("No packets captured yet.")]
    } else {
        state
            .packet_log
            .iter()
            .enumerate()
            .map(|(index, frame_data)| {
                let style = if index == state.selected_packet_idx {
                    theme::selected()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(
                    format!(
                        "{} {} {}",
                        frame_data.timestamp,
                        frame_data.direction,
                        frame_data
                            .envelope
                            .as_ref()
                            .map(|envelope| format!("cmd:{:02X}", envelope.command))
                            .unwrap_or_else(|| "raw".into())
                    ),
                    style,
                )))
            })
            .collect()
    };

    let min_detail_height = 8;
    let min_activity_height = 4;
    let max_packet_list_height = area
        .height
        .saturating_sub(min_detail_height + min_activity_height)
        .max(4);
    let packet_list_height = (items.len() as u16 + 2).max(4).min(max_packet_list_height);

    let events = if state.event_log.is_empty() {
        vec![ListItem::new("No activity recorded yet.")]
    } else {
        state
            .event_log
            .iter()
            .rev()
            .take(8)
            .map(|line| ListItem::new(line.clone()))
            .collect::<Vec<_>>()
    };
    let max_activity_height = area
        .height
        .saturating_sub(packet_list_height + min_detail_height)
        .max(min_activity_height);
    let activity_height = (events.len() as u16 + 2)
        .max(min_activity_height)
        .min(10)
        .min(max_activity_height);
    let sections = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(packet_list_height),
            Constraint::Min(0),
            Constraint::Length(activity_height),
        ])
        .split(area);

    render_list_section(frame, sections[0], "Packets", items);

    let detail = state
        .packet_log
        .get(state.selected_packet_idx)
        .map(packet_detail_lines)
        .unwrap_or_else(|| vec![Line::from("No packet detail available.")]);

    render_text_section(frame, sections[1], "Packet Detail", detail, false, false);

    render_list_section(frame, sections[2], "Activity", events);
}

fn render_help_modal(frame: &mut Frame) {
    let area = centered_rect(frame.area(), 72, 70);
    frame.render_widget(Clear, area);

    let body = vec![
        Line::from("Global"),
        Line::from("  q / Ctrl-C  quit"),
        Line::from("  Tab         next section"),
        Line::from("  Shift-Tab   previous section"),
        Line::from("  p           jump to Activity Log"),
        Line::from("  ? / Esc     close help"),
        Line::from(""),
        Line::from("Device workflow"),
        Line::from("  r           rescan MIDI ports"),
        Line::from("  Enter       connect selected device"),
        Line::from("  d           disconnect"),
        Line::from("  g           refresh snapshot"),
        Line::from("  Tab         visit Pad Colors to inspect bulk color buffers"),
        Line::from(""),
        Line::from("Settings workflow"),
        Line::from("  Up/Down     move selection"),
        Line::from("  Left/Right  adjust selected setting"),
        Line::from("  Space       toggle or cycle selected setting"),
        Line::from("  u / U       undo current edit / discard all edits"),
        Line::from("  a           open apply preview"),
        Line::from(""),
        Line::from("Pad color workflow"),
        Line::from("  Arrow keys  move the pad cursor across the active 8x8 bank"),
        Line::from("  Shift+Arrows extend selection while moving"),
        Line::from("  b           toggle between bank 1 and bank 2"),
        Line::from("  Space       toggle the current pad into the active selection"),
        Line::from("  x           clear the active pad selection"),
        Line::from("  [ / ]       cycle the palette for the current selection"),
        Line::from("  Enter       toggle active vs inactive color layer"),
        Line::from("  u           undo the current selection"),
    ];

    frame.render_widget(
        Paragraph::new(body)
            .block(themed_block("Help"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_apply_modal(frame: &mut Frame, state: &AppState) {
    let area = centered_rect(frame.area(), 72, 60);
    frame.render_widget(Clear, area);

    let preview_frames = build_apply_messages(state);

    let mut lines = vec![
        Line::from(Span::styled(
            "Review staged changes",
            theme::accent().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (field_id, value) in &state.staged_edits {
        lines.push(Line::from(format!("{field_id} -> {value}")));
    }
    if state.dirty_pad_color_count() > 0 {
        lines.push(Line::from(format!(
            "pad colors -> {} changed pad(s)",
            state.dirty_pad_color_count()
        )));
    }

    match preview_frames {
        Some(Ok(frames)) => {
            lines.extend([
                Line::from(""),
                Line::from(format!("Preview packets: {}", frames.len())),
                Line::from("Enter: apply to the connected device"),
                Line::from("Esc: cancel"),
            ]);
        }
        Some(Err(err)) => {
            lines.extend([
                Line::from(""),
                Line::from(err.to_string()),
                Line::from("Enter is disabled until the serializer is decoded."),
                Line::from("Esc: cancel"),
            ]);
        }
        None => {
            lines.extend([
                Line::from(""),
                Line::from("No config snapshot loaded."),
                Line::from("Esc: cancel"),
            ]);
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(themed_block("Apply Preview").border_style(theme::accent()))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn field_detail_lines(
    field: &DecodedField,
    current: &FieldValue,
    dirty: bool,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(format!("Label: {}", field.label)),
        Line::from(format!("Field id: {}", field.id)),
        Line::from(format!("Tag: {}", field.tag_id)),
        Line::from(format!("Current value: {}", current)),
        Line::from(format!("Device value: {}", field.value)),
        Line::from(format!("Confidence: {}", field.confidence)),
        Line::from(format!(
            "Editable: {}",
            if field.editable { "yes" } else { "no" }
        )),
        Line::from(format!("Dirty: {}", if dirty { "yes" } else { "no" })),
        Line::from(format!("Source bytes: {}", hex_bytes(&field.source_bytes))),
    ];

    if let Some(notes) = &field.notes {
        lines.push(Line::from(""));
        lines.push(Line::from(notes.clone()));
    }

    lines
}

fn packet_detail_lines(frame_data: &SysexFrame) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(format!("Timestamp: {}", frame_data.timestamp)),
        Line::from(format!("Direction: {}", frame_data.direction)),
        Line::from(format!("Raw: {}", frame_data.hex_string())),
    ];

    if let Some(envelope) = &frame_data.envelope {
        lines.push(Line::from(format!(
            "Manufacturer: {}",
            hex_bytes(&envelope.manufacturer)
        )));
        lines.push(Line::from(format!("Command: {:02X}", envelope.command)));
        if let Some(tag) = envelope.tag {
            lines.push(Line::from(format!("Tag: {tag}")));
        }
        if let Some(note) = &envelope.note {
            lines.push(Line::from(""));
            lines.push(Line::from(note.clone()));
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Raw MIDI message captured. Not a complete SysEx frame.",
        ));
    }

    lines
}

fn render_color_bank(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    buffer: Option<&[u8]>,
    _device_buffer: Option<&[u8]>,
    bank: usize,
    target: ColorTarget,
    state: &AppState,
) {
    let Some(buffer) = buffer else {
        render_text_section(
            frame,
            area,
            title,
            vec![Line::from("Buffer pending.")],
            true,
            false,
        );
        return;
    };

    let mut lines = vec![Line::from(Span::styled(
        title.to_string(),
        theme::accent().add_modifier(Modifier::BOLD),
    ))];
    lines.extend(
        (0..8)
            .map(|row| {
                let spans = (0..8)
                    .flat_map(|col| {
                        let image_index = bank * 64 + row * 8 + col;
                        let button_index = button_for_image_index(image_index);
                        let color = color_for_button(buffer, button_index);
                        let is_active_layer = state.selected_color_target == target;
                        let is_cursor =
                            is_active_layer && state.selected_pad_button_idx == button_index;
                        let is_selected =
                            is_active_layer && state.selected_pad_buttons.contains(&button_index);
                        let marker_style = match (is_cursor, is_selected) {
                            (true, true) => theme::selected(),
                            (true, false) => theme::selected(),
                            (false, true) => Style::default().bg(Color::Gray),
                            (false, false) => Style::default(),
                        };
                        [
                            Span::styled(" ", marker_style),
                            Span::styled("   ", Style::default().bg(color)),
                            Span::styled(" ", marker_style),
                            Span::raw(" "),
                        ]
                    })
                    .collect::<Vec<_>>();
                Line::from(spans)
            })
            .collect::<Vec<_>>(),
    );

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn color_for_button(buffer: &[u8], button_index: usize) -> Color {
    let rgb = rgb_for_button(Some(buffer), button_index);
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

fn rgb_for_button(buffer: Option<&[u8]>, button_index: usize) -> (u8, u8, u8) {
    let start = button_index * 3;
    let rgb = buffer
        .and_then(|bytes| bytes.get(start..start + 3))
        .map(|bytes| [bytes[0], bytes[1], bytes[2]])
        .unwrap_or([0, 0, 0]);
    (
        rgb[0].saturating_mul(2),
        rgb[1].saturating_mul(2),
        rgb[2].saturating_mul(2),
    )
}

fn selected_pad_summary(state: &AppState) -> String {
    if state.selected_pad_buttons.is_empty() {
        return "none; cursor only".into();
    }

    let mut pads = state
        .selected_pad_buttons
        .iter()
        .copied()
        .collect::<Vec<_>>();
    pads.sort_unstable();

    let summary = pads
        .iter()
        .take(8)
        .map(|pad| pad.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if pads.len() > 8 {
        format!("{summary}, ... ({} total)", pads.len())
    } else {
        summary
    }
}

fn themed_block<'a>(title: &'a str) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::border())
}

fn render_text_section(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    trim: bool,
    bordered: bool,
) {
    let paragraph = if bordered {
        Paragraph::new(lines)
            .block(themed_block(title))
            .wrap(Wrap { trim })
    } else {
        let mut body = vec![
            Line::from(Span::styled(
                title.to_string(),
                theme::accent().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        body.extend(lines);
        Paragraph::new(body).wrap(Wrap { trim })
    };

    frame.render_widget(paragraph, area);
}

fn render_list_section(frame: &mut Frame, area: Rect, title: &str, items: Vec<ListItem<'static>>) {
    let sections = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            title.to_string(),
            theme::accent().add_modifier(Modifier::BOLD),
        ))),
        sections[0],
    );
    frame.render_widget(List::new(items), sections[1]);
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
