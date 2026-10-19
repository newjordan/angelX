//! A Delve frame as parts, for a window that draws them itself (crates/glass;
//! docs/RENDER_FOUNDATION.md, "The game pipeline").
//!
//! The room's still art is handed over whole and unchanged between torch
//! steps, so a window uploads it once and scrolls it with the camera. The
//! knights and monsters come one by one, each with its shadow, bar and
//! marks, and with the fraction of a pixel it stands past its whole pixel,
//! so a window can place it between pixels. What is painted under them and
//! over them comes in the room's own pixels, the screen's marks in the
//! view's. A terminal does not compose these parts: `frame_for` (and
//! `frame_for_live`) paints its frame directly, and putting the parts back
//! together costs several times more at native size (0.22-0.33 ms a frame
//! against 0.05-0.21 ms, measured 2026-10-16), so `compose` serves only the
//! parity test.

use std::sync::Arc;

use super::*;

/// One knight or monster with everything painted with it, at its whole
/// pixel: `img`'s top-left at `(x, y)` in the room's pixels. `frac` is how
/// far past that pixel it truly stands; `depth` is where its feet are, in
/// arena units down the room. `bodies` are the sprites stood up in it.
/// `foe`: a monster, its picture held to the brand guide (`graded`).
#[derive(Clone)]
pub struct Figure {
    pub img: Img,
    pub x: i32,
    pub y: i32,
    pub frac: (f32, f32),
    pub depth: f32,
    pub bodies: Vec<Body>,
    pub foe: bool,
}

/// A monster's picture held to the brand guide (docs/BRAND.md): the fewest
/// steps down its own inks' banks (`light::step_down`, the house's way to
/// darken: every pixel stays on the palette) that leave it under the
/// knights, its mean no brighter than `brand::FIGURE_MEAN` and no more of
/// it than `brand::FIGURE_BRIGHT_SHARE` over `brand::BRIGHT`. Signal inks
/// are game state and keep their light. A window onto a greater picture is
/// left as it is.
pub fn graded(img: Img) -> Img {
    use angel_stage::brand::{BRIGHT, FIGURE_BRIGHT_SHARE, FIGURE_MEAN, luma};
    use super::super::{ink::is_signal, light::step_down};
    if !img.whole() {
        return img;
    }
    let shown: Vec<(usize, Rgb)> = img
        .words()
        .iter()
        .enumerate()
        .filter(|&(_, &w)| w >> 24 >= 128)
        .map(|(i, &w)| (i, [(w >> 16) as u8, (w >> 8) as u8, w as u8]))
        .collect();
    if shown.is_empty() {
        return img;
    }
    let stepped = |c: Rgb, n: usize| if is_signal(c) { c } else { step_down(c, n) };
    let fits = |n: usize| {
        let lumas: Vec<f32> = shown.iter().map(|&(_, c)| luma(stepped(c, n))).collect();
        let count = lumas.len() as f32;
        lumas.iter().sum::<f32>() / count <= FIGURE_MEAN
            && lumas.iter().filter(|&&l| l > BRIGHT).count() as f32 / count <= FIGURE_BRIGHT_SHARE
    };
    let Some(n) = (0..=3).find(|&n| fits(n)).or(Some(3)).filter(|&n| n > 0) else {
        return img;
    };
    let mut words = img.words().to_vec();
    for (i, c) in shown {
        let [r, g, b] = stepped(c, n);
        words[i] = words[i] & 0xff00_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
    }
    Img::from_words(img.w, img.h, words)
}

/// A sprite stood up in a figure: its look (`look_key`), and the sprite
/// itself with its top-left at `(x, y)` in the room's pixels. A window with
/// an HD picture of that look draws it in the sprite's place.
#[derive(Clone)]
pub struct Body {
    pub key: u64,
    pub x: i32,
    pub y: i32,
    pub img: Img,
}

/// A sprite's look as a number: the same pixels, the same key, on every
/// machine and every run (FNV-1a over its size and pixels).
pub fn look_key(img: &Img) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |word: u32| {
        for byte in word.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(img.w as u32);
    eat(img.h as u32);
    for &word in img.words() {
        eat(word);
    }
    hash
}

/// What colour a light casts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    /// Torches, braziers, embers, lava: warm, and it flickers.
    Fire,
    /// Frost and arcane bolts.
    Cold,
    /// Hexes and spores.
    Gas,
    /// The light home, sacred things.
    Holy,
}

