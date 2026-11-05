//! The Heartglass in the window: its thread, what it hands the window, and
//! the layers that glow, beat and move over the printed plate.
//!
//! The thread keeps what is on the GPU (the stages' meshes, the plate's
//! picture, the clock's small pictures, the stage titles) and sends each
//! again only when what it shows changed. Every frame it sends the scene
//! (the heart turning) and the layer set, as the Orrery does.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use angel_glass::{Host, LAYER_PICTURES, Layer, LayerKind, Particles, Placement};

mod float;
#[cfg(test)]
pub(crate) use float::halo_for_test;

use super::campaign::{Campaign, Cloud, Watcher};
use super::heart::{self, Built, Flare, Pose};
use super::plate::{self, CloudView, Ink, Typeset, View};
use super::{Grid, Layout};
use crate::ui::orrery::segment_rect;

/// The Heartglass's layer group ("HART").
const GROUP: u32 = 0x4841_5254;
/// Its pictures: the scene, the plate, the clock's three, the scanlines, the
/// flare's banner, and one title a stage from `TITLES`.
const PICTURES: u32 = LAYER_PICTURES + 0x4845_0000;
pub(crate) const SCENE: u32 = PICTURES;
const PLATE: u32 = PICTURES + 1;
const CLOCK: u32 = PICTURES + 2;
const RESET: u32 = PICTURES + 3;
const AGO: u32 = PICTURES + 4;
const SCANLINES: u32 = PICTURES + 5;
const BANNER: u32 = PICTURES + 6;
const TITLES: u32 = PICTURES + 0x10;

/// Frames a second the thread describes, as the Orrery.
const FPS: f32 = 30.0;
/// Pixels of picture to a shown pixel: the plate and the scene are drawn
/// twice their shown size and shrunk by the layer, so a leaned plate's
/// text and the heart's points stay smooth.
pub(crate) const SCALE: f32 = 2.0;
/// How long a new window size holds before the plate is drawn for it.
const SETTLE: Duration = Duration::from_millis(250);
/// The scene's texture is never larger than this a side.
const MAX_SCENE_SIDE: u32 = 1400;

/// The plate's colours as the layers want them.
const PLATE_FILL: [f32; 4] = [0.008, 0.016, 0.032, 0.87];
const ROSE: [f32; 4] = [1.0, 0.30, 0.42, 1.0];
const CYAN: [f32; 4] = [0.35, 0.85, 1.0, 1.0];
const GOLD: [f32; 4] = [1.0, 0.78, 0.30, 1.0];

/// The panel's typeface: Share Tech Mono, carried in the binary and set
/// down once under `~/.angelX/fonts` for the font loader, which reads a
/// file. `ANGEL_GLASS_HEART_FONT` names another (a family or a file); the
/// window's own font stands in when the file cannot be written.
fn heart_font() -> String {
    const SHARE_TECH_MONO: &[u8] =
        include_bytes!("../../../assets/fonts/ShareTechMono-Regular.ttf");
    if let Ok(family) = std::env::var("ANGEL_GLASS_HEART_FONT")
        && !family.trim().is_empty()
    {
        return family;
    }
    let dir = crate::platform::workspace_store::angel_subdir("fonts");
    let path = dir.join("ShareTechMono-Regular.ttf");
    let current = std::fs::read(&path).is_ok_and(|bytes| bytes == SHARE_TECH_MONO);
    // Written aside and renamed in, so another window never reads half a file.
    let written = current || {
        let aside = dir.join(format!(".ShareTechMono-{}.ttf", std::process::id()));
        std::fs::create_dir_all(&dir).is_ok()
            && std::fs::write(&aside, SHARE_TECH_MONO).is_ok()
            && std::fs::rename(&aside, &path).is_ok()
    };
    if written {
        path.to_string_lossy().into_owned()
    } else {
        angel_glass::Options::from_environment().font_family
    }
}

/// The panel's font, cached per size: loading runs fontconfig.
struct Fonts {
    family: String,
    /// The window's own font, and the size (picture pixels) under which
    /// the floating HUD sets its text in it; 0 for never (the plate).
    small_family: String,
    small_below: f32,
    pagers: HashMap<(bool, u32), angel_glass::page::Pager>,
    /// A size failed to load since this was last cleared: fontconfig can
    /// fail under load, so what was drawn without it is drawn again later.
    missed: bool,
}

