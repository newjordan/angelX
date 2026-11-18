//! The champions at the bar: seven big knights who play like nobody else
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
//! | Phantom Lancer | Phantom Lance | Spirit Lance: through every foe, an image where it ends | Doppelganger: a blink, three images left | Juxtapose: a thrust may make an image | Phantom Legion |
//! | Mirage Duelist | Mirage Foil | Swashbuckle: a dash that bounces off walls, then four slashes | Mirror Swap: change places with an image | Echo Strike: the next three thrusts strike twice | Hall of Mirrors |
//!
//! Flame meeting foul air flashes (`skills::flash`): the Alchemist's fire
//! thrown into his own miasma is his best stroke. Every number is one fixed
//! number, picked by play.

pub mod mirage;

use super::skills::{Blaze, BoomKind, Cloud};
use super::ults::Ult;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Champion {
    Hammerer,
    Butcher,
    Alchemist,
    Reaver,
    Lancer,
    PhantomLancer,
    MirageDuelist,
}

impl Champion {
    pub const ALL: [Champion; 7] = [
        Champion::Hammerer,
        Champion::Butcher,
        Champion::Alchemist,
        Champion::Reaver,
        Champion::Lancer,
        Champion::PhantomLancer,
        Champion::MirageDuelist,
    ];

    pub fn kit(self) -> &'static str {
        match self {
            Champion::Hammerer => "hammerer",
            Champion::Butcher => "butcher",
            Champion::Alchemist => "alchemist",
            Champion::Reaver => "reaver",
            Champion::Lancer => "lancer",
            Champion::PhantomLancer => "phantomlancer",
            Champion::MirageDuelist => "mirageduelist",
        }
    }

    pub fn of_kit(kit: &str) -> Option<Champion> {
        Champion::ALL.into_iter().find(|c| c.kit() == kit)
    }

    pub fn spells(self) -> [&'static str; 3] {
        match self {
            Champion::Hammerer => ["storm-hammer", "groundslam", "brute-force"],
            Champion::Butcher => ["meat-hook", "rot", "chain-haul"],
            Champion::Alchemist => ["miasma-flask", "quickfire", "elixir"],
            Champion::Reaver => ["hurl-axe", "leap", "bloodlust"],
            Champion::Lancer => ["javelin", "vault", "brace"],
            Champion::PhantomLancer => ["spirit-lance", "doppelganger", "juxtapose"],
            Champion::MirageDuelist => ["swashbuckle", "mirror-swap", "echo-strike"],
        }
    }
}

pub fn of(hero: &Hero) -> Option<Champion> {
    hero.knight.as_deref().and_then(Champion::of_kit)
}

pub fn is_spell(card: &str) -> bool {
    heroes::is_spell(card) || Champion::ALL.iter().any(|c| c.spells().contains(&card))
}

pub const HAMMER_SPEED: f32 = 18.0;
pub const HAMMER_LIFE: u32 = 22;
pub const HAMMER_DAMAGE: u32 = 50;
const HAMMER_STUN: u32 = 2 * HZ;
pub const HAMMER_BURST: f32 = 2.8;
pub const SLAM_WINDUP: u32 = 12;
pub const SLAM_REACH: f32 = 5.0;
pub const SLAM_DAMAGE: u32 = 45;
const SLAM_STUN: u32 = HZ;
const SLAM_THROW: f32 = 2.2;
pub const FURY_TICKS: u32 = 6 * HZ;
pub const CRACK_LEN: f32 = 18.0;
pub const CRACK_RUN: u32 = 20;
pub const CRACK_ERUPT: u32 = 28;
pub const CRACK_LIFE: u32 = 50;
const CRACK_WIDE: f32 = 1.9;
const CRACK_DAMAGE: u32 = 130;
const CRACK_STUN: u32 = 2 * HZ;