/// A light a window draws over the room itself, in the room's pixels:
/// where it is, how far it reaches and how strong it is at its middle (0
/// to 1), its colour, and whether it flickers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneLight {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub strength: f32,
    pub hue: Hue,
    pub flicker: bool,
}

/// What a window's motes are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motes {
    /// Rising off a fire.
    Embers,
    /// Drifting in a strong light.
    Dust,
    /// Thrown from a blow that landed.
    Sparks,
}

/// Where a window animates motes over the room, in the room's pixels: the
/// rectangle they are born in, how strong the source is (0 to 1), a seed
/// that keeps one emitter's motes its own, and for a burst how long ago it
/// went off, in seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub motes: Motes,
    pub rect: [f32; 4],
    pub strength: f32,
    pub seed: u32,
    pub age: Option<f32>,
}

/// What a shot is, for a window that builds it in 3D (`shot_meshes`): the
/// look the terminal paints as a sprite, said as a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotForm {
    /// One of the game's kinds, with its own look.
    Kind(crate::rules::Shot),
    /// A forged bolt (a rune weapon): its colour and its shape.
    Forged(ForgedColour, ForgedShape),
    /// A class's own shot: the Mage's arcane bolt, the Healer's spark.
    Class(ClassShot),
    /// A card's own drawing (`sprite shot`): art someone made, which stays a
    /// sprite in every window.
    Card,
}

/// A forged shot's colour, as `sprites::forged_shot` reads its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForgedColour {
    Ember,
    Ice,
    Gold,
    Venom,
    Void,
    Rose,
    Plain,
}

impl ForgedColour {
    pub fn of(name: &str) -> Self {
        match name {
            "ember" => Self::Ember,
            "ice" => Self::Ice,
            "gold" => Self::Gold,
            "venom" => Self::Venom,
            "void" => Self::Void,
            "rose" => Self::Rose,
            _ => Self::Plain,
        }
    }
}

/// A forged shot's shape, as `sprites::forged_shot` reads its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForgedShape {
    Arrow,
    /// A shard or a blade.
    Shard,
    Star,
    Wave,
    Coal,
    /// Any other name: a small cross of light.
    Bolt,
}

impl ForgedShape {
    pub fn of(name: &str) -> Self {
        match name {
            "arrow" => Self::Arrow,
            "shard" | "blade" => Self::Shard,
            "star" => Self::Star,
            "wave" => Self::Wave,
            "coal" => Self::Coal,
            _ => Self::Bolt,
        }
    }
}

/// A class's own shot (`classes::ARCANE`, `classes::HOLY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassShot {
    Arcane,
    Holy,
}

/// A shot in flight, for a window that draws it itself: where it is in the
/// room's pixels (between pixels) and how fast it goes (pixels a second),
/// its light and what it is, and its sprite as the terminal paints it
/// (`mark`, drawn at twice its size centred on the shot). A terminal shows
/// the sprite; a window lifts it off the floor in a glow with a trail.
#[derive(Clone, PartialEq)]
pub struct SceneShot {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub hue: Hue,
    /// Spells and fire glow; arrows, blades, bones, ore and pages do not.
    pub bright: bool,
    pub hostile: bool,
    /// Passed through a friend's wall: gold round it.
    pub empowered: bool,
    /// What it is, for a window that builds it as a shape.
    pub form: ShotForm,
    /// How it stands in the room's space (`dimensions`): low over the
    /// floor, or raised on a floor that stands up in 3D and for a knight
    /// whose powers have climbed far enough.
    pub dimension: super::dimensions::Dimension,
    pub mark: Img,
}

/// A dropped item as a physical solid, in room pixels. It is not baked into
/// `under`: a stage renders it procedurally and a terminal projects it once.
#[derive(Clone)]
pub struct SceneSolid {
    pub x: f32,
    pub y: f32,
    pub shape: u32,
    pub material: [u8; 3],
    pub phase: f32,
    pub rune: Option<crate::rules::runes::RuneKind>,
}

