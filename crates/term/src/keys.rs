//! The window's keys, pastes and mouse as bytes a program takes in: the
//! legacy dialect by default, or the CSI u progressive keyboard dialect
//! when the program's flags ask for it.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode, MouseButton, MouseEvent, MouseEventKind};

/// What a program asked to hear about the mouse (DECSET 1000/1002/1003).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseProtocolMode {
    None,
    PressRelease,
    ButtonMotion,
    AnyMotion,
}

/// How the program asked to hear it (DECSET 1005/1006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseProtocolEncoding {
    Default,
    Utf8,
    Sgr,
}

/// The flags of the CSI u keyboard dialect, as a program pushes them
/// (`CSI > flags u`). With none set, keys stay legacy bytes.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct KeyboardDialect {
    /// `Esc`, `alt`+key, `ctrl`+key and their kin arrive as `CSI u`
    /// codes instead of the raw bytes they have always typed.
    pub disambiguate: bool,
    /// Press, repeat and release are told apart.
    pub event_types: bool,
    /// The shifted key rides along with the base one.
    pub alternate_keys: bool,
    /// Every key arrives as a code, text keys included.
    pub all_keys_as_escape: bool,
    /// The text a key types rides along.
    pub associated_text: bool,
}

impl KeyboardDialect {
    /// The flags as `engine::keyboard_flags` reports them.
    pub fn from_bits(bits: u8) -> Self {
        Self {
            disambiguate: bits & 1 != 0,
            event_types: bits & 2 != 0,
            alternate_keys: bits & 4 != 0,
            all_keys_as_escape: bits & 8 != 0,
            associated_text: bits & 16 != 0,
        }
    }
}

/// The modifier parameter: 1 plus one bit per modifier held. The legacy
/// dialect counts the first three only; the CSI u dialect counts them
/// all, `super`, `hyper` and `meta` included.
fn modifier_param(mods: KeyModifiers, legacy: bool) -> u32 {
    let mut bits = 0u32;
    if mods.contains(KeyModifiers::SHIFT) {
        bits += 1;
    }
    if mods.contains(KeyModifiers::ALT) {
        bits += 2;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        bits += 4;
    }
    if !legacy {
        if mods.contains(KeyModifiers::SUPER) {
            bits += 8;
        }
        if mods.contains(KeyModifiers::HYPER) {
            bits += 16;
        }
        if mods.contains(KeyModifiers::META) {
            bits += 32;
        }
    }
    1 + bits
}

/// Ctrl with a key: the C0 control it has always typed.
fn control(c: char) -> Option<u8> {
    Some(match c.to_ascii_lowercase() {
        c @ 'a'..='z' => c as u8 - b'a' + 1,
        '@' | ' ' | '2' => 0,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '-' | '7' | '/' => 0x1f,
        '?' | '8' => 0x7f,
        _ => return None,
    })
}

/// One key, as the program's flags ask to hear it.
pub fn key(key: &KeyEvent, application_cursor: bool, dialect: KeyboardDialect) -> Option<Vec<u8>> {
    // The legacy dialect has no release; a repeat types the key again.
    if dialect == KeyboardDialect::default() {
        return if key.kind == KeyEventKind::Release { None } else { legacy(key, application_cursor) };
    }
    // Without event types a repeat is a press again.
    let mut key = *key;
    if !dialect.event_types && key.kind == KeyEventKind::Repeat {
        key.kind = KeyEventKind::Press;
    }
    if let Some(bytes) = csi_u(&key, dialect) {
        return Some(bytes);
    }
    // The key keeps its text or legacy bytes: a press or repeat types
    // them, and a release, which those forms cannot say, sends nothing.
    if key.kind == KeyEventKind::Release {
        return None;
    }
    legacy(&key, application_cursor)
}

/// A key in the CSI u dialect: the number it is known by and the form
/// its escape code takes.
#[derive(Clone, Copy)]
enum Functional {
    /// `CSI 1;mods A/B/C/D/H/F/P/Q/S`, with the `1` and the modifiers
    /// field omitted when nothing follows them.
    Letter(char),
    /// `CSI n;mods ~`.
    Tilde(u32),
    /// `CSI n;mods u`.
    Code(u32),
}

