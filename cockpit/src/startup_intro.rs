//! Once per launch: take up Excalibur by beginning a draft. All video sampling,
//! Dotmax dithering and fine-dot encoding happen on a single bounded worker, which
//! prepares the settled and late-clip canvases once and re-uses them for every
//! ambient tick. Only the dots are ever painted: the transported fine-dot frame
//! carries a transparent background and the Braille fallback skips empty cells,
//! so the shell and the mini-viz keep the pane behind the animation.

use crate::dot_canvas::{DotGeometry, TRANSPARENT};
use crate::dot_protocol::DotProtocol;
use crate::lifecycle_viz::MotionMode;
use crate::terminal_art::{ColoredBrailleCell, ColoredBrailleImage, braille_dot_bit};
use image::{GrayImage, imageops};
use ratatui::{
    Frame,
    buffer::CellDiffOption,
    layout::{Rect, Size},
    style::Color,
};
use std::hash::{BuildHasher, RandomState};
use std::num::NonZeroU16;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

const ATLAS: &[u8] = include_bytes!("../assets/excalibur/rise.png");
const FRAME_W: u32 = 680;
const FRAME_H: u32 = 384;
const FRAMES: u8 = 60;
const FPS: u128 = 12;
/// The blade settles here; afterwards only the water keeps moving.
const HOLD: u8 = 49;
/// The plot reaches its summit as the blade locks; then the wizard walks in
/// from the right and settles on the summit shelf.
const WIZARD_WALK_TICKS: u8 = 24;
const WIZARD_SETTLE: u8 = HOLD + WIZARD_WALK_TICKS;
const WIZARD_IDLE_FRAMES: u8 = 8;
/// Late-clip frames whose water band still shimmers. Cycled forever under the
/// settled sword, so the rise plays once and the flow becomes ambient; the hold
/// itself supplies frame 0 of the cycle.
const RIPPLE: &[u8] = &[50, 51, 52, 53, 54, 55, 56, 57, 58, 59];
/// Ticks spent dissolving the last late-clip frame back into the first. A hard wrap
/// steps 6.3 levels in the water band, three times a neighbouring frame's 2.1, which
/// reads as a lurch once a cycle; spread over four ticks the worst step in the cycle
/// drops back under the natural one and the flow keeps its direction.
const SEAM: u8 = 4;
const FADE: Duration = Duration::from_millis(600);
/// The empty Braille cell: a canvas cell with no dots is never painted, so it
/// leaves the pane's own pixels rather than blanking them.
const NO_DOTS: char = '\u{2800}';

/// One tick of the ambient loop: which late-clip frame the water shows, and how much
/// of the next frame has been dissolved into it (0 = none). The dissolve closes the
/// cycle, so it is part of the tick's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Flow {
    frame: u8,
    next: u8,
    melt: u8,
}

