//! A room in 3D, as the game hands it over: whether the host draws rooms in
//! 3D at all, and the room's world as a window places it on its stage.
//!
//! A function of depth ([`Mode`], docs/WORLD-3D.md "Deeper floors"): the
//! upper floors keep the 2D black look, the deep ones ([`DEEP`] and below)
//! stand up in 3D, the floor between ([`TRANSITION`]) its guardian's hall
//! only. `ANGEL_WORLD3D=1` stands every room up, the kingdom too, and
//! `ANGEL_WORLD3D=0` none. A room that stands up, seen from above, stands
//! up lit in the monolith look (`rooms_lit`, docs/REALM-MONOLITH.md "Rooms,
//! lit"), no plate needed; a host that says `ANGEL_WORLD3D_PLATES=1` draws
//! them as before, a room with an item (`assets`) its painted plate cast on
//! its blocks, one with none plain (`plain`) or flat.

use std::sync::{Arc, Mutex};

use angel_stage::scene::{Node, SetRef};
use angel_stage::set::{Kind, Set, place};

use super::assets::{self, Fit, Item};
use super::fit::Variant;
use super::plain::Plain;
use super::rooms_lit::Lit;
use super::rooms_lit_look::Activity;
use crate::render::overworld::Img;
use crate::rules::{Pack, Room};

/// How far down the dungeon stands up in 3D: the host's one setting, sent
/// to every seat with the welcome (`wire::Params`), so a page and its host
/// always agree which floor is which.
///
/// - `Off`: every room as it always was, the 2D black look.
/// - `Deep` (the default): the dungeon's deep floors stand up lit
///   ([`DEEP`] and below), the floor above them its guardian's hall only
///   ([`TRANSITION`]); the upper floors, the home and the kingdom stay 2D.
/// - `Always` (`ANGEL_WORLD3D=1`, for testing): every room, the kingdom too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Off,
    #[default]
    Deep,
    Always,
}

impl Mode {
    /// The mode a host's environment asks for: `ANGEL_WORLD3D=1` every room,
    /// `ANGEL_WORLD3D=0` none, anything else (or nothing) the deep floors.
    pub fn from_env(value: Option<&str>) -> Mode {
        match value {
            Some("1") => Mode::Always,
            Some("0") => Mode::Off,
            _ => Mode::Deep,
        }
    }

    fn code(self) -> u8 {
        match self {
            Mode::Off => 0,
            Mode::Deep => 1,
            Mode::Always => 2,
        }
    }

    fn of_code(code: u8) -> Mode {
        match code {
            1 => Mode::Deep,
            2 => Mode::Always,
            _ => Mode::Off,
        }
    }
}

/// The first floor that stands up in 3D, every room of it: the Archive, the
/// first floor below the Hellforge's dragon (`rules::DEEP_FLOOR`).
pub const DEEP: u32 = crate::rules::DEEP_FLOOR;

/// The floor above [`DEEP`], where the change begins: only its guardian's
/// hall (a lair or the pit) stands up, the last room before the stair down.
pub const TRANSITION: u32 = 3;

#[cfg(not(test))]
static MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

