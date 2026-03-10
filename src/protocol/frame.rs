use serde::{Deserialize, Serialize};

use super::TagId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Sent,
    Received,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Sent => write!(f, "sent"),
            Direction::Received => write!(f, "recv"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SysexEnvelope {
    pub manufacturer: Vec<u8>,
    pub command: u8,
    pub tag: Option<TagId>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SysexFrame {
    pub timestamp: String,
    pub direction: Direction,
    pub raw: Vec<u8>,
    pub envelope: Option<SysexEnvelope>,
}

impl SysexFrame {
    pub fn hex_string(&self) -> String {
        self.raw
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