/// One frame of the Delve as parts.
#[derive(Clone)]
pub struct RoomScene {
    /// The view's size in native pixels: the canvas the parts are laid out on.
    pub view: (i32, i32),
    /// The room's still art, the whole room at native size. The same `Arc`
    /// comes back while it is unchanged. `None` when the frame is flat.
    pub ground: Option<Arc<Img>>,
    /// The view's top-left on the room, in native pixels, between pixels:
    /// its whole pixel is the terminal's camera.
    pub camera: (f32, f32),
    /// What is painted under the figures, in the room's pixels.
    pub under: Option<Img>,
    /// The figures, back to front.
    pub figures: Vec<Figure>,
    /// What is painted over the figures, in the room's pixels.
    pub over: Option<Img>,
    /// The marks on the view itself, in view pixels, over everything. A flat
    /// frame (a screen the parts don't cover yet) is all here.
    pub screen: Img,
    /// The lights in the room: its own (already in its still art, as dusk)
    /// and what burns and flies in it now. A terminal shows none of them.
    pub lights: Vec<SceneLight>,
    /// Motes over the room; a terminal shows none.
    pub emitters: Vec<Emitter>,
    /// Shots in flight, when the window draws them (`draw_parts_outside`);
    /// otherwise they are painted in `over`.
    pub shots: Vec<SceneShot>,
    /// Floor pickups as shapes, when the page draws props and the sprites were left off the canvases.
    pub pickups: Vec<super::pickup_meshes::ScenePickup>,
    /// How hard the world was just hit, 0 to 1 (`impact_of`): a window's
    /// post pass splits the colours and lifts the light by it.
    pub impact: f32,
    /// Floor pickups as ray-marched solids, when the frame goes out in parts
    /// to a page that draws no props (`LayerKind::Primitive`).
    pub solids: Vec<SceneSolid>,
    /// Which room this is (0: not a room), and the rooms through its doors
    /// to the north, east, south and west: a window that saw the party in
    /// one of those last slides this one in from that side.
    pub room: u64,
    pub beside: [Option<u64>; 4],
    /// A guardian just come into the room: where it stands, in the room's
    /// pixels, and how many seconds ago it came; a window's camera may lean
    /// in to meet it.
    pub entrance: Option<([f32; 2], f32)>,
    /// The party just come down into the deep (`world3d::room::beat`): the
    /// knight on show, in the room's pixels, and how many seconds ago; a
    /// window holds the cut black, leans in on them and eases back as the
    /// room's lamps come up.
    pub descent: Option<([f32; 2], f32)>,
    /// Where the room shows darkened under the screen's marks, `[x, y, w,
    /// h]` in view pixels: the skills bar's veil along the view's foot.
    pub veil: Option<[i32; 4]>,
    /// The room is in a delivered room kit, whose frame is snapped to the
    /// realm's palette (`dungeon_art::palette_frame`): `compose` snaps what
    /// it takes from the room (the screen's marks come snapped). A window
    /// that lights the room itself may leave its colours as they are.
    pub palette: bool,
    /// The room in 3D (`world3d`, docs/WORLD-3D.md): its blocks with their
    /// plate cast on them, which a window that draws scenes stands the
    /// figures in instead of laying them over `ground`. `None` unless the
    /// host draws rooms so and this room has an item.
    pub world: Option<crate::world3d::room::RoomWorld>,
    /// The kingdom in 3D (`world3d::kingdom`): the chunks near the view,
    /// each its blocks with its own plate cast on them, which a window that
    /// draws scenes stands the knights in. Empty unless the host draws the
    /// world so and this is the kingdom.
    pub chunks: Vec<std::sync::Arc<crate::world3d::kingdom::Chunk>>,
    /// What the run stands in the kingdom that the chunks' sets have not
    /// (`world3d::lots`): the party's builds on its lots and the run's
    /// finds, extra blocks in the chunks they stand in (`chunks` are cut
    /// where a lot clears them already). `None` off the kingdom in 3D.
    pub lots: Option<std::sync::Arc<crate::world3d::lots::Lots>>,
    /// The kingdom in 3D: the view the realm's look is made for (where the
    /// view is, the knight it follows, the clock); `None` off the kingdom.
    pub realm: Option<crate::world3d::realm_look::View>,
}