impl Typeset for Fonts {
    fn text(&mut self, line: &str, size: f32, ink: [u8; 3], bold: bool) -> Option<Ink> {
        let line: String = line.chars().filter(|c| !c.is_control()).take(200).collect();
        if line.trim().is_empty() {
            return None;
        }
        let px = size.round().clamp(6.0, 160.0) as u32;
        let small = size < self.small_below;
        if self.pagers.len() > 24 {
            self.pagers.clear();
        }
        if !self.pagers.contains_key(&(small, px)) {
            let family = if small {
                &self.small_family
            } else {
                &self.family
            };
            match angel_glass::page::Pager::load(family, px) {
                Some(pager) => {
                    self.pagers.insert((small, px), pager);
                }
                None => {
                    self.missed = true;
                    return None;
                }
            }
        }
        let pager = self.pagers.get_mut(&(small, px))?;
        let style = if bold {
            angel_glass::page::BOLD_STYLE
        } else {
            angel_glass::page::PLAIN
        };
        pager.render(&[line.as_str()], style, ink, [0, 0, 0, 0])
    }
}

/// What is on the GPU and on screen, so each thing goes up only when it
/// changed.
struct Show {
    fonts: Fonts,
    /// The cloud the meshes were made from, and what they are.
    meshes_of: Option<Arc<Cloud>>,
    built: Vec<Built>,
    view: Option<CloudView>,
    /// Meshes must go up again (first show, or shown again after hiding).
    stale: bool,
    plate_key: Option<(Layout, Option<Arc<Campaign>>, Option<CloudView>)>,
    /// A layout the window has just changed to, and since when: while a
    /// window is being sized the plate is stretched, and drawn afresh once
    /// the size has held for [`SETTLE`].
    settling: Option<(Layout, Instant)>,
    titles_key: Option<(Option<Arc<Cloud>>, u32)>,
    /// When text that failed to set (no font yet) is tried again.
    retry_text: Option<Instant>,
    scanlines_key: Option<(u32, u32)>,
    clock: [String; 3],
    clock_size: [(f32, f32); 3],
    titles: Vec<(f32, f32)>,
    banner: Option<(f32, f32)>,
    before: Option<Arc<Campaign>>,
    flare: Option<Flare>,
    shown: bool,
    /// The floating HUD's own state (`/heart float`).
    float: float::Float,
}

/// The thread: polls the folder, keeps the GPU's copies fresh and
/// describes a frame 30 times a second while the panel is wanted.
pub(crate) fn run(host: Host) {
    let started = Instant::now();
    let dir = super::campaign::home();
    let real_dir = dir
        .as_ref()
        .map_or_else(|| "$ANGEL_VEC_HOME".to_string(), |d| tilde(d));
    let now_unix = || {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    };
    let mut demo_on = super::demo();
    let mut shown_dir = if demo_on {
        "synthetic demo".to_string()
    } else {
        real_dir.clone()
    };
    let mut watcher = if demo_on {
        Some(Watcher::demo(now_unix()))
    } else {
        dir.clone().map(Watcher::new)
    };
    let mut show = Show {
        fonts: Fonts {
            family: heart_font(),
            small_family: angel_glass::Options::from_environment().font_family,
            small_below: 0.0,
            pagers: HashMap::new(),
            missed: false,
        },
        meshes_of: None,
        built: Vec::new(),
        view: None,
        stale: true,
        plate_key: None,
        settling: None,
        titles_key: None,
        retry_text: None,
        scanlines_key: None,
        clock: Default::default(),
        clock_size: [(0.0, 0.0); 3],
        titles: Vec::new(),
        banner: None,
        before: None,
        flare: None,
        shown: false,
        float: float::Float::new(),
    };
    let frame = Duration::from_secs_f32(1.0 / FPS);
    loop {
        if !super::visible() {
            show.hide(&host);
            std::thread::sleep(Duration::from_millis(150));
            continue;
        }
        let t = started.elapsed().as_secs_f32();
        // `/heart demo` (and leaving it by naming a look) swaps the
        // watcher wholesale: demo serves from memory, real from folder.
        if super::demo() != demo_on {
            demo_on = super::demo();
            watcher = if demo_on {
                Some(Watcher::demo(now_unix()))
            } else {
                dir.clone().map(Watcher::new)
            };
            shown_dir = if demo_on {
                "synthetic demo".to_string()
            } else {
                real_dir.clone()
            };
            show.board_changed(watcher.as_ref().and_then(|w| w.campaign().cloned()), t);
        }
        if let Some(watcher) = watcher.as_mut() {
            let changes = watcher.poll(Instant::now());
            if changes.board {
                show.board_changed(watcher.campaign().cloned(), t);
            }
        }
        let campaign = watcher.as_ref().and_then(|w| w.campaign().cloned());
        let cloud = watcher.as_ref().and_then(|w| w.cloud().cloned());
        let grid = host.grid_size();
        let grid = Grid {
            width: grid.width_px,
            height: grid.height_px,
        };
        if super::floating() {
            let size = host.grid_size();
            let cell = (size.cell_width as f32, size.cell_height as f32);
            show.frame_float(&host, grid, cell, t, campaign, cloud, &shown_dir);
            std::thread::sleep(frame);
            continue;
        }
        show.float.release_zones(&host);
        if show.float.active {
            // From the floating look: the clock and titles again, in the
            // plate's font.
            show.float.active = false;
            show.clock = Default::default();
            show.titles_key = None;
        }
        let boards = campaign
            .as_ref()
            .map_or(super::DEFAULT_BOARDS, |c| c.boards.len());
        let layout = Layout::for_grid(grid, crate::ui::orrery::shown_panel(grid), boards);
        match layout {
            None => show.hide(&host),
            Some(layout) => show.frame(&host, &layout, t, campaign, cloud, &shown_dir),
        }
        std::thread::sleep(frame);
    }
}

