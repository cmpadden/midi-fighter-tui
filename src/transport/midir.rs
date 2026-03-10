use std::{
    collections::{BTreeMap, BTreeSet},
    sync::mpsc::{self, Receiver},
    time::{SystemTime, UNIX_EPOCH},
};

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::protocol::{parse_frame, Confidence, DeviceFamily, Direction, SysexFrame};

use super::{ConnectionState, DeviceRef, MidiTransport, TransportError};

struct IncomingMessage {
    timestamp: u64,
    data: Vec<u8>,
}

struct LiveConnection {
    _input: Option<MidiInputConnection<()>>,
    output: Option<MidiOutputConnection>,
    receiver: Receiver<IncomingMessage>,
}

pub struct MidirTransport {
    connection: Option<LiveConnection>,
}

impl MidirTransport {
    pub fn new() -> Self {
        Self { connection: None }
    }
}

impl MidiTransport for MidirTransport {
    fn scan_devices(&mut self) -> Result<Vec<DeviceRef>, TransportError> {
        let mut midi_input =
            MidiInput::new("midi-fighter-tui-scan").map_err(|err| TransportError::Backend(err.to_string()))?;
        midi_input.ignore(Ignore::None);
        let midi_output =
            MidiOutput::new("midi-fighter-tui-scan").map_err(|err| TransportError::Backend(err.to_string()))?;

        let inputs = collect_input_names(&midi_input);
        let outputs = collect_output_names(&midi_output);

        let candidate_names: BTreeSet<String> = inputs
            .keys()
            .chain(outputs.keys())
            .filter(|name| is_midi_fighter_name(name))
            .cloned()
            .collect();

        let devices = candidate_names
            .into_iter()
            .map(|name| {
                let input_port = inputs.get(&name).cloned();
                let output_port = outputs.get(&name).cloned();
                let family = DeviceFamily::from_name(&name);
                let protocol_confidence = if input_port.is_some() && output_port.is_some() {
                    Confidence::Tentative
                } else {
                    Confidence::ObservedOnly
                };

                DeviceRef {
                    id: slugify(&name),
                    name,
                    input_port,
                    output_port,
                    family,
                    protocol_confidence,
                    connection_state: ConnectionState::Disconnected,
                }
            })
            .collect();

        Ok(devices)
    }

    fn connect(&mut self, device: &DeviceRef) -> Result<Option<String>, TransportError> {
        self.disconnect()?;

        let mut midi_input =
            MidiInput::new("midi-fighter-tui-live-in").map_err(|err| TransportError::Backend(err.to_string()))?;
        midi_input.ignore(Ignore::None);
        let midi_output =
            MidiOutput::new("midi-fighter-tui-live-out").map_err(|err| TransportError::Backend(err.to_string()))?;

        let (sender, receiver) = mpsc::channel();

        let input = if let Some(port_name) = device.input_port.as_deref() {
            let port = find_input_port(&midi_input, port_name)?;
            Some(
                midi_input
                    .connect(
                        &port,
                        "midi-fighter-tui-capture",
                        move |timestamp, message, _| {
                            let _ = sender.send(IncomingMessage {
                                timestamp,
                                data: message.to_vec(),
                            });
                        },
                        (),
                    )
                    .map_err(|err| TransportError::Backend(err.to_string()))?,
            )
        } else {
            None
        };

        let output = if let Some(port_name) = device.output_port.as_deref() {
            let port = find_output_port(&midi_output, port_name)?;
            Some(
                midi_output
                    .connect(&port, "midi-fighter-tui-send")
                    .map_err(|err| TransportError::Backend(err.to_string()))?,
            )
        } else {
            None
        };

        if input.is_none() && output.is_none() {
            return Err(TransportError::MissingPort(format!(
                "No openable ports found for {}",
                device.name
            )));
        }

        self.connection = Some(LiveConnection {
            _input: input,
            output,
            receiver,
        });

        Ok(Some(
            "Connected to live MIDI ports. Packet log is now backed by real incoming traffic.".into(),
        ))
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connection = None;
        Ok(())
    }

    fn poll_packets(&mut self) -> Result<Vec<SysexFrame>, TransportError> {
        let Some(connection) = self.connection.as_mut() else {
            return Ok(Vec::new());
        };

        let mut frames = Vec::new();
        while let Ok(message) = connection.receiver.try_recv() {
            frames.push(parse_frame(
                message.data,
                Direction::Received,
                format!("in:{}", message.timestamp),
            ));
        }

        Ok(frames)
    }

    fn send(&mut self, bytes: &[u8]) -> Result<SysexFrame, TransportError> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(TransportError::NotConnected);
        };
        let Some(output) = connection.output.as_mut() else {
            return Err(TransportError::OutputUnavailable);
        };

        output
            .send(bytes)
            .map_err(|err| TransportError::Backend(err.to_string()))?;

        Ok(parse_frame(
            bytes.to_vec(),
            Direction::Sent,
            format!("out:{}", unix_timestamp()),
        ))
    }
}

fn collect_input_names(midi_input: &MidiInput) -> BTreeMap<String, String> {
    midi_input
        .ports()
        .into_iter()
        .filter_map(|port| midi_input.port_name(&port).ok())
        .map(|name| (name.clone(), name))
        .collect()
}

fn collect_output_names(midi_output: &MidiOutput) -> BTreeMap<String, String> {
    midi_output
        .ports()
        .into_iter()
        .filter_map(|port| midi_output.port_name(&port).ok())
        .map(|name| (name.clone(), name))
        .collect()
}

fn is_midi_fighter_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("midi fighter")
}

fn slugify(name: &str) -> String {
    name.to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn find_input_port(midi_input: &MidiInput, port_name: &str) -> Result<midir::MidiInputPort, TransportError> {
    midi_input
        .ports()
        .into_iter()
        .find(|port| midi_input.port_name(port).map(|name| name == port_name).unwrap_or(false))
        .ok_or_else(|| TransportError::MissingPort(port_name.to_string()))
}

fn find_output_port(
    midi_output: &MidiOutput,
    port_name: &str,
) -> Result<midir::MidiOutputPort, TransportError> {
    midi_output
        .ports()
        .into_iter()
        .find(|port| midi_output.port_name(port).map(|name| name == port_name).unwrap_or(false))
        .ok_or_else(|| TransportError::MissingPort(port_name.to_string()))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
