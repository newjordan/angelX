//! Terminal lifecycle: raw-mode/alt-screen/mouse-capture setup and
//! teardown, enter/restore escape sequences, pending-stdin flush, and
//! the panic hook that restores the host terminal. Split out of main.rs.

use crate::*;
use ratatui::crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};

thread_local! {
    static CONTAINED_BACKGROUND_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) type AngelTerminal = Terminal<CrosstermBackend<crate::ui::term::pipe::TeeOut>>;

pub(crate) fn init_terminal() -> std::io::Result<AngelTerminal> {
    set_terminal_panic_hook();
    enable_raw_mode()?;
    let init = (|| {
        write_enter_sequences(
            &mut std::io::stdout(),
            crate::agent::harness::env_flag("ANGEL_TUI_MOUSE", true),
        )?;
        let backend = CrosstermBackend::new(crate::ui::term::pipe::tee_stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(terminal)
    })();
    if init.is_err() {
        restore_terminal();
    }
    init
}

pub(crate) fn write_enter_sequences<W: std::io::Write>(
    w: &mut W,
    mouse_capture: bool,
) -> std::io::Result<()> {
    if mouse_capture {
        execute!(
            w,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
            EnableFocusChange,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )
    } else {
        execute!(
            w,
            EnterAlternateScreen,
            EnableBracketedPaste,
            EnableFocusChange,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )
    }
}

pub(crate) fn write_game_keyboard_mode<W: std::io::Write>(
    w: &mut W,
    active: bool,
) -> std::io::Result<()> {
    let mut flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
    if active {
        flags |= KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;
    }
    execute!(
        w,
        PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags(flags)
    )
}

pub(crate) fn write_restore_sequences<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    execute!(
        w,
        PopKeyboardEnhancementFlags,
        DisableFocusChange,
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    )
}

pub(crate) fn write_attention_signal<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    w.write_all(b"\x07")?;
    w.flush()
}

pub(crate) fn restore_terminal() {
    let _ = write_restore_sequences(&mut std::io::stdout());
    let _ = disable_raw_mode();
}

#[cfg(unix)]
pub(crate) fn flush_pending_stdin() {
    unsafe {
        let _ = libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH);
    }
}

#[cfg(not(unix))]
pub(crate) fn flush_pending_stdin() {}

pub(crate) fn set_terminal_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if contained_background_panic() {
            return;
        }
        flush_pending_stdin();
        restore_terminal();
        prev(info);
    }));
}

pub(crate) fn catch_background_unwind<F, T>(
    work: F,
) -> Result<T, Box<dyn std::any::Any + Send + 'static>>
where
    F: FnOnce() -> T,
{
    CONTAINED_BACKGROUND_PANIC.with(|flag| {
        let previous = flag.replace(true);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        flag.set(previous);
        outcome
    })
}

fn contained_background_panic() -> bool {
    CONTAINED_BACKGROUND_PANIC.with(std::cell::Cell::get)
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/term__tests.rs"]
mod tests;

pub(crate) mod art;
pub(crate) mod lookup;
pub(crate) mod pipe;
