//! Fixed-step, model-free dungeon crawl: knights against the monsters and
//! demons of a freshly generated dungeon. One step is 1/30 second; input
//! arrival never advances time. All entity counts and per-frame work are
//! bounded.
//!
//! The loop: walk into a room, its doors bar shut, fight what lives there,
//! pick up what it drops, take the stairs down. Each floor is new (see
//! `layout`), the third ends in the dragon's lair. Knights fire quick streams,
//! piercing rails and close fans; monsters answer with slow shots worth reading.

pub mod aerie;
pub mod altar;
mod aftermath;
pub mod aoe;
mod arms;
pub mod bodies;
#[cfg(test)]
#[path = "../tests/rules/together_audience__tests.rs"]
mod audience_retirement_tests;
#[cfg(all(test, feature = "bend-cases-core"))]
#[path = "../tests/rules/core_bend_cases__tests.rs"]
mod core_bend_cases_tests;
pub mod barony;
pub mod bard;
pub mod peasant;
pub mod champions;
pub mod heroes;
pub mod drills;
pub mod weave;
pub mod recruit_ults;
pub mod muster_tent;
pub mod squire_rooms;
pub mod bestiary;
mod boss_ecology;
mod boss_gates;
mod boss_population;
mod pacing;
pub mod bosses;
pub mod bounties;
pub mod bowcountry;
pub mod cards;
pub mod card_paint;
pub mod speculative;
pub mod scrolls;
pub mod cauldron;
pub mod brew_jobs;
pub mod reforge_jobs;
pub mod wish_jobs;
pub mod cat;
pub mod citizens;
pub mod chariot;
pub mod chatter;
pub mod classes;
pub mod command;
pub mod coastal;
pub mod crown;
pub mod college;
pub mod colosseum;
pub mod alehouse;
pub mod bazaar;
pub mod curios;
pub mod countryside;
pub mod court;
pub mod critters;
pub mod dares;
pub mod depths;
pub mod bombs;
pub mod desperate;
mod encounter_catalog;
pub mod exhibits;
pub mod exits;
mod mend;
pub mod feats;
pub mod fellowship;
pub mod feel;
pub mod melee;
pub mod ranged;
mod flight;
mod scratch;
pub mod floor_map;
mod floors;
pub mod foes;
pub mod folly;
pub mod folk;
pub mod foreman;
pub mod fortune;
pub mod furnace;
pub mod frogs;
pub mod gantry;
pub mod gear;
pub mod guide;
pub mod hazards;
mod hell;
mod hero;
mod spell_upgrades;
pub mod hexer;
pub mod identity;
pub mod hippodrome;
pub mod hireling;
pub mod squires;
pub mod squires_lodge;
pub mod squire_guild;
pub mod countryside_squires;
pub mod lane_squires;
pub mod raisings;
pub mod squire_company;
pub mod squire_hall;
pub mod squire_academy;
pub mod squires_town;
pub mod minions;
pub mod dead_wishes;
pub mod holdings;
pub mod villagers;
pub mod hollow;
pub mod home;
pub mod hud;
pub mod hunters;
pub mod imps;
pub mod items;
pub mod joust;
pub mod keyring;
pub mod kingdom;
pub mod tune;
pub mod runtime_state;
pub mod session;
pub mod hitstop;
pub mod gore;
pub mod knights;
pub mod layout;
pub mod ledge;
pub mod lich;
pub mod looks;
pub mod vfx;
pub mod creator;
mod loot;
pub mod merchant;
pub mod merlin;
pub mod mirror;
pub mod missions;
pub mod music;
mod monsters;
pub mod mummers;
pub mod outlaw;
pub mod overclass;
pub mod genie;
pub mod cosmetic_wish;
pub mod freedom;
pub mod starfall;
pub mod rewards;
pub mod pastimes;
pub mod crossroads;
pub mod cinder;
pub mod latch;
pub mod brasswake;
pub mod rafterwake;
pub mod pennon;
pub mod mossbell;
pub mod patterns;
pub mod perils;
pub mod gauntlets;
pub mod phrasebook;
pub mod artistry;
pub mod content;
pub mod fidelity;
pub mod resolution;
pub mod combos;
pub mod cover;
pub mod questline;
pub mod campaign;
pub mod pit;
pub mod pitstop;
pub mod pots;
pub mod hop;
mod hunt;
pub mod activity;
pub mod fx;
pub mod map;
pub use fx::Fx;
pub mod race3d;
pub mod powder;
pub mod presence;
pub mod again;
pub use presence::Hold;
pub mod press;
pub mod race_wagers;
pub mod ranch;
pub mod raids;
pub mod relics;
pub mod rescues;
pub mod rooms;
pub mod salvage;
mod risen;
pub mod runes;
pub mod rubble;
pub mod script;
pub mod secrets;
pub mod shield;
pub mod team;
pub mod shoulders;
pub mod signatures;
pub mod skills;
pub mod skulls;
pub mod spectacle;
mod spells;
pub mod abilities;
pub mod host_commands;
pub mod home_settlement;
pub mod spin;
mod spoils;
pub mod stakes;
pub mod thief;
pub mod wetland;
pub mod swamp;
pub mod horrors;
pub mod windcountry;
pub mod wishcraft;
pub mod woodland;
mod wounds;
use stakes::FURY;
pub mod floor_plates;
pub mod floor_stats;
pub mod forest;
pub mod forest_road;
// The Round Table adventures (docs/ROUNDTABLE-ADVENTURES.md): the shared shell,
// then one module each. Set in the prep commit; a builder adds no line here.
pub mod roundtable;
mod room_end;

pub mod lanternward;

pub mod moonfall;

pub mod bannerpeak;

pub mod griffinwing;

pub mod lightgallop;
pub mod knight_racers;
pub mod deathmatch;
pub mod talents;
pub mod tavern;
pub mod tide;
pub mod nooks;
pub mod deep;
pub mod town;
pub mod tray;
pub mod trinkets;
pub mod trophies;
pub mod ults;
pub mod votes;
pub mod works;
pub mod world;
pub mod yard;

use crate::realm::{Haul, Spoil, Spoils};
pub use boss_gates::Gates as BossGateState;
pub use bosses::Boss;
pub use cards::{Book, Card};
pub use hazards::{Rock, Trap, Waves};
pub use hero::Hero;
use hero::Loosed;
pub use layout::{COLS, DIRS, Floor, Pack, ROWS, Room, RoomKind, TILE_UNITS, Tile};
pub use loot::{Item, Rng, Weapon};
pub use monsters::{Enemy, EnemyKind, compact_save};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WIDTH: f32 = 48.0;
pub const HEIGHT: f32 = 28.0;
pub const HZ: u32 = 30;
/// Dragon Keep's floor: the dragon, and the old delve's end.
pub const FLOORS: u32 = 3;
/// The bottom of the deep below it: the Unknown.
pub const DEEPEST: u32 = 6;
/// The first floor of the deep below the dragon (the Archive): from here
/// down the dungeon stands up in 3D (`world3d::room`, docs/WORLD-3D.md
/// "Deeper floors").
pub const DEEP_FLOOR: u32 = 4;
pub const MAX_PROJECTILES: usize = 384;
/// The most shots one knight may have in flight at once: their arm's
/// volleys (`KNIGHT_VOLLEY`) and what those fork and split into
/// (`KNIGHT_CHILDREN`), however many wishes stack
/// (`tests/client_stacked_wishes__tests.rs`). Was 48 with forks sharing it:
/// a stacked arm filled the room with a solid wall of shots, cleared every
/// room standing still and was the bulk of the late-game lag (the 2026-11
/// play-test). Now a stacked knight's fire is a third smaller and every
/// tick's shot-by-monster test has a third less to do.
pub const KNIGHT_SHOTS: usize = KNIGHT_VOLLEY + KNIGHT_CHILDREN;
/// Of `KNIGHT_SHOTS`, the most a knight's own loosed shots take.
pub const KNIGHT_VOLLEY: usize = 22;
/// Forks and splits spawn only while their knight has fewer than this many
/// children (`ShotTraits::forked`) in flight. The volley does not count: a
/// spread arm's own fan used to fill the budget and starve every fork.
pub const KNIGHT_CHILDREN: usize = 12;

/// Knight `id`'s own loosed shots in flight (not forks or splits), for
/// `KNIGHT_VOLLEY`.
pub fn volley_in_flight(shots: &[Projectile], id: u32) -> usize {
    shots.iter().filter(|p| p.loosed_by(id) && !p.traits.forked).count()
}

/// Knight `id`'s shots in flight among `shots`.
pub fn in_flight(shots: &[Projectile], id: u32) -> usize {
    shots
        .iter()
        .filter(|p| p.loosed_by(id))
        .count()
}

/// Of the shots loosed from `from` on, at most `room` fly: the volley's
/// middle, so a knight at `KNIGHT_SHOTS` still shoots where they aim.
fn fit_volley(shots: &mut Vec<Projectile>, from: usize, room: usize) {
    let loosed = shots.len().saturating_sub(from);
    if loosed <= room {
        return;
    }
    let cut = loosed - room;
    shots.truncate(shots.len() - (cut - cut / 2));
    shots.drain(from..from + cut / 2);
}
/// Absolute bomb ceiling; each floor grants its own belt (`bombs::at`).
pub const MAX_BOMBS: u32 = 12;
pub const MAX_ARMOR: u32 = 2;
/// A party of six: the host and five invited friends. A host who plays no
/// knight (`Hero::absent`) leaves seats 2 to 6 for five friends.
pub const MAX_PLAYERS: u32 = 6;
/// A knight whose player is here but has pressed nothing for this long is
/// away too (`Run::away`): a host gone back to their code with the game
/// still on screen. (A player not here at all, a page closed or hidden, is
/// away from the moment the host's server says so: `Run::set_away`.) The
/// party neither waits on an away knight nor is moved by one; it carries
/// them.
pub const AWAY_TICKS: u32 = 20 * HZ;
const MAX_ITEMS: usize = 24;
pub const DT: f32 = 1.0 / HZ as f32;
/// How far apart a party of `count` stands in a row when it walks in: 2.4
/// a knight up to four, then no wider a row than those four make (the
/// doorway's clear ground), so a fifth and sixth stand closer.
pub(crate) fn stand_apart(count: f32) -> f32 {
    if count > 4.0 { 3.0 * 2.4 / (count - 1.0) } else { 2.4 }
}
pub const HERO_RADIUS: f32 = 0.42;
const HERO_SPEED: f32 = 13.0;
/// The dodge roll: a short tumble, untouchable throughout, then a rearm.
pub const ROLL_TICKS: u32 = 10;
const ROLL_SPEED: f32 = 27.0;
const ROLL_REARM: u32 = 36;
/// How much of its speed a knight keeps behind a raised shield.
const SHIELD_PACE: f32 = 0.45;
/// Mana: what a held wall drinks. It flows back after a short rest.
pub const MAX_MANA: f32 = 100.0;
const MANA_REST: u32 = HZ / 2;
const MANA_FLOW: f32 = 22.0;
/// A wall stands this far ahead of the knight holding it.
const WALL_REACH: f32 = 1.7;
const WALL_PACE: f32 = 0.6;

/// Ticks a knight walks one way with no aim held before facing it: half a
/// second.
pub const TURN_TO_WALK: u32 = HZ / 2;

/// Down to this floor a boss stands alone in its room: no camp troops,
/// escort or party extras come in with it (its own summons still do).
pub const BOSS_ALONE_TO: u32 = 3;

/// How far a boss rises from every knight in the room (units; a tile is
/// two): a party coming in through a door never stands on top of one.
pub const BOSS_CLEARANCE: f32 = 12.0;

/// A fallen knight rises at the stairs with this share of their health,
/// in percent.
pub const REVIVE_SHARE: u32 = 25;
/// Ticks a knight brought back to their feet by the stairs is warded.
pub const REVIVE_WARD: u32 = 2 * HZ;
/// Ticks a knight walking into a room is warded: a whole second to see it.
pub const ENTRY_WARD: u32 = HZ;
/// Arena bombs keep their fixed, first-floor damage.
pub const BOMB_DAMAGE: u32 = bombs::FLOORS[0].damage;
/// Arena bomb reach; dungeon bombs use `bombs::at`.
pub const BOMB_CLEAR: f32 = 9.0;
pub const BOMB_REARM: u32 = HZ;
/// The vigil, for a player stepping away: how far the statue mends, how
/// often, and how long its key rests after raising it and after waking.
pub const VIGIL_REACH: f32 = 7.0;
const VIGIL_MEND_EVERY: u32 = HZ / 3;
const VIGIL_SETTLE: u32 = HZ;
const VIGIL_REARM: u32 = 5 * HZ;
const PICKUP_REACH: f32 = 1.3;
/// How near a weapon must lie for the take key to swap to it: on it, or
/// beside it.
pub const TAKE_REACH: f32 = 2.4;
/// What a knight standing on a card for the hand hears when the hand is
/// already full (`gather`).
pub const HAND_FULL: &str = "hand full";

/// Whether a `found` line is a knight's own take off the floor (`gather`),
/// which their seat shows over them a moment.
pub fn take_line(line: &str) -> bool {
    line == HAND_FULL
        || ["took ", "tempered "].iter().any(|w| line.starts_with(w))
        || [": worn", " to drink", " to use", ": in hand"].iter().any(|w| line.ends_with(w))
}
/// A held number key plays one card, then waits this long.
pub const PLAY_REARM: u32 = HZ / 2;
/// The fan between extra shots from held cards, in radians.
const FAN: f32 = 0.2;
/// A volley flies wider: three shots across about forty degrees.
const VOLLEY_FAN: f32 = 0.35;
/// How far a chain hit's spark reaches for the next monster.
const CHAIN_REACH: f32 = 6.5;
/// How long a burst's ring shows.
pub const BLAST_TICKS: u8 = 9;
/// How far a handgonne ball's burst reaches.
const BURST_REACH: f32 = 2.5;
/// How fast a thrown blade flies, out and back.
const BLADE_SPEED: f32 = 20.0;
/// A morningstar's orbit: its distance from the knight, its spin (radians
/// a second), how often it strikes what it touches, and how hard.
pub const ORBIT_RADIUS: f32 = 2.4;
pub const ORBIT_SPIN: f32 = 5.0;
/// Morningstars strike on a beat, every this many ticks: each monster they
/// touched since the last beat is struck once, as one stroke (`aoe`).
pub const ORBIT_EVERY: u32 = 10;
/// What each strike deals.
pub const ORBIT_DAMAGE: u32 = 12;
/// The shared orbit layout, also used when the targets are other knights.
pub fn orbit_points(hero: &Hero, id: u32, tick: u64) -> Vec<(f32, f32)> {
    let n = hero.bonus.orbit.min(cards::cap("orbit"));
    (0..n)
        .map(|k| {
            let a = tick as f32 * ORBIT_SPIN * DT
                + k as f32 * std::f32::consts::TAU / n as f32
                + id as f32;
            (
                hero.x + a.cos() * ORBIT_RADIUS,
                hero.y + a.sin() * ORBIT_RADIUS,
            )
        })
        .collect()
}
/// How fast a homing shot turns, radians a second per point of homing.
pub const HOMING_TURN: f32 = 2.2;
/// Every knight's sword: a close arc that also turns shots in the air. Its
/// slash carries further and wider than the blade (`melee::slash_reach`).
pub const SWORD_DAMAGE: u32 = 30;
pub const SWORD_REACH: f32 = 2.2;
pub const SWORD_ARC: f32 = 130.0;
pub const SWORD_REARM: u32 = 18;
/// A Sanctuary's privy: how near its door a knight steps in (`pitstop`).
const PRIVY_REACH: f32 = 1.2;
/// A fresh room's monsters hold still and silent this long.
pub const TELEGRAPH: u32 = HZ;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    #[serde(default, skip_serializing_if = "mirror_idle")]
    pub mirror: u8,
    /// The picture tick for a duel counter or table stroke. Repeats clear it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latch_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leave_activity: bool,
    /// Menu held in a mini-game; the host counts consecutive fixed steps.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub leave_held: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub activity_menu: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vote: Option<votes::Command>,
    pub move_x: i8,
    pub move_y: i8,
    pub aim_x: i8,
    pub aim_y: i8,
    pub fire: bool,
    pub dash: bool,
    pub bomb: bool,
    /// A sword swing: every knight carries a blade beside their weapon.
    #[serde(default)]
    pub swing: bool,
    /// Hand slot 1–4 to play; 0 plays nothing.
    #[serde(default)]
    pub play: u8,
    /// Spell slot 1–4; 0 casts nothing.
    #[serde(default)]
    pub cast: u8,
    /// The vigil key: its press turns the knight to warded stone, or wakes
    /// it. Left out when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub vigil: bool,
    /// The ultimate's key (R): its press casts a full charge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ult: bool,
    /// The take key (P): its press swaps to the weapon lying beside the
    /// knight. Left out when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub take: bool,
    /// The interact key (E): plates, levers, cages, the folk and their
    /// booths. Fire only shoots. Left out when unpressed, so an older host
    /// reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub interact: bool,
    /// The skill key held: 1 Fart Cloud, 2 Fire Pit, 3 Mines, 4 Barrier,
    /// 5 Shield Throw (0 none).
    /// Left out when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "skills::unpressed")]
    pub skill: u8,
    /// The ride key (C): climbs onto the shoulders of the knight beside,
    /// hops off them, or, for a carrier, throws the top rider
    /// (`shoulders`). Left out when unpressed, so an older host reads the
    /// rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ride: bool,
    /// H: tap to relay; hold facing a friend to tether.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub team: bool,
    /// The next-card key (P): its press puts the first card in hand at the
    /// back, so the one behind it is next (`Hero::cycle_hand`). Left out
    /// when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cycle: bool,
    /// A fresh key press dismisses only this seat’s transition picture.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip_plate: bool,
    /// Asks to go back to the kingdom once the run is over (`again`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub again: bool,
}

