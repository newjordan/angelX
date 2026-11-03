//! `glass`: a terminal in angelX's own window. A program (`$SHELL` by
//! default) runs in a pseudo-terminal, the engine keeps the screen its
//! output draws, and glass shows that screen as it shows the cockpit.
//!
//! Arguments follow the terminal convention `xdg-terminal-exec` speaks
//! (`glass.desktop`): `-e`/`--` and the command, `--title`, `--class`
//! (or `--app-id`), `--working-directory` (or `--dir`) and `--hold`.

#[path = "../../../lib/native/native_module.rs"]
mod native_module;

mod affinity;
mod blink;
mod engine;
mod intake;
mod keys;
mod read_ahead;
mod session;
#[cfg(test)]
mod speed;
mod view;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui_core::terminal::Terminal;

use keys::{KeyboardDialect, MouseProtocolMode};
use session::{Msg, Session};
use view::Selection;

/// While output keeps coming, the screen is drawn at most this often. A
/// draw holds the engine the reader parses into, so drawing after every
/// read starved the reader (a flood ran at a third of its speed), and the
/// displays show 60 frames a second anyway.
const OUTPUT_PACE: Duration = Duration::from_millis(8);

/// How long output after a quiet spell waits for the rest of its write,
/// so a program that sends one frame in pieces (a cursor move, then its
/// picture) is drawn once. Output answering a key does not wait.
const OUTPUT_GATHER: Duration = Duration::from_millis(1);

/// What the command line asked for.
#[derive(Default)]
struct Args {
    command: Vec<String>,
    title: Option<String>,
    class: Option<String>,
    dir: Option<PathBuf>,
    hold: bool,
}

const USAGE: &str = "usage: glass [--title T] [--class C] [--working-directory D] [--hold] [-e|--] [command [args...]]";

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        // `--flag=value` and `--flag value` both.
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let mut value = |name: &str| {
            inline
                .clone()
                .or_else(|| it.next())
                .ok_or_else(|| format!("{name} needs a value\n{USAGE}"))
        };
        match flag.as_str() {
            "-e" | "--" => {
                args.command = it.by_ref().collect();
                break;
            }
            "--title" | "-T" => args.title = Some(value("--title")?),
            "--class" | "--app-id" => args.class = Some(value("--class")?),
            "--working-directory" | "--dir" | "-d" => args.dir = Some(value("--working-directory")?.into()),
            "--hold" => args.hold = true,
            "-h" | "--help" => return Err(USAGE.to_string()),
            _ if arg.starts_with('-') => return Err(format!("unknown option {arg}\n{USAGE}")),
            // A bare command, as `glass htop` would mean it.
            _ => {
                args.command = std::iter::once(arg).chain(it.by_ref()).collect();
                break;
            }
        }
    }
    Ok(args)
}

/// Open a link the way the desktop opens them.
/// Open a link the program's output carried. Only `http`, `https`, `file`
/// and `mailto` URIs open: the URI came from program output, so anything
/// else is not something to hand to the desktop. The helper runs on its
/// own thread and is reaped there, so clicks do not leave zombies.
fn open(url: &str) {
    if !opens_outside(url) {
        return;
    }
    let url = url.to_owned();
    std::thread::spawn(move || {
        let _ = std::process::Command::new("xdg-open").arg(&url).status();
    });
}

/// Whether this URI is one the desktop is asked to open: `http`, `https`,
/// `file` and `mailto` only. The URI came from program output (OSC 8 or
/// plain text), so anything else stays unopened.
fn opens_outside(url: &str) -> bool {
    let scheme = url.split(':').next().unwrap_or_default().to_ascii_lowercase();
    matches!(scheme.as_str(), "http" | "https" | "file" | "mailto")
}

