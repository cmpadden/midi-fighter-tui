mod actions;
pub(crate) mod apply;
mod commands;
pub(crate) mod pad_grid;
mod reducer;
mod state;

pub use actions::Action;
pub use state::{AppMode, AppState, ColorTarget, Screen};

use crate::transport::MidiTransport;

pub struct App<T: MidiTransport> {
    pub state: AppState,
    transport: T,
}

impl<T: MidiTransport> App<T> {
    pub fn new(transport: T) -> Self {
        Self {
            state: AppState::new(),
            transport,
        }
    }

    pub fn dispatch(&mut self, action: Action) {
        if self.state.help_modal_open
            && !matches!(action, Action::Tick | Action::CancelModal | Action::Quit)
        {
            return;
        }

        if self.state.apply_modal_open
            && !matches!(
                action,
                Action::Tick | Action::ConfirmApply | Action::CancelModal | Action::Quit
            )
        {
            return;
        }

        match action {
            Action::Tick => {
                reducer::reduce(&mut self.state, action);
                if let Err(err) = commands::poll_transport(&mut self.state, &mut self.transport) {
                    self.state.mark_error(err.to_string());
                }
            }
            Action::RefreshDevices => {
                if let Err(err) = commands::refresh_devices(&mut self.state, &mut self.transport) {
                    self.state.mark_error(err.to_string());
                }
            }
            Action::ConnectSelected => {
                if let Err(err) = commands::connect_selected(&mut self.state, &mut self.transport) {
                    self.state.mark_error(err.to_string());
                }
            }
            Action::Disconnect => {
                if let Err(err) = commands::disconnect(&mut self.state, &mut self.transport) {
                    self.state.mark_error(err.to_string());
                }
            }
            Action::RefreshConfig => {
                if let Err(err) = commands::refresh_config(&mut self.state, &mut self.transport) {
                    self.state.mark_error(err.to_string());
                }
            }
            Action::ConfirmApply => {
                if let Err(err) = commands::confirm_apply(&mut self.state, &mut self.transport) {
                    self.state.mark_error(err.to_string());
                }
            }
            Action::TogglePadBank => {
                reducer::reduce(&mut self.state, Action::TogglePadBank);
                if let Err(err) =
                    commands::notify_pad_bank_changed(&mut self.state, &mut self.transport)
                {
                    self.state.mark_error(err.to_string());
                }
            }
            other => reducer::reduce(&mut self.state, other),
        }
    }

    pub fn refresh_devices(&mut self) {
        self.dispatch(Action::RefreshDevices);
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        protocol::SysexFrame,
        transport::{DeviceRef, MidiTransport, TransportError},
    };

    use super::{Action, App, Screen};

    #[derive(Default)]
    struct MockTransport {
        scan_calls: usize,
    }

    impl MidiTransport for MockTransport {
        fn scan_devices(&mut self) -> Result<Vec<DeviceRef>, TransportError> {
            self.scan_calls += 1;
            Ok(Vec::new())
        }

        fn connect(&mut self, _device: &DeviceRef) -> Result<Option<String>, TransportError> {
            Ok(None)
        }

        fn disconnect(&mut self) -> Result<(), TransportError> {
            Ok(())
        }

        fn poll_packets(&mut self) -> Result<Vec<SysexFrame>, TransportError> {
            Ok(Vec::new())
        }

        fn send(&mut self, _bytes: &[u8]) -> Result<SysexFrame, TransportError> {
            Err(TransportError::NotConnected)
        }
    }

    #[test]
    fn apply_modal_blocks_command_dispatch() {
        let mut app = App::new(MockTransport::default());
        app.state.apply_modal_open = true;

        app.dispatch(Action::RefreshDevices);

        assert_eq!(app.transport.scan_calls, 0);
    }

    #[test]
    fn apply_modal_still_allows_cancel() {
        let mut app = App::new(MockTransport::default());
        app.state.apply_modal_open = true;
        app.state.screen = Screen::Settings;

        app.dispatch(Action::CancelModal);

        assert!(!app.state.apply_modal_open);
    }
}
