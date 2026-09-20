//! Embedded full-access PTY shell pane.
//!
//! A real `$SHELL` runs in a pseudo-terminal (portable-pty); a background thread
//! feeds its output to a `vt100` parser, and tui-term renders that screen as a
//! ratatui widget. This is the HUMAN's full-access shell into Apollo — it is
//! deliberately NOT sandboxed (unlike the agent's landlock-confined `shell`
//! tool). The `vt100::Parser` sits behind a `Mutex`, so rendering is `&self`.

use crate::sandbox::process_owner::{ChildClaim, claim_spawn};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tui_term::widget::PseudoTerminal;

pub struct ShellPane {
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    size: (u16, u16), // (rows, cols)
    child: Arc<Mutex<PaneChild>>,
}

struct PaneChild {
    handle: Box<dyn Child + Send + Sync>,
    claim: Option<ChildClaim>,
}

impl ShellPane {
    /// Cheap capability probe: can this host allocate a pseudo-terminal?
    ///
    /// Some sandboxes and containers mount `devpts` with `ptmxmode=000`,
    /// which makes `openpty(3)` fail with `EACCES`. Call this before
    /// relying on [`ShellPane::spawn`] so callers (and tests) can degrade
    /// gracefully instead of treating a missing PTY as a bug.
    pub fn can_spawn() -> bool {
        native_pty_system()
            .openpty(PtySize {
                rows: 1,
                cols: 1,
                pixel_width: 0,
                pixel_height: 0,
            })
            .is_ok()
    }

    /// Spawn `$SHELL` (or bash) in a PTY of the given size.
    pub fn spawn(rows: u16, cols: u16) -> Result<Self, String> {
        Self::spawn_in(rows, cols, None)
    }

    /// Spawn `$SHELL` (or bash) in a PTY of the given size and optional cwd.
    pub fn spawn_in(rows: u16, cols: u16, cwd: Option<&Path>) -> Result<Self, String> {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("openpty: {e}"))?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
        let mut command = CommandBuilder::new(shell);
        if let Some(cwd) = cwd {
            command.cwd(cwd.as_os_str());
        }
        let (child, claim) = claim_spawn(
            || pair.slave.spawn_command(command),
            |child| child.process_id(),
        )
        .map_err(|e| format!("spawn shell: {e}"))?;
        let child = Arc::new(Mutex::new(PaneChild {
            handle: child,
            claim: Some(claim),
        }));
        drop(pair.slave); // close the slave fd so the reader sees EOF when the shell exits

        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("clone reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("take writer: {e}"))?;

        {
            let parser = Arc::clone(&parser);
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if let Ok(mut p) = parser.lock() {
                                p.process(&buf[..n]);
                            }
                        }
                    }
                }
            });
        }
        {
            let child = Arc::clone(&child);
            std::thread::spawn(move || {
                // A descendant can hold the PTY open after its shell exits,
                // so wait ownership must not depend on the output reader's
                // EOF. Never hold this lock across a blocking wait: teardown
                // can still stop the shell while this observer polls.
                loop {
                    let Ok(mut child) = child.lock() else {
                        break;
                    };
                    match child.handle.try_wait() {
                        Ok(Some(_)) => {
                            child.claim.take();
                            break;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(_) => break,
                        Ok(None) => {}
                    }
                    drop(child);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            });
        }

        Ok(Self {
            parser,
            writer,
            master: pair.master,
            size: (rows, cols),
            child,
        })
    }

    /// Feed input bytes to the shell.
    pub fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Resize the PTY + parser to match the pane (no-op if unchanged).
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        if (rows, cols) == self.size {
            return;
        }
        self.size = (rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        if let Ok(mut p) = self.parser.lock() {
            p.screen_mut().set_size(rows, cols);
        }
    }

    #[cfg(test)]
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// The xterm mouse-tracking mode + report encoding the program currently
    /// running in the PTY has requested (via DECSET 1000/1002/1003 + 1006). The
    /// cockpit uses this to decide whether to *forward* the mouse to the program
    /// (vim/htop/etc.) or to do its own selection over the shell pane.
    pub fn mouse_mode(&self) -> (crate::mouse::TrackMode, crate::mouse::ReportEncoding) {
        use vt100::{MouseProtocolEncoding as E, MouseProtocolMode as M};
        let Ok(parser) = self.parser.lock() else {
            return (
                crate::mouse::TrackMode::None,
                crate::mouse::ReportEncoding::Sgr,
            );
        };
        let screen = parser.screen();
        let mode = match screen.mouse_protocol_mode() {
            M::None => crate::mouse::TrackMode::None,
            M::Press => crate::mouse::TrackMode::Press,
            M::PressRelease => crate::mouse::TrackMode::PressRelease,
            M::ButtonMotion => crate::mouse::TrackMode::ButtonMotion,
            M::AnyMotion => crate::mouse::TrackMode::AnyMotion,
        };
        let enc = match screen.mouse_protocol_encoding() {
            E::Sgr => crate::mouse::ReportEncoding::Sgr,
            _ => crate::mouse::ReportEncoding::X10,
        };
        (mode, enc)
    }

    /// Render the shell's current screen into `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if let Ok(parser) = self.parser.lock() {
            frame.render_widget(PseudoTerminal::new(parser.screen()), area);
        }
    }
}

impl Drop for ShellPane {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock()
            && child.claim.is_some()
        {
            let _ = child.handle.kill();
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "../../tests/cockpit/app/pty__process_ownership_tests.rs"]
mod process_ownership_tests;

/// Translate a crossterm key event into the bytes a terminal expects on stdin.
pub fn key_to_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let bytes = match key.code {
        KeyCode::Char(c) if ctrl => {
            let up = c.to_ascii_uppercase();
            if up.is_ascii_uppercase() {
                vec![(up as u8) - b'A' + 1] // Ctrl-A..Ctrl-Z -> 0x01..0x1A
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        _ => return None,
    };
    Some(bytes)
}