fn main() {
    angel_glass::startup::mark("main");
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let mut options = angel_glass::Options::from_environment();
    options.app_id = args.class.clone().unwrap_or_else(|| "glass".into());
    options.title = args.title.clone().unwrap_or_else(|| match args.command.first() {
        Some(program) => program.clone(),
        None => "glass".into(),
    });
    // A terminal opens on its prompt, not on the cockpit's assembly.
    // SAFETY: nothing else runs yet; the window and its threads start below.
    unsafe { std::env::set_var("ANGEL_GLASS_TRANSITION", "0") };
    let palette = options.palette.clone();
    angel_glass::startup::mark("options");
    match angel_glass::run(options, move |host| run(host, args, palette)) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("glass: {error}");
            std::process::exit(1);
        }
    }
}

/// The terminal's life, on the thread glass gives it: until the program
/// exits or the window closes. Returns the exit code.
fn run(host: angel_glass::Host, args: Args, palette: angel_glass::Palette) -> i32 {
    let grid = host.grid_size();
    let (tx, rx) = mpsc::channel();
    let focused = Arc::new(AtomicBool::new(true));
    let mut session = match Session::spawn(&args.command, args.dir.as_deref(), grid, &palette, host.clone(), tx.clone(), Arc::clone(&focused)) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("glass: {error}");
            return 1;
        }
    };
    let Ok(mut reader) = host.input_reader() else {
        return 1;
    };
    std::thread::spawn(move || {
        loop {
            match reader(Duration::from_secs(1)) {
                Ok(Some(event)) => {
                    if tx.send(Msg::Input(event)).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(_) => {
                    let _ = tx.send(Msg::Closed);
                    break;
                }
            }
        }
    });
    let Ok(mut terminal) = Terminal::new(host.backend()) else {
        return 1;
    };

    let mut ui = Ui::default();
    let mut exited: Option<i32> = None;
    let mut dirty = true;
    // Output not drawn yet, since when; the last draw; whether a key,
    // paste or click went to the program since, so its answer (an echo)
    // is drawn the moment it comes.
    let mut owed: Option<Duration> = None;
    let mut last_draw = Duration::ZERO;
    let mut interactive = false;
    let clock = Instant::now();
    let mut blink = blink::CursorBlink::new();
    let mut cursor_on = true;
    let mut display_frame = 0u64;
    loop {
        // Tweens and clips of glass's own dialect run at the display's rate.
        let frame = host.frame_interval().as_millis() as u64;
        if frame != display_frame {
            display_frame = frame;
            session.set_display_frame(frame);
        }
        // Synchronized output (DECSET 2026) holds the frame until the
        // program has finished it, for a moment at most. A blinking
        // cursor wakes at its next edge instead of the idle poll, and
        // only while it is shown and blinking.
        let now = clock.elapsed();
        let now_ms = now.as_millis() as u64;
        session.touch_animation_clock(now_ms);
        // A frame already due — the clock jumped, or a control arrived on a
        // clock this wait has passed — is applied before the wait. Otherwise
        // the deadline is in the past, the wait is zero, and the loop spins.
        if session.animation_deadline().is_some_and(|at| at <= now_ms) && session.tick_animations(now_ms) {
            dirty = true;
        }
        let anim_wait = session.animation_deadline().map(|at| Duration::from_millis(at.saturating_sub(now_ms)));
        let focus = Some(focused.load(Ordering::Relaxed));
        let blink_wait = blink.next_wake(now, session.cursor_blinking(), session.cursor_shown(), focus);
        // Owed output wakes the loop when it falls due. Once due and still
        // undrawn, a synchronized update holds it: the 20 ms tick below
        // comes back for it, rather than a zero wait spinning.
        let output_wait = owed.map(|since| output_due(since, last_draw)).filter(|at| *at > now).map(|at| at - now);
        let wait = if session.synchronized() {
            blink_wait
                .into_iter()
                .chain(anim_wait)
                .chain(output_wait)
                .min()
                .unwrap_or(Duration::from_millis(20))
                .min(Duration::from_millis(20))
        } else {
            blink_wait.into_iter().chain(anim_wait).chain(output_wait).min().unwrap_or(Duration::from_millis(250))
        };
        let first = match rx.recv_timeout(wait) {
            Ok(msg) => Some(msg),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        for msg in first.into_iter().chain(rx.try_iter()) {
            match msg {
                Msg::Output => {
                    let at = clock.elapsed();
                    owed.get_or_insert(at);
                    blink.activity(at);
                }
                Msg::Closed => return exited.unwrap_or(0),
                Msg::Input(event) => {
                    if exited.is_some() {
                        // `--hold`: the first key after the exit closes.
                        if matches!(event, Event::Key(k) if k.kind != KeyEventKind::Release) {
                            return exited.unwrap_or(0);
                        }
                        continue;
                    }
                    if matches!(event, Event::Key(_) | Event::Paste(_)) {
                        blink.activity(clock.elapsed());
                    }
                    // A pointer passing over is not waited on; the rest is.
                    interactive |= match &event {
                        Event::Key(_) | Event::Paste(_) => true,
                        Event::Mouse(mouse) => mouse.kind != MouseEventKind::Moved,
                        _ => false,
                    };
                    dirty |= ui.input(event, &mut session, &host, &focused);
                }
            }
        }
        // A synchronized update whose deadline passed ends here, before
        // the frame is drawn, so a program that opens one and stalls
        // cannot hold the screen: the tick already wakes every 20 ms
        // while one is open.
        if session.synchronized() && session.end_sync_update() {
            dirty = true;
        }
        if exited.is_none()
            && let Some(code) = session.exit_code()
        {
            if !args.hold {
                session.program_exited();
                return code;
            }
            session.feed(b"\r\n\x1b[2m[process exited; press a key to close]\x1b[0m");
            exited = Some(code);
            dirty = true;
        }
        let want_cursor = blink.visible(
            clock.elapsed(),
            session.cursor_blinking(),
            session.cursor_shown(),
            Some(focused.load(Ordering::Relaxed)),
        );
        if want_cursor != cursor_on {
            dirty = true;
            cursor_on = want_cursor;
        }
        if session.tick_animations(clock.elapsed().as_millis() as u64) {
            dirty = true;
        }
        let now = clock.elapsed();
        let output_ready = owed.is_some_and(|since| interactive || now >= output_due(since, last_draw));
        if (dirty || output_ready) && !session.holding_frame() {
            dirty = false;
            // A key's own redraw (a selection cleared) comes before its
            // echo; the echo is still owed the quick draw.
            if owed.take().is_some() {
                interactive = false;
            }
            last_draw = now;
            session.take_output_flag();
            draw(&mut terminal, &session, &ui, &host, cursor_on);
            session.frame_drawn();
        }
    }
    exited.unwrap_or(0)
}

/// When output that arrived at `since` is drawn, unless a key is waiting
/// on it: once the rest of its write has had a moment, and not sooner
/// than the pace after the last draw.
fn output_due(since: Duration, last_draw: Duration) -> Duration {
    (since + OUTPUT_GATHER).max(last_draw + OUTPUT_PACE)
}

/// One frame: the cursor's look (the shape and colour the program asked
/// for) goes to the backend first, then the cells are drawn.
fn draw(
    terminal: &mut Terminal<angel_glass::GlassBackend>,
    session: &Session,
    ui: &Ui,
    host: &angel_glass::Host,
    show_cursor: bool,
) {
    let style = angel_glass::CursorStyle {
        kind: match session.cursor_shape() {
            view::CursorLook::Block => angel_glass::CursorKind::Block,
            view::CursorLook::Underline => angel_glass::CursorKind::Underline,
            view::CursorLook::Beam => angel_glass::CursorKind::Bar,
            view::CursorLook::Hidden => angel_glass::CursorKind::Block,
        },
        color: session.cursor_color(),
    };
    terminal.backend_mut().set_cursor_style(style);
    // The underline is the word the link covers, so the hover already
    // holds what the frame needs.
    let hover = ui.hover_link;
    let _ = terminal.draw(|frame| {
        session.render(frame, ui.selection.as_ref(), hover.as_ref(), host, show_cursor)
    });
}

/// What the window does with the mouse and keys itself: selection and
/// copying, scrolling back, and handing the rest to the program.
#[derive(Default)]
struct Ui {
    selection: Option<Selection>,
    selecting: bool,
    last_click: Option<(Instant, u16, u16, u8)>,
    /// What a Ctrl+hover last underlined: the word the link covers.
    hover_link: Option<Selection>,
}

/// What one of the window's own shortcuts asks for.
#[derive(Debug, PartialEq, Eq)]
enum Shortcut {
    /// Not one of the window's shortcuts.
    Swallow,
    Copy,
    /// Page by this many lines, negative toward the live screen.
    Page(isize),
}

/// Whether the key is one of the window's own shortcuts, and what it asks
/// for. Releases of shortcuts are swallowed (`Swallow`): the press already
/// acted, and the release is never the program's news.
fn shortcut_of(key: &KeyEvent, rows: u16, alternate_screen: bool) -> Option<Shortcut> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    // Ctrl+Shift+C, and Ctrl+Insert (Omarchy's Super+C in a terminal).
    if (ctrl && shift && matches!(key.code, KeyCode::Char('c' | 'C')))
        || (ctrl && key.code == KeyCode::Insert)
    {
        return Some(match key.kind {
            KeyEventKind::Release => Shortcut::Swallow,
            _ => Shortcut::Copy,
        });
    }
    // Shift+PageUp/PageDown page through the scrollback, on the main
    // screen only: the alternate screen belongs to the program.
    if shift && !ctrl && !alternate_screen && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown)
    {
        let page = rows.saturating_sub(1).max(1) as isize;
        let lines = if key.code == KeyCode::PageUp { page } else { -page };
        return Some(match key.kind {
            KeyEventKind::Release => Shortcut::Swallow,
            _ => Shortcut::Page(lines),
        });
    }
    None
}

