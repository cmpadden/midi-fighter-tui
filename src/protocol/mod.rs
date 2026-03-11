mod frame;
mod registry;

use std::collections::BTreeMap;

use thiserror::Error;

pub use frame::{Direction, SysexEnvelope, SysexFrame};
pub use registry::{
    Confidence, ConfigSnapshot, DecodedField, DeviceFamily, FieldValue, ReadCompleteness, TagId,
};

#[derive(Clone, Debug)]
pub struct SnapshotUpdate {
    pub snapshot: ConfigSnapshot,
    pub updated_tags: Vec<TagId>,
}

#[derive(Clone, Debug)]
pub struct BulkTransferChunk {
    pub tag: u8,
    pub chunk_index: u8,
    pub total_chunks: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("{0}")]
    Unsupported(String),
}

pub fn empty_snapshot(family: DeviceFamily) -> ConfigSnapshot {
    ConfigSnapshot {
        family,
        completeness: ReadCompleteness::Partial,
        fields: Vec::new(),
    }
}

pub fn parse_frame(raw: Vec<u8>, direction: Direction, timestamp: String) -> SysexFrame {
    let envelope = parse_envelope(&raw);
    SysexFrame {
        timestamp,
        direction,
        raw,
        envelope,
    }
}

pub fn device_inquiry_request() -> Vec<u8> {
    vec![0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]
}

pub fn config_read_request(family: &DeviceFamily) -> Result<Vec<u8>, ProtocolError> {
    match family {
        DeviceFamily::Unknown(name) => Err(ProtocolError::Unsupported(format!(
            "Tagged config read is not enabled for unknown device family {name}."
        ))),
        _ => Ok(vec![0xF0, 0x00, 0x01, 0x79, 0x02, 0x00, 0xF7]),
    }
}

pub fn bulk_read_request(family: &DeviceFamily, tag: u8) -> Result<Vec<u8>, ProtocolError> {
    match family {
        DeviceFamily::MidiFighter64 if matches!(tag, 1 | 2) => {
            Ok(vec![0xF0, 0x00, 0x01, 0x79, 0x04, 0x01, tag, 0xF7])
        }
        DeviceFamily::MidiFighter64 => Err(ProtocolError::Unsupported(format!(
            "Bulk color read tag {tag} is not supported for Midi Fighter 64."
        ))),
        DeviceFamily::Unknown(name) => Err(ProtocolError::Unsupported(format!(
            "Bulk color read is not enabled for unknown device family {name}."
        ))),
        _ => Err(ProtocolError::Unsupported(format!(
            "Bulk color read is not enabled for device family {family}."
        ))),
    }
}

pub fn pad_color_tags(family: &DeviceFamily) -> Option<(u8, u8)> {
    match family {
        DeviceFamily::MidiFighter64 => Some((1, 2)),
        _ => None,
    }
}

pub fn bank_switch_message(
    family: &DeviceFamily,
    midi_channel: u8,
    bank: usize,
) -> Result<Vec<u8>, ProtocolError> {
    match family {
        DeviceFamily::MidiFighter64 if bank < 2 => {
            Ok(vec![0xB0 | (midi_channel & 0x0F), 0x03, bank as u8])
        }
        DeviceFamily::MidiFighter64 => Err(ProtocolError::Unsupported(format!(
            "Pad color bank {bank} is not supported for Midi Fighter 64."
        ))),
        DeviceFamily::Unknown(name) => Err(ProtocolError::Unsupported(format!(
            "Bank switching is not enabled for unknown device family {name}."
        ))),
        _ => Err(ProtocolError::Unsupported(format!(
            "Bank switching is not enabled for device family {family}."
        ))),
    }
}

pub fn build_write_messages(
    _snapshot: &ConfigSnapshot,
    _edits: &BTreeMap<String, FieldValue>,
) -> Result<Vec<Vec<u8>>, ProtocolError> {
    Err(ProtocolError::Unsupported(
        "Config write serializers have not been reverse engineered yet.".into(),
    ))
}

