//! Pure text builders, actions/keybinds, key parsing, clipboard delivery.
use super::*;

// --- pure text builders for the ported Codex commands ----------------------

pub(crate) fn keymap_text() -> String {
    "keymap (defaults; /keymap overrides)\n  \
     Tab        next box (agent)\n  \
     ←/→        move through a draft · with empty composer, page transcript\n  \
     Home/End   draft bounds · with empty composer, transcript bounds\n  \
     ↑/↓        wrapped draft rows · with empty composer, focused-view rows\n  \
     ^G         toggle the shell pane\n  \
     F2-F6      core, agent, artifacts, graph, web modules\n  \
     F9/F10     model/thinking deck\n  \
     F4 Stage   map h/l or ←/→ selects · Enter explores · m toggles map/explore\n  \
     Explore    h/l/←/→ yaw · j/k/↑/↓ pitch · +/- lens · 0/r recenter · Esc back\n  \
     Enter      send · Esc/^C  interrupt the running turn\n  \
     ^W/^U/^K   cut word/before/after caret · ^Y restores the latest cut\n  \
     ^P/^N      recall older/newer text-only prompts; newest restores the draft\n  \
     ^R         reverse-search recent text-only prompts by the current draft\n  \
     Tab        after file-taking commands, complete a confined workspace path\n  \
     Shift+←/→  select prompt text · add Ctrl for words · ^Shift+A selects all\n  \
     Shift+Home/End extends selection to draft bounds\n  \
     ^Shift+C   copy selected prompt text (idle ^C also copies a selection)\n  \
     PgUp/PgDn  scroll · Home/End  jump\n  \
     drag       select + copy · Shift-drag selects inside mouse-aware shell apps\n  \
     native     ANGEL_TUI_MOUSE=0 restores terminal-owned selection/scrollback\n  \
     exit       close the cockpit"
        .to_string()
}

pub(crate) fn debug_config_text() -> String {
    const KEYS: &[&str] = &[
        "ANGEL_DRIVER",
        "ANGEL_OPENAI_MODEL",
        "ANGEL_BAG_PROBE",
        "ANGEL_BAG_PROBE_SECS",
        "ANGEL_TAILNET_RESOLVE",
        "ANGEL_FALLBACK",
        "ANGEL_SWARM_MAX",
        "ANGEL_CODE_MODE",
        "ANGEL_LSP",
        "ANGEL_TUI_ATTENTION",
        "ANGEL_TUI_MOUSE",
        "ANGEL_YOLO",
        "ANGEL_YOLO_SMART",
        "CODEX_HOME",
    ];
    let set: Vec<String> = KEYS
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| format!("  {k} = {v}")))
        .collect();
    if set.is_empty() {
        "config — no ANGEL_* overrides set; all defaults".to_string()
    } else {
        format!("config — env overrides in effect\n{}", set.join("\n"))
    }
}

pub(crate) fn sandbox_text(workspace: &Path) -> String {
    // Report the registry's ACTIVE workspace — where the tools actually operate —
    // not the process cwd (which can differ after `/cd`).
    let ws = workspace.display();
    let profile = crate::platform::authority_profile::active(false);
    format!(
        "{}\nworkspace: {ws}\n(workspace defaults to the launch cwd; change it with /cd <path>)",
        profile.text
    )
}