impl Ui {
    /// Returns whether the screen needs drawing again.
    fn input(&mut self, event: Event, session: &mut Session, host: &angel_glass::Host, focused: &AtomicBool) -> bool {
        match event {
            Event::Key(key) => {
                // The window's own shortcuts act on press and repeat only;
                // their releases are swallowed, never sent to the program.
                let rows = session.rows();
                let alternate = session.alternate_screen();
                if let Some(redraw) = self.shortcut(&key, rows, alternate, session, host) {
                    return redraw;
                }
                let dialect = KeyboardDialect::from_bits(session.keyboard_flags());
                // A release is only news when the program asked for event
                // types; the legacy dialect has no way to say it.
                if key.kind == KeyEventKind::Release && !dialect.event_types {
                    return false;
                }
                let Some(bytes) = keys::key(&key, session.application_cursor(), dialect) else {
                    return false;
                };
                let redraw = self.selection.take().is_some() | session.scroll_to_bottom();
                session.write(&bytes);
                redraw
            }
            Event::Paste(text) => {
                let redraw = self.selection.take().is_some() | session.scroll_to_bottom();
                let bytes = keys::paste(&text, session.bracketed_paste());
                session.write(&bytes);
                redraw
            }
            Event::Mouse(mouse) => self.mouse(mouse, session),
            Event::Resize(cols, rows) => {
                session.resize(rows, cols, host.grid_size());
                self.selection = None;
                true
            }
            Event::FocusGained | Event::FocusLost => {
                let gained = matches!(event, Event::FocusGained);
                focused.store(gained, Ordering::Relaxed);
                if session.focus_reports() {
                    session.write(if gained { b"\x1b[I" } else { b"\x1b[O" });
                }
                false
            }
        }
    }