impl Flow {
    /// The cycle in order: every late-clip frame at 12 fps, then `SEAM` ticks
    /// dissolving the last one back into the first.
    fn at(tick: u128) -> Self {
        let frames = RIPPLE.len() as u128;
        let step = tick % (frames + u128::from(SEAM));
        if step < frames {
            let frame = RIPPLE[step as usize];
            Self {
                frame,
                next: frame,
                melt: 0,
            }
        } else {
            Self {
                frame: RIPPLE[RIPPLE.len() - 1],
                next: RIPPLE[0],
                // Never reaches 255: the next tick is the first frame outright.
                melt: ((step - frames + 1) * 255 / (u128::from(SEAM) + 1)) as u8,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scene {
    Sword,
    Mountain { tick: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    scene: Scene,
    frame: u8,
    /// Ambient water under the settled blade; `None` while the blade is still rising
    /// and wherever the flow is suppressed.
    flow: Option<Flow>,
    /// 0 = water as filmed; 255 = water dots fully faded / replaced by flow.
    water: u8,
    opacity: u8,
    size: Size,
    geometry: Option<DotGeometry>,
}

struct Prepared {
    dots: ColoredBrailleImage,
    protocol: Option<DotProtocol>,
}

struct Worker {
    tx: SyncSender<(usize, Key, u32)>,
    rx: Receiver<(usize, Key, Result<Prepared, String>)>,
}

#[derive(Default)]
struct Surface {
    pending: bool,
    current: Option<(Key, Prepared)>,
    next_id: u32,
}

#[derive(Default)]
pub(crate) struct StartupIntro {
    started: Option<Instant>,
    fading: Option<Instant>,
    finished: bool,
    visible: bool,
    worker: Option<Worker>,
    surfaces: [Surface; 2],
}

impl StartupIntro {
    /// Called even when the transcript is hidden, so restoring/clearing a
    /// conversation cannot bring the sword back, and hidden art cannot tick.
    pub(crate) fn begin_frame(&mut self, occupied: bool, draft: bool, motion: MotionMode) {
        self.visible = false;
        if occupied {
            self.cancel();
        } else if draft {
            self.dismiss(Instant::now(), motion);
        }
    }

    pub(crate) fn dismiss(&mut self, now: Instant, motion: MotionMode) {
        if self.finished || self.fading.is_some() {
            return;
        }
        if self.started.is_none() || motion == MotionMode::Off {
            self.cancel();
        } else {
            self.fading = Some(now);
        }
    }

    fn cancel(&mut self) {
        self.finished = true;
        self.visible = false;
        self.worker = None;
        self.surfaces = Default::default();
    }

    fn elapsed_tick(&self, now: Instant) -> Option<u128> {
        let started = self.started?;
        Some(
            self.fading
                .unwrap_or(now)
                .saturating_duration_since(started)
                .as_millis()
                * FPS
                / 1000,
        )
    }

    fn landscape_tick(&self, now: Instant, motion: MotionMode) -> Option<u8> {
        Some(match motion {
            MotionMode::Full => {
                let tick = self.elapsed_tick(now)?;
                if tick < u128::from(WIZARD_SETTLE) {
                    tick as u8
                } else {
                    WIZARD_SETTLE
                        + (((tick - u128::from(WIZARD_SETTLE)) / 3)
                            % u128::from(WIZARD_IDLE_FRAMES)) as u8
                }
            }
            MotionMode::Reduced | MotionMode::Off => WIZARD_SETTLE,
        })
    }

    fn sample(&self, now: Instant, motion: MotionMode) -> Option<(u8, Option<Flow>, u8, u8)> {
        if self.finished {
            return None;
        }
        let tick = self.elapsed_tick(now)?;
        // Water handoff starts once the blade settles: the flow fades in over
        // the settled frame's water, and under reduced motion the water dots
        // fade to black instead (a static image keeps no shimmering dots).
        const WATER_FADE_TICKS: u128 = 16; // ~1.3 s at 12 fps
        let (frame, flow) = if motion == MotionMode::Full {
            if tick < u128::from(HOLD) {
                (tick as u8, None)
            } else {
                // The sword holds still; the water band cycles the late-clip
                // shimmer forever, so the flow stays ambient under it.
                (HOLD, Some(Flow::at(tick - u128::from(HOLD))))
            }
        } else {
            (FRAMES - 1, None)
        };
        let water = if tick < u128::from(HOLD) {
            0
        } else if motion == MotionMode::Off {
            255
        } else {
            let t = (tick - u128::from(HOLD)).min(WATER_FADE_TICKS) * 255 / WATER_FADE_TICKS;
            // Smoothstep: no visible snap where the ramp meets the rise.
            ((t as f32 / 255.0).powi(2) * (3.0 - 2.0 * (t as f32 / 255.0)) * 255.0).round() as u8
        };
        let opacity = if let Some(fading) = self.fading {
            let duration = match motion {
                MotionMode::Full => FADE,
                MotionMode::Reduced => Duration::from_millis(200),
                MotionMode::Off => return None,
            };
            let t = now.saturating_duration_since(fading).as_secs_f32() / duration.as_secs_f32();
            if t >= 1.0 {
                return None;
            }
            // Smoothstep fades brightness without moving the dithering pattern.
            ((1.0 - t * t * (3.0 - 2.0 * t)) * 255.0).round() as u8
        } else {
            255
        };
        Some((frame, flow, water, opacity))
    }

    pub(crate) fn animating(&self, _now: Instant, motion: MotionMode) -> bool {
        self.visible
            && !self.finished
            && (self.surfaces.iter().any(|surface| surface.pending)
                || self.fading.is_some()
                // Full motion keeps the ambient water flowing while visible.
                || (motion == MotionMode::Full && self.started.is_some()))
    }

    pub(crate) fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        geometry: Option<DotGeometry>,
        motion: MotionMode,
    ) {
        self.render_surface(0, frame, area, geometry, motion);
    }

    pub(crate) fn render_miniviz(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        geometry: Option<DotGeometry>,
        motion: MotionMode,
    ) -> bool {
        self.render_surface(1, frame, area, geometry, motion);
        !self.finished
    }

    fn render_surface(
        &mut self,
        surface: usize,
        frame: &mut Frame,
        area: Rect,
        geometry: Option<DotGeometry>,
        motion: MotionMode,
    ) {
        if self.finished || area.width == 0 || area.height == 0 {
            return;
        }
        let now = Instant::now();
        self.started.get_or_insert(now);
        let Some((index, flow, water, opacity)) = self.sample(now, motion) else {
            self.cancel();
            return;
        };
        // If typing beat the first prepared frame, never flash late artwork.
        if self.fading.is_some()
            && self
                .surfaces
                .iter()
                .all(|surface| surface.current.is_none())
        {
            self.cancel();
            return;
        }
        self.visible = true;
        let key = Key {
            scene: if surface == 0 {
                Scene::Sword
            } else {
                Scene::Mountain {
                    tick: self.landscape_tick(now, motion).unwrap_or(WIZARD_SETTLE),
                }
            },
            // Mountain frames normalize sword-only state; the sprite has its own
            // slower four-frame-per-second idle cadence.
            frame: if surface == 0 { index } else { 0 },
            flow: (surface == 0).then_some(flow).flatten(),
            water: if surface == 0 { water } else { 0 },
            opacity,
            size: area.as_size(),
            geometry,
        };
        if self.worker.is_none() {
            // World owns ..10/..11; shell owns ..000/..001 and mini-viz
            // ..100/..101. All three surfaces retain separate frame pairs.
            let base =
                ((RandomState::new().hash_one(std::process::id()) as u32) & 0x00ff_fff8).max(8);
            self.surfaces[0].next_id = base;
            self.surfaces[1].next_id = base + 4;
            self.worker = spawn_worker();
            if self.worker.is_none() {
                self.cancel();
                return;
            }
        }
        loop {
            let (slot, ready_key, result) = match self.worker.as_ref().unwrap().rx.try_recv() {
                Ok(ready) => ready,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.cancel();
                    return;
                }
            };
            self.surfaces[slot].pending = false;
            match result {
                Ok(ready) => self.surfaces[slot].current = Some((ready_key, ready)),
                Err(_) => {
                    self.cancel();
                    return;
                }
            }
        }
        let surface = &mut self.surfaces[surface];
        if surface
            .current
            .as_ref()
            .is_some_and(|(k, _)| k.size != key.size || k.geometry != key.geometry)
        {
            surface.current = None;
        }
        if !surface.pending && surface.current.as_ref().is_none_or(|(k, _)| *k != key) {
            let worker = self.worker.as_ref().unwrap();
            let slot = usize::from(surface.next_id & 4 != 0);
            if worker.tx.try_send((slot, key, surface.next_id)).is_err() {
                self.cancel();
                return;
            }
            surface.next_id ^= 1;
            surface.pending = true;
        }
        // Only the dots themselves are drawn, and nothing clears the area first:
        // the glyph grid stays see-through, exactly like the transparent
        // background of the transported fine-dot frame.
        if let Some((_, ready)) = &surface.current {
            if let Some(protocol) = &ready.protocol {
                protocol.render(area, frame.buffer_mut());
            } else {
                for (i, dot) in ready.dots.cells.iter().enumerate() {
                    if dot.glyph == NO_DOTS {
                        continue;
                    }
                    let x = i % ready.dots.width;
                    let y = i / ready.dots.width;
                    if let Some(cell) = frame
                        .buffer_mut()
                        .cell_mut((area.x + x as u16, area.y + y as u16))
                    {
                        cell.set_char(dot.glyph)
                            .set_fg(Color::Rgb(dot.fg[0], dot.fg[1], dot.fg[2]));
                    }
                }
            }
        }
    }

    /// After overlays, just like world dots; no raw source image is uploaded.
    pub(crate) fn flush_upload(&mut self, frame: &mut Frame) {
        for surface in &mut self.surfaces {
            let upload = surface
                .current
                .as_mut()
                .and_then(|(_, prepared)| prepared.protocol.as_mut())
                .and_then(DotProtocol::take_upload);
            let Some(upload) = upload else { continue };
            let area = frame.area();
            if let Some(cell) = frame.buffer_mut().cell_mut((area.x, area.y)) {
                cell.set_symbol(&format!("{upload}{}", cell.symbol()))
                    .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::new(1).unwrap()));
            }
        }
    }
}

/// Read the shipped atlas into the level image the canvas pipeline works in.
///
/// `rise.png` is a white-ink alpha mask: the ink plane is uniformly white and the
/// alpha plane carries the frame luminance, so the saved source shows the same
/// white-on-transparent animation the terminal does. Alpha is the level; a plain
/// luminance atlas still decodes by luminance.
fn decode_atlas(bytes: &[u8]) -> Result<GrayImage, String> {
    let decoded = image::load_from_memory(bytes).map_err(|error| error.to_string())?;
    if !decoded.color().has_alpha() {
        return Ok(decoded.to_luma8());
    }
    let mask = match decoded {
        image::DynamicImage::ImageLumaA8(mask) => mask,
        other => other.to_luma_alpha8(),
    };
    let (width, height) = mask.dimensions();
    GrayImage::from_raw(
        width,
        height,
        mask.into_raw()
            .chunks_exact(2)
            .map(|pixel| pixel[1])
            .collect(),
    )
    .ok_or_else(|| "invalid Excalibur atlas dimensions".into())
}

fn spawn_worker() -> Option<Worker> {
    let (tx, jobs) = mpsc::sync_channel::<(usize, Key, u32)>(2);
    let (results, rx) = mpsc::sync_channel(2);
    std::thread::Builder::new()
        .name("excalibur-dots".into())
        .spawn(move || {
            let atlas = decode_atlas(ATLAS);
            // Ambient ticks re-read the prepared canvases, so the flow costs a copy
            // and a band blend instead of two Lanczos resamples of the atlas.
            let mut cache = Cache::default();
            while let Ok((slot, key, id)) = jobs.recv() {
                let prepared = atlas.as_ref().map_err(Clone::clone).and_then(|atlas| {
                    let (w, h) = key.geometry.map_or(
                        (usize::from(key.size.width), usize::from(key.size.height)),
                        |g| (g.grid_width, g.grid_height),
                    );
                    let dots = compose(atlas, &mut cache, &key, w, h)?;
                    // The transported frame is the animation: dots on a
                    // transparent background, never an opaque plate around them.
                    let protocol = key.geometry.and_then(|g| {
                        DotProtocol::encode_on(g, &dots, key.size, id, Some(TRANSPARENT)).ok()
                    });
                    // An unsupported/oversized fine transport falls back to actual
                    // terminal-cell dimensions, never a cropped fine-dot grid.
                    let dots = if key.geometry.is_some() && protocol.is_none() {
                        compose(
                            atlas,
                            &mut cache,
                            &key,
                            key.size.width.into(),
                            key.size.height.into(),
                        )?
                    } else {
                        dots
                    };
                    Ok(Prepared { dots, protocol })
                });
                if results.send((slot, key, prepared)).is_err() {
                    break;
                }
            }
        })
        .ok()?;
    Some(Worker { tx, rx })
}

/// The ambient canvases for one dot geometry, prepared once and re-used. Two slots,
/// because a terminal that rejects fine dots composes the same tick twice: once at
/// grid and once at terminal-cell dimensions.
#[derive(Default)]
struct Cache {
    slots: Vec<Slot>,
}

struct Slot {
    columns: usize,
    rows: usize,
    /// The settled frame (`HOLD`), cropped, scaled and levelled: the canvas the flow
    /// blends into.
    settled: GrayImage,
    /// The late-clip frames, in `RIPPLE` order, prepared the same way.
    ripple: Vec<GrayImage>,
    /// Working canvas: the settled frame is copied here each tick and the band blend
    /// runs in place, so an ambient tick allocates nothing.
    scratch: GrayImage,
}

impl Slot {
    fn new(atlas: &GrayImage, columns: usize, rows: usize) -> Self {
        Self {
            columns,
            rows,
            settled: prepare(atlas, HOLD, columns, rows),
            ripple: RIPPLE
                .iter()
                .map(|&index| prepare(atlas, index, columns, rows))
                .collect(),
            scratch: GrayImage::new(columns as u32 * 2, rows as u32 * 4),
        }
    }
}

impl Cache {
    fn slot(&mut self, atlas: &GrayImage, columns: usize, rows: usize) -> &mut Slot {
        if let Some(index) = self
            .slots
            .iter()
            .position(|slot| slot.columns == columns && slot.rows == rows)
        {
            return &mut self.slots[index];
        }
        if self.slots.len() >= 2 {
            self.slots.remove(0);
        }
        self.slots.push(Slot::new(atlas, columns, rows));
        self.slots.last_mut().expect("just pushed")
    }
}

/// Crop the clip's fixed side wings, fit the whole blade, hand and waterline into the
/// dot canvas, and lift steel midtones. The level curve runs once per prepared frame
/// rather than once per tick, which is what lets the ambient loop blend cached
/// canvases instead of resampling the atlas.
fn prepare(atlas: &GrayImage, index: u8, columns: usize, rows: usize) -> GrayImage {
    // Fixed framing throughout the clip: trim empty side wings, retain the whole
    // blade, hand and water. Narrow mini-viz can then use its height.
    const CROP_X: u32 = 140;
    const CROP_W: u32 = FRAME_W - CROP_X * 2;
    let source = imageops::crop_imm(
        atlas,
        u32::from(index % 10) * FRAME_W + CROP_X,
        u32::from(index / 10) * FRAME_H,
        CROP_W,
        FRAME_H,
    )
    .to_image();
    let width = columns as u32 * 2;
    let height = rows as u32 * 4;
    let scale = (width as f32 / CROP_W as f32).min(height as f32 / FRAME_H as f32);
    let fitted = imageops::resize(
        &source,
        (CROP_W as f32 * scale).round().max(1.0) as u32,
        (FRAME_H as f32 * scale).round().max(1.0) as u32,
        imageops::FilterType::Lanczos3,
    );
    let mut canvas = GrayImage::new(width, height);
    imageops::overlay(
        &mut canvas,
        &fitted,
        i64::from((width - fitted.width()) / 2),
        i64::from((height - fitted.height()) / 2),
    );
    // A fixed toe removes compression haze; lift steel midtones before spatial
    // dithering. Never normalize per frame (which would pump/flicker).
    let curve: [u8; 256] = std::array::from_fn(|value| {
        let linear = ((value as f32 - 10.0) / 220.0).clamp(0.0, 1.0);
        (linear.powf(0.68) * 255.0).round() as u8
    });
    for value in canvas.as_mut() {
        *value = curve[usize::from(*value)];
    }
    canvas
}

fn compose(
    atlas: &GrayImage,
    cache: &mut Cache,
    key: &Key,
    columns: usize,
    rows: usize,
) -> Result<ColoredBrailleImage, String> {
    if columns == 0 || rows == 0 || columns > 1024 || rows > 512 {
        return Err("invalid Excalibur atlas or dot geometry".into());
    }
    if let Scene::Mountain { tick } = key.scene {
        if tick >= WIZARD_SETTLE + WIZARD_IDLE_FRAMES {
            return Err("invalid mountain intro tick".into());
        }
        return compose_mountain(tick, columns, rows, key.opacity);
    }
    if key.frame >= FRAMES
        || key
            .flow
            .is_some_and(|flow| !RIPPLE.contains(&flow.frame) || !RIPPLE.contains(&flow.next))
        || atlas.dimensions() != (FRAME_W * 10, FRAME_H * 6)
    {
        return Err("invalid Excalibur atlas or dot geometry".into());
    }
    let Slot {
        settled,
        ripple,
        scratch,
        ..
    } = cache.slot(atlas, columns, rows);
    // The flow only ever plays under the settled blade, so that is the frame the
    // cache holds. The rise, and the still frame where motion is suppressed, is
    // prepared on the spot: once per launch, not once per ambient tick.
    match key.flow.filter(|_| key.frame == HOLD) {
        Some(flow) => {
            scratch.as_mut().copy_from_slice(settled.as_raw());
            flow_water(scratch, ripple, flow, key.water);
        }
        None => {
            *scratch = prepare(atlas, key.frame, columns, rows);
            if key.water > 0 {
                // With the flow suppressed the water dissolves to black rather than
                // sitting frozen on the settled frame.
                damp_water(scratch, key.water);
            }
        }
    }
    render_cells(scratch, columns, rows, key.opacity)
}

/// The water band the flow may touch, and how it feathers in from the settled steel
/// above the waterline.
fn water_band(height: u32) -> (u32, u32) {
    (height * 2 / 3, (height / 12).max(1))
}

fn band_weight(y: u32, band_top: u32, feather: u32) -> u32 {
    let t = ((y - band_top) as f32 / feather as f32).min(1.0);
    (t * t * (3.0 - 2.0 * t) * 255.0).round() as u32
}

/// 255 where the settled frame shows free water, 0 where it shows steel: the loop's
/// permission slip, in the same levelled scale the dithering uses.
fn openness(level: u32) -> u32 {
    const WATER: u32 = 64; // at or below: free water
    const STEEL: u32 = 128; // at or above: blade, hand, hilt — never handed to the loop
    if level >= STEEL {
        0
    } else if level <= WATER {
        255
    } else {
        (STEEL - level) * 255 / (STEEL - WATER)
    }
}

/// Ambient water. Only the pixels the settled frame already shows as water accept the
/// loop: whatever it carries above the water tone — the blade, the hand, the hilt and
/// the solid core of their reflection — is left exactly as it settled, so the flow can
/// never jitter the hand or the hilt. Dark water is crossfaded into the flow, which
/// keeps both the waterline (the band's feather) and the object edges (the openness
/// ramp) continuous.
fn flow_water(canvas: &mut GrayImage, ripple: &[GrayImage], flow: Flow, water: u8) {
    let (band_top, feather) = water_band(canvas.height());
    let first = &ripple[(flow.frame - RIPPLE[0]) as usize];
    let second = &ripple[(flow.next - RIPPLE[0]) as usize];
    let melt = u32::from(flow.melt);
    for y in band_top..canvas.height() {
        let weight = band_weight(y, band_top, feather);
        for x in 0..canvas.width() {
            let pixel = canvas.get_pixel_mut(x, y);
            let base = u32::from(pixel[0]);
            let open = openness(base);
            if open == 0 {
                continue;
            }
            let blend = u32::from(water) * weight * open / (255 * 255);
            if blend == 0 {
                continue;
            }
            // The seam dissolves one late-clip frame into the next; every other tick
            // reads a single frame outright (melt 0).
            let a = u32::from(first.get_pixel(x, y)[0]);
            let b = u32::from(second.get_pixel(x, y)[0]);
            let shimmer = (a * (255 - melt) + b * melt) / 255;
            pixel[0] = ((base * (255 - blend) + shimmer * blend) / 255).min(255) as u8;
        }
    }
}

/// Reduced motion: dissolve the settled water's own speckle toward black as the
/// handoff completes. Scaled by darkness and by the openness ramp, so bright steel
/// keeps its level and the waterline edge never shows a hard cut.
fn damp_water(canvas: &mut GrayImage, water: u8) {
    let (band_top, feather) = water_band(canvas.height());
    for y in band_top..canvas.height() {
        let weight = band_weight(y, band_top, feather);
        for x in 0..canvas.width() {
            let pixel = canvas.get_pixel_mut(x, y);
            let base = u32::from(pixel[0]);
            let damp =
                u32::from(water) * weight * openness(base) * (255 - base) / (255 * 255 * 255);
            pixel[0] = (base - base * damp / 255).min(255) as u8;
        }
    }
}

/// Dither a prepared canvas into braille cells with plain white ink: one SGR for
/// the whole frame on the fallback path, one uniform color on the dot-protocol
/// path, and no background underneath, so the terminal shows only the dots. The
/// fade is carried by the Bayer threshold (fewer dots, same white) rather than
/// dimmed grey ink, and black negative space stays empty either way.
fn render_cells(
    canvas: &GrayImage,
    columns: usize,
    rows: usize,
    opacity: u8,
) -> Result<ColoredBrailleImage, String> {
    let binary = dotmax::image::dither::bayer(canvas).map_err(|e| e.to_string())?;
    let mut cells = vec![
        ColoredBrailleCell {
            glyph: NO_DOTS,
            fg: [0; 3]
        };
        columns * rows
    ];
    // The opacity fade gates which dots exist rather than dimming ink: full
    // opacity lights every dithered dot, lower opacity drops dimmer dots until the
    // frame empties out entirely. Ink stays pure white throughout.
    let floor = u32::from(255u8.saturating_sub(opacity));
    for y in 0..canvas.height() as usize {
        for x in 0..canvas.width() as usize {
            // Dotmax's threshold bias can light the zero Bayer entry even on
            // black. Keep the source's black negative space truly empty.
            let level = canvas.get_pixel(x as u32, y as u32)[0];
            if u32::from(level) > floor && binary.get_pixel(x as u32, y as u32) == Some(true) {
                let cell = &mut cells[(y / 4) * columns + x / 2];
                cell.glyph =
                    char::from_u32(cell.glyph as u32 | u32::from(braille_dot_bit(x % 2, y % 4)))
                        .unwrap();
                cell.fg = [255; 3];
            }
        }
    }
    Ok(ColoredBrailleImage {
        width: columns,
        height: rows,
        cells,
    })
}

// A single rising contour: no mesh, hidden edges, axes or terrain underneath.
// Coordinates are pane-relative so the low base and summit headroom survive resize.
const INTRO_INK: [u8; 3] = [255; 3];
const WIZARD_INK: [u8; 3] = INTRO_INK;
const ASCENT: [(f32, f32); 9] = [
    (0.06, 0.89),
    (0.19, 0.86),
    (0.30, 0.76),
    (0.40, 0.74),
    (0.51, 0.63),
    (0.60, 0.60),
    (0.70, 0.48),
    (0.79, 0.43),
    (0.95, 0.43),
];

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn plot_point(point: (f32, f32), dot_w: usize, dot_h: usize) -> (i32, i32) {
    (
        (point.0 * dot_w.saturating_sub(1) as f32).round() as i32,
        (point.1 * dot_h.saturating_sub(1) as f32).round() as i32,
    )
}

fn paint_dot(image: &mut ColoredBrailleImage, x: i32, y: i32, color: [u8; 3], opacity: u8) {
    let dot_w = image.width.saturating_mul(2) as i32;
    let dot_h = image.height.saturating_mul(4) as i32;
    if opacity == 0 || x < 0 || y < 0 || x >= dot_w || y >= dot_h {
        return;
    }
    if opacity < 255 {
        let gate = (x as u32)
            .wrapping_mul(37)
            .wrapping_add((y as u32).wrapping_mul(73))
            .wrapping_add((x as u32 ^ y as u32).wrapping_mul(17)) as u8;
        if gate >= opacity {
            return;
        }
    }
    let cell_x = x as usize / 2;
    let cell_y = y as usize / 4;
    let cell = &mut image.cells[cell_y * image.width + cell_x];
    cell.glyph = char::from_u32(
        cell.glyph as u32 | u32::from(braille_dot_bit(x as usize % 2, y as usize % 4)),
    )
    .unwrap();
    let old_energy = cell.fg.iter().map(|&v| u16::from(v)).sum::<u16>();
    let new_energy = color.iter().map(|&v| u16::from(v)).sum::<u16>();
    if new_energy >= old_energy {
        cell.fg = color;
    }
}

fn clip_dot_line(
    image: &ColoredBrailleImage,
    from: (i32, i32),
    to: (i32, i32),
) -> Option<((i32, i32), (i32, i32))> {
    let max_x = image.width.checked_mul(2)?.checked_sub(1)? as f32;
    let max_y = image.height.checked_mul(4)?.checked_sub(1)? as f32;
    let x0 = from.0 as f32;
    let y0 = from.1 as f32;
    let dx = (to.0 - from.0) as f32;
    let dy = (to.1 - from.1) as f32;
    let mut enter = 0.0f32;
    let mut leave = 1.0f32;

    // Liang-Barsky clipping keeps Bresenham bounded to the actual dot canvas.
    // This matters while the camera reveal places most of the world offscreen.
    for (p, q) in [(-dx, x0), (dx, max_x - x0), (-dy, y0), (dy, max_y - y0)] {
        if p.abs() < f32::EPSILON {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let crossing = q / p;
        if p < 0.0 {
            enter = enter.max(crossing);
        } else {
            leave = leave.min(crossing);
        }
        if enter > leave {
            return None;
        }
    }

    let point = |t: f32| {
        (
            (x0 + t * dx).round().clamp(0.0, max_x) as i32,
            (y0 + t * dy).round().clamp(0.0, max_y) as i32,
        )
    };
    Some((point(enter), point(leave)))
}

fn draw_dot_line(
    image: &mut ColoredBrailleImage,
    from: (i32, i32),
    to: (i32, i32),
    color: [u8; 3],
    opacity: u8,
) {
    let Some((from, to)) = clip_dot_line(image, from, to) else {
        return;
    };
    let (mut x, mut y) = from;
    let (to_x, to_y) = to;
    let dx = (to_x - x).abs();
    let sx = if x < to_x { 1 } else { -1 };
    let dy = -(to_y - y).abs();
    let sy = if y < to_y { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        paint_dot(image, x, y, color, opacity);
        if x == to_x && y == to_y {
            break;
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn draw_wizard(
    image: &mut ColoredBrailleImage,
    centre_x: i32,
    ground_y: i32,
    walk_pose: u8,
    settled: bool,
    opacity: u8,
) {
    // Continuous contours sample directly onto Dotmax's dot grid: the hat and
    // cloak stay fine-edged instead of enlarging a chunky bitmap in integer steps.
    let height = (image.height as f32 * 4.0 * 0.23)
        .min(image.width as f32 * 2.0 * 0.28)
        .max(12.0)
        * 0.5;
    let scale = height / 100.0;
    let point = |x: f32, y: f32| {
        (
            centre_x + (x * scale).round() as i32,
            ground_y + (y * scale).round() as i32,
        )
    };
    // Left-facing profile: hooked hat, wide brim, brow/nose, long tapered beard,
    // a bent sleeve, and a wind-caught travelling cloak with an uneven hem.
    const OUTLINE: &[(f32, f32)] = &[
        (-19.0, -96.0),
        (-7.0, -100.0),
        (1.0, -96.0),
        (7.0, -84.0),
        (10.0, -77.0),
        (21.0, -72.0),
        (8.0, -70.0),
        (9.0, -63.0),
        (15.0, -56.0),
        (17.0, -42.0),
        (22.0, -24.0),
        (29.0, -8.0),
        (36.0, -3.0),
        (21.0, -5.0),
        (13.0, -2.0),
        (3.0, -4.0),
        (-9.0, -2.0),
        (-16.0, -5.0),
        (-11.0, -22.0),
        (-7.0, -39.0),
        (-15.0, -44.0),
        (-22.0, -42.0),
        (-28.0, -47.0),
        (-28.0, -52.0),
        (-22.0, -50.0),
        (-14.0, -55.0),
        (-10.0, -60.0),
        (-15.0, -54.0),
        (-18.0, -48.0),
        (-18.0, -62.0),
        (-21.0, -64.0),
        (-16.0, -68.0),
        (-16.0, -71.0),
        (-29.0, -72.0),
        (-24.0, -76.0),
        (-12.0, -79.0),
        (-9.0, -88.0),
        (-9.0, -94.0),
    ];
    let wind = [0.0, 0.7, 1.0, 0.7, 0.0, -0.7, -1.0, -0.7][walk_pose as usize % 8];
    // Negative space separates beard from shoulder and opens a long robe fold.
    const BEARD_GAP: &[(f32, f32)] = &[(-9.0, -66.0), (-5.0, -62.0), (-7.0, -53.0), (-15.0, -44.0)];
    const BRIM_SHADOW: &[(f32, f32)] =
        &[(-14.0, -71.0), (6.0, -71.0), (1.0, -67.0), (-10.0, -68.0)];
    const CLOAK_SHADOW: &[(f32, f32)] = &[
        (11.0, -53.0),
        (13.0, -33.0),
        (24.0, -10.0),
        (15.0, -15.0),
        (8.0, -32.0),
    ];
    const CLOAK_FOLD: &[(f32, f32)] = &[(4.0, -44.0), (3.0, -24.0), (10.0, -7.0), (6.0, -25.0)];
    let inside = |x: f32, y: f32, polygon: &[(f32, f32)]| {
        let mut hit = false;
        let mut previous = polygon[polygon.len() - 1];
        for &next in polygon {
            if (next.1 > y) != (previous.1 > y)
                && x < (previous.0 - next.0) * (y - next.1) / (previous.1 - next.1) + next.0
            {
                hit = !hit;
            }
            previous = next;
        }
        hit
    };
    let (left, top) = point(-30.0, -101.0);
    let (right, bottom) = point(37.0, 0.0);
    for y in top.max(0)..=bottom.min(image.height as i32 * 4 - 1) {
        for x in left.max(0)..=right.min(image.width as i32 * 2 - 1) {
            let local_y = (y - ground_y) as f32 / scale;
            // Warp the silhouette and its shadow cuts together. The planted
            // feet/staff stay still while cloth and the hat tip catch the wind.
            let cloth = ((local_y + 48.0) / 48.0).clamp(0.0, 1.0);
            let hat = ((-local_y - 80.0) / 20.0).clamp(0.0, 1.0);
            let local_x = (x - centre_x) as f32 / scale - wind * (cloth * 5.0 + hat * 4.0);
            if inside(local_x, local_y, OUTLINE)
                && !inside(local_x, local_y, BEARD_GAP)
                && !inside(local_x, local_y, CLOAK_FOLD)
                && !inside(local_x, local_y, BRIM_SHADOW)
                && !inside(local_x, local_y, CLOAK_SHADOW)
            {
                // Ordered stippling models cloth turning away from the light.
                // Anchor the pattern to the sprite, never the frame clock, so
                // walking does not turn the shading into random sparkling.
                const BAYER: [[u8; 2]; 2] = [[0, 2], [3, 1]];
                let cloth_or_hat = !(-79.0..=-58.0).contains(&local_y);
                let shade = if local_x > 5.0 { 2 } else { 3 };
                let stipple = BAYER[(y - ground_y).rem_euclid(2) as usize]
                    [(x - centre_x).rem_euclid(2) as usize];
                // Keep the light-facing contour, facial profile and narrow tip
                // intact: a half-size sprite has very few dots to describe them.
                let lit_edge = !inside(local_x - 1.0 / scale, local_y, OUTLINE);
                if !cloth_or_hat || lit_edge || local_y < -92.0 || stipple < shade {
                    paint_dot(image, x, y, WIZARD_INK, opacity);
                }
            }
        }
    }
    // Open crook, separated from the face, reads as an ancient wooden staff.
    let staff = [
        (-30.0, 0.0),
        (-29.0, -36.0),
        (-31.0, -65.0),
        (-35.0, -74.0),
        (-33.0, -80.0),
        (-27.0, -82.0),
        (-23.0, -78.0),
        (-25.0, -74.0),
    ];
    for segment in staff.windows(2) {
        draw_dot_line(
            image,
            point(segment[0].0, segment[0].1),
            point(segment[1].0, segment[1].1),
            WIZARD_INK,
            opacity,
        );
    }
    let stride = if settled {
        0.0
    } else {
        [-3.0, 0.0, 3.0, 0.0][walk_pose as usize % 4]
    };
    for (ankle, toe) in [(-7.0, -13.0 - stride), (10.0, 15.0 + stride)] {
        draw_dot_line(
            image,
            point(ankle, -5.0),
            point(toe, 0.0),
            WIZARD_INK,
            opacity,
        );
    }
}

fn wizard_x_at(tick: u8) -> Option<f32> {
    if tick < HOLD {
        return None;
    }
    let walk = smoothstep(f32::from(tick - HOLD) / f32::from(WIZARD_WALK_TICKS));
    Some(1.24 + (0.84 - 1.24) * walk)
}

fn compose_mountain(
    tick: u8,
    columns: usize,
    rows: usize,
    opacity: u8,
) -> Result<ColoredBrailleImage, String> {
    if columns == 0
        || rows == 0
        || columns > 1024
        || rows > 512
        || tick >= WIZARD_SETTLE + WIZARD_IDLE_FRAMES
    {
        return Err("invalid mountain canvas".into());
    }
    let mut image = ColoredBrailleImage {
        width: columns,
        height: rows,
        cells: vec![ColoredBrailleCell::default(); columns * rows],
    };
    let dot_w = columns * 2;
    let dot_h = rows * 4;
    // Trace the chart from its low left origin. Length-based reveal keeps the pen
    // moving evenly over both steep gains and quieter connecting segments.
    let lengths: Vec<f32> = ASCENT
        .windows(2)
        .map(|pair| {
            let dx = (pair[1].0 - pair[0].0) * dot_w as f32;
            let dy = (pair[1].1 - pair[0].1) * dot_h as f32;
            dx.hypot(dy)
        })
        .collect();
    let reveal = smoothstep(f32::from(tick.min(HOLD)) / f32::from(HOLD));
    let mut remaining = lengths.iter().sum::<f32>() * reveal;
    for (pair, length) in ASCENT.windows(2).zip(lengths) {
        if remaining <= 0.0 {
            break;
        }
        let portion = (remaining / length).min(1.0);
        let end = (
            pair[0].0 + (pair[1].0 - pair[0].0) * portion,
            pair[0].1 + (pair[1].1 - pair[0].1) * portion,
        );
        draw_dot_line(
            &mut image,
            plot_point(pair[0], dot_w, dot_h),
            plot_point(end, dot_w, dot_h),
            INTRO_INK,
            opacity,
        );
        remaining -= length;
    }

    if let Some(x) = wizard_x_at(tick) {
        let (wizard_x, ground_y) = plot_point((x, ASCENT[8].1), dot_w, dot_h);
        draw_wizard(
            &mut image,
            wizard_x,
            ground_y - 1,
            if tick >= WIZARD_SETTLE {
                tick - WIZARD_SETTLE
            } else {
                (tick - HOLD) / 3
            },
            tick >= WIZARD_SETTLE,
            opacity,
        );
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn startup_intro_rises_once_holds_and_fades_without_restarting() {
        let now = Instant::now();
        let mut intro = StartupIntro {
            started: Some(now),
            ..Default::default()
        };
        assert_eq!(intro.sample(now, MotionMode::Full), Some((0, None, 0, 255)));
        assert_eq!(
            intro.sample(now + Duration::from_secs(2), MotionMode::Full),
            Some((24, None, 0, 255))
        );
        // After the rise the sword holds; the water keeps flowing: the late-clip
        // frames cycle while the blade frame stays pinned.
        let (held, flow_a, _, _) = intro
            .sample(now + Duration::from_secs(50), MotionMode::Full)
            .unwrap();
        assert_eq!(held, HOLD);
        let flow_a = flow_a.expect("the settled blade keeps the water flowing");
        assert!(RIPPLE.contains(&flow_a.frame));
        let (_, flow_b, _, _) = intro
            .sample(now + Duration::from_secs(51), MotionMode::Full)
            .unwrap();
        assert!(RIPPLE.contains(&flow_b.unwrap().frame));
        // The cycle closes on a dissolve rather than the 6.3-level jump a hard wrap
        // left: the last late-clip frame melts into the first over SEAM ticks, then
        // the cycle restarts on that first frame outright.
        let cycle = RIPPLE.len() as u128;
        assert_eq!(
            Flow::at(0),
            Flow {
                frame: RIPPLE[0],
                next: RIPPLE[0],
                melt: 0
            }
        );
        let melts: Vec<u8> = (cycle..cycle + u128::from(SEAM))
            .map(|tick| {
                let flow = Flow::at(tick);
                assert_eq!(
                    (flow.frame, flow.next),
                    (RIPPLE[RIPPLE.len() - 1], RIPPLE[0])
                );
                flow.melt
            })
            .collect();
        assert!(
            melts.windows(2).all(|pair| pair[0] < pair[1])
                && melts.iter().all(|&melt| melt > 0 && melt < 255),
            "the whole seam span dissolves, never snapping: {melts:?}"
        );
        assert_eq!(
            Flow::at(cycle + u128::from(SEAM)),
            Flow {
                frame: RIPPLE[0],
                next: RIPPLE[0],
                melt: 0
            }
        );
        intro.dismiss(now + Duration::from_secs(2), MotionMode::Full);
        assert_eq!(
            intro.sample(now + Duration::from_millis(2300), MotionMode::Full),
            Some((24, None, 0, 128))
        );
        intro.dismiss(now + Duration::from_millis(2500), MotionMode::Full);
        assert!(
            intro
                .sample(now + Duration::from_millis(2600), MotionMode::Full)
                .is_none()
        );
        intro.cancel();
        intro.begin_frame(false, false, MotionMode::Full);
        assert!(intro.sample(now, MotionMode::Full).is_none());
    }

    #[test]
    fn startup_intro_respects_motion_and_hidden_tick_policy() {
        let now = Instant::now();
        let mut intro = StartupIntro {
            started: Some(now),
            visible: true,
            ..Default::default()
        };
        for motion in [MotionMode::Reduced, MotionMode::Off] {
            assert_eq!(intro.sample(now, motion), Some((59, None, 0, 255)));
            assert!(!intro.animating(now, motion));
        }
        assert!(intro.animating(now, MotionMode::Full));
        intro.begin_frame(false, false, MotionMode::Full);
        assert!(!intro.animating(now, MotionMode::Full));
        intro.dismiss(now, MotionMode::Reduced);
        assert!(
            intro
                .sample(now + Duration::from_millis(200), MotionMode::Reduced)
                .is_none()
        );
        let mut off = StartupIntro {
            started: Some(now),
            ..Default::default()
        };
        off.dismiss(now, MotionMode::Off);
        assert!(off.finished);
    }

    #[test]
    fn startup_intro_keys_mountain_reveal_and_summit_walk_to_the_sword() {
        let now = Instant::now();
        let intro = StartupIntro {
            started: Some(now),
            ..Default::default()
        };
        assert_eq!(intro.landscape_tick(now, MotionMode::Full), Some(0));
        assert_eq!(
            intro.landscape_tick(now + Duration::from_millis(4_100), MotionMode::Full),
            Some(HOLD)
        );
        assert_eq!(
            intro.landscape_tick(now + Duration::from_millis(6_100), MotionMode::Full),
            Some(WIZARD_SETTLE)
        );
        assert_eq!(
            intro.landscape_tick(now + Duration::from_secs(60), MotionMode::Full),
            Some(WIZARD_SETTLE + (((720 - u128::from(WIZARD_SETTLE)) / 3) % 8) as u8),
            "the completed summit keeps a bounded sprite idle loop"
        );
        assert_eq!(
            intro.landscape_tick(now, MotionMode::Reduced),
            Some(WIZARD_SETTLE),
            "reduced motion selects the final static tableau"
        );
    }

    #[test]
    fn startup_intro_keystroke_and_paste_dismiss_before_submission() {
        let _guard = crate::tests::env_lock();
        for paste in [false, true] {
            let mut app = crate::seed_preview_app();
            app.messages.clear();
            app.input.clear();
            app.cursor = 0;
            app.visual_motion = MotionMode::Full;
            app.startup_intro = StartupIntro::default();
            app.startup_intro.started = Some(Instant::now());
            app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
            app.on_paste("");
            assert!(app.startup_intro.fading.is_none());
            if paste {
                app.on_paste("pick up the sword");
                assert_eq!(app.input, "pick up the sword");
            } else {
                app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
                assert_eq!(app.input, "a");
                app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
                assert!(app.input.is_empty());
            }
            assert!(app.startup_intro.fading.is_some());
            assert!(app.messages.is_empty());
            assert!(app.pending_turn.is_none());
        }
    }

    #[test]
    fn startup_intro_real_content_and_prefilled_drafts_never_flash_art() {
        let _guard = crate::tests::env_lock();
        for occupied in [false, true] {
            let mut intro = StartupIntro::default();
            intro.begin_frame(occupied, !occupied, MotionMode::Full);
            intro.begin_frame(false, false, MotionMode::Full);
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal
                .draw(|frame| intro.render(frame, frame.area(), None, MotionMode::Full))
                .unwrap();
            assert!(intro.finished);
            assert!(intro.worker.is_none());
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .all(|c| c.symbol() == " ")
            );
        }
    }

    #[test]
    fn startup_intro_actual_clip_is_braille_fitted_and_fades_without_dot_shimmer() {
        let atlas = decode_atlas(ATLAS).unwrap();
        // A settled frame with no flow: the still case the fade assertions cover.
        let still = |opacity| Key {
            scene: Scene::Sword,
            frame: 59,
            flow: None,
            water: 0,
            opacity,
            size: Size::new(0, 0),
            geometry: None,
        };
        for (w, h) in [(1, 1), (24, 40), (100, 35), (240, 70)] {
            let raised = compose(&atlas, &mut Cache::default(), &still(255), w, h).unwrap();
            let faded = compose(&atlas, &mut Cache::default(), &still(64), w, h).unwrap();
            assert_eq!(raised.cells.len(), w * h);
            assert!(
                raised
                    .cells
                    .iter()
                    .all(|c| ('\u{2800}'..='\u{28ff}').contains(&c.glyph))
            );
            // The fade now drops dimmer dots instead of dimming ink, so the faded
            // frame keeps a subset of the raised frame's glyphs and pure white ink.
            let raised_lit = raised
                .cells
                .iter()
                .filter(|c| c.glyph != '\u{2800}')
                .count();
            let faded_lit = faded.cells.iter().filter(|c| c.glyph != '\u{2800}').count();
            assert!(
                faded_lit < raised_lit || raised_lit == 0,
                "fade did not drop dots at w={w} h={h}"
            );
            assert!(
                raised
                    .cells
                    .iter()
                    .all(|c| c.fg == [0; 3] || c.fg == [255; 3])
            );
            assert!(
                faded
                    .cells
                    .iter()
                    .all(|c| c.fg == [0; 3] || c.fg == [255; 3])
            );
            if w > 1 {
                assert!(
                    raised
                        .cells
                        .iter()
                        .filter(|c| c.glyph != '\u{2800}')
                        .count()
                        > 10
                );
                // Letterboxing preserves the sword tip and the waterline.
                assert!(raised.cells[..w].iter().all(|c| c.glyph == '\u{2800}'));
            }
        }
        let black = GrayImage::new(FRAME_W * 10, FRAME_H * 6);
        assert!(
            compose(&black, &mut Cache::default(), &still(255), 100, 40)
                .unwrap()
                .cells
                .iter()
                .all(|c| c.glyph == '\u{2800}')
        );
    }

    #[test]
    fn startup_mountain_is_a_low_rising_contour_with_a_compact_summit_wizard() {
        assert!(ASCENT[0].1 > 0.85, "the base sits near the bottom");
        assert!(
            ASCENT
                .windows(2)
                .all(|p| p[1].0 > p[0].0 && p[1].1 <= p[0].1)
        );
        assert_eq!(wizard_x_at(HOLD - 1), None);
        assert!(wizard_x_at(HOLD).unwrap() > 1.0);
        assert!((wizard_x_at(WIZARD_SETTLE).unwrap() - 0.84).abs() < 0.001);
        for (columns, rows) in [(36, 16), (72, 32), (120, 24)] {
            let ridge = compose_mountain(HOLD, columns, rows, 255).unwrap();
            let settled = compose_mountain(WIZARD_SETTLE, columns, rows, 255).unwrap();
            let lit = |cell: &&ColoredBrailleCell| cell.glyph != NO_DOTS;
            assert!(
                settled.cells.iter().filter(lit).count() > ridge.cells.iter().filter(lit).count()
            );
            // Below the low contour there is no floor, wireframe or back face.
            assert!(
                settled.cells[(rows - 1) * columns..]
                    .iter()
                    .all(|c| c.glyph == NO_DOTS)
            );
            assert!(settled.cells.iter().filter(lit).all(|c| c.fg == INTRO_INK));
            assert!(
                compose_mountain(WIZARD_SETTLE, columns, rows, 0)
                    .unwrap()
                    .cells
                    .iter()
                    .all(|c| c.glyph == NO_DOTS)
            );
        }
    }

    #[test]
    fn startup_wizard_idle_moves_cloth_without_moving_the_summit() {
        let still = compose_mountain(WIZARD_SETTLE, 72, 32, 255).unwrap();
        let breeze = compose_mountain(WIZARD_SETTLE + 2, 72, 32, 255).unwrap();
        assert!(
            still
                .cells
                .iter()
                .zip(&breeze.cells)
                .any(|(a, b)| a.glyph != b.glyph)
        );
        assert!(
            still.cells[14 * 72..]
                .iter()
                .zip(&breeze.cells[14 * 72..])
                .all(|(a, b)| a.glyph == b.glyph)
        );
        let now = Instant::now();
        let intro = StartupIntro {
            started: Some(now),
            ..Default::default()
        };
        for motion in [MotionMode::Reduced, MotionMode::Off] {
            assert_eq!(
                intro.landscape_tick(now + Duration::from_secs(90), motion),
                Some(WIZARD_SETTLE)
            );
        }
    }

    #[test]
    fn startup_mountain_clips_plot_lines_to_the_dot_canvas() {
        let mut image = ColoredBrailleImage {
            width: 4,
            height: 2,
            cells: vec![ColoredBrailleCell::default(); 8],
        };
        draw_dot_line(&mut image, (-10_000, 4), (10_000, 4), INTRO_INK, 255);
        assert_eq!(
            image
                .cells
                .iter()
                .filter(|cell| cell.glyph != NO_DOTS)
                .count(),
            4,
            "a crossing line is clipped to one dot row across the four cells"
        );
        let before = image.clone();
        draw_dot_line(
            &mut image,
            (-10_000, -10_000),
            (-5_000, -5_000),
            INTRO_INK,
            255,
        );
        assert_eq!(image, before, "a wholly offscreen line is rejected");
    }

    #[test]
    fn startup_intro_worker_is_bounded_and_resize_rejects_old_geometry() {
        let mut intro = StartupIntro::default();
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let first = Rect::new(2, 2, 80, 30);
        let second = Rect::new(2, 2, 40, 20);
        terminal
            .draw(|frame| intro.render(frame, first, None, MotionMode::Reduced))
            .unwrap();
        assert!(intro.surfaces[0].pending);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            terminal
                .draw(|frame| intro.render(frame, second, None, MotionMode::Reduced))
                .unwrap();
            if let Some((key, _)) = &intro.surfaces[0].current {
                assert_eq!(key.size, second.as_size());
                break;
            }
            assert!(Instant::now() < deadline, "intro worker did not deliver");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!intro.surfaces[0].pending);
        assert!(!intro.animating(Instant::now(), MotionMode::Reduced));
        intro.dismiss(Instant::now(), MotionMode::Off);
        assert!(intro.surfaces[0].current.is_none() && intro.worker.is_none());
    }

    #[test]
    fn startup_intro_fine_transport_owns_separate_ids_and_transparent_dots() {
        let mut intro = StartupIntro::default();
        let mut terminal = Terminal::new(TestBackend::new(80, 30)).unwrap();
        let area = Rect::new(2, 2, 60, 24);
        let geometry = DotGeometry::new(area.width, area.height, (8, 16), 2).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            terminal
                .draw(|frame| {
                    intro.render(frame, area, Some(geometry), MotionMode::Reduced);
                    intro.flush_upload(frame);
                })
                .unwrap();
            if let Some((_, ready)) = &intro.surfaces[0].current {
                let protocol = ready.protocol.as_ref().expect("fine-dot transport");
                assert_eq!(protocol.image_id() & 2, 0, "world owns the other ID pair");
                assert_eq!(ready.dots.width, geometry.grid_width);
                let buffer = terminal.backend().buffer();
                assert!(buffer.cell((0, 0)).unwrap().symbol().contains("\x1b_G"));
                assert!(
                    buffer
                        .cell((2, 2))
                        .unwrap()
                        .symbol()
                        .starts_with('\u{10eeee}')
                );
                assert_eq!(buffer.cell((1, 2)).unwrap().symbol(), " ");
                // The transported pixels are the animation: the generated dots on
                // a transparent background, so the terminal composites the sword
                // over the pane instead of over a plate uploaded behind it.
                let upload = buffer
                    .cell((0, 0))
                    .unwrap()
                    .symbol()
                    .strip_suffix(' ')
                    .expect("upload keeps the cell's own blank");
                let transported = crate::dot_protocol::decode_upload(upload);
                assert_eq!(
                    transported,
                    geometry.rasterize_on(&ready.dots, TRANSPARENT).unwrap()
                );
                assert_eq!(
                    transported.get_pixel(0, 0).0[3],
                    0,
                    "the fitted frame's empty margin must stay transparent"
                );
                assert!(transported.pixels().any(|pixel| pixel.0[3] == 255));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        intro.begin_frame(true, false, MotionMode::Full);
        assert!(intro.surfaces[0].current.is_none() && intro.worker.is_none());
    }

    /// Terminals without fine-dot transport get the Braille fallback. It paints
    /// white ink and nothing else: cells the sword never reaches keep whatever the
    /// pane painted there, and no cell carries a background under the dots.
    #[test]
    fn startup_intro_braille_fallback_paints_only_white_dots() {
        let mut intro = StartupIntro {
            started: Some(Instant::now()),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(70, 30)).unwrap();
        let area = Rect::new(2, 2, 60, 22);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            terminal
                .draw(|frame| {
                    for y in area.y..area.bottom() {
                        for x in area.x..area.right() {
                            frame.buffer_mut().cell_mut((x, y)).unwrap().set_symbol("#");
                        }
                    }
                    intro.render(frame, area, None, MotionMode::Reduced);
                })
                .unwrap();
            if intro.surfaces[0].current.is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "intro worker did not deliver");
            std::thread::sleep(Duration::from_millis(5));
        }
        let buffer = terminal.backend().buffer();
        let backdrop = (area.y..area.bottom())
            .filter(|&y| buffer.cell((area.x, y)).unwrap().symbol() == "#")
            .count();
        assert_eq!(
            backdrop,
            usize::from(area.height),
            "the empty margin left of the fitted sword must keep the backdrop"
        );
        let mut painted = 0;
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let cell = buffer.cell((x, y)).unwrap();
                if cell.symbol() == "#" {
                    continue;
                }
                painted += 1;
                assert_eq!(cell.bg, Color::Reset, "the fallback paints no background");
                assert_eq!(cell.fg, Color::Rgb(255, 255, 255), "dots are white ink");
                assert_ne!(cell.symbol(), " ");
                assert_ne!(cell.symbol(), "\u{2800}", "empty cells stay backdrop");
            }
        }
        assert!(painted > 0, "the fallback painted no dots");
    }

    #[test]
    fn startup_intro_keeps_sword_and_mountain_in_distinct_panes_with_shared_dismissal() {
        let mut intro = StartupIntro::default();
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let areas = [Rect::new(2, 2, 70, 35), Rect::new(80, 20, 36, 16)];
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            terminal
                .draw(|frame| {
                    intro.begin_frame(false, false, MotionMode::Reduced);
                    intro.render(
                        frame,
                        areas[0],
                        DotGeometry::new(70, 35, (8, 16), 2),
                        MotionMode::Reduced,
                    );
                    assert!(intro.render_miniviz(
                        frame,
                        areas[1],
                        DotGeometry::new(36, 16, (8, 16), 2),
                        MotionMode::Reduced
                    ));
                    intro.flush_upload(frame);
                })
                .unwrap();
            if intro
                .surfaces
                .iter()
                .all(|surface| surface.current.is_some())
            {
                break;
            }
            assert!(Instant::now() < deadline, "both intro panes must finish");
            std::thread::sleep(Duration::from_millis(5));
        }
        let sword = &intro.surfaces[0].current.as_ref().unwrap().1.dots;
        assert!(
            sword
                .cells
                .iter()
                .all(|cell| cell.fg == [0; 3] || cell.fg == [255; 3]),
            "the left pane remains the white Excalibur clip"
        );
        let mountain = &intro.surfaces[1].current.as_ref().unwrap().1.dots;
        assert!(mountain.cells.iter().any(|cell| cell.glyph != NO_DOTS));
        assert!(
            mountain
                .cells
                .iter()
                .filter(|cell| cell.glyph != NO_DOTS)
                .all(|cell| cell.fg == [255; 3]),
            "the right pane must match the sword's white ink"
        );
        let ids = intro.surfaces.each_ref().map(|surface| {
            surface
                .current
                .as_ref()
                .unwrap()
                .1
                .protocol
                .as_ref()
                .unwrap()
                .image_id()
        });
        assert_ne!(ids[0], ids[1]);
        assert!(ids.iter().all(|id| id & 2 == 0));
        for area in areas {
            assert!(
                terminal
                    .backend()
                    .buffer()
                    .cell((area.x, area.y))
                    .unwrap()
                    .symbol()
                    .starts_with('\u{10eeee}')
            );
        }
        intro.dismiss(Instant::now() - FADE, MotionMode::Full);
        terminal
            .draw(|frame| {
                assert!(!intro.render_miniviz(frame, areas[1], None, MotionMode::Full));
            })
            .unwrap();
        assert!(
            intro
                .surfaces
                .iter()
                .all(|surface| surface.current.is_none() && !surface.pending)
        );
        assert!(intro.worker.is_none());
    }