pub fn build_bulk_write_messages(
    family: &DeviceFamily,
    writes: &[(u8, &[u8])],
) -> Result<Vec<Vec<u8>>, ProtocolError> {
    match family {
        DeviceFamily::MidiFighter64 => {
            let mut messages = Vec::new();
            for (tag, data) in writes {
                if !matches!(tag, 1 | 2) {
                    return Err(ProtocolError::Unsupported(format!(
                        "Bulk write tag {tag} is not supported for Midi Fighter 64."
                    )));
                }

                const CHUNK_SIZE: usize = 24;
                let total_chunks = data.len().div_ceil(CHUNK_SIZE) as u8;
                for (index, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
                    let chunk_index = (index + 1) as u8;
                    let mut message = vec![
                        0xF0,
                        0x00,
                        0x01,
                        0x79,
                        0x04,
                        0x00,
                        *tag,
                        chunk_index,
                        total_chunks,
                        chunk.len() as u8,
                    ];
                    message.extend_from_slice(chunk);
                    message.push(0xF7);
                    messages.push(message);
                }
            }
            Ok(messages)
        }
        DeviceFamily::Unknown(name) => Err(ProtocolError::Unsupported(format!(
            "Bulk color write is not enabled for unknown device family {name}."
        ))),
        _ => Err(ProtocolError::Unsupported(format!(
            "Bulk color write is not enabled for device family {family}."
        ))),
    }
}

pub fn apply_frame_to_snapshot(
    snapshot: &ConfigSnapshot,
    frame: &SysexFrame,
) -> Option<SnapshotUpdate> {
    let tag_values = decode_tagged_config_response(&frame.raw)?;

    let mut fields_by_tag = snapshot
        .fields
        .iter()
        .cloned()
        .map(|field| (field.tag_id.0, field))
        .collect::<BTreeMap<_, _>>();
    let mut updated_tags = Vec::new();

    for (tag, value) in tag_values {
        fields_by_tag.insert(
            tag as u16,
            decode_field(snapshot.family.clone(), tag, value),
        );
        updated_tags.push(TagId(tag as u16));
    }

    if updated_tags.is_empty() {
        return None;
    }

    let mut fields = Vec::new();
    for definition in setting_definitions(&snapshot.family) {
        if let Some(field) = fields_by_tag.remove(&(definition.tag as u16)) {
            fields.push(field);
        }
    }

    let mut unknown_fields = fields_by_tag.into_values().collect::<Vec<_>>();
    unknown_fields.sort_by_key(|field| field.tag_id.0);
    fields.extend(unknown_fields);

    let known_total = setting_definitions(&snapshot.family).len();
    let known_loaded = fields.iter().filter(|field| field.known).count();
    let completeness = if known_total > 0 && known_loaded == known_total {
        ReadCompleteness::Complete
    } else {
        ReadCompleteness::Partial
    };

    Some(SnapshotUpdate {
        snapshot: ConfigSnapshot {
            family: snapshot.family.clone(),
            completeness,
            fields,
        },
        updated_tags,
    })
}

pub fn decode_bulk_transfer_chunk(frame: &SysexFrame) -> Option<BulkTransferChunk> {
    decode_bulk_transfer_chunk_raw(&frame.raw)
}

