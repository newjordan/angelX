//! A headless throughput bench: the harness's workloads fed through the
//! reader's path (splitter, pump, engine) with no window, so the parse
//! path can be timed and profiled on its own. Ignored by default:
//!
//! `cargo test --release -p angel-glass-term speed -- --ignored --nocapture`
//!
//! `GLASS_BENCH_SCALE` scales the workloads (1.0 is the harness's own
//! sizes), `GLASS_BENCH_READ` is the read size the stream is cut into,
//! and `GLASS_BENCH_ONLY` names one workload.

use std::time::Instant;

use crate::engine::{Engine, HostInfo};
use crate::intake::{Intake, Splitter};
#[cfg(test)]
use crate::intake::Piece;
use crate::session::pump;

/// The harness's full-screen grid on a 1080p output.
pub(crate) const ROWS: usize = 39;
pub(crate) const COLS: usize = 154;

/// A small deterministic generator, so every run feeds the same bytes.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }
    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn printable() -> Vec<char> {
    (0x21u8..0x7f).map(char::from).collect()
}

fn build(target: usize, mut line: impl FnMut() -> Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(target + 4096);
    while out.len() < target {
        out.extend_from_slice(&line());
    }
    out
}

/// The workloads of the harness's `tput.py`, by name, at `scale` of its
/// sizes, for a `rows` by `cols` screen.
pub(crate) fn workloads(scale: f64, rows: usize, cols: usize) -> Vec<(&'static str, Vec<u8>)> {
    let size = |mb: usize| ((mb << 20) as f64 * scale) as usize;
    let mut rng = Rng::new(7);
    let print = printable();
    let mut out = Vec::new();

    let base: Vec<Vec<u8>> = (0..400)
        .map(|_| {
            let mut s: String = (0..cols - 1).map(|_| print[rng.below(print.len())]).collect();
            s.push_str("\r\n");
            s.into_bytes()
        })
        .collect();
    out.push(("ascii", build(size(24), || base[rng.below(base.len())].clone())));

    let uni = ["漢字", "テスト", "🙂", "é", "ñ", "Ж", "αβγ", "á", "→", "█", "─"];
    let mut pool: Vec<String> = uni.iter().map(|s| s.to_string()).collect();
    pool.extend(print[..20].iter().map(|c| c.to_string()));
    let ulines: Vec<Vec<u8>> = (0..400)
        .map(|_| {
            let mut s: String = (0..cols / 3).map(|_| pool[rng.below(pool.len())].as_str()).collect();
            s.push_str("\r\n");
            s.into_bytes()
        })
        .collect();
    out.push(("unicode", build(size(16), || ulines[rng.below(ulines.len())].clone())));

    let slines: Vec<Vec<u8>> = (0..200)
        .map(|_| {
            let mut s = String::new();
            for _ in 0..cols - 1 {
                s.push_str(&format!(
                    "\x1b[38;2;{};{};{}m{}",
                    rng.below(256),
                    rng.below(256),
                    rng.below(256),
                    print[rng.below(print.len())]
                ));
            }
            s.push_str("\x1b[0m\r\n");
            s.into_bytes()
        })
        .collect();
    out.push(("truecolor", build(size(24), || slines[rng.below(slines.len())].clone())));

    let cups: Vec<Vec<u8>> = (0..400)
        .map(|_| {
            let mut s = String::new();
            for _ in 0..50 {
                let c = print[rng.below(print.len())];
                s.push_str(&format!(
                    "\x1b[{};{}H\x1b[3{}m{}",
                    1 + rng.below(rows),
                    1 + rng.below(cols - 9),
                    rng.below(8),
                    c.to_string().repeat(6)
                ));
            }
            s.into_bytes()
        })
        .collect();
    out.push(("cursor_motion", build(size(16), || cups[rng.below(cups.len())].clone())));

    let mut region = format!("\x1b[2;{}r", rows - 2).into_bytes();
    region.extend(build(size(16), || base[rng.below(base.len())].clone()));
    region.extend_from_slice(b"\x1b[r");
    out.push(("scroll_region", region));

    let frames: Vec<Vec<u8>> = (0..4)
        .map(|_| {
            let mut s = String::new();
            for r in 0..rows {
                s.push_str(&format!("\x1b[{};1H", r + 1));
                for c in 0..cols {
                    s.push_str(&format!("\x1b[48;5;{}m{}", (r * 7 + c) % 256, print[(r + c) % 94]));
                }
            }
            s.into_bytes()
        })
        .collect();
    let nf = ((300.0 * scale) as usize).max(4);
    let mut tui = b"\x1b[?1049h".to_vec();
    for i in 0..nf {
        tui.extend_from_slice(&frames[i % 4]);
    }
    tui.extend_from_slice(b"\x1b[?1049l");
    out.push(("tui_frames", tui));
    out
}

