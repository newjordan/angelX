//! Window input as the events the terminal reader delivers: winit keys and
//! the mouse become crossterm `Event`s, with press, repeat and release, so
//! the games get every key's whole life.

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode, MouseButton,
};
use winit::event::{ElementState, KeyEvent as WindowKey, MouseButton as WindowButton};
use winit::keyboard::{
    Key, KeyCode as Physical, KeyLocation, ModifiersState, NamedKey, PhysicalKey,
};

/// The window's own keys; the cockpit never sees them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Paste,
    /// Replay the assembly transition.
    Transition,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

/// Ctrl+Shift+V or Shift+Insert pastes; Ctrl+Shift+= / − / 0 (or Backspace) zoom. Matched by
/// the character the key types, so any layout works,
/// and keymaps made up on the fly (virtual keyboards) too. A key that types
/// no Latin character falls back to its place on a US keyboard.
pub fn shortcut(event: &WindowKey, state: ModifiersState) -> Option<Shortcut> {
    shortcut_of(&event.logical_key, event.physical_key, state)
}

/// The shortcut logic on the event's parts, so it can be tested without
/// fabricating a winit event (which only winit can construct).
fn shortcut_of(logical: &Key, physical: PhysicalKey, state: ModifiersState) -> Option<Shortcut> {
    // Shift+Insert pastes too.
    if state.shift_key()
        && !state.control_key()
        && !state.alt_key()
        && *logical == Key::Named(NamedKey::Insert)
    {
        return Some(Shortcut::Paste);
    }
    // On macOS the window keys are Cmd: Cmd+V pastes,
    // Cmd+= / − / 0 zoom.
    if cfg!(target_os = "macos")
        && state.super_key()
        && !state.control_key()
        && let Key::Character(text) = logical
    {
        match text.to_ascii_lowercase().as_str() {
            "v" => return Some(Shortcut::Paste),
            "=" | "+" => return Some(Shortcut::ZoomIn),
            "-" | "_" => return Some(Shortcut::ZoomOut),
            "0" => return Some(Shortcut::ZoomReset),
            _ => {}
        }
    }
    if !(state.control_key() && state.shift_key()) {
        return None;
    }
    match logical {
        Key::Named(NamedKey::Backspace) => return Some(Shortcut::ZoomReset),
        Key::Character(text) if text.is_ascii() => {
            return match text.to_ascii_lowercase().as_str() {
                "v" => Some(Shortcut::Paste),
                "t" => Some(Shortcut::Transition),
                "=" | "+" => Some(Shortcut::ZoomIn),
                "-" | "_" => Some(Shortcut::ZoomOut),
                "0" | ")" => Some(Shortcut::ZoomReset),
                _ => None,
            };
        }
        _ => {}
    }
    match physical {
        PhysicalKey::Code(Physical::KeyV) => Some(Shortcut::Paste),
        PhysicalKey::Code(Physical::KeyT) => Some(Shortcut::Transition),
        PhysicalKey::Code(Physical::Equal) => Some(Shortcut::ZoomIn),
        PhysicalKey::Code(Physical::Minus) => Some(Shortcut::ZoomOut),
        PhysicalKey::Code(Physical::Digit0) => Some(Shortcut::ZoomReset),
        _ => None,
    }
}

pub fn modifiers(state: ModifiersState) -> KeyModifiers {
    let mut out = KeyModifiers::NONE;
    if state.shift_key() {
        out |= KeyModifiers::SHIFT;
    }
    if state.control_key() {
        out |= KeyModifiers::CONTROL;
    }
    if state.alt_key() {
        out |= KeyModifiers::ALT;
    }
    if state.super_key() {
        out |= KeyModifiers::SUPER;
    }
    out
}

/// `game`: a game holds the keyboard, so modifier keys report on their own
/// and Shift stays on every key, as the terminal's game keyboard mode does.
pub fn key(event: &WindowKey, state: ModifiersState, game: bool) -> Option<KeyEvent> {
    key_of(
        &event.logical_key,
        event.text.as_deref(),
        event.location,
        event.state,
        event.repeat,
        state,
        game,
    )
}

/// The key logic on the event's parts (`shortcut_of`'s companion).
#[allow(clippy::too_many_arguments)]
fn key_of(
    logical: &Key,
    text: Option<&str>,
    location: KeyLocation,
    state: ElementState,
    repeat: bool,
    mods_state: ModifiersState,
    game: bool,
) -> Option<KeyEvent> {
    let kind = match (state, repeat) {
        (ElementState::Pressed, false) => KeyEventKind::Press,
        (ElementState::Pressed, true) => KeyEventKind::Repeat,
        (ElementState::Released, _) => KeyEventKind::Release,
    };
    let mut mods = modifiers(mods_state);
    let code = match logical {
        Key::Named(named) => named_key(*named, location, game, &mut mods)?,
        Key::Character(text) => character(text.chars().next()?, game, &mut mods),
        Key::Dead(_) | Key::Unidentified(_) => {
            let ch = text?.chars().next()?;
            character(ch, game, &mut mods)
        }
    };
    Some(KeyEvent::new_with_kind(code, mods, kind))
}

