//! The floating Heartglass in the window: each module of the HUD
//! (`heartglass::hud`) as its own printed picture over a soft halo, the
//! layers that move on it, the pointer taking modules by their bodies,
//! grips and hide buttons, and the hover chrome that shows which is which.
//!
//! A module's print and halo are drawn on the CPU only when what it shows
//! or its size changed, the halo at one pixel per shown pixel from the
//! print's own ink, spread and feathered: a drop shadow that follows the
//! text, never a slab. Moving a module moves its pictures; resizing one
//! stretches them until the new size has held, then draws them afresh.

use std::sync::Arc;
use std::time::{Duration, Instant};

use angel_glass::{Host, Layer, LayerKind, Particles, Placement, PointerEvent, Zone, ZoneKind};

use super::super::campaign::{Campaign, Cloud};
use super::super::heart::{self, Pose};
use super::super::hud::{self, Grab, Hud, Module, Part, Placed, Saved};
use super::super::plate::{self, CloudView, Ink, View};
use super::super::{Grid, Panel};
use super::{
    AGO, BANNER, CLOCK, CYAN, GOLD, GROUP, PICTURES, RESET, ROSE, SCALE, SCENE, SETTLE, Show,
    TITLES,
};
use crate::ui::orrery::segment_rect;

/// The modules' prints, their halos and their names on the hover tab, one
/// each by `Module::index`; the heart's scanlines.
const PRINTS: u32 = PICTURES + 0x20;
const HALOS: u32 = PICTURES + 0x28;
const NAMES: u32 = PICTURES + 0x30;
const SCANLINES: u32 = PICTURES + 0x38;

/// The halo's colour: a blue so deep it reads as shade.
const SHADE: [u8; 3] = [3, 6, 12];
/// Its strength: hugging the print's marks (a soft outline, so the HUD's
/// text holds over the text under it), bloomed wide round them, washed
/// over the module's content, and behind the heart. Never opaque: the
/// text under it stays readable when looked for.
const HALO_TIGHT: f32 = 0.66;
const HALO_SOFT: f32 = 0.26;
const HALO_WASH: f32 = 0.58;
const HALO_HEART: f32 = 0.45;
const HALO_MAX: f32 = 0.84;

/// Layer ids of the floating panel, clear of the plate's.
const ID: u64 = 1000;

/// What a module's pictures were drawn for.
#[derive(Clone, Debug, PartialEq)]
struct PrintKey {
    natural: (f32, f32),
    scale: f32,
    campaign: Option<Arc<Campaign>>,
    view: Option<CloudView>,
}

/// A module's new size at scale 1, its new scale, and since when.
type Pending = ((f32, f32), f32, Instant);

/// The floating panel's own state on the thread.
pub(super) struct Float {
    pub(super) saved: Saved,
    path: std::path::PathBuf,
    grab: Option<Grab>,
    /// The module the pointer was over last frame: its tab and hide
    /// button show, and take the pointer.
    hovered: Option<Module>,
    /// The floating look was drawn last frame (a switch from the plate
    /// sets the clock's and titles' text afresh, in this look's fonts).
    pub(super) active: bool,
    /// The zones last handed to the window.
    zones: Vec<Zone>,
    drawn: [Option<PrintKey>; 6],
    /// A size a module has changed to and since when: its pictures stretch
    /// until it has held for [`SETTLE`].
    pending: [Option<Pending>; 6],
    /// The names on the hover tabs, shown size; drawn once.
    names: [(f32, f32); 6],
    scanlines_key: Option<(u32, u32)>,
}

impl Float {
    pub(super) fn new() -> Self {
        let path = hud::saved_path();
        Self {
            saved: Saved::load(&path),
            path,
            grab: None,
            hovered: None,
            active: false,
            zones: Vec::new(),
            drawn: Default::default(),
            pending: [None; 6],
            names: [(0.0, 0.0); 6],
            scanlines_key: None,
        }
    }

    /// Everything goes up again when shown next.
    pub(super) fn forget(&mut self) {
        self.drawn = Default::default();
        self.pending = [None; 6];
        self.names = [(0.0, 0.0); 6];
        self.scanlines_key = None;
        self.grab = None;
        self.hovered = None;
    }

    /// Give the window its buttons back.
    pub(super) fn release_zones(&mut self, host: &Host) {
        if !self.zones.is_empty() {
            host.set_pointer_zones(GROUP, Vec::new());
            self.zones.clear();
        }
    }

    fn keep(&self) {
        if let Err(error) = self.saved.save(&self.path) {
            eprintln!("heartglass: the floating layout was not kept: {error}");
        }
    }
}

