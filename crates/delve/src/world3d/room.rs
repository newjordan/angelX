//! A room in 3D, as the game hands it over: whether the host draws rooms in
//! 3D at all, and the room's world as a window places it on its stage.
//!
//! A function of depth ([`Mode`], docs/WORLD-3D.md "Deeper floors"): every
//! floor keeps the 2D black look unless the host asks (the operator,
//! 2026-10-29: "we are going to move away from the 3D after third bosses").
//! `ANGEL_WORLD3D=deep` stands the deep ones ([`DEEP`] and below) up in 3D,
//! the floor between ([`TRANSITION`]) its guardian's hall only;
//! `ANGEL_WORLD3D=1` every room, the kingdom too. A room that stands up, seen from above, stands
//! up lit in the monolith look (`rooms_lit`, docs/REALM-MONOLITH.md "Rooms,
//! lit"), no plate needed. (The painted plates it once had, and the rooms
//! stood up plain in a theme's patches, were cut on 2026-10-20: see
//! docs/WORLD-TIERS.md.)

use std::sync::Arc;

use super::rooms_lit::Lit;
use super::rooms_lit_look::Activity;
use crate::render::overworld::Img;
use crate::rules::{Pack, Room};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Deep,
    Always,
}

impl Mode {
    pub fn from_env(value: Option<&str>) -> Mode {
        match value {
            Some("1") => Mode::Always,
            Some("deep") => Mode::Deep,
            _ => Mode::Off,
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

pub const DEEP: u32 = crate::rules::DEEP_FLOOR;

pub const TRANSITION: u32 = 3;

#[cfg(not(test))]
static MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

#[cfg(test)]
std::thread_local! {
    static MODE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

pub fn set_mode(mode: Mode) {
    if mode == Mode::Always {
        super::kingdom::warm();
    }
    #[cfg(not(test))]
    MODE.store(mode.code(), std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    MODE.with(|c| c.set(mode.code()));
}

pub fn mode() -> Mode {
    #[cfg(not(test))]
    return Mode::of_code(MODE.load(std::sync::atomic::Ordering::Relaxed));
    #[cfg(test)]
    return Mode::of_code(MODE.with(|c| c.get()));
}

pub fn set_enabled(on: bool) {
    set_mode(if on { Mode::Always } else { Mode::Off });
}

pub fn enabled() -> bool {
    mode() == Mode::Always
}

pub fn stands_in_mode(mode: Mode, depth: u32, kind: crate::rules::RoomKind) -> bool {
    use crate::rules::RoomKind;
    match mode {
        Mode::Off => false,
        Mode::Always => true,
        Mode::Deep if kind == RoomKind::DeepVillage => false,
        Mode::Deep => {
            depth >= DEEP || (depth == TRANSITION && matches!(kind, RoomKind::Lair | RoomKind::Pit | RoomKind::Threshold))
        }
    }
}

pub fn stands(depth: u32, kind: crate::rules::RoomKind) -> bool {
    stands_in_mode(mode(), depth, kind)
}

pub fn stands_here(run: &crate::rules::Run) -> bool {
    stands(run.dungeon.depth, run.room().kind)
}

pub const BEAT: f32 = 3.0;

pub fn beat(run: &crate::rules::Run) -> Option<f32> {
    let at = run.deep_beat?;
    let s = run.tick.checked_sub(at)? as f32 / crate::rules::HZ as f32;
    (s < BEAT && stands_here(run)).then_some(s)
}

fn smooth(lo: f32, hi: f32, s: f32) -> f32 {
    let t = ((s - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn beat_dark(s: f32) -> f32 {
    1.0 - smooth(0.2, 0.9, s)
}

pub fn beat_zoom(s: f32) -> f32 {
    1.0 + 0.45 * (1.0 - smooth(0.0, BEAT * 0.9, s))
}

pub fn beat_rise(s: f32) -> f32 {
    smooth(0.35, BEAT * 0.8, s)
}

pub fn lamp_up(rise: f32, near: f32) -> f32 {
    let start = near.clamp(0.0, 1.0) * 0.75;
    ((rise - start) / 0.25).clamp(0.0, 1.0)
}

#[derive(Clone, Debug)]
pub struct RoomWorld {
    pub lit: Arc<Lit>,
    pub room: (i32, i32),
    pub moment: Moment,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    pub activity: Activity,
    pub chest: Option<[f32; 3]>,
    pub time: f32,
    pub rise: f32,
    pub party: [Option<[f32; 2]>; 4],
}

impl Default for Moment {
    fn default() -> Self {
        Self { activity: Activity::Steady, chest: None, time: 0.0, rise: 1.0, party: [None; 4] }
    }
}

impl RoomWorld {
    pub fn lit_world(lit: Arc<Lit>, room: (i32, i32)) -> RoomWorld {
        RoomWorld { lit, room, moment: Moment::default() }
    }

    pub fn lit(&self) -> &Arc<Lit> {
        &self.lit
    }

    pub fn key(&self) -> u64 {
        self.lit.key
    }

    pub fn scale(&self) -> f32 {
        self.room.0.max(1) as f32 / self.lit.stage().0.max(1.0)
    }

    pub fn lit_staged(
        &self,
        camera: (f32, f32),
        lean: ([f32; 2], f32),
        view: (f32, f32),
        sparks: &[angel_stage::look::Lamp],
        seen: &dyn Fn([f32; 3], [f32; 3]) -> bool,
    ) -> super::rooms_lit::Staged {
        let k = self.scale().max(1e-6);
        let rect = [camera.0 / k, camera.1 / k, (camera.0 + view.0) / k, (camera.1 + view.1) / k];
        self.lit.staged(self.placing(camera, lean), lean.1, view, rect, &self.moment, sparks, seen)
    }

    pub fn placing(&self, camera: (f32, f32), (focus, zoom): ([f32; 2], f32)) -> (f32, [f32; 3]) {
        let offset = [focus[0] * (1.0 - zoom) - camera.0 * zoom, 0.0, focus[1] * (1.0 - zoom) - camera.1 * zoom];
        (self.scale() * zoom, offset)
    }
}

impl RoomWorld {
    pub fn picture(&self) -> Img {
        self.lit.picture(self.room)
    }
}

pub fn world(room: &Room, pack: Pack, depth: u32, size: (i32, i32)) -> Option<RoomWorld> {
    if !stands(depth, room.kind) {
        return None;
    }
    super::rooms_lit::lit_for(room, pack).map(|lit| RoomWorld::lit_world(lit, size))
}

pub fn world_in(run: &crate::rules::Run, focus: Option<u32>, size: (i32, i32)) -> Option<RoomWorld> {
    let mut world = world(run.room(), run.dungeon.pack, run.dungeon.depth, size)?;
    world.moment = moment_of(run, focus);
    Some(world)
}

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
        party: {
            let mut feet = [None; 4];
            for (slot, h) in feet.iter_mut().zip(run.players.values().filter(standing)) {
                *slot = Some([h.x * UNIT, h.y * UNIT]);
            }
            feet
        },
    }
}

#[cfg(any(test, feature = "testing"))]
pub struct Proof {
    pub id: &'static str,
    pub run: crate::rules::Run,
    pub behind: [f32; 2],
    pub in_front: [f32; 2],
}

#[cfg(any(test, feature = "testing"))]
pub fn proof(id: &str) -> Option<Proof> {
    use super::pilot::Source;
    use crate::render::overworld::arena::UNIT;
    use crate::rules::{Rng, Run, RoomKind, home::Home, layout, rooms, world};
    let pilot = super::pilot::item(id)?;
    let template = pilot.room();
    let shapes = angel_stage::set::Set::parse(&super::rooms_lit::room_text(&template, &pilot.dressing, true, None)).ok()?.shapes;
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
                _ => laid_as_item()?,
            }
        }
        Source::Great { .. } | Source::Nook(_) | Source::Sanctuary | Source::Vault | Source::Forge => laid_as_item()?,
        Source::Chamber { pack, small, doors } => {
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

#[cfg(any(test, feature = "testing"))]
fn spots(room: &Room, shapes: &[angel_stage::set::Shape]) -> ([f32; 2], [f32; 2]) {
    use super::blockout::{TILE, walkable};
    let k = 1.0;
    let height = |c: i32, r: i32| -> f32 {
        let (x, z) = ((c as f32 + 0.5) * TILE / k, (r as f32 + 0.5) * TILE / k);
        shapes
            .iter()
            .filter(|s| !s.on_floor())
            .filter(|s| {
                let quarter = (s.turn.rem_euclid(180.0) - 90.0).abs() < 45.0;
                let (sx, sz) = if quarter { (s.size[2], s.size[0]) } else { (s.size[0], s.size[2]) };
                let (dx, dz) = ((x - s.at[0]) / (sx / 2.0).max(1e-3), (z - s.at[2]) / (sz / 2.0).max(1e-3));
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
    let behind = best(&|c, r| (open(c, r) && r + 1 < rows - 1 && !open(c, r + 1)).then(|| height(c, r + 1)))
        .map_or([cols as f32 * TILE / 2.0, rows as f32 * TILE / 2.0], |(c, r, _)| {
            [(c as f32 + 0.5) * TILE, (r + 1) as f32 * TILE - 2.0]
        });
    let (bc, br) = ((behind[0] / TILE) as i32, (behind[1] / TILE) as i32);
    let clear = |c: i32, r: i32| (c - bc).abs() > 3 || (r - br).abs() > 3;
    let in_front = best(&|c, r| (open(c, r) && r - 1 >= 1 && !open(c, r - 1) && clear(c, r)).then(|| height(c, r - 1).min(24.0)))
        .map_or([cols as f32 * TILE / 2.0 + 24.0, rows as f32 * TILE / 2.0], |(c, r, _)| {
            [(c as f32 + 0.5) * TILE, r as f32 * TILE + 10.0]
        });
    (behind, in_front)
}

#[cfg(any(test, feature = "testing"))]
pub fn proof_run() -> crate::rules::Run {
    proof("village").expect("the Village is an item").run
}

#[cfg(test)]
#[path = "../../tests/world3d/room__tests.rs"]
mod depth_tests;
