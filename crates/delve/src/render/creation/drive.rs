//! The Test drive: the made creature stands in the real arena renderer. A fixture floor (a
//! throwaway `Run`, never the live one) is drawn by `arena::frame_for`; the creature is a sprite
//! placed with `arena::frame_px`; the training foes are the floor's own enemies, so a blow flashes
//! them through the game's own hit path. The live fight is never touched.

use super::super::genie::{self, centred, paper};
use super::super::overworld::arena;
use super::super::overworld::arena::pfx::{Pen, ramps};
use super::super::overworld::ink::{Img, text_width};
use super::baked::{self, Motion, Reel, Way};
use crate::rules::creation::drive::{ATTACK_TICKS, Hit};
use crate::rules::creation::{Act, Drive, Pose, Session};
use crate::rules::{Run, Tile};

/// A fixture floor: the room, where the foes and the friend stand, where a creature starts.
pub struct Floor {
    pub run: Run,
    pub foes: Vec<(f32, f32)>,
    pub friend: (f32, f32),
    pub spawn: (f32, f32),
}

/// What the friend says when met, in turn.
pub const FRIEND_LINES: [&str; 4] = [
    "Well met, strange one!",
    "It is... rather large.",
    "Does it bite?",
    "Ada approves!",
];

impl Floor {
    /// A crypt room with its stone cleared: two skeletons to strike and a knight to meet.
    #[cfg(any(test, feature = "testing"))]
    pub fn new(friend: &str) -> Self {
        use crate::rules::{EnemyKind, Pack, RoomKind};
        let mut run = Run::new(11, 1, Some(friend));
        let i = run
            .dungeon
            .rooms
            .iter()
            .position(|room| room.kind == RoomKind::Fight && !room.great())
            .unwrap_or(run.at);
        run.enter_for_test(i);
        run.dungeon.pack = Pack::Cavern;
        run.calm_for_test();
        run.enemies.clear();
        run.projectiles.clear();
        // The room keeps its own stone; every standing place is snapped to open floor.
        let foes = vec![Self::open(&run, (31.0, 16.0)), Self::open(&run, (39.0, 21.0))];
        for &(x, y) in &foes {
            run.spawn_at_for_test(EnemyKind::Skeleton, x, y);
        }
        for e in &mut run.enemies {
            e.age = 100;
            e.hurt = 0;
        }
        let (spawn, friend_at) = (Self::open(&run, (22.0, 20.0)), Self::open(&run, (12.0, 17.0)));
        for (id, at) in [(1, spawn), (2, friend_at)] {
            if let Some(h) = run.players.get_mut(&id) {
                (h.x, h.y) = at;
                h.invulnerable = 0;
            }
        }
        Self {
            run,
            foes,
            friend: friend_at,
            spawn,
        }
    }

    /// The open floor nearest `at`: plain floor all round it, so a sprite's feet fit.
    #[cfg(any(test, feature = "testing"))]
    fn open(run: &Run, at: (f32, f32)) -> (f32, f32) {
        let room = run.room();
        let unit = crate::rules::TILE_UNITS;
        let (c0, r0) = ((at.0 / unit) as i32, (at.1 / unit) as i32);
        let clear = |c: i32, r: i32| (-1..=1).all(|dr| (-1..=1).all(|dc| matches!(room.tile(c + dc, r + dr), Tile::Floor)));
        for ring in 0..16i32 {
            for dr in -ring..=ring {
                for dc in -ring..=ring {
                    if (dr.abs().max(dc.abs())) == ring && clear(c0 + dc, r0 + dr) {
                        return (((c0 + dc) as f32 + 0.5) * unit, ((r0 + dr) as f32 + 0.5) * unit);
                    }
                }
            }
        }
        at
    }

    /// The open floor nearest `at`.
    #[cfg(any(test, feature = "testing"))]
    pub fn standing(&self, at: (f32, f32)) -> (f32, f32) {
        Self::open(&self.run, at)
    }

    /// Feet may stand on the room's plain floor, nowhere else.
    pub fn walkable(&self, x: f32, y: f32) -> bool {
        let room = self.run.room();
        let (col, row) = ((x / crate::rules::TILE_UNITS).floor() as i32, (y / crate::rules::TILE_UNITS).floor() as i32);
        matches!(room.tile(col, row), Tile::Floor) && x > 3.0 && y > 10.5
    }