impl Show {
    /// One frame of the floating HUD.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_float(
        &mut self,
        host: &Host,
        grid: Grid,
        cell: (f32, f32),
        t: f32,
        campaign: Option<Arc<Campaign>>,
        cloud: Option<Arc<Cloud>>,
        dir: &str,
    ) {
        let boards = campaign
            .as_ref()
            .map_or(super::super::DEFAULT_BOARDS, |c| c.boards.len());
        if super::super::take_reset() {
            self.float.saved = Saved::default();
            self.float.grab = None;
        }
        let Some(before) = hud::arrange(grid, cell, boards, &self.float.saved) else {
            self.hide(host);
            return;
        };
        // The pointer, against the layout as it stood when it acted.
        for event in host.take_pointer_events(GROUP) {
            match event {
                PointerEvent::Press {
                    at, shift, ctrl, ..
                } => {
                    self.float.grab = hud::press(&before, at, shift || ctrl, self.float.hovered);
                }
                PointerEvent::Move { at } => {
                    if let Some(grab) = self.float.grab {
                        hud::drag(&mut self.float.saved, &before, &grab, at);
                    }
                }
                PointerEvent::Release { at, click } => {
                    if let Some(grab) = self.float.grab.take()
                        && hud::release(&mut self.float.saved, &before, &grab, at, click)
                    {
                        self.float.keep();
                    }
                }
            }
        }
        let Some(hud) = hud::arrange(grid, cell, boards, &self.float.saved) else {
            self.hide(host);
            return;
        };
        if !self.float.active {
            // From the plate: its clock and titles are set again in this
            // look's fonts.
            self.float.active = true;
            self.clock = Default::default();
            self.titles_key = None;
        }
        self.shown = true;
        self.upload_meshes(host, &cloud);
        self.retry_due();

        // The window's buttons: each module's hide button, grip and body.
        let zones: Vec<Zone> = hud
            .zones(self.float.hovered)
            .into_iter()
            .map(|(_, r, part)| {
                let kind = match part {
                    Part::Body => ZoneKind::Move,
                    Part::Resize => ZoneKind::Resize,
                    Part::Close => ZoneKind::Button,
                };
                Zone::new([r.x, r.y, r.w, r.h], kind)
            })
            .collect();
        if zones != self.float.zones {
            host.set_pointer_zones(GROUP, zones.clone());
            self.float.zones = zones;
        }

        // Each module's print and halo, when what it shows or its size
        // changed (a new size once it has held).
        let view = View {
            campaign: campaign.as_deref(),
            cloud: self.view.as_ref(),
            dir,
        };
        let held = self.float.grab.is_some();
        for placed in &hud.modules {
            let i = placed.module.index();
            let key = PrintKey {
                natural: placed.natural,
                scale: placed.scale,
                campaign: campaign.clone(),
                view: self.view.clone(),
            };
            if self.float.drawn[i].as_ref() == Some(&key) {
                self.float.pending[i] = None;
                continue;
            }
            let resized = self.float.drawn[i].as_ref().is_some_and(|d| {
                d.campaign == key.campaign
                    && d.view == key.view
                    && (d.natural != key.natural || d.scale != key.scale)
            });
            if resized {
                let geometry = (placed.natural, placed.scale);
                let since = match self.float.pending[i] {
                    Some((n, s, since)) if (n, s) == geometry => since,
                    _ => {
                        self.float.pending[i] = Some((geometry.0, geometry.1, Instant::now()));
                        continue;
                    }
                };
                if held || since.elapsed() < SETTLE {
                    continue;
                }
            }
            let layout = hud::local_layout(placed, boards);
            let k = placed.scale;
            self.fonts.small_below = small_below(SCALE * k);
            let ink = plate::module(placed.module, &layout, &view, SCALE * k, &mut self.fonts);
            let halo = halo(
                &ink,
                (
                    (placed.natural.0 * k).round().max(1.0) as u32,
                    (placed.natural.1 * k).round().max(1.0) as u32,
                ),
                k,
                &shade_for(placed, &layout, self.titles.first().copied()),
            );
            super::upload(host, PRINTS + i as u32, ink);
            super::upload(host, HALOS + i as u32, halo);
            self.float.drawn[i] = Some(key);
            self.float.pending[i] = None;
        }

        // Everything else here is drawn at the plate's scale.
        self.fonts.small_below = small_below(SCALE);

        // The names on the hover tabs.
        if self.float.names.iter().all(|n| n.0 == 0.0) {
            for module in Module::ALL {
                if let Some(ink) = plate::label(
                    module.name(),
                    plate::TINY,
                    [150, 168, 188],
                    false,
                    SCALE,
                    &mut self.fonts,
                ) {
                    self.float.names[module.index()] = (ink.0 as f32 / SCALE, ink.1 as f32 / SCALE);
                    super::upload(host, NAMES + module.index() as u32, ink);
                }
            }
        }

        let heart_panel = hud.get(Module::Heart).map(|p| (p, p.content()));
        let heart_w = heart_panel.map_or(320.0, |(p, c)| c.w / p.scale);
        let settled = self.float.pending[Module::Heart.index()].is_none();
        self.stage_titles(host, &cloud, campaign.as_deref(), heart_w, settled);
        if self.fonts.missed && self.retry_text.is_none() {
            self.retry_text = Some(Instant::now() + Duration::from_secs(3));
        }

        // The heart's scanlines, once per size.
        if let Some((p, _)) = heart_panel {
            let c = p.local_content();
            let size = ((c.w * SCALE).round() as u32, (c.h * SCALE).round() as u32);
            if self.float.scanlines_key != Some(size) {
                super::upload(host, SCANLINES, lens_lines(size));
                self.float.scanlines_key = Some(size);
            }
        }

        self.clock_pictures(host, campaign.as_deref());
        let flare = self.flare_and_banner(host, t, campaign.as_deref());

        // The scene: the heart turning, every frame it shows.
        if let Some((p, _)) = heart_panel {
            let c = p.local_content();
            let k = p.scale;
            let side = |v: f32| {
                (v * k * SCALE)
                    .round()
                    .clamp(16.0, super::MAX_SCENE_SIDE as f32) as u32
            };
            host.set_scene(
                SCENE,
                heart::scene(&self.built, Pose { t, flare }),
                (side(c.w), side(c.h)),
            );
        } else {
            host.remove_scene(SCENE);
        }

        let hover = match self.float.grab {
            Some(g) => Some((g.module, true)),
            None => host
                .pointer()
                .and_then(|p| hud.hit(p, self.float.hovered))
                .map(|(m, _)| (m, false)),
        };
        self.float.hovered = hover.map(|(m, _)| m);
        self.fonts.small_below = 0.0;
        let layers = self.float_layers(&hud, boards, t, flare, campaign.as_deref(), hover);
        host.set_layer_group(GROUP, layers);
    }

    /// The floating panel's layers for this frame.
    fn float_layers(
        &self,
        hud: &Hud,
        boards: usize,
        t: f32,
        flare: f32,
        campaign: Option<&Campaign>,
        hover: Option<(Module, bool)>,
    ) -> Vec<Layer> {
        let mut out: Vec<Layer> = Vec::with_capacity(256);
        let beat = heart::beat(t);
        let now = heart::moment(t, self.built.len());
        let ours = now.is_some_and(|m| self.built[m.to].predicted);
        let mut id = ID;
        let mut push = |out: &mut Vec<Layer>, z: i32, rect: [f32; 4], kind: LayerKind| -> usize {
            out.push(Layer::new(id, z, Placement::Pixels(rect), kind));
            id += 1;
            out.len() - 1
        };

        for placed in &hud.modules {
            let i = placed.module.index();
            let p = placed;
            let k = p.scale;
            let l = hud::local_layout(p, boards);
            let r = p.rect;
            if self.float.drawn[i].is_none() {
                continue;
            }
            // The halo under the text's level, the print over it.
            push(
                &mut out,
                0,
                [r.x, r.y, r.w, r.h],
                LayerKind::Picture(HALOS + i as u32),
            );
            match p.module {
                Module::Title => {
                    if campaign.is_some() {
                        let (w, h) = self.clock_size[0];
                        if w > 0.0 {
                            let t = l.title;
                            push(
                                &mut out,
                                6,
                                p.rect_at([t.x + t.w - w, t.y + 16.0, w, h]),
                                LayerKind::Picture(CLOCK),
                            );
                        }
                    }
                    // The heartbeat's trace: the newest beat at the right.
                    let e = plate::ecg_strip(l.title);
                    let samples = ((e.w / 7.0) as usize).clamp(40, 160);
                    let window = (e.w / 150.0).clamp(2.6, 6.0);
                    let amp = e.h * 0.78 * (1.0 + 0.5 * flare);
                    let base = e.y + e.h * 0.62;
                    let points: Vec<[f32; 2]> = (0..samples)
                        .map(|s| {
                            let f = s as f32 / (samples - 1) as f32;
                            let at = t - (1.0 - f) * window;
                            p.at([e.x + e.w * f, base - heart::ecg(at) * amp])
                        })
                        .collect();
                    for (s, pair) in points.windows(2).enumerate() {
                        let (rect, angle) = segment_rect(pair[0], pair[1], 1.6 * k);
                        let f = (s + 1) as f32 / (samples - 1) as f32;
                        let n = push(&mut out, 7, rect, LayerKind::Solid([1.0, 0.40, 0.52, 1.0]));
                        out[n].angle = angle.rem_euclid(std::f32::consts::TAU);
                        out[n].opacity = 0.10 + 0.80 * f * f;
                        out[n].additive = true;
                    }
                    if let Some(head) = points.last() {
                        let n = push(
                            &mut out,
                            7,
                            [head[0] - 8.0 * k, head[1] - 6.0 * k, 16.0 * k, 12.0 * k],
                            LayerKind::Glow([1.0, 0.72, 0.78, 0.85]),
                        );
                        out[n].additive = true;
                    }
                }
                Module::Heart => {
                    let v = l.heart;
                    let size = ((v.w * SCALE) as u32, (v.h * SCALE) as u32);
                    let at3 = |p3: [f32; 3]| {
                        heart::project(p3, size).map(|f| p.at([v.x + v.w * f[0], v.y + v.h * f[1]]))
                    };
                    let vr = p.rect_at([v.x, v.y, v.w, v.h]);
                    if let Some(c) = at3([0.0, 0.0, 0.0]) {
                        let rr = vr[3] * 0.5 * (1.0 + 0.25 * flare);
                        let n = push(
                            &mut out,
                            1,
                            [c[0] - rr, c[1] - rr * 0.95, 2.0 * rr, 1.9 * rr],
                            LayerKind::Glow([1.0, 0.24, 0.38, 1.0]),
                        );
                        out[n].opacity = (0.10 + 0.10 * beat + 0.45 * flare).min(1.0);
                        out[n].additive = true;
                    }
                    if let Some(c) = at3([0.0, heart::RING_Y, 0.0]) {
                        let (w, h) = (vr[2] * 0.62, vr[3] * 0.20);
                        let rgb = if ours { GOLD } else { CYAN };
                        let n = push(
                            &mut out,
                            2,
                            [c[0] - w / 2.0, c[1] - h / 2.0, w, h],
                            LayerKind::Glow([rgb[0], rgb[1], rgb[2], 0.26]),
                        );
                        out[n].additive = true;
                        out[n].flicker = 0.10;
                    }
                    push(&mut out, 3, vr, LayerKind::Picture(SCENE));
                    let n = push(&mut out, 4, vr, LayerKind::Picture(SCANLINES));
                    out[n].opacity = 0.7;
                    let sweep = (t * 0.11).fract();
                    let n = push(
                        &mut out,
                        4,
                        p.rect_at([v.x + v.w * 0.1, v.y + sweep * (v.h - 24.0), v.w * 0.8, 24.0]),
                        LayerKind::Glow([0.40, 0.80, 1.0, 0.10]),
                    );
                    out[n].additive = true;
                    push(
                        &mut out,
                        5,
                        [r.x, r.y, r.w, r.h],
                        LayerKind::Picture(PRINTS + i as u32),
                    );
                    if let Some(m) = now {
                        let mut title = |out: &mut Vec<Layer>, s: usize, opacity: f32| {
                            if let Some(&(w, h)) = self.titles.get(s)
                                && opacity > 0.01
                            {
                                let n = push(
                                    out,
                                    6,
                                    p.rect_at([v.x + 10.0, v.y + 8.0, w, h]),
                                    LayerKind::Picture(TITLES + s as u32),
                                );
                                out[n].opacity = opacity;
                            }
                        };
                        if m.scanning() {
                            if let Some(from) = m.from {
                                title(&mut out, from, 1.0 - heart::smoothstep(0.0, 0.35, m.k));
                            }
                            title(&mut out, m.to, heart::smoothstep(0.5, 1.0, m.k));
                        } else {
                            title(&mut out, m.to, 1.0);
                        }
                        if m.scanning()
                            && let Some(c) = at3([0.0, heart::scan_height(m.k), 0.0])
                        {
                            let fade = heart::smoothstep(0.0, 0.08, m.k)
                                * (1.0 - heart::smoothstep(0.9, 1.0, m.k));
                            let n = push(
                                &mut out,
                                7,
                                [vr[0] + vr[2] * 0.12, c[1] - k, vr[2] * 0.76, 2.0 * k],
                                LayerKind::Solid([0.75, 0.95, 1.0, 0.45]),
                            );
                            out[n].opacity = fade;
                            out[n].additive = true;
                            let n = push(
                                &mut out,
                                7,
                                [vr[0], c[1] - 18.0 * k, vr[2], 36.0 * k],
                                LayerKind::Glow([0.4, 0.85, 1.0, 0.30]),
                            );
                            out[n].opacity = fade;
                            out[n].additive = true;
                        }
                    }
                    let n = push(
                        &mut out,
                        8,
                        p.rect_at([v.x + v.w * 0.2, v.y + 10.0, v.w * 0.6, v.h - 20.0]),
                        LayerKind::Particles(Particles {
                            color: [1.0, 0.45, 0.58, 0.26],
                            count: 14,
                            size: 2.4 * k,
                            life: 5.5,
                            rise: 26.0 * k,
                            sway: 5.0 * k,
                            seed: 0x4845_4152,
                            burst: None,
                        }),
                    );
                    out[n].additive = true;
                    if let Some(f) = &self.flare {
                        let age = t - f.at;
                        if let Some(c) = at3([0.0, 0.0, 0.0]) {
                            let rr = vr[3] * (0.4 + 0.5 * (age / 1.2).min(1.0));
                            let n = push(
                                &mut out,
                                8,
                                [c[0] - rr, c[1] - rr, 2.0 * rr, 2.0 * rr],
                                LayerKind::Glow([1.0, 0.55, 0.62, 0.8]),
                            );
                            out[n].opacity = flare;
                            out[n].additive = true;
                            let n = push(
                                &mut out,
                                9,
                                [c[0] - 20.0, c[1] - 20.0, 40.0, 40.0],
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
                            out[n].additive = true;
                        }
                        if let Some((w, h)) = self.banner {
                            let n = push(
                                &mut out,
                                9,
                                p.rect_at([v.x + (v.w - w) / 2.0, v.y + v.h - h - 10.0, w, h]),
                                LayerKind::Picture(BANNER),
                            );
                            out[n].opacity = (flare * 1.6).min(1.0);
                        }
                    }
                    continue;
                }
                Module::Stages => {
                    if let Some(m) = now {
                        let n_st = self.built.len();
                        let to = plate::stage_slot(l.stages, m.to, n_st);
                        let from = m.from.map_or(to, |f| plate::stage_slot(l.stages, f, n_st));
                        let kk = if m.scanning() {
                            heart::smoothstep(0.0, 1.0, m.k)
                        } else {
                            1.0
                        };
                        let x = from[0] + (to[0] - from[0]) * kk;
                        let rgb = if self.built[m.to].predicted {
                            GOLD
                        } else {
                            CYAN
                        };
                        let n = push(
                            &mut out,
                            4,
                            p.rect_at([x - 44.0, to[1] - 13.0, 88.0, 26.0]),
                            LayerKind::Glow([rgb[0], rgb[1], rgb[2], 0.36]),
                        );
                        out[n].additive = true;
                        let held = if m.scanning() {
                            0.0
                        } else {
                            super::hold_fraction(t, n_st)
                        };
                        let n = push(
                            &mut out,
                            6,
                            p.rect_at([x - 26.0, to[1] + 9.0, (52.0 * held).max(1.0), 1.6]),
                            LayerKind::Solid([rgb[0], rgb[1], rgb[2], 0.8]),
                        );
                        out[n].additive = true;
                    }
                }
                Module::Boards => {
                    if let Some(c) = campaign {
                        let domain = plate::domain(Some(c));
                        for (row, board) in l.rows.iter().zip(&c.boards) {
                            let Some(best) = &board.ours_best else {
                                continue;
                            };
                            let tr = plate::track(*row);
                            let x = plate::score_x(best.score, domain, tr.x0, tr.x1);
                            let rgb = if board.heart { ROSE } else { CYAN };
                            let n = push(
                                &mut out,
                                6,
                                p.rect_at([x - 14.0, tr.y - 9.0, 28.0, 18.0]),
                                LayerKind::Glow([rgb[0], rgb[1], rgb[2], 0.7]),
                            );
                            out[n].additive = true;
                            out[n].flicker = 0.25;
                            out[n].opacity = 0.65 + 0.35 * beat;
                        }
                        if let Some(f) = &self.flare
                            && let Some(row) = l.rows.get(f.row)
                        {
                            let n = push(
                                &mut out,
                                8,
                                p.rect_at([row.x - 6.0, row.y - 1.0, row.w + 12.0, row.h]),
                                LayerKind::Solid([1.0, 0.35, 0.5, 0.18]),
                            );
                            out[n].opacity = flare;
                            out[n].additive = true;
                        }
                    }
                }
                Module::Legend => {
                    let (w, h) = self.clock_size[1];
                    if campaign.is_some() && w > 0.0 {
                        let g = l.legend;
                        push(
                            &mut out,
                            6,
                            p.rect_at([g.x + g.w - w, g.y + (g.h - h) / 2.0, w, h]),
                            LayerKind::Picture(RESET),
                        );
                    }
                }
                Module::Ticker => {
                    let (w, h) = self.clock_size[2];
                    if campaign.is_some() && w > 0.0 {
                        let g = l.ticker;
                        push(
                            &mut out,
                            6,
                            p.rect_at([g.x + g.w - w, g.y + 8.0, w, h]),
                            LayerKind::Picture(AGO),
                        );
                    }
                }
            }
            push(
                &mut out,
                5,
                [r.x, r.y, r.w, r.h],
                LayerKind::Picture(PRINTS + i as u32),
            );
        }

        // The hover chrome: a hairline round the module the pointer is on,
        // its corners, a tab with its name and hide button, and the grip.
        if let Some((module, grabbed)) = hover
            && let Some(p) = hud.get(module)
        {
            let o = p.outline();
            let a = if grabbed { 0.42 } else { 0.24 };
            let line = [0.62, 0.72, 0.84, a];
            for rect in [
                [o.x, o.y, o.w, 1.0],
                [o.x, o.y + o.h - 1.0, o.w, 1.0],
                [o.x, o.y, 1.0, o.h],
                [o.x + o.w - 1.0, o.y, 1.0, o.h],
            ] {
                push(&mut out, 10, rect, LayerKind::Solid(line));
            }
            let arm = 12.0f32.min(o.w / 3.0).min(o.h / 3.0);
            let tick = [0.78, 0.88, 0.98, (a * 2.2).min(0.85)];
            for (cx, cy, sx, sy) in [
                (o.x, o.y, 1.0, 1.0),
                (o.x + o.w, o.y, -1.0, 1.0),
                (o.x, o.y + o.h, 1.0, -1.0),
            ] {
                let across = [
                    if sx > 0.0 { cx } else { cx - arm },
                    if sy > 0.0 { cy } else { cy - 1.5 },
                    arm,
                    1.5,
                ];
                let down = [
                    if sx > 0.0 { cx } else { cx - 1.5 },
                    if sy > 0.0 { cy } else { cy - arm },
                    1.5,
                    arm,
                ];
                push(&mut out, 10, across, LayerKind::Solid(tick));
                push(&mut out, 10, down, LayerKind::Solid(tick));
            }
            // The tab: a dark strip, the name, the hide button.
            let tab = p.tab();
            push(
                &mut out,
                10,
                [tab.x, tab.y, tab.w, tab.h],
                LayerKind::Solid([0.012, 0.02, 0.035, 0.62]),
            );
            let (nw, nh) = self.float.names[module.index()];
            if nw > 0.0 {
                push(
                    &mut out,
                    11,
                    [tab.x + 6.0, tab.y + (tab.h - nh) / 2.0, nw, nh],
                    LayerKind::Picture(NAMES + module.index() as u32),
                );
            }
            let c = p.close();
            let (cx, cy) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
            for angle in [std::f32::consts::FRAC_PI_4, -std::f32::consts::FRAC_PI_4] {
                let n = push(
                    &mut out,
                    11,
                    [cx - 5.0, cy - 0.6, 10.0, 1.2],
                    LayerKind::Solid([0.80, 0.86, 0.94, 0.75]),
                );
                out[n].angle = angle.rem_euclid(std::f32::consts::TAU);
            }
            // The grip: two short strokes across the corner.
            let (gx, gy) = (o.x + o.w, o.y + o.h);
            for (len, back) in [(12.0f32, 4.0f32), (6.0, 1.5)] {
                let a = [gx - back - len * 0.707, gy - back];
                let b = [gx - back, gy - back - len * 0.707];
                let (rect, angle) = segment_rect(a, b, 1.4);
                let n = push(
                    &mut out,
                    11,
                    rect,
                    LayerKind::Solid([0.80, 0.88, 0.98, 0.8]),
                );
                out[n].angle = angle.rem_euclid(std::f32::consts::TAU);
            }
        }
        out
    }
}

