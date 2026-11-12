//! Area spells: the waves, bursts, cones, lines, fields, pillars and novas a
//! wish can cast (`cast wave frost`, `cast burst fire 2`).
//!
//! A cast makes one `Zone` on the run. Each tick a zone makes the strokes
//! its shape says (a front a little wider, an eruption, a pool's pulse, a
//! beat of a cone) and every stroke lands as the area rule says (`aoe`):
//! the foes it meets first, nearest first, take it in full, the rest a
//! quarter; what the element does besides (burn, chill and freeze, arcs on,
//! poison, mend, pull, shove) still reaches all of them. Nothing here rolls:
//! a zone is its cast tick, its place and its aim, so the same run draws and
//! plays the same on the host, a mirror and a guest.
//!
//! Frost freezes from size 2 up (a boss only slows), and a frozen foe that
//! dies shatters: its splinters strike what stands near it, and they may
//! shatter the next.

use super::cards::{Area, Element, Form};
use super::vfx::Voice;
use super::*;
use serde::{Deserialize, Serialize};

/// Zones a room holds, and a knight's own at once (the oldest gives way).
pub const MAX_ZONES: usize = 12;
pub const MAX_PER_KNIGHT: usize = 3;
/// Foes and friends one zone remembers having struck.
const MAX_HIT: usize = 128;
/// The most arcs, shatters and frozen foes kept for the picture and rules.
const MAX_ARCS: usize = 48;
const MAX_SHARDS: usize = 24;
const MAX_RIME: usize = 128;
/// How long a lightning arc, and a shatter's burst, are drawn (ticks).
pub const ARC_TICKS: u8 = 8;
pub const SHARD_TICKS: u8 = 20;

/// A wave's and a nova's run out to full reach, and the wave's half-angle.
const WAVE_TICKS: u32 = 20;
const WAVE_HALF: f32 = 0.95;
const NOVA_TICKS: u32 = 16;
/// A burst's and a pillar's wait before they land (the telegraph).
pub const BURST_WAIT: u32 = 8;
pub const PILLAR_WAIT: u32 = 14;
/// How long a pillar's afterglow is drawn past its landing.
const PILLAR_AFTER: u32 = 16;
/// A cone's beat and life, its half-angle.
const CONE_BEAT: u32 = 4;
const CONE_LIFE: u32 = 38;
const CONE_HALF: f32 = 0.72;
/// A line's segments: ticks between two, the first's wait, how far apart
/// they stand, how wide each erupts.
pub const LINE_STEP: u32 = 2;
pub const LINE_WAIT: u32 = 2;
pub const LINE_GAP: f32 = 1.5;
const LINE_WIDE: f32 = 1.7;
/// A pool's or field's beat, and how long the lingering lasts, by element.
const POOL_BEAT: u32 = 10;
/// How far a burst, field or pillar lands from the knight, at most.
const AIM_REACH: f32 = 6.5;
/// A pillar finds a foe this near its mark.
const PILLAR_SNAP: f32 = 3.5;

/// How hard a stroke of each form lands (before size and the knight's
/// damage), per stroke: one front, one eruption, one beat.
const BASE: [(Form, u32); 7] = [
    (Form::Wave, 40),
    (Form::Burst, 52),
    (Form::Cone, 15),
    (Form::Line, 44),
    (Form::Field, 18),
    (Form::Pillar, 80),
    (Form::Nova, 36),
];

/// Frost: ticks a foe stands frozen at each size (size 1 only chills), and
/// how long a chill lasts, a boss's slow.
const FREEZE: [u32; 3] = [0, 30, 54];
const CHILL_BOSS: u32 = 18;
/// Earth's stun at each size, and the shove.
const STUN: [u32; 3] = [18, 28, 40];
const SHOVE: [f32; 3] = [1.6, 2.2, 2.8];
/// Arcane's pull in one stroke, and in a pool's beat.
const PULL: f32 = 2.4;
const PULL_BEAT: f32 = 0.7;
/// Storm: an arc reaches this far, and jumps to a size more foes.
const ARC_REACH: f32 = 4.0;
/// Holy: what a stroke mends each knight it covers, plus a step a size.
const MEND: u32 = 4;
/// A shatter's splinters: how far they fly, and the share of the stroke.
const SPLINTER_REACH: f32 = 2.8;
const SPLINTER_SHARE: u32 = 45;
/// A pool beats at a quarter of its eruption's stroke.
const POOL_SHARE: u32 = 30;

