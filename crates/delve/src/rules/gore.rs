//! Finishers: how a knight's last blow on a minion looks. The rules never
//! read any of this back; the kill is what it was (who dies, when, what
//! they drop, where the living stand). A finisher only decides how the
//! body is shown: kicked back and pinned, knocked skidding, beheaded, cut
//! in half, burst into chunks, splattered.
//!
//! - **The choice** is made once, on the host, as the dead are settled
//!   (`finish_slain`), from what the blow was (`Blow`, stamped where the
//!   blow killed), who struck it, how far they stood, what stone lies
//!   behind it and a seeded variant (a hash of the monster and the tick,
//!   never the dice). It lands in `Run::gore`, which the wire's tree
//!   carries beside the run (`wire::DRAWN`) so every seat draws the same
//!   body. A seat that predates this reads no `gore` and draws the
//!   ordinary death.
//! - **The picture** is a pure function of a `Gore` and the run's tick
//!   (`render::overworld::arena::gore`): the body's flight, its pieces, the
//!   spray and the stains are worked out each frame from the record, not
//!   kept, so a record is small and a frame costs a few hundred motes at
//!   most. Records are capped (`MAX_LIVE`), age out (`LIFE`), and a room's
//!   end clears them.
//! - **The sound** goes the way the game's sounds go (`Run::sounds`); one
//!   that belongs to a later moment of the picture (the thunk of a body
//!   into stone) waits in `Feel::echoes` until its tick.
//! - **Hit-stop** is the game's own (`hitstop`), and off for finishers by
//!   default (`Tune::hitstop_finisher`): a real hold stands the rules'
//!   time still, which would move every run that has a kill in it. The
//!   picture holds its own impact frame (`HOLD`) and kicks the camera
//!   (`kick`) without touching the rules.
//! - Bosses, guardians, wards and dummies have none; the Pit's beast and
//!   the dragon neither.

use super::*;
use feel::Feel;

/// Ticks the impact frame is held before the body moves.
pub const HOLD: u64 = 3;
/// Ticks a record lives, stains and all.
pub const LIFE: u64 = 8 * HZ as u64;
/// Ticks a body lies or hangs before it fades from the floor and wall.
pub const BODY: u64 = 4 * HZ as u64;
/// The most records alive at once, and the most one tick may add: a room
/// of deaths at once shows its nearest few and the rest die as they did.
pub const MAX_LIVE: usize = 14;
pub const MAX_NEW: usize = 5;
/// How near a knight stands, centre to centre in arena units, for a kick.
pub const POINT_BLANK: f32 = 4.0;
/// How far a kicked body flies at most, and how far it skids when shot at
/// range.
pub const KICK_REACH: f32 = 5.0;
pub const SKID: f32 = 2.6;

/// How the last blow ended the monster.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cut {
    /// Point-blank, ranged: kicked back, the killshot takes it in the
    /// air and pins it to the stone behind, or the floor.
    Kick,
    /// At range: knocked back along the shot's line, skidding.
    Punch,
    /// A blade took the head: it tumbles, the neck sprays.
    Behead,
    /// A blade cut it across: two halves fall apart.
    Halve,
    /// Splash or an explosive blow: chunks.
    Burst,
    /// A slime flattened.
    Splat,
    /// A death with no blow to tell: a spray and a stain.
    Pop,
}

/// What a body is pinned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pin {
    No,
    Wall,
    Floor,
}

/// What a monster is made of, for what comes out of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    Flesh,
    Bone,
    Ichor,
    Ash,
    Shard,
    Wood,
}

/// What a monster is made of; `None`: it has no finisher (a spirit comes
/// apart as it always did, and the great ones are the bosses' business).
pub fn body(kind: EnemyKind) -> Option<Body> {
    use EnemyKind as K;
    Some(match kind {
        K::Dragon | K::Boss | K::PitTyrant | K::Risen | K::Broodmarrow | K::Dummy | K::Ward | K::Whirlgrave => {
            return None;
        }
        K::Wraith | K::Wisp | K::Hollow | K::Mirrorling => return None,
        K::Skeleton | K::Oathbound | K::Lich => Body::Bone,
        K::Slime | K::Spitter | K::Leech | K::Spiderling | K::Silkmother | K::Biter | K::BogHand | K::Drowned => {
            Body::Ichor
        }
        K::Imp | K::Demon | K::Hound | K::Hob => Body::Ash,
        K::Geode | K::Shard => Body::Shard,
        K::Mimic => Body::Wood,
        _ => Body::Flesh,
    })
}

