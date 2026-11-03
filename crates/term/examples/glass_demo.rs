//! A demo of glass's own dialect: a plain program writing the dialect
//! to stdout. It probes with `hello` and DA1, and — only on glass —
//! draws a short showcase: a glow under some text, a picture it
//! generates and uploads, particles, lit primitives, a small scene,
//! a few generated video frames, a chart, and a group animated for a
//! few seconds at about 30 frames per second. It removes everything
//! and restores the tty on exit, including on Ctrl+C.
//!
//! Run it inside glass:
//!
//! ```text
//! cargo run --example glass_demo
//! ```
//!
//! Only std is used: raw mode comes and goes through `stty`, so no
//! termios bindings are needed.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The tty state as `stty -g` printed it, to put back on the way out.
struct Tty(Option<String>);

impl Tty {
    /// Take the tty raw, keeping its state to restore.
    fn raw() -> Self {
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        let _ = Command::new("stty")
            .args(["raw", "-echo"])
            .stdin(Stdio::inherit())
            .status();
        Self(saved)
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        if let Some(saved) = self.0.take() {
            let _ = Command::new("stty")
                .arg(saved)
                .stdin(Stdio::inherit())
                .status();
        }
        // Show the cursor again and drop any stray styling, whatever
        // happened to the tty state itself.
        let _ = std::io::stdout().write_all(b"\x1b[?25h\x1b[0m");
        let _ = std::io::stdout().flush();
    }
}

fn out(bytes: &[u8]) {
    let mut stdout = std::io::stdout();
    stdout.write_all(bytes).unwrap();
    stdout.flush().unwrap();
}

/// Everything this dialect made, gone, and the showcase closed: what
/// the demo leaves behind on every exit path, Ctrl+C included (a
/// `panic` hook catches it, and `Drop` runs on normal return).
fn clean_up() {
    out(b"\x1b_glass;remove;all=1\x1b\\\x1b_glass;showcase;close=1\x1b\\");
}

fn main() {
    // Ctrl+C under raw mode raises SIGINT all the same: catch it so the
    // layers go and the tty comes back before the process dies.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        clean_up();
        default(info);
    }));

    let tty = Tty::raw();
    let deadline = Instant::now() + Duration::from_secs(2);

    // Probe: hello, then DA1. glass answers both, hello first (replies
    // go out in stream order). Any other terminal answers DA1 alone.
    let hello = ask_and_read("hello", deadline);
    out(b"\x1b[c");
    let da1 = read_until("\x1b[?", deadline);

    if hello.is_none() || da1.is_none() {
        eprintln!("this terminal is not glass; the demo needs glass's own dialect");
        clean_up();
        return;
    }

    let (w, h) = (48u32, 24u32);
    let rgba = weave(w, h);

    // A glow under some text, the picture, particles, primitives.
    ask(&format!("picture;id=1;f=32;s={w};v={h};{}", b64(&rgba)));
    ask("layer;id=1;k=glow;c=8844ccff;add=1;at=0,1,80,3");
    ask("layer;id=2;k=picture;p=1;at=8,4,24,12");
    ask("layer;id=3;k=particles;c=ffd080;n=120;size=2;life=3;rise=30;sway=12;at=0,20,80,4");
    ask("layer;id=4;k=prim;shape=3;c=4060ff;at=30,8,20,10");
    // A small cube, pinned to the window, drawn from an eye off the axis
    // so three faces show. The animation loop turns it a little each frame.
    let scene = br#"{"v":1,"scene":{"camera":{"eye":[2.2,1.6,2.6]},"nodes":[{"id":1,"mesh":"cube","color":[0.85,0.45,0.2,1],"lit":true}]}}"#;
    ask(&format!("scene;id=8;px=480,80,200,160;s=160;v=120;{}", b64(scene)));
    ask("showcase;id=1");
    println!("\r\rglass answered; drawing for a few seconds");

    // A group animated for a few seconds at about 30 frames per second.
    let start = Instant::now();
    let mut frames = 0u32;
    while start.elapsed() < Duration::from_secs(4) {
        let t = frames as f32 / 30.0;
        let y = (t * 8.0).sin() * 4.0 + 10.0;
        let opacity = 0.4 + 0.4 * (t * 3.0).cos();
        let record = format!("layer;id=9;k=glow;c=ff8800;o={opacity:.2};at=10,{y:.1},40,2");
        out(format!("\x1b_glass;group;g=1;n=1;{}\x1b\\", b64(record.as_bytes())).as_bytes());
        let half = t * 0.45;
        let scene = format!(
            r#"{{"v":1,"scene":{{"camera":{{"eye":[2.2,1.6,2.6]}},"nodes":[{{"id":1,"mesh":"cube","color":[0.85,0.45,0.2,1],"lit":true,"rot":[0,{sy:.4},0,{sw:.4}]}}]}}}}"#,
            sy = half.sin(),
            sw = half.cos(),
        );
        out(format!(
            "\x1b_glass;scene;id=8;px=480,80,200,160;s=160;v=120;{}\x1b\\",
            b64(scene.as_bytes())
        )
        .as_bytes());
        frames += 1;
        std::thread::sleep(Duration::from_millis(33));
    }

    // A few generated video frames, then a bar chart in the showcase.
    ask("showcase;close=1");
    ask("showcase;s=64;v=36;events=1");
    for i in 0..4u32 {
        let mut rgba = vec![0u8; 64 * 36 * 4];
        for y in 0..36 {
            for x in 0..64 {
                let on = x / 16 == i;
                let px = ((y * 64 + x) * 4) as usize;
                rgba[px] = if on { 220 } else { 30 };
                rgba[px + 1] = if on { 140 } else { 28 };
                rgba[px + 2] = if on { 70 } else { 36 };
                rgba[px + 3] = 255;
            }
        }
        ask(&format!("frame;f=32;s=64;v=36;{}", b64(&rgba)));
        ask(&format!("progress;t={:.2}", (i + 1) as f32 / 4.0));
        std::thread::sleep(Duration::from_millis(80));
    }
    let (w, h, rgba, marks) = bar_chart();
    ask(&format!("picture;id=7;f=32;s={w};v={h};{}", b64(&rgba)));
    ask(&format!("chart;p=7;n=4;{}", b64(marks.as_bytes())));
    std::thread::sleep(Duration::from_secs(2));

    clean_up();
    drop(tty);
    println!("\r\rdone: {frames} frames");
}

