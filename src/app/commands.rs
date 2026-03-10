use crate::{
    protocol::{
        apply_frame_to_snapshot, build_bulk_write_messages, build_write_messages, bulk_read_request,
        config_read_request, decode_bulk_transfer_chunk, device_inquiry_request, empty_snapshot,
        pad_color_tags, ReadCompleteness,
    },
    transport::{MidiTransport, TransportError},
};

use std::{collections::BTreeSet, thread, time::Duration};

use super::{AppMode, AppState, Screen};

pub fn refresh_devices<T: MidiTransport>(
    state: &mut AppState,
    transport: &mut T,
) -> Result<(), TransportError> {
    state.app_mode = AppMode::Scanning;
    state.clear_error();

    let previous_devices = state.devices.clone();
    let previous_id = state.connected_device_id.clone();
    let previous_selected_id = state.selected_device().map(|device| device.id.clone());
    let devices = scan_devices_with_retry(transport)?;

    if devices.is_empty() && !previous_devices.is_empty() {
        state.devices = previous_devices;
        state.set_connected_device(previous_id);
        restore_selected_device(state, previous_selected_id.as_deref());
        state.app_mode = if state.connected_device_id.is_some() {
            AppMode::Connected
        } else {
            AppMode::Idle
        };
        state.set_status(
            "Rescan returned no Midi Fighter ports; keeping the last seen device list.",
        );
        state.push_event(
            "Rescan returned no Midi Fighter ports; preserved the previous device list.",
        );
        return Ok(());
    }

    state.devices = devices;
    state.set_connected_device(previous_id);
    restore_selected_device(state, previous_selected_id.as_deref());
    state.normalize_selection();

    if state.devices.is_empty() {
        state.app_mode = AppMode::Idle;
        state.set_status("No Midi Fighter ports detected.");
        state.push_event("Scan completed: no Midi Fighter ports detected.");
    } else {
        state.app_mode = if state.connected_device_id.is_some() {
            AppMode::Connected
        } else {
            AppMode::Idle
        };
        state.set_status(format!("Found {} candidate device(s).", state.devices.len()));
        state.push_event(format!("Scan completed: {} candidate device(s).", state.devices.len()));
    }

    Ok(())
}

fn scan_devices_with_retry<T: MidiTransport>(transport: &mut T) -> Result<Vec<crate::transport::DeviceRef>, TransportError> {
    const ATTEMPTS: usize = 4;
    const RETRY_DELAY_MS: u64 = 150;

    let mut devices = Vec::new();
    for attempt in 0..ATTEMPTS {
        devices = transport.scan_devices()?;
        if !devices.is_empty() {
            return Ok(devices);
        }

        if attempt + 1 < ATTEMPTS {
            thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
        }
    }

    Ok(devices)
}

fn restore_selected_device(state: &mut AppState, selected_id: Option<&str>) {
    let Some(selected_id) = selected_id else {
        return;
    };

    if let Some(index) = state.devices.iter().position(|device| device.id == selected_id) {
        state.selected_device_idx = index;
    }
}

pub fn connect_selected<T: MidiTransport>(
    state: &mut AppState,
    transport: &mut T,
) -> Result<(), TransportError> {
    let Some(device) = state.selected_device().cloned() else {
        state.set_status("No device selected.");
        return Ok(());
    };

    state.clear_error();
    state.app_mode = AppMode::ReadingConfig;
    state.set_status(format!("Connecting to {}...", device.name));

    let note = transport.connect(&device)?;

    state.set_connected_device(Some(device.id.clone()));
    state.snapshot = Some(empty_snapshot(device.family.clone()));
    state.pad_colors = None;
    state.staged_pad_colors = None;
    state.selected_pad_button_idx = 0;
    state.selected_pad_buttons.clear();
    state.pending_bulk_reads.clear();
    state.pad_color_reads_requested = false;
    state.staged_edits.clear();
    state.packet_log.clear();
    state.selected_setting_idx = 0;
    state.selected_raw_tag_idx = 0;
    state.selected_packet_idx = 0;
    state.app_mode = AppMode::ReadingConfig;
    state.screen = Screen::Settings;
    state.set_status(format!("Connected to {}. Requesting device settings...", device.name));
    state.push_event(format!("Connected to {}.", device.name));

    if let Some(note) = note {
        state.push_event(note);
    }

    let inquiry = transport.send(&device_inquiry_request())?;
    state.packet_log.push(inquiry);
    state.push_event(format!("Sent device inquiry to {}.", device.name));

    match config_read_request(&device.family) {
        Ok(request) => {
            let sent = transport.send(&request)?;
            state.packet_log.push(sent);
            state.push_event(format!("Sent tagged config read request to {}.", device.name));
            state.set_status(format!("Waiting for tagged config reply from {}...", device.name));
        }
        Err(err) => {
            state.app_mode = AppMode::Connected;
            state.set_status(err.to_string());
            state.push_event(format!("Config read unavailable: {err}"));
        }
    }

    Ok(())
}

