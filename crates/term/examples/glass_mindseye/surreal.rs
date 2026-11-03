//! `--surreal`: the mind's eye turned on its head.
//!
//! The same show, invaded. Bowler-hatted men drift down through the field
//! in rows, big lit solids float past, giant characters turn in the dark,
//! pizza slices tumble through a 3D scene beside cubes whose faces play
//! live video (ffmpeg's own generators: a Mandelbrot zoom, life, a
//! cellular automaton, Sierpinski), eyeballs roll, clocks melt and run
//! backwards. Text streams cross the screen in every direction, in front
//! of the geometry and behind it, and the eye watches the pizza. Later
//! the terminal's own text rains down as the Matrix, red cables crawl in
//! toward the eye pulsing, it weeps light, and glitch bands tear the
//! picture: techno gore.
//!
//! Everything here is sent the way the rest of the show is: layers in the
//! frame's groups, one 3D scene op a frame, video frames as picture
//! uploads under fixed ids, and the rain as plain terminal text.

use std::f32::consts::{PI, TAU};
use std::fmt::Write as _;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use super::{ByteWriter, Canvas, H, W, b64, envelope, gradient, hex, mix, smooth};

/// Program picture ids this show uploads.
const PIZZA: u32 = 900;
const EYEBALL: u32 = 901;
const BALLOON_FACE: u32 = 902;
/// The cast: Mixamo characters glass reads from their files, by asset id.
/// Michelle (samba), Soldier (walk), X Bot (nods, head shakes) and the
/// star, a low-poly Mixamo-rigged man exported from Blender, dancing seven
/// dances retargeted from the CMU motion capture database.
const CAST: [(u32, &str); 4] = [(1, "Michelle.glb"), (2, "Soldier.glb"), (3, "Xbot.glb"), (4, "man.glb")];
/// Whether the glass on the other end takes `chrome` and `liquid` on nodes.
const CHROME: bool = true;
/// The man's clips, as man.glb lists them.
const CHICKEN: u32 = 0;
const DANCING_ANT: u32 = 2;
const HIPPO_BALLERINA: u32 = 3;
const LAMBADA: u32 = 5;
const PIROUETTES: u32 = 6;
const SALSA: u32 = 7;
const WALK: u32 = 8;
const WHIRL: u32 = 9;
/// Whether it takes the `roundcube` mesh with `faces` and a glowing `edge`.
const ROUNDCUBE: bool = true;
const VIDEO_BASE: u32 = 910;
const TEXT_BASE: u32 = 1000;
/// The 3D scene's program id.
const SCENE: u32 = 60;
/// The scene behind the sacred geometry, for what looms at the back.
const SCENE_BACK: u32 = 61;
const VIDEO_SIDE: u32 = 160;
/// ffmpeg's own generators, one a video.
const VIDEOS: [&str; 4] = [
    "mandelbrot=s=160x160:rate=30",
    "life=s=160x160:rate=30:mold=10:ratio=0.12:life_color=0x00ff66:death_color=0x880022",
    "cellauto=s=160x160:rate=30:rule=110",
    "sierpinski=s=160x160:rate=30",
];
/// The scene's camera: on +Z looking at the middle, this field of view.
const CAMERA_Z: f32 = 9.0;
const FOV: f32 = 0.8;

/// Text that streams across, and its colour.
const STRIPS: [(&str, &str); 24] = [
    ("fn main() { let eye = Eye::open(); loop { eye.see(); } }", "#33ff77"),
    ("SEGFAULT IN THE THIRD EYE", "#ff2244"),
    ("0xDEADBEEF 0xCAFEBABE 0x8BADF00D 0xFEEDFACE", "#00e5ff"),
    ("sudo rm -rf /reality", "#ff3fa4"),
    ("[  OK  ] Reached target Mind's Eye.", "#33ff77"),
    ("PIZZA.EXE HAS STOPPED RESPONDING", "#ffc861"),
    ("the grid remembers everything you forgot", "#c8d4ff"),
    ("ESC [ ? 2 0 2 6 h", "#00e5ff"),
    ("01001101 01001001 01001110 01000100", "#33ff77"),
    ("WAKE UP", "#ffffff"),
    ("follow the white rabbit", "#33ff77"),
    ("flesh.dll not found", "#ff2244"),
    ("kernel: eye0: link up, 10000 Mbps, full duplex", "#8a5cff"),
    ("every stroke is an escape code", "#00e5ff"),
    ("TECHNO GORE", "#ff2244"),
    ("while true; do glass --see; done", "#33ff77"),
    ("ｦｱｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃ", "#33ff77"),
    ("NO CARRIER", "#ffffff"),
    ("the pizza is watching back", "#ffc861"),
    ("dmesg | grep soul", "#8a5cff"),
    ("ERROR: too many dimensions", "#ff2244"),
    ("<<< SIGNAL LOST >>>", "#ffc861"),
    ("I AM THE TERMINAL", "#00e5ff"),
    ("10,847 layers a frame and still hungry", "#ff3fa4"),
];
/// Giant characters, drifting.
const GLYPHS: [(&str, &str, &str); 6] = [
    ("眼", "Noto-Sans-CJK-JP-Bold", "#ff2244"),
    ("∞", "Berkeley-Mono-Regular", "#00e5ff"),
    ("λ", "Berkeley-Mono-Regular", "#33ff77"),
    ("Ω", "Berkeley-Mono-Regular", "#ffc861"),
    ("夢", "Noto-Sans-CJK-JP-Bold", "#8a5cff"),
    ("目", "Noto-Sans-CJK-JP-Bold", "#ff3fa4"),
];

