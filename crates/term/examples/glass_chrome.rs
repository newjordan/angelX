//! Liquid chrome in glass's own dialect: skinned glTF figures dancing and
//! walking in chrome, a ring of chrome, liquid and glowing shapes round
//! them, and a rounded cube with a picture on each face and glowing
//! edges, the whole scene sent again every display frame.
//!
//! Run it inside glass, with the figures' `.glb` files in a directory:
//!
//! ```text
//! cargo run --release --example glass_chrome -- --dir ~/models/mixamo
//! ```
//!
//! It looks there for `Michelle.glb`, `Soldier.glb`, `Xbot.glb` and
//! `man.glb` and plays a clip on each it finds (a figure that is missing
//! is left out). Options:
//!
//! - `--seconds S` how long to run (20);
//! - `--size WxH` the scene's pixels and its place in the window
//!   (1920x1080);
//! - `--extras N` the shapes round the figures (60);
//! - `--figures N` how many figures (3; 4 adds `man.glb`);
//! - `--chrome C`, `--liquid L` the figures' finish (1, 0.35);
//! - `--near` the figures closer;
//! - `--solo FILE` that figure alone, close up, `--clip N` its clip (0);
//! - `--cube` the rounded cube close up instead, with no figures and a
//!   dozen shapes round it unless told otherwise;
//! - `--log PATH` writes glass's replies there.
//!
//! Only std is used: raw mode comes and goes through `stty`.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The tty raw, so glass's replies are not echoed; put back on drop.
struct Tty(Option<String>);

impl Tty {
    fn raw() -> Self {
        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        let _ = Command::new("stty").args(["raw", "-echo"]).stdin(Stdio::inherit()).status();
        Self(saved)
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        out(b"\x1b_glass;remove;all=1\x1b\\\x1b[?25h");
        if let Some(saved) = self.0.take() {
            let _ = Command::new("stty").arg(saved).stdin(Stdio::inherit()).status();
        }
    }
}

fn out(bytes: &[u8]) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(bytes);
    let _ = stdout.flush();
}

/// One op of the dialect, its payload (if any) base64 last.
fn op(body: &str, payload: Option<&[u8]>) {
    match payload {
        Some(p) => out(format!("\x1b_glass;{body};{}\x1b\\", b64(p)).as_bytes()),
        None => out(format!("\x1b_glass;{body}\x1b\\").as_bytes()),
    }
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

struct Options {
    seconds: f32,
    size: (u32, u32),
    extras: usize,
    figures: usize,
    chrome: f32,
    liquid: f32,
    cube: bool,
    near: bool,
    solo: Option<String>,
    clip: u32,
    dir: std::path::PathBuf,
    log: Option<std::path::PathBuf>,
}

fn options() -> Options {
    let mut o = Options {
        seconds: 20.0,
        size: (1920, 1080),
        extras: 60,
        figures: 3,
        chrome: 1.0,
        liquid: 0.35,
        cube: false,
        near: false,
        solo: None,
        clip: 0,
        dir: std::env::var_os("GLASS_MIXAMO").map_or_else(
            || std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join("models/mixamo"),
            Into::into,
        ),
        log: None,
    };
    let mut args = std::env::args().skip(1);
    let (mut extras, mut figures) = (None, None);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match arg.as_str() {
            "--seconds" => o.seconds = value().parse().unwrap_or(o.seconds),
            "--size" => {
                let v = value();
                if let Some((w, h)) = v.split_once('x')
                    && let (Ok(w), Ok(h)) = (w.parse(), h.parse())
                {
                    o.size = (w, h);
                }
            }
            "--extras" => extras = value().parse().ok(),
            "--figures" => figures = value().parse().ok(),
            "--chrome" => o.chrome = value().parse().unwrap_or(o.chrome),
            "--liquid" => o.liquid = value().parse().unwrap_or(o.liquid),
            "--cube" => o.cube = true,
            "--near" => o.near = true,
            "--solo" => o.solo = Some(value()),
            "--clip" => o.clip = value().parse().unwrap_or(0),
            "--dir" => o.dir = value().into(),
            "--log" => o.log = Some(value().into()),
            _ => {}
        }
    }
    o.extras = extras.unwrap_or(if o.cube { 12 } else { o.extras });
    o.figures = figures.unwrap_or(if o.cube { 0 } else { o.figures });
    o
}

/// A figure: its file, the clip it plays, where it stands, its chrome's
/// tint.
struct Figure {
    file: &'static str,
    clip: u32,
    x: f32,
    tint: [f32; 4],
}

const FIGURES: [Figure; 4] = [
    Figure { file: "Xbot.glb", clip: 6, x: -1.9, tint: [0.93, 0.95, 1.0, 1.0] },
    Figure { file: "Michelle.glb", clip: 0, x: 0.0, tint: [1.0, 0.8, 0.46, 1.0] },
    Figure { file: "Soldier.glb", clip: 3, x: 1.9, tint: [1.0, 0.66, 0.6, 1.0] },
    // Its clips: ChickenDance, Dance, DancingAnt, HippoBallerina, Idle,
    // Lambada, Pirouettes, Salsa, Walk, Whirl.
    Figure { file: "man.glb", clip: 7, x: 3.6, tint: [0.7, 0.95, 0.85, 1.0] },
];