// Tests run in parallel threads: each keeps its own switch, so one test's
// rooms in 3D never show up in another's frames. Off unless a test says.
#[cfg(test)]
std::thread_local! {
    static MODE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// How far down rooms stand up in 3D. A host turning any on looks its asset
/// folder over (`assets::warm`).
pub fn set_mode(mode: Mode) {
    if mode != Mode::Off {
        assets::warm();
    }
    if mode == Mode::Always {
        super::kingdom::warm();
    }
    #[cfg(not(test))]
    MODE.store(mode.code(), std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    MODE.with(|c| c.set(mode.code()));
}

/// The mode set (`set_mode`); `Off` until a host or a welcome sets one.
pub fn mode() -> Mode {
    #[cfg(not(test))]
    return Mode::of_code(MODE.load(std::sync::atomic::Ordering::Relaxed));
    #[cfg(test)]
    return Mode::of_code(MODE.with(|c| c.get()));
}

/// Every room in 3D (true, `Mode::Always`), or none ([`Mode::Off`]).
pub fn set_enabled(on: bool) {
    set_mode(if on { Mode::Always } else { Mode::Off });
}

/// Whether everything is drawn in 3D, the kingdom too (`Mode::Always`).
/// Whether one room is: [`stands`].
pub fn enabled() -> bool {
    mode() == Mode::Always
}

/// Whether a room of kind `kind` on floor `depth` stands up in 3D, in mode
/// `mode`: in `Deep`, every room of a floor at [`DEEP`] or below, and the
/// guardian's hall of the [`TRANSITION`] floor; never the home (floor 0).
pub fn stands_in_mode(mode: Mode, depth: u32, kind: crate::rules::RoomKind) -> bool {
    use crate::rules::RoomKind;
    match mode {
        Mode::Off => false,
        Mode::Always => true,
        Mode::Deep => {
            depth >= DEEP || (depth == TRANSITION && matches!(kind, RoomKind::Lair | RoomKind::Pit | RoomKind::Threshold))
        }
    }
}

/// Whether a room of kind `kind` on floor `depth` stands up in 3D now.
pub fn stands(depth: u32, kind: crate::rules::RoomKind) -> bool {
    stands_in_mode(mode(), depth, kind)
}

/// Whether the room run `run` is in stands up in 3D now.
pub fn stands_here(run: &crate::rules::Run) -> bool {
    stands(run.dungeon.depth, run.room().kind)
}

// ------------------------------------------------------------ the beat ----

/// How long the beat lasts as the party first comes down into the deep
/// (`Run::deep_beat`), in seconds: the cut from the stair held black, the
/// camera easing back from close on the party, and the room's lamps coming
/// up one after another from the nearest out, till the room is as it burns.
pub const BEAT: f32 = 3.0;

/// Seconds into the beat, while it plays in a room that stands up; `None`
/// otherwise. A function of the run's clock alone, so every seat plays it
/// alike, frame for frame.
pub fn beat(run: &crate::rules::Run) -> Option<f32> {
    let at = run.deep_beat?;
    let s = run.tick.checked_sub(at)? as f32 / crate::rules::HZ as f32;
    (s < BEAT && stands_here(run)).then_some(s)
}

fn smooth(lo: f32, hi: f32, s: f32) -> f32 {
    let t = ((s - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How dark the beat holds the view at second `s`: black over the cut,
/// lifting by the first second.
pub fn beat_dark(s: f32) -> f32 {
    1.0 - smooth(0.2, 0.9, s)
}

/// How close the camera leans in on the party at second `s`, easing back
/// to the room's own view by the beat's end.
pub fn beat_zoom(s: f32) -> f32 {
    1.0 + 0.45 * (1.0 - smooth(0.0, BEAT * 0.9, s))
}

/// How far the room's lamps have come up at second `s`: 0, none; 1, all.
pub fn beat_rise(s: f32) -> f32 {
    smooth(0.35, BEAT * 0.8, s)
}

/// A lamp `near` (0 nearest the party, 1 farthest) lit as far as the beat's
/// rise `rise` has come: each lamp comes up over a quarter of the rise,
/// the nearest first.
pub fn lamp_up(rise: f32, near: f32) -> f32 {
    let start = near.clamp(0.0, 1.0) * 0.75;
    ((rise - start) / 0.25).clamp(0.0, 1.0)
}

/// Room `room`'s item (on a floor of `pack`), when rooms are drawn in 3D
/// with the painted plates (`ANGEL_WORLD3D_PLATES=1`) and it has one
/// (`assets::item_for`). Lit rooms need none.
pub fn item(room: &Room, pack: Pack) -> Option<Fit> {
    if enabled() && !super::rooms_lit::on() { assets::item_for(room, pack) } else { None }
}

/// A room drawn in 3D: what it is drawn from, and the room's size in its
/// own pixels, which the world's stage is drawn over (a room's set is drawn
/// on a stage of the room's shape: its whole stage is the whole room).
#[derive(Clone, Debug)]
pub struct RoomWorld {
    pub look: Look,
    pub room: (i32, i32),
    /// A lit room's moment: how it burns, where the knight is, its clock.
    pub moment: Moment,
}

/// What a lit room's look takes from the run each frame (`world_in`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    /// Its fires: full while it is fought, embers once it is won.
    pub activity: Activity,
    /// The knight on show's chest, in the room's pixels (x, height, y):
    /// what stands in the way of it is opened.
    pub chest: Option<[f32; 3]>,
    /// The run's clock, in seconds.
    pub time: f32,
    /// How far its lamps have come up (`beat_rise`): 1 but in the beat as
    /// the party first comes down into the deep.
    pub rise: f32,
}

impl Default for Moment {
    fn default() -> Self {
        Self { activity: Activity::Steady, chest: None, time: 0.0, rise: 1.0 }
    }
}

/// What a room in 3D is drawn from.
#[derive(Clone, Debug)]
pub enum Look {
    /// Its item, and how the room differs from the item's own
    /// (`fit::Variant`): the item's plate cast on its blocks.
    Painted { item: Arc<Item>, variant: Option<Arc<Variant>> },
    /// No item: its own blockout in its theme's patches (`plain`).
    Plain(Arc<Plain>),
    /// Lit, in the monolith look, no plate (`rooms_lit`).
    Lit(Arc<Lit>),
}

impl RoomWorld {
    /// Item `item`'s world over a room `room` pixels big, laid as the item's.
    pub fn of(item: Arc<Item>, room: (i32, i32)) -> RoomWorld {
        RoomWorld::painted(item, None, room)
    }

    /// Item `item`'s world over a room `room` pixels big, laid as `variant` says.
    pub fn painted(item: Arc<Item>, variant: Option<Arc<Variant>>, room: (i32, i32)) -> RoomWorld {
        RoomWorld { look: Look::Painted { item, variant }, room, moment: Moment::default() }
    }

    /// Lit room `lit`'s world over a room `room` pixels big.
    pub fn lit_world(lit: Arc<Lit>, room: (i32, i32)) -> RoomWorld {
        RoomWorld { look: Look::Lit(lit), room, moment: Moment::default() }
    }

    /// The room lit, when it is.
    pub fn lit(&self) -> Option<&Arc<Lit>> {
        match &self.look {
            Look::Lit(lit) => Some(lit),
            _ => None,
        }
    }

    /// Its item, when it is painted.
    pub fn item(&self) -> Option<&Arc<Item>> {
        match &self.look {
            Look::Painted { item, .. } => Some(item),
            Look::Plain(_) | Look::Lit(_) => None,
        }
    }

    /// How the room differs from its item's, when it is painted and does.
    pub fn variant(&self) -> Option<&Arc<Variant>> {
        match &self.look {
            Look::Painted { variant, .. } => variant.as_ref(),
            Look::Plain(_) | Look::Lit(_) => None,
        }
    }

    /// The room with no item, when it is plain.
    pub fn plain(&self) -> Option<&Arc<Plain>> {
        match &self.look {
            Look::Plain(plain) => Some(plain),
            Look::Painted { .. } | Look::Lit(_) => None,
        }
    }

    /// The number its blocks and plate are kept under: the item's, or the
    /// variant's when the room differs; a plain room's own.
    pub fn key(&self) -> u64 {
        match &self.look {
            Look::Painted { item, variant } => variant.as_ref().map_or(item.key, |v| v.key),
            Look::Plain(plain) => plain.key,
            Look::Lit(lit) => lit.key,
        }
    }

    /// The room's pixels to a pixel of the set's stage.
    pub fn scale(&self) -> f32 {
        let stage = match &self.look {
            Look::Painted { item, .. } => item.set.camera.size().0 as f32,
            Look::Plain(plain) => plain.stage().0,
            Look::Lit(lit) => lit.stage().0,
        };
        self.room.0.max(1) as f32 / stage.max(1.0)
    }

    /// The set its blocks are made from, when it is painted: the item's, or,
    /// where the room differs from the item's, the item's with what stands
    /// there cut away.
    pub fn set(&self) -> Option<&Set> {
        match &self.look {
            Look::Painted { item, variant } => Some(variant.as_ref().map_or(&item.set, |v| &v.set)),
            Look::Plain(_) | Look::Lit(_) => None,
        }
    }

    /// The set's blocks on a stage that shows the room from `camera` (the
    /// view's top-left on the room, in the room's pixels), its plate as
    /// picture `plate`, leaning in by `zoom` toward `focus` (stage pixels)
    /// as a guardian's entrance leans the camera in: what lies on the floor,
    /// and what stands on it. A surface the plate does not see is its kind's
    /// patch, the item's patches being picture `atlas` (`Item::patches`).
    /// None for a plain room ([`RoomWorld::plain_nodes`]).
    pub fn nodes(&self, plate: u32, atlas: u32, camera: (f32, f32), lean: ([f32; 2], f32)) -> (Vec<Node>, Vec<Node>) {
        let Look::Painted { item, variant } = &self.look else {
            return (Vec::new(), Vec::new());
        };
        let (scale, offset) = self.placing(camera, lean);
        let set = variant.as_ref().map_or(&item.set, |v| &v.set);
        let patches = item.patches(atlas);
        let kept = variant.as_ref().map_or(&item.made, |v| &v.made);
        let made = kept.get_or_init(|| {
            let (floor, blocks) = set.world_nodes(plate, patches);
            ((plate, patches), floor, blocks)
        });
        if made.0 != (plate, patches) {
            return set.placed_world(plate, patches, scale, offset);
        }
        (place(&made.1, scale, offset), place(&made.2, scale, offset))
    }


    /// What the room has that the item's set has not (`fit::Variant`), as
    /// blocks on the same stage, each of its kind's patch of the plate, the
    /// patch of kind `k` being picture `swatch(k)`: none where the room is
    /// laid as the item's.
    pub fn extras(&self, swatch: &dyn Fn(Kind) -> u32, camera: (f32, f32), lean: ([f32; 2], f32)) -> Vec<Node> {
        let Look::Painted { item, variant: Some(variant) } = &self.look else {
            return Vec::new();
        };
        let (scale, offset) = self.placing(camera, lean);
        place(variant.extra_nodes(item, swatch), scale, offset)
    }

    /// A plain room's blocks on the stage as [`RoomWorld::nodes`] places a
    /// painted one's, each of its kind's patch of its theme's, the patch of
    /// kind `k` being picture `swatch(k)`: what lies on the floor, and what
    /// stands on it, of the pieces near a view `view` pixels big (an open
    /// floor's; a room's one piece whatever the view) that `seen` says are
    /// in sight (the box each piece's blocks fill, on the stage). None for a
    /// painted room.
    pub fn plain_nodes(
        &self,
        swatch: &dyn Fn(Kind) -> u32,
        camera: (f32, f32),
        lean: ([f32; 2], f32),
        view: (f32, f32),
        seen: &dyn Fn([f32; 3], [f32; 3]) -> bool,
    ) -> (Vec<Node>, Vec<Node>) {
        let Look::Plain(plain) = &self.look else {
            return (Vec::new(), Vec::new());
        };
        let (scale, offset) = self.placing(camera, lean);
        // The view on the room's stage: its pixels over the room's to a stage pixel.
        let k = self.scale().max(1e-6);
        let rect = [camera.0 / k, camera.1 / k, (camera.0 + view.0) / k, (camera.1 + view.1) / k];
        let (mut floor, mut standing) = (Vec::new(), Vec::new());
        for piece in plain.near(rect) {
            let (lo, hi) = piece.bounds;
            let at = |p: [f32; 3]| [p[0] * scale + offset[0], p[1] * scale + offset[1], p[2] * scale + offset[2]];
            if !seen(at(lo), at(hi)) {
                continue;
            }
            let blocks = plain.blocks(&piece, swatch);
            floor.extend(place(&blocks.floor, scale, offset));
            standing.extend(place(&blocks.standing, scale, offset));
        }
        (floor, standing)
    }

    /// A lit room on the stage as [`RoomWorld::nodes`] places a painted one
    /// (`rooms_lit::Lit::staged`): the stage `view` pixels big, showing the
    /// room from `camera`, leaning in by `lean`, only the pieces `seen` says
    /// are in sight; with its look for this moment, `sparks` (what burns in
    /// flight, in the room's pixels) lighting it too. `None` unless it is lit.
    pub fn lit_staged(
        &self,
        camera: (f32, f32),
        lean: ([f32; 2], f32),
        view: (f32, f32),
        sparks: &[angel_stage::look::Lamp],
        seen: &dyn Fn([f32; 3], [f32; 3]) -> bool,
    ) -> Option<super::rooms_lit::Staged> {
        let Look::Lit(lit) = &self.look else {
            return None;
        };
        let k = self.scale().max(1e-6);
        let rect = [camera.0 / k, camera.1 / k, (camera.0 + view.0) / k, (camera.1 + view.1) / k];
        Some(lit.staged(self.placing(camera, lean), lean.1, view, rect, &self.moment, sparks, seen))
    }

    /// The same blocks by reference, for a page that makes them itself from
    /// the set (`angel_stage::set::Sets`): sent with the set's text when
    /// `with_text` (the page may not have it yet), by its key alone after;
    /// and the plate as its PNG when `with_png` (the page has not been sent
    /// this item's plate); its patches as `nodes` has them. None for a plain
    /// room, whose blocks are no set's.
    pub fn set_ref(
        &self,
        plate: u32,
        atlas: u32,
        camera: (f32, f32),
        lean: ([f32; 2], f32),
        with_text: bool,
        with_png: bool,
    ) -> Option<SetRef> {
        let Look::Painted { item, variant } = &self.look else {
            return None;
        };
        let (scale, offset) = self.placing(camera, lean);
        let text = variant.as_ref().map_or(&item.text, |v| &v.text);
        Some(SetRef {
            key: self.key(),
            text: with_text.then(|| Arc::clone(text)),
            plate,
            png: with_png.then(|| Arc::clone(&item.png)),
            scale,
            offset,
            patches: item.patches(atlas),
        })
    }

    /// How the set's world is placed on the stage (`Set::placed_nodes`).
    pub fn placing(&self, camera: (f32, f32), (focus, zoom): ([f32; 2], f32)) -> (f32, [f32; 3]) {
        // A room pixel `p` is at `p - camera` on the stage, then grows about
        // the focus: `focus + (k p - camera - focus) zoom`.
        let offset = [focus[0] * (1.0 - zoom) - camera.0 * zoom, 0.0, focus[1] * (1.0 - zoom) - camera.1 * zoom];
        (self.scale() * zoom, offset)
    }
}

impl RoomWorld {
    /// The plate as the room's still art, for a page that draws no scenes
    /// (a terminal, a page with no 3D): the plate laid back down on the
    /// floor it was painted over. Each of the room's pixels is the plate
    /// averaged over the area its patch of floor is cast from, so a figure
    /// drawn flat at its place stands where the window stands it, and a
    /// patch of floor the plate shows a wall or a roof in front of is that
    /// wall or roof (as the room's painted art has always drawn a building
    /// up from its foot). Kept: drawn once a room and size. `None` when the
    /// plate does not decode.
    ///
    /// A room of the world is drawn in true colour, never snapped to the
    /// realm's palette (only a delivered dungeon kit's frame is), so neither
    /// is its plate.
    pub fn picture(&self) -> Option<Img> {
        if let Look::Plain(plain) = &self.look {
            return Some(plain.picture(self.room));
        }
        if let Look::Lit(lit) = &self.look {
            return Some(lit.picture(self.room));
        }
        type Kept = Option<((u64, (i32, i32)), Arc<Img>)>;
        static KEPT: Mutex<Kept> = Mutex::new(None);
        let key = (self.key(), self.room);
        if let Ok(kept) = KEPT.lock()
            && let Some((k, img)) = kept.as_ref()
            && *k == key
        {
            return Some((**img).clone());
        }
        let img = Arc::new(self.laid_down()?);
        if let Ok(mut kept) = KEPT.lock() {
            *kept = Some((key, Arc::clone(&img)));
        }
        Some(Arc::unwrap_or_clone(img))
    }

    fn laid_down(&self) -> Option<Img> {
        let Look::Painted { item, variant } = &self.look else {
            return None;
        };
        let (w, h) = (self.room.0.max(0), self.room.1.max(0));
        let plate = item.plate()?;
        let (pw, ph) = (plate.width as i32, plate.height as i32);
        let m = item.set.projector().view_projection;
        let to_set = 1.0 / self.scale();
        // Where the floor under room point (x, y) is cast from, in plate pixels.
        let cast = |x: f32, y: f32| {
            let p = [x * to_set, 0.0, y * to_set];
            let row = |r: usize| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r];
            let q = row(3).max(1e-6);
            [(row(0) / q + 1.0) * 0.5 * pw as f32, (1.0 - row(1) / q) * 0.5 * ph as f32]
        };
        // Each row's corners once: the floor's row y cast to plate rows, its columns to plate columns.
        let corners: Vec<Vec<[f32; 2]>> =
            (0..=h).map(|y| (0..=w).map(|x| cast(x as f32, y as f32)).collect()).collect();
        let mut pixels = Vec::with_capacity((w * h) as usize);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let quad = [corners[y][x], corners[y][x + 1], corners[y + 1][x], corners[y + 1][x + 1]];
                let lo = |k: usize| quad.iter().map(|c| c[k]).fold(f32::INFINITY, f32::min);
                let hi = |k: usize| quad.iter().map(|c| c[k]).fold(f32::NEG_INFINITY, f32::max);
                let (x0, x1) = ((lo(0).floor() as i32).max(0), (hi(0).ceil() as i32).min(pw));
                let (y0, y1) = ((lo(1).floor() as i32).max(0), (hi(1).ceil() as i32).min(ph));
                let mut sum = [0u32; 3];
                let mut n = 0u32;
                for py in y0..y1 {
                    for px in x0..x1 {
                        let i = ((py * pw + px) * 4) as usize;
                        // A clear texel is the black paper under the plate.
                        let a = u32::from(plate.rgba[i + 3]);
                        for (k, total) in sum.iter_mut().enumerate() {
                            *total += u32::from(plate.rgba[i + k]) * a / 255;
                        }
                        n += 1;
                    }
                }
                pixels.push(if n == 0 { [0, 0, 0] } else { sum.map(|v| ((v + n / 2) / n) as u8) });
            }
        }
        // Where the room differs from the item's, what stands there instead
        // (a doorway walled up, a boulder rolled in): its kind's patch,
        // tiled mirrored at the plate's density, shaded as its top.
        if let Some(variant) = variant {
            let k = self.scale();
            let tile = super::blockout::TILE * k;
            let (sw, sh) = item.set.camera.size();
            let density = (1.0 - 2.0 * angel_stage::set::PLATE_MARGIN) * (pw as f32 / sw.max(1) as f32).min(ph as f32 / sh.max(1) as f32);
            for &(c, r) in &variant.differs {
                let Some(shape) = variant.over_tile((c, r)) else { continue };
                let swatch = item.swatch(shape.kind);
                let flat = shape.kind.capture();
                let (x0, y0) = ((c as f32 * tile).round() as i32, (r as f32 * tile).round() as i32);
                let (x1, y1) = ((((c + 1) as f32) * tile).round() as i32, (((r + 1) as f32) * tile).round() as i32);
                for y in y0.max(0)..y1.min(h) {
                    for x in x0.max(0)..x1.min(w) {
                        let rgb = match swatch {
                            Some(s) => {
                                // The room pixel in plate pixels, mirrored into the patch.
                                let mirrored = |v: f32, n: u32| {
                                    let n = n.max(1) as i32;
                                    let t = (v.floor() as i32).rem_euclid(2 * n);
                                    if t < n { t } else { 2 * n - 1 - t }
                                };
                                let px = mirrored(x as f32 / k * density, s.width);
                                let py = mirrored(y as f32 / k * density, s.height);
                                let i = ((py as u32 * s.width + px as u32) * 4) as usize;
                                [s.rgba[i], s.rgba[i + 1], s.rgba[i + 2]]
                            }
                            None => flat,
                        };
                        pixels[(y * w + x) as usize] = rgb;
                    }
                }
            }
        }
        Some(Img::from_rgb(w, h, &pixels))
    }
}

/// Room `room`'s world (on floor `depth`, of `pack`), `size` pixels big,
/// when it stands up in 3D ([`stands`]): lit (`rooms_lit`), or its item's when it has one, else its own blockout
/// in its theme's patches (`plain`) when the theme has any; `None` for a
/// room never drawn so (the kingdom, a side-on hall).
pub fn world(room: &Room, pack: Pack, depth: u32, size: (i32, i32)) -> Option<RoomWorld> {
    if !stands(depth, room.kind) {
        return None;
    }
    if super::rooms_lit::on() {
        return super::rooms_lit::lit_for(room, pack).map(|lit| RoomWorld::lit_world(lit, size));
    }
    if let Some(Fit { item, variant }) = item(room, pack) {
        return Some(RoomWorld::painted(item, variant, size));
    }
    if !enabled() || !super::plain::on() {
        return None;
    }
    super::plain::plain_for(room, pack).map(|plain| RoomWorld { look: Look::Plain(plain), room: size, moment: Moment::default() })
}

/// Run `run`'s room's world as `world` has it, `size` pixels big, with the
/// run's moment for a lit room: its fires full while the room is fought,
/// embers once a fight room is won, steady elsewhere; the chest of the knight
/// on show (seat `focus`'s, else the first standing); its clock.
pub fn world_in(run: &crate::rules::Run, focus: Option<u32>, size: (i32, i32)) -> Option<RoomWorld> {
    let mut world = world(run.room(), run.dungeon.pack, run.dungeon.depth, size)?;
    if world.lit().is_some() {
        world.moment = moment_of(run, focus);
    }
    Some(world)
}

/// Run `run`'s moment (`world_in`).
pub fn moment_of(run: &crate::rules::Run, focus: Option<u32>) -> Moment {
    use crate::render::overworld::arena::UNIT;
    use crate::rules::RoomKind;
    let room = run.room();
    let activity = if run.enemies.iter().any(|e| e.hp > 0) {
        Activity::Fought
    } else if room.cleared && matches!(room.kind, RoomKind::Fight | RoomKind::Lair | RoomKind::Pit) {
        Activity::Cleared
    } else {
        Activity::Steady
    };
    let standing = |h: &&crate::rules::Hero| h.hp > 0;
    let hero = focus.and_then(|id| run.players.get(&id)).filter(standing).or_else(|| run.players.values().find(standing));
    Moment {
        activity,
        chest: hero.map(|h| [h.x * UNIT, super::rooms_lit_look::CHEST, h.y * UNIT]),
        time: run.tick as f32 / crate::rules::HZ as f32,
        rise: beat(run).map_or(1.0, beat_rise),
    }
}

/// A proof room (docs/WORLD-3D.md, "Proof shots"): item `id`'s room as a run
/// stands in it, its own people and props where the game puts them, a knight
/// half behind the tallest thing that stands in the room with open floor
/// behind it and one just in front of something that stands, and an ember in
/// flight past the near one.
#[cfg(any(test, feature = "testing"))]
pub struct Proof {
    pub id: &'static str,
    pub run: crate::rules::Run,
    /// Where the two knights' feet are, in the room's pixels.
    pub behind: [f32; 2],
    pub in_front: [f32; 2],
}

/// Item `id`'s proof room, if it is an item.
#[cfg(any(test, feature = "testing"))]
pub fn proof(id: &str) -> Option<Proof> {
    use super::pilot::Source;
    use crate::render::overworld::arena::UNIT;
    use crate::rules::{Rng, Run, RoomKind, home::Home, layout, rooms, world};
    let pilot = super::pilot::item(id)?;
    // The item's room as the game lays it, and what stands in it: its lit
    // set (no plate needed).
    let template = pilot.room();
    let shapes = angel_stage::set::Set::parse(&super::rooms_lit::room_text(&template, &pilot.dressing, true, None)).ok()?.shapes;
    // A floor of the item's delve (the Crypt for a room of none), its first
    // fight room laid as the item's room where it lies.
    let laid_as_item = || {
        let pack = match pilot.dressing.setting {
            super::blockout::Setting::Dungeon(pack) => pack,
            _ => crate::rules::Pack::Crypt,
        };
        let mut run = Run::new(0x3D, 0x3D, Some("Friend"));
        let mut floor = layout::floor(1, pack, &mut Rng::new(0x3D));
        let at = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight)?;
        let was = &floor.rooms[at];
        let roster = if template.kind == RoomKind::Fight { was.roster.clone() } else { Vec::new() };
        let mut room = template.clone();
        (room.cell, room.roster) = (was.cell, roster);
        floor.rooms[at] = room;
        run.dungeon = floor;
        run.enter_for_test(at);
        Some(run)
    };
    let mut run = match pilot.source {
        Source::Place(kind) => {
            let mut run = Run::at_home(0x3D, 0x3D, Some("Friend"), Home::default(), Default::default());
            run.join_seat(2, "Goose Inspector");
            match world::room_of(&run.dungeon, kind) {
                Some(at) if super::fit::same_tiles(&template, &run.dungeon.rooms[at]) => {
                    run.arrive(at, (12.0, 9.5));
                    run
                }
                // A place the home floor does not raise as the item was made
                // (Maud's tavern is dug out): laid where a fight room was.
                _ => laid_as_item()?,
            }
        }
        Source::Great { .. } | Source::Nook(_) | Source::Sanctuary | Source::Vault | Source::Forge => laid_as_item()?,
        Source::Chamber { pack, small, doors } => {
            // A floor of its delve, one of whose fight rooms is laid as the chamber.
            let mut run = Run::new(0x3D, 0x3D, Some("Friend"));
            let mut floor = layout::floor(1, pack, &mut Rng::new(0x3D));
            let size = (template.cols, template.rows);
            let at = floor.rooms.iter().position(|r| r.kind == RoomKind::Fight && (r.cols, r.rows) == size)?;
            let quadrant: Vec<Vec<u8>> = rooms::small(pack)[small].iter().map(|r| r.as_bytes().to_vec()).collect();
            floor.rooms[at].doors = doors;
            layout::mirror(&mut floor.rooms[at], doors, &quadrant);
            run.dungeon = floor;
            run.enter_for_test(at);
            run
        }
    };
    let (behind, in_front) = spots(&template, &shapes);
    let ids: Vec<u32> = run.players.keys().copied().collect();
    for (id, [x, y]) in ids.into_iter().zip([in_front, behind]) {
        let hero = run.players.get_mut(&id).expect("a seat");
        (hero.x, hero.y) = (x / UNIT, y / UNIT);
    }
    let [x, y] = in_front;
    let shot: crate::rules::Projectile = serde_json::from_value(serde_json::json!({
        "x": (x + 26.0) / UNIT, "y": (y - 6.0) / UNIT, "vx": 9.0, "vy": 0.0, "hostile": false,
        "kind": "ember", "look": null, "damage": 1, "pierce": 0, "ttl": 30
    }))
    .expect("an ember");
    run.projectiles.push(shot);
    Some(Proof { id: pilot.id(), run, behind, in_front })
}