/// A small, steady hash: the same input, the same 0..1 every frame.
fn hash(a: u32, b: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// A picture op for straight RGBA under `id`.
pub fn upload(out: &mut Vec<u8>, id: u32, width: u32, height: u32, rgba: &[u8]) {
    let _ = write!(ByteWriter(out), "\x1b_glass;picture;id={id};f=32;s={width};v={height};{}\x1b\\", b64(rgba));
}

/// Text drawn by ImageMagick into RGBA (PAM out), or nothing without it.
pub fn render_text(text: &str, font: &str, points: u32, color: &str) -> Option<(u32, u32, Vec<u8>)> {
    let out = Command::new("magick")
        .args(["-background", "none", "-fill", color, "-font", font, "-pointsize", &points.to_string()])
        .arg(format!("label:{text}"))
        .args(["-trim", "+repage", "-depth", "8", "pam:-"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let bytes = out.stdout;
    let end = bytes.windows(7).position(|w| w == b"ENDHDR\n")? + 7;
    let header = String::from_utf8_lossy(&bytes[..end]);
    let field = |name: &str| -> Option<u32> {
        header.lines().find_map(|l| l.strip_prefix(name)).and_then(|v| v.trim().parse().ok())
    };
    let (width, height, depth) = (field("WIDTH ")?, field("HEIGHT ")?, field("DEPTH ")?);
    if field("MAXVAL ")? != 255 {
        return None;
    }
    let body = &bytes[end..];
    let pixels = (width * height) as usize;
    if body.len() < pixels * depth as usize || width == 0 || height == 0 || width > 4096 || height > 4096 {
        return None;
    }
    let rgba = match depth {
        4 => body[..pixels * 4].to_vec(),
        2 => body[..pixels * 2].chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        _ => return None,
    };
    Some((width, height, rgba))
}

/// A pizza slice's top: cheese, pepperoni, basil, and the crust where the
/// slice is widest. The slice's apex is at (0.5, 0.02), its axis down.
fn pizza_texture() -> Vec<u8> {
    let side = 256usize;
    let pepperoni = [(0.0f32, 0.35f32), (-0.18, 0.62), (0.2, 0.7), (0.02, 0.82), (-0.05, 0.55)];
    let mut rgba = vec![0u8; side * side * 4];
    for py in 0..side {
        for px in 0..side {
            let (u, v) = (px as f32 / side as f32, py as f32 / side as f32);
            let (x, y) = ((u - 0.5) * 2.0, (v - 0.02) / 0.9);
            let r = (x * x + y * y).sqrt();
            let noise = hash(px as u32, py as u32);
            let mut c = if r > 0.88 {
                // Crust, browned unevenly.
                mix(hex("#c98a3a"), hex("#7a4416"), noise * 0.6 + 0.2 * ((r - 0.88) * 30.0).sin().abs())
            } else {
                let melt = 0.5 + 0.5 * (x * 9.0 + (y * 13.0).sin() * 2.0).sin();
                mix(hex("#ffcf4a"), hex("#ff9d2a"), melt * 0.5 + noise * 0.25)
            };
            for &(cx, cy) in &pepperoni {
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                if d < 0.095 && r < 0.86 {
                    c = mix(hex("#a3141c"), hex("#5c070c"), noise * 0.5 + d * 3.0);
                    if hash(px as u32 + 7, py as u32) > 0.93 {
                        c = hex("#e8b07a");
                    }
                }
            }
            if r < 0.85 && hash(px as u32 / 6, py as u32 / 6) > 0.985 {
                c = hex("#2f8a2f");
            }
            let i = (py * side + px) * 4;
            rgba[i..i + 4].copy_from_slice(&[(c.0 * 255.0) as u8, (c.1 * 255.0) as u8, (c.2 * 255.0) as u8, 255]);
        }
    }
    rgba
}

/// An eyeball, wrapped round a sphere: white, red veins, an iris and a
/// pupil where u is a half and v a half.
fn eyeball_texture() -> Vec<u8> {
    let (w, h) = (256usize, 128usize);
    let mut rgba = vec![0u8; w * h * 4];
    for py in 0..h {
        for px in 0..w {
            let (u, v) = (px as f32 / w as f32, py as f32 / h as f32);
            let (dx, dy) = ((u - 0.5) * 2.0, (v - 0.5) * 2.0);
            let d = (dx * dx + dy * dy).sqrt();
            let vein = ((u * 40.0 + (v * 23.0).sin() * 3.0).sin() * (v * 31.0 + (u * 17.0).cos() * 2.0).sin()).abs();
            let mut c = mix(hex("#f4eee6"), hex("#d9c7b8"), d * 0.4);
            if vein > 0.985 && d > 0.3 {
                c = hex("#b0202a");
            }
            if d < 0.32 {
                let a = dy.atan2(dx);
                let fibre = 0.5 + 0.5 * (a * 24.0).sin();
                c = mix(hex("#1fb8a8"), hex("#0d4a6e"), d / 0.32 * 0.6 + fibre * 0.3);
            }
            if d < 0.14 {
                c = hex("#050508");
            }
            if ((dx + 0.08).powi(2) + (dy + 0.1).powi(2)).sqrt() < 0.04 {
                c = hex("#ffffff");
            }
            let i = (py * w + px) * 4;
            rgba[i..i + 4].copy_from_slice(&[(c.0 * 255.0) as u8, (c.1 * 255.0) as u8, (c.2 * 255.0) as u8, 255]);
        }
    }
    rgba
}

/// A pizza slice as an inline mesh: apex at the origin's side, the crust
/// arc beyond, a top that shows the picture, a crust round the rest.
fn pizza_mesh() -> String {
    let (segments, span, thick) = (8usize, 0.8f32, 0.12f32);
    let (mut pos, mut uv, mut nrm, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    // The slice's middle sits on the origin: shift it back along its axis.
    let shift = 0.6;
    let arc: Vec<(f32, f32)> = (0..=segments)
        .map(|i| {
            let a = -span / 2.0 + span * i as f32 / segments as f32;
            (a.sin(), a.cos())
        })
        .collect();
    let mut face = |points: &[[f32; 3]], uvs: &[[f32; 2]], n: [f32; 3], tri: &[[usize; 3]]| {
        let base = pos.len();
        pos.extend_from_slice(points);
        uv.extend_from_slice(uvs);
        nrm.extend(std::iter::repeat_n(n, points.len()));
        for t in tri {
            idx.extend([base + t[0], base + t[1], base + t[2]]);
        }
    };
    for (z, n, flip) in [(thick / 2.0, [0.0, 0.0, 1.0], false), (-thick / 2.0, [0.0, 0.0, -1.0], true)] {
        let mut points = vec![[0.0, -shift, z]];
        let mut uvs = vec![[0.5, 0.02]];
        for &(x, y) in &arc {
            points.push([x, y - shift, z]);
            // The bottom is all crust.
            uvs.push(if flip { [0.5, 0.97] } else { [0.5 + x * 0.5, 0.02 + y * 0.9] });
        }
        if flip {
            uvs[0] = [0.5, 0.97];
        }
        let tri: Vec<[usize; 3]> =
            (1..=segments).map(|i| if flip { [0, i + 1, i] } else { [0, i, i + 1] }).collect();
        face(&points, &uvs, n, &tri);
    }
    // The crust's rim and the two cut sides.
    let mut edges: Vec<((f32, f32), (f32, f32))> = arc.windows(2).map(|w| (w[0], w[1])).collect();
    edges.push(((0.0, 0.0), arc[0]));
    edges.push((arc[segments], (0.0, 0.0)));
    for ((x0, y0), (x1, y1)) in edges {
        let (nx, ny) = (y1 - y0, -(x1 - x0));
        let l = (nx * nx + ny * ny).sqrt().max(1e-6);
        let n = [nx / l, ny / l, 0.0];
        let points = [
            [x0, y0 - shift, thick / 2.0],
            [x1, y1 - shift, thick / 2.0],
            [x1, y1 - shift, -thick / 2.0],
            [x0, y0 - shift, -thick / 2.0],
        ];
        face(&points, &[[0.5, 0.97]; 4], n, &[[0, 2, 1], [0, 3, 2]]);
    }
    let f = |v: &[f32]| v.iter().map(|x| format!("{x:.4}")).collect::<Vec<_>>().join(",");
    let triples = |v: &[[f32; 3]]| v.iter().map(|p| format!("[{}]", f(p))).collect::<Vec<_>>().join(",");
    let pairs = |v: &[[f32; 2]]| v.iter().map(|p| format!("[{}]", f(p))).collect::<Vec<_>>().join(",");
    format!(
        r#"{{"kind":"inline","positions":[{}],"normals":[{}],"uvs":[{}],"indices":[{}]}}"#,
        triples(&pos),
        triples(&nrm),
        pairs(&uv),
        idx.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
    )
}

/// A rotation about an axis as the wire's quaternion.
fn quat(axis: [f32; 3], angle: f32) -> [f32; 4] {
    let l = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt().max(1e-6);
    let s = (angle / 2.0).sin() / l;
    [axis[0] * s, axis[1] * s, axis[2] * s, (angle / 2.0).cos()]
}

/// Where a world point in the scene lands in design units.
fn on_screen(p: [f32; 3]) -> (f32, f32) {
    let d = (CAMERA_Z - p[2]).max(0.1);
    let half = (FOV / 2.0).tan();
    (W / 2.0 + p[0] / (d * half * W / H) * W / 2.0, H / 2.0 - p[1] / (d * half) * H / 2.0)
}

/// One of ffmpeg's generators, its newest frame kept.
struct Video {
    child: Child,
    latest: Arc<Mutex<Option<Vec<u8>>>>,
}

impl Video {
    fn start(source: &str) -> Option<Self> {
        let mut child = Command::new("ffmpeg")
            .args(["-loglevel", "quiet", "-re", "-f", "lavfi", "-i", source, "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut stdout = child.stdout.take()?;
        let latest = Arc::new(Mutex::new(None));
        let keep = Arc::clone(&latest);
        std::thread::spawn(move || {
            let mut frame = vec![0u8; (VIDEO_SIDE * VIDEO_SIDE * 4) as usize];
            while stdout.read_exact(&mut frame).is_ok() {
                if let Ok(mut slot) = keep.lock() {
                    *slot = Some(frame.clone());
                }
            }
        });
        Some(Self { child, latest })
    }

    fn take(&self) -> Option<Vec<u8>> {
        self.latest.lock().ok()?.take()
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The terminal's own text, raining.
struct Rain {
    cols: u16,
    rows: u16,
    /// Per column: where its head is (rows, may be negative), its speed in
    /// rows a second, its trail's length.
    drops: Vec<(f32, f32, f32)>,
    glyphs: Vec<char>,
    /// What each cell of the rain shows now, so a frame writes only the
    /// cells that change.
    cells: Vec<(char, (u8, u8, u8))>,
}

impl Rain {
    fn new(cols: u16, rows: u16) -> Self {
        let glyphs: Vec<char> = ('\u{ff66}'..='\u{ff9d}').chain("0123456789Z:.=*+-<>|".chars()).collect();
        let drops = (0..cols)
            .map(|c| {
                let r = hash(u32::from(c), 77);
                (-(r * f32::from(rows) * 2.0), 6.0 + 22.0 * hash(u32::from(c), 78), 6.0 + 20.0 * hash(u32::from(c), 79))
            })
            .collect();
        let cells = vec![(' ', (0, 0, 0)); usize::from(cols) * usize::from(rows.saturating_sub(2))];
        Self { cols, rows, drops, glyphs, cells }
    }

    /// The rain at `t` over rows 2..rows-1 (the corner rows stay), in
    /// `density` of the columns. Only the cells that changed are written,
    /// in as few cursor moves and colours as they take.
    fn draw(&mut self, out: &mut Vec<u8>, t: f32, dt: f32, density: f32, frame: u64) {
        let rows = self.rows.saturating_sub(2);
        if density <= 0.0 && self.cells.iter().all(|c| c.0 == ' ') {
            return;
        }
        for (c, drop) in self.drops.iter_mut().enumerate() {
            drop.0 += drop.1 * dt;
            if drop.0 - drop.2 > f32::from(rows) {
                drop.0 = -hash(c as u32, (t * 10.0) as u32) * f32::from(rows);
            }
        }
        let mut last: Option<(u8, u8, u8)> = None;
        let mut cursor: Option<(u16, u16)> = None;
        for row in 0..rows {
            for col in 0..self.cols {
                let (head, _, len) = self.drops[usize::from(col)];
                let on = hash(u32::from(col), 5) < density;
                let back = head - f32::from(row);
                let cell = if !on || !(0.0..len).contains(&back) {
                    (' ', (0, 0, 0))
                } else {
                    // Ten shades of green, so colours repeat.
                    let fade = ((1.0 - back / len) * 10.0).round() / 10.0;
                    let color = if back < 1.0 {
                        (220, 255, 230)
                    } else {
                        ((40.0 * fade) as u8, (90.0 + 165.0 * fade) as u8, (60.0 * fade) as u8)
                    };
                    // Each cell changes glyph at its own pace; heads every frame.
                    let key = u32::from(col) * 131 + u32::from(row);
                    let period = 4 + (hash(key, 9) * 14.0) as u64;
                    let turn = if back < 1.0 { frame } else { (frame + (hash(key, 10) * 50.0) as u64) / period };
                    let g = self.glyphs[(hash(key, turn as u32) * self.glyphs.len() as f32) as usize % self.glyphs.len()];
                    (g, color)
                };
                let at = usize::from(row) * usize::from(self.cols) + usize::from(col);
                if self.cells[at] == cell {
                    continue;
                }
                self.cells[at] = cell;
                if cursor != Some((row, col)) {
                    let _ = write!(ByteWriter(out), "\x1b[{};{}H", row + 2, col + 1);
                }
                if cell.0 != ' ' && last != Some(cell.1) {
                    let _ = write!(ByteWriter(out), "\x1b[38;2;{};{};{}m", cell.1 .0, cell.1 .1, cell.1 .2);
                    last = Some(cell.1);
                }
                let mut buf = [0u8; 4];
                out.extend_from_slice(cell.0.encode_utf8(&mut buf).as_bytes());
                // A half-width glyph or a space moves the cursor one cell.
                cursor = Some((row, col + 1));
            }
        }
        if last.is_some() {
            out.extend_from_slice(b"\x1b[0m");
        }
    }
}

/// A text stream: which strip, which way, how fast, where across, how
/// big, in front or behind, and from when.
struct Stream {
    picture: usize,
    angle: f32,
    spin: f32,
    speed: f32,
    lane: f32,
    scale: f32,
    z: i32,
    alpha: f32,
    start: f32,
    zoom: bool,
}

pub struct Surreal {
    rain: Rain,
    videos: Vec<Video>,
    /// Uploaded strips and glyphs: (id, width, height).
    strips: Vec<(u32, u32, u32)>,
    glyphs: Vec<(u32, u32, u32)>,
    streams: Vec<Stream>,
    pizza: String,
    /// The window's size in pixels, for the scene.
    window: (f32, f32),
    last_t: f32,
    /// The cast's assets that were found and sent.
    cast: Vec<u32>,
    /// Character nodes whose clip has been started.
    playing: std::collections::HashSet<u64>,
    /// Balloons popping this frame: where on screen, how long ago, colour.
    pops: Vec<(f32, f32, f32, [f32; 3])>,
}

/// One of the cast in a frame: its node, which asset and clip, where, and
/// how it looks.
struct Actor {
    node: u64,
    asset: u32,
    clip: u32,
    speed: f32,
    seek: f32,
    at: [f32; 3],
    rot: [f32; 4],
    scale: f32,
    color: [f32; 3],
    liquid: f32,
    /// In the scene behind the geometry rather than in front of it.
    back: bool,
}

/// `a` then `b`: the rotation `b` applied after `a`.
fn quat_mul(b: [f32; 4], a: [f32; 4]) -> [f32; 4] {
    [
        b[3] * a[0] + b[0] * a[3] + b[1] * a[2] - b[2] * a[1],
        b[3] * a[1] - b[0] * a[2] + b[1] * a[3] + b[2] * a[0],
        b[3] * a[2] + b[0] * a[1] - b[1] * a[0] + b[2] * a[3],
        b[3] * a[3] - b[0] * a[0] - b[1] * a[1] - b[2] * a[2],
    ]
}

/// A balloon's face, badly: eyes that do not match, looking two ways, a
/// lopsided grin, angry brows and pink cheeks, all hard edged, on white
/// the balloon's colour tints. The face is where u and v are a half.
fn balloon_face() -> Vec<u8> {
    let (w, h) = (256usize, 128usize);
    let mut rgba = vec![255u8; w * h * 4];
    for py in 0..h {
        for px in 0..w {
            // Angles round the sphere, so circles stay round near the face.
            let (x, y) = ((px as f32 / w as f32 - 0.5) * TAU, (py as f32 / h as f32 - 0.5) * PI);
            let d = |cx: f32, cy: f32| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            let mut c: Option<[u8; 3]> = None;
            if d(-0.42, 0.42) < 0.2 || d(0.5, 0.38) < 0.2 {
                c = Some([255, 120, 160]);
            }
            let mouth = d(0.03, -0.05);
            let warp = 0.06 * (x * 7.0).sin();
            if (0.52..0.64).contains(&(mouth + warp)) && y > 0.18 + 0.1 * x {
                c = Some([20, 10, 10]);
            }
            if d(-0.3, -0.25) < 0.24 {
                c = Some([255, 255, 255]);
                if d(-0.24, -0.2) < 0.1 {
                    c = Some([0, 0, 0]);
                }
            }
            if d(0.33, -0.3) < 0.31 {
                c = Some([255, 255, 255]);
                if d(0.43, -0.38) < 0.13 {
                    c = Some([0, 0, 0]);
                }
            }
            for (bx, by, tilt) in [(-0.32f32, -0.58f32, 0.5f32), (0.33, -0.7, -0.25)] {
                let (dx, dy) = (x - bx, y - by);
                if dx.abs() < 0.22 && (dy - dx * tilt).abs() < 0.04 {
                    c = Some([30, 15, 5]);
                }
            }
            if let Some(c) = c {
                let i = (py * w + px) * 4;
                rgba[i..i + 3].copy_from_slice(&c);
            }
        }
    }
    rgba
}

impl Surreal {
    pub fn new(cols: u16, rows: u16, window: (f32, f32)) -> Self {
        let mut show = Self::bare(cols, rows, window);
        show.videos = VIDEOS.iter().filter_map(|s| Video::start(s)).collect();
        show
    }

    /// The show without its video generators running.
    fn bare(cols: u16, rows: u16, window: (f32, f32)) -> Self {
        let streams = (0..40)
            .map(|i| {
                let r = |salt: u32| hash(i, salt);
                let ways = [0.0, PI, PI / 2.0, -PI / 2.0, PI / 4.0, -PI / 4.0, 3.0 * PI / 4.0, -3.0 * PI / 4.0];
                Stream {
                    picture: i as usize,
                    angle: ways[(r(1) * ways.len() as f32) as usize % ways.len()] + (r(2) - 0.5) * 0.3,
                    spin: if r(3) > 0.8 { (r(4) - 0.5) * 0.6 } else { 0.0 },
                    speed: 90.0 + 380.0 * r(5),
                    lane: (r(6) - 0.5) * 1100.0,
                    scale: 0.55 + 1.2 * r(7),
                    z: if r(8) > 0.5 { 1 } else { -2 },
                    alpha: 0.35 + 0.55 * r(9),
                    start: 17.0 + 48.0 * r(10),
                    zoom: r(11) > 0.82,
                }
            })
            .collect();
        Self {
            rain: Rain::new(cols, rows),
            videos: Vec::new(),
            strips: Vec::new(),
            glyphs: Vec::new(),
            streams,
            pizza: pizza_mesh(),
            window,
            last_t: 0.0,
            cast: Vec::new(),
            playing: std::collections::HashSet::new(),
            pops: Vec::new(),
        }
    }

    /// The pictures that do not move: the pizza, the eyeball, the text.
    pub fn setup(&mut self, out: &mut Vec<u8>) {
        upload(out, PIZZA, 256, 256, &pizza_texture());
        upload(out, EYEBALL, 256, 128, &eyeball_texture());
        upload(out, BALLOON_FACE, 256, 128, &balloon_face());
        // glass reads the characters itself (`asset` with `t=f`).
        let dir = std::env::var("MINDSEYE_ASSETS")
            .unwrap_or_else(|_| format!("{}/models/mixamo", std::env::var("HOME").unwrap_or_default()));
        for (id, file) in CAST {
            let path = std::path::Path::new(&dir).join(file);
            if path.is_file() {
                let _ = write!(ByteWriter(out), "\x1b_glass;asset;id={id};t=f;{}\x1b\\", b64(path.to_string_lossy().as_bytes()));
                self.cast.push(id);
            }
        }
        let mut id = TEXT_BASE;
        for (text, color) in STRIPS {
            // Berkeley Mono for the ASCII, a CJK face for the rest.
            let font = if text.is_ascii() { "Berkeley-Mono-Regular" } else { "Noto-Sans-CJK-JP-Bold" };
            if let Some((w, h, rgba)) = render_text(text, font, 40, color) {
                upload(out, id, w, h, &rgba);
                self.strips.push((id, w, h));
                id += 1;
            }
        }
        for (text, font, color) in GLYPHS {
            if let Some((w, h, rgba)) = render_text(text, font, 260, color) {
                upload(out, id, w, h, &rgba);
                self.glyphs.push((id, w, h));
                id += 1;
            }
        }
    }

    /// Where the pizza slices are at `t`: (world point, spin axis, angle).
    fn pizzas(t: f32) -> Vec<([f32; 3], [f32; 3], f32)> {
        let storm = envelope(t, 66.0, 80.0, 1.5, 2.0);
        let count = if t > 6.0 { 3 } else { 0 } + (storm * 16.0) as usize;
        (0..count)
            .map(|i| {
                let r = |salt: u32| hash(i as u32, salt);
                let (speed, phase) = (0.35 + 0.5 * r(1), r(2) * 40.0);
                let path = t * speed + phase;
                let x = ((path * 1.7).rem_euclid(18.0)) - 9.0;
                let y = 3.2 * (path * 0.9 + r(3) * 6.0).sin();
                let z = -3.0 + 5.0 * r(4) + (path * 0.5).sin();
                ([x, y, z], [r(5) - 0.5, r(6) - 0.5, r(7) - 0.5], t * (1.0 + 2.0 * r(8)))
            })
            .collect()
    }

    /// Before the show draws: where the eye looks, how far the fractal
    /// goes, and the infection's tint.
    pub fn before(&mut self, cv: &mut Canvas, t: f32) {
        let pizzas = Self::pizzas(t);
        let (cx, cy) = (W / 2.0, H / 2.0);
        let nearest = pizzas
            .iter()
            .map(|p| on_screen(p.0))
            .min_by(|a, b| {
                let d = |p: &(f32, f32)| (p.0 - cx).powi(2) + (p.1 - cy).powi(2);
                d(a).total_cmp(&d(b))
            });
        cv.look = match nearest {
            Some((x, y)) => ((x - cx) * 0.22, (y - cy) * 0.12),
            None => (60.0 * (t * 0.7).sin(), 20.0 * (t * 1.3).cos()),
        };
        cv.fractal_depth = if t > 62.0 { 2 } else { 3 };
        let gore = envelope(t, 58.0, 80.0, 4.0, 2.0);
        cv.tint = (gore > 0.0).then(|| (hex("#ff1133"), 0.4 * gore));
    }

    /// After the show draws: this show's own layers into the frame, and
    /// what goes beside them (`pre` before the groups: video frames; `post`
    /// after: the scene and the rain).
    pub fn after(&mut self, cv: &mut Canvas, t: f32, frame: u64, pre: &mut Vec<u8>, post: &mut Vec<u8>) {
        let dt = (t - self.last_t).clamp(0.0, 0.1);
        self.last_t = t;
        cv.tint = None;
        // MINDSEYE_SKIP=scene,video,... leaves pieces out, to see what each costs.
        let skip = std::env::var("MINDSEYE_SKIP").unwrap_or_default();
        let on = |name: &str| !skip.split(',').any(|s| s == name);
        if on("prims") {
            self.prims(cv, t);
        }
        if on("glyphs") {
            self.glyphs(cv, t);
        }
        if on("men") {
            self.men(cv, t);
        }
        if on("clocks") {
            self.clocks(cv, t);
        }
        if on("text") {
            self.streams(cv, t);
        }
        if on("gore") {
            self.gore(cv, t, frame);
        }
        // Video frames, at the generators' own pace.
        if on("video") {
            for (n, video) in self.videos.iter().enumerate() {
                if let Some(rgba) = video.take() {
                    upload(pre, VIDEO_BASE + n as u32, VIDEO_SIDE, VIDEO_SIDE, &rgba);
                }
            }
        }
        if on("scene") {
            self.scene(t, post);
            // Popped balloons burst into confetti, glass's own particles.
            for &(x, y, age, c) in &self.pops {
                let rgb = super::Rgb(c[0], c[1], c[2]);
                let rec = format!(
                    "k=particles;c={};n=90;size=5;life=1.4;rise=620;sway=900;seed={};burst={age:.3};px={:.1},{:.1},120,120",
                    cv.colour(rgb, 1.0),
                    (x * 7.0 + y) as u32,
                    cv.at(x, y).0 - 60.0,
                    cv.at(x, y).1 - 60.0
                );
                cv.push_at(2, true, rec);
            }
        }
        if on("rain") {
            let density = envelope(t, 40.0, 79.0, 6.0, 2.0) * (0.3 + 0.4 * smooth(55.0, 70.0, t));
            self.rain.draw(post, t, dt, density, frame);
        }
    }

    /// Big lit solids, slowly crossing.
    fn prims(&self, cv: &mut Canvas, t: f32) {
        let k = envelope(t, 3.0, 82.0, 3.0, 3.0);
        for i in 0..5u32 {
            let r = |salt: u32| hash(i, salt + 20);
            let size = 120.0 + 150.0 * r(1);
            let travel = W + 2.0 * size + 200.0;
            let x = (t * (25.0 + 40.0 * r(2)) + r(3) * travel).rem_euclid(travel) - size - 100.0;
            let x = if i % 2 == 0 { x } else { W - x };
            let y = 120.0 + (H - 240.0) * r(4) + 90.0 * (t * 0.3 + i as f32).sin();
            let c = gradient(&[hex("#ff3fa4"), hex("#00e5ff"), hex("#ffc861"), hex("#8a5cff")], r(5) + t / 30.0);
            cv.prim(x, y, size, i % 7, c, k * 0.9, t * (0.3 + 0.4 * r(6)));
            // A glowing border: marching dashes, and comets running round.
            let edge = gradient(&[hex("#00e5ff"), hex("#ff3fa4"), hex("#ffc861"), hex("#33ff77")], r(7) + t / 5.0);
            let ring = size * 1.12;
            let dashes = 20;
            for d in 0..dashes {
                let a0 = d as f32 * TAU / dashes as f32 + t * (0.6 + 0.4 * r(8)) * if i % 2 == 0 { 1.0 } else { -1.0 };
                let a1 = a0 + TAU / dashes as f32 * 0.55;
                cv.line(x + ring * a0.cos(), y + ring * a0.sin(), x + ring * a1.cos(), y + ring * a1.sin(), edge, k * 0.75, 2.4, 8.0);
            }
            for c3 in 0..3 {
                let a = t * 2.2 + c3 as f32 * TAU / 3.0;
                cv.glow(x + ring * a.cos(), y + ring * a.sin(), 22.0, mix(edge, hex("#ffffff"), 0.5), k);
            }
        }
    }

    /// Giant characters turning in the dark.
    fn glyphs(&self, cv: &mut Canvas, t: f32) {
        let k = envelope(t, 8.0, 82.0, 4.0, 3.0);
        for (n, &(id, w, h)) in self.glyphs.iter().enumerate() {
            let r = |salt: u32| hash(n as u32, salt + 40);
            let x = W * (0.1 + 0.8 * r(1)) + 200.0 * (t * 0.11 + r(2) * 6.0).sin();
            let y = H * (0.15 + 0.7 * r(3)) + 140.0 * (t * 0.17 + r(4) * 6.0).cos();
            let scale = 0.8 + 0.6 * (t * 0.2 + r(5) * 5.0).sin().abs();
            let z = if n % 2 == 0 { -2 } else { 1 };
            let angle = t * (r(6) - 0.5) * 0.5;
            let (bw, bh) = (w as f32 * scale, h as f32 * scale);
            cv.picture(id, x, y, bw, bh, angle, k * 0.45, z);
            // A glowing frame with a light running round it.
            let (sn, cs) = angle.sin_cos();
            let corner = |cx: f32, cy: f32| (x + cx * cs - cy * sn, y + cx * sn + cy * cs);
            let (hw, hh) = (bw * 0.56, bh * 0.56);
            let frame = [corner(-hw, -hh), corner(hw, -hh), corner(hw, hh), corner(-hw, hh)];
            let edge = gradient(&[hex("#ff1133"), hex("#00e5ff"), hex("#ffc861")], n as f32 / 6.0 + t / 4.0);
            cv.polyline(&frame, true, edge, k * 0.55, 1.6, 6.0);
            let run = (t * 0.5 + r(7)).fract() * 4.0;
            let (side, at) = (run as usize % 4, run.fract());
            let (a, b) = (frame[side], frame[(side + 1) % 4]);
            cv.glow(a.0 + (b.0 - a.0) * at, a.1 + (b.1 - a.1) * at, 26.0, hex("#ffffff"), k * 0.9);
        }
    }

    /// Magritte's men, falling in rows like rain: coats, bowler hats.
    fn men(&self, cv: &mut Canvas, t: f32) {
        let k = envelope(t, 2.0, 27.0, 2.0, 3.0).max(0.85 * envelope(t, 50.0, 81.0, 3.0, 3.0));
        if k <= 0.0 {
            return;
        }
        let dark = hex("#07070b");
        for i in 0..48u32 {
            let r = |salt: u32| hash(i, salt + 60);
            let depth = [0.45, 0.7, 1.0][(i % 3) as usize];
            let col = (i / 3) % 8;
            let x = (col as f32 + 0.5) * W / 8.0 + (r(1) - 0.5) * 80.0 + if i % 2 == 0 { 40.0 } else { -40.0 };
            let fall = 18.0 + 20.0 * depth;
            let span = H + 300.0;
            let y = (r(2) * span + t * fall).rem_euclid(span) - 150.0;
            let tilt = 0.15 * (t * 0.4 + r(3) * 6.0).sin();
            let s = depth * 1.15;
            let a = k * (0.55 + 0.4 * depth);
            let (sin, cos) = tilt.sin_cos();
            cv.glow(x, y - 10.0 * s, 85.0 * s, hex("#3a5cff"), a * 0.45);
            let part = |cv: &mut Canvas, ox: f32, oy: f32, w: f32, h: f32| {
                let (px, py) = (ox * s, oy * s);
                cv.solid(x + px * cos - py * sin, y + px * sin + py * cos, w * s, h * s, tilt, dark, a, -1, false);
            };
            part(cv, 0.0, 0.0, 34.0, 64.0);
            part(cv, -7.0, 44.0, 9.0, 28.0);
            part(cv, 7.0, 44.0, 9.0, 28.0);
            part(cv, 0.0, -42.0, 18.0, 20.0);
            part(cv, 0.0, -53.0, 34.0, 4.0);
            part(cv, 0.0, -60.0, 20.0, 13.0);
        }
    }

    /// Melting clocks running backwards; one hangs over the eye's lid.
    fn clocks(&self, cv: &mut Canvas, t: f32) {
        let k = envelope(t, 34.0, 72.0, 3.0, 3.0);
        if k <= 0.0 {
            return;
        }
        let cream = hex("#ffe7b0");
        let hung = envelope(t, 47.0, 70.0, 2.0, 3.0);
        for n in 0..3u32 {
            let r = |salt: u32| hash(n, salt + 80);
            let (mut x, mut y) = (W * (0.15 + 0.7 * r(1)) + 150.0 * (t * 0.13 + r(2) * 6.0).sin(), H * (0.2 + 0.6 * r(3)) + 80.0 * (t * 0.21).cos());
            let mut radius = 60.0 + 40.0 * r(4);
            let mut sag = 50.0 + 60.0 * (t * 0.3 + r(5) * 4.0).sin().abs();
            if n == 0 {
                // The one that found the eye.
                x = x + (W / 2.0 + 190.0 - x) * hung;
                y = y + (H / 2.0 - 175.0 - y) * hung;
                radius += 20.0 * hung;
                sag += 80.0 * hung;
            }
            let melt = |a: f32, rr: f32| {
                let (s, c) = a.sin_cos();
                let drip = s.max(0.0).powi(2) * sag * (1.0 + 0.5 * (3.0 * a + t).sin()) * rr / radius;
                (x + rr * c, y + rr * s + drip)
            };
            let rim: Vec<(f32, f32)> = (0..48).map(|i| melt(i as f32 * TAU / 48.0, radius)).collect();
            cv.polyline(&rim, true, cream, k * 0.85, 2.0, 5.0);
            for h in 0..12 {
                let a = h as f32 * TAU / 12.0;
                let (a0, a1) = (melt(a, radius * 0.82), melt(a, radius * 0.95));
                cv.line(a0.0, a0.1, a1.0, a1.1, cream, k * 0.7, 1.4, 2.0);
            }
            for (len, speed) in [(0.5f32, -0.6f32), (0.78, -7.0)] {
                let a = t * speed - PI / 2.0;
                let tip = melt(a, radius * len);
                cv.line(x, y, tip.0, tip.1, hex("#ff3fa4"), k * 0.9, 2.2, 4.0);
            }
        }
    }

    /// Text strips crossing every way, in front and behind; a few rush out
    /// of the middle at the eye.
    fn streams(&self, cv: &mut Canvas, t: f32) {
        if self.strips.is_empty() {
            return;
        }
        let k = envelope(t, 17.0, 81.0, 2.0, 3.0);
        for s in &self.streams {
            if t < s.start || k <= 0.0 {
                continue;
            }
            let (id, w, h) = self.strips[s.picture % self.strips.len()];
            let (w, h) = (w as f32 * s.scale, h as f32 * s.scale);
            let fade = k * smooth(s.start, s.start + 1.5, t);
            if s.zoom {
                // Out of the middle at the eye: growing, turning, gone.
                let p = ((t - s.start) * 0.25).fract();
                let grow = 0.15 + 4.0 * p * p;
                cv.picture(id, W / 2.0, H / 2.0 + s.lane * 0.2, w * grow, h * grow, s.angle * 0.3 + t * s.spin, fade * s.alpha * (1.0 - p), s.z);
                continue;
            }
            let angle = s.angle + t * s.spin;
            let (dx, dy) = (angle.cos(), angle.sin());
            let travel = 2300.0 + w;
            let along = ((t - s.start) * s.speed).rem_euclid(travel) - travel / 2.0;
            let (x, y) = (W / 2.0 + dx * along - dy * s.lane, H / 2.0 + dy * along + dx * s.lane);
            cv.picture(id, x, y, w, h, angle, fade * s.alpha, s.z);
        }
    }

    /// Techno gore: cables crawling in to the eye with pulses running
    /// along them, the eye weeping light, and bands tearing the picture.
    fn gore(&self, cv: &mut Canvas, t: f32, frame: u64) {
        let k = envelope(t, 56.0, 81.0, 4.0, 2.0);
        if k <= 0.0 {
            return;
        }
        let (cx, cy) = (W / 2.0, H / 2.0);
        let blood = hex("#ff1133");
        let deep = hex("#5a0010");
        for i in 0..8u32 {
            let r = |salt: u32| hash(i, salt + 100);
            let edge = i as f32 * TAU / 8.0 + r(1) * 0.5;
            let (ax, ay) = (cx + 1300.0 * edge.cos(), cy + 800.0 * edge.sin());
            let reach = smooth(56.0 + r(2) * 4.0, 63.0 + r(2) * 4.0, t);
            let (bx, by) = (cx + 210.0 * edge.cos(), cy + 120.0 * edge.sin());
            let (ex, ey) = (ax + (bx - ax) * reach, ay + (by - ay) * reach);
            let (nx, ny) = (-(ey - ay), ex - ax);
            let nl = (nx * nx + ny * ny).sqrt().max(1.0);
            let points: Vec<(f32, f32)> = (0..=28)
                .map(|j| {
                    let s = j as f32 / 28.0;
                    let wave = 46.0 * (s * 7.0 + t * 3.1 + i as f32).sin() * (s * PI).sin();
                    (ax + (ex - ax) * s + nx / nl * wave, ay + (ey - ay) * s + ny / nl * wave)
                })
                .collect();
            cv.polyline(&points, false, deep, k * 0.75, 7.0, 8.0);
            // A pulse running in, toward the eye.
            let at = (t * 0.7 + r(3)).fract();
            for pair in points.windows(2).enumerate() {
                let s = pair.0 as f32 / 28.0;
                let pulse = (-((s - at) * 9.0).powi(2)).exp();
                let (a, b) = (pair.1[0], pair.1[1]);
                cv.line(a.0, a.1, b.0, b.1, mix(blood, hex("#ffd0d8"), pulse), k * (0.35 + 0.65 * pulse), 2.0 + 2.0 * pulse, 4.0 + 6.0 * pulse);
            }
        }
        // The eye weeps light from its lower lid.
        let weep = envelope(t, 60.0, 80.0, 2.0, 2.0);
        for i in 0..14u32 {
            let r = |salt: u32| hash(i, salt + 120);
            let x = cx + (r(1) - 0.5) * 560.0;
            let lid = cy + 165.0 * (1.0 - ((x - cx) / 410.0).powi(2)).max(0.0);
            let period = 1.4 + 1.6 * r(2);
            let tau = (t + r(3) * period).rem_euclid(period);
            let y = lid + 0.5 * 420.0 * tau * tau;
            cv.line(x, lid, x, y - 6.0, blood, weep * 0.5 * (1.0 - tau / period), 1.2, 2.5);
            cv.glow(x, y, 9.0 + 6.0 * r(4), blood, weep * (1.0 - tau / period));
        }
        // Bands tear across, a few frames each.
        let tear = k * smooth(58.0, 72.0, t);
        let beat = (-((t * 1.2).fract() * 8.0)).exp();
        cv.glow(cx, cy, 1400.0, blood, tear * 0.18 * beat);
        let slot = (frame / 3) as u32;
        for b in 0..10u32 {
            if hash(slot, b) > 0.35 * tear {
                continue;
            }
            let y = hash(slot, b + 50) * H;
            let height = 2.0 + 38.0 * hash(slot, b + 60).powi(3);
            let c = [blood, hex("#00e5ff"), hex("#ffffff"), hex("#33ff77")][(hash(slot, b + 70) * 4.0) as usize % 4];
            let shift = (hash(slot, b + 80) - 0.5) * 300.0;
            cv.solid(W / 2.0 + shift, y, W * (0.4 + 0.8 * hash(slot, b + 90)), height, 0.0, c, 0.25 + 0.55 * hash(slot, b + 95), 1, true);
        }
    }

    /// The cast's choreography at `t`. The man (4) walks the ceiling and a
    /// wall, dances in a conga line along the floor in gold, each in a
    /// different dance and out of step, looms as a giant dancing ant beside
    /// the eye, and rings the eye upside down doing the chicken dance at
    /// the end. Michelle (1) does the samba on top of the eye, then lifts
    /// off spinning. X Bot (3) rides the first pizza nodding, then towers
    /// behind it all shaking its head.
    fn actors(&self, t: f32) -> Vec<Actor> {
        let mut cast = Vec::new();
        let y_axis = |a: f32| quat([0.0, 1.0, 0.0], a);
        let z_axis = |a: f32| quat([0.0, 0.0, 1.0], a);
        let base = |node, asset, clip, at, rot, scale, color, liquid| Actor { node, asset, clip, speed: 1.0, seek: 0.0, at, rot, scale, color, liquid, back: false };
        if (18.0..62.0).contains(&t) {
            let walk = ((t - 18.0) / 22.0).fract();
            cast.push(base(1001, 4, WALK, [-8.0 + 16.0 * walk, 3.7, -1.0], quat_mul(z_axis(PI), y_axis(PI / 2.0)), 1.0, [0.75, 0.85, 1.0], 0.25));
        }
        if (26.0..62.0).contains(&t) {
            let climb = ((t - 26.0) / 18.0).fract();
            cast.push(base(1002, 4, WALK, [-7.4, -5.0 + 10.0 * climb, -1.5], quat_mul(z_axis(-PI / 2.0), y_axis(-PI / 2.0)), 1.0, [0.8, 1.0, 0.85], 0.25));
        }
        if (30.0..81.0).contains(&t) {
            let dances = [CHICKEN, LAMBADA, SALSA, WHIRL, HIPPO_BALLERINA, DANCING_ANT, PIROUETTES];
            for (i, &clip) in dances.iter().enumerate() {
                let x = ((-7.0 + 2.1 * i as f32 + (t - 30.0) * 0.5) + 7.5).rem_euclid(15.0) - 7.5;
                let mut a = base(1010 + i as u64, 4, clip, [x, -3.75, 0.5], y_axis(0.3 * (t + i as f32).sin()), 0.85, [1.0, 0.8, 0.35], 0.1);
                a.speed = 0.95 + 0.05 * i as f32;
                cast.push(a);
            }
        }
        if (44.0..81.0).contains(&t) {
            let lift = smooth(66.0, 72.0, t);
            let spin = y_axis(lift * (t - 66.0) * 1.2);
            cast.push(base(1020, 1, 0, [0.0, 1.16 + 2.0 * lift, 0.5], spin, 1.8 - 0.6 * lift, [1.0, 0.86, 0.95], 0.4));
        }
        if (50.0..81.0).contains(&t) {
            let mut ant = base(1040, 4, DANCING_ANT, [-4.6, -3.6, -3.0], y_axis(0.5), 2.4, [0.6, 1.0, 0.8], 0.6);
            ant.back = true;
            cast.push(ant);
        }
        if (66.0..81.0).contains(&t) {
            for i in 0..6u32 {
                let a = i as f32 * TAU / 6.0 + t * 0.4;
                let mut m = base(1021 + u64::from(i), 4, CHICKEN, [3.3 * a.cos(), 3.3 * a.sin() * 0.8, -1.0], z_axis(a - PI / 2.0), 0.8, [0.85, 0.95, 1.0], 0.8);
                m.seek = i as f32 * 0.3;
                cast.push(m);
            }
        }
        if (8.0..66.0).contains(&t)
            && let Some(&(at, _, _)) = Self::pizzas(t).first()
        {
            cast.push(base(1031, 3, 0, [at[0], at[1] + 0.25, at[2]], y_axis(PI / 2.0), 0.3, [0.92, 0.92, 0.95], 0.0));
        }
        if (58.0..81.0).contains(&t) {
            let mut giant = base(1030, 3, 1, [0.0, -6.5, -7.0], y_axis(0.0), 4.5, [0.7, 0.9, 1.0], 1.0);
            giant.back = true;
            cast.push(giant);
        }
        cast
    }

    /// The 3D things: pizza, video cubes, eyeballs, Escher's stairs.
    fn scene(&mut self, t: f32, out: &mut Vec<u8>) {
        let skip = std::env::var("MINDSEYE_SKIP").unwrap_or_default();
        let on = |name: &str| !skip.split(',').any(|s| s == name);
        let mut nodes: Vec<String> = Vec::new();
        let mut id = 1u32;
        let mut node = |nodes: &mut Vec<String>, body: String| {
            nodes.push(format!(r#"{{"id":{id},{body}}}"#));
            id += 1;
            id - 1
        };
        let f3 = |v: [f32; 3]| format!("[{:.3},{:.3},{:.3}]", v[0], v[1], v[2]);
        let f4 = |v: [f32; 4]| format!("[{:.4},{:.4},{:.4},{:.4}]", v[0], v[1], v[2], v[3]);
        for (at, axis, angle) in Self::pizzas(t).into_iter().filter(|_| on("pizza")) {
            node(
                &mut nodes,
                format!(r#""at":{},"rot":{},"scale":[0.9,0.9,0.9],"mesh":{},"picture":{PIZZA},"lit":true,"emissive":0.35"#, f3(at), f4(quat(axis, angle)), self.pizza),
            );
        }
        // Cubes playing video on every face.
        let cubes = if t > 16.0 { 2 } else { 0 } + if t > 31.0 { 2 } else { 0 };
        let cube_k = envelope(t, 16.0, 82.0, 1.0, 3.0);
        if !self.videos.is_empty() && cube_k > 0.0 && on("cubes") {
            let faces: [([f32; 3], [f32; 3], f32); 6] = [
                ([0.0, 0.0, 0.5], [0.0, 1.0, 0.0], 0.0),
                ([0.0, 0.0, -0.5], [0.0, 1.0, 0.0], PI),
                ([0.5, 0.0, 0.0], [0.0, 1.0, 0.0], PI / 2.0),
                ([-0.5, 0.0, 0.0], [0.0, 1.0, 0.0], -PI / 2.0),
                ([0.0, 0.5, 0.0], [1.0, 0.0, 0.0], -PI / 2.0),
                ([0.0, -0.5, 0.0], [1.0, 0.0, 0.0], PI / 2.0),
            ];
            for c in 0..cubes {
                let r = |salt: u32| hash(c as u32, salt + 140);
                let orbit = t * (0.25 + 0.2 * r(1)) + c as f32 * TAU / 4.0;
                let at = [5.2 * orbit.cos(), 2.2 * (orbit * 1.3).sin(), -1.5 + 2.5 * orbit.sin()];
                let size = 1.4 + 0.5 * r(2);
                if ROUNDCUBE {
                    // One rounded cube, a video on each face, its edges a
                    // glowing border cycling colour and breathing.
                    let faces: Vec<String> = (0..6).map(|n| (VIDEO_BASE + ((n + c) % self.videos.len()) as u32).to_string()).collect();
                    let edge = gradient(&[hex("#00e5ff"), hex("#ff3fa4"), hex("#ffc861"), hex("#33ff77")], t / 3.0 + c as f32 * 0.25);
                    let glow = 0.6 + 0.4 * (t * 3.0 + c as f32).sin();
                    node(
                        &mut nodes,
                        format!(
                            r#""at":{},"rot":{},"scale":[{size:.2},{size:.2},{size:.2}],"mesh":{{"kind":"roundcube","radius":0.16,"segments":5}},"faces":[{}],"edge":{{"color":[{:.3},{:.3},{:.3},{cube_k:.3}],"glow":{glow:.3}}},"color":[1,1,1,{cube_k:.3}]"#,
                            f3(at),
                            f4(quat([r(3) - 0.5, 1.0, r(4) - 0.5], t * (0.6 + 0.6 * r(5)))),
                            faces.join(","),
                            edge.0,
                            edge.1,
                            edge.2
                        ),
                    );
                    continue;
                }
                let parent = node(
                    &mut nodes,
                    format!(r#""at":{},"rot":{},"scale":[{size:.2},{size:.2},{size:.2}]"#, f3(at), f4(quat([r(3) - 0.5, 1.0, r(4) - 0.5], t * (0.6 + 0.6 * r(5))))),
                );
                for (n, (offset, axis, angle)) in faces.iter().enumerate() {
                    let video = VIDEO_BASE + ((n + c) % self.videos.len()) as u32;
                    node(
                        &mut nodes,
                        format!(r#""parent":{parent},"at":{},"rot":{},"mesh":"quad","picture":{video},"color":[1,1,1,{cube_k:.3}]"#, f3(*offset), f4(quat(*axis, *angle))),
                    );
                }
            }
        }
        // Eyeballs rolling, looking about.
        let eyes_k = envelope(t, 12.0, 66.0, 2.0, 3.0);
        if eyes_k > 0.0 && on("eyes") {
            for e in 0..3u32 {
                let r = |salt: u32| hash(e, salt + 160);
                let at = [-6.0 + 12.0 * r(1) + 1.5 * (t * 0.2 + r(2) * 6.0).sin(), -3.0 + 6.0 * r(3) + (t * 0.3).cos(), -2.0 + 3.0 * r(4)];
                let look = quat([0.0, 1.0, 0.0], -PI / 2.0 + 0.6 * (t * 0.9 + r(5) * 6.0).sin());
                node(
                    &mut nodes,
                    format!(r#""at":{},"rot":{},"scale":[0.7,0.7,0.7],"mesh":{{"kind":"sphere","segments":24}},"picture":{EYEBALL},"lit":true,"emissive":0.3"#, f3(at), f4(look)),
                );
            }
        }
        // Escher's stairs and an arch, floating behind it all, going nowhere.
        let mut back: Vec<String> = Vec::new();
        let stairs_k = envelope(t, 20.0, 82.0, 3.0, 3.0);
        if stairs_k > 0.0 && on("stairs") {
            for s in 0..3u32 {
                let r = |salt: u32| hash(s, salt + 180);
                let at = [-7.0 + 14.0 * r(1), -3.5 + 7.0 * r(2) + 0.6 * (t * 0.25 + s as f32).sin(), -4.0 - 3.0 * r(3)];
                let mesh = if s == 2 { r#"{"kind":"arch","segments":16}"# } else { r#"{"kind":"stairs","steps":10}"# };
                node(
                    &mut back,
                    format!(
                        r#""at":{},"rot":{},"scale":[1.6,1.6,1.6],"mesh":{mesh},"color":[0.85,0.8,1,{stairs_k:.3}],"lit":true,"emissive":0.2"#,
                        f3(at),
                        f4(quat([r(4) - 0.5, 1.0, r(5) - 0.5], t * 0.15 + r(6) * 6.0))
                    ),
                );
            }
        }
        // Balloons with faces, cheap and cheerful, rising; now and then one pops.
        let balloon_k = envelope(t, 8.0, 82.0, 2.0, 3.0);
        let mut pops: Vec<(f32, f32, f32, [f32; 3])> = Vec::new();
        if on("balloons") && balloon_k > 0.0 {
            let colors = [[1.0, 0.1, 0.15], [1.0, 0.85, 0.1], [0.1, 0.4, 1.0], [0.1, 0.9, 0.3], [0.7, 0.2, 1.0], [1.0, 0.5, 0.05], [1.0, 0.3, 0.8], [0.2, 0.95, 0.95]];
            for b in 0..8u32 {
                let r = |salt: u32| hash(b, salt + 200);
                let period = 13.0 + 7.0 * r(1);
                let cycle = ((t + r(2) * period) / period).floor() as u32;
                let phase = ((t + r(2) * period) / period).fract();
                // Half the balloons pop on their way up, each cycle anew.
                let pop_at = 0.45 + 0.4 * hash(b, cycle + 300);
                let pops_now = hash(b, cycle + 400) > 0.5;
                let since = (phase - pop_at) * period;
                if pops_now && since > 0.12 {
                    if since < 1.5 {
                        let at = [-6.5 + 13.0 * r(3) + 0.8 * (t * 0.5 + r(4) * 6.0).sin(), -5.0 + 10.5 * pop_at, -1.0 + 3.0 * r(5)];
                        pops.push((on_screen(at).0, on_screen(at).1, since, colors[b as usize]));
                    }
                    continue;
                }
                let swell = if pops_now && since > 0.0 { 1.0 + 3.0 * since } else { 1.0 };
                let at = [-6.5 + 13.0 * r(3) + 0.8 * (t * 0.5 + r(4) * 6.0).sin(), -5.0 + 10.5 * phase, -1.0 + 3.0 * r(5)];
                let face = quat([0.2 * (t * 0.7 + r(6) * 6.0).sin(), 1.0, 0.15 * (t * 0.9).cos()], -PI / 2.0 + 0.5 * (t * 0.6 + r(7) * 6.0).sin());
                let c = colors[b as usize];
                let parent = node(&mut nodes, format!(r#""at":{},"rot":{}"#, f3(at), f4(face)));
                node(
                    &mut nodes,
                    format!(
                        r#""parent":{parent},"scale":[{:.3},{:.3},{:.3}],"mesh":{{"kind":"sphere","segments":10}},"picture":{BALLOON_FACE},"color":[{:.2},{:.2},{:.2},{balloon_k:.3}],"lit":true,"emissive":0.12"#,
                        0.55 * swell, 0.68 * swell, 0.55 * swell, c[0], c[1], c[2]
                    ),
                );
                node(&mut nodes, format!(r#""parent":{parent},"at":[0,-0.72,0],"rot":[1,0,0,0],"scale":[0.12,0.14,0.12],"mesh":{{"kind":"cone","segments":6}},"color":[{:.2},{:.2},{:.2},{balloon_k:.3}],"lit":true"#, c[0], c[1], c[2]));
                node(&mut nodes, format!(r#""parent":{parent},"at":[0,-1.55,0],"scale":[0.012,1.6,0.012],"mesh":{{"kind":"cylinder","segments":4}},"color":[0.9,0.9,0.9,{balloon_k:.3}]"#));
            }
        }
        self.pops = pops;
        // The cast, in chrome where glass takes it.
        let mut present = Vec::new();
        let mut plays = Vec::new();
        if on("cast") {
            for actor in self.actors(t) {
                if !self.cast.contains(&actor.asset) {
                    continue;
                }
                let chrome = if CHROME { format!(r#","chrome":1,"liquid":{:.2}"#, actor.liquid) } else { String::new() };
                let (into, scene) = if actor.back { (&mut back, SCENE_BACK) } else { (&mut nodes, SCENE) };
                into.push(format!(
                    r#"{{"id":{},"at":{},"rot":{},"scale":[{s:.3},{s:.3},{s:.3}],"mesh":{{"kind":"gltf","asset":{}}},"color":[{:.2},{:.2},{:.2},1]{chrome}}}"#,
                    actor.node,
                    f3(actor.at),
                    f4(actor.rot),
                    actor.asset,
                    actor.color[0],
                    actor.color[1],
                    actor.color[2],
                    s = actor.scale
                ));
                present.push(actor.node);
                if !self.playing.contains(&actor.node) {
                    plays.push(format!(
                        "\x1b_glass;play;id={scene};node={};clip={};speed={:.2};loop=1;t={:.2}\x1b\\",
                        actor.node, actor.clip, actor.speed, actor.seek
                    ));
                }
            }
        }
        self.playing = present.into_iter().collect();
        let (w, h) = self.window;
        let (sw, sh) = (w.min(2048.0) as u32, h.min(2048.0) as u32);
        // The back scene under every layer of the geometry, the front over.
        for (id, z, list) in [(SCENE_BACK, -3, &back), (SCENE, 0, &nodes)] {
            let json = format!(
                r#"{{"v":1,"scene":{{"camera":{{"eye":[0,0,{CAMERA_Z}],"fov":{FOV}}},"nodes":[{}]}}}}"#,
                list.join(",")
            );
            let _ = write!(ByteWriter(out), "\x1b_glass;scene;id={id};z={z};px=0,0,{w:.0},{h:.0};s={sw};v={sh};{}\x1b\\", b64(json.as_bytes()));
        }
        for play in plays {
            out.extend_from_slice(play.as_bytes());
        }
    }

    /// Take it all down: the scene, the rain's text.
    pub fn end(&mut self, out: &mut Vec<u8>) {
        let _ = write!(ByteWriter(out), "\x1b_glass;remove;all=1\x1b\\");
        self.videos.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pizza_mesh_is_whole_and_under_the_caps() {
        let mesh = pizza_mesh();
        let v: serde_json::Value = serde_json::from_str(&mesh).unwrap();
        let positions = v["positions"].as_array().unwrap().len();
        let indices = v["indices"].as_array().unwrap();
        assert_eq!(v["uvs"].as_array().unwrap().len(), positions);
        assert_eq!(v["normals"].as_array().unwrap().len(), positions);
        assert!(indices.iter().all(|i| (i.as_u64().unwrap() as usize) < positions));
        // The storm's slices together stay under a scene's 4096 vertices.
        assert!(positions * 29 <= 4096, "{positions}");
    }

    #[test]
    fn the_scene_is_a_valid_envelope_at_its_busiest() {
        let mut show = Surreal::bare(150, 40, (1920.0, 1080.0));
        let mut out = Vec::new();
        show.scene(74.0, &mut out);
        let text = String::from_utf8(out).unwrap();
        // Two scenes: the back one under the geometry, the front over it,
        // each under the 1 MiB a scene may carry.
        assert!(text.starts_with("\x1b_glass;scene;id=61;z=-3;"));
        assert!(text.contains("\x1b_glass;scene;id=60;z=0;"));
        for op in text.split("\x1b_glass;scene;").skip(1) {
            let payload = op.split("\x1b\\").next().unwrap().rsplit(';').next().unwrap();
            assert!(payload.len() / 4 * 3 < 1 << 20, "{}", payload.len());
        }
    }

    #[test]
    fn rain_writes_only_what_changed_keeps_the_corner_rows_and_clears() {
        let mut rain = Rain::new(20, 10);
        rain.drops.iter_mut().for_each(|d| *d = (6.0, 0.0, 5.0));
        let mut out = Vec::new();
        rain.draw(&mut out, 1.0, 0.016, 1.0, 1);
        let text = String::from_utf8_lossy(&out).to_string();
        assert!(!text.is_empty());
        for row in [1, 10] {
            assert!(!text.contains(&format!("\x1b[{row};")), "row {row} is the corner's");
        }
        // Standing still, the next frame rewrites only the heads.
        let first = out.len();
        out.clear();
        rain.draw(&mut out, 1.0, 0.0, 1.0, 2);
        assert!(out.len() < first / 2, "{} of {first}", out.len());
        // Gone: one frame clears it, then nothing.
        out.clear();
        rain.draw(&mut out, 1.1, 0.0, 0.0, 3);
        assert!(!out.is_empty() && rain.cells.iter().all(|c| c.0 == ' '));
        out.clear();
        rain.draw(&mut out, 1.2, 0.0, 0.0, 4);
        assert!(out.is_empty());
    }
}