/// One area spell in play.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub owner: u32,
    pub area: Area,
    /// The spell's card, for the look a wish painted it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub spell: String,
    /// Where it stands: the knight's feet for a wave, cone, line and nova
    /// (a cone follows them), the mark for a burst, field and pillar.
    pub x: f32,
    pub y: f32,
    /// The aim it was cast along, a unit vector.
    pub dx: f32,
    pub dy: f32,
    pub age: u32,
    pub life: u32,
    /// One stroke of it, already the knight's.
    pub damage: u32,
    /// Foes it has struck, with the zone's age at the blow: a front meets
    /// each once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hit: Vec<(u32, u32)>,
    /// Knights a holy front has already mended.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mended: Vec<u32>,
}

/// A foe frozen by frost, who shatters if it dies before it thaws.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rime {
    pub foe: u32,
    pub owner: u32,
    pub left: u32,
    /// What its splinters strike for.
    pub splinter: u32,
}

/// A lightning arc between two foes, drawn for `ARC_TICKS`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arc {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub age: u8,
    pub seed: u16,
    /// A bolt from the sky onto `to` rather than an arc from `from`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sky: bool,
}

/// A frozen foe's shatter, drawn for `SHARD_TICKS`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shard {
    pub x: f32,
    pub y: f32,
    pub age: u8,
    pub seed: u16,
}

/// Every area spell in the room. Left out of a save when empty.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<Zone>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rime: Vec<Rime>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arcs: Vec<Arc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shards: Vec<Shard>,
}

impl Field {
    pub fn is_empty(&self) -> bool {
        self.zones.is_empty() && self.rime.is_empty() && self.arcs.is_empty() && self.shards.is_empty()
    }

    /// Within bounds, every number a finite one a zone's maths can take.
    pub fn valid(&self) -> bool {
        self.zones.len() <= MAX_ZONES
            && self.rime.len() <= MAX_RIME
            && self.arcs.len() <= MAX_ARCS
            && self.shards.len() <= MAX_SHARDS
            && self.zones.iter().all(Zone::valid)
            && self.arcs.iter().all(|a| {
                [a.from.0, a.from.1, a.to.0, a.to.1].iter().all(|v| v.is_finite()) && a.age <= ARC_TICKS
            })
            && self.shards.iter().all(|s| s.x.is_finite() && s.y.is_finite() && s.age <= SHARD_TICKS)
    }

    pub fn clear(&mut self) {
        *self = Field::default();
    }
}

impl Zone {
    pub fn valid(&self) -> bool {
        (Area::SIZES.0..=Area::SIZES.1).contains(&self.area.size)
            && [self.x, self.y, self.dx, self.dy].iter().all(|v| v.is_finite())
            && self.life <= 600
            && self.age <= self.life
            && self.hit.len() <= MAX_HIT
            && self.mended.len() <= 8
            && self.spell.len() <= 96
    }

    /// Whether the zone is still in play.
    pub fn live(&self) -> bool {
        self.age < self.life
    }

    pub fn size(&self) -> usize {
        usize::from(self.area.size.clamp(Area::SIZES.0, Area::SIZES.1)) - 1
    }

    /// How far it reaches at full: a front's run, a burst's eruption, a
    /// field's width, a cone's length, a line's length.
    pub fn reach(&self) -> f32 {
        reach(self.area)
    }
}

/// How far an area reaches at full, by form and size.
pub fn reach(area: Area) -> f32 {
    let s = usize::from(area.size.clamp(Area::SIZES.0, Area::SIZES.1)) - 1;
    let pick = |table: [f32; 3]| table[s];
    match area.form {
        Form::Wave => pick([10.0, 12.0, 14.0]),
        Form::Nova => pick([7.5, 9.0, 10.5]),
        Form::Burst => pick([3.4, 4.0, 4.6]),
        Form::Cone => pick([7.0, 8.5, 10.0]),
        Form::Line => pick([10.5, 13.0, 15.5]),
        Form::Field => pick([4.0, 4.8, 5.6]),
        Form::Pillar => pick([2.4, 2.9, 3.4]),
    }
}