fn functional(key: &KeyCode) -> Option<Functional> {
    Some(match key {
        KeyCode::Up => Functional::Letter('A'),
        KeyCode::Down => Functional::Letter('B'),
        KeyCode::Right => Functional::Letter('C'),
        KeyCode::Left => Functional::Letter('D'),
        KeyCode::Home => Functional::Letter('H'),
        KeyCode::End => Functional::Letter('F'),
        KeyCode::F(1) => Functional::Letter('P'),
        KeyCode::F(2) => Functional::Letter('Q'),
        KeyCode::F(4) => Functional::Letter('S'),
        KeyCode::Insert => Functional::Tilde(2),
        KeyCode::Delete => Functional::Tilde(3),
        KeyCode::F(3) => Functional::Tilde(13),
        KeyCode::PageUp => Functional::Tilde(5),
        KeyCode::PageDown => Functional::Tilde(6),
        KeyCode::F(n @ 5..=12) => Functional::Tilde(match n {
            5 => 15,
            6 => 17,
            7 => 18,
            8 => 19,
            9 => 20,
            10 => 21,
            11 => 23,
            _ => 24,
        }),
        KeyCode::CapsLock => Functional::Code(57358),
        KeyCode::ScrollLock => Functional::Code(57359),
        KeyCode::NumLock => Functional::Code(57360),
        KeyCode::PrintScreen => Functional::Code(57361),
        KeyCode::Pause => Functional::Code(57362),
        KeyCode::Menu => Functional::Code(57363),
        KeyCode::F(n @ 13..=35) => Functional::Code(57363 + u32::from(*n)),
        KeyCode::Modifier(ModifierKeyCode::LeftShift) => Functional::Code(57441),
        KeyCode::Modifier(ModifierKeyCode::LeftControl) => Functional::Code(57442),
        KeyCode::Modifier(ModifierKeyCode::LeftAlt) => Functional::Code(57443),
        KeyCode::Modifier(ModifierKeyCode::LeftSuper) => Functional::Code(57444),
        KeyCode::Modifier(ModifierKeyCode::LeftHyper) => Functional::Code(57445),
        KeyCode::Modifier(ModifierKeyCode::LeftMeta) => Functional::Code(57446),
        KeyCode::Modifier(ModifierKeyCode::RightShift) => Functional::Code(57447),
        KeyCode::Modifier(ModifierKeyCode::RightControl) => Functional::Code(57448),
        KeyCode::Modifier(ModifierKeyCode::RightAlt) => Functional::Code(57449),
        KeyCode::Modifier(ModifierKeyCode::RightSuper) => Functional::Code(57450),
        KeyCode::Modifier(ModifierKeyCode::RightHyper) => Functional::Code(57451),
        KeyCode::Modifier(ModifierKeyCode::RightMeta) => Functional::Code(57452),
        _ => return None,
    })
}

/// The codepoint a key is known by in the CSI u dialect: always the
/// unshifted one. The window turns Shift+a into `Char('A')` with SHIFT
/// held, and Shift+Tab into `BackTab`; the number sent is the one the
/// key has unshifted, and the shifted form rides along only when
/// alternate keys are asked for. Returns the base and, when Shift is
/// held and the key has a distinct shifted form, that form.
fn codepoint(code: KeyCode, shift: bool) -> Option<(u32, Option<u32>)> {
    Some(match code {
        KeyCode::Char(c) => {
            let base = c.to_ascii_lowercase();
            let shifted = if shift && base != c { Some(u32::from(c)) } else { None };
            (u32::from(base), shifted)
        }
        KeyCode::BackTab => (9, None),
        KeyCode::Tab => (9, None),
        KeyCode::Enter => (13, None),
        KeyCode::Backspace => (127, None),
        KeyCode::Esc => (27, None),
        KeyCode::Null => (0, None),
        // A functional key is built from its table entry below, not a
        // codepoint of its own.
        other => return functional(&other).is_none().then_some((0, None)).filter(|_| false),
    })
}