    /// Put the floor as the drive has it at `tick`: the camera knight under the creature, the foe
    /// it struck flashing as the game flashes a hurt enemy.
    pub fn prepare(&mut self, drive: &Drive, tick: u64) {
        self.run.tick = tick;
        if let Some(h) = self.run.players.get_mut(&1) {
            (h.x, h.y) = (drive.x, drive.y);
        }
        let hit: Option<Hit> = drive.last_hit.filter(|h| tick.saturating_sub(h.tick) < 8);
        // A foe that strikes back lunges at the creature for a moment (out and back over six ticks).
        let lunging = drive.hurt.filter(|h| tick.saturating_sub(h.tick) < 6);
        for (i, e) in self.run.enemies.iter_mut().enumerate() {
            e.hurt = match hit {
                Some(h) if h.foe == i => (8 - tick.saturating_sub(h.tick)) as u8,
                _ => 0,
            };
            if let Some(&(bx, by)) = self.foes.get(i) {
                let push = lunging.filter(|h| h.foe == i).map_or(0.0, |h| {
                    let t = tick.saturating_sub(h.tick);
                    0.5 * if t < 3 { t as f32 + 1.0 } else { (6 - t) as f32 }
                });
                let (dx, dy) = (drive.x - bx, drive.y - by);
                let n = dx.hypot(dy).max(0.001);
                (e.x, e.y) = (bx + dx / n * push, by + dy / n * push);
            }
        }
    }
}

/// What the drive's creature is doing. A set with two ways is shown the way it always was: east along
/// the floor (turned about for west), south for up and down. A set with eight faces where it walks.
fn motion(drive: &Drive, tick: u64, eight: bool) -> Motion {
    match drive.pose(tick) {
        Pose::Windup => Motion::Windup,
        Pose::Strike => Motion::Strike,
        Pose::Rest if drive.moving => {
            let way = if eight { drive.way } else if drive.vertical { Way::South } else { Way::East };
            if drive.running { Motion::Run(way) } else { Motion::Walk(way) }
        }
        Pose::Rest => Motion::Idle,
    }
}

/// The clip an act plays.
fn reel(act: Act) -> Reel {
    match act {
        Act::Cast => Reel::Cast,
        Act::Hit => Reel::Hit,
        Act::Fell => Reel::Fall,
        Act::Rose => Reel::Rise,
    }
}

/// How long a foe's blow marks the creature: its number rises, the arc between them fades.
const HURT_MARK: u64 = 20;

/// The room, the creature, the foes' hurt, the friend's answer and the plate, at native size.
pub fn frame(session: &Session, floor: &Floor, tick: u64, w: i32, h: i32) -> Img {
    frame_with(session, floor, tick, w, h, true)
}

/// The frame as the drive drew a set with eight ways before it played its clips at their own times: the
/// library's fixed smash over the clip, the stance cut to when it ends. The proof's "before", nothing else.
#[cfg(test)]
pub fn frame_fixed(session: &Session, floor: &Floor, tick: u64, w: i32, h: i32) -> Img {
    frame_with(session, floor, tick, w, h, false)
}

