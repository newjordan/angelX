//! The window's event loop. winit owns the main thread; the cockpit runs on
//! its own thread and talks to this one through the [`Host`].

use std::collections::VecDeque;
use std::io::{BufWriter, Write};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{UserAttentionType, Window, WindowId};

use crate::config::Options;
use crate::input::Shortcut;
use crate::layer::{Layer, LayerKind, Placement};
use crate::probe::Probe;
use crate::render::Renderer;
use crate::{Host, LockExt, Shared, Snapshot, UserEvent, clipboard, graphics, input};

/// The assumed display rate when the monitor does not say: 60 a second.
const FALLBACK_MILLIHERTZ: u32 = 60_000;

/// (one display frame, how recent a present lets an animation ride on
/// it) for a monitor refresh rate. The ride window is most of one
/// frame; both were hard-coded 60 Hz values until the scorecard
/// pointed out they are simply wrong on a faster display.
fn cadence(millihertz: Option<u32>) -> (Duration, Duration) {
    let mhz = millihertz
        .filter(|m| (24_000..=480_000).contains(m))
        .unwrap_or(FALLBACK_MILLIHERTZ);
    let frame = Duration::from_nanos(1_000_000_000_000 / u64::from(mhz));
    (frame, frame.mul_f64(0.9))
}

/// The cadence of the monitor `window` is on right now.
fn window_cadence(window: &Window) -> (Duration, Duration) {
    cadence(
        window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz()),
    )
}

/// What a requested redraw does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RedrawPlan {
    /// Present now.
    Draw,
    /// A cockpit frame presented within this display frame; the
    /// animation rides on it and asks again one display frame later.
    Ride,
    /// Nothing is waiting on a present.
    Nothing,
}

/// Decide a redraw: an owed change, unpublished layers, or an overdue
/// frame presents at once; a bare animation rides a frame the cockpit
/// just presented instead of presenting twice in one display frame.
/// Whether compositor-owned motion needs another frame. Renderer-owned
/// transitions, slides and stage layers count, and so does every
/// window layer `Layer::animating` claims (breathing, flicker, motes and
/// turning primitives): the next-frame request after a present and the
/// redraw decision must agree on this, or the chain dies after one
/// frame and the wall-clock phases jump at whatever rate the cockpit
/// happens to redraw.
fn animation_wants_frame(renderer_animating: bool, layers: &[Layer]) -> bool {
    renderer_animating || layers.iter().any(Layer::animating)
}

fn redraw_plan(
    owed: bool,
    layers_dirty: bool,
    overdue: bool,
    animating: bool,
    since_present: Duration,
    rides: Duration,
) -> RedrawPlan {
    if owed || layers_dirty || overdue {
        RedrawPlan::Draw
    } else if animating {
        if since_present < rides {
            RedrawPlan::Ride
        } else {
            RedrawPlan::Draw
        }
    } else {
        RedrawPlan::Nothing
    }
}

type App<T> = Box<dyn FnOnce(Host) -> T + Send>;