/// One key in the CSI u dialect, or `None` when the program does not
/// hear this event from it: a key that goes out as text or as a legacy
/// byte has no release, and a modifier key on its own is heard only
/// under "all keys". `None` lets the caller fall back to the legacy
/// form for the keys that keep one.
fn csi_u(key: &KeyEvent, dialect: KeyboardDialect) -> Option<Vec<u8>> {
    let mods = key.modifiers;
    let shift = mods.contains(KeyModifiers::SHIFT);
    let all = dialect.all_keys_as_escape;
    let plain = mods == KeyModifiers::NONE;

    // The window sends modifier keys only in game mode; under "all keys"
    // they are codes, otherwise a program hears nothing of them.
    if let KeyCode::Modifier(_) = key.code && !all {
        return None;
    }

    // Which form the key takes: a functional key's own, or the
    // codepoint of the character it types.
    let func = functional(&key.code);

    // Enter, Tab and Backspace stay their legacy bytes under
    // disambiguation alone, with no modifiers, so a shell stays usable
    // after a program that pushed flags dies without clearing them;
    // under "all keys" they are codes like everything else.
    let legacy_fallback = matches!(
        key.code,
        KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab | KeyCode::Backspace
    ) && !all;

    // A text key with no modifier but Shift stays the text it types
    // (Shift+a types `A`) unless every key was asked for as a code. The
    // program hears the text on press and again on each repeat, and
    // nothing at all on release.
    let text_fallback = matches!(key.code, KeyCode::Char(_))
        && mods.difference(KeyModifiers::SHIFT).is_empty()
        && !all;

    if key.kind == KeyEventKind::Press {
        // On press, a key that keeps its legacy form falls through so
        // the legacy encoder produces it; under "all keys" nothing
        // falls through.
        if (legacy_fallback && plain) || text_fallback {
            return None;
        }
    } else {
        // A release or a repeat is reported only for a key that this
        // dialect sends as an escape code: functional keys always, the
        // character keys that disambiguation turns into codes, and
        // everything under "all keys".
        let as_code = all
            || func.is_some()
            || matches!(key.code, KeyCode::Char(_) | KeyCode::Esc)
                && !((legacy_fallback && plain) || text_fallback);
        if !as_code {
            return None;
        }
    }

    let param = modifier_param(mods, false);
    let event_type = match (dialect.event_types, key.kind) {
        (false, _) => None,
        (_, KeyEventKind::Press) => Some(1),
        (_, KeyEventKind::Repeat) => Some(2),
        (_, KeyEventKind::Release) => Some(3),
    };
    // The modifiers field is written whenever an event type or the text
    // field follows it, even when no modifier is held.
    let write_params = param > 1 || event_type.is_some() || dialect.associated_text;

    // A functional key: one of the three shapes, with the modifiers
    // field and event type packed as `1;mods:type`.
    if let Some(f) = func {
        let params = || -> String {
            if !write_params {
                return String::new();
            }
            match event_type {
                Some(t) => format!(";{param}:{t}"),
                None => format!(";{param}"),
            }
        };
        return Some(
            match f {
                // The letter form omits the `1` when no parameter
                // follows it: `CSI A`, not `CSI 1 A`.
                Functional::Letter(end) if params().is_empty() => format!("\x1b[{end}"),
                Functional::Letter(end) => format!("\x1b[1{}{end}", params()),
                Functional::Tilde(n) => format!("\x1b[{n}{}~", params()),
                Functional::Code(n) => format!("\x1b[{n}{}u", params()),
            }
            .into_bytes(),
        );
    }

    let (base, shifted) = codepoint(key.code, shift)?;
    let mut key_field = base.to_string();
    if let (true, Some(shifted)) = (dialect.alternate_keys, shifted) {
        key_field = format!("{base}:{shifted}");
    }
    let mut out = format!("\x1b[{key_field}");
    if write_params {
        out.push_str(&format!(";{param}"));
        if let Some(t) = event_type {
            out.push_str(&format!(":{t}"));
        }
    }
    // The text a key types rides on press and repeat; a release types
    // nothing, so it carries no text field.
    if let Some(text) = (dialect.associated_text && key.kind != KeyEventKind::Release)
        .then(|| match key.code {
            KeyCode::Char(c) => Some(u32::from(c)),
            _ => None,
        })
        && let Some(text) = text
    {
        out.push_str(&format!(";{text}"));
    }
    out.push('u');
    Some(out.into_bytes())
}

/// The legacy dialect: what keys have always typed.
fn legacy(key: &KeyEvent, application_cursor: bool) -> Option<Vec<u8>> {
    let mods = key.modifiers;
    let alt = mods.contains(KeyModifiers::ALT);
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let shift = mods.contains(KeyModifiers::SHIFT);
    let param = modifier_param(mods, true);
    // Cursor keys: SS3 in application mode, CSI otherwise, and CSI 1;m
    // with a modifier.
    let cursor = |end: char| -> Vec<u8> {
        if param > 1 {
            format!("\x1b[1;{param}{end}").into_bytes()
        } else if application_cursor {
            format!("\x1bO{end}").into_bytes()
        } else {
            format!("\x1b[{end}").into_bytes()
        }
    };
    let tilde = |n: u8| -> Vec<u8> {
        if param > 1 { format!("\x1b[{n};{param}~").into_bytes() } else { format!("\x1b[{n}~").into_bytes() }
    };
    // F1 to F4: SS3 P to S without modifiers, `CSI 1;m P` to `S` with.
    let f_key = |end: char, param: u32| -> Vec<u8> {
        if param > 1 {
            format!("\x1b[1;{param}{end}").into_bytes()
        } else {
            format!("\x1bO{end}").into_bytes()
        }
    };
    let escaped = |bytes: &[u8]| -> Vec<u8> {
        let mut out = Vec::with_capacity(bytes.len() + 1);
        if alt {
            out.push(0x1b);
        }
        out.extend_from_slice(bytes);
        out
    };
    Some(match key.code {
        KeyCode::Char(c) => match (ctrl, control(c)) {
            (true, Some(byte)) => escaped(&[byte]),
            _ => escaped(c.encode_utf8(&mut [0; 4]).as_bytes()),
        },
        // Shift+Enter is Alt+Enter: the newline that multi-line prompts
        // take when keys arrive as legacy bytes.
        KeyCode::Enter if shift && !ctrl => b"\x1b\r".to_vec(),
        KeyCode::Enter => escaped(b"\r"),
        KeyCode::Tab => escaped(b"\t"),
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Backspace if ctrl => escaped(&[0x08]),
        KeyCode::Backspace => escaped(b"\x7f"),
        KeyCode::Esc => escaped(&[0x1b]),
        KeyCode::Up => cursor('A'),
        KeyCode::Down => cursor('B'),
        KeyCode::Right => cursor('C'),
        KeyCode::Left => cursor('D'),
        KeyCode::Home => cursor('H'),
        KeyCode::End => cursor('F'),
        KeyCode::PageUp => tilde(5),
        KeyCode::PageDown => tilde(6),
        KeyCode::Insert => tilde(2),
        KeyCode::Delete => tilde(3),
        // F1 to F4 are SS3 without modifiers whatever cursor mode says;
        // with a modifier they take the CSI form, which is not DCH.
        KeyCode::F(1) => f_key('P', param),
        KeyCode::F(2) => f_key('Q', param),
        KeyCode::F(3) => f_key('R', param),
        KeyCode::F(4) => f_key('S', param),
        KeyCode::F(n @ 5..=12) => tilde(match n {
            5 => 15,
            6 => 17,
            7 => 18,
            8 => 19,
            9 =>  20,
            10 => 21,
            11 => 23,
            _ => 24,
        }),
        // The window has no F13 and above of its own to send; nothing
        // rather than a repeat of F12.
        KeyCode::F(_) => return None,
        KeyCode::Menu => tilde(29),
        KeyCode::Null => vec![0],
        _ => return None,
    })
}