/// One `glass;` op, framed, with a reply token: returns the token.
fn ask(op: &str) -> u32 {
    let token = NEXT_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    // The token goes right after the op's name, never last: the payload
    // ops (`picture`, `group`) carry their payload as the last field.
    let (name, rest) = op.split_once(';').unwrap_or((op, ""));
    let fields = if rest.is_empty() { String::new() } else { format!(";{rest}") };
    out(format!("\x1b_glass;{name};r={token}{fields}\x1b\\").as_bytes());
    token
}

/// Ask and wait for this token's answer.
fn ask_and_read(op: &str, deadline: Instant) -> Option<Vec<u8>> {
    let token = ask(op);
    read_until(&format!("r={token};"), deadline)
}

/// Read from the tty until this text lands, or the deadline passes.
fn read_until(needle: &str, deadline: Instant) -> Option<Vec<u8>> {
    let needle = needle.as_bytes();
    let mut seen: Vec<u8> = Vec::new();
    let mut tty = std::io::stdin();
    while Instant::now() < deadline {
        let mut piece = [0u8; 256];
        let n = match tty.read(&mut piece) {
            Ok(0) => return None,
            Ok(n) => n,
            Err(_) => return None,
        };
        seen.extend_from_slice(&piece[..n]);
        if find(&seen, needle).is_some() {
            return Some(seen);
        }
    }
    None
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len().max(1)).position(|w| w == needle)
}

static NEXT_TOKEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// A small bar chart: four bars on a baseline, and one mark per bar.
fn bar_chart() -> (u32, u32, Vec<u8>, String) {
    let (w, h) = (64u32, 40u32);
    let mut px = [18u8, 20, 28, 255].repeat((w * h) as usize);
    for x in 4..60 {
        let i = ((32 * w + x) * 4) as usize;
        px[i..i + 4].copy_from_slice(&[176, 180, 188, 255]);
    }
    let bars = [
        (8u32, 18u32, [196u8, 134, 92, 255], "one"),
        (22, 24, [138, 172, 124, 255], "two"),
        (36, 12, [120, 160, 196, 255], "three"),
        (50, 22, [196, 160, 96, 255], "four"),
    ];
    let mut marks = String::new();
    for (x, bh, color, tip) in bars {
        let top = 32 - bh;
        for y in top..32 {
            for xx in x..x + 10 {
                let i = ((y * w + xx) * 4) as usize;
                px[i..i + 4].copy_from_slice(&color);
            }
        }
        if !marks.is_empty() {
            marks.push('\n');
        }
        marks.push_str(&format!("mark;x={x};y={top};w=10;h={bh};t={tip}"));
    }
    (w, h, px, marks)
}

/// Standard base64, no line breaks.
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A small picture worth uploading: a blue-and-gold diagonal weave.
fn weave(w: u32, h: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let d = (x + y) % 32;
            let (r, g, b) = if d < 16 {
                (40u8.saturating_add(d as u8 * 4), 96u8.saturating_sub(d as u8), 200)
            } else {
                (200, 160u8.saturating_sub((d - 16) as u8 * 4), 40)
            };
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    rgba
}