pub(crate) fn engine(rows: usize, cols: usize) -> Engine {
    Engine::new(rows, cols, HostInfo { colors: [[0xd0; 3], [0, 0, 0], [0xff; 3]], cell_px: (10, 20) })
}

/// The reader thread's path for one stream, cut into reads of `read`
/// bytes: split, then pump. Returns the replies, so nothing is optimized
/// away.
pub(crate) fn feed(engine: &mut Engine, intake: &mut Intake, splitter: &mut Splitter, stream: &[u8], read: usize) -> usize {
    let mut replies = 0;
    let mut pieces = Vec::new();
    for chunk in stream.chunks(read.max(1)) {
        pieces.clear();
        splitter.split(chunk, &mut pieces);
        replies += pump(engine, intake, &pieces).replies.len();
    }
    replies
}

/// This thread's CPU time in seconds: steadier than the wall clock on a
/// machine other work shares.
fn cpu_seconds() -> f64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `t` is a valid timespec for the call to fill.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as f64 + t.tv_nsec as f64 * 1e-9
}

/// The stream written into a pseudo-terminal by another thread and read
/// back as the reader thread reads it: the pty's own read sizes and the
/// system calls are part of what is timed. Returns the reader's CPU
/// seconds.
fn feed_pty(engine: &mut Engine, intake: &mut Intake, splitter: &mut Splitter, stream: &[u8]) -> f64 {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    let (mut master, mut slave) = (0, 0);
    // SAFETY: openpty fills the two descriptors; the termios is a plain
    // struct cfmakeraw fills before it is applied.
    unsafe {
        assert_eq!(libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()), 0);
        let mut tio: libc::termios = std::mem::zeroed();
        libc::tcgetattr(slave, &mut tio);
        libc::cfmakeraw(&mut tio);
        libc::tcsetattr(slave, libc::TCSANOW, &tio);
    }
    // SAFETY: both descriptors were just opened and are owned here.
    let (reader, mut writer) = unsafe { (std::fs::File::from_raw_fd(master), std::fs::File::from_raw_fd(slave)) };
    let fd = reader.try_clone().ok().map(std::os::fd::OwnedFd::from);
    let mut reader: Box<dyn Read> = if env("GLASS_BENCH_AHEAD", 1) == 1 {
        Box::new(crate::read_ahead::ReadAhead::spawn(Box::new(reader), fd).unwrap())
    } else {
        Box::new(reader)
    };
    let data = stream.to_vec();
    let total = data.len();
    let writing = std::thread::spawn(move || {
        for chunk in data.chunks(1 << 16) {
            writer.write_all(chunk).unwrap();
        }
        writer
    });
    let mut buf = vec![0u8; 64 * 1024];
    let mut got = 0;
    let c = cpu_seconds();
    while got < total {
        let n = reader.read(&mut buf).unwrap();
        got += n;
        if env("GLASS_BENCH_READ_ONLY", 0) == 1 {
            continue;
        }
        let mut pieces = Vec::new();
        splitter.split(&buf[..n], &mut pieces);
        std::hint::black_box(pump(engine, intake, &pieces));
    }
    let used = cpu_seconds() - c;
    drop(writing.join());
    if env("GLASS_BENCH_READS", 0) == 1 {
        eprintln!("  {got} bytes in {:.1} us of reader CPU per 4 KiB", used * 1e6 / (got as f64 / 4096.0));
    }
    used
}

fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[test]
#[ignore]
fn speed() {
    let scale: f64 = env("GLASS_BENCH_SCALE", 0.25);
    let read: usize = env("GLASS_BENCH_READ", 4095);
    let rounds: usize = env("GLASS_BENCH_ROUNDS", 3);
    let only: String = env("GLASS_BENCH_ONLY", String::new());
    for (name, stream) in workloads(scale, ROWS, COLS) {
        if !only.is_empty() && !only.split(',').any(|o| o == name) {
            continue;
        }
        let mut best = f64::MAX;
        let mut best_cpu = f64::MAX;
        // One terminal for every round, as the harness runs each workload
        // three times in one window: after the first, the history is full
        // and scrolling recycles rows rather than allocating them.
        let (mut e, mut i, mut s) = (engine(ROWS, COLS), Intake::new(angel_glass::LAYER_PICTURES), Splitter::default());
        e.set_fast(env("GLASS_BENCH_SLOW", 0) == 0);
        for _ in 0..rounds {
            feed(&mut e, &mut i, &mut s, b"\x1b[0m\x1b[H\x1b[2J", read);
            let (t, c) = (Instant::now(), cpu_seconds());
            let used = if env("GLASS_BENCH_PTY", 0) == 1 {
                feed_pty(&mut e, &mut i, &mut s, &stream)
            } else {
                std::hint::black_box(feed(&mut e, &mut i, &mut s, &stream, read));
                cpu_seconds() - c
            };
            best = best.min(t.elapsed().as_secs_f64());
            best_cpu = best_cpu.min(used);
        }
        eprintln!(
            "{name:>14}: {:7.1} MB/s  {:7.1} MB/cpu-s  ({} bytes, read {read})",
            stream.len() as f64 / best / 1e6,
            stream.len() as f64 / best_cpu / 1e6,
            stream.len()
        );
    }
}

