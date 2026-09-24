//! Once per launch: take up Excalibur by beginning a draft. All video sampling,
//! Dotmax dithering and fine-dot encoding happen on a single bounded worker, which
//! prepares the settled and late-clip canvases once and re-uses them for every
//! ambient tick. Only the dots are ever painted: the transported fine-dot frame
//! carries a transparent background and the Braille fallback skips empty cells,
//! so the shell and the mini-viz keep the pane behind the animation.

use crate::ui::dots::canvas::{DotGeometry, TRANSPARENT};
use crate::ui::dots::protocol::DotProtocol;
use crate::ui::term::art::{ColoredBrailleCell, ColoredBrailleImage, braille_dot_bit};
use crate::ui::viz::lifecycle_viz::MotionMode;
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

const ATLAS: &[u8] = include_bytes!("../../assets/excalibur/rise.png");
const FRAME_W: u32 = 680;
const FRAME_H: u32 = 384;
/// The clip's fixed framing, and the aspect every intro band is measured
/// against: empty side wings trimmed, the whole blade, hand and water retained.
const CROP_X: u32 = 140;
const CROP_W: u32 = FRAME_W - CROP_X * 2;
/// No frame of the clip lights a pixel below row 369: the last 14 rows are
/// black in every one of the 60 frames (measured on the atlas alpha plane).
/// Cropping them off makes the water's own bottom the frame's bottom, so the
/// floor-pinned fit in `prepare` puts the last ripple on the floor of the pane
/// instead of a dead strip.
const CROP_H: u32 = 370;
/// Widest intro band we will compose, in terminal columns. The crop is nearly
/// square (`CROP_W` × `CROP_H`), so the blade is always height-limited: past
/// this many columns a wider pane buys no extra art, only a wider dithered
/// canvas, a wider fine-dot raster and a wider Braille grid carrying the same
/// blade — which is what read as the width pixelating the clip. Bounding the
/// band, centring it and standing it on the floor of the pane keeps the art the
/// same size it already was while making the composed surface width-invariant.
const INTRO_MAX_COLUMNS: u16 = 96;
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

/// The presented intro band for one pane: width-clamped, horizontally centred,
/// and stood on the floor of the pane.
///
/// The two bounds are one rule. The clip is nearly square, so the art it can
/// actually fill at `rows` Braille rows is `rows * 4` dot rows tall and — at the
/// crop's aspect — `rows * 4 * CROP_W / CROP_H` dot columns wide, which is
/// `rows * 2 * CROP_W / CROP_H` terminal columns. Clamping to that means the
/// composed surface is never wider than the dots that can carry it; clamping to
/// `INTRO_MAX_COLUMNS` keeps a very tall pane from ballooning the band. Either
/// way the canvas, the fine-dot raster and the Braille grid all shrink to the
/// art, and the leftover width becomes margin on both sides instead of stretch
/// or a dead letterbox band. Height is untouched, so the waterline stays on the
/// floor of the pane. Idempotent: re-banding a band returns it.
pub(crate) fn intro_band(area: Rect) -> Rect {
    if area.width == 0 || area.height == 0 {
        return area;
    }
    let rows = usize::from(area.height);
    let fill = (rows * 4 * CROP_W as usize / CROP_H as usize / 2).max(1);
    let width = area
        .width
        .min(INTRO_MAX_COLUMNS)
        .min(u16::try_from(fill).unwrap_or(u16::MAX))
        .max(1);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y,
        width,
        height: area.height,
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
    let source = imageops::crop_imm(
        atlas,
        u32::from(index % 10) * FRAME_W + CROP_X,
        u32::from(index / 10) * FRAME_H,
        CROP_W,
        CROP_H,
    )
    .to_image();
    let width = columns as u32 * 2;
    let height = rows as u32 * 4;
    let scale = (width as f32 / CROP_W as f32).min(height as f32 / CROP_H as f32);
    let fitted = imageops::resize(
        &source,
        (CROP_W as f32 * scale).round().max(1.0) as u32,
        (CROP_H as f32 * scale).round().max(1.0) as u32,
        imageops::FilterType::Lanczos3,
    );
    let mut canvas = GrayImage::new(width, height);
    // X stays centered; Y is pinned to the floor. In a wide pane the fit is
    // height-limited and the two agree, but in a narrow pane (the mini-viz, a
    // tall bay) the fit is width-limited, and a centered Y floated the whole
    // blade — waterline, hand and all — into the middle of the pane with a dead
    // strip underneath: the water read as a hard cut mid-frame instead of a
    // surface sitting on the floor. The slack belongs above the blade. Uses
    // saturating_sub because the rounded fit can land a pixel past the canvas.
    imageops::overlay(
        &mut canvas,
        &fitted,
        i64::from((width - fitted.width()) / 2),
        i64::from(height.saturating_sub(fitted.height())),
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
pub(crate) const INTRO_INK: [u8; 3] = [255; 3];
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

pub(crate) fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn plot_point(point: (f32, f32), dot_w: usize, dot_h: usize) -> (i32, i32) {
    (
        (point.0 * dot_w.saturating_sub(1) as f32).round() as i32,
        (point.1 * dot_h.saturating_sub(1) as f32).round() as i32,
    )
}

pub(crate) fn paint_dot(
    image: &mut ColoredBrailleImage,
    x: i32,
    y: i32,
    color: [u8; 3],
    opacity: u8,
) {
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

pub(crate) fn draw_dot_line(
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

pub(crate) fn draw_wizard(
    image: &mut ColoredBrailleImage,
    centre_x: i32,
    ground_y: i32,
    walk_pose: u8,
    settled: bool,
    opacity: u8,
) {
    let height = (image.height as f32 * 4.0 * 0.23)
        .min(image.width as f32 * 2.0 * 0.28)
        .max(12.0)
        * 0.5;
    draw_wizard_sized(
        image, centre_x, ground_y, height, walk_pose, settled, opacity,
    );
}

/// The wizard `height` dots tall (hat tip to feet).
pub(crate) fn draw_wizard_sized(
    image: &mut ColoredBrailleImage,
    centre_x: i32,
    ground_y: i32,
    height: f32,
    walk_pose: u8,
    settled: bool,
    opacity: u8,
) {
    // Continuous contours sample directly onto Dotmax's dot grid: the hat and
    // cloak stay fine-edged instead of enlarging a chunky bitmap in integer steps.
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
#[path = "../../../tests/cockpit/app/startup_intro__tests.rs"]
mod tests;
