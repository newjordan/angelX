//! The champions at the bar: five big knights who play like nobody else
//! in the company. Each has a great arm on F, three spells of their own on
//! keys 1 to 3 (cards in the spell slots, so the bar shows their waits), and
//! an ultimate on R.
//!
//! | Champion | F | 1 | 2 | 3 | R |
//! |---|---|---|---|---|---|
//! | Hammerer | Great Maul | Storm Hammer: thrown, stuns, bursts | Groundslam: a wind-up, then a shockwave | Brute Force: the maul twice as fast | Earthsplitter |
//! | Butcher | Cleaver | Meat Hook: drags a foe to his feet | Rot: a stink that eats the near | Chain Haul: hauls himself to the wall | Dismember |
//! | Alchemist | lit flasks, lobbed | Miasma Flask: foul air | Quickfire: a field of flame | Elixir: the near friends mend | Philosopher's Rain |
//! | Reaver | Twin Axes | Hurl Axe: out and home, cutting both ways | Leap: a bound that lands in a quake | Bloodlust: his cuts feed him | Red Harvest |
//! | Lancer | War Lance | Javelin: through every foe on its line | Vault: over the front rank | Brace: the lance set against a rush | Skewer Drive |
//!
//! Flame meeting foul air flashes (`skills::flash`): the Alchemist's fire
//! thrown into his own miasma is his best stroke. Every number is one fixed
//! number, picked by play.

use super::skills::{Blaze, BoomKind, Cloud};
use super::ults::Ult;
use super::*;

/// Which champion a knight is, by their kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Champion {
    Hammerer,
    Butcher,
    Alchemist,
    Reaver,
    Lancer,
}

impl Champion {
    pub const ALL: [Champion; 5] =
        [Champion::Hammerer, Champion::Butcher, Champion::Alchemist, Champion::Reaver, Champion::Lancer];

    pub fn kit(self) -> &'static str {
        match self {
            Champion::Hammerer => "hammerer",
            Champion::Butcher => "butcher",
            Champion::Alchemist => "alchemist",
            Champion::Reaver => "reaver",
            Champion::Lancer => "lancer",
        }
    }

    pub fn of_kit(kit: &str) -> Option<Champion> {
        Champion::ALL.into_iter().find(|c| c.kit() == kit)
    }

    /// Their three spells, for keys 1 to 3.
    pub fn spells(self) -> [&'static str; 3] {
        match self {
            Champion::Hammerer => ["storm-hammer", "groundslam", "brute-force"],
            Champion::Butcher => ["meat-hook", "rot", "chain-haul"],
            Champion::Alchemist => ["miasma-flask", "quickfire", "elixir"],
            Champion::Reaver => ["hurl-axe", "leap", "bloodlust"],
            Champion::Lancer => ["javelin", "vault", "brace"],
        }
    }
}

/// The champion a knight is, if any.
pub fn of(hero: &Hero) -> Option<Champion> {
    hero.knight.as_deref().and_then(Champion::of_kit)
}

/// Whether a spell card is one of the champions'.
pub fn is_spell(card: &str) -> bool {
    heroes::is_spell(card) || Champion::ALL.iter().any(|c| c.spells().contains(&card))
}

// ─── the Hammerer ────────────────────────────────────────────────────────────

/// Storm Hammer: how fast it flies, how long (ticks), what it deals to the
/// first foe, how long it stuns them, and how wide the burst round them.
pub const HAMMER_SPEED: f32 = 18.0;
pub const HAMMER_LIFE: u32 = 22;
pub const HAMMER_DAMAGE: u32 = 50;
const HAMMER_STUN: u32 = 2 * HZ;
pub const HAMMER_BURST: f32 = 2.8;
/// Groundslam: the wind-up (rooted, the maul overhead), the shockwave's
/// reach, its blow, its stun and how far it throws foes back.
pub const SLAM_WINDUP: u32 = 12;
pub const SLAM_REACH: f32 = 5.0;
pub const SLAM_DAMAGE: u32 = 45;
const SLAM_STUN: u32 = HZ;
const SLAM_THROW: f32 = 2.2;
/// Brute Force: how long the giant's blood is up.
pub const FURY_TICKS: u32 = 6 * HZ;
/// Earthsplitter: the crack's length, how long it takes to run out, when
/// the ground bursts, how long the rubble lies, how near the line a foe
/// must stand, and the blow and stun of the burst.
pub const CRACK_LEN: f32 = 18.0;
pub const CRACK_RUN: u32 = 20;
pub const CRACK_ERUPT: u32 = 28;
pub const CRACK_LIFE: u32 = 50;
const CRACK_WIDE: f32 = 1.9;
const CRACK_DAMAGE: u32 = 130;
const CRACK_STUN: u32 = 2 * HZ;

// ─── the Butcher ─────────────────────────────────────────────────────────────

/// Meat Hook: its speed out and home, how long it flies out, the blow and
/// stun when a foe lands at the Butcher's feet.
pub const HOOK_SPEED: f32 = 26.0;
const HOOK_OUT: u32 = 16;
pub const HOOK_DAMAGE: u32 = 55;
const HOOK_STUN: u32 = HZ;
/// Rot: how long, how near, how often and how hard it eats.
pub const ROT_TICKS: u32 = 5 * HZ;
pub const ROT_REACH: f32 = 3.4;
pub const ROT_EVERY: u32 = 10;
pub const ROT_DAMAGE: u32 = 9;
/// Chain Haul: how far the hook reaches, how fast he is hauled, and what
/// each foe he bowls over takes.
pub const HAUL_SPEED: f32 = 36.0;
const HAUL_DAMAGE: u32 = 25;
/// Dismember: how near a foe must stand, how long it is held, how often
/// and how hard it is carved; the Butcher mends half of every cut.
const GRIP_REACH: f32 = 5.0;
pub const GRIP_TICKS: u32 = 3 * HZ;
const GRIP_EVERY: u32 = 10;
const GRIP_DAMAGE: u32 = 16;

// ─── the Alchemist ───────────────────────────────────────────────────────────

/// A lit flask off the bandolier: its rearm, how far it is lobbed, how long
/// it flies, its burst, and the small fire it leaves.
pub const FLASK_REARM: u32 = 20;
const FLASK_REACH: f32 = 8.0;
pub const FLASK_FLIGHT: u32 = 14;
pub const FLASK_DAMAGE: u32 = 22;
pub const FLASK_BURST: f32 = 1.8;
pub const FLASK_FIRE: f32 = 1.4;
pub const FLASK_FIRE_LIFE: u32 = 2 * HZ;
/// Quickfire: the great flask's burst and its field of flame.
pub const QUICK_DAMAGE: u32 = 34;
pub(super) const QUICK_RADIUS: f32 = 3.0;
pub(super) const QUICK_LIFE: u32 = 5 * HZ;
/// Miasma: the foul air's poison.
pub const FOUL_POWER: u32 = 5;
/// Elixir: how near a friend must stand, and what it mends.
pub const ELIXIR_REACH: f32 = 6.0;
pub const ELIXIR_MEND: u32 = 35;
/// Philosopher's Rain: how many foes, how near, and the wait between each
/// foe's flasks.
const RAIN_FOES: usize = 8;
const RAIN_REACH: f32 = 16.0;
const RAIN_STAGGER: u32 = 3;

