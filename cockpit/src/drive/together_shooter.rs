//! Fixed-step, model-free dungeon crawl: knights against the monsters and
//! demons of a freshly generated dungeon. One step is 1/30 second; input
//! arrival never advances time. All entity counts and per-frame work are
//! bounded.
//!
//! The loop: walk into a room, its doors bar shut, fight what lives there,
//! pick up what it drops, take the stairs down. Each floor is new (see
//! `layout`), the third ends in the dragon's lair. Knights shoot slow, heavy
//! single shots — a bow to start, crossbow and handgonne as loot — so every
//! shot is a decision; monsters answer with slow patterns worth reading.

pub(crate) mod audience;
pub(crate) mod barony;
pub(crate) mod bestiary;
mod boss_ecology;
mod boss_gates;
mod boss_population;
pub(crate) mod bosses;
pub(crate) mod bounties;
pub(crate) mod cards;
pub(crate) mod cat;
pub(crate) mod dares;
mod encounter_catalog;
pub(crate) mod feats;
pub(crate) mod foes;
pub(crate) mod folk;
pub(crate) mod fortune;
pub(crate) mod hazards;
mod hero;
pub(crate) mod hexer;
pub(crate) mod hireling;
pub(crate) mod hollow;
pub(crate) mod home;
pub(crate) mod hunters;
pub(crate) mod items;
pub(crate) mod joust;
pub(crate) mod knights;
pub(crate) mod layout;
pub(crate) mod ledge;
pub(crate) mod lich;
mod loot;
pub(crate) mod merlin;
pub(crate) mod mirror;
mod monsters;
pub(crate) mod overclass;
pub(crate) mod phrasebook;
pub(crate) mod pit;
pub(crate) mod rescues;
pub(crate) mod runes;
pub(crate) mod script;
pub(crate) mod secrets;
mod spells;
pub(crate) mod talents;
pub(crate) mod tavern;
pub(crate) mod tide;
pub(crate) mod trophies;
pub(crate) mod ults;
pub(crate) mod world;
pub(crate) mod yard;

use crate::drive::together_realm::{Haul, Spoil, Spoils};
pub(crate) use boss_gates::Gates as BossGateState;
pub(crate) use bosses::Boss;
pub(crate) use cards::{Book, Card};
pub(crate) use hazards::{Rock, Trap, Waves};
pub(crate) use hero::Hero;
pub(crate) use layout::{COLS, DIRS, Floor, Pack, ROWS, Room, RoomKind, TILE_UNITS, Tile};
pub(crate) use loot::{Item, Rng, Weapon};
pub(crate) use monsters::{Enemy, EnemyKind};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const WIDTH: f32 = 48.0;
pub(crate) const HEIGHT: f32 = 28.0;
pub(crate) const HZ: u32 = 30;
/// Dragon Keep's floor: the dragon, and the old delve's end.
pub(crate) const FLOORS: u32 = 3;
/// The bottom of the deep below it: the Unknown.
pub(crate) const DEEPEST: u32 = 6;
pub(crate) const MAX_PROJECTILES: usize = 384;
pub(crate) const MAX_BOMBS: u32 = 5;
pub(crate) const MAX_ARMOR: u32 = 3;
/// A party of four: the host and three invited friends.
pub(crate) const MAX_PLAYERS: u32 = 4;
const MAX_ITEMS: usize = 24;
const DT: f32 = 1.0 / HZ as f32;
const HERO_RADIUS: f32 = 0.42;
const HERO_SPEED: f32 = 13.0;
/// The dodge roll: a short tumble, untouchable throughout, then a rearm.
const ROLL_TICKS: u32 = 10;
const ROLL_SPEED: f32 = 27.0;
const ROLL_REARM: u32 = 36;
/// How much of its speed a knight keeps behind a raised shield.
const SHIELD_PACE: f32 = 0.45;
/// A shield turns shots arriving within this many degrees of its facing.
const SHIELD_ARC: f32 = 65.0;
/// Mana: what a held wall drinks. It flows back after a short rest.
pub(crate) const MAX_MANA: f32 = 100.0;
const MANA_REST: u32 = HZ / 2;
const MANA_FLOW: f32 = 22.0;
/// A wall stands this far ahead of the knight holding it.
const WALL_REACH: f32 = 1.7;
const WALL_PACE: f32 = 0.6;
const BOMB_DAMAGE: u32 = 60;
pub(crate) const BOMB_REARM: u32 = HZ;
/// The vigil, for a player stepping away: how far the statue mends, how
/// often, and how long its key rests after raising it and after waking.
pub(crate) const VIGIL_REACH: f32 = 7.0;
const VIGIL_MEND_EVERY: u32 = HZ / 3;
const VIGIL_SETTLE: u32 = HZ;
const VIGIL_REARM: u32 = 5 * HZ;
const PICKUP_REACH: f32 = 1.3;
/// A held number key plays one card, then waits this long.
const PLAY_REARM: u32 = HZ / 2;
/// The fan between extra shots from held cards, in radians.
const FAN: f32 = 0.2;
/// A volley flies wider: three shots across about forty degrees.
const VOLLEY_FAN: f32 = 0.35;
/// How far a chain hit's spark reaches for the next monster.
const CHAIN_REACH: f32 = 6.5;
/// How long a burst's ring shows.
pub(crate) const BLAST_TICKS: u8 = 9;
/// How far a handgonne ball's burst reaches.
const BURST_REACH: f32 = 2.5;
/// How fast a thrown blade flies, out and back.
const BLADE_SPEED: f32 = 20.0;
/// A morningstar's orbit: its distance from the knight, its spin (radians
/// a second), how often it strikes what it touches, and how hard.
pub(crate) const ORBIT_RADIUS: f32 = 2.4;
pub(crate) const ORBIT_SPIN: f32 = 5.0;
/// Damage each tick a morningstar touches a monster (a pass lasts a few).
const ORBIT_DAMAGE: u32 = 5;
/// How fast a homing shot turns, radians a second per point of homing.
const HOMING_TURN: f32 = 2.2;
/// Every knight's sword: a close arc that also cuts shots out of the air.
const SWORD_DAMAGE: u32 = 30;
const SWORD_REACH: f32 = 2.2;
const SWORD_ARC: f32 = 130.0;
const SWORD_REARM: u32 = 18;
/// A Sanctuary's privy: how near its door a knight steps in, how long they
/// are gone, and what they come out with (mana full, and this much health).
const PRIVY_REACH: f32 = 1.2;
const PRIVY_TICKS: u32 = 2 * HZ;
const PRIVY_MEND: u32 = 10;
/// A fresh room's monsters hold still and silent this long.
pub(crate) const TELEGRAPH: u32 = HZ;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    pub(crate) move_x: i8,
    pub(crate) move_y: i8,
    pub(crate) aim_x: i8,
    pub(crate) aim_y: i8,
    pub(crate) fire: bool,
    pub(crate) dash: bool,
    pub(crate) bomb: bool,
    /// A sword swing: every knight carries a blade beside their weapon.
    #[serde(default)]
    pub(crate) swing: bool,
    /// Hand slot 1–4 to play; 0 plays nothing.
    #[serde(default)]
    pub(crate) play: u8,
    /// Spell slot 1–3; 0 casts nothing.
    #[serde(default)]
    pub(crate) cast: u8,
    /// The vigil key: its press turns the knight to warded stone, or wakes
    /// it. Left out when unpressed, so an older host reads the rest.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) vigil: bool,
    /// The ultimate's key (R): its press casts a full charge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) ult: bool,
}