/// A path with the home folder written `~`.
fn tilde(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && text.starts_with(&home) => {
            format!("~{}", &text[home.len()..])
        }
        _ => text,
    }
}

/// Unix seconds now.
fn wall_clock() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn upload(host: &Host, id: u32, ink: Ink) {
    let (width, height, rgba) = ink;
    if width == 0 || height == 0 || rgba.len() != (width * height * 4) as usize {
        return;
    }
    host.upload_picture(angel_glass::graphics::Picture {
        id,
        width,
        height,
        rgba,
        cols: 0,
        rows: 0,
    });
}

impl Show {
    fn hide(&mut self, host: &Host) {
        self.float.release_zones(host);
        if self.shown {
            host.set_layer_group(GROUP, Vec::new());
            host.remove_scene(SCENE);
            self.shown = false;
            // Shown again later, everything goes up again: a long hide may
            // have let the window's budgets drop the meshes and pictures.
            self.stale = true;
            self.plate_key = None;
            self.titles_key = None;
            self.scanlines_key = None;
            self.clock = Default::default();
            self.banner = None;
            self.float.forget();
        }
    }

    /// A new board: a new official score since the last one flares.
    fn board_changed(&mut self, campaign: Option<Arc<Campaign>>, t: f32) {
        if let (Some(now), Some(before)) = (campaign.as_ref(), self.before.as_ref())
            && let Some((row, _)) = now.new_official_since(before)
        {
            self.flare = Some(Flare { at: t, row });
            self.banner = None;
        }
        if campaign.is_some() {
            self.before = campaign;
        }
    }

    fn frame(
        &mut self,
        host: &Host,
        layout: &Layout,
        t: f32,
        campaign: Option<Arc<Campaign>>,
        cloud: Option<Arc<Cloud>>,
        dir: &str,
    ) {
        self.shown = true;
        self.upload_meshes(host, &cloud);
        self.retry_due();

        // The plate, when anything printed on it changed; a new size only
        // once it has settled, so sizing a window does not draw it per frame.
        let drawn_layout = self.plate_key.as_ref().map(|k| &k.0);
        let settled = match (&self.settling, drawn_layout) {
            _ if drawn_layout.is_none_or(|d| d == layout) => {
                self.settling = None;
                true
            }
            (Some((pending, since)), _) if pending == layout => since.elapsed() >= SETTLE,
            _ => {
                self.settling = Some((layout.clone(), Instant::now()));
                false
            }
        };
        let key = (layout.clone(), campaign.clone(), self.view.clone());
        if settled && self.plate_key.as_ref() != Some(&key) {
            let view = View {
                campaign: campaign.as_deref(),
                cloud: self.view.as_ref(),
                dir,
            };
            upload(
                host,
                PLATE,
                plate::draw(layout, &view, SCALE, &mut self.fonts),
            );
            self.plate_key = Some(key);
            self.clock = Default::default();
        }

        // The stage titles, once per cloud and width.
        self.stage_titles(host, &cloud, campaign.as_deref(), layout.heart.w, settled);

        if self.fonts.missed && self.retry_text.is_none() {
            self.retry_text = Some(Instant::now() + Duration::from_secs(3));
        }

        // The scanlines over the viewport, once per size.
        let size = (
            (layout.heart.w * SCALE).round() as u32,
            (layout.heart.h * SCALE).round() as u32,
        );
        if self.scanlines_key.is_none() || (settled && self.scanlines_key != Some(size)) {
            upload(host, SCANLINES, scanlines(size));
            self.scanlines_key = Some(size);
        }

        self.clock_pictures(host, campaign.as_deref());
        let flare = self.flare_and_banner(host, t, campaign.as_deref());

        // The scene: the heart turning, every frame.
        let scene_size = (
            (layout.heart.w * SCALE)
                .round()
                .clamp(16.0, MAX_SCENE_SIDE as f32) as u32,
            (layout.heart.h * SCALE)
                .round()
                .clamp(16.0, MAX_SCENE_SIDE as f32) as u32,
        );
        host.set_scene(
            SCENE,
            heart::scene(&self.built, Pose { t, flare }),
            scene_size,
        );
        let layers = self.layers(layout, t, flare, campaign.as_deref());
        host.set_layer_group(GROUP, layers);
    }