/// How long the pool or field lingers, by element.
fn linger(element: Element) -> u32 {
    match element {
        Element::Fire | Element::Poison => 5 * HZ,
        Element::Frost => 4 * HZ,
        Element::Storm | Element::Holy | Element::Arcane => 3 * HZ,
        Element::Earth => 2 * HZ,
    }
}

/// How long a zone is in play.
pub fn life(area: Area) -> u32 {
    match area.form {
        Form::Wave => WAVE_TICKS + 12,
        Form::Nova => NOVA_TICKS + 14,
        Form::Burst => BURST_WAIT + linger(area.element),
        Form::Cone => CONE_LIFE,
        Form::Line => LINE_WAIT + LINE_STEP * segments(area) + 18,
        Form::Field => 4 + linger(area.element) + POOL_BEAT,
        Form::Pillar => PILLAR_WAIT + PILLAR_AFTER,
    }
}

/// How many segments a line erupts in.
pub fn segments(area: Area) -> u32 {
    (reach(area) / LINE_GAP).ceil() as u32
}

/// One stroke of `area` at its size, before the knight's damage.
fn base(area: Area) -> u32 {
    let form = BASE.iter().find(|(f, _)| *f == area.form).map_or(30, |&(_, d)| d);
    form * (100 + 25 * u32::from(area.size.clamp(Area::SIZES.0, Area::SIZES.1) - 1)) / 100
}

/// What a rival in the colosseum takes of an area cast: where it lands, how
/// far it reaches and what it deals (one stroke; there is no ground).
pub(super) fn duel_stroke(area: &Area, at: (f32, f32), aim: (f32, f32), damage: u32) -> ((f32, f32), f32, u32) {
    let centre = match area.form {
        Form::Wave | Form::Cone | Form::Line => ((at.0 + aim.0) / 2.0, (at.1 + aim.1) / 2.0),
        Form::Nova => at,
        Form::Burst | Form::Field | Form::Pillar => aim,
    };
    (centre, reach(*area).min(6.0), damage * base(*area) / 40)
}

/// Whether a spell card casts an area, and which.
pub fn area_of(card: &Card) -> Option<Area> {
    card.effects.iter().find_map(|e| match e {
        cards::Effect::Area(area) => Some(*area),
        _ => None,
    })
}

fn aim_of(hero: &Hero) -> (f32, f32) {
    let (ax, ay) = (hero.aim_x, hero.aim_y);
    if ax == 0.0 && ay == 0.0 || !ax.is_finite() || !ay.is_finite() {
        (0.0, -1.0)
    } else {
        unit(ax, ay)
    }
}

/// A knight casts `area` from spell `spell`: its zone, standing where its
/// form says (the mark is the aim, short of stone). A pillar snaps to a foe
/// near its mark.
pub(super) fn cast(
    field: &mut Field,
    hero: &Hero,
    spell: &str,
    area: Area,
    owner: u32,
    grid: &Grid,
    enemies: &[Enemy],
) {
    let (ux, uy) = aim_of(hero);
    let mut at = (hero.x, hero.y);
    if matches!(area.form, Form::Burst | Form::Field | Form::Pillar) {
        let (mut x, mut y) = at;
        let mut flown = 0.0;
        while flown < AIM_REACH {
            let (nx, ny) = (x + ux * 0.4, y + uy * 0.4);
            if grid.stops_shot(nx, ny) {
                break;
            }
            (x, y) = (nx, ny);
            flown += 0.4;
        }
        at = (x, y);
        if area.form == Form::Pillar
            && let Some(near) = enemies
                .iter()
                .filter(|e| e.hp > 0 && (e.x - x).hypot(e.y - y) <= PILLAR_SNAP)
                .min_by(|a, b| {
                    let (da, db) = ((a.x - x).hypot(a.y - y), (b.x - x).hypot(b.y - y));
                    da.total_cmp(&db).then(a.id.cmp(&b.id))
                })
        {
            at = (near.x, near.y);
        }
    }
    // A knight keeps a few at once: the oldest of theirs gives way.
    let mine = field.zones.iter().filter(|z| z.owner == owner).count();
    if mine >= MAX_PER_KNIGHT
        && let Some(old) = field.zones.iter().position(|z| z.owner == owner)
    {
        field.zones.remove(old);
    }
    if field.zones.len() >= MAX_ZONES {
        field.zones.remove(0);
    }
    field.zones.push(Zone {
        owner,
        area,
        spell: spell.chars().take(96).collect(),
        x: at.0,
        y: at.1,
        dx: ux,
        dy: uy,
        age: 0,
        life: life(area),
        damage: hero.spell_damage(spell, base(area)),
        hit: Vec::new(),
        mended: Vec::new(),
    });
}