/// A face's picture: a die's face of `pips`, on a gradient of its own
/// hue, `side` pixels square, RGBA.
fn die(pips: u32, side: u32) -> Vec<u8> {
    let hue = pips as f32 / 6.0;
    let spots: &[(f32, f32)] = match pips {
        1 => &[(0.5, 0.5)],
        2 => &[(0.27, 0.27), (0.73, 0.73)],
        3 => &[(0.25, 0.25), (0.5, 0.5), (0.75, 0.75)],
        4 => &[(0.27, 0.27), (0.73, 0.27), (0.27, 0.73), (0.73, 0.73)],
        5 => &[(0.25, 0.25), (0.75, 0.25), (0.5, 0.5), (0.25, 0.75), (0.75, 0.75)],
        _ => &[(0.28, 0.22), (0.72, 0.22), (0.28, 0.5), (0.72, 0.5), (0.28, 0.78), (0.72, 0.78)],
    };
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let (u, v) = ((x as f32 + 0.5) / side as f32, (y as f32 + 0.5) / side as f32);
            let [r, g, b] = hsv(hue + 0.08 * v, 0.75, 0.55 + 0.4 * (1.0 - v));
            let inside = spots.iter().any(|(sx, sy)| (u - sx).powi(2) + (v - sy).powi(2) < 0.011);
            let frame = u < 0.04 || v < 0.04 || u > 0.96 || v > 0.96;
            let c = if inside || frame { [0.98, 0.98, 0.96] } else { [r, g, b] };
            rgba.extend(c.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8));
            rgba.push(255);
        }
    }
    rgba
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let f = h - h.floor();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match h as u32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A quaternion turning `angle` about `axis` (unit).
fn turn(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let (s, c) = (angle * 0.5).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

fn quat(q: [f32; 4]) -> String {
    format!("[{:.4},{:.4},{:.4},{:.4}]", q[0], q[1], q[2], q[3])
}

fn rgba(c: [f32; 4]) -> String {
    format!("[{:.3},{:.3},{:.3},{:.3}]", c[0], c[1], c[2], c[3])
}

/// The scene at `t` seconds: the figures, the rounded cube, and `extras`
/// shapes round them.
fn scene(o: &Options, figures: &[(u32, &Figure)], t: f32) -> String {
    let mut nodes: Vec<String> = Vec::new();
    let edge = hsv(t * 0.15, 0.85, 1.0);
    let glow = 0.65 + 0.35 * (t * 2.3).sin().abs();
    let (camera, cube_at, cube_scale) = if o.cube {
        (r#"{"eye":[0,0.25,3.1],"target":[0,0,0],"fov":0.7,"clear":[0.015,0.015,0.025,1]}"#, [0.0, 0.0, 0.0], 1.25)
    } else if o.solo.is_some() {
        (r#"{"eye":[0,1.0,3.9],"target":[0,0.9,0],"fov":0.62,"clear":[0.015,0.015,0.025,1]}"#, [0.0, 2.75, -1.2], 0.8)
    } else if o.near {
        (r#"{"eye":[0,1.3,3.7],"target":[0,0.95,0],"fov":0.62,"clear":[0.015,0.015,0.025,1]}"#, [0.0, 2.75, -1.2], 0.8)
    } else {
        (r#"{"eye":[0,1.65,6.4],"target":[0,1.05,0],"fov":0.72,"clear":[0.015,0.015,0.025,1]}"#, [0.0, 2.75, -1.2], 0.8)
    };
    // The floor.
    if !o.cube {
        nodes.push(r#"{"id":1,"mesh":"plane","at":[0,0,0],"scale":[16,1,9],"color":[0.07,0.07,0.09,1],"lit":true}"#.into());
    }
    // The rounded cube: a die's pictures on its faces, its edges glowing
    // in a colour that walks round the wheel.
    let spin = turn([0.36, 0.86, 0.36], t * 0.5);
    nodes.push(format!(
        r#"{{"id":2,"at":[{:.3},{:.3},{:.3}],"rot":{},"scale":[{s},{s},{s}],"mesh":{{"kind":"roundcube","radius":0.14,"segments":6}},"lit":true,"color":[1,1,1,1],"faces":[1,2,3,4,5,6],"edge":{{"color":[{:.3},{:.3},{:.3},1],"glow":{glow:.3}}}}}"#,
        cube_at[0],
        cube_at[1] + if o.cube { 0.0 } else { 0.12 * (t * 1.3).sin() },
        cube_at[2],
        quat(spin),
        edge[0],
        edge[1],
        edge[2],
        s = cube_scale,
    ));
    for (asset, f) in figures {
        nodes.push(format!(
            r#"{{"id":{},"at":[{:.2},0,0],"mesh":{{"kind":"gltf","asset":{asset}}},"color":{},"chrome":{:.3},"liquid":{:.3}}}"#,
            10 + asset,
            f.x,
            rgba(f.tint),
            o.chrome,
            o.liquid
        ));
    }
    // The ring: liquid chrome balls, plain lit cubes, chrome rounded cubes
    // and cubes with glowing edges, orbiting at their own heights.
    for i in 0..o.extras {
        let k = i as f32;
        let n = o.extras.max(1) as f32;
        let a = k / n * std::f32::consts::TAU + t * (0.12 + 0.03 * (i % 3) as f32);
        let radius = 3.6 + 0.9 * (k * 1.7).sin();
        let (x, z) = (a.sin() * radius, a.cos() * radius - 1.0);
        let y = 0.4 + 2.4 * (0.5 + 0.5 * (k * 2.3).sin()) + 0.15 * (t * 1.1 + k).sin();
        let s = 0.18 + 0.1 * (0.5 + 0.5 * (k * 3.1).cos());
        let rot = quat(turn([0.6, 0.64, 0.48], t * (0.7 + 0.2 * (i % 4) as f32) + k));
        let c = hsv(k / n, 0.55, 1.0);
        let node = match i % 4 {
            0 => r#""mesh":{"kind":"sphere","segments":32},"color":[0.92,0.94,1,1],"chrome":1,"liquid":0.8"#.to_string(),
            1 => format!(r#""mesh":"cube","lit":true,"color":[{:.3},{:.3},{:.3},1]"#, c[0], c[1], c[2]),
            2 => format!(r#""mesh":{{"kind":"roundcube","radius":0.2}},"color":[{:.3},{:.3},{:.3},1],"chrome":1"#, c[0], c[1], c[2]),
            _ => {
                let e = hsv(k / n + 0.5, 0.9, 1.0);
                format!(
                    r#""mesh":{{"kind":"roundcube","radius":0.1,"segments":3}},"lit":true,"color":[0.12,0.12,0.14,1],"edge":{{"color":[{:.3},{:.3},{:.3},1],"glow":0.8}}"#,
                    e[0], e[1], e[2]
                )
            }
        };
        nodes.push(format!(
            r#"{{"id":{},"at":[{x:.3},{y:.3},{z:.3}],"rot":{rot},"scale":[{s:.3},{s:.3},{s:.3}],{node}}}"#,
            100 + i
        ));
    }
    format!(r#"{{"v":1,"scene":{{"camera":{camera},"nodes":[{}]}}}}"#, nodes.join(","))
}

fn main() {
    let o = options();
    let tty = Tty::raw();
    // glass's replies, kept for --log.
    let replies = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    {
        let replies = std::sync::Arc::clone(&replies);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut stdin = std::io::stdin();
            while let Ok(n) = stdin.read(&mut buf) {
                if n == 0 {
                    break;
                }
                if let Ok(mut r) = replies.lock() {
                    r.extend_from_slice(&buf[..n]);
                }
            }
        });
    }
    out(b"\x1b[?25l\x1b[2J");
    // The faces' pictures, ids 1..6.
    for pips in 1..=6u32 {
        op(&format!("picture;id={pips};f=32;s=128;v=128;r=p{pips}"), Some(&die(pips, 128)));
    }
    // The figures, read by glass from their files.
    let mut figures = Vec::new();
    let solo = o.solo.as_ref().map(|file| {
        let f = FIGURES.iter().find(|f| f.file == file.as_str()).map_or(FIGURES[3].tint, |f| f.tint);
        Figure { file: Box::leak(file.clone().into_boxed_str()), clip: o.clip, x: 0.0, tint: f }
    });
    let chosen: Vec<&Figure> = match &solo {
        Some(f) => vec![f],
        None => FIGURES.iter().take(o.figures).collect(),
    };
    for (k, f) in chosen.into_iter().enumerate() {
        let path = o.dir.join(f.file);
        if !path.is_file() {
            continue;
        }
        let asset = k as u32 + 1;
        op(&format!("asset;id={asset};t=f;r=a{asset}"), Some(path.to_string_lossy().as_bytes()));
        figures.push((asset, f));
    }
    let (w, h) = o.size;
    let place = format!("scene;id=1;px=0,0,{w},{h};s={w};v={h};z=1");
    op(&format!("{place};r=s"), Some(scene(&o, &figures, 0.0).as_bytes()));
    for (asset, f) in &figures {
        op(&format!("play;id=1;node={};clip={};r=c{asset}", 10 + asset, f.clip), None);
    }
    // Every display frame: the scene again, as a program that moves
    // everything itself would send it.
    let start = Instant::now();
    let frame = Duration::from_micros(16_667);
    let mut next = start;
    let mut frames = 0u64;
    while start.elapsed().as_secs_f32() < o.seconds {
        let t = start.elapsed().as_secs_f32();
        op(&place, Some(scene(&o, &figures, t).as_bytes()));
        frames += 1;
        next += frame;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    if let Some(path) = &o.log
        && let Ok(r) = replies.lock()
    {
        let text = String::from_utf8_lossy(&r).replace('\x1b', "^[");
        let secs = start.elapsed().as_secs_f32();
        let _ = std::fs::write(path, format!("sent {frames} scenes in {secs:.1} s ({:.1} a second)\n{text}\n", frames as f32 / secs));
    }
    drop(tty);
}
