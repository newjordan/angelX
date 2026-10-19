//! The showcase: one floating panel over everything the window draws,
//! holding a picture at its own size — an image, a video's frames, a
//! rendered page, a 3D scene drawn into the picture — with a close
//! button, a title bar to drag it by and a corner grip to size it.
//!
//! The window owns it outright: the pointer is read here in window
//! pixels, so moving and sizing follow the hand at the display's pace
//! with no trip through the cockpit's cells. The cockpit only hands
//! over what to show (`Host::showcase`, `Host::showcase_frame`) and
//! learns that the operator closed it (`Host::showcase_live`).
//!
//! The panel is drawn as layers above the text and the foreground
//! scroll: a nine-sliced frame (rounded, hairline-edged, its shadow
//! baked in), the title, the two controls and the content, all from
//! pictures this module makes once per open at the window's scale.

use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::graphics::Picture;
use crate::layer::{LAYER_PICTURES, Layer, LayerKind, Placement};
use crate::squire;
use crate::wizard;
pub use crate::squire::Livery;
use crate::{Host, LockExt, UserEvent};

/// The picture the showcase shows. Producers upload into it
/// (`Host::showcase_frame`) or draw a scene into it (`Host::set_scene`).
pub const SHOWCASE_PICTURE: u32 = LAYER_PICTURES + 0xA0;
const FRAME_PICTURE: u32 = LAYER_PICTURES + 0xA1;
const TITLE_PICTURE: u32 = LAYER_PICTURES + 0xA2;
const ICONS_PICTURE: u32 = LAYER_PICTURES + 0xA3;
const TIP_PICTURE: u32 = LAYER_PICTURES + 0xA6;
/// A model's picture: the window draws its scene into it.
pub(crate) const MODEL_PICTURE: u32 = LAYER_PICTURES + 0xA7;
/// The pivot the window turns a model by; a model's nodes hang from it.
pub const MODEL_ROOT: u64 = 1;

/// A place in the content that answers the pointer: a chart's bar or
/// point. `rect` is where it answers and `anchor` where it is, both in
/// the content's own pixels; `text` the lines its tip shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub rect: [f32; 4],
    pub anchor: [f32; 2],
    pub text: Vec<String>,
}

/// How the content sits in the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Whole, aspect kept, letterboxed: pictures, video, scenes.
    Contain,
    /// Across the panel's width, scrolled top to bottom by the wheel:
    /// pages of text.
    Width,
}

/// One showing: what it is called, the size of its picture and how it
/// sits, who brings it in and in what colours, and whether there are
/// showings before and after it to step to.
#[derive(Clone, Debug)]
pub struct Showing {
    pub title: String,
    pub size: (u32, u32),
    pub fit: Fit,
    pub herald: Herald,
    /// The squire's colours: the serving house's, `None` for the Keep's.
    pub livery: Option<Livery>,
    /// A formation's other houses, who come along.
    pub retinue: Vec<Livery>,
    /// Who each squire is, the lead first then the retinue: the lines a
    /// squire's tag shows under the pointer (his name and house, his
    /// epithet).
    pub names: Vec<Vec<String>>,
    pub back: bool,
    pub forward: bool,
}

impl Showing {
    pub fn new(title: &str, size: (u32, u32), fit: Fit) -> Self {
        Self {
            title: title.to_string(),
            size,
            fit,
            herald: Herald::Squire,
            livery: None,
            retinue: Vec::new(),
            names: Vec::new(),
            back: false,
            forward: false,
        }
    }
}

/// Who brings the showing in: the squire, who kicks it open, or the
/// wizard, who conjures it (charts).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Herald {
    #[default]
    Squire,
    Wizard,
}

pub(crate) enum Request {
    Open {
        title: String,
        size: (u32, u32),
        fit: Fit,
        livery: Option<Livery>,
        herald: Herald,
        retinue: Vec<Livery>,
        names: Vec<Vec<String>>,
        nav: (bool, bool),
    },
    Close,
}

impl Host {
    /// Open the showcase (or give the open one new content) for a
    /// `size`-pixel picture titled `title`. The pixels go up through
    /// [`Host::showcase_frame`], before or after. Returns the showing's
    /// serial: [`Host::showcase_live`] answers it until this showing is
    /// replaced or the operator closes it.
    /// The squire who brings it in wears `livery`: the serving house's
    /// colours, or the Keep's own when none serves.
    pub fn showcase(&self, showing: Showing) -> u64 {
        let serial = self.shared.showcase_serial.fetch_add(1, Ordering::Relaxed) + 1;
        self.shared.showcase_live.store(serial, Ordering::Release);
        // A new showing plays nothing until its producer says so.
        self.shared.showcase_progress.store(f32::NAN.to_bits(), Ordering::Relaxed);
        self.shared.showcase_seek.store(f32::NAN.to_bits(), Ordering::Relaxed);
        let Showing { title, size, fit, herald, livery, retinue, names, back, forward } = showing;
        *self.shared.showcase.lock_recover() = Some(Request::Open {
            title,
            size,
            fit,
            livery,
            herald,
            retinue,
            names,
            nav: (back, forward),
        });
        self.send(UserEvent::Showcase);
        serial
    }

    /// The showcase's pixels, straight RGBA of exactly `width × height`.
    /// Send again to change them (a video's next frame); the window
    /// overwrites its texture in place.
    pub fn showcase_frame(&self, width: u32, height: u32, rgba: Vec<u8>) {
        if u64::from(width) * u64::from(height) * 4 != rgba.len() as u64 {
            return;
        }
        {
            // Only the newest frame matters: one still waiting to go up is
            // replaced, not queued behind, so a window that falls behind a
            // video catches up instead of uploading every frame it missed
            // (and keeping the pointer waiting behind them).
            let mut queue = self.shared.pictures.lock_recover();
            queue.retain(|p| p.id != SHOWCASE_PICTURE);
            queue.push(Picture {
                id: SHOWCASE_PICTURE,
                width,
                height,
                rgba,
                cols: 0,
                rows: 0,
            });
        }
        // One wake-up for however many frames arrive before it is served.
        if !self.shared.showcase_frame_waiting.swap(true, Ordering::AcqRel) {
            self.send(UserEvent::Pictures);
        }
    }

    /// Close the showcase, as its close button does.
    pub fn close_showcase(&self) {
        self.shared.showcase_live.store(0, Ordering::Release);
        *self.shared.showcase.lock_recover() = Some(Request::Close);
        self.send(UserEvent::Showcase);
    }

    /// Whether showing `serial` is still up: false once it is replaced
    /// or closed. A producer streaming frames stops when this goes false.
    pub fn showcase_live(&self, serial: u64) -> bool {
        serial != 0 && self.shared.showcase_live.load(Ordering::Acquire) == serial
    }

    /// The marks of the next showing (a chart's bars and points): hand
    /// them over before [`Host::showcase`].
    pub fn showcase_marks(&self, marks: Vec<Mark>) {
        *self.shared.showcase_marks.lock_recover() = marks;
    }

    /// Steps through the showings the operator asked for with the panel's
    /// arrows so far: back is -1, forward +1. The producer keeps the
    /// history and answers by showing that one.
    pub fn showcase_steps(&self) -> i32 {
        self.shared.showcase_steps.load(Ordering::Relaxed)
    }

    /// The model of the next showing: 3D nodes (hanging from
    /// [`MODEL_ROOT`], fit in a unit sphere at the origin) the window
    /// turns on a turntable and lets the operator turn by dragging. Hand
    /// it over before [`Host::showcase`]; its meshes go up first
    /// ([`Host::upload_mesh`]).
    pub fn showcase_model(&self, nodes: Vec<crate::scene::Node>) {
        *self.shared.showcase_model.lock_recover() = Some(nodes);
    }

    /// A mesh for scenes to name (`MeshRef::Uploaded(id)`).
    pub fn upload_mesh(&self, id: u32, mesh: crate::scene::MeshData) {
        self.shared.meshes.lock_recover().push((id, mesh));
        self.shared.pictures_waiting.store(true, Ordering::Release);
    }

    /// Where a playing showing is, 0 to 1 (`None` for one that does not
    /// play), and whether it is paused: the panel draws its progress and
    /// a pause mark, and a click on the progress line asks to seek.
    pub fn showcase_progress(&self, at: Option<f32>, paused: bool) {
        let bits = at.filter(|a| a.is_finite()).map_or(f32::NAN, |a| a.clamp(0.0, 1.0)).to_bits();
        let old = self.shared.showcase_progress.swap(bits, Ordering::Relaxed);
        let was = self.shared.showcase_paused.swap(paused, Ordering::Relaxed);
        if was != paused || (f32::from_bits(old).is_nan() != f32::from_bits(bits).is_nan()) {
            self.send(UserEvent::Layers);
        }
    }

    /// A seek the operator asked for with the progress line, 0 to 1,
    /// taken once.
    pub fn showcase_seek(&self) -> Option<f32> {
        let bits = self.shared.showcase_seek.swap(f32::NAN.to_bits(), Ordering::Relaxed);
        Some(f32::from_bits(bits)).filter(|v| !v.is_nan())
    }

    /// Presses of the showcase's copy button so far: the producer copies
    /// what is showing.
    pub fn showcase_copies(&self) -> u32 {
        self.shared.showcase_copies.load(Ordering::Relaxed)
    }

    /// Clicks on the showcase's content so far (a video pauses on one).
    pub fn showcase_clicks(&self) -> u32 {
        self.shared.showcase_clicks.load(Ordering::Relaxed)
    }
}

/// What the pointer is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Spot {
    Outside,
    Close,
    Copy,
    Back,
    Forward,
    Grip,
    Bar,
    Content,
}

#[derive(Clone, Copy, Debug)]
enum Grab {
    /// Turning a model: where the drag began and how it was turned then.
    Orbit { from: [f32; 2], yaw: f32, pitch: f32 },
    /// Moving about a zoomed picture: where the drag began, and the
    /// view's centre then.
    Pan { from: [f32; 2], centre: [f32; 2] },
    Move { offset: [f32; 2] },
    Size { from: [f32; 2], rect: [f32; 4] },
    /// Pressed on the close button; closes if released over it.
    Close,
}