/// Text smaller than this (picture pixels, at `scale` pixels a shown
/// pixel) is set in the window's own font: Share Tech Mono reads well
/// large and grows thin and faint when small over busy text.
fn small_below(scale: f32) -> f32 {
    (plate::MEDIUM - 0.5) * scale
}

/// Where a module's halo darkens beyond its print: the parts that move
/// over it (the trace, the clock, the stage title) as rects in its own
/// pixels with a strength, and for the heart, the oval it turns in.
pub(super) struct Shade {
    pub(super) extras: Vec<([f32; 4], f32)>,
    pub(super) oval: Option<([f32; 2], [f32; 2])>,
    pub(super) content: Panel,
}

fn shade_for(p: &Placed, l: &super::super::Layout, title: Option<(f32, f32)>) -> Shade {
    let c = p.local_content();
    let mut extras = Vec::new();
    let mut oval = None;
    match p.module {
        Module::Title => {
            let e = plate::ecg_strip(l.title);
            extras.push(([e.x, e.y - 2.0, e.w, e.h + 6.0], 0.55));
            let t = l.title;
            extras.push(([t.x + t.w - 118.0, t.y + 14.0, 118.0, 24.0], 0.9));
        }
        Module::Heart => {
            let v = l.heart;
            let (w, h) = title.unwrap_or((v.w * 0.5, 46.0));
            extras.push(([v.x + 8.0, v.y + 6.0, w + 4.0, h + 4.0], 0.85));
            let size = ((v.w * SCALE) as u32, (v.h * SCALE) as u32);
            let centre = heart::project([0.0, -0.25, 0.0], size)
                .map_or([v.x + v.w / 2.0, v.y + v.h / 2.0], |f| {
                    [v.x + v.w * f[0], v.y + v.h * f[1]]
                });
            oval = Some((centre, [v.h * 0.42, v.h * 0.58]));
        }
        Module::Legend => {
            let g = l.legend;
            extras.push(([g.x + g.w - 150.0, g.y, 150.0, g.h], 0.9));
        }
        Module::Ticker => {
            let g = l.ticker;
            extras.push(([g.x + g.w - 72.0, g.y + 6.0, 72.0, 18.0], 0.9));
        }
        Module::Stages | Module::Boards => {}
    }
    Shade {
        extras,
        oval,
        content: c,
    }
}