/// A typed character. Outside games, Shift only stays on letters, as a
/// terminal reports `A` but plain `!`.
fn character(ch: char, game: bool, mods: &mut KeyModifiers) -> KeyCode {
    if !game && !ch.is_alphabetic() {
        mods.remove(KeyModifiers::SHIFT);
    }
    KeyCode::Char(ch)
}

fn named_key(
    key: NamedKey,
    location: KeyLocation,
    game: bool,
    mods: &mut KeyModifiers,
) -> Option<KeyCode> {
    let right = location == KeyLocation::Right;
    let modifier =
        |left, right_code| game.then_some(KeyCode::Modifier(if right { right_code } else { left }));
    Some(match key {
        NamedKey::Enter => KeyCode::Enter,
        NamedKey::Tab if mods.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
        NamedKey::Tab => KeyCode::Tab,
        NamedKey::Space => {
            if !game {
                mods.remove(KeyModifiers::SHIFT);
            }
            KeyCode::Char(' ')
        }
        NamedKey::Backspace => KeyCode::Backspace,
        NamedKey::Escape => KeyCode::Esc,
        NamedKey::Delete => KeyCode::Delete,
        NamedKey::Insert => KeyCode::Insert,
        NamedKey::Home => KeyCode::Home,
        NamedKey::End => KeyCode::End,
        NamedKey::PageUp => KeyCode::PageUp,
        NamedKey::PageDown => KeyCode::PageDown,
        NamedKey::ArrowLeft => KeyCode::Left,
        NamedKey::ArrowRight => KeyCode::Right,
        NamedKey::ArrowUp => KeyCode::Up,
        NamedKey::ArrowDown => KeyCode::Down,
        NamedKey::PrintScreen => KeyCode::PrintScreen,
        NamedKey::Pause => KeyCode::Pause,
        NamedKey::ContextMenu => KeyCode::Menu,
        NamedKey::CapsLock if game => KeyCode::CapsLock,
        NamedKey::NumLock if game => KeyCode::NumLock,
        NamedKey::ScrollLock if game => KeyCode::ScrollLock,
        NamedKey::Shift => modifier(ModifierKeyCode::LeftShift, ModifierKeyCode::RightShift)?,
        NamedKey::Control => modifier(ModifierKeyCode::LeftControl, ModifierKeyCode::RightControl)?,
        NamedKey::Alt => modifier(ModifierKeyCode::LeftAlt, ModifierKeyCode::RightAlt)?,
        NamedKey::Super => modifier(ModifierKeyCode::LeftSuper, ModifierKeyCode::RightSuper)?,
        NamedKey::Meta => modifier(ModifierKeyCode::LeftMeta, ModifierKeyCode::RightMeta)?,
        NamedKey::Hyper => modifier(ModifierKeyCode::LeftHyper, ModifierKeyCode::RightHyper)?,
        other => function_key(other)?,
    })
}

fn function_key(key: NamedKey) -> Option<KeyCode> {
    use NamedKey::*;
    let n = match key {
        F1 => 1,
        F2 => 2,
        F3 => 3,
        F4 => 4,
        F5 => 5,
        F6 => 6,
        F7 => 7,
        F8 => 8,
        F9 => 9,
        F10 => 10,
        F11 => 11,
        F12 => 12,
        F13 => 13,
        F14 => 14,
        F15 => 15,
        F16 => 16,
        F17 => 17,
        F18 => 18,
        F19 => 19,
        F20 => 20,
        F21 => 21,
        F22 => 22,
        F23 => 23,
        F24 => 24,
        _ => return None,
    };
    Some(KeyCode::F(n))
}

