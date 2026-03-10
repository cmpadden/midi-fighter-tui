mod midir;
mod ports;

pub use midir::MidirTransport;
pub use ports::{ConnectionState, DeviceRef, TransportError};

use crate::protocol::SysexFrame;

pub trait MidiTransport {
    fn scan_devices(&mut self) -> Result<Vec<DeviceRef>, TransportError>;
    fn connect(&mut self, device: &DeviceRef) -> Result<Option<String>, TransportError>;
    fn disconnect(&mut self) -> Result<(), TransportError>;
    fn poll_packets(&mut self) -> Result<Vec<SysexFrame>, TransportError>;
    fn send(&mut self, bytes: &[u8]) -> Result<SysexFrame, TransportError>;
}