impl RoomScene {
    /// The way the party travelled to get here from room `from`, as a unit
    /// step (east is `[1, 0]`); `None` unless `from` is through a door.
    pub fn travelled_from(&self, from: u64) -> Option<[f32; 2]> {
        if from == 0 || from == self.room {
            return None;
        }
        let side = self.beside.iter().position(|&b| b == Some(from))?;
        let (dx, dy) = crate::rules::layout::DIRS[side];
        Some([-dx as f32, -dy as f32])
    }
}

/// A room's own number in this run.
fn room_id(run: &Run, at: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (run.raid_id, run.dungeon.depth, at).hash(&mut h);
    h.finish() | 1
}

impl RoomScene {
    /// A finished picture as a scene with nothing else in it.
    pub fn flat(img: Img) -> Self {
        Self {
            view: (img.w, img.h),
            ground: None,
            camera: (0.0, 0.0),
            under: None,
            figures: Vec::new(),
            over: None,
            screen: img,
            lights: Vec::new(),
            emitters: Vec::new(),
            shots: Vec::new(),
            pickups: Vec::new(),
            impact: 0.0,
            solids: Vec::new(),
            room: 0,
            beside: [None; 4],
            entrance: None,
            descent: None,
            veil: None,
            palette: false,
            world: None,
            chunks: Vec::new(),
            lots: None,
            realm: None,
        }
    }

    pub fn is_flat(&self) -> bool {
        self.ground.is_none() && self.under.is_none() && self.chunks.is_empty()
    }

    /// The parts put back together, the same pixels `frame_for` paints.
    /// Not the terminal path: only the parity test calls this.
    #[cfg(any(test, feature = "testing"))]
    pub fn compose(&self) -> Img {
        let (w, h) = self.view;
        let (cx, cy) = (self.camera.0.floor() as i32, self.camera.1.floor() as i32);
        let room = self.under.as_ref().or(self.over.as_ref()).map(|i| (i.w, i.h));
        let veiled = |x: i32, y: i32| {
            self.veil.is_some_and(|[vx, vy, vw, vh]| x >= vx && x < vx + vw && y >= vy && y < vy + vh)
        };
        // Only the view of what moves is ever read: keep only that.
        let mut moving = room.map(|(rw, rh)| Img::window(rw, rh, (cx, cy, w, h)));
        if let Some(moving) = &mut moving {
            if let Some(under) = &self.under {
                moving.stamp(under, 0, 0);
            }
            for solid in &self.solids {
                let im = match solid.rune {
                    Some(kind) => super::runes::solid_orb(kind, solid.phase),
                    None => super::cards::pickup_image(solid.shape, solid.material, solid.phase),
                };
                moving.stamp(&im, solid.x.floor() as i32 - im.w / 2, solid.y.floor() as i32 - im.h / 2);
            }
            for figure in &self.figures {
                moving.stamp(&figure.img, figure.x, figure.y);
            }
            if let Some(over) = &self.over {
                moving.stamp(over, 0, 0);
            }
            for shot in &self.shots {
                moving.stamp_twice(&shot.mark, shot.x.floor() as i32 - 24, shot.y.floor() as i32 - 24);
            }
        }
        let mut out = Img::black(w, h);
        let mut snap = self.palette.then(dungeon_art::snapper);
        for y in 0..h {
            for x in 0..w {
                let ground = || {
                    self.ground
                        .as_ref()
                        .and_then(|g| g.get(x + cx, y + cy))
                        .map(|c| if veiled(x, y) { veil(c) } else { c })
                };
                if let Some(c) = self.screen.get(x, y) {
                    out.set(x, y, c);
                    continue;
                }
                let c = moving.as_ref().and_then(|m| m.get(x + cx, y + cy)).or_else(ground);
                match (&mut snap, c) {
                    (Some(snap), c) => out.set(x, y, snap(c.unwrap_or(BLACK))),
                    (None, Some(c)) => out.set(x, y, c),
                    (None, None) => {}
                }
            }
        }
        out
    }
}

/// Whether this frame comes apart into parts; the rest (plates, the
/// kingdom, a pastime's own screen, the Forest Road and the Round Table
/// adventures that stand in the forest's room, rooms lit from below or put
/// out, the second art kit) stay one picture for now.
fn layered(run: &Run, w: i32, h: i32, focus: Option<u32>) -> bool {
    w >= NATIVE_W
        && h >= NATIVE_H
        && !(art_v2::enabled() && art_v2::supported(run))
        && run.floor_plate(focus).is_none()
        && run.results_showing(focus).is_none()
        && run.room().kind != RoomKind::Kingdom
        && !stage::takeover(run)
        && !run.forest_road_showing()
        && !run.roundtable_showing()
        && !(run.circle() > 0 && !run.at_home_now())
        && !(run.mode == crate::rules::fortune::Mode::LightsOut && !run.at_home_now())
        && !super::swamp::fogged(run)
}