impl Input {
    pub(crate) fn valid(self) -> bool {
        [self.move_x, self.move_y, self.aim_x, self.aim_y]
            .into_iter()
            .all(|axis| (-1..=1).contains(&axis))
            && usize::from(self.play) <= cards::HAND
            && usize::from(self.cast) <= cards::SPELL_SLOTS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
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
pub(crate) enum Shot {
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Projectile {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) vx: f32,
    pub(crate) vy: f32,
    pub(crate) hostile: bool,
    pub(crate) kind: Shot,
    #[serde(default)]
    pub(crate) look: Option<(String, String)>,
    damage: u32,
    pierce: u8,
    last_hit: Option<u32>,
    ttl: u32,
    /// Passed through a friend's wall: it hits harder and burns gold.
    #[serde(default)]
    pub(crate) empowered: bool,
    /// What the shooter's held cards gave this shot.
    #[serde(default)]
    pub(crate) traits: ShotTraits,
}

/// A knight's shot's gifts from held cards: walls it may bounce off, how
/// hard it curves toward monsters, how many it sparks on to, how much each
/// hit mends a friend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ShotTraits {
    pub(crate) bounce: u8,
    pub(crate) homing: u8,
    pub(crate) chain: u8,
    pub(crate) mend: u8,
    /// A thrown blade: 1 flying out (`out` ticks left), 2 coming back to
    /// knight `owner`'s hand.
    #[serde(default)]
    pub(crate) back: u8,
    #[serde(default)]
    pub(crate) owner: u8,
    #[serde(default)]
    pub(crate) out: u16,
    /// Bursts where it hits: this percent of its damage to all around.
    #[serde(default)]
    pub(crate) burst: u8,
    /// Stuns what it hits for this many ticks.
    #[serde(default)]
    pub(crate) stun: u8,
}

impl ShotTraits {
    fn of(bonus: &cards::Bonus) -> ShotTraits {
        ShotTraits {
            bounce: bonus.bounce.min(8) as u8,
            homing: bonus.homing.min(8) as u8,
            chain: bonus.chain.min(8) as u8,
            mend: bonus.mend.min(16) as u8,
            burst: bonus.burst.min(100) as u8,
            ..ShotTraits::default()
        }
    }
}

/// A chain hit's spark, from one monster to the next, for a few frames.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Spark {
    pub(crate) from: (f32, f32),
    pub(crate) to: (f32, f32),
    pub(crate) ttl: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Run {
    pub(crate) raid_id: u64,
    pub(crate) tick: u64,
    pub(crate) phase: Phase,
    pub(crate) players: BTreeMap<u32, Hero>,
    pub(crate) enemies: Vec<Enemy>,
    pub(crate) projectiles: Vec<Projectile>,
    pub(crate) score: u32,
    pub(crate) dungeon: Floor,
    /// None preserves the current floor of a legacy save; new floors get a graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boss_gates: Option<boss_gates::Gates>,
    /// Snapshot of the durable loop site admitted at the start of this Delve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) settlement_site: Option<String>,
    /// Public, generic stand geometry only. Private provenance lives in Site.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) settlement_exhibits: Vec<super::together_settlement::exhibits::Marker>,
    /// Index of the room the party stands in.
    pub(crate) at: usize,
    /// Spoils banked by this run and not yet handed to the realm.
    #[serde(default)]
    pub(crate) bank: Vec<Haul>,
    /// Delve packs whose floor this run cleared, not yet handed to the realm.
    #[serde(default)]
    pub(crate) reclaimed: Vec<Pack>,
    /// The Undercroft as the realm has built it, and the treasury it can
    /// spend: the cockpit's word, refreshed whenever either changes.
    #[serde(default)]
    pub(crate) home: home::Home,
    #[serde(default)]
    pub(crate) treasury: Spoils,
    /// Purchases asked for at the Undercroft's plates, for the cockpit.
    #[serde(skip)]
    pub(crate) orders: Vec<home::Order>,
    /// Which keepers had a knight beside them last tick (one bit each).
    #[serde(skip)]
    greeted: u8,
    /// How this delve goes, as Fortune's wheel decided it.
    #[serde(default)]
    pub(crate) mode: fortune::Mode,
    /// The wheel's spin this delve, if it was pulled.
    #[serde(default)]
    pub(crate) spin: Option<fortune::Spin>,
    /// The spin whose result has been announced.
    #[serde(default)]
    mode_said: Option<u64>,
    /// The Collapse: the tick this floor falls in.
    #[serde(default)]
    pub(crate) collapse_at: Option<u64>,
    /// Ultimates playing out: Trebuchet's shells, Stillhours, phantoms.
    #[serde(default)]
    pub(crate) strikes: Vec<ults::Strike>,
    #[serde(default)]
    pub(crate) spheres: Vec<ults::Sphere>,
    #[serde(default)]
    pub(crate) phantoms: Vec<ults::Phantom>,
    /// The pack of the delve's first floor, for the floors below it.
    #[serde(default)]
    pub(crate) first: Pack,
    /// The light home, once the dragon is slain: step into it to go home
    /// with everything.
    #[serde(default)]
    pub(crate) light: Option<(f32, f32)>,
    /// A great end reached this tick, for the realm: a dragon, or the Grail.
    #[serde(skip)]
    pub(crate) triumph: Option<Triumph>,
    /// Achievements the game noticed this tick, for the realm to keep.
    #[serde(skip)]
    pub(crate) feats: Vec<&'static str>,
    /// A new achievement's banner: when, and which.
    #[serde(default)]
    pub(crate) banner: Option<(u64, String)>,
    /// A box just opened at the coffer: when, its grade, what was in it.
    #[serde(default)]
    pub(crate) unboxed: Option<(u64, feats::Tier, String)>,
    /// Slimes slain in this room, for a family pruned.
    #[serde(skip)]
    slimes_slain: u32,
    /// Grubbins' wares this delve (an empty id: sold).
    #[serde(default)]
    pub(crate) stall: Vec<String>,
    /// Ticks a knight has stood on the Winding Stair at home.
    #[serde(default)]
    pub(crate) descending: u32,
    /// Fortune's audience this delve, in thousands of viewers; the fan
    /// boxes it has thrown, and those still floating down.
    #[serde(default)]
    pub(crate) audience: u32,
    #[serde(default)]
    pub(crate) fans: u32,
    #[serde(default)]
    pub(crate) fan_boxes: Vec<audience::FanBox>,
    /// Whoever waits in a cage on this floor.
    #[serde(default)]
    pub(crate) captive: Option<rescues::Captive>,
    /// Which of the people at home a knight is standing by (each speaks
    /// once per approach).
    #[serde(skip)]
    met: u8,
    /// Lady Tallow, Blaise's cat, when she has come along.
    #[serde(default)]
    pub(crate) cat: Option<cat::Cat>,
    /// Fortune's dare on this floor, and how many the party has kept this
    /// delve.
    #[serde(default)]
    pub(crate) dare: Option<dares::Dare>,
    #[serde(default)]
    pub(crate) dares_kept: u32,
    /// A power rune in this room (welling up a few seconds into a fight),
    /// and how many the party has taken this delve.
    #[serde(default)]
    pub(crate) rune: Option<runes::Rune>,
    #[serde(default)]
    pub(crate) runes_taken: u32,
    /// Knights hexed into frogs this delve.
    #[serde(default)]
    pub(crate) hexes: u32,
    /// Maud's round, drunk at the top of the stair for this delve.
    #[serde(default)]
    pub(crate) round: Option<String>,
    /// Sir Dinadan's song for this delve.
    #[serde(default)]
    pub(crate) song: Option<String>,
    /// Kills and moments since the cockpit last looked, for Wren's
    /// bounties.
    #[serde(skip)]
    pub(crate) marks: BTreeMap<String, u32>,
    /// The small secrets already lifted (`cup:{depth}`), and those
    /// the Herald has already hinted at: each happens once, saves included.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) lifted: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) hinted: Vec<String>,
    /// Sappers' kegs burning down, and hobs' bombs in the air.
    #[serde(default)]
    pub(crate) kegs: Vec<foes::Keg>,
    #[serde(default)]
    pub(crate) lobs: Vec<foes::Lob>,
    /// The Flesher's hooks in the air, and the Silkmother's webs.
    #[serde(default)]
    pub(crate) hooks: Vec<hunters::Hook>,
    #[serde(default)]
    pub(crate) webs: Vec<hunters::Web>,
    /// The Pit Tyrant's slams, gathering.
    #[serde(default)]
    pub(crate) slams: Vec<pit::Slam>,
    /// A guardian's Ravages, rippling and bursting.
    #[serde(default)]
    pub(crate) ravages: Vec<tide::Ravage>,
    /// Beaumains, hired for this delve.
    #[serde(default)]
    pub(crate) hireling: Option<hireling::Hireling>,
    /// A bout at the lists, while one is ridden.
    #[serde(default)]
    pub(crate) joust: Option<joust::Joust>,
    /// A knight stood by the groom last tick (he speaks once per approach).
    #[serde(skip)]
    groom_near: bool,
    /// A knight stood by King Brannoc last tick.
    #[serde(skip)]
    king_near: bool,
    /// The Hollow Ones' Black Holes, warning and pulling.
    #[serde(default)]
    pub(crate) holes: Vec<hollow::Hole>,
    /// The floors' guardians; its own copy, so a save replays.
    #[serde(default = "bosses::builtin")]
    pub(crate) bosses: Vec<Boss>,
    /// The cards this run can find; its own copy, so a save replays.
    #[serde(default)]
    pub(crate) book: Book,
    /// The last card taken or played, by whom and when, for the notice line.
    #[serde(skip)]
    pub(crate) found: Option<(u64, u32, String)>,
    /// Moments the chorus may speak about, drained by the cockpit.
    #[serde(skip)]
    pub(crate) cues: Vec<String>,
    /// Sounds this step made, drained by the cockpit's mixer.
    #[serde(skip)]
    pub(crate) sounds: Vec<&'static str>,
    /// Ticks of screen shake left (a guardian's fall, a bomb).
    #[serde(skip)]
    pub(crate) shake: u32,
    /// The last kill's tick and the kills since, for slay streaks.
    #[serde(skip)]
    streak: (u64, u32),
    /// A knight was hurt in this room (no flawless clear).
    #[serde(skip)]
    room_hurt: bool,
    #[serde(skip)]
    blooded: bool,
    /// Waves still to pour through this room's doorways.
    #[serde(default)]
    pub(crate) waves: Waves,
    /// This room's traps, and the rocks falling in it.
    #[serde(default)]
    pub(crate) traps: Vec<Trap>,
    #[serde(default)]
    pub(crate) rocks: Vec<Rock>,
    /// Spikes have bitten a knight in this room (said once).
    #[serde(skip)]
    spiked: bool,
    /// The Overclass window open now, if any: one wish each until the next
    /// fight.
    #[serde(default)]
    pub(crate) window: Option<overclass::Window>,
    /// Chain hits' sparks, fading.
    #[serde(skip)]
    pub(crate) sparks: Vec<Spark>,
    /// Handgonne bursts: where, and ticks left.
    #[serde(skip)]
    pub(crate) blasts: Vec<(f32, f32, u8)>,
    /// Wishes just granted: to which knight, and when.
    #[serde(skip)]
    pub(crate) boons: Vec<(u32, u64)>,
    /// A wish is being forged at the Sanctuary's dais (set by the cockpit
    /// while angelX writes it): its ring burns brighter.
    #[serde(skip)]
    pub(crate) wishing: bool,
    /// The tick a Sanctuary opened around the party, for its sign.
    #[serde(skip)]
    pub(crate) hallowed: Option<u64>,
    /// The stairs wait for a knight to step off them first: a guardian
    /// falling at your feet never drops you past the Sanctuary.
    #[serde(default)]
    stairs_held: bool,
    next_id: u32,
    rng: Rng,
    seed: u64,
}

/// A great end a delve reached, for the realm's tally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Triumph {
    /// The dragon slain.
    Dragon,
    /// The Unknown met, and the Grail found.
    Grail,
}