// ─── the Reaver ──────────────────────────────────────────────────────────────

/// Hurl Axe: how fast it spins out and home, how long it flies out, and
/// what it cuts on each pass (once a foe, out and again on the way home).
pub const AXE_SPEED: f32 = 20.0;
const AXE_OUT: u32 = 14;
pub const AXE_DAMAGE: u32 = 34;
const AXE_STUN: u32 = 8;
/// Leap: the farthest bound, how fast he flies, and the quake he lands in.
pub const LEAP_REACH: f32 = 9.0;
pub const LEAP_SPEED: f32 = 30.0;
pub const LEAP_QUAKE: f32 = 3.0;
pub const LEAP_DAMAGE: u32 = 40;
const LEAP_STUN: u32 = HZ;
/// Bloodlust: how long, the share of every cut he takes back as health
/// (percent), and how much faster he goes.
pub const LUST_TICKS: u32 = 6 * HZ;
pub const LUST_FEED: u32 = 30;
pub const LUST_PACE: f32 = 1.3;
/// Red Harvest: the sweep's reach, the share of health under which a foe
/// is cut down outright (percent), the blow everything else takes, and the
/// charge each foe cut down gives back.
pub const HARVEST_REACH: f32 = 4.5;
pub const HARVEST_CULL: u32 = 30;
pub const HARVEST_DAMAGE: u32 = 90;
const HARVEST_REFUND: u32 = ults::ULT_FULL / 5;

// ─── the Lancer ──────────────────────────────────────────────────────────────

/// Javelin: how fast, how long, and the blow every foe on its line takes.
pub const JAVELIN_SPEED: f32 = 28.0;
pub const JAVELIN_LIFE: u32 = 18;
pub const JAVELIN_DAMAGE: u32 = 42;
const JAVELIN_STUN: u32 = 10;
/// Vault: how far, how fast, and what each foe she vaults over takes.
pub const VAULT_REACH: f32 = 5.5;
pub const VAULT_SPEED: f32 = 26.0;
const VAULT_DAMAGE: u32 = 20;
const VAULT_STUN: u32 = 14;
/// Brace: how long the lance is set, the share of a blow she still takes
/// (percent), and the lance's reach and blow against whatever rushes it.
pub const BRACE_TICKS: u32 = 2 * HZ;
pub const BRACE_TAKES: u32 = 50;
pub const BRACE_REACH: f32 = 3.2;
const BRACE_DAMAGE: u32 = 30;
const BRACE_STUN: u32 = HZ;
const BRACE_THROW: f32 = 1.8;
/// Skewer Drive: how far the charge runs, how fast, the blow every foe it
/// carries takes when it stops, and how much more if it stops at a wall.
pub const DRIVE_REACH: f32 = 14.0;
pub const DRIVE_SPEED: f32 = 34.0;
pub const DRIVE_DAMAGE: u32 = 110;
const DRIVE_WALL: u32 = 50;
const DRIVE_STUN: u32 = 2 * HZ;

/// How long the figure holds a spell's strike once it has left the hand.
pub const CAST_POSE: u32 = 10;

/// A champion's own going, on the knight: a wind-up, the giant's blood, a
/// stink, a haul, and the spell last cast, for the figure. Left out when
/// still, so a knight of the company is written as it always was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stance {
    /// Groundslam's wind-up: rooted, the maul overhead.
    #[serde(default)]
    pub windup: u32,
    /// Brute Force.
    #[serde(default)]
    pub fury: u32,
    /// Rot.
    #[serde(default)]
    pub rot: u32,
    /// A Chain Haul under way, and the way it runs.
    #[serde(default)]
    pub haul: u32,
    #[serde(default)]
    pub haul_x: f32,
    #[serde(default)]
    pub haul_y: f32,
    /// Ticks left of a spell's strike on the figure, and which key cast it.
    #[serde(default)]
    pub cast: u32,
    #[serde(default)]
    pub spell: u8,
    /// Bloodlust.
    #[serde(default)]
    pub lust: u32,
    /// Brace: the lance set.
    #[serde(default)]
    pub brace: u32,
    /// A bound under way (Leap, Vault, Skewer Drive): ticks left, which,
    /// and the way it runs.
    #[serde(default)]
    pub dash: u32,
    #[serde(default)]
    pub dash_kind: Dash,
    #[serde(default)]
    pub dash_x: f32,
    #[serde(default)]
    pub dash_y: f32,
}

/// Which bound a champion is in the middle of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dash {
    #[default]
    Leap,
    Vault,
    Drive,
}

impl Dash {
    /// How fast the bound runs (arena units a second).
    pub fn speed(self) -> f32 {
        match self {
            Dash::Leap => LEAP_SPEED,
            Dash::Vault => VAULT_SPEED,
            Dash::Drive => DRIVE_SPEED,
        }
    }
}

impl Stance {
    pub fn idle(&self) -> bool {
        *self == Stance::default()
    }

    /// Too taken up to shoot: winding up a slam, hauled along a chain, in
    /// the middle of a bound, or with the lance set.
    pub fn busy(&self) -> bool {
        self.windup > 0 || self.haul > 0 || self.dash > 0 || self.brace > 0
    }
}

/// What a champion threw, in the air.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thrown {
    Hammer,
    Hook,
    /// Chain Haul's hook, fast in the wall: the chain the Butcher rides.
    Haul,
    Flask(Brew),
    /// The Reaver's axe: out, then home to his hand.
    Axe,
    /// The Lancer's javelin, through everything on its line.
    Javelin,
}

/// What is in a flask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Brew {
    Fire,
    Foul,
    Quickfire,
}

/// A thing a champion threw: where it is, how it flies, how long, and for
/// a hook the foe it bit and whether it is coming home. A flask is lobbed
/// from `from` to `to` over its `life`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Throw {
    pub owner: u32,
    pub kind: Thrown,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub age: u32,
    pub life: u32,
    #[serde(default)]
    pub caught: Option<u32>,
    #[serde(default)]
    pub back: bool,
}