/// A module's halo at `size` (one pixel a shown pixel, at scale `k`):
/// the coverage of its print `ink`, spread a little and feathered wide,
/// over a faint wash of its content that fades downward, and for the
/// heart a soft oval; deep blue shade, never opaque.
pub(super) fn halo(ink: &Ink, size: (u32, u32), k: f32, shade: &Shade) -> Ink {
    let (w, h) = (size.0.max(1) as usize, size.1.max(1) as usize);
    let (iw, ih) = (ink.0 as usize, ink.1 as usize);
    let mut ink_mask = vec![0f32; w * h];
    if iw > 0 && ih > 0 && ink.2.len() == iw * ih * 4 {
        // The print's coverage, box-averaged down to shown pixels.
        let (rx, ry) = (iw as f32 / w as f32, ih as f32 / h as f32);
        for y in 0..h {
            let (y0, y1) = (
                (y as f32 * ry) as usize,
                (((y + 1) as f32 * ry).ceil() as usize).min(ih),
            );
            for x in 0..w {
                let (x0, x1) = (
                    (x as f32 * rx) as usize,
                    (((x + 1) as f32 * rx).ceil() as usize).min(iw),
                );
                let (mut sum, mut n) = (0u32, 0u32);
                for sy in y0..y1 {
                    for sx in x0..x1 {
                        sum += u32::from(ink.2[(sy * iw + sx) * 4 + 3]);
                        n += 1;
                    }
                }
                if n > 0 {
                    ink_mask[y * w + x] = sum as f32 / (255.0 * n as f32);
                }
            }
        }
    }
    let fill = |mask: &mut [f32], r: [f32; 4], v: f32| {
        let x0 = (r[0] * k).max(0.0) as usize;
        let y0 = (r[1] * k).max(0.0) as usize;
        let x1 = (((r[0] + r[2]) * k).ceil() as usize).min(w);
        let y1 = (((r[1] + r[3]) * k).ceil() as usize).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                let m = &mut mask[y * w + x];
                *m = m.max(v);
            }
        }
    };
    // Any mark at all counts in full: thin text has little coverage, and
    // its shade must not be thinner for it.
    for v in &mut ink_mask {
        *v = if *v > 0.12 { 1.0 } else { 0.0 };
    }
    // Tight: each mark swollen two pixels and softened, a band of shade
    // under each line of print. Soft: swollen further and feathered
    // wide, the bloom; the parts that move are in it.
    let px = |v: f32| (v * k).round().max(1.0) as usize;
    let mut tight = dilate(&ink_mask, w, h, px(2.0));
    blur(&mut tight, w, h, px(1.5), 2);
    for (r, v) in &shade.extras {
        fill(&mut ink_mask, *r, *v);
    }
    let mut soft = dilate(&ink_mask, w, h, px(4.0));
    blur(&mut soft, w, h, px(9.0), 3);

    // The wash over the content, feathered at its edges, fading down.
    let c = shade.content;
    let mut wash = vec![0f32; w * h];
    fill(&mut wash, [c.x, c.y, c.w, c.h], 1.0);
    blur(&mut wash, w, h, (9.0 * k).round().max(2.0) as usize, 2);

    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let down = ((y as f32 / k - c.y) / c.h.max(1.0)).clamp(0.0, 1.0);
        for x in 0..w {
            let i = y * w + x;
            let oval = shade.oval.map_or(0.0, |(o, r)| {
                let dx = (x as f32 / k - o[0]) / r[0].max(1.0);
                let dy = (y as f32 / k - o[1]) / r[1].max(1.0);
                let d = (dx * dx + dy * dy).min(1.0);
                (1.0 - d) * (1.0 - d)
            });
            let clear = (1.0 - HALO_TIGHT * tight[i])
                * (1.0 - HALO_SOFT * soft[i])
                * (1.0 - HALO_WASH * wash[i] * (1.0 - 0.25 * down))
                * (1.0 - HALO_HEART * oval);
            let a = (1.0 - clear).min(HALO_MAX);
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[
                SHADE[0],
                SHADE[1],
                SHADE[2],
                (a * 255.0).round() as u8,
            ]);
        }
    }
    (w as u32, h as u32, rgba)
}