/// A random stream of what programs write: text, UTF-8, controls, CSI
/// and plain escapes, pass-through and lifted strings of every dialect,
/// torn and aborted sequences, and stray bytes.
pub(crate) fn random_stream(rng: &mut Rng, len: usize) -> Vec<u8> {
    const PARTS: &[&[u8]] = &[
        b"hello", b"world ", b"\r\n", b"\n", b"\r", b"\t", b"\x08", b"\x07", b"x", b"0123456789",
        "漢字".as_bytes(), "🙂".as_bytes(), "é".as_bytes(), b"\xe6\x97", b"\xff", b"\xc3",
        b"\x1b[31m", b"\x1b[0m", b"\x1b[38;2;10;20;30m", b"\x1b[48;5;200m", b"\x1b[1;4:3m",
        b"\x1b[H", b"\x1b[5;7H", b"\x1b[2J", b"\x1b[3J", b"\x1b[K", b"\x1b[2K", b"\x1b[1J",
        b"\x1b[3;8r", b"\x1b[r", b"\x1b[2S", b"\x1b[3T", b"\x1b[2L", b"\x1b[2M", b"\x1b[4@", b"\x1b[3P",
        b"\x1b[5X", b"\x1b[3b", b"\x1b[?25l", b"\x1b[?25h", b"\x1b[?7l", b"\x1b[?7h", b"\x1b[4h", b"\x1b[4l",
        b"\x1b[?1049h", b"\x1b[?1049l", b"\x1b[?47h", b"\x1b[?47l", b"\x1b[?2026h", b"\x1b[?2026l",
        b"\x1b[c", b"\x1b[0c", b"\x1b[>c", b"\x1b[>q", b"\x1b[6n", b"\x1b[5n", b"\x1b[16t", b"\x1b[18t",
        b"\x1b[?2026$p", b"\x1b[?1;1S", b"\x1b[?2;1S", b"\x1b[?25$p", b"\x1b[>1u", b"\x1b[?u", b"\x1b[<u",
        b"\x1bM", b"\x1bD", b"\x1bE", b"\x1b7", b"\x1b8", b"\x1b(0", b"\x1b(B", b"\x1b)0", b"\x0e", b"\x0f",
        b"\x1b#8", b"\x1b=", b"\x1b>", b"\x1bc", b"\x1b\x1b", b"\x1b", b"\x1b[", b"\x1b[1;", b"\x18", b"\x1a",
        b"\x1b]0;title\x07", b"\x1b]2;name\x1b\\", b"\x1b]8;;http://x\x1b\\", b"\x1b]8;;\x1b\\",
        b"\x1b]52;c;aGk=\x07", b"\x1b]11;?\x07", b"\x1b]1337;CurrentDir=/tmp\x07",
        b"\x1b]1337;File=inline=1;size=4:AAAA\x07", b"\x1b]133;A\x07",
        b"\x1b_Gi=31,s=1,v=1,a=T,f=24;AAAA\x1b\\", b"\x1b_Ga=d\x1b\\", b"\x1b_Gi=5,a=q;\x07",
        b"\x1b_glass;layer id=1\x1b\\", b"\x1b_other\x1b\\", b"\x1b_gla\x07",
        b"\x1bPq#0;2;0;0;0#0~~@@\x1b\\", b"\x1bP$qm\x1b\\", b"\x1bP+q6d74\x1b\\", b"\x1bP1$r0m\x1b\\",
        b"\x1bPtmux;\x1b\x1b[31mred\x1b\x1b[0m\x1b\\", b"\x1bPtmux;\x1b\x1b_Gi=9,a=q;\x1b\x1b\\\x1b\\",
        b"\x1bX sos \x1b\\", b"\x1b^ pm \x1b\\", b"\x1b[?1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17;18;19h",
        b"\x1b[58;2;1;2;3m", b"\x1b[59m", b"\x1b[4:3m", b"\x1b[7m", b"\x1b[24m", b"\x1b[3D", b"\x1b[2C", b"\x1b[G",
        b"\x1b[20G", b"\x1b[999C", b"\x1b[A", b"\x1b[B", b"\x1b[1;1H", b"\x1b[0K", b"\x1b[1K", b"\x1b[12X",
        "漢字漢字漢字漢字".as_bytes(), "\u{301}".as_bytes(), b"\x1bP0q\x1b\\", b"\x1b]8;id=1;file:///x\x07",
        b"\x1b]0;\xe6\x97\xa5\x07", b"\x1bP1;2|abc\x9c", b"\x9c", b"\xc2\x9b", b"\x1b[?2026hpart\x1b[?2026l",
    ];
    let mut out = Vec::with_capacity(len + 64);
    while out.len() < len {
        match rng.below(12) {
            0 => out.push(rng.below(256) as u8),
            // A CSI near and past the head cap, ended or not.
            2 => {
                out.extend_from_slice(b"\x1b[");
                out.extend((0..250 + rng.below(20)).map(|i| if i % 4 == 3 { b';' } else { b'1' }));
                if rng.below(2) == 0 {
                    out.push(b'm');
                }
            }
            1 => out.extend((0..rng.below(40)).map(|_| 0x20 + rng.below(95) as u8)),
            // A long run, as plain output is: across lines, wrapping.
            3 => out.extend((0..rng.below(400)).map(|_| 0x20 + rng.below(95) as u8)),
            // A long run of text past ASCII: wide, narrow, marks that join
            // the cell before, a C1 control, and ASCII between.
            4 => {
                const TEXT: [&str; 12] = ["漢", "字", "é", "Ж", "🙂", "\u{301}", "─", "\u{85}", "a", "b c", "\u{200b}", "\u{ff21}"];
                for _ in 0..rng.below(120) {
                    out.extend_from_slice(TEXT[rng.below(TEXT.len())].as_bytes());
                }
            }
            _ => out.extend_from_slice(PARTS[rng.below(PARTS.len())]),
        }
    }
    out
}