fn mirror_idle(n: &u8) -> bool { *n == 0 }

impl Input {
    pub fn valid(self) -> bool {
        [self.move_x, self.move_y, self.aim_x, self.aim_y]
            .into_iter()
            .all(|axis| (-1..=1).contains(&axis))
            && usize::from(self.play) <= cards::HAND
            && usize::from(self.cast) <= cards::SPELL_SLOTS
            && usize::from(self.skill) <= skills::SKILLS
            && self.mirror <= 6
            && self.vote.is_none_or(votes::Command::valid)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Monsters in the room; its doors are barred.
    Fighting,
    /// The room is clear: doors open, loot to gather, stairs to find.
    Exploring,
    Won,
    Wiped,
}

/// What a projectile is, for its damage and its look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shot {
    /// A thrown blade, spinning out and back.
    Blade,
    Arrow,
    Bolt,
    Ball,
    Bone,
    Orb,
    Ember,
    /// Pilgrim's Arrow and Assassinate: one great shot.
    Sacred,
    /// The Hexer's bolt: it hurts nobody, and makes a frog of a knight.
    Hex,
    /// The Lich's frost: it chills, and Rimeleap leaps knight to knight.
    Frost,
    /// A heat-seeking missile.
    Missile,
    /// The monsters' pattern shots, one look to a delve (`patterns`): the
    /// Crypt's pale wisps, the Mines' ore, Dragon Keep's flames, the
    /// Archive's torn pages, the Fungal Deep's spores, the Unknown's eyes.
    Wisp,
    Ore,
    Flame,
    Page,
    Spore,
    Eye,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Projectile {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub hostile: bool,
    pub kind: Shot,
    #[serde(default)]
    pub look: Option<(String, String)>,
    pub damage: u32,
    pub pierce: u8,
    pub last_hit: Option<u32>,
    pub ttl: u32,
    /// Passed through a friend's wall: it hits harder and burns gold.
    #[serde(default)]
    pub empowered: bool,
    /// What the shooter's held cards gave this shot.
    #[serde(default)]
    pub traits: ShotTraits,
}

/// A knight's shot's gifts from held cards: walls it may bounce off, how
/// hard it curves toward monsters, how many it sparks on to, how much each
/// hit mends a friend. A monster's pattern shot carries its `path` here
/// too, and comes off a wall when it has a `bounce`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShotTraits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_origin: Option<abilities::Origin>,
    /// Beam receipts for the six knights' separate companions.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub squire_hits: u8,
    pub bounce: u8,
    pub homing: u8,
    pub chain: u8,
    pub mend: u8,
    /// A thrown blade: 1 flying out (`out` ticks left), 2 coming back to
    /// knight `owner`'s hand.
    #[serde(default)]
    pub back: u8,
    #[serde(default)]
    pub owner: u8,
    /// A companion's shot: credit its owner, but not their arm's cap or
    /// client-predicted fire/ghost handover.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub squire: bool,
    /// Owners whose companion this burning beam has already struck.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub squire_struck: Vec<u32>,
    #[serde(default)]
    pub out: u16,
    /// Bursts where it hits: this percent of its damage to all around.
    #[serde(default)]
    pub burst: u8,
    /// Monsters it has struck so far, for the share the next one takes
    /// (`aoe`).
    #[serde(default)]
    pub struck: u8,
    /// Stuns what it hits for this many ticks.
    #[serde(default)]
    pub stun: u8,
    /// A monster's pattern shot: what it does after it leaves
    /// (`patterns::Path`); empty for every other shot.
    #[serde(default)]
    pub path: patterns::Path,
    /// Its one trick is spent: a monster's shot has grazed a knight, a
    /// knight's rail has turned off a flying shield (`feel`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub once: bool,
    /// A fork's or split's child, not a shot the knight loosed: only these
    /// count against `KNIGHT_CHILDREN`, so a wide spread's own volley never
    /// leaves its forks without room.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forked: bool,
    /// How often the knight's arm was tempered when it left (`feel`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub temper: u8,
    /// Fire Arrows: it sets what it hits burning.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fire: bool,
    /// A plain arm's shot, rather than a spell or a forged bolt.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub arm: bool,
    /// A lobbed stone: gameplay identity is independent of cosmetic dressing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stone: bool,
    /// Foes this rail has passed through; each may be struck only once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pierced: Vec<u32>,
    /// Forks it has left: stopping on a monster, it splits in two (`fork`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fork: u8,
    /// Fart Bombs: what it hits gets a cloud (`skills::fart_hits`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub fart: u8,
    /// What a wish made literal (`wishcraft`): a bigger shot, a harder
    /// shove, a burning, chilling or poisoning hit, a shot that comes back,
    /// a faster one (percent).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub size: u8,
    /// The ticks between two pulls of the arm that loosed it, rate counted
    /// (0: no arm behind it, a spell): how big it is drawn follows it
    /// (`wishcraft::rate_pct`). Left off the wire when 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub beat: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub knock: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub burn: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub chill: u8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub poison: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub boomerang: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub velocity: u8,
    /// Part of a guardian's charge-up (`signatures::tag`): it strikes as
    /// that charge-up's blow, once a knight (`Hero::blow`).
    #[serde(default, skip_serializing_if = "zero")]
    pub blow: u32,
    /// A monster's shot loosed by a knight in its body (`bodies`): a lob
    /// comes down on the monsters, a beam burns them, and what turns after
    /// someone turns after them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub borrowed: bool,
    /// A monster's bullet turned back off a knight's blade (`melee::parry`):
    /// the knight's own now, and drawn as one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub turned: bool,
    /// Loosed by a spell from its knight's slot one less than this (0: no
    /// spell's): for the look alone, so a spell's shots and the breaks
    /// they make are told from the knight's own (`render`'s `magic`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spell: u8,
}

/// A forked shot's turn off its parent's line, each way (radians), and the
/// least it flies on for.
pub const FORK_TURN: f32 = 0.6;
pub const FORK_TTL: u32 = HZ;

fn zero(n: &u32) -> bool {
    *n == 0
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl ShotTraits {
    pub fn of(bonus: &cards::Bonus) -> ShotTraits {
        ShotTraits {
            bounce: bonus.bounce.min(8) as u8,
            homing: bonus.homing.min(8) as u8,
            chain: bonus.chain.min(8) as u8,
            mend: bonus.mend.min(16) as u8,
            burst: bonus.burst.min(100) as u8,
            fork: bonus.fork.min(3) as u8,
            fart: bonus.fart.min(3) as u8,
            size: bonus.size.min(3) as u8,
            knock: bonus.knock.min(3) as u8,
            burn: bonus.burn.min(3) as u8,
            chill: bonus.chill.min(3) as u8,
            poison: bonus.poison.min(3) as u8,
            boomerang: bonus.boomerang > 0,
            velocity: bonus.velocity.min(100) as u8,
            ..ShotTraits::default()
        }
    }
}

/// A chain hit's spark, from one monster to the next, for a few frames.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Spark {
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub ttl: u8,
}

// Both parents saved their different companion types under `squires`. Keep the
// public and serialized Run unchanged, but decode that one field as either
// parent shape before moving incoming companions to their new home. The macro
// keeps the deserialization mirror in lockstep with every other Run field.
#[derive(Deserialize)]
#[serde(untagged)]
enum SavedSquire {
    Head(squires::Companion),
    Countryside(countryside_squires::Companion),
}
enum SavedSquireField {
    Companions(Vec<SavedSquire>),
    Academy(squire_academy::Field),
    Guild(squire_guild::Field),
}
impl<'de> Deserialize<'de> for SavedSquireField {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_array() {
            serde_json::from_value(value).map(Self::Companions).map_err(serde::de::Error::custom)
        } else if value.get("menus").is_some() {
            serde_json::from_value(value).map(Self::Guild).map_err(serde::de::Error::custom)
        } else {
            serde_json::from_value(value).map(Self::Academy).map_err(serde::de::Error::custom)
        }
    }
}
impl Default for SavedSquireField {
    fn default() -> Self { Self::Companions(Vec::new()) }
}