fn unit(x: f32, y: f32) -> (f32, f32) {
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
pub(crate) fn crawl_floor(depth: u32, pack: Pack, seed: u64) -> Floor {
    layout::floor(depth, pack, &mut Rng::new(seed))
}

fn mix(mut h: u64) -> u64 {
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
}

/// The current room's tiles as movement and shots see them.
struct Grid<'a> {
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
            Tile::Floor | Tile::Stairs | Tile::Ledge => true,
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

    /// Move with wall sliding, one axis at a time.
    fn slide(&self, (x, y): (f32, f32), (dx, dy): (f32, f32), r: f32, mover: Mover) -> (f32, f32) {
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
}

impl Run {
    #[cfg(test)]
    pub(crate) fn settlement_walkable_for_test(&self, x: f32, y: f32) -> bool {
        Grid {
            room: self.room(),
            barred: self.barred(),
        }
        .clear(x, y, HERO_RADIUS, Mover::Hero)
    }

    pub(crate) fn valid_snapshot(&self) -> bool {
        (0..=DEEPEST).contains(&self.dungeon.depth)
            && !self.dungeon.rooms.is_empty()
            && self.dungeon.rooms.len() <= 64
            && self.at < self.dungeon.rooms.len()
            && self.settlement_exhibits.len() <= 32
            && (self.settlement_exhibits.is_empty() || self.settlement_site.is_some())
            && self.dungeon.rooms.iter().all(Room::valid_snapshot)
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
            && self.bosses.len() <= 64
            && self.enemies.iter().enumerate().all(|(i, e)| {
                e.boss
                    .is_none_or(|b| usize::from(b) < self.bosses.len() && e.kind == EnemyKind::Boss)
                    && !self.enemies[..i].iter().any(|a| a.id == e.id)
            })
            && self.valid_boss_gates()
    }

    pub(crate) fn new(seed: u64, raid_id: u64, guest: Option<&str>) -> Self {
        let mut rng = Rng::new(mix(seed ^ raid_id.wrapping_mul(0x9e37_79b9_7f4a_7c15)));
        let pack = if rng.chance(50) {
            Pack::Crypt
        } else {
            Pack::Cavern
        };
        let dungeon = layout::floor(1, pack, &mut rng);
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
            raid_id,
            tick: 0,
            phase: Phase::Exploring,
            players,
            enemies: Vec::new(),
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
            orders: Vec::new(),
            greeted: 0,
            mode: fortune::Mode::default(),
            spin: None,
            mode_said: None,
            collapse_at: None,
            strikes: Vec::new(),
            spheres: Vec::new(),
            phantoms: Vec::new(),
            kegs: Vec::new(),
            lobs: Vec::new(),
            hooks: Vec::new(),
            webs: Vec::new(),
            slams: Vec::new(),
            ravages: Vec::new(),
            hireling: None,
            joust: None,
            groom_near: false,
            king_near: false,
            holes: Vec::new(),
            first: pack,
            light: None,
            triumph: None,
            feats: Vec::new(),
            banner: None,
            unboxed: None,
            slimes_slain: 0,
            stall: Vec::new(),
            descending: 0,
            audience: 0,
            fans: 0,
            fan_boxes: Vec::new(),
            captive: None,
            met: 0,
            cat: None,
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
            shake: 0,
            streak: (0, 0),
            room_hurt: false,
            blooded: false,
            waves: Waves::default(),
            traps: Vec::new(),
            rocks: Vec::new(),
            spiked: false,
            window: None,
            sparks: Vec::new(),
            blasts: Vec::new(),
            boons: Vec::new(),
            wishing: false,
            hallowed: None,
            stairs_held: false,
            next_id: 0,
            rng,
            seed,
        };
        run.populate_boss_gates();
        run.enter(0, None);
        run
    }

    pub(crate) fn active(&self) -> bool {
        matches!(self.phase, Phase::Fighting | Phase::Exploring)
    }

    pub(crate) fn floor(&self) -> u32 {
        self.dungeon.depth
    }

    pub(crate) fn room(&self) -> &Room {
        &self.dungeon.rooms[self.at]
    }

    /// Doors stay barred while anything in the room still lives.
    pub(crate) fn barred(&self) -> bool {
        self.foes_left() || (self.phase == Phase::Fighting && self.waves.pending())
    }

    /// Something in the room still fights (the yard's quintains never do).
    pub(crate) fn foes_left(&self) -> bool {
        self.enemies.iter().any(|e| e.kind != EnemyKind::Dummy)
    }

    /// The party still stands in the Undercroft or the first floor's
    /// entrance room.
    pub(crate) fn at_entrance(&self) -> bool {
        self.dungeon.depth == 0 || (self.dungeon.depth == 1 && self.at == 0)
    }

    /// The last pickup, for a few seconds after it was taken.
    pub(crate) fn found_line(&self) -> Option<String> {
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
    pub(crate) fn retreat(&mut self) {
        if self.active() {
            self.bank_all(false, "the party turned back");
        }
    }

    /// Add a card to this run's book and drop one at knight `id`'s feet.
    pub(crate) fn add_card(&mut self, card: Card, id: u32) -> bool {
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
    pub(crate) fn join(&mut self, name: &str) {
        self.join_seat(2, name);
    }

    /// Knight `id` (2 to 4) joins beside player 1, or takes a new name if
    /// already here.
    pub(crate) fn join_seat(&mut self, id: u32, name: &str) {
        if !(2..=MAX_PLAYERS).contains(&id) {
            return;
        }
        if let Some(hero) = self.players.get_mut(&id) {
            hero.name = Hero::new(name, 0.0, 0.0).name;
            return;
        }
        let (x, y) = self
            .players
            .get(&1)
            .map_or((WIDTH / 2.0, HEIGHT - 6.0), |h| (h.x, h.y));
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let (x, y) = [(2.4, 0.0), (-2.4, 0.0), (0.0, 2.4), (0.0, -2.4)]
            .into_iter()
            .map(|(dx, dy)| (x + dx, y + dy))
            .find(|&(x, y)| grid.clear(x, y, HERO_RADIUS, Mover::Hero))
            .unwrap_or((x, y));
        self.players.insert(id, Hero::new(name, x, y));
        self.rescale(self.players.len() as u32 - 1);
        self.deal_home(id);
        if !self.at_home_now() {
            self.harden();
        }
        if self.players.len() >= 4 {
            self.notice("party_of_four");
        }
    }

    /// A knight leaves the party; what they dropped is free to take again.
    pub(crate) fn leave(&mut self, id: u32) {
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
        self.rescale(self.players.len() as u32 + 1);
    }

    /// Monster health follows the party size, keeping each one's share of
    /// damage already dealt.
    fn rescale(&mut self, before: u32) {
        let now = (self.players.len() as u32).max(1);
        let before = before.max(1);
        for enemy in &mut self.enemies {
            enemy.max_hp = enemy.base_hp() * now;
            enemy.hp = (enemy.hp * now).div_ceil(before).clamp(1, enemy.max_hp);
        }
    }

    /// Walk the party into room `index` through its door `from` (the side
    /// they enter on), or onto a new floor's entrance with `None`.
    fn enter(&mut self, index: usize, from: Option<usize>) {
        // Leaving/recreating a room discards its extra bodies, never refunds
        // provision. Active serialized bodies instead keep their journal IDs.
        if let Some(g) = &mut self.boss_gates {
            for e in &self.enemies {
                g.economy.retire(e.id);
            }
        }
        self.at = index;
        self.projectiles.clear();
        self.enemies.clear();
        self.strikes.clear();
        self.spheres.clear();
        self.phantoms.clear();
        self.kegs.clear();
        self.lobs.clear();
        self.hooks.clear();
        self.webs.clear();
        self.slams.clear();
        self.ravages.clear();
        self.holes.clear();
        self.slimes_slain = 0;
        if self.dungeon.rooms[index].kind != RoomKind::Lair {
            self.light = None;
        }
        let living: Vec<u32> = self.players.keys().copied().collect();
        let count = living.len() as f32;
        let (w, h) = (
            self.dungeon.rooms[index].width(),
            self.dungeon.rooms[index].height(),
        );
        for (slot, id) in living.into_iter().enumerate() {
            let offset = (slot as f32 - (count - 1.0) / 2.0) * 2.4;
            let (x, y) = match from {
                Some(0) => (w / 2.0 + offset, 3.3),
                Some(1) => (w - 3.3, h / 2.0 + offset),
                Some(2) => (w / 2.0 + offset, h - 3.3),
                Some(_) => (3.3, h / 2.0 + offset),
                // Home: on the rug below the Winding Stair's mouth.
                // Home: at the foot of the rug, a walk from the stair.
                None if self.dungeon.depth == 0 => (w / 2.0 + offset * 1.4, h - 4.5),
                None => (w / 2.0 + offset * 2.0, h - 6.0),
            };
            let hero = self.players.get_mut(&id).expect("listed hero");
            (hero.x, hero.y) = (x, y);
            hero.invulnerable = hero.invulnerable.max(20);
            hero.dash_ticks = 0;
            hero.privy = 0;
        }
        if self.dungeon.rooms[index].kind == RoomKind::Ledge {
            self.arrive_side_on();
        }
        self.follow_fan_boxes();
        self.tallow_follows();
        self.hireling_follows();
        let sealed = self.guardian_locked();
        let first_visit = !self.dungeon.rooms[index].visited;
        let room = &mut self.dungeon.rooms[index];
        room.visited = true;
        if !room.cleared && !sealed {
            let roster = room.roster.clone();
            let guarded = room.kind == RoomKind::Stairs;
            let kind = room.kind;
            let pack = self.dungeon.pack;
            // A delve meets one of its pack's guardians, the same one for
            // the whole delve.
            let guardians: Vec<usize> = (0..self.bosses.len())
                .filter(|&i| self.bosses[i].only_in == pack)
                .collect();
            let pick = (mix(self.seed ^ self.raid_id.rotate_left(17))
                % guardians.len().max(1) as u64) as usize;
            if self
                .boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(index).is_some())
            {
                let (leader, apex, defeated, gate) = {
                    let g = self.boss_gates.as_ref().unwrap();
                    (
                        g.leader(index).filter(|l| !l.defeated).map(|l| l.boss),
                        g.guardian_boss,
                        g.guardian_defeated,
                        g.guardian == index,
                    )
                };
                let troops = self
                    .boss_gates
                    .as_mut()
                    .unwrap()
                    .economy
                    .deploy(index)
                    .unwrap();
                let faction = self
                    .boss_gates
                    .as_ref()
                    .unwrap()
                    .economy
                    .camp(index)
                    .unwrap()
                    .faction;
                for (slot, kind, troop_faction) in troops {
                    self.spawn(kind);
                    if let Some(e) = self.enemies.last_mut() {
                        e.camp_slot = Some(slot);
                        e.faction = Some(troop_faction);
                    }
                }
                if let Some(boss) = leader {
                    self.spawn_boss(usize::from(boss));
                    self.enemies.last_mut().unwrap().faction = Some(faction);
                } else if gate && !defeated {
                    if kind == RoomKind::Lair {
                        self.spawn(EnemyKind::Dragon);
                        self.enemies.last_mut().unwrap().faction = Some(faction);
                    } else if let Some(boss) = apex {
                        self.spawn_boss(usize::from(boss));
                        self.enemies.last_mut().unwrap().faction = Some(faction);
                    }
                }
            } else if guarded && let Some(&boss) = guardians.get(pick) {
                self.spawn_boss(boss);
                // A guardian keeps a lighter escort.
                for kind in roster.into_iter().take(2) {
                    self.spawn(kind);
                }
            } else {
                for kind in roster {
                    if kind == EnemyKind::PitTyrant
                        && self
                            .boss_gates
                            .as_ref()
                            .is_some_and(|g| g.optional_pit == Some(index) && g.pit_defeated)
                    {
                        continue;
                    }
                    if kind != EnemyKind::Dragon
                        || !self
                            .boss_gates
                            .as_ref()
                            .is_some_and(|g| g.guardian == index && g.guardian_defeated)
                    {
                        self.spawn(kind);
                    }
                }
            }
        }
        if !self.enemies.is_empty() && (self.boss_gates.is_none() || first_visit) {
            self.maybe_goblin();
        }
        if let Some(caged) = self
            .captive
            .as_ref()
            .filter(|c| c.room == index && c.freed.is_none())
        {
            self.cues.push(format!("caged:{}", caged.who));
        }
        if self.dungeon.rooms[index].kind == RoomKind::Yard {
            self.raise_quintains();
        }
        self.phase = if self.foes_left()
            || (!sealed
                && !self.room().cleared
                && self.boss_gates.as_ref().is_some_and(|g| {
                    g.economy.camp(index).is_some() || g.optional_pit == Some(index)
                })) {
            Phase::Fighting
        } else {
            Phase::Exploring
        };
        self.room_hurt = false;
        if self.phase == Phase::Fighting {
            self.sounds.push("door_seal");
            // A fight closes the window: wishes come between fights.
            self.window = None;
        }
        if let Some(boss) = self.enemies.iter().find_map(|e| e.boss) {
            let id = self.bosses[usize::from(boss)].id.clone();
            self.cues.push(format!("boss_rise:{id}"));
            self.sounds.push("boss_rise");
        } else if self.enemies.iter().any(|e| e.kind == EnemyKind::Dragon) {
            self.cues.push("lair".into());
        } else if self.enemies.iter().any(|e| e.kind == EnemyKind::PitTyrant) {
            self.cues.push("pit_rise".into());
            self.sounds.push("boss_rise");
        } else if self.phase == Phase::Fighting && self.room().great() {
            self.cues.push("great_hall".into());
        } else if self.phase == Phase::Fighting {
            self.cues.push("room_fight".into());
        }
        if sealed {
            self.rune = None;
            self.window = None;
            self.waves = Waves::default();
            self.traps.clear();
            self.rocks.clear();
            self.light = None;
            self.show_boss_lock();
        } else {
            self.place_rune();
            self.arm_room();
        }
        self.notice_crack(first_visit);
        if self.room().kind == RoomKind::Sanctuary {
            self.hallow();
        }
    }

    /// Where knight `id`'s morningstars are this tick (top-down: a circle;
    /// side-on too).
    pub(crate) fn orbits(&self, id: u32) -> Vec<(f32, f32)> {
        let Some(hero) = self.players.get(&id).filter(|h| h.hp > 0 && !h.stone) else {
            return Vec::new();
        };
        let n = hero.bonus.orbit.min(3);
        (0..n)
            .map(|k| {
                let a = self.tick as f32 * ORBIT_SPIN * DT
                    + k as f32 * std::f32::consts::TAU / n as f32
                    + id as f32;
                (
                    hero.x + a.cos() * ORBIT_RADIUS,
                    hero.y + a.sin() * ORBIT_RADIUS,
                )
            })
            .collect()
    }

    /// Morningstars strike what they touch, each tick they touch it, and
    /// knock monsters' shots out of the air.
    fn swing_orbits(&mut self) {
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let balls = self.orbits(id);
            if balls.is_empty() {
                continue;
            }
            let damage = self.players[&id].scaled(ORBIT_DAMAGE);
            for (bx, by) in balls {
                for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                    if (enemy.x - bx).hypot(enemy.y - by) <= enemy.radius() + 0.8 {
                        enemy.hp = enemy.hp.saturating_sub(damage);
                        if self.tick.is_multiple_of(4) {
                            self.sounds.push("hit");
                        }
                    }
                }
                for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                    if (shot.x - bx).hypot(shot.y - by) <= 0.9 {
                        shot.ttl = 0;
                    }
                }
            }
        }
    }

    /// Chain hits spark on: each to the nearest monsters within reach that
    /// it has not touched, for half its damage.
    fn spark(&mut self, chains: Vec<(u32, f32, f32, u32, u8)>) {
        for (first, x, y, damage, links) in chains {
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
                enemy.hp = enemy.hp.saturating_sub(damage.max(1));
                touched.push(enemy.id);
                if self.sparks.len() < 32 {
                    self.sparks.push(Spark {
                        from: at,
                        to: (enemy.x, enemy.y),
                        ttl: 6,
                    });
                }
                at = (enemy.x, enemy.y);
            }
        }
    }

    /// A knight at a Sanctuary privy's door steps in, once a Sanctuary.
    fn visit_privy(&mut self) {
        let Some((px, py)) = self.room().privy() else {
            return;
        };
        if self.phase != Phase::Exploring {
            return;
        }
        let (depth, here) = (self.dungeon.depth, self.at);
        let mut went = false;
        for hero in self.players.values_mut() {
            if hero.hp > 0
                && !hero.stone
                && hero.privy == 0
                && hero.relieved != Some((depth, here))
                && (hero.x - px).hypot(hero.y - py) < PRIVY_REACH
            {
                hero.privy = PRIVY_TICKS;
                hero.dash_ticks = 0;
                went = true;
            }
        }
        if went {
            self.sounds.push("door_seal");
            self.cues.push("privy".into());
        }
    }

    /// A quiet room: wounds close a little, mana fills, and the altar's
    /// sign says what it is for.
    fn hallow(&mut self) {
        for hero in self.players.values_mut() {
            if hero.hp > 0 {
                hero.hp = (hero.hp + hero.max_hp * 3 / 10).min(hero.max_hp);
            }
            hero.mana = MAX_MANA;
        }
        self.hallowed = Some(self.tick);
        self.cues.push("sanctuary".into());
        // A Sanctuary is an Overclass window too: a known wish each.
        self.window = Some(overclass::Window {
            opened: self.tick,
            wished: Vec::new(),
        });
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
        let room = &self.dungeon.rooms[self.at];
        let (x, y) = if kind == EnemyKind::Dragon {
            (room.width() / 2.0, 7.0)
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
                if room.tile(col, row) == Tile::Floor && far && !taken {
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
        let base = if giant { kind.hp() * 9 / 5 } else { kind.hp() };
        let hp = base * self.players.len().max(1) as u32;
        self.next_id = self.next_id.checked_add(1)?;
        self.enemies.push(Enemy {
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
            base_hp: giant.then_some(base),
            raged: false,
            frozen: 0,
            stage: 0,
            timer: 0,
            dir: (0.0, 0.0),
        });
        Some(self.next_id)
    }

    /// The floor's guardian takes the top of the stairs room.
    fn spawn_boss(&mut self, index: usize) {
        if self.boss_gates.is_some() && self.enemies.len() >= boss_ecology::ENTITY_CAP {
            return;
        }
        let boss = &self.bosses[index];
        let hp = boss.hp * self.players.len().max(1) as u32;
        let room = self.room();
        let (mut x, mut y) = (room.width() / 2.0, 7.0);
        // Threshold scenery and chamber pillars must not bury an anchor boss.
        let col = (x / TILE_UNITS) as i32;
        let row = (y / TILE_UNITS) as i32;
        let open = |c: i32, r: i32| {
            (-1..=1).all(|dc| (-1..=1).all(|dr| room.tile(c + dc, r + dr) == Tile::Floor))
        };
        if !open(col, row)
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
            base_hp: Some(boss.hp),
            raged: false,
            frozen: 0,
            stage: 0,
            timer: 0,
            dir: (0.0, 0.0),
        });
    }

    fn descend(&mut self) {
        let from_home = self.dungeon.depth == 0;
        // Guard the transition itself, not just the stair tile/UI.
        if !from_home
            && self
                .boss_gates
                .as_ref()
                .is_some_and(|g| !g.unlocked() || !self.dungeon.rooms[g.guardian].cleared)
        {
            if self.guardian_locked() {
                self.show_boss_lock();
            }
            return;
        }
        if !from_home && self.dungeon.depth >= DEEPEST {
            return;
        }
        let (depth, pack) = if from_home {
            // Down the Winding Stair: to the landing chosen, in the delve
            // chosen for the top.
            let depth = self.home.landing().clamp(1, DEEPEST - 1);
            self.first = self.dungeon.pack;
            (depth, Pack::at(depth, self.first))
        } else {
            self.settle_dare();
            if self.dungeon.depth != FLOORS {
                // A dragon's floor was reclaimed and banked when it fell.
                self.reclaimed.push(self.dungeon.pack);
                self.bank_all(true, &format!("floor {} cleared", self.dungeon.depth));
            }
            let depth = self.dungeon.depth + 1;
            let first = if self.dungeon.depth == 1 {
                self.dungeon.pack
            } else {
                self.first
            };
            self.first = first;
            (depth, Pack::at(depth, first))
        };
        if from_home {
            self.notice("first_delve");
        } else {
            if self.mode == fortune::Mode::GlassJaw {
                self.notice("fragile");
            }
            if self.collapse_at.is_some_and(|at| self.tick >= at) {
                self.notice("out_of_time");
            }
        }
        if depth == FLOORS + 1 {
            self.notice("below_the_bottom");
        }
        self.settlement_site = None;
        self.settlement_exhibits.clear();
        self.dungeon = layout::floor_for(self.mode, depth, pack, &mut self.rng);
        // A delve the King holds has its forge-hall off its first hall.
        barony::add_forge_hall(&mut self.dungeon, &self.home.barony);
        self.populate_boss_gates();
        self.cage_someone();
        self.pips_map();
        if from_home {
            self.kit_out();
            self.harden();
            self.deal_ults();
            self.bring_tallow();
            self.residents_help();
            self.runes_taken = 0;
            self.hexes = 0;
            self.drink_round();
            self.sing_song();
            self.hire();
            self.iron();
            self.cues.push("run_start".into());
        } else {
            self.refill_winds();
        }
        self.collapse_at = (self.mode == fortune::Mode::Collapse)
            .then(|| self.tick + u64::from(fortune::Mode::collapse_secs(depth) * HZ));
        self.cues.push(format!("descend:{depth}"));
        self.sounds.push("descend");
        if self.players.values().any(|h| h.hp == 0) {
            self.cues.push("revive".into());
        }
        for hero in self.players.values_mut() {
            // The stairs bring a fallen companion back to their feet.
            hero.hp = if hero.hp == 0 {
                hero.max_hp / 2
            } else {
                (hero.hp + 20).min(hero.max_hp)
            };
        }
        self.half_charge();
        self.enter(0, None);
        self.call_dare();
    }

    /// One fixed step of the run, and of the show watching it.
    /// Deterministic simulation: no clock, ambient randomness, network or
    /// model calls.
    pub(crate) fn step(&mut self, inputs: &BTreeMap<u32, Input>) {
        let (live, heard, sounded) = (self.active(), self.cues.len(), self.sounds.len());
        self.advance(inputs);
        if self.turbo() {
            self.advance(inputs);
        }
        if live {
            self.watch_dare(heard, sounded);
            self.mark_moments(heard);
            self.mark_trophies(heard);
            self.tick_audience(heard);
        }
    }

    fn advance(&mut self, inputs: &BTreeMap<u32, Input>) {
        if !self.active() {
            return;
        }
        // At the lists: the mount plate, and a bout's keys. A knight in the
        // saddle rides with them and nothing else this tick.
        let reined;
        let inputs = if self.at_home_now() && self.room().kind == RoomKind::Lists {
            self.tick_lists(inputs);
            match self.joust.as_ref().map(|j| j.knight) {
                Some(rider) => {
                    reined = joust::reins(inputs, rider);
                    &reined
                }
                None => inputs,
            }
        } else {
            inputs
        };
        self.tick += 1;
        self.shake = self.shake.saturating_sub(1);
        let before: Vec<(u32, u32)> = self.players.values().map(|h| (h.hp, h.max_hp)).collect();
        let mut shots = Vec::with_capacity(32);
        let mut bombs = 0;
        let mut nova = 0;
        // Where knights' bombs went off and draughts were drunk this tick,
        // for the small secrets that answer them.
        let mut booms: Vec<(f32, f32)> = Vec::new();
        let mut draughts: Vec<(f32, f32)> = Vec::new();
        // Damage each knight dealt this tick, for their ultimate's charge,
        // and the ultimates cast.
        let mut credits: Vec<(u32, u32)> = Vec::new();
        let mut casts: Vec<u32> = Vec::new();
        let side = self.side_on();
        let grid = Grid {
            room: &self.dungeon.rooms[self.at],
            barred: self.barred(),
        };
        let depth = self.dungeon.depth;
        let here = self.at;
        for (id, hero) in &mut self.players {
            let input = inputs
                .get(id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            // The vigil answers its key's press, from stone too, and holds
            // with no key at all: its player has stepped away.
            hero.vigil_rearm = hero.vigil_rearm.saturating_sub(1);
            if input.vigil && !hero.vigil_key && hero.vigil_rearm == 0 && hero.hp > 0 {
                hero.vigil = !hero.vigil;
                hero.stone = hero.vigil;
                hero.vigil_rearm = if hero.vigil {
                    VIGIL_SETTLE
                } else {
                    VIGIL_REARM
                };
                (hero.shielding, hero.walling, hero.dash_ticks) = (false, false, 0);
                self.sounds.push("wall_up");
            }
            hero.vigil_key = input.vigil;
            if hero.hp == 0 || hero.stone {
                continue;
            }
            // Hexed: a frog hops about, and does nothing else (aiming is
            // shooting, so not even that). Thrown by a Ravage: not even that.
            let input = if hero.tossed > 0 {
                Input::default()
            } else if hero.frog() {
                Input {
                    move_x: input.move_x,
                    move_y: input.move_y,
                    ..Input::default()
                }
            } else {
                input
            };
            if hero.privy > 0 {
                // Behind the privy's door: out of the world until done.
                hero.privy -= 1;
                if hero.privy == 0 {
                    hero.mana = MAX_MANA;
                    hero.hp = (hero.hp + PRIVY_MEND).min(hero.max_hp);
                    hero.relieved = Some((depth, here));
                    self.sounds.push("door_open");
                }
                continue;
            }
            let pressed = input.ult && !hero.ult_key;
            hero.ult_key = input.ult;
            if pressed
                && hero.ult_charge >= ults::ULT_FULL
                && hero.slashes == 0
                && hero.aiming.is_none()
            {
                casts.push(*id);
            }
            if hero.slashes > 0 {
                // Mid-Bladewind: the blade does the moving.
                continue;
            }
            hero.invulnerable = hero.invulnerable.saturating_sub(1);
            hero.dash_cooldown = hero.dash_cooldown.saturating_sub(1);
            hero.bomb_cooldown = hero.bomb_cooldown.saturating_sub(1);
            hero.fire_cooldown = hero.fire_cooldown.saturating_sub(1);
            hero.swing = hero.swing.saturating_sub(1);
            let (mx, my) = unit(input.move_x as f32, input.move_y as f32);
            let (ax, ay) = unit(input.aim_x as f32, input.aim_y as f32);
            if ax != 0.0 || ay != 0.0 {
                hero.aim_x = ax;
                hero.aim_y = ay;
            } else if side && mx != 0.0 {
                // Side-on, a knight faces the way they run.
                (hero.aim_x, hero.aim_y) = (mx.signum(), 0.0);
            }
            hero.shielding = false;
            match hero.guard {
                cards::Guard::Roll => {
                    if input.dash && hero.dash_cooldown == 0 && hero.dash_ticks == 0 {
                        // Roll where you're going, or where you aim when standing.
                        let (rx, ry) = if side {
                            // Side-on, a roll runs along the ground.
                            let along = if mx != 0.0 { mx } else { hero.aim_x };
                            (if along < 0.0 { -1.0 } else { 1.0 }, 0.0)
                        } else if mx != 0.0 || my != 0.0 {
                            (mx, my)
                        } else {
                            (hero.aim_x, hero.aim_y)
                        };
                        hero.dash_ticks = ROLL_TICKS;
                        hero.dash_x = rx;
                        hero.dash_y = ry;
                        hero.invulnerable = hero.invulnerable.max(ROLL_TICKS + 1);
                        hero.dash_cooldown = ROLL_REARM;
                        self.sounds.push("roll");
                    }
                }
                cards::Guard::Shield => hero.shielding = input.dash,
                cards::Guard::Wall(wall) => {
                    // Held while there is mana; once spent, it must be let go
                    // and raised again.
                    if !input.dash {
                        hero.wall_spent = false;
                    }
                    let up = input.dash && !hero.wall_spent && hero.mana > 0.0;
                    if up {
                        if !hero.walling {
                            self.sounds.push("wall_up");
                        }
                        hero.mana = (hero.mana - wall.drain as f32 * DT).max(0.0);
                        hero.mana_rest = 0;
                        if hero.mana <= 0.0 {
                            hero.wall_spent = true;
                            self.sounds.push("mana_empty");
                        }
                    }
                    hero.walling = up && hero.mana > 0.0;
                }
            }
            if !hero.walling {
                hero.mana_rest = hero.mana_rest.saturating_add(1);
                if hero.mana_rest > MANA_REST {
                    hero.mana = (hero.mana + MANA_FLOW * DT).min(MAX_MANA);
                }
            }
            // A Fae Dagger played last tick: there, now.
            if hero.blink > 0 {
                items::blink(hero, &grid);
                self.sounds.push("roll");
            }
            let pace = HERO_SPEED * (100 + hero.bonus.speed) as f32 / 100.0;
            let pace = if hero.immune == 0 && Run::webbed(&self.webs, hero.x, hero.y) {
                pace * hunters::WEB_PACE
            } else {
                pace
            };
            let pace = if hero.has_rune(runes::RuneKind::Haste) {
                pace * runes::HASTE
            } else {
                pace
            };
            let pace = if hero.frog() {
                pace * hexer::FROG_PACE
            } else {
                pace
            };
            let pace = if hero.chilled > 0 {
                pace * lich::CHILL_PACE
            } else {
                pace
            };
            // Sir Dinadan's Lay of Haste, to a knight who hears it.
            let pace = if hero.singing && self.song.as_deref() == Some("haste") {
                pace * tavern::SONG_HASTE
            } else {
                pace
            };
            let (dx, dy, speed) = if hero.dash_ticks > 0 {
                hero.dash_ticks -= 1;
                // Fast out of the tuck, slowing as the knight comes up.
                let ease = 0.55 + 0.45 * hero.dash_ticks as f32 / ROLL_TICKS as f32;
                (hero.dash_x, hero.dash_y, ROLL_SPEED * ease)
            } else if hero.shielding {
                (mx, my, pace * SHIELD_PACE)
            } else if hero.walling {
                (mx, my, pace * WALL_PACE)
            } else {
                (mx, my, pace)
            };
            if side {
                let rolling = hero.dash_ticks > 0;
                if ledge::walk(hero, &input, &grid, dx * speed * DT, rolling) {
                    self.sounds.push("hero_hurt");
                }
            } else {
                (hero.x, hero.y) = grid.slide(
                    (hero.x, hero.y),
                    (dx * speed * DT, dy * speed * DT),
                    HERO_RADIUS,
                    Mover::Hero,
                );
            }
            for timer in &mut hero.spell_cooldowns {
                *timer = timer.saturating_sub(1);
            }
            if let Some(slot) = usize::from(input.cast)
                .checked_sub(1)
                .filter(|s| *s < cards::SPELL_SLOTS)
                && hero.spell_cooldowns[slot] == 0
                && let Some(card) = hero.spells[slot]
                    .as_deref()
                    .and_then(|id| self.book.get(id))
                    .filter(|c| c.kind == cards::Kind::Spell)
            {
                hero.spell_cooldowns[slot] = card
                    .cooldown
                    .clamp(cards::SPELL_COOLDOWN.0, cards::SPELL_COOLDOWN.1)
                    * HZ;
                hero.spend(card, &mut self.score, &mut nova);
                for effect in &card.effects {
                    if let cards::Effect::Cast(shape) = effect {
                        hero.spell_shots(*shape, *id, &mut shots);
                    }
                }
                self.found = Some((self.tick, *id, format!("cast {}", card.name)));
                self.sounds.push("play_card");
            }
            hero.play_cooldown = hero.play_cooldown.saturating_sub(1);
            let slot = usize::from(input.play);
            if slot >= 1 && slot <= hero.hand.len() && hero.play_cooldown == 0 {
                hero.play_cooldown = PLAY_REARM;
                let summon = self.book.get(&hero.hand[slot - 1]).and_then(Card::summon);
                // A minion takes the ally slot from another minion, never
                // from Beaumains: the card stays in hand.
                let beaumains = self
                    .hireling
                    .as_ref()
                    .is_some_and(|h| h.kind == hireling::AllyKind::Beaumains);
                if summon.is_some() && beaumains {
                    self.found = Some((
                        self.tick,
                        *id,
                        "Beaumains will not share the road with it".into(),
                    ));
                } else {
                    let id_card = hero.hand.remove(slot - 1);
                    if let Some(card) = self.book.get(&id_card) {
                        hero.spend(card, &mut self.score, &mut nova);
                        if let Some((kind, power)) = summon {
                            let at = hireling::beside(&grid, (hero.x, hero.y));
                            self.hireling =
                                Some(hireling::Hireling::minion(kind, power, depth, at, *id));
                            self.cues.push(format!("summoned:{}", kind.word()));
                        }
                        if card
                            .effects
                            .iter()
                            .any(|e| matches!(e, cards::Effect::Heal(_)))
                        {
                            draughts.push((hero.x, hero.y));
                        }
                        self.found = Some((self.tick, *id, format!("played {}", card.name)));
                        self.sounds.push("play_card");
                        // The Herald calls the items by name.
                        for effect in &card.effects {
                            match effect {
                                cards::Effect::Blink(_) => self.cues.push("blink".into()),
                                cards::Effect::Immune(_) => self.cues.push("sceptre".into()),
                                _ => {}
                            }
                        }
                    }
                }
            }
            if input.bomb && hero.bombs > 0 && hero.bomb_cooldown == 0 {
                hero.bombs -= 1;
                hero.bomb_cooldown = BOMB_REARM;
                hero.invulnerable = hero.invulnerable.max(HZ / 2);
                bombs += 1;
                booms.push((hero.x, hero.y));
                self.shake = self.shake.max(8);
                self.sounds.push("bomb");
            }
            hero.sword_cooldown = hero.sword_cooldown.saturating_sub(1);
            if input.swing && hero.sword_cooldown == 0 && hero.dash_ticks == 0 {
                hero.sword_cooldown = hero.cooldown(SWORD_REARM);
                hero.swing = 8;
                hero.sword = true;
                self.sounds.push("swing");
                let angle = hero.aim_y.atan2(hero.aim_x);
                let half = SWORD_ARC.to_radians() / 2.0;
                let within = |dx: f32, dy: f32, reach: f32| {
                    let delta = (dy.atan2(dx) - angle)
                        .sin()
                        .atan2((dy.atan2(dx) - angle).cos())
                        .abs();
                    dx.hypot(dy) <= reach && delta <= half
                };
                let damage = hero.scaled(SWORD_DAMAGE);
                for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                    let (dx, dy) = (enemy.x - hero.x, enemy.y - hero.y);
                    if within(dx, dy, SWORD_REACH + enemy.radius()) {
                        enemy.hp = enemy.hp.saturating_sub(damage);
                        credits.push((*id, damage));
                        self.sounds.push("hit");
                    }
                }
                // The blade parries: hostile shots in its arc are cut down.
                for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                    if within(shot.x - hero.x, shot.y - hero.y, SWORD_REACH + 0.6) {
                        shot.ttl = 0;
                    }
                }
            }
            let guarded = hero.dash_ticks > 0 || hero.shielding || hero.walling;
            if (input.fire || ax != 0.0 || ay != 0.0) && hero.fire_cooldown == 0 && !guarded {
                if let Some(weapon) = &hero.forged {
                    let target = self.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                        ((a.x - hero.x).hypot(a.y - hero.y))
                            .total_cmp(&(b.x - hero.x).hypot(b.y - hero.y))
                    });
                    let melee = weapon.melee.filter(|m| {
                        (weapon.bolt.is_none() && weapon.throw.is_none())
                            || target.is_some_and(|e| {
                                (e.x - hero.x).hypot(e.y - hero.y) <= m.reach + e.radius()
                            })
                    });
                    if let Some(melee) = melee {
                        if let Some(enemy) = target.filter(|e| {
                            (e.x - hero.x).hypot(e.y - hero.y) <= melee.reach + e.radius()
                        }) {
                            (hero.aim_x, hero.aim_y) = unit(enemy.x - hero.x, enemy.y - hero.y);
                        }
                        hero.fire_cooldown = hero.cooldown(melee.every);
                        let damage = hero.scaled(melee.damage);
                        hero.swing = 6;
                        hero.sword = false;
                        self.sounds.push("swing");
                        let angle = hero.aim_y.atan2(hero.aim_x);
                        for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0) {
                            let dx = enemy.x - hero.x;
                            let dy = enemy.y - hero.y;
                            let delta = (dy.atan2(dx) - angle)
                                .sin()
                                .atan2((dy.atan2(dx) - angle).cos())
                                .abs();
                            if dx.hypot(dy) <= melee.reach + enemy.radius()
                                && delta <= melee.arc.to_radians() / 2.0
                                && grid.open(
                                    (hero.x + enemy.x) / 2.0,
                                    (hero.y + enemy.y) / 2.0,
                                    Mover::Hero,
                                )
                            {
                                enemy.hp = enemy.hp.saturating_sub(damage);
                                credits.push((*id, damage));
                            }
                        }
                        // The blade parries: shots in its sweep are cut down.
                        for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
                            let (dx, dy) = (shot.x - hero.x, shot.y - hero.y);
                            let delta = (dy.atan2(dx) - angle)
                                .sin()
                                .atan2((dy.atan2(dx) - angle).cos())
                                .abs();
                            if dx.hypot(dy) <= melee.reach + 0.6
                                && delta <= melee.arc.to_radians() / 2.0
                            {
                                shot.ttl = 0;
                            }
                        }
                    } else if let Some(throw) = weapon.throw {
                        // The blade flies out along the aim and comes back.
                        hero.fire_cooldown = hero.cooldown(throw.every);
                        hero.swing = 6;
                        hero.sword = false;
                        self.sounds.push("swing");
                        let (ux, uy) = (hero.aim_x, hero.aim_y);
                        let out = (throw.range / BLADE_SPEED * HZ as f32).ceil() as u16;
                        shots.push(Projectile {
                            x: hero.x + ux * 0.6,
                            y: hero.y + uy * 0.6,
                            vx: ux * BLADE_SPEED,
                            vy: uy * BLADE_SPEED,
                            hostile: false,
                            kind: Shot::Blade,
                            look: None,
                            damage: hero.scaled(throw.damage),
                            pierce: u8::MAX,
                            last_hit: None,
                            empowered: false,
                            ttl: u32::from(out) * 2 + 3 * HZ,
                            traits: ShotTraits {
                                back: 1,
                                owner: *id as u8,
                                out,
                                ..ShotTraits::of(&hero.bonus)
                            },
                        });
                    } else if let Some(bolt) = weapon.bolt {
                        hero.fire_cooldown = hero.cooldown(bolt.every);
                        self.sounds.push("shot_bolt");
                        // (Hero::volley, by field: the forged weapon is borrowed.)
                        hero.loosed = hero.loosed.wrapping_add(1);
                        let volley = hero.bonus.volley > 0 && hero.loosed % hero.bonus.volley == 0;
                        let count = weapon.shots() + hero.bonus.shots + if volley { 2 } else { 0 };
                        let traits = ShotTraits {
                            owner: *id as u8,
                            ..ShotTraits::of(&hero.bonus)
                        };
                        let arc = weapon
                            .spread
                            .map_or(0.0, |spread| spread.arc.to_radians())
                            .max(if volley { VOLLEY_FAN } else { FAN } * (count - 1) as f32);
                        let angle = hero.aim_y.atan2(hero.aim_x);
                        for i in 0..count {
                            let theta = angle
                                + if count > 1 {
                                    arc * (i as f32 / (count - 1) as f32 - 0.5)
                                } else {
                                    0.0
                                };
                            shots.push(Projectile {
                                x: hero.x + theta.cos() * 0.6,
                                y: hero.y + theta.sin() * 0.6,
                                vx: theta.cos() * bolt.speed,
                                vy: theta.sin() * bolt.speed,
                                hostile: false,
                                kind: Shot::Bolt,
                                damage: hero.scaled(bolt.damage),
                                pierce: hero.bonus.pierce as u8,
                                last_hit: None,
                                empowered: false,
                                ttl: (bolt.range / bolt.speed * HZ as f32).ceil() as u32,
                                look: Some((weapon.colour.clone(), weapon.shape.clone())),
                                traits,
                            });
                        }
                    }
                } else {
                    let arms = hero.weapon.arms();
                    hero.fire_cooldown = hero.cooldown(arms.cooldown);
                    self.sounds.push(match hero.weapon {
                        Weapon::Bow => "shot_bow",
                        Weapon::Crossbow => "shot_crossbow",
                        Weapon::Handgonne => "shot_handgonne",
                    });
                    let volley = hero.volley();
                    let count = 1 + hero.bonus.shots + if volley { 2 } else { 0 };
                    let fan = if volley { VOLLEY_FAN } else { FAN };
                    let traits = ShotTraits {
                        owner: *id as u8,
                        ..ShotTraits::of(&hero.bonus)
                    };
                    let angle = hero.aim_y.atan2(hero.aim_x);
                    for i in 0..count {
                        let theta = angle + fan * (i as f32 - (count - 1) as f32 / 2.0);
                        let (ux, uy) = (theta.cos(), theta.sin());
                        shots.push(Projectile {
                            x: hero.x + ux * 0.6,
                            y: hero.y + uy * 0.6,
                            vx: ux * arms.speed,
                            vy: uy * arms.speed,
                            hostile: false,
                            kind: match hero.weapon {
                                Weapon::Bow => Shot::Arrow,
                                Weapon::Crossbow => Shot::Bolt,
                                Weapon::Handgonne => Shot::Ball,
                            },
                            look: None,
                            damage: hero.scaled(arms.damage),
                            pierce: arms.pierce + hero.bonus.pierce as u8,
                            last_hit: None,
                            empowered: false,
                            traits,
                            ttl: 4 * HZ,
                        });
                    }
                }
            }
        }
        if bombs > 0 {
            self.projectiles.retain(|p| !p.hostile);
        }
        let blast = BOMB_DAMAGE * bombs + nova;
        if blast > 0 {
            for enemy in &mut self.enemies {
                enemy.hp = enemy.hp.saturating_sub(blast);
            }
        }
        ults::phantom_shots(&self.phantoms, &self.players, &mut shots);
        let mut deeds = foes::Deeds::default();
        // Monsters see knights (not one under an Invisibility rune), and the
        // images Phantasm made of them.
        let living: Vec<(f32, f32)> = self
            .players
            .values()
            .filter(|hero| {
                hero.hp > 0 && !hero.stone && !hero.has_rune(runes::RuneKind::Invisibility)
            })
            .map(|hero| (hero.x, hero.y))
            .chain(self.phantoms.iter().map(|p| (p.x, p.y)))
            .chain(
                self.hireling
                    .iter()
                    .filter(|h| !h.down())
                    .map(|h| (h.x, h.y)),
            )
            .collect();
        for enemy in &mut self.enemies {
            if enemy.hp == 0 {
                continue;
            }
            if enemy.frozen > 0 {
                // Stunned, or held in a Stillhour: nothing moves.
                enemy.frozen -= 1;
                continue;
            }
            enemy.age += 1;
            if enemy.age < TELEGRAPH {
                continue;
            }
            let rest = enemy.y;
            monsters::act(
                enemy,
                &living,
                &grid,
                self.seed,
                &self.bosses,
                &mut shots,
                &mut deeds,
            );
            if side && !enemy.kind.flies() {
                // Side-on, walkers walk; the ground decides their height.
                enemy.y = rest;
                ledge::fall(enemy, &grid);
            }
        }
        self.projectiles.extend(
            shots
                .into_iter()
                .take(MAX_PROJECTILES.saturating_sub(self.projectiles.len())),
        );
        // Swept segments prevent fast shots tunnelling through small targets.
        let walls: Vec<((f32, f32), (f32, f32), u32)> = self
            .players
            .values()
            .filter(|h| h.walling && h.hp > 0 && !h.stone)
            .filter_map(|h| h.wall_span().map(|(a, b)| (a, b, h.wall_empower())))
            .collect();
        let mut chains: Vec<(u32, f32, f32, u32, u8)> = Vec::new();
        let mut bursts: Vec<(u32, f32, f32, u32)> = Vec::new();
        let mut mends: u32 = 0;
        let mut hexed: Vec<u32> = Vec::new();
        // Who Rimeleap can leap to: the knights standing.
        let standing: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let mut leaps = 0u32;
        let hands: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let spheres = self.spheres.clone();
        let mut overkill = false;
        for bullet in &mut self.projectiles {
            // Time stands still in a Stillhour for the monsters' shots.
            if bullet.hostile
                && spheres
                    .iter()
                    .any(|s| (bullet.x - s.x).hypot(bullet.y - s.y) < s.r)
            {
                continue;
            }
            let old_x = bullet.x;
            let old_y = bullet.y;
            // A thrown blade: out for its range, then home to the hand.
            match bullet.traits.back {
                1 => {
                    bullet.traits.out = bullet.traits.out.saturating_sub(1);
                    if bullet.traits.out == 0 {
                        bullet.traits.back = 2;
                        bullet.last_hit = None;
                    }
                }
                2 => match hands.iter().find(|h| h.0 == u32::from(bullet.traits.owner)) {
                    Some(&(_, hx, hy)) => {
                        let (dx, dy) = (hx - bullet.x, hy - bullet.y);
                        let d = dx.hypot(dy);
                        if d < 1.2 {
                            bullet.ttl = 0;
                            continue;
                        }
                        (bullet.vx, bullet.vy) = (dx / d * BLADE_SPEED, dy / d * BLADE_SPEED);
                    }
                    None => bullet.ttl = 0,
                },
                _ => {}
            }
            // A homing shot turns toward the nearest monster, a little a tick.
            if !bullet.hostile
                && bullet.traits.homing > 0
                && let Some(target) = self.enemies.iter().filter(|e| e.hp > 0).min_by(|a, b| {
                    (a.x - bullet.x)
                        .hypot(a.y - bullet.y)
                        .total_cmp(&(b.x - bullet.x).hypot(b.y - bullet.y))
                })
            {
                let speed = bullet.vx.hypot(bullet.vy);
                let now = bullet.vy.atan2(bullet.vx);
                let want = (target.y - bullet.y).atan2(target.x - bullet.x);
                let turn = (want - now).sin().atan2((want - now).cos());
                let most = HOMING_TURN * f32::from(bullet.traits.homing) * DT;
                let next = now + turn.clamp(-most, most);
                (bullet.vx, bullet.vy) = (next.cos() * speed, next.sin() * speed);
            }
            bullet.x += bullet.vx * DT;
            bullet.y += bullet.vy * DT;
            bullet.ttl = bullet.ttl.saturating_sub(1);
            // A wall stops what monsters throw and lets friends' shots
            // through, stronger.
            if let Some(&(_, _, empower)) = walls
                .iter()
                .find(|&&(a, b, _)| crosses((old_x, old_y), (bullet.x, bullet.y), a, b))
            {
                if bullet.hostile {
                    bullet.ttl = 0;
                    self.sounds.push("shield_block");
                    continue;
                } else if !bullet.empowered {
                    bullet.empowered = true;
                    bullet.damage = bullet.damage * (100 + empower) / 100;
                }
            }
            if bullet.hostile
                && let Some(phantom) = self.phantoms.iter_mut().find(|p| {
                    p.left > 0 && hit_segment(old_x, old_y, bullet.x, bullet.y, p.x, p.y, 0.6)
                })
            {
                // An image takes the shot meant for its knight, and is gone.
                phantom.left = 0;
                bullet.ttl = 0;
                continue;
            }
            if bullet.hostile {
                for (&id, hero) in self
                    .players
                    .iter_mut()
                    .filter(|(_, hero)| hero.hp > 0 && !hero.stone)
                {
                    // Rimeleap never strikes the knight it just left.
                    if bullet.kind == Shot::Frost && bullet.last_hit == Some(id) {
                        continue;
                    }
                    if hit_segment(
                        old_x,
                        old_y,
                        bullet.x,
                        bullet.y,
                        hero.x,
                        hero.y,
                        HERO_RADIUS + 0.16,
                    ) {
                        bullet.ttl = 0;
                        // A raised shield turns what comes at its face.
                        let speed = bullet.vx.hypot(bullet.vy).max(0.001);
                        let facing = -(bullet.vx * hero.aim_x + bullet.vy * hero.aim_y) / speed;
                        if hero.shielding && facing >= SHIELD_ARC.to_radians().cos() {
                            self.sounds.push("shield_block");
                        } else if hero.invulnerable == 0 && bullet.kind == Shot::Hex {
                            hexed.push(id);
                        } else if bullet.kind == Shot::Frost {
                            if hero.invulnerable == 0 {
                                hero.hurt(bullet.damage);
                                self.sounds.push("hero_hurt");
                            }
                            if hero.immune == 0 {
                                hero.chilled = lich::CHILL;
                            }
                            // Rimeleap leaps on to the nearest other knight.
                            if bullet.pierce > 0
                                && let Some((next, nx, ny)) =
                                    lich::next_link(&standing, id, (hero.x, hero.y))
                            {
                                let speed = bullet.vx.hypot(bullet.vy).max(1.0);
                                let (ux, uy) = unit(nx - hero.x, ny - hero.y);
                                (bullet.x, bullet.y) = (hero.x, hero.y);
                                (bullet.vx, bullet.vy) = (ux * speed, uy * speed);
                                bullet.pierce -= 1;
                                bullet.last_hit = Some(id);
                                bullet.ttl = 4 * HZ;
                                let _ = next;
                                leaps += 1;
                            }
                        } else if hero.invulnerable == 0 {
                            hero.hurt(bullet.damage);
                            self.sounds.push("hero_hurt");
                        }
                        break;
                    }
                }
            } else {
                for enemy in self.enemies.iter_mut().filter(|enemy| enemy.hp > 0) {
                    if bullet.last_hit != Some(enemy.id)
                        && hit_segment(
                            old_x,
                            old_y,
                            bullet.x,
                            bullet.y,
                            enemy.x,
                            enemy.y,
                            enemy.radius(),
                        )
                    {
                        enemy.hp = enemy.hp.saturating_sub(bullet.damage);
                        bullet.last_hit = Some(enemy.id);
                        if bullet.traits.owner > 0 {
                            credits.push((u32::from(bullet.traits.owner), bullet.damage));
                        }
                        if bullet.damage >= 300 {
                            overkill = true;
                        }
                        if bullet.traits.stun > 0 {
                            enemy.frozen = enemy.frozen.max(u32::from(bullet.traits.stun));
                        }
                        self.sounds.push("hit");
                        if bullet.kind == Shot::Ball {
                            // A handgonne ball bursts: half to all around.
                            bursts.push((enemy.id, enemy.x, enemy.y, bullet.damage / 2));
                        } else if bullet.traits.burst > 0 {
                            let part = bullet.damage * u32::from(bullet.traits.burst) / 100;
                            bursts.push((enemy.id, enemy.x, enemy.y, part.max(1)));
                        }
                        if bullet.traits.chain > 0 {
                            chains.push((
                                enemy.id,
                                enemy.x,
                                enemy.y,
                                bullet.damage / 2,
                                bullet.traits.chain,
                            ));
                        }
                        mends += u32::from(bullet.traits.mend);
                        if bullet.pierce == 0 {
                            bullet.ttl = 0;
                        } else {
                            bullet.pierce -= 1;
                        }
                        break;
                    }
                }
            }
            if grid.stops_shot(bullet.x, bullet.y) {
                if bullet.traits.back == 1 {
                    // A thrown blade turns home off the stone; coming back
                    // it passes over everything.
                    (bullet.x, bullet.y) = (old_x, old_y);
                    bullet.traits.back = 2;
                    bullet.last_hit = None;
                } else if bullet.traits.back == 2 {
                } else if !bullet.hostile && bullet.traits.bounce > 0 && bullet.ttl > 0 {
                    // Off the stone it came against: across, along, or both
                    // in a corner. It may hit the same monster again.
                    let across = grid.stops_shot(bullet.x, old_y);
                    let along = grid.stops_shot(old_x, bullet.y);
                    if across || !along {
                        bullet.vx = -bullet.vx;
                    }
                    if along || !across {
                        bullet.vy = -bullet.vy;
                    }
                    (bullet.x, bullet.y) = (old_x, old_y);
                    bullet.traits.bounce -= 1;
                    bullet.last_hit = None;
                } else {
                    bullet.ttl = 0;
                }
            }
        }
        self.projectiles.retain(|p| p.ttl > 0);
        if overkill {
            self.notice("overkill");
        }
        self.phantoms.retain(|p| p.left > 0);
        self.cast_ults(casts);
        self.tick_ults();
        self.charge_ults(&credits);
        self.apply_deeds(deeds);
        self.shoot_kegs();
        self.tick_kegs_and_lobs();
        self.tick_hunters();
        self.tick_tallow();
        self.tick_cage(inputs);
        self.tick_runes();
        self.tick_hexes();
        self.tick_chill();
        self.tick_items();
        self.tick_song();
        self.tick_hireling();
        self.tick_snibbet();
        self.small_secrets(&booms, &draughts);
        self.spark(chains);
        for (hit, x, y, damage) in bursts {
            for enemy in self.enemies.iter_mut().filter(|e| e.hp > 0 && e.id != hit) {
                if (enemy.x - x).hypot(enemy.y - y) <= BURST_REACH + enemy.radius() {
                    enemy.hp = enemy.hp.saturating_sub(damage);
                }
            }
            self.shake = self.shake.max(4);
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
        }
        for blast in &mut self.blasts {
            blast.2 = blast.2.saturating_sub(1);
        }
        self.blasts.retain(|b| b.2 > 0);
        let now = self.tick;
        self.boons
            .retain(|b| now.saturating_sub(b.1) < u64::from(HZ));
        for id in hexed {
            self.hex(id);
        }
        if leaps > 0 {
            self.cues.push("frost_leap".into());
        }
        if bombs > 0 {
            // A knight's bomb fills the room: a cracked wall comes down.
            self.blast_wall(None);
        }
        if mends > 0 {
            // The most hurt knight standing is mended.
            if let Some(hero) = self
                .players
                .values_mut()
                .filter(|h| h.hp > 0 && !h.stone && h.hp < h.max_hp)
                .min_by_key(|h| h.hp * 100 / h.max_hp.max(1))
            {
                hero.hp = (hero.hp + mends).min(hero.max_hp);
            }
        }
        for spark in &mut self.sparks {
            spark.ttl = spark.ttl.saturating_sub(1);
        }
        self.sparks.retain(|s| s.ttl > 0);
        self.swing_orbits();
        self.projectiles.retain(|p| p.ttl > 0);
        self.visit_privy();
        if self.dungeon.depth == 0 {
            self.tick_home(inputs);
        }
        self.tick_barony(inputs);
        self.tick_traps();
        self.tick_collapse();
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            if self.enemies.iter().any(|e| {
                e.hp > 0
                    && e.age >= TELEGRAPH
                    && e.frozen == 0
                    && e.kind != EnemyKind::Dummy
                    && (hero.x - e.x).hypot(hero.y - e.y) < e.radius() + 0.5
            }) {
                hero.hurt(12);
            }
        }
        // A knight keeping vigil mends the living knights within its reach.
        if self.tick.is_multiple_of(u64::from(VIGIL_MEND_EVERY)) {
            let wards: Vec<(f32, f32)> = self
                .players
                .values()
                .filter(|h| h.vigil && h.hp > 0)
                .map(|h| (h.x, h.y))
                .collect();
            for hero in self.players.values_mut() {
                if hero.hp > 0
                    && hero.hp < hero.max_hp
                    && !hero.vigil
                    && wards
                        .iter()
                        .any(|&(x, y)| (hero.x - x).hypot(hero.y - y) < VIGIL_REACH)
                {
                    hero.hp += 1;
                }
            }
        }
        self.second_winds();
        for ((was, max), hero) in before.into_iter().zip(self.players.values()) {
            if hero.hp < was {
                self.room_hurt = true;
                if hero.hp == 0 {
                    self.cues.push("knight_down".into());
                } else if was * 10 >= max * 3 && hero.hp * 10 < max * 3 {
                    self.cues.push("low_hp".into());
                }
            }
        }
        for enemy in self.enemies.iter_mut().filter(|e| !e.raged && e.hp > 0) {
            if let Some(boss) = enemy.boss.and_then(|i| self.bosses.get(usize::from(i)))
                && boss.rage > 0.0
                && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32
            {
                enemy.raged = true;
                self.cues.push(format!("boss_rage:{}", boss.id));
            }
        }
        self.settle_kills();
        self.gather();
        self.tick_waves();
        // A party falls when no knight is left to fight and one has fallen:
        // knights only turned to stone (their player away) never wipe it.
        if self.players.values().all(|h| h.hp == 0 || h.stone)
            && self.players.values().any(|h| h.hp == 0)
        {
            self.phase = Phase::Wiped;
            self.projectiles.clear();
            self.cues.push("wipe".into());
            self.bank_all(false, "the party fell");
            return;
        }
        if !self.foes_left()
            && self.phase == Phase::Fighting
            && !self.waves.pending()
            && self.boss_room_can_clear()
        {
            self.projectiles.clear();
            self.rocks.clear();
            self.ravages.clear();
            self.holes.clear();
            self.dungeon.rooms[self.at].cleared = true;
            // For King Brannoc's missions: a hall of a delve freed.
            if self.dungeon.depth > 0 {
                *self
                    .marks
                    .entry(barony::clear_mark(self.dungeon.pack))
                    .or_default() += 1;
            }
            self.fair_chest();
            // Two knights standing together earn the realm a bond each.
            let standing = self
                .players
                .values()
                .filter(|h| h.hp > 0 && !h.stone)
                .count();
            if standing >= 2 {
                for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
                    hero.carried.add(Spoil::Bond, 1);
                }
                self.cues.push("bond".into());
                self.notice("bonded");
            }
            if !self.room_hurt {
                self.notice("flawless");
            }
            self.cues.push(
                if self.room_hurt {
                    "room_clear"
                } else {
                    "flawless"
                }
                .into(),
            );
            self.sounds.push("door_open");
            // The first floor's guardian guarded a Sanctuary: every run
            // reaches a reforge before the descent.
            if self.room().kind == RoomKind::Stairs && self.dungeon.depth == 1 {
                self.dungeon.rooms[self.at].kind = RoomKind::Sanctuary;
                self.stairs_held = true;
                self.hallow();
            }
            if self.room().kind == RoomKind::Lair {
                // The dragon is slain: its floor reclaimed and banked, and
                // the way splits — the stairs into the deep, or the light home.
                if let Some(hero) = self.players.values_mut().find(|h| h.hp > 0 && !h.stone) {
                    hero.carried.add(Spoil::Scale, 1);
                }
                self.reclaimed.push(self.dungeon.pack);
                self.cues.push("victory".into());
                self.shake = self.shake.max(36);
                self.sounds.push("boss_fall");
                self.bank_all(true, "the dragon is slain");
                self.triumph = Some(Triumph::Dragon);
                self.notice("dragonslayer");
                let room = &mut self.dungeon.rooms[self.at];
                room.open_stairs();
                self.light = Some((room.width() / 2.0, room.height() / 2.0 - 4.0));
                self.stairs_held = true;
                self.cues.push("the_deep".into());
            }
            if self.room().kind == RoomKind::Threshold {
                // The Unknown met: the Grail, and home.
                self.phase = Phase::Won;
                for hero in self.players.values_mut().filter(|h| h.hp > 0) {
                    hero.carried.add(Spoil::Gem, 3);
                    hero.carried.add(Spoil::Scale, 1);
                }
                self.reclaimed.push(self.dungeon.pack);
                self.cues.push("grail".into());
                self.shake = self.shake.max(24);
                self.sounds.push("wheel_land");
                self.bank_all(true, "the Grail");
                self.triumph = Some(Triumph::Grail);
                self.notice("the_grail");
                return;
            }
            self.phase = Phase::Exploring;
            self.open_window();
        }
        if self.phase == Phase::Exploring {
            self.travel();
        }
    }

    fn settle_kills(&mut self) {
        let mut camp_deaths = Vec::new();
        let mut emission_deaths = Vec::new();
        let mut fallen: Vec<(EnemyKind, f32, f32)> = Vec::new();
        let mut guardians: Vec<(usize, f32, f32)> = Vec::new();
        let mut mimics = Vec::new();
        let mut foes_fallen = Vec::new();
        self.enemies.retain(|e| {
            let camp_duplicate = e.camp_slot.is_some_and(|slot| {
                camp_deaths.contains(&slot)
                    || self
                        .boss_gates
                        .as_ref()
                        .is_none_or(|g| !g.economy.alive_slot(self.at, slot, e.kind))
            });
            if e.hp == 0
                && !camp_duplicate
                && let Some(slot) = e.camp_slot
            {
                camp_deaths.push(slot);
            }
            let emission_duplicate = self.boss_gates.as_ref().is_some_and(|g| {
                e.camp_slot.is_none()
                    && !g.encounter(self.at, e.kind, e.boss)
                    && (emission_deaths.contains(&e.id)
                        || !g.economy.alive_emission(self.at, e.id, e.kind, e.stage))
            });
            if e.hp == 0
                && !emission_duplicate
                && e.camp_slot.is_none()
                && self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.economy.emission(e.id).is_some())
            {
                emission_deaths.push(e.id);
            }
            let duplicate = camp_duplicate
                || emission_duplicate
                || self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.encounter(self.at, e.kind, e.boss))
                    && (e
                        .boss
                        .is_some_and(|b| guardians.iter().any(|&(i, _, _)| i == usize::from(b)))
                        || (matches!(e.kind, EnemyKind::Dragon | EnemyKind::PitTyrant)
                            && fallen.iter().any(|&(k, _, _)| k == e.kind)));
            if e.hp == 0 && !duplicate && e.kind == EnemyKind::Mimic {
                mimics.push((e.x, e.y));
            }
            if e.hp == 0 && !duplicate && matches!(e.kind, EnemyKind::Slime | EnemyKind::Goblin) {
                foes_fallen.push((e.kind, e.x, e.y, e.stage));
            }
            if e.hp == 0
                && !duplicate
                && !self
                    .boss_gates
                    .as_ref()
                    .is_some_and(|g| g.already_defeated(self.at, e.kind, e.boss))
            {
                fallen.push((e.kind, e.x, e.y));
                if e.kind == EnemyKind::Boss
                    && let Some(boss) = e.boss
                {
                    guardians.push((usize::from(boss), e.x, e.y));
                }
            }
            e.hp > 0
        });
        if let Some(g) = &mut self.boss_gates {
            for slot in camp_deaths {
                g.economy.casualty(self.at, slot);
            }
            for id in emission_deaths {
                g.economy.retire(id);
            }
        }
        // Actual corpse settlement is the only source of leader-loss evidence.
        let was_open = self
            .boss_gates
            .as_ref()
            .is_some_and(boss_gates::Gates::unlocked);
        if let Some(g) = &mut self.boss_gates {
            for &(boss, _, _) in &guardians {
                let leader = g.leader(self.at).is_some();
                if g.record(self.at, EnemyKind::Boss, Some(boss as u8)) && leader {
                    self.cues
                        .push(format!("leader_fall:{}", self.bosses[boss].name));
                }
            }
            if fallen.iter().any(|&(kind, _, _)| kind == EnemyKind::Dragon) {
                g.record(self.at, EnemyKind::Dragon, None);
            }
            if fallen
                .iter()
                .any(|&(kind, _, _)| kind == EnemyKind::PitTyrant)
            {
                g.record(self.at, EnemyKind::PitTyrant, None);
            }
            if !was_open && g.unlocked() {
                self.cues.push("boss_unsealed".into());
            }
        }
        // A mimic coughs up the treasure it was pretending to be.
        for (x, y) in mimics {
            let pack = self.dungeon.pack;
            let loot = self.book.roll_chest(&mut self.rng, pack);
            for (i, card) in loot.into_iter().enumerate() {
                let angle = std::f32::consts::FRAC_PI_2 + (i as f32 - 1.0) * 1.1;
                let bottom = self.room().height() - 3.0;
                self.drop_item(
                    card,
                    x + angle.cos() * 2.6,
                    (y + angle.sin() * 2.2).clamp(3.0, bottom),
                    None,
                );
            }
            self.cues.push("mimic_fall".into());
        }
        // A guardian scatters its spoils as cards, plus one prize.
        for (index, x, y) in guardians {
            let Some(boss) = self.bosses.get(index).cloned() else {
                continue;
            };
            let mut drops: Vec<String> = Vec::new();
            for (&spoil, &n) in &boss.drops.0 {
                let copies = if spoil == Spoil::Gold {
                    n.div_ceil(50)
                } else {
                    n
                };
                drops.extend((0..copies).map(|_| spoil.word().to_string()));
            }
            let pack = self.dungeon.pack;
            drops.extend(
                self.book
                    .roll_chest(&mut self.rng, pack)
                    .into_iter()
                    .take(1),
            );
            let count = drops.len().max(1) as f32;
            for (i, card) in drops.into_iter().enumerate() {
                if self.book.get(&card).is_none() {
                    continue;
                }
                let angle = i as f32 / count * std::f32::consts::TAU;
                let (w, h) = (self.room().width(), self.room().height());
                let (cx, cy) = (
                    x + angle.cos() * 3.2,
                    (y + angle.sin() * 2.4).clamp(3.0, h - 3.0),
                );
                self.drop_item(card, cx.clamp(3.0, w - 3.0), cy, None);
            }
            self.found = Some((self.tick, 0, format!("{} falls", boss.name)));
            self.notice("bigger_they_are");
            self.shake = self.shake.max(24);
            self.sounds.push("boss_fall");
            self.cues.push(format!("boss_fall:{}", boss.id));
        }
        let slain = fallen.len() as u32;
        if slain > 0 {
            self.sounds.push("kill");
        }
        if slain > 0 {
            if !self.blooded {
                self.blooded = true;
                self.cues.push("first_blood".into());
            }
            let count = if self.tick.saturating_sub(self.streak.0) <= u64::from(2 * HZ) {
                self.streak.1 + slain
            } else {
                slain
            };
            self.streak = (self.tick, count);
            match count {
                2 => self.cues.push("slay2".into()),
                3 => self.cues.push("slay3".into()),
                n if n >= 4 && n - slain < 4 => self.cues.push("slay4".into()),
                _ => {}
            }
        }
        let red = self.song.as_deref() == Some("red");
        for hero in self.players.values_mut().filter(|h| h.hp > 0 && !h.stone) {
            // The Red Ballad mends a knight who hears it, kill by kill.
            let vamp = hero.bonus.vamp
                + if red && hero.singing {
                    tavern::SONG_VAMP
                } else {
                    0
                };
            hero.hp = (hero.hp + vamp * slain).min(hero.max_hp);
        }
        let pack = self.dungeon.pack;
        self.slimes_slain += foes_fallen
            .iter()
            .filter(|f| f.0 == EnemyKind::Slime)
            .count() as u32
            + fallen.iter().filter(|f| f.0 == EnemyKind::Slime).count() as u32
                * u32::from(foes_fallen.is_empty());
        if self.slimes_slain >= 7 {
            self.notice("splitting_headache");
        }
        self.fallen_foes(&foes_fallen);
        // The audience counts the fallen too: a little each, more for the
        // dangerous ones.
        let watched: u32 = fallen
            .iter()
            .map(|(kind, ..)| if kind.elite() { 8 } else { 2 })
            .sum();
        self.thrill(watched);
        for &(kind, ..) in &fallen {
            self.mark_kill(kind);
        }
        let kinds: Vec<EnemyKind> = fallen.iter().map(|&(kind, ..)| kind).collect();
        self.mark_experience(&kinds);
        self.pit_fallen(&fallen);
        let generous = if self.mode == fortune::Mode::GiantsFeast {
            2
        } else {
            1
        };
        for (kind, x, y) in fallen {
            self.score += kind.bounty() * generous;
            if self
                .rng
                .chance((loot::drop_chance(kind) * generous).min(100))
            {
                let card = if kind == EnemyKind::Dragon && self.book.get("heart").is_some() {
                    Some("heart".to_string())
                } else {
                    self.book.roll_drop(&mut self.rng, pack)
                };
                if let Some(card) = card {
                    self.drop_item(card, x, y, None);
                }
            }
            if self
                .rng
                .chance((loot::spoil_chance(kind) * generous).min(100))
                && let Some(card) = self.book.roll_spoil(&mut self.rng, pack)
            {
                self.drop_item(card, x + 0.9, y + 0.5, None);
            }
        }
    }

    fn drop_item(&mut self, card: String, x: f32, y: f32, held_off: Option<u32>) {
        let room = &mut self.dungeon.rooms[self.at];
        if room.items.len() < MAX_ITEMS {
            room.items.push(Item {
                card,
                x,
                y,
                held_off,
            });
        }
    }

    /// Knights pick up the cards they walk over and open chests they touch.
    fn gather(&mut self) {
        let touched = self.dungeon.rooms[self.at]
            .chest
            .filter(|c| !c.open)
            .is_some_and(|chest| {
                self.players
                    .values()
                    .any(|h| h.hp > 0 && !h.stone && (h.x - chest.x).hypot(h.y - chest.y) < 1.8)
            });
        if touched {
            self.wake_mimic();
        }
        let room = &mut self.dungeon.rooms[self.at];
        if let Some(chest) = room.chest.as_mut().filter(|c| !c.open)
            && self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - chest.x).hypot(h.y - chest.y) < 1.8)
        {
            chest.open = true;
            self.sounds.push("chest_open");
            let (cx, cy) = (chest.x, chest.y);
            let pack = self.dungeon.pack;
            for (i, card) in self
                .book
                .roll_chest(&mut self.rng, pack)
                .into_iter()
                .enumerate()
            {
                let angle = std::f32::consts::FRAC_PI_2 + (i as f32 - 1.0) * 1.1;
                room.items.push(Item {
                    card,
                    x: cx + angle.cos() * 3.0,
                    y: cy + angle.sin() * 2.6,
                    held_off: None,
                });
            }
        }
        let mut dropped = Vec::new();
        let mut nova = 0;
        let beaumains = self
            .hireling
            .as_ref()
            .is_some_and(|h| h.kind == hireling::AllyKind::Beaumains);
        for (&id, hero) in self
            .players
            .iter_mut()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
        {
            for item in &mut room.items {
                if item.held_off == Some(id)
                    && (hero.x - item.x).hypot(hero.y - item.y) > PICKUP_REACH + 0.6
                {
                    item.held_off = None;
                }
            }
            let book = &self.book;
            // While Beaumains walks with the party a summoning card would
            // only clog a hand: it stays where it lies.
            let Some(index) = room.items.iter().position(|item| {
                item.held_off.is_none()
                    && (hero.x - item.x).hypot(hero.y - item.y) < PICKUP_REACH
                    && book.get(&item.card).is_some_and(|card| {
                        hero.takes(card) && !(beaumains && card.summon().is_some())
                    })
            }) else {
                continue;
            };
            let item = room.items.swap_remove(index);
            let Some(card) = book.get(&item.card) else {
                continue;
            };
            match card.kind {
                cards::Kind::Take => {
                    hero.spend(card, &mut self.score, &mut nova);
                    if card.id == "talisman" {
                        hero.talisman = true;
                        self.cues.push("talisman".into());
                    }
                }
                cards::Kind::Play => hero.hand.push(card.id.clone()),
                cards::Kind::Hold => {
                    hero.deck.push(card.id.clone());
                    hero.rebonus(book);
                    for effect in &card.effects {
                        if let cards::Effect::MaxHp(v) = *effect {
                            hero.max_hp += v;
                            hero.hp = (hero.hp + v).min(hero.max_hp);
                        }
                    }
                }
                cards::Kind::Arm => {
                    if let Some(old) = hero.arm.take() {
                        dropped.push((old, item.x, item.y, id));
                    }
                    hero.equip(card);
                }
                cards::Kind::Spell => {
                    hero.equip_spell(card);
                    if self.blasts.len() < 16 {
                        self.blasts.push((hero.x, hero.y, BLAST_TICKS));
                    }
                }
                cards::Kind::Guard => {
                    if let Some(old) = hero.guard_card.take() {
                        dropped.push((old, item.x, item.y, id));
                    }
                    hero.guard = card.guard.unwrap_or_default();
                    hero.guard_card = Some(card.id.clone());
                }
            }
            self.found = Some((self.tick, id, format!("took {}", card.name)));
            self.sounds.push(if card.is_spoil() {
                "spoil_pickup"
            } else {
                "card_pickup"
            });
            // Spoils are counted, not cheered; other finds are named.
            match (card.kind, card.rarity) {
                _ if card.is_spoil() => {}
                (cards::Kind::Arm, _) => self.cues.push(format!("card_arm:{}", card.id)),
                (cards::Kind::Guard, _) => self.cues.push(format!("card_guard:{}", card.id)),
                (_, cards::Rarity::Relic) => self.cues.push("card_relic".into()),
                (_, cards::Rarity::Rare) => self.cues.push(format!("card_rare:{}", card.id)),
                _ => {}
            }
        }
        for (card, x, y, id) in dropped {
            room.items.push(Item {
                card,
                x,
                y,
                held_off: Some(id),
            });
        }
        if nova > 0 {
            for enemy in &mut self.enemies {
                enemy.hp = enemy.hp.saturating_sub(nova);
            }
        }
    }

    /// Through an open doorway into the next room, down the stairs, or
    /// into the light home.
    fn travel(&mut self) {
        if let Some((lx, ly)) = self.light
            && self
                .players
                .values()
                .any(|h| h.hp > 0 && !h.stone && (h.x - lx).hypot(h.y - ly) < 1.6)
        {
            // Keep the finite, validated exit marker as victory evidence for
            // saves and sequential mirrors. Won no longer runs travel.
            self.phase = Phase::Won;
            self.cues.push("homeward".into());
            self.sounds.push("descend");
            self.bank_all(true, "home with the dragon's hoard");
            return;
        }
        let mut exit = None;
        let mut stairs = false;
        // At home, stairs that belong to an entrance go where it goes; only
        // the Undercroft's Winding Stair goes down into the delve.
        let mut entrance: Option<&'static world::Entrance> = None;
        let home = self.at_home_now();
        let room = self.room();
        for hero in self.players.values().filter(|h| h.hp > 0 && !h.stone) {
            let dir = if hero.y < 0.6 {
                Some(0)
            } else if hero.x > room.width() - 0.6 {
                Some(1)
            } else if hero.y > room.height() - 0.6 {
                Some(2)
            } else if hero.x < 0.6 {
                Some(3)
            } else {
                None
            };
            if dir.is_some() {
                exit = dir;
            }
            let col = (hero.x / TILE_UNITS) as i32;
            let row = (hero.y / TILE_UNITS) as i32;
            if room.tile(col, row) == Tile::Stairs {
                let gate = home
                    .then(|| world::entrance_at(room.kind, col, row))
                    .flatten();
                // A door the realm has not built yet is only a wall.
                if gate.is_some_and(|e| !world::open(e, &self.home)) {
                    continue;
                }
                // Home stairs that are no entrance are the Winding Stair's
                // only in the Undercroft itself.
                if gate.is_some() || !home || room.kind == RoomKind::Home {
                    stairs = true;
                    entrance = entrance.or(gate);
                }
            }
        }
        if !stairs {
            self.stairs_held = false;
            self.descending = 0;
        }
        // Out of a side-on hall, any way out is its one doorway back.
        let exit = exit.map(|dir| self.room().way_back().unwrap_or(dir));
        // At home the Winding Stair is taken on purpose: a knight stands on
        // it a moment (its ring fills) rather than brushing past it.
        let deliberate = if self.at_home_now() && stairs && !self.stairs_held {
            self.descending += 1;
            self.descending >= home::DESCEND_HOLD
        } else {
            true
        };
        if stairs && self.guardian_locked() {
            // Do not let a stair attempt swallow a simultaneous retreat.
            if self.tick.is_multiple_of(u64::from(HZ)) {
                self.show_boss_lock();
            }
            stairs = false;
        }
        if stairs && !self.stairs_held && deliberate {
            self.descending = 0;
            match entrance {
                Some(e) => self.take_entrance(e),
                None => self.descend(),
            }
        } else if let Some(dir) = exit
            && let Some(next) = self.dungeon.neighbour(self.at, dir)
        {
            self.enter(next, Some((dir + 2) % 4));
        }
    }
}

