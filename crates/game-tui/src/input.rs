//! Pure keyboard routing for TUI-local interaction state.

use crate::state::{Activity, InputLayer, UiState};
use crossterm::event::KeyCode;

/// Intent produced by [`route_key`]. Application requests remain outside this
/// pure routing layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputAction {
    None,
    Quit,
    CloseLayer,
    Switch(Activity),
    ToggleRun,
    Step,
    CycleTickRate,
    ToggleHelp,
    MoveUp,
    MoveDown,
    OpenQuantity,
    Buy,
    Sell,
    BeginTravel,
    Sort,
    ToggleSortDirection,
    Decrease,
    Increase,
}

/// Routes a key according to the input ownership contract.
///
/// An unsupported layout accepts only quit. An active overlay consumes every
/// key (including `Esc`) before global navigation is considered. Root input
/// then receives global actions before the current activity's local actions.
pub fn route_key(code: KeyCode, ui: &UiState, layout_supported: bool) -> InputAction {
    if !layout_supported {
        return matches!(code, KeyCode::Char('q'))
            .then_some(InputAction::Quit)
            .unwrap_or(InputAction::None);
    }

    if ui.input_layer != InputLayer::Root {
        return matches!(code, KeyCode::Esc)
            .then_some(InputAction::CloseLayer)
            .unwrap_or(InputAction::None);
    }

    match code {
        KeyCode::F(1) => return InputAction::Switch(Activity::Systems),
        KeyCode::F(2) => return InputAction::Switch(Activity::Trade),
        KeyCode::F(3) => return InputAction::Switch(Activity::Governance),
        KeyCode::F(4) => return InputAction::Switch(Activity::Intelligence),
        KeyCode::Char('q') => return InputAction::Quit,
        KeyCode::Char(' ') => return InputAction::ToggleRun,
        KeyCode::Char('s') => return InputAction::Step,
        KeyCode::Char('r') => return InputAction::CycleTickRate,
        KeyCode::Char('?') => return InputAction::ToggleHelp,
        _ => {}
    }

    match ui.activity {
        Activity::Systems => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::Enter => InputAction::BeginTravel,
            KeyCode::Char('o') => InputAction::Sort,
            KeyCode::Char('d') => InputAction::ToggleSortDirection,
            _ => InputAction::None,
        },
        Activity::Trade => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::Char('n') => InputAction::OpenQuantity,
            KeyCode::Char('b') => InputAction::Buy,
            KeyCode::Char('x') => InputAction::Sell,
            KeyCode::Char('t') | KeyCode::Enter => InputAction::BeginTravel,
            _ => InputAction::None,
        },
        Activity::Governance => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::Left => InputAction::Decrease,
            KeyCode::Right => InputAction::Increase,
            _ => InputAction::None,
        },
        Activity::Intelligence => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            _ => InputAction::None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlays_block_global_and_activity_actions() {
        let ui = UiState {
            activity: Activity::Trade,
            input_layer: InputLayer::Quantity,
            ..UiState::default()
        };
        assert_eq!(route_key(KeyCode::F(2), &ui, true), InputAction::None);
        assert_eq!(route_key(KeyCode::Char('b'), &ui, true), InputAction::None);
        assert_eq!(route_key(KeyCode::Esc, &ui, true), InputAction::CloseLayer);
    }
}