/// A ring going out over the floor: a Groundslam's shockwave, a flask's
/// burst, an Elixir's mending.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub age: u32,
    pub life: u32,
    pub kind: RingKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RingKind {
    Quake,
    Burst,
    Mend,
    /// Red Harvest's sweep.
    Reap,
}

pub const RING_LIFE: u32 = 14;

/// Earthsplitter's crack: from where, along which way, how far, how old.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Crack {
    pub owner: u32,
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub len: f32,
    pub age: u32,
}

/// Dismember: the foe the Butcher holds, and how long is left.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grip {
    pub owner: u32,
    pub foe: u32,
    pub left: u32,
}

/// The champions' things in the room. Left out when there are none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub throws: Vec<Throw>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rings: Vec<Ring>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cracks: Vec<Crack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grips: Vec<Grip>,
    /// The foes a Skewer Drive carries before the lance (`left` unused).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skewered: Vec<Grip>,
}

impl Field {
    pub fn is_empty(&self) -> bool {
        self.throws.is_empty()
            && self.rings.is_empty()
            && self.cracks.is_empty()
            && self.grips.is_empty()
            && self.skewered.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Field::default();
    }

    fn ring(&mut self, (x, y): (f32, f32), r: f32, kind: RingKind) {
        if self.rings.len() < 24 {
            self.rings.push(Ring { x, y, r, age: 0, life: RING_LIFE, kind });
        }
    }
}

/// Where a lobbed flask is at `age` of `life`, and how high off the floor
/// (arena units).
pub fn lob(t: &Throw) -> ((f32, f32), f32) {
    let k = (t.age as f32 / t.life.max(1) as f32).clamp(0.0, 1.0);
    let at = (t.from.0 + (t.to.0 - t.from.0) * k, t.from.1 + (t.to.1 - t.from.1) * k);
    (at, 4.0 * 2.2 * k * (1.0 - k))
}

/// Whether a foe is held fast where it stands: a boss, a dragon, a dummy.
fn anchored(e: &Enemy) -> bool {
    e.boss.is_some() || matches!(e.kind, EnemyKind::Dragon | EnemyKind::Dummy | EnemyKind::Ward)
}