/// Whether this frame is the kingdom stood up in 3D (`world3d::kingdom`):
/// rooms in 3D, a window that draws scenes, the kingdom on show at the
/// native size or more, nothing taking the screen over.
fn kingdom_3d(run: &Run, w: i32, h: i32, focus: Option<u32>) -> bool {
    run.room().kind == RoomKind::Kingdom
        && crate::world3d::enabled()
        && super::props_meshes_on()
        && w >= NATIVE_W
        && h >= NATIVE_H
        && run.floor_plate(focus).is_none()
        && !(art_v2::enabled() && art_v2::supported(run))
        && !(run.mossbell_showing()
            || run.brasswake_showing()
            || run.forest_road_showing()
            || run.pennon_showing()
            || run.rafterwake_showing()
            || run.latch_showing()
            || run.cinder_showing())
}

/// The kingdom stood up in 3D: the chunks near the view (`chunks`), each
/// knight a figure in them, what moves and marks the floor lying on it, the
/// screen's own marks over all. `None` while a chunk the view overlaps is
/// not there to draw (a page is sent them a few a snapshot, the nearest its
/// view first, `wire::pace`): the kingdom is drawn flat, as ever, never with
/// a hole in it.
fn kingdom_scene(run: &Run, focus: Option<u32>) -> Option<RoomScene> {
    let origin = camera(run, focus);
    let (ox, oy) = (origin.0 as f32, origin.1 as f32);
    let rect = [ox, oy, ox + NATIVE_W as f32, oy + NATIVE_H as f32];
    let near = crate::world3d::kingdom::near(rect);
    // The first time, every chunk's set made and the folder looked over,
    // off the frame; the log says which have no plate.
    crate::world3d::kingdom::warm();
    let chunks = crate::world3d::kingdom::chunks().of(&near);
    let there = |index: u32| chunks.iter().any(|c| c.index == index);
    if chunks.is_empty() || !crate::world3d::kingdom::overlaps(rect).into_iter().all(there) {
        return None;
    }
    // What the party built on its lots and the run's finds stand up as
    // extra blocks in their chunks, each chunk cut where a lot clears it
    // (`world3d::lots`); the party's names lie on the ground before them.
    let lots = crate::world3d::lots::of(run);
    let chunks = match &lots {
        Some(lots) => chunks.iter().map(|c| lots.chunk(c)).collect(),
        None => chunks,
    };
    let (view, screen, figures) = kingdom::layers_3d(run, focus);
    let room = room_px(run.room());
    let window = (origin.0, origin.1, NATIVE_W, NATIVE_H);
    let mut over = Img::window(room.0, room.1, window);
    over.stamp(&view, origin.0, origin.1);
    let under = lots.as_ref().filter(|l| !l.plaques.is_empty()).map(|lots| {
        let mut under = Img::window(room.0, room.1, window);
        lots.lay_plaques(&mut under, (0, 0), &|_| true);
        under
    });
    Some(RoomScene {
        view: (NATIVE_W, NATIVE_H),
        ground: None,
        // Whole pixels, as the view's marks are drawn at.
        camera: (ox, oy),
        under,
        figures,
        over: Some(over),
        screen,
        lights: Vec::new(),
        emitters: Vec::new(),
        shots: Vec::new(),
        pickups: Vec::new(),
        impact: impact_of(run),
        solids: Vec::new(),
        room: room_id(run, run.at),
        beside: [None; 4],
        entrance: None,
        descent: None,
        veil: None,
        palette: false,
        world: None,
        chunks,
        lots,
        realm: Some(crate::world3d::realm_look::View {
            origin: [ox, oy],
            size: [NATIVE_W as f32, NATIVE_H as f32],
            knight: focus
                .and_then(|id| run.players.get(&id))
                .or_else(|| run.players.values().next())
                .map(|h| [h.x * UNIT, h.y * UNIT]),
            others: {
                let followed = focus.filter(|id| run.players.contains_key(id)).or_else(|| run.players.keys().next().copied());
                let mut others = run.players.iter().filter(|(id, _)| Some(**id) != followed).map(|(_, h)| Some([h.x * UNIT, h.y * UNIT]));
                [others.next().flatten(), others.next().flatten(), others.next().flatten()]
            },
            time: run.tick as f32 / crate::rules::HZ as f32,
        }),
    })
}

