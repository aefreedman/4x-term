use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Player-selected navigation layout. Arrow keys are universal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum KeyboardLayout {
    #[default]
    Qwerty,
    ColemakDh,
}

impl KeyboardLayout {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Qwerty => "QWERTY (h/j/k/l)",
            Self::ColemakDh => "Colemak-DH (u/n/e/i)",
        }
    }

    #[must_use]
    pub const fn directional_hint(self) -> &'static str {
        match self {
            Self::Qwerty => "Arrows/h/j/k/l",
            Self::ColemakDh => "Arrows/u/n/e/i",
        }
    }

    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::Qwerty => Self::ColemakDh,
            Self::ColemakDh => Self::Qwerty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Semantic input consumed by state components. Layout-specific letters never
/// enter a widget directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Navigate(Direction),
    NextFocus,
    PreviousFocus,
    Confirm,
    Cancel,
    PageUp,
    PageDown,
    Home,
    End,
    Help,
    Settings,
    Quit,
    AdvanceOne,
    AdvanceMany,
    Pause,
    Backspace,
    Delete,
    Character(char),
    Ignore,
}

/// Maps a key after the router has declared whether an editor or a navigable
/// component has focus. Editors receive printable characters before layout and
/// global shortcuts, implementing the approved modal precedence.
#[must_use]
pub fn map_key(
    key: KeyEvent,
    layout: KeyboardLayout,
    editor_focused: bool,
    accepts_navigation: bool,
) -> Action {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return Action::Ignore;
    }
    match key.code {
        KeyCode::Esc => return Action::Cancel,
        KeyCode::Enter => return Action::Confirm,
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
            return Action::PreviousFocus;
        }
        KeyCode::Tab => return Action::NextFocus,
        KeyCode::BackTab => return Action::PreviousFocus,
        KeyCode::Backspace => return Action::Backspace,
        KeyCode::Delete => return Action::Delete,
        KeyCode::Up if accepts_navigation => return Action::Navigate(Direction::Up),
        KeyCode::Down if accepts_navigation => return Action::Navigate(Direction::Down),
        KeyCode::Left if accepts_navigation => return Action::Navigate(Direction::Left),
        KeyCode::Right if accepts_navigation => return Action::Navigate(Direction::Right),
        KeyCode::PageUp => return Action::PageUp,
        KeyCode::PageDown => return Action::PageDown,
        KeyCode::Home => return Action::Home,
        KeyCode::End => return Action::End,
        KeyCode::F(2) => return Action::Settings,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Action::Quit;
        }
        _ => {}
    }

    if let KeyCode::Char(character) = key.code {
        if editor_focused {
            return Action::Character(character);
        }
        let command_modifiers = KeyModifiers::CONTROL
            | KeyModifiers::ALT
            | KeyModifiers::SUPER
            | KeyModifiers::HYPER
            | KeyModifiers::META;
        let shortcut_allowed = !key.modifiers.intersects(command_modifiers);
        let shortcut = character.to_ascii_lowercase();
        if shortcut == '?' && shortcut_allowed {
            return Action::Help;
        }
        if accepts_navigation && shortcut_allowed {
            let direction = match (layout, shortcut) {
                (KeyboardLayout::Qwerty, 'k') => Some(Direction::Up),
                (KeyboardLayout::Qwerty, 'j') => Some(Direction::Down),
                (KeyboardLayout::Qwerty, 'h') => Some(Direction::Left),
                (KeyboardLayout::Qwerty, 'l') => Some(Direction::Right),
                (KeyboardLayout::ColemakDh, 'u') => Some(Direction::Up),
                (KeyboardLayout::ColemakDh, 'e') => Some(Direction::Down),
                (KeyboardLayout::ColemakDh, 'n') => Some(Direction::Left),
                (KeyboardLayout::ColemakDh, 'i') => Some(Direction::Right),
                _ => None,
            };
            if let Some(direction) = direction {
                return Action::Navigate(direction);
            }
        }
        if shortcut_allowed {
            return match shortcut {
                '?' => Action::Help,
                'q' => Action::Quit,
                's' => Action::Settings,
                'r' => Action::Character('r'),
                '.' => Action::AdvanceOne,
                't' => Action::AdvanceMany,
                ' ' => Action::Pause,
                _ => Action::Character(shortcut),
            };
        }
    }
    Action::Ignore
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn every_direction_maps_for_all_layouts_and_arrows() {
        for (code, expected) in [
            (KeyCode::Up, Direction::Up),
            (KeyCode::Down, Direction::Down),
            (KeyCode::Left, Direction::Left),
            (KeyCode::Right, Direction::Right),
        ] {
            assert_eq!(
                map_key(key(code), KeyboardLayout::Qwerty, false, true),
                Action::Navigate(expected)
            );
            assert_eq!(
                map_key(key(code), KeyboardLayout::ColemakDh, false, true),
                Action::Navigate(expected)
            );
        }
        for (layout, values) in [
            (
                KeyboardLayout::Qwerty,
                [
                    ('k', Direction::Up),
                    ('j', Direction::Down),
                    ('h', Direction::Left),
                    ('l', Direction::Right),
                ],
            ),
            (
                KeyboardLayout::ColemakDh,
                [
                    ('u', Direction::Up),
                    ('e', Direction::Down),
                    ('n', Direction::Left),
                    ('i', Direction::Right),
                ],
            ),
        ] {
            for (character, expected) in values {
                assert_eq!(
                    map_key(key(KeyCode::Char(character)), layout, false, true),
                    Action::Navigate(expected)
                );
            }
        }
    }

    #[test]
    fn shifted_question_mark_and_settings_keys_map_to_modals() {
        assert_eq!(
            map_key(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
                KeyboardLayout::Qwerty,
                false,
                true,
            ),
            Action::Help
        );
        assert_eq!(
            map_key(key(KeyCode::F(2)), KeyboardLayout::Qwerty, false, true,),
            Action::Settings
        );
        assert_eq!(
            map_key(key(KeyCode::Char('s')), KeyboardLayout::Qwerty, false, true,),
            Action::Settings
        );
    }

    #[test]
    fn shifted_and_uppercase_terminal_events_still_reach_shortcuts() {
        for (character, expected) in [
            ('Q', Action::Quit),
            ('S', Action::Settings),
            ('B', Action::Character('b')),
        ] {
            assert_eq!(
                map_key(
                    KeyEvent::new(KeyCode::Char(character), KeyModifiers::SHIFT),
                    KeyboardLayout::Qwerty,
                    false,
                    true,
                ),
                expected
            );
        }
    }

    #[test]
    fn focused_editor_wins_over_navigation_and_global_keys() {
        for character in ['q', 't', 'r', 'u', 'n', 'e', 'i'] {
            assert_eq!(
                map_key(
                    key(KeyCode::Char(character)),
                    KeyboardLayout::ColemakDh,
                    true,
                    true
                ),
                Action::Character(character)
            );
        }
    }
}