/// The panel's measures at a scale, in window pixels.
#[derive(Clone, Copy, Debug)]
struct Metrics {
    radius: f32,
    shadow: f32,
    bar: f32,
    pad: f32,
    /// The margin under the content, deep enough for the grip.
    foot: f32,
    icon: f32,
    grip: f32,
    min: [f32; 2],
    /// The scale, and the squire's pixel.
    s: f32,
    knight: f32,
}

impl Metrics {
    fn at(scale: f32) -> Self {
        let s = scale.max(0.5);
        Self {
            radius: (12.0 * s).round(),
            // No shadow: a pixel or two round the edge for its smoothing.
            shadow: (2.0 * s).round().max(2.0),
            bar: (36.0 * s).round(),
            pad: (10.0 * s).round(),
            foot: (18.0 * s).round(),
            icon: (24.0 * s).round(),
            grip: (22.0 * s).round(),
            min: [(280.0 * s).round(), (190.0 * s).round()],
            s,
            knight: squire::pixel(s),
        }
    }

    /// The frame picture's corner tile and its whole side.
    fn tile(&self) -> (u32, u32) {
        let corner = (self.shadow + self.radius) as u32;
        (corner, 2 * corner + MIDDLE)
    }
}

/// The frame picture's stretched middle, in pixels.
const MIDDLE: u32 = 4;
/// Opening and closing, in seconds.
const POP_SECONDS: f32 = 0.3;
const DUST_SECONDS: f32 = 0.5;
const CLOSE_SECONDS: f32 = 0.5;
const DOUBLE_CLICK_SECONDS: f32 = 0.35;

/// Ink, tonal: the frame's hairline, the title, the controls, and the
/// one warm accent the close button lights with.
const PANEL: [u8; 3] = [12, 14, 18];
/// How much of the terminal the glass dims, and how bright its edge is.
const GLASS: f32 = 0.9;
const EDGE: f32 = 0.2;
const RULE: [f32; 4] = [1.0, 1.0, 1.0, 0.06];
const INK: [u8; 3] = [168, 173, 182];
const ACCENT: [u8; 3] = [196, 134, 92];
const TIP_INK: [u8; 3] = [196, 200, 208];
const TIP_FILL: [f32; 4] = [0.055, 0.062, 0.078, 0.96];

pub(crate) struct Popup {
    open: Option<Open>,
    closing: Option<(Instant, Open)>,
    serial_seen: u64,
    metrics: Metrics,
    pager: Option<(u32, crate::page::Pager)>,
    /// The pointer, for the squire to face.
    pointer: [f32; 2],
    /// The window's size, for the wizard to keep inside.
    window: (f32, f32),
    /// A playing showing's place and pause, set by the window each draw.
    pub(crate) progress: Option<(f32, bool)>,
    font: String,
    /// Pictures made since the last draw (a mark's tip).
    pending: Vec<Picture>,
    /// The model's scene changed (or went) since the window last set it.
    scene_dirty: bool,
}

struct Open {
    size: (u32, u32),
    fit: Fit,
    rect: [f32; 4],
    /// The rect before a double-click filled the window.
    restore: Option<[f32; 4]>,
    since: Instant,
    grab: Option<Grab>,
    hover: Spot,
    scroll: f32,
    /// The title, kept to draw it again at a new scale.
    title: String,
    title_px: (u32, u32),
    last_bar_press: Option<Instant>,
    /// When new content arrived in the open panel: the squire waves.
    waved: Option<Instant>,
    herald: Herald,
    marks: Vec<Mark>,
    /// The mark under the pointer, and its tip's size.
    hot: Option<(usize, (u32, u32))>,
    /// How many of the retinue came along (their sheets are up).
    retinue: usize,
    /// Whether there are showings to step back and forward to.
    nav: (bool, bool),
    /// A 3D model, turned by the window.
    model: Option<Model>,
    /// How far a picture is zoomed (1 shows it whole), and the point of
    /// it, in its own pixels, at the middle of the view.
    zoom: f32,
    centre: [f32; 2],
    last_content_press: Option<Instant>,
    names: Vec<Vec<String>>,
    /// The squire under the pointer, and his tag's size.
    named: Option<(usize, (u32, u32))>,
    /// When the copy button was last pressed: it glows warm a moment.
    copied: Option<Instant>,
}

/// How long the copy button glows after a press.
const COPIED_SECONDS: f32 = 0.9;

/// How tall the strip along a playing showing's bottom is that seeks.
const SEEK_STRIP: f32 = 18.0;

/// The furthest a picture zooms in.
const MOST_ZOOM: f32 = 8.0;

/// Whether wheel and drag zoom and pan this showing: a contained
/// picture, not a model or a chart whose marks answer the pointer.
fn zoomable(open: &Open) -> bool {
    open.fit == Fit::Contain && open.model.is_none() && open.marks.is_empty()
}

/// The point of the picture under `p`, in its pixels.
fn view_point(open: &Open, rect: [f32; 4], s0: f32, p: [f32; 2]) -> [f32; 2] {
    let s = s0 * open.zoom;
    let mid = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
    [open.centre[0] + (p[0] - mid[0]) / s, open.centre[1] + (p[1] - mid[1]) / s]
}

/// Keep the view on the picture.
fn clamp_view(open: &mut Open, rect: [f32; 4], s0: f32) {
    let s = s0 * open.zoom;
    let (pw, ph) = (open.size.0 as f32, open.size.1 as f32);
    let (hw, hh) = (rect[2] / s / 2.0, rect[3] / s / 2.0);
    open.centre = [open.centre[0].clamp(hw.min(pw / 2.0), (pw - hw).max(pw / 2.0)), open.centre[1].clamp(hh.min(ph / 2.0), (ph - hh).max(ph / 2.0))];
}

/// The part of the picture a zoomed view shows, in its pixels.
fn view_part(open: &Open, rect: [f32; 4], s0: f32) -> [f32; 4] {
    let s = s0 * open.zoom;
    let (w, h) = (rect[2] / s, rect[3] / s);
    [open.centre[0] - w / 2.0, open.centre[1] - h / 2.0, w, h]
}

/// A model on the turntable: its nodes, how it is turned, and since when
/// it has been turning by itself (until the operator takes it in hand).
struct Model {
    nodes: Vec<crate::scene::Node>,
    yaw: f32,
    pitch: f32,
    spinning: Option<Instant>,
}

/// The turntable's pace, radians a second.
const SPIN: f32 = 0.45;

impl Herald {
    /// When the panel springs open, and when the entrance is over.
    fn impact(self) -> f32 {
        match self {
            Herald::Squire => squire::IMPACT,
            Herald::Wizard => wizard::IMPACT,
        }
    }

    /// How long it takes to leave: the wizard's spell lingers.
    fn close(self) -> f32 {
        match self {
            Herald::Squire => CLOSE_SECONDS,
            Herald::Wizard => 0.8,
        }
    }

    fn intro_end(self) -> f32 {
        match self {
            // The last of a retinue lands a little after the lead.
            Herald::Squire => (squire::INTRO_END + 0.12 * squire::MOST_RETINUE as f32).max(squire::IMPACT + DUST_SECONDS),
            Herald::Wizard => wizard::INTRO_END,
        }
    }
}

impl Default for Popup {
    fn default() -> Self {
        Self {
            open: None,
            closing: None,
            serial_seen: 0,
            metrics: Metrics::at(1.0),
            pager: None,
            pointer: [0.0, 0.0],
            window: (1600.0, 900.0),
            progress: None,
            font: String::new(),
            pending: Vec::new(),
            scene_dirty: false,
        }
    }
}

