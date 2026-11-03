//! A real, 31-second film written to glass's native terminal dialect.
//!
//! No renderer, compositor, or Rust dependencies: ffmpeg decodes the two
//! supplied films into RGB before the take. Native `picture` uploads dress
//! the video cube and panel, `scene` draws every object at 2x, and `play`
//! bends the three original glTF skins on glass's own animation clock.
//!
//! Run inside glass with --dir DIR [--log PATH]. DIR holds the three
//! three.js r170 models (Michelle.glb, Xbot.glb, Soldier.glb), two films
//! (film-a.mp4, at least 20 s, plays on the cube; film-b.mp4, at least 6 s,
//! loops on a card) and cover.png; docs/GLASS_TERM.md shows how to fetch
//! the models. SHOWOFF_START_MS can align the first frame to a recorder's
//! delayed start (Unix milliseconds).
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const FPS: f32 = 60.0;
const SECONDS: f32 = 31.0;
const W: usize = 512;
const H: usize = 288;

struct Tty(Option<String>);
impl Tty {
    fn raw() -> Self {
        let state = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
        let _ = Command::new("stty")
            .args(["raw", "-echo"])
            .stdin(Stdio::inherit())
            .status();
        Self(state)
    }
}
impl Drop for Tty {
    fn drop(&mut self) {
        out(b"\x1b_glass;remove;all=1\x1b\\\x1b[r\x1b[0m\x1b[?25h");
        if let Some(s) = self.0.take() {
            let _ = Command::new("stty").arg(s).stdin(Stdio::inherit()).status();
        }
    }
}
fn out(b: &[u8]) {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(b).expect("terminal write");
    stdout.flush().expect("terminal flush");
}
fn b64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((*c.get(1).unwrap_or(&0) as u32) << 8)
            | *c.get(2).unwrap_or(&0) as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}
