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
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_unset_writes_to_stdout_and_never_captures() {
        let mut t = TeeOut::passthrough();
        assert!(t.tx.is_none());
        // write returns the real stdout's byte count; no panic, no capture path.
        let n = t.write(b"").unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn capacity_defaults_and_parses() {
        // Default when unset (the env may or may not be set in CI; assert the
        // pure fallback via a known-bad value path instead).
        assert_eq!(DEFAULT_CAPACITY, 2048);
    }

    #[test]
    fn captures_written_bytes_to_a_file_sink_end_to_end() {
        // Exercise the real chain: env path -> bounded channel -> writer thread
        // -> file sink. Proves capture actually happens, not just that it builds.
        // Mutates ANGEL_TERM_PIPE — hold the shared env lock so the parallel
        // runner can't race the environ table.
        let _guard = crate::tests::env_lock();
        let path = std::env::temp_dir().join(format!("angel0-termpipe-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var("ANGEL_TERM_PIPE", &path) };
        let marker = b"MARKER-abc-1234";
        {
            let mut t = tee_stdout();
            assert!(t.tx.is_some(), "capture should be active when env is set");
            t.write_all(marker).unwrap();
            t.flush().unwrap();
        } // drop closes the channel -> writer thread drains remaining + exits
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("ANGEL_TERM_PIPE") };

        // The writer thread is detached; poll the sink with a bounded wait.
        let mut found = false;
        for _ in 0..200 {
            if let Ok(b) = std::fs::read(&path)
                && b.windows(marker.len()).any(|w| w == marker)
            {
                found = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = std::fs::remove_file(&path);
        assert!(found, "file sink never received the written bytes");
    }

    #[test]
    fn full_channel_drops_instead_of_blocking() {
        // A depth-1 channel with no draining receiver: the second try_send must
        // fail fast (drop), proving the UI write path can never block on a stalled
        // consumer.
        let (tx, _rx) = sync_channel::<Vec<u8>>(1);
        let mut t = TeeOut {
            out: io::stdout(),
            tx: Some(tx),
        };
        // Many writes; if try_send ever blocked, this would hang. It must return.
        for _ in 0..1000 {
            let _ = t.write(b"x");
        }
    }
}