/// The frame `frame_for` draws at `w` × `h`, as parts.
/// How hard the world was just hit, 0 to 1: a guardian's fall or a bomb
/// while the screen still shakes (full at the shake's height), and the
/// heavy blows the host holds the world still for (a little less).
pub fn impact_of(run: &Run) -> f32 {
    /// The shake's ticks that make a full impact, and a held blow's.
    const SHAKE_FULL: f32 = 12.0;
    const HOLD_FULL: f32 = 12.0;
    const HOLD_SHARE: f32 = 0.6;
    let shake = (run.shake as f32 / SHAKE_FULL).min(1.0);
    let hold = (run.hitstop.left as f32 / HOLD_FULL).min(1.0) * HOLD_SHARE;
    shake.max(hold)
}

pub fn scene_for(run: &Run, w: i32, h: i32, focus: Option<u32>) -> RoomScene {
    if kingdom_3d(run, w, h, focus)
        && let Some(scene) = kingdom_scene(run, focus)
    {
        return scene;
    }
    if !layered(run, w, h, focus) {
        // A flat picture has no shapes: it draws every pickup itself.
        let (props, cards) = (super::props_meshes_on(), super::pickup_cards_on());
        super::props_meshes(false);
        super::pickup_cards(false);
        let flat = RoomScene::flat(frame_for(run, w, h, focus));
        super::props_meshes(props);
        super::pickup_cards(cards);
        return flat;
    }
    let room = room_px(run.room());
    let win = scene_window(run, focus, room);
    let (under, capture) = captured(room, win, || {
        let (low, high) = room_list(run, focus, true, false).split(draw_list::Layer::Figures);
        let mut under = Img::window(room.0, room.1, win);
        // A room in 3D stands its world where its still art lay: what lies
        // on that floor that the game moves (its traps, a cracked wall) lies
        // on the world, under all else.
        if super::world_stood(run) {
            super::room_details(run, &mut under, &mut Vec::new());
        }
        low.paint(&mut under);
        (under, high)
    });
    let (under, high) = under;
    let mut over = capture.after;
    high.paint(&mut over);
    let over = match &capture.base {
        Some(base) => over.changes_over(base),
        None => over,
    };
    let origin = view_origin(run, focus);
    let (cx, cy) = camera_exact(run, focus);
    // The shake is whole pixels on top of the camera.
    let shake = (origin.0 - cx.floor() as i32, origin.1 - cy.floor() as i32);
    let mut screen = Img::new(NATIVE_W, NATIVE_H);
    screen_marks(&mut screen, run, focus, true, false, origin);
    votes::panel(&mut screen, run, focus);
    // A delivered room kit's frame is snapped to the palette, its marks
    // with it (`frame_v1`).
    let palette = dungeon_art::supported(run);
    if palette {
        dungeon_art::palette_layer(&mut screen);
    }
    // A seat's skills bar is in its band, off the view: no veil there.
    let veil = (!hud_in_band())
        .then(|| tray::bar_rect(run, focus, NATIVE_W, NATIVE_H))
        .flatten()
        .map(|(x, y, w, h)| [x, y, w, h]);
    let ground = scenery_cached(run);
    // A lit room in 3D (`world3d::rooms_lit`) is lit by its own lamps: its
    // still art's are not laid over it.
    let world = crate::world3d::room::world_in(run, focus, room);
    let lights = scene_lights(run, &ground, world.as_ref().is_none_or(|w| w.lit().is_none()));
    let emitters = scene_emitters(run, &lights);
    RoomScene {
        view: (NATIVE_W, NATIVE_H),
        ground: Some(ground),
        camera: (cx + shake.0 as f32, cy + shake.1 as f32),
        under: Some(under),
        figures: capture.figures,
        over: Some(over),
        screen,
        lights,
        emitters,
        shots: capture.shots,
        pickups: capture.pickups,
        impact: impact_of(run),
        solids: capture.solids,
        entrance: entrance(run),
        descent: crate::world3d::room::beat(run).map(|s| {
            let centre = [cx + NATIVE_W as f32 / 2.0, cy + NATIVE_H as f32 / 2.0];
            (world.as_ref().and_then(|w| w.moment.chest).map_or(centre, |c| [c[0], c[2]]), s)
        }),
        room: room_id(run, run.at),
        beside: std::array::from_fn(|side| run.dungeon.neighbour(run.at, side).map(|i| room_id(run, i))),
        veil,
        palette,
        world,
        chunks: Vec::new(),
        lots: None,
        realm: None,
    }
}