pub fn button(button: WindowButton) -> Option<MouseButton> {
    match button {
        WindowButton::Left => Some(MouseButton::Left),
        WindowButton::Right => Some(MouseButton::Right),
        WindowButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL_SHIFT: ModifiersState = ModifiersState::CONTROL.union(ModifiersState::SHIFT);
    const SHIFT: ModifiersState = ModifiersState::SHIFT;

    fn press(logical: &Key, mods: ModifiersState, game: bool) -> Option<KeyEvent> {
        key_of(logical, None, KeyLocation::Standard, ElementState::Pressed, false, mods, game)
    }

    #[test]
    fn window_shortcuts_take_ctrl_shift() {
        let at = |text: &str| {
            shortcut_of(&Key::Character(text.into()), PhysicalKey::Code(Physical::Space), CTRL_SHIFT)
        };
        assert_eq!(at("v"), Some(Shortcut::Paste));
        assert_eq!(at("V"), Some(Shortcut::Paste));
        assert_eq!(at("t"), Some(Shortcut::Transition));
        assert_eq!(at("+"), Some(Shortcut::ZoomIn));
        assert_eq!(at("="), Some(Shortcut::ZoomIn));
        assert_eq!(at("_"), Some(Shortcut::ZoomOut));
        assert_eq!(at(")"), Some(Shortcut::ZoomReset));
        assert_eq!(at("x"), None);
        // Without both modifiers the cockpit keeps the key.
        assert_eq!(
            shortcut_of(&Key::Character("v".into()), PhysicalKey::Code(Physical::Space), SHIFT),
            None
        );
    }

    #[test]
    fn shift_insert_pastes_and_non_latin_falls_back_to_the_physical_key() {
        assert_eq!(
            shortcut_of(&Key::Named(NamedKey::Insert), PhysicalKey::Code(Physical::Space), SHIFT),
            Some(Shortcut::Paste)
        );
        // A layout where the V position types a Cyrillic letter still pastes.
        assert_eq!(
            shortcut_of(
                &Key::Character("м".into()),
                PhysicalKey::Code(Physical::KeyV),
                CTRL_SHIFT
            ),
            Some(Shortcut::Paste)
        );
    }

    #[test]
    fn press_repeat_and_release_survive() {
        let logical = Key::Character("a".into());
        let kind = |state, repeat| {
            key_of(&logical, None, KeyLocation::Standard, state, repeat, ModifiersState::empty(), false)
                .unwrap()
                .kind
        };
        assert_eq!(kind(ElementState::Pressed, false), KeyEventKind::Press);
        assert_eq!(kind(ElementState::Pressed, true), KeyEventKind::Repeat);
        assert_eq!(kind(ElementState::Released, false), KeyEventKind::Release);
    }

    #[test]
    fn shift_stays_on_letters_not_symbols_outside_games() {
        let upper = press(&Key::Character("A".into()), SHIFT, false).unwrap();
        assert!(upper.modifiers.contains(KeyModifiers::SHIFT));
        let bang = press(&Key::Character("!".into()), SHIFT, false).unwrap();
        assert!(!bang.modifiers.contains(KeyModifiers::SHIFT), "a terminal reports plain !");
        let bang_game = press(&Key::Character("!".into()), SHIFT, true).unwrap();
        assert!(bang_game.modifiers.contains(KeyModifiers::SHIFT), "games keep it");
    }

    #[test]
    fn named_keys_follow_the_terminal_contract() {
        let backtab = press(&Key::Named(NamedKey::Tab), SHIFT, false).unwrap();
        assert_eq!(backtab.code, KeyCode::BackTab);
        let tab = press(&Key::Named(NamedKey::Tab), ModifiersState::empty(), false).unwrap();
        assert_eq!(tab.code, KeyCode::Tab);
        let f13 = press(&Key::Named(NamedKey::F13), ModifiersState::empty(), false).unwrap();
        assert_eq!(f13.code, KeyCode::F(13));
        // Lock and bare modifier keys only speak in game keyboard mode.
        assert!(press(&Key::Named(NamedKey::CapsLock), ModifiersState::empty(), false).is_none());
        assert!(press(&Key::Named(NamedKey::CapsLock), ModifiersState::empty(), true).is_some());
        assert!(press(&Key::Named(NamedKey::Shift), SHIFT, false).is_none());
        let right_shift = key_of(
            &Key::Named(NamedKey::Shift),
            None,
            KeyLocation::Right,
            ElementState::Pressed,
            false,
            SHIFT,
            true,
        )
        .unwrap();
        assert_eq!(right_shift.code, KeyCode::Modifier(ModifierKeyCode::RightShift));
    }

    #[test]
    fn dead_and_unidentified_keys_use_their_text_or_stay_silent() {
        let dead = key_of(
            &Key::Dead(Some('´')),
            Some("´"),
            KeyLocation::Standard,
            ElementState::Pressed,
            false,
            ModifiersState::empty(),
            false,
        )
        .unwrap();
        assert_eq!(dead.code, KeyCode::Char('´'));
        assert!(
            key_of(
                &Key::Unidentified(winit::keyboard::NativeKey::Unidentified),
                None,
                KeyLocation::Standard,
                ElementState::Pressed,
                false,
                ModifiersState::empty(),
                false,
            )
            .is_none()
        );
    }

    #[test]
    fn buttons_map_and_extras_drop() {
        assert_eq!(button(WindowButton::Left), Some(MouseButton::Left));
        assert_eq!(button(WindowButton::Middle), Some(MouseButton::Middle));
        assert_eq!(button(WindowButton::Other(7)), None);
    }
}