fn op(s: &str, payload: Option<&[u8]>) {
    let mut b = format!("\x1b_glass;{s}");
    if let Some(p) = payload {
        b.push(';');
        b.push_str(&b64(p));
    }
    b.push_str("\x1b\\");
    out(b.as_bytes());
}
fn wait_reply(replies: &Arc<Mutex<Vec<u8>>>, token: &str) -> Result<String, String> {
    let until = Instant::now() + Duration::from_secs(8);
    loop {
        let all = String::from_utf8_lossy(&replies.lock().unwrap()).into_owned();
        if let Some(start) = all.find(&format!("r={token};")) {
            let piece = &all[start..];
            if let Some(end) = piece.find("\x1b\\") {
                let answer = piece[..end].to_string();
                return if answer.contains(";OK") {
                    Ok(answer)
                } else {
                    Err(answer)
                };
            }
        }
        if Instant::now() > until {
            return Err(format!("timed out waiting for {token}"));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
/// PNG frames keep larger video textures off the PTY's raw-RGB bandwidth path.
struct Media {
    frames: Vec<Vec<u8>>,
}
fn decode(path: &str, seek: &str, duration: &str, fps: &str) -> Result<Media, String> {
    let o = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-threads",
            "2",
            "-ss",
            seek,
            "-i",
            path,
            "-t",
            duration,
            "-vf",
            &format!("fps={fps},scale={W}:{H}:flags=lanczos"),
            "-threads",
            "2",
            "-c:v",
            "png",
            "-pred",
            "mixed",
            "-compression_level",
            "2",
            "-f",
            "image2pipe",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(format!(
            "decode {path}: {}",
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    let mut frames = Vec::new();
    let mut at = 0;
    let b = o.stdout;
    while at < b.len() {
        let begin = at;
        if b.get(at..at + 8) != Some(b"\x89PNG\r\n\x1a\n") {
            return Err("bad ffmpeg PNG signature".into());
        }
        at += 8;
        loop {
            let header = b.get(at..at + 8).ok_or("truncated ffmpeg PNG")?;
            let n = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
            let end = at
                .checked_add(n + 12)
                .filter(|&x| x <= b.len())
                .ok_or("truncated ffmpeg PNG chunk")?;
            let done = &header[4..8] == b"IEND";
            at = end;
            if done {
                break;
            }
        }
        frames.push(b[begin..at].to_vec());
    }
    if frames.is_empty() {
        return Err("ffmpeg produced no frames".into());
    }
    Ok(Media { frames })
}
fn picture(id: u32, media: &Media, frame: usize) {
    op(
        &format!("picture;id={id};f=100"),
        Some(&media.frames[frame % media.frames.len()]),
    );
}
fn ease(t: f32, begin: f32, duration: f32) -> f32 {
    let u = ((t - begin) / duration).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}
fn spring(t: f32, begin: f32, duration: f32) -> f32 {
    let u = ((t - begin) / duration).clamp(0.0, 1.0);
    if u == 0.0 || u == 1.0 {
        u
    } else {
        1.0 - (-7.5 * u).exp() * (10.0 * u).cos()
    }
}
fn q(axis: [f32; 3], angle: f32) -> String {
    let (s, c) = (angle * 0.5).sin_cos();
    format!(
        "[{:.5},{:.5},{:.5},{:.5}]",
        axis[0] * s,
        axis[1] * s,
        axis[2] * s,
        c
    )
}
fn envelope(camera: &str, nodes: &[String]) -> String {
    format!(
        r#"{{"v":1,"scene":{{"camera":{camera},"nodes":[{}]}}}}"#,
        nodes.join(",")
    )
}
/// One world, three adjoining orthographic crops. Each 640x900 crop is
/// rendered at 1280x1800, so the complete 1920x900 stage is really 2x.
/// All crops share orientation, size, geometry, clip times and depth ordering.
fn camera(t: f32, tile: usize) -> (String, f32) {
    let angle = -0.10 + 0.14 * ease(t, 4.0, 9.0) + 110.0f32.to_radians() * ease(t, 19.0, 5.8);
    let height = 4.18 - 0.18 * ease(t, 3.5, 9.0) + 0.06 * ease(t, 24.0, 2.0);
    let distance = 8.8 - 0.8 * ease(t, 4.0, 15.0);
    let (sin, cos) = angle.sin_cos();
    let offset = (tile as f32 - 1.0) * height * 640.0 / 900.0;
    let target = [cos * offset, 1.99, -sin * offset];
    let eye = [target[0] + sin * distance, 1.37, target[2] + cos * distance];
    (
        format!(
            r#"{{"eye":[{:.5},{:.5},{:.5}],"target":[{:.5},{:.5},{:.5}],"ortho":{height:.5},"near":0.1,"far":40,"clear":[0,0,0,0]}}"#,
            eye[0], eye[1], eye[2], target[0], target[1], target[2]
        ),
        angle,
    )
}
fn panel_nodes(nodes: &mut Vec<String>, t: f32, which: usize, angle: f32, warm: bool) {
    let begin = [14.0, 15.0, 16.0][which];
    let entry = spring(t, begin, 1.1);
    let leave = ease(t, 26.0 + which as f32 * 0.25, 0.85);
    let visible = warm || (t >= begin && leave < 1.0);
    // The cards occupy the same world/depth buffer as the figures.
    let at = [
        [-2.15, 2.05, -3.45],
        [2.15, 2.05, -3.70],
        [-2.10, 1.15, -3.30],
    ][which];
    // Shallow travelling rear arcs preserve parallax without crossing faces.
    let drift = 0.32 * (angle * 1.1).sin() * if which == 1 { -1.0 } else { 1.0 };
    let x = at[0] + drift;
    let (sn, cs) = angle.sin_cos();
    let position = [cs * x + sn * at[2], at[1], -sn * x + cs * at[2]];
    let id = 200 + which * 50;
    let turn = angle + [0.18, -0.18, 0.15][which] + 1.35 * (1.0 - entry) + 1.5 * leave;
    let size = (1.0 - leave).max(0.0001);
    nodes.push(format!(r#"{{"id":{id},"visible":{visible},"at":[{:.4},{:.4},{:.4}],"rot":{},"scale":[{size:.5},{size:.5},{size:.5}]}}"#,
        position[0],position[1]-3.3*(1.0-entry)+1.4*leave,position[2],q([0.0,1.0,0.0],turn)));
    if which < 2 {
        let picture = if which == 0 { 103 } else { 102 };
        nodes.push(format!(r#"{{"id":{},"parent":{id},"mesh":{{"kind":"roundcube","radius":0.004,"segments":3}},"scale":[2.43,1.367,0.014],"faces":[null,null,null,null,{picture},null],"color":[0.94,0.94,0.94,1],"edge":{{"color":[0.35,0.36,0.39,1],"glow":0}}}}"#,id+1));
    } else {
        nodes.push(format!(r#"{{"id":{},"parent":{id},"mesh":"cube","scale":[2.43,1.367,0.012],"color":[0.022,0.023,0.028,0.88]}}"#,id+1));
        nodes.push(format!(r#"{{"id":{},"parent":{id},"mesh":"cube","at":[0,-0.52,0.02],"scale":[2.12,0.006,0.012],"color":[0.32,0.34,0.37,1]}}"#,id+2));
        for i in 0..7 {
            let h = (0.22 + 0.095 * i as f32 + 0.055 * (t * 1.1 + i as f32).sin())
                * ease(t, begin + 0.2, 1.2);
            nodes.push(format!(r#"{{"id":{},"parent":{id},"mesh":{{"kind":"roundcube","radius":0.03,"segments":3}},"at":[{:.4},{:.4},0.04],"scale":[0.16,{h:.4},0.055],"color":[0.73,0.64,0.48,1],"chrome":0.35}}"#,id+10+i,-0.87+i as f32*0.29,-0.51+h*0.5));
        }
        for (k, x, y, w, h) in [
            (40, 0.0, 0.6835, 2.43, 0.006),
            (41, 0.0, -0.6835, 2.43, 0.006),
            (42, -1.215, 0.0, 0.006, 1.367),
            (43, 1.215, 0.0, 0.006, 1.367),
        ] {
            nodes.push(format!(r#"{{"id":{},"parent":{id},"mesh":"cube","at":[{x},{y},0.02],"scale":[{w},{h},0.012],"color":[0.35,0.36,0.39,1]}}"#,id+k));
        }
    }
}
fn world(t: f32, tile: usize, warm: bool) -> String {
    let (camera, angle) = camera(t, tile);
    let mut nodes = Vec::new();
    for (i, x, z, scale, begin, color) in [
        (0, 0.0, 0.0, 1.95, 3.5, "0.91,0.93,0.97"),
        (1, -1.72, 2.05, 1.28, 6.1, "0.96,0.87,0.72"),
        (2, 1.83, -2.05, 1.28, 7.5, "0.94,0.81,0.79"),
    ] {
        let entry = spring(t, begin, 1.05);
        let exit = ease(t, 27.15 + i as f32 * 0.23, 0.9);
        let visible = warm || (t >= begin && exit < 1.0);
        let yy = if i == 0 {
            (1.0 - entry).max(0.0) * 6.0 - 4.5 * exit
        } else {
            0.0
        };
        let depth = angle.sin() * x + angle.cos() * z;
        let yy = yy + 0.12 - depth * 0.0775;
        let xx = x + if i == 1 {
            -7.0 * (1.0 - entry)
        } else if i == 2 {
            7.0 * (1.0 - entry)
        } else {
            0.0
        };
        // Leave along camera right/left after the orbit, fully offscreen
        // before visibility clears. World X would carry the flankers inward.
        let departure = if i == 1 {
            -8.5
        } else if i == 2 {
            8.5
        } else {
            0.0
        };
        let xx = xx + departure * angle.cos() * exit;
        let zz = z - departure * angle.sin() * exit;
        nodes.push(format!(r#"{{"id":{},"mesh":{{"kind":"gltf","asset":{}}},"visible":{visible},"at":[{xx:.5},{yy:.5},{zz:.5}],"rot":{},"scale":[{scale},{scale},{scale}],"color":[{color},1],"chrome":1,"liquid":0.22}}"#,
            10+i,1+i,q([0.0,1.0,0.0],if i==1 {0.12} else if i==2 {-0.12} else {0.0})));
    }
    let rise = spring(t, 12.0, 1.3).min(1.012);
    let sink = ease(t, 26.7, 1.0);
    let size = 2.05 * (1.0 - sink).max(0.0001);
    let cube_depth = -3.2;
    let cube_x = cube_depth * angle.sin() + 0.12 * (t * 0.35).sin();
    let cube_z = cube_depth * angle.cos();
    nodes.push(format!(r#"{{"id":20,"visible":{},"at":[{cube_x:.5},{:.5},{cube_z:.5}],"rot":{},"scale":[{size:.5},{size:.5},{size:.5}],"mesh":{{"kind":"roundcube","radius":0.035,"segments":6}},"faces":[101,101,101,101,101,101],"color":[0.96,0.96,0.96,1],"edge":{{"color":[0.53,0.43,0.28,1],"glow":0.11}}}}"#,
        warm || (t>=12.0 && sink<1.0),3.10-6.0*(1.0-rise)-4.5*sink,
        q([0.0,1.0,0.0],0.24+angle+(t-12.0).max(0.0)*0.08)));
    for which in 0..3 {
        panel_nodes(&mut nodes, t, which, angle, warm);
    }
    for i in 0..12 {
        let k = i as f32;
        let entry = spring(t, 20.0 + k * 0.045, 1.0);
        let leave = ease(t, 26.6 + k * 0.025, 0.7);
        let a = k / 12.0 * std::f32::consts::TAU + t * 0.1;
        let scale = (0.085 + 0.015 * (k * 2.3).sin()) * entry * (1.0 - leave);
        let material = match i % 3 {
            0 => {
                r#""mesh":{"kind":"sphere","segments":28},"color":[0.83,0.86,0.9,1],"chrome":1,"liquid":0.38"#
            }
            1 => {
                r#""mesh":{"kind":"roundcube","radius":0.14,"segments":4},"color":[0.9,0.8,0.63,1],"chrome":1,"liquid":0.1"#
            }
            _ => {
                r#""mesh":{"kind":"roundcube","radius":0.1,"segments":4},"color":[0.07,0.065,0.06,1],"edge":{"color":[0.54,0.42,0.25,1],"glow":0.12}"#
            }
        };
        nodes.push(format!(r#"{{"id":{},"visible":{},"at":[{:.5},{:.5},{:.5}],"rot":{},"scale":[{scale:.5},{scale:.5},{scale:.5}],{material}}}"#,
            100+i,warm || (t>=20.0+k*0.045 && leave<1.0),a.cos()*3.0,1.6+1.45*a.sin(),-0.9+0.6*(a*1.6).sin(),q([0.6,0.64,0.48],t*0.5+k)));
    }
    envelope(&camera, &nodes)
}
fn send_scenes(t: f32, warm: bool, token: bool) {
    for tile in 0..3 {
        let r = if token {
            format!(";r=stage{tile}")
        } else {
            String::new()
        };
        op(
            &format!(
                "scene;id={};px={},70,640,900;s=1280;v=1800;z=2;o=0.97{r}",
                tile + 1,
                tile * 640
            ),
            Some(world(t, tile, warm).as_bytes()),
        );
    }
}
/// During 20..22 s every original clip receives the identical playhead,
/// from 0 to .8 seconds, inside even the shortest supplied walking clip.
fn playhead(t: f32, i: usize) -> f32 {
    if t < 20.0 {
        (t - [3.5, 6.1, 7.5][i]).max(0.0)
    } else if t < 22.0 {
        (t - 20.0) * 0.4
    } else {
        0.8 + t - 22.0
    }
}
fn pose(t: f32) {
    for scene in 1..=3 {
        for i in 0..3 {
            op(
                &format!(
                    "play;id={scene};node={};pause=1;t={:.5}",
                    10 + i,
                    playhead(t, i)
                ),
                None,
            );
        }
    }
}
fn resume_node(i: usize, time: f32) {
    for scene in 1..=3 {
        op(
            &format!("play;id={scene};node={};pause=0;t={time:.5}", 10 + i),
            None,
        );
    }
}
fn tint(line: &str) -> String {
    let mut s = String::new();
    let line = line.replace('\t', "    ");
    let line: String = line.chars().take(112).collect();
    if line.trim_start().starts_with("//") {
        return format!("\x1b[2m\x1b[38;2;138;138;148m{line}");
    }
    for word in line.split_inclusive(|c: char| !c.is_alphanumeric() && c != '_') {
        let key = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        let c = if [
            "pub", "fn", "let", "mut", "impl", "use", "struct", "enum", "match", "if", "else",
            "return", "self", "Self", "async", "await", "const", "for", "in", "Some", "None",
        ]
        .contains(&key)
        {
            "120;151;161"
        } else {
            "138;138;148"
        };
        let _ = write!(
            s,
            "\x1b[{}m\x1b[38;2;{c}m{word}",
            if c == "138;138;148" { 2 } else { 22 }
        );
    }
    s
}
struct Source {
    lines: Vec<String>,
    cursor: usize,
    rows: usize,
}
impl Source {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut lines = Vec::new();
        for file in [
            "cockpit/src/agent/tools/hands.rs",
            "crates/gpu/src/scene.rs",
        ] {
            let text = std::fs::read_to_string(root.join(file)).expect("real source file");
            lines.push(format!("// {file}"));
            lines.extend(
                text.lines()
                    .take(if file.contains("hands") { 45 } else { 180 })
                    .map(tint),
            );
            lines.push(String::new());
        }
        let size = Command::new("stty")
            .arg("size")
            .stdin(Stdio::inherit())
            .output()
            .unwrap();
        let rows = String::from_utf8_lossy(&size.stdout)
            .split_whitespace()
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(34);
        Self {
            lines,
            cursor: 0,
            rows,
        }
    }
    fn init(&mut self) {
        out(b"\x1b[?25l\x1b[48;2;5;5;6m\x1b[2J\x1b[H");
        out(b"\x1b[2;4H\x1b[38;2;164;167;176mangelX\x1b[38;2;89;93;102m  /  source\x1b[0m\x1b[48;2;5;5;6m");
        out(format!("\x1b[4;{}r\x1b[4;1H", self.rows - 3).as_bytes());
        for _ in 4..=self.rows - 3 {
            self.line();
        }
        self.prompt("");
    }
    fn line(&mut self) {
        let text = &self.lines[self.cursor % self.lines.len()];
        out(format!("\r\x1b[2K   {text}\x1b[48;2;5;5;6m\r\n").as_bytes());
        self.cursor += 1;
    }
    fn scroll(&mut self) {
        out(format!("\x1b[{};1H", self.rows - 3).as_bytes());
        self.line();
    }
    fn prompt(&self, text: &str) {
        out(format!("\x1b[{};4H\x1b[2K\x1b[22m\x1b[38;2;154;161;170m~/angelX  \x1b[38;2;112;137;146mdev\x1b[48;2;5;5;6m",self.rows-2).as_bytes());
        out(format!(
            "\x1b[{};4H\x1b[2K\x1b[22m\x1b[38;2;140;153;159m❯ \x1b[38;2;210;211;219m{text}\x1b[48;2;5;5;6m",
            self.rows - 1
        )
        .as_bytes());
    }
}
fn run() -> Result<(), String> {
    let mut dir = PathBuf::from("showoff");
    let mut log = None::<PathBuf>;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dir" => dir = args.next().ok_or("--dir needs a path")?.into(),
            "--log" => log = Some(args.next().ok_or("--log needs a path")?.into()),
            _ => return Err(format!("unknown option {a}")),
        }
    }
    let log = log.unwrap_or_else(|| dir.join("replies.log"));
    let media = |name: &str| -> Result<String, String> {
        let path = dir.join(name);
        if !path.is_file() {
            return Err(format!("missing {}", path.display()));
        }
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{} is not UTF-8", path.display()))
    };
    let (film_a, film_b, cover_png) = (
        media("film-a.mp4")?,
        media("film-b.mp4")?,
        media("cover.png")?,
    );
    let still_time = std::env::var("SHOWOFF_REVIEW_TIME")
        .ok()
        .and_then(|x| x.parse::<f32>().ok());
    let _tty = Tty::raw();
    let replies = Arc::new(Mutex::new(Vec::new()));
    let thread_replies = Arc::clone(&replies);
    std::thread::spawn(move || {
        let mut b = [0; 4096];
        let mut input = std::io::stdin();
        while let Ok(n) = input.read(&mut b) {
            if n == 0 {
                break;
            }
            thread_replies.lock().unwrap().extend_from_slice(&b[..n]);
        }
    });
    let mut report = String::new();
    for (i, file) in ["Michelle.glb", "Xbot.glb", "Soldier.glb"]
        .iter()
        .enumerate()
    {
        let path = dir.join(file);
        let token = format!("asset{i}");
        op(
            &format!("asset;id={};t=f;r={token}", i + 1),
            Some(path.to_str().ok_or("model path is not UTF-8")?.as_bytes()),
        );
        let answer = wait_reply(&replies, &token)?;
        writeln!(report, "{file}: {answer}").unwrap();
        std::fs::write(&log, &report).map_err(|e| e.to_string())?;
    }
    let delve = decode(&film_a, "0", "20", "30")?;
    let hero = decode(&film_b, "0", "6", "60")?;
    let cover = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            &cover_png,
            "-vf",
            "scale=640:360:flags=lanczos",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !cover.status.success() || cover.stdout.len() != 640 * 360 * 3 {
        return Err("cover decode failed".into());
    }
    picture(101, &delve, 0);
    picture(102, &hero, 0);
    op("picture;id=103;f=24;s=640;v=360", Some(&cover.stdout));
    // Prewarm actual geometry/pipelines before the recorder's delayed start.
    send_scenes(22.0, true, true);
    for tile in 0..3 {
        wait_reply(&replies, &format!("stage{tile}"))?;
    }
    for scene in 1..=3 {
        for (node, clip) in [(10, 0), (11, 6), (12, 3)] {
            op(
                &format!(
                    "play;id={scene};node={node};clip={clip};pause=1;t=0;r=play{scene}-{node}"
                ),
                None,
            );
            wait_reply(&replies, &format!("play{scene}-{node}"))?;
        }
    }
    std::thread::sleep(Duration::from_millis(500));
    send_scenes(0.0, false, false);
    let mut source = Source::new();
    source.init();
    if let Ok(ms) = std::env::var("SHOWOFF_START_MS") {
        let ms: u128 = ms.parse().map_err(|_| "invalid SHOWOFF_START_MS")?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        if now > ms {
            return Err(format!("prewarm missed film start by {} ms", now - ms));
        }
        std::thread::sleep(Duration::from_millis((ms - now) as u64));
    }
    let start = Instant::now();
    let mut last_scroll = 0;
    let mut last_d = usize::MAX;
    let mut last_h = usize::MAX;
    let mut last_type = usize::MAX;
    let phrase = "glass — any media, over any terminal.";
    let mut sent = 0u32;
    let mut late = 0u32;
    for frame in 0..(SECONDS * FPS) as u32 {
        let due = Duration::from_secs_f64(frame as f64 / FPS as f64);
        if let Some(rest) = due.checked_sub(start.elapsed()) {
            std::thread::sleep(rest)
        } else if frame > 0 && start.elapsed().saturating_sub(due) > Duration::from_millis(16) {
            late += 1;
        }
        let t = still_time.unwrap_or(frame as f32 / FPS);
        out(b"\x1b[?2026h");
        if t < 28.6 {
            let line = (t / 0.48) as usize;
            if line != last_scroll {
                source.scroll();
                last_scroll = line;
            }
        }
        if (12.0..27.7).contains(&t) {
            let f = ((t - 12.0) * 30.0) as usize;
            if f != last_d {
                picture(101, &delve, f);
                last_d = f;
            }
        }
        if (15.0..27.2).contains(&t) {
            let f = ((t - 15.0) * 60.0) as usize;
            if f != last_h {
                picture(102, &hero, f);
                last_h = f;
            }
        }
        if (3.5..28.8).contains(&t) {
            send_scenes(t, false, false);
            // Native continuous playback outside the exact 2 s common-time cue.
            for (i, begin) in [210, 366, 450].iter().enumerate() {
                if frame == *begin {
                    resume_node(i, 0.0);
                }
            }
            if (20.0..22.0).contains(&t) || still_time.is_some() {
                pose(t);
            }
            if frame == 1320 {
                for i in 0..3 {
                    resume_node(i, 0.8);
                }
            }
            sent += 1;
        }
        if frame == 1728 && still_time.is_none() {
            op("remove;id=1-3", None);
        }
        if t >= 28.8 {
            let count = (((t - 28.8) / 0.7) * phrase.chars().count() as f32) as usize;
            let count = count.min(phrase.chars().count());
            if count != last_type {
                source.prompt(&phrase.chars().take(count).collect::<String>());
                last_type = count;
            }
        }
        out(b"\x1b[?2026l");
    }
    writeln!(
        report,
        "sent {sent} scene sets; {late} producer deadlines more than 16 ms late; elapsed {:.3}s",
        start.elapsed().as_secs_f64()
    )
    .unwrap();
    let all = String::from_utf8_lossy(&replies.lock().unwrap()).replace('\x1b', "^[");
    writeln!(report, "{all}").unwrap();
    std::fs::write(log, report).map_err(|e| e.to_string())?;
    // Hold until the recorder is closed, including a little launch jitter.
    std::thread::sleep(Duration::from_secs(4));
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("glass_showoff: {e}");
        std::process::exit(1)
    }
}
