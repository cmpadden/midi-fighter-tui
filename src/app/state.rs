use std::collections::{BTreeMap, BTreeSet};

use crate::{
    protocol::{ConfigSnapshot, DecodedField, FieldValue, SysexFrame},
    transport::{ConnectionState, DeviceRef},
};

#[derive(Clone, Debug, Default)]
pub struct PendingBulkRead {
    pub total_chunks: u8,
    pub chunks: BTreeMap<u8, Vec<u8>>,
}

impl PendingBulkRead {
    pub fn push_chunk(
        &mut self,
        chunk_index: u8,
        total_chunks: u8,
        data: Vec<u8>,
    ) -> Option<Vec<u8>> {
        self.total_chunks = total_chunks;
        self.chunks.insert(chunk_index, data);

        if self.total_chunks == 0 || self.chunks.len() != self.total_chunks as usize {
            return None;
        }

        let mut combined = Vec::new();
        for index in 1..=self.total_chunks {
            let bytes = self.chunks.get(&index)?;
            combined.extend_from_slice(bytes);
        }

        Some(combined)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PadColorSnapshot {
    pub inactive: Option<Vec<u8>>,
    pub active: Option<Vec<u8>>,
}

impl PadColorSnapshot {
    pub fn loaded_buffers(&self) -> usize {
        self.inactive.iter().count() + self.active.iter().count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorTarget {
    Inactive,
    Active,
}

impl ColorTarget {
    pub fn toggle(self) -> Self {
        match self {
            Self::Inactive => Self::Active,
            Self::Active => Self::Inactive,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Active => "active",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Devices,
    Settings,
    PadColors,
    PacketLog,
    ImportExport,
}

impl Screen {
    pub const ALL: [Screen; 5] = [
        Screen::Devices,
        Screen::Settings,
        Screen::PadColors,
        Screen::PacketLog,
        Screen::ImportExport,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Devices => "Devices",
            Screen::Settings => "Settings",
            Screen::PadColors => "Pad Colors",
            Screen::PacketLog => "Activity Log",
            Screen::ImportExport => "Import/Export",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|screen| *screen == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|screen| *screen == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    Booting,
    Idle,
    Scanning,
    Connected,
    ReadingConfig,
    Editing,
    ApplyingChanges,
    CaptureMode,
    Error,
}

impl AppMode {
    pub fn label(self) -> &'static str {
        match self {
            AppMode::Booting => "booting",
            AppMode::Idle => "idle",
            AppMode::Scanning => "scanning",
            AppMode::Connected => "connected",
            AppMode::ReadingConfig => "reading",
            AppMode::Editing => "editing",
            AppMode::ApplyingChanges => "applying",
            AppMode::CaptureMode => "capture",
            AppMode::Error => "error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub screen: Screen,
    pub should_quit: bool,
    pub devices: Vec<DeviceRef>,
    pub selected_device_idx: usize,
    pub selected_setting_idx: usize,
    pub selected_packet_idx: usize,
    pub connected_device_id: Option<String>,
    pub snapshot: Option<ConfigSnapshot>,
    pub pad_colors: Option<PadColorSnapshot>,
    pub staged_pad_colors: Option<PadColorSnapshot>,
    pub selected_pad_bank: usize,
    pub selected_pad_button_idx: usize,
    pub selected_pad_buttons: BTreeSet<usize>,
    pub selected_color_target: ColorTarget,
    pub pending_bulk_reads: BTreeMap<u8, PendingBulkRead>,
    pub pad_color_reads_requested: bool,
    pub staged_edits: BTreeMap<String, FieldValue>,
    pub packet_log: Vec<SysexFrame>,
    pub event_log: Vec<String>,
    pub status_message: String,
    pub last_error: Option<String>,
    pub app_mode: AppMode,
    pub apply_modal_open: bool,
    pub help_modal_open: bool,
}

impl AppState {
    pub fn new() -> Self {
        let mut state = Self {
            screen: Screen::Devices,
            should_quit: false,
            devices: Vec::new(),
            selected_device_idx: 0,
            selected_setting_idx: 0,
            selected_packet_idx: 0,
            connected_device_id: None,
            snapshot: None,
            pad_colors: None,
            staged_pad_colors: None,
            selected_pad_bank: 0,
            selected_pad_button_idx: 0,
            selected_pad_buttons: BTreeSet::new(),
            selected_color_target: ColorTarget::Active,
            pending_bulk_reads: BTreeMap::new(),
            pad_color_reads_requested: false,
            staged_edits: BTreeMap::new(),
            packet_log: Vec::new(),
            event_log: Vec::new(),
            status_message: String::from("Press r to scan MIDI ports."),
            last_error: None,
            app_mode: AppMode::Booting,
            apply_modal_open: false,
            help_modal_open: false,
        };
        state.push_event("Application booted.");
        state
    }

    pub fn selected_device(&self) -> Option<&DeviceRef> {
        self.devices.get(self.selected_device_idx)
    }

    pub fn connected_device(&self) -> Option<&DeviceRef> {
        let connected_id = self.connected_device_id.as_ref()?;
        self.devices
            .iter()
            .find(|device| &device.id == connected_id)
    }

    pub fn known_fields(&self) -> Vec<&DecodedField> {
        self.snapshot
            .as_ref()
            .map(|snapshot| snapshot.fields.iter().filter(|field| field.known).collect())
            .unwrap_or_default()
    }

    pub fn selected_known_field(&self) -> Option<&DecodedField> {
        self.snapshot
            .as_ref()?
            .fields
            .iter()
            .filter(|field| field.known)
            .nth(self.selected_setting_idx)
    }

    pub fn current_value_for(&self, field: &DecodedField) -> FieldValue {
        self.staged_edits
            .get(&field.id)
            .cloned()
            .unwrap_or_else(|| field.value.clone())
    }

    pub fn dirty_count(&self) -> usize {
        self.staged_edits.len() + self.dirty_pad_color_count()
    }

    pub fn current_pad_colors(&self) -> Option<&PadColorSnapshot> {
        self.staged_pad_colors.as_ref().or(self.pad_colors.as_ref())
    }

    pub fn dirty_pad_color_count(&self) -> usize {
        let Some(device) = self.pad_colors.as_ref() else {
            return 0;
        };
        let Some(staged) = self.staged_pad_colors.as_ref() else {
            return 0;
        };

        count_changed_pads(device.inactive.as_deref(), staged.inactive.as_deref())
            + count_changed_pads(device.active.as_deref(), staged.active.as_deref())
    }

    pub fn push_event(&mut self, message: impl Into<String>) {
        self.event_log.push(message.into());
        if self.event_log.len() > 200 {
            let overflow = self.event_log.len() - 200;
            self.event_log.drain(0..overflow);
        }
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status_message = status.into();
    }

    pub fn mark_error(&mut self, error: impl Into<String>) {
        let error = error.into();
        self.last_error = Some(error.clone());
        self.status_message = error.clone();
        self.app_mode = AppMode::Error;
        self.push_event(format!("error: {error}"));
    }

    pub fn clear_error(&mut self) {
        self.last_error = None;
    }

    pub fn normalize_selection(&mut self) {
        if self.selected_device_idx >= self.devices.len() && !self.devices.is_empty() {
            self.selected_device_idx = self.devices.len() - 1;
        }

        let known_len = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.fields.iter().filter(|field| field.known).count())
            .unwrap_or(0);
        if self.selected_setting_idx >= known_len && known_len > 0 {
            self.selected_setting_idx = known_len - 1;
        }

        if self.selected_packet_idx >= self.packet_log.len() && !self.packet_log.is_empty() {
            self.selected_packet_idx = self.packet_log.len() - 1;
        }

        self.selected_pad_button_idx = self.selected_pad_button_idx.min(127);
        self.selected_pad_buttons.retain(|index| *index < 128);
    }

    pub fn set_connected_device(&mut self, device_id: Option<String>) {
        self.connected_device_id = device_id.clone();
        for device in &mut self.devices {
            device.connection_state = match device_id.as_ref() {
                Some(current) if *current == device.id => ConnectionState::Connected,
                _ => ConnectionState::Disconnected,
            };
        }
    }
}

fn count_changed_pads(device: Option<&[u8]>, staged: Option<&[u8]>) -> usize {
    let (Some(device), Some(staged)) = (device, staged) else {
        return 0;
    };

    let pad_count = device.len().min(staged.len()) / 3;
    (0..pad_count)
        .filter(|index| {
            let start = index * 3;
            device.get(start..start + 3) != staged.get(start..start + 3)
        })
        .count()
}