/// One stroke a zone makes this tick: where it is centred, the ground it
/// covers, how hard it lands, and how the element moves what it meets.
struct Stroke {
    at: (f32, f32),
    cover: Cover,
    damage: u32,
    /// A front or an eruption of segments: each foe is struck once, ranked
    /// by the order it was met; otherwise the nearest are the first.
    once: bool,
    /// A pool's beat: the element's lasting touch only.
    pool: bool,
    /// Where an arcane stroke drags foes to, and what an earth stroke
    /// shoves them from.
    knot: (f32, f32),
    /// A lightning bolt from the sky: it strikes, and arcs on.
    bolt: bool,
}

#[derive(Clone, Copy)]
enum Cover {
    Disc(f32),
    Sector { dir: (f32, f32), r: f32, half: f32 },
}

impl Cover {
    fn has(self, at: (f32, f32), (x, y): (f32, f32), slack: f32) -> bool {
        let (dx, dy) = (x - at.0, y - at.1);
        let d = dx.hypot(dy);
        match self {
            Cover::Disc(r) => d <= r + slack,
            Cover::Sector { dir, r, half } => {
                if d > r + slack {
                    return false;
                }
                if d < 0.3 + slack {
                    return true;
                }
                // The sector widens a little by the body it is meeting.
                let widen = (slack / d).clamp(0.0, 1.0).asin();
                let cos = ((dx * dir.0 + dy * dir.1) / d).clamp(-1.0, 1.0);
                cos.acos() <= half + widen
            }
        }
    }

}

/// How far a front has run at `age` of its `ticks`: fast off the hand,
/// slowing as it spends itself.
pub fn front(age: u32, ticks: u32, reach: f32) -> f32 {
    let k = (age as f32 / ticks as f32).min(1.0);
    reach * (1.0 - (1.0 - k) * (1.0 - k))
}

/// The segment point `i` of a line stands at.
pub fn segment_at(zone: &Zone, i: u32) -> (f32, f32) {
    let along = LINE_GAP * (i as f32 + 0.6);
    (zone.x + zone.dx * along, zone.y + zone.dy * along)
}

impl Run {
    /// Every zone makes its strokes, and the frozen who died shatter.
    pub(super) fn tick_zones(&mut self) {
        for arc in &mut self.zones.arcs {
            arc.age += 1;
        }
        self.zones.arcs.retain(|a| a.age <= ARC_TICKS);
        for shard in &mut self.zones.shards {
            shard.age += 1;
        }
        self.zones.shards.retain(|s| s.age <= SHARD_TICKS);
        if self.zones.zones.is_empty() {
            return;
        }
        let mut live = std::mem::take(&mut self.zones.zones);
        for zone in &mut live {
            self.zone_step(zone);
        }
        live.retain(Zone::live);
        self.zones.zones = live;
    }

    /// A knight's zone follows them, a cone most of all.
    fn zone_step(&mut self, z: &mut Zone) {
        z.age += 1;
        let owner = self.players.get(&z.owner).filter(|h| h.hp > 0).map(|h| (h.x, h.y, aim_of(h)));
        if z.area.form == Form::Cone {
            let Some((x, y, (ux, uy))) = owner else {
                z.age = z.life;
                return;
            };
            (z.x, z.y, z.dx, z.dy) = (x, y, ux, uy);
        }
        let strokes = self.zone_strokes(z);
        let first = z.age == 1;
        if first {
            self.zone_cast_noise(z);
        }
        for stroke in strokes {
            self.zone_stroke(z, stroke);
        }
    }