/// Byte offset of the `char_idx`-th char (or the string length past the end).
pub(crate) fn byte_of(s: &str, char_idx: usize) -> usize {
    if char_idx == s.len() {
        return s.len();
    }
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// vim `w`: start of the next word (char index).
pub(crate) fn next_word(s: &str, cursor: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut i = cursor.min(n);
    while i < n && !chars[i].is_whitespace() {
        i += 1;
    }
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// vim `b`: start of the previous word (char index).
pub(crate) fn prev_word(s: &str, cursor: usize) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut i = cursor.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// Composer indices remain scalar offsets for history/clipboard compatibility,
/// but every editable position is an extended-grapheme boundary.
pub(crate) fn grapheme_floor(s: &str, cursor: usize) -> usize {
    grapheme_boundary(s, cursor, false)
}

pub(crate) fn grapheme_ceil(s: &str, cursor: usize) -> usize {
    grapheme_boundary(s, cursor, true)
}

fn grapheme_boundary(s: &str, cursor: usize, ceil: bool) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    if cursor == 0 {
        return cursor;
    }
    if s.is_ascii() {
        let cursor = cursor.min(s.len());
        return if cursor > 0 && cursor < s.len() && &s.as_bytes()[cursor - 1..=cursor] == b"\r\n" {
            if ceil { cursor + 1 } else { cursor - 1 }
        } else {
            cursor
        };
    }
    let mut start = 0;
    for grapheme in s.graphemes(true) {
        let end = start + grapheme.chars().count();
        if cursor < end {
            return if ceil && cursor > start { end } else { start };
        }
        start = end;
    }
    start
}

pub(crate) fn previous_grapheme(s: &str, cursor: usize) -> usize {
    // Keep ordinary end-of-ASCII typing/backspace independent of draft size.
    if cursor == s.len() && s.as_bytes().last().is_some_and(u8::is_ascii) && !s.ends_with("\r\n") {
        return cursor.saturating_sub(1);
    }
    grapheme_floor(s, cursor.saturating_sub(1))
}

pub(crate) fn next_grapheme(s: &str, cursor: usize) -> usize {
    if cursor == s.len() {
        return cursor;
    }
    grapheme_ceil(s, cursor.saturating_add(1))
}

/// A remappable input action (see `/keymap`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    NextBox,
    PrevMode,
    NextMode,
    ToggleShell,
    Interrupt,
    Send,
    ScrollUp,
    ScrollDown,
    ScrollTop,
    ScrollBottom,
    OpenModel,
    OpenThinking,
}

/// A custom key → action binding set via `/keymap`.
pub(crate) struct Keybind {
    pub code: KeyCode,
    pub mods: KeyModifiers,
    pub action: Action,
}

pub(crate) fn action_name(a: Action) -> &'static str {
    match a {
        Action::NextBox => "next-box",
        Action::PrevMode => "prev-mode",
        Action::NextMode => "next-mode",
        Action::ToggleShell => "shell",
        Action::Interrupt => "interrupt",
        Action::Send => "send",
        Action::ScrollUp => "scroll-up",
        Action::ScrollDown => "scroll-down",
        Action::ScrollTop => "scroll-top",
        Action::ScrollBottom => "scroll-bottom",
        Action::OpenModel => "model",
        Action::OpenThinking => "thinking",
    }
}

pub(crate) fn parse_action(s: &str) -> Option<Action> {
    Some(match s.to_ascii_lowercase().replace('_', "-").as_str() {
        "next-box" | "box" | "next-agent" => Action::NextBox,
        "prev-mode" | "prev" => Action::PrevMode,
        "next-mode" | "mode" | "next" => Action::NextMode,
        "shell" => Action::ToggleShell,
        "interrupt" | "stop" | "cancel" => Action::Interrupt,
        "send" => Action::Send,
        "scroll-up" => Action::ScrollUp,
        "scroll-down" => Action::ScrollDown,
        "scroll-top" | "top" => Action::ScrollTop,
        "scroll-bottom" | "bottom" => Action::ScrollBottom,
        "model" | "models" | "brain" | "brain-route" => Action::OpenModel,
        "thinking" | "think" | "effort" | "reasoning" => Action::OpenThinking,
        _ => return None,
    })
}

pub(crate) fn parse_key(s: &str) -> Option<(KeyCode, KeyModifiers)> {
    let lower = s.trim().to_ascii_lowercase();
    let (mods, base) = if let Some(rest) = lower
        .strip_prefix("ctrl+")
        .or_else(|| lower.strip_prefix('^'))
    {
        (KeyModifiers::CONTROL, rest)
    } else {
        (KeyModifiers::NONE, lower.as_str())
    };
    let code = match base {
        "tab" => KeyCode::Tab,
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        f if f.starts_with('f') && f.len() > 1 => KeyCode::F(f[1..].parse().ok()?),
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next()?),
        _ => return None,
    };
    Some((code, mods))
}