/// Where a proof's knights stand on room `room`'s floor, `shapes` standing
/// in it (room pixels): one
/// on the walkable tile just behind (north of) the tallest block inside the
/// room, its feet near the tile's foot, so the block hides its legs; and
/// one on the walkable tile just in front of (south of) one, so it stands
/// over it. Of equals, the one nearest the room's middle.
#[cfg(any(test, feature = "testing"))]
fn spots(room: &Room, shapes: &[angel_stage::set::Shape]) -> ([f32; 2], [f32; 2]) {
    use super::blockout::{TILE, walkable};
    let k = 1.0;
    // How tall what stands over tile (c, r)'s middle is, in room pixels.
    let height = |c: i32, r: i32| -> f32 {
        let (x, z) = ((c as f32 + 0.5) * TILE / k, (r as f32 + 0.5) * TILE / k);
        shapes
            .iter()
            .filter(|s| !s.on_floor())
            .filter(|s| {
                let quarter = (s.turn.rem_euclid(180.0) - 90.0).abs() < 45.0;
                let (sx, sz) = if quarter { (s.size[2], s.size[0]) } else { (s.size[0], s.size[2]) };
                let (dx, dz) = ((x - s.at[0]) / (sx / 2.0).max(1e-3), (z - s.at[2]) / (sz / 2.0).max(1e-3));
                // A round thing (a crown, a cone) covers its round footprint.
                let round = ["sphere", "cone", "blob", "rock", "cylinder"].iter().any(|r| s.shape.starts_with(r));
                if round { dx * dx + dz * dz <= 0.8 } else { dx.abs() <= 1.0 && dz.abs() <= 1.0 }
            })
            .map(|s| (s.at[1] + s.size[1]) * k)
            .fold(0.0, f32::max)
    };
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let open = |c: i32, r: i32| walkable(room.tile(c, r)) && height(c, r) <= super::blockout::LOW;
    let mid = |c: i32, r: i32| ((c - cols / 2) * (c - cols / 2) + (r - rows / 2) * (r - rows / 2)) as f32;
    let best = |pick: &dyn Fn(i32, i32) -> Option<f32>| {
        (1..rows - 1)
            .flat_map(|r| (1..cols - 1).map(move |c| (c, r)))
            .filter_map(|(c, r)| pick(c, r).map(|h| (c, r, h)))
            .max_by(|a, b| a.2.total_cmp(&b.2).then(mid(b.0, b.1).total_cmp(&mid(a.0, a.1))))
    };
    // Behind: open floor, a tall block (not a wall) just south of it.
    let behind = best(&|c, r| (open(c, r) && r + 1 < rows - 1 && !open(c, r + 1)).then(|| height(c, r + 1)))
        .map_or([cols as f32 * TILE / 2.0, rows as f32 * TILE / 2.0], |(c, r, _)| {
            [(c as f32 + 0.5) * TILE, (r + 1) as f32 * TILE - 2.0]
        });
    // In front: open floor, a block just north of it, not the far wall, and
    // clear of the knight behind.
    let (bc, br) = ((behind[0] / TILE) as i32, (behind[1] / TILE) as i32);
    let clear = |c: i32, r: i32| (c - bc).abs() > 3 || (r - br).abs() > 3;
    let in_front = best(&|c, r| (open(c, r) && r - 1 >= 1 && !open(c, r - 1) && clear(c, r)).then(|| height(c, r - 1).min(24.0)))
        .map_or([cols as f32 * TILE / 2.0 + 24.0, rows as f32 * TILE / 2.0], |(c, r, _)| {
            [(c as f32 + 0.5) * TILE, r as f32 * TILE + 10.0]
        });
    (behind, in_front)
}

