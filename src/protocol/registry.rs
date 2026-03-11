use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceFamily {
    MidiFighter64,
    MidiFighter3D,
    MidiFighterTwister,
    MidiFighterSpectra,
    Unknown(String),
}

impl DeviceFamily {
    pub fn from_name(name: &str) -> Self {
        let normalized = name.to_ascii_lowercase();
        if normalized.contains("twister") {
            Self::MidiFighterTwister
        } else if normalized.contains("spectra") {
            Self::MidiFighterSpectra
        } else if normalized.contains("3d") {
            Self::MidiFighter3D
        } else if normalized.contains("midi fighter") {
            Self::MidiFighter64
        } else {
            Self::Unknown(name.to_string())
        }
    }
}

impl std::fmt::Display for DeviceFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceFamily::MidiFighter64 => write!(f, "Midi Fighter 64"),
            DeviceFamily::MidiFighter3D => write!(f, "Midi Fighter 3D"),
            DeviceFamily::MidiFighterTwister => write!(f, "Midi Fighter Twister"),
            DeviceFamily::MidiFighterSpectra => write!(f, "Midi Fighter Spectra"),
            DeviceFamily::Unknown(name) => write!(f, "{name}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    ObservedOnly,
    Tentative,
    Verified,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::ObservedOnly => write!(f, "observed"),
            Confidence::Tentative => write!(f, "tentative"),
            Confidence::Verified => write!(f, "verified"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadCompleteness {
    Partial,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagId(pub u16);

impl std::fmt::Display for TagId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:02X}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldValue {
    Bool(bool),
    Integer(i32),
    Text(String),
    Bytes(Vec<u8>),
}

impl std::fmt::Display for FieldValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldValue::Bool(value) => write!(f, "{}", if *value { "enabled" } else { "disabled" }),
            FieldValue::Integer(value) => write!(f, "{value}"),
            FieldValue::Text(value) => write!(f, "{value}"),
            FieldValue::Bytes(bytes) => {
                let value = bytes
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(f, "{value}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingKind {
    Boolean,
    Integer { min: i32, max: i32, step: i32 },
    Enum { options: Vec<String> },
    Bytes,
}

impl SettingKind {
    pub fn activate(&self, current: &FieldValue) -> Option<FieldValue> {
        match self {
            SettingKind::Boolean => match current {
                FieldValue::Bool(value) => Some(FieldValue::Bool(!value)),
                _ => None,
            },
            SettingKind::Enum { options } => cycle_enum(options, current, 1),
            SettingKind::Integer { .. } => self.adjust(current, 1),
            SettingKind::Bytes => None,
        }
    }

    pub fn adjust(&self, current: &FieldValue, delta: i32) -> Option<FieldValue> {
        match self {
            SettingKind::Boolean => self.activate(current),
            SettingKind::Integer { min, max, step } => match current {
                FieldValue::Integer(value) => {
                    let next = (*value + delta * *step).clamp(*min, *max);
                    Some(FieldValue::Integer(next))
                }
                _ => None,
            },
            SettingKind::Enum { options } => cycle_enum(options, current, delta),
            SettingKind::Bytes => None,
        }
    }
}

fn cycle_enum(options: &[String], current: &FieldValue, delta: i32) -> Option<FieldValue> {
    let FieldValue::Text(current) = current else {
        return None;
    };

    let current_index = options
        .iter()
        .position(|option| option == current)
        .unwrap_or(0) as i32;
    let len = options.len() as i32;
    if len == 0 {
        return None;
    }

    let next_index = (current_index + delta).rem_euclid(len) as usize;
    options.get(next_index).cloned().map(FieldValue::Text)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedField {
    pub id: String,
    pub label: String,
    pub tag_id: TagId,
    pub value: FieldValue,
    pub kind: SettingKind,
    pub confidence: Confidence,
    pub editable: bool,
    pub known: bool,
    pub notes: Option<String>,
    pub source_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub family: DeviceFamily,
    pub completeness: ReadCompleteness,
    pub fields: Vec<DecodedField>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedConfig {
    pub base_snapshot: ConfigSnapshot,
    pub edits: BTreeMap<String, FieldValue>,
}