macro_rules! define_run {
    (
        before { $( $(#[$am:meta])* $av:vis $a:ident : $at:ty, )* }
        middle { $( $(#[$bm:meta])* $bv:vis $b:ident : $bt:ty, )* }
        after { $( $(#[$cm:meta])* $cv:vis $c:ident : $ct:ty, )* }
    ) => {
        #[derive(Clone, Debug, Serialize)]
        pub struct Run {
            $( $(#[$am])* $av $a: $at, )*
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub squires: Vec<squires::Companion>,
            #[serde(default)]
            pub squire_academy: squire_academy::Field,
            #[serde(default, skip_serializing_if = "squire_guild::Field::is_empty")]
            pub squire_guild: squire_guild::Field,
            $( $(#[$bm])* $bv $b: $bt, )*
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub countryside_squires: Vec<countryside_squires::Companion>,
            $( $(#[$cm])* $cv $c: $ct, )*
        }

        #[derive(Deserialize)]
        struct SavedRun {
            $( $(#[$am])* $a: $at, )*
            #[serde(default)]
            squires: SavedSquireField,
            #[serde(default)]
            squire_academy: squire_academy::Field,
            #[serde(default)]
            squire_guild: squire_guild::Field,
            $( $(#[$bm])* $b: $bt, )*
            #[serde(default)]
            countryside_squires: Vec<countryside_squires::Companion>,
            $( $(#[$cm])* $c: $ct, )*
        }

        impl<'de> Deserialize<'de> for Run {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let SavedRun { $( $a, )* squires, mut squire_academy, mut squire_guild, $( $b, )* mut countryside_squires, $( $c, )* } =
                    SavedRun::deserialize(deserializer)?;
                let mut head_squires = Vec::new();
                let squires = match squires {
                    SavedSquireField::Companions(squires) => squires,
                    SavedSquireField::Guild(field) => { if squire_guild == Default::default() { squire_guild = field; } Vec::new() },
                    SavedSquireField::Academy(academy) => {
                        if squire_academy == Default::default() { squire_academy = academy; }
                        Vec::new()
                    }
                };
                for squire in squires {
                    match squire {
                        SavedSquire::Head(squire) => head_squires.push(squire),
                        SavedSquire::Countryside(squire) => countryside_squires.push(squire),
                    }
                }
                Ok(Self {
                    $( $a, )* squires: head_squires, squire_academy, squire_guild, $( $b, )* countryside_squires, $( $c, )*
                })
            }
        }
    };
}

define_run! {
    before {
    #[serde(default, skip_serializing_if = "runtime_state::omit", serialize_with = "runtime_state::serialize_visuals", deserialize_with = "runtime_state::transient")]
    pub module_visuals: crate::realm::effects::Scene,
    #[serde(default, skip_serializing_if = "runtime_state::omit", deserialize_with = "runtime_state::transient")]
    pub module_casts: Vec<abilities::Request>,
    #[serde(default)]
    pub playtest: votes::State,
    pub raid_id: u64,
    #[serde(default, skip_serializing_if = "team::State::quiet")]
    pub team: team::State,
    /// Seat incarnation fences delayed work, including scrolls not chosen yet.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub seat_epochs: BTreeMap<u32, u64>,
    /// Knights with nothing kept at home start in their smallclothes and
    /// find their gear (`gear`). The game sets it; bare runs start dressed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub naked_start: bool,
    /// Knights begin as shapes and wish themselves into being
    /// (`identity`). The game sets it (`identity::on`); bare runs keep the
    /// kit start.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub identity: bool,
    pub tick: u64,
    /// The magics this run invented mid-play: a wish granted while the party
    /// fought, kept by the name the wish gave it (`spellbook`). A knight's
    /// slot holds the name; this is what the name does. Left out of the wire
    /// when empty, and old saves load with none.
    #[serde(default, skip_serializing_if = "spellbook::Spellbook::is_empty")]
    pub spellbook: spellbook::Spellbook,
    /// Shared impact hold; transport ticks continue while room time stands.
    #[serde(default, skip_serializing_if = "hitstop::State::quiet")]
    pub hitstop: hitstop::State,
    pub phase: Phase,
    /// Cosmetic playback; never participates in travel or combat gates.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub floor_plates: BTreeMap<u32, floor_plates::Playing>,
    #[serde(default)]
    pub exits: exits::State,
    pub players: BTreeMap<u32, Hero>,
    pub enemies: Vec<Enemy>,
    /// An open floor's companies while the party is off its map, in a side
    /// room (`depths`): back where they were when it returns. The server's
    /// own (`wire::PRIVATE`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parked: Vec<Enemy>,
    pub projectiles: Vec<Projectile>,
    pub score: u32,
    pub dungeon: Floor,
    /// None preserves the current floor of a legacy save; new floors get a graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss_gates: Option<boss_gates::Gates>,
    /// Snapshot of the durable loop site admitted at the start of this Delve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_site: Option<String>,
    /// Public, generic stand geometry only. Private provenance lives in Site.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settlement_exhibits: Vec<exhibits::Marker>,
    /// Index of the room the party stands in.
    pub at: usize,
    #[serde(default)]
    pub kingdom: kingdom::State,
    /// The knobs a developer moved (`tune`); written only once one has.
    #[serde(default, skip_serializing_if = "tune::Tune::is_default")]
    pub tune: tune::Tune,
    /// Spoils banked by this run and not yet handed to the realm.
    #[serde(default)]
    pub bank: Vec<Haul>,
    /// Delve packs whose floor this run cleared, not yet handed to the realm.
    #[serde(default)]
    pub reclaimed: Vec<Pack>,
    /// The Undercroft as the realm has built it, and the treasury it can
    /// spend: the cockpit's word, refreshed whenever either changes.
    #[serde(default)]
    pub home: home::Home,
    #[serde(default)]
    pub treasury: Spoils,
    /// Travelling peddler approach latch; never an estate purchase order.
    #[serde(default)]
    pub merchant: merchant::State,
    /// The Sozzled Griffin and its bazaar: purses, tries, and what each
    /// knight was last told (`alehouse`).
    #[serde(default, skip_serializing_if = "alehouse::House::is_default")]
    pub alehouse: alehouse::House,
    /// Purchases asked for at the Undercroft's plates, for the cockpit.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub orders: Vec<home::Order>,
    /// Purchases at the surface peddler, for the cockpit to pay from the
    /// realm's treasury.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub peddled: Vec<merchant::Purchase>,
    /// Which keepers had a knight beside them last tick (one bit each).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    greeted: u8,
    /// How this delve goes, as Fortune's wheel decided it.
    #[serde(default)]
    pub mode: fortune::Mode,
    /// Every floor of this delve is laid open (`depths`), whatever the
    /// wheel says: the host's switch (`settings::Settings::open_floors`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open_floors: bool,
    /// The wheel's spin this delve, if it was pulled.
    #[serde(default)]
    pub spin: Option<fortune::Spin>,
    /// The spin whose result has been announced.
    #[serde(default)]
    mode_said: Option<u64>,
    /// The Collapse: the tick this floor falls in.
    #[serde(default)]
    pub collapse_at: Option<u64>,
    /// The tick the party came down into the deep ([`DEEP_FLOOR`] and
    /// below) from above it, for the beat a window plays as the dungeon
    /// stands up around it (`world3d::room::beat`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_beat: Option<u64>,
    /// Ultimates playing out: Trebuchet's shells, Stillhours, phantoms.
    #[serde(default)]
    pub strikes: Vec<ults::Strike>,
    #[serde(default)]
    pub spheres: Vec<ults::Sphere>,
    #[serde(default)]
    pub phantoms: Vec<ults::Phantom>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bard_songs: Vec<bard::Song>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scarecrows: Vec<peasant::Scarecrow>,
    /// The champions' hammers, hooks and flasks in the air, their rings,
    /// cracks and grips (`champions`).
    #[serde(default, skip_serializing_if = "champions::Field::is_empty")]
    pub champions: champions::Field,
    #[serde(default, skip_serializing_if = "heroes::Field::is_empty")]
    pub heroes: heroes::Field,
    /// Creatures cards summoned (`imps`).
    #[serde(default)]
    pub imps: Vec<imps::Imp>,
    /// The floor's cauldron, risen when its guardian fell (`cauldron`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cauldron: Option<cauldron::Cauldron>,
    /// Brews waiting on the host's model for a spell of their own, oldest
    /// first; the cockpit takes them (`cauldron::Brewing`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub brewing: Vec<cauldron::Brewing>,
    /// The pack of the delve's first floor, for the floors below it.
    #[serde(default)]
    pub first: Pack,
    /// The light home, once the dragon is slain: step into it to go home
    /// with everything.
    #[serde(default)]
    pub light: Option<(f32, f32)>,
    /// A great end reached this tick, for the realm: a dragon, or the Grail.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub triumph: Option<Triumph>,
    /// Achievements the game noticed this tick, for the realm to keep.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::symbols"
    )]
    pub feats: Vec<&'static str>,
    /// A new achievement's banner: when, and which.
    #[serde(default)]
    pub banner: Option<(u64, String)>,
    /// Achievements earned down in this delve, in the order earned: kept
    /// quiet through the fighting and shown together when the match ends.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deeds: Vec<String>,
    /// A box just opened at the coffer: when, its grade, what was in it.
    #[serde(default)]
    pub unboxed: Option<(u64, feats::Tier, String)>,
    /// Slimes slain in this room, for a family pruned.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    slimes_slain: u32,
    /// Grubbins' wares this delve (an empty id: sold).
    #[serde(default)]
    pub stall: Vec<String>,
    /// Ticks a knight has stood on the Winding Stair at home.
    #[serde(default)]
    pub descending: u32,
    /// The entrance that hold is for: stepping onto a neighbouring plate
    /// starts its own count.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::symbol_option"
    )]
    descend_for: Option<runtime_state::Symbol>,
    /// Whoever waits in a cage on this floor.
    #[serde(default)]
    pub captive: Option<rescues::Captive>,
    /// Which of the people at home a knight is standing by (each speaks
    /// once per approach).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    met: u8,
    /// This floor's folly room, and the folly when one is being played.
    #[serde(default)]
    pub folly_room: Option<usize>,
    #[serde(default)]
    pub folly: Option<folly::WaxMaze>,
    #[serde(default)]
    pub bats: Option<folly::BatShoot>,
    /// This floor's Keg Gantry hall, and the climb when one is on.
    #[serde(default)]
    pub gantry_room: Option<usize>,
    #[serde(default)]
    pub gantry: Option<gantry::Gantry>,
    #[serde(default)]
    pub powder: Option<powder::Powder>,
    /// Book Hop, while a hall plays it.
    #[serde(default)]
    pub hop: Option<hop::Hop>,
    /// This floor's race hall, and the race when one is on (the Shaft Run,
    /// the Ore Cart or the Griffin Run).
    #[serde(default)]
    pub race3d_room: Option<usize>,
    #[serde(default)]
    pub race3d: Option<race3d::Race3d>,
    /// The pots broken on this floor, by room (a bit per pot), and the
    /// shards of the last few lying about.
    #[serde(default)]
    pub pots_broken: BTreeMap<usize, u8>,
    /// The bays of a held floor's steward's hall used on this floor (a bit
    /// per bay): each gives its boon once a floor.
    #[serde(default)]
    pub holding_used: u16,
    /// The raid being fought in a holding's hall, if one is.
    #[serde(default)]
    pub holding_raid: Option<raids::RaidFight>,
    /// The lamp-oil thief on the Strand, while the coast errand wants it.
    #[serde(default)]
    pub crab: Option<coastal::Crab>,
    /// The badger cook's mushrooms, while the woods errand wants them.
    #[serde(default)]
    pub forage: Option<woodland::Forage>,
    /// The sluice valves' open ticks left, while the wetland errand wants them.
    #[serde(default)]
    pub valves: Option<[u32; 3]>,
    /// The pigeons with the Parliament's papers, while the minutes are out.
    #[serde(default)]
    pub pigeons: Option<aerie::Pigeons>,
    /// The goose and the picnic sail, while the wind-country job wants it.
    #[serde(default)]
    pub heist: Option<windcountry::Heist>,
    /// The town's stalls, tubs and anvil (`town`).
    #[serde(default)]
    pub town: town::Town,
    /// The floor's nook below ground and what is under way in it (`nooks`).
    #[serde(default)]
    pub nook: Option<nooks::Nook>,
    /// The Deep, while a delve has gone down below the Unknown (`deep`):
    /// its own floor count, a village's state, the errands' signs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep: Option<deep::Descent>,
    /// Broken pots' shards settling. Saves leave them out; the wire's tree
    /// carries them beside the run, for drawing (`wire::publish`).
    #[serde(default, skip_serializing_if = "crate::rules::runtime_state::omit")]
    pub pot_shards: Vec<(f32, f32, u32)>,
    /// The finishers of monsters just felled (`gore`): how each body is
    /// shown. Saves leave them out; the wire's tree carries them beside the
    /// run, for drawing (`wire::publish`).
    #[serde(default, skip_serializing_if = "crate::rules::runtime_state::omit")]
    pub gore: Vec<gore::Gore>,
    /// Lady Tallow, Blaise's cat, when she has come along.
    #[serde(default)]
    pub cat: Option<cat::Cat>,
    /// Fortune's dare on this floor, and how many the party has kept this
    /// delve.
    #[serde(default)]
    pub dare: Option<dares::Dare>,
    #[serde(default)]
    pub dares_kept: u32,
    /// A power rune in this room (welling up a few seconds into a fight),
    /// and how many the party has taken this delve.
    #[serde(default)]
    pub rune: Option<runes::Rune>,
    #[serde(default)]
    pub runes_taken: u32,
    /// Knights hexed into frogs this delve.
    #[serde(default)]
    pub hexes: u32,
    /// Maud's round, drunk at the top of the stair for this delve.
    #[serde(default)]
    pub round: Option<String>,
    /// Sir Dinadan's song for this delve.
    #[serde(default)]
    pub song: Option<String>,
    /// Kills and moments since the cockpit last looked, for Wren's
    /// bounties. Saves leave them out (the cockpit journals the intents it
    /// keeps); the renderer reads them, so the wire's tree carries them
    /// beside the run (`wire::publish`).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub marks: BTreeMap<String, u32>,
    /// The small secrets already lifted (`cup:{depth}`), and those
    /// the Herald has already hinted at: each happens once, saves included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifted: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hinted: Vec<String>,
    /// Sappers' kegs burning down, and hobs' bombs in the air.
    #[serde(default)]
    pub kegs: Vec<foes::Keg>,
    #[serde(default)]
    pub lobs: Vec<foes::Lob>,
    /// The Broodmarrows' children in flight.
    #[serde(default)]
    pub flings: Vec<foes::Fling>,
    /// The Whirlgraves' open wells.
    #[serde(default)]
    pub wells: Vec<foes::Well>,
    /// The Flesher's hooks in the air, and the Silkmother's webs.
    #[serde(default)]
    pub hooks: Vec<hunters::Hook>,
    #[serde(default)]
    pub webs: Vec<hunters::Web>,
    /// The Pit Tyrant's slams, gathering.
    #[serde(default)]
    pub slams: Vec<pit::Slam>,
    /// A guardian's Ravages, rippling and bursting.
    #[serde(default)]
    pub ravages: Vec<tide::Ravage>,
    /// Beaumains, hired for this delve.
    #[serde(default)]
    pub hireling: Option<hireling::Hireling>,
    }
    middle {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lane_squires: Vec<lane_squires::Squire>,
    #[serde(skip)]
    pub lane_squire_orders: Vec<lane_squires::Order>,
    /// Raisings ordered in the kingdom, waiting on the next home claim.
    #[serde(skip)]
    pub raise_orders: Vec<raisings::Order>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    lane_squire_holds: BTreeMap<u32, (lane_squires::Action, u32)>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    squire_yard_holds: BTreeMap<u32, squires::Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squire_mark: Option<squires::Mark>,
    #[serde(default, skip_serializing_if = "squire_company::Field::is_empty")]
    pub squire_field: squire_company::Field,
    /// Host settlement reserves only company edits, never simulation progress.
    #[serde(default, skip_serializing_if = "runtime_state::omit", deserialize_with = "runtime_state::transient")]
    pub squire_company_reserved: bool,
    /// A journal claim owns the company until its identified delivery commits.
    #[serde(default, skip_serializing_if = "runtime_state::omit", deserialize_with = "runtime_state::transient")]
    pub squire_company_claimed: bool,
    /// Descents during a realm save are applied to the live company afterwards.
    #[serde(default, skip_serializing_if = "runtime_state::omit", deserialize_with = "runtime_state::transient")]
    pub squire_pending_delves: u32,
    #[serde(default, skip_serializing_if = "squire_hall::Party::is_empty")]
    pub squire_hall: squire_hall::Party,
    }
    after {
    #[serde(skip)]
    pub(crate) countryside_squire_holds: BTreeMap<u32, (usize, u32)>,
    /// A summoner's own dead, their orders and marks, and the graves
    /// (`minions`). Left out while there are none, so a save is as it was.
    #[serde(default, skip_serializing_if = "minions::Pack::is_empty")]
    pub minions: minions::Pack,
    /// A bout at the lists, while one is ridden.
    #[serde(default)]
    pub joust: Option<joust::Joust>,
    /// A chariot race at the Hippodrome, while one is run.
    #[serde(default)]
    pub chariot: Option<chariot::Chariot>,
    /// The clearing's shared games and personal-best ledger.
    #[serde(default)]
    pub forest: forest::Forest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forest_road: Option<forest_road::Session>,
    /// The Round Table adventures, while one is run: each a session in the
    /// Dark Forest's room, opened at its kingdom tile (`roundtable`).
    /// Lanternward (rules/lanternward.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lanternward: Option<lanternward::Session>,
    /// Moonfall (rules/moonfall.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moonfall: Option<moonfall::Session>,
    /// Bannerpeak (rules/bannerpeak.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bannerpeak: Option<bannerpeak::Session>,
    /// Griffinwing (rules/griffinwing.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub griffinwing: Option<griffinwing::Session>,
    /// Lightgallop (rules/lightgallop.rs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lightgallop: Option<lightgallop::Session>,
    /// One-lap Knight Racer, shared by all seats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knight_racers: Option<knight_racers::Session>,
    /// Knights Wish, a short first-person free-for-all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deathmatch: Option<deathmatch::Session>,
    #[serde(default)]
    pub college: college::Dialogue,
    /// The teachers' questlines: whose reading is open, and this tick's
    /// queued counter notes.
    #[serde(default)]
    pub questline: questline::Board,
    /// The campaign the kingdom's heart questline mirrors: real work counted
    /// outside the realm (`campaign`). The host sets it from the campaign's
    /// board, off the sim thread; nothing in the game raises it. Saves leave
    /// it out; the wire's tree carries it beside the run, for the sheet and
    /// the hall's lanterns (`wire::publish`).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub campaign: campaign::Campaign,
    /// Shared, model-free festival round; records live in the realm's home.
    #[serde(default)]
    pub pastimes: pastimes::State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossroads: Option<crossroads::State>,
    #[serde(default)]
    pub cinder: Option<cinder::Session>,
    #[serde(default)]
    pub latch: Option<latch::Session>,
    #[serde(default)]
    pub brasswake: Option<brasswake::Session>,
    #[serde(default)]
    pub rafterwake: Option<rafterwake::Session>,
    #[serde(default)]
    pub pennon: Option<pennon::Session>,
    /// Host-only input age allowance from the existing network measurement.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub latch_input_age: BTreeMap<u32, u64>,
    #[serde(default)]
    pub mossbell: Option<mossbell::Session>,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    build_hold: Option<(usize, u32, u32)>,
    /// Which knights (a bit each) held E at the signal hill last tick: a
    /// lever answers a press, not a hold.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    lever_held: u8,
    /// The same for the new rooms (longbow country and the town).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    rooms_held: u8,
    /// A knight stood by the groom last tick (he speaks once per approach).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    groom_near: bool,
    /// A knight stood by King Brannoc last tick.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    king_near: bool,
    /// The Hollow Ones' Black Holes, warning and pulling.
    #[serde(default)]
    pub holes: Vec<hollow::Hole>,
    /// The third company's globs, puddles and flashes.
    #[serde(default)]
    pub critters: critters::Field,
    /// The party's pit-stop times at the Sanctuary's privy.
    #[serde(default)]
    pub pits: pitstop::Board,
    /// Frogs about the room, and the figures they became in a Sanctuary.
    #[serde(default)]
    pub frogs: frogs::Field,
    /// Knights on each other's shoulders, and knights thrown off the top
    /// (`shoulders`).
    #[serde(default, skip_serializing_if = "runtime_state::shoulders_quiet")]
    pub shoulders: shoulders::Shoulders,
    /// The floors' guardians; its own copy, so a save replays.
    #[serde(default = "bosses::builtin")]
    pub bosses: Vec<Boss>,
    /// The cards this run can find; its own copy, so a save replays.
    #[serde(default)]
    pub book: Book,
    /// The last card taken or played, by whom and when, for the notice line.
    #[serde(default)]
    pub found: Option<(u64, u32, String)>,
    /// Moments the chorus may speak about, drained by the cockpit.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub cues: Vec<String>,
    /// Sounds this step made, drained by the cockpit's mixer.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::symbols"
    )]
    pub sounds: Vec<&'static str>,
    /// Sounds this step made for one knight's ears alone (the fellowship's
    /// bell), drained with the rest (`wire::take_sounds`).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::seat_symbols"
    )]
    pub seat_sounds: Vec<(u32, &'static str)>,
    /// The tick each sound kept at a distance last sounded (`sound_every`).
    #[serde(skip)]
    pub sounded_at: BTreeMap<&'static str, u64>,
    /// Hostile shots loosed since the run began, bursts included: what the
    /// boss census counts.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub hostile_shots: u64,
    /// Ticks of screen shake left (a guardian's fall, a bomb).
    #[serde(default)]
    pub shake: u32,
    /// The last kill's tick and the kills since, for slay streaks.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    streak: (u64, u32),
    /// A knight was hurt in this room (no flawless clear).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    room_hurt: bool,
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    blooded: bool,
    /// Waves still to pour through this room's doorways.
    #[serde(default)]
    pub waves: Waves,
    /// This room's traps, and the rocks falling in it.
    #[serde(default)]
    pub traps: Vec<Trap>,
    #[serde(default)]
    pub rocks: Vec<Rock>,
    /// Spikes have bitten a knight in this room (said once).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    spiked: bool,
    /// The skills' gas, fire, mines and barricades, the rooms' own perils,
    /// the Boss Key and the Wanted hall's showdown.
    #[serde(default)]
    pub works: works::Works,
    /// Wishes just given (`overclass`), for their sign, until the next
    /// fight. The wishes themselves are each knight's (`Hero::wishes`).
    #[serde(default)]
    pub window: Option<overclass::Window>,
    /// The wishes knights made in their own words this run, by wish id:
    /// who made it (knight, name). The card's `by` line and the sheet's
    /// YOUR WISHES are theirs (`overclass`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub makers: BTreeMap<String, (u32, String)>,
    /// Each learned wish a knight here was granted, by wish id: its name
    /// and what each tier gives, as the scroll says. A seat's copy of the
    /// phrasebook has only the delve's own wishes, so its wish line reads a
    /// learned one's next tier here (`genie::Upgrade`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ladders: BTreeMap<String, (String, Vec<String>)>,
    /// Lines everyone is shown, drained by the server (`wire::take_notices`).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub notices: Vec<String>,
    /// Lines one knight is shown on their own screen, drained with the rest
    /// (`Run::seat_notices` → `Hear::Knight`).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub seat_notices: Vec<(u32, String)>,
    /// The demon hand's claims, the hell gate and its circles.
    #[serde(default)]
    pub spectacle: spectacle::Spectacle,
    /// The knights' grunts and few words, and when they last spoke.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub chatter: chatter::Chatter,
    /// Chain hits' sparks, fading.
    #[serde(default)]
    pub sparks: Vec<Spark>,
    /// What this tick showed, for `present` to draw (`fx`).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    pub fx: Vec<Fx>,
    /// Handgonne bursts: where, and ticks left.
    #[serde(default)]
    pub blasts: Vec<(f32, f32, u8)>,
    /// How the arms feel: sparks, stuck shafts, smoke, deaths, a room's
    /// end in coins (`feel`).
    #[serde(default, skip_serializing_if = "runtime_state::feel_quiet")]
    pub feel: feel::Feel,
    /// The floor as it is played, knight by knight (`floor_stats`). The
    /// server's own (`wire::PRIVATE`).
    #[serde(default)]
    pub floor_tally: floor_stats::Tally,
    /// The last floor closed: its results, as the plate shows them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_results: Option<floor_stats::Results>,
    /// Wishes just granted: to which knight, and when.
    #[serde(default)]
    pub boons: Vec<(u32, u64)>,
    /// A wish is being forged at the Sanctuary's dais (set by the cockpit
    /// while angelX writes it): its ring burns brighter.
    #[serde(default)]
    pub wishing: bool,
    /// The wish altar's lamp: who rubbed it, the wishes the host's grant
    /// runners are writing, the grants landed (`altar`).
    #[serde(default, skip_serializing_if = "runtime_state::altar_quiet")]
    pub lamp: altar::Lamp,
    /// The wish scroll's genie: the party's freedom gauge, the last wish
    /// sealed, and whether he is free (`genie`).
    #[serde(default, skip_serializing_if = "genie::Genie::quiet")]
    pub genie: genie::Genie,
    /// Learned words a knight spent a wish on from the scroll, for the
    /// host's angelX to teach (`genie`). Kept until the grant lands or the
    /// wish is given back, and saved, so a host restart can still teach it.
    /// A client is not sent it (`wire::PRIVATE`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asked: Vec<genie::Asked>,
    /// The freedom wishes' side of the run: the wish just landed in a
    /// place, and what the safe room has given on this floor (`freedom`).
    #[serde(default, skip_serializing_if = "freedom::State::omit")]
    pub freedom: freedom::State,
    /// The floor's shooting stars: how many have fallen, the next one due,
    /// the one crossing or lying fallen, and the star wishes lately
    /// granted (`starfall`).
    #[serde(default, skip_serializing_if = "starfall::Starfall::omit")]
    pub starfall: starfall::Starfall,
    /// The moments a knight gets stronger, playing (`rewards`).
    #[serde(default, skip_serializing_if = "rewards::Rewards::quiet")]
    pub rewards: rewards::Rewards,
    /// What each knight earned this delve, for the road ahead at its end
    /// (`rewards`). The server's own (`wire::PRIVATE`).
    #[serde(default, skip_serializing_if = "rewards::Ledger::quiet")]
    pub ledger: rewards::Ledger,
    /// The knight a fairy wish is being granted to, while it is
    /// (`nooks::gnome`): their grant is drawn Legendary. Never saved.
    #[serde(skip)]
    pub fairy_grant: Option<u32>,
    /// The spell key each knight held on the scroll, so a pick held down
    /// casts nothing until it is let go (`Run::offer_inputs`).
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    offer_keys: BTreeMap<u32, u8>,
    /// The tick a Sanctuary opened around the party, for its sign.
    #[serde(default)]
    pub hallowed: Option<u64>,
    /// The stairs wait for a knight to step off them first: a guardian
    /// falling at your feet never drops you past the Sanctuary.
    #[serde(default)]
    stairs_held: bool,
    /// The room holds while the party arrives (`Run::hold`): its foes and
    /// their shots wait and no knight is hurt, until every player here has
    /// the room on screen, or nobody is here at all. The host's server sets
    /// it each tick; the rules alone never do.
    #[serde(default)]
    pub held: Hold,
    /// Knights standing in a doorway or on the stairs the rest of the
    /// party has yet to reach (`fellowship`): the way waits for everyone.
    #[serde(default, skip_serializing_if = "fellowship::State::is_empty")]
    pub fellowship: fellowship::State,
    /// How many knights the monsters' health was last scaled for
    /// (`Run::rescale_party`); 0 for every knight in the party, which is
    /// how a run is written while all of them are here.
    #[serde(default, skip_serializing_if = "zero")]
    scaled_for: u32,
    /// Once the run has fallen or is won: who has asked to go back to the
    /// kingdom, and when the party goes (`again::Again`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub again: Option<again::Again>,
    /// Each knight's ticks since it last pressed or held anything (`step`):
    /// the server's own, never saved nor sent.
    #[serde(
        default,
        skip_serializing_if = "crate::rules::runtime_state::omit",
        deserialize_with = "crate::rules::runtime_state::transient"
    )]
    idle: BTreeMap<u32, u32>,
    /// The server's own: never sent to a client (`wire::PRIVATE`), so a
    /// client's run reads them back as their defaults. A client never steps
    /// the rules.
    #[serde(default)]
    next_id: u32,
    #[serde(default = "unseeded")]
    rng: Rng,
    /// Sent: the renderer places the merchant's camp and the pots by it.
    #[serde(default)]
    pub seed: u64,
    }
}

/// The dice of a run read without them: a client's copy (`wire::PRIVATE`).
fn unseeded() -> Rng {
    Rng::new(0)
}

/// A great end a delve reached, for the realm's tally.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Triumph {
    /// The dragon slain.
    Dragon,
    /// The Unknown met, and the Grail found.
    Grail,
}

pub fn unit(x: f32, y: f32) -> (f32, f32) {
    let length = x.hypot(y);
    if length > 0.0 {
        (x / length, y / length)
    } else {
        (0.0, 0.0)
    }
}

/// Whether segment `p`–`q` crosses segment `a`–`b`.
fn crosses(p: (f32, f32), q: (f32, f32), a: (f32, f32), b: (f32, f32)) -> bool {
    let side = |o: (f32, f32), u: (f32, f32), v: (f32, f32)| {
        (u.0 - o.0) * (v.1 - o.1) - (u.1 - o.1) * (v.0 - o.0)
    };
    let (d1, d2) = (side(a, b, p), side(a, b, q));
    let (d3, d4) = (side(p, q, a), side(p, q, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn hit_segment(ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32, radius: f32) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((x - ax) * dx + (y - ay) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (x - ax - t * dx).hypot(y - ay - t * dy) <= radius
}

/// A floor of the Delve for another view to walk (the loop's crawl in the
/// realm's 3D pane): floor `depth` of `pack`, laid out from `seed`.
pub fn crawl_floor(depth: u32, pack: Pack, seed: u64) -> Floor {
    layout::floor(depth, pack, &mut Rng::new(seed))
}

pub fn mix(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^ (h >> 33)
}

/// Who is moving, for what stops them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mover {
    Hero,
    Walker,
    Flier,
    /// Walks as a walker does, but the bog does not hold it: the swamp's
    /// own horrors (`horrors`).
    Wader,
}

/// The current room's tiles as movement and shots see them.
pub struct Grid<'a> {
    room: &'a Room,
    barred: bool,
}

impl Grid<'_> {
    fn tile_at(&self, x: f32, y: f32) -> Tile {
        // Off the edge reads as the edge tile, so an open doorway lets a
        // knight step through it and out of the room.
        let col = ((x / TILE_UNITS).floor() as i32).clamp(0, self.room.cols as i32 - 1);
        let row = ((y / TILE_UNITS).floor() as i32).clamp(0, self.room.rows as i32 - 1);
        self.room.tile(col, row)
    }

    fn open(&self, x: f32, y: f32, mover: Mover) -> bool {
        match self.tile_at(x, y) {
            Tile::Floor | Tile::Stairs | Tile::Ledge | Tile::Bog => true,
            Tile::Wall | Tile::Block => false,
            Tile::Hazard => mover == Mover::Flier,
            Tile::Door => mover == Mover::Hero && !self.barred,
        }
    }

    fn clear(&self, x: f32, y: f32, r: f32, mover: Mover) -> bool {
        [(-r, -r), (r, -r), (-r, r), (r, r)]
            .into_iter()
            .all(|(dx, dy)| self.open(x + dx, y + dy, mover))
    }

    /// Move with wall sliding, one axis at a time. Feet in the bog go at
    /// `swamp::BOG_PACE`; wings and the swamp's own waders do not.
    fn slide(&self, (x, y): (f32, f32), (dx, dy): (f32, f32), r: f32, mover: Mover) -> (f32, f32) {
        let (dx, dy) = if matches!(mover, Mover::Hero | Mover::Walker)
            && self.tile_at(x, y) == Tile::Bog
        {
            (dx * swamp::BOG_PACE, dy * swamp::BOG_PACE)
        } else {
            (dx, dy)
        };
        // A knight already in stone (shoved into blocks, or caught in a
        // doorway when the room bars it) walks out toward the room's middle:
        // nothing holds him in place, and nothing lets him out of the room.
        // 2026-10-24: rgb stood wedged in the blocks by the way in, unable
        // to move or land a shot, until the host moved him by hand.
        if mover == Mover::Hero && !self.clear(x, y, r, mover) {
            let (cx, cy) = (self.room.cols as f32 * TILE_UNITS / 2.0, self.room.rows as f32 * TILE_UNITS / 2.0);
            let x = if dx * (cx - x) > 0.0 { x + dx } else { x };
            let y = if dy * (cy - y) > 0.0 { y + dy } else { y };
            return (x, y);
        }
        let x = if self.clear(x + dx, y, r, mover) {
            x + dx
        } else {
            x
        };
        let y = if self.clear(x, y + dy, r, mover) {
            y + dy
        } else {
            y
        };
        (x, y)
    }

    /// A knight's displacement, stopped at walls and barred doors the way
    /// walking is. A clear path lands where one `slide` of the whole
    /// delta would. A destination that is itself stone keeps that one
    /// slide (no creep). A destination that is clear only because a door
    /// or a wall sits between here and there stops at the obstacle
    /// instead of jumping it.
    pub fn halted(&self, from: (f32, f32), (dx, dy): (f32, f32)) -> (f32, f32) {
        const STEP: f32 = 0.4;
        let dist = dx.hypot(dy);
        if dist < 1e-6 {
            return from;
        }
        if !self.clear(from.0 + dx, from.1 + dy, HERO_RADIUS, Mover::Hero) {
            return self.slide(from, (dx, dy), HERO_RADIUS, Mover::Hero);
        }
        let (ux, uy) = (dx / dist, dy / dist);
        let mut at = from;
        let mut gone = 0.0;
        while gone < dist - 1e-4 {
            let hop = STEP.min(dist - gone);
            let next = self.slide(at, (ux * hop, uy * hop), HERO_RADIUS, Mover::Hero);
            if (next.0 - at.0).abs() < 1e-5 && (next.1 - at.1).abs() < 1e-5 {
                return at;
            }
            at = next;
            gone += hop;
        }
        at
    }

    /// Where a placed knight lands. A rider sits on its carrier and a
    /// risen knight takes the body's place: walls and an open doorway
    /// do not stop them, and they land exactly. A barred door on the
    /// straight way stops them short of it, as a step would stop the
    /// carrier or the body.
    pub(super) fn across_bar(&self, from: (f32, f32), to: (f32, f32)) -> (f32, f32) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let dist = dx.hypot(dy);
        if !self.barred || dist < 1e-6 {
            return to;
        }
        const STEP: f32 = 0.2;
        let (ux, uy) = (dx / dist, dy / dist);
        let mut gone = 0.0;
        let mut hits = false;
        while gone < dist - 1e-4 {
            gone += STEP.min(dist - gone);
            if self.tile_at(from.0 + ux * gone, from.1 + uy * gone) == Tile::Door {
                hits = true;
                break;
            }
        }
        if !hits {
            return to;
        }
        let mut at = from;
        gone = 0.0;
        while gone < dist - 1e-4 {
            let hop = STEP.min(dist - gone);
            let next = (at.0 + ux * hop, at.1 + uy * hop);
            let r = HERO_RADIUS;
            let in_door = [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)]
                .into_iter()
                .any(|(ox, oy)| self.tile_at(next.0 + ox, next.1 + oy) == Tile::Door);
            if in_door {
                return at;
            }
            at = next;
            gone += hop;
        }
        at
    }

    fn stops_shot(&self, x: f32, y: f32) -> bool {
        if !(0.0..self.room.width()).contains(&x) || !(0.0..self.room.height()).contains(&y) {
            return true;
        }
        match self.tile_at(x, y) {
            Tile::Wall | Tile::Block => true,
            Tile::Door => self.barred,
            _ => false,
        }
    }

    /// A fast friendly shot stops at the first stone it crosses, including
    /// a thin wall between two open endpoints.
    pub fn shot_stop(&self, from: (f32, f32), to: (f32, f32)) -> Option<(f32, f32)> {
        self.shot_stop_body(from, to, 0.0)
    }

    /// A shot whose body is `body` units round its middle: stopped by stone
    /// at its middle, and by a prop (a tomb, a pillar, a shelf, a crate,
    /// furniture: anything a knight cannot walk into that is not the
    /// room's own wall or door) wherever its body touches one. What a
    /// knight walks over (water, lava, a pit, a plate) never stops it.
    fn stops_body(&self, x: f32, y: f32, body: f32) -> bool {
        self.stops_shot(x, y)
            || (body > 0.0
                && [(body, 0.0), (-body, 0.0), (0.0, body), (0.0, -body)]
                    .into_iter()
                    .any(|(dx, dy)| self.tile_at(x + dx, y + dy) == Tile::Block))
    }

    /// `shot_stop` for a shot with a body (`stops_body`): where it first
    /// touches stone or a prop on its way from `from` to `to`.
    pub fn shot_stop_body(&self, from: (f32, f32), to: (f32, f32), body: f32) -> Option<(f32, f32)> {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let steps = (dx.hypot(dy) / 0.25).ceil().clamp(1.0, 256.0) as u32;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let at = (from.0 + dx * t, from.1 + dy * t);
            if self.stops_body(at.0, at.1, body) {
                return Some(at);
            }
        }
        None
    }
}

impl Run {
    #[cfg(any(test, feature = "testing"))]
    pub fn settlement_walkable_for_test(&self, x: f32, y: f32) -> bool {
        Grid {
            room: self.room(),
            barred: self.barred(),
        }
        .clear(x, y, HERO_RADIUS, Mover::Hero)
    }

    pub fn valid_snapshot(&self) -> bool {
        self.valid_snapshot_with_id_limit(self.next_id)
    }

    /// Read mirrors omit the private allocator. Validate the same public
    /// invariants without treating a default allocator as authoritative.
    pub(crate) fn valid_read_view(&self) -> bool {
        self.valid_snapshot_with_id_limit(u32::MAX)
    }

    fn valid_snapshot_with_id_limit(&self, allocated_id: u32) -> bool {
        (0..=DEEPEST).contains(&self.dungeon.depth)
            && self.seat_epochs.keys().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.kingdom.valid()
            && self.tune.valid()
            && self.hitstop.valid()
            && self.team.valid()
            && self.spectacle.phase.is_none_or(|p| p.x.is_finite() && p.y.is_finite()
                && p.room < self.dungeon.rooms.len() && p.born <= self.tick)
            && self.exits.valid(self.dungeon.rooms.len())
            && self
                .exits
                .outside
                .keys()
                .all(|id| self.players.contains_key(id))
            && self
                .players
                .values()
                .all(|h| h.camera.is_none_or(|(x, y)| x.is_finite() && y.is_finite()))
            && self.players.values().all(|h| h.dead_wishes.len() <= 64)
            && self.players.values().all(|h| h.parrying.is_none_or(|g| g.valid()))
            && self.players.values().all(|h| h.squire.as_ref().is_none_or(squires_lodge::Companion::valid))
            && self.forest.valid()
            && self.forest_road.as_ref().is_none_or(|s| s.valid())
            && roundtable::valid(self)
            && self.college.valid()
            && self.home.college.valid()
            && self.pastimes.valid()
            && self.crossroads.as_ref().is_none_or(crossroads::State::valid)
            && self.cinder.as_ref().is_none_or(|s| s.valid())
            && self.latch.as_ref().is_none_or(|s| s.valid())
            && self.brasswake.as_ref().is_none_or(|s| s.valid())
            && self.rafterwake.as_ref().is_none_or(|s| s.valid())
            && self.pennon.as_ref().is_none_or(|s| s.valid())
            && self.mossbell.as_ref().is_none_or(|s| s.valid())
            && self.home.countryside.valid()
            && self.home.privy_wall.len() <= pitstop::WALL
            && self.home.privy_wall.iter().all(|l| l.len() <= 64)
            && self.home.wished.valid()
            && self.freedom.valid()
            && self.starfall.valid(self.dungeon.rooms.len())
            && self.rewards.valid()
            && self.ledger.valid()
            && self.home.sky.valid()
            && self.players.values().all(|h| h.freedom_wishes <= freedom::MOST_HELD)
            && self.home.ranch.valid()
            && self.folly.as_ref().is_none_or(|f| f.valid() && f.room < self.dungeon.rooms.len())
            && self.bats.as_ref().is_none_or(|b| b.valid() && b.room < self.dungeon.rooms.len())
            && self.gantry.as_ref().is_none_or(|g| g.valid() && g.room < self.dungeon.rooms.len())
            && self.gantry_room.is_none_or(|i| i < self.dungeon.rooms.len())
            && self.powder.as_ref().is_none_or(|p| p.valid() && p.room < self.dungeon.rooms.len())
            && self.hop.as_ref().is_none_or(|h| h.valid() && h.room < self.dungeon.rooms.len())
            && self.race3d.as_ref().is_none_or(|r| r.valid() && r.room < self.dungeon.rooms.len())
            && self.race3d_room.is_none_or(|i| i < self.dungeon.rooms.len())
            && self.pots_broken.len() <= 64
            && self.pots_broken.keys().all(|&i| i < self.dungeon.rooms.len())
            && self.holding_used < 1 << holdings::SLOTS
            && self.holding_raid.as_ref().is_none_or(|f| f.valid(self.dungeon.rooms.len()))
            && self.home.holdings.valid()
            && self.crab.as_ref().is_none_or(coastal::Crab::valid)
            && self.forage.as_ref().is_none_or(woodland::Forage::valid)
            && self.valves.is_none_or(|v| v.iter().all(|&t| t <= wetland::VALVE_OPEN))
            && self.pigeons.as_ref().is_none_or(aerie::Pigeons::valid)
            && self.heist.as_ref().is_none_or(windcountry::Heist::valid)
            && self.joust.as_ref().is_none_or(joust::Joust::valid)
            && self.minions.valid()
            && self.home.squire_hall.valid()
            && self.squire_hall.valid()
            && countryside_squires::valid(self)
            && self.home.squire_academy.valid()
            && self.squire_academy.valid()
            && self.town.valid()
            && self.nook.as_ref().is_none_or(|n| n.valid(self.dungeon.rooms.len()))
            && self.deep.as_ref().is_none_or(|d| d.valid(self.dungeon.rooms.len()))
            && self.home.deep.valid()
            && self.players.values().all(|h| h.soak <= town::SOAK)
            && self.folly_room.is_none_or(|i| i < self.dungeon.rooms.len())
            && !self.dungeon.rooms.is_empty()
            && self.dungeon.rooms.len()
                <= if self.at_home_now() {
                    world::HOME_ROOM_LIMIT
                } else {
                    64
                }
            && self.at < self.dungeon.rooms.len()
            && self.settlement_exhibits.len() <= 32
            && (self.settlement_exhibits.is_empty() || self.settlement_site.is_some())
            && self.dungeon.rooms.iter().all(Room::valid_snapshot)
            && self.dungeon.altar_at.keys().all(|&i| (i as usize) < self.dungeon.rooms.len())
            && self
                .dungeon
                .altar_at
                .values()
                .all(|at| at.is_none_or(|(x, y)| x.is_finite() && y.is_finite()))
            && self.dungeon.open.as_ref().is_none_or(|o| o.valid(&self.dungeon.rooms))
            && self.parked.len() <= 128
            && self.dungeon.valid_secret()
            && self
                .light
                .is_none_or(|(x, y)| x.is_finite() && y.is_finite())
            && self.settlement_exhibits.iter().enumerate().all(|(i, m)| {
                m.valid(&self.dungeon)
                    && !self.settlement_exhibits[..i]
                        .iter()
                        .any(|prior| (prior.room, prior.col, prior.row) == (m.room, m.col, m.row))
            })
            && self.players.contains_key(&1)
            && self.players.len() <= MAX_PLAYERS as usize
            && self
                .players
                .iter()
                .all(|(id, h)| (1..=MAX_PLAYERS).contains(id) && h.x.is_finite() && h.y.is_finite())
            && self.enemies.len() <= 128
            && self.projectiles.len() <= MAX_PROJECTILES
            && self
                .projectiles
                .iter()
                .all(|p| p.traits.pierced.len() <= 128)
            && self.bosses.len() <= 64
            && self.enemies.iter().enumerate().all(|(i, e)| {
                e.boss
                    .is_none_or(|b| usize::from(b) < self.bosses.len() && e.kind == EnemyKind::Boss)
                    && e.signature.valid()
                    && e.hoard.len() <= boss_ecology::ROSTER_CAP
                    && !self.enemies[..i].iter().any(|a| a.id == e.id)
            })
            && self.valid_boss_gates_with_id_limit(allocated_id)
            && self.works.valid()
            && self.merchant.valid()
            && self.alehouse.valid()
            && self.players.values().all(|h| h.pockets.valid())
            && self.feel.valid()
    }

    pub fn new(seed: u64, raid_id: u64, guest: Option<&str>) -> Self {
        let mut rng = Rng::new(mix(seed ^ raid_id.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        let pack = if rng.chance(50) {
            Pack::Crypt
        } else {
            Pack::Cavern
        };
        let open_floors = crate::settings::get().open_floors;
        let dungeon = if open_floors {
            layout::open_floor(1, pack, 1 + usize::from(guest.is_some()), &mut rng)
        } else {
            layout::floor(1, pack, &mut rng)
        };
        let mut players = BTreeMap::new();
        for (id, name) in std::iter::once((1, "You")).chain(guest.map(|name| (2, name))) {
            let x = if guest.is_some() {
                16.0 + id as f32 * 5.0
            } else {
                WIDTH / 2.0
            };
            players.insert(id, Hero::new(name, x, HEIGHT - 6.0));
        }
        let mut run = Self {
            module_visuals: Default::default(),
            module_casts: Vec::new(),
            playtest: votes::State::default(),
            raid_id,
            seat_epochs: BTreeMap::new(),
            naked_start: false,
            identity: false,
            questline: questline::Board::default(),
            campaign: campaign::Campaign::default(),
            kingdom: kingdom::State::default(),
            tick: 0,
            hitstop: Default::default(),
            spellbook: spellbook::Spellbook::default(),
            phase: Phase::Exploring,
            floor_plates: BTreeMap::new(),
            players,
            enemies: Vec::new(),
            parked: Vec::new(),
            projectiles: Vec::new(),
            score: 0,
            dungeon,
            boss_gates: None,
            settlement_site: None,
            settlement_exhibits: Vec::new(),
            at: 0,
            bank: Vec::new(),
            reclaimed: Vec::new(),
            home: home::Home::default(),
            treasury: Spoils::default(),
            merchant: merchant::State::default(),
            alehouse: alehouse::House::default(),
            orders: Vec::new(),
            peddled: Vec::new(),
            greeted: 0,
            mode: fortune::Mode::default(),
            open_floors,
            spin: None,
            mode_said: None,
            collapse_at: None,
            deep_beat: None,
            strikes: Vec::new(),
            spheres: Vec::new(),
            phantoms: Vec::new(),
            bard_songs: Vec::new(),
            scarecrows: Vec::new(),
            champions: champions::Field::default(),
            heroes: heroes::Field::default(),
            imps: Vec::new(),
            cauldron: None,
            brewing: Vec::new(),
            kegs: Vec::new(),
            lobs: Vec::new(),
            flings: Vec::new(),
            wells: Vec::new(),
            hooks: Vec::new(),
            webs: Vec::new(),
            slams: Vec::new(),
            ravages: Vec::new(),
            hireling: None,
            squires: Vec::new(),
            lane_squires: Vec::new(),
            lane_squire_orders: Vec::new(),
            raise_orders: Vec::new(),
            lane_squire_holds: BTreeMap::new(),
            squire_yard_holds: BTreeMap::new(),
            squire_mark: None,
            squire_field: squire_company::Field::default(),
            squire_company_reserved: false,
            squire_company_claimed: false,
            squire_pending_delves: 0,
            squire_hall: Default::default(),
            countryside_squires: Vec::new(),
            countryside_squire_holds: BTreeMap::new(),
            squire_academy: Default::default(),
            minions: Default::default(),
            squire_guild: Default::default(),
            joust: None,
            chariot: None,
            forest: forest::Forest::default(),
            forest_road: None,
            lanternward: None,

            moonfall: None,

            bannerpeak: None,

            griffinwing: None,

            lightgallop: None,
            knight_racers: None,
            deathmatch: None,

            college: college::Dialogue::default(),
            pastimes: pastimes::State::default(),
            crossroads: None,
            cinder: None,
            latch: None,
            brasswake: None,
            rafterwake: None,
            pennon: None,
            latch_input_age: BTreeMap::new(),
            mossbell: None,
            exits: exits::State::default(),
            build_hold: None,
            lever_held: 0,
            rooms_held: 0,
            groom_near: false,
            king_near: false,
            holes: Vec::new(),
            critters: critters::Field::default(),
            pits: pitstop::Board::default(),
            frogs: frogs::Field::default(),
            shoulders: shoulders::Shoulders::default(),
            first: pack,
            light: None,
            triumph: None,
            feats: Vec::new(),
            banner: None,
            deeds: Vec::new(),
            unboxed: None,
            slimes_slain: 0,
            stall: Vec::new(),
            descending: 0,
            descend_for: None,
            captive: None,
            met: 0,
            cat: None,
            folly_room: None,
            folly: None,
            bats: None,
            gantry_room: None,
            gantry: None,
            powder: None,
            hop: None,
            race3d_room: None,
            race3d: None,
            pots_broken: BTreeMap::new(),
            holding_used: 0,
            holding_raid: None,
            crab: None,
            forage: None,
            valves: None,
            pigeons: None,
            heist: None,
            town: town::Town::default(),
            nook: None,
            deep: None,
            pot_shards: Vec::new(),
            gore: Vec::new(),
            dare: None,
            dares_kept: 0,
            rune: None,
            runes_taken: 0,
            hexes: 0,
            round: None,
            song: None,
            marks: BTreeMap::new(),
            lifted: Vec::new(),
            hinted: Vec::new(),
            bosses: bosses::builtin(),
            book: Book::builtin(),
            found: None,
            cues: vec!["run_start".into()],
            sounds: Vec::new(),
            seat_sounds: Vec::new(),
            sounded_at: BTreeMap::new(),
            hostile_shots: 0,
            shake: 0,
            streak: (0, 0),
            room_hurt: false,
            blooded: false,
            waves: Waves::default(),
            traps: Vec::new(),
            rocks: Vec::new(),
            spiked: false,
            works: works::Works::default(),
            team: team::State::default(),
            window: None,
            makers: BTreeMap::new(),
            ladders: BTreeMap::new(),
            notices: Vec::new(),
            seat_notices: Vec::new(),
            sparks: Vec::new(),
            fx: Vec::new(),
            blasts: Vec::new(),
            feel: feel::Feel::default(),
            floor_tally: floor_stats::Tally::default(),
            floor_results: None,
            boons: Vec::new(),
            spectacle: spectacle::Spectacle::default(),
            chatter: chatter::Chatter::default(),
            wishing: false,
            lamp: altar::Lamp::default(),
            genie: genie::Genie::default(),
            asked: Vec::new(),
            freedom: freedom::State::default(),
            starfall: starfall::Starfall::default(),
            rewards: rewards::Rewards::default(),
            ledger: rewards::Ledger::default(),
            fairy_grant: None,
            offer_keys: BTreeMap::new(),
            hallowed: None,
            stairs_held: false,
            held: Hold::Open,
            fellowship: fellowship::State::default(),
            scaled_for: 0,
            again: None,
            tune: tune::Tune::default(),
            idle: BTreeMap::new(),
            next_id: 0,
            rng,
            seed,
        };
        run.populate_boss_gates();
        run.stock_gear_cache();
        run.enter(0, None);
        run
    }

    pub fn active(&self) -> bool {
        matches!(self.phase, Phase::Fighting | Phase::Exploring)
    }

    /// The floor the party stands on, as it is counted: in the Deep, the
    /// Deep's own count (`deep`), whatever floor it plays as.
    pub fn floor(&self) -> u32 {
        self.deep_floor().unwrap_or(self.dungeon.depth)
    }

    pub fn room(&self) -> &Room {
        &self.dungeon.rooms[self.at]
    }

    /// Doors stay barred while anything in the room still lives.
    pub fn barred(&self) -> bool {
        // On an open map only the guardian's lock-in bars a door.
        if let Some(open) = self.open_here() {
            return open.chambers[usize::from(open.guardian)].sealed;
        }
        self.foes_left()
            || (self.phase == Phase::Fighting && self.waves.pending())
            || self.folly.as_ref().is_some_and(|f| f.room == self.at && !f.done)
            || self.bats.as_ref().is_some_and(|b| b.room == self.at && !b.done)
            || self.gantry.as_ref().is_some_and(|g| g.room == self.at && !g.done)
            || self.powder.as_ref().is_some_and(|p| p.room == self.at && !p.done)
            || self.hop.as_ref().is_some_and(|h| h.room == self.at && !h.done)
            || self.race3d.as_ref().is_some_and(|r| r.room == self.at && !r.done)
    }

    /// Where a friendly shot's body first meets stone on the way from
    /// `from` to `to`, as `play` stops it (`shot_stop_body`). `None` if
    /// the way is open. The wire flies a shot through the same stone, so
    /// a wall is not sent again every tick.
    pub fn shot_stone(&self, from: (f32, f32), to: (f32, f32), body: f32) -> Option<(f32, f32)> {
        Grid {
            room: self.room(),
            barred: self.barred(),
        }
        .shot_stop_body(from, to, body)
    }

    /// Something in the room still fights (the yard's quintains never do).
    pub fn foes_left(&self) -> bool {
        self.enemies.iter().any(|e| e.kind != EnemyKind::Dummy)
    }

    /// The party still stands in the Undercroft or the first floor's
    /// entrance room.
    pub fn at_entrance(&self) -> bool {
        self.dungeon.depth == 0 || (self.dungeon.depth == 1 && self.at == 0)
    }

    /// Whether `item` lies on the floor in knight `viewer`'s view: what
    /// they have taken their own of, another's set-down piece, and a helm
    /// or mail they already wear are not there for them (the operator,
    /// 2026-10-19: "only cards we can pickup"). Every seat decides its own.
    pub fn lies_for(&self, item: &Item, viewer: Option<u32>) -> bool {
        item.seen_by(viewer)
            && !viewer
                .and_then(|v| self.players.get(&v))
                .zip(self.book.get(&item.card))
                .is_some_and(|(hero, card)| matches!(card.id.as_str(), "iron-helm" | "body-armour") && !hero.takes(card))
    }

    /// The last pickup, for a few seconds after it was taken.
    pub fn found_line(&self) -> Option<String> {
        let (at, id, card) = self.found.as_ref()?;
        (self.tick.saturating_sub(*at) < u64::from(3 * HZ)).then(|| {
            match self.players.get(id) {
                Some(hero) => format!("{}: {card}", hero.name),
                // Id 0 is the dungeon itself: a guardian falling.
                None => card.clone(),
            }
        })
    }

    /// Hand every knight's carried spoils to the bank: all of them, or half
    /// when the party fell or turned back.
    fn bank_all(&mut self, whole: bool, why: &str) {
        if whole { self.keep_all_gear(); }
        self.bank_coins();
        let pay = self.mode.spoils();
        for (&id, hero) in &mut self.players {
            let carried = std::mem::take(&mut hero.carried);
            let spoils = if whole { carried } else { carried.half() };
            let spoils = spoils.scaled(pay);
            if !spoils.is_empty() {
                self.bank.push(Haul {
                    hero: id,
                    spoils,
                    why: why.to_string(),
                });
            }
        }
    }

    /// The party turns back from an unfinished run: half comes home.
    pub fn retreat(&mut self) {
        if self.active() {
            self.keep_all_gear();
            self.wishes_home(false);
            self.bank_all(false, "the party turned back");
        }
    }

    /// Add a card to this run's book and drop one at knight `id`'s feet.
    pub fn add_card(&mut self, card: Card, id: u32) -> bool {
        let card_id = card.id.clone();
        let spell = card.kind == cards::Kind::Spell;
        let name = card.name.clone();
        if !self.book.insert(card) {
            return false;
        }
        if spell {
            let card = self.book.get(&card_id).unwrap();
            if self
                .players
                .get_mut(&id)
                .is_some_and(|hero| hero.equip_spell(card))
            {
                self.spell_flourish(id, &name);
                return true;
            }
            // Updating an equipped spell refreshes its rules, not its timer.
            if self
                .players
                .get(&id)
                .is_some_and(|hero| hero.spells.iter().flatten().any(|s| s == &card_id))
            {
                return true;
            }
        }
        if let Some(hero) = self.players.get(&id) {
            let (x, y) = (hero.x, hero.y + 2.0);
            let bottom = self.room().height() - 3.0;
            self.drop_item(card_id, x, y.min(bottom), Some(id));
        }
        true
    }

    /// Player 2 joins beside player 1, or takes a new name if already here.
    /// Monsters in the room grow to the party's size, as fresh rooms do.
    pub fn join(&mut self, name: &str) {
        self.join_seat(2, name);
    }

    /// Knight `id` (2 to `MAX_PLAYERS`) joins beside player 1, or takes a new name if
    /// already here.
    pub fn join_seat(&mut self, id: u32, name: &str) {
        if !(2..=MAX_PLAYERS).contains(&id) {
            return;
        }
        if let Some(hero) = self.players.get_mut(&id) {
            let name = Hero::new(name, 0.0, 0.0).name;
            if hero.name != name {
                self.playtest.seats.remove(&id);
            }
            let changed = hero.name != name;
            hero.name = name;
            if changed { self.retire_wish_seat(id); }
            if changed { self.restore_gear(id); self.stock_gear_cache(); }
            self.sync_squires();
            return;
        }
        let (x, y) = self
            .players
            .iter()
            .find(|(seat, _)| !self.exits.outside.contains_key(seat))
            .map_or((WIDTH / 2.0, HEIGHT - 6.0), |(_, h)| (h.x, h.y));
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let (x, y) = [(2.4, 0.0), (-2.4, 0.0), (0.0, 2.4), (0.0, -2.4)]
            .into_iter()
            .map(|(dx, dy)| (x + dx, y + dy))
            .find(|&(x, y)| grid.clear(x, y, HERO_RADIUS, Mover::Hero))
            .unwrap_or((x, y));
        let before = self.scaled();
        self.players.insert(id, Hero::new(name, x, y));
        self.restore_gear(id);
        self.stock_gear_cache();
        if let Some(plate) = self.floor_plate(Some(1)) {
            self.floor_plates.insert(id, plate);
        }
        self.rescale_party(before);
        self.deal_home(id);
        if !self.at_home_now() {
            self.harden();
        }
        self.wishes_for_newcomer(id);
        self.sync_squires();
        if self.party_len() >= 4 {
            self.notice("party_of_four");
        }
    }

    pub fn seat_epoch(&self, player: u32) -> u64 {
        self.seat_epochs.get(&player).copied().unwrap_or(0)
    }

    pub(super) fn retire_wish_seat(&mut self, player: u32) {
        let epoch = self.seat_epochs.entry(player).or_default();
        *epoch = epoch.checked_add(1).expect("seat incarnation exhausted");
        self.asked.retain(|ask| ask.knight != player);
        self.lamp.wishing.retain(|pending| pending.knight != player);
        if let Some(hero) = self.players.get_mut(&player) { hero.wish_jobs.clear(); }
    }

    /// A knight leaves the party; what they dropped is free to take again.
    pub fn leave(&mut self, id: u32) {
        let before = self.scaled();
        self.floor_plates.remove(&id);
        self.exits.outside.remove(&id);
        self.playtest.seats.remove(&id);
        if let Some(race) = &mut self.race3d { race.riders.remove(&id); }
        self.keep_gear(id);
        if self.players.contains_key(&id) { self.retire_wish_seat(id); }
        self.countryside_squires.retain(|c| c.owner != id);
        self.countryside_squire_holds.remove(&id);
        self.squire_hall.companions.retain(|c| c.owner != id);
        self.squire_hall.holds.remove(&id);
        self.squire_yard_holds.remove(&id);
        let Some(hero) = self.players.remove(&id) else {
            return;
        };
        if !hero.carried.is_empty() {
            self.bank.push(Haul {
                hero: id,
                spoils: hero.carried,
                why: format!("{} went home", hero.name),
            });
        }
        for room in &mut self.dungeon.rooms {
            for item in &mut room.items {
                if item.held_off == Some(id) {
                    item.held_off = None;
                }
            }
        }
        self.rescale_party(before);
        if !self.players.is_empty()
            && self
                .players
                .keys()
                .all(|i| self.exits.outside.contains_key(i))
            && let Some(back) = self.exits.outside.values().next().cloned()
        {
            self.finish_activity_return(&back);
        }
    }

    /// The knights who count as the party: every one whose player is here
    /// (`Hero::away` and `Hero::absent` count for nothing: monsters are
    /// sized, and hit, for the players who are playing).
    pub fn party_len(&self) -> usize {
        self.players.values().filter(|h| !h.away && !h.absent).count()
    }

    /// The party size the monsters stand scaled for.
    fn scaled(&self) -> u32 {
        if self.scaled_for == 0 { (self.players.len() as u32).max(1) } else { self.scaled_for }
    }

    /// Monsters scaled from `before` knights to the party as it is now
    /// (`party_len`), once a knight joins, leaves, goes away or comes back.
    fn rescale_party(&mut self, before: u32) {
        let now = (self.party_len() as u32).max(1);
        if now != before.max(1) {
            self.rescale(before, now);
        }
        self.scaled_for = if now as usize == self.players.len() { 0 } else { now };
    }

    /// Monster health follows the party size, keeping each one's share of
    /// damage already dealt.
    fn rescale(&mut self, before: u32, now: u32) {
        let now = now.max(1);
        let before = before.max(1);
        let (was, is) = (
            stakes::party_fury(before as usize),
            stakes::party_fury(now as usize),
        );
        for enemy in &mut self.enemies {
            enemy.max_hp = u64::from(enemy.base_hp()).saturating_mul(u64::from(now))
                .min(u64::from(u32::MAX)) as u32;
            enemy.hp = (u64::from(enemy.hp) * u64::from(now))
                .div_ceil(u64::from(before.max(1)))
                .clamp(1, u64::from(enemy.max_hp)) as u32;
            enemy.fury = enemy.fury.saturating_sub(was).saturating_add(is);
        }
    }

    /// Where knight `id`'s morningstars are this tick (top-down: a circle;
    /// side-on too).
    pub fn orbits(&self, id: u32) -> Vec<(f32, f32)> {
        self.orbits_at(id, self.tick)
    }

    /// Where knight `id`'s morningstars were (or are) at `tick`, about where
    /// the knight stands now.
    pub(crate) fn orbits_at(&self, id: u32, tick: u64) -> Vec<(f32, f32)> {
        let Some(hero) = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone) else {
            return Vec::new();
        };
        orbit_points(hero, id, tick)
    }

    /// Morningstars knock monsters' shots out of the air on every tick, and
    /// strike on the beat: each monster they touched since the last beat,
    /// once, as one stroke from the knight (`aoe`).
    fn swing_orbits(&mut self) {
        let beat = self.tick.is_multiple_of(u64::from(ORBIT_EVERY));
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let balls = self.orbits(id);
            if balls.is_empty() {
                continue;
            }
            for &(bx, by) in &balls {
                for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                    if (shot.x - bx).hypot(shot.y - by) <= 0.9 {
                        shot.ttl = 0;
                    }
                }
            }
            if !beat {
                continue;
            }
            let swept: Vec<(f32, f32)> = (0..u64::from(ORBIT_EVERY))
                .flat_map(|back| self.orbits_at(id, self.tick.saturating_sub(back)))
                .collect();
            let hero = &self.players[&id];
            let (at, damage) = ((hero.x, hero.y), hero.scaled(ORBIT_DAMAGE));
            let struck = aoe::strike(&mut self.enemies, at, damage, |e| {
                swept
                    .iter()
                    .any(|&(bx, by)| (e.x - bx).hypot(e.y - by) <= e.radius() + 0.8)
            });
            if !struck.is_empty() {
                self.sounds.push("hit");
            }
            for (i, damage) in struck {
                let enemy = &mut self.enemies[i];
                enemy.wits.foe = id;
                self.feel.credit(id, enemy.id, damage, self.tick);
                if damage > 0 { self.feel.melee.contact(id, hero, enemy, damage, false); }
            }
        }
    }

    /// Chain hits spark on: each to the nearest monsters within reach that
    /// it has not touched, for half its damage.
    fn spark(&mut self, chains: Vec<flight::Chain>) {
        let mut children = Vec::new();
        for (first, x, y, damage, links, owner, fork) in chains {
            let mut fork = fork;
            let mut touched = vec![first];
            let mut at = (x, y);
            for _ in 0..links {
                let next = self
                    .enemies
                    .iter_mut()
                    .filter(|e| e.hp > 0 && !touched.contains(&e.id))
                    .filter(|e| (e.x - at.0).hypot(e.y - at.1) <= CHAIN_REACH)
                    .min_by(|a, b| {
                        (a.x - at.0)
                            .hypot(a.y - at.1)
                            .total_cmp(&(b.x - at.0).hypot(b.y - at.1))
                    });
                let Some(enemy) = next else {
                    break;
                };
                let actual = damage.max(1).min(enemy.hp);
                enemy.hp = enemy.hp.saturating_sub(damage.max(1));
                enemy.wits.foe = u32::from(owner);
                self.feel
                    .credit(u32::from(owner), enemy.id, actual, self.tick);
                touched.push(enemy.id);
                self.fx.push(Fx::Spark(Spark {
                    from: at,
                    to: (enemy.x, enemy.y),
                    ttl: 6,
                }));
                // A forking shot's chain forks where its first link lands:
                // two shots fly on past that monster along the arc.
                if let Some(shot) = fork.take() {
                    let (hit, to) = (enemy.id, (enemy.x, enemy.y));
                    let (dx, dy) = (to.0 - at.0, to.1 - at.1);
                    let len = dx.hypot(dy).max(0.001);
                    let speed = shot.vx.hypot(shot.vy).max(1.0);
                    children.extend(flight::fork_children(&shot, (dx / len * speed, dy / len * speed), to, hit));
                }
                at = (enemy.x, enemy.y);
            }
        }
        self.adopt_children(children);
    }

    /// A quiet room: wounds close a little, mana fills, and the altar's
    /// sign says what it is for.
    fn hallow(&mut self) {
        // The well of the floor: a knight in a borrowed body is themselves.
        self.unpossess_all();
        for hero in self.players.values_mut() {
            if hero.hp > 0 {
                hero.mend(hero.max_hp / 5, stakes::Mend::Rest);
            }
            hero.mana = MAX_MANA;
        }
        self.hallowed = Some(self.tick);
        self.cues.push("sanctuary".into());
        self.frogs_arrive();
        // What the realm's freedom wishes made of the safe room (`freedom`).
        self.freedom_hallow();
    }

    /// Where a boss (or the dragon) rises in the room: at the head of the
    /// room, unless the party has just come in there. Then the far end, or
    /// a quarter in from a side, whichever first lies [`BOSS_CLEARANCE`]
    /// from every knight;
    /// failing all of them, the one farthest from the nearest knight.
    fn boss_anchor(&self) -> (f32, f32) {
        let room = self.room();
        let (w, h) = (room.width(), room.height());
        let anchors = [(w / 2.0, 7.0), (w / 2.0, h - 7.0), (w / 4.0, h / 2.0), (w * 3.0 / 4.0, h / 2.0)];
        let nearest = |(x, y): (f32, f32)| {
            self.players
                .values()
                .filter(|hero| !hero.absent)
                .map(|hero| (hero.x - x).hypot(hero.y - y))
                .fold(f32::MAX, f32::min)
        };
        anchors
            .iter()
            .copied()
            .find(|&a| nearest(a) >= BOSS_CLEARANCE)
            .unwrap_or_else(|| {
                anchors.iter().copied().max_by(|&a, &b| nearest(a).total_cmp(&nearest(b))).unwrap_or(anchors[0])
            })
    }

    /// Place a monster on open floor, well away from the knights.
    fn spawn(&mut self, kind: EnemyKind) {
        if self.side_on() {
            let (x, y) = self.perch(kind.flies());
            if self.boss_gates.as_ref().is_some_and(|g| {
                g.economy.camp(self.at).is_none() && !g.encounter(self.at, kind, None)
            }) {
                // Non-camp initial rosters (entrance/ledge/etc.) also use durable
                // provision, not a raw reentry path around finite accounting.
                self.spawn_at(kind, x, y);
            } else {
                self.spawn_body(kind, x, y);
            }
            return;
        }
        // On an open map a newcomer comes into the party's chamber, awake.
        if self.open_map() {
            if let Some(((x, y), home)) = self.open_spawn_spot()
                && self.spawn_body(kind, x, y).is_some()
                && let Some(e) = self.enemies.last_mut()
            {
                e.home = Some(home);
            }
            return;
        }
        let room = &self.dungeon.rooms[self.at];
        let (x, y) = if kind == EnemyKind::Dragon {
            self.boss_anchor()
        } else {
            let mut spot = (room.width() / 2.0, 6.0);
            for attempt in 0..64 {
                let col = 2 + self.rng.below(room.cols - 4) as i32;
                let row = 2 + self.rng.below(room.rows - 4) as i32;
                let (x, y) = (
                    (col as f32 + 0.5) * TILE_UNITS,
                    (row as f32 + 0.5) * TILE_UNITS,
                );
                let far = self
                    .players
                    .values()
                    .all(|hero| (hero.x - x).hypot(hero.y - y) > 12.0 - attempt as f32 / 8.0);
                let taken = self
                    .enemies
                    .iter()
                    .any(|e| (e.x - x).abs() < 1.0 && (e.y - y).abs() < 1.0);
                if room.tile(col, row).ground() && far && !taken {
                    spot = (x, y);
                    break;
                }
            }
            spot
        };
        if self
            .boss_gates
            .as_ref()
            .is_some_and(|g| g.economy.camp(self.at).is_none() && !g.encounter(self.at, kind, None))
        {
            // Non-camp initial rosters (entrance/ledge/etc.) also use durable
            // provision, not a raw reentry path around finite accounting.
            self.spawn_at(kind, x, y);
        } else {
            self.spawn_body(kind, x, y);
        }
    }

    /// A monster at a chosen spot, sized to the party.
    fn spawn_at(&mut self, kind: EnemyKind, x: f32, y: f32) -> Option<u32> {
        if self.boss_gates.is_some() {
            self.spawn_staged(kind, x, y, 0)
        } else {
            self.spawn_body(kind, x, y)
        }
    }

    /// Raw body insertion, for admitted roster/encounter bodies or a provisioned
    /// emission. Callers account only the returned successful insertion.
    fn spawn_body(&mut self, kind: EnemyKind, x: f32, y: f32) -> Option<u32> {
        if self.boss_gates.is_some() && self.enemies.len() >= boss_ecology::ENTITY_CAP {
            return None;
        }
        // Giant's Feast: every ordinary monster a giant.
        let giant = self.mode == fortune::Mode::GiantsFeast
            && !matches!(kind, EnemyKind::Dragon | EnemyKind::Boss);
        // The floor and the party's strength set how tough it is; the
        // dragon, a guardian, grows with the party's strength as one.
        let tough = if kind == EnemyKind::Dragon { self.guardian_toughness() } else { self.toughness() };
        let base = (if giant { kind.hp() * 9 / 5 } else { kind.hp() }) * tough / 100;
        let hp = base * self.party_len().max(1) as u32;
        self.next_id = self.next_id.checked_add(1)?;
        self.enemies.push(Enemy {
            hoard: Vec::new(),
            faction: None,
            camp_slot: None,
            x,
            y,
            hp,
            max_hp: hp,
            kind,
            age: 0,
            id: self.next_id,
            origin_x: x,
            origin_y: y,
            boss: None,
            size: giant.then(|| kind.radius() * 1.4),
            base_hp: Some(base),
            raged: false,
            hurt: 0,
            seen_hp: hp,
            drain: 0,
            dread: 0,
            elite: false,
            stash: Default::default(),
            loot: Vec::new(),
            cry: 0,
            hint: 0,
            hints: 0,
            fury: FURY[(self.dungeon.depth.clamp(1, 6) - 1) as usize]
                + stakes::party_fury(self.party_len()),
            rung: 0,
            chilled: 0,
            pattern_volleys: 0,
            frozen: 0,
            sleep_until: 0,
            stage: 0,
            timer: 0,
            dir: (0.0, 0.0),
            guard: 0,
            wits: press::Wits::default(),
            signature: signatures::State::default(),
            home: None,
            dormant: false,
        });
        Some(self.next_id)
    }

    /// The floor's guardian takes the top of the stairs room.
    fn spawn_boss(&mut self, index: usize) {
        if self.boss_gates.is_some() && self.enemies.len() >= boss_ecology::ENTITY_CAP {
            return;
        }
        let boss = &self.bosses[index];
        // A guardian is tougher than the floor's rank and file.
        let scaled = u64::from(boss.hp) * u64::from(self.guardian_toughness()) / 100
            * u64::from(self.boss_health()) / 100;
        let base = (scaled.saturating_mul(70) / 100).clamp(1, u64::from(u32::MAX)) as u32;
        let hp = u64::from(base).saturating_mul(self.party_len().max(1) as u64)
            .min(u64::from(u32::MAX)) as u32;
        let room = self.room();
        let (mut x, mut y) = self.boss_anchor();
        // Threshold scenery and chamber pillars must not bury an anchor boss.
        let col = (x / TILE_UNITS) as i32;
        let row = (y / TILE_UNITS) as i32;
        let grid = Grid {
            room,
            barred: self.barred(),
        };
        let open = |c: i32, r: i32| {
            if boss.id == signatures::ENGINE {
                room.tile(c, r) == Tile::Floor
                    && grid.clear(
                        (c as f32 + 0.5) * TILE_UNITS,
                        (r as f32 + 0.5) * TILE_UNITS,
                        boss.radius,
                        Mover::Walker,
                    )
            } else {
                (-1..=1).all(|dc| (-1..=1).all(|dr| room.tile(c + dc, r + dr) == Tile::Floor))
            }
        };
        let clear = if boss.id == signatures::ENGINE {
            room.tile(col, row) == Tile::Floor && grid.clear(x, y, boss.radius, Mover::Walker)
        } else {
            open(col, row)
        };
        if !clear
            && let Some((c, r)) = (3..room.rows as i32 - 1)
                .flat_map(|r| (1..room.cols as i32 - 1).map(move |c| (c, r)))
                .filter(|&(c, r)| open(c, r))
                .min_by_key(|&(c, r)| (c - col).abs() + (r - row).abs())
        {
            x = (c as f32 + 0.5) * TILE_UNITS;
            y = (r as f32 + 0.5) * TILE_UNITS;
        }
        self.next_id += 1;
        self.enemies.push(Enemy {
            hoard: Vec::new(),
            faction: None,
            camp_slot: None,
            x,
            y,
            hp,
            max_hp: hp,
            kind: EnemyKind::Boss,
            age: 0,
            id: self.next_id,
            origin_x: x,
            origin_y: y,
            boss: Some(index as u8),
            size: Some(boss.radius),
            base_hp: Some(base),
            raged: false,
            hurt: 0,
            seen_hp: hp,
            drain: 0,
            dread: 0,
            elite: false,
            stash: Default::default(),
            loot: Vec::new(),
            cry: 0,
            hint: 0,
            hints: 0,
            // A guardian's volleys keep their shape on every floor; only
            // the head count thickens them.
            fury: stakes::party_fury(self.party_len()),
            rung: 0,
            chilled: 0,
            pattern_volleys: 0,
            frozen: 0,
            sleep_until: 0,
            stage: 0,
            timer: 0,
            dir: (0.0, 0.0),
            guard: 0,
            wits: press::Wits::default(),
            signature: signatures::State::default(),
            home: None,
            dormant: false,
        });
    }

    /// How many knights the floors are laid out for: every seat with a
    /// knight in it, the host's empty seat not counted.
    pub fn party_size(&self) -> usize {
        self.players.values().filter(|h| !h.absent).count()
    }

    /// One fixed step of the run, and of the show watching it.
    /// Deterministic simulation: no clock, ambient randomness, network or
    /// model calls.
    pub fn step(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.clear_finished_powder_board();
        let creator_inputs = self.creator_inputs(inputs);
        let inputs = &creator_inputs;
        if self.crossroads.is_some() {
            self.count_idle(inputs);
            self.tick_crossroads(inputs);
            return;
        }
        // Leaving follows transport time, even while an impact holds the scene.
        // The map is copied only when a hold actually blanks a seat.
        let blanks = self.tick_leave_hold(inputs);
        let transport_owned;
        let transport_inputs = if blanks.iter().any(Option::is_some) {
            let mut owned = inputs.clone();
            for id in blanks.into_iter().flatten() {
                owned.insert(id, Input::default());
            }
            transport_owned = owned;
            &transport_owned
        } else {
            inputs
        };
        let fresh;
        let inputs = match self.hitstop.inputs(&self.tune, &self.players, transport_inputs) {
            hitstop::Resume::Wait => return,
            hitstop::Resume::Same => transport_inputs,
            hitstop::Resume::Fresh(map) => {
                fresh = map;
                &fresh
            }
        };
        // A knight's wishes are held to the budget of the floor they stand on.
        self.keep_wish_floor();
        // Who is here may have changed since the last tick: the monsters
        // stand scaled for the knights who are.
        let before = self.scaled();
        self.rescale_party(before);
        self.count_idle(&inputs);
        // Retreat is processed before reins, overlays, or combat. Discard the
        // rest of a departing seat's controls, including a held interact key.
        let stripped;
        let inputs = if inputs.values().any(|i| i.valid() && i.leave_activity) {
            let mut owned = inputs.clone();
            for (&id, input) in &mut owned {
                if input.valid() && input.leave_activity && self.leave_activity(id) {
                    *input = Input::default();
                }
            }
            stripped = owned;
            &stripped
        } else {
            inputs
        };
        self.close_behind_the_away();
        let detached = self.detached_seats(&inputs);
        if self.players.is_empty() {
            self.players.extend(detached);
            return;
        }
        self.tick_floor_plates(&inputs);
        let balloted;
        let inputs = match self.ballot_inputs(&inputs) {
            Some(map) => {
                balloted = map;
                &balloted
            }
            None => inputs,
        };
        let offered;
        let inputs = match self.offer_inputs(inputs) {
            Some(map) => {
                offered = map;
                &offered
            }
            None => inputs,
        };
        // A village of the Deep takes the keys of a falling star and of a
        // knight in conversation (`deep::Run::deep_inputs`).
        let villaged;
        let inputs = match self.deep_inputs(inputs) {
            Some(map) => {
                villaged = map;
                &villaged
            }
            None => inputs,
        };
        let (live, heard, sounded) = (self.active(), self.cues.len(), self.sounds.len());
        let seen = live.then(|| self.seen());
        let was_over = self.over();
        self.advance(inputs);
        if self.turbo() && self.hitstop.left == 0 {
            self.advance(inputs);
        }
        // Those the fellowship waits for are called to it (`fellowship`).
        self.call_fellowship();
        self.flush_questlines();
        self.tick_teachers(&inputs);
        self.follow_kingdom_cameras();
        if let Some(seen) = seen {
            self.watch(seen);
        }
        if live {
            self.watch_dare(heard, sounded);
            self.mark_moments(heard);
            self.mark_trophies(heard);
            self.find_salvage(heard);
        }
        if self.phase == Phase::Wiped && detached.values().any(|h| h.hp > 0) {
            self.phase = if self.barred() && !self.open_map() {
                Phase::Fighting
            } else {
                Phase::Exploring
            };
        }
        self.players.extend(detached);
        // The steps up made this tick, and the ledger (`rewards`).
        self.watch_rewards();
        if !was_over && self.over() {
            self.close_run();
        }
        self.stand_verdict();
        if !self.active() {
            self.floor_plates.clear();
        }
        // What the step's watchers showed after the tick (`fx`).
        self.show_fx();
    }

    /// Sound `name` unless it sounded less than `every` ticks ago: a ring
    /// that says the same thing again (an armour, a ward) keeps its
    /// distance however many blows land, and the first blow always rings.
    pub(crate) fn sound_every(&mut self, name: &'static str, every: u64) {
        if self.sounded_at.get(name).is_none_or(|&at| self.tick < at || self.tick - at >= every) {
            self.sounded_at.insert(name, self.tick);
            self.sounds.push(name);
        }
    }

    /// Each knight's idle ticks: back to none on any key, one more on a
    /// tick with no key or no seat at all.
    fn count_idle(&mut self, inputs: &BTreeMap<u32, Input>) {
        let players = &self.players;
        self.idle.retain(|id, _| players.contains_key(id));
        for &id in self.players.keys() {
            let pressing = inputs.get(&id).is_some_and(|i| i.valid() && *i != Input::default());
            let idle = self.idle.entry(id).or_insert(0);
            *idle = if pressing { 0 } else { idle.saturating_add(1) };
        }
    }

    /// Whether knight `id` is away: its player is not here (`set_away`, from
    /// what the host's server knows of the seat: a page loading or hidden, a
    /// link gone quiet, an empty seat), or is here but has pressed nothing
    /// for `AWAY_TICKS`. Away knights go where the party goes and hold up
    /// nothing: no door waits for them, no stair or doorway is theirs to
    /// take, and a place closes when only they are left in it.
    pub fn away(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.away) || self.idle.get(&id).is_some_and(|&t| t >= AWAY_TICKS)
    }

    /// Whether F belongs to a festival round playing in this room (the
    /// courier, the turnip) rather than to the party's arms.
    fn arcade(&self) -> bool {
        self.at_home_now()
            && self
                .pastimes
                .round
                .as_ref()
                .is_some_and(|r| r.game.room() == self.room().kind)
    }

    fn advance(&mut self, inputs: &BTreeMap<u32, Input>) {
        if !self.active() {
            return;
        }
        if self.deathmatch_showing() {
            self.tick_deathmatch(inputs);
            self.tick += 1;
            return;
        }
        self.play(inputs);
        // What the tick showed: its shakes and sparks (`fx`).
        self.present();
        self.finish_hitstop();
    }

    /// One tick of the rules, phase by phase: the knights' turn and their
    /// bombs, the foes' turn, the shots in flight and what follows them,
    /// the knights' wounds, the dead and the gathered, and the room's end.
    fn play(&mut self, inputs: &BTreeMap<u32, Input>) {
        // A ride under way (the lists, the Hippodrome, a race hall) plays
        // and takes the keys of the knights it seats (`activity::RIDES`).
        let reined = self.ride(inputs);
        let inputs = reined.as_ref().unwrap_or(inputs);
        // The cauldron answers its own keys first (`cauldron`).
        let stirred = self.tend_cauldron(inputs);
        let inputs = stirred.as_ref().unwrap_or(inputs);
        self.fade();
        let boss_beats: Vec<_> = self.enemies.iter()
            .filter(|e| e.is_boss() && e.hp > 0)
            .map(|e| (e.id, e.raged, e.frozen, self.boss_phase(e)))
            .collect();
        self.tick += 1;
        // Who waits in a way out is said afresh each tick (`travel`).
        self.fellowship.clear();
        // A room holding for its party wards every knight through the tick.
        self.ward_while_held();
        self.set_stakes();
        self.tick_pockets();
        self.tick_thieves();
        // Towers stand or come down, the thrown fly on, and the riders'
        // keys are cut to aiming and shooting (`shoulders`).
        let (shouldered, tower) = self.shoulders_begin(inputs);
        let inputs = shouldered.as_ref().unwrap_or(inputs);
        let before: Vec<(u32, u32)> = self.players.values().map(|h| (h.hp, h.max_hp)).collect();
        let mut sortie = self.knights_turn(inputs);
        self.tick_team(inputs);
        self.tick_gravecallers(inputs);
        self.bombs_burst(&sortie);
        ults::phantom_shots(&self.phantoms, &self.players, &mut sortie.shots);
        let deeds = self.foes_turn(&mut sortie);
        self.hold_hero_roots();
        self.skeletons_block();
        let struck = self.shots_fly(&mut sortie);
        self.aftermath(inputs, &mut sortie, struck, deeds);
        self.hold_hero_roots();
        self.tally_wounds(before, boss_beats);
        self.tick_feel(&sortie.credits, &sortie.grazes);
        self.settle_kills();
        self.gather();
        self.tick_waves();
        self.room_end();
        self.shoulders_end(tower);
    }
}

/// The hostile shots past `from` in `projectiles`.
fn hostile_past(projectiles: &[Projectile], from: usize) -> u64 {
    projectiles[from.min(projectiles.len())..]
        .iter()
        .filter(|p| p.hostile)
        .count() as u64
}

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// Room `index` with its interior cleared of stone and pits: an open
    /// floor for fixtures that walk, aim or summon across it.
    pub fn open_room_for_test(&mut self, index: usize) {
        let room = &mut self.dungeon.rooms[index];
        for row in 1..room.rows - 1 {
            for col in 1..room.cols - 1 {
                room.set(col, row, Tile::Floor);
            }
        }
    }

    pub fn enter_for_test(&mut self, index: usize) {
        // Unrelated combat/wave/art fixtures explicitly exercise the retained
        // legacy encounter path. Policy tests call production `enter` instead.
        self.boss_gates = None;
        let builtin = bosses::builtin();
        self.bosses
            .retain(|b| builtin.iter().any(|n| n.id == b.id) || encounter_catalog::custom(b));
        self.enter(index, Some(2));
    }
    pub fn exhibit_pattern_for_test(
        &mut self,
        emitter: patterns::Emitter,
        kind: Shot,
        from: (f32, f32),
        to: (f32, f32),
    ) {
        let grid = Grid { room: self.room(), barred: false };
        let aim = patterns::Aim {
            from, at: to, pace: (8.0, 0.0), grid: &grid,
            count: 1, salt: 0x5eed, step: 1, kind, damage: 12, guardian: None, shield_line: false,
        };
        let mut shots = Vec::new();
        patterns::emit(emitter, &aim, &mut shots);
        self.projectiles = shots;
    }

    pub fn spawn_at_for_test(&mut self, kind: EnemyKind, x: f32, y: f32) {
        // Unrelated combat/card/song fixtures explicitly use legacy actors,
        // just like enter_for_test. Ecology regressions call production paths.
        self.boss_gates = None;
        self.spawn_at(kind, x, y);
        if let Some(enemy) = self.enemies.last_mut() {
            enemy.age = TELEGRAPH;
        }
    }

    /// Every boss a floor of `pack` can raise (its guardians, its route
    /// chiefs and its apex), by index into `bosses`.
    pub fn pack_bosses_for_test(&mut self, pack: Pack) -> Vec<usize> {
        encounter_catalog::leaders(&mut self.bosses, pack, 0);
        if pack == Pack::Unknown {
            // The threshold's heart.
            encounter_catalog::apex(&mut self.bosses, pack, RoomKind::Threshold, &[]);
        }
        (0..self.bosses.len())
            .filter(|&i| self.bosses[i].only_in == pack)
            .collect()
    }

    /// Boss `index` stands at `(x, y)`, past its telegraph.
    pub fn spawn_boss_at_for_test(&mut self, index: usize, x: f32, y: f32) {
        self.spawn_boss(index);
        if let Some(enemy) = self.enemies.last_mut() {
            (enemy.x, enemy.y, enemy.origin_x, enemy.origin_y) = (x, y, x, y);
            enemy.age = TELEGRAPH;
        }
    }

    /// A ring of harmless shots thrown out from `(x, y)`, one of every kind
    /// (a gallery of how shots look in flight).
    pub fn volley_for_test(&mut self, x: f32, y: f32, turn: f32) {
        const KINDS: [Shot; 17] = [
            Shot::Blade, Shot::Arrow, Shot::Bolt, Shot::Ball, Shot::Bone, Shot::Orb, Shot::Ember,
            Shot::Sacred, Shot::Hex, Shot::Frost, Shot::Missile, Shot::Wisp, Shot::Ore, Shot::Flame,
            Shot::Page, Shot::Spore, Shot::Eye,
        ];
        for (i, kind) in KINDS.into_iter().enumerate() {
            let a = turn + i as f32 / KINDS.len() as f32 * std::f32::consts::TAU;
            let speed = 6.0 + (i % 3) as f32 * 3.0;
            self.projectiles.push(Projectile {
                x,
                y,
                vx: a.cos() * speed,
                vy: a.sin() * speed,
                hostile: false,
                kind,
                look: None,
                damage: 0,
                pierce: u8::MAX,
                last_hit: None,
                ttl: 3 * HZ,
                empowered: i % 5 == 0,
                traits: ShotTraits::default(),
            });
        }
    }

    /// A storm for the bullet bench (`tests/client_bullet_storm__tests.rs`):
    /// top the air up to `target` shots, past every cap, scattered over the
    /// room and flying every way. `mix` picks whose: 0 a bit of everything,
    /// 1 the monsters', 2 the knights' plain arrows, 3 the knights' homing
    /// arrows. `rng` is its own xorshift state, so a run repeats.
    pub fn storm_for_test(&mut self, target: usize, mix: u8, rng: &mut u64) {
        const HOSTILE: [Shot; 4] = [Shot::Orb, Shot::Bone, Shot::Wisp, Shot::Eye];
        let knights = self.players.len().max(1) as u64;
        let mut next = move || {
            *rng ^= *rng << 13;
            *rng ^= *rng >> 7;
            *rng ^= *rng << 17;
            (*rng >> 11) as f32 / (1u64 << 53) as f32
        };
        self.projectiles.reserve(target.saturating_sub(self.projectiles.len()));
        let mut n = self.projectiles.len() as u64;
        while self.projectiles.len() < target {
            n += 1;
            let (x, y) = (3.0 + next() * 42.0, 3.0 + next() * 22.0);
            let a = next() * std::f32::consts::TAU;
            let speed = 3.0 + next() * 4.0;
            let pick = match mix {
                0 => match n % 20 {
                    0..=7 => 1,
                    8..=16 => 2,
                    _ => 3,
                },
                m => m,
            };
            let hostile = pick == 1;
            let mut shot = Projectile {
                x,
                y,
                vx: a.cos() * speed,
                vy: a.sin() * speed,
                hostile,
                kind: if hostile { HOSTILE[(n % 4) as usize] } else { Shot::Arrow },
                look: None,
                damage: if hostile { 1 } else { 2 },
                pierce: 0,
                last_hit: None,
                ttl: if hostile { 60 + (next() * 120.0) as u32 } else { 45 + (next() * 45.0) as u32 },
                empowered: false,
                traits: ShotTraits::default(),
            };
            if !hostile {
                shot.traits.owner = 1 + (n % knights) as u8;
                shot.traits.arm = true;
                if pick == 3 {
                    shot.traits.homing = 2;
                }
            }
            self.projectiles.push(shot);
        }
    }

    pub fn clear_prerequisites_for_test(&mut self) {
        if let Some(g) = &mut self.boss_gates {
            for i in 0..g.leaders.len() {
                let room = g.leaders[i].room;
                for (slot, _, _) in g.economy.deploy(room).unwrap() {
                    g.economy.casualty(room, slot);
                }
                let w = &mut g.leaders[i];
                w.defeated = true;
                self.dungeon.rooms[w.room].visited = true;
                self.dungeon.rooms[w.room].cleared = true;
                g.economy.lose(i as u8);
            }
        }
    }

    pub fn descend_for_test(&mut self) {
        // Floor-setup helper, not a test of the production travel guard.
        self.clear_prerequisites_for_test();
        if let Some(g) = &mut self.boss_gates {
            g.guardian_defeated = true;
            self.dungeon.rooms[g.guardian].visited = true;
            self.dungeon.rooms[g.guardian].cleared = true;
        }
        self.descend();
        // This helper sets up a floor for unrelated combat/art tests.
        // Production transition tests call the actual travel/descend path.
        if let Some(c) = self.crossroads.take() {
            self.finish_descent(c.depth, c.pack, false);
        }
        self.floor_plates.clear();
    }

    pub fn kit_out_for_test(&mut self) {
        self.kit_out();
    }

    /// No more waves, traps or falling rocks in this room.
    pub fn calm_for_test(&mut self) {
        self.waves = Waves::default();
        self.traps.clear();
        self.rocks.clear();
    }

    /// Advance one simulation tick, allowing an existing impact hold to end.
    /// Timing tests call `step` directly to count transport ticks instead.
    pub fn step_sim_for_test(&mut self, inputs: &BTreeMap<u32, Input>) {
        while self.hitstop.left > 0 { self.step(&BTreeMap::new()); }
        self.step(inputs);
    }

    pub fn clear_for_test(&mut self) {
        self.calm_for_test();
        // Some kinds do not go quietly: a geode splits, a slime divides.
        // Sweep until the room is actually empty, so "cleared" is true.
        for _ in 0..6 {
            for enemy in &mut self.enemies {
                enemy.hp = 0;
            }
            self.step_sim_for_test(&BTreeMap::new());
            if self.enemies.is_empty() {
                break;
            }
        }
        while self.hitstop.left > 0 { self.step(&BTreeMap::new()); }
    }

    pub fn lair_for_test(&mut self) {
        while self.dungeon.depth < FLOORS {
            self.descend_for_test();
        }
        let lair = self
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::Lair)
            .expect("the last floor has a lair");
        self.clear_prerequisites_for_test();
        self.enter(lair, Some(2));
    }
}

#[cfg(test)]
#[path = "../tests/rules/together_shooter__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/rules/together_ledge__tests.rs"]
mod ledge_tests;

#[cfg(test)]
#[path = "../tests/rules/together_balance__tests.rs"]
mod balance_tests;

#[cfg(test)]
#[path = "../tests/rules/together_deep__tests.rs"]
mod deep_tests;

#[cfg(test)]
#[path = "../tests/rules/together_polish__tests.rs"]
mod polish_tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../tests/rules/together_polish__fixtures.rs"]
pub mod polish_testing;

#[cfg(test)]
#[path = "../tests/rules/together_balance_sim__tests.rs"]
pub(crate) mod balance_sim_tests;

#[cfg(test)]
#[path = "../tests/rules/together_boss_rules__tests.rs"]
mod boss_rules_tests;

#[cfg(test)]
#[path = "../tests/rules/together_stakes__tests.rs"]
mod stakes_tests;

#[cfg(test)]
#[path = "../tests/rules/together_boss_gates__tests.rs"]
mod boss_gate_tests;

#[cfg(any(test, feature = "testing"))]
#[path = "../tests/rules/together_boss_gates__fixtures.rs"]
pub mod boss_gate_testing;

#[cfg(test)]
#[path = "../tests/rules/together_pacing__tests.rs"]
mod pacing_tests;

#[cfg(test)]
#[path = "../tests/rules/together_hand__tests.rs"]
mod hand_tests;

#[cfg(test)]
#[path = "../tests/rules/together_skills__tests.rs"]
mod skills_tests;
#[cfg(test)]
#[path = "../tests/rules/together_spectacle__tests.rs"]
mod spectacle_tests;

#[cfg(test)]
#[path = "../tests/rules/together_party_rooms__tests.rs"]
mod party_rooms_tests;

#[cfg(test)]
#[path = "../tests/rules/together_cinder__tests.rs"]
mod cinder_tests;

#[cfg(test)]
#[path = "../tests/rules/together_latch__tests.rs"]
mod latch_tests;

#[cfg(test)]
#[path = "../tests/rules/together_mossbell__tests.rs"]
mod mossbell_tests;

#[cfg(test)]
#[path = "../tests/rules/together_brasswake__tests.rs"]
mod brasswake_tests;
#[cfg(test)]
#[path = "../tests/rules/together_rafterwake__tests.rs"]
mod rafterwake_tests;
#[cfg(test)]
#[path = "../tests/rules/together_pennon__tests.rs"]
mod pennon_tests;

#[cfg(test)]
#[path = "../tests/rules/together_party_scale__tests.rs"]
mod party_scale_tests;

#[cfg(test)]
#[path = "../tests/rules/together_six_seats__tests.rs"]
mod six_seats_tests;

#[cfg(test)]
#[path = "../tests/rules/restore_fuzz__tests.rs"]
mod restore_fuzz_tests;

#[cfg(test)]
#[path = "../tests/rules/overflow__tests.rs"]
mod overflow_tests;

pub mod invented;

pub mod field;

pub mod manifest;

pub mod prophecy;

pub mod spellbook;

impl Run {
    /// Teaches the run a magic a wish made up. The bounds are the checker's
    /// (`invented::check`); a wish that cannot be taught is priced and played
    /// weaker by the prophecy gate before it gets here, never refused.
    pub fn learn_spell(
        &mut self,
        spell: invented::Spell,
    ) -> Result<(), invented::Reject> {
        self.spellbook.learn(spell)
    }

    /// Casts an invented magic by the name its wish gave it, at the caster's
    /// place and aim. A name the run did not invent casts nothing.
    pub fn cast_invented(
        &self,
        name: &str,
        at: (f32, f32),
        aim: f32,
    ) -> Vec<invented::Effect> {
        self.spellbook.cast(name, at, aim)
    }
}

#[cfg(test)]
#[path = "../tests/rules/three_heroes__tests.rs"]
mod three_heroes_tests;

#[cfg(test)]
#[path = "../tests/rules/together_team__tests.rs"]
mod team_tests;

#[cfg(test)]
#[path = "../tests/rules/together_squire_lane__tests.rs"]
mod together_squire_lane__tests;

#[cfg(test)]
#[path = "../tests/rules/lane_squires__tests.rs"]
mod lane_squires_tests;

#[cfg(test)]
#[path = "../tests/rules/squire_save_migration__tests.rs"]
mod squire_save_migration_tests;
