//! Synchronous terminal input and presentation adapter for `game-app`.
//!
//! The crate maps terminal events to semantic actions, owns transient UI state,
//! renders immutable application views, and paces explicit manual tick batches.
//! It has no direct dependency on `game-core`.

pub mod clock;
pub mod input;
pub mod render;
pub mod state;
pub mod terminal;

use clock::{Clock, MonotonicClock};
use game_app::{ApplicationError, ProfileDescriptor};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use state::TuiState;
use std::{
    io::{self, stdout},
    time::Duration,
};
use terminal::{CrosstermEvents, CrosstermTerminalOps, EventSource, TerminalGuard};
use thiserror::Error;

pub use input::KeyboardLayout;
pub use state::{BatchStatus, MIN_HEIGHT, MIN_WIDTH, Screen, TickBatch};
pub use terminal::{TerminalEvent, TerminalOps};

#[cfg(test)]
mod state_tests;

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("terminal I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("application projection failed: {0}")]
    Application(#[from] ApplicationError),
}

/// Creates the startup UI without acquiring a terminal. This is suitable for
/// `game-play` composition, tests, and alternate synchronous frontends.
#[must_use]
pub fn start(profile: ProfileDescriptor, seed: u64) -> TuiState {
    TuiState::new(profile, seed)
}

/// Acquires the real terminal with staged RAII cleanup and runs until the user
/// confirms quit.
pub fn run(profile: ProfileDescriptor, seed: u64) -> Result<(), TuiError> {
    run_state(start(profile, seed))
}

/// Runs a previously constructed state on the production crossterm adapter.
pub fn run_state(mut state: TuiState) -> Result<(), TuiError> {
    let _guard = TerminalGuard::acquire(CrosstermTerminalOps)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let size = terminal.size()?;
    state.resize(size.width, size.height);
    let mut events = CrosstermEvents;
    let clock = MonotonicClock::default();
    let result = run_loop(&mut terminal, &mut events, &clock, &mut state);
    let _ = terminal.show_cursor();
    result
}

/// Injectable synchronous loop. One atomic application tick is dispatched per
/// due iteration, ensuring each intermediate view is rendered and controls are
/// polled between ticks.
pub fn run_loop<B, E, C>(
    terminal: &mut Terminal<B>,
    events: &mut E,
    clock: &C,
    state: &mut TuiState,
) -> Result<(), TuiError>
where
    B: Backend<Error = io::Error>,
    E: EventSource,
    C: Clock,
{
    while !state.should_quit {
        let now = clock.now();
        state.advance_due(now)?;
        terminal.draw(|frame| render::render(frame, state))?;
        let timeout = state.next_wake(clock.now()).min(Duration::from_millis(100));
        if events.poll(timeout)? {
            let event = events.read()?;
            state.handle_event(event, clock.now())?;
        }
    }
    Ok(())
}
