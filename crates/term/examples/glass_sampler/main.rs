//! A showcase of every graphic dialect glass expands.
//!
//! It sizes each picture from the window it probed: columns from `stty
//! size` or XTWINOPS 18, one cell's pixels from XTWINOPS 16 (10×20 if
//! that query is not answered). The main picture of a scene fills about
//! half the width and never passes the screen. One thread owns stdin;
//! the probe reads replies from that channel and never blocks past its
//! deadline.
//!
//! The glass dialect's part is the long one: lit solids orbiting the text,
//! a cloud of 720 lights that becomes five shapes, a tesseract, a glass
//! knot, an equaliser city and an orrery, each a 3D scene rendered by the
//! window at twice its size and shaded on the CPU (see `motion.rs`), a glTF
//! model made in code (`model.rs`) that glass animates and flies round by
//! itself, easing curves side by side, then all of it stacked at once.
//!
//! ```text
//! cargo run --example glass_sampler
//! cargo run --example glass_sampler -- --check
//! cargo run --example glass_sampler -- --scene knot,orrery --loop
//! cargo run --example glass_sampler -- --scene gltf,tweens --loop
//! ```
//!
//! `--check` asks glass to answer every op (an `r=` token on each
//! `glass;` op, and no `q=2` on APC graphics) and prints to stderr: ops
//! sent, OK, each error with its code, and the frame rate each animated
//! scene kept. `--scene a,b` plays only those scenes (`--list` names
//! them), and `--loop` plays the show until `q`.

mod glb;
mod model;
mod motion;
mod stills;

use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use motion::{hex, mix, neon, WHITE};

const HELLO: &[u8] = b"\x1b_hello\x1b\\";
const HOLD_TEXT: u64 = 2200;
const HOLD_CURSOR: u64 = 400;
const HOLD_SIXEL: u64 = 3000;
const HOLD_APC_UNDER: u64 = 1200;
const HOLD_APC_OVER: u64 = 1100;
const HOLD_APC_CROP: u64 = 1300;
const HOLD_PLACE: u64 = 1000;
const HOLD_SCROLL: u64 = 80;
const HOLD_SCROLL_REST: u64 = 500;
const HOLD_OSC: u64 = 3000;
const HOLD_ANIM: u64 = 4000;
const HOLD_PRIMS: u64 = 8000;
const HOLD_CLOUD: u64 = 12_500;
const HOLD_TESSERACT: u64 = 9000;
const HOLD_KNOT: u64 = 9000;
const HOLD_CITY: u64 = 9000;
const HOLD_ORRERY: u64 = 10_000;
const HOLD_CHART: u64 = 2000;
const HOLD_FRAME: u64 = 33;
const HOLD_CLOSE: u64 = 900;
const HOLD_FINALE: u64 = 12_000;
const HOLD_MODEL: u64 = 10_000;
const HOLD_TWEENS: u64 = 12_000;
/// The base64 a piece of a chunked asset carries.
const ASSET_PIECE: usize = 96 * 1024;
const SCROLL_LINES: u64 = 24;
const VIDEO_FRAMES: u64 = 90;
const ANIM_FRAMES: usize = 12;
const ANIM_GAP_MS: u32 = 80;
/// Frames a second the 3D scenes are sent at.
const SCENE_FPS: u32 = 60;
/// How many lights the cloud is made of.
const CLOUD_POINTS: usize = 720;
/// A scene is rendered at this many times the pixels it covers, so the
/// linear filter that draws it smaller smooths every edge.
const SUPERSAMPLE: f32 = 2.0;
/// The most pixels a scene side may be (`glass_scene`'s cap).
const SCENE_SIDE: u32 = 2048;
const FALLBACK_CELL_W: u32 = 10;
const FALLBACK_CELL_H: u32 = 20;
/// How long the probe waits for glass to answer. Checked on the channel,
/// never by blocking in a read.
const PROBE_MS: u64 = 600;

/// A frame payload is capped at 1 MiB (`glass_scene::SCENE_BYTES`), and
/// the check is on the bytes the op carries (base64 of raw RGBA) before
/// decode. 480×270 raw is 518_400 bytes; its base64 is 691_200, under
/// that cap. The showcase opens at this size and `fit` scales it up.
const FRAME_BYTES: u32 = 1 << 20;
/// Panel size shared by the showcase and every video frame.
const VIDEO_W: u32 = 480;
const VIDEO_H: u32 = 270;
const _: () = assert!(
    (VIDEO_W as usize) * (VIDEO_H as usize) * 4 <= FRAME_BYTES as usize
);