impl Run {
    /// A champion's spell, cast from its slot this tick (`arms`), or the
    /// Alchemist's F (`flask`).
    pub(super) fn champion_feat(&mut self, id: u32, feat: &str) {
        if let Some(spell) = feat.strip_prefix("weave:").and_then(|s| s.parse().ok()) {
            self.weave_cast(id, spell);
            return;
        }
        if heroes::is_spell(feat) { self.hero_feat(id, feat); return; }
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        let (hx, hy) = (hero.x, hero.y);
        let (ax, ay) = unit(hero.aim_x, hero.aim_y);
        let (ax, ay) = if ax == 0.0 && ay == 0.0 { (0.0, -1.0) } else { (ax, ay) };
        if let Some(slot) = of(hero).and_then(|c| c.spells().iter().position(|s| *s == feat)) {
            hero.champ.cast = CAST_POSE;
            hero.champ.spell = slot as u8 + 1;
        }
        let fly = |kind, speed: f32, life| Throw {
            owner: id,
            kind,
            x: hx + ax * 0.9,
            y: hy + ay * 0.9,
            vx: ax * speed,
            vy: ay * speed,
            from: (hx, hy),
            to: (hx, hy),
            age: 0,
            life,
            caught: None,
            back: false,
        };
        match feat {
            "storm-hammer" => {
                self.champions.throws.push(fly(Thrown::Hammer, HAMMER_SPEED, HAMMER_LIFE));
                self.sounds.push("swing");
            }
            "groundslam" => {
                hero.champ.windup = SLAM_WINDUP;
                hero.invulnerable = hero.invulnerable.max(SLAM_WINDUP / 2);
                self.sounds.push("wall_up");
            }
            "brute-force" => {
                hero.champ.fury = hero.spell_step(feat).fury;
                self.sounds.push("boss_rise");
            }
            "meat-hook" => {
                self.champions.throws.retain(|t| !(t.owner == id && t.kind == Thrown::Hook));
                self.champions.throws.push(fly(Thrown::Hook, HOOK_SPEED, HOOK_OUT));
                self.sounds.push("swing");
            }
            "rot" => {
                hero.champ.rot = hero.spell_step(feat).rot;
                self.sounds.push("gas_release");
            }
            "chain-haul" => {
                let reach = hero.spell_step(feat).haul_reach;
                let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
                // Along the aim to the first wall or foe, short of it.
                let mut flown = 0.0;
                while flown < reach {
                    let (nx, ny) = (hx + ax * (flown + 0.4), hy + ay * (flown + 0.4));
                    let foe = self.enemies.iter().any(|e| e.hp > 0 && (e.x - nx).hypot(e.y - ny) < e.radius() + 0.3);
                    if grid.stops_shot(nx, ny) || foe {
                        break;
                    }
                    flown += 0.4;
                }
                let run = (flown - 1.0).max(0.0);
                let ticks = (run / (HAUL_SPEED * DT)).round() as u32;
                if ticks > 0 {
                    let hero = self.players.get_mut(&id).expect("the hauler");
                    hero.champ.haul = ticks;
                    (hero.champ.haul_x, hero.champ.haul_y) = (ax, ay);
                    hero.invulnerable = hero.invulnerable.max(ticks + 2);
                    hero.dash_ticks = 0;
                    let mut chain = fly(Thrown::Haul, 0.0, ticks + 2);
                    chain.to = (hx + ax * (flown + 0.4), hy + ay * (flown + 0.4));
                    (chain.x, chain.y) = chain.to;
                    self.champions.throws.push(chain);
                }
                self.sounds.push("roll");
            }
            "flask" | "miasma-flask" | "quickfire" => {
                let brew = match feat {
                    "miasma-flask" => Brew::Foul,
                    "quickfire" => Brew::Quickfire,
                    _ => Brew::Fire,
                };
                let to = self.lob_target(id, FLASK_REACH);
                let mut flask = fly(Thrown::Flask(brew), 0.0, FLASK_FLIGHT);
                flask.to = to;
                self.champions.throws.push(flask);
                self.sounds.push("swing");
            }
            "hurl-axe" => {
                self.champions.throws.retain(|t| !(t.owner == id && t.kind == Thrown::Axe));
                self.champions.throws.push(fly(Thrown::Axe, AXE_SPEED, AXE_OUT));
                self.sounds.push("swing");
            }
            "leap" => {
                let reach = hero.spell_step(feat).haul_reach.max(LEAP_REACH);
                let ticks = self.bound(id, (hx, hy), (ax, ay), reach, Dash::Leap, false);
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.invulnerable = hero.invulnerable.max(ticks + 2);
                }
                self.sounds.push("roll");
            }
            "bloodlust" => {
                hero.champ.lust = hero.spell_step(feat).fury.max(LUST_TICKS);
                self.sounds.push("boss_rise");
            }
            "javelin" => {
                self.champions.throws.push(fly(Thrown::Javelin, JAVELIN_SPEED, JAVELIN_LIFE));
                self.sounds.push("shot_bolt");
            }
            "vault" => {
                let ticks = self.bound(id, (hx, hy), (ax, ay), VAULT_REACH, Dash::Vault, true);
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.invulnerable = hero.invulnerable.max(ticks + 2);
                }
                self.sounds.push("roll");
            }
            "brace" => {
                hero.champ.brace = BRACE_TICKS;
                self.sounds.push("shield_up");
            }
            "elixir" => {
                let reach = hero.spell_step(feat).elixir_reach;
                let mend = hero.spell_power(feat, ELIXIR_MEND);
                for knight in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                    if (knight.x - hx).hypot(knight.y - hy) <= reach {
                        knight.mend(mend, stakes::Mend::Party);
                    }
                }
                self.champions.ring((hx, hy), reach, RingKind::Mend);
                self.sounds.push("potion");
            }
            _ => {}
        }
    }

    /// Start a bound along `(ax, ay)`: as far as `reach`, short of the first
    /// wall (and, unless it goes `over` them, the first foe). The ticks it
    /// takes; nothing when there is no room to move.
    fn bound(&mut self, id: u32, (hx, hy): (f32, f32), (ax, ay): (f32, f32), reach: f32, kind: Dash, over: bool) -> u32 {
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let mut flown = 0.0;
        while flown < reach {
            let (nx, ny) = (hx + ax * (flown + 0.4), hy + ay * (flown + 0.4));
            let foe = !over && self.enemies.iter().any(|e| e.hp > 0 && (e.x - nx).hypot(e.y - ny) < e.radius() + 0.3);
            if grid.stops_shot(nx, ny) || foe {
                break;
            }
            flown += 0.4;
        }
        let run = (flown - 0.6).max(0.0);
        // At least two ticks, so the bound always has a last one to land on.
        let ticks = match (run / (kind.speed() * DT)).round() as u32 {
            0 => 0,
            n => n.max(2),
        };
        if ticks > 0
            && let Some(hero) = self.players.get_mut(&id)
        {
            hero.champ.dash = ticks;
            hero.champ.dash_kind = kind;
            (hero.champ.dash_x, hero.champ.dash_y) = (ax, ay);
            hero.dash_ticks = 0;
        }
        ticks
    }

    /// A champion's ultimate, cast (`cast_ults`). Any knight may carry one
    /// off the wheel of fortune, so none of them asks who casts it.
    pub(super) fn champion_ult(&mut self, id: u32, ult: Ult) {
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let (hx, hy) = (hero.x, hero.y);
        let (ax, ay) = unit(hero.aim_x, hero.aim_y);
        let (ax, ay) = if ax == 0.0 && ay == 0.0 { (0.0, -1.0) } else { (ax, ay) };
        match ult {
            Ult::Earthsplitter => {
                let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
                let mut len = 0.0;
                while len < CRACK_LEN && !grid.stops_shot(hx + ax * (len + 0.5), hy + ay * (len + 0.5)) {
                    len += 0.5;
                }
                self.champions.cracks.push(Crack { owner: id, x: hx + ax * 0.8, y: hy + ay * 0.8, dx: ax, dy: ay, len, age: 0 });
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.champ.cast = CAST_POSE;
                    hero.champ.spell = 4;
                }
                self.fx.push(Fx::Shake(8));
            }
            Ult::Dismember => {
                let near = self
                    .enemies
                    .iter()
                    .filter(|e| e.hp > 0 && e.kind != EnemyKind::Dummy)
                    .map(|e| ((e.x - hx).hypot(e.y - hy) - e.radius(), e.id))
                    .filter(|&(d, _)| d <= GRIP_REACH)
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                match near {
                    Some((_, foe)) => {
                        self.champions.grips.retain(|g| g.owner != id);
                        self.champions.grips.push(Grip { owner: id, foe, left: GRIP_TICKS });
                    }
                    // Nobody near to take hold of: the charge keeps.
                    None => {
                        if let Some(hero) = self.players.get_mut(&id) {
                            hero.ult_charge = ults::ULT_FULL;
                        }
                    }
                }
            }
            Ult::RedHarvest => {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.champ.cast = CAST_POSE;
                    hero.champ.spell = 4;
                }
                if let Some(hero) = self.players.get(&id) {
                    self.feel.melee.swing(id, hero, melee::Stroke::Slash, HARVEST_REACH, 360.0);
                }
                let damage = self.players.get(&id).map_or(HARVEST_DAMAGE, |h| h.scaled(HARVEST_DAMAGE));
                let mut cut = 0;
                for i in self.near((hx, hy), HARVEST_REACH) {
                    let e = &self.enemies[i];
                    // The wounded are reaped outright; a boss only bleeds.
                    let reaped = !e.is_boss() && e.kind != EnemyKind::Dummy && e.hp * 100 <= e.max_hp * HARVEST_CULL;
                    let blow = if reaped { e.hp } else { damage };
                    self.smite(id, i, blow, 6, false);
                    if reaped && self.enemies[i].hp == 0 {
                        cut += 1;
                    }
                }
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.ult_charge = (hero.ult_charge + cut * HARVEST_REFUND).min(ults::ULT_FULL);
                }
                self.champions.ring((hx, hy), HARVEST_REACH, RingKind::Reap);
                self.fx.push(Fx::Shake(10));
                self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                self.sounds.push("hit_blast");
            }
            Ult::SkewerDrive => {
                self.champions.skewered.retain(|g| g.owner != id);
                let ticks = self.bound(id, (hx, hy), (ax, ay), DRIVE_REACH, Dash::Drive, true);
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.champ.cast = CAST_POSE;
                    hero.champ.spell = 4;
                    hero.invulnerable = hero.invulnerable.max(ticks + 2);
                    if ticks == 0 {
                        // Hard against a wall already: the charge keeps.
                        hero.ult_charge = ults::ULT_FULL;
                    }
                }
                self.sounds.push("boss_rise");
            }
            Ult::PhilosophersRain => {
                let mut near: Vec<(f32, f32, f32)> = self
                    .enemies
                    .iter()
                    .filter(|e| e.hp > 0)
                    .map(|e| ((e.x - hx).hypot(e.y - hy), e.x, e.y))
                    .filter(|&(d, _, _)| d <= RAIN_REACH)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                if near.is_empty() {
                    near.push((0.0, hx + ax * FLASK_REACH, hy + ay * FLASK_REACH));
                }
                for (k, &(_, x, y)) in near.iter().take(RAIN_FOES).enumerate() {
                    let wait = k as u32 * RAIN_STAGGER;
                    // The foul air first, the fire after it: each one flashes.
                    for (brew, life) in [(Brew::Foul, FLASK_FLIGHT + wait), (Brew::Quickfire, FLASK_FLIGHT + 10 + wait)] {
                        self.champions.throws.push(Throw {
                            owner: id,
                            kind: Thrown::Flask(brew),
                            x: hx,
                            y: hy,
                            vx: 0.0,
                            vy: 0.0,
                            from: (hx, hy - 1.5),
                            to: (x, y),
                            age: 0,
                            life,
                            caught: None,
                            back: false,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// A blow from a champion's spell: dealt, the foe stunned (a boss a
    /// third as long), credited, and the charge filled when `charge`.
    fn smite(&mut self, owner: u32, i: usize, damage: u32, stun: u32, charge: bool) -> u32 {
        let enemy = &mut self.enemies[i];
        let dealt = damage.min(enemy.hp);
        enemy.hp -= dealt;
        enemy.wits.foe = owner;
        let boss = enemy.is_boss();
        enemy.frozen = enemy.frozen.max(if boss { stun / 3 } else { stun });
        let foe = enemy.id;
        self.feel.credit(owner, foe, dealt, self.tick);
        if charge && let Some(hero) = self.players.get_mut(&owner) {
            hero.ult_charge = (hero.ult_charge + dealt / 2).min(ults::ULT_FULL);
        }
        if boss && dealt > 0 {
            self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
        }
        if dealt > 0 {
            if let Some(hero) = self.players.get(&owner) {
                if matches!(of(hero), Some(Champion::Hammerer | Champion::Butcher | Champion::Reaver | Champion::Lancer)) && stun > 0 {
                    self.feel.melee.contact(owner, hero, enemy, dealt, false);
                }
            }
            self.sounds.push("hit");
        }
        // Bloodlust: the Reaver's cuts feed him.
        self.feed_bloodlust(&[(owner, dealt)]);
        dealt
    }

    /// Bloodlust: a share of what each knight in it dealt, back as health,
    /// within the second's lifesteal (`Hero::leech`). Fed the tick's credits
    /// (`aftermath`) and each of a champion's own blows (`smite`).
    pub(super) fn feed_bloodlust(&mut self, credits: &[(u32, u32)]) {
        for &(id, dealt) in credits {
            if let Some(hero) = self.players.get_mut(&id)
                && hero.champ.lust > 0
                && hero.hp > 0
                && dealt > 0
            {
                hero.leech((dealt * LUST_FEED / 100).max(1), stakes::Mend::Passive);
            }
        }
    }

    /// Every foe within `r` of `at`, nearest first.
    fn near(&self, (x, y): (f32, f32), r: f32) -> Vec<usize> {
        aoe::reached(&self.enemies, (x, y), |e| (e.x - x).hypot(e.y - y) <= r + e.radius() * 0.5)
    }

    /// One tick of the champions: their stances, what they threw, the
    /// rings, cracks and grips.
    pub(super) fn tick_champions(&mut self) {
        self.champion_stances();
        self.champion_throws();
        for ring in &mut self.champions.rings {
            ring.age += 1;
        }
        self.champions.rings.retain(|r| r.age < r.life);
        self.champion_cracks();
        self.champion_grips();
    }

    fn champion_stances(&mut self) {
        let tick = self.tick;
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let Some(hero) = self.players.get_mut(&id) else {
                continue;
            };
            if hero.champ.idle() {
                continue;
            }
            let s = &mut hero.champ;
            s.fury = s.fury.saturating_sub(1);
            s.rot = s.rot.saturating_sub(1);
            s.cast = s.cast.saturating_sub(1);
            s.lust = s.lust.saturating_sub(1);
            s.brace = s.brace.saturating_sub(1);
            if hero.hp == 0 {
                hero.champ = Stance::default();
                self.champions.skewered.retain(|g| g.owner != id);
                continue;
            }
            let (hx, hy) = (hero.x, hero.y);
            let (rot, slam) = (hero.champ.rot > 0, hero.champ.windup == 1);
            if hero.champ.windup > 0 {
                hero.champ.windup -= 1;
            }
            if hero.champ.windup == 0 && hero.champ.spell == 2 && slam {
                hero.champ.cast = CAST_POSE;
            }
            if rot && tick.is_multiple_of(u64::from(ROT_EVERY)) {
                let reach = hero.spell_step("rot").rot_reach;
                let damage = hero.spell_power("rot", ROT_DAMAGE);
                for i in self.near((hx, hy), reach) {
                    self.smite(id, i, damage, 0, true);
                }
            }
            if slam {
                self.groundslam(id, (hx, hy));
            }
            // A haul bowls over every foe it passes.
            let hauling = self.players.get(&id).is_some_and(|h| h.champ.haul > 0);
            if hauling {
                for i in self.near((hx, hy), 1.6) {
                    if self.enemies[i].wits.flinch == 0 {
                        let damage = self.players.get(&id).map_or(HAUL_DAMAGE, |h| h.spell_power("chain-haul", HAUL_DAMAGE));
                        self.smite(id, i, damage, 12, true);
                        self.enemies[i].wits.flinch = feel::FLINCH;
                    }
                }
            }
            if let Some(hero) = self.players.get_mut(&id)
                && hero.champ.dash == 0
            {
                // A bound done leaves nothing of itself on the stance.
                (hero.champ.dash_kind, hero.champ.dash_x, hero.champ.dash_y) = (Dash::default(), 0.0, 0.0);
            }
            self.champion_bound(id, (hx, hy));
            self.champion_brace(id, (hx, hy));
        }
    }

    /// A bound under way: a vault cuts what it passes over, a drive gathers
    /// foes before the lance, and on its last tick a leap lands in a quake
    /// and a drive pins what it carries.
    fn champion_bound(&mut self, id: u32, (hx, hy): (f32, f32)) {
        let Some((dash, kind, dx, dy)) = self
            .players
            .get(&id)
            .filter(|h| h.champ.dash > 0)
            .map(|h| (h.champ.dash, h.champ.dash_kind, h.champ.dash_x, h.champ.dash_y))
        else {
            return;
        };
        match kind {
            Dash::Leap => {}
            Dash::Vault => {
                for i in self.near((hx, hy), 1.4) {
                    if self.enemies[i].wits.flinch == 0 {
                        let damage = self.players.get(&id).map_or(VAULT_DAMAGE, |h| h.spell_damage("vault", VAULT_DAMAGE));
                        self.smite(id, i, damage, VAULT_STUN, true);
                        self.enemies[i].wits.flinch = feel::FLINCH;
                    }
                }
            }
            Dash::Drive => {
                let tip = (hx + dx * 1.6, hy + dy * 1.6);
                let mut stop = false;
                for i in self.near(tip, 1.3) {
                    let e = &self.enemies[i];
                    if e.kind == EnemyKind::Dummy || self.champions.skewered.iter().any(|g| g.foe == e.id) {
                        continue;
                    }
                    // Too big to carry: the charge stops on it.
                    stop |= anchored(e);
                    let foe = e.id;
                    self.champions.skewered.push(Grip { owner: id, foe, left: 0 });
                }
                let barred = self.barred();
                let grid = Grid { room: &self.dungeon.rooms[self.at], barred };
                for g in self.champions.skewered.iter().filter(|g| g.owner == id) {
                    let Some(e) = self.enemies.iter_mut().find(|e| e.id == g.foe && e.hp > 0) else { continue };
                    e.frozen = e.frozen.max(2);
                    if anchored(e) {
                        continue;
                    }
                    let mover = if e.kind.flies() { Mover::Flier } else { Mover::Walker };
                    let to = (hx + dx * (1.2 + e.radius()), hy + dy * (1.2 + e.radius()));
                    if grid.clear(to.0, to.1, e.radius(), mover) {
                        (e.x, e.y) = to;
                    }
                }
                if stop && let Some(hero) = self.players.get_mut(&id) {
                    hero.champ.dash = 1;
                }
            }
        }
        if self.players.get(&id).is_some_and(|h| h.champ.dash == 1) || dash == 1 {
            match kind {
                Dash::Leap => {
                    let damage = self.players.get(&id).map_or(LEAP_DAMAGE, |h| h.spell_damage("leap", LEAP_DAMAGE));
                    if let Some(hero) = self.players.get(&id) {
                        self.feel.melee.hit(0, (hx, hy), (hero.aim_x, hero.aim_y), melee::Tier::Heavy, true);
                    }
                    for i in self.near((hx, hy), LEAP_QUAKE) {
                        self.smite(id, i, damage, LEAP_STUN, true);
                    }
                    self.champions.ring((hx, hy), LEAP_QUAKE, RingKind::Quake);
                    self.boom(hx, hy, BoomKind::Dust, LEAP_QUAKE * 0.6);
                    self.fx.push(Fx::Shake(9));
                    self.sounds.push("hit_blast");
                }
                Dash::Vault => {}
                Dash::Drive => {
                    // Driven into a wall, the pin goes deeper.
                    let walled = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() }.stops_shot(hx + dx * 2.4, hy + dy * 2.4);
                    let base = self.players.get(&id).map_or(DRIVE_DAMAGE, |h| h.scaled(DRIVE_DAMAGE));
                    let damage = if walled { base * (100 + DRIVE_WALL) / 100 } else { base };
                    let carried: Vec<u32> = self.champions.skewered.iter().filter(|g| g.owner == id).map(|g| g.foe).collect();
                    self.champions.skewered.retain(|g| g.owner != id);
                    for foe in carried {
                        if let Some(i) = self.enemies.iter().position(|e| e.id == foe && e.hp > 0) {
                            self.smite(id, i, damage, DRIVE_STUN, false);
                            let at = (self.enemies[i].x, self.enemies[i].y);
                            self.champions.ring(at, 1.4, RingKind::Burst);
                        }
                    }
                    self.fx.push(Fx::Shake(if walled { 14 } else { 8 }));
                    self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
                    self.sounds.push(if walled { "bomb" } else { "hit_blast" });
                }
            }
        }
    }

    /// The lance set: whatever comes within its reach is struck and thrown
    /// back, once a flinch.
    fn champion_brace(&mut self, id: u32, (hx, hy): (f32, f32)) {
        if !self.players.get(&id).is_some_and(|h| h.champ.brace > 0) {
            return;
        }
        for i in self.near((hx, hy), BRACE_REACH) {
            if self.enemies[i].wits.flinch > 0 {
                continue;
            }
            let damage = self.players.get(&id).map_or(BRACE_DAMAGE, |h| h.spell_damage("brace", BRACE_DAMAGE));
            self.smite(id, i, damage, BRACE_STUN, true);
            self.enemies[i].wits.flinch = feel::FLINCH;
            let e = &self.enemies[i];
            if anchored(e) {
                continue;
            }
            let (ux, uy) = unit(e.x - hx, e.y - hy);
            let mover = if e.kind.flies() { Mover::Flier } else { Mover::Walker };
            let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
            let to = grid.slide((e.x, e.y), (ux * BRACE_THROW, uy * BRACE_THROW), e.radius(), mover);
            (self.enemies[i].x, self.enemies[i].y) = to;
        }
    }

    /// The maul comes down: every foe in reach is struck, stunned and thrown
    /// back, and every foe's shot in reach is knocked out of the air.
    fn groundslam(&mut self, id: u32, (hx, hy): (f32, f32)) {
        let damage = self.players.get(&id).map_or(SLAM_DAMAGE, |h| h.spell_damage("groundslam", SLAM_DAMAGE));
        let reach = self.players.get(&id).map_or(SLAM_REACH, |h| h.spell_step("groundslam").slam_reach);
        if let Some(hero) = self.players.get(&id) {
            self.feel.melee.swing(id, hero, melee::Stroke::Slam, reach, 360.0);
            self.feel.melee.hit(0, (hx, hy), (hero.aim_x, hero.aim_y), melee::Tier::Heavy, true);
        }
        for i in self.near((hx, hy), reach) {
            let hp = self.enemies[i].hp;
            self.smite(id, i, damage, SLAM_STUN, true);
            if self.enemies[i].hp < hp {
                self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
            }
            let e = &self.enemies[i];
            if anchored(e) {
                continue;
            }
            let (ux, uy) = unit(e.x - hx, e.y - hy);
            let mover = if e.kind.flies() { Mover::Flier } else { Mover::Walker };
            let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
            let to = grid.slide((e.x, e.y), (ux * SLAM_THROW, uy * SLAM_THROW), e.radius(), mover);
            (self.enemies[i].x, self.enemies[i].y) = to;
        }
        for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
            if (shot.x - hx).hypot(shot.y - hy) <= reach {
                shot.ttl = 0;
            }
        }
        self.champions.ring((hx, hy), reach, RingKind::Quake);
        self.boom(hx, hy, BoomKind::Dust, reach * 0.6);
        self.fx.push(Fx::Shake(12));
        self.sounds.push("hit_blast");
    }

    fn champion_throws(&mut self) {
        let mut throws = std::mem::take(&mut self.champions.throws);
        let mut kept = Vec::with_capacity(throws.len());
        for mut t in throws.drain(..) {
            t.age += 1;
            let keep = match t.kind {
                Thrown::Hammer => self.fly_hammer(&mut t),
                Thrown::Hook => self.fly_hook(&mut t),
                Thrown::Haul => {
                    // The chain hangs from the wall until the Butcher is there.
                    let hauling = self.players.get(&t.owner).is_some_and(|h| h.champ.haul > 0);
                    hauling && t.age < t.life
                }
                Thrown::Axe => self.fly_axe(&mut t),
                Thrown::Javelin => self.fly_javelin(&mut t),
                Thrown::Flask(brew) => {
                    let ((x, y), _) = lob(&t);
                    (t.x, t.y) = (x, y);
                    if t.age >= t.life {
                        self.flask_lands(t.owner, brew, t.to);
                        false
                    } else {
                        true
                    }
                }
            };
            if keep {
                kept.push(t);
            }
        }
        // Thrown this tick, while these flew: kept after them.
        kept.append(&mut self.champions.throws);
        self.champions.throws = kept;
    }

    fn fly_hammer(&mut self, t: &mut Throw) -> bool {
        let (nx, ny) = (t.x + t.vx * DT, t.y + t.vy * DT);
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let walled = grid.stops_shot(nx, ny);
        (t.x, t.y) = (nx, ny);
        let hit = self
            .enemies
            .iter()
            .position(|e| e.hp > 0 && (e.x - nx).hypot(e.y - ny) <= e.radius() + 0.7);
        if let Some(i) = hit {
            let damage = self.players.get(&t.owner).map_or(HAMMER_DAMAGE, |h| h.spell_damage("storm-hammer", HAMMER_DAMAGE));
            self.smite(t.owner, i, damage, HAMMER_STUN, true);
            let (ex, ey) = (self.enemies[i].x, self.enemies[i].y);
            let burst = self.players.get(&t.owner).map_or(HAMMER_BURST, |h| h.spell_step("storm-hammer").hammer_burst);
            for j in self.near((ex, ey), burst) {
                if j != i {
                    self.smite(t.owner, j, damage / 2, HAMMER_STUN / 2, true);
                }
            }
            self.champions.ring((ex, ey), burst, RingKind::Burst);
            self.fx.push(Fx::Shake(8));
            return false;
        }
        if walled || t.age >= t.life {
            self.champions.ring((t.x, t.y), 1.2, RingKind::Burst);
            return false;
        }
        true
    }

    fn fly_hook(&mut self, t: &mut Throw) -> bool {
        let Some((ox, oy)) = self.players.get(&t.owner).filter(|h| h.hp > 0).map(|h| (h.x, h.y)) else {
            return false;
        };
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        if !t.back {
            let (nx, ny) = (t.x + t.vx * DT, t.y + t.vy * DT);
            if grid.stops_shot(nx, ny) || t.age >= t.life {
                t.back = true;
                return true;
            }
            (t.x, t.y) = (nx, ny);
            let bit = self
                .enemies
                .iter()
                .position(|e| e.hp > 0 && e.kind != EnemyKind::Dummy && (e.x - nx).hypot(e.y - ny) <= e.radius() + 0.6);
            if let Some(i) = bit {
                t.back = true;
                let e = &mut self.enemies[i];
                if anchored(e) {
                    // Too big to drag: it takes the hook where it stands.
                    let damage = self.players.get(&t.owner).map_or(HOOK_DAMAGE, |h| h.spell_damage("meat-hook", HOOK_DAMAGE));
                    self.smite(t.owner, i, damage, HOOK_STUN, true);
                } else {
                    e.frozen = e.frozen.max(HOOK_STUN);
                    t.caught = Some(e.id);
                }
                self.sounds.push("hit");
            }
            return true;
        }
        // Home along the chain, the foe with it.
        let (dx, dy) = (ox - t.x, oy - t.y);
        let d = dx.hypot(dy);
        let step = HOOK_SPEED * DT;
        let home = d <= step + 1.4;
        let (ux, uy) = unit(dx, dy);
        (t.x, t.y) = if home { (ox - ux * 1.4, oy - uy * 1.4) } else { (t.x + ux * step, t.y + uy * step) };
        if let Some(foe) = t.caught
            && let Some(i) = self.enemies.iter().position(|e| e.id == foe && e.hp > 0)
        {
            let e = &mut self.enemies[i];
            e.frozen = e.frozen.max(2);
            let mover = if e.kind.flies() { Mover::Flier } else { Mover::Walker };
            if grid.clear(t.x, t.y, e.radius(), mover) {
                (e.x, e.y) = (t.x, t.y);
            } else {
                (e.x, e.y) = grid.slide((e.x, e.y), (t.x - e.x, t.y - e.y), e.radius(), mover);
            }
            if home {
                let damage = self.players.get(&t.owner).map_or(HOOK_DAMAGE, |h| h.spell_damage("meat-hook", HOOK_DAMAGE));
                self.smite(t.owner, i, damage, HOOK_STUN, true);
                let at = (self.enemies[i].x, self.enemies[i].y);
                self.champions.ring(at, 1.4, RingKind::Burst);
                self.fx.push(Fx::Shake(6));
            }
        }
        !home
    }

    /// The axe spins out, cutting each foe it passes, and comes home to the
    /// Reaver's hand cutting again on the way.
    fn fly_axe(&mut self, t: &mut Throw) -> bool {
        let Some((ox, oy)) = self.players.get(&t.owner).filter(|h| h.hp > 0).map(|h| (h.x, h.y)) else {
            return false;
        };
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let home = if t.back {
            let (dx, dy) = (ox - t.x, oy - t.y);
            let step = AXE_SPEED * DT;
            let (ux, uy) = unit(dx, dy);
            (t.x, t.y) = (t.x + ux * step, t.y + uy * step);
            dx.hypot(dy) <= step + 0.8
        } else {
            let (nx, ny) = (t.x + t.vx * DT, t.y + t.vy * DT);
            if grid.stops_shot(nx, ny) || t.age >= t.life {
                t.back = true;
                return true;
            }
            (t.x, t.y) = (nx, ny);
            false
        };
        let damage = self.players.get(&t.owner).map_or(AXE_DAMAGE, |h| h.spell_damage("hurl-axe", AXE_DAMAGE));
        for i in self.near((t.x, t.y), 0.8) {
            if self.enemies[i].wits.flinch == 0 {
                self.smite(t.owner, i, damage, AXE_STUN, true);
                self.enemies[i].wits.flinch = feel::FLINCH;
            }
        }
        // Lost if it has been out far too long (a wall between it and him).
        !home && t.age < AXE_OUT * 6
    }

    /// The javelin flies straight through every foe on its line.
    fn fly_javelin(&mut self, t: &mut Throw) -> bool {
        let (nx, ny) = (t.x + t.vx * DT, t.y + t.vy * DT);
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        if grid.stops_shot(nx, ny) || t.age >= t.life {
            self.champions.ring((t.x, t.y), 0.9, RingKind::Burst);
            return false;
        }
        (t.x, t.y) = (nx, ny);
        let damage = self.players.get(&t.owner).map_or(JAVELIN_DAMAGE, |h| h.spell_damage("javelin", JAVELIN_DAMAGE));
        for i in self.near((nx, ny), 0.7) {
            if self.enemies[i].wits.flinch == 0 {
                self.smite(t.owner, i, damage, JAVELIN_STUN, true);
                self.enemies[i].wits.flinch = feel::FLINCH;
            }
        }
        true
    }

    fn flask_lands(&mut self, owner: u32, brew: Brew, (x, y): (f32, f32)) {
        let scaled = |n: u32| self.players.get(&owner).map_or(n, |h| h.scaled(n));
        match brew {
            Brew::Foul => {
                self.works.clouds.push(Cloud { x, y, vx: 0.0, vy: 0.0, age: 0, owner, power: self.players.get(&owner).map_or(FOUL_POWER, |h| h.spell_power("miasma-flask", FOUL_POWER)), tint: 0 });
                self.sounds.push("gas_release");
            }
            Brew::Fire | Brew::Quickfire => {
                let quick = brew == Brew::Quickfire;
                let (damage, burst, radius, life) = if quick {
                    let hero = self.players.get(&owner);
                    let reach = hero.map_or(QUICK_RADIUS, |h| h.spell_step("quickfire").quick_reach);
                    let life = hero.map_or(QUICK_LIFE, |h| h.spell_step("quickfire").quick_life);
                    (hero.map_or(QUICK_DAMAGE, |h| h.spell_damage("quickfire", QUICK_DAMAGE)), reach, reach, life)
                } else {
                    (scaled(FLASK_DAMAGE), FLASK_BURST, FLASK_FIRE, FLASK_FIRE_LIFE)
                };
                for i in self.near((x, y), burst) {
                    self.smite(owner, i, damage, 0, true);
                }
                if self.works.blazes.len() < 24 {
                    self.works.blazes.push(Blaze { x, y, age: 0, life, owner, power: if quick { self.players.get(&owner).map_or(skills::BURN_BASE, |h| h.spell_power("quickfire", skills::BURN_BASE)) } else { skills::BURN_BASE }, radius });
                }
                self.boom(x, y, BoomKind::Burst, burst);
                self.champions.ring((x, y), burst, RingKind::Burst);
                self.sounds.push(if quick { "bomb" } else { "spell_fire" });
                if quick {
                    self.fx.push(Fx::Shake(5));
                }
            }
        }
    }

    fn champion_cracks(&mut self) {
        let mut erupt = Vec::new();
        for c in &mut self.champions.cracks {
            c.age += 1;
            if c.age == CRACK_ERUPT {
                erupt.push(*c);
            }
        }
        self.champions.cracks.retain(|c| c.age < CRACK_LIFE);
        for c in erupt {
            let damage = self.players.get(&c.owner).map_or(CRACK_DAMAGE, |h| h.scaled(CRACK_DAMAGE));
            let caught = aoe::reached(&self.enemies, (c.x, c.y), |e| {
                let along = ((e.x - c.x) * c.dx + (e.y - c.y) * c.dy).clamp(0.0, c.len);
                let (px, py) = (c.x + c.dx * along, c.y + c.dy * along);
                (e.x - px).hypot(e.y - py) <= CRACK_WIDE + e.radius() * 0.5
            });
            // One great blow: every foe on the line takes it whole.
            for i in caught {
                self.smite(c.owner, i, damage, CRACK_STUN, false);
            }
            let mut along = 0.0;
            while along <= c.len {
                self.boom(c.x + c.dx * along, c.y + c.dy * along, BoomKind::Dust, 1.4);
                along += 3.0;
            }
            for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                let along = ((shot.x - c.x) * c.dx + (shot.y - c.y) * c.dy).clamp(0.0, c.len);
                if (shot.x - c.x - c.dx * along).hypot(shot.y - c.y - c.dy * along) <= CRACK_WIDE {
                    shot.ttl = 0;
                }
            }
            self.fx.push(Fx::Shake(16));
            self.hitstop.request(&self.tune, hitstop::Impact::Heavy);
            self.sounds.push("bomb");
        }
    }

    fn champion_grips(&mut self) {
        let (tick, barred) = (self.tick, self.barred());
        let mut grips = std::mem::take(&mut self.champions.grips);
        grips.retain_mut(|g| {
            g.left = g.left.saturating_sub(1);
            let Some((hx, hy, ax, ay)) =
                self.players.get(&g.owner).filter(|h| h.hp > 0).map(|h| (h.x, h.y, h.aim_x, h.aim_y))
            else {
                return false;
            };
            let Some(i) = self.enemies.iter().position(|e| e.id == g.foe && e.hp > 0) else {
                return false;
            };
            // Held fast, unless it is too big to hold: then only carved.
            let e = &mut self.enemies[i];
            if !anchored(e) {
                e.frozen = e.frozen.max(2);
                let (ux, uy) = unit(ax, ay);
                let (ux, uy) = if ux == 0.0 && uy == 0.0 { (0.0, 1.0) } else { (ux, uy) };
                let to = (hx + ux * (1.2 + e.radius()), hy + uy * (1.2 + e.radius()));
                let mover = if e.kind.flies() { Mover::Flier } else { Mover::Walker };
                let grid = Grid { room: &self.dungeon.rooms[self.at], barred };
                if grid.clear(to.0, to.1, e.radius(), mover) {
                    (e.x, e.y) = to;
                }
            }
            if tick.is_multiple_of(u64::from(GRIP_EVERY)) {
                let damage = self.players.get(&g.owner).map_or(GRIP_DAMAGE, |h| h.scaled(GRIP_DAMAGE));
                let dealt = self.smite(g.owner, i, damage, 0, false);
                if let Some(hero) = self.players.get(&g.owner) {
                    self.feel.melee.swing(g.owner, hero, melee::Stroke::Slash, 2.4, 150.0);
                    if dealt > 0 { self.feel.melee.contact(g.owner, hero, &self.enemies[i], dealt, false); }
                }
                if let Some(hero) = self.players.get_mut(&g.owner) {
                    hero.mend(dealt / 2, stakes::Mend::Passive);
                }
                self.fx.push(Fx::Shake(3));
            }
            g.left > 0
        });
        grips.append(&mut self.champions.grips);
        self.champions.grips = grips;
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_champions__tests.rs"]
mod tests;