/// Item `id`'s proof room laid otherwise than the item, as the dice may lay
/// it (`fit::differing`): its east doorway walled up (its west, when it has
/// no east one) and, in a delve that scatters loose blocks, one boulder
/// rolled in, mirrored four ways as the dice lay it, on open floor clear of
/// the knights. `None` for a place of the kingdom (laid only as it is) or a
/// room with no doorway to wall up.
#[cfg(any(test, feature = "testing"))]
pub fn proof_variant(id: &str) -> Option<Proof> {
    use super::blockout::TILE;
    use crate::render::overworld::arena::UNIT;
    use crate::rules::Tile;
    if matches!(super::pilot::item(id)?.source, super::pilot::Source::Place(_)) {
        return None;
    }
    let mut proof = proof(id)?;
    let pack = proof.run.dungeon.pack;
    let feet: Vec<(i32, i32)> = proof.run.players.values().map(|h| ((h.x * UNIT / TILE) as i32, (h.y * UNIT / TILE) as i32)).collect();
    let at = proof.run.at;
    let room = &mut proof.run.dungeon.rooms[at];
    let (cols, rows) = (room.cols as i32, room.rows as i32);
    let side = [1, 3].into_iter().find(|&d| room.doors[d])?;
    let col = if side == 1 { cols - 1 } else { 0 };
    for row in 0..rows {
        if room.tile(col, row) == Tile::Door {
            room.set(col as usize, row as usize, Tile::Wall);
        }
    }
    room.doors[side] = false;
    if pack.scatter() > 0 {
        let clear = |c: i32, r: i32| feet.iter().all(|&(x, y)| (x - c).abs() > 2 || (y - r).abs() > 2);
        let mirrored = |c: i32, r: i32| [(c, r), (cols - 1 - c, r), (c, rows - 1 - r), (cols - 1 - c, rows - 1 - r)];
        let spot = (2..rows / 2 - 1).flat_map(|r| (2..cols / 2 - 2).map(move |c| (c, r))).find(|&(c, r)| {
            mirrored(c, r).iter().all(|&(x, y)| room.tile(x, y) == Tile::Floor && clear(x, y))
        });
        if let Some((c, r)) = spot {
            for (x, y) in mirrored(c, r) {
                room.set(x as usize, y as usize, Tile::Block);
            }
        }
    }
    Some(proof)
}

