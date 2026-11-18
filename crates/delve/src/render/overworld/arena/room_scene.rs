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

#[derive(Clone)]
pub struct Figure {
    pub img: Img,
    pub x: i32,
    pub y: i32,
    pub frac: (f32, f32),
    pub depth: f32,
    pub height: f32,
    pub additive: bool,
    pub bodies: Vec<Body>,
    pub foe: bool,
    pub paints: Vec<super::Painting>,
}

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

#[derive(Clone)]
pub struct Body {
    pub key: u64,
    pub x: i32,
    pub y: i32,
    pub img: Img,
    pub named: Option<super::figure_keys::Named>,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    Fire,
    Cold,
    Gas,
    Holy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneLight {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub strength: f32,
    pub hue: Hue,
    pub flicker: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motes {
    Embers,
    Dust,
    Sparks,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub motes: Motes,
    pub rect: [f32; 4],
    pub strength: f32,
    pub seed: u32,
    pub age: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotForm {
    Kind(crate::rules::Shot),
    Forged(ForgedColour, ForgedShape),
    Class(ClassShot),
    Card,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForgedShape {
    Arrow,
    Shard,
    Star,
    Wave,
    Coal,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassShot {
    Arcane,
    Holy,
    Song,
}

#[derive(Clone, PartialEq)]
pub struct SceneShot {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub hue: Hue,
    pub bright: bool,
    pub hostile: bool,
    pub empowered: bool,
    pub form: ShotForm,
    pub dimension: super::dimensions::Dimension,
    pub mark: Img,
    pub paint: Option<super::Painting>,
    pub scale: f32,
}

#[derive(Clone)]
pub struct SceneSolid {
    pub x: f32,
    pub y: f32,
    pub shape: u32,
    pub material: [u8; 3],
    pub phase: f32,
    pub rune: Option<crate::rules::runes::RuneKind>,
}

#[derive(Clone)]
pub struct FineGround {
    pub img: Arc<Img>,
    pub scale: u32,
}

#[derive(Clone)]
pub struct RoomScene {
    pub view: (i32, i32),
    pub ground: Option<Arc<Img>>,
    pub fine: Option<FineGround>,
    pub camera: (f32, f32),
    pub under: Option<Img>,
    pub figures: Vec<Figure>,
    pub over: Option<Img>,
    pub screen: Img,
    pub lights: Vec<SceneLight>,
    pub emitters: Vec<Emitter>,
    pub shots: Vec<SceneShot>,
    pub pickups: Vec<super::pickup_meshes::ScenePickup>,
    pub paints: Vec<super::Painting>,
    pub impact: f32,
    pub solids: Vec<SceneSolid>,
    pub room: u64,
    pub beside: [Option<u64>; 4],
    pub entrance: Option<([f32; 2], f32)>,
    pub descent: Option<([f32; 2], f32)>,
    pub veil: Option<[i32; 4]>,
    pub palette: bool,
    pub world: Option<crate::world3d::room::RoomWorld>,
    pub chunks: Vec<std::sync::Arc<crate::world3d::kingdom::Chunk>>,
    pub lots: Option<std::sync::Arc<crate::world3d::lots::Lots>>,
    pub works: Option<std::sync::Arc<crate::world3d::works_site::WorksView>>,
    pub hill: Option<std::sync::Arc<crate::world3d::hillside::HillNodes>>,
    pub realm: Option<crate::world3d::realm_look::View>,
    pub board: bool,
}

impl RoomScene {
    pub fn travelled_from(&self, from: u64) -> Option<[f32; 2]> {
        if from == 0 || from == self.room {
            return None;
        }
        let side = self.beside.iter().position(|&b| b == Some(from))?;
        let (dx, dy) = crate::rules::layout::DIRS[side];
        Some([-dx as f32, -dy as f32])
    }
}

fn room_id(run: &Run, at: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (run.raid_id, run.dungeon.depth, at).hash(&mut h);
    h.finish() | 1
}

impl RoomScene {
    pub fn flat(img: Img) -> Self {
        Self {
            view: (img.w, img.h),
            ground: None,
            fine: None,
            camera: (0.0, 0.0),
            under: None,
            figures: Vec::new(),
            over: None,
            screen: img,
            lights: Vec::new(),
            emitters: Vec::new(),
            shots: Vec::new(),
            pickups: Vec::new(),
            paints: Vec::new(),
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
            works: None,
            hill: None,
            realm: None,
            board: false,
        }
    }

    pub fn is_flat(&self) -> bool {
        self.ground.is_none() && self.under.is_none() && self.chunks.is_empty()
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn compose(&self) -> Img {
        let (w, h) = self.view;
        let (cx, cy) = (self.camera.0.floor() as i32, self.camera.1.floor() as i32);
        let room = self.under.as_ref().or(self.over.as_ref()).map(|i| (i.w, i.h));
        let veiled = |x: i32, y: i32| {
            self.veil.is_some_and(|[vx, vy, vw, vh]| x >= vx && x < vx + vw && y >= vy && y < vy + vh)
        };
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
                if shot.mark.w == 0 { super::volleys::ranged::replay(moving, shot); continue; }
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

fn layered(run: &Run, w: i32, h: i32, focus: Option<u32>) -> bool {
    w >= NATIVE_W
        && h >= NATIVE_H
        && run.floor_plate(focus).is_none()
        && run.results_showing(focus).is_none()
        && run.room().kind != RoomKind::Kingdom
        && !stage::takeover(run)
        && !run.joust_showing()
        && !run.forest_road_showing()
        && !run.roundtable_showing()
        && !(run.circle() > 0 && !run.at_home_now())
        && !(run.mode == crate::rules::fortune::Mode::LightsOut && !run.at_home_now())
        && !super::swamp::fogged(run)
}

fn kingdom_3d(run: &Run, w: i32, h: i32, focus: Option<u32>) -> bool {
    run.room().kind == RoomKind::Kingdom
        && crate::world3d::enabled()
        && super::props_meshes_on()
        && w >= NATIVE_W
        && h >= NATIVE_H
        && run.floor_plate(focus).is_none()
        && !(run.mossbell_showing()
            || run.brasswake_showing()
            || run.forest_road_showing()
            || run.pennon_showing()
            || run.rafterwake_showing()
            || run.latch_showing()
            || run.cinder_showing())
}

fn kingdom_scene(run: &Run, focus: Option<u32>) -> Option<RoomScene> {
    let cam = camera(run, focus);
    let k = super::stand_back::zoom();
    let origin = if k > 1 { super::stand_back::origin() } else { cam };
    let (vw, vh) = (NATIVE_W * k, NATIVE_H * k);
    let (ox, oy) = (origin.0 as f32, origin.1 as f32);
    let rect = [ox, oy, ox + vw as f32, oy + vh as f32];
    let near = crate::world3d::kingdom::near(rect);
    crate::world3d::kingdom::warm();
    let chunks = crate::world3d::kingdom::chunks().of(&near);
    let there = |index: u32| chunks.iter().any(|c| c.index == index);
    let needed = if k > 1 {
        let (x0, y0, w, h) = run.sites.keep().and_then(|s| s.castle()).map_or((0, 0, 0, 0), |c| c.tiles());
        crate::world3d::kingdom::overlaps([x0 as f32 * 16.0, y0 as f32 * 16.0, (x0 + w) as f32 * 16.0, (y0 + h) as f32 * 16.0])
    } else {
        crate::world3d::kingdom::overlaps(rect)
    };
    if chunks.is_empty() || !needed.into_iter().all(there) {
        return None;
    }
    let lots = crate::world3d::lots::of(run);
    let chunks: Vec<_> = match &lots {
        Some(lots) => chunks.iter().map(|c| lots.chunk(c)).collect(),
        None => chunks,
    };
    let knight = focus
        .and_then(|id| run.players.get(&id))
        .or_else(|| run.players.values().next())
        .map(|h| [h.x * UNIT, h.y * UNIT]);
    let works = crate::world3d::works_site::of(run, knight);
    let hill = crate::world3d::hillside::of(run, [ox - 96.0, oy - 64.0, ox + vw as f32 + 96.0, oy + vh as f32 + 192.0], focus).map(std::sync::Arc::new);
    let chunks: Vec<_> = chunks.iter().map(|c| crate::world3d::works_site::dig(run, c)).collect();
    let (view, mut screen, mut figures) = kingdom::layers_3d(run, focus);
    if k > 1 {
        kingdom::crew_in(&mut figures, run, origin, (vw, vh));
        screen = scaled(&super::stand_back::screen(run, focus), k);
    }
    let room = room_px(run.room());
    let window = (origin.0, origin.1, vw, vh);
    let mut over = Img::window(room.0, room.1, window);
    over.stamp(&view, cam.0, cam.1);
    let under = lots.as_ref().filter(|l| !l.plaques.is_empty()).map(|lots| {
        let mut under = Img::window(room.0, room.1, window);
        lots.lay_plaques(&mut under, (0, 0), &|_| true);
        under
    });
    Some(RoomScene {
        view: (vw, vh),
        ground: None,
        fine: None,
        camera: (ox, oy),
        under,
        figures,
        over: Some(over),
        screen,
        lights: Vec::new(),
        emitters: Vec::new(),
        shots: Vec::new(),
        pickups: Vec::new(),
        paints: Vec::new(),
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
        works,
        hill,
        realm: Some(crate::world3d::realm_look::View {
            origin: [ox, oy],
            size: [vw as f32, vh as f32],
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
        board: false,
    })
}

fn scaled(im: &Img, k: i32) -> Img {
    let mut out = Img::new(im.w * k, im.h * k);
    for y in 0..out.h {
        for x in 0..out.w {
            if im.solid(x / k, y / k)
                && let Some(c) = im.get(x / k, y / k)
            {
                out.set(x, y, c);
            }
        }
    }
    out
}

pub fn impact_of(run: &Run) -> f32 {
    const SHAKE_FULL: f32 = 12.0;
    const HOLD_FULL: f32 = 12.0;
    const HOLD_SHARE: f32 = 0.6;
    let shake = (run.shake as f32 / SHAKE_FULL).min(1.0);
    let hold = (run.hitstop.left as f32 / HOLD_FULL).min(1.0) * HOLD_SHARE;
    shake.max(hold)
}

fn pastime_impact(run: &Run) -> f32 {
    use crate::rules::pastimes::{self, Game};
    let Some(round) = run.pastimes.round.as_ref() else {
        return 0.0;
    };
    if round.game != Game::DuckShoot {
        return 0.0;
    }
    let Some(range) = round.duckshoot.as_ref() else {
        return 0.0;
    };
    let reset = pastimes::DOWN_RESET.max(1) as f32;
    range
        .ducks
        .iter()
        .map(|d| {
            if d.down == 0 {
                0.0
            } else {
                (1.0 - (d.down as f32 - 1.0) / (reset - 1.0)).clamp(0.0, 1.0)
            }
        })
        .fold(0.0_f32, f32::max)
}

fn pastime_scene(run: &Run, w: i32, h: i32, focus: Option<u32>) -> Option<RoomScene> {
    if w != NATIVE_W || h != NATIVE_H {
        return None;
    }
    let (backdrop, figures, hud, emitters) = stage::parts(run, focus)?;
    Some(RoomScene {
        view: (w, h),
        ground: None,
        camera: (0.0, 0.0),
        under: Some(backdrop),
        figures,
        over: None,
        screen: hud,
        lights: Vec::new(),
        emitters,
        shots: Vec::new(),
        pickups: Vec::new(),
        paints: Vec::new(),
        impact: pastime_impact(run),
        solids: Vec::new(),
        room: room_id(run, run.at),
        beside: [None; 4],
        entrance: None,
        descent: None,
        veil: None,
        palette: false,
        world: None,
        chunks: Vec::new(),
        lots: None,
        works: None,
        hill: None,
        realm: None,
        fine: None,
        board: false,
    })
}

pub fn scene_for(run: &Run, w: i32, h: i32, focus: Option<u32>) -> RoomScene {
    if industry::showing(run, focus) || industry::frontier(run) { return RoomScene::flat(frame_for(run, w, h, focus)); }
    super::paintings::with_hd(super::paintings::hd_floor(run.floor()), || scene_here(run, w, h, focus))
}

fn scene_here(run: &Run, w: i32, h: i32, focus: Option<u32>) -> RoomScene {
    if super::crossroads::showing(run) || super::crossroads::pastime(run) || super::encounter::pastime(run) {
        return RoomScene::flat(frame_for(run, w, h, focus));
    }
    if kingdom_3d(run, w, h, focus)
        && let Some(scene) = kingdom_scene(run, focus)
    {
        return scene;
    }
    if stage::takeover(run)
        && let Some(scene) = pastime_scene(run, w, h, focus)
    {
        return scene;
    }
    if !layered(run, w, h, focus) {
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
    let board = super::atari::here(run);
    let ((under, capture, over), paints) = super::paintings::noting(|| {
        let ((under, high), mut capture) = captured(room, win, || {
            let (low, high) = room_list(run, focus, true).split(draw_list::Layer::Figures);
            let mut under = Img::window(room.0, room.1, win);
            if super::world_stood(run) {
                super::room_details(run, &mut under, &mut Vec::new());
            }
            if board {
                let (ground, figures) = low.split(draw_list::Layer::Ground);
                ground.paint(&mut under);
                super::atari::board(&mut under);
                figures.paint(&mut under);
            } else {
                low.paint(&mut under);
            }
            (under, high)
        });
        let mut over = std::mem::replace(&mut capture.after, Img::new(0, 0));
        high.paint(&mut over);
        (under, capture, over)
    });
    let over = match &capture.base {
        Some(base) => over.changes_over(base),
        None => over,
    };
    let origin = view_origin(run, focus);
    let (cx, cy) = camera_exact(run, focus);
    let shake = (origin.0 - cx.floor() as i32, origin.1 - cy.floor() as i32);
    let mut screen = Img::new(NATIVE_W, NATIVE_H);
    screen_marks(&mut screen, run, focus, true, origin);
    votes::panel(&mut screen, run, focus);
    let palette = dungeon_art::supported(run);
    if palette {
        dungeon_art::palette_layer(&mut screen);
    }
    let veil = (!hud_in_band())
        .then(|| tray::bar_rect(run, focus, NATIVE_W, NATIVE_H))
        .flatten()
        .map(|(x, y, w, h)| [x, y, w, h]);
    let ground = scenery_cached(run);
    let fine = super::fine_scenery_cached(run).map(|(img, scale)| FineGround { img, scale: scale as u32 });
    let world = crate::world3d::room::world_in(run, focus, room);
    let lights = if board { Vec::new() } else { scene_lights(run, &ground, world.is_none()) };
    let emitters = if board { Vec::new() } else { scene_emitters(run, &lights) };
    RoomScene {
        view: (NATIVE_W, NATIVE_H),
        ground: Some(ground),
        fine,
        camera: (cx + shake.0 as f32, cy + shake.1 as f32),
        under: Some(under),
        figures: capture.figures,
        over: Some(over),
        screen,
        lights,
        emitters,
        shots: capture.shots,
        pickups: capture.pickups,
        paints,
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
        works: None,
        hill: None,
        realm: None,
        board,
    }
}

const WINDOW_MARGIN: i32 = 64;

fn scene_window(run: &Run, focus: Option<u32>, (pw, ph): (i32, i32)) -> (i32, i32, i32, i32) {
    let room = run.room();
    if room.cols <= crate::rules::layout::MAX_COLS && room.rows <= crate::rules::layout::MAX_ROWS {
        return (0, 0, pw, ph);
    }
    let (ox, oy) = view_origin(run, focus);
    (ox - WINDOW_MARGIN, oy - WINDOW_MARGIN, NATIVE_W + 2 * WINDOW_MARGIN, NATIVE_H + 2 * WINDOW_MARGIN)
}

const VEIL_STEPS: usize = 3;
pub const VEIL_COVER: f32 = 0.65;

pub(super) fn veil(c: Rgb) -> Rgb {
    step_down(c, VEIL_STEPS)
}

pub(super) fn darken(img: &mut Img, (x, y, w, h): (i32, i32, i32, i32)) {
    for py in y.max(0)..(y + h).min(img.h) {
        for px in x.max(0)..(x + w).min(img.w) {
            if let Some(c) = img.get(px, py) {
                img.set(px, py, veil(c));
            }
        }
    }
}

const ENTRANCE_TICKS: u32 = 2 * crate::rules::HZ;

fn entrance(run: &Run) -> Option<([f32; 2], f32)> {
    run.enemies
        .iter()
        .filter(|e| e.boss.is_some() && e.hp > 0 && e.age < ENTRANCE_TICKS)
        .min_by_key(|e| e.age)
        .map(|e| ([e.x * UNIT, e.y * UNIT], e.age as f32 / crate::rules::HZ as f32))
}

const DUSTY_LIGHT: f32 = 0.28;

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

const SHOT_LIGHT: (f32, f32) = (16.0, 0.55);
const HOME_LIGHT: (f32, f32) = (64.0, 0.7);

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
    for (x, y, radius, strength) in vfx::bolt_lights(run) {
        out.push(SceneLight {
            x,
            y,
            radius,
            strength,
            hue: Hue::Cold,
            flicker: false,
        });
    }
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

pub fn scene_for_live(run: &Run, w: i32, h: i32, focus: Option<u32>) -> RoomScene {
    during_live(|| scene_for(run, w, h, focus))
}