/// Cut `stream` at `cuts` random points.
pub(crate) fn random_cuts(rng: &mut Rng, stream: &[u8], cuts: usize) -> Vec<usize> {
    let mut at: Vec<usize> = (0..cuts).map(|_| rng.below(stream.len() + 1)).collect();
    at.push(0);
    at.push(stream.len());
    at.sort_unstable();
    at.dedup();
    at
}

/// The splitter's bulk path gives the pieces the byte-at-a-time steps
/// give, piece for piece, however the stream is cut into reads.
#[test]
fn splitter_bulk_matches_byte_steps() {
    let mut rng = Rng::new(11);
    for round in 0..600 {
        let len = 50 + rng.below(1500);
        let stream = random_stream(&mut rng, len);
        let cap = if round % 5 == 0 { 1 + rng.below(40) } else { crate::intake::BODY_CAP };
        let n = rng.below(30);
        let cuts = random_cuts(&mut rng, &stream, n);
        let (mut bulk, mut step) = (Splitter::with_body_cap(cap), Splitter::with_body_cap(cap));
        for w in cuts.windows(2) {
            let read = &stream[w[0]..w[1]];
            let (mut a, mut b) = (Vec::new(), Vec::new());
            bulk.split(read, &mut a);
            for byte in read.chunks(1) {
                step.split(byte, &mut b);
            }
            fn joined(pieces: Vec<Piece<'_>>) -> Vec<Piece<'static>> {
                let mut out: Vec<Piece<'static>> = Vec::new();
                for piece in pieces {
                    match piece {
                        Piece::Text(text) => match out.last_mut() {
                            Some(Piece::Text(last)) => last.to_mut().extend_from_slice(&text),
                            _ => out.push(Piece::Text(std::borrow::Cow::Owned(text.into_owned()))),
                        },
                        Piece::Media(media) => out.push(Piece::Media(media)),
                    }
                }
                out
            }
            assert_eq!(joined(a), joined(b), "round {round}, read {:?}", String::from_utf8_lossy(read));
        }
    }
}

/// While the store holds nothing that text could move, the reader feeds
/// a read's text pieces to the engine at once. That leaves the grid, the
/// cursor, the scroll count, the replies and the store exactly as
/// feeding them one at a time does, for random streams with pictures
/// coming and going between the text.
#[test]
fn a_feed_of_many_pieces_matches_one_at_a_time() {
    let mut rng = Rng::new(31);
    for round in 0..300 {
        let len = 200 + rng.below(3000);
        let stream = random_stream(&mut rng, len);
        let n = rng.below(20);
        let cuts = random_cuts(&mut rng, &stream, n);
        let (rows, cols) = [(6, 10), (24, 80), (3, 40)][round % 3];
        let run = |batched: bool| {
            let mut e = engine(rows, cols);
            let (mut i, mut s) = (Intake::new(angel_glass::LAYER_PICTURES), Splitter::default());
            i.set_cell((10, 20));
            let mut replies = Vec::new();
            let mut pieces = Vec::new();
            for w in cuts.windows(2) {
                pieces.clear();
                s.split(&stream[w[0]..w[1]], &mut pieces);
                if batched {
                    replies.extend(pump(&mut e, &mut i, &pieces).replies);
                } else {
                    for piece in &pieces {
                        replies.extend(pump(&mut e, &mut i, std::slice::from_ref(piece)).replies);
                    }
                }
            }
            // An open synchronized update holds bytes in the parser (a
            // UTF-8 character half read before it among them) until it
            // ends; the frame is held meanwhile. End it, then compare.
            e.settle();
            let view = crate::intake::View { top: e.top_line(), rows: rows as u16, alternate: e.alternate_screen() };
            let shown: Vec<(f32, f32, f32, f32)> =
                i.describe(view).placements.iter().map(|p| (p.row, p.column, p.width, p.height)).collect();
            (e.grid_state(), replies, e.alt_scrolled(), i.scrolled(), shown)
        };
        let (batched, single) = (run(true), run(false));
        assert!(batched.0.0 == single.0.0, "cells differ, round {round}");
        assert_eq!(batched.0.1, single.0.1, "cursor, round {round}");
        assert_eq!((batched.0.2, batched.0.3), (single.0.2, single.0.3), "history and count, round {round}");
        assert_eq!(batched.1, single.1, "replies, round {round}");
        assert_eq!((batched.2, batched.3), (single.2, single.3), "scroll counts, round {round}");
        assert_eq!(batched.4, single.4, "pictures shown, round {round}");
    }
}

/// Writing plain text runs straight into the grid leaves every cell of
/// history and screen, the cursor, the pen, the modes, the scroll count
/// and the replies exactly as the parser alone does, for random streams
/// cut into random reads, on small screens that wrap and scroll a lot.
#[test]
fn plain_runs_match_the_parser() {
    let mut rng = Rng::new(23);
    for round in 0..400 {
        let len = 200 + rng.below(4000);
        let stream = random_stream(&mut rng, len);
        let n = rng.below(20);
        let cuts = random_cuts(&mut rng, &stream, n);
        let (rows, cols) = [(6, 10), (24, 80), (3, 40), (39, 154)][round % 4];
        let run = |fast: bool| {
            let mut e = engine(rows, cols);
            e.set_fast(fast);
            let (mut i, mut s) = (Intake::new(angel_glass::LAYER_PICTURES), Splitter::default());
            let mut replies = Vec::new();
            let mut pieces = Vec::new();
            for w in cuts.windows(2) {
                pieces.clear();
                s.split(&stream[w[0]..w[1]], &mut pieces);
                replies.extend(pump(&mut e, &mut i, &pieces).replies);
            }
            (e.grid_state(), replies, e.alt_scrolled(), e.alternate_screen())
        };
        let (fast, slow) = (run(true), run(false));
        assert!(fast.0.0 == slow.0.0, "cells differ, round {round}: {:?}", String::from_utf8_lossy(&stream));
        assert_eq!(fast.0.1, slow.0.1, "cursor, round {round}");
        assert_eq!((fast.0.2, fast.0.3), (slow.0.2, slow.0.3), "history and count, round {round}");
        assert_eq!(fast.1, slow.1, "replies, round {round}");
        assert_eq!((fast.2, fast.3), (slow.2, slow.3), "alternate screen, round {round}");
    }
}

/// How long one draw holds the engine for the cells: `view::paint` of a
/// full harness-sized screen of coloured text. Ignored by default.
#[test]
#[ignore]
fn paint_speed() {
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    let mut e = engine(ROWS, COLS);
    let (mut i, mut s) = (Intake::new(angel_glass::LAYER_PICTURES), Splitter::default());
    let (_, frames) = workloads(0.01, ROWS, COLS).pop().unwrap();
    feed(&mut e, &mut i, &mut s, &frames[..frames.len() - 8], 4095);
    let area = Rect::new(0, 0, COLS as u16, ROWS as u16);
    let mut buf = Buffer::empty(area);
    let t = Instant::now();
    for _ in 0..500 {
        crate::view::paint(&mut buf, area, &mut e, None, None);
    }
    eprintln!("paint: {:.1} us per frame", t.elapsed().as_secs_f64() * 1e6 / 500.0);
}