    /// Act on the window's own shortcuts: copy (Ctrl+Shift+C,
    /// Ctrl+Insert) and paging through the scrollback
    /// (Shift+PageUp/PageDown). They act on press and repeat only; a
    /// release that matches one is swallowed rather than sent to the
    /// program, so one press pages once and copy runs once.
    fn shortcut(
        &mut self,
        key: &KeyEvent,
        rows: u16,
        alternate_screen: bool,
        session: &mut Session,
        host: &angel_glass::Host,
    ) -> Option<bool> {
        match shortcut_of(key, rows, alternate_screen) {
            None => None,
            // The release of a shortcut never reaches the program.
            Some(Shortcut::Swallow) => Some(false),
            Some(Shortcut::Copy) => {
                if let Some(selection) = &self.selection {
                    let text = session.selected_text(selection);
                    if !text.is_empty() {
                        host.copy_to_clipboard(&text);
                    }
                }
                Some(false)
            }
            Some(Shortcut::Page(lines)) => Some(self.scroll(session, lines)),
        }
    }

    /// Selections are in the cells shown, so moving the view drops one.
    fn scroll(&mut self, session: &mut Session, lines: isize) -> bool {
        let moved = session.scroll_back(lines);
        if moved {
            self.selection = None;
        }
        moved
    }