#[cfg(test)]
impl Run {
    pub(crate) fn enter_for_test(&mut self, index: usize) {
        // Unrelated combat/wave/art fixtures explicitly exercise the retained
        // legacy encounter path. Policy tests call production `enter` instead.
        self.boss_gates = None;
        let builtin = bosses::builtin();
        self.bosses
            .retain(|b| builtin.iter().any(|n| n.id == b.id) || encounter_catalog::custom(b));
        self.enter(index, Some(2));
    }
    pub(crate) fn spawn_at_for_test(&mut self, kind: EnemyKind, x: f32, y: f32) {
        // Unrelated combat/card/song fixtures explicitly use legacy actors,
        // just like enter_for_test. Ecology regressions call production paths.
        self.boss_gates = None;
        self.spawn_at(kind, x, y);
        if let Some(enemy) = self.enemies.last_mut() {
            enemy.age = TELEGRAPH;
        }
    }

    pub(crate) fn clear_prerequisites_for_test(&mut self) {
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

    pub(crate) fn descend_for_test(&mut self) {
        // Floor-setup helper, not a test of the production travel guard.
        self.clear_prerequisites_for_test();
        if let Some(g) = &mut self.boss_gates {
            g.guardian_defeated = true;
            self.dungeon.rooms[g.guardian].visited = true;
            self.dungeon.rooms[g.guardian].cleared = true;
        }
        self.descend();
    }

    pub(crate) fn thrill_for_test(&mut self, thousands: u32) {
        self.thrill(thousands);
    }

    pub(crate) fn kit_out_for_test(&mut self) {
        self.kit_out();
    }

    /// No more waves, traps or falling rocks in this room.
    pub(crate) fn calm_for_test(&mut self) {
        self.waves = Waves::default();
        self.traps.clear();
        self.rocks.clear();
    }

    pub(crate) fn clear_for_test(&mut self) {
        self.calm_for_test();
        for enemy in &mut self.enemies {
            enemy.hp = 0;
        }
        self.step(&BTreeMap::new());
    }

    pub(crate) fn lair_for_test(&mut self) {
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
#[path = "../../../tests/cockpit/app/together_shooter__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_ledge__tests.rs"]
mod ledge_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_balance__tests.rs"]
mod balance_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_deep__tests.rs"]
mod deep_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_boss_gates__tests.rs"]
mod boss_gate_tests;
