use crate::protocol::{DecodedField, FieldValue};

use super::{Action, AppMode, AppState, ColorTarget, Screen};

const MF64_IMAGE_TO_BUTTON_ID: [usize; 128] = [
    28, 29, 30, 31, 60, 61, 62, 63, 24, 25, 26, 27, 56, 57, 58, 59, 20, 21, 22, 23, 52, 53, 54, 55,
    16, 17, 18, 19, 48, 49, 50, 51, 12, 13, 14, 15, 44, 45, 46, 47, 8, 9, 10, 11, 40, 41, 42, 43,
    4, 5, 6, 7, 36, 37, 38, 39, 0, 1, 2, 3, 32, 33, 34, 35, 92, 93, 94, 95, 124, 125, 126, 127, 88,
    89, 90, 91, 120, 121, 122, 123, 84, 85, 86, 87, 116, 117, 118, 119, 80, 81, 82, 83, 112, 113,
    114, 115, 76, 77, 78, 79, 108, 109, 110, 111, 72, 73, 74, 75, 104, 105, 106, 107, 68, 69, 70,
    71, 100, 101, 102, 103, 64, 65, 66, 67, 96, 97, 98, 99,
];

pub fn reduce(state: &mut AppState, action: Action) {
    match action {
        Action::Tick => {}
        Action::Quit => state.should_quit = true,
        Action::NextScreen => {
            if !state.apply_modal_open && !state.help_modal_open {
                state.screen = state.screen.next();
            }
        }
        Action::PrevScreen => {
            if !state.apply_modal_open && !state.help_modal_open {
                state.screen = state.screen.prev();
            }
        }
        Action::MoveLeft => move_horizontal(state, -1),
        Action::MoveRight => move_horizontal(state, 1),
        Action::MoveUp => move_selection(state, -1),
        Action::MoveDown => move_selection(state, 1),
        Action::AdjustSelected(delta) => stage_adjustment(state, delta),
        Action::ActivateSelected => activate_selected(state),
        Action::ShowPacketLog => state.screen = Screen::PacketLog,
        Action::ToggleApplyModal => {
            if state.dirty_count() > 0 {
                state.apply_modal_open = !state.apply_modal_open;
            } else {
                state.set_status("No staged changes to apply.");
            }
        }
        Action::CancelModal => {
            if state.apply_modal_open {
                state.apply_modal_open = false;
            } else if state.help_modal_open {
                state.help_modal_open = false;
            }
        }
        Action::TogglePadSelection => toggle_pad_selection(state),
        Action::TogglePadBank => toggle_pad_bank(state),
        Action::ClearPadSelection => clear_pad_selection(state),
        Action::UndoField => undo_current_field(state),
        Action::DiscardAll => {
            state.staged_edits.clear();
            state.staged_pad_colors = None;
            state.apply_modal_open = false;
            state.set_status("Discarded all staged edits.");
            state.push_event("Discarded all staged edits.");
        }
        Action::ShowHelp => state.help_modal_open = true,
        Action::RefreshDevices
        | Action::ConnectSelected
        | Action::Disconnect
        | Action::RefreshConfig
        | Action::ConfirmApply => {}
    }

    sync_mode(state);
    state.normalize_selection();
}

fn move_selection(state: &mut AppState, delta: isize) {
    if matches!(state.screen, Screen::PadColors) {
        move_pad_cursor(state, 0, delta);
        return;
    }

    let max = match state.screen {
        Screen::Devices => state.devices.len(),
        Screen::Settings => state.known_fields().len(),
        Screen::PadColors => 0,
        Screen::PacketLog => state.packet_log.len(),
        Screen::ImportExport => 0,
    };

    let target = match state.screen {
        Screen::Devices => &mut state.selected_device_idx,
        Screen::Settings => &mut state.selected_setting_idx,
        Screen::PadColors => return,
        Screen::PacketLog => &mut state.selected_packet_idx,
        Screen::ImportExport => return,
    };

    if max == 0 {
        *target = 0;
        return;
    }

    let current = *target as isize + delta;
    *target = current.clamp(0, max as isize - 1) as usize;
}

fn move_horizontal(state: &mut AppState, delta: isize) {
    if !matches!(state.screen, Screen::PadColors) {
        return;
    }

    move_pad_cursor(state, delta, 0);
}

fn activate_selected(state: &mut AppState) {
    if matches!(state.screen, Screen::PadColors) {
        state.selected_color_target = state.selected_color_target.toggle();
        state.set_status(format!(
            "Pad color target set to {}.",
            state.selected_color_target.label()
        ));
        return;
    }

    stage_value_change(state, |field, current| field.kind.activate(current));
}

fn stage_adjustment(state: &mut AppState, delta: i32) {
    if matches!(state.screen, Screen::PadColors) {
        stage_pad_color_adjustment(state, delta);
        return;
    }

    stage_value_change(state, |field, current| field.kind.adjust(current, delta));
}