    /// The stages' meshes, when the cloud changed or the panel came back.
    fn upload_meshes(&mut self, host: &Host, cloud: &Option<Arc<Cloud>>) {
        let new_cloud = match (&self.meshes_of, cloud) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        };
        if new_cloud || self.stale {
            if self.stale {
                host.upload_mesh(heart::RING_MESH, heart::ring_mesh());
            }
            self.built.clear();
            if let Some(cloud) = cloud {
                for i in 0..cloud.stages.len() {
                    if let Some((built, meshes)) = heart::stage_meshes(cloud, i) {
                        for (id, mesh) in meshes {
                            host.upload_mesh(id, mesh);
                        }
                        self.built.push(built);
                    }
                }
            }
            self.view = cloud.as_deref().map(CloudView::of);
            self.meshes_of = cloud.clone();
            self.stale = false;
        }
    }

    /// Text that could not be set for want of a font is set again a little
    /// later: the plate, the modules and the titles are drawn afresh.
    fn retry_due(&mut self) {
        if self.retry_text.is_some_and(|at| Instant::now() >= at) {
            self.retry_text = None;
            self.plate_key = None;
            self.titles_key = None;
            self.float.forget();
        }
        self.fonts.missed = false;
    }

    /// The stage titles, once per cloud and viewport width (a new width
    /// once it has settled).
    fn stage_titles(
        &mut self,
        host: &Host,
        cloud: &Option<Arc<Cloud>>,
        campaign: Option<&Campaign>,
        heart_w: f32,
        settled: bool,
    ) {
        let width = heart_w.round() as u32;
        let titles_key = (cloud.clone(), width);
        let stale_titles = self
            .titles_key
            .as_ref()
            .is_none_or(|k| !same(&k.0, cloud) || (settled && k.1 != width));
        if stale_titles {
            self.titles.clear();
            for (i, built) in self.built.iter().enumerate() {
                let tag = plate::board_tag(&built.board, campaign);
                let mut ink =
                    plate::stage_title(built, &tag, heart_w * 0.6, SCALE, &mut self.fonts);
                if self.float.active {
                    plate::lift(&mut ink.2);
                }
                self.titles
                    .push((ink.0 as f32 / SCALE, ink.1 as f32 / SCALE));
                upload(host, TITLES + i as u32, ink);
            }
            self.titles_key = Some(titles_key);
        }
    }

    /// The clock's pictures, when their text changed.
    fn clock_pictures(&mut self, host: &Host, campaign: Option<&Campaign>) {
        if let Some(c) = campaign {
            let (countdown, reset, age) = plate::clock_text(c, wall_clock());
            for (slot, (text, id, size, ink)) in [
                (countdown, CLOCK, plate::MEDIUM, plate::INK),
                (reset, RESET, plate::TINY, plate::DIM),
                (age, AGO, plate::TINY, plate::DIM),
            ]
            .into_iter()
            .enumerate()
            {
                if self.clock[slot] != text {
                    match plate::label(&text, size, ink, slot == 0, SCALE, &mut self.fonts) {
                        Some(mut picture) => {
                            if self.float.active {
                                plate::lift(&mut picture.2);
                            }
                            self.clock_size[slot] =
                                (picture.0 as f32 / SCALE, picture.1 as f32 / SCALE);
                            upload(host, id, picture);
                        }
                        None => self.clock_size[slot] = (0.0, 0.0),
                    }
                    self.clock[slot] = text;
                }
            }
        }
    }

    /// The flare's strength now, and its banner when one is wanted.
    fn flare_and_banner(&mut self, host: &Host, t: f32, campaign: Option<&Campaign>) -> f32 {
        let flare = self.flare.as_ref().map_or(0.0, |f| f.strength(t));
        if self.flare.as_ref().is_some_and(|f| f.over(t)) {
            self.flare = None;
        }
        if let (Some(f), Some(c)) = (&self.flare, campaign)
            && self.banner.is_none()
            && let Some(board) = c.boards.get(f.row)
            && let Some(mark) = board.latest_official()
            && let Some(mut ink) =
                plate::flare_banner(&board.short, mark.score, SCALE, &mut self.fonts)
        {
            if self.float.active {
                plate::lift(&mut ink.2);
            }
            self.banner = Some((ink.0 as f32 / SCALE, ink.1 as f32 / SCALE));
            upload(host, BANNER, ink);
        }
        flare
    }

    /// The layer set for this frame, back to front.
    fn layers(&self, l: &Layout, t: f32, flare: f32, campaign: Option<&Campaign>) -> Vec<Layer> {
        let p = l.panel;
        let centre = p.centre();
        let leaned = |id: u64, z: i32, rect: [f32; 4], kind: LayerKind| {
            let (rect, angle) = super::leaned(rect, centre);
            let mut layer = Layer::new(id, z, Placement::Pixels(rect), kind);
            layer.angle = angle;
            layer
        };
        let beat = heart::beat(t);
        let now = heart::moment(t, self.built.len());
        let ours = now.is_some_and(|m| self.built[m.to].predicted);
        let mut out: Vec<Layer> = Vec::with_capacity(96);

        // The cast shadow on the terminal's floor, unturned, and the
        // plate's own light pooling under its near edge.
        for (id, (dx, dy, h, a)) in [
            (0u64, (8.0f32, 28.0f32, 0.42f32, 0.16f32)),
            (1, (4.0, 14.0, 0.28, 0.22)),
        ] {
            out.push(Layer::new(
                id,
                0,
                Placement::Pixels([p.x + dx, p.y + dy, p.w, p.h * h]),
                LayerKind::Solid([0.0, 0.0, 0.0, a]),
            ));
        }
        let mut pool = Layer::new(
            2,
            0,
            Placement::Pixels([p.x + 16.0, p.y + p.h, p.w - 32.0, 90.0_f32.min(p.h * 0.2)]),
            LayerKind::Glow([0.95, 0.25, 0.40, 0.14]),
        );
        pool.additive = true;
        out.push(pool);

        // The plate.
        let mut plate = leaned(3, 1, [p.x, p.y, p.w, p.h], LayerKind::Solid(PLATE_FILL));
        plate.breathe = 1.0;
        out.push(plate);

        // Behind the heart: its aura, beating, and the projector's light.
        let v = l.heart;
        let size = ((v.w * SCALE) as u32, (v.h * SCALE) as u32);
        let at =
            |p3: [f32; 3]| heart::project(p3, size).map(|f| [v.x + v.w * f[0], v.y + v.h * f[1]]);
        if let Some(c) = at([0.0, 0.0, 0.0]) {
            let r = v.h * 0.62 * (1.0 + 0.25 * flare);
            let mut aura = leaned(
                4,
                2,
                [c[0] - r, c[1] - r * 0.95, 2.0 * r, 1.9 * r],
                LayerKind::Glow([1.0, 0.22, 0.36, 1.0]),
            );
            aura.opacity = (0.16 + 0.14 * beat + 0.45 * flare).min(1.0);
            aura.additive = true;
            out.push(aura);
        }
        if let Some(c) = at([0.0, heart::RING_Y, 0.0]) {
            let (w, h) = (v.w * 0.62, v.h * 0.22);
            let rgb = if ours { GOLD } else { CYAN };
            let mut light = leaned(
                5,
                2,
                [c[0] - w / 2.0, c[1] - h / 2.0, w, h],
                LayerKind::Glow([rgb[0], rgb[1], rgb[2], 0.32]),
            );
            light.additive = true;
            light.flicker = 0.10;
            out.push(light);
        }

        // The heart itself, the scanlines over it, and a slow sweep.
        out.push(leaned(
            6,
            3,
            [v.x, v.y, v.w, v.h],
            LayerKind::Picture(SCENE),
        ));
        let mut lines = leaned(7, 4, [v.x, v.y, v.w, v.h], LayerKind::Picture(SCANLINES));
        lines.opacity = 0.85;
        out.push(lines);
        let sweep = (t * 0.11).fract();
        let mut band = leaned(
            8,
            4,
            [v.x, v.y + sweep * (v.h - 20.0), v.w, 20.0],
            LayerKind::Solid([0.40, 0.80, 1.0, 0.045]),
        );
        band.additive = true;
        out.push(band);

        // The printed plate.
        out.push(leaned(
            9,
            5,
            [p.x, p.y, p.w, p.h],
            LayerKind::Picture(PLATE),
        ));

        // The clock: the countdown under COLLAB ENDS, the reset over the
        // right column, the ticker's age at its right.
        if campaign.is_some() {
            let (w, h) = self.clock_size[0];
            if w > 0.0 {
                out.push(leaned(
                    10,
                    6,
                    [l.title.x + l.title.w - w, l.title.y + 16.0, w, h],
                    LayerKind::Picture(CLOCK),
                ));
            }
            let (w, h) = self.clock_size[1];
            if w > 0.0 {
                let right = l.legend.x + l.legend.w;
                out.push(leaned(
                    11,
                    6,
                    [right - w, l.legend.y + (l.legend.h - h) / 2.0, w, h],
                    LayerKind::Picture(RESET),
                ));
            }
            let (w, h) = self.clock_size[2];
            if w > 0.0 {
                out.push(leaned(
                    12,
                    6,
                    [l.ticker.x + l.ticker.w - w, l.ticker.y + 8.0, w, h],
                    LayerKind::Picture(AGO),
                ));
            }
        }

        // The stage's title, top left in the viewport, crossfading with
        // the scan; the rail's highlight sliding to the stage coming in.
        if let Some(m) = now {
            let n = self.built.len();
            let mut title = |i: usize, opacity: f32, id: u64| {
                if let Some(&(w, h)) = self.titles.get(i)
                    && opacity > 0.01
                {
                    let mut layer = leaned(
                        id,
                        6,
                        [v.x + 10.0, v.y + 8.0, w, h],
                        LayerKind::Picture(TITLES + i as u32),
                    );
                    layer.opacity = opacity;
                    out.push(layer);
                }
            };
            if m.scanning() {
                if let Some(from) = m.from {
                    title(from, 1.0 - heart::smoothstep(0.0, 0.35, m.k), 13);
                }
                title(m.to, heart::smoothstep(0.5, 1.0, m.k), 14);
            } else {
                title(m.to, 1.0, 14);
            }
            let to = plate::stage_slot(l.stages, m.to, n);
            let from = m.from.map_or(to, |f| plate::stage_slot(l.stages, f, n));
            let k = if m.scanning() {
                heart::smoothstep(0.0, 1.0, m.k)
            } else {
                1.0
            };
            let x = from[0] + (to[0] - from[0]) * k;
            let rgb = if self.built[m.to].predicted {
                GOLD
            } else {
                CYAN
            };
            let mut glow = leaned(
                15,
                4,
                [x - 44.0, to[1] - 13.0, 88.0, 26.0],
                LayerKind::Glow([rgb[0], rgb[1], rgb[2], 0.42]),
            );
            glow.additive = true;
            out.push(glow);
            // How long the stage has been held, as a line under its label.
            let held = if m.scanning() {
                0.0
            } else {
                hold_fraction(t, n)
            };
            let mut bar = leaned(
                16,
                6,
                [x - 26.0, to[1] + 9.0, (52.0 * held).max(1.0), 1.6],
                LayerKind::Solid([rgb[0], rgb[1], rgb[2], 0.8]),
            );
            bar.additive = true;
            out.push(bar);
            // The scan's line across the viewport while a stage builds.
            if m.scanning()
                && let Some(c) = at([0.0, heart::scan_height(m.k), 0.0])
            {
                let fade =
                    heart::smoothstep(0.0, 0.08, m.k) * (1.0 - heart::smoothstep(0.9, 1.0, m.k));
                let mut line = leaned(
                    17,
                    7,
                    [v.x + 4.0, c[1] - 1.0, v.w - 8.0, 2.0],
                    LayerKind::Solid([0.75, 0.95, 1.0, 0.55]),
                );
                line.opacity = fade;
                line.additive = true;
                out.push(line);
                let mut haze = leaned(
                    18,
                    7,
                    [v.x, c[1] - 18.0, v.w, 36.0],
                    LayerKind::Glow([0.4, 0.85, 1.0, 0.35]),
                );
                haze.opacity = fade;
                haze.additive = true;
                out.push(haze);
            }
        }

        // Our best on each board: a live ember on the head of its bar.
        if let Some(c) = campaign {
            let domain = plate::domain(Some(c));
            for (i, (row, board)) in l.rows.iter().zip(&c.boards).enumerate() {
                let Some(best) = &board.ours_best else {
                    continue;
                };
                let tr = plate::track(*row);
                let x = plate::score_x(best.score, domain, tr.x0, tr.x1);
                let rgb = if board.heart { ROSE } else { CYAN };
                let mut ember = leaned(
                    30 + i as u64,
                    6,
                    [x - 14.0, tr.y - 9.0, 28.0, 18.0],
                    LayerKind::Glow([rgb[0], rgb[1], rgb[2], 0.75]),
                );
                ember.additive = true;
                ember.flicker = 0.25;
                ember.opacity = 0.65 + 0.35 * beat;
                out.push(ember);
            }
        }

        // The rim of light round the plate, and the bracket chrome: an L
        // at each corner, a little out from the rim.
        let (bx, by, bw, bh) = (p.x - 3.0, p.y - 3.0, p.w + 6.0, p.h + 6.0);
        for (id, edge) in [
            (50u64, [bx, by, bw, 1.5]),
            (51, [bx, by + bh - 1.5, bw, 1.5]),
            (52, [bx, by, 1.5, bh]),
            (53, [bx + bw - 1.5, by, 1.5, bh]),
        ] {
            let mut rim = leaned(
                id,
                7,
                edge,
                LayerKind::Solid([1.0, 0.36, 0.50, 0.30 + 0.30 * flare]),
            );
            rim.additive = true;
            out.push(rim);
        }
        let arm = (p.w * 0.12).clamp(22.0, 64.0);
        let (x0, y0, x1, y1) = (p.x - 9.0, p.y - 9.0, p.x + p.w + 9.0, p.y + p.h + 9.0);
        let mut id = 60u64;
        for (cx, cy, sx, sy) in [
            (x0, y0, 1.0, 1.0),
            (x1, y0, -1.0, 1.0),
            (x0, y1, 1.0, -1.0),
            (x1, y1, -1.0, -1.0),
        ] {
            // The arm along the edge and the arm down the side, from the corner.
            let across = [
                if sx > 0.0 { cx } else { cx - arm },
                if sy > 0.0 { cy } else { cy - 3.0 },
                arm,
                3.0,
            ];
            let down = [
                if sx > 0.0 { cx } else { cx - 3.0 },
                if sy > 0.0 { cy } else { cy - arm },
                3.0,
                arm,
            ];
            for rect in [across, down] {
                let mut bracket = leaned(id, 7, rect, LayerKind::Solid([1.0, 0.42, 0.55, 0.92]));
                bracket.flicker = 0.08;
                out.push(bracket);
                id += 1;
            }
        }

        // The heartbeat's trace in the title strip: the newest beat at the
        // right, older ones fading off to the left, a bright head.
        let e = plate::ecg_strip(l.title);
        const SAMPLES: usize = 40;
        const WINDOW: f32 = 2.6;
        let amp = e.h * 0.78 * (1.0 + 0.5 * flare);
        let base = e.y + e.h * 0.62;
        let points: Vec<[f32; 2]> = (0..SAMPLES)
            .map(|i| {
                let k = i as f32 / (SAMPLES - 1) as f32;
                let at = t - (1.0 - k) * WINDOW;
                super::leaned_point([e.x + e.w * k, base - heart::ecg(at) * amp], centre)
            })
            .collect();
        for (i, pair) in points.windows(2).enumerate() {
            let (rect, angle) = segment_rect(pair[0], pair[1], 1.7);
            let k = (i + 1) as f32 / (SAMPLES - 1) as f32;
            let mut seg = Layer::new(
                100 + i as u64,
                7,
                Placement::Pixels(rect),
                LayerKind::Solid([1.0, 0.36, 0.48, 1.0]),
            );
            seg.angle = angle.rem_euclid(std::f32::consts::TAU);
            seg.opacity = 0.12 + 0.88 * k * k;
            seg.additive = true;
            out.push(seg);
        }
        if let Some(head) = points.last() {
            let mut dot = Layer::new(
                99,
                7,
                Placement::Pixels([head[0] - 9.0, head[1] - 7.0, 18.0, 14.0]),
                LayerKind::Glow([1.0, 0.75, 0.80, 1.0]),
            );
            dot.additive = true;
            out.push(dot);
        }

        // Motes rising through the viewport.
        let mut motes = Layer::new(
            70,
            8,
            Placement::Pixels([v.x + 10.0, v.y + 10.0, v.w - 20.0, v.h - 20.0]),
            LayerKind::Particles(Particles {
                color: [1.0, 0.45, 0.58, 0.30],
                count: 16,
                size: 2.6,
                life: 5.5,
                rise: 26.0,
                sway: 5.0,
                seed: 0x4845_4152,
                burst: None,
            }),
        );
        motes.additive = true;
        out.push(motes);

        // A newly landed official score: the heart flares, sparks fly, the
        // board's row lights and a banner says what landed.
        if let Some(f) = &self.flare {
            let age = t - f.at;
            if let Some(c) = at([0.0, 0.0, 0.0]) {
                let r = v.h * (0.4 + 0.5 * (age / 1.2).min(1.0));
                let mut burst = leaned(
                    80,
                    8,
                    [c[0] - r, c[1] - r, 2.0 * r, 2.0 * r],
                    LayerKind::Glow([1.0, 0.55, 0.62, 0.9]),
                );
                burst.opacity = flare;
                burst.additive = true;
                out.push(burst);
                let mut sparks = Layer::new(
                    81,
                    9,
                    Placement::Pixels([c[0] - 20.0, c[1] - 20.0, 40.0, 40.0]),
                    LayerKind::Particles(Particles {
                        color: [1.0, 0.70, 0.75, 0.9],
                        count: 48,
                        size: 3.0,
                        life: 2.4,
                        rise: 150.0,
                        sway: 60.0,
                        seed: 0x464C_4152,
                        burst: Some(age),
                    }),
                );
                sparks.additive = true;
                out.push(sparks);
            }
            if let Some(row) = l.rows.get(f.row) {
                let mut lit = leaned(
                    82,
                    8,
                    [row.x - 6.0, row.y - 1.0, row.w + 12.0, row.h],
                    LayerKind::Solid([1.0, 0.35, 0.5, 0.20]),
                );
                lit.opacity = flare;
                lit.additive = true;
                out.push(lit);
            }
            if let Some((w, h)) = self.banner {
                let mut banner = leaned(
                    83,
                    9,
                    [v.x + (v.w - w) / 2.0, v.y + v.h - h - 10.0, w, h],
                    LayerKind::Picture(BANNER),
                );
                banner.opacity = (flare * 1.6).min(1.0);
                out.push(banner);
            }
        }
        out
    }
}

/// Whether two optional `Arc`s are the same value by pointer.
fn same<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// How far into its hold the stage in front is, 0 to 1.
fn hold_fraction(t: f32, n: usize) -> f32 {
    if n < 2 {
        return 1.0;
    }
    let slot = heart::HOLD + heart::SCAN;
    ((t.rem_euclid(slot) - heart::SCAN) / heart::HOLD).clamp(0.0, 1.0)
}

/// The viewport's scanlines: a dark line every third shown pixel, and a
/// faint dark edge so the heart sits in a lens. Straight RGBA.
fn scanlines((w, h): (u32, u32)) -> Ink {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let line = (y as f32 / SCALE) as u32 % 3 == 2;
        for x in 0..w {
            let edge = {
                let fx = (x as f32 / w.max(1) as f32 - 0.5).abs() * 2.0;
                let fy = (y as f32 / h.max(1) as f32 - 0.5).abs() * 2.0;
                (fx.max(fy) - 0.86).max(0.0) / 0.14
            };
            let a = (if line { 0.13f32 } else { 0.0 }).max(edge * edge * 0.6);
            let o = ((y * w + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[0, 4, 12, (a * 255.0).round() as u8]);
        }
    }
    (w, h, rgba)
}
