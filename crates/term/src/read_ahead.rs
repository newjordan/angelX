//! The pty read on a thread of its own, ahead of the parsing.
//!
//! A read from the pty returns at most what its line discipline holds (4
//! KiB on Linux) and costs a few microseconds of system time each: as much
//! as parsing the bytes it brings. Reading on another thread lets the two
//! overlap. The reading thread keeps reading into one hand-off while more
//! is already waiting and there is room, so a flood moves in large pieces
//! and a single keystroke's echo is handed on at once.
//!
//! Only two hand-offs are ever in flight, so the program can run at most
//! that much further ahead of the screen than the pty itself lets it.

use std::io::{self, Read};
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::mpsc::{self, Receiver, Sender};

/// The most one hand-off holds.
const CHUNK: usize = 16 * 1024;
/// Hand-offs in flight: how far reading may run ahead of parsing.
const AHEAD: usize = 2;
/// The most one pty read returns: a hand-off keeps reading only while it
/// has room for a whole one.
const PTY_READ: usize = 4096;

/// The parsing side: reads come out of here, in order, exactly as the
/// pty gave them, and the end of the pty is the end of these.
pub struct ReadAhead {
    /// Hand-offs, each with how much of it was read.
    full: Receiver<(Vec<u8>, usize)>,
    empty: Sender<Vec<u8>>,
    current: Vec<u8>,
    end: usize,
    at: usize,
}

impl ReadAhead {
    /// Start reading `reader` on a thread of its own. `fd` (the pty's own
    /// descriptor, duplicated) says whether more is waiting; without it
    /// every read is handed on by itself.
    pub fn spawn(mut reader: Box<dyn Read + Send>, fd: Option<OwnedFd>) -> io::Result<Self> {
        let (full_tx, full) = mpsc::channel::<(Vec<u8>, usize)>();
        let (empty, empty_rx) = mpsc::channel::<Vec<u8>>();
        // Each hand-off is filled once and keeps its length, so a reused
        // one is read into as it is, not cleared again.
        for _ in 0..AHEAD {
            let _ = empty.send(vec![0; CHUNK]);
        }
        std::thread::Builder::new().name("pty-read".into()).spawn(move || {
            crate::affinity::place_hot_thread();
            while let Ok(mut buf) = empty_rx.recv() {
                let mut filled = 0;
                let mut ended = false;
                loop {
                    match reader.read(&mut buf[filled..]) {
                        Ok(0) => ended = true,
                        Ok(n) => filled += n,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => ended = true,
                    }
                    if ended || CHUNK - filled < PTY_READ || !waiting(fd.as_ref()) {
                        break;
                    }
                }
                if filled > 0 && full_tx.send((buf, filled)).is_err() {
                    return;
                }
                if ended {
                    return;
                }
            }
        })?;
        Ok(Self { full, empty, current: Vec::new(), end: 0, at: 0 })
    }
}

/// Whether the pty holds bytes not read yet.
fn waiting(fd: Option<&OwnedFd>) -> bool {
    let Some(fd) = fd else {
        return false;
    };
    let mut n: libc::c_int = 0;
    // SAFETY: FIONREAD writes one int through the pointer, which points
    // at `n`; the descriptor is owned and open.
    let r = unsafe { libc::ioctl(fd.as_raw_fd(), libc::FIONREAD, &mut n as *mut libc::c_int) };
    r == 0 && n > 0
}

impl Read for ReadAhead {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.at >= self.end {
            let spent = std::mem::take(&mut self.current);
            if !spent.is_empty() {
                let _ = self.empty.send(spent);
            }
            match self.full.recv() {
                Ok((buf, end)) => {
                    self.end = end.min(buf.len());
                    self.current = buf;
                    self.at = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = out.len().min(self.end - self.at);
        out[..n].copy_from_slice(&self.current[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::FromRawFd;

    /// A pty pair in raw mode, as the program's side and glass's.
    fn pty() -> (std::fs::File, std::fs::File) {
        let (mut master, mut slave) = (0, 0);
        // SAFETY: openpty fills the two descriptors; the termios is a
        // plain struct cfmakeraw fills before it is applied.
        unsafe {
            assert_eq!(libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()), 0);
            let mut tio: libc::termios = std::mem::zeroed();
            libc::tcgetattr(slave, &mut tio);
            libc::cfmakeraw(&mut tio);
            libc::tcsetattr(slave, libc::TCSANOW, &tio);
            (std::fs::File::from_raw_fd(master), std::fs::File::from_raw_fd(slave))
        }
    }

    /// Every byte the program writes comes out once, in order, however it
    /// is written, and the program's side closing ends the reads.
    #[test]
    fn reading_ahead_gives_every_byte_in_order_then_the_end() {
        let (master, mut slave) = pty();
        let fd = master.try_clone().ok().map(OwnedFd::from);
        let mut ahead = ReadAhead::spawn(Box::new(master), fd).unwrap();
        let data: Vec<u8> = (0..300_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        let sent = data.clone();
        let writer = std::thread::spawn(move || {
            let mut at = 0;
            let mut size = 1;
            while at < sent.len() {
                let end = (at + size).min(sent.len());
                slave.write_all(&sent[at..end]).unwrap();
                at = end;
                size = size * 7 % 9001 + 1;
            }
            // Keep the program's side open until everything is read.
            slave
        });
        let mut got = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        while got.len() < data.len() {
            let n = ahead.read(&mut buf).unwrap();
            assert!(n > 0, "the reads ended early at {}", got.len());
            got.extend_from_slice(&buf[..n]);
        }
        assert!(got == data, "the bytes came out changed");
        drop(writer.join().unwrap());
        assert_eq!(ahead.read(&mut buf).unwrap(), 0, "a closed pty ends the reads");
    }
}