pub const HOOK_SPEED: f32 = 26.0;
const HOOK_OUT: u32 = 16;
pub const HOOK_DAMAGE: u32 = 55;
const HOOK_STUN: u32 = HZ;
pub const ROT_TICKS: u32 = 5 * HZ;
pub const ROT_REACH: f32 = 3.4;
pub const ROT_EVERY: u32 = 10;
pub const ROT_DAMAGE: u32 = 9;
pub const HAUL_SPEED: f32 = 36.0;
const HAUL_DAMAGE: u32 = 25;
const GRIP_REACH: f32 = 5.0;
pub const GRIP_TICKS: u32 = 3 * HZ;
const GRIP_EVERY: u32 = 10;
const GRIP_DAMAGE: u32 = 16;

pub const FLASK_REARM: u32 = 20;
const FLASK_REACH: f32 = 8.0;
pub const FLASK_FLIGHT: u32 = 14;
pub const FLASK_DAMAGE: u32 = 22;
pub const FLASK_BURST: f32 = 1.8;
pub const FLASK_FIRE: f32 = 1.4;
pub const FLASK_FIRE_LIFE: u32 = 2 * HZ;
pub const QUICK_DAMAGE: u32 = 34;
pub(super) const QUICK_RADIUS: f32 = 3.0;
pub(super) const QUICK_LIFE: u32 = 5 * HZ;
pub const FOUL_POWER: u32 = 5;
pub const ELIXIR_REACH: f32 = 6.0;
pub const ELIXIR_MEND: u32 = 35;
const RAIN_FOES: usize = 8;
const RAIN_REACH: f32 = 16.0;
const RAIN_STAGGER: u32 = 3;

pub const AXE_SPEED: f32 = 20.0;
const AXE_OUT: u32 = 14;
pub const AXE_DAMAGE: u32 = 34;
const AXE_STUN: u32 = 8;
pub const LEAP_REACH: f32 = 9.0;
pub const LEAP_SPEED: f32 = 30.0;
pub const LEAP_QUAKE: f32 = 3.0;
pub const LEAP_DAMAGE: u32 = 40;
const LEAP_STUN: u32 = HZ;
pub const LUST_TICKS: u32 = 6 * HZ;
pub const LUST_FEED: u32 = 30;
pub const LUST_PACE: f32 = 1.3;
pub const HARVEST_REACH: f32 = 4.5;
pub const HARVEST_CULL: u32 = 30;
pub const HARVEST_DAMAGE: u32 = 90;
const HARVEST_REFUND: u32 = ults::ULT_FULL / 5;

pub const JAVELIN_SPEED: f32 = 28.0;
pub const JAVELIN_LIFE: u32 = 18;
pub const JAVELIN_DAMAGE: u32 = 42;
const JAVELIN_STUN: u32 = 10;
pub const VAULT_REACH: f32 = 5.5;
pub const VAULT_SPEED: f32 = 26.0;
const VAULT_DAMAGE: u32 = 20;
const VAULT_STUN: u32 = 14;
pub const BRACE_TICKS: u32 = 2 * HZ;
pub const BRACE_TAKES: u32 = 50;
pub const BRACE_REACH: f32 = 3.2;
const BRACE_DAMAGE: u32 = 30;
const BRACE_STUN: u32 = HZ;
const BRACE_THROW: f32 = 1.8;
pub const DRIVE_REACH: f32 = 14.0;
pub const DRIVE_SPEED: f32 = 34.0;
pub const DRIVE_DAMAGE: u32 = 110;
const DRIVE_WALL: u32 = 50;
const DRIVE_STUN: u32 = 2 * HZ;

pub const SPIRIT_SPEED: f32 = 28.0;
pub const SPIRIT_LIFE: u32 = 18;
pub const SPIRIT_DAMAGE: u32 = 36;
const SPIRIT_STUN: u32 = 8;
pub const SPIRIT_IMAGE: u32 = 6 * HZ;
pub const DOPPEL_REACH: f32 = 6.0;
pub const DOPPEL_IMAGE: u32 = 5 * HZ;
const DOPPEL_APART: f32 = 0.9;
pub const JUXTA_TICKS: u32 = 8 * HZ;
pub const JUXTA_ODDS: u64 = 3;
pub const JUXTA_IMAGE: u32 = 4 * HZ;
const JUXTA_BESIDE: f32 = 1.8;
pub const LEGION_IMAGES: usize = 4;
pub const LEGION_TICKS: u32 = 8 * HZ;
const LEGION_RING: f32 = 2.6;