impl Popup {
    /// Take the cockpit's latest request. Returns pictures to upload
    /// before the next draw (the frame and its controls, the title).
    pub(crate) fn take_request(
        &mut self,
        host: &Host,
        window: (f32, f32),
        scale: f32,
        font: &str,
    ) -> Vec<Picture> {
        let Some(request) = host.shared.showcase.lock_recover().take() else {
            return Vec::new();
        };
        match request {
            Request::Close => {
                self.close();
                Vec::new()
            }
            Request::Open { title, size, fit, livery, herald, retinue, names, nav } => {
                self.window = window;
                self.font = font.to_string();
                let marks = std::mem::take(&mut *host.shared.showcase_marks.lock_recover());
                let model = host.shared.showcase_model.lock_recover().take().map(|nodes| Model {
                    nodes,
                    yaw: 0.6,
                    pitch: -0.25,
                    spinning: Some(Instant::now()),
                });
                self.scene_dirty = true;
                let retinue: Vec<Livery> = retinue.into_iter().take(squire::MOST_RETINUE).collect();
                self.serial_seen = host.shared.showcase_live.load(Ordering::Acquire);
                self.metrics = Metrics::at(scale);
                let mut pictures = vec![self.frame_picture(), self.icons_picture(), squire::sheet(livery.as_ref(), squire::SHEET), wizard::sheet()];
                for (i, livery) in retinue.iter().enumerate() {
                    pictures.push(squire::sheet(Some(livery), squire::RETINUE_SHEETS + i as u32));
                }
                let title_px = match self.title_picture(&title, font) {
                    Some(picture) => {
                        let px = (picture.width, picture.height);
                        pictures.push(picture);
                        px
                    }
                    None => (0, 0),
                };
                let rect = match &self.open {
                    // New content in an open panel keeps where the
                    // operator put it, refit around its middle.
                    Some(open) => {
                        let fresh = self.initial_rect(size, fit, window);
                        let [x, y, w, h] = open.rect;
                        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
                        self.clamped([cx - fresh[2] / 2.0, cy - fresh[3] / 2.0, fresh[2], fresh[3]], window)
                    }
                    None => self.initial_rect(size, fit, window),
                };
                let (since, waved) = match &self.open {
                    Some(open) => (open.since, Some(Instant::now())),
                    None => (Instant::now(), None),
                };
                self.closing = None;
                self.open = Some(Open {
                    size,
                    fit,
                    rect,
                    restore: None,
                    since,
                    grab: None,
                    hover: Spot::Outside,
                    scroll: 0.0,
                    title: title.clone(),
                    title_px,
                    last_bar_press: None,
                    waved,
                    herald,
                    marks,
                    hot: None,
                    retinue: retinue.len(),
                    nav,
                    model,
                    zoom: 1.0,
                    centre: [size.0 as f32 / 2.0, size.1 as f32 / 2.0],
                    last_content_press: None,
                    names,
                    named: None,
                    copied: None,
                });
                pictures
            }
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Whether frames must keep coming: opening or closing.
    pub(crate) fn animating(&self) -> bool {
        let opening = self
            .open
            .as_ref()
            .is_some_and(|open| open.since.elapsed().as_secs_f32() < open.herald.intro_end());
        let turning = self.open.as_ref().is_some_and(|o| o.model.as_ref().is_some_and(|m| m.spinning.is_some()));
        let glowing = self
            .open
            .as_ref()
            .is_some_and(|o| o.copied.is_some_and(|at| at.elapsed().as_secs_f32() < COPIED_SECONDS));
        opening || turning || glowing || self.closing.is_some()
    }

    /// The model's scene for the window to draw into its picture: `Some`
    /// when it changed (`Some(None)` takes it down), `None` when the one
    /// set still stands.
    pub(crate) fn take_scene(&mut self) -> Option<Option<(crate::scene::Scene, (u32, u32))>> {
        let open = self.open.as_mut();
        let turning = open.as_ref().is_some_and(|o| o.model.as_ref().is_some_and(|m| m.spinning.is_some()));
        if !self.scene_dirty && !turning {
            return None;
        }
        self.scene_dirty = false;
        let Some(open) = self.open.as_mut() else {
            return Some(None);
        };
        let size = open.size;
        let Some(model) = open.model.as_mut() else {
            return Some(None);
        };
        if let Some(since) = model.spinning {
            let now = Instant::now();
            model.yaw += SPIN * now.duration_since(since).as_secs_f32();
            model.spinning = Some(now);
        }
        use crate::scene::{Camera, Node, Scene, Transform, quat_axis_angle, quat_mul};
        let mut camera = Camera::default();
        camera.transform = Transform::from_translation([0.0, 0.0, 2.9]);
        // Clear: the model stands on the glass, not on a backdrop.
        camera.clear = Some([0.0; 4]);
        let mut scene = Scene::new(camera);
        let mut root = Node::new(MODEL_ROOT, crate::scene::MeshRef::Cube, crate::scene::Material::color([0.0; 4]));
        root.mesh = None;
        root.transform.rotation =
            quat_mul(quat_axis_angle([1.0, 0.0, 0.0], model.pitch), quat_axis_angle([0.0, 1.0, 0.0], model.yaw));
        scene.nodes.push(root);
        scene.nodes.extend(model.nodes.iter().cloned());
        Some(Some((scene, size)))
    }

    fn close(&mut self) {
        if let Some(open) = self.open.take() {
            self.closing = Some((Instant::now(), open));
        }
    }

    /// The operator closed it: tell the producer, then fade out.
    fn dismiss(&mut self, shared: &crate::Shared) {
        let _ = shared.showcase_live.compare_exchange(
            self.serial_seen,
            0,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
        self.close();
    }

    /// Escape closes it, while the pointer is over it. Elsewhere Escape
    /// stays the cockpit's: it interrupts a running turn, and the agent
    /// presents mid-turn, so an open panel must never take that key from
    /// the operator. True when the key was the showcase's.
    pub(crate) fn escape(&mut self, shared: &crate::Shared) -> bool {
        if self.open.is_none() || self.spot(self.pointer) == Spot::Outside {
            return false;
        }
        self.dismiss(shared);
        true
    }

    /// What is under `p`.
    pub(crate) fn spot(&self, p: [f32; 2]) -> Spot {
        let Some(open) = &self.open else {
            return Spot::Outside;
        };
        let m = self.metrics;
        let [x, y, w, h] = open.rect;
        if !inside(p, open.rect) {
            return Spot::Outside;
        }
        let close = close_centre(open.rect, m);
        if dist(p, close) <= m.icon / 2.0 + 4.0 {
            return Spot::Close;
        }
        if dist(p, copy_centre(open.rect, m)) <= m.icon / 2.0 + 3.0 {
            return Spot::Copy;
        }
        if open.nav.0 || open.nav.1 {
            let [back, forward] = nav_centres(open.rect, m);
            if open.nav.0 && dist(p, back) <= m.icon / 2.0 + 3.0 {
                return Spot::Back;
            }
            if open.nav.1 && dist(p, forward) <= m.icon / 2.0 + 3.0 {
                return Spot::Forward;
            }
        }
        if p[0] >= x + w - m.grip && p[1] >= y + h - m.grip {
            return Spot::Grip;
        }
        if p[1] < y + m.bar {
            return Spot::Bar;
        }
        Spot::Content
    }

    /// The pointer moved. True when the showcase took it (the cockpit
    /// does not see it) and whether the panel must redraw.
    pub(crate) fn pointer(&mut self, p: [f32; 2], window: (f32, f32)) -> (bool, bool) {
        let spot = self.spot(p);
        let turned = self.faces_left(p) != self.faces_left(self.pointer);
        self.pointer = p;
        let named = self.hover_squire(p);
        let turned = turned || named;
        let (taken, redraw) = self.pointer_on(p, spot, window);
        (taken, redraw || turned)
    }

    /// The seated squire under `p`: his place (0 the lead).
    fn squire_at(&self, p: [f32; 2]) -> Option<usize> {
        let open = self.open.as_ref()?;
        if open.herald != Herald::Squire || open.since.elapsed().as_secs_f32() < squire::INTRO_END || open.grab.is_some() {
            return None;
        }
        let m = self.metrics;
        let px = m.knight;
        let [w, h] = squire::size(px);
        (0..=open.retinue).find(|&place| {
            let seat = seat_x(open.rect, m) - place as f32 * (w + 3.0 * m.s);
            if place > 0 && seat < m.pad {
                return false;
            }
            inside(p, [open.rect[0] + seat, open.rect[1] - 16.0 * px, w, h])
        })
    }

    /// Name the squire under `p`: his tag drawn when he changes. True
    /// when what is shown changed.
    fn hover_squire(&mut self, p: [f32; 2]) -> bool {
        let at = self.squire_at(p);
        let held = self.open.as_ref().and_then(|o| o.named.map(|(i, _)| i));
        if at == held {
            return false;
        }
        let tag = at.and_then(|i| {
            let lines = self.open.as_ref()?.names.get(i)?.clone();
            self.tip_picture(&lines)
        });
        if let Some(open) = self.open.as_mut() {
            open.named = match (at, tag) {
                (Some(i), Some(picture)) => {
                    let size = (picture.width, picture.height);
                    self.pending.push(picture);
                    Some((i, size))
                }
                _ => None,
            };
        }
        true
    }

    /// Whether the squire, seated, faces left toward `p`.
    fn faces_left(&self, p: [f32; 2]) -> bool {
        let Some(open) = &self.open else { return false };
        let m = self.metrics;
        let centre = open.rect[0] + seat_x(open.rect, m) + squire::size(m.knight)[0] / 2.0;
        p[0] < centre
    }

    /// When the squire's next idle move is due (a leg swing, the next beat
    /// of a wave), so the window can sleep until then.
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        let open = self.open.as_ref()?;
        let t = open.since.elapsed().as_secs_f32();
        if open.herald == Herald::Wizard {
            if t < wizard::INTRO_END {
                return None;
            }
            let (_, wait) = wizard::idle(t - wizard::INTRO_END);
            return Some(Instant::now() + std::time::Duration::from_secs_f32(wait.max(0.01)));
        }
        if t < squire::INTRO_END {
            return None;
        }
        let (_, wait) = squire::idle(t - squire::INTRO_END, open.waved.map(|w| w.elapsed().as_secs_f32()));
        Some(Instant::now() + std::time::Duration::from_secs_f32(wait.max(0.02)))
    }

    /// Pictures to upload before the next draw.
    pub(crate) fn take_pictures(&mut self) -> Vec<Picture> {
        std::mem::take(&mut self.pending)
    }

    /// The mark under `p`, if the content has marks and its entrance is
    /// over: the nearest whose answering rect holds the pointer.
    fn mark_at(&self, p: [f32; 2]) -> Option<usize> {
        let open = self.open.as_ref()?;
        if open.marks.is_empty() || open.since.elapsed().as_secs_f32() < open.herald.intro_end() {
            return None;
        }
        let (rect, s) = content_rect(open, self.metrics)?;
        let c = [(p[0] - rect[0]) / s, (p[1] - rect[1]) / s];
        open.marks
            .iter()
            .enumerate()
            .filter(|(_, m)| inside(c, m.rect))
            .min_by(|(_, a), (_, b)| dist(c, a.anchor).total_cmp(&dist(c, b.anchor)))
            .map(|(i, _)| i)
    }

    /// Point at the mark under `p`: its tip drawn when it changes. True
    /// when what is shown changed.
    fn hover_mark(&mut self, p: [f32; 2]) -> bool {
        let hot = self.mark_at(p);
        let held = self.open.as_ref().and_then(|o| o.hot.map(|(i, _)| i));
        if hot == held {
            return false;
        }
        let tip = hot.and_then(|i| {
            let text = self.open.as_ref()?.marks.get(i)?.text.clone();
            self.tip_picture(&text)
        });
        if let Some(open) = self.open.as_mut() {
            open.hot = match (hot, tip) {
                (Some(i), Some(picture)) => {
                    let size = (picture.width, picture.height);
                    self.pending.push(picture);
                    Some((i, size))
                }
                _ => None,
            };
        }
        true
    }

    /// A mark's tip as pixels: its lines in the window's font.
    fn tip_picture(&mut self, text: &[String]) -> Option<Picture> {
        let lines: Vec<String> = text.iter().map(|l| l.chars().filter(|c| !c.is_control()).take(60).collect()).collect();
        if lines.iter().all(|l| l.trim().is_empty()) {
            return None;
        }
        let height = (self.metrics.bar * 0.5).round().max(8.0) as u32;
        if self.pager.as_ref().is_none_or(|(h, _)| *h != height) {
            let font = self.font.clone();
            self.pager = crate::page::Pager::load(&font, height).map(|pager| (height, pager));
        }
        let (_, pager) = self.pager.as_mut()?;
        let (width, height, rgba) = pager.render(&lines, crate::page::PLAIN, TIP_INK, [0, 0, 0, 0])?;
        Some(Picture { id: TIP_PICTURE, width, height, rgba, cols: 0, rows: 0 })
    }

    fn pointer_on(&mut self, p: [f32; 2], spot: Spot, window: (f32, f32)) -> (bool, bool) {
        let marked = self.open.as_ref().is_some_and(|o| o.grab.is_none()) && self.hover_mark(p);
        let (taken, redraw) = self.pointer_moved(p, spot, window);
        (taken, redraw || marked)
    }

    fn pointer_moved(&mut self, p: [f32; 2], spot: Spot, window: (f32, f32)) -> (bool, bool) {
        let m = self.metrics;
        let Some(open) = self.open.as_mut() else {
            return (false, false);
        };
        match open.grab {
            Some(Grab::Move { offset }) => {
                let [_, _, w, h] = open.rect;
                let rect = [p[0] - offset[0], p[1] - offset[1], w, h];
                open.rect = clamp_rect(rect, window, m);
                open.restore = None;
                (true, true)
            }
            Some(Grab::Size { from, rect }) => {
                let w = (rect[2] + p[0] - from[0]).clamp(m.min[0], (window.0 - rect[0]).max(m.min[0]));
                let h = (rect[3] + p[1] - from[1]).clamp(m.min[1], (window.1 - rect[1]).max(m.min[1]));
                open.rect = [rect[0], rect[1], w.round(), h.round()];
                open.restore = None;
                let max = max_scroll(open, m);
                open.scroll = open.scroll.min(max);
                (true, true)
            }
            Some(Grab::Close) => {
                let changed = open.hover != spot;
                open.hover = spot;
                (true, changed)
            }
            Some(Grab::Pan { from, centre }) => {
                if let Some((rect, s0)) = content_rect(open, m) {
                    let s = s0 * open.zoom;
                    open.centre = [centre[0] - (p[0] - from[0]) / s, centre[1] - (p[1] - from[1]) / s];
                    clamp_view(open, rect, s0);
                }
                (true, true)
            }
            Some(Grab::Orbit { from, yaw, pitch }) => {
                if let Some(model) = open.model.as_mut() {
                    let k = 0.008 / m.s.max(0.5);
                    model.yaw = yaw + (p[0] - from[0]) * k;
                    model.pitch = (pitch + (p[1] - from[1]) * k).clamp(-1.3, 1.3);
                }
                self.scene_dirty = true;
                (true, true)
            }
            None => {
                let changed = open.hover != spot;
                open.hover = spot;
                (spot != Spot::Outside, changed)
            }
        }
    }

    /// The left button went down at `p`. True when the showcase took it.
    pub(crate) fn press(&mut self, p: [f32; 2], window: (f32, f32), host: &Host) -> bool {
        let spot = self.spot(p);
        let m = self.metrics;
        let Some(open) = self.open.as_mut() else {
            return false;
        };
        // A press on the panel while the squire is still at it skips
        // to the end of his entrance.
        if spot != Spot::Outside && open.since.elapsed().as_secs_f32() < open.herald.intro_end() {
            if let Some(done) = Instant::now()
                .checked_sub(std::time::Duration::from_secs_f32(open.herald.intro_end()))
            {
                open.since = done;
            }
            return true;
        }
        match spot {
            Spot::Outside => return false,
            Spot::Close => open.grab = Some(Grab::Close),
            Spot::Copy => {
                host.shared.showcase_copies.fetch_add(1, Ordering::Relaxed);
                open.copied = Some(Instant::now());
            }
            Spot::Back | Spot::Forward => {
                let step = if spot == Spot::Back { -1 } else { 1 };
                host.shared.showcase_steps.fetch_add(step, Ordering::Relaxed);
            }
            Spot::Grip => {
                open.grab = Some(Grab::Size {
                    from: p,
                    rect: open.rect,
                })
            }
            Spot::Bar => {
                let double = open
                    .last_bar_press
                    .is_some_and(|at| at.elapsed().as_secs_f32() < DOUBLE_CLICK_SECONDS);
                if double {
                    open.last_bar_press = None;
                    open.rect = match open.restore.take() {
                        Some(rect) => rect,
                        None => {
                            open.restore = Some(open.rect);
                            let inset = 2.0 * m.pad;
                            [inset, inset, window.0 - 2.0 * inset, window.1 - 2.0 * inset]
                        }
                    };
                    let max = max_scroll(open, m);
                    open.scroll = open.scroll.min(max);
                } else {
                    open.last_bar_press = Some(Instant::now());
                    open.grab = Some(Grab::Move {
                        offset: [p[0] - open.rect[0], p[1] - open.rect[1]],
                    });
                }
            }
            Spot::Content
                if self.progress.is_some()
                    && content_rect(open, m).is_some_and(|(r, _)| p[1] >= r[1] + r[3] - SEEK_STRIP * m.s) =>
            {
                if let Some((r, _)) = content_rect(open, m) {
                    let at = ((p[0] - r[0]) / r[2]).clamp(0.0, 1.0);
                    host.shared.showcase_seek.store(at.to_bits(), Ordering::Relaxed);
                }
            }
            Spot::Content => {
                let double = open
                    .last_content_press
                    .is_some_and(|at| at.elapsed().as_secs_f32() < DOUBLE_CLICK_SECONDS);
                open.last_content_press = Some(Instant::now());
                if let Some(model) = open.model.as_mut() {
                    model.spinning = None;
                    open.grab = Some(Grab::Orbit { from: p, yaw: model.yaw, pitch: model.pitch });
                } else if double && open.zoom > 1.0 {
                    // Back to the whole picture.
                    open.zoom = 1.0;
                    open.last_content_press = None;
                } else if open.zoom > 1.0 {
                    open.grab = Some(Grab::Pan { from: p, centre: open.centre });
                } else {
                    host.shared.showcase_clicks.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        true
    }

    /// The left button came up. True when the showcase took it.
    pub(crate) fn release(&mut self, p: [f32; 2], host: &Host) -> bool {
        let spot = self.spot(p);
        let Some(open) = self.open.as_mut() else {
            return false;
        };
        match open.grab.take() {
            Some(Grab::Close) => {
                if spot == Spot::Close {
                    self.dismiss(&host.shared);
                }
                true
            }
            Some(_) => true,
            None => spot != Spot::Outside,
        }
    }

    /// The wheel turned by `px` pixels (up positive) at `p`. True when
    /// the showcase took it.
    pub(crate) fn wheel(&mut self, p: [f32; 2], px: f32) -> bool {
        if self.spot(p) == Spot::Outside {
            return false;
        }
        let m = self.metrics;
        let zoomable = self.open.as_ref().is_some_and(zoomable);
        let Some(open) = self.open.as_mut() else {
            return true;
        };
        if zoomable {
            // Toward the pointer: the point under it stays under it.
            if let Some((rect, s0)) = content_rect(open, m) {
                let under = view_point(open, rect, s0, p);
                let z = (open.zoom * (px * 0.002).exp()).clamp(1.0, MOST_ZOOM);
                open.zoom = z;
                let s = s0 * z;
                let mid = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
                open.centre = [under[0] - (p[0] - mid[0]) / s, under[1] - (p[1] - mid[1]) / s];
                clamp_view(open, rect, s0);
            }
        } else {
            let max = max_scroll(open, m);
            open.scroll = (open.scroll - px).clamp(0.0, max);
        }
        true
    }

    /// Whether the pointer is mid-drag on the panel.
    pub(crate) fn grabbing(&self) -> bool {
        self.open.as_ref().is_some_and(|open| open.grab.is_some())
    }

    /// The pointer's look over the panel.
    pub(crate) fn cursor(&self) -> winit::window::CursorIcon {
        use winit::window::CursorIcon;
        let Some(open) = &self.open else {
            return CursorIcon::Default;
        };
        match (open.grab, open.hover) {
            (Some(Grab::Move { .. } | Grab::Orbit { .. } | Grab::Pan { .. }), _) => CursorIcon::Grabbing,
            (None, Spot::Content) if open.zoom > 1.0 => CursorIcon::Grab,
            (None, Spot::Content) if open.model.is_some() => CursorIcon::Grab,
            (Some(Grab::Size { .. }), _) | (None, Spot::Grip) => CursorIcon::NwseResize,
            (_, Spot::Close | Spot::Copy | Spot::Back | Spot::Forward) => CursorIcon::Pointer,
            (None, Spot::Bar) => CursorIcon::Grab,
            _ => CursorIcon::Default,
        }
    }

    /// The window moved to a display of another scale: the frame, the
    /// controls and the title drawn again for it, the panel's place and
    /// size kept in proportion, and any tip let go (it is drawn again
    /// under the pointer). Returns the pictures to upload.
    pub(crate) fn rescale(&mut self, scale: f32) -> Vec<Picture> {
        let old = self.metrics.s;
        let fresh = Metrics::at(scale);
        if (fresh.s - old).abs() < 1e-3 {
            return Vec::new();
        }
        self.metrics = fresh;
        let k = fresh.s / old;
        let mut pictures = vec![self.frame_picture(), self.icons_picture()];
        let title = self.open.as_ref().map(|open| open.title.clone());
        let title = title.and_then(|title| {
            let font = self.font.clone();
            self.title_picture(&title, &font)
        });
        if let Some(open) = self.open.as_mut() {
            open.rect = open.rect.map(|v| (v * k).round());
            open.hot = None;
            open.named = None;
            open.title_px = title.as_ref().map_or((0, 0), |p| (p.width, p.height));
        }
        pictures.extend(title);
        pictures
    }

    /// The window was resized: keep the panel on it.
    pub(crate) fn fit_window(&mut self, window: (f32, f32)) {
        self.window = window;
        let m = self.metrics;
        if let Some(open) = self.open.as_mut() {
            let [x, y, w, h] = open.rect;
            let w = w.min(window.0).max(m.min[0]);
            let h = h.min(window.1).max(m.min[1]);
            open.rect = clamp_rect([x, y, w, h], window, m);
        }
    }

    /// The panel as layers, back to front. Empty when nothing is up;
    /// a finished close lets go of its state here.
    pub(crate) fn layers(&mut self) -> Vec<Layer> {
        let m = self.metrics;
        let (open, phase) = match (&self.open, &self.closing) {
            (Some(open), _) => (open, Phase::Open(open.since.elapsed().as_secs_f32())),
            (None, Some((at, open))) => {
                let t = at.elapsed().as_secs_f32();
                if t >= open.herald.close() {
                    self.closing = None;
                    self.scene_dirty = true;
                    return Vec::new();
                }
                (open, Phase::Close(t))
            }
            (None, None) => return Vec::new(),
        };
        let closing = matches!(phase, Phase::Close(_));
        let [x, y, w, h] = open.rect;
        let px = m.knight;
        let seat_x = seat_x(open.rect, m);
        // Opening, the panel waits folded and faint until the squire's
        // kick, then springs open from the corner he kicked, shaking
        // off the blow. Closing, he walks it off to the right.
        // The wizard's: it waits folded until his spark lands, unfolds
        // from its middle, and folds away into it again.
        let wizard = open.herald == Herald::Wizard;
        let impact = open.herald.impact();
        let (scale, k, content, dx) = match phase {
            Phase::Open(t) if t < impact => (0.9, 0.3 * (t / 0.15).min(1.0), 0.0, 0.0),
            Phase::Open(t) => {
                let u = ((t - impact) / POP_SECONDS).min(1.0);
                let shake = if wizard { 0.0 } else { 3.0 * m.s * (u * 45.0).sin() * (1.0 - u) * (1.0 - u) };
                let k = 0.3 + 0.7 * (u * 2.5).min(1.0);
                (0.9 + 0.1 * ease_out_back(u), k, k, shake)
            }
            Phase::Close(t) if wizard => {
                let u = ease_in(t / open.herald.close());
                (1.0 - 0.7 * u, 1.0 - u, 1.0 - u, 0.0)
            }
            Phase::Close(t) => {
                let u = t / CLOSE_SECONDS;
                let k = 1.0 - ease_in(u);
                (1.0, k, k, ease_in(u) * 360.0 * m.s)
            }
        };
        // How much of the content the wizard's sweep has brought in.
        let swept = match phase {
            Phase::Open(t) if wizard => ease_out(((t - impact) / wizard::SWEEP).clamp(0.0, 1.0)),
            _ => 1.0,
        };
        let pivot = if wizard { [x + w / 2.0, y + h / 2.0] } else { [x, y + h] };
        let place = |r: [f32; 4]| -> Placement {
            Placement::Pixels([
                pivot[0] + (r[0] - pivot[0]) * scale + dx,
                pivot[1] + (r[1] - pivot[1]) * scale,
                r[2] * scale,
                r[3] * scale,
            ])
        };
        let mut layers = Vec::with_capacity(24);
        let mut sweep: Option<(f32, f32, f32)> = None;
        let mut push = |rect: [f32; 4], kind: LayerKind, opacity: f32| {
            let mut layer = Layer::new(layers.len() as u64, 0, place(rect), kind);
            layer.opacity = opacity * if matches!(kind_class(&layer.kind), Class::Content) { content } else { k };
            layers.push(layer);
        };
        // The frame, nine slices of one picture.
        let (corner, side) = m.tile();
        let (c, mid) = (corner as f32, MIDDLE as f32);
        let outer = [x - m.shadow, y - m.shadow, w + 2.0 * m.shadow, h + 2.0 * m.shadow];
        let cols = [
            (outer[0], c, 0.0, c),
            (outer[0] + c, outer[2] - 2.0 * c, c + 1.0, mid - 2.0),
            (outer[0] + outer[2] - c, c, c + mid, c),
        ];
        let rows = [
            (outer[1], c, 0.0, c),
            (outer[1] + c, outer[3] - 2.0 * c, c + 1.0, mid - 2.0),
            (outer[1] + outer[3] - c, c, c + mid, c),
        ];
        debug_assert_eq!(side as f32, 2.0 * c + mid);
        for &(ry, rh, sy, sh) in &rows {
            for &(rx, rw, sx, sw) in &cols {
                if rw > 0.0 && rh > 0.0 {
                    push([rx, ry, rw, rh], LayerKind::PicturePart(FRAME_PICTURE, [sx, sy, sw, sh]), 1.0);
                }
            }
        }
        // The rule under the title bar; the content sits on the glass.
        push([x + m.pad, y + m.bar - 1.0, w - 2.0 * m.pad, 1.0], LayerKind::Solid(RULE), 1.0);
        let well = content_area(open.rect, m);
        // The content.
        let (pw, ph) = (open.size.0 as f32, open.size.1 as f32);
        if pw > 0.0 && ph > 0.0 && well[2] > 0.0 && well[3] > 0.0 {
            match open.fit {
                Fit::Contain => {
                    let Some((rect, _)) = content_rect(open, m) else { return layers };
                    let [_, _, dw, dh] = rect;
                    let picture = if open.model.is_some() { MODEL_PICTURE } else { SHOWCASE_PICTURE };
                    if swept < 1.0 {
                        if swept > 0.0 {
                            push(
                                [rect[0], rect[1], (dw * swept).round().max(1.0), dh],
                                LayerKind::PicturePart(picture, [0.0, 0.0, pw * swept, ph]),
                                1.0,
                            );
                            sweep = Some((rect[0] + dw * swept, rect[1], rect[1] + dh));
                        }
                    } else if open.zoom > 1.0 {
                        let s0 = rect[2] / pw;
                        push(rect, LayerKind::PicturePart(picture, view_part(open, rect, s0)), 1.0);
                    } else {
                        push(rect, LayerKind::Picture(picture), 1.0);
                    }
                    // A playing showing: its progress along the bottom,
                    // fuller under the pointer, and a pause mark.
                    if let Some((at, paused)) = self.progress {
                        let near = inside(self.pointer, [rect[0], rect[1] + dh - SEEK_STRIP * m.s, dw, SEEK_STRIP * m.s]);
                        let thick = if near { 4.0 } else { 2.0 } * m.s;
                        let y = rect[1] + dh - thick - 4.0 * m.s;
                        push([rect[0] + 6.0 * m.s, y, dw - 12.0 * m.s, thick], LayerKind::Solid([1.0, 1.0, 1.0, 0.16]), 1.0);
                        push(
                            [rect[0] + 6.0 * m.s, y, ((dw - 12.0 * m.s) * at).max(0.0), thick],
                            LayerKind::Solid([0.77, 0.53, 0.36, 0.95]),
                            1.0,
                        );
                        if paused {
                            let (cx, cy) = (rect[0] + dw / 2.0, rect[1] + dh / 2.0);
                            push([cx - 40.0 * m.s, cy - 40.0 * m.s, 80.0 * m.s, 80.0 * m.s], LayerKind::Glow([0.0, 0.0, 0.0, 0.7]), 1.0);
                            for dx in [-11.0, 4.0] {
                                push([cx + dx * m.s, cy - 13.0 * m.s, 7.0 * m.s, 26.0 * m.s], LayerKind::Solid([0.86, 0.87, 0.9, 0.9]), 1.0);
                            }
                        }
                    }
                }
                Fit::Width => {
                    let s = (well[2] / pw).min(1.0);
                    let dw = (pw * s).round();
                    let seen = (well[3] / s).min(ph);
                    let scroll = open.scroll.min((ph - seen).max(0.0));
                    let rect = [well[0] + ((well[2] - dw) / 2.0).round(), well[1], dw, (seen * s).round()];
                    push(
                        rect,
                        LayerKind::PicturePart(SHOWCASE_PICTURE, [0.0, scroll, pw, seen]),
                        1.0,
                    );
                    // A quiet thumb when there is more page than panel.
                    if ph > seen + 0.5 {
                        let track = well[3] - 2.0 * m.pad;
                        let thumb = (track * seen / ph).max(m.pad * 2.0);
                        let at = (track - thumb) * scroll / (ph - seen);
                        push(
                            [well[0] + well[2] - m.pad * 0.6, well[1] + m.pad + at, (m.pad * 0.3).max(2.0), thumb],
                            LayerKind::Solid([1.0, 1.0, 1.0, 0.22]),
                            1.0,
                        );
                    }
                }
            }
        }
        // The title, cut to the bar's room.
        if open.title_px.0 > 0 {
            let (tw, th) = (open.title_px.0 as f32, open.title_px.1 as f32);
            let room = if wizard {
                (w - 2.0 * m.pad - 2.0 * m.icon - 3.0 * m.pad).max(0.0)
            } else {
                let party = (0..=open.retinue)
                    .map(|place| seat_x - place as f32 * (squire::size(px)[0] + 3.0 * m.s))
                    .filter(|&seat| seat >= m.pad)
                    .fold(seat_x, f32::min);
                (party - m.pad - 8.0 * m.s).max(0.0)
            };
            let lead = nav_width(open, m);
            let shown = tw.min(room - lead);
            if shown > 0.0 {
                push(
                    [x + m.pad + 4.0 + lead, (y + (m.bar - th) / 2.0).round(), shown, th],
                    LayerKind::PicturePart(TITLE_PICTURE, [0.0, 0.0, shown, th]),
                    1.0,
                );
            }
        }
        // The arrows to earlier and later showings, lit under the pointer
        // and faint where there is nowhere to go.
        let i = m.icon;
        if open.nav.0 || open.nav.1 {
            let [back, forward] = nav_centres(open.rect, m);
            for (centre, slot, live, spot) in [(back, 3.0, open.nav.0, Spot::Back), (forward, 4.0, open.nav.1, Spot::Forward)] {
                let glow = match (live, open.hover == spot) {
                    (false, _) => 0.22,
                    (true, true) => 1.0,
                    (true, false) => 0.6,
                };
                push(
                    [(centre[0] - i / 2.0).round(), (centre[1] - i / 2.0).round(), i, i],
                    LayerKind::PicturePart(ICONS_PICTURE, [slot * i, 0.0, i, i]),
                    glow,
                );
            }
        }
        // The copy button: lit under the pointer, warm a moment after a press.
        {
            let [cx, cy] = copy_centre(open.rect, m);
            let warm = open.copied.map_or(0.0, |at| 1.0 - (at.elapsed().as_secs_f32() / COPIED_SECONDS).min(1.0));
            let glow = if !closing && open.hover == Spot::Copy { 1.0 } else { 0.6 };
            push(
                [(cx - i / 2.0).round(), (cy - i / 2.0).round(), i, i],
                LayerKind::PicturePart(ICONS_PICTURE, [5.0 * i, 0.0, i, i]),
                glow,
            );
            if warm > 0.0 {
                push(
                    [(cx - i / 2.0).round(), (cy - i / 2.0).round(), i, i],
                    LayerKind::PicturePart(ICONS_PICTURE, [6.0 * i, 0.0, i, i]),
                    warm,
                );
            }
        }
        // The close button, lit under the pointer.
        let [ccx, ccy] = close_centre(open.rect, m);
        let lit = !closing && open.hover == Spot::Close;
        let face = if lit { 1.0 } else { 0.0 };
        push(
            [(ccx - i / 2.0).round(), (ccy - i / 2.0).round(), i, i],
            LayerKind::PicturePart(ICONS_PICTURE, [face * i, 0.0, i, i]),
            1.0,
        );
        // The grip, brighter under the pointer or in hand.
        let held = matches!(open.grab, Some(Grab::Size { .. }));
        let grip_glow = if held || open.hover == Spot::Grip { 1.0 } else { 0.55 };
        push(
            [x + w - i - 1.0, y + h - i - 1.0, i, i],
            LayerKind::PicturePart(ICONS_PICTURE, [2.0 * i, 0.0, i, i]),
            grip_glow,
        );
        // The hot mark: a quiet guide down to the axis, and its tip.
        let pointing = match (phase, open.hot, content_rect(open, m)) {
            (Phase::Open(_), Some((i, tip)), Some((rect, s))) => open.marks.get(i).map(|mark| {
                let at = [rect[0] + mark.anchor[0] * s, rect[1] + mark.anchor[1] * s];
                layers.push(Layer::new(
                    layers.len() as u64,
                    0,
                    Placement::Pixels([at[0].round() - 0.5, at[1], 1.0, (rect[1] + rect[3] - at[1]).max(0.0)]),
                    LayerKind::Solid([1.0, 1.0, 1.0, 0.10]),
                ));
                let (tw, th) = (tip.0 as f32, tip.1 as f32);
                let pad = 8.0 * m.s;
                let (bw, bh) = (tw + 2.0 * pad, th + 2.0 * pad);
                let bx = (at[0] - bw / 2.0).clamp(rect[0], (rect[0] + rect[2] - bw).max(rect[0]));
                let above = at[1] - bh - 12.0 * m.s;
                let by = if above >= rect[1] { above } else { at[1] + 12.0 * m.s };
                layers.push(Layer::new(layers.len() as u64, 0, Placement::Pixels([bx, by, bw, bh]), LayerKind::Solid(TIP_FILL)));
                layers.push(Layer::new(layers.len() as u64, 0, Placement::Pixels([bx, by, bw, 1.0]), LayerKind::Solid(RULE)));
                layers.push(Layer::new(
                    layers.len() as u64,
                    0,
                    Placement::Pixels([(bx + pad).round(), (by + pad).round(), tw, th]),
                    LayerKind::Picture(TIP_PICTURE),
                ));
                at
            }),
            _ => None,
        };
        if wizard {
            let wpx = wizard::pixel(m.s);
            let id = layers.len() as u64;
            match phase {
                Phase::Open(t) => {
                    // Ready at the close button; now and then, idle magic.
                    let casting = if t < wizard::INTRO_END {
                        None
                    } else if open.hover == Spot::Close {
                        Some(f32::MAX)
                    } else {
                        wizard::idle(t - wizard::INTRO_END).0
                    };
                    layers.extend(wizard::entrance(t, open.rect, self.window, wpx, m.s, id, pointing, casting))
                }
                Phase::Close(t) => {
                    layers.extend(wizard::exit(t, open.herald.close(), open.rect, self.window, wpx, m.s, id))
                }
            }
            if let Some((edge, top, bottom)) = sweep {
                layers.push(wizard::sweep_motes(layers.len() as u64, edge, top, bottom, m.s));
            }
            return layers;
        }
        // The squire and his retinue (a formation's other houses, each in
        // its colours), and the dust and spark of his kick.
        let shifted = [x + dx, y, w, h];
        let faces_left = self.faces_left(self.pointer);
        for place in 0..=open.retinue {
            let seat = seat_x - place as f32 * (squire::size(px)[0] + 3.0 * m.s);
            if place > 0 && seat < m.pad {
                break;
            }
            let sheet = if place == 0 { squire::SHEET } else { squire::RETINUE_SHEETS + place as u32 - 1 };
            let (pose, his) = match phase {
                Phase::Open(t) => {
                    let mut pose = squire::follow(t, open.rect, seat, px, m.s, place);
                    if pose.frame == squire::Frame::Sit {
                        if open.hover == Spot::Close {
                            // Up on their feet, ready to walk it off.
                            pose.frame = squire::Frame::Stand;
                            pose.at = squire::roof_stand(open.rect, seat, px);
                        } else {
                            let waved = open.waved.map(|w| w.elapsed().as_secs_f32());
                            // Each swings his legs in his own time; named
                            // under the pointer, he raises a hand.
                            let own = t - squire::INTRO_END + 1.7 * place as f32;
                            pose.frame = if open.named.is_some_and(|(i, _)| i == place) {
                                squire::Frame::SitWave
                            } else {
                                squire::idle(own, waved).0
                            };
                            pose.left = faces_left;
                        }
                    }
                    (pose, ((t - 0.12 * place as f32) / 0.1).clamp(0.0, 1.0))
                }
                Phase::Close(t) => {
                    let u = t / CLOSE_SECONDS;
                    (squire::exit(t, shifted, seat, px), 1.0 - ((u - 0.6) / 0.4).clamp(0.0, 1.0))
                }
            };
            layers.push(squire::layer(layers.len() as u64, sheet, pose, px, his));
            // His tag, over his head.
            if let (Phase::Open(_), Some((i, tag))) = (phase, open.named)
                && i == place
            {
                let (tw, th) = (tag.0 as f32, tag.1 as f32);
                let pad = 7.0 * m.s;
                let (bw, bh) = (tw + 2.0 * pad, th + 2.0 * pad);
                let head = [pose.at[0] + squire::size(px)[0] / 2.0, pose.at[1]];
                let bx = (head[0] - bw / 2.0).clamp(0.0, (self.window.0 - bw).max(0.0));
                let by = (head[1] - bh - 6.0 * m.s).max(0.0);
                for layer in [
                    Layer::new(layers.len() as u64, 0, Placement::Pixels([bx, by, bw, bh]), LayerKind::Solid(TIP_FILL)),
                    Layer::new(layers.len() as u64 + 1, 0, Placement::Pixels([bx, by + bh - 1.0, bw, 1.0]), LayerKind::Solid(RULE)),
                    Layer::new(
                        layers.len() as u64 + 2,
                        0,
                        Placement::Pixels([(bx + pad).round(), (by + pad).round(), tw, th]),
                        LayerKind::Picture(TIP_PICTURE),
                    ),
                ] {
                    layers.push(layer);
                }
            }
        }
        if let Phase::Open(t) = phase {
            let age = t - squire::IMPACT;
            if (0.0..DUST_SECONDS).contains(&age) {
                let corner = [x - 8.0 * m.s, y + h - 8.0 * m.s, 16.0 * m.s, 16.0 * m.s];
                let mut dust = Layer::new(
                    layers.len() as u64,
                    0,
                    Placement::Pixels(corner),
                    LayerKind::Particles(crate::layer::Particles {
                        color: [0.80, 0.76, 0.70, 0.9],
                        count: 18,
                        size: (px * 0.7).round().max(2.0),
                        life: DUST_SECONDS,
                        rise: 260.0 * m.s,
                        sway: 900.0 * m.s,
                        seed: 7,
                        burst: Some(age),
                    }),
                );
                dust.opacity = 1.0;
                layers.push(dust);
                let flash = 1.0 - (age / 0.2).min(1.0);
                if flash > 0.0 {
                    let r = 70.0 * m.s;
                    let mut spark = Layer::new(
                        layers.len() as u64,
                        0,
                        Placement::Pixels([x - r, y + h - r, 2.0 * r, 2.0 * r]),
                        LayerKind::Glow([0.77, 0.52, 0.36, 0.55]),
                    );
                    spark.additive = true;
                    spark.opacity = flash;
                    layers.push(spark);
                }
            }
        }
        layers
    }

    /// The first rect for content of `size`: at its own pixels when that
    /// fits most of the window, smaller to fit otherwise, centred.
    fn initial_rect(&self, size: (u32, u32), fit: Fit, window: (f32, f32)) -> [f32; 4] {
        let m = self.metrics;
        let chrome = [2.0 * m.pad, m.bar + m.foot];
        let room = [window.0 * 0.76 - chrome[0], window.1 * 0.8 - chrome[1]];
        let (pw, ph) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let (cw, ch) = match fit {
            Fit::Contain => {
                let s = (room[0] / pw).min(room[1] / ph).min(1.0);
                (pw * s, ph * s)
            }
            Fit::Width => {
                let s = (room[0] / pw).min(1.0);
                (pw * s, (ph * s).min(room[1]))
            }
        };
        let w = (cw + chrome[0]).round().max(m.min[0]).min(window.0);
        let h = (ch + chrome[1]).round().max(m.min[1]).min(window.1);
        [((window.0 - w) / 2.0).round(), ((window.1 - h) / 2.0).round(), w, h]
    }

    fn clamped(&self, rect: [f32; 4], window: (f32, f32)) -> [f32; 4] {
        clamp_rect(rect, window, self.metrics)
    }

    /// The frame: shadow, rounded panel and hairline edge, straight RGBA.
    fn frame_picture(&self) -> Picture {
        let m = self.metrics;
        let (_, side) = m.tile();
        let n = side as usize;
        let mut rgba = vec![0u8; n * n * 4];
        let half = (side as f32 - 2.0 * m.shadow) / 2.0;
        let centre = side as f32 / 2.0;
        for py in 0..n {
            for px in 0..n {
                let p = [px as f32 + 0.5 - centre, py as f32 + 0.5 - centre];
                let d = rounded_box(p, half, m.radius);
                let cover = (0.5 - d).clamp(0.0, 1.0);
                // Glass: a faint dim the terminal shows through, and a
                // hairline just inside the edge. No shadow.
                let edge = (1.0 - (d + 1.0).abs()).clamp(0.0, 1.0) * cover;
                let (fill, line) = (cover * GLASS, edge * EDGE);
                let a = line + fill * (1.0 - line);
                let o = (py * n + px) * 4;
                if a > 0.0 {
                    // The hairline over the dim, un-premultiplied.
                    for ch in 0..3 {
                        let c = (255.0 * line + f32::from(PANEL[ch]) * fill * (1.0 - line)) / a;
                        rgba[o + ch] = c.round().clamp(0.0, 255.0) as u8;
                    }
                    rgba[o + 3] = (a * 255.0).round() as u8;
                }
            }
        }
        Picture {
            id: FRAME_PICTURE,
            width: side,
            height: side,
            rgba,
            cols: 0,
            rows: 0,
        }
    }

    /// Three icons in a row: the close mark, the close mark lit, and
    /// the corner grip.
    fn icons_picture(&self) -> Picture {
        let m = self.metrics;
        let i = m.icon as usize;
        let (w, h) = (7 * i, i);
        let mut rgba = vec![0u8; w * h * 4];
        let s = m.icon / 24.0;
        let c = m.icon / 2.0;
        let arm = 4.6 * s;
        let stroke = 0.85 * s.max(1.0);
        for py in 0..h {
            for px in 0..w {
                let slot = px / i;
                let p = [(px % i) as f32 + 0.5, py as f32 + 0.5];
                let (rgb, a) = match slot {
                    0 | 1 => {
                        let q = [p[0] - c, p[1] - c];
                        let cross = segment(q, [-arm, -arm], [arm, arm])
                            .min(segment(q, [-arm, arm], [arm, -arm]));
                        let mark = (stroke + 0.5 - cross).clamp(0.0, 1.0);
                        if slot == 0 {
                            (INK, mark * 0.85)
                        } else {
                            // Lit: a warm disc, the mark cut dark into it.
                            let disc = (c - 1.0 + 0.5 - dist(p, [c, c])).clamp(0.0, 1.0);
                            let rgb = [
                                lerp(f32::from(ACCENT[0]), 20.0, mark) as u8,
                                lerp(f32::from(ACCENT[1]), 20.0, mark) as u8,
                                lerp(f32::from(ACCENT[2]), 22.0, mark) as u8,
                            ];
                            (rgb, disc * 0.92)
                        }
                    }
                    5 | 6 => {
                        // Two leaves, one behind the other: copy.
                        let leaf = |p: [f32; 2], at: [f32; 2]| {
                            let half = 4.2 * s;
                            let q = [(p[0] - at[0]).abs() - half, (p[1] - at[1]).abs() - half];
                            let out = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt() + q[0].max(q[1]).min(0.0);
                            out.abs()
                        };
                        let back = leaf(p, [c - 1.6 * s, c - 1.6 * s]);
                        let front = leaf(p, [c + 1.6 * s, c + 1.6 * s]);
                        // The front leaf hides the back one where they cross.
                        let inside_front = (p[0] - (c + 1.6 * s)).abs() < 4.2 * s && (p[1] - (c + 1.6 * s)).abs() < 4.2 * s;
                        let d = if inside_front { front } else { back.min(front) };
                        let a = (stroke + 0.5 - d).clamp(0.0, 1.0);
                        (if slot == 5 { INK } else { ACCENT }, a * if slot == 5 { 0.85 } else { 1.0 })
                    }
                    3 | 4 => {
                        // A chevron, pointing back or forward.
                        let dir = if slot == 3 { -1.0 } else { 1.0 };
                        let tip = [c + dir * 2.4 * s, c];
                        let back = c - dir * 2.4 * s;
                        let d = segment(p, [back, c - 5.0 * s], tip).min(segment(p, [back, c + 5.0 * s], tip));
                        (INK, (stroke * 1.1 + 0.5 - d).clamp(0.0, 1.0))
                    }
                    _ => {
                        // Three short diagonals in the bottom-right corner.
                        let mut best = f32::MAX;
                        for k in [6.0f32, 10.0, 14.0] {
                            let (k, inset) = (k * s, 3.5 * s);
                            let a = [m.icon - inset, m.icon - k];
                            let b = [m.icon - k, m.icon - inset];
                            best = best.min(segment(p, a, b));
                        }
                        (INK, (stroke + 0.5 - best).clamp(0.0, 1.0) * 0.7)
                    }
                };
                let o = (py * w + px) * 4;
                rgba[o] = rgb[0];
                rgba[o + 1] = rgb[1];
                rgba[o + 2] = rgb[2];
                rgba[o + 3] = (a * 255.0).round() as u8;
            }
        }
        Picture {
            id: ICONS_PICTURE,
            width: w as u32,
            height: h as u32,
            rgba,
            cols: 0,
            rows: 0,
        }
    }

    /// The title as pixels, in the window's font, sized to the bar.
    fn title_picture(&mut self, title: &str, font: &str) -> Option<Picture> {
        let title: String = title.chars().filter(|c| !c.is_control()).take(160).collect();
        if title.trim().is_empty() {
            return None;
        }
        let height = (self.metrics.bar * 0.5).round().max(8.0) as u32;
        if self.pager.as_ref().is_none_or(|(h, _)| *h != height) {
            self.pager = crate::page::Pager::load(font, height).map(|pager| (height, pager));
        }
        let (_, pager) = self.pager.as_mut()?;
        let (width, height, rgba) = pager.render(&[title.as_str()], crate::page::PLAIN, INK, [0, 0, 0, 0])?;
        Some(Picture {
            id: TITLE_PICTURE,
            width,
            height,
            rgba,
            cols: 0,
            rows: 0,
        })
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Open(f32),
    Close(f32),
}

/// What a layer is, for its fade: the content waits for the kick.
enum Class {
    Content,
    Chrome,
}

fn kind_class(kind: &LayerKind) -> Class {
    match kind {
        LayerKind::Picture(SHOWCASE_PICTURE | MODEL_PICTURE)
        | LayerKind::PicturePart(SHOWCASE_PICTURE | MODEL_PICTURE, _) => Class::Content,
        _ => Class::Chrome,
    }
}

/// Where the squire sits on the roof, from its left: just short of the
/// close button, so the title has the rest.
fn seat_x(rect: [f32; 4], m: Metrics) -> f32 {
    let him = squire::size(m.knight)[0];
    (rect[2] - m.pad - 2.0 * m.icon - 16.0 * m.s - him).max(m.pad)
}

/// Where contained content sits on screen, and its scale.
fn content_rect(open: &Open, m: Metrics) -> Option<([f32; 4], f32)> {
    let well = content_area(open.rect, m);
    let (pw, ph) = (open.size.0 as f32, open.size.1 as f32);
    if open.fit != Fit::Contain || pw <= 0.0 || ph <= 0.0 || well[2] <= 0.0 || well[3] <= 0.0 {
        return None;
    }
    let s = (well[2] / pw).min(well[3] / ph);
    let (dw, dh) = ((pw * s).round(), (ph * s).round());
    Some((
        [
            (well[0] + (well[2] - dw) / 2.0).round(),
            (well[1] + (well[3] - dh) / 2.0).round(),
            dw,
            dh,
        ],
        s,
    ))
}

fn content_area(rect: [f32; 4], m: Metrics) -> [f32; 4] {
    let [x, y, w, h] = rect;
    [x + m.pad, y + m.bar, (w - 2.0 * m.pad).max(0.0), (h - m.bar - m.foot).max(0.0)]
}

/// The back and forward arrows, at the bar's left.
fn nav_centres(rect: [f32; 4], m: Metrics) -> [[f32; 2]; 2] {
    let [x, y, _, _] = rect;
    let cy = y + m.bar / 2.0;
    [[x + m.pad + m.icon / 2.0, cy], [x + m.pad + m.icon * 1.5 + 2.0 * m.s, cy]]
}

/// The room the arrows take before the title.
fn nav_width(open: &Open, m: Metrics) -> f32 {
    if open.nav.0 || open.nav.1 { 2.0 * m.icon + 2.0 * m.s + m.pad } else { 0.0 }
}

/// The copy button, left of the close button.
fn copy_centre(rect: [f32; 4], m: Metrics) -> [f32; 2] {
    let [x, y] = close_centre(rect, m);
    [x - m.icon - 6.0 * m.s, y]
}

fn close_centre(rect: [f32; 4], m: Metrics) -> [f32; 2] {
    let [x, y, w, _] = rect;
    [x + w - m.pad - m.icon / 2.0, y + m.bar / 2.0]
}

/// How far a page can scroll in the panel, in its own pixels.
fn max_scroll(open: &Open, m: Metrics) -> f32 {
    if open.fit != Fit::Width {
        return 0.0;
    }
    let well = content_area(open.rect, m);
    let (pw, ph) = (open.size.0.max(1) as f32, open.size.1 as f32);
    let s = (well[2] / pw).min(1.0).max(1e-3);
    (ph - well[3] / s).max(0.0)
}

/// Keep the title bar where the hand can reach it.
fn clamp_rect(rect: [f32; 4], window: (f32, f32), m: Metrics) -> [f32; 4] {
    let [x, y, w, h] = rect;
    let keep = (m.min[0] * 0.4).min(w);
    let x = x.clamp(keep - w, (window.0 - keep).max(keep - w));
    let y = y.clamp(0.0, (window.1 - m.bar).max(0.0));
    [x.round(), y.round(), w, h]
}

fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Distance from `p` to the segment `a`–`b`.
fn segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let len = abx * abx + aby * aby;
    let t = if len > 0.0 {
        (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist(p, [a[0] + abx * t, a[1] + aby * t])
}

/// Signed distance to a square of half-side `half` with corners of
/// radius `r`, centred on the origin: negative inside.
fn rounded_box(p: [f32; 2], half: f32, r: f32) -> f32 {
    let q = [p[0].abs() - half + r, p[1].abs() - half + r];
    let outside = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt();
    outside + q[0].max(q[1]).min(0.0) - r
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn ease_in(t: f32) -> f32 {
    t.clamp(0.0, 1.0).powi(3)
}

/// Out past the mark and back: a spring's first swing.
fn ease_out_back(t: f32) -> f32 {
    let (c1, t) = (1.9, t.clamp(0.0, 1.0) - 1.0);
    1.0 + (c1 + 1.0) * t.powi(3) + c1 * t.powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_popup(size: (u32, u32), fit: Fit) -> Popup {
        let mut popup = Popup::default();
        let rect = popup.initial_rect(size, fit, (1600.0, 1000.0));
        popup.open = Some(Open {
            size,
            fit,
            rect,
            restore: None,
            since: Instant::now(),
            grab: None,
            hover: Spot::Outside,
            scroll: 0.0,
            title: String::new(),
            title_px: (0, 0),
            last_bar_press: None,
            waved: None,
            herald: Herald::Squire,
            marks: Vec::new(),
            hot: None,
            retinue: 0,
            nav: (false, false),
            model: None,
            zoom: 1.0,
            centre: [size.0 as f32 / 2.0, size.1 as f32 / 2.0],
            last_content_press: None,
            names: Vec::new(),
            named: None,
            copied: None,
        });
        popup
    }

    #[test]
    fn a_small_picture_opens_at_its_own_pixels_and_a_big_one_fits() {
        let popup = Popup::default();
        let m = popup.metrics;
        let small = popup.initial_rect((400, 300), Fit::Contain, (1600.0, 1000.0));
        assert_eq!(small[2], 400.0 + 2.0 * m.pad);
        assert_eq!(small[3], 300.0 + m.bar + m.foot);
        let big = popup.initial_rect((4000, 3000), Fit::Contain, (1600.0, 1000.0));
        assert!(big[2] <= 1600.0 * 0.76 + 1.0 && big[3] <= 1000.0 * 0.8 + 1.0);
        assert!(big[0] > 0.0 && big[1] > 0.0, "centred, inside the window");
    }

    #[test]
    fn the_spots_are_where_they_look() {
        let popup = open_popup((800, 600), Fit::Contain);
        let m = popup.metrics;
        let [x, y, w, h] = popup.open.as_ref().unwrap().rect;
        assert_eq!(popup.spot([x - 5.0, y]), Spot::Outside);
        assert_eq!(popup.spot(close_centre([x, y, w, h], m)), Spot::Close);
        assert_eq!(popup.spot([x + 20.0, y + 5.0]), Spot::Bar);
        assert_eq!(popup.spot([x + w - 4.0, y + h - 4.0]), Spot::Grip);
        assert_eq!(popup.spot([x + w / 2.0, y + h / 2.0]), Spot::Content);
    }

    #[test]
    fn dragging_the_bar_moves_and_the_grip_sizes_with_a_floor() {
        // Press and release need a host (for clicks and closing); drive
        // the grab directly.
        let host_free = |popup: &mut Popup, from: [f32; 2], to: [f32; 2]| {
            let open = popup.open.as_mut().unwrap();
            let rect = open.rect;
            open.grab = Some(if from[1] < rect[1] + Metrics::at(1.0).bar {
                Grab::Move { offset: [from[0] - rect[0], from[1] - rect[1]] }
            } else {
                Grab::Size { from, rect }
            });
            popup.pointer(to, (1600.0, 1000.0));
            popup.open.as_mut().unwrap().grab = None;
        };
        let mut popup = open_popup((800, 600), Fit::Contain);
        let [x, y, w, h] = popup.open.as_ref().unwrap().rect;
        host_free(&mut popup, [x + 40.0, y + 10.0], [x + 140.0, y + 60.0]);
        let moved = popup.open.as_ref().unwrap().rect;
        assert_eq!((moved[0], moved[1], moved[2], moved[3]), (x + 100.0, y + 50.0, w, h));
        host_free(&mut popup, [moved[0] + w - 2.0, moved[1] + h - 2.0], [moved[0], moved[1] + 40.0]);
        let sized = popup.open.as_ref().unwrap().rect;
        let m = popup.metrics;
        assert_eq!((sized[2], sized[3]), (m.min[0], m.min[1]), "never smaller than the floor");
    }

    #[test]
    fn a_page_scrolls_only_as_far_as_it_goes() {
        let mut popup = open_popup((800, 5000), Fit::Width);
        let [x, y, w, h] = popup.open.as_ref().unwrap().rect;
        let mid = [x + w / 2.0, y + h / 2.0];
        assert!(popup.wheel(mid, -100_000.0));
        let m = popup.metrics;
        let max = max_scroll(popup.open.as_ref().unwrap(), m);
        assert!(max > 0.0);
        assert_eq!(popup.open.as_ref().unwrap().scroll, max);
        assert!(popup.wheel(mid, 100_000.0));
        assert_eq!(popup.open.as_ref().unwrap().scroll, 0.0);
        assert!(!popup.wheel([x - 50.0, y], 10.0), "outside it the terminal scrolls");
    }

    #[test]
    fn the_frame_is_clear_outside_and_glass_inside() {
        let popup = Popup::default();
        let frame = popup.frame_picture();
        let side = frame.width as usize;
        let at = |x: usize, y: usize| frame.rgba[(y * side + x) * 4 + 3];
        assert_eq!(at(0, 0), 0, "the far corner is empty");
        let middle = at(side / 2, side / 2);
        assert!(middle > 150 && middle < 240, "the middle is glass: dim, not solid");
        assert_eq!(frame.rgba.len(), side * side * 4);
        let icons = popup.icons_picture();
        assert_eq!(icons.rgba.len() as u32, icons.width * icons.height * 4);
    }

    #[test]
    fn the_wheel_zooms_toward_the_pointer_and_stays_on_the_picture() {
        let mut popup = open_popup((800, 600), Fit::Contain);
        let m = popup.metrics;
        let (rect, s0) = content_rect(popup.open.as_ref().unwrap(), m).unwrap();
        let corner = [rect[0] + 10.0, rect[1] + 10.0];
        let before = view_point(popup.open.as_ref().unwrap(), rect, s0, corner);
        assert!(popup.wheel(corner, 400.0));
        let open = popup.open.as_ref().unwrap();
        assert!(open.zoom > 2.0);
        let after = view_point(open, rect, s0, corner);
        assert!((before[0] - after[0]).abs() < 1.0 && (before[1] - after[1]).abs() < 1.0, "the point under the pointer stays");
        let part = view_part(open, rect, s0);
        assert!(part[0] >= -0.5 && part[1] >= -0.5, "the view stays on the picture");
        popup.wheel(corner, -100_000.0);
        assert_eq!(popup.open.as_ref().unwrap().zoom, 1.0, "never smaller than whole");
    }

    #[test]
    fn a_new_scale_redraws_the_chrome_and_keeps_the_panel_in_proportion() {
        let mut popup = open_popup((640, 480), Fit::Contain);
        let before = popup.open.as_ref().unwrap().rect;
        let small = popup.frame_picture().width;
        let pictures = popup.rescale(2.0);
        assert!(pictures.len() >= 2, "frame and controls drawn again");
        assert!(popup.frame_picture().width > small);
        let after = popup.open.as_ref().unwrap().rect;
        assert!((after[2] - before[2] * 2.0).abs() <= 1.0 && (after[0] - before[0] * 2.0).abs() <= 1.0);
        assert!(popup.rescale(2.0).is_empty(), "the same scale again changes nothing");
    }

    #[test]
    fn escape_is_the_panels_only_under_the_pointer() {
        let mut popup = open_popup((640, 480), Fit::Contain);
        let shared = crate::Shared::default();
        let [x, y, w, h] = popup.open.as_ref().unwrap().rect;
        popup.pointer = [x - 40.0, y];
        assert!(!popup.escape(&shared), "away from the panel, Escape is the cockpit's");
        assert!(popup.is_open());
        popup.pointer = [x + w / 2.0, y + h / 2.0];
        assert!(popup.escape(&shared));
        assert!(!popup.is_open());
    }

    #[test]
    fn layers_fade_in_and_none_are_left_after_closing() {
        let mut popup = open_popup((640, 480), Fit::Contain);
        let layers = popup.layers();
        assert!(layers.iter().any(|l| l.kind == LayerKind::Picture(SHOWCASE_PICTURE)));
        assert!(layers.iter().all(|l| l.z >= 0));
        popup.close();
        popup.closing.as_mut().unwrap().0 -= std::time::Duration::from_secs(1);
        assert!(popup.layers().is_empty());
        assert!(!popup.animating());
    }
}