fn parse_envelope(raw: &[u8]) -> Option<SysexEnvelope> {
    if raw.first() != Some(&0xF0) || raw.last() != Some(&0xF7) || raw.len() < 4 {
        return None;
    }

    if raw.len() >= 6 && raw[1] == 0x7E && raw[3] == 0x06 {
        let note = match raw[4] {
            0x01 => "Universal Device Inquiry request.",
            0x02 => "Universal Device Inquiry reply.",
            _ => "Universal SysEx inquiry frame.",
        };
        return Some(SysexEnvelope {
            manufacturer: vec![raw[1]],
            command: raw[3],
            tag: None,
            note: Some(note.into()),
        });
    }

    if raw.len() >= 7 && raw[1..4] == [0x00, 0x01, 0x79] {
        let command = raw[4];
        let payload_note = match (command, raw.get(5).copied()) {
            (0x02, Some(0x00)) => Some("Tagged config read request.".into()),
            (0x02, Some(0x01)) => Some(format!(
                "Tagged config response with {} tag/value pair(s).",
                raw[6..raw.len() - 1].chunks_exact(2).len()
            )),
            (0x04, Some(0x00)) => Some("Bulk transfer write request.".into()),
            (0x04, Some(0x01)) => Some("Bulk transfer read request.".into()),
            (0x04, Some(0x02)) => Some("Bulk transfer response.".into()),
            _ => Some("Midi Fighter vendor SysEx frame.".into()),
        };
        return Some(SysexEnvelope {
            manufacturer: raw[1..4].to_vec(),
            command,
            tag: raw
                .get(6)
                .copied()
                .filter(|_| command == 0x04)
                .map(|tag| TagId(tag as u16)),
            note: payload_note,
        });
    }

    let (manufacturer, command) = if raw.get(1) == Some(&0x00) && raw.len() >= 6 {
        (raw[1..4].to_vec(), raw[4])
    } else {
        (vec![raw[1]], *raw.get(2)?)
    };

    Some(SysexEnvelope {
        manufacturer,
        command,
        tag: None,
        note: Some("Captured raw SysEx frame.".into()),
    })
}

fn decode_tagged_config_response(raw: &[u8]) -> Option<Vec<(u8, u8)>> {
    if raw.len() < 8
        || raw.first() != Some(&0xF0)
        || raw.last() != Some(&0xF7)
        || raw[1..4] != [0x00, 0x01, 0x79]
        || raw[4] != 0x02
        || raw[5] != 0x01
    {
        return None;
    }

    let payload = &raw[6..raw.len() - 1];
    let mut tag_values = Vec::new();
    for chunk in payload.chunks_exact(2) {
        tag_values.push((chunk[0], chunk[1]));
    }
    Some(tag_values)
}

fn decode_bulk_transfer_chunk_raw(raw: &[u8]) -> Option<BulkTransferChunk> {
    if raw.len() < 11
        || raw.first() != Some(&0xF0)
        || raw.last() != Some(&0xF7)
        || raw[1..4] != [0x00, 0x01, 0x79]
        || raw[4] != 0x04
        || raw[5] != 0x00
    {
        return None;
    }

    let data_len = raw[9] as usize;
    let data_end = (10 + data_len).min(raw.len().saturating_sub(1));

    Some(BulkTransferChunk {
        tag: raw[6],
        chunk_index: raw[7],
        total_chunks: raw[8],
        data: raw[10..data_end].to_vec(),
    })
}

fn decode_field(family: DeviceFamily, tag: u8, value: u8) -> DecodedField {
    if let Some(definition) = setting_definitions(&family)
        .iter()
        .find(|definition| definition.tag == tag)
    {
        return definition.decode(value);
    }

    DecodedField {
        id: format!("raw.tag.{tag:02x}"),
        label: format!("Unknown Tag 0x{tag:02X}"),
        tag_id: TagId(tag as u16),
        value: FieldValue::Integer(value as i32),
        kind: registry::SettingKind::Integer {
            min: 0,
            max: 127,
            step: 1,
        },
        confidence: Confidence::ObservedOnly,
        editable: false,
        known: false,
        notes: Some(
            "Observed in a live tagged config reply; semantic meaning is still unknown.".into(),
        ),
        source_bytes: vec![tag, value],
    }
}

struct SettingDefinition {
    tag: u8,
    id: &'static str,
    label: &'static str,
    kind: DefinitionKind,
    notes: &'static str,
}