pub(crate) fn fmt_key(code: KeyCode, mods: KeyModifiers) -> String {
    let base = match code {
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Char(c) => c.to_string(),
        other => format!("{other:?}"),
    };
    if mods.contains(KeyModifiers::CONTROL) {
        format!("^{base}")
    } else {
        base
    }
}

pub(crate) fn relentless_output_delivered(reply: &str) -> bool {
    let text = reply.trim();
    if text.is_empty() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    ![
        "stopped after",
        "tool loop hit",
        "turn hit the",
        "interrupted after",
        "worker vanished",
        "agent error",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Map a pet name to an ASCII sprite (or echo the literal for anything else).
pub(crate) fn pet_glyph(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "cat" => "(=^·^=)".to_string(),
        "dog" => "U・ᴥ・U".to_string(),
        "fish" => "><(((°>".to_string(),
        "owl" => "{O,o}".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn hooks_text() -> String {
    "hooks\n  the cockpit fires Pre/PostToolUse hooks configured via env\n  \
     PostToolUse sees: ANGEL_TOOL_NAME / _ARGS / _RESULT / _TIMEOUT\n  \
     (configured via env, not a runtime menu)"
        .to_string()
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardTransport {
    RequestSent,
    PayloadTooLarge { encoded_bytes: usize },
    WriteFailed,
}

pub(crate) fn clipboard_receipt(
    chars: usize,
    transport: ClipboardTransport,
    fallback: Result<&Path, &str>,
) -> String {
    let transport = match transport {
        ClipboardTransport::RequestSent => {
            "OSC-52 request sent (terminal policy decides clipboard access)".to_string()
        }
        ClipboardTransport::PayloadTooLarge { encoded_bytes } => format!(
            "OSC-52 skipped ({encoded_bytes} encoded bytes exceed {})",
            crate::ui::mouse::OSC52_MAX_PAYLOAD_BYTES
        ),
        ClipboardTransport::WriteFailed => "OSC-52 write failed".to_string(),
    };
    let fallback = match fallback {
        Ok(path) => format!("recoverable copy: {}", path.display()),
        Err(error) => format!("fallback write failed: {error}"),
    };
    format!("copy · {chars} chars · {transport} · {fallback}")
}

/// Request a clipboard update via OSC-52 and always mirror the selection to a
/// fallback file under `~/.angelX/`. Emitting OSC-52 cannot prove that the outer
/// terminal accepted it, so the receipt reports a request rather than claiming
/// clipboard success. The returned string is shown in the transcript.
pub(crate) fn deliver_to_clipboard(text: &str, file_name: &str) -> String {
    use std::io::Write;
    let chars = text.chars().count();

    // OSC-52 is best effort. Cap the actual base64 payload, and write between
    // draws so it cannot race the ratatui backend's own stdout.
    let transport = if !crate::ui::mouse::osc52_payload_fits(text) {
        ClipboardTransport::PayloadTooLarge {
            encoded_bytes: crate::ui::mouse::base64_clipboard_len(text).unwrap_or(usize::MAX),
        }
    } else {
        let seq = crate::ui::mouse::osc52_sequence(text);
        let mut out = std::io::stdout();
        if out
            .write_all(seq.as_bytes())
            .and_then(|()| out.flush())
            .is_ok()
        {
            ClipboardTransport::RequestSent
        } else {
            ClipboardTransport::WriteFailed
        }
    };

    // Always-on file fallback.
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::Path::new(&home).join(".angelX").join(file_name);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, text) {
        Ok(()) => clipboard_receipt(chars, transport, Ok(path.as_path())),
        Err(error) => {
            let error = error.to_string();
            clipboard_receipt(chars, transport, Err(&error))
        }
    }
}