/// A run one door away from chamber item `id`'s room, its own room clear
/// and its knights by the door that leads in, and which way that is (0
/// north, 1 east, 2 south, 3 west): a browser check walks a knight in.
/// `None` for a place of the kingdom, or a chamber whose floor joins it to
/// nothing.
#[cfg(any(test, feature = "testing"))]
pub fn approach(id: &str) -> Option<(crate::rules::Run, usize)> {
    use crate::render::overworld::arena::UNIT;
    use super::blockout::TILE;
    if !matches!(super::pilot::item(id)?.source, super::pilot::Source::Chamber { .. }) {
        return None;
    }
    let mut run = proof(id)?.run;
    // One knight, so a seat walking it through the door goes alone.
    let first = *run.players.keys().next()?;
    run.players.retain(|&id, _| id == first);
    let at = run.at;
    let (side, next) = (0..4).find_map(|side| {
        let next = run.dungeon.neighbour(at, side)?;
        run.dungeon.rooms[next].doors[(side + 2) % 4].then_some((side, next))
    })?;
    let room = &mut run.dungeon.rooms[next];
    (room.visited, room.cleared) = (true, true);
    room.roster.clear();
    run.enter_for_test(next);
    run.enemies.clear();
    run.projectiles.clear();
    // By the door back toward the chamber, two tiles in.
    let toward = (side + 2) % 4;
    let room = run.room();
    let (w, h) = (room.cols as f32 * TILE, room.rows as f32 * TILE);
    let by = match toward {
        0 => [w / 2.0, 2.5 * TILE],
        1 => [w - 2.5 * TILE, h / 2.0],
        2 => [w / 2.0, h - 2.5 * TILE],
        _ => [2.5 * TILE, h / 2.0],
    };
    for (k, hero) in run.players.values_mut().enumerate() {
        let across = if toward % 2 == 0 { [k as f32 * 20.0 - 10.0, 0.0] } else { [0.0, k as f32 * 20.0 - 10.0] };
        (hero.x, hero.y) = ((by[0] + across[0]) / UNIT, (by[1] + across[1]) / UNIT);
    }
    Some((run, toward))
}