/// Open the window and run `app` on its own thread with the [`Host`].
/// Returns what `app` returns once it does; the window closes with it.
pub fn run<T, F>(options: Options, app: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(Host) -> T + Send + 'static,
{
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| format!("no window system: {e}"))?;
    let (keys, events) = mpsc::channel();
    let host = Host {
        shared: Arc::new(Shared::default()),
        proxy: Arc::new(Mutex::new(event_loop.create_proxy())),
        input: Arc::new(Mutex::new(Some(events))),
    };
    let mut glass = Glass {
        options,
        host,
        keys: Some(keys),
        app: Some(Box::new(app)),
        thread: None,
        window: None,
        renderer: None,
        current: Snapshot::default(),
        layers: Arc::new(Vec::new()),
        focused: true,
        mods: ModifiersState::empty(),
        mouse_cell: (0, 0),
        held: Vec::new(),
        scroll_px: 0.0,
        closing: 0,
        probe: Probe::from_env(),
        error: None,
        stats: Stats::from_env(),
        assembled: std::env::var("ANGEL_GLASS_TRANSITION").is_ok_and(|v| v == "0"),
        last_present: Instant::now(),
        redraw_owed: false,
        layers_dirty: false,
        animation_due: None,
        key_queue: VecDeque::new(),
        cadence: cadence(None),
        showcase: crate::showcase::Popup::default(),
        pointer: [0.0, 0.0],
        showcase_press: false,
        cursor: winit::window::CursorIcon::Default,
    };
    event_loop
        .run_app(&mut glass)
        .map_err(|e| format!("window event loop: {e}"))?;
    if let Some(error) = glass.error {
        return Err(error);
    }
    let thread = glass.thread.take().ok_or("the window never opened")?;
    if !thread.is_finished() && glass.closing > 1 {
        return Err("the window was closed before the cockpit finished".into());
    }
    match thread.join() {
        Ok(value) => Ok(value),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// `ANGEL_GLASS_DEMO=lob`: a bomb lobbed across the window over the
/// cockpit, rising toward the camera with its shadow on the ground, again
/// and again. The layer sink's height, shown.
fn lob_demo(host: &Host) {
    let id = crate::layer::LAYER_PICTURES + 1;
    host.upload_picture(graphics::Picture {
        id,
        width: 32,
        height: 32,
        rgba: bomb_sprite(),
        cols: 0,
        rows: 0,
    });
    // One lob: flight time, peak height, size; then a pause.
    const FLIGHT: f32 = 1.4;
    const PEAK: f32 = 360.0;
    const SIZE: f32 = 48.0;
    const PAUSE: f32 = 0.6;
    let started = Instant::now();
    loop {
        let t = started.elapsed().as_secs_f32() % (FLIGHT + PAUSE);
        let grid = host.grid_size();
        let (w, h) = (grid.width_px as f32, grid.height_px as f32);
        if t <= FLIGHT {
            let p = t / FLIGHT;
            let mut layer = Layer::new(
                u64::MAX - 1,
                10,
                Placement::Pixels([w * (0.15 + 0.7 * p) - SIZE / 2.0, h * 0.62, SIZE, SIZE]),
                LayerKind::Picture(id),
            );
            layer.height = PEAK * 4.0 * p * (1.0 - p);
            layer.angle = p * 2.0 * std::f32::consts::PI;
            host.set_layer(layer);
        } else {
            host.remove_layer(u64::MAX - 1);
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// `ANGEL_GLASS_DEMO=slide`: a stage whose scene changes every so often,
/// each new one sliding in from the next side round (right, below, left,
/// above), shown over the cockpit. The stage's slide, shown.
fn slide_demo(host: &Host) {
    const STAGE: u32 = crate::layer::LAYER_PICTURES + 0x50;
    const SCENES: [u32; 2] = [STAGE + 1, STAGE + 2];
    const SIZE: (u32, u32) = (96, 56);
    const EVERY: f32 = 1.5;
    // Two scenes: warm and cool checks with a bar across, so a slide reads.
    for (i, &id) in SCENES.iter().enumerate() {
        let mut rgba = Vec::with_capacity((SIZE.0 * SIZE.1 * 4) as usize);
        for y in 0..SIZE.1 {
            for x in 0..SIZE.0 {
                let check = ((x / 8 + y / 8) % 2) as u8;
                let bar = (24..32).contains(&y);
                let [r, g, b] = match (i, bar) {
                    (_, true) => [230, 222, 204],
                    (0, false) => [120 + 40 * check, 70 + 20 * check, 40],
                    _ => [40, 70 + 20 * check, 120 + 40 * check],
                };
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }
        host.upload_picture(graphics::Picture {
            id,
            width: SIZE.0,
            height: SIZE.1,
            rgba,
            cols: 0,
            rows: 0,
        });
    }
    let sides = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
    let started = Instant::now();
    loop {
        let n = (started.elapsed().as_secs_f32() / EVERY) as usize;
        let grid = host.grid_size();
        host.set_stage(crate::Stage {
            id: STAGE,
            size: SIZE,
            cols: 48,
            rows: 14,
            layers: vec![Layer::new(
                0,
                0,
                Placement::Pixels([0.0, 0.0, SIZE.0 as f32, SIZE.1 as f32]),
                LayerKind::Picture(SCENES[n % 2]),
            )],
            scene: n as u64 + 1,
            enter: Some(sides[n % 4]),
            tilt: 0.0,
            props: None,
            impact: 0.0,
        });
        let (cw, ch) = (grid.cell_width as f32, grid.cell_height as f32);
        host.set_layer(Layer::new(
            u64::MAX - 2,
            10,
            Placement::Pixels([cw * 4.0, ch * 4.0, cw * 48.0, ch * 14.0]),
            LayerKind::Picture(STAGE),
        ));
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// `ANGEL_GLASS_DEMO=blast`: the full multimedia blast over the cockpit
/// (`blast.rs`) — a 3D-warped video stream, raycast primitives, sine line
/// formations across the z axis, everything drifting like a screensaver.
/// Streams four pictures and replaces one layer group every frame, and
/// reports its own generation cost every five seconds, so the scene is a
/// measurement as much as a show. Pair with `ANGEL_GLASS_STATS=1`.
/// `ANGEL_GLASS_BLAST_X=N` (1..=64) repeats the line formation N times
/// over, for finding how many layers the compositor can carry.
fn blast_demo(host: &Host) {
    use crate::blast;
    const GROUP: u32 = 0xB1A5_7E57;
    let copies = std::env::var("ANGEL_GLASS_BLAST_X")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, 64);
    let started = Instant::now();
    let mut frames = 0u32;
    let mut generated = Duration::ZERO;
    let mut reported = Instant::now();
    loop {
        let t = started.elapsed().as_secs_f32();
        let grid = host.grid_size();
        let (w, h) = (grid.width_px as f32, grid.height_px as f32);
        if w < 50.0 || h < 50.0 {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        let began = Instant::now();
        let stream = |id: u32, width: u32, height: u32, rgba: Vec<u8>| {
            host.upload_picture(graphics::Picture {
                id,
                width,
                height,
                rgba,
                cols: 0,
                rows: 0,
            });
        };
        stream(blast::VIDEO_ID, blast::VIDEO_W, blast::VIDEO_H, blast::warped_video(t));
        stream(blast::ORBS_ID, blast::ORBS_W, blast::ORBS_H, blast::raycast_orbs(t));
        stream(blast::CUBE_ID, blast::CUBE_W, blast::CUBE_H, blast::raycast_cube(t));
        stream(blast::RIBBONS_ID, blast::RIBBONS_W, blast::RIBBONS_H, blast::line_ribbons(t));
        let layers = blast::scene_layers_x(t, (w, h), copies);
        generated += began.elapsed();
        frames += 1;
        if reported.elapsed() >= Duration::from_secs(5) {
            eprintln!(
                "blast: {frames} frames generated, {:.2} ms/frame on the CPU (4 picture streams + {} layers)",
                generated.as_secs_f64() * 1000.0 / f64::from(frames.max(1)),
                layers.len()
            );
            frames = 0;
            generated = Duration::ZERO;
            reported = Instant::now();
        }
        host.set_layer_group(GROUP, layers);
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// `ANGEL_GLASS_DEMO=cube`: a floating, turning cube with a different test
/// picture on each of its six faces (`cube.rs`), drawn by the renderer's 3D
/// pipeline into a picture and shown over the cockpit as a lifted layer with
/// its shadow on the ground, as the blast's layers are.
fn cube_demo(host: &Host) {
    use crate::cube;
    for i in 0..6u32 {
        host.upload_picture(graphics::Picture {
            id: cube::FACE_ID + i,
            width: cube::FACE_SIDE,
            height: cube::FACE_SIDE,
            rgba: cube::face_picture(i as usize),
            cols: 0,
            rows: 0,
        });
    }
    let started = Instant::now();
    loop {
        let t = started.elapsed().as_secs_f32();
        let grid = host.grid_size();
        let (w, h) = (grid.width_px as f32, grid.height_px as f32);
        if w < 50.0 || h < 50.0 {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        host.set_scene(cube::SCENE_ID, cube::scene(t), cube::TARGET);
        let size = cube::SHOWN.min(w * 0.6).min(h * 0.7);
        let mut layer = Layer::new(
            u64::MAX - 3,
            10,
            Placement::Pixels([w * 0.62 - size / 2.0, h * 0.5 - size / 2.0, size, size]),
            LayerKind::Picture(cube::SCENE_ID),
        );
        layer.height = 70.0 + 25.0 * (t * 1.1).sin();
        host.set_layer(layer);
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// `ANGEL_GLASS_DEMO=shore`: a night shoreline in the world's look
/// (`shore.rs`), filling the window under the cockpit's text. The scene is
/// drawn at the look's internal size (540 rows, or `ANGEL_GLASS_SHORE_ROWS`)
/// and the layer that shows it enlarges it by nearest, so each art pixel is
/// several window pixels, as in the reference.
fn shore_demo(host: &Host) {
    use crate::shore;
    if let Some((width, height, rgba)) = shore::knight_sprite() {
        host.upload_picture(graphics::Picture {
            id: shore::KNIGHT_ID,
            width,
            height,
            rgba,
            cols: 0,
            rows: 0,
        });
    }
    let rows = std::env::var("ANGEL_GLASS_SHORE_ROWS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(shore::ROWS);
    let started = Instant::now();
    loop {
        let t = started.elapsed().as_secs_f32();
        let grid = host.grid_size();
        let (w, h) = (grid.width_px, grid.height_px);
        if w < 50 || h < 50 {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        host.set_scene(shore::SCENE_ID, shore::scene(t, rows), (w, h));
        // Below the text: negative z.
        host.set_layer(Layer::new(
            u64::MAX - 4,
            -10,
            Placement::Pixels([0.0, 0.0, w as f32, h as f32]),
            LayerKind::Picture(shore::SCENE_ID),
        ));
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// A round iron bomb with a lit fuse, 32 × 32, straight alpha.
fn bomb_sprite() -> Vec<u8> {
    let mut rgba = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let (dx, dy) = (fx - 16.0, fy - 18.0);
            let d = (dx * dx + dy * dy).sqrt();
            let i = (y * 32 + x) * 4;
            let mut px = [0u8; 4];
            if d <= 12.5 {
                // Iron, lit from the top left.
                let light =
                    (1.0 - ((dx + 5.0).powi(2) + (dy + 5.0).powi(2)).sqrt() / 18.0).clamp(0.0, 1.0);
                let v = 28.0 + 70.0 * light * light;
                px = [v as u8, v as u8, (v + 8.0) as u8, 255];
            } else if (14.0..=17.0).contains(&fx) && (2.0..6.5).contains(&fy) {
                px = [120, 82, 40, 255];
            }
            if ((fx - 17.0).powi(2) + (fy - 2.5).powi(2)).sqrt() < 2.2 {
                px = [255, 214, 90, 255];
            }
            rgba[i..i + 4].copy_from_slice(&px);
        }
    }
    rgba
}

struct Glass<T> {
    options: Options,
    host: Host,
    /// The cockpit's input. Dropped when the window closes.
    keys: Option<Sender<Event>>,
    app: Option<App<T>>,
    thread: Option<JoinHandle<T>>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    current: Snapshot,
    layers: Arc<Vec<Layer>>,
    focused: bool,
    mods: ModifiersState,
    mouse_cell: (u16, u16),
    held: Vec<MouseButton>,
    scroll_px: f64,
    closing: u8,
    probe: Option<Arc<Probe>>,
    error: Option<String>,
    stats: Option<Stats>,
    /// The cockpit's first frame has had its assembly transition.
    assembled: bool,
    last_present: Instant,
    /// A change only the window draws (focus) still needs a frame.
    redraw_owed: bool,
    /// Layers changed since the last present. Their present waits for
    /// the redraw cycle (or the cockpit frame already on its way), so a
    /// layer set handed over mid-draw is never shown against the
    /// previous frame's cells.
    layers_dirty: bool,
    /// When an animation frame that rode on the cockpit's is due again.
    animation_due: Option<Instant>,
    /// Key-press instants awaiting their frame, for the stats key-hop
    /// trace. Only filled while stats are on.
    key_queue: VecDeque<Instant>,
    /// The current monitor's display cadence (frame, ride window),
    /// re-read when the window opens or its scale changes.
    cadence: (Duration, Duration),
    /// The floating panel over everything (`showcase.rs`).
    showcase: crate::showcase::Popup,
    /// The pointer in window pixels.
    pointer: [f32; 2],
    /// The left button went down on the showcase: its release is the
    /// showcase's too, wherever it happens.
    showcase_press: bool,
    cursor: winit::window::CursorIcon,
}

/// `ANGEL_GLASS_STATS=1`: every two seconds, one stderr line with the frames
/// presented, their render times and the pictures uploaded.
/// `ANGEL_GLASS_STATS_RAW=path` also appends each window's raw presented
/// frame times there, one JSON array a line, for bench analysis that needs
/// the series rather than a percentile someone else chose.
struct Stats {
    since: Instant,
    frames: Vec<Duration>,
    /// Per frame: build, acquire, submit, present.
    parts: Vec<[Duration; 4]>,
    /// Per key with a frame: (key → render start, key → present end).
    key_hops: Vec<(Duration, Duration)>,
    upload_bytes: usize,
    upload: Duration,
    raw: Option<std::path::PathBuf>,
}

impl Stats {
    fn from_env() -> Option<Self> {
        std::env::var("ANGEL_GLASS_STATS")
            .is_ok_and(|v| v == "1")
            .then(|| Self {
                since: Instant::now(),
                frames: Vec::new(),
                parts: Vec::new(),
                key_hops: Vec::new(),
                upload_bytes: 0,
                upload: Duration::ZERO,
                raw: std::env::var_os("ANGEL_GLASS_STATS_RAW").map(Into::into),
            })
    }

    /// Picture pixels handed to the GPU, and what applying them cost.
    fn uploads(&mut self, bytes: usize, took: Duration) {
        self.upload_bytes += bytes;
        self.upload += took;
    }

    fn frame(&mut self, took: Duration, parts: [Duration; 4]) {
        self.frames.push(took);
        self.parts.push(parts);
        let window = self.since.elapsed();
        if window < Duration::from_secs(2) {
            return;
        }
        if let Some(path) = &self.raw {
            // Before the percentile sort: the series keeps its order.
            use std::io::Write as _;
            let mut line = String::from("[");
            for (i, d) in self.frames.iter().enumerate() {
                if i > 0 {
                    line.push(',');
                }
                line.push_str(&format!("{:.3}", d.as_secs_f64() * 1000.0));
            }
            line.push_str("]\n");
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = file.write_all(line.as_bytes());
            }
        }
        self.frames.sort_unstable();
        let at = |p: usize| {
            self.frames
                .get((self.frames.len().saturating_sub(1)) * p / 100)
                .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
        };
        let worst_part = |k: usize| {
            self.parts
                .iter()
                .map(|p| p[k])
                .max()
                .unwrap_or_default()
                .as_secs_f64()
                * 1000.0
        };
        let mean_part = |k: usize| {
            self.parts.iter().map(|p| p[k].as_secs_f64()).sum::<f64>() * 1000.0
                / self.parts.len().max(1) as f64
        };
        eprintln!(
            "GLASS_PARTS build_mean_ms={:.3} build_worst_ms={:.3} acquire_mean_ms={:.3} acquire_worst_ms={:.3} submit_mean_ms={:.3} submit_worst_ms={:.3} present_mean_ms={:.3} present_worst_ms={:.3}",
            mean_part(0),
            worst_part(0),
            mean_part(1),
            worst_part(1),
            mean_part(2),
            worst_part(2),
            mean_part(3),
            worst_part(3),
        );
        eprintln!(
            "GLASS_STATS fps={:.1} render_p50_ms={:.3} render_p95_ms={:.3} render_worst_ms={:.3} uploads_kib={} upload_ms={:.2}",
            self.frames.len() as f64 / window.as_secs_f64(),
            at(50),
            at(95),
            at(100),
            self.upload_bytes / 1024,
            self.upload.as_secs_f64() * 1000.0,
        );
        if !self.key_hops.is_empty() {
            let mut hops: Vec<Duration> = self.key_hops.iter().map(|h| h.0).collect();
            let mut totals: Vec<Duration> = self.key_hops.iter().map(|h| h.1).collect();
            hops.sort_unstable();
            totals.sort_unstable();
            let pick = |v: &[Duration], p: usize| {
                v.get((v.len().saturating_sub(1)) * p / 100)
                    .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
            };
            eprintln!(
                "GLASS_KEYHOP n={} hop_p50_ms={:.3} hop_p95_ms={:.3} total_p50_ms={:.3} total_p95_ms={:.3}",
                hops.len(),
                pick(&hops, 50),
                pick(&hops, 95),
                pick(&totals, 50),
                pick(&totals, 95),
            );
        }
        let raw = self.raw.take();
        *self = Self {
            since: Instant::now(),
            frames: Vec::new(),
            parts: Vec::new(),
            key_hops: Vec::new(),
            upload_bytes: 0,
            upload: Duration::ZERO,
            raw,
        };
    }
}

impl<T: Send + 'static> Glass<T> {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attributes = Window::default_attributes()
            .with_title(self.options.title.clone())
            .with_transparent(true);
        #[cfg(all(unix, not(target_os = "macos")))]
        let attributes = winit::platform::wayland::WindowAttributesExtWayland::with_name(
            attributes,
            self.options.app_id.clone(),
            self.options.app_id.clone(),
        );
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| format!("cannot open a window: {e}"))?,
        );
        let renderer = Renderer::new(Arc::clone(&window), &self.options)?;
        let cell = renderer.cell();
        eprintln!(
            "GLASS: gpu={} present={:?} alpha={:?} cell={}x{}",
            renderer.adapter_name,
            renderer.present_mode,
            renderer.alpha_mode,
            cell.width,
            cell.height
        );
        let (width, height) = renderer.pixels_for(self.options.columns, self.options.rows);
        let _ = window.request_inner_size(PhysicalSize::new(width, height));
        *self.host.shared.grid.lock_recover() = renderer.grid_size();
        self.cadence = window_cadence(&window);
        self.window = Some(window);
        self.renderer = Some(renderer);
        // The first frame maps the window on Wayland.
        self.draw(true);

        let app = self.app.take().ok_or("the cockpit already ran")?;
        let host = self.host.clone();
        self.thread = Some(
            std::thread::Builder::new()
                .name("cockpit".into())
                // The terminal cockpit runs on the main thread's stack.
                .stack_size(64 << 20)
                .spawn(move || {
                    // The window closes with the cockpit, even when it panics.
                    struct ExitOnDrop(Host);
                    impl Drop for ExitOnDrop {
                        fn drop(&mut self) {
                            self.0.send(UserEvent::Exit);
                        }
                    }
                    let _exit = ExitOnDrop(host.clone());
                    app(host)
                })
                .map_err(|e| format!("cockpit thread: {e}"))?,
        );
        if let (Some(probe), Some(keys)) = (&self.probe, &self.keys) {
            probe.start(self.host.clone(), keys.clone());
        }
        self.environment_extras();
        Ok(())
    }

    /// `ANGEL_GLASS_DEMO=1` puts one translucent quad above the text;
    /// `ANGEL_GLASS_SHOT=path.png` saves a frame after a delay.
    fn environment_extras(&self) {
        if std::env::var("ANGEL_GLASS_DEMO").is_ok_and(|v| v == "1") {
            let grid = self.host.grid_size();
            let width = 34.0_f32.min(f32::from(grid.cols));
            self.host.set_layer(Layer::new(
                u64::MAX,
                1,
                Placement::Cells([f32::from(grid.cols) - width - 2.0, 2.0, width, 9.0]),
                LayerKind::Solid([0.0, 0.85, 1.0, 0.22]),
            ));
        }
        if std::env::var("ANGEL_GLASS_DEMO").is_ok_and(|v| v == "lob") {
            let host = self.host.clone();
            std::thread::spawn(move || lob_demo(&host));
        }
        if std::env::var("ANGEL_GLASS_DEMO").is_ok_and(|v| v == "slide") {
            let host = self.host.clone();
            std::thread::spawn(move || slide_demo(&host));
        }
        if std::env::var("ANGEL_GLASS_DEMO").is_ok_and(|v| v == "cube") {
            let host = self.host.clone();
            std::thread::spawn(move || cube_demo(&host));
        }
        if std::env::var("ANGEL_GLASS_DEMO").is_ok_and(|v| v == "shore") {
            let host = self.host.clone();
            std::thread::spawn(move || shore_demo(&host));
        }
        if std::env::var("ANGEL_GLASS_DEMO").is_ok_and(|v| v == "blast") {
            let host = self.host.clone();
            std::thread::spawn(move || blast_demo(&host));
        }
        if let Some(dir) = std::env::var_os("ANGEL_GLASS_BLAST_FILM") {
            // Film the live scene (the blast, normally) at 30 frames a
            // second through the same capture path as a still, then:
            //   ffmpeg -framerate 30 -i frame-%03d.png blast.mp4
            let seconds: f64 = std::env::var("ANGEL_GLASS_BLAST_FILM_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v: &f64| v.is_finite())
                .map_or(6.0, |v: f64| v.clamp(1.0, 60.0));
            let delay = std::env::var("ANGEL_GLASS_BLAST_FILM_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000u64);
            let host = self.host.clone();
            std::thread::spawn(move || {
                let dir = std::path::PathBuf::from(dir);
                let _ = std::fs::create_dir_all(&dir);
                std::thread::sleep(Duration::from_millis(delay));
                let frames = (seconds * 30.0).round() as u32;
                let started = std::time::Instant::now();
                for i in 0..frames {
                    host.capture(dir.join(format!("frame-{i:03}.png")));
                    let due = Duration::from_secs_f64(f64::from(i + 1) / 30.0);
                    if let Some(rest) = due.checked_sub(started.elapsed()) {
                        std::thread::sleep(rest);
                    }
                }
                eprintln!(
                    "GLASS: filmed {frames} blast frames into {}",
                    dir.display()
                );
            });
        }
        if let Some(dir) = std::env::var_os("ANGEL_GLASS_FILM") {
            let delay = std::env::var("ANGEL_GLASS_FILM_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8000);
            let host = self.host.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(delay));
                host.send(UserEvent::Film(dir.into()));
            });
        }
        if let Some(path) = std::env::var_os("ANGEL_GLASS_SHOT") {
            let delay = std::env::var("ANGEL_GLASS_SHOT_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000);
            let close = std::env::var("ANGEL_GLASS_SHOT_CLOSE").is_ok_and(|v| v == "1");
            let host = self.host.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(delay));
                host.capture(path);
                if close {
                    std::thread::sleep(Duration::from_millis(500));
                    host.send(UserEvent::Close);
                }
            });
        }
    }

    /// A frame owed a redraw shows within this long.
    fn overdue(&self) -> bool {
        self.last_present.elapsed() >= Duration::from_millis(50)
    }

    /// Present the latest frame. Without `force`, only when there is a new one.
    fn draw(&mut self, force: bool) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        // Pictures handed over go up before the frame draws. A showcase
        // frame arriving after this asks for its own wake-up.
        self.host
            .shared
            .showcase_frame_waiting
            .store(false, std::sync::atomic::Ordering::Release);
        let pictures = std::mem::take(&mut *self.host.shared.pictures.lock_recover());
        if !pictures.is_empty() {
            let bytes: usize = pictures.iter().map(|picture| picture.rgba.len()).sum();
            let began = Instant::now();
            renderer.apply(pictures);
            if let Some(stats) = &mut self.stats {
                stats.uploads(bytes, began.elapsed());
            }
        }
        let stages = std::mem::take(&mut *self.host.shared.stages.lock_recover());
        for (id, stage) in stages {
            renderer.set_stage(id, stage);
        }
        let scenes = std::mem::take(&mut *self.host.shared.scenes.lock_recover());
        for (id, scene) in scenes {
            renderer.set_scene(id, scene);
        }
        let fresh = {
            let mut slot = self.host.shared.slot.lock_recover();
            let fresh = slot.fresh;
            if fresh {
                std::mem::swap(&mut slot.snapshot, &mut self.current);
                slot.fresh = false;
            }
            fresh
        };
        if !fresh && !force {
            return;
        }
        // The first frame with content assembles itself.
        if !self.assembled && fresh && self.current.cells.iter().any(|c| c.symbol() != " ") {
            self.assembled = true;
            renderer.start_transition();
        }
        self.layers = Arc::clone(&self.host.shared.layers.lock_recover());
        let meshes = std::mem::take(&mut *self.host.shared.meshes.lock_recover());
        for (id, mesh) in &meshes {
            renderer.upload_mesh(*id, mesh);
        }
        {
            use std::sync::atomic::Ordering;
            let at = f32::from_bits(self.host.shared.showcase_progress.load(Ordering::Relaxed));
            let paused = self.host.shared.showcase_paused.load(Ordering::Relaxed);
            self.showcase.progress = (!at.is_nan()).then_some((at, paused));
        }
        renderer.overlay = self.showcase.layers();
        if let Some(scene) = self.showcase.take_scene() {
            renderer.set_scene(crate::showcase::MODEL_PICTURE, scene);
        }
        let tips = self.showcase.take_pictures();
        if !tips.is_empty() {
            renderer.apply(tips);
        }
        let started = Instant::now();
        if renderer.render(&self.current, &self.layers, self.focused) {
            self.last_present = Instant::now();
            self.redraw_owed = false;
            self.layers_dirty = false;
            let took = started.elapsed();
            if let Some(probe) = &self.probe {
                probe.presented(&self.current, took);
            }
            if let Some(stats) = &mut self.stats {
                if fresh {
                    // Discard keys whose frame never came (or came
                    // without fresh cells): they would poison the
                    // next key's hop with their own age.
                    while self
                        .key_queue
                        .front()
                        .is_some_and(|at| at.elapsed() > Duration::from_millis(250))
                    {
                        self.key_queue.pop_front();
                    }
                    if let Some(key_at) = self.key_queue.pop_front() {
                        stats.key_hops.push((
                            started.saturating_duration_since(key_at),
                            key_at.elapsed(),
                        ));
                    }
                }
                stats.frame(took, renderer.timing);
            }
        }
        // Compositor-owned motion keeps the frames coming on its own,
        // at the animation pace the redraw handler metes out.
        if animation_wants_frame(
            renderer.animating() || self.showcase.animating(),
            &self.layers,
        ) && let Some(window) = &self.window
        {
            window.request_redraw();
        }
    }

    fn send(&self, event: Event) {
        if let Some(keys) = &self.keys {
            let _ = keys.send(event);
        }
    }

    /// Fit the grid to the window; tell the cockpit when the cell count changed.
    fn regrid(&mut self) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        let grid = renderer.grid_size();
        let old = std::mem::replace(&mut *self.host.shared.grid.lock_recover(), grid);
        if (old.cols, old.rows) != (grid.cols, grid.rows) {
            self.send(Event::Resize(grid.cols, grid.rows));
        }
    }

    fn close(&mut self, event_loop: &ActiveEventLoop) {
        self.closing += 1;
        let finished = self.thread.as_ref().is_none_or(JoinHandle::is_finished);
        if finished || self.closing > 1 {
            event_loop.exit();
            return;
        }
        // The cockpit's reader sees the window closed and it quits the way it
        // quits when its terminal goes away.
        self.keys = None;
    }

    fn on_key(&mut self, event: &KeyEvent) {
        if let Some(shortcut) = input::shortcut(event, self.mods) {
            if event.state == ElementState::Pressed {
                match shortcut {
                    Shortcut::Paste if !event.repeat => self.paste(),
                    Shortcut::Paste => {}
                    Shortcut::ZoomIn => self.zoom(1.0),
                    Shortcut::ZoomOut => self.zoom(-1.0),
                    Shortcut::ZoomReset => self.zoom(0.0),
                    Shortcut::Transition => {
                        if let Some(renderer) = self.renderer.as_mut() {
                            renderer.start_transition();
                        }
                        self.draw(true);
                    }
                }
            }
            return;
        }
        // Escape closes the showcase before the cockpit hears it.
        if event.state == ElementState::Pressed
            && event.logical_key
                == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
            && self.showcase.escape(&self.host.shared)
        {
            self.update_cursor();
            self.draw(true);
            return;
        }
        // Any key ends a transition; the key still goes to the cockpit.
        if event.state == ElementState::Pressed
            && let Some(renderer) = self.renderer.as_mut()
            && renderer.animating()
        {
            renderer.skip_transition();
            self.draw(true);
        }
        let game = self
            .host
            .shared
            .game_keyboard
            .load(std::sync::atomic::Ordering::Relaxed);
        if (game || event.state == ElementState::Pressed)
            && let Some(key) = input::key(event, self.mods, game)
        {
            if self.stats.is_some() && event.state == ElementState::Pressed {
                self.key_queue.push_back(Instant::now());
            }
            self.send(Event::Key(key));
        }
    }

    fn paste(&self) {
        let Some(keys) = self.keys.clone() else {
            return;
        };
        std::thread::spawn(move || {
            if let Some(text) = clipboard::paste() {
                let _ = keys.send(Event::Paste(text));
            }
        });
    }

    fn zoom(&mut self, steps: f32) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.zoom(steps);
        }
        self.regrid();
        self.draw(true);
    }

    fn mouse(&self, kind: MouseEventKind) {
        // The cells as displayed. A foreground scroll's page maps them back
        // to its uncurled rows on the cockpit's side (`App::setup_mouse`);
        // mapping here as well took the paper's offset off twice, a row or
        // two above the item under the pointer.
        let (column, row) = self.mouse_cell;
        self.send(Event::Mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: input::modifiers(self.mods),
        }));
    }

    fn on_pointer(&mut self, position: PhysicalPosition<f64>) {
        self.pointer = [position.x as f32, position.y as f32];
        if self.showcase.is_open() {
            let window = self.window_px();
            let (taken, redraw) = self.showcase.pointer(self.pointer, window);
            self.update_cursor();
            if redraw {
                self.draw(true);
            }
            if taken {
                return;
            }
        }
        let Some(renderer) = &self.renderer else {
            return;
        };
        let cell = renderer.cell_at(position.x, position.y);
        if cell == self.mouse_cell {
            return;
        }
        self.mouse_cell = cell;
        let kind = match self.held.last() {
            Some(&button) => MouseEventKind::Drag(button),
            None => MouseEventKind::Moved,
        };
        self.mouse(kind);
    }

    fn on_wheel(&mut self, delta: MouseScrollDelta) {
        let cell_height = f64::from(self.host.grid_size().cell_height.max(1));
        if self.showcase.is_open() {
            let px = match delta {
                MouseScrollDelta::LineDelta(_, y) => y as f64 * cell_height * 3.0,
                MouseScrollDelta::PixelDelta(p) => p.y,
            };
            if self.showcase.wheel(self.pointer, px as f32) {
                self.draw(true);
                return;
            }
        }
        let lines = match delta {
            MouseScrollDelta::LineDelta(_, y) => {
                let y = f64::from(y);
                if y == 0.0 {
                    0.0
                } else {
                    y.signum() * y.abs().round().max(1.0)
                }
            }
            MouseScrollDelta::PixelDelta(p) => {
                self.scroll_px += p.y;
                let whole = (self.scroll_px / cell_height).trunc();
                self.scroll_px -= whole * cell_height;
                whole
            }
        };
        let kind = if lines > 0.0 {
            MouseEventKind::ScrollUp
        } else {
            MouseEventKind::ScrollDown
        };
        for _ in 0..(lines.abs() as usize).min(10) {
            self.mouse(kind);
        }
    }

    /// The window's size in pixels.
    fn window_px(&self) -> (f32, f32) {
        let grid = self.host.grid_size();
        (grid.width_px as f32, grid.height_px as f32)
    }

    /// A button over the showcase, or the release of one pressed on it,
    /// is the showcase's alone. True when it took the event.
    fn showcase_button(&mut self, state: ElementState, button: winit::event::MouseButton) -> bool {
        if !self.showcase.is_open() && !self.showcase_press {
            return false;
        }
        let window = self.window_px();
        let taken = match (state, button) {
            (ElementState::Pressed, winit::event::MouseButton::Left) => {
                let asked = (self.host.showcase_steps(), self.host.showcase_copies());
                let taken = self.showcase.press(self.pointer, window, &self.host);
                self.showcase_press = taken;
                // An arrow asked for another showing, or the copy button
                // for a copy: wake the cockpit's loop now (it answers
                // there) rather than on its next idle tick.
                if (self.host.showcase_steps(), self.host.showcase_copies()) != asked {
                    self.send(Event::FocusGained);
                }
                taken
            }
            (ElementState::Released, winit::event::MouseButton::Left) if self.showcase_press => {
                self.showcase_press = false;
                self.showcase.release(self.pointer, &self.host);
                true
            }
            (ElementState::Pressed, _) => {
                self.showcase.spot(self.pointer) != crate::showcase::Spot::Outside
            }
            _ => false,
        };
        if taken {
            self.update_cursor();
            self.draw(true);
        }
        taken
    }

    /// The pointer's look: the showcase's over it, the default elsewhere.
    fn update_cursor(&mut self) {
        let over = self.showcase.grabbing()
            || self.showcase.spot(self.pointer) != crate::showcase::Spot::Outside;
        let cursor = if over {
            self.showcase.cursor()
        } else {
            winit::window::CursorIcon::Default
        };
        if cursor != self.cursor {
            self.cursor = cursor;
            if let Some(window) = &self.window {
                window.set_cursor(cursor);
            }
        }
    }

    /// Drop the renderer (surface, device) before the window it draws into.
    fn release(&mut self) {
        self.renderer = None;
        self.window = None;
    }

    /// `ANGEL_GLASS_FILM`: the assembly transition at 30 frames a second, as
    /// numbered PNGs, for review.
    fn film(&mut self, dir: &std::path::Path) {
        let _ = std::fs::create_dir_all(dir);
        // Thirty frames a second, so the film plays at the transition's speed.
        let frames = (crate::transition::DURATION * 30.0).round() as u32;
        for i in 0..=frames {
            let seconds = crate::transition::DURATION * i as f32 / frames as f32;
            let Some(renderer) = self.renderer.as_mut() else {
                return;
            };
            let (width, height, pixels) =
                renderer.capture_transition(&self.current, &self.layers, seconds);
            write_png(
                &dir.join(format!("frame-{i:03}.png")),
                width,
                height,
                pixels,
            );
        }
        eprintln!("GLASS: filmed {} frames into {}", frames + 1, dir.display());
    }

    fn save_capture(&mut self, path: &std::path::Path) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let (width, height, pixels) = renderer.capture(&self.current, &self.layers);
        write_png(path, width, height, pixels);
        eprintln!("GLASS: saved {} ({width}x{height})", path.display());
    }
}

/// Save captured RGBA (premultiplied) as a PNG (straight alpha).
fn write_png(path: &std::path::Path, width: u32, height: u32, mut pixels: Vec<u8>) {
    // PNG wants straight alpha.
    for p in pixels.chunks_exact_mut(4) {
        let a = u16::from(p[3]);
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = (u16::from(*c) * 255 / a).min(255) as u8;
            }
        }
    }
    let written = std::fs::File::create(path)
        .map_err(|e| e.to_string())
        .and_then(|file| {
            let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            writer
                .write_image_data(&pixels)
                .map_err(|e| e.to_string())?;
            writer.finish().map_err(|e| e.to_string())
        });
    if let Err(e) = written {
        eprintln!("GLASS: could not save {}: {e}", path.display());
    }
    let _ = std::io::stderr().flush();
}

