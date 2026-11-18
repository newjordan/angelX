//! The window's keys, pastes and mouse as bytes a program takes in: the
//! legacy dialect by default, or the CSI u progressive keyboard dialect
//! when the program's flags ask for it.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode, MouseButton, MouseEvent, MouseEventKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseProtocolMode {
    None,
    PressRelease,
    ButtonMotion,
    AnyMotion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseProtocolEncoding {
    Default,
    Utf8,
    Sgr,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct KeyboardDialect {
    pub disambiguate: bool,
    pub event_types: bool,
    pub alternate_keys: bool,
    pub all_keys_as_escape: bool,
    pub associated_text: bool,
}

impl KeyboardDialect {
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

pub fn key(key: &KeyEvent, application_cursor: bool, dialect: KeyboardDialect) -> Option<Vec<u8>> {
    if dialect == KeyboardDialect::default() {
        return if key.kind == KeyEventKind::Release { None } else { legacy(key, application_cursor) };
    }
    let mut key = *key;
    if !dialect.event_types && key.kind == KeyEventKind::Repeat {
        key.kind = KeyEventKind::Press;
    }
    if let Some(bytes) = csi_u(&key, dialect) {
        return Some(bytes);
    }
    if key.kind == KeyEventKind::Release {
        return None;
    }
    legacy(&key, application_cursor)
}

#[derive(Clone, Copy)]
enum Functional {
    Letter(char),
    Tilde(u32),
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
        other => return functional(&other).is_none().then_some((0, None)).filter(|_| false),
    })
}

fn csi_u(key: &KeyEvent, dialect: KeyboardDialect) -> Option<Vec<u8>> {
    let mods = key.modifiers;
    let shift = mods.contains(KeyModifiers::SHIFT);
    let all = dialect.all_keys_as_escape;
    let plain = mods == KeyModifiers::NONE;

    if let KeyCode::Modifier(_) = key.code && !all {
        return None;
    }

    let func = functional(&key.code);

    let legacy_fallback = matches!(
        key.code,
        KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab | KeyCode::Backspace
    ) && !all;

    let text_fallback = matches!(key.code, KeyCode::Char(_))
        && mods.difference(KeyModifiers::SHIFT).is_empty()
        && !all;

    if key.kind == KeyEventKind::Press {
        if (legacy_fallback && plain) || text_fallback {
            return None;
        }
    } else {
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
    let write_params = param > 1 || event_type.is_some() || dialect.associated_text;

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

fn legacy(key: &KeyEvent, application_cursor: bool) -> Option<Vec<u8>> {
    let mods = key.modifiers;
    let alt = mods.contains(KeyModifiers::ALT);
    let ctrl = mods.contains(KeyModifiers::CONTROL);
    let shift = mods.contains(KeyModifiers::SHIFT);
    let param = modifier_param(mods, true);
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
        KeyCode::F(_) => return None,
        KeyCode::Menu => tilde(29),
        KeyCode::Null => vec![0],
        _ => return None,
    })
}

pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace('\x1b', "");
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

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
        MouseProtocolEncoding::Default | MouseProtocolEncoding::Utf8 => {
            let code = if release { 3 } else { code } + held;
            match encoding {
                MouseProtocolEncoding::Utf8 => encode_utf8_mouse(code, x, y),
                _ => encode_default_mouse(code, x, y),
            }
        }
    })
}

