//! Whole-session terminal capture — a passive, one-way tee on the cockpit's
//! own output stream, with a hard non-interference guarantee.
//!
//! [`tee_stdout`] returns the `Write` the ratatui backend renders through. By
//! default (no env) it's a zero-overhead transparent passthrough to the real
//! stdout — byte-for-byte what the cockpit emits today. When `ANGEL_TERM_PIPE`
//! is set, every byte the TUI writes (the whole composed session: panels, the
//! embedded shell pane, everything ratatui draws) is ALSO mirrored to a backend
//! sink so a tmux session / agent / log on the other end can consume it.
//!
//! Why this and not tmux-as-a-wrapper: running the shell/cockpit *inside* tmux
//! inserts a terminal emulator in the live path and breaks current systems
//! (kitty graphics passthrough for inline image previews, DECSET mouse-mode
//! detection, direct PTY resize). This tee adds NO layer — it's a side-write on
//! the bytes the cockpit already produces, so the live terminal path is
//! untouched.
//!
//! Non-interference guarantee: the render loop only ever `try_send`s a copy into
//! a bounded channel and DROPS on a full/closed channel — it never blocks on the
//! sink. A dedicated writer thread owns the sink and may block (e.g. opening a
//! FIFO with no reader yet) without ever touching the UI.
//!
//! Boundary (honest): this captures the cockpit's rendered text/escape stream —
//! the whole TUI. It cannot capture pixels a host terminal itself adds outside
//! that byte stream.
//!
//! Sink selection (`ANGEL_TERM_PIPE=<path>`):
//!   * a path you `mkfifo` first  -> live pipe; a reader (tmux/agent) streams it.
//!   * any other path             -> a regular file, appended (session recording).
//!
//! `ANGEL_TERM_PIPE_CAP=<n>` overrides the channel depth (chunks; default 2048).

use std::ffi::OsString;
use std::io::{self, Write};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

const DEFAULT_CAPACITY: usize = 2048;

/// The cockpit's terminal output writer: always forwards to stdout; optionally
/// mirrors a copy to the capture channel.
pub struct TeeOut {
    out: io::Stdout,
    tx: Option<SyncSender<Vec<u8>>>,
}

impl TeeOut {
    fn passthrough() -> Self {
        TeeOut {
            out: io::stdout(),
            tx: None,
        }
    }
}

impl Write for TeeOut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // The real terminal write is authoritative — its result is returned
        // unchanged, so ratatui sees identical behavior to a bare Stdout.
        let n = self.out.write(buf)?;
        if let Some(tx) = &self.tx {
            // Best-effort, non-blocking. A full channel (slow/absent consumer)
            // or a dropped receiver means we discard this copy rather than ever
            // stall the render loop.
            let _ = tx.try_send(buf[..n].to_vec());
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// Build the terminal output writer, honoring `ANGEL_TERM_PIPE`. Unset/empty =>
/// transparent passthrough (no thread, no allocation, no capture).
pub fn tee_stdout() -> TeeOut {
    let Some(path) = std::env::var_os("ANGEL_TERM_PIPE").filter(|s| !s.is_empty()) else {
        return TeeOut::passthrough();
    };
    let shown = path.to_string_lossy().into_owned();
    let (tx, rx) = sync_channel::<Vec<u8>>(capacity());
    // The writer thread owns the sink. Opening a FIFO for write blocks until a
    // reader attaches — fine here; the UI keeps running and drops copies until
    // then. Detached: it ends when the sender (TeeOut) is dropped at shutdown.
    let _ = std::thread::Builder::new()
        .name("angel-term-pipe".into())
        .spawn(move || writer_loop(path, rx));
    eprintln!(
        "TERM_PIPE: mirroring the cockpit session to {shown} (best-effort; drops if the consumer stalls)"
    );
    TeeOut {
        out: io::stdout(),
        tx: Some(tx),
    }
}

fn capacity() -> usize {
    std::env::var("ANGEL_TERM_PIPE_CAP")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_CAPACITY)
}

fn writer_loop(path: OsString, rx: Receiver<Vec<u8>>) {
    // append+create: an existing FIFO opens for write (blocking until a reader);
    // a non-existent path becomes a regular file appended to. Either way, on any
    // failure we keep draining the channel so the sender side never backs up.
    let sink = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path);
    let mut sink: Box<dyn Write + Send> = match sink {
        Ok(f) => Box::new(f),
        Err(_) => {
            drain_and_discard(rx);
            return;
        }
    };
    for chunk in rx.iter() {
        if sink.write_all(&chunk).is_err() {
            drain_and_discard(rx);
            return;
        }
        let _ = sink.flush();
    }
}

/// Keep receiving (and dropping) until the sender disconnects, so a dead sink
/// can never cause `try_send` on the UI thread to see a full channel forever.
fn drain_and_discard(rx: Receiver<Vec<u8>>) {
    for _ in rx.iter() {}
}

#[cfg(test)]
#[path = "../../tests/cockpit/app/term_pipe__tests.rs"]
mod tests;