    /// What the form does at this age.
    fn zone_strokes(&self, z: &Zone) -> Vec<Stroke> {
        let (age, d) = (z.age, (z.dx, z.dy));
        let at = (z.x, z.y);
        let area = z.area;
        let reach = z.reach();
        let beat = |every: u32, from: u32| age >= from && (age - from).is_multiple_of(every);
        let pool_damage = (z.damage * POOL_SHARE / 100).max(1);
        match area.form {
            Form::Wave if age <= WAVE_TICKS => vec![Stroke {
                at,
                cover: Cover::Sector { dir: d, r: front(age, WAVE_TICKS, reach), half: WAVE_HALF },
                damage: z.damage,
                once: true,
                pool: false,
                knot: (at.0 + d.0 * 3.2, at.1 + d.1 * 3.2),
                bolt: false,
            }],
            Form::Nova if age <= NOVA_TICKS => vec![Stroke {
                at,
                cover: Cover::Disc(front(age, NOVA_TICKS, reach)),
                damage: z.damage,
                once: true,
                pool: false,
                knot: (at.0 + d.0 * 3.2, at.1 + d.1 * 3.2),
                bolt: false,
            }],
            Form::Burst if age == BURST_WAIT => vec![Stroke {
                at,
                cover: Cover::Disc(reach),
                damage: z.damage,
                once: false,
                pool: false,
                knot: at,
                bolt: false,
            }],
            Form::Burst if age > BURST_WAIT && beat(POOL_BEAT, BURST_WAIT + POOL_BEAT) && age < z.life => {
                vec![Stroke { at, cover: Cover::Disc(reach * 0.9), damage: pool_damage, once: false, pool: true, knot: at, bolt: false }]
            }
            Form::Cone if beat(CONE_BEAT, 2) => vec![Stroke {
                at,
                cover: Cover::Sector { dir: d, r: reach, half: CONE_HALF },
                damage: z.damage,
                once: false,
                pool: true,
                knot: (at.0 + d.0 * reach * 0.5, at.1 + d.1 * reach * 0.5),
                bolt: false,
            }],
            Form::Line => {
                let n = segments(area);
                let first = LINE_WAIT;
                if age >= first && (age - first).is_multiple_of(LINE_STEP) && (age - first) / LINE_STEP < n {
                    let i = (age - first) / LINE_STEP;
                    let p = segment_at(z, i);
                    vec![Stroke {
                        at: p,
                        cover: Cover::Disc(LINE_WIDE),
                        damage: z.damage,
                        once: true,
                        pool: false,
                        knot: (z.x + d.0 * reach * 0.5, z.y + d.1 * reach * 0.5),
                        bolt: false,
                    }]
                } else {
                    Vec::new()
                }
            }
            Form::Field if beat(POOL_BEAT, 4) && age < z.life => {
                if area.element == Element::Storm {
                    self.storm_bolts(z)
                } else {
                    vec![Stroke { at, cover: Cover::Disc(reach), damage: z.damage, once: false, pool: true, knot: at, bolt: false }]
                }
            }
            Form::Pillar if age == PILLAR_WAIT => vec![Stroke {
                at,
                cover: Cover::Disc(reach),
                damage: z.damage,
                once: false,
                pool: false,
                knot: at,
                bolt: area.element == Element::Storm,
            }],
            _ => Vec::new(),
        }
    }

    /// A storm field's beat: a bolt from the sky onto each of the next foes
    /// in it, in the order of their ids, a size more of them at a time.
    fn storm_bolts(&self, z: &Zone) -> Vec<Stroke> {
        let reach = z.reach();
        let mut inside: Vec<&Enemy> = self
            .enemies
            .iter()
            .filter(|e| e.hp > 0 && !e.buried() && Cover::Disc(reach).has((z.x, z.y), (e.x, e.y), e.radius() * 0.5))
            .collect();
        inside.sort_by_key(|e| e.id);
        let n = inside.len();
        if n == 0 {
            return Vec::new();
        }
        let beat_no = ((z.age - 4) / POOL_BEAT) as usize;
        let bolts = (z.size() + 2).min(n);
        (0..bolts)
            .map(|k| {
                let e = inside[(beat_no * bolts + k) % n];
                Stroke {
                    at: (e.x, e.y),
                    cover: Cover::Disc(1.3),
                    damage: z.damage,
                    once: false,
                    pool: false,
                    knot: (z.x, z.y),
                    bolt: true,
                }
            })
            .collect()
    }