fn frame_with(session: &Session, floor: &Floor, tick: u64, w: i32, h: i32, clips: bool) -> Img {
    let drive = session.drive.clone().unwrap_or_else(|| Drive::new(floor.spawn, &session.creation.stats(), tick));
    let mut out = arena::frame_for(&floor.run, w, h, Some(1));
    let at = |x: f32, y: f32| arena::frame_px(&floor.run, w, h, Some(1), x, y);
    // Friends and foes are drawn by the arena; the creature stands over its camera knight.
    let set = baked::for_creation(&session.creation);
    let eight = set.eight_way();
    let m = motion(&drive, tick, eight);
    let walking = matches!(m, Motion::Walk(_) | Motion::Run(_));
    let age = tick.saturating_sub(drive.born);
    // A set with eight ways stands, walks and strikes facing where it was last pointed; one with two
    // faces its art about when it walks left.
    let facing = if eight { drive.way } else { Way::South };
    // A set with clips of its own plays them at their own frame times: a cast, a flinch from a foe's
    // blow, a fall and the rise from it, and the blow itself, each melting into the stance at its end
    // (a creature that walks on in the settle walks). Where the set has none the library's fixed
    // smash plays: from the first tick of the blow through its recovery, windup and strike poses held.
    let own_clips = clips && eight;
    let acted = drive.acting(tick).filter(|_| own_clips && !(walking && drive.settling(tick)));
    let played = match acted {
        Some((act, since)) => baked::figure_shot(&session.creation, reel(act), since, facing, age),
        None if own_clips && !walking => drive.attack_at.and_then(|at| baked::figure_shot(&session.creation, Reel::Blow, tick.saturating_sub(at), facing, age)),
        None => None,
    };
    let since = drive.attack_at.map(|at| tick.saturating_sub(at)).filter(|_| !walking && !own_clips);
    let (mut figure, drop) = match played {
        Some((figure, drop)) => (figure, drop),
        None if own_clips => (baked::figure_facing(&session.creation, m, facing, age, drive.walked, None), set.drop_facing(m, facing)),
        None => (baked::figure_fixed(&session.creation, m, facing, age, drive.walked, since), set.drop_facing(m, facing)),
    };
    if drive.facing < 0 && !eight {
        figure = figure.flip_h();
    }
    // A blow carries a stand-in a pixel forward and a hop in the walk keeps its feet honest; a set
    // that came with its own animation has its own bob and lunge, and its soles meet the ground
    // by the anchor it declares (the front walk is drawn lower).
    let (fx, fy) = at(drive.x, drive.y);
    let own = set.carries_motion();
    let hop = if drive.moving && !own { i32::from((drive.walked / 0.55) as u64 % 2 == 1) } else { 0 };
    let lunge = match m {
        Motion::Strike if !own => 2 * i32::from(drive.facing),
        _ => 0,
    };
    // A training foe winds up (a blood-red ring closing on it), then strikes: an arc from it toward the
    // creature, under the creature, and the damage over its head, rising.
    for (i, p) in drive.tells() {
        let (px, py) = at(floor.foes[i.min(floor.foes.len() - 1)].0, floor.foes[i.min(floor.foes.len() - 1)].1);
        let mut pen = Pen::new(&mut out, 97);
        // Closing on the foe and burning brighter as the blow nears (a ring's fade is how much of it is gone).
        pen.ring((px as f32, py as f32 - 8.0), 13.0 - p * 7.0, 1.7, 0.8, ramps::BLOOD, 0.55 - 0.5 * p);
    }
    let hurt = drive.hurt.filter(|h| tick.saturating_sub(h.tick) < HURT_MARK);
    if let Some(hurt) = hurt.filter(|h| tick.saturating_sub(h.tick) < 5) {
        let since = tick.saturating_sub(hurt.tick);
        let (px, py) = at(floor.foes[hurt.foe.min(floor.foes.len() - 1)].0, floor.foes[hurt.foe.min(floor.foes.len() - 1)].1);
        let (ax, ay) = (px as f32, py as f32 - 8.0);
        let (bx, by) = (fx as f32, (fy - figure.h / 2) as f32);
        let reach = (since as f32 + 1.0) / 5.0;
        let (ex, ey) = (ax + (bx - ax) * reach, ay + (by - ay) * reach);
        let (sx, sy) = (ax + (bx - ax) * (reach - 0.4).max(0.0), ay + (by - ay) * (reach - 0.4).max(0.0));
        out.line(sx as i32, sy as i32, ex as i32, ey as i32, 'c');
    }
    // Shadow on the floor, then the sprite, feet at the bottom centre.
    out.ellipse(fx as f32, fy as f32 - 1.0, figure.w as f32 * 0.42, 3.5, 'K');
    out.stamp(&figure, fx - figure.w / 2 + lunge, fy - figure.h - hop + drop);
    // The blow itself: a short bright arc where it lands.
    if let Some(age) = drive.blow_age(tick) {
        if age < 5 {
            // Ahead of the creature: left or right for the two-way art, the way it faces for eight (below the
            // feet when it faces the camera, above the head when it faces away, never over its body).
            let (ox, oy) = if eight { drive.way.axes() } else { (drive.facing, 0) };
            let dir = if ox != 0 { i32::from(ox) } else { i32::from(drive.facing) };
            let (cx, cy) = (fx + i32::from(ox) * (figure.w / 2 + 6), fy - figure.h / 2 + 4 + i32::from(oy) * (figure.h / 2 + 4));
            for i in 0..7 {
                let a = (i as f32 - 3.0) * 0.42;
                let (px, py) = (cx + (a.cos() * (9.0 - age as f32)) as i32 * dir, cy + (a.sin() * (11.0 - age as f32)) as i32);
                out.put(px, py, if i % 2 == 0 { '4' } else { 'T' });
            }
        }
    }
    // A cast's release: a ring opens ahead of the creature on the clip's own cast frame.
    if let (Some((Act::Cast, since)), Some(cast)) = (drive.acting(tick), drive.timing.cast)
        && since >= cast.event
        && since < cast.event + 14
    {
        let (ox, oy) = drive.way.axes();
        let (cx, cy) = (fx + i32::from(ox) * (figure.w / 2 + 2), fy - figure.h / 2 + i32::from(oy) * (figure.h / 3));
        let t = (since - cast.event) as f32;
        let mut pen = Pen::new(&mut out, 91);
        pen.ring((cx as f32, cy as f32), 3.0 + t * 1.4, 1.4, 0.8, ramps::GOLD, (t / 13.0).min(1.0));
    }
    // The friend answers a meeting.
    if let Some((said, n)) = drive.spoke.filter(|(t, _)| tick.saturating_sub(*t) < 70) {
        let _ = said;
        let line = FRIEND_LINES[(n as usize + FRIEND_LINES.len() - 1) % FRIEND_LINES.len()];
        let (px, py) = at(floor.friend.0, floor.friend.1);
        let bubble = genie::bubble(line, 92);
        out.stamp(&bubble, (px - 14).clamp(2, w - bubble.w - 2), (py - 30 - bubble.h).max(24));
    }
    // The blow's mark on the foe it struck: a ring and the damage, rising.
    if let Some(hit) = drive.last_hit.filter(|h| tick.saturating_sub(h.tick) < 16) {
        let since = tick.saturating_sub(hit.tick);
        let (px, py) = at(floor.foes[hit.foe.min(floor.foes.len() - 1)].0, floor.foes[hit.foe.min(floor.foes.len() - 1)].1);
        let mut pen = Pen::new(&mut out, 77);
        pen.ring((px as f32, py as f32 - 8.0), 4.0 + since as f32 * 1.1, 1.2, 0.8, ramps::GOLD, (since as f32 / 14.0).min(1.0));
        let text = format!("-{}", hit.damage);
        let (tx, ty) = (px - text_width(&text) / 2, py - 24 - since as i32 / 2);
        out.text(tx + 1, ty + 1, &text, 'K');
        out.text(tx, ty, &text, '4');
    }
    if let Some(hurt) = hurt {
        let since = tick.saturating_sub(hurt.tick);
        let text = format!("-{}", hurt.damage);
        let (tx, ty) = (fx - text_width(&text) / 2, fy - figure.h - 6 - since as i32 / 2);
        out.text(tx + 1, ty + 1, &text, 'K');
        out.text(tx, ty, &text, 'c');
    }
    // Black bands under the flow's line and the keys, so nothing of the room's walls reads through.
    out.rect(0, 0, w, 20, 'K');
    out.rect(0, h - 13, w, 13, 'K');
    // The plate keeps to the side of the room the creature is not in.
    plate(&mut out, session, &drive, tick, if drive.x > 26.0 { 4 } else { w - 156 });
    // A creation whose role has looks flips them with the number keys, here as on the scroll's Paint page.
    let looks = session.creation.looks().len();
    let keys = if looks > 1 { format!("ARROWS: MOVE  F: STRIKE  E: MEET  1-{looks}: LOOK  Q: BACK") } else { "ARROWS: MOVE   F: STRIKE   E: MEET   Q: BACK".into() };
    centred(&mut out, h - 10, &keys, 'o', false);
    out
}