    /// What Ctrl+hover underlines, if anything. Returns whether it
    /// changed, so the hover itself redraws and the underline follows the
    /// mouse instead of waiting for some later frame.
    fn hover(&mut self, link: Option<&Selection>) -> bool {
        let changed = link != self.hover_link.as_ref();
        self.hover_link = link.copied();
        changed
    }

    fn mouse(&mut self, mouse: crossterm::event::MouseEvent, session: &mut Session) -> bool {
        let (mode, encoding) = session.mouse_protocol();
        // A program that asked for the mouse gets it; Shift keeps it for
        // selecting, as in other terminals.
        if mode != MouseProtocolMode::None && !mouse.modifiers.contains(KeyModifiers::SHIFT) {
            if let Some(bytes) = keys::mouse(&mouse, mode, encoding) {
                session.write(&bytes);
            }
            return false;
        }
        let (row, col) = (mouse.row, mouse.column);
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = mouse.kind == MouseEventKind::ScrollUp;
                if session.alternate_screen() {
                    // A full-screen program without the mouse (less, man)
                    // scrolls by its arrow keys.
                    let arrow = keys::arrow(up, session.application_cursor());
                    session.write(&arrow.repeat(3));
                    false
                } else {
                    self.scroll(session, if up { 3 } else { -3 })
                }
            }
            MouseEventKind::Down(MouseButton::Left)
                if mouse.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                // Ctrl+click opens a link (OSC 8 or a plain URL) where it
                // landed; a plain click still selects.
                let url = session.link_or_url_at(row, col);
                if let Some(url) = url {
                    open(&url);
                }
                true
            }
            MouseEventKind::Moved => {
                // Ctrl+hover points at a link, so the view underlines the
                // word it covers. What is underlined is what `draw`
                // underlines, so the two cannot drift apart.
                let link = if mouse.modifiers.contains(KeyModifiers::CONTROL) {
                    session.link_or_url_at(row, col).map(|_| session.word_at(row, col))
                } else {
                    None
                };
                self.hover(link.as_ref())
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let clicks = match self.last_click {
                    Some((at, r, c, n)) if at.elapsed() < Duration::from_millis(400) && (r, c) == (row, col) => n % 3 + 1,
                    _ => 1,
                };
                self.last_click = Some((Instant::now(), row, col, clicks));
                self.selection = Some(match clicks {
                    2 => session.word_at(row, col),
                    3 => Selection::line(row, session.cols()),
                    _ => Selection::at(row, col),
                });
                // A single click drags out a selection; double and triple
                // clicks take a word or a line as they are.
                self.selecting = clicks == 1;
                true
            }
            MouseEventKind::Drag(MouseButton::Left) if self.selecting => {
                if let Some(selection) = &mut self.selection {
                    selection.head = (row, col);
                }
                true
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.selecting = false;
                if self.selection.as_ref().is_some_and(Selection::is_empty) {
                    self.selection = None;
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode, modifiers: KeyModifiers, kind: KeyEventKind) -> KeyEvent {
        KeyEvent { code, modifiers, kind, state: KeyEventState::empty() }
    }

    #[test]
    fn window_shortcuts_act_on_press_and_repeat_only() {
        // Copy: the press acts, the release is swallowed, never sent.
        let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert!(matches!(
            shortcut_of(&key(KeyCode::Char('c'), ctrl_shift, KeyEventKind::Press), 24, false),
            Some(Shortcut::Copy)
        ));
        assert!(matches!(
            shortcut_of(&key(KeyCode::Char('c'), ctrl_shift, KeyEventKind::Repeat), 24, false),
            Some(Shortcut::Copy)
        ));
        assert!(matches!(
            shortcut_of(&key(KeyCode::Char('c'), ctrl_shift, KeyEventKind::Release), 24, false),
            Some(Shortcut::Swallow)
        ));
        // Ctrl+Insert is the same shortcut in another dress.
        assert!(matches!(
            shortcut_of(&key(KeyCode::Insert, KeyModifiers::CONTROL, KeyEventKind::Press), 24, false),
            Some(Shortcut::Copy)
        ));
        assert!(matches!(
            shortcut_of(&key(KeyCode::Insert, KeyModifiers::CONTROL, KeyEventKind::Release), 24, false),
            Some(Shortcut::Swallow)
        ));
        // Shift+PageUp pages one screen less a line, and only upward from
        // the press: the release does not page again.
        assert_eq!(
            shortcut_of(&key(KeyCode::PageUp, KeyModifiers::SHIFT, KeyEventKind::Press), 24, false),
            Some(Shortcut::Page(23))
        );
        assert_eq!(
            shortcut_of(&key(KeyCode::PageDown, KeyModifiers::SHIFT, KeyEventKind::Press), 24, false),
            Some(Shortcut::Page(-23))
        );
        assert!(matches!(
            shortcut_of(&key(KeyCode::PageUp, KeyModifiers::SHIFT, KeyEventKind::Release), 24, false),
            Some(Shortcut::Swallow)
        ));
        // On the alternate screen the scrollback is the program's.
        assert_eq!(
            shortcut_of(&key(KeyCode::PageUp, KeyModifiers::SHIFT, KeyEventKind::Press), 24, true),
            None
        );
        // Plain keys are not the window's.
        assert_eq!(shortcut_of(&key(KeyCode::Char('a'), KeyModifiers::NONE, KeyEventKind::Press), 24, false), None);
        assert_eq!(shortcut_of(&key(KeyCode::PageUp, KeyModifiers::NONE, KeyEventKind::Press), 24, false), None);
    }

    #[test]
    fn a_changed_hover_redraws() {
        let mut ui = Ui::default();
        let word = Selection::at(3, 5);
        // Nothing underlined to begin with: finding a link is news.
        assert!(ui.hover(Some(&word)));
        assert_eq!(ui.hover_link, Some(word));
        // The same word underlined again is not.
        assert!(!ui.hover(Some(&word)));
        // Another word is.
        let other = Selection::at(4, 0);
        assert!(ui.hover(Some(&other)));
        // Losing the link is news again.
        assert!(ui.hover(None));
        assert_eq!(ui.hover_link, None);
    }

    #[test]
    fn only_outside_uris_open() {
        for url in [
            "http://example.com",
            "https://example.com/a b",
            "file:///tmp/report.txt",
            "mailto:hi@example.com",
            "HTTP://EXAMPLE.COM",
        ] {
            assert!(opens_outside(url), "{url} should open");
        }
        // OSC 8 URIs come from program output; anything else stays put.
        for url in [
            "ftp://example.com",
            "ssh://root@example.com",
            "gopher://example.com",
            "javascript:alert(1)",
            "data:text/html,hi",
            "example.com/no-scheme",
            "",
        ] {
            assert!(!opens_outside(url), "{url} should not open");
        }
    }
}