fn stage_value_change<F>(state: &mut AppState, change: F)
where
    F: Fn(&DecodedField, &FieldValue) -> Option<FieldValue>,
{
    if state.apply_modal_open || !matches!(state.screen, Screen::Settings) {
        return;
    }

    let Some(field) = state.selected_known_field().cloned() else {
        return;
    };

    if !field.editable {
        state.set_status(format!("{} is not editable yet.", field.label));
        return;
    }

    let current = state.current_value_for(&field);
    let Some(next_value) = change(&field, &current) else {
        state.set_status(format!(
            "{} cannot be adjusted from the TUI yet.",
            field.label
        ));
        return;
    };

    if next_value == field.value {
        state.staged_edits.remove(&field.id);
        state.set_status(format!("Reverted {} to device value.", field.label));
        state.push_event(format!("Reverted {}", field.label));
    } else {
        state
            .staged_edits
            .insert(field.id.clone(), next_value.clone());
        state.set_status(format!("Staged {} = {}", field.label, next_value));
        state.push_event(format!("staged {} = {}", field.id, next_value));
    }
}

fn undo_current_field(state: &mut AppState) {
    if matches!(state.screen, Screen::PadColors) {
        undo_current_pad_color(state);
        return;
    }

    let Some(field) = state.selected_known_field().cloned() else {
        return;
    };

    if state.staged_edits.remove(&field.id).is_some() {
        state.set_status(format!("Removed staged edit for {}.", field.label));
        state.push_event(format!("Removed staged edit for {}", field.id));
    }
}

fn stage_pad_color_adjustment(state: &mut AppState, delta: i32) {
    const PALETTE: &[(u8, u8, u8)] = &[
        (127, 0, 0),
        (105, 31, 0),
        (84, 66, 0),
        (66, 84, 0),
        (0, 127, 0),
        (0, 79, 79),
        (0, 0, 127),
        (66, 18, 84),
        (95, 0, 47),
        (63, 63, 63),
        (63, 0, 0),
        (52, 15, 0),
        (42, 31, 0),
        (31, 42, 0),
        (0, 63, 0),
        (0, 39, 39),
        (0, 0, 63),
        (34, 7, 44),
        (47, 0, 23),
        (0, 0, 0),
    ];

    let Some(device_colors) = state.pad_colors.as_ref().cloned() else {
        state.set_status("Pad color buffers are not loaded yet.");
        return;
    };
    let targets = targeted_pad_buttons(state);
    if targets.is_empty() {
        state.set_status("Select one or more pads before changing colors.");
        return;
    }

    let first_button = targets[0];
    let current_bytes = match state.selected_color_target {
        ColorTarget::Inactive => state
            .current_pad_colors()
            .and_then(|colors| colors.inactive.as_deref())
            .and_then(|buffer| buffer.get(first_button * 3..first_button * 3 + 3)),
        ColorTarget::Active => state
            .current_pad_colors()
            .and_then(|colors| colors.active.as_deref())
            .and_then(|buffer| buffer.get(first_button * 3..first_button * 3 + 3)),
    };
    let Some(current_bytes) = current_bytes else {
        state.set_status("Selected color buffer is not loaded yet.");
        return;
    };

    let current = (current_bytes[0], current_bytes[1], current_bytes[2]);
    let current_index = PALETTE
        .iter()
        .position(|entry| *entry == current)
        .unwrap_or(0) as i32;
    let next_index = (current_index + delta).rem_euclid(PALETTE.len() as i32) as usize;
    let next = PALETTE[next_index];

    {
        let staged = state
            .staged_pad_colors
            .get_or_insert_with(|| device_colors.clone());

        let buffer = match state.selected_color_target {
            ColorTarget::Inactive => staged.inactive.as_mut(),
            ColorTarget::Active => staged.active.as_mut(),
        };

        let Some(buffer) = buffer else {
            state.set_status("Selected color buffer is not loaded yet.");
            return;
        };

        for button_index in &targets {
            let start = button_index * 3;
            if start + 2 >= buffer.len() {
                continue;
            }
            buffer[start] = next.0;
            buffer[start + 1] = next.1;
            buffer[start + 2] = next.2;
        }
    }

    let matches_device = state
        .pad_colors
        .as_ref()
        .zip(state.staged_pad_colors.as_ref())
        .map(|(device, staged)| device == staged)
        .unwrap_or(false);

    if matches_device {
        state.staged_pad_colors = None;
        state.set_status("Reverted staged pad color edits.");
        state.push_event("Reverted staged pad color edits.");
    } else {
        state.set_status(format!(
            "Staged {} color for {} pad(s) = #{:02X}{:02X}{:02X}",
            state.selected_color_target.label(),
            targets.len(),
            next.0.saturating_mul(2),
            next.1.saturating_mul(2),
            next.2.saturating_mul(2)
        ));
        state.push_event(format!(
            "staged {} color for {} pad(s)",
            state.selected_color_target.label(),
            targets.len()
        ));
    }
}

