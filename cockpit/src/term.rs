//! Terminal lifecycle: raw-mode/alt-screen/mouse-capture setup and
//! teardown, enter/restore escape sequences, pending-stdin flush, and
//! the panic hook that restores the host terminal. Split out of main.rs.

use super::*;

thread_local! {
    /// A panic inside a deliberately caught background boundary must not tear
    /// down the live terminal or print through the alternate screen. The flag
    /// is thread-local so unrelated fatal panics retain normal restoration.
    static CONTAINED_BACKGROUND_PANIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The live TUI terminal type. Like ratatui's `DefaultTerminal`
/// (`Terminal<CrosstermBackend<Stdout>>`) but renders through [`term_pipe::TeeOut`]
/// so the whole session can be mirrored to a backend sink (default: a transparent
/// stdout passthrough — see `term_pipe`).
pub(crate) type AngelTerminal = Terminal<CrosstermBackend<term_pipe::TeeOut>>;

/// Enter the TUI: raw mode + alternate screen + bracketed paste + focus-change
/// reporting, with app-owned mouse capture enabled by default.
/// `ANGEL_TUI_MOUSE=0` leaves pointer handling to the outer terminal for native
/// selection and scrollback. A panic hook is installed *first* so any panic (or
/// normal teardown) restores every mode.
pub(crate) fn init_terminal() -> std::io::Result<AngelTerminal> {
    set_terminal_panic_hook();
    enable_raw_mode()?;
    // From here on, any failure must undo what's already on — `?` returns the
    // error to `main` (it doesn't panic, so the panic hook won't fire), so each
    // fallible step self-cleans rather than stranding the terminal in raw /
    // alt-screen / mouse-capture state.
    let init = (|| {
        write_enter_sequences(
            &mut std::io::stdout(),
            crate::harness::env_flag("ANGEL_TUI_MOUSE", true),
        )?;
        // Render through the tee so the whole session can be mirrored to a sink
        // when ANGEL_TERM_PIPE is set (a transparent stdout passthrough otherwise).
        let backend = CrosstermBackend::new(term_pipe::tee_stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(terminal)
    })();
    if init.is_err() {
        restore_terminal();
    }
    init
}

/// Enter the alt-screen, optionally enable mouse capture, then enable bracketed
/// paste. Factored out so both policies and their ordering can be tested against
/// a buffer without mutating a real terminal.
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
            EnableFocusChange
        )
    } else {
        execute!(
            w,
            EnterAlternateScreen,
            EnableBracketedPaste,
            EnableFocusChange
        )
    }
}

/// Disable focus, paste, and mouse reporting *then* leave the alt-screen — the
/// inverse order of setup (factored out for the same unit-test reason).
pub(crate) fn write_restore_sequences<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    execute!(
        w,
        DisableFocusChange,
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    )
}

/// Emit the terminal's standard attention signal between frames. A plain BEL
/// is intentionally used instead of a vendor-specific desktop-notification OSC:
/// terminals can map it to sound, a visual bell, or their native notification
/// policy without the cockpit injecting text into the alternate-screen buffer.
pub(crate) fn write_attention_signal<W: std::io::Write>(w: &mut W) -> std::io::Result<()> {
    w.write_all(b"\x07")?;
    w.flush()
}

/// Best-effort teardown — the mirror of [`init_terminal`]. Order matters: drop
/// mouse capture and leave the alt-screen *before* disabling raw mode. Every
/// step is best-effort so a single failure can't strand the others. This runs
/// from BOTH the normal exit path and the panic hook, so an unclean exit can
/// never leave the host terminal in mouse-reporting / raw / alt-screen state.
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

/// Install a panic hook that restores the terminal (raw mode, alt screen, AND
/// mouse capture) before delegating to the previous hook. Without the
/// `DisableMouseCapture` here a panic would leave the user's terminal spewing
/// mouse escape codes.
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

/// Catch a background panic without letting the terminal panic hook dismantle
/// the still-live cockpit. Callers must convert the payload into an ordinary
/// typed failure; fatal or foreground work must continue to panic normally.
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
#[path = "../../tests/cockpit/app/term__tests.rs"]
mod tests;