/// Whether a monster has a head to take.
pub fn headed(kind: EnemyKind) -> bool {
    use EnemyKind as K;
    matches!(
        kind,
        K::Skeleton
            | K::Goblin
            | K::Sapper
            | K::Necromancer
            | K::Hexer
            | K::Shaman
            | K::Marksman
            | K::Hob
            | K::Ringer
            | K::Pavise
            | K::Flesher
            | K::Imp
            | K::Demon
            | K::Lich
            | K::Warboar
            | K::Oathbound
            | K::Drowned
    )
}

/// One monster's finishing, from the tick of the blow.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gore {
    pub id: u32,
    pub born: u64,
    pub kind: EnemyKind,
    pub cut: Cut,
    /// Which of its variants (0 to 2).
    pub var: u8,
    pub pin: Pin,
    /// What stands in a pinned body: an arrow, a bolt or a thrown blade.
    pub shaft: Shot,
    /// One of the best: the picture kicks the camera.
    pub big: bool,
    /// Ticks the body moves, after the held frame.
    pub span: u8,
    /// Where it fell, and how big it was.
    pub x: f32,
    pub y: f32,
    pub r: f32,
    /// The blow's line, a unit vector pointing away from the one who struck.
    pub dx: f32,
    pub dy: f32,
    /// Where the body ends: the stone it is pinned to, the floor it skids
    /// to, or where the head lands.
    pub to_x: f32,
    pub to_y: f32,
    /// Where the one who struck stood.
    pub from_x: f32,
    pub from_y: f32,
    /// A body a knight's blade cut in two: the cut's slope across it, in
    /// degrees from level as drawn (falling to the right when positive).
    /// The upper piece slides off down it and falls. None: the older cut
    /// across the blow, the halves parting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slant: Option<i8>,
}

impl Gore {
    /// Ticks since the blow.
    pub fn age(&self, tick: u64) -> u64 {
        tick.saturating_sub(self.born)
    }

    /// Whether the record has outlived its stains.
    pub fn over(&self, tick: u64) -> bool {
        self.age(tick) >= LIFE
    }

    /// Whether this record is the one the death at `(x, y)` made.
    pub fn marks(&self, x: f32, y: f32) -> bool {
        self.x == x && self.y == y
    }
}

/// The blow that killed, stamped where it landed.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Blow {
    /// A party shield takes heads; the channel cuts across a body.
    Team { dir: (f32, f32), halve: bool },
    /// A knight's shot, `dir` the way it flew; `burst` an exploding one.
    Shot { shot: Shot, dir: (f32, f32), burst: bool },
    /// A sword or a forged blade across it; fists when `fists`. The stroke
    /// it was, and where it stood in the knight's three-stroke chain.
    Swing { dir: (f32, f32), forged: bool, fists: bool, stroke: melee::Stroke, combo: u8 },
    /// A bomb or a burst that went off at `at`.
    Blast { at: (f32, f32) },
}

/// A blow with the monster and the knight it fell to.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Blown {
    pub id: u32,
    pub by: u32,
    pub blow: Blow,
}

/// A monster settled as dead this tick.
#[derive(Clone, Copy, Debug)]
pub(super) struct Slain {
    pub id: u32,
    pub kind: EnemyKind,
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub foe: u32,
    pub boss: bool,
}

/// The sounds a finisher makes, by name: the kick, a body into stone, a
/// blade through, a splat, bones, a burst; then a head taken (the thwack
/// and the spray) and landing, a body cut through (the wet cleave) and its
/// halves falling.
pub const SOUNDS: [&str; 10] = ["gore_kick", "gore_thunk", "gore_chop", "gore_splat", "gore_bones", "gore_burst", "gore_behead", "gore_head_thud", "gore_bisect", "gore_slump"];

