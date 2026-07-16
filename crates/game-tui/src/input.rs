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
    AmountDigit(char),
    AmountBackspace,
    ConfirmAmount,
    UseAmountMaximum,
    Switch(Activity),
    ToggleRun,
    Step,
    CycleTickRate,
    ToggleHelp,
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    OpenBuyAmount,
    OpenSellAmount,
    ActivateLogistics,
    OpenDetail,
    OpenMarketDetail,
    NextSection,
    PreviousSection,
    BeginTravel,
    TravelUntilArrival,
    ClearContext,
    Inspect,
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

    match ui.input_layer {
        InputLayer::Amount => {
            return match code {
                KeyCode::Char(digit) if digit.is_ascii_digit() => InputAction::AmountDigit(digit),
                KeyCode::Backspace => InputAction::AmountBackspace,
                KeyCode::Enter => InputAction::ConfirmAmount,
                KeyCode::Char('m' | 'M') => InputAction::UseAmountMaximum,
                KeyCode::Esc => InputAction::CloseLayer,
                _ => InputAction::None,
            };
        }
        InputLayer::Help => {
            return matches!(code, KeyCode::Esc | KeyCode::Char('?'))
                .then_some(InputAction::CloseLayer)
                .unwrap_or(InputAction::None);
        }
        InputLayer::Detail => {
            return matches!(code, KeyCode::Esc)
                .then_some(InputAction::CloseLayer)
                .unwrap_or(InputAction::None);
        }
        InputLayer::Root => {}
    }

    match code {
        KeyCode::F(1) => return InputAction::Switch(Activity::Systems),
        KeyCode::F(2) => return InputAction::Switch(Activity::Trade),
        KeyCode::F(3) => return InputAction::Switch(Activity::Logistics),
        KeyCode::F(4) => return InputAction::Switch(Activity::Governance),
        KeyCode::F(5) => return InputAction::Switch(Activity::Intelligence),
        KeyCode::F(6) => return InputAction::Switch(Activity::Encyclopedia),
        KeyCode::Char('q') => return InputAction::Quit,
        KeyCode::Char(' ') => return InputAction::ToggleRun,
        KeyCode::Char('.') => return InputAction::Step,
        KeyCode::Char('r') => return InputAction::CycleTickRate,
        KeyCode::Char('?') => return InputAction::ToggleHelp,
        _ => {}
    }

    match ui.activity {
        Activity::Systems => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::Char('o') => InputAction::Sort,
            KeyCode::Char('d') => InputAction::ToggleSortDirection,
            KeyCode::Enter => InputAction::OpenDetail,
            KeyCode::Char('m') => InputAction::OpenMarketDetail,
            _ => InputAction::None,
        },
        Activity::Trade => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::Char('b' | 'B') => InputAction::OpenBuyAmount,
            KeyCode::Char('s' | 'S') => InputAction::OpenSellAmount,
            KeyCode::Char('t') | KeyCode::Enter => InputAction::BeginTravel,
            KeyCode::Char('g') => InputAction::TravelUntilArrival,
            KeyCode::Tab => InputAction::NextSection,
            KeyCode::BackTab => InputAction::PreviousSection,
            KeyCode::Esc => InputAction::ClearContext,
            _ => InputAction::None,
        },
        Activity::Logistics => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::Tab => InputAction::NextSection,
            KeyCode::BackTab => InputAction::PreviousSection,
            KeyCode::Enter => InputAction::ActivateLogistics,
            _ => InputAction::None,
        },
        Activity::Governance => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::Left => InputAction::Decrease,
            KeyCode::Right => InputAction::Increase,
            KeyCode::Char('i') => InputAction::Inspect,
            KeyCode::Tab => InputAction::NextSection,
            KeyCode::BackTab => InputAction::PreviousSection,
            KeyCode::Esc => InputAction::ClearContext,
            _ => InputAction::None,
        },
        Activity::Intelligence => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            _ => InputAction::None,
        },
        Activity::Encyclopedia => match code {
            KeyCode::Up | KeyCode::Char('k') => InputAction::MoveUp,
            KeyCode::Down | KeyCode::Char('j') => InputAction::MoveDown,
            KeyCode::PageUp => InputAction::PageUp,
            KeyCode::PageDown => InputAction::PageDown,
            KeyCode::Tab => InputAction::NextSection,
            KeyCode::BackTab => InputAction::PreviousSection,
            _ => InputAction::None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_entry_blocks_global_and_activity_actions() {
        let ui = UiState {
            activity: Activity::Trade,
            input_layer: InputLayer::Amount,
            ..UiState::default()
        };
        assert_eq!(route_key(KeyCode::F(2), &ui, true), InputAction::None);
        assert_eq!(route_key(KeyCode::Char('b'), &ui, true), InputAction::None);
        for key in ['m', 'M'] {
            assert_eq!(
                route_key(KeyCode::Char(key), &ui, true),
                InputAction::UseAmountMaximum
            );
        }
        assert_eq!(
            route_key(KeyCode::Enter, &ui, true),
            InputAction::ConfirmAmount
        );
        assert_eq!(route_key(KeyCode::Esc, &ui, true), InputAction::CloseLayer);
    }

    #[test]
    fn every_buy_and_sell_key_opens_exact_amount_entry() {
        let ui = UiState {
            activity: Activity::Trade,
            ..UiState::default()
        };
        for key in ['b', 'B'] {
            assert_eq!(
                route_key(KeyCode::Char(key), &ui, true),
                InputAction::OpenBuyAmount
            );
        }
        for key in ['s', 'S'] {
            assert_eq!(
                route_key(KeyCode::Char(key), &ui, true),
                InputAction::OpenSellAmount
            );
        }
        assert_eq!(
            route_key(KeyCode::Char('g'), &ui, true),
            InputAction::TravelUntilArrival
        );
    }

    #[test]
    fn logistics_uses_focused_actions_instead_of_opaque_letter_shortcuts() {
        let ui = UiState {
            activity: Activity::Logistics,
            ..UiState::default()
        };
        for key in ['e', 'x', 'f', 'p'] {
            assert_eq!(route_key(KeyCode::Char(key), &ui, true), InputAction::None);
        }
        assert_eq!(
            route_key(KeyCode::Enter, &ui, true),
            InputAction::ActivateLogistics
        );
        assert_eq!(route_key(KeyCode::Tab, &ui, true), InputAction::NextSection);
        assert_eq!(
            route_key(KeyCode::F(3), &UiState::default(), true),
            InputAction::Switch(Activity::Logistics)
        );
    }
}