impl SettingDefinition {
    fn decode(&self, value: u8) -> DecodedField {
        let (field_value, kind) = match &self.kind {
            DefinitionKind::Boolean => {
                (FieldValue::Bool(value != 0), registry::SettingKind::Boolean)
            }
            DefinitionKind::Integer { min, max, step } => (
                FieldValue::Integer(value as i32),
                registry::SettingKind::Integer {
                    min: *min,
                    max: *max,
                    step: *step,
                },
            ),
            DefinitionKind::Enum { options } => (
                options
                    .get(value as usize)
                    .map(|option| (*option).to_string())
                    .map(FieldValue::Text)
                    .unwrap_or_else(|| FieldValue::Text(format!("Unknown ({value})"))),
                registry::SettingKind::Enum {
                    options: options.iter().map(|option| (*option).to_string()).collect(),
                },
            ),
            DefinitionKind::Bytes => (FieldValue::Bytes(vec![value]), registry::SettingKind::Bytes),
        };

        DecodedField {
            id: self.id.into(),
            label: self.label.into(),
            tag_id: TagId(self.tag as u16),
            value: field_value,
            kind,
            confidence: Confidence::Verified,
            editable: true,
            known: true,
            notes: Some(self.notes.into()),
            source_bytes: vec![self.tag, value],
        }
    }
}

#[allow(dead_code)]
enum DefinitionKind {
    Boolean,
    Integer { min: i32, max: i32, step: i32 },
    Enum { options: &'static [&'static str] },
    Bytes,
}

fn setting_definitions(family: &DeviceFamily) -> &'static [SettingDefinition] {
    match family {
        DeviceFamily::MidiFighter64 => MF64_SETTINGS,
        _ => &[],
    }
}

static MIDI_TYPE_OPTIONS: &[&str] = &["Notes", "Notes + CCs", "CCs"];
static CORNER_BANK_OPTIONS: &[&str] = &["Disabled", "Hold", "Press"];
static ANIMATION_OPTIONS: &[&str] = &["Square", "Circle", "Star", "Triangle", "None"];

static MF64_SETTINGS: &[SettingDefinition] = &[
    SettingDefinition {
        tag: 0,
        id: "midi_channel",
        label: "MIDI Channel",
        kind: DefinitionKind::Integer {
            min: 1,
            max: 13,
            step: 1,
        },
        notes: "Recovered from the official Midi Fighter 64 plugin registry.",
    },
    SettingDefinition {
        tag: 1,
        id: "midi_velocity",
        label: "MIDI Velocity",
        kind: DefinitionKind::Integer {
            min: 0,
            max: 127,
            step: 1,
        },
        notes: "Recovered from the official Midi Fighter 64 plugin registry.",
    },
    SettingDefinition {
        tag: 7,
        id: "midi_type",
        label: "MIDI Type",
        kind: DefinitionKind::Enum {
            options: MIDI_TYPE_OPTIONS,
        },
        notes: "Recovered from the official Midi Fighter 64 plugin registry.",
    },
    SettingDefinition {
        tag: 8,
        id: "super_combos",
        label: "Super Combos",
        kind: DefinitionKind::Boolean,
        notes: "Recovered from the official Midi Fighter 64 plugin registry.",
    },
    SettingDefinition {
        tag: 22,
        id: "sleep_timer",
        label: "Sleep Timer",
        kind: DefinitionKind::Integer {
            min: 0,
            max: 120,
            step: 1,
        },
        notes: "Recovered from the official Midi Fighter 64 plugin registry.",
    },
    SettingDefinition {
        tag: 23,
        id: "corner_bank_change",
        label: "Corner Button Bank Change",
        kind: DefinitionKind::Enum {
            options: CORNER_BANK_OPTIONS,
        },
        notes: "Recovered from the official Midi Fighter 64 plugin registry.",
    },
    SettingDefinition {
        tag: 10,
        id: "animation",
        label: "Animation",
        kind: DefinitionKind::Enum {
            options: ANIMATION_OPTIONS,
        },
        notes: "Recovered from the official Midi Fighter 64 plugin registry.",
    },
];