/// A blade's finisher as it sounds: a head taken now and the head landing
/// when the picture has it land; a body cut through now and its halves
/// falling when they reach the floor. Bones crack and clatter as before.
fn cut_sounds(cut: Cut, bony: bool, span: u8) -> (&'static str, (u64, &'static str)) {
    match (cut, bony) {
        (_, true) => ("gore_chop", (HOLD + 10, "gore_bones")),
        (Cut::Behead, false) => ("gore_behead", (HOLD + u64::from(span), "gore_head_thud")),
        _ => ("gore_bisect", (HOLD + u64::from(span), "gore_slump")),
    }
}

impl Feel {
    /// A knight's blow just killed monster `id`; the settling of the dead
    /// will turn it into a finisher.
    pub(super) fn blow(&mut self, id: u32, by: u32, blow: Blow) {
        if self.blows.len() < 64 {
            self.blows.push(Blown { id, by, blow });
        }
    }
}

impl Run {
    /// The tick's dead minions get their finishers: chosen, recorded for
    /// the picture, sounded. Bosses and the rest (`body`) are left alone.
    pub(super) fn finish_slain(&mut self, slain: &[Slain]) {
        let blows = std::mem::take(&mut self.feel.blows);
        let mut made: Vec<(Gore, Option<&'static str>, Option<(u64, &'static str)>)> = Vec::new();
        {
            let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
            for s in slain.iter().filter(|s| !s.boss && body(s.kind).is_some()) {
                if made.len() >= MAX_NEW {
                    break;
                }
                let blow = blows.iter().rev().find(|b| b.id == s.id);
                made.push(self.finisher(&grid, s, blow));
            }
        }
        for (gore, now, later) in made {
            if gore.big {
                self.hitstop.request(&self.tune, hitstop::Impact::Finisher);
            }
            if let Some(name) = now {
                self.sounds.push(name);
            }
            if let Some((after, name)) = later
                && self.feel.echoes.len() < 16
            {
                self.feel.echoes.push((self.tick + after, name));
            }
            if self.gore.len() >= MAX_LIVE {
                self.gore.remove(0);
            }
            self.gore.push(gore);
        }
    }

    /// What the blow `blow` that felled `s` did to it: the record, the
    /// sound now and the sound to come.
    fn finisher(
        &self,
        grid: &Grid,
        s: &Slain,
        blow: Option<&Blown>,
    ) -> (Gore, Option<&'static str>, Option<(u64, &'static str)>) {
        let stuff = body(s.kind).unwrap_or(Body::Flesh);
        let h = mix(u64::from(s.id) << 20 ^ self.tick ^ (self.seed & 0xffff));
        let var = (h % 3) as u8;
        let by = blow.map_or(s.foe, |b| b.by);
        let hero = self.players.get(&by).map(|h| (h.x, h.y));
        let (hx, hy) = hero.unwrap_or((s.x, s.y));
        let mut gore = Gore {
            id: s.id,
            born: self.tick,
            kind: s.kind,
            cut: Cut::Pop,
            var,
            pin: Pin::No,
            shaft: Shot::Arrow,
            big: false,
            span: 10,
            x: s.x,
            y: s.y,
            r: s.r,
            dx: 0.0,
            dy: 1.0,
            to_x: s.x,
            to_y: s.y,
            from_x: hx,
            from_y: hy,
            slant: None,
        };
        let away = |from: (f32, f32)| {
            let (ux, uy) = unit(s.x - from.0, s.y - from.1);
            if ux == 0.0 && uy == 0.0 { (0.0, 1.0) } else { (ux, uy) }
        };
        // How far a body gets along a line before stone, and where that is.
        let along = |d: (f32, f32), reach: f32| -> (Option<(f32, f32)>, f32) {
            let end = (s.x + d.0 * reach, s.y + d.1 * reach);
            match grid.shot_stop_body((s.x, s.y), end, 0.0) {
                Some(p) => (Some(p), (p.0 - s.x).hypot(p.1 - s.y)),
                None => (None, reach),
            }
        };
        let (mut now, mut later): (Option<&'static str>, Option<(u64, &'static str)>) = (None, None);
        let bony = stuff == Body::Bone;
        let wet = if bony { "gore_bones" } else { "gore_splat" };
        match blow.map(|b| b.blow) {
            Some(Blow::Team { dir, halve }) => {
                gore.cut = if halve { Cut::Halve } else { Cut::Behead };
                (gore.dx, gore.dy) = dir;
                let (cut, after) = cut_sounds(gore.cut, bony, gore.span);
                now = Some(cut);
                later = (!bony).then_some(after);
            }
            Some(Blow::Shot { shot, dir, burst }) => {
                // Away from the knight when one is known and near; else the
                // way the shot flew.
                let d = if hero.is_some() { away((hx, hy)) } else { unit(dir.0, dir.1) };
                let d = if d == (0.0, 0.0) { (0.0, 1.0) } else { d };
                (gore.dx, gore.dy) = d;
                let range = (s.x - hx).hypot(s.y - hy);
                let pins = matches!(shot, Shot::Arrow | Shot::Bolt | Shot::Blade);
                if stuff == Body::Ichor && s.kind == EnemyKind::Slime {
                    gore.cut = Cut::Splat;
                    now = Some("gore_splat");
                } else if burst || matches!(shot, Shot::Missile) || matches!(stuff, Body::Shard | Body::Wood) {
                    gore.cut = Cut::Burst;
                    gore.big = shot == Shot::Ball || burst;
                    now = Some("gore_burst");
                } else if pins && hero.is_some() && range <= POINT_BLANK {
                    gore.cut = Cut::Kick;
                    gore.shaft = shot;
                    gore.big = true;
                    let (stone, far) = along(d, KICK_REACH);
                    match stone {
                        Some(p) => {
                            gore.pin = Pin::Wall;
                            // The body stops a little short of the stone.
                            let back = (s.r * 0.55).min(far);
                            (gore.to_x, gore.to_y) = (p.0 - d.0 * back, p.1 - d.1 * back);
                        }
                        None => {
                            gore.pin = Pin::Floor;
                            (gore.to_x, gore.to_y) = (s.x + d.0 * far * 0.72, s.y + d.1 * far * 0.72);
                        }
                    }
                    let travel = (gore.to_x - s.x).hypot(gore.to_y - s.y);
                    gore.span = (7.0 + travel * 1.6).clamp(8.0, 16.0) as u8;
                    now = Some("gore_kick");
                    later = Some((HOLD + u64::from(gore.span), "gore_thunk"));
                } else if shot == Shot::Ball && range <= POINT_BLANK {
                    gore.cut = Cut::Burst;
                    gore.big = true;
                    now = Some("gore_burst");
                } else {
                    gore.cut = Cut::Punch;
                    gore.big = shot == Shot::Ball;
                    let (stone, far) = along(d, SKID * if gore.big { 1.4 } else { 1.0 });
                    let far = if stone.is_some() { (far - s.r * 0.5).max(0.0) } else { far };
                    (gore.to_x, gore.to_y) = (s.x + d.0 * far, s.y + d.1 * far);
                    gore.span = 9;
                    now = Some(wet);
                }
            }
            Some(Blow::Swing { dir, forged, fists, stroke, combo }) => {
                let d = if dir == (0.0, 0.0) { away((hx, hy)) } else { unit(dir.0, dir.1) };
                (gore.dx, gore.dy) = d;
                if stuff == Body::Ichor && s.kind == EnemyKind::Slime {
                    gore.cut = Cut::Splat;
                    now = Some("gore_splat");
                } else if matches!(stuff, Body::Shard | Body::Wood) {
                    gore.cut = Cut::Burst;
                    now = Some("gore_burst");
                } else if fists {
                    gore.cut = Cut::Punch;
                    let (stone, far) = along(d, SKID);
                    let far = if stone.is_some() { (far - s.r * 0.5).max(0.0) } else { far };
                    (gore.to_x, gore.to_y) = (s.x + d.0 * far, s.y + d.1 * far);
                    gore.span = 9;
                    now = Some(wet);
                } else if melee::before() {
                    let head = headed(s.kind) && (var + u8::from(forged)) % 2 == 0;
                    gore.cut = if head { Cut::Behead } else { Cut::Halve };
                    gore.big = true;
                    gore.span = 16;
                    if head {
                        // The head lands out along the swing, short of stone.
                        let reach = 1.7 + f32::from(var) * 0.7;
                        let (stone, far) = along(d, reach);
                        let far = if stone.is_some() { (far - 0.3).max(0.0) } else { far };
                        (gore.to_x, gore.to_y) = (s.x + d.0 * far, s.y + d.1 * far);
                    }
                    now = Some("gore_chop");
                    later = Some((HOLD + 10, wet));
                } else {
                    // The blow chooses: a level sweep takes the head, the
                    // backhand and a headless thing are cut through the
                    // middle, a heavy stroke from above splits it nearly
                    // down its length. The finisher throws the head
                    // furthest.
                    let heavy = matches!(stroke, melee::Stroke::Overhead | melee::Stroke::Slam);
                    let level = match stroke {
                        melee::Stroke::Slash => combo != 2,
                        melee::Stroke::Chain => var != 1,
                        melee::Stroke::Thrust => (var + u8::from(forged)) % 2 == 0,
                        _ => false,
                    };
                    let head = headed(s.kind) && !heavy && level;
                    gore.cut = if head { Cut::Behead } else { Cut::Halve };
                    gore.big = true;
                    gore.span = if head && combo == 3 { 18 } else if head { 16 } else { 20 };
                    if !head {
                        // The cut slopes down away from the knight, so the
                        // upper piece slides off away from the blade.
                        let fall = if d.0 >= 0.0 { 1 } else { -1 };
                        let steep = if heavy { 58 + 7 * var as i8 } else { 14 + 8 * var as i8 };
                        gore.slant = Some(fall * steep);
                    }
                    if head {
                        // The head lands out along the swing, short of stone.
                        let reach = 1.7 + f32::from(var) * 0.7 + if combo == 3 { 1.2 } else { 0.0 };
                        let (stone, far) = along(d, reach);
                        let far = if stone.is_some() { (far - 0.3).max(0.0) } else { far };
                        (gore.to_x, gore.to_y) = (s.x + d.0 * far, s.y + d.1 * far);
                    }
                    let (cut, after) = cut_sounds(gore.cut, bony, gore.span);
                    now = Some(cut);
                    later = Some(after);
                }
            }
            Some(Blow::Blast { at }) => {
                let d = away(at);
                (gore.dx, gore.dy) = d;
                gore.cut = if s.kind == EnemyKind::Slime { Cut::Splat } else { Cut::Burst };
                gore.big = true;
                now = Some(if gore.cut == Cut::Splat { "gore_splat" } else { "gore_burst" });
            }
            None => {
                // No blow to tell (a burn, a skill, a trap): the ordinary
                // death, with a spray and a stain.
                let d = away((hx, hy));
                (gore.dx, gore.dy) = if hero.is_some() { d } else { (0.0, 1.0) };
            }
        }
        (gore, now, later)
    }

    /// The tick's end: records age out, and an echo whose moment has come
    /// sounds.
    pub(super) fn age_gore(&mut self) {
        let tick = self.tick;
        self.gore.retain(|g| !g.over(tick));
        if !self.feel.echoes.is_empty() {
            let mut due = Vec::new();
            self.feel.echoes.retain(|&(at, name)| {
                let ready = at <= tick;
                if ready {
                    due.push(name);
                }
                !ready
            });
            self.sounds.extend(due);
        }
    }

    /// How hard the picture kicks the camera this tick, in the screen
    /// shake's own steps: a best finisher's first few ticks. The rules'
    /// `shake` is untouched.
    pub fn gore_kick(&self) -> u32 {
        self.gore
            .iter()
            .filter(|g| g.big)
            .map(|g| {
                let age = g.age(self.tick);
                if age < HOLD { 0 } else { 6u32.saturating_sub((age - HOLD) as u32) }
            })
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_gore__tests.rs"]
mod tests;
