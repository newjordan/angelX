//! The system clipboard through the desktop's own tools.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

fn tool(paste: bool) -> (&'static str, &'static [&'static str]) {
    if cfg!(target_os = "macos") {
        (if paste { "pbpaste" } else { "pbcopy" }, &[])
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        if paste {
            ("wl-paste", &["--no-newline"])
        } else {
            ("wl-copy", &[])
        }
    } else if paste {
        ("xclip", &["-o", "-selection", "clipboard"])
    } else {
        ("xclip", &["-selection", "clipboard"])
    }
}

pub fn copy(text: &str) -> bool {
    let (program, args) = tool(false);
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    // The write runs on its own thread with a bounded wait for the
    // acknowledgement: a wedged clipboard tool with a full pipe must not
    // block the caller (the cockpit) on write_all forever.
    let text = text.to_owned();
    let (done, written_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let written = child
            .stdin
            .take()
            .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        let _ = done.send(written);
        let _ = child.wait();
    });
    written_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap_or(false)
}

/// The most a paste may carry (16 MiB): pasting more into a composer is
/// never useful, and reading it all first is unbounded memory on the
/// window thread's time.
const MAX_PASTE_BYTES: u64 = 16 * 1024 * 1024;

/// Read at most `cap` bytes; one byte more and the whole paste is refused.
fn read_capped<R: Read>(reader: R, cap: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .ok()
        .filter(|_| bytes.len() as u64 <= cap)
        .map(|_| bytes)
}

pub fn paste() -> Option<String> {
    let (program, args) = tool(true);
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // The reader runs beside the wait so a full pipe never stalls either,
    // and the wait is bounded: a stuck clipboard tool must not freeze the
    // window that asked for the paste.
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || read_capped(stdout, MAX_PASTE_BYTES));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    }?;
    let bytes = reader.join().ok()??;
    status
        .success()
        .then(|| String::from_utf8_lossy(&bytes).into_owned())
        .filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pastes_over_the_cap_are_refused_whole() {
        assert_eq!(read_capped(&b"hello"[..], 5), Some(b"hello".to_vec()));
        assert_eq!(read_capped(&b"hello!"[..], 5), None);
        assert_eq!(read_capped(&b""[..], 0), Some(Vec::new()));
    }
}