fn encode_default_mouse(code: u32, x: u32, y: u32) -> Vec<u8> {
    let byte = |v: u32| v.clamp(0, 223) as u8;
    vec![0x1b, b'[', b'M', 32 + byte(code), 32 + byte(x), 32 + byte(y)]
}

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

    fn in_dialect(code: KeyCode, modifiers: KeyModifiers, bits: u8) -> Vec<u8> {
        key(&press(code, modifiers), false, dialect(bits)).unwrap()
    }

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
        assert_eq!(key(&press(KeyCode::Up, KeyModifiers::CONTROL), false, d).unwrap(), b"\x1b[1;5A");
    }

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
        assert_eq!(key(&press(KeyCode::PageUp, KeyModifiers::SHIFT), false, d).unwrap(), b"\x1b[5;2~");
    }

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
        assert_eq!(key(&press(KeyCode::F(1), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[P");
    }

    #[test]
    fn enter_tab_and_backspace_keep_their_bytes_until_asked() {
        let d = dialect(1);
        assert_eq!(key(&press(KeyCode::Enter, KeyModifiers::NONE), false, d).unwrap(), b"\r");
        assert_eq!(key(&press(KeyCode::Tab, KeyModifiers::NONE), false, d).unwrap(), b"\t");
        assert_eq!(key(&press(KeyCode::Backspace, KeyModifiers::NONE), false, d).unwrap(), b"\x7f");
        assert_eq!(in_dialect(KeyCode::Enter, KeyModifiers::SHIFT, 1), b"\x1b[13;2u");
        let all = dialect(1 | 8);
        assert_eq!(key(&press(KeyCode::Enter, KeyModifiers::NONE), false, all).unwrap(), b"\x1b[13u");
        assert_eq!(key(&press(KeyCode::Tab, KeyModifiers::NONE), false, all).unwrap(), b"\x1b[9u");
        assert_eq!(key(&press(KeyCode::Backspace, KeyModifiers::NONE), false, all).unwrap(), b"\x1b[127u");
    }

    #[test]
    fn a_key_sent_as_text_has_no_release() {
        let d = dialect(1 | 2);
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Press), false, d).unwrap(), b"a");
        assert!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Release), false, d).is_none());
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Repeat), false, d).unwrap(), b"a");
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, d).unwrap(), b"A");
        assert!(key(&kind(press(KeyCode::Char('A'), KeyModifiers::SHIFT), KeyEventKind::Release), false, d).is_none());
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, dialect(1)).unwrap(), b"A");
        let only = dialect(1);
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Repeat), false, only).unwrap(), b"a");
        assert_eq!(key(&kind(press(KeyCode::Backspace, KeyModifiers::NONE), KeyEventKind::Repeat), false, only).unwrap(), b"\x7f");
        assert_eq!(key(&kind(press(KeyCode::Up, KeyModifiers::NONE), KeyEventKind::Repeat), false, only).unwrap(), b"\x1b[A");
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::CONTROL), KeyEventKind::Repeat), false, only).unwrap(), b"\x1b[97;5u");
        let events = dialect(2);
        assert_eq!(key(&press(KeyCode::Char('a'), KeyModifiers::NONE), false, events).unwrap(), b"a");
        assert!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Release), false, events).is_none());
        assert_eq!(key(&press(KeyCode::Char('a'), KeyModifiers::SUPER), false, only).unwrap(), b"\x1b[97;9u");
        assert_eq!(key(&kind(press(KeyCode::Enter, KeyModifiers::NONE), KeyEventKind::Press), false, d).unwrap(), b"\r");
        assert!(key(&kind(press(KeyCode::Enter, KeyModifiers::NONE), KeyEventKind::Release), false, d).is_none());
        assert_eq!(key(&kind(press(KeyCode::Esc, KeyModifiers::NONE), KeyEventKind::Release), false, d).unwrap(), b"\x1b[27;1:3u");
        assert_eq!(key(&kind(press(KeyCode::Up, KeyModifiers::NONE), KeyEventKind::Repeat), false, d).unwrap(), b"\x1b[1;1:2A");
        let all = dialect(1 | 2 | 8);
        assert_eq!(key(&kind(press(KeyCode::Char('a'), KeyModifiers::NONE), KeyEventKind::Release), false, all).unwrap(), b"\x1b[97;1:3u");
    }

    #[test]
    fn the_key_code_is_the_unshifted_codepoint() {
        assert_eq!(
            in_dialect(KeyCode::Char('A'), KeyModifiers::CONTROL | KeyModifiers::SHIFT, 1),
            b"\x1b[97;6u"
        );
        assert_eq!(
            in_dialect(KeyCode::Char('A'), KeyModifiers::CONTROL | KeyModifiers::SHIFT, 1 | 4),
            b"\x1b[97:65;6u"
        );
        assert_eq!(in_dialect(KeyCode::BackTab, KeyModifiers::SHIFT, 1), b"\x1b[9;2u");
        assert_eq!(
            in_dialect(KeyCode::Char('!'), KeyModifiers::NONE, 1 | 4 | 8),
            b"\x1b[33u"
        );
        assert_eq!(in_dialect(KeyCode::Char('A'), KeyModifiers::SHIFT, 1 | 4 | 8), b"\x1b[97:65;2u");
    }

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

    #[test]
    fn associated_text_carries_what_the_key_types() {
        let d = dialect(1 | 8 | 16);
        assert_eq!(key(&press(KeyCode::Char('a'), KeyModifiers::NONE), false, d).unwrap(), b"\x1b[97;1;97u");
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, d).unwrap(), b"\x1b[97;2;65u");
        assert_eq!(
            key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, dialect(1 | 4 | 8 | 16)).unwrap(),
            b"\x1b[97:65;2;65u"
        );
        assert_eq!(key(&press(KeyCode::Char('A'), KeyModifiers::SHIFT), false, dialect(1 | 8 | 16)).unwrap(), b"\x1b[97;2;65u");
        let with_events = dialect(1 | 2 | 8 | 16);
        let mut ev = press(KeyCode::Char('a'), KeyModifiers::NONE);
        ev.kind = KeyEventKind::Press;
        assert_eq!(key(&ev, false, with_events).unwrap(), b"\x1b[97;1:1;97u");
        ev.kind = KeyEventKind::Repeat;
        assert_eq!(key(&ev, false, with_events).unwrap(), b"\x1b[97;1:2;97u");
        ev.kind = KeyEventKind::Release;
        assert_eq!(key(&ev, false, with_events).unwrap(), b"\x1b[97;1:3u");
        assert_eq!(key(&press(KeyCode::Up, KeyModifiers::NONE), false, with_events).unwrap(), b"\x1b[1;1:1A");
    }

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

    #[test]
    fn f1_to_f4_are_ss3_without_modifiers_whatever_the_cursor_mode() {
        assert_eq!(bytes(KeyCode::F(1), KeyModifiers::NONE), b"\x1bOP");
        assert_eq!(bytes(KeyCode::F(2), KeyModifiers::NONE), b"\x1bOQ");
        assert_eq!(bytes(KeyCode::F(3), KeyModifiers::NONE), b"\x1bOR");
        assert_eq!(bytes(KeyCode::F(4), KeyModifiers::NONE), b"\x1bOS");
        let with_app = |code| key(&press(code, KeyModifiers::NONE), true, KeyboardDialect::default()).unwrap();
        assert_eq!(with_app(KeyCode::F(1)), b"\x1bOP");
        assert_eq!(with_app(KeyCode::F(4)), b"\x1bOS");
        assert_eq!(bytes(KeyCode::F(1), KeyModifiers::SHIFT), b"\x1b[1;2P");
        assert_eq!(bytes(KeyCode::F(4), KeyModifiers::CONTROL), b"\x1b[1;5S");
    }

    #[test]
    fn f13_and_above_have_no_legacy_bytes() {
        assert_eq!(bytes(KeyCode::F(12), KeyModifiers::NONE), b"\x1b[24~");
        assert!(key(&press(KeyCode::F(13), KeyModifiers::NONE), false, KeyboardDialect::default()).is_none());
        assert!(key(&press(KeyCode::F(35), KeyModifiers::NONE), false, KeyboardDialect::default()).is_none());
    }

    #[test]
    fn null_types_nothing_but_nul() {
        assert_eq!(bytes(KeyCode::Null, KeyModifiers::NONE), b"\x00");
    }
}