/// How far round the view a window onto a great room keeps (native
/// pixels): past a hit's shake and the party's band above the view.
const WINDOW_MARGIN: i32 = 64;

/// What of the room a frame takes apart: all of it, but for a room bigger
/// than a party's great hall (an open floor's map), only a window round
/// the view, so nothing far off is painted, compared or handed over.
fn scene_window(run: &Run, focus: Option<u32>, (pw, ph): (i32, i32)) -> (i32, i32, i32, i32) {
    let room = run.room();
    if room.cols <= crate::rules::layout::MAX_COLS && room.rows <= crate::rules::layout::MAX_ROWS {
        return (0, 0, pw, ph);
    }
    let (ox, oy) = view_origin(run, focus);
    (ox - WINDOW_MARGIN, oy - WINDOW_MARGIN, NATIVE_W + 2 * WINDOW_MARGIN, NATIVE_H + 2 * WINDOW_MARGIN)
}

/// How many steps down its bank the room goes under a veil.
const VEIL_STEPS: usize = 3;
/// How dark a window lays a veil: the steps above, as black's cover.
pub const VEIL_COVER: f32 = 0.65;

/// A pixel of the room under a veil: stepped down its own bank, so it
/// stays on the palette, and state (signal ink) never dims.
pub(super) fn veil(c: Rgb) -> Rgb {
    step_down(c, VEIL_STEPS)
}

/// Every pixel of `img` in `(x, y, w, h)` under a veil.
pub(super) fn darken(img: &mut Img, (x, y, w, h): (i32, i32, i32, i32)) {
    for py in y.max(0)..(y + h).min(img.h) {
        for px in x.max(0)..(x + w).min(img.w) {
            if let Some(c) = img.get(px, py) {
                img.set(px, py, veil(c));
            }
        }
    }
}

/// How long a guardian's coming counts as its entrance.
const ENTRANCE_TICKS: u32 = 2 * crate::rules::HZ;

/// The newest guardian still making its entrance.
fn entrance(run: &Run) -> Option<([f32; 2], f32)> {
    run.enemies
        .iter()
        .filter(|e| e.boss.is_some() && e.hp > 0 && e.age < ENTRANCE_TICKS)
        .min_by_key(|e| e.age)
        .map(|e| ([e.x * UNIT, e.y * UNIT], e.age as f32 / crate::rules::HZ as f32))
}

/// A fire this strong or more keeps dust turning in its light.
const DUSTY_LIGHT: f32 = 0.28;