fn undo_current_pad_color(state: &mut AppState) {
    let Some(device_colors) = state.pad_colors.as_ref().cloned() else {
        return;
    };
    let targets = targeted_pad_buttons(state);
    if targets.is_empty() {
        state.set_status("Select one or more pads before undoing colors.");
        return;
    }
    let Some(staged) = state.staged_pad_colors.as_mut() else {
        return;
    };

    let (device_buffer, staged_buffer) = match state.selected_color_target {
        ColorTarget::Inactive => (device_colors.inactive.as_deref(), staged.inactive.as_mut()),
        ColorTarget::Active => (device_colors.active.as_deref(), staged.active.as_mut()),
    };

    let (Some(device_buffer), Some(staged_buffer)) = (device_buffer, staged_buffer) else {
        return;
    };

    for button_index in &targets {
        let start = button_index * 3;
        if let (Some(device_bytes), Some(staged_bytes)) = (
            device_buffer.get(start..start + 3),
            staged_buffer.get_mut(start..start + 3),
        ) {
            staged_bytes.copy_from_slice(device_bytes);
        }
    }

    let matches_device = state
        .pad_colors
        .as_ref()
        .zip(state.staged_pad_colors.as_ref())
        .map(|(device, staged)| device == staged)
        .unwrap_or(false);

    if matches_device {
        state.staged_pad_colors = None;
    }

    state.set_status(format!(
        "Removed staged {} color for {} pad(s).",
        state.selected_color_target.label(),
        targets.len()
    ));
    state.push_event(format!(
        "Removed staged {} color for {} pad(s)",
        state.selected_color_target.label(),
        targets.len()
    ));
}

fn toggle_pad_selection(state: &mut AppState) {
    if !matches!(state.screen, Screen::PadColors) {
        return;
    }

    if !state
        .selected_pad_buttons
        .insert(state.selected_pad_button_idx)
    {
        state
            .selected_pad_buttons
            .remove(&state.selected_pad_button_idx);
    }

    state.set_status(format!(
        "Selected {} pad(s) for bulk color edits.",
        state.selected_pad_buttons.len()
    ));
}

fn clear_pad_selection(state: &mut AppState) {
    if !matches!(state.screen, Screen::PadColors) {
        return;
    }

    state.selected_pad_buttons.clear();
    state.set_status("Cleared pad selection.");
}

fn toggle_pad_bank(state: &mut AppState) {
    if !matches!(state.screen, Screen::PadColors) {
        return;
    }

    let image_index = MF64_IMAGE_TO_BUTTON_ID
        .iter()
        .position(|button| *button == state.selected_pad_button_idx)
        .unwrap_or(0);
    let slot = image_index % 64;

    state.selected_pad_bank = (state.selected_pad_bank + 1) % 2;
    state.selected_pad_button_idx = MF64_IMAGE_TO_BUTTON_ID[state.selected_pad_bank * 64 + slot];
    state.selected_pad_buttons.clear();
    state.set_status(format!(
        "Switched to pad color bank {}.",
        state.selected_pad_bank + 1
    ));
    state.push_event(format!(
        "Switched to pad color bank {}.",
        state.selected_pad_bank + 1
    ));
}

fn targeted_pad_buttons(state: &AppState) -> Vec<usize> {
    if state.selected_pad_buttons.is_empty() {
        Vec::new()
    } else {
        state.selected_pad_buttons.iter().copied().collect()
    }
}

fn move_pad_cursor(state: &mut AppState, dx: isize, dy: isize) {
    let image_index = MF64_IMAGE_TO_BUTTON_ID
        .iter()
        .position(|button| *button == state.selected_pad_button_idx)
        .unwrap_or(0);
    let bank_offset = state.selected_pad_bank * 64;
    let bank_image_index = image_index.saturating_sub(bank_offset).min(63);
    let row = bank_image_index / 8;
    let col = image_index % 8;
    let next_row = (row as isize + dy).clamp(0, 7) as usize;
    let next_col = (col as isize + dx).clamp(0, 7) as usize;
    let next_image_index = bank_offset + next_row * 8 + next_col;
    state.selected_pad_button_idx = MF64_IMAGE_TO_BUTTON_ID[next_image_index];
}

fn sync_mode(state: &mut AppState) {
    if state.connected_device_id.is_none() {
        if !matches!(
            state.app_mode,
            AppMode::Scanning | AppMode::Error | AppMode::Booting
        ) {
            state.app_mode = AppMode::Idle;
        }
        return;
    }

    if state.dirty_count() > 0 {
        state.app_mode = AppMode::Editing;
    } else if matches!(state.app_mode, AppMode::Editing) {
        state.app_mode = AppMode::Connected;
    }
}