pub fn disconnect<T: MidiTransport>(state: &mut AppState, transport: &mut T) -> Result<(), TransportError> {
    transport.disconnect()?;

    let disconnected = state
        .connected_device()
        .map(|device| device.name.clone())
        .unwrap_or_else(|| "device".to_string());

    state.set_connected_device(None);
    state.snapshot = None;
    state.pad_colors = None;
    state.staged_pad_colors = None;
    state.selected_pad_buttons.clear();
    state.pending_bulk_reads.clear();
    state.pad_color_reads_requested = false;
    state.staged_edits.clear();
    state.packet_log.clear();
    state.apply_modal_open = false;
    state.app_mode = AppMode::Idle;
    state.set_status("Disconnected.");
    state.push_event(format!("Disconnected from {disconnected}."));

    Ok(())
}

pub fn refresh_config<T: MidiTransport>(
    state: &mut AppState,
    transport: &mut T,
) -> Result<(), TransportError> {
    let Some(device) = state.connected_device().cloned() else {
        state.set_status("No connected device.");
        return Ok(());
    };

    state.clear_error();
    state.app_mode = AppMode::ReadingConfig;
    state.set_status(format!("Refreshing config from {}...", device.name));
    state.pad_colors = None;
    state.staged_pad_colors = None;
    state.selected_pad_buttons.clear();
    state.pending_bulk_reads.clear();
    state.pad_color_reads_requested = false;

    match config_read_request(&device.family) {
        Ok(request) => {
            let sent = transport.send(&request)?;
            state.packet_log.push(sent);
            state.set_status("Sent config read request. Waiting for tagged device reply.");
            state.push_event(format!("Sent tagged config read request to {}.", device.name));
        }
        Err(err) => {
            state.app_mode = AppMode::Connected;
            state.set_status(err.to_string());
            state.push_event(format!("Config refresh unavailable: {err}"));
        }
    }

    Ok(())
}

pub fn confirm_apply<T: MidiTransport>(
    state: &mut AppState,
    transport: &mut T,
) -> Result<(), TransportError> {
    if state.snapshot.is_none() && state.staged_pad_colors.is_none() {
        state.set_status("No config snapshot loaded.");
        return Ok(());
    }

    if state.staged_edits.is_empty() && state.dirty_pad_color_count() == 0 {
        state.apply_modal_open = false;
        state.set_status("No staged changes to apply.");
        return Ok(());
    }

    state.app_mode = AppMode::ApplyingChanges;
    let mut messages = Vec::new();

    if let Some(snapshot) = state.snapshot.as_ref() {
        if !state.staged_edits.is_empty() {
            let setting_messages = match build_write_messages(snapshot, &state.staged_edits) {
                Ok(messages) => messages,
                Err(err) => {
                    state.app_mode = AppMode::Connected;
                    state.set_status(err.to_string());
                    state.push_event(format!("Apply unavailable: {err}"));
                    return Ok(());
                }
            };
            messages.extend(setting_messages);
        }

        if let (Some(device_colors), Some(staged_colors)) =
            (state.pad_colors.as_ref(), state.staged_pad_colors.as_ref())
        {
            let mut writes = Vec::new();
            if staged_colors.inactive != device_colors.inactive {
                if let Some(buffer) = staged_colors.inactive.as_deref() {
                    writes.push((1u8, buffer));
                }
            }
            if staged_colors.active != device_colors.active {
                if let Some(buffer) = staged_colors.active.as_deref() {
                    writes.push((2u8, buffer));
                }
            }

            if !writes.is_empty() {
                let color_messages = match build_bulk_write_messages(&snapshot.family, &writes) {
                    Ok(messages) => messages,
                    Err(err) => {
                        state.app_mode = AppMode::Connected;
                        state.set_status(err.to_string());
                        state.push_event(format!("Apply unavailable: {err}"));
                        return Ok(());
                    }
                };
                messages.extend(color_messages);
            }
        }
    }

    for message in &messages {
        let frame = transport.send(message)?;
        state.packet_log.push(frame);
    }

    if let Some(snapshot) = state.snapshot.as_mut() {
        for field in &mut snapshot.fields {
            if let Some(next_value) = state.staged_edits.get(&field.id) {
                field.value = next_value.clone();
            }
        }
    }

    if let Some(staged_colors) = state.staged_pad_colors.take() {
        state.pad_colors = Some(staged_colors);
    }

    state.staged_edits.clear();
    state.apply_modal_open = false;
    state.app_mode = AppMode::Connected;
    state.set_status("Applied staged edits to the connected device.");
    state.push_event("Applied staged edits to the connected device.");

    Ok(())
}

