mod actions;
mod commands;
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
            other => reducer::reduce(&mut self.state, other),
        }
    }

    pub fn refresh_devices(&mut self) {
        self.dispatch(Action::RefreshDevices);
    }
}