impl<T: Send + 'static> ApplicationHandler<UserEvent> for Glass<T> {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        // An animation frame that rode on the cockpit's is due: ask again.
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. })
            && self.animation_due.is_some_and(|due| Instant::now() >= due)
        {
            self.animation_due = None;
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    /// The showcase's squire moves now and then while it sits (a leg
    /// swing, a wave): wake for his next move, and only then.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.animation_due.is_none()
            && let Some(due) = self.showcase.next_wake()
        {
            self.animation_due = Some(due);
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(due));
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(error) = self.open(event_loop) {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Frame => self.draw(false),
            UserEvent::Pictures => self.draw(true),
            // Layers are handed over mid-draw (the Delve's thrown things,
            // the map's dock): presenting here would show them against
            // the previous frame's cells, then present again at the
            // flush — two presents a frame, the first one wrong. Ask
            // for a redraw instead: the frame on its way presents them
            // with its own cells, and a layers-only change (a demo's
            // thread) presents on the redraw that follows.
            UserEvent::Layers => {
                self.layers_dirty = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            UserEvent::Showcase => {
                if let Some(renderer) = self.renderer.as_mut() {
                    let grid = self.host.grid_size();
                    let pictures = self.showcase.take_request(
                        &self.host,
                        (grid.width_px as f32, grid.height_px as f32),
                        renderer.scale(),
                        &self.options.font_family,
                    );
                    renderer.apply(pictures);
                }
                self.update_cursor();
                self.draw(true);
            }
            UserEvent::Title(title) => {
                if let Some(window) = &self.window {
                    window.set_title(&title);
                }
            }
            UserEvent::Attention => {
                if let Some(window) = &self.window
                    && !self.focused
                {
                    window.request_user_attention(Some(UserAttentionType::Informational));
                }
            }
            UserEvent::Capture(path) => self.save_capture(&path),
            UserEvent::Film(dir) => self.film(&dir),
            UserEvent::Close => self.close(event_loop),
            UserEvent::Exit => event_loop.exit(),
            UserEvent::ExitProcess(code) => {
                self.release();
                std::process::exit(code);
            }
        }
    }

    /// Let go of the GPU and the window while the display connection is
    /// still open; the event loop closes it after this returns.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.release();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close(event_loop),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                self.showcase
                    .fit_window((size.width as f32, size.height as f32));
                self.regrid();
                self.draw(true);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_scale(scale_factor as f32);
                    // The showcase's chrome, drawn again for the new scale.
                    let pictures = self.showcase.rescale(scale_factor as f32);
                    renderer.apply(pictures);
                }
                // A scale change usually means a new monitor; its
                // refresh rate comes with it.
                if let Some(window) = &self.window {
                    self.cadence = window_cadence(window);
                }
                self.regrid();
            }
            WindowEvent::RedrawRequested => {
                let animating = animation_wants_frame(
                    self.renderer.as_ref().is_some_and(Renderer::animating)
                        || self.showcase.animating(),
                    &self.layers,
                );
                match redraw_plan(
                    self.redraw_owed,
                    self.layers_dirty,
                    self.overdue(),
                    animating,
                    self.last_present.elapsed(),
                    self.cadence.1,
                ) {
                    RedrawPlan::Draw => self.draw(true),
                    // A frame from the cockpit was presented within this
                    // display frame (a game at 60 a second): the animation
                    // rides on it rather than presenting again, and asks
                    // again a display frame after it (a timer, not a spin).
                    RedrawPlan::Ride => {
                        let due = self.last_present + self.cadence.0;
                        self.animation_due = Some(due);
                        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(due));
                    }
                    RedrawPlan::Nothing => {}
                }
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.send(if focused {
                    Event::FocusGained
                } else {
                    Event::FocusLost
                });
                // The cockpit answers focus with a frame, which shows the
                // cursor change; draw here only if it hasn't by the next
                // compositor frame. A second present now would take that
                // frame from the cockpit's.
                self.redraw_owed = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(mods) => self.mods = mods.state(),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.on_key(&event),
            WindowEvent::CursorMoved { position, .. } => self.on_pointer(position),
            WindowEvent::MouseInput { state, button, .. } => {
                if self.showcase_button(state, button) {
                    return;
                }
                if let Some(button) = input::button(button) {
                    match state {
                        ElementState::Pressed => {
                            self.held.push(button);
                            self.mouse(MouseEventKind::Down(button));
                        }
                        ElementState::Released => {
                            self.held.retain(|&held| held != button);
                            self.mouse(MouseEventKind::Up(button));
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => self.on_wheel(delta),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JUST_NOW: Duration = Duration::from_millis(1);
    const LONG_AGO: Duration = Duration::from_millis(100);
    const RIDES: Duration = Duration::from_millis(15);

    #[test]
    fn unpublished_layers_draw_on_the_redraw_not_on_arrival() {
        // A layer set handed over mid-draw waits for the redraw cycle:
        // the plan presents it then, even though a present just
        // happened and nothing is animating. (Presenting on arrival
        // showed new layers against the previous frame's cells, then
        // presented again at the flush — the gameplay flicker.)
        assert_eq!(
            redraw_plan(false, true, false, false, JUST_NOW, RIDES),
            RedrawPlan::Draw
        );
    }

    #[test]
    fn a_redraw_with_nothing_waiting_does_nothing() {
        assert_eq!(
            redraw_plan(false, false, false, false, JUST_NOW, RIDES),
            RedrawPlan::Nothing
        );
        assert_eq!(
            redraw_plan(false, false, false, false, LONG_AGO, RIDES),
            RedrawPlan::Nothing
        );
    }

    #[test]
    fn an_animation_rides_a_just_presented_frame() {
        assert_eq!(
            redraw_plan(false, false, false, true, JUST_NOW, RIDES),
            RedrawPlan::Ride
        );
        assert_eq!(
            redraw_plan(false, false, false, true, LONG_AGO, RIDES),
            RedrawPlan::Draw
        );
    }

    #[test]
    fn every_autonomous_layer_kind_keeps_the_frame_chain_alive() {
        // The regression: the post-present next-frame request only knew
        // breathing layers, so a turning primitive (the object dock),
        // flicker or motes advanced only when the cockpit happened to
        // redraw — smooth while typing, skipping frames while idle,
        // because their phases run on the wall clock.
        let still = Layer::new(1, 0, Placement::Cells([0.0, 0.0, 1.0, 1.0]), LayerKind::Solid([1.0; 4]));
        assert!(!animation_wants_frame(false, std::slice::from_ref(&still)));
        assert!(animation_wants_frame(true, &[]), "renderer-owned motion");
        let mut breathing = still.clone();
        breathing.breathe = 1.0;
        assert!(animation_wants_frame(false, std::slice::from_ref(&breathing)));
        let mut flickering = still.clone();
        flickering.flicker = 0.5;
        assert!(animation_wants_frame(false, std::slice::from_ref(&flickering)));
        let motes = Layer::new(
            2,
            0,
            Placement::Cells([0.0, 0.0, 1.0, 1.0]),
            LayerKind::Particles(crate::layer::Particles {
                color: [1.0; 4],
                count: 1,
                size: 2.0,
                life: 1.0,
                rise: 1.0,
                sway: 0.0,
                seed: 1,
                burst: None,
            }),
        );
        assert!(animation_wants_frame(false, std::slice::from_ref(&motes)));
        let turning = Layer::new(
            3,
            0,
            Placement::Cells([0.0, 0.0, 1.0, 1.0]),
            LayerKind::Primitive { shape: 0, color: [1.0; 4], phase: 0.0, speed: 0.4 },
        );
        assert!(animation_wants_frame(false, std::slice::from_ref(&turning)));
        let frozen = Layer::new(
            4,
            0,
            Placement::Cells([0.0, 0.0, 1.0, 1.0]),
            LayerKind::Primitive { shape: 0, color: [1.0; 4], phase: 0.0, speed: 0.0 },
        );
        assert!(!animation_wants_frame(false, std::slice::from_ref(&frozen)));
    }

    #[test]
    fn cadence_derives_from_refresh_with_a_60hz_fallback() {
        let (frame, rides) = cadence(Some(60_000));
        assert_eq!(frame, Duration::from_nanos(16_666_666));
        assert!((rides.as_secs_f64() - 0.015).abs() < 0.0002);
        let (frame, _) = cadence(Some(144_000));
        assert_eq!(frame, Duration::from_nanos(6_944_444));
        let (frame, _) = cadence(Some(120_000));
        assert_eq!(frame, Duration::from_nanos(8_333_333));
        // Unknown and implausible rates fall back to 60.
        assert_eq!(cadence(None).0, cadence(Some(60_000)).0);
        assert_eq!(cadence(Some(0)).0, cadence(Some(60_000)).0);
        assert_eq!(cadence(Some(9_999_999)).0, cadence(Some(60_000)).0);
    }

    #[test]
    fn owed_and_overdue_frames_always_draw() {
        assert_eq!(
            redraw_plan(true, false, false, true, JUST_NOW, RIDES),
            RedrawPlan::Draw
        );
        assert_eq!(
            redraw_plan(false, false, true, true, JUST_NOW, RIDES),
            RedrawPlan::Draw
        );
    }
}