pub fn poll_transport<T: MidiTransport>(
    state: &mut AppState,
    transport: &mut T,
) -> Result<(), TransportError> {
    let frames = transport.poll_packets()?;
    if frames.is_empty() {
        return Ok(());
    }

    let count = frames.len();
    let mut updated_tags = BTreeSet::new();
    let mut latest_snapshot = state.snapshot.clone();

    for frame in &frames {
        if let Some(snapshot) = latest_snapshot.as_ref() {
            if let Some(update) = apply_frame_to_snapshot(snapshot, frame) {
                updated_tags.extend(update.updated_tags.into_iter().map(|tag| tag.0));
                latest_snapshot = Some(update.snapshot);
            }
        }

        if let Some(chunk) = decode_bulk_transfer_chunk(frame) {
            let pending = state.pending_bulk_reads.entry(chunk.tag).or_default();
            if let Some(buffer) = pending.push_chunk(chunk.chunk_index, chunk.total_chunks, chunk.data) {
                let pad_colors = state.pad_colors.get_or_insert_with(Default::default);
                match chunk.tag {
                    1 => pad_colors.inactive = Some(buffer),
                    2 => pad_colors.active = Some(buffer),
                    _ => {}
                }
            }
        }
    }

    state.packet_log.extend(frames);
    if let Some(snapshot) = latest_snapshot {
        state.snapshot = Some(snapshot);
    }
    state.normalize_selection();

    if !updated_tags.is_empty() {
        let (known_fields, unknown_fields, completeness) = state
            .snapshot
            .as_ref()
            .map(|snapshot| {
                (
                    snapshot.fields.iter().filter(|field| field.known).count(),
                    snapshot.fields.iter().filter(|field| !field.known).count(),
                    match snapshot.completeness {
                        ReadCompleteness::Complete => "complete",
                        ReadCompleteness::Partial => "partial",
                    },
                )
            })
            .unwrap_or((0, 0, "partial"));

        state.app_mode = AppMode::Connected;
        state.push_event(format!(
            "Decoded {} tagged setting(s) into the current snapshot.",
            updated_tags.len()
        ));
        state.set_status(format!(
            "Loaded {} tagged setting(s): {} known, {} unknown, {} snapshot.",
            updated_tags.len(),
            known_fields,
            unknown_fields,
            completeness
        ));
    } else {
        state.push_event(format!("Captured {count} incoming MIDI message(s)."));
        state.set_status(format!("Captured {count} incoming MIDI message(s)."));
    }

    maybe_request_pad_colors(state, transport)?;
    maybe_report_loaded_pad_colors(state);

    Ok(())
}

fn maybe_request_pad_colors<T: MidiTransport>(
    state: &mut AppState,
    transport: &mut T,
) -> Result<(), TransportError> {
    let Some((family, completeness)) = state
        .snapshot
        .as_ref()
        .map(|snapshot| (snapshot.family.clone(), snapshot.completeness))
    else {
        return Ok(());
    };

    if state.pad_color_reads_requested || completeness != ReadCompleteness::Complete {
        return Ok(());
    }

    let Some((inactive_tag, active_tag)) = pad_color_tags(&family) else {
        return Ok(());
    };

    for (tag, label) in [(inactive_tag, "inactive"), (active_tag, "active")] {
        let request = match bulk_read_request(&family, tag) {
            Ok(request) => request,
            Err(err) => {
                state.push_event(format!("Pad color read unavailable: {err}"));
                return Ok(());
            }
        };
        let sent = transport.send(&request)?;
        state.packet_log.push(sent);
        state.push_event(format!("Requested {label} pad colors (bulk tag {tag})."));
    }

    state.pad_color_reads_requested = true;
    state.set_status("Requested pad color buffers. Waiting for bulk transfer replies.");
    Ok(())
}

fn maybe_report_loaded_pad_colors(state: &mut AppState) {
    let Some(pad_colors) = state.pad_colors.as_ref() else {
        return;
    };

    if pad_colors.loaded_buffers() == 2 {
        state.set_status("Loaded both pad color buffers from the device.");
    } else if pad_colors.loaded_buffers() == 1 {
        state.set_status("Loaded one pad color buffer; waiting for the second buffer.");
    }
}
