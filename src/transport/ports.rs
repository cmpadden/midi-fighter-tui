use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::protocol::{Confidence, DeviceFamily};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRef {
    pub id: String,
    pub name: String,
    pub input_port: Option<String>,
    pub output_port: Option<String>,
    pub family: DeviceFamily,
    pub protocol_confidence: Confidence,
    pub connection_state: ConnectionState,
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("midi backend error: {0}")]
    Backend(String),
    #[error("midi port unavailable: {0}")]
    MissingPort(String),
    #[error("no MIDI device is currently connected")]
    NotConnected,
    #[error("the connected MIDI device has no output port")]
    OutputUnavailable,
}
