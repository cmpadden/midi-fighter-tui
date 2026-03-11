mod app;
mod persistence;
mod protocol;
mod transport;
mod tui;

use std::{
    io::{self, Stdout},
    time::Duration,
};

use anyhow::Result;
use app::{Action, App, Screen};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use transport::MidirTransport;

fn main() -> Result<()> {
    let transport = MidirTransport::new();
    let mut app = App::new(transport);
    app.refresh_devices();

    let mut terminal = init_terminal()?;
    let result = run_app(&mut terminal, &mut app);
    restore_terminal(&mut terminal)?;
    result
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App<MidirTransport>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| tui::render(frame, &app.state))?;

        if app.state.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(125))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(action) = map_key(key, &app.state) {
                        app.dispatch(action);
                    }
                }
            }
        } else {
            app.dispatch(Action::Tick);
        }
    }

    Ok(())
}

fn map_key(key: KeyEvent, state: &app::AppState) -> Option<Action> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Action::Quit);
    }

    if state.help_modal_open {
        return match key.code {
            KeyCode::Esc | KeyCode::Char('?') => Some(Action::CancelModal),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    if state.apply_modal_open {
        return match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => Some(Action::ConfirmApply),
            KeyCode::Esc => Some(Action::CancelModal),
            KeyCode::Char('q') => Some(Action::Quit),
            _ => None,
        };
    }

    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Tab => Some(Action::NextScreen),
        KeyCode::BackTab => Some(Action::PrevScreen),
        KeyCode::Left
            if matches!(state.screen, Screen::PadColors)
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            Some(Action::ExtendPadSelection(-1, 0))
        }
        KeyCode::Right
            if matches!(state.screen, Screen::PadColors)
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            Some(Action::ExtendPadSelection(1, 0))
        }
        KeyCode::Up
            if matches!(state.screen, Screen::PadColors)
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            Some(Action::ExtendPadSelection(0, -1))
        }
        KeyCode::Down
            if matches!(state.screen, Screen::PadColors)
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            Some(Action::ExtendPadSelection(0, 1))
        }
        KeyCode::Left if matches!(state.screen, Screen::PadColors) => Some(Action::MoveLeft),
        KeyCode::Right if matches!(state.screen, Screen::PadColors) => Some(Action::MoveRight),
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Left => Some(Action::AdjustSelected(-1)),
        KeyCode::Right => Some(Action::AdjustSelected(1)),
        KeyCode::Enter => {
            if state.apply_modal_open {
                Some(Action::ConfirmApply)
            } else if matches!(state.screen, Screen::Devices) {
                Some(Action::ConnectSelected)
            } else {
                Some(Action::ActivateSelected)
            }
        }
        KeyCode::Char(' ') => {
            if state.apply_modal_open {
                Some(Action::ConfirmApply)
            } else if matches!(state.screen, Screen::PadColors) {
                Some(Action::TogglePadSelection)
            } else {
                Some(Action::ActivateSelected)
            }
        }
        KeyCode::Esc => Some(Action::CancelModal),
        KeyCode::Char('r') => Some(Action::RefreshDevices),
        KeyCode::Char('g') => Some(Action::RefreshConfig),
        KeyCode::Char('d') => Some(Action::Disconnect),
        KeyCode::Char('p') => Some(Action::ShowPacketLog),
        KeyCode::Char('a') => Some(Action::ToggleApplyModal),
        KeyCode::Char('[') if matches!(state.screen, Screen::PadColors) => {
            Some(Action::AdjustSelected(-1))
        }
        KeyCode::Char(']') if matches!(state.screen, Screen::PadColors) => {
            Some(Action::AdjustSelected(1))
        }
        KeyCode::Char('x') if matches!(state.screen, Screen::PadColors) => {
            Some(Action::ClearPadSelection)
        }
        KeyCode::Char('b') if matches!(state.screen, Screen::PadColors) => {
            Some(Action::TogglePadBank)
        }
        KeyCode::Char('u') => Some(Action::UndoField),
        KeyCode::Char('U') => Some(Action::DiscardAll),
        KeyCode::Char('?') => Some(Action::ShowHelp),
        _ => None,
    }
}