/// The Village's proof room (`proof`), as the tests and the proof shots
/// have stood it since the first.
#[cfg(any(test, feature = "testing"))]
pub fn proof_run() -> crate::rules::Run {
    proof("village").expect("the Village is an item").run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RoomKind;
    use angel_stage::set::Kind;

    fn village() -> Arc<Item> {
        assets::item("village").expect("the Village reads")
    }

    /// The Village as a run has it.
    fn village_room() -> Room {
        use crate::rules::{Run, home::Home, world as places};
        let run = Run::at_home(0xD3D, 0xD3D, Some("Friend"), Home::default(), Default::default());
        run.dungeon.rooms[places::room_of(&run.dungeon, RoomKind::Village).unwrap()].clone()
    }

    #[test]
    fn off_unless_turned_on() {
        // The painted path (`ANGEL_WORLD3D_PLATES=1`).
        super::super::rooms_lit::set_plates(true);
        let room = village_room();
        let pack = Pack::Crypt;
        assert!(!enabled());
        assert!(item(&room, pack).is_none(), "off: no room has its item");
        set_enabled(true);
        assert_eq!(item(&room, pack).map(|f| f.item.id), Some("village"));
        let mut other = room.clone();
        other.kind = RoomKind::Fight;
        assert!(item(&other, pack).is_none());
        set_enabled(false);
        assert!(item(&room, pack).is_none());
    }

    /// The picture the item's patches are, as a window names one.
    const ATLAS: u32 = 931;

    #[test]
    fn a_rooms_world_is_its_set_over_the_whole_room() {
        let item = village();
        let world = RoomWorld::of(Arc::clone(&item), (768, 448));
        assert_eq!(world.scale(), 2.0, "the room's pixels twice the set's");
        let world = RoomWorld::of(Arc::clone(&item), (384, 224));
        assert_eq!(world.scale(), 1.0, "a set's stage is its room's pixels");
        let (floor, blocks) = world.nodes(7, ATLAS, (0.0, 0.0), ([0.0, 0.0], 1.0));
        assert_eq!(floor.len() + blocks.len(), item.set.shapes.len());
        assert!(floor.len() >= 3, "the grass and the roads lie on the floor");
        assert!(
            blocks.iter().all(|n| n.transform.translation[1] + n.transform.scale[1].abs() / 2.0 > 0.0),
            "all of it stands over the floor line"
        );
        // A cottage's walls stand where the set says, in the room's pixels.
        let (at, walls) = item
            .set
            .shapes
            .iter()
            .enumerate()
            .find(|(_, s)| s.shape == "box" && s.kind == Kind::Stone && s.turn == 0.0 && !s.on_floor())
            .expect("a stone cottage");
        let id = at as u64 + 1;
        let node = blocks.iter().find(|n| n.id == id).unwrap();
        let [x, y, z] = walls.at;
        assert_eq!(node.transform.translation, [x, y + walls.size[1] / 2.0, z]);
        // Scrolled, everything moves the other way; leaning in, it grows about the focus.
        let (_, scrolled) = world.nodes(7, ATLAS, (10.0, -4.0), ([0.0, 0.0], 1.0));
        assert_eq!(scrolled.iter().find(|n| n.id == id).unwrap().transform.translation, [x - 10.0, y + walls.size[1] / 2.0, z + 4.0]);
        let (_, leaning) = world.nodes(7, ATLAS, (0.0, 0.0), ([x, z], 2.0));
        let lean = leaning.iter().find(|n| n.id == id).unwrap().transform;
        assert_eq!((lean.translation, lean.scale[1]), ([x, 2.0 * (y + walls.size[1] / 2.0), z], node.transform.scale[1] * 2.0));
        // By reference, a far side makes the same blocks again.
        let mut sets = angel_stage::set::Sets::default();
        let mut far = angel_stage::scene::World { set: world.set_ref(7, ATLAS, (10.0, -4.0), ([60.0, 20.0], 1.5), true, true), ..Default::default() };
        assert!(sets.fill(&mut far));
        assert_eq!((far.floor, far.blocks), world.nodes(7, ATLAS, (10.0, -4.0), ([60.0, 20.0], 1.5)));
        let bare = world.set_ref(7, ATLAS, (0.0, 0.0), ([0.0, 0.0], 1.0), false, false).unwrap();
        assert_eq!((bare.text, bare.png), (None, None), "by its key alone");
        let sent = world.set_ref(7, ATLAS, (0.0, 0.0), ([0.0, 0.0], 1.0), false, true).unwrap().png.expect("the plate");
        assert_eq!(&sent[..], &item.png[..], "the plate goes as the PNG it is read as");
        // A surface the plate does not see is its kind's patch, where the item has patches.
        if !item.swatches().is_empty() {
            assert_eq!(bare.patches, item.patches(ATLAS));
            for (node, shape) in floor.iter().chain(&blocks).filter(|n| n.id <= item.set.shapes.len() as u64).map(|n| (n, &item.set.shapes[n.id as usize - 1])) {
                let want = item.swatch(shape.kind).map(|s| {
                    let (x, y) = angel_stage::set::swatch_cell(s.kind);
                    (ATLAS, [x, y, s.width, s.height])
                });
                assert_eq!(node.material.patch.map(|p| (p.picture, p.rect)), want, "{:?}", shape.kind);
            }
        }
    }

    /// The plate's texel that room point `(x, y)` on the floor is cast from.
    fn texel(world: &RoomWorld, x: f32, y: f32) -> [u8; 3] {
        let item = world.item().unwrap();
        let uv = item.set.projector().uv([x / world.scale(), 0.0, y / world.scale()]).unwrap();
        let plate = item.plate().unwrap();
        let (px, py) = ((uv[0] * plate.width as f32) as u32, (uv[1] * plate.height as f32) as u32);
        let i = ((py * plate.width + px) * 4) as usize;
        [plate.rgba[i], plate.rgba[i + 1], plate.rgba[i + 2]]
    }

    #[test]
    fn a_terminal_has_the_plate_laid_back_down_on_the_floor() {
        let world = RoomWorld::of(village(), (384, 224));
        let img = world.picture().expect("the plate decodes");
        assert_eq!((img.w, img.h), (384, 224));
        assert!(img.whole());
        // Where the floor is open, each pixel is near the texel it is cast
        // from (it is their average over its patch).
        for (x, y) in [(192, 112), (40, 120), (330, 120)] {
            let got = img.get(x, y).unwrap();
            let want = texel(&world, x as f32 + 0.5, y as f32 + 0.5);
            assert!((0..3).all(|k| got[k].abs_diff(want[k]) < 48), "({x}, {y}): {got:?} for {want:?}");
        }
        // Every pixel is drawn (the plate's margin holds the whole floor).
        assert!((0..224).all(|y| (0..384).all(|x| img.get(x, y).is_some())));
        // Kept: the same picture asked again.
        assert_eq!(world.picture(), Some(img));
    }

    #[test]
    fn a_terminal_room_with_an_item_shows_its_plate_and_one_without_is_as_it_was() {
        // The painted path (`ANGEL_WORLD3D_PLATES=1`).
        super::super::rooms_lit::set_plates(true);
        use crate::render::overworld::arena;
        use crate::rules::{Run, home::Home, world as places};
        let frame = |kind: RoomKind| {
            let mut run = Run::at_home(0xD3D, 0xD3D, Some("Friend"), Home::default(), Default::default());
            run.arrive(places::room_of(&run.dungeon, kind).unwrap(), (12.0, 9.5));
            arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1))
        };
        // (Each frame after another room's: a room's kept still art is keyed
        // by the room and its torch step, not by the switch, which a host
        // sets once.)
        let (village, meadow) = (frame(RoomKind::Village), frame(RoomKind::Meadow));
        set_enabled(true);
        let village3d = frame(RoomKind::Village);
        // A room with no item: the meadow, drawn from a folder with no
        // plates (whichever this checkout's asset pack has).
        let empty = std::env::temp_dir().join(format!("world3d-empty-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        let meadow3d = assets::with_folder(&empty, || frame(RoomKind::Meadow));
        set_enabled(false);
        assert_eq!(meadow3d, meadow, "a room with no item draws as it always has");
        assert_ne!(village3d, village);
        // The plate where the village had its painted lawns and roads.
        let laid = RoomWorld::of(assets::item("village").unwrap(), (384, 224)).picture().unwrap();
        assert_eq!(village3d.get(192, 112), laid.get(192, 112));
        assert_eq!(frame(RoomKind::Village), village, "and back off, as it was");
    }

    #[test]
    fn every_items_proof_room_is_its_item() {
        // The painted path (`ANGEL_WORLD3D_PLATES=1`).
        super::super::rooms_lit::set_plates(true);
        for id in assets::ids() {
            let proof = proof(id).unwrap_or_else(|| panic!("{id}: a proof room"));
            set_enabled(true);
            let got = item(proof.run.room(), proof.run.dungeon.pack).map(|f| f.item.id);
            set_enabled(false);
            assert_eq!(got, Some(id), "the proof stands in the item's own room");
            assert_ne!(proof.behind, proof.in_front);
        }
    }

    #[test]
    fn a_chambers_approach_is_one_door_from_it() {
        // The painted path (`ANGEL_WORLD3D_PLATES=1`).
        super::super::rooms_lit::set_plates(true);
        let (run, toward) = approach("crypt-bone-chapel").expect("the chapel's floor joins it to a room");
        assert!(!run.barred(), "its own room is clear");
        let chapel = run.dungeon.neighbour(run.at, toward).expect("a door that way");
        set_enabled(true);
        assert_eq!(item(&run.dungeon.rooms[chapel], run.dungeon.pack).map(|f| f.item.id), Some("crypt-bone-chapel"));
        set_enabled(false);
        assert!(approach("village").is_none());
    }

    /// `ANGEL_WORLD3D_APPROACH=<file> cargo test -p angel-delve
    /// write_world3d_approach -- --ignored`: the Bone Chapel's approach
    /// (`approach`) as a run's JSON, for `delve-dev-host --scene first
    /// --snapshot <file>` (not the default fight scene, which stands the
    /// party back at the door every tick); it prints the way in.
    #[test]
    #[ignore]
    fn write_world3d_approach() {
        let Some(file) = std::env::var_os("ANGEL_WORLD3D_APPROACH") else {
            return;
        };
        let (run, toward) = approach("crypt-bone-chapel").unwrap();
        std::fs::write(&file, serde_json::to_vec(&run).unwrap()).unwrap();
        println!("the way in: {}", ["north", "east", "south", "west"][toward]);
    }

    /// `ANGEL_WORLD3D_ROOM=<file> [ANGEL_WORLD3D_ITEM=<id>] cargo test -p
    /// angel-delve write_world3d_room -- --ignored`: item `<id>`'s proof room
    /// laid otherwise than the item (`proof_variant`; the Boulder Rill's by
    /// default) as a run's JSON, for `delve-dev-host --scene first --snapshot
    /// <file>`: a page shows the room's differences from its first frame.
    #[test]
    #[ignore]
    fn write_world3d_room() {
        let Some(file) = std::env::var_os("ANGEL_WORLD3D_ROOM") else {
            return;
        };
        let id = std::env::var("ANGEL_WORLD3D_ITEM").unwrap_or_else(|_| "cavern-boulder-rill".into());
        let proof = proof_variant(&id).unwrap_or_else(|| panic!("{id}: no room laid otherwise"));
        std::fs::write(&file, serde_json::to_vec(&proof.run).unwrap()).unwrap();
    }

    /// `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_world3d_shots
    /// -- --ignored`: each item's proof room as a terminal draws it, with
    /// rooms in 3D and without (`terminal-<item>-3d.png`,
    /// `terminal-<item>-flat.png`, three times the native size). The
    /// window's are `angel-glass`'s `delve_world` example.
    #[test]
    #[ignore]
    fn write_world3d_shots() {
        use crate::render::overworld::arena;
        let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let save = |name: &str, img: &Img| {
            let k = 3u32;
            image::RgbaImage::from_raw(img.w as u32 * k, img.h as u32 * k, img.rgba_scaled(k))
                .unwrap()
                .save(dir.join(name))
                .unwrap();
        };
        for id in assets::ids() {
            let run = proof(id).unwrap().run;
            // Another room between the two, so the room's kept still art (one
            // room's, keyed by the room and its torch step, not by the switch,
            // which a host sets once) is drawn again.
            let mut elsewhere = run.clone();
            let other = (0..run.dungeon.rooms.len()).find(|&i| i != run.at).unwrap();
            elsewhere.enter_for_test(other);
            for (on, name) in [(false, format!("terminal-{id}-flat.png")), (true, format!("terminal-{id}-3d.png"))] {
                set_enabled(on);
                let _ = arena::frame_for(&elsewhere, arena::NATIVE_W, arena::NATIVE_H, Some(1));
                save(&name, &arena::frame_for(&run, arena::NATIVE_W, arena::NATIVE_H, Some(1)));
            }
            set_enabled(false);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/world3d/room__tests.rs"]
mod depth_tests;