/// Motes: embers off every fire, dust turning in the strong ones, sparks
/// where a blow just landed on a knight or a monster.
fn scene_emitters(run: &Run, lights: &[SceneLight]) -> Vec<Emitter> {
    let seed = |x: f32, y: f32| (x as i32 as u32).wrapping_mul(73_856_093) ^ (y as i32 as u32).wrapping_mul(19_349_663);
    let mut out = Vec::new();
    for light in lights.iter().filter(|l| l.flicker) {
        out.push(Emitter {
            motes: Motes::Embers,
            rect: [light.x - 3.0, light.y - 6.0, 6.0, 4.0],
            strength: light.strength,
            seed: seed(light.x, light.y),
            age: None,
        });
        if light.strength >= DUSTY_LIGHT {
            let r = light.radius * 0.55;
            out.push(Emitter {
                motes: Motes::Dust,
                rect: [light.x - r, light.y - r * 0.6, 2.0 * r, 1.2 * r],
                strength: light.strength,
                seed: seed(light.y, light.x),
                age: None,
            });
        }
    }
    let hz = crate::rules::HZ as f32;
    for (&id, hero) in &run.players {
        if hero.struck > 0 && hero.hp > 0 {
            let ticks = crate::rules::spectacle::STRUCK.saturating_sub(hero.struck);
            let age = ticks as f32 / hz;
            // The tick the blow landed: the same sparks all through it.
            let landed = run.tick.saturating_sub(u64::from(ticks)) as u32;
            let (x, y) = (hero.x * UNIT, hero.y * UNIT - 8.0);
            out.push(Emitter {
                motes: Motes::Sparks,
                rect: [x - 2.0, y - 2.0, 4.0, 4.0],
                strength: if hero.struck_big { 1.0 } else { 0.6 },
                seed: id.wrapping_mul(2_654_435_761) ^ landed,
                age: Some(age),
            });
        }
    }
    for (i, enemy) in run.enemies.iter().enumerate() {
        if enemy.hurt > 0 {
            let ticks = crate::rules::desperate::HIT_FLASH.saturating_sub(enemy.hurt);
            let age = f32::from(ticks) / hz;
            let landed = run.tick.saturating_sub(u64::from(ticks)) as u32;
            let (x, y) = (enemy.x * UNIT, enemy.y * UNIT - 4.0);
            out.push(Emitter {
                motes: Motes::Sparks,
                rect: [x - 2.0, y - 2.0, 4.0, 4.0],
                strength: 0.5,
                seed: (i as u32).wrapping_mul(2_246_822_519) ^ landed,
                age: Some(age),
            });
        }
    }
    out
}

/// How far a shot's light reaches and how strong it is.
const SHOT_LIGHT: (f32, f32) = (16.0, 0.55);
/// The light home's.
const HOME_LIGHT: (f32, f32) = (64.0, 0.7);

/// The room's lights: those its still art was lit by, then the shots in
/// flight, the bursts and the light home, where they are now.
fn scene_lights(run: &Run, ground: &Arc<Img>, baked: bool) -> Vec<SceneLight> {
    let mut out: Vec<SceneLight> = baked_lights(ground)
        .iter()
        .filter(|_| baked)
        .map(|l| SceneLight {
            x: l.x,
            y: l.y,
            radius: l.r,
            strength: l.s,
            hue: if l.fire { Hue::Fire } else { Hue::Cold },
            flicker: l.fire,
        })
        .collect();
    for shot in &run.projectiles {
        let hue = match shot.kind {
            Shot::Ember | Shot::Flame | Shot::Ball => Hue::Fire,
            Shot::Sacred => Hue::Holy,
            Shot::Hex | Shot::Spore => Hue::Gas,
            _ => Hue::Cold,
        };
        out.push(SceneLight {
            x: shot.x * UNIT,
            y: shot.y * UNIT,
            radius: SHOT_LIGHT.0,
            strength: SHOT_LIGHT.1,
            hue,
            flicker: false,
        });
    }
    for &(x, y, left) in &run.blasts {
        let k = 1.0 - f32::from(left) / f32::from(crate::rules::BLAST_TICKS);
        out.push(SceneLight {
            x: x * UNIT,
            y: y * UNIT,
            radius: 20.0 + 44.0 * k,
            strength: 0.9 * (1.0 - k),
            hue: Hue::Fire,
            flicker: false,
        });
    }
    // A shield's block flares on its face.
    for (x, y, strength, hue) in vfx::block_lights(run) {
        out.push(SceneLight {
            x,
            y,
            radius: 12.0,
            strength,
            hue,
            flicker: false,
        });
    }
    // The gauntlets' lanterns and their dangers' heat.
    for (x, y, radius, strength, _) in super::gauntlets::lights(run) {
        out.push(SceneLight {
            x: x as f32,
            y: y as f32,
            radius: radius as f32,
            strength,
            hue: Hue::Fire,
            flicker: false,
        });
    }
    if let Some((x, y)) = run.light {
        out.push(SceneLight {
            x: x * UNIT,
            y: y * UNIT,
            radius: HOME_LIGHT.0,
            strength: HOME_LIGHT.1,
            hue: Hue::Holy,
            flicker: false,
        });
    }
    out
}

/// A live seat's scene: as `frame_for_live`, a torch step may show the
/// previous one until its bake lands.
pub fn scene_for_live(run: &Run, w: i32, h: i32, focus: Option<u32>) -> RoomScene {
    during_live(|| scene_for(run, w, h, focus))
}