/// The greatest of each `2r + 1` square: every mark swollen by `r`.
fn dilate(v: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut across = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            across[y * w + x] = v[y * w + x0..y * w + x1]
                .iter()
                .copied()
                .fold(0.0, f32::max);
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
        for x in 0..w {
            out[y * w + x] = (y0..y1).map(|yy| across[yy * w + x]).fold(0.0, f32::max);
        }
    }
    out
}

/// A box blur of radius `r`, `passes` times each way (three passes are
/// near enough a Gaussian). Outside the picture is clear.
fn blur(v: &mut [f32], w: usize, h: usize, r: usize, passes: usize) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut line = vec![0f32; w.max(h)];
    for _ in 0..passes {
        for y in 0..h {
            let row = &mut v[y * w..(y + 1) * w];
            line[..w].copy_from_slice(row);
            let mut sum: f32 = line[..r.min(w)].iter().sum();
            for x in 0..w {
                if x + r < w {
                    sum += line[x + r];
                }
                row[x] = sum * norm;
                if x >= r {
                    sum -= line[x - r];
                }
            }
        }
        for x in 0..w {
            for y in 0..h {
                line[y] = v[y * w + x];
            }
            let mut sum: f32 = line[..r.min(h)].iter().sum();
            for y in 0..h {
                if y + r < h {
                    sum += line[y + r];
                }
                v[y * w + x] = sum * norm;
                if y >= r {
                    sum -= line[y - r];
                }
            }
        }
    }
}

/// The heart's scanlines floating free: a faint dark line every third
/// shown pixel, strongest in the middle and gone toward the edges, so no
/// box shows over the text.
fn lens_lines((w, h): (u32, u32)) -> Ink {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        if (y as f32 / SCALE) as u32 % 3 != 2 {
            continue;
        }
        let fy = (y as f32 / h.max(1) as f32 - 0.5) * 2.0;
        for x in 0..w {
            let fx = (x as f32 / w.max(1) as f32 - 0.5) * 2.0;
            let d = (fx * fx * 0.8 + fy * fy).min(1.0);
            let a = 0.11 * (1.0 - d) * (1.0 - d);
            let o = ((y * w + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[0, 4, 12, (a * 255.0).round() as u8]);
        }
    }
    (w, h, rgba)
}

#[cfg(test)]
pub(crate) fn halo_for_test(ink: &Ink, size: (u32, u32), k: f32, content: Panel) -> Ink {
    halo(
        ink,
        size,
        k,
        &Shade {
            extras: Vec::new(),
            oval: None,
            content,
        },
    )
}