/// A paste, bracketed when the program asked for that, with the
/// sequences a paste must never contain defanged.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    // A paste must never carry an escape of its own: strip them first,
    // then bracket what is left.
    let text = text.replace('\x1b', "");
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

/// Scroll on the alternate screen becomes arrow keys, three lines a notch.
pub fn arrow(up: bool, application_cursor: bool) -> Vec<u8> {
    legacy(
        &KeyEvent {
            code: if up { KeyCode::Up } else { KeyCode::Down },
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::empty(),
        },
        application_cursor,
    )
    .unwrap_or_default()
}

/// A mouse event reported to a program that asked for the mouse, in the
/// mode and encoding it asked for.
pub fn mouse(event: &MouseEvent, mode: MouseProtocolMode, encoding: MouseProtocolEncoding) -> Option<Vec<u8>> {
    use MouseProtocolMode as Mode;
    let button = |b: MouseButton| match b {
        MouseButton::Left => 0u32,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    let (code, release) = match event.kind {
        MouseEventKind::Down(b) => (button(b), false),
        MouseEventKind::Up(_) if mode == Mode::None => return None,
        MouseEventKind::Up(b) => (button(b), true),
        MouseEventKind::Drag(b) if matches!(mode, Mode::ButtonMotion | Mode::AnyMotion) => (32 + button(b), false),
        MouseEventKind::Moved if mode == Mode::AnyMotion => (35, false),
        MouseEventKind::Drag(_) | MouseEventKind::Moved => return None,
        MouseEventKind::ScrollUp => (64, false),
        MouseEventKind::ScrollDown => (65, false),
        MouseEventKind::ScrollLeft => (66, false),
        MouseEventKind::ScrollRight => (67, false),
    };
    let mods = event.modifiers;
    let held = 4 * u32::from(mods.contains(KeyModifiers::SHIFT))
        + 8 * u32::from(mods.contains(KeyModifiers::ALT))
        + 16 * u32::from(mods.contains(KeyModifiers::CONTROL));
    let (x, y) = (u32::from(event.column) + 1, u32::from(event.row) + 1);
    Some(match encoding {
        MouseProtocolEncoding::Sgr => {
            format!("\x1b[<{};{x};{y}{}", code + held, if release { 'm' } else { 'M' }).into_bytes()
        }
        // The older encodings have no release of their own: button 3.
        MouseProtocolEncoding::Default | MouseProtocolEncoding::Utf8 => {
            let code = if release { 3 } else { code } + held;
            match encoding {
                MouseProtocolEncoding::Utf8 => encode_utf8_mouse(code, x, y),
                _ => encode_default_mouse(code, x, y),
            }
        }
    })
}

/// The default (X10) encoding: one byte each, offset by 32, wrapped at 255.
fn encode_default_mouse(code: u32, x: u32, y: u32) -> Vec<u8> {
    let byte = |v: u32| v.clamp(0, 223) as u8;
    vec![0x1b, b'[', b'M', 32 + byte(code), 32 + byte(x), 32 + byte(y)]
}

/// The UTF-8 encoding: the same numbers as UTF-8 runes.
fn encode_utf8_mouse(code: u32, x: u32, y: u32) -> Vec<u8> {
    let rune = |v: u32| char::from_u32(v + 32).unwrap_or(' ');
    let mut out = b"\x1b[M".to_vec();
    for v in [code, x, y] {
        let mut buf = [0; 4];
        out.extend_from_slice(rune(v).encode_utf8(&mut buf).as_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, ModifierKeyCode};

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::empty() }
    }

    fn kind(mut key: KeyEvent, kind: KeyEventKind) -> KeyEvent {
        key.kind = kind;
        key
    }

    fn bytes(code: KeyCode, modifiers: KeyModifiers) -> Vec<u8> {
        key(&press(code, modifiers), false, KeyboardDialect::default()).unwrap()
    }

    fn dialect(bits: u8) -> KeyboardDialect {
        KeyboardDialect::from_bits(bits)
    }

    #[test]
    fn keys_type_what_an_xterm_types() {
        assert_eq!(bytes(KeyCode::Char('c'), KeyModifiers::CONTROL), b"\x03");
        assert_eq!(bytes(KeyCode::Char('C'), KeyModifiers::CONTROL | KeyModifiers::SHIFT), b"\x03");
        assert_eq!(bytes(KeyCode::Char(' '), KeyModifiers::CONTROL), b"\x00");
        assert_eq!(bytes(KeyCode::Char('b'), KeyModifiers::ALT), b"\x1bb");
        assert_eq!(bytes(KeyCode::Char('é'), KeyModifiers::NONE), "é".as_bytes());
        assert_eq!(bytes(KeyCode::Left, KeyModifiers::CONTROL), b"\x1b[1;5D");
        assert_eq!(bytes(KeyCode::Delete, KeyModifiers::NONE), b"\x1b[3~");
        assert_eq!(bytes(KeyCode::PageUp, KeyModifiers::SHIFT), b"\x1b[5;2~");
        assert_eq!(bytes(KeyCode::F(1), KeyModifiers::NONE), b"\x1bOP");
        assert_eq!(bytes(KeyCode::F(5), KeyModifiers::NONE), b"\x1b[15~");
        assert_eq!(bytes(KeyCode::F(12), KeyModifiers::ALT), b"\x1b[24;3~");
        assert_eq!(bytes(KeyCode::Enter, KeyModifiers::SHIFT), b"\x1b\r");
        assert_eq!(bytes(KeyCode::Backspace, KeyModifiers::ALT), b"\x1b\x7f");
        assert_eq!(key(&press(KeyCode::Up, KeyModifiers::NONE), true, KeyboardDialect::default()).unwrap(), b"\x1bOA");
    }

    #[test]
    fn pastes_are_bracketed_and_defanged() {
        assert_eq!(paste("a\nb", false), b"a\rb");
        assert_eq!(paste("x\x1b[201~y", true), b"\x1b[200~x[201~y\x1b[201~");
        // A CRLF paste is one Enter, not two: the carriage return is
        // consumed before a lone newline becomes one.
        assert_eq!(paste("a\r\nb", false), b"a\rb");
    }

    #[test]
    fn mouse_reports_follow_the_requested_encoding() {
        let event = |kind| MouseEvent { kind, column: 4, row: 2, modifiers: KeyModifiers::NONE };
        let down = event(MouseEventKind::Down(MouseButton::Left));
        let up = event(MouseEventKind::Up(MouseButton::Left));
        let sgr = MouseProtocolEncoding::Sgr;
        assert_eq!(mouse(&down, MouseProtocolMode::PressRelease, sgr).unwrap(), b"\x1b[<0;5;3M");
        assert_eq!(mouse(&up, MouseProtocolMode::PressRelease, sgr).unwrap(), b"\x1b[<0;5;3m");
        assert_eq!(
            mouse(&up, MouseProtocolMode::PressRelease, MouseProtocolEncoding::Default).unwrap(),
            b"\x1b[M#%#"
        );
        let moved = event(MouseEventKind::Moved);
        assert!(mouse(&moved, MouseProtocolMode::ButtonMotion, sgr).is_none());
        assert_eq!(mouse(&moved, MouseProtocolMode::AnyMotion, sgr).unwrap(), b"\x1b[<35;5;3M");
        assert_eq!(mouse(&event(MouseEventKind::ScrollUp), MouseProtocolMode::PressRelease, sgr).unwrap(), b"\x1b[<64;5;3M");
    }

    // The CSI u dialect, one rule at a time.

    fn in_dialect(code: KeyCode, modifiers: KeyModifiers, bits: u8) -> Vec<u8> {
        key(&press(code, modifiers), false, dialect(bits)).unwrap()
    }

    /// The functional keys come from the dialect's own table: arrows,
    /// Home, End and F1/F2/F4 as `CSI 1;mods letter` with nothing
    /// written when no modifier and no event type follows.
    #[test]
    fn the_functional_letters_are_the_dialects_own() {
        let d = dialect(1);
        assert_eq!(key(&press(KeyCode::Up, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[A");
        assert_eq!(key(&press(KeyCode::Down, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[B");
        assert_eq!(key(&press(KeyCode::Right, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[C");
        assert_eq!(key(&press(KeyCode::Left, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[D");
        assert_eq!(key(&press(KeyCode::Home, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[H");
        assert_eq!(key(&press(KeyCode::End, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[F");
        assert_eq!(key(&press(KeyCode::F(1), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[P");
        assert_eq!(key(&press(KeyCode::F(2), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[Q");
        assert_eq!(key(&press(KeyCode::F(4), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[S");
        // With a modifier the field is written; Up is not 57352.
        assert_eq!(key(&press(KeyCode::Up, KeyModifiers::CONTROL), false, d).unwrap(), b"\x1b[1;5A");
    }

    /// Insert, Delete, PageUp, PageDown, F3 and F5 to F12 are
    /// `CSI n;mods ~` with the dialect's numbers; F3 is 13.
    #[test]
    fn the_tilde_keys_use_the_dialects_numbers() {
        let d = dialect(1);
        assert_eq!(key(&press(KeyCode::Insert, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[2~");
        assert_eq!(key(&press(KeyCode::Delete, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[3~");
        assert_eq!(key(&press(KeyCode::PageUp, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[5~");
        assert_eq!(key(&press(KeyCode::PageDown, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[6~");
        assert_eq!(key(&press(KeyCode::F(3), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[13~");
        assert_eq!(key(&press(KeyCode::F(5), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[15~");
        assert_eq!(key(&press(KeyCode::F(12), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[24~");
        // PageUp is 5, not 57358 (CAPS_LOCK).
        assert_eq!(key(&press(KeyCode::PageUp, KeyModifiers::SHIFT), false, d).unwrap(), b"\x1b[5;2~");
    }

    /// CapsLock 57358, ScrollLock 57359, NumLock 57360, PrintScreen
    /// 57361, Pause 57362, Menu 57363, F13 to F35 from 57376 up, as
    /// `CSI n;mods u`. With any flag set the SS3 forms are never used.
    #[test]
    fn the_locks_pads_and_high_function_keys_are_codes() {
        let d = dialect(1);
        assert_eq!(key(&press(KeyCode::CapsLock, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57358u");
        assert_eq!(key(&press(KeyCode::ScrollLock, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57359u");
        assert_eq!(key(&press(KeyCode::NumLock, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57360u");
        assert_eq!(key(&press(KeyCode::PrintScreen, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57361u");
        assert_eq!(key(&press(KeyCode::Pause, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57362u");
        assert_eq!(key(&press(KeyCode::Menu, KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57363u");
        assert_eq!(key(&press(KeyCode::F(13), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57376u");
        assert_eq!(key(&press(KeyCode::F(20), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57383u");
        assert_eq!(key(&press(KeyCode::F(35), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[57398u");
        // F1 under disambiguation is `CSI 1 P`, never SS3.
        assert_eq!(key(&press(KeyCode::F(1), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[P");
    }

    /// Under disambiguation alone, Enter, Tab and Backspace with no
    /// modifiers stay their legacy bytes; only under "all keys" do they
    /// become codes. With modifiers they are codes under
    /// disambiguation.
    #[test]
    fn enter_tab_and_backspace_keep_their_bytes_until_asked() {
        let d = dialect(1);
        assert_eq!(key(&press(KeyCode::Enter, KeyModifiers::NONE), false, d).unwrap(), b"\r");
        assert_eq!(key(&press(KeyCode::Tab, KeyModifiers::NONE), false, d).unwrap(), b"\t");
        assert_eq!(key(&press(KeyCode::Backspace, KeyModifiers::NONE), false, d).unwrap(), b"\x7f");
        // Shift+Enter is a code under disambiguation.
        assert_eq!(in_dialect(KeyCode::Enter, KeyModifiers::SHIFT, 1), b"\x1b[13;2u");
        let all = dialect(1 | 8);
        assert_eq!(key(&press(KeyCode::Enter, KeyModifiers::NONE), false, all).unwrap(), b"\x1b[13u");
        assert_eq!(key(&press(KeyCode::Tab, KeyModifiers::NONE), false, all).unwrap(), b"\x1b[9u");
        assert_eq!(key(&press(KeyCode::Backspace, KeyModifiers::NONE), false, all).unwrap(), b"\x1b[127u");
    }

    /// Releases and repeats are reported only for keys sent as escape
    /// codes. A key that goes out as text (a plain text key without
    /// "all keys") or as a legacy byte (plain Enter, Tab, Backspace
    /// without it) has no release; the encoder returns nothing for it.
    #[test]
    fn a_key_sent_as_text_has_no_release() {
        let d = dialect(1 | 2);
        // A plain text key: press is text, release is nothing at all —
        // not the character again.
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Press), false, d).unwrap(), b"a");
        assert!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Release), false, d).is_none());
        // A repeat types the text again.
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Repeat), false, d).unwrap(), b"a");
        // Shift+a types `A`, as text, and its release is nothing.
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, d).unwrap(), b"A");
        assert!(key(&kind(press(KeyCode::Char('A'), KeyModifiers::SHIFT), KeyEventKind::Release), false, d).is_none());
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, dialect(1)).unwrap(), b"A");
        // Without event types, a held key repeats as presses: text, the
        // legacy byte, or the code, each again.
        let only = dialect(1);
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Repeat), false, only).unwrap(), b"a");
        assert_eq!(key(&kind(press(KeyCode::Backspace, KeyModifiers::NONE), KeyEventKind::Repeat), false, only).unwrap(), b"\x7f");
        assert_eq!(key(&kind(press(KeyCode::Up, KeyModifiers::NONE), KeyEventKind::Repeat), false, only).unwrap(), b"\x1b[A");
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::CONTROL), KeyEventKind::Repeat), false, only).unwrap(), b"\x1b[97;5u");
        // Event types alone: a text key's release still types nothing.
        let events = dialect(2);
        assert_eq!(key(&press(KeyCode::Char('a'), KeyModifiers::NONE), false, events).unwrap(), b"a");
        assert!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Release), false, events).is_none());
        // Super with a letter types no text, so disambiguation makes it a code.
        assert_eq!(key(&press(KeyCode::Char('a'), KeyModifiers::SUPER), false, only).unwrap(), b"\x1b[97;9u");
        // Plain Enter keeps its byte, so its release is nothing too.
        assert_eq!(key(&kind(press(KeyCode::Enter, KeyModifiers::NONE), KeyEventKind::Press), false, d).unwrap(), b"\r");
        assert!(key(&kind(press(KeyCode::Enter, KeyModifiers::NONE), KeyEventKind::Release), false, d).is_none());
        // A key that is a code (Esc, arrows) does get its release.
        assert_eq!(key(&kind(press(KeyCode::Esc, KeyModifiers::NONE), KeyEventKind::Release), false, d).unwrap(), b"\x1b[27;1:3u");
        assert_eq!(key(&kind(press(KeyCode::Up, KeyModifiers::NONE), KeyEventKind::Repeat), false, d).unwrap(), b"\x1b[1;1:2A");
        // Under "all keys" the text key's release is a code.
        let all = dialect(1 | 2 | 8);
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Release), false, all).unwrap(), b"\x1b[97;1:3u");
    }

    /// The key code is always the unshifted codepoint. The window sends
    /// Shift+a as `Char('A')` with SHIFT held and Shift+Tab as
    /// `BackTab` with SHIFT held; symbols arrive with Shift already
    /// removed, as plain text. The shifted key is present only when
    /// Shift is held and the key has a distinct shifted form.
    #[test]
    fn the_key_code_is_the_unshifted_codepoint() {
        // Ctrl+Shift+a is 97, not 65, with the shifted key under
        // alternate keys.
        assert_eq!(
            in_dialect(KeyCode::Char('A'), KeyModifiers::CONTROL | KeyModifiers::SHIFT, 1),
            b"\x1b[97;6u"
        );
        assert_eq!(
            in_dialect(KeyCode::Char('A'), KeyModifiers::CONTROL | KeyModifiers::SHIFT, 1 | 4),
            b"\x1b[97:65;6u"
        );
        // Shift+Tab is 9;2, not BackTab's own number.
        assert_eq!(in_dialect(KeyCode::BackTab, KeyModifiers::SHIFT, 1), b"\x1b[9;2u");
        // A symbol arrives as plain text with no Shift: no shifted key.
        assert_eq!(
            in_dialect(KeyCode::Char('!'), KeyModifiers::NONE, 1 | 4 | 8),
            b"\x1b[33u"
        );
        // Shift held on a key with no distinct shifted form: no second
        // field either.
        assert_eq!(in_dialect(KeyCode::Char('A'), KeyModifiers::SHIFT, 1 | 4 | 8), b"\x1b[97:65;2u");
    }

    /// Modifiers are 1 + the bits: shift 1, alt 2, ctrl 4, super 8 (and
    /// the rest when the window reports them). Super counts in this
    /// dialect even though the legacy encoder ignores it.
    #[test]
    fn modifiers_carry_every_bit_the_dialect_defines() {
        assert_eq!(in_dialect(KeyCode::Esc, KeyModifiers::SHIFT, 1), b"\x1b[27;2u");
        assert_eq!(in_dialect(KeyCode::Esc, KeyModifiers::ALT, 1), b"\x1b[27;3u");
        assert_eq!(in_dialect(KeyCode::Esc, KeyModifiers::CONTROL, 1), b"\x1b[27;5u");
        assert_eq!(
            in_dialect(KeyCode::Esc, KeyModifiers::SUPER, 1),
            b"\x1b[27;9u"
        );
        assert_eq!(
            in_dialect(KeyCode::Esc, KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::SUPER, 1),
            b"\x1b[27;14u"
        );
        assert_eq!(
            in_dialect(KeyCode::Esc, KeyModifiers::HYPER, 1),
            b"\x1b[27;17u"
        );
        assert_eq!(
            in_dialect(KeyCode::Esc, KeyModifiers::META, 1),
            b"\x1b[27;33u"
        );
    }

    /// Associated text is defined only together with "all keys"; its
    /// field carries the codepoints of the text the key types (Shift+a
    /// types A, so 65). Event types use :1 press (may be omitted), :2
    /// repeat, :3 release, with the modifiers field written (as 1)
    /// whenever an event type or text follows it.
    #[test]
    fn associated_text_carries_what_the_key_types() {
        let d = dialect(1 | 8 | 16);
        // Plain a: code 97, modifiers 1 (written because text follows),
        // text 97.
        assert_eq!(key(&press(KeyCode::Char('a'), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[97;1;97u");
        // Shift+a types A: base 97, modifiers 2, text 65; the shifted
        // key rides along only under alternate keys.
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, d).unwrap(), b"\x1b[97;2;65u");
        assert_eq!(
            key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, dialect(1 | 4 | 8 | 16)).unwrap(),
            b"\x1b[97:65;2;65u"
        );
        // The shifted key rides along only when alternate keys are asked for; without it the base code alone.
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, dialect(1 | 8 | 16)).unwrap(), b"\x1b[97;2;65u");
        // With event types too, the event rides the modifiers field.
        let with_events = dialect(1 | 2 | 8 | 16);
        let mut ev = press(KeyCode::Char('a'), KeyModifiers::NONE);
        ev.kind = KeyEventKind::Press;
        assert_eq!(key(&ev, false, with_events).unwrap(), b"\x1b[97;1:1;97u");
        ev.kind = KeyEventKind::Repeat;
        assert_eq!(key(&ev, false, with_events).unwrap(), b"\x1b[97;1:2;97u");
        ev.kind = KeyEventKind::Release;
        assert_eq!(key(&ev, false, with_events).unwrap(), b"\x1b[97;1:3u");
        // A key that types no text (an arrow) has no text field.
        assert_eq!(key(&press(KeyCode::Up, KeyModifiers::NONE), false, with_events).unwrap(), b"\x1b[1;1:1A");
    }

    /// Modifier keys on their own are reported only under "all keys",
    /// and the window only emits them in game mode: it is fine to leave
    /// them unreported, and this says so.
    #[test]
    fn modifier_keys_report_only_under_all_keys() {
        let d = dialect(1);
        assert!(key(&press(KeyCode::Modifier(ModifierKeyCode::LeftShift), KeyModifiers::SHIFT), false, d).is_none());
        let all = dialect(1 | 8);
        assert_eq!(
            key(&press(KeyCode::Modifier(ModifierKeyCode::LeftShift), KeyModifiers::SHIFT), false, all).unwrap(),
            b"\x1b[57441;2u"
        );
    }

    /// The legacy dialect regressed nowhere: F1 to F4 without
    /// modifiers are SS3 whatever cursor mode says; with modifiers they
    /// are `CSI 1;mods P` and friends, never DCH.
    #[test]
    fn f1_to_f4_are_ss3_without_modifiers_whatever_the_cursor_mode() {
        assert_eq!(bytes(KeyCode::F(1), KeyModifiers::NONE), b"\x1bOP");
        assert_eq!(bytes(KeyCode::F(2), KeyModifiers::NONE), b"\x1bOQ");
        assert_eq!(bytes(KeyCode::F(3), KeyModifiers::NONE), b"\x1bOR");
        assert_eq!(bytes(KeyCode::F(4), KeyModifiers::NONE), b"\x1bOS");
        // Application cursor mode does not move them either.
        let with_app = |code| key(&press(code, KeyModifiers::NONE), true, KeyboardDialect::default()).unwrap();
        assert_eq!(with_app(KeyCode::F(1)), b"\x1bOP");
        assert_eq!(with_app(KeyCode::F(4)), b"\x1bOS");
        // With modifiers: the CSI form, which is not DCH.
        assert_eq!(bytes(KeyCode::F(1), KeyModifiers::SHIFT), b"\x1b[1;2P");
        assert_eq!(bytes(KeyCode::F(4), KeyModifiers::CONTROL), b"\x1b[1;5S");
    }

    /// F13 and above have no legacy bytes and return nothing; F12 is
    /// not 57376 mis-sent as `CSI 24~`.
    #[test]
    fn f13_and_above_have_no_legacy_bytes() {
        assert_eq!(bytes(KeyCode::F(12), KeyModifiers::NONE), b"\x1b[24~");
        assert!(key(&press(KeyCode::F(13), KeyModifiers::NONE), false, KeyboardDialect::default()).is_none());
        assert!(key(&press(KeyCode::F(35), KeyModifiers::NONE), false, KeyboardDialect::default()).is_none());
    }

    /// `KeyCode::Null` types NUL, as it always did.
    #[test]
    fn null_types_nothing_but_nul() {
        assert_eq!(bytes(KeyCode::Null, KeyModifiers::NONE), b"\x00");
    }
}