    #[test]
    fn startup_intro_full_cockpit_wires_empty_shell_and_never_replays_after_draft() {
        let _guard = crate::tests::env_lock();
        let _comp = crate::tests::TestEnvGuard::unset("ANGEL_COMP_MODE");
        let _turbo = crate::tests::TestEnvGuard::unset("ANGEL_TURBO");
        let mut app = crate::app::App::preview(crate::viewer::Viewer::static_preview());
        app.startup_intro = StartupIntro::default();
        app.visual_motion = MotionMode::Reduced;
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app
            .startup_intro
            .surfaces
            .iter()
            .any(|surface| surface.current.is_none())
        {
            terminal
                .draw(|frame| crate::draw::ui(frame, &mut app))
                .unwrap();
            assert!(
                Instant::now() < deadline,
                "full UI never painted both startup panes"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.startup_intro.visible);
        // Entering an actual room must retain the world even before typing;
        // only the still-empty agent shell keeps the launch ceremony.
        app.world
            .settle_at_for_test(crate::world_viz::Building::Keep);
        assert!(app.world.enter_interior());
        terminal
            .draw(|frame| crate::draw::ui(frame, &mut app))
            .unwrap();
        assert!(app.world_pane_visible);
        assert!(app.startup_intro.visible);
        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(app.input, "a");
        app.startup_intro.fading = Some(Instant::now() - FADE);
        terminal
            .draw(|frame| crate::draw::ui(frame, &mut app))
            .unwrap();
        assert!(app.startup_intro.finished);
        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        terminal
            .draw(|frame| crate::draw::ui(frame, &mut app))
            .unwrap();
        assert!(app.input.is_empty());
        assert!(app.startup_intro.worker.is_none());
        assert!(!app.startup_intro.visible);
    }

    /// Visual proof sheet of the production renderer, saved the way the terminal
    /// receives it: dots on transparency. Private output only.
    #[test]
    #[ignore = "writes a visual proof sheet from the production renderer"]
    fn startup_intro_export_dotmax_proof() {
        let dir =
            std::env::var("ANGEL_T_INTRO_PROOF_DIR").expect("explicit proof output directory");
        let atlas = decode_atlas(ATLAS).unwrap();
        let geometry = DotGeometry::new(100, 35, (8, 16), 2).unwrap();
        let mut sheet = image::RgbaImage::new(geometry.width * 3, geometry.height * 2);
        for (i, (index, opacity)) in [
            (0, 255),
            (18, 255),
            (36, 255),
            (59, 255),
            (59, 128),
            (59, 20),
        ]
        .into_iter()
        .enumerate()
        {
            let key = Key {
                scene: Scene::Sword,
                frame: index,
                flow: None,
                water: 0,
                opacity,
                size: Size::new(0, 0),
                geometry: None,
            };
            let dots = compose(
                &atlas,
                &mut Cache::default(),
                &key,
                geometry.grid_width,
                geometry.grid_height,
            )
            .unwrap();
            // The proof carries the transported pixels: dots on transparency.
            let raster = geometry.rasterize_on(&dots, TRANSPARENT).unwrap();
            imageops::overlay(
                &mut sheet,
                &raster,
                (i % 3) as i64 * i64::from(geometry.width),
                (i / 3) as i64 * i64::from(geometry.height),
            );
        }
        sheet
            .save(std::path::Path::new(&dir).join("dotmax-proof.png"))
            .unwrap();
        let mini = DotGeometry::new(36, 16, (8, 16), 2).unwrap();
        let key = Key {
            scene: Scene::Mountain {
                tick: WIZARD_SETTLE,
            },
            frame: 0,
            flow: None,
            water: 0,
            opacity: 255,
            size: Size::new(0, 0),
            geometry: None,
        };
        let dots = compose(
            &atlas,
            &mut Cache::default(),
            &key,
            mini.grid_width,
            mini.grid_height,
        )
        .unwrap();
        mini.rasterize_on(&dots, TRANSPARENT)
            .unwrap()
            .save(std::path::Path::new(&dir).join("miniviz-proof.png"))
            .unwrap();

        let mut idle_sheet = image::RgbaImage::new(mini.width * 4, mini.height * 2);
        for pose in 0..WIZARD_IDLE_FRAMES {
            let dots =
                compose_mountain(WIZARD_SETTLE + pose, mini.grid_width, mini.grid_height, 255)
                    .unwrap();
            let raster = mini.rasterize_on(&dots, TRANSPARENT).unwrap();
            raster
                .save(std::path::Path::new(&dir).join(format!("wizard-idle-{pose}.png")))
                .unwrap();
            imageops::overlay(
                &mut idle_sheet,
                &raster,
                i64::from(pose % 4) * i64::from(mini.width),
                i64::from(pose / 4) * i64::from(mini.height),
            );
        }
        idle_sheet
            .save(std::path::Path::new(&dir).join("wizard-idle-proof.png"))
            .unwrap();

        let mut mountain_sheet = image::RgbaImage::new(mini.width * 3, mini.height * 2);
        for (i, tick) in [0, 16, 32, HOLD, HOLD + WIZARD_WALK_TICKS / 2, WIZARD_SETTLE]
            .into_iter()
            .enumerate()
        {
            let dots = compose_mountain(tick, mini.grid_width, mini.grid_height, 255).unwrap();
            let raster = mini.rasterize_on(&dots, TRANSPARENT).unwrap();
            imageops::overlay(
                &mut mountain_sheet,
                &raster,
                (i % 3) as i64 * i64::from(mini.width),
                (i / 3) as i64 * i64::from(mini.height),
            );
        }
        mountain_sheet
            .save(std::path::Path::new(&dir).join("mountain-sequence-proof.png"))
            .unwrap();

        // Timing proof: the actual sword and miniviz renderers sampled from the
        // same 12 fps clock. The smaller right frame is bottom-aligned, matching
        // its role as the cockpit's compact world window rather than a second
        // full-size presentation panel.
        let sword = DotGeometry::new(56, 20, (8, 16), 2).unwrap();
        let ticks = [0, 16, 32, HOLD, HOLD + WIZARD_WALK_TICKS / 2, WIZARD_SETTLE];
        let row_height = sword.height.max(mini.height);
        let mut timed = image::RgbaImage::new(sword.width + mini.width, row_height * 6);
        for (row, tick) in ticks.into_iter().enumerate() {
            let sword_key = Key {
                scene: Scene::Sword,
                frame: tick.min(HOLD),
                flow: None,
                water: 0,
                opacity: 255,
                size: Size::new(0, 0),
                geometry: None,
            };
            let sword_dots = compose(
                &atlas,
                &mut Cache::default(),
                &sword_key,
                sword.grid_width,
                sword.grid_height,
            )
            .unwrap();
            let sword_raster = sword.rasterize_on(&sword_dots, TRANSPARENT).unwrap();
            imageops::overlay(
                &mut timed,
                &sword_raster,
                0,
                i64::from(row as u32 * row_height),
            );

            let mountain_dots =
                compose_mountain(tick, mini.grid_width, mini.grid_height, 255).unwrap();
            let mountain_raster = mini.rasterize_on(&mountain_dots, TRANSPARENT).unwrap();
            imageops::overlay(
                &mut timed,
                &mountain_raster,
                i64::from(sword.width),
                i64::from(row as u32 * row_height + row_height - mini.height),
            );
        }
        timed
            .save(std::path::Path::new(&dir).join("startup-sequence-proof.png"))
            .unwrap();
    }

    /// Visual proof of the ambient loop: every tick of two cycles, as PNGs, so the
    /// seam and the hand/hilt isolation can be seen rather than argued. Ignored by
    /// default — it writes a frame sequence, it asserts nothing. Each frame is the
    /// transported pixels: dots on transparency, never a plate behind them.
    #[test]
    #[ignore = "writes a visual proof sequence from the production renderer"]
    fn startup_intro_export_water_flow_proof() {
        let dir =
            std::env::var("ANGEL_T_INTRO_PROOF_DIR").expect("explicit proof output directory");
        let atlas = decode_atlas(ATLAS).unwrap();
        let geometry = DotGeometry::new(100, 35, (8, 16), 2).unwrap();
        let mut cache = Cache::default();
        let ticks = (RIPPLE.len() as u128 + u128::from(SEAM)) * 2;
        for tick in 0..ticks {
            let key = Key {
                scene: Scene::Sword,
                frame: HOLD,
                flow: Some(Flow::at(tick)),
                // Past the handoff: the flow is fully in charge of the water.
                water: 255,
                opacity: 255,
                size: Size::new(geometry.width as u16, geometry.height as u16),
                geometry: None,
            };
            let dots = compose(
                &atlas,
                &mut cache,
                &key,
                geometry.grid_width,
                geometry.grid_height,
            )
            .unwrap();
            geometry
                .rasterize_on(&dots, TRANSPARENT)
                .unwrap()
                .save(std::path::Path::new(&dir).join(format!("flow-{tick:03}.png")))
                .unwrap();
        }
    }
}