static CHECK: AtomicBool = AtomicBool::new(false);
static SHOWCASE_OPEN: AtomicBool = AtomicBool::new(false);
static NEXT_REPLY: AtomicU32 = AtomicU32::new(1);
static TALLY: Mutex<Tally> = Mutex::new(Tally::new());
/// Each animated scene's frames and seconds, for `--check`.
static PACE: Mutex<Vec<(&'static str, u32, f32)>> = Mutex::new(Vec::new());

struct Tally {
    sent: u32,
    ok: u32,
    errors: Vec<String>,
    pending: Vec<u8>,
}

impl Tally {
    const fn new() -> Self {
        Self {
            sent: 0,
            ok: 0,
            errors: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn note_bytes(&mut self, chunk: &[u8]) {
        self.pending.extend_from_slice(chunk);
        self.drain_replies();
        if self.pending.len() > (1 << 20) {
            let drop = self.pending.len() - (1 << 16);
            self.pending.drain(..drop);
        }
    }

    fn drain_replies(&mut self) {
        let mut bodies = Vec::new();
        let mut consumed = 0;
        let mut i = 0;
        while i + 1 < self.pending.len() {
            if self.pending[i] == 0x1b && self.pending[i + 1] == b'_' {
                match find_st(&self.pending[i + 2..]) {
                    Some(rel) => {
                        let start = i + 2;
                        let end = start + rel;
                        if end + 1 < self.pending.len()
                            && self.pending[end] == 0x1b
                            && self.pending[end + 1] == b'\\'
                        {
                            bodies.push(self.pending[start..end].to_vec());
                            i = end + 2;
                            consumed = i;
                            continue;
                        }
                    }
                    None => break,
                }
            }
            i += 1;
        }
        if consumed > 0 {
            self.pending.drain(..consumed);
        }
        for body in bodies {
            self.classify(&body);
        }
    }

    fn classify(&mut self, body: &[u8]) {
        let text = String::from_utf8_lossy(body);
        if let Some(rest) = text.strip_prefix("glass;") {
            let mut parts = rest.splitn(3, ';');
            let _token = parts.next();
            let status = parts.next().unwrap_or("");
            self.note_status(status);
        } else if let Some(rest) = text.strip_prefix("Gi=")
            && let Some((_, status)) = rest.split_once(';')
        {
            self.note_status(status);
        }
    }

    fn note_status(&mut self, status: &str) {
        if status == "OK" || status.starts_with("OK;") {
            self.ok += 1;
        } else if !status.is_empty() {
            let code = status.split(':').next().unwrap_or(status);
            let message = status.split_once(':').map(|(_, m)| m).unwrap_or("");
            if message.is_empty() {
                self.errors.push(code.to_string());
            } else {
                self.errors.push(format!("{code}:{message}"));
            }
        }
    }

    fn summary(&self) -> String {
        let errors = if self.errors.is_empty() {
            "none".to_string()
        } else {
            self.errors.join(", ")
        };
        format!(
            "glass_sampler --check: ops sent {}, OK {}, errors {}",
            self.sent, self.ok, errors
        )
    }
}

fn find_st(buf: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i] == 0x1b && buf[i + 1] == b'\\' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn checking() -> bool {
    CHECK.load(Ordering::Relaxed)
}

fn note_sent() {
    if let Ok(mut tally) = TALLY.lock() {
        tally.sent += 1;
    }
}

fn note_input(chunk: &[u8]) {
    if !checking() {
        return;
    }
    if let Ok(mut tally) = TALLY.lock() {
        tally.note_bytes(chunk);
    }
}

fn finish_check(rx: &Receiver<Vec<u8>>) {
    if !checking() {
        return;
    }
    let end = Instant::now() + Duration::from_millis(300);
    while Instant::now() < end {
        match rx.recv_timeout(Duration::from_millis(40)) {
            Ok(chunk) => note_input(&chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let line = TALLY
        .lock()
        .map(|tally| tally.summary())
        .unwrap_or_else(|_| "glass_sampler --check: ops sent 0, OK 0, errors tally poisoned".into());
    eprintln!("{line}");
    if let Ok(pace) = PACE.lock()
        && !pace.is_empty()
    {
        let rates: Vec<String> = pace
            .iter()
            .map(|(name, frames, secs)| format!("{name} {:.1}", *frames as f32 / secs.max(0.001)))
            .collect();
        eprintln!("glass_sampler --check: frames a second {}", rates.join(", "));
    }
}
/// Picture the chart panel shows. Uploaded with `picture`, opened with `chart;p=`.
const CHART_PICTURE: u32 = 11;

const CLOSING: &str = "every dialect, expanded into layers";

/// The window the probe measured. Pictures are sized from this, never
/// from a hardcoded thumbnail.
#[derive(Clone, Copy)]
struct Fit {
    rows: u16,
    cols: u16,
    cell_w: u32,
    cell_h: u32,
}

impl Fit {
    fn screen_w(self) -> u32 {
        u32::from(self.cols).saturating_mul(self.cell_w).max(1)
    }

    fn screen_h(self) -> u32 {
        u32::from(self.rows).saturating_mul(self.cell_h).max(1)
    }

    /// A main picture: `frac` of the screen width, held in 40–60%, never
    /// past the screen. `floor` applies only when it still fits.
    fn picture(self, frac: f32, aspect: f32, floor_w: u32, floor_h: u32) -> (u32, u32) {
        let sw = self.screen_w();
        let sh = self.screen_h();
        let frac = frac.clamp(0.40, 0.60);
        let mut w = (sw as f32 * frac).round() as u32;
        if floor_w > 0 && floor_w <= sw {
            w = w.max(floor_w);
        }
        w = w.min(sw).max(1);
        let mut h = (w as f32 * aspect).round() as u32;
        if floor_h > 0 && floor_h <= sh {
            h = h.max(floor_h);
        }
        h = h.min(sh).max(1);
        (w, h)
    }

    /// Cell box for a picture the terminal scales into cells. The width
    /// is `frac` of the columns, held in 40–60%, and it never passes the
    /// screen.
    fn cell_box(self, frac: f32, aspect: f32) -> (u32, u32) {
        let cols = u32::from(self.cols).max(1);
        let rows = u32::from(self.rows).max(1);
        let frac = frac.clamp(0.40, 0.60);
        let c = ((cols as f32) * frac).round() as u32;
        let c = c.clamp(1, cols);
        let mut r = ((c as f32) * aspect * self.cell_w.max(1) as f32 / self.cell_h.max(1) as f32)
            .round() as u32;
        r = r.clamp(1, rows);
        (c, r)
    }

    /// A window-pinned scene box at `x, y` (cells from the top left, 0
    /// based), `c` by `r` cells, and the pixels it is rendered at.
    fn stage(self, x: f32, y: f32, c: f32, r: f32) -> Stage3 {
        let pw = (c * self.cell_w as f32).max(1.0);
        let ph = (r * self.cell_h as f32).max(1.0);
        let k = (SCENE_SIDE as f32 / pw.max(ph)).min(SUPERSAMPLE);
        Stage3 {
            x,
            y,
            c,
            r,
            s: ((pw * k).round() as u32).clamp(1, SCENE_SIDE),
            v: ((ph * k).round() as u32).clamp(1, SCENE_SIDE),
        }
    }

    /// The biggest scene box of `aspect` (width over height, in pixels)
    /// within `rows` rows from `top` and `cols` columns, centred on column
    /// `cx`.
    fn stage_fit(self, cx: f32, top: f32, rows: f32, cols: f32, aspect: f32) -> Stage3 {
        let (cw, ch) = (self.cell_w.max(1) as f32, self.cell_h.max(1) as f32);
        let mut r = rows.max(1.0);
        let mut c = r * ch * aspect / cw;
        if c > cols {
            c = cols.max(1.0);
            r = c * cw / aspect / ch;
        }
        self.stage(cx - c / 2.0, top + (rows - r) / 2.0, c, r)
    }

    fn cols_f(self) -> f32 {
        f32::from(self.cols)
    }

    fn rows_f(self) -> f32 {
        f32::from(self.rows)
    }

    /// Cells as tall as `w` cells are wide, so a light or a solid is round.
    fn square(self, w: f32) -> f32 {
        w * self.cell_w.max(1) as f32 / self.cell_h.max(1) as f32
    }
}

/// Where a scene is placed and how big it is rendered.
#[derive(Clone, Copy, Debug)]
struct Stage3 {
    x: f32,
    y: f32,
    c: f32,
    r: f32,
    s: u32,
    v: u32,
}

struct Tty {
    saved: Option<std::process::Output>,
}

impl Tty {
    fn raw() -> Self {
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .ok();
        let _ = Command::new("stty")
            .args(["raw", "-echo"])
            .stdin(Stdio::inherit())
            .status();
        Self { saved }
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        clean_up();
        if let Some(saved) = self.saved.take() {
            let _ = Command::new("stty")
                .arg(String::from_utf8_lossy(&saved.stdout).trim())
                .stdin(Stdio::inherit())
                .status();
        }
    }
}

fn out(bytes: &[u8]) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(bytes);
    let _ = stdout.flush();
}

fn emit(bytes: &[u8]) {
    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(bytes);
    let _ = stdout.flush();
}

fn clean_up() {
    out(b"\x1b[0m\x1b[?25h\x1b[?1049l");
    apc("a=d,d=A,q=2", &[]);
    for id in [1, 2, 3, 4, 5, 9] {
        apc(&format!("a=d,i={id},q=2"), &[]);
    }
    close_showcase_if_open();
    glass(&remove_all_op());
    out(b"\n");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    CHECK.store(args.iter().any(|arg| arg == "--check"), Ordering::Relaxed);
    if args.iter().any(|arg| arg == "--list") {
        let names: Vec<&str> = SCENES.iter().map(|(name, _)| *name).collect();
        println!("{}", names.join(" "));
        return;
    }
    let picked = match picked_scenes(&args) {
        Ok(picked) => picked,
        Err(unknown) => {
            eprintln!("glass_sampler: no scene {unknown}; --list names them");
            std::process::exit(2);
        }
    };
    let looping = args.iter().any(|arg| arg == "--loop");
    let tty = Tty::raw();
    // Size the fallback before the reader owns stdin. After this, nothing
    // else reads stdin: the probe and the scenes take bytes from the channel.
    let (rows, cols) = stty_size();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || read_stdin(tx));
    emit(b"\x1b[?1049h\x1b[?25l");
    if let Some(fit) = probe(&rx, rows, cols) {
        loop {
            if show(&rx, fit, &picked) {
                break;
            }
            if !looping {
                hold(&rx, 400);
                break;
            }
        }
    } else {
        out(b"glass_sampler: not running inside glass; nothing drawn.\r\n");
    }
    drop(tty);
    finish_check(&rx);
}

/// The scenes `--scene a,b` names, in the show's order; every scene when
/// it is not given. `Err` carries a name no scene has.
fn picked_scenes(args: &[String]) -> Result<Vec<&'static str>, String> {
    let mut wanted: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(list) = arg.strip_prefix("--scene=") {
            wanted.extend(list.split(',').map(str::to_string));
        } else if arg == "--scene"
            && let Some(list) = args.get(i + 1)
        {
            wanted.extend(list.split(',').map(str::to_string));
            i += 1;
        }
        i += 1;
    }
    wanted.retain(|name| !name.is_empty());
    if let Some(unknown) = wanted.iter().find(|name| !SCENES.iter().any(|(n, _)| n == name)) {
        return Err(unknown.clone());
    }
    Ok(SCENES
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| wanted.is_empty() || wanted.iter().any(|w| w == name))
        .collect())
}

/// The only reader of stdin. Every byte is forwarded. A second read would
/// race this one and could swallow the probe's replies for good.
fn read_stdin(tx: mpsc::Sender<Vec<u8>>) {
    let mut stdin = io::stdin();
    let mut buf = [0u8; 256];
    loop {
        match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

fn stty_size() -> (u16, u16) {
    let Ok(output) = Command::new("stty").arg("size").output() else {
        return (24, 80);
    };
    let Ok(text) = String::from_utf8(output.stdout) else {
        return (24, 80);
    };
    let mut parts = text.split_whitespace();
    let rows = parts.next().and_then(|s| s.parse().ok()).unwrap_or(24).max(1);
    let cols = parts.next().and_then(|s| s.parse().ok()).unwrap_or(80).max(1);
    (rows, cols)
}

/// DA1, XTWINOPS 18 (rows and columns) and XTWINOPS 16 (one cell in
/// pixels). A size report or `hello` means glass. Cell size falls back
/// to 10×20 when 16 is not answered. Replies come from the stdin channel
/// with `recv_timeout`, so the deadline is a real bound: a second reader
/// cannot swallow the burst, and this function never reads stdin. `q` or
/// Ctrl+C leaves before any picture. A late reply or a focus report does not.
fn probe(rx: &Receiver<Vec<u8>>, fallback_rows: u16, fallback_cols: u16) -> Option<Fit> {
    emit(b"\x1b[c\x1b[>c\x1b[18t\x1b[16t");
    let deadline = Instant::now() + Duration::from_millis(PROBE_MS);
    let mut seen = Vec::new();
    let mut glass_at: Option<Instant> = None;
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let wait_until = match glass_at {
            Some(started) => deadline.min(started + Duration::from_millis(120)),
            None => deadline,
        };
        if now >= wait_until {
            break;
        }
        match rx.recv_timeout(wait_until.saturating_duration_since(now)) {
            Ok(chunk) => {
                if wants_quit(&chunk) {
                    return None;
                }
                seen.extend_from_slice(&chunk);
                if from_glass(&seen) {
                    let started = *glass_at.get_or_insert_with(Instant::now);
                    if cell_from(&seen).is_some() || started.elapsed() >= Duration::from_millis(120) {
                        return Some(fit_from(&seen, fallback_rows, fallback_cols));
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if !from_glass(&seen) {
        return None;
    }
    Some(fit_from(&seen, fallback_rows, fallback_cols))
}

fn fit_from(seen: &[u8], fallback_rows: u16, fallback_cols: u16) -> Fit {
    let (rows, cols) = area_from(seen).unwrap_or((fallback_rows, fallback_cols));
    let (cell_w, cell_h) = cell_from(seen).unwrap_or((FALLBACK_CELL_W, FALLBACK_CELL_H));
    Fit {
        rows: rows.max(1),
        cols: cols.max(1),
        cell_w,
        cell_h,
    }
}

fn from_glass(seen: &[u8]) -> bool {
    seen.windows(HELLO.len()).any(|w| w == HELLO)
        || seen.windows(5).any(|w| w == b"hello")
        || area_from(seen).is_some()
}

fn find_seq(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn area_from(seen: &[u8]) -> Option<(u16, u16)> {
    let start = find_seq(seen, b"\x1b[8;")?;
    let rest = &seen[start + 4..];
    let end = rest.iter().position(|b| *b == b't')?;
    let text = std::str::from_utf8(&rest[..end]).ok()?;
    let mut parts = text.split(';');
    let rows: u16 = parts.next()?.parse().ok()?;
    let cols: u16 = parts.next()?.parse().ok()?;
    if rows == 0 || cols == 0 {
        None
    } else {
        Some((rows, cols))
    }
}

/// XTWINOPS 16 answers `CSI 6 ; height ; width t`. A wild reply falls
/// back to 10×20 rather than sizing a picture off the screen.
fn cell_from(seen: &[u8]) -> Option<(u32, u32)> {
    let start = find_seq(seen, b"\x1b[6;")?;
    let rest = &seen[start + 4..];
    let end = rest.iter().position(|b| *b == b't')?;
    let text = std::str::from_utf8(&rest[..end]).ok()?;
    let mut parts = text.split(';');
    let h: u32 = parts.next()?.parse().ok()?;
    let w: u32 = parts.next()?.parse().ok()?;
    if (1..=64).contains(&w) && (1..=128).contains(&h) {
        Some((w, h))
    } else {
        None
    }
}

/// `q` or Ctrl+C, and only outside a control sequence. A late size reply,
/// a glass answer, or a focus report (`ESC [ I`, `ESC [ O`) is not a quit.
fn wants_quit(chunk: &[u8]) -> bool {
    let mut i = 0;
    while i < chunk.len() {
        match chunk[i] {
            3 => return true,
            0x1b => i = skip_control(chunk, i),
            b'q' => return true,
            _ => i += 1,
        }
    }
    false
}

fn skip_control(chunk: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    if i >= chunk.len() {
        return i;
    }
    match chunk[i] {
        b'[' => {
            i += 1;
            while i < chunk.len() && !is_csi_final(chunk[i]) {
                if chunk[i] == 3 {
                    return i;
                }
                i += 1;
            }
            if i < chunk.len() {
                i += 1;
            }
            i
        }
        b']' => skip_until_st(chunk, i + 1, true),
        b'_' | b'P' | b'X' | b'^' => skip_until_st(chunk, i + 1, false),
        _ => i + 1,
    }
}

fn is_csi_final(byte: u8) -> bool {
    (0x40..=0x7e).contains(&byte)
}

fn skip_until_st(chunk: &[u8], mut i: usize, bel: bool) -> usize {
    while i < chunk.len() {
        if chunk[i] == 3 {
            return i;
        }
        if chunk[i] == 0x1b && chunk.get(i + 1) == Some(&b'\\') {
            return i + 2;
        }
        if bel && chunk[i] == 0x07 {
            return i + 1;
        }
        i += 1;
    }
    i
}

fn quit(rx: &Receiver<Vec<u8>>) -> bool {
    let mut stop = false;
    while let Ok(chunk) = rx.try_recv() {
        note_input(&chunk);
        if wants_quit(&chunk) {
            stop = true;
        }
    }
    stop
}

fn hold(rx: &Receiver<Vec<u8>>, ms: u64) -> bool {
    hold_until(rx, Instant::now() + Duration::from_millis(ms))
}

fn hold_until(rx: &Receiver<Vec<u8>>, end: Instant) -> bool {
    loop {
        if quit(rx) {
            return true;
        }
        let now = Instant::now();
        if now >= end {
            return false;
        }
        thread::sleep((end - now).min(Duration::from_millis(5)));
    }
}

/// Runs `draw` with the scene's clock in seconds, `fps` times a second for
/// `ms`. Frames are paced against the clock, so a slow frame is not
/// followed by a full wait, and motion keeps time however fast glass
/// takes the bytes. True on a quit.
fn animate(rx: &Receiver<Vec<u8>>, name: &'static str, ms: u64, fps: u32, mut draw: impl FnMut(f32)) -> bool {
    let start = Instant::now();
    let length = Duration::from_millis(ms);
    let step = Duration::from_secs_f32(1.0 / fps.max(1) as f32);
    let mut next = start;
    let mut frames = 0u32;
    let mut stopped = false;
    while start.elapsed() < length {
        draw(start.elapsed().as_secs_f32());
        frames += 1;
        next += step;
        let now = Instant::now();
        if next < now {
            next = now;
        }
        if hold_until(rx, next.min(start + length)) {
            stopped = true;
            break;
        }
    }
    if let Ok(mut pace) = PACE.lock() {
        pace.push((name, frames, start.elapsed().as_secs_f32()));
    }
    stopped
}

type SceneFn = fn(&Receiver<Vec<u8>>, Fit) -> bool;

/// The show, in order. `--scene` picks from these names.
const SCENES: &[(&str, SceneFn)] = &[
    ("text", scene_text),
    ("sixel", scene_sixel),
    ("apc", scene_apc),
    ("placeholders", scene_placeholders),
    ("osc", scene_osc1337),
    ("animation", scene_animation),
    ("prims", scene_prims),
    ("cloud", scene_cloud),
    ("tesseract", scene_tesseract),
    ("knot", scene_knot),
    ("city", scene_city),
    ("orrery", scene_orrery),
    ("gltf", scene_gltf),
    ("tweens", scene_tweens),
    ("chart", scene_chart),
    ("video", scene_video),
    ("close", scene_close),
    ("finale", scene_finale),
];

fn show(rx: &Receiver<Vec<u8>>, fit: Fit, picked: &[&str]) -> bool {
    SCENES
        .iter()
        .filter(|(name, _)| picked.contains(name))
        .any(|(_, scene)| scene(rx, fit))
}

fn retire() {
    // A showcase closes only when one is open, and only as its scene
    // ends — the next scene's retire is that moment. Closing when none
    // is open makes the following chart open onto a refused panel.
    close_showcase_if_open();
    glass(&remove_all_op());
    apc("a=d,d=A,q=2", &[]);
    clear_screen();
}

fn scene_text(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    gradient_title(fit, "glass", 0.0);
    cup(4, 2);
    styled(b"\x1b[2m");
    out(b"every dialect a program speaks, expanded into layers");
    styled(b"\x1b[0m");
    let styles: &[(&str, &str, &[u8])] = &[
        ("bold", "BOLD", b"\x1b[1m"),
        ("dim", "dim", b"\x1b[2m"),
        ("italic", "italic", b"\x1b[3m"),
        ("under", "under", b"\x1b[4m"),
        ("strike", "strike", b"\x1b[9m"),
        ("reverse", "reverse", b"\x1b[7m"),
        ("curly", "curly", b"\x1b[38;2;120;220;255m\x1b[4:3m"),
        ("colour", "colour", b"\x1b[4m\x1b[58;2;255;96;32m"),
    ];
    let n = styles.len() as u16;
    let col_w = (fit.cols / n.max(1)).max(8);
    for (i, (label, sample, seq)) in styles.iter().enumerate() {
        let x = 2 + i as u16 * col_w;
        if x + 6 >= fit.cols {
            break;
        }
        cup(7, x);
        styled(b"\x1b[2m");
        out(label.as_bytes());
        styled(b"\x1b[0m");
        cup(9, x);
        styled(seq);
        out(sample.as_bytes());
        styled(b"\x1b[0m");
    }
    caption(fit, "curly is SGR 4:3; the coloured underline is SGR 58");
    // The band is repainted with its light sliding across: truecolour
    // backgrounds, cell by cell, twenty times a second.
    if animate(rx, "text", HOLD_TEXT, 20, |t| gradient_title(fit, "glass", t)) {
        return true;
    }
    for (i, shape) in [2u8, 4, 6].into_iter().enumerate() {
        cup(12, 2 + i as u16 * 18);
        out(format!("\x1b[{shape} q").as_bytes());
        out(match shape {
            2 => b"block  " as &[u8],
            4 => b"under  ",
            _ => b"bar",
        });
        if hold(rx, HOLD_CURSOR) {
            return true;
        }
    }
    out(b"\x1b[2 q");
    false
}

/// A three-row band in the neon ramp with the title spaced out in its
/// middle. `phase` (seconds) slides a highlight across it.
fn gradient_title(fit: Fit, title: &str, phase: f32) {
    let cols = fit.cols.max(1);
    let spaced: Vec<u8> = title.bytes().flat_map(|b| [b, b' ']).take(title.len() * 2 - 1).collect();
    let start = cols.saturating_sub(spaced.len() as u16) / 2;
    let mut line = String::with_capacity(usize::from(cols) * 24);
    for band in 1..=3u16 {
        line.clear();
        line.push_str(&format!("\x1b[{band};1H"));
        for col in 1..=cols {
            let t = f32::from(col - 1) / f32::from(cols);
            let sweep = (-((t - (phase * 0.45).fract() * 1.6 + 0.3) * 6.0).powi(2)).exp();
            let edge = if band == 2 { 1.0 } else { 0.62 };
            let c = mix(motion::scale(neon(0.05 + t * 0.55 + phase * 0.05), 0.55 * edge), WHITE, sweep * 0.35);
            let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
            line.push_str(&format!("\x1b[48;2;{r};{g};{b}m"));
            if band == 2 && col > start && usize::from(col - start) <= spaced.len() {
                line.push_str("\x1b[1;38;2;255;248;236m");
                line.push(spaced[usize::from(col - start - 1)] as char);
            } else {
                line.push(' ');
            }
        }
        line.push_str("\x1b[0m");
        out(line.as_bytes());
    }
}

fn scene_sixel(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    // Build the picture before retire, so the previous scene stays up
    // while the plasma is drawn and is not cleared early.
    let (w, h) = fit.picture(0.50, 0.50, 480, 240);
    let (w, h, _) = clamp_in_cell_pixels(fit, 5, w, h);
    let rgba = stills::synthwave(w, h);
    let sixel = encode_sixel(w, h, &rgba);
    retire();
    title("DCS sixel", "a synthwave sunset in 252 colour registers, ordered dither");
    cup(5, 2);
    out(&sixel);
    // The sixel is in-cell: a later write on its rows hides those cells.
    // The caption sits below the rectangle; the cursor moves with CUP only.
    caption_below(
        fit,
        5,
        h.div_ceil(fit.cell_h.max(1)),
        fit.rows.saturating_sub(1).max(1),
        "sized from the window, at least 480x240 when it fits",
    );
    hold(rx, HOLD_SIXEL)
}

fn scene_apc(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    let (c, r) = fit.cell_box(0.48, 0.70);
    let r = r.min(u32::from(fit.rows).saturating_sub(6)).max(4);
    let w = (c * fit.cell_w).clamp(1, 640);
    let h = (r * fit.cell_h).clamp(1, 400);
    let glow = glow(w, h);
    retire();
    title("APC graphics", "a round glow, once under the text and once over it");
    apc(&format!("a=t,i=1,f=32,q=2,s={w},v={h}"), &glow);
    let paragraph = [
        "Glass takes in every dialect a program speaks and expands",
        "it into layers. This paragraph is real text, not a caption",
        "parked under a thumbnail. The glow is a radial gradient",
        "whose alpha falls off to nothing at the round edge.",
    ];
    paint_paragraph(6, 4, &paragraph);
    cup(6, 4);
    apc(&format!("a=p,i=1,z=-1,c={c},r={r},q=2,C=1"), &[]);
    caption(fit, "z<0  the text reads on top of the glow");
    if hold(rx, HOLD_APC_UNDER) {
        return true;
    }
    title("APC graphics", "the same glow, now in front of the text");
    paint_paragraph(6, 4, &paragraph);
    cup(6, 4);
    apc(&format!("a=p,i=1,z=1,c={c},r={r},q=2,C=1"), &[]);
    caption(fit, "z>0  the glow veils the text");
    if hold(rx, HOLD_APC_OVER) {
        return true;
    }
    title("APC graphics", "source and crop, side by side");
    let each = (u32::from(fit.cols) * 3 / 10).max(6);
    let each_r = r.min(u32::from(fit.rows).saturating_sub(6)).max(4);
    cup(5, 2);
    apc(&format!("a=p,i=1,c={each},r={each_r},q=2,C=1"), &[]);
    let right = 2 + each as u16 + 2;
    cup(5, right);
    let crop_x = w / 5;
    let crop_y = h / 5;
    let crop_w = (w / 2).max(1);
    let crop_h = (h / 2).max(1);
    apc(
        &format!("a=p,i=1,c={each},r={each_r},x={crop_x},y={crop_y},w={crop_w},h={crop_h},q=2"),
        &[],
    );
    caption(fit, "left: the source    right: a crop, scaled into the same box");
    hold(rx, HOLD_APC_CROP)
}

fn paint_paragraph(row: u16, col: u16, lines: &[&str]) {
    for (i, line) in lines.iter().enumerate() {
        cup(row + i as u16, col);
        styled(b"\x1b[38;2;236;232;220m");
        out(line.as_bytes());
        styled(b"\x1b[0m");
    }
}

fn scene_placeholders(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    let cols = u32::from(fit.cols).max(1);
    let rows = u32::from(fit.rows).max(1);
    let marks = MARKS.len() as u32;
    let mut grid_c = ((cols as f32) * 0.48).round() as u32;
    grid_c = grid_c.clamp(16.min(cols).max(1), marks.min(cols));
    let mut grid_r = 8u32;
    grid_r = grid_r.min(marks).min(rows.saturating_sub(4).max(1));
    if grid_r < 6 && rows >= 6 {
        grid_r = 6.min(rows);
    }
    // Leave a caption row below the grid. The placeholder cells themselves
    // are the text the picture rides; nothing else is written on them.
    let top = rows.saturating_sub(grid_r).saturating_sub(2).max(4);
    let room = rows.saturating_sub(top).saturating_sub(1).max(1);
    grid_r = grid_r.min(room).max(1);
    let top = top as u16;
    let pw = grid_c.saturating_mul(fit.cell_w).clamp(1, 4096);
    let ph = grid_r.saturating_mul(fit.cell_h).clamp(1, 4096);
    let rgba = blocks(pw, ph);
    retire();
    title("Unicode placeholders", "a gradient with an outline, riding the text");
    cup(top, 2);
    apc(
        &format!("a=T,i=3,z=-1,f=32,q=2,s={pw},v={ph},c={grid_c},r={grid_r}"),
        &rgba,
    );
    for row in 0..grid_r {
        cup(top + row as u16, 2);
        for col in 0..grid_c {
            out(placeholder_cell(row as usize, col as usize).as_bytes());
        }
    }
    caption_below(
        fit,
        top,
        grid_r,
        fit.rows.saturating_sub(1).max(1),
        "the picture rides the text and scrolls up with the lines",
    );
    if hold(rx, HOLD_PLACE) {
        return true;
    }
    let steps = SCROLL_LINES.min(u64::from(rows.saturating_sub(2)));
    for _ in 0..steps {
        out(b"\n");
        if hold(rx, HOLD_SCROLL) {
            return true;
        }
    }
    hold(rx, HOLD_SCROLL_REST)
}

fn scene_osc1337(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    let (c, r) = fit.cell_box(0.50, 0.62);
    let (c, r, _) = in_cell_span(fit, 5, c, r);
    let w = (c * fit.cell_w).clamp(1, 960);
    let h = (r * fit.cell_h).clamp(1, 640);
    let rgba = stills::raytrace(w, h);
    let bmp = write_bmp(w, h, &rgba);
    let name = b64(b"raytrace.bmp");
    let record = format!(
        "\x1b]1337;File=name={name};inline=1;width={c};height={r};preserveAspectRatio=1:{}\x07",
        b64(&bmp)
    );
    retire();
    title("OSC 1337", "ray traced: a mirror ball, gloss, soft shadows, a reflecting floor at sunset");
    cup(5, 2);
    out(record.as_bytes());
    // Inline images are in-cell. The caption goes below the cell
    // rectangle; nothing else is written on those rows.
    caption_below(
        fit,
        5,
        r,
        fit.rows.saturating_sub(1).max(1),
        "inline 24-bit BMP, four rays a pixel, sized from the window",
    );
    hold(rx, HOLD_OSC)
}

fn scene_animation(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    let (c, r) = fit.cell_box(0.42, 1.0);
    let r = r.min(u32::from(fit.rows).saturating_sub(6)).max(1);
    let size = (c * fit.cell_w)
        .min(r * fit.cell_h)
        .min(420)
        .max(200.min(fit.screen_w()))
        .min(fit.screen_w())
        .min(fit.screen_h())
        .max(1);
    let frames: Vec<Vec<u8>> = (0..ANIM_FRAMES).map(|i| sprite(i, ANIM_FRAMES, size)).collect();
    retire();
    title("Animation", "a bouncing ball, twelve frames, eighty milliseconds");
    // Frame 1 is transmit-and-place. `c` and `r` here are columns and
    // rows. Later `a=a` uses those keys as frame numbers, not a placement.
    cup(5, 2);
    apc(
        &apc_transmit_place(5, size, size, c, r, true),
        frames.first().map(Vec::as_slice).unwrap_or(&[]),
    );
    for frame in frames.iter().skip(1) {
        apc(&apc_anim_frame(5, size, size, ANIM_GAP_MS, true), frame);
    }
    apc(&apc_frame_gap(5, 1, ANIM_GAP_MS, true), &[]);
    // `v=5` shows the first frame five times: four full loops, then rest.
    // No `c` or `r`: those are frame numbers on `a=a`, not a placement.
    apc(&apc_anim_run(5, 5, true), &[]);
    caption_below(fit, 5, r, fit.rows.saturating_sub(1).max(1), "four loops, eighty milliseconds");
    hold(rx, HOLD_ANIM)
}

/// One line of a `group` payload.
fn push_record(records: &mut String, record: &str) {
    if !records.is_empty() {
        records.push('\n');
    }
    records.push_str(record);
}

/// A window-pinned layer box in cells, standing flat on the screen: a
/// layer left on the floor is laid on the stage's tilted floor instead.
fn cells(x: f32, y: f32, w: f32, h: f32) -> String {
    format!("stand=screen;anchor=window;at={x:.2},{y:.2},{w:.2},{h:.2}")
}

/// The seven solids round the ring at `t`, and a light under each, as
/// group records. A solid on the near half rides over the text (z > 0);
/// on the far half it passes under it.
fn ring_records(fit: Fit, t: f32, cx: f32, cy: f32, spread: f32, size: f32) -> String {
    let mut records = String::new();
    for i in 0..7usize {
        let (ax, ay, near) = motion::ring(i, t);
        let color = hex(neon(i as f32 / 7.0));
        let w = size * (1.0 + 0.38 * near);
        let h = fit.square(w);
        let x = cx + ax * spread - w / 2.0;
        let y = cy + ay * fit.square(spread) - h / 2.0;
        let z = if near > 0.0 { 3 } else { -3 };
        let o = 0.62 + 0.38 * (near + 1.0) / 2.0;
        push_record(
            &mut records,
            &format!(
                "layer;id={};k=prim;shape={i};c={color};speed=0.8;phase={:.2};o={o:.2};z={z};{}",
                40 + i,
                i as f32 * 0.9,
                cells(x, y, w, h)
            ),
        );
        let g = 2.3;
        push_record(
            &mut records,
            &format!(
                "layer;id={};k=glow;c={color};add=1;o={:.2};z={};{}",
                50 + i,
                0.22 + 0.30 * (near + 1.0) / 2.0,
                z - 1,
                cells(x - w * (g - 1.0) / 2.0, y - h * (g - 1.0) / 2.0, w * g, h * g)
            ),
        );
    }
    records
}

fn scene_prims(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title(
        "glass dialect: lit solids",
        "seven ray-marched solids orbit the text; the far half of the orbit passes under it",
    );
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let lines = [
        "This paragraph is ordinary terminal text.",
        "The solids are k=prim layers, lit and turning on the GPU;",
        "each frame one group op moves all fourteen layers at once.",
        "Near the eye a solid's z is above the text, far from it below.",
    ];
    let mid = (rows * 0.52) as u16;
    for (i, line) in lines.iter().enumerate() {
        let col = (fit.cols.saturating_sub(line.len() as u16) / 2).max(1);
        cup(mid.saturating_sub(2) + i as u16, col);
        styled(b"\x1b[38;2;236;232;220m");
        out(line.as_bytes());
        styled(b"\x1b[0m");
    }
    glass(&format!(
        "layer;id=60;k=glow;c=5a30d0;o=0.55;add=1;z=-6;{}",
        cells(cols * 0.15, rows * 0.18, cols * 0.70, rows * 0.70)
    ));
    glass(&format!(
        "layer;id=61;k=particles;c=ffb060;n=260;size=3;life=4;rise=28;sway=16;seed=3;z=-5;{}",
        cells(0.0, rows * 0.55, cols, rows * 0.45)
    ));
    glass(&format!(
        "layer;id=62;k=particles;c=a080ff;n=160;size=2;life=5;rise=14;sway=24;seed=9;z=-5;{}",
        cells(0.0, rows * 0.10, cols, rows * 0.80)
    ));
    caption(fit, "k=prim  k=glow add=1  k=particles   z<0 under the text, z>0 over it");
    let size = (cols * 0.085).max(6.0);
    animate(rx, "prims", HOLD_PRIMS, SCENE_FPS, |t| {
        let records = ring_records(fit, t, cols * 0.5, rows * 0.52, cols * 0.37, size);
        glass(&group_op(2, 14, &b64(records.as_bytes())));
    })
}

fn scene_cloud(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title(
        "glass dialect: a group of 720 lights",
        "one group op a frame replaces every layer whole: a cloud that becomes five shapes",
    );
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let (cw, ch) = (fit.cell_w.max(1) as f32, fit.cell_h.max(1) as f32);
    let unit = (fit.screen_w() as f32 * 0.24).min(fit.screen_h() as f32 * 0.30);
    let (cx, cy) = (cols * cw * 0.5, rows * ch * 0.54);
    glass(&format!(
        "layer;id=60;k=glow;c=2a1460;o=0.7;add=1;z=-6;{}",
        cells(cols * 0.2, rows * 0.15, cols * 0.6, rows * 0.8)
    ));
    caption(fit, "k=glow add=1, sized and faded by depth; the shape eases and swirls into the next");
    let mut shown = usize::MAX;
    animate(rx, "cloud", HOLD_CLOUD, SCENE_FPS, |t| {
        let mut records = String::with_capacity(CLOUD_POINTS * 100);
        let mut held = 0;
        for i in 0..CLOUD_POINTS {
            let (p, hue, shape) = motion::cloud(i, CLOUD_POINTS, t);
            held = shape;
            let k = 4.4 / (4.4 - p.z);
            let near = ((p.z + 1.6) / 3.2).clamp(0.0, 1.0);
            let x = cx + p.x * k * unit;
            let y = cy - p.y * k * unit;
            let size = (14.0 + 30.0 * near * near) * k;
            let c = mix(neon(hue + t * 0.04), WHITE, 0.10 + 0.40 * near * near);
            let o = 0.18 + 0.62 * near;
            push_record(
                &mut records,
                &format!(
                    "layer;id={};k=glow;c={};o={o:.2};add=1;z=-1;{}",
                    100 + i,
                    hex(c),
                    cells((x - size / 2.0) / cw, (y - size / 2.0) / ch, size / cw, size / ch)
                ),
            );
        }
        glass(&group_op(3, CLOUD_POINTS as u32, &b64(records.as_bytes())));
        if held != shown {
            shown = held;
            let name = motion::CLOUD_NAMES[held];
            cup(4, 3);
            out(b"\x1b[2K");
            styled(b"\x1b[1;38;2;255;244;220m");
            out(name.as_bytes());
            styled(b"\x1b[0m\x1b[38;2;150;140;190m");
            out(format!("   {} of {}", held + 1, motion::CLOUD_NAMES.len()).as_bytes());
            styled(b"\x1b[0m");
        }
    })
}

fn scene_place(id: u32, st: Stage3, z: i32, o: f32, payload: &str) -> String {
    format!(
        "scene;id={id};s={};v={};at={:.2},{:.2},{:.2},{:.2};anchor=window;z={z};o={o:.2};{payload}",
        st.s, st.v, st.x, st.y, st.c, st.r
    )
}

fn scene_tesseract(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title(
        "glass dialect: scene",
        "a tesseract turning through the fourth dimension: neon lines, soft lights, faint glass",
    );
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let st = fit.stage_fit(cols * 0.5, 3.0, rows - 5.0, cols * 0.96, 1.7);
    glass(&format!(
        "layer;id=60;k=glow;c=6020c0;o=0.6;add=1;z=-6;{}",
        cells(cols * 0.22, rows * 0.12, cols * 0.56, rows * 0.82)
    ));
    glass(&format!(
        "layer;id=61;k=particles;c=80c8ff;n=180;size=2;life=6;rise=-10;sway=30;seed=5;z=-5;{}",
        cells(0.0, 3.0, cols, rows - 4.0)
    ));
    let readout = |t: f32| {
        let [xw, yz, zw] = motion::tesseract_angles(t);
        for (i, (plane, angle)) in [("xw", xw), ("yz", yz), ("zw", zw)].into_iter().enumerate() {
            cup(5 + i as u16, 3);
            styled(b"\x1b[38;2;120;200;255m");
            out(plane.as_bytes());
            styled(b"\x1b[38;2;236;232;220m");
            out(format!(" {angle:5.2} rad ").as_bytes());
            let bar = ((angle / std::f32::consts::TAU) * 12.0) as usize;
            styled(b"\x1b[38;2;255;80;180m");
            out("━".repeat(bar).as_bytes());
            styled(b"\x1b[38;2;70;60;110m");
            out("━".repeat(12 - bar.min(12)).as_bytes());
            styled(b"\x1b[0m");
        }
    };
    cup(9, 3);
    styled(b"\x1b[2m");
    out(b"16 corners  32 edges  24 faces");
    styled(b"\x1b[0m");
    caption(fit, "scene: one inline mesh a frame, sorted far to near, rendered at twice its size");
    let mut frame = 0u32;
    animate(rx, "tesseract", HOLD_TESSERACT, SCENE_FPS, |t| {
        glass(&scene_place(8, st, 1, 1.0, &b64(&motion::tesseract(t))));
        if frame.is_multiple_of(3) {
            readout(t);
        }
        frame += 1;
    })
}

fn scene_knot(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title(
        "glass dialect: scene",
        "glass tubing with a neon filament; the text behind it shows through the glass",
    );
    // The page behind the knot is the op that draws it, written as text.
    let words = "glass;scene;id=9;anchor=window;z=2  ·  ";
    let width = usize::from(fit.cols.saturating_sub(2));
    for row in 4..fit.rows.saturating_sub(1) {
        cup(row, 2);
        let shift = (usize::from(row) * 7) % words.len();
        let text: String = words.chars().cycle().skip(shift).take(width).collect();
        let tone = 70 + (u32::from(row) * 9 % 50);
        styled(format!("\x1b[38;2;{};{};{}m", tone, tone - 10, tone + 60).as_bytes());
        out(text.as_bytes());
    }
    styled(b"\x1b[0m");
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let st = fit.stage_fit(cols * 0.5, 3.0, rows - 5.0, cols * 0.96, 1.55);
    caption(fit, "an opaque filament writes depth; the glass blends over it, fresnel-bright at the edges");
    animate(rx, "knot", HOLD_KNOT, SCENE_FPS, |t| {
        glass(&scene_place(9, st, 2, 1.0, &b64(&motion::knot(t))));
    })
}

fn scene_city(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title(
        "glass dialect: scene",
        "an equaliser city: 144 columns on a wave, risen through a sheet of tinted glass",
    );
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let st = fit.stage_fit(cols * 0.5, 3.0, rows - 5.0, cols * 0.96, 1.75);
    glass(&format!(
        "layer;id=60;k=glow;c=ff3c90;o=0.40;add=1;z=-6;{}",
        cells(cols * 0.1, rows * 0.30, cols * 0.8, rows * 0.55)
    ));
    glass(&format!(
        "layer;id=61;k=glow;c=3060ff;o=0.35;add=1;z=-6;{}",
        cells(cols * 0.25, rows * 0.05, cols * 0.5, rows * 0.5)
    ));
    caption(fit, "opaque columns write depth, so the glass sheet cuts through each one pixel by pixel");
    animate(rx, "city", HOLD_CITY, SCENE_FPS, |t| {
        glass(&scene_place(10, st, 1, 1.0, &b64(&motion::city(t))));
    })
}

fn scene_orrery(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title(
        "glass dialect: scene, under the text",
        "an orrery behind the terminal: planets lit from their sun, orbits drawn as comet tails",
    );
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let st = fit.stage_fit(cols * 0.5, 2.5, rows - 3.5, cols, 2.1);
    let lines = [
        ("this text is the terminal's own", "\x1b[1;38;2;255;244;220m"),
        ("the scene sits at z=-2, under it", "\x1b[38;2;236;232;220m"),
        ("six planets, a moon, a ringed giant", "\x1b[38;2;236;232;220m"),
        ("day side, night side, a rim of air", "\x1b[38;2;236;232;220m"),
        ("the ring is translucent; so are the orbits", "\x1b[38;2;236;232;220m"),
    ];
    for (i, (line, sgr)) in lines.iter().enumerate() {
        cup(5 + i as u16 * 2, 3);
        styled(sgr.as_bytes());
        out(line.as_bytes());
        styled(b"\x1b[0m");
    }
    caption(fit, "lit on the CPU from the sun's place, so the terminator turns as each planet goes round");
    animate(rx, "orrery", HOLD_ORRERY, SCENE_FPS, |t| {
        glass(&scene_place(11, st, -2, 1.0, &b64(&motion::orrery(t + 6.0))));
    })
}

/// An asset, `m=1` piece by piece as `picture` goes: only the last piece
/// is answered, and only by the first piece's token.
fn send_asset(id: u32, file: &[u8]) {
    let text = b64(file);
    let pieces: Vec<&str> = text.as_bytes().chunks(ASSET_PIECE).map(|c| std::str::from_utf8(c).unwrap_or("")).collect();
    for (i, piece) in pieces.iter().enumerate() {
        let more = u8::from(i + 1 < pieces.len());
        glass(&asset_op(if i == 0 { Some(id) } else { None }, more, piece));
    }
}

fn asset_op(id: Option<u32>, more: u8, payload: &str) -> String {
    match id {
        Some(id) => format!("asset;id={id};m={more};{payload}"),
        None => format!("asset;m={more};{payload}"),
    }
}

fn play_op(scene: u32, clip: u32) -> String {
    format!("play;id={scene};clip={clip};loop=1")
}

fn tween_op(target: &str, props: &str, timing: &str) -> String {
    format!("tween;{target};{props};{timing}")
}

/// A glTF model made in code: one `asset` op in pieces, one `scene` that
/// hangs it from a node, one `play`, and a camera fly-through of four
/// chained tweens. After that the program sends nothing; glass plays the
/// clip and eases the camera every frame.
fn scene_gltf(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title(
        "glass dialect: asset",
        "a glTF model made in code, uploaded once; glass plays its clip and flies the camera round it",
    );
    let (file, summary) = model::lantern();
    send_asset(31, &file);
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let st = fit.stage_fit(cols * 0.64, 3.0, rows - 5.0, cols * 0.66, 1.2);
    let json = r#"{"v":1,"scene":{"camera":{"eye":[0,1.9,5.8],"target":[0,0.95,0],"fov":0.62},"nodes":[{"id":1,"mesh":{"kind":"gltf","asset":31}},{"id":2,"mesh":{"kind":"cylinder","segments":48},"at":[0,-0.06,0],"scale":[2.7,0.06,2.7],"color":[0.09,0.1,0.14,1],"lit":true}]}}"#;
    glass(&format!(
        "layer;id=60;k=glow;c=2a6cff;o=0.35;add=1;z=-6;{}",
        cells(st.x + st.c * 0.15, st.y + st.r * 0.2, st.c * 0.7, st.r * 0.75)
    ));
    glass(&scene_place(31, st, 1, 1.0, &b64(json.as_bytes())));
    glass(&play_op(31, 0));
    let camera = "id=31;camera=1";
    glass(&tween_op(camera, "eye=3.7,2.7,3.3;target=0,1.3,0", "dur=2.2;ease=sine-in-out;tid=310"));
    glass(&tween_op(camera, "eye=-3.3,0.8,2.9;target=0,1.6,0;fov=0.48", "dur=2.4;ease=cubic-in-out;tid=311;after=310"));
    glass(&tween_op(camera, "eye=0.3,4.8,1.1;target=0,1.15,0;fov=0.75", "dur=2.0;ease=quad-in-out;tid=312;after=311"));
    glass(&tween_op(camera, "eye=0,1.9,5.8;target=0,0.95,0;fov=0.62", "dur=2.4;ease=back-out;tid=313;after=312"));
    let lines = [
        ("asset;id=31;m=1 ... m=0", "\x1b[38;2;120;200;255m"),
        ("  a .glb made in code, sent in pieces", "\x1b[2m"),
        ("scene: {\"kind\":\"gltf\",\"asset\":31}", "\x1b[38;2;120;200;255m"),
        ("  its node tree hangs from node 1", "\x1b[2m"),
        ("play;id=31;clip=0;loop=1", "\x1b[38;2;255;170;80m"),
        ("  linear, step and cubic-spline keys", "\x1b[2m"),
        ("tween;id=31;camera=1;...;after=", "\x1b[38;2;255;120;190m"),
        ("  four chained moves of the eye", "\x1b[2m"),
    ];
    for (i, (line, sgr)) in lines.iter().enumerate() {
        cup(5 + i as u16 + (i as u16 / 2), 3);
        styled(sgr.as_bytes());
        out(line.as_bytes());
        styled(b"\x1b[0m");
    }
    cup(5 + lines.len() as u16 + 5, 3);
    styled(b"\x1b[38;2;236;232;220m");
    out(format!("{} nodes, {} triangles, {} clips, 1 texture", summary.nodes, summary.triangles, summary.clips).as_bytes());
    styled(b"\x1b[0m");
    caption(fit, "after these few ops the program is silent: every frame here is glass's own");
    hold(rx, HOLD_MODEL)
}

/// The easings side by side: one `tween` per dot, each running back and
/// forth for ever, and a scene whose nodes and camera tween beside them.
fn scene_tweens(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    title("glass dialect: tween", "one op per motion: glass eases every frame itself while the program says nothing");
    const EASES: [&str; 12] = [
        "linear",
        "quad-in-out",
        "cubic-out",
        "quint-in",
        "sine-in-out",
        "expo-out",
        "circ-in-out",
        "back-out",
        "elastic-out",
        "bounce-out",
        "cubic-bezier(.68,-.6,.32,1.6)",
        "spring(120,9)",
    ];
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let top = 4.0f32;
    let step = ((rows - top - 3.0) / EASES.len() as f32).clamp(1.0, 2.0);
    let x0 = 33.0f32.min(cols * 0.3);
    let x1 = (cols * 0.56).max(x0 + 8.0);
    let dot = 1.6f32;
    let dot_h = fit.square(dot);
    let halo = 3.6f32;
    let halo_h = fit.square(halo);
    for (i, ease) in EASES.iter().enumerate() {
        let y = top + i as f32 * step;
        let color = neon(i as f32 / EASES.len() as f32);
        let row = y.round() as u16;
        cup(row, 3);
        styled(format!("\x1b[38;2;{};{};{}m", (color[0] * 255.0) as u8, (color[1] * 255.0) as u8, (color[2] * 255.0) as u8).as_bytes());
        out(ease.as_bytes());
        styled(b"\x1b[0m");
        // The row's middle, in the window's own cells (CUP counts from 1).
        let mid = f32::from(row) - 0.5;
        glass(&format!("layer;id={};k=solid;c=ffffff22;z=-1;{}", 400 + i, cells(x0, mid - 0.05, x1 - x0 + dot, 0.1)));
        // A lit sphere over a soft light, both eased the same way.
        let spread = (halo - dot) / 2.0;
        glass(&format!("layer;id={};k=glow;c={};add=1;o=0.7;z=1;{}", 420 + i, hex(color), cells(x0 - spread, mid - halo_h / 2.0, halo, halo_h)));
        glass(&format!("layer;id={};k=prim;shape=1;c={};z=2;{}", 440 + i, hex(color), cells(x0, mid - dot_h / 2.0, dot, dot_h)));
        let timing = format!("dur=1.6;delay=0.5;ease={ease};repeat=forever;yoyo=1");
        let halo_at = format!("at={:.2},{:.2},{halo:.2},{halo_h:.2}", x1 - spread, mid - halo_h / 2.0);
        let dot_at = format!("at={x1:.2},{:.2},{dot:.2},{dot_h:.2}", mid - dot_h / 2.0);
        glass(&tween_op(&format!("layer={}", 420 + i), &halo_at, &timing));
        glass(&tween_op(&format!("layer={}", 440 + i), &dot_at, &timing));
    }
    let st = fit.stage_fit((x1 + dot + cols) * 0.5 + 1.0, 4.0, rows - 7.0, cols - x1 - dot - 4.0, 0.9);
    let json = r#"{"v":1,"scene":{"camera":{"eye":[0,0.6,6.2],"target":[0,0,0],"fov":0.6},"nodes":[{"id":1,"mesh":"cube","at":[-0.8,0.9,0],"scale":[0.8,0.8,0.8],"color":[1,0.55,0.2,1],"lit":true},{"id":2,"mesh":"gem","at":[0.8,0.9,0],"scale":[0.6,0.6,0.6],"color":[0.3,0.9,1,1],"lit":true,"emissive":0.3},{"id":3,"mesh":{"kind":"sphere","segments":32},"at":[-0.8,-0.9,0],"scale":[0.9,0.9,0.9],"color":[1,0.3,0.6,1],"lit":true},{"id":4,"mesh":{"kind":"cone","segments":32},"at":[0.8,-1.2,0],"scale":[0.7,0.9,0.7],"color":[0.6,1,0.4,1],"lit":true}]}}"#;
    glass(&scene_place(50, st, 1, 1.0, &b64(json.as_bytes())));
    let forever = "repeat=forever;yoyo=1";
    glass(&tween_op("id=50;node=1", "rot=0.6124,0.6124,0,0.5", &format!("dur=1.4;ease=elastic-out;{forever}")));
    glass(&tween_op("id=50;node=2", "scale=1,1,1", &format!("dur=1.2;ease=bounce-out;{forever}")));
    glass(&tween_op("id=50;node=3", "color=0.2,0.6,1,1;emissive=0.7", &format!("dur=1.5;ease=sine-in-out;{forever}")));
    glass(&tween_op("id=50;node=4", "at=0.8,-0.5,0", &format!("dur=1.0;ease=spring(140,7);{forever}")));
    glass(&tween_op("id=50;camera=1", "eye=3.2,1.6,5.2", &format!("dur=3;ease=sine-in-out;{forever}")));
    caption(fit, "cube: elastic turn (slerp)  gem: bounce  sphere: colour  cone: spring  camera: sine");
    hold(rx, HOLD_TWEENS)
}

fn scene_chart(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    let (w, h) = chart_panel(fit);
    let (rgba, marks) = bar_chart(w, h);
    let (picture, chart) = chart_upload(CHART_PICTURE, &rgba, w, h, &marks);
    retire();
    title("glass dialect", "a chart");
    glass(&picture);
    glass(&chart);
    mark_showcase_open();
    caption(fit, "picture uploaded, chart open");
    hold(rx, HOLD_CHART)
}

fn scene_video(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    let (panel_w, panel_h) = video_panel(fit);
    // First frame before retire, so the chart stays up while it is built
    // and the panel is not opened onto a cleared screen.
    let first = video_frame(0, VIDEO_FRAMES, panel_w, panel_h);
    retire();
    title("glass dialect", "a video showcase: a neon tunnel, ninety frames");
    // No id: with id, showcase shows a picture. A video panel is size only.
    // frame takes no id here — id would name an uploaded picture.
    open_showcase(&showcase_video(panel_w, panel_h));
    glass(&frame_op(32, panel_w, panel_h, &b64(&first)));
    glass(&progress_op(0.0));
    if hold(rx, HOLD_FRAME) {
        return true;
    }
    for i in 1..VIDEO_FRAMES {
        let frame = video_frame(i, VIDEO_FRAMES, panel_w, panel_h);
        glass(&frame_op(32, panel_w, panel_h, &b64(&frame)));
        let t = if VIDEO_FRAMES <= 1 {
            1.0
        } else {
            i as f32 / (VIDEO_FRAMES - 1) as f32
        };
        glass(&progress_op(t));
        if hold(rx, HOLD_FRAME) {
            return true;
        }
    }
    false
}

fn scene_close(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    close_showcase_if_open();
    glass(&remove_all_op());
    apc("a=d,d=A,q=2", &[]);
    clear_screen();
    title("Sampler", CLOSING);
    caption(fit, "showcase closed");
    hold(rx, HOLD_CLOSE)
}

/// Everything at once, stacked: an orrery filling the window under the
/// text, the glass knot over it, the solids orbiting the knot, lights and
/// dust behind, the title band shimmering, the closing line typed out.
fn scene_finale(rx: &Receiver<Vec<u8>>, fit: Fit) -> bool {
    retire();
    let (cols, rows) = (fit.cols_f(), fit.rows_f());
    let back = fit.stage(0.0, 3.0, cols, rows - 3.0);
    let front = fit.stage_fit(cols * 0.5, 4.0, rows - 7.0, cols * 0.62, 1.45);
    glass(&format!(
        "layer;id=60;k=glow;c=7030e0;o=0.5;add=1;z=-7;{}",
        cells(cols * 0.05, rows * 0.2, cols * 0.5, rows * 0.7)
    ));
    glass(&format!(
        "layer;id=61;k=glow;c=ff3c90;o=0.4;add=1;z=-7;{}",
        cells(cols * 0.45, rows * 0.25, cols * 0.5, rows * 0.7)
    ));
    glass(&format!(
        "layer;id=62;k=particles;c=ffc070;n=300;size=3;life=5;rise=22;sway=20;seed=11;z=-6;{}",
        cells(0.0, rows * 0.4, cols, rows * 0.6)
    ));
    glass(&format!(
        "layer;id=63;k=particles;c=a0d8ff;n=200;size=2;life=6;rise=-8;sway=30;seed=12;z=-6;{}",
        cells(0.0, 3.0, cols, rows - 3.0)
    ));
    let size = (cols * 0.06).max(5.0);
    let typed_row = fit.rows.saturating_sub(1).max(1);
    let typed_col = (fit.cols.saturating_sub(CLOSING.len() as u16) / 2).max(1);
    let mut typed = 0usize;
    animate(rx, "finale", HOLD_FINALE, SCENE_FPS, |t| {
        glass(&scene_place(11, back, -2, 0.85, &b64(&motion::orrery(t + 20.0))));
        glass(&scene_place(9, front, 2, 1.0, &b64(&motion::knot(t + 3.0))));
        let records = ring_records(fit, t, cols * 0.5, rows * 0.55, cols * 0.33, size);
        glass(&group_op(2, 14, &b64(records.as_bytes())));
        gradient_title(fit, "glass", t);
        let due = (((t - 1.5) * 18.0).max(0.0) as usize).min(CLOSING.len());
        if due > typed {
            cup(typed_row, typed_col + typed as u16);
            styled(b"\x1b[1;38;2;255;244;220m");
            out(&CLOSING.as_bytes()[typed..due]);
            styled(b"\x1b[0m");
            typed = due;
        }
    })
}

fn title(head: &str, sub: &str) {
    cup(1, 2);
    styled(b"\x1b[1;38;2;255;244;220m");
    out(head.as_bytes());
    styled(b"\x1b[0m");
    cup(2, 2);
    styled(b"\x1b[2m");
    out(sub.as_bytes());
    styled(b"\x1b[0m");
}

fn caption(fit: Fit, line: &str) {
    cup(fit.rows.saturating_sub(1).max(1), 2);
    styled(b"\x1b[38;2;180;190;210m");
    out(line.as_bytes());
    styled(b"\x1b[0m");
}

fn styled(bytes: &[u8]) {
    out(bytes);
}

fn cup(row: u16, col: u16) {
    out(format!("\x1b[{};{}H", row.max(1), col.max(1)).as_bytes());
}

fn clear_screen() {
    out(b"\x1b[2J\x1b[H");
}

/// Keys a glass op may carry. Mirrors the intake's `allowed_keys`.
/// A glass op never carries `q` — that key belongs to APC graphics.
#[cfg(test)]
fn glass_allowed(op: &str) -> &'static [&'static str] {
    match op {
        "picture" => &["id", "f", "s", "v", "o", "m", "r"],
        "layer" => &[
            "id", "k", "z", "o", "a", "h", "b", "add", "fl", "stand", "up", "at", "px",
            "anchor", "c", "p", "src", "n", "size", "life", "rise", "sway", "seed", "burst",
            "shape", "speed", "phase", "r", "m",
        ],
        "group" => &["g", "n", "r", "m"],
        "remove" => &["id", "g", "all", "asset", "r", "m"],
        "showcase" => &["id", "close", "s", "v", "fit", "events", "r", "m"],
        "scene" => &["id", "at", "px", "anchor", "s", "v", "z", "o", "r"],
        "frame" => &["id", "f", "s", "v", "o", "m", "r"],
        "chart" => &["p", "n", "fit", "r"],
        "model" => &["s", "v", "fit", "r"],
        "page" => &["cols", "rows", "r"],
        "progress" => &["t", "pause", "r"],
        "asset" => &["id", "t", "m", "r"],
        "tween" => &[
            "layer", "id", "node", "camera", "at", "px", "o", "a", "c", "size", "b", "h", "rot",
            "scale", "color", "emissive", "visible", "eye", "target", "fov", "dur", "delay", "ease",
            "repeat", "yoyo", "tid", "after", "stop", "r",
        ],
        "play" => &["id", "node", "clip", "speed", "loop", "t", "pause", "stop", "r"],
        _ => &[],
    }
}

fn picture_op(id: u32, format: u32, s: u32, v: u32, payload: &str) -> String {
    format!("picture;id={id};f={format};s={s};v={v};{payload}")
}

fn group_op(g: u32, n: u32, payload: &str) -> String {
    format!("group;g={g};n={n};{payload}")
}

fn remove_all_op() -> String {
    "remove;all=1".to_string()
}

fn showcase_video(s: u32, v: u32) -> String {
    // `fit=contain` scales a small panel up to the showcase well.
    format!("showcase;s={s};v={v};fit=contain")
}

fn showcase_close_op() -> String {
    "showcase;close=1".to_string()
}

fn frame_op(format: u32, s: u32, v: u32, payload: &str) -> String {
    format!("frame;f={format};s={s};v={v};{payload}")
}

fn chart_op(p: u32, n: u32, payload: &str) -> String {
    format!("chart;p={p};n={n};{payload}")
}

fn progress_op(t: f32) -> String {
    format!("progress;t={t:.3}")
}

/// `--check` appends `r=` so glass answers. The token is a key, so it
/// sits before a payload: the dialect reads the last field as the
/// payload when that field cannot be a key.
fn glass_checked(record: &str, token: u32) -> String {
    if let Some((head, payload)) = split_glass_payload(record) {
        format!("{head};r={token};{payload}")
    } else {
        format!("{record};r={token}")
    }
}

fn split_glass_payload(record: &str) -> Option<(String, String)> {
    let (op, rest) = record.split_once(';')?;
    let takes = matches!(
        op,
        "picture" | "group" | "scene" | "frame" | "chart" | "model" | "page" | "asset"
    );
    if !takes {
        return None;
    }
    let fields: Vec<&str> = rest.split(';').collect();
    if fields.len() < 2 {
        return None;
    }
    let last = *fields.last()?;
    if !last.contains('=') || last.ends_with('=') {
        let head = format!("{op};{}", fields[..fields.len() - 1].join(";"));
        Some((head, last.to_string()))
    } else {
        None
    }
}

fn apc_transmit_place(id: u32, s: u32, v: u32, cols: u32, rows: u32, quiet: bool) -> String {
    let q = if quiet { ",q=2" } else { "" };
    format!("a=T,i={id},f=32,s={s},v={v},c={cols},r={rows},C=1{q}")
}

fn apc_anim_frame(id: u32, s: u32, v: u32, gap: u32, quiet: bool) -> String {
    let q = if quiet { ",q=2" } else { "" };
    format!("a=f,i={id},f=32,s={s},v={v},z={gap}{q}")
}

fn apc_frame_gap(id: u32, frame: u32, gap: u32, quiet: bool) -> String {
    let q = if quiet { ",q=2" } else { "" };
    format!("a=a,i={id},r={frame},z={gap}{q}")
}

fn apc_anim_run(id: u32, loops: u32, quiet: bool) -> String {
    let q = if quiet { ",q=2" } else { "" };
    format!("a=a,i={id},s=3,v={loops}{q}")
}

fn drop_apc_quiet(keys: &str) -> String {
    keys.split(',')
        .filter(|field| !field.is_empty() && !field.starts_with("q="))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
fn glass_key_pairs(record: &str) -> (String, Vec<(String, String)>) {
    let mut fields: Vec<&str> = record.split(';').collect();
    let op = fields.first().copied().unwrap_or("").to_string();
    if !fields.is_empty() {
        fields.remove(0);
    }
    let takes_payload = matches!(
        op.as_str(),
        "picture" | "group" | "scene" | "frame" | "chart" | "model" | "page" | "asset"
    );
    if takes_payload && fields.len() >= 2 {
        if let Some(last) = fields.last().copied() {
            if !last.contains('=') || last.ends_with('=') {
                fields.pop();
            }
        }
    }
    let mut pairs = Vec::new();
    for field in fields {
        if let Some((key, value)) = field.split_once('=') {
            pairs.push((key.to_string(), value.to_string()));
        }
    }
    (op, pairs)
}

#[cfg(test)]
fn apc_key_map(keys: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for field in keys.split(',') {
        if let Some((key, value)) = field.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        } else if !field.is_empty() {
            map.insert(field.to_string(), String::new());
        }
    }
    map
}

/// The video panel is a fixed size under the frame cap, not the window.
/// `fit` on the showcase scales that picture up.
fn video_panel(_fit: Fit) -> (u32, u32) {
    (VIDEO_W, VIDEO_H)
}

fn chart_panel(fit: Fit) -> (u32, u32) {
    let (c, _r) = fit.cell_box(0.46, 0.55);
    let w = (c * fit.cell_w).clamp(1, 640);
    let h = ((w as f32) * 0.55).round() as u32;
    (w, h.clamp(1, 360).min(fit.screen_h().max(1)))
}

/// Columns and rows an in-cell picture may occupy starting at `top`
/// (1-based), leaving a row below for a caption.
fn in_cell_span(fit: Fit, top: u16, want_cols: u32, want_rows: u32) -> (u32, u32, u16) {
    let cols = want_cols
        .min(u32::from(fit.cols.saturating_sub(1)))
        .max(1);
    let room = u32::from(fit.rows.saturating_sub(top))
        .saturating_sub(1)
        .max(1);
    let rows = want_rows.min(room).max(1);
    let caption = top.saturating_add(rows as u16).min(fit.rows).max(1);
    (cols, rows, caption)
}

/// Pixel size of an in-cell picture, clamped so its cell rows end
/// above a caption row.
fn clamp_in_cell_pixels(fit: Fit, top: u16, w: u32, h: u32) -> (u32, u32, u16) {
    let cell_w = fit.cell_w.max(1);
    let cell_h = fit.cell_h.max(1);
    let max_cols = u32::from(fit.cols.saturating_sub(1)).max(1);
    let max_rows = u32::from(fit.rows.saturating_sub(top))
        .saturating_sub(1)
        .max(1);
    let w = w.min(max_cols.saturating_mul(cell_w)).max(1);
    let h = h.min(max_rows.saturating_mul(cell_h)).max(1);
    let rows = h.div_ceil(cell_h).max(1).min(max_rows);
    let caption = top.saturating_add(rows as u16).min(fit.rows).max(1);
    (w, h, caption)
}

fn caption_at(row: u16, line: &str) {
    cup(row.max(1), 2);
    styled(b"\x1b[38;2;180;190;210m");
    out(line.as_bytes());
    styled(b"\x1b[0m");
}

/// Caption below an in-cell picture. If the usual bottom row is already
/// below the rectangle, it stays there so a scene that fitted keeps its
/// look. Otherwise the caption moves to the first row under the picture.
fn caption_below(fit: Fit, top: u16, rows: u32, preferred: u16, line: &str) {
    let end = top.saturating_add(rows as u16);
    let row = if preferred >= end {
        preferred.min(fit.rows)
    } else {
        end.min(fit.rows)
    };
    caption_at(row.max(1), line);
}

fn mark_showcase_open() {
    SHOWCASE_OPEN.store(true, Ordering::Relaxed);
}

fn open_showcase(record: &str) {
    glass(record);
    mark_showcase_open();
}

fn close_showcase_if_open() {
    if SHOWCASE_OPEN.swap(false, Ordering::Relaxed) {
        glass(&showcase_close_op());
    }
}

fn glass(record: &str) {
    let record = if checking() {
        let token = NEXT_REPLY.fetch_add(1, Ordering::Relaxed);
        note_sent();
        glass_checked(record, token)
    } else {
        record.to_string()
    };
    out(format!("\x1b_glass;{record}\x1b\\").as_bytes());
}

fn apc(keys: &str, payload: &[u8]) {
    let keys = if checking() {
        note_sent();
        drop_apc_quiet(keys)
    } else {
        keys.to_string()
    };
    let mut bytes = Vec::with_capacity(keys.len() + payload.len() + 8);
    bytes.extend_from_slice(b"\x1b_G");
    bytes.extend_from_slice(keys.as_bytes());
    if !payload.is_empty() {
        bytes.push(b';');
        bytes.extend_from_slice(b64(payload).as_bytes());
    }
    bytes.extend_from_slice(b"\x1b\\");
    out(&bytes);
}


/// A radial glow. Alpha is 228 at the centre and 0 at the corner.
fn glow(w: u32, h: u32) -> Vec<u8> {
    let w = w.max(1);
    let h = h.max(1);
    let cx = (w as f32 - 1.0) * 0.5;
    let cy = (h as f32 - 1.0) * 0.5;
    let max_r = cx.hypot(cy).max(1.0);
    let mut rgba = Vec::with_capacity((w as usize).saturating_mul(h as usize).saturating_mul(4));
    for y in 0..h {
        for x in 0..w {
            let d = (x as f32 - cx).hypot(y as f32 - cy) / max_r;
            let fall = (1.0 - d).max(0.0);
            let alpha = (fall.powf(1.7) * 228.0).round() as u8;
            let g = (160.0 + 50.0 * fall).clamp(0.0, 255.0) as u8;
            rgba.extend_from_slice(&[255, g, 64, alpha]);
        }
    }
    rgba
}

/// A gradient with a bright outline. Shade is scaled in `u32`: doing it
/// in `u8` saturates every product at 255 and the divide collapses the
/// picture to near-black.
fn blocks(w: u32, h: u32) -> Vec<u8> {
    let w = w.max(1);
    let h = h.max(1);
    let mut rgba = Vec::with_capacity((w as usize).saturating_mul(h as usize).saturating_mul(4));
    for y in 0..h {
        for x in 0..w {
            if x < 2 || y < 2 || x + 2 >= w || y + 2 >= h {
                rgba.extend_from_slice(&[255, 236, 180, 255]);
                continue;
            }
            let r = 40 + (x * 180) / w;
            let g = 36 + (y * 90) / h;
            let b = 210 - (x * 150) / w;
            let shade = 180 + (y * 40) / h;
            let scale = |c: u32| -> u8 { ((c * shade) / 220).min(255) as u8 };
            rgba.extend_from_slice(&[scale(r), scale(g), scale(b), 255]);
        }
    }
    rgba
}

/// A bouncing ball with squash on the ground. Frames differ because the
/// ball moves; colour is mixed in `f32`.
fn sprite(frame: usize, frames: usize, size: u32) -> Vec<u8> {
    let size = size.max(1);
    let mut rgba = vec![0u8; (size as usize).saturating_mul(size as usize).saturating_mul(4)];
    let frames = frames.max(1);
    let t = frame as f32 / frames as f32;
    let bounce = (t * std::f32::consts::PI).sin();
    let squash = 1.0 - bounce;
    let rx = size as f32 * (0.12 + 0.07 * squash);
    let ry = size as f32 * (0.12 - 0.05 * squash).max(0.05);
    let cx = size as f32 * (0.18 + 0.64 * t);
    let ground = size as f32 * 0.78;
    let cy = ground - bounce * size as f32 * 0.52;
    let (hr, hg, hb) = hue(frame, frames);
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5 - cx) / rx.max(0.5);
            let dy = (y as f32 + 0.5 - cy) / ry.max(0.5);
            if dx * dx + dy * dy <= 1.0 {
                let light = (0.55 - 0.35 * dx - 0.25 * dy).clamp(0.3, 1.0);
                let i = ((y * size + x) * 4) as usize;
                rgba[i] = (f32::from(hr) * light) as u8;
                rgba[i + 1] = (f32::from(hg) * light) as u8;
                rgba[i + 2] = (f32::from(hb) * light) as u8;
                rgba[i + 3] = 255;
            }
        }
    }
    rgba
}

fn hue(frame: usize, frames: usize) -> (u8, u8, u8) {
    let t = frame as f32 / frames.max(1) as f32;
    let r = (180.0 + 70.0 * (t * 6.0).sin()) as u8;
    let g = (120.0 + 80.0 * (t * 4.0 + 1.0).sin()) as u8;
    let b = (80.0 + 60.0 * (t * 5.0 + 2.0).sin()) as u8;
    (r, g, b)
}

/// One frame of the video showcase: the neon tunnel, flying.
fn video_frame(i: u64, frames: u64, w: u32, h: u32) -> Vec<u8> {
    stills::tunnel(i, frames, w, h)
}

fn bar_chart(w: u32, h: u32) -> (Vec<u8>, String) {
    let w = w.max(1);
    let h = h.max(1);
    let heights = [0.45f32, 0.72, 0.38, 0.88, 0.56, 0.66];
    let colors = [
        [255, 120, 64],
        [255, 196, 72],
        [120, 210, 255],
        [255, 96, 140],
        [130, 230, 160],
        [200, 150, 255],
    ];
    let tips = ["one", "two", "three", "four", "five", "six"];
    let mut rgba = [16u8, 18, 28, 255].repeat((w as usize).saturating_mul(h as usize));
    let gap = (w / (heights.len() as u32 + 1)).max(1);
    let bar_w = (gap * 3 / 5).max(1);
    let mut marks = String::new();
    for (i, ((height, color), tip)) in heights.iter().zip(colors).zip(tips).enumerate() {
        let x0 = gap.saturating_mul(i as u32 + 1).saturating_sub(bar_w / 2);
        let bh = ((h as f32) * height) as u32;
        let bh = bh.max(1).min(h);
        let y0 = h.saturating_sub(bh).saturating_sub(h / 12).min(h.saturating_sub(1));
        let y1 = h.saturating_sub(h / 14).max(y0.saturating_add(1));
        let drawn = y1.saturating_sub(y0).max(1);
        for y in y0..y1 {
            for x in x0..x0.saturating_add(bar_w).min(w) {
                let p = ((y * w + x) * 4) as usize;
                if p + 3 < rgba.len() {
                    rgba[p..p + 3].copy_from_slice(&color);
                }
            }
        }
        if !marks.is_empty() {
            marks.push('\n');
        }
        marks.push_str(&format!(
            "mark;x={x0};y={y0};w={bar_w};h={drawn};t={tip}|{:.0}%",
            height * 100.0
        ));
    }
    (rgba, marks)
}

/// `picture` upload, then `chart;p=` over those pixels. `n` is the mark count.
fn chart_upload(id: u32, rgba: &[u8], w: u32, h: u32, marks: &str) -> (String, String) {
    let n = marks.lines().filter(|line| line.starts_with("mark;")).count();
    let picture = picture_op(id, 32, w, h, &b64(rgba));
    let chart = chart_op(id, n as u32, &b64(marks.as_bytes()));
    (picture, chart)
}

#[cfg(test)]
fn checker(w: u32, h: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for y in 0..h {
        for x in 0..w {
            let on = ((x / 4) + (y / 4)) % 2 == 0;
            if on {
                rgba.extend_from_slice(&[40, 180, 220, 230]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    rgba
}

/// Levels a channel the sixel palette quantises to: 6 × 7 × 6 = 252
/// registers, inside the 256 a sixel picture commonly gets.
const SIXEL_LEVELS: [u32; 3] = [6, 7, 6];

/// An 8×8 Bayer matrix: the ordered dither's thresholds, 0..64.
const BAYER8: [u8; 64] = [
    0, 32, 8, 40, 2, 34, 10, 42, 48, 16, 56, 24, 50, 18, 58, 26, 12, 44, 4, 36, 14, 46, 6, 38, 60, 28,
    52, 20, 62, 30, 54, 22, 3, 35, 11, 43, 1, 33, 9, 41, 51, 19, 59, 27, 49, 17, 57, 25, 15, 47, 7, 39,
    13, 45, 5, 37, 63, 31, 55, 23, 61, 29, 53, 21,
];

fn encode_sixel(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let pixels = (w as usize).saturating_mul(h as usize);
    let (palette, index) = indexed(rgba, w, pixels);
    let mut out = Vec::new();
    out.extend_from_slice(b"\x1bPq");
    out.extend_from_slice(format!("\"1;1;{w};{h}").as_bytes());
    for (i, color) in palette.iter().enumerate() {
        let (pr, pg, pb) = (
            u16::from(color[0]) * 100 / 255,
            u16::from(color[1]) * 100 / 255,
            u16::from(color[2]) * 100 / 255,
        );
        out.extend_from_slice(format!("#{i};2;{pr};{pg};{pb}").as_bytes());
    }
    // One pass a band builds every register's six-pixel columns at once,
    // then each register that appears in the band is written as a row.
    let w_us = w as usize;
    let mut sixes = vec![0u8; palette.len() * w_us];
    let mut used = vec![false; palette.len()];
    let mut y = 0u32;
    while y < h {
        let band = (h - y).min(6);
        sixes.fill(0);
        used.fill(false);
        for dy in 0..band {
            let row = ((y + dy) as usize) * w_us;
            for x in 0..w_us {
                if let Some(slot) = index.get(row + x).copied().flatten() {
                    let slot = usize::from(slot);
                    sixes[slot * w_us + x] |= 1 << dy;
                    used[slot] = true;
                }
            }
        }
        for slot in 0..palette.len() {
            if !used[slot] {
                continue;
            }
            out.extend_from_slice(format!("#{slot}").as_bytes());
            let line = &sixes[slot * w_us..(slot + 1) * w_us];
            let mut run = 0u32;
            let mut prev = 0u8;
            for (x, six) in line.iter().copied().enumerate() {
                if x > 0 && six == prev {
                    run += 1;
                } else {
                    if x > 0 {
                        push_sixel(&mut out, prev, run);
                    }
                    prev = six;
                    run = 1;
                }
            }
            push_sixel(&mut out, prev, run);
            out.push(b'$');
        }
        out.push(b'-');
        y += band;
    }
    out.extend_from_slice(b"\x1b\\");
    out
}

/// Exact registers when the picture has 256 colours or fewer, otherwise
/// the 252-register cube with an ordered dither.
fn indexed(rgba: &[u8], width: u32, pixels: usize) -> (Vec<[u8; 3]>, Vec<Option<u8>>) {
    let mut colors: Vec<[u8; 3]> = Vec::new();
    let mut index = vec![None; pixels];
    for i in 0..pixels {
        let px = &rgba[i * 4..i * 4 + 4];
        if px[3] < 16 {
            continue;
        }
        let rgb = [px[0], px[1], px[2]];
        if let Some(slot) = colors.iter().position(|c| *c == rgb) {
            index[i] = Some(slot as u8);
        } else if colors.len() < 256 {
            index[i] = Some(colors.len() as u8);
            colors.push(rgb);
        } else {
            return quantize(rgba, width, pixels);
        }
    }
    if colors.is_empty() {
        colors.push([0, 0, 0]);
    }
    (colors, index)
}

/// One channel to a level of `levels`, dithered by the threshold at
/// `x, y`.
fn dither_level(value: u8, levels: u32, x: u32, y: u32) -> u32 {
    let step = 255.0 / (levels - 1) as f32;
    let threshold = (f32::from(BAYER8[((y & 7) * 8 + (x & 7)) as usize]) + 0.5) / 64.0 - 0.5;
    ((f32::from(value) / step + threshold).round() as i32).clamp(0, levels as i32 - 1) as u32
}

fn quantize(rgba: &[u8], width: u32, pixels: usize) -> (Vec<[u8; 3]>, Vec<Option<u8>>) {
    let width = width.max(1);
    let [lr, lg, lb] = SIXEL_LEVELS;
    let cube = (lr * lg * lb) as usize;
    let mut slot_of = vec![None; cube];
    let mut colors = Vec::new();
    let mut index = vec![None; pixels];
    let value = |level: u32, levels: u32| ((level * 255) as f32 / (levels - 1) as f32).round() as u8;
    for i in 0..pixels {
        let px = &rgba[i * 4..i * 4 + 4];
        if px[3] < 16 {
            continue;
        }
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        let (r, g, b) = (dither_level(px[0], lr, x, y), dither_level(px[1], lg, x, y), dither_level(px[2], lb, x, y));
        let key = ((r * lg + g) * lb + b) as usize;
        let slot = *slot_of[key].get_or_insert_with(|| {
            colors.push([value(r, lr), value(g, lg), value(b, lb)]);
            (colors.len() - 1) as u8
        });
        index[i] = Some(slot);
    }
    if colors.is_empty() {
        colors.push([0, 0, 0]);
    }
    (colors, index)
}

fn push_sixel(out: &mut Vec<u8>, six: u8, run: u32) {
    if run > 3 {
        out.extend_from_slice(format!("!{run}").as_bytes());
    } else {
        for _ in 1..run {
            out.push(six + 63);
        }
    }
    out.push(six + 63);
}

fn write_bmp(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let row = (w.saturating_mul(3) + 3) & !3;
    let pixels = row.saturating_mul(h);
    let file = 54u32.saturating_add(pixels);
    let mut bmp = vec![0u8; file as usize];
    bmp[0] = b'B';
    bmp[1] = b'M';
    bmp[2..6].copy_from_slice(&file.to_le_bytes());
    bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&w.to_le_bytes());
    bmp[22..26].copy_from_slice(&h.to_le_bytes());
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
    bmp[34..38].copy_from_slice(&pixels.to_le_bytes());
    for y in 0..h {
        let src_y = h - 1 - y;
        for x in 0..w {
            let s = ((src_y * w + x) * 4) as usize;
            let d = (54 + y * row + x * 3) as usize;
            if s + 2 < rgba.len() && d + 2 < bmp.len() {
                bmp[d] = rgba[s + 2];
                bmp[d + 1] = rgba[s + 1];
                bmp[d + 2] = rgba[s];
            }
        }
    }
    bmp
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = (u32::from(bytes[i]) << 16) | (u32::from(bytes[i + 1]) << 8) | u32::from(bytes[i + 2]);
        for shift in [18, 12, 6, 0] {
            out.push(T[((n >> shift) & 63) as usize] as char);
        }
        i += 3;
    }
    if i < bytes.len() {
        let mut n = u32::from(bytes[i]) << 16;
        let mut pads = 2;
        if i + 1 < bytes.len() {
            n |= u32::from(bytes[i + 1]) << 8;
            pads = 1;
        }
        for shift in [18, 12, 6, 0].into_iter().take(4 - pads) {
            out.push(T[((n >> shift) & 63) as usize] as char);
        }
        for _ in 0..pads {
            out.push('=');
        }
    }
    out
}

const MARKS: &[u32] = &[
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F,
    0x0346, 0x034A, 0x034B, 0x034C, 0x0350, 0x0351, 0x0352, 0x0357,
    0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484,
    0x0485, 0x0486, 0x0487, 0x0592, 0x0593, 0x0594, 0x0595, 0x0597,
    0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
    0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611,
    0x0612, 0x0613, 0x0614, 0x0615, 0x0616, 0x0617, 0x0657, 0x0658,
    0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
    0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2,
    0x06E4, 0x06E7, 0x06E8, 0x06EB, 0x06EC, 0x0730, 0x0732, 0x0733,
    0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE,
    0x07EF, 0x07F0, 0x07F1, 0x07F3, 0x0816, 0x0817, 0x0818, 0x0819,
    0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C,
    0x082D, 0x0951, 0x0953, 0x0954, 0x0F82, 0x0F83, 0x0F86, 0x0F87,
    0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
    0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D,
    0x1B6E, 0x1B6F, 0x1B70, 0x1B71, 0x1B72, 0x1B73, 0x1CD0, 0x1CD1,
    0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
    0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1,
    0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5, 0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9,
    0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
    0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1,
    0x20D4, 0x20D5, 0x20D6, 0x20D7, 0x20DB, 0x20DC, 0x20E1, 0x20E7,
    0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
    0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA,
    0x2DEB, 0x2DEC, 0x2DED, 0x2DEE, 0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2,
    0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D,
    0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1, 0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5,
    0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
    0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7,
    0xAAB8, 0xAABE, 0xAABF, 0xAAC1, 0xFE20, 0xFE21, 0xFE22, 0xFE23,
    0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186, 0x1D187,
    0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243,
    0x1D244,
];

fn mark(i: usize) -> char {
    char::from_u32(MARKS.get(i).copied().unwrap_or(0x0305)).unwrap_or('\u{0305}')
}

fn placeholder_cell(row: usize, col: usize) -> String {
    format!("\u{10EEEE}{}{}", mark(row), mark(col))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(buf: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn the_show_runs_under_two_minutes_and_most_of_it_is_3d() {
        let motion = HOLD_PRIMS + HOLD_CLOUD + HOLD_TESSERACT + HOLD_KNOT + HOLD_CITY + HOLD_ORRERY + HOLD_MODEL + HOLD_TWEENS + HOLD_FINALE;
        let ms = HOLD_TEXT
            + HOLD_CURSOR * 3
            + HOLD_SIXEL
            + HOLD_APC_UNDER
            + HOLD_APC_OVER
            + HOLD_APC_CROP
            + HOLD_PLACE
            + HOLD_SCROLL * SCROLL_LINES
            + HOLD_SCROLL_REST
            + HOLD_OSC
            + HOLD_ANIM
            + motion
            + HOLD_CHART
            + HOLD_FRAME * VIDEO_FRAMES
            + HOLD_CLOSE;
        assert!(
            (90_000..120_000).contains(&ms),
            "the show is {ms} ms, not a minute and a half to two"
        );
        assert!(motion * 3 >= ms * 2, "the 3D and motion scenes are {motion} of {ms} ms");
        let names: Vec<&str> = SCENES.iter().map(|(name, _)| *name).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "scene names are unique");
    }

    #[test]
    fn scene_picks_keep_the_show_order_and_refuse_a_wrong_name() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(picked_scenes(&args(&[])).unwrap().len(), SCENES.len());
        assert_eq!(
            picked_scenes(&args(&["--scene", "orrery,knot"])).unwrap(),
            vec!["knot", "orrery"]
        );
        assert_eq!(picked_scenes(&args(&["--scene=finale", "--loop"])).unwrap(), vec!["finale"]);
        assert_eq!(picked_scenes(&args(&["--scene", "nope"])), Err("nope".to_string()));
    }

    #[test]
    fn a_scene_renders_at_twice_its_pixels_inside_the_side_cap() {
        let fit = Fit {
            rows: 53,
            cols: 189,
            cell_w: 10,
            cell_h: 20,
        };
        let small = fit.stage(0.0, 0.0, 40.0, 20.0);
        assert_eq!((small.s, small.v), (800, 800));
        let big = fit.stage(0.0, 0.0, 189.0, 50.0);
        assert!(big.s <= SCENE_SIDE && big.v <= SCENE_SIDE, "{big:?}");
        assert!(big.s as f32 > 189.0 * 10.0, "still more pixels than it covers: {big:?}");
        let fitted = fit.stage_fit(94.5, 3.0, 48.0, 180.0, 1.7);
        let aspect = (fitted.c * 10.0) / (fitted.r * 20.0);
        assert!((aspect - 1.7).abs() < 0.01, "{aspect}");
        assert!(fitted.c <= 180.0 + 1e-3 && fitted.r <= 48.0 + 1e-3);
        assert!(fitted.x >= 0.0 && fitted.y >= 3.0);
    }

    #[test]
    fn quit_during_the_probe_exits() {
        let (tx, rx) = mpsc::channel();
        tx.send(vec![b'q']).unwrap();
        let started = Instant::now();
        assert!(probe(&rx, 24, 80).is_none());
        assert!(
            started.elapsed() < Duration::from_millis(50),
            "q during the probe must not wait out the deadline"
        );
        let _keep = tx;
    }

    /// The race: glass answers every query at once. With those bytes already
    /// on the channel, the probe must take them and return a fit at once.
    /// It must not read stdin (there is no tty here).
    #[test]
    fn queued_replies_return_a_fit_at_once() {
        let (tx, rx) = mpsc::channel();
        tx.send(b"\x1b[I".to_vec()).unwrap();
        tx.send(
            b"\x1b[?62;4;22c\x1b[>1;10;0c\x1b[8;39;154t\x1b[6;25;12t".to_vec(),
        )
        .unwrap();
        let started = Instant::now();
        let fit = probe(&rx, 24, 80).expect("queued replies are a fit");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(50),
            "probe blocked {elapsed:?} with replies already queued"
        );
        assert_eq!(fit.rows, 39);
        assert_eq!(fit.cols, 154);
        assert_eq!(fit.cell_h, 25);
        assert_eq!(fit.cell_w, 12);
        let _keep = tx;
    }

    /// No reply on the channel: return None when the deadline lands, and
    /// do not block past it in a read.
    #[test]
    fn no_replies_return_none_by_the_deadline() {
        let (tx, rx) = mpsc::channel();
        let started = Instant::now();
        let fit = probe(&rx, 24, 80);
        let elapsed = started.elapsed();
        assert!(fit.is_none(), "silence is not glass");
        assert!(
            elapsed >= Duration::from_millis(PROBE_MS.saturating_sub(30)),
            "returned {elapsed:?} before the deadline"
        );
        assert!(
            elapsed < Duration::from_millis(PROBE_MS + 200),
            "blocked {elapsed:?} past the deadline"
        );
        let _keep = tx;
    }

    #[test]
    fn focus_reports_and_late_replies_are_not_a_quit() {
        assert!(!wants_quit(b"\x1b[I"));
        assert!(!wants_quit(b"\x1b[O"));
        assert!(!wants_quit(
            b"\x1b[?62;4;22c\x1b[>1;10;0c\x1b[8;39;154t\x1b[6;25;12t"
        ));
        assert!(!wants_quit(b"\x1b_glass;r=1;OK;\x1b\\"));
        assert!(!wants_quit(b"\x1b[2 q"));
        assert!(wants_quit(b"q"));
        assert!(wants_quit(&[3]));
        assert!(wants_quit(b"\x1b[Iq"));
    }

    #[test]
    fn chart_is_uploaded_then_opened_with_chart_p() {
        let (rgba, marks) = bar_chart(64, 40);
        assert_eq!(rgba.len(), 64 * 40 * 4);
        assert!(marks.lines().any(|line| line.starts_with("mark;")));
        let (picture, chart) = chart_upload(CHART_PICTURE, &rgba, 64, 40, &marks);
        assert!(picture.starts_with("picture;id=11;f=32;s=64;v=40;"), "{picture}");
        assert!(chart.starts_with("chart;p=11;n="), "{chart}");
        assert!(!picture.contains("k=chart"), "{picture}");
        assert!(!chart.contains("k=chart"), "{chart}");
        let n = chart
            .split(';')
            .find_map(|field| field.strip_prefix("n="))
            .and_then(|n| n.parse::<usize>().ok())
            .expect("n=");
        let marks_n = marks.lines().filter(|line| line.starts_with("mark;")).count();
        assert_eq!(n, marks_n);
    }

    #[test]
    fn sixel_header_carries_the_raster_and_a_named_colour() {
        let rgba = vec![255u8, 0, 0, 255, 0, 0, 255, 255];
        let sixel = encode_sixel(2, 1, &rgba);
        let text = String::from_utf8(sixel).unwrap();
        assert!(text.starts_with("\u{1b}Pq"), "{text}");
        assert!(text.contains("\"1;1;2;1"), "{text}");
        assert!(text.contains("#0;2;100;0;0"), "{text}");
        assert!(text.contains("#1;2;0;0;100"), "{text}");
        assert!(text.ends_with("\u{1b}\\"), "{text}");
    }

    #[test]
    fn sixel_quantises_past_256_colours() {
        let mut rgba = Vec::new();
        for i in 0..600u32 {
            rgba.extend_from_slice(&[(i % 256) as u8, (i / 2 % 256) as u8, (i * 13 % 256) as u8, 255]);
        }
        let sixel = String::from_utf8(encode_sixel(600, 1, &rgba)).unwrap();
        // Palette entries are `#i;2;r;g;b`; a band's `#i` carries no `;`.
        let colours = sixel
            .split('#')
            .filter(|entry| entry.split(';').count() == 5 && entry.split(';').nth(1) == Some("2"))
            .count();
        assert!(colours <= 252, "{colours} palette entries");
        assert!(colours > 64, "the cube keeps more than the old 64: {colours}");
    }

    #[test]
    fn sixel_bands_paint_each_pixel_once_in_its_register() {
        // 3 x 7: two bands; each pixel's register sets exactly its bit.
        let (w, h) = (3u32, 7u32);
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(if (x + y) % 2 == 0 { &[255, 0, 0, 255] } else { &[0, 0, 255, 255] });
            }
        }
        let text = String::from_utf8(encode_sixel(w, h, &rgba)).unwrap();
        let body = text.split("#1;2;0;0;100").nth(1).unwrap();
        let bands: Vec<&str> = body.trim_end_matches("\u{1b}\\").split('-').filter(|b| !b.is_empty()).collect();
        assert_eq!(bands.len(), 2, "{text}");
        // Band one, register 0 (red): column 0 is rows 0, 2, 4 -> bits 0b010101.
        assert!(bands[0].starts_with(&format!("#0{}", (0b010101u8 + 63) as char)), "{}", bands[0]);
        // Band two is row 6 alone: red at columns 0 and 2.
        assert!(bands[1].contains(&format!("#0{}{}{}", (1u8 + 63) as char, 63u8 as char, (1u8 + 63) as char)), "{}", bands[1]);
    }

    #[test]
    fn bmp_is_a_bottom_up_24bit_file() {
        let rgba = vec![10u8, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255];
        let bmp = write_bmp(2, 2, &rgba);
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[18..22].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(bmp[22..26].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
        assert_eq!(&bmp[54..57], &[90, 80, 70]);
    }

    #[test]
    fn base64_roundtrip_matches_the_standard_alphabet() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn placeholder_marks_match_the_dialect_table() {
        // The reference table is not in the repository: `GLASS_REF_DIR` names
        // the folder that holds `rowcolumn-diacritics.txt`.
        let Some(dir) = std::env::var_os("GLASS_REF_DIR") else {
            eprintln!("GLASS_REF_DIR is unset; skipping the row/column diacritic table check");
            return;
        };
        let reference: Vec<u32> = std::fs::read_to_string(std::path::Path::new(&dir).join("rowcolumn-diacritics.txt"))
        .expect("the row/column diacritic table")
        .lines()
        .filter_map(|line| {
            let line = line.split('#').next()?.trim();
            let code = line.split(';').next()?.trim();
            if code.is_empty() {
                None
            } else {
                u32::from_str_radix(code, 16).ok()
            }
        })
        .take(10)
        .collect();
        assert_eq!(reference.len(), 10, "the reference table should list the marks");
        for (i, code) in reference.iter().enumerate() {
            assert_eq!(mark(i) as u32, *code, "mark {i}");
        }
    }

    #[test]
    fn sprite_frames_are_distinct_and_checker_has_real_alpha() {
        let first = sprite(0, ANIM_FRAMES, 28);
        for i in 1..ANIM_FRAMES {
            assert_ne!(first, sprite(i, ANIM_FRAMES, 28), "frame {i} matches frame 0");
        }
        let board = checker(24, 16);
        assert!(board.iter().step_by(4).any(|a| *a == 0));
        assert!(board.iter().skip(3).step_by(4).any(|a| *a > 200));
    }

    #[test]
    fn blocks_shade_is_scaled_in_u32() {
        // Pixel (8, 4) of a 32×16 gradient is interior. The old `u8`
        // multiply saturated at 255 and this pixel came out (1, 1, 1).
        let picture = blocks(32, 16);
        assert_eq!(px(&picture, 32, 0, 0), [255, 236, 180, 255]);
        assert_eq!(px(&picture, 32, 8, 4), [73, 50, 149, 255]);
    }

    #[test]
    fn glow_is_opaque_at_the_centre_and_clear_at_the_edge() {
        let picture = glow(32, 24);
        assert_eq!(picture.len(), 32 * 24 * 4);
        assert!(px(&picture, 32, 16, 12)[3] >= 200, "centre alpha");
        assert_eq!(px(&picture, 32, 0, 0)[3], 0, "corner alpha");
        assert_eq!(px(&picture, 32, 31, 23)[3], 0, "far corner alpha");
    }

    #[test]
    fn a_bouncing_ball_squashes_on_the_ground_and_frames_move() {
        let ground = sprite(0, 12, 64);
        let air = sprite(6, 12, 64);
        assert_eq!(ground.len(), 64 * 64 * 4);
        assert_ne!(ground, air);
        let bounds = |buf: &[u8]| {
            let mut min = (64u32, 64u32);
            let mut max = (0u32, 0u32);
            let mut any = false;
            for y in 0..64u32 {
                for x in 0..64u32 {
                    if px(buf, 64, x, y)[3] > 0 {
                        any = true;
                        min.0 = min.0.min(x);
                        min.1 = min.1.min(y);
                        max.0 = max.0.max(x);
                        max.1 = max.1.max(y);
                    }
                }
            }
            assert!(any);
            (max.0 - min.0, max.1 - min.1)
        };
        let (gw, gh) = bounds(&ground);
        let (_aw, ah) = bounds(&air);
        assert!(gw > gh, "squash on the ground: {gw}x{gh}");
        assert!(ah > gh, "the ball is taller in the air");
    }

    #[test]
    fn video_frames_fill_the_frame_and_move() {
        let a = video_frame(0, 20, 80, 40);
        let b = video_frame(7, 20, 80, 40);
        assert_eq!(a.len(), 80 * 40 * 4);
        assert_ne!(a, b);
        assert!(a.chunks(4).all(|p| p[3] == 255));
    }

    #[test]
    fn a_main_picture_fills_about_half_the_window() {
        let fit = Fit {
            rows: 53,
            cols: 189,
            cell_w: 10,
            cell_h: 20,
        };
        let (w, h) = fit.picture(0.50, 0.50, 480, 240);
        assert!(w >= 480, "{w}");
        assert!(h >= 240, "{h}");
        assert!(w <= fit.screen_w() && h <= fit.screen_h());
        let frac = w as f32 / fit.screen_w() as f32;
        assert!((0.40..=0.60).contains(&frac), "{frac}");
    }

    #[test]
    fn glass_builders_use_only_allowed_keys_and_never_carry_q() {
        let fit = Fit {
            rows: 53,
            cols: 189,
            cell_w: 10,
            cell_h: 20,
        };
        let st = fit.stage(2.0, 3.0, 80.0, 30.0);
        let mut samples = vec![
            picture_op(11, 32, 64, 40, "Zg=="),
            group_op(1, 7, "Zg=="),
            remove_all_op(),
            showcase_video(320, 180),
            showcase_close_op(),
            scene_place(8, st, -2, 0.85, "Zg=="),
            frame_op(32, 320, 180, "Zg=="),
            chart_op(11, 6, "Zg=="),
            progress_op(0.5),
            glass_checked("scene;id=1;s=2;v=2;at=1,1,2,2", 7),
            asset_op(Some(31), 1, "Zg=="),
            asset_op(None, 0, "Zg=="),
            play_op(31, 0),
            tween_op("id=31;camera=1", "eye=0,1,5;target=0,1,0;fov=0.6", "dur=2;ease=back-out;tid=1;after=2"),
            tween_op("layer=420", "at=1,2,3,4;o=0.5;c=ff00ff", "dur=1.6;delay=0.5;ease=spring(120,9);repeat=forever;yoyo=1"),
            tween_op("id=50;node=1", "rot=0,0,0,1;scale=1,1,1;color=1,1,1,1;emissive=0;visible=1", "stop=1"),
            "remove;asset=31".to_string(),
        ];
        // The records the motion scenes put in their groups, each a layer.
        samples.extend(ring_records(fit, 1.3, 94.0, 27.0, 70.0, 16.0).lines().map(str::to_string));
        let rings = ring_records(fit, 1.3, 94.0, 27.0, 70.0, 16.0);
        assert_eq!(rings.lines().count(), 14);
        assert!(rings.lines().all(|r| r.contains("anchor=window")), "{rings}");
        assert!(rings.contains("k=prim;shape=0;") && rings.contains("k=prim;shape=6;"), "{rings}");
        assert!(rings.contains("z=3") && rings.contains("z=-3"), "some solids over the text, some under");
        for record in samples {
            assert!(!record.contains("q="), "{record}");
            let (op, keys) = glass_key_pairs(&record);
            let allowed = glass_allowed(&op);
            assert!(!allowed.is_empty(), "unknown op in {record}");
            for (key, _) in &keys {
                assert!(
                    allowed.contains(&key.as_str()),
                    "{op} key {key} is not allowed in {record}"
                );
            }
        }
        let checked = glass_checked(&scene_place(8, st, 1, 1.0, "Zg=="), 3);
        let (op, keys) = glass_key_pairs(&checked);
        assert_eq!(op, "scene");
        assert_eq!(
            keys.iter().find(|(k, _)| k == "r").map(|(_, v)| v.as_str()),
            Some("3")
        );
        assert!(checked.ends_with(";Zg=="), "{checked}");
    }

    #[test]
    fn animation_places_frame_one_and_run_does_not_use_c_or_r_as_placement() {
        let place = apc_transmit_place(5, 200, 200, 40, 20, true);
        let placed = apc_key_map(&place);
        assert_eq!(placed.get("a").map(String::as_str), Some("T"));
        assert_eq!(placed.get("i").map(String::as_str), Some("5"));
        assert_eq!(placed.get("c").map(String::as_str), Some("40"));
        assert_eq!(placed.get("r").map(String::as_str), Some("20"));
        assert_eq!(placed.get("C").map(String::as_str), Some("1"));
        let frame = apc_anim_frame(5, 200, 200, ANIM_GAP_MS, true);
        let framed = apc_key_map(&frame);
        assert_eq!(framed.get("a").map(String::as_str), Some("f"));
        assert_eq!(framed.get("z").map(String::as_str), Some("80"));
        assert!(framed.get("c").is_none(), "{frame}");
        assert!(framed.get("r").is_none(), "{frame}");
        let gap = apc_key_map(&apc_frame_gap(5, 1, ANIM_GAP_MS, true));
        assert_eq!(gap.get("a").map(String::as_str), Some("a"));
        assert_eq!(gap.get("r").map(String::as_str), Some("1"));
        assert_eq!(gap.get("z").map(String::as_str), Some("80"));
        assert!(gap.get("c").is_none());
        let run = apc_key_map(&apc_anim_run(5, 5, true));
        assert_eq!(run.get("s").map(String::as_str), Some("3"));
        assert_eq!(run.get("v").map(String::as_str), Some("5"));
        assert!(run.get("c").is_none(), "a=a c= is a frame number, not columns");
        assert!(run.get("r").is_none(), "a=a r= is a frame number, not rows");
        let loud = drop_apc_quiet(&place);
        assert!(!loud.contains("q="), "{loud}");
        assert!(loud.contains("a=T"));
        assert!(loud.contains("c=40"));
    }

    #[test]
    fn video_frames_match_the_panel_and_fit_the_byte_cap() {
        let fit = Fit {
            rows: 53,
            cols: 189,
            cell_w: 10,
            cell_h: 20,
        };
        let (w, h) = video_panel(fit);
        assert!(w.saturating_mul(h).saturating_mul(4) <= FRAME_BYTES, "{w}x{h}");
        let opened = showcase_video(w, h);
        assert!(!opened.contains("id="), "{opened}");
        let frame = frame_op(32, w, h, "Zg==");
        assert!(frame.contains(&format!("s={w}")));
        assert!(frame.contains(&format!("v={h}")));
        assert!(!frame.contains("id="));
        assert!(!progress_op(0.25).contains("id="));
    }

    /// Every frame and picture payload the sampler builds stays under the
    /// dialect caps. A frame is capped at 1 MiB on the payload bytes
    /// (`store_frame`, before base64 decode). A picture is not under that
    /// cap: glass_dialect sends it through the APC loader, which caps a
    /// picture at 64 MiB and each side at 8192. The dialect reference
    /// also keeps a picture side in 1..=4096.
    #[test]
    fn frame_and_picture_payloads_stay_under_the_dialect_caps() {
        const FRAME_CAP: usize = FRAME_BYTES as usize;
        const PICTURE_CAP: usize = 64 << 20;
        const PICTURE_SIDE: u32 = 4096;
        let fit = Fit {
            rows: 53,
            cols: 189,
            cell_w: 10,
            cell_h: 20,
        };
        let (w, h) = video_panel(fit);
        assert_eq!((w, h), (VIDEO_W, VIDEO_H));
        assert!((w as usize) * (h as usize) * 4 <= FRAME_CAP, "raw {w}x{h}");
        let opened = showcase_video(w, h);
        assert!(opened.contains("fit=contain"), "{opened}");
        assert!(opened.contains(&format!("s={w}")));
        assert!(opened.contains(&format!("v={h}")));
        assert!(!opened.contains("id="), "{opened}");
        let mut previous = Vec::new();
        for i in 0..VIDEO_FRAMES {
            let pixels = video_frame(i, VIDEO_FRAMES, w, h);
            assert_eq!(pixels.len(), (w as usize) * (h as usize) * 4);
            assert!(pixels.len() <= FRAME_CAP, "raw frame {i}");
            let payload = b64(&pixels);
            assert!(
                payload.len() <= FRAME_CAP,
                "frame payload {i} is {} bytes",
                payload.len()
            );
            let op = frame_op(32, w, h, &payload);
            assert!(op.contains(&format!("s={w}")));
            assert!(op.contains(&format!("v={h}")));
            if i > 0 {
                assert_ne!(pixels, previous, "frame {i} should move");
            }
            previous = pixels;
        }
        let (cw, ch) = chart_panel(fit);
        assert!(cw >= 1 && ch >= 1 && cw <= PICTURE_SIDE && ch <= PICTURE_SIDE, "{cw}x{ch}");
        let (rgba, _marks) = bar_chart(cw, ch);
        assert!(rgba.len() <= PICTURE_CAP, "chart picture is {} bytes", rgba.len());
        let picture = b64(&rgba);
        assert!(
            picture.len() <= PICTURE_CAP,
            "chart payload is {} bytes",
            picture.len()
        );
    }

    #[test]
    fn in_cell_caption_stays_outside_the_picture() {
        let wide = Fit {
            rows: 30,
            cols: 200,
            cell_w: 16,
            cell_h: 16,
        };
        let (c, r) = wide.cell_box(0.50, 0.62);
        let (_c, rows, caption) = in_cell_span(wide, 5, c, r);
        assert!(5 + rows <= u32::from(caption));
        assert!(caption <= wide.rows);
        let tall = Fit {
            rows: 53,
            cols: 189,
            cell_w: 10,
            cell_h: 20,
        };
        let (w, h) = tall.picture(0.50, 0.50, 480, 240);
        let (cw, ch, cap) = clamp_in_cell_pixels(tall, 5, w, h);
        assert_eq!((cw, ch), (w, h), "a fitting sixel keeps its size");
        let cells = ch.div_ceil(tall.cell_h);
        assert!(5 + cells <= u32::from(cap));
    }

    #[test]
    fn showcase_closes_only_when_one_is_open() {
        SHOWCASE_OPEN.store(false, Ordering::Relaxed);
        assert!(!SHOWCASE_OPEN.swap(false, Ordering::Relaxed));
        SHOWCASE_OPEN.store(true, Ordering::Relaxed);
        assert!(SHOWCASE_OPEN.swap(false, Ordering::Relaxed));
        assert!(!SHOWCASE_OPEN.load(Ordering::Relaxed));
        assert_eq!(showcase_close_op(), "showcase;close=1");
    }

    #[test]
    fn check_summary_counts_ok_and_names_each_error_code() {
        let mut tally = Tally::new();
        tally.sent = 4;
        tally.note_bytes(b"noise\x1b_glass;r=1;OK;");
        tally.note_bytes(b"\x1b\\");
        tally.note_bytes(b"\x1b_Gi=5;ENOENT:no such image\x1b\\");
        tally.note_bytes(b"\x1b_glass;r=2;EINVAL:unknown key q\x1b\\");
        tally.note_bytes(b"\x1b_Gi=9;OK\x1b\\");
        assert_eq!(tally.ok, 2);
        assert_eq!(tally.errors.len(), 2);
        let line = tally.summary();
        assert!(line.contains("ops sent 4"), "{line}");
        assert!(line.contains("OK 2"), "{line}");
        assert!(line.contains("ENOENT"), "{line}");
        assert!(line.contains("EINVAL"), "{line}");
        assert!(!line.contains("q="));
    }

    #[test]
    fn chart_hold_is_at_least_two_seconds() {
        assert!(HOLD_CHART >= 2000);
    }
}