pub const CAST_POSE: u32 = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stance {
    #[serde(default)]
    pub windup: u32,
    #[serde(default)]
    pub fury: u32,
    #[serde(default)]
    pub rot: u32,
    #[serde(default)]
    pub haul: u32,
    #[serde(default)]
    pub haul_x: f32,
    #[serde(default)]
    pub haul_y: f32,
    #[serde(default)]
    pub cast: u32,
    #[serde(default)]
    pub spell: u8,
    #[serde(default)]
    pub lust: u32,
    #[serde(default)]
    pub brace: u32,
    #[serde(default, skip_serializing_if = "super::zero")]
    pub juxta: u32,
    #[serde(default, skip_serializing_if = "mirage::Swash::idle")]
    pub swash: mirage::Swash,
    #[serde(default, skip_serializing_if = "super::is_zero")]
    pub echoes: u8,
    #[serde(default, skip_serializing_if = "super::zero")]
    pub echo_ticks: u32,
    #[serde(default)]
    pub dash: u32,
    #[serde(default)]
    pub dash_kind: Dash,
    #[serde(default)]
    pub dash_x: f32,
    #[serde(default)]
    pub dash_y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dash {
    #[default]
    Leap,
    Vault,
    Drive,
}

impl Dash {
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

    pub fn busy(&self) -> bool {
        self.windup > 0 || self.haul > 0 || self.dash > 0 || self.brace > 0 || self.swash.dashing()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thrown {
    Hammer,
    Hook,
    Haul,
    Flask(Brew),
    Axe,
    Javelin,
    SpiritLance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Brew {
    Fire,
    Foul,
    Quickfire,
}

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
    Reap,
}

pub const RING_LIFE: u32 = 14;

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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grip {
    pub owner: u32,
    pub foe: u32,
    pub left: u32,
}

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skewered: Vec<Grip>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub echoes: Vec<mirage::Echo>,
}

impl Field {
    pub fn is_empty(&self) -> bool {
        self.throws.is_empty()
            && self.rings.is_empty()
            && self.cracks.is_empty()
            && self.grips.is_empty()
            && self.skewered.is_empty()
            && self.echoes.is_empty()
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

pub fn lob(t: &Throw) -> ((f32, f32), f32) {
    let k = (t.age as f32 / t.life.max(1) as f32).clamp(0.0, 1.0);
    let at = (t.from.0 + (t.to.0 - t.from.0) * k, t.from.1 + (t.to.1 - t.from.1) * k);
    (at, 4.0 * 2.2 * k * (1.0 - k))
}

fn anchored(e: &Enemy) -> bool {
    e.boss.is_some() || matches!(e.kind, EnemyKind::Dragon | EnemyKind::Dummy | EnemyKind::Ward)
}

impl Run {
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
            "spirit-lance" => {
                self.champions.throws.push(fly(Thrown::SpiritLance, SPIRIT_SPEED, SPIRIT_LIFE));
                self.sounds.push("shot_bolt");
            }
            "doppelganger" => {
                self.doppelganger(id);
                self.sounds.push("roll");
            }
            "juxtapose" => {
                hero.champ.juxta = JUXTA_TICKS;
                self.sounds.push("shield_up");
            }
            "swashbuckle" => self.swashbuckle(id),
            "mirror-swap" => {
                self.mirror_swap(id);
                self.sounds.push("roll");
            }
            "echo-strike" => {
                hero.champ.echoes = mirage::ECHO_THRUSTS;
                hero.champ.echo_ticks = mirage::ECHO_TICKS;
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
                        hero.ult_charge = ults::ULT_FULL;
                    }
                }
                self.sounds.push("boss_rise");
            }
            Ult::PhantomLegion => {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.champ.cast = CAST_POSE;
                    hero.champ.spell = 4;
                }
                self.phantoms.retain(|p| p.owner != id);
                let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
                for k in 0..LEGION_IMAGES {
                    let a = ay.atan2(ax) + std::f32::consts::TAU * (k as f32 + 0.5) / LEGION_IMAGES as f32;
                    for r in [LEGION_RING, LEGION_RING * 0.7, LEGION_RING * 0.4] {
                        let (dx, dy) = (a.cos() * r, a.sin() * r);
                        if grid.clear(hx + dx, hy + dy, HERO_RADIUS, Mover::Hero) {
                            ults::add_image(&mut self.phantoms, ults::Phantom::beside(id, (hx, hy), (dx, dy), LEGION_TICKS));
                            break;
                        }
                    }
                }
            }
            Ult::HallOfMirrors => self.hall_of_mirrors(id),
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
                if matches!(of(hero), Some(Champion::Hammerer | Champion::Butcher | Champion::Reaver | Champion::Lancer | Champion::PhantomLancer)) && stun > 0 {
                    self.feel.melee.contact(owner, hero, enemy, dealt, false);
                }
            }
            self.sounds.push("hit");
        }
        self.feed_bloodlust(&[(owner, dealt)]);
        dealt
    }

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

    fn near(&self, (x, y): (f32, f32), r: f32) -> Vec<usize> {
        aoe::reached(&self.enemies, (x, y), |e| (e.x - x).hypot(e.y - y) <= r + e.radius() * 0.5)
    }

    pub(super) fn tick_champions(&mut self) {
        self.champion_stances();
        self.champion_throws();
        for ring in &mut self.champions.rings {
            ring.age += 1;
        }
        self.champions.rings.retain(|r| r.age < r.life);
        self.champion_cracks();
        self.champion_grips();
        self.tick_echoes();
        self.tick_mirrors();
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
            s.juxta = s.juxta.saturating_sub(1);
            s.echo_ticks = s.echo_ticks.saturating_sub(1);
            if s.echo_ticks == 0 {
                s.echoes = 0;
            }
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
                (hero.champ.dash_kind, hero.champ.dash_x, hero.champ.dash_y) = (Dash::default(), 0.0, 0.0);
            }
            self.champion_bound(id, (hx, hy));
            self.champion_brace(id, (hx, hy));
            self.swash_flurry(id);
        }
    }

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
                    let hauling = self.players.get(&t.owner).is_some_and(|h| h.champ.haul > 0);
                    hauling && t.age < t.life
                }
                Thrown::Axe => self.fly_axe(&mut t),
                Thrown::Javelin | Thrown::SpiritLance => self.fly_javelin(&mut t),
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
        !home && t.age < AXE_OUT * 6
    }

    fn fly_javelin(&mut self, t: &mut Throw) -> bool {
        let spirit = t.kind == Thrown::SpiritLance;
        let (spell, base, stun) = if spirit { ("spirit-lance", SPIRIT_DAMAGE, SPIRIT_STUN) } else { ("javelin", JAVELIN_DAMAGE, JAVELIN_STUN) };
        let (nx, ny) = (t.x + t.vx * DT, t.y + t.vy * DT);
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        if grid.stops_shot(nx, ny) || t.age >= t.life {
            self.champions.ring((t.x, t.y), 0.9, RingKind::Burst);
            if spirit {
                let at = grid.halted(t.from, (t.x - t.from.0, t.y - t.from.1));
                if self.players.get(&t.owner).is_some_and(|h| h.hp > 0) {
                    ults::add_image(&mut self.phantoms, ults::Phantom::standing(t.owner, at, SPIRIT_IMAGE));
                }
            }
            return false;
        }
        (t.x, t.y) = (nx, ny);
        let damage = self.players.get(&t.owner).map_or(base, |h| h.spell_damage(spell, base));
        for i in self.near((nx, ny), 0.7) {
            if self.enemies[i].wits.flinch == 0 {
                self.smite(t.owner, i, damage, stun, true);
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

impl Run {
    fn doppelganger(&mut self, id: u32) {
        let Some(hero) = self.players.get(&id).filter(|h| h.hp > 0) else {
            return;
        };
        let (hx, hy) = (hero.x, hero.y);
        let (ax, ay) = unit(hero.aim_x, hero.aim_y);
        let (ax, ay) = if ax == 0.0 && ay == 0.0 { (0.0, -1.0) } else { (ax, ay) };
        let grid = Grid { room: &self.dungeon.rooms[self.at], barred: self.barred() };
        let on_floor = |x: f32, y: f32| {
            let r = HERO_RADIUS;
            [(-r, -r), (r, -r), (-r, r), (r, r), (0.0, 0.0)]
                .into_iter()
                .all(|(dx, dy)| {
                    let (px, py) = (x + dx, y + dy);
                    let col = ((px / TILE_UNITS).floor() as i32).clamp(0, grid.room.cols as i32 - 1);
                    let row = ((py / TILE_UNITS).floor() as i32).clamp(0, grid.room.rows as i32 - 1);
                    matches!(
                        grid.room.tile(col, row),
                        Tile::Floor | Tile::Stairs | Tile::Ledge | Tile::Bog
                    )
                })
        };
        let (cx, cy) = (
            grid.room.cols as f32 * TILE_UNITS / 2.0,
            grid.room.rows as f32 * TILE_UNITS / 2.0,
        );
        let mut to = (hx, hy);
        let mut far = DOPPEL_REACH;
        while far > 0.0 {
            let at = grid.halted((hx, hy), (ax * far, ay * far));
            if on_floor(at.0, at.1)
                && !self.enemies.iter().any(|e| e.hp > 0 && (e.x - at.0).hypot(e.y - at.1) < e.radius() + HERO_RADIUS)
            {
                to = at;
                break;
            }
            far -= 0.5;
        }
        let beside = |c: (f32, f32), side: f32| {
            for apart in [DOPPEL_APART, DOPPEL_APART * 0.5] {
                for s in [side, -side] {
                    let at = (c.0 - ay * s * apart, c.1 + ax * s * apart);
                    if on_floor(at.0, at.1) {
                        return at;
                    }
                }
            }
            if on_floor(c.0, c.1) {
                return c;
            }
            let (dx, dy) = (cx - c.0, cy - c.1);
            let dist = dx.hypot(dy).max(0.01);
            let mut gone = 0.4;
            while gone < dist {
                let at = (c.0 + dx / dist * gone, c.1 + dy / dist * gone);
                if on_floor(at.0, at.1) {
                    return at;
                }
                gone += 0.4;
            }
            (cx, cy)
        };
        if !on_floor(to.0, to.1) {
            to = beside(to, 1.0);
        }
        let made = [beside((hx, hy), 1.0), beside((hx, hy), -1.0), beside(to, 1.0)];
        for at in made {
            ults::add_image(&mut self.phantoms, ults::Phantom::standing(id, at, DOPPEL_IMAGE));
        }
        if fx::sparks_room(&self.sparks, &self.fx) {
            self.sparks.push(Spark { from: (hx, hy), to, ttl: 7 });
        }
        if let Some(hero) = self.players.get_mut(&id) {
            (hero.x, hero.y) = to;
        }
    }
}

pub(super) fn juxtapose(phantoms: &mut Vec<ults::Phantom>, hero: &Hero, id: u32, salt: u64, grid: &Grid) {
    if hero.champ.juxta == 0 || hero.hp == 0 || phantoms.iter().any(|p| p.owner == id && p.charge) {
        return;
    }
    let roll = mix(salt.wrapping_mul(0x9e37_79b9) ^ (u64::from(id) << 48));
    if roll % JUXTA_ODDS != 0 {
        return;
    }
    let (ax, ay) = unit(hero.aim_x, hero.aim_y);
    let (ax, ay) = if ax == 0.0 && ay == 0.0 { (0.0, -1.0) } else { (ax, ay) };
    let side = if (roll >> 20) & 1 == 0 { 1.0 } else { -1.0 };
    for s in [side, -side] {
        let (dx, dy) = (-ay * s * JUXTA_BESIDE, ax * s * JUXTA_BESIDE);
        if grid.clear(hero.x + dx, hero.y + dy, HERO_RADIUS, Mover::Hero) {
            ults::add_image(phantoms, ults::Phantom::beside(id, (hero.x, hero.y), (dx, dy), JUXTA_IMAGE));
            return;
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_champions__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/together_phantom_lancer__tests.rs"]
mod phantom_lancer_tests;
