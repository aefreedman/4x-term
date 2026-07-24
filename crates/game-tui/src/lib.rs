//! Synchronous terminal input and presentation adapter for `game-app`.
//!
//! The crate maps terminal events to semantic actions, owns transient UI state,
//! renders immutable application views, and paces explicit manual tick batches.
//! It has no direct dependency on `game-core`.

pub mod clock;
pub mod input;
pub mod playtest;
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
pub use playtest::*;
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
    #[error("playtest trace failed: {0}")]
    Playtest(String),
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
pub fn run_state(state: TuiState) -> Result<(), TuiError> {
    let mut observer = NoopPlaytestObserver;
    run_state_observed(state, &mut observer)
}

/// Runs a previously constructed state while forwarding opt-in semantic
/// playtest events to a caller-owned observer. The observer and its files stay
/// outside terminal lifecycle ownership so finalization occurs after cleanup.
pub fn run_state_observed(
    mut state: TuiState,
    observer: &mut dyn PlaytestObserver,
) -> Result<(), TuiError> {
    drain_playtest_events(&mut state, observer)?;
    let _guard = TerminalGuard::acquire(CrosstermTerminalOps)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let size = terminal.size()?;
    state.resize(size.width, size.height);
    let mut events = CrosstermEvents;
    let clock = MonotonicClock::default();
    let result = run_loop_observed(&mut terminal, &mut events, &clock, &mut state, observer);
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
    let mut observer = NoopPlaytestObserver;
    run_loop_observed(terminal, events, clock, state, &mut observer)
}

pub fn run_loop_observed<B, E, C>(
    terminal: &mut Terminal<B>,
    events: &mut E,
    clock: &C,
    state: &mut TuiState,
    observer: &mut dyn PlaytestObserver,
) -> Result<(), TuiError>
where
    B: Backend<Error = io::Error>,
    E: EventSource,
    C: Clock,
{
    drain_playtest_events(state, observer)?;
    while !state.should_quit {
        let now = clock.now();
        let advance_result = state.advance_due(now);
        drain_playtest_events(state, observer)?;
        advance_result?;
        terminal.draw(|frame| render::render(frame, state))?;
        let timeout = state.next_wake(clock.now()).min(Duration::from_millis(100));
        if events.poll(timeout)? {
            let event = events.read()?;
            let event_result = state.handle_event(event, clock.now());
            drain_playtest_events(state, observer)?;
            event_result?;
        }
    }
    Ok(())
}

struct NoopPlaytestObserver;

impl PlaytestObserver for NoopPlaytestObserver {
    fn observe(&mut self, _event: &PlaytestEvent) -> Result<(), String> {
        Ok(())
    }
}

fn drain_playtest_events(
    state: &mut TuiState,
    observer: &mut dyn PlaytestObserver,
) -> Result<(), TuiError> {
    for event in state.drain_playtest_events() {
        observer.observe(&event).map_err(TuiError::Playtest)?;
    }
    Ok(())
}