    /// The shake of a cast, once.
    fn zone_cast_noise(&mut self, z: &Zone) {
        // The cast's own sound is the card's (`arms`).
        if matches!(z.area.form, Form::Wave | Form::Nova) {
            self.fx.push(Fx::Shake(3));
        }
    }

    /// One stroke lands.
    fn zone_stroke(&mut self, z: &mut Zone, stroke: Stroke) {
        let owner = z.owner;
        let element = z.area.element;
        let size = z.size();
        let mut caught: Vec<(f32, usize)> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| e.hp > 0 && !e.buried())
            .filter(|(_, e)| stroke.cover.has(stroke.at, (e.x, e.y), e.radius() * 0.5))
            .filter(|(_, e)| !stroke.once || !z.hit.iter().any(|&(id, _)| id == e.id))
            .map(|(i, e)| ((e.x - stroke.at.0).hypot(e.y - stroke.at.1), i))
            .collect();
        caught.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let struck = caught.len();
        let mut boss_hit = false;
        let mut killed = 0;
        let mut arcs_from: Vec<usize> = Vec::new();
        let met = z.hit.len();
        for (rank, &(_, i)) in caught.iter().enumerate() {
            let rank = if stroke.once { met + rank } else { rank };
            let dealt = aoe::part(stroke.damage, rank);
            let (id, boss) = (self.enemies[i].id, self.enemies[i].is_boss());
            let dealt = self.zone_hurt(owner, i, dealt, stroke.at, element);
            killed += u32::from(self.enemies[i].hp == 0 && dealt > 0);
            boss_hit |= boss && dealt > 0;
            self.zone_touch(z, &stroke, i, size);
            if stroke.once && z.hit.len() < MAX_HIT {
                z.hit.push((id, z.age));
            }
            if !stroke.pool && rank < aoe::FULL {
                arcs_from.push(i);
            }
        }
        if element == Element::Storm && !stroke.pool {
            self.zone_arcs(z, &stroke, &arcs_from);
        }
        if element == Element::Holy {
            self.zone_mend(z, &stroke);
        }
        // The shake and the hold a heavy stroke earns.
        if !stroke.pool && !stroke.once && struck > 0 {
            self.fx.push(Fx::Shake(if matches!(z.area.form, Form::Pillar) { 7 } else { 5 }));
            if boss_hit || struck >= 5 {
                self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
            }
        }
        if struck > 0 && (killed > 0 || !stroke.pool) {
            self.sound_every(impact_of(element), 5);
        }
    }

    /// Damage to foe `i`, credited to `owner`; what was dealt. A kill is a
    /// blast to the body (`gore`).
    fn zone_hurt(&mut self, owner: u32, i: usize, dealt: u32, at: (f32, f32), element: Element) -> u32 {
        let enemy = &mut self.enemies[i];
        let dealt = dealt.min(enemy.hp);
        enemy.hp -= dealt;
        enemy.wits.foe = owner;
        let id = enemy.id;
        let dead = enemy.hp == 0;
        self.feel.credit(owner, id, dealt, self.tick);
        if dead && dealt > 0 {
            // A bolt kills clean; the rest burst the body outward from the stroke.
            let blow = if element == Element::Storm {
                let enemy = &self.enemies[i];
                gore::Blow::Shot { shot: Shot::Bolt, dir: unit(enemy.x - at.0, enemy.y - at.1), burst: false }
            } else {
                gore::Blow::Blast { at }
            };
            self.feel.blow(id, owner, blow);
        }
        dealt
    }

    /// What the element does to a foe the stroke met, whatever its rank.
    fn zone_touch(&mut self, z: &Zone, stroke: &Stroke, i: usize, size: usize) {
        let owner = z.owner;
        let tick = self.tick;
        let base_hero = self.players.get(&owner);
        let enemy = &mut self.enemies[i];
        if enemy.hp == 0 {
            return;
        }
        let boss = enemy.is_boss();
        let anchored = boss
            || matches!(enemy.kind, EnemyKind::Dragon | EnemyKind::Dummy | EnemyKind::Ward)
            || enemy.asleep(tick);
        let (foe, radius, flies) = (enemy.id, enemy.radius(), enemy.kind.flies());
        match z.area.element {
            Element::Fire => {
                let power = base_hero.map_or(skills::BURN_BASE, |h| h.scaled(skills::BURN_BASE)) * (2 + size as u32);
                skills::afflict(&mut self.works.banes, foe, skills::Bane::Burn, power, owner);
            }
            Element::Poison => {
                let power = base_hero.map_or(skills::POISON_BASE, |h| h.scaled(skills::POISON_BASE)) * (2 + size as u32);
                skills::afflict(&mut self.works.banes, foe, skills::Bane::Poison, power, owner);
            }
            Element::Frost => {
                if boss {
                    enemy.chilled = enemy.chilled.max(CHILL_BOSS);
                } else {
                    enemy.chilled = enemy.chilled.max(wishcraft::CHILL_FOE * (1 + size as u32));
                    let freeze = if stroke.pool { 0 } else { FREEZE[size] };
                    if freeze > 0 && !anchored {
                        enemy.frozen = enemy.frozen.max(freeze);
                        let splinter = (z.damage * SPLINTER_SHARE / 100).max(1);
                        let rime = &mut self.zones.rime;
                        if let Some(r) = rime.iter_mut().find(|r| r.foe == foe) {
                            r.left = r.left.max(freeze + 12);
                            r.splinter = r.splinter.max(splinter);
                        } else if rime.len() < MAX_RIME {
                            rime.push(Rime { foe, owner, left: freeze + 12, splinter });
                        }
                    }
                }
            }
            Element::Earth => {
                if boss {
                    enemy.chilled = enemy.chilled.max(CHILL_BOSS / 2);
                } else if !anchored {
                    enemy.frozen = enemy.frozen.max(if stroke.pool { STUN[0] / 3 } else { STUN[size] });
                }
            }
            Element::Storm | Element::Holy | Element::Arcane => {}
        }
        // The shove and the drag (anchored foes stand their ground).
        let to = match z.area.element {
            Element::Arcane if !anchored => {
                let step = if stroke.pool { PULL_BEAT } else { PULL };
                Some(toward(enemy.x, enemy.y, stroke.knot, step))
            }
            Element::Earth if !anchored && !stroke.pool => {
                let (ux, uy) = unit(enemy.x - stroke.knot.0, enemy.y - stroke.knot.1);
                let (ux, uy) = if ux == 0.0 && uy == 0.0 { (z.dx, z.dy) } else { (ux, uy) };
                Some((ux * SHOVE[size], uy * SHOVE[size]))
            }
            _ => None,
        };
        if let Some(by) = to {
            let mover = if flies { Mover::Flier } else { Mover::Walker };
            let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
            let enemy = &mut self.enemies[i];
            (enemy.x, enemy.y) = grid.slide((enemy.x, enemy.y), by, radius, mover);
        }
    }

    /// Storm: the struck foes arc on to the nearest foes about them, a size
    /// more each, at half a stroke; an arc is the picture's guide.
    fn zone_arcs(&mut self, z: &Zone, stroke: &Stroke, from: &[usize]) {
        let owner = z.owner;
        let jumps = z.size() + 1;
        let mut struck: Vec<usize> = from.to_vec();
        let mut rank = 0;
        // Each bolt arcs on to two more foes at the most: a storm over a
        // crowd is bolts, not a net.
        let mut left = 2;
        for &i in from.iter().take(3) {
            let (fx_, fy_) = (self.enemies[i].x, self.enemies[i].y);
            if stroke.bolt && self.zones.arcs.len() < MAX_ARCS {
                let seed = (z.age as u16).wrapping_mul(31).wrapping_add(self.enemies[i].id as u16);
                self.zones.arcs.push(Arc { from: (fx_, fy_ - 9.0), to: (fx_, fy_), age: 0, seed, sky: true });
            }
            let mut near: Vec<(f32, usize)> = self
                .enemies
                .iter()
                .enumerate()
                .filter(|(j, e)| e.hp > 0 && !struck.contains(j) && !e.buried())
                .map(|(j, e)| ((e.x - fx_).hypot(e.y - fy_), j))
                .filter(|&(d, _)| d <= ARC_REACH)
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            for &(_, j) in near.iter().take(jumps) {
                if left == 0 {
                    break;
                }
                left -= 1;
                struck.push(j);
                let dealt = aoe::part((stroke.damage / 2).max(1), rank);
                rank += 1;
                let at = (self.enemies[j].x, self.enemies[j].y);
                self.zone_hurt(owner, j, dealt, at, Element::Storm);
                if self.zones.arcs.len() < MAX_ARCS {
                    let seed = (z.age as u16).wrapping_mul(17).wrapping_add(self.enemies[j].id as u16);
                    self.zones.arcs.push(Arc { from: (fx_, fy_), to: at, age: 0, seed, sky: false });
                }
            }
        }
    }

    /// Holy: the knights a stroke covers are mended, a front once.
    fn zone_mend(&mut self, z: &mut Zone, stroke: &Stroke) {
        let amount = MEND + 3 * z.size() as u32;
        let ids: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .filter(|(id, h)| {
                stroke.cover.has(stroke.at, (h.x, h.y), HERO_RADIUS) && (!stroke.once || !z.mended.contains(id))
            })
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.mend(amount, stakes::Mend::Party);
            }
            if stroke.once && z.mended.len() < 8 {
                z.mended.push(id);
            }
        }
    }

    /// Frozen foes that died shatter: their splinters strike what stands
    /// near, and may shatter the next. Run before the dead are reaped.
    pub(super) fn shatter_frozen(&mut self) {
        if self.zones.rime.is_empty() {
            return;
        }
        let mut heavy = false;
        for _pass in 0..4 {
            let mut burst: Vec<(usize, Rime)> = Vec::new();
            for r in &self.zones.rime {
                if let Some(i) = self.enemies.iter().position(|e| e.id == r.foe)
                    && self.enemies[i].hp == 0
                {
                    burst.push((i, *r));
                }
            }
            if burst.is_empty() {
                break;
            }
            for (i, r) in burst {
                let (x, y) = (self.enemies[i].x, self.enemies[i].y);
                self.zones.rime.retain(|q| q.foe != r.foe);
                if self.zones.shards.len() < MAX_SHARDS {
                    let seed = (x * 9.0) as i32 as u16 ^ ((y * 7.0) as i32 as u16).rotate_left(5);
                    self.zones.shards.push(Shard { x, y, age: 0, seed });
                }
                let struck = aoe::strike(&mut self.enemies, (x, y), r.splinter, |e| {
                    e.hp > 0 && (e.x - x).hypot(e.y - y) <= SPLINTER_REACH + e.radius()
                });
                for (j, dealt) in struck {
                    let enemy = &mut self.enemies[j];
                    enemy.wits.foe = r.owner;
                    let id = enemy.id;
                    let dead = enemy.hp == 0;
                    enemy.chilled = enemy.chilled.max(wishcraft::CHILL_FOE);
                    self.feel.credit(r.owner, id, dealt, self.tick);
                    if dead && dealt > 0 {
                        self.feel.blow(id, r.owner, gore::Blow::Blast { at: (x, y) });
                    }
                }
                heavy = true;
                self.fx.push(Fx::Shake(6));
                self.sound_every(Voice::Frost.impact(), 4);
            }
        }
        if heavy {
            self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
        }
        // The thawed let go.
        let enemies = &self.enemies;
        for r in &mut self.zones.rime {
            r.left = r.left.saturating_sub(1);
        }
        self.zones.rime.retain(|r| r.left > 0 && enemies.iter().any(|e| e.id == r.foe && e.hp > 0));
    }
}

/// `(x, y)` stepped `step` toward `to`, as the shove that gets it there.
fn toward(x: f32, y: f32, to: (f32, f32), step: f32) -> (f32, f32) {
    let (dx, dy) = (to.0 - x, to.1 - y);
    let d = dx.hypot(dy);
    if d < 0.5 {
        (0.0, 0.0)
    } else {
        let k = step.min(d - 0.4).max(0.0) / d;
        (dx * k, dy * k)
    }
}

/// The family an element sounds in.
pub fn voice_of(element: Element) -> Voice {
    match element {
        Element::Fire => Voice::Fire,
        Element::Frost => Voice::Frost,
        Element::Storm => Voice::Storm,
        Element::Poison => Voice::Poison,
        Element::Holy => Voice::Holy,
        Element::Arcane => Voice::Arcane,
        Element::Earth => Voice::Earth,
    }
}

fn impact_of(element: Element) -> &'static str {
    voice_of(element).impact()
}

#[cfg(test)]
#[path = "../../tests/rules/zones__tests.rs"]
mod tests;