/// The parchment plate: name, power gauge and the stats the parts derived.
fn plate(out: &mut Img, session: &Session, drive: &Drive, tick: u64, x: i32) {
    let c = &session.creation;
    let stats = c.stats();
    let (used, budget) = session.power();
    // The keys a creature has only when its art brings the clips (the bottom line has the ones every creature has).
    let timing = baked::for_creation(c).timing();
    let clip_keys = match (timing.cast.is_some(), timing.death.is_some()) {
        (true, true) => Some("R: CAST   X: FALL"),
        (false, true) => Some("X: FALL"),
        (true, false) => Some("R: CAST"),
        (false, false) => None,
    };
    let ph = if clip_keys.is_some() { 77 } else { 68 };
    let mut p = Img::new(152, ph);
    paper(&mut p, 0, 0, 152, ph, 0x2bd1);
    let name = c.name.to_uppercase();
    let name = if name.chars().count() > 21 { format!("{}..", name.chars().take(19).collect::<String>()) } else { name };
    genie::bold(&mut p, 6, 5, &name, 'n');
    let set = baked::for_creation(c).name();
    // "HERO - KNIGHT STAND-IN" for a real stand-in when it fits the plate (a longer set name drops the tag
    // rather than the name); a finished character from the pipeline is only named.
    let tag = if baked::for_creation(c).stand_in() { " STAND-IN" } else { "" };
    let line = format!("{} - {set}{tag}", c.role.name().to_uppercase());
    let line = if line.chars().count() > 24 { format!("{} - {set}", c.role.name().to_uppercase()) } else { line };
    p.text(6, 15, &line.chars().take(24).collect::<String>(), 'P');
    // The gauge, the scroll's own: ink outline, paper, ochre fill.
    let (gx, gy, gw) = (42, 25, 66);
    genie::bold(&mut p, 6, gy + 1, "POWER", 'n');
    p.rect(gx, gy, gw, 9, 'b');
    p.rect(gx + 1, gy + 1, gw - 2, 7, 't');
    for dx in 0..(gw - 2) * i32::from(used.min(budget)) / i32::from(budget.max(1)) {
        p.rect(gx + 1 + dx, gy + 1, 1, 7, if used > budget { 'p' } else if dx % 10 == 9 { 'r' } else { 'R' });
        p.put(gx + 1 + dx, gy + 1, 'o');
    }
    let count = format!("{used}/{budget}");
    p.text(gx + gw + 5, gy + 2, &count, 'n');
    p.text(6, 38, &format!("HP {}  SPD {}", stats.hp, stats.speed), 'n');
    p.text(6, 47, &format!("MELEE {}  MAGIC {}", stats.melee, stats.magic), 'n');
    // What it last did to a foe, or what a foe last did to it, whichever was later.
    let last = match (drive.last_hit, drive.hurt) {
        (_, Some(h)) if drive.last_hit.is_none_or(|l| h.tick >= l.tick) => format!("ARMOUR {}  TOOK -{}", stats.armour, h.damage),
        (Some(h), _) => format!("ARMOUR {}  HIT -{}", stats.armour, h.damage),
        _ => "ARMOUR ".to_string() + &stats.armour.to_string(),
    };
    p.text(6, 57, &last, 'P');
    if let Some(keys) = clip_keys {
        p.text(6, 66, keys, 'n');
    }
    let _ = tick;
    out.stamp(&p, x, 24);
    // A strike landing over the plate's last line would hide it: nothing else lives here.
    let _ = (text_width, ATTACK_TICKS);
}

/// The whole Test drive screen, over the room view: the arena, then the flow's progress line.
pub fn paint(out: &mut Img, session: &Session, floor: &Floor, tick: u64) {
    let w = out.w.min(super::WIDTH);
    let k = (out.w / super::WIDTH).min(out.h / 150).max(1);
    let native_h = (out.h / k).min(super::HEIGHT.max(224));
    let mut canvas = frame(session, floor, tick, super::WIDTH, native_h);
    super::progress(&mut canvas, session);
    let _ = w;
    out.stamp_times(&canvas, (out.w - super::WIDTH * k) / 2, (out.h - native_h * k) / 2, k);
}
