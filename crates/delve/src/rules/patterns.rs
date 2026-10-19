//! The monsters' bullet patterns: a small vocabulary of emitters, each a
//! set of fixed numbers, each the same shots from the same seed.
//!
//! An emitter is one volley's shape: an aimed stream, a ring with a gap to
//! slip through, a flower whose petals turn a little each volley, a pair
//! of spirals turning against each other, weaving streams, hooks that
//! curve in, rings that start slow and quicken, shots that stop and then
//! come for you, shots that burst, shots that come off a wall once, a
//! laser and a sweeping laser behind a warning line, mines that hang and
//! pop, a wall of shots with one gap, crossfire from the side walls, rain
//! from the top of the room, and slow orbs that turn after a knight.
//!
//! Each kind of monster has a signature: the emitters it walks through,
//! one volley each, chosen by the floor it lives on (`signature`). An
//! elite walks a longer chain and fires one step thicker. A guardian's
//! `.boss` file can name any emitter as an attack and chain two to four in
//! `phases`.
//!
//! What a shot does after it leaves (its speed change, turn, weave, the
//! fuse that stops or bursts it, a beam's warning and burn) rides in its
//! `ShotTraits::path`, which is empty for every other shot, so nothing
//! that flew before flies differently now. A shot that waits (the later
//! shots of a stream, rain, crossfire) neither moves nor strikes while it
//! waits; the ones marked show where they will come from.

use super::*;
use std::f32::consts::{PI, TAU};

/// The vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emitter {
    /// Three shots in a line at a knight, one after another.
    Burst,
    /// A fan aimed where a knight is going, not where it is.
    Lead,
    /// A full ring with one wide gap to slip through.
    GapRing,
    /// Petals of three, the whole flower turned a little each volley.
    Flower,
    /// Two pairs of spiral arms, turning against each other.
    TwinSpiral,
    /// A stream at a knight, its shots weaving either side of the line.
    Weave,
    /// A fan whose outer shots hook back in across the line.
    Curve,
    /// A ring that leaves slow and quickens.
    Surge,
    /// A ring that stops in the air, then flies at the nearest knight.
    Reaim,
    /// A slow heavy shot that bursts into a small ring.
    Split,
    /// A fan that comes off the first wall it meets.
    Bounce,
    /// A thin warning line, then a beam down it.
    Laser,
    /// A warning line, then a beam that sweeps across an arc.
    Sweep,
    /// Shots that slow to a stop, hang, and pop into rings.
    Mines,
    /// A wall of shots with one gap, and a second wall with the gap moved.
    GapWall,
    /// Rows of shots from the side walls, marked first.
    Crossfire,
    /// Drops from the top of the room, marked first.
    Rain,
    /// Slow orbs that turn after the nearest knight, and fade.
    Seeker,
    /// A quick line of thin, fast darts at a knight.
    Darts,
    /// Slow heavy orbs, wide enough to walk round, not to slip between.
    Orbs,
    /// Shots thrown up and over, landing on a marked spot where a knight
    /// stood: harmless in the air, a splash where they come down.
    Lob,
    /// A fan that leaves fast and slows to a crawl, hanging in the way.
    Drift,
}

impl Emitter {
    pub const ALL: [Emitter; 22] = [
        Emitter::Burst,
        Emitter::Lead,
        Emitter::GapRing,
        Emitter::Flower,
        Emitter::TwinSpiral,
        Emitter::Weave,
        Emitter::Curve,
        Emitter::Surge,
        Emitter::Reaim,
        Emitter::Split,
        Emitter::Bounce,
        Emitter::Laser,
        Emitter::Sweep,
        Emitter::Mines,
        Emitter::GapWall,
        Emitter::Crossfire,
        Emitter::Rain,
        Emitter::Seeker,
        Emitter::Darts,
        Emitter::Orbs,
        Emitter::Lob,
        Emitter::Drift,
    ];

    /// Its name in a `.boss` file.
    pub fn name(self) -> &'static str {
        match self {
            Emitter::Burst => "burst",
            Emitter::Lead => "lead",
            Emitter::GapRing => "gap_ring",
            Emitter::Flower => "flower",
            Emitter::TwinSpiral => "twin_spiral",
            Emitter::Weave => "weave",
            Emitter::Curve => "curve",
            Emitter::Surge => "surge",
            Emitter::Reaim => "reaim",
            Emitter::Split => "split",
            Emitter::Bounce => "bounce",
            Emitter::Laser => "laser",
            Emitter::Sweep => "sweep",
            Emitter::Mines => "mines",
            Emitter::GapWall => "gap_wall",
            Emitter::Crossfire => "crossfire",
            Emitter::Rain => "rain",
            Emitter::Seeker => "seeker",
            Emitter::Darts => "darts",
            Emitter::Orbs => "orbs",
            Emitter::Lob => "lob",
            Emitter::Drift => "drift",
        }
    }

    pub fn named(word: &str) -> Option<Emitter> {
        let word = word.to_ascii_lowercase().replace('-', "_");
        Emitter::ALL.into_iter().find(|e| e.name() == word)
    }

    /// The most shots one volley fires at no extra thickness (a beam counts
    /// as four): what a boss's bullet budget counts.
    pub fn shots(self) -> u32 {
        match self {
            Emitter::Burst => BURST.0,
            Emitter::Lead => LEAD.0,
            Emitter::GapRing => GAP_RING.0,
            Emitter::Flower => FLOWER.0 * 3,
            Emitter::TwinSpiral => TWIN_SPIRAL.0 * 4,
            Emitter::Weave => WEAVE.0,
            Emitter::Curve => CURVE.0,
            Emitter::Surge => SURGE.0,
            Emitter::Reaim => REAIM.0,
            Emitter::Split => SPLIT.0 * (1 + SPLIT_RING),
            Emitter::Bounce => BOUNCE.0,
            Emitter::Laser | Emitter::Sweep => 4,
            Emitter::Mines => MINES.0 * (1 + MINE_RING),
            Emitter::GapWall => GAP_WALL.0 * 2,
            Emitter::Crossfire => CROSS_LEFT.len() as u32 + CROSS_RIGHT.len() as u32,
            Emitter::Rain => RAIN.0,
            Emitter::Seeker => SEEKER.0,
            Emitter::Darts => DARTS.0,
            Emitter::Orbs => ORBS.0,
            Emitter::Lob => LOBS.0,
            Emitter::Drift => DRIFT.0,
        }
    }

    /// Its count in a guardian's volley (shots, petals, shots an arm,
    /// mines or a wall's slots): smaller than a monster's, so no volley
    /// holds more than `bosses::VOLLEY_MOST`.
    pub fn guardian_count(self) -> u32 {
        match self {
            Emitter::Burst => 3,
            Emitter::Lead => 3,
            Emitter::GapRing => 10,
            Emitter::Flower => 3,
            Emitter::TwinSpiral => 2,
            Emitter::Weave => 5,
            Emitter::Curve => 5,
            Emitter::Surge => 8,
            Emitter::Reaim => 6,
            Emitter::Split => 1,
            Emitter::Bounce => 3,
            Emitter::Laser | Emitter::Sweep => 1,
            Emitter::Mines => 1,
            Emitter::GapWall => 7,
            Emitter::Crossfire => 5,
            Emitter::Rain => 6,
            Emitter::Seeker => 2,
            Emitter::Darts => 3,
            Emitter::Orbs => 2,
            Emitter::Lob => 2,
            Emitter::Drift => 3,
        }
    }

    /// The most shots a guardian's volley of it holds, a beam as one.
    pub fn guardian_shots(self) -> u32 {
        let n = self.guardian_count();
        match self {
            Emitter::GapRing => n - (GAP_HALF * 2.0 / TAU * n as f32).floor() as u32,
            Emitter::Flower => n * 3,
            Emitter::TwinSpiral => n * 4,
            Emitter::Split => n * (1 + SPLIT_RING),
            Emitter::Mines => n * (1 + MINE_RING),
            Emitter::GapWall => (n - WALL_GAP as u32) * 2,
            Emitter::Crossfire => CROSS_LEFT.len() as u32 + CROSS_RIGHT.len() as u32,
            _ => n,
        }
    }
}

// Each emitter's numbers: shots at no extra thickness, shots added per
// step of thickness, and its speed in units a second. A knight runs 13; a
// monster's shot stays between 4 and 9 so it can be read and stepped
// round.
const BURST: (u32, u32, f32) = (3, 1, 8.0);
/// Ticks between a burst's shots.
const BURST_GAP: u8 = 4;
const LEAD: (u32, u32, f32) = (3, 2, 7.0);
/// How far ahead of a knight a lead is aimed, in seconds of its running,
/// and its fan's width (radians).
const LEAD_AHEAD: f32 = 0.5;
const LEAD_ARC: f32 = 0.6;
const GAP_RING: (u32, u32, f32) = (20, 4, 4.5);
/// Half the gap's width, in radians: some 71 degrees in all, whatever the
/// ring's count. Its middle lands within this much of the knight's line.
pub const GAP_HALF: f32 = 0.62;
const GAP_SWING: f32 = 0.8;
const FLOWER: (u32, u32, f32) = (6, 1, 4.0);
/// Each volley's flower stands this much turned from the last; its petals
/// lean in flight by this many radians a second.
const FLOWER_TURN: f32 = 0.35;
const FLOWER_LEAN: f32 = 0.35;
/// Shots per arm; four arms.
const TWIN_SPIRAL: (u32, u32, f32) = (7, 1, 5.0);
const SPIRAL_GAP: u8 = 3;
const SPIRAL_STEP: f32 = 0.26;
const WEAVE: (u32, u32, f32) = (6, 2, 6.0);
const WEAVE_GAP: u8 = 4;
/// How far a weaving shot swings off its line (units), and how fast
/// (radians a tick: about a swing a second).
const WEAVE_REACH: f32 = 1.2;
const WEAVE_RATE: f32 = 0.21;
const CURVE: (u32, u32, f32) = (6, 2, 6.5);
const CURVE_ARC: f32 = 1.3;
/// Radians a second a hooking shot turns.
const CURVE_TURN: f32 = 0.9;
const SURGE: (u32, u32, f32) = (12, 3, 1.2);
/// A surge quickens this much a second, to this speed.
const SURGE_GAIN: f32 = 7.0;
const SURGE_TOP: f32 = 9.0;
const REAIM: (u32, u32, f32) = (8, 2, 7.0);
/// A re-aiming ring slows this hard to a stop (some three units out),
/// hangs, then at its fuse flies at the nearest knight this fast.
const REAIM_BRAKE: f32 = 8.0;
const REAIM_FUSE: u16 = 45;
const REAIM_SPEED: f32 = 7.5;
const SPLIT: (u32, u32, f32) = (1, 1, 5.0);
const SPLIT_FUSE: u16 = 27;
const SPLIT_RING: u32 = 6;
const SPLIT_SPEED: f32 = 4.5;
const BOUNCE: (u32, u32, f32) = (3, 1, 7.0);
const BOUNCE_ARC: f32 = 0.45;
/// A beam's warning and burn, in ticks; a sweep warns longer and burns
/// longer, across a narrower arc. A beam is this wide either side of its
/// line.
pub const BEAM_WARN: u8 = 21;
pub const BEAM_BURN: u8 = 9;
pub const SWEEP_WARN: u8 = 30;
pub const SWEEP_BURN: u8 = 18;
const SWEEP_ARC: f32 = 1.1;
/// A beam strikes for this share of its shot's damage, in percent.
pub const BEAM_SHARE: u32 = 60;
pub const BEAM_HALF: f32 = 0.3;
/// The longest beam, and the side beams' lean when thicker.
const BEAM_REACH: f32 = 40.0;
const BEAM_FAN: f32 = 0.4;
const MINES: (u32, u32, f32) = (4, 1, 8.0);
const MINE_BRAKE: f32 = 8.0;
const MINE_FUSE: u16 = 51;
const MINE_RING: u32 = 6;
const MINE_SPEED: f32 = 4.0;
const MINE_SPREAD: f32 = 0.45;
/// Slots in a wall, a unit apart; two slots are left out for the gap, and
/// the second wall follows this many ticks behind with its gap moved.
const GAP_WALL: (u32, u32, f32) = (13, 2, 5.0);
const WALL_SPACING: f32 = 1.0;
const WALL_GAP: usize = 2;
const WALL_FOLLOWS: u8 = 24;
/// Crossfire rows, in units off the knight's row, from the left wall and
/// from the right; marked this long before they fly.
const CROSS_LEFT: [f32; 3] = [-3.0, 0.0, 3.0];
const CROSS_RIGHT: [f32; 2] = [-1.5, 1.5];
const CROSS_SPEED: f32 = 6.0;
pub const CROSS_MARK: u8 = 18;
const RAIN: (u32, u32, f32) = (8, 3, 5.0);
/// Rain is marked this long; the drops fall at its speed and up to this
/// much faster.
const RAIN_MARK: u8 = 24;
const RAIN_SPREAD: f32 = 1.5;
const SEEKER: (u32, u32, f32) = (2, 1, 3.5);

// The families' own numbers, so a knight reads a shot's speed and threat
// from its look (`Family`): darts are the fastest and thinnest, orbs the
// slowest and widest, lobs come down where they are marked, drift leaves
// fast and hangs.
/// The fastest any monster's shot flies, in units a second, unless a line
/// drawn first warns of it (a beam, the Marksman's bolt): 88 pixels a
/// second on the 384x224 screen, under a knight's 13. From the eight units
/// a shooter keeps off that is three quarters of a second, two of them
/// left after a quarter-second's reaction, and a knight needs a twentieth
/// to step its body out of a dart's line.
pub const SHOT_TOP: f32 = 11.0;
const DARTS: (u32, u32, f32) = (3, 1, SHOT_TOP);
/// Ticks between a line of darts.
const DART_GAP: u8 = 5;
/// How far past a knight's body a dart strikes: half a plain shot's.
pub const DART_STRIKE: f32 = 0.06;
/// Heavy orbs: the speed of a stroll, a fan's lean between them, how far
/// past a knight's body one strikes (it is drawn wider still), its life.
const ORBS: (u32, u32, f32) = (1, 1, 2.6);
const ORB_SPREAD: f32 = 0.55;
pub const ORB_STRIKE: f32 = 0.5;
const ORB_LIFE: u32 = 9 * HZ;
/// Lobs: ticks in the air (the landing mark shows all of them), how far
/// round its spot one splashes (past a knight's body), how far the others
/// of a thicker volley land from the first, how high one rises a unit
/// thrown, and the longest throw, which keeps the shadow's ground speed
/// under `SHOT_TOP`.
const LOBS: (u32, u32, f32) = (1, 1, 0.0);
pub const LOB_FLIGHT: u16 = 33;
pub const LOB_SPLASH: f32 = 0.85;
const LOB_SCATTER: f32 = 2.6;
pub const LOB_RISE: f32 = 0.3;
const LOB_REACH: f32 = 12.0;
/// Drift: a fan that leaves at ten, sheds this much a second and creeps on
/// at the last.
const DRIFT: (u32, u32, f32) = (3, 1, 10.0);
const DRIFT_ARC: f32 = 0.7;
const DRIFT_BRAKE: f32 = 9.0;
const DRIFT_TOP: f32 = 2.2;
const DRIFT_LIFE: u32 = 5 * HZ;
/// Radians a second a seeker turns after a knight, and how long it lasts.
const SEEKER_TURN: f32 = 1.1;
const SEEKER_LIFE: u32 = 5 * HZ;

/// The thickness a floor adds, by floor (index 0 is floor 1), and the
/// thickness each knight standing adds (index: how many), and the most
/// there is. An elite adds one more.
pub const DEPTH_STEP: [u8; 6] = [0, 0, 1, 2, 2, 3];
pub const HEAD_STEP: [u8; 5] = [0, 0, 1, 2, 3];
pub const MOST_STEP: u8 = 5;
/// The most a greater demon's volleys thicken, whatever the floor and the
/// party: its twin spiral is 32 shots at most and its sweep one beam.
pub const DEMON_STEP: u8 = 1;
/// The first two floors teach: an ordinary monster waits this much longer
/// between volleys, in percent (floor 1, floor 2). Health, blows and the
/// shots themselves are as ever; guardians and deeper floors are untouched.
pub const STARTER_PACE: [u32; 2] = [180, 135];
/// A guardian's thickness by knights standing: a full party of four adds a
/// step.
pub const BOSS_HEAD_STEP: [u8; 5] = [0, 0, 0, 0, 1];

/// What a shot does after it leaves. Empty for every shot but a pattern's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Path {
    /// Ticks it has flown.
    #[serde(default)]
    pub age: u16,
    /// Ticks it waits before it flies (or a beam before it starts its
    /// warning): still, and harmless.
    #[serde(default)]
    pub wait: u8,
    /// Still harmless on the tick that consumes the last wait. Rendering
    /// and hirelings see the same warning interval as the main collision loop.
    #[serde(default)]
    pub held: bool,
    /// While it waits, its spot is marked for the knights to see.
    #[serde(default)]
    pub mark: bool,
    /// Speed it gains a second (lost, if below nothing) on its way to `top`.
    #[serde(default)]
    pub accel: f32,
    #[serde(default)]
    pub top: f32,
    /// Radians a second it turns.
    #[serde(default)]
    pub turn: f32,
    /// Radians a second it turns toward the nearest knight.
    #[serde(default)]
    pub seek: f32,
    /// How far either side of its line it weaves, and how fast.
    #[serde(default)]
    pub wave: f32,
    #[serde(default)]
    pub wave_rate: f32,
    /// The tick of its flight `then` comes.
    #[serde(default)]
    pub fuse: u16,
    #[serde(default)]
    pub then: Then,
    #[serde(default)]
    pub beam: Option<Beam>,
    /// Its body: plain, a thin dart, a heavy orb, or a lob in the air
    /// (`fuse` is when it lands).
    #[serde(default, skip_serializing_if = "Form::plain")]
    pub form: Form,
}

/// A monster's shot's body.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    #[default]
    Plain,
    Dart,
    Orb,
    Lob,
}

impl Form {
    fn plain(&self) -> bool {
        *self == Form::Plain
    }
}

/// What comes when a shot's fuse burns down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Then {
    #[default]
    Nothing,
    /// It turns to fly at the nearest knight this fast.
    Aim(f32),
    /// It bursts into a ring of this many, this fast.
    Burst(u8, f32),
}

/// A beam: its line from the shot's spot, which way, how far it reaches
/// before stone, how much it turns a tick while it burns, its warning and
/// burn, and the knights it has struck (one bit each).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Beam {
    pub angle: f32,
    pub reach: f32,
    pub sweep: f32,
    pub warn: u8,
    pub burn: u8,
    pub struck: u8,
}

impl Beam {
    /// Past its warning: it strikes.
    pub fn burning(&self, age: u16) -> bool {
        age > u16::from(self.warn) && age <= u16::from(self.warn) + u16::from(self.burn)
    }

    /// Where a sweep will end, for its warning to show the arc.
    pub fn end_angle(&self) -> f32 {
        self.angle + self.sweep * f32::from(self.burn.saturating_sub(1))
    }

    pub fn crosses(&self, from: (f32, f32), at: (f32, f32)) -> bool {
        let (ux, uy) = (self.angle.cos(), self.angle.sin());
        let (rx, ry) = (at.0 - from.0, at.1 - from.1);
        let along = (rx * ux + ry * uy).clamp(0.0, self.reach);
        (rx - ux * along).hypot(ry - uy * along) <= HERO_RADIUS + BEAM_HALF
    }
}

impl Path {
    /// Still waiting to fly.
    pub fn waiting(&self) -> bool {
        self.wait > 0 || self.held
    }

    /// A heavy shot that will burst.
    pub fn bursts(&self) -> bool {
        matches!(self.then, Then::Burst(..))
    }
}

impl Projectile {
    /// Out in the room as a shot: not still waiting its turn, and not a
    /// beam, which strikes down its line rather than where it stands.
    pub fn in_flight(&self) -> bool {
        !self.traits.path.waiting() && self.traits.path.beam.is_none()
    }
}

/// How far past a knight's body a monster's shot strikes, in units: the
/// plain shot's.
pub const STRIKE: f32 = 0.16;

/// What a monster's shot does in flight, as a knight reads it: each family
/// is drawn its own way (`render::overworld::arena::volleys`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    Plain,
    Dart,
    Orb,
    Lob,
    /// Quickens as it flies.
    Quicken,
    /// Slows (and may stop and come again).
    Brake,
    Wave,
    Hook,
    Split,
    Bounce,
    Seeker,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Plain => "plain",
            Family::Dart => "dart",
            Family::Orb => "orb",
            Family::Lob => "lob",
            Family::Quicken => "quicken",
            Family::Brake => "brake",
            Family::Wave => "wave",
            Family::Hook => "hook",
            Family::Split => "split",
            Family::Bounce => "bounce",
            Family::Seeker => "seeker",
        }
    }
}

/// A shot's family, from what its path does.
pub fn family(shot: &Projectile) -> Family {
    let path = &shot.traits.path;
    if path.form == Form::Dart {
        Family::Dart
    } else if path.form == Form::Orb {
        Family::Orb
    } else if path.form == Form::Lob {
        Family::Lob
    } else if path.bursts() {
        Family::Split
    } else if shot.traits.bounce > 0 {
        Family::Bounce
    } else if path.wave != 0.0 {
        Family::Wave
    } else if path.seek > 0.0 {
        Family::Seeker
    } else if path.accel > 0.0 {
        Family::Quicken
    } else if path.accel < 0.0 || matches!(path.then, Then::Aim(_)) {
        Family::Brake
    } else if path.turn != 0.0 {
        Family::Hook
    } else {
        Family::Plain
    }
}

/// How far past a knight's body `shot` strikes.
pub fn girth(shot: &Projectile) -> f32 {
    match shot.traits.path.form {
        Form::Dart => DART_STRIKE,
        Form::Orb => ORB_STRIKE,
        Form::Lob => LOB_SPLASH,
        Form::Plain => STRIKE,
    }
}

/// How near a shot's middle comes to a prop (a tomb, a pillar, a shelf, a
/// crate, furniture: a `Tile::Block`) before it stops on it: its drawn
/// body, so nothing drawn solid is flown through. A lob is over it all
/// until it lands.
pub const BODY: f32 = 0.25;
pub const DART_BODY: f32 = 0.1;
pub const ORB_BODY: f32 = 0.6;
/// A knight's own shot's body, and a wish's bigger shot's on top.
pub const KNIGHT_BODY: f32 = 0.2;

pub fn body(shot: &Projectile) -> f32 {
    if !shot.hostile {
        return KNIGHT_BODY + shot.reach();
    }
    match shot.traits.path.form {
        Form::Dart => DART_BODY,
        Form::Orb => ORB_BODY,
        Form::Lob => 0.0,
        Form::Plain => BODY,
    }
}

/// A lob's tick: it flies on over everything, and at its fuse comes down,
/// striking every knight within `LOB_SPLASH` of its spot once (a raised
/// shield does not turn what falls from above; a roll's grace does). True
/// on the tick it lands, when it is gone.
pub(super) fn lob(
    bullet: &mut Projectile,
    players: &mut BTreeMap<u32, Hero>,
    sounds: &mut Vec<&'static str>,
) -> bool {
    let path = &mut bullet.traits.path;
    path.age = path.age.saturating_add(1);
    bullet.x += bullet.vx * DT;
    bullet.y += bullet.vy * DT;
    bullet.ttl = bullet.ttl.saturating_sub(1);
    if path.age < path.fuse {
        return false;
    }
    for hero in players.values_mut().filter(|h| h.targetable()) {
        if (hero.x - bullet.x).hypot(hero.y - bullet.y) > HERO_RADIUS + LOB_SPLASH {
            continue;
        }
        if bullet.traits.blow != 0 {
            if hero.blow(bullet.damage, bullet.traits.blow) {
                sounds.push("hero_hurt");
            }
        } else if hero.invulnerable == 0 {
            hero.hurt(bullet.damage);
            sounds.push("hero_hurt");
        }
    }
    bullet.ttl = 0;
    true
}

/// A borrowed lob (`bodies`): it flies as a monster's does and comes down
/// on the monsters, each in its splash taking a share as one stroke
/// (`aoe`). Who it struck and what each took, once it has come down.
pub(super) fn lob_foes(bullet: &mut Projectile, enemies: &mut [Enemy]) -> Option<Vec<(usize, u32)>> {
    let path = &mut bullet.traits.path;
    path.age = path.age.saturating_add(1);
    bullet.x += bullet.vx * DT;
    bullet.y += bullet.vy * DT;
    bullet.ttl = bullet.ttl.saturating_sub(1);
    if path.age < path.fuse {
        return None;
    }
    bullet.ttl = 0;
    let at = (bullet.x, bullet.y);
    Some(aoe::strike(enemies, at, bullet.damage, |e| {
        !e.buried() && (e.x - at.0).hypot(e.y - at.1) <= e.radius() + LOB_SPLASH
    }))
}

/// Where a lob comes down, and how high it is now (units): its shadow's
/// spot run on to its landing at its ground speed.
pub fn lob_flight(shot: &Projectile) -> Option<((f32, f32), f32)> {
    let path = &shot.traits.path;
    if path.form != Form::Lob || path.fuse == 0 {
        return None;
    }
    let left = f32::from(path.fuse.saturating_sub(path.age));
    let land = (shot.x + shot.vx * DT * left, shot.y + shot.vy * DT * left);
    let thrown = shot.vx.hypot(shot.vy) * DT * f32::from(path.fuse);
    let k = f32::from(path.age) / f32::from(path.fuse);
    Some((land, 4.0 * LOB_RISE * thrown * k * (1.0 - k)))
}

/// Where a volley comes from and whom it is for.
pub(super) struct Aim<'a, 'g> {
    pub(super) from: (f32, f32),
    pub(super) at: (f32, f32),
    /// Which way and how fast the knight is running, units a second.
    pub(super) pace: (f32, f32),
    pub(super) grid: &'a Grid<'g>,
    /// This volley's number, for what turns volley to volley.
    pub(super) count: u32,
    pub(super) salt: u64,
    /// How much thicker than its plain self, 0 to `MOST_STEP`.
    pub(super) step: u8,
    pub(super) kind: Shot,
    pub(super) damage: u32,
    /// A guardian's own count for this emitter (`Emitter::guardian_count`),
    /// in place of a monster's.
    pub(super) guardian: Option<u32>,
}

impl Aim<'_, '_> {
    fn line(&self) -> f32 {
        (self.at.1 - self.from.1).atan2(self.at.0 - self.from.0)
    }

    fn count(&self, (base, per, _): (u32, u32, f32)) -> u32 {
        self.guardian.unwrap_or(base) + per * u32::from(self.step.min(MOST_STEP))
    }

    /// A number from 0 to 1, the same for the same volley and `k`.
    fn roll(&self, k: u64) -> f32 {
        (mix(self.salt ^ u64::from(self.count).wrapping_mul(0x9e37_79b9) ^ k << 40) % 10_000) as f32
            / 10_000.0
    }

    /// The starting angle stays seeded while successive volleys rotate.
    fn start(&self) -> f32 {
        (mix(self.salt) % 10_000) as f32 / 10_000.0 * TAU
    }

    fn shot(&self, (x, y): (f32, f32), angle: f32, speed: f32, path: Path, ttl: u32) -> Projectile {
        Projectile {
            x,
            y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed,
            hostile: true,
            look: None,
            kind: self.kind,
            damage: self.damage,
            pierce: 0,
            last_hit: None,
            empowered: false,
            traits: ShotTraits {
                path,
                ..ShotTraits::default()
            },
            ttl: ttl + u32::from(path.wait),
        }
    }
}

const LIFE: u32 = 8 * HZ;

fn waits(ticks: u32) -> Path {
    Path {
        wait: ticks.min(255) as u8,
        ..Path::default()
    }
}

/// One volley of `emitter`, into `shots`.
pub(super) fn emit(emitter: Emitter, aim: &Aim, shots: &mut Vec<Projectile>) {
    if shots.len() >= MAX_PROJECTILES {
        return;
    }
    let line = aim.line();
    let from = aim.from;
    match emitter {
        Emitter::Burst => {
            for i in 0..aim.count(BURST) {
                let path = waits(i * u32::from(BURST_GAP));
                shots.push(aim.shot(from, line, BURST.2, path, LIFE));
            }
        }
        Emitter::Lead => {
            let ahead = (
                aim.at.0 + aim.pace.0 * LEAD_AHEAD,
                aim.at.1 + aim.pace.1 * LEAD_AHEAD,
            );
            let toward = (ahead.1 - from.1).atan2(ahead.0 - from.0);
            let n = aim.count(LEAD);
            for i in 0..n {
                let a = toward + LEAD_ARC * (i as f32 / (n - 1).max(1) as f32 - 0.5);
                shots.push(aim.shot(from, a, LEAD.2, Path::default(), LIFE));
            }
        }
        Emitter::GapRing => {
            for a in gap_ring(aim) {
                shots.push(aim.shot(from, a, GAP_RING.2, Path::default(), LIFE));
            }
        }
        Emitter::Flower => {
            let petals = aim.count(FLOWER);
            let turn = aim.count as f32 * FLOWER_TURN + aim.start();
            let lean = if aim.count.is_multiple_of(2) {
                FLOWER_LEAN
            } else {
                -FLOWER_LEAN
            };
            for k in 0..petals {
                let a = turn + k as f32 * TAU / petals as f32;
                for (off, speed) in [(-0.09, FLOWER.2), (0.0, FLOWER.2 + 1.2), (0.09, FLOWER.2)] {
                    let path = Path {
                        turn: lean,
                        ..Path::default()
                    };
                    shots.push(aim.shot(from, a + off, speed, path, LIFE));
                }
            }
        }
        Emitter::TwinSpiral => {
            let start = aim.start() + aim.count as f32 * SPIRAL_STEP;
            for j in 0..aim.count(TWIN_SPIRAL) {
                for way in [1.0f32, -1.0] {
                    for arm in [0.0, PI] {
                        let offset = if way < 0.0 { 0.13 } else { 0.0 };
                        let a = start + arm + offset + way * j as f32 * SPIRAL_STEP;
                        let path = waits(j * u32::from(SPIRAL_GAP));
                        shots.push(aim.shot(from, a, TWIN_SPIRAL.2, path, LIFE));
                    }
                }
            }
        }
        Emitter::Weave => {
            for i in 0..aim.count(WEAVE) {
                let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                let path = Path {
                    wave: WEAVE_REACH * side,
                    wave_rate: WEAVE_RATE,
                    ..waits(i * u32::from(WEAVE_GAP))
                };
                shots.push(aim.shot(from, line, WEAVE.2, path, LIFE));
            }
        }
        Emitter::Curve => {
            let n = aim.count(CURVE);
            for i in 0..n {
                let off = CURVE_ARC * (i as f32 / (n - 1).max(1) as f32 - 0.5);
                let path = Path {
                    turn: -off.signum() * CURVE_TURN,
                    ..Path::default()
                };
                shots.push(aim.shot(from, line + off, CURVE.2, path, LIFE));
            }
        }
        Emitter::Surge => {
            let n = aim.count(SURGE);
            let turn = aim.roll(3) * TAU;
            for i in 0..n {
                let path = Path {
                    accel: SURGE_GAIN,
                    top: SURGE_TOP,
                    ..Path::default()
                };
                let a = turn + i as f32 * TAU / n as f32;
                shots.push(aim.shot(from, a, SURGE.2, path, LIFE));
            }
        }
        Emitter::Reaim => {
            let n = aim.count(REAIM);
            let turn = aim.roll(4) * TAU;
            for i in 0..n {
                let path = Path {
                    accel: -REAIM_BRAKE,
                    top: 0.0,
                    fuse: REAIM_FUSE,
                    then: Then::Aim(REAIM_SPEED),
                    ..Path::default()
                };
                let a = turn + i as f32 * TAU / n as f32;
                shots.push(aim.shot(from, a, REAIM.2, path, LIFE));
            }
        }
        Emitter::Split => {
            let n = aim.count(SPLIT);
            for i in 0..n {
                let a = line + (i as f32 - (n - 1) as f32 / 2.0) * 0.3;
                let path = Path {
                    fuse: SPLIT_FUSE,
                    then: Then::Burst(SPLIT_RING as u8, SPLIT_SPEED),
                    ..Path::default()
                };
                shots.push(aim.shot(from, a, SPLIT.2, path, LIFE));
            }
        }
        Emitter::Bounce => {
            let n = aim.count(BOUNCE);
            for i in 0..n {
                let a = line + BOUNCE_ARC * (i as f32 / (n - 1).max(1) as f32 - 0.5);
                let mut shot = aim.shot(from, a, BOUNCE.2, Path::default(), 6 * HZ);
                shot.traits.bounce = 1;
                shots.push(shot);
            }
        }
        Emitter::Laser => {
            let side = 2 * (u32::from(aim.step) / 2);
            for k in 0..=side {
                let a = line + (k as f32 - side as f32 / 2.0) * BEAM_FAN;
                shots.push(beam(aim, a, 0.0, BEAM_WARN, BEAM_BURN));
            }
        }
        Emitter::Sweep => {
            let way = if aim.count.is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            let per = SWEEP_ARC / f32::from(SWEEP_BURN - 1) * way;
            shots.push(beam(
                aim,
                line - SWEEP_ARC / 2.0 * way,
                per,
                SWEEP_WARN,
                SWEEP_BURN,
            ));
            if aim.step >= 2 {
                shots.push(beam(
                    aim,
                    line + SWEEP_ARC / 2.0 * way,
                    -per,
                    SWEEP_WARN,
                    SWEEP_BURN,
                ));
            }
        }
        Emitter::Mines => {
            let n = aim.count(MINES);
            for i in 0..n {
                let a = line + (i as f32 - (n - 1) as f32 / 2.0) * MINE_SPREAD;
                let path = Path {
                    accel: -MINE_BRAKE,
                    top: 0.0,
                    fuse: MINE_FUSE,
                    then: Then::Burst(MINE_RING as u8, MINE_SPEED),
                    ..Path::default()
                };
                shots.push(aim.shot(from, a, MINES.2, path, LIFE));
            }
        }
        Emitter::GapWall => {
            let n = aim.count(GAP_WALL) as usize;
            let room = n - WALL_GAP - 1;
            let first = (aim.count as usize * 3 + (aim.roll(5) * 5.0) as usize) % room + 1;
            let second = (first + 4) % room + 1;
            for (gap, wait) in [(first, 0), (second, WALL_FOLLOWS)] {
                for at in wall(aim, n, gap) {
                    let path = waits(u32::from(wait));
                    shots.push(aim.shot(at, line, GAP_WALL.2, path, LIFE));
                }
            }
        }
        Emitter::Crossfire => {
            let (w, h) = (aim.grid.room.width(), aim.grid.room.height());
            let mark = Path {
                wait: CROSS_MARK,
                mark: true,
                ..Path::default()
            };
            for (rows, left) in [(&CROSS_LEFT[..], true), (&CROSS_RIGHT[..], false)] {
                for &dy in rows {
                    let y = aim.at.1 + dy;
                    if !(0.5..h - 0.5).contains(&y) {
                        continue;
                    }
                    let Some(x) = edge(aim.grid, y, left, w) else {
                        continue;
                    };
                    let a = if left { 0.0 } else { PI };
                    shots.push(aim.shot((x, y), a, CROSS_SPEED, mark, LIFE));
                }
            }
        }
        Emitter::Rain => {
            let (w, h) = (aim.grid.room.width(), aim.grid.room.height());
            let n = aim.count(RAIN);
            for i in 0..n {
                let x = (i as f32 + 0.2 + aim.roll(10 + u64::from(i)) * 0.6) / n as f32 * w;
                let Some(y) = (1..12)
                    .map(|k| k as f32 * 0.5)
                    .find(|&y| !aim.grid.stops_body(x, y, BODY))
                else {
                    continue;
                };
                let speed = RAIN.2 + aim.roll(40 + u64::from(i)) * RAIN_SPREAD;
                let path = Path {
                    wait: RAIN_MARK + (i % 3) as u8 * 4,
                    mark: true,
                    ..Path::default()
                };
                let ttl = (h / speed * HZ as f32) as u32 + HZ;
                shots.push(aim.shot((x, y + 0.3), PI / 2.0, speed, path, ttl));
            }
        }
        Emitter::Seeker => {
            let n = aim.count(SEEKER);
            for i in 0..n {
                let a = line + 1.8 * (i as f32 / (n - 1).max(1) as f32 - 0.5);
                let path = Path {
                    seek: SEEKER_TURN,
                    ..Path::default()
                };
                shots.push(aim.shot(from, a, SEEKER.2, path, SEEKER_LIFE));
            }
        }
        Emitter::Darts => {
            for i in 0..aim.count(DARTS) {
                let path = Path {
                    form: Form::Dart,
                    ..waits(i * u32::from(DART_GAP))
                };
                shots.push(aim.shot(from, line, DARTS.2, path, LIFE));
            }
        }
        Emitter::Orbs => {
            let n = aim.count(ORBS);
            for i in 0..n {
                let a = line + (i as f32 - (n - 1) as f32 / 2.0) * ORB_SPREAD;
                let path = Path {
                    form: Form::Orb,
                    ..Path::default()
                };
                shots.push(aim.shot(from, a, ORBS.2, path, ORB_LIFE));
            }
        }
        Emitter::Lob => {
            let n = aim.count(LOBS);
            let turn = aim.roll(7) * TAU;
            for i in 0..n {
                // The first comes down where the knight stands; the rest
                // round it.
                let spot = if i == 0 {
                    aim.at
                } else {
                    let a = turn + i as f32 * TAU / (n - 1) as f32;
                    (aim.at.0 + a.cos() * LOB_SCATTER, aim.at.1 + a.sin() * LOB_SCATTER)
                };
                let Some(land) = landing(aim.grid, from, spot) else {
                    continue;
                };
                let (dx, dy) = (land.0 - from.0, land.1 - from.1);
                let speed = dx.hypot(dy) / (f32::from(LOB_FLIGHT) * DT);
                let path = Path {
                    form: Form::Lob,
                    fuse: LOB_FLIGHT,
                    ..Path::default()
                };
                shots.push(aim.shot(from, dy.atan2(dx), speed, path, u32::from(LOB_FLIGHT) + 2));
            }
        }
        Emitter::Drift => {
            let n = aim.count(DRIFT);
            for i in 0..n {
                let a = line + DRIFT_ARC * (i as f32 / (n - 1).max(1) as f32 - 0.5);
                let path = Path {
                    accel: -DRIFT_BRAKE,
                    top: DRIFT_TOP,
                    ..Path::default()
                };
                shots.push(aim.shot(from, a, DRIFT.2, path, DRIFT_LIFE));
            }
        }
    }
    shots.truncate(MAX_PROJECTILES);
}

/// Where a lob thrown from `from` at `spot` comes down: no further than
/// `LOB_REACH`, inside the room, on open ground (drawn back toward the
/// thrower off stone); `None` if there is nowhere.
fn landing(grid: &Grid, from: (f32, f32), spot: (f32, f32)) -> Option<(f32, f32)> {
    let (dx, dy) = (spot.0 - from.0, spot.1 - from.1);
    let d = dx.hypot(dy).max(0.01);
    let reach = d.min(LOB_REACH);
    let (ux, uy) = (dx / d, dy / d);
    let mut r = reach;
    while r > 1.0 {
        let at = (from.0 + ux * r, from.1 + uy * r);
        if !grid.stops_shot(at.0, at.1) && grid.open(at.0, at.1, Mover::Flier) {
            return Some(at);
        }
        r -= 0.5;
    }
    None
}

/// The angles of a gap ring's shots: a turning ring with the slots inside
/// `GAP_HALF` of the gap's middle left out.
pub(super) fn gap_ring(aim: &Aim) -> Vec<f32> {
    let n = aim.count(GAP_RING);
    let gap = aim.line() + (aim.roll(6) - 0.5) * 2.0 * GAP_SWING;
    let turn = aim.count as f32 * 0.13;
    (0..n)
        .map(|i| turn + i as f32 * TAU / n as f32)
        .filter(|&a| {
            let off = (a - gap).sin().atan2((a - gap).cos());
            off.abs() >= GAP_HALF
        })
        .collect()
}

/// A wall's spots: `n` slots a unit apart across the line at the shooter,
/// slot `gap` and the next left out, and any in stone left out.
fn wall(aim: &Aim, n: usize, gap: usize) -> Vec<(f32, f32)> {
    let line = aim.line();
    let (px, py) = (-line.sin(), line.cos());
    (0..n)
        .filter(|&i| i < gap || i >= gap + WALL_GAP)
        .map(|i| {
            let k = (i as f32 - (n - 1) as f32 / 2.0) * WALL_SPACING;
            (aim.from.0 + px * k, aim.from.1 + py * k)
        })
        .filter(|&(x, y)| !aim.grid.stops_body(x, y, BODY))
        .collect()
}

/// The first open spot in from the left (or right) wall on row `y`.
fn edge(grid: &Grid, y: f32, left: bool, w: f32) -> Option<f32> {
    (1..((w / 2.0) as i32 * 2))
        .map(|k| k as f32 * 0.5)
        .map(|d| if left { d } else { w - d })
        .find(|&x| !grid.stops_body(x, y, BODY))
        .map(|x| if left { x + 0.3 } else { x - 0.3 })
}

/// A guardian's charged beam: a warning line for `warn` ticks from
/// `from`, then a burn of `burn` ticks that sweeps `arc` radians starting
/// at `angle`.
pub(super) fn charged_beam(
    grid: &Grid,
    from: (f32, f32),
    angle: f32,
    arc: f32,
    warn: u8,
    burn: u8,
    kind: Shot,
    damage: u32,
) -> Projectile {
    let burn = burn.max(2);
    let aim = Aim {
        from,
        at: from,
        pace: (0.0, 0.0),
        grid,
        count: 0,
        salt: 0,
        step: 0,
        kind,
        damage,
        guardian: None,
    };
    beam(&aim, angle, arc / f32::from(burn - 1), warn, burn)
}

fn beam(aim: &Aim, angle: f32, sweep: f32, warn: u8, burn: u8) -> Projectile {
    let path = Path {
        beam: Some(Beam {
            angle,
            reach: reach(aim.grid, aim.from, angle),
            sweep,
            warn,
            burn,
            struck: 0,
        }),
        ..Path::default()
    };
    let mut shot = aim.shot(aim.from, angle, 0.0, path, 0);
    shot.ttl = u32::from(warn) + u32::from(burn) + 1;
    shot
}

/// How far a line from `from` runs before stone.
pub(super) fn reach(grid: &Grid, from: (f32, f32), angle: f32) -> f32 {
    let (ux, uy) = (angle.cos(), angle.sin());
    let mut d = 0.0;
    while d < BEAM_REACH {
        d += 0.25;
        if grid.stops_shot(from.0 + ux * d, from.1 + uy * d) {
            return d - 0.25;
        }
    }
    BEAM_REACH
}

/// The renderer clips every ray of a sweep's warning to the same stone.
pub fn warning_reach(run: &Run, from: (f32, f32), angle: f32) -> f32 {
    let grid = Grid {
        room: run.room(),
        barred: run.barred(),
    };
    reach(&grid, from, angle)
}

/// What `steer` left a shot to do this tick.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Steer {
    /// Fly and strike as any shot does.
    Fly,
    /// Waiting: neither moves nor strikes.
    Hold,
}

/// A pattern shot's step before it moves: its wait, its speed, its turns,
/// its weave and its fuse. Bursts go into `pops`. A shot with an empty
/// path is left as it was.
pub(super) fn steer(
    bullet: &mut Projectile,
    knights: &[(u32, f32, f32)],
    pops: &mut Vec<Projectile>,
) -> Steer {
    let path = &mut bullet.traits.path;
    if path.wait > 0 {
        path.wait -= 1;
        path.held = true;
        bullet.ttl = bullet.ttl.saturating_sub(1);
        return Steer::Hold;
    }
    path.held = false;
    if *path == Path::default() {
        return Steer::Fly;
    }
    path.age = path.age.saturating_add(1);
    let speed = bullet.vx.hypot(bullet.vy);
    let mut heading = bullet.vy.atan2(bullet.vx);
    let mut next = speed;
    if path.accel != 0.0 {
        let change = path.accel.abs() * DT;
        next = if speed < path.top {
            (speed + change).min(path.top)
        } else {
            (speed - change).max(path.top)
        };
    }
    heading += path.turn * DT;
    if path.seek > 0.0
        && let Some(&(_, kx, ky)) = knights.iter().min_by(|a, b| {
            (a.1 - bullet.x)
                .hypot(a.2 - bullet.y)
                .total_cmp(&(b.1 - bullet.x).hypot(b.2 - bullet.y))
        })
    {
        let want = (ky - bullet.y).atan2(kx - bullet.x);
        let off = (want - heading).sin().atan2((want - heading).cos());
        let most = path.seek * DT;
        heading += off.clamp(-most, most);
    }
    if speed > 0.0 || next > 0.0 {
        (bullet.vx, bullet.vy) = (heading.cos() * next, heading.sin() * next);
    }
    if path.wave != 0.0 && next > 0.0 {
        let t = f32::from(path.age);
        let side = path.wave * ((path.wave_rate * t).sin() - (path.wave_rate * (t - 1.0)).sin());
        bullet.x += -heading.sin() * side;
        bullet.y += heading.cos() * side;
    }
    if path.fuse > 0 && path.age == path.fuse {
        match path.then {
            Then::Nothing => {}
            Then::Aim(fast) => {
                let to = knights
                    .iter()
                    .min_by(|a, b| {
                        (a.1 - bullet.x)
                            .hypot(a.2 - bullet.y)
                            .total_cmp(&(b.1 - bullet.x).hypot(b.2 - bullet.y))
                    })
                    .map(|&(_, kx, ky)| (ky - bullet.y).atan2(kx - bullet.x))
                    .unwrap_or(heading);
                (bullet.vx, bullet.vy) = (to.cos() * fast, to.sin() * fast);
                path.accel = 0.0;
                path.top = fast;
            }
            Then::Burst(n, fast) => {
                let turn = (mix(u64::from(path.age)
                    ^ (bullet.x.to_bits() as u64) << 20
                    ^ u64::from(bullet.y.to_bits()))
                    % 628) as f32
                    / 100.0;
                for i in 0..n {
                    let a = turn + f32::from(i) * TAU / f32::from(n);
                    pops.push(Projectile {
                        x: bullet.x,
                        y: bullet.y,
                        vx: a.cos() * fast,
                        vy: a.sin() * fast,
                        hostile: bullet.hostile,
                        look: super::bosses::Art::of_shot(bullet).and_then(|art| art.look()),
                        kind: bullet.kind,
                        damage: bullet.damage,
                        pierce: 0,
                        last_hit: None,
                        empowered: false,
                        // A borrowed shot's pieces are the knight's too.
                        traits: ShotTraits {
                            owner: bullet.traits.owner,
                            borrowed: bullet.traits.borrowed,
                            ..ShotTraits::default()
                        },
                        ttl: 5 * HZ,
                    });
                }
                bullet.ttl = 0;
                return Steer::Hold;
            }
        }
    }
    Steer::Fly
}

/// A beam's tick: its warning, then its burn down the line (turning as a
/// sweep does), striking each knight it crosses once. A raised shield
/// facing it stops it at the face (`shield`): while the shield holds, the
/// beam reaches no further down its line, and nobody behind is touched.
pub(super) fn shine(
    bullet: &mut Projectile,
    players: &mut BTreeMap<u32, Hero>,
    grid: &Grid,
    sounds: &mut Vec<&'static str>,
    feel: &mut feel::Feel,
) {
    let Some(mut beam) = bullet.traits.path.beam else {
        return;
    };
    bullet.traits.path.held = false;
    bullet.ttl = bullet.ttl.saturating_sub(1);
    if !bullet.hostile {
        // A beam turned is gone.
        bullet.ttl = 0;
        return;
    }
    let age = bullet.traits.path.age.saturating_add(1);
    bullet.traits.path.age = age;
    if age > u16::from(beam.warn) + u16::from(beam.burn) {
        bullet.ttl = 0;
        return;
    }
    let origin = (bullet.x, bullet.y);
    if beam.burning(age) {
        if age > u16::from(beam.warn) + 1 {
            beam.angle += beam.sweep;
        }
        beam.reach = reach(grid, origin, beam.angle);
        let (ux, uy) = (beam.angle.cos(), beam.angle.sin());
        let standing = |h: &&Hero| h.hp > 0 && !h.stone;
        // The nearest raised shield on its line, facing it, and how far down
        // the line its face stands.
        let stop = players
            .iter()
            .filter(|(_, h)| standing(h))
            .filter(|(_, h)| beam.crosses(origin, (h.x, h.y)) && shield::turns(h, (ux, uy)))
            .map(|(&id, h)| {
                let along = (h.x - origin.0) * ux + (h.y - origin.1) * uy;
                (id, (along - shield::SHIELD_REACH).max(0.0))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((_, d)) = stop {
            beam.reach = beam.reach.min(d);
        }
        for (&id, hero) in players.iter_mut().filter(|(_, h)| standing(&&**h)) {
            let bit = 1u8 << (id.saturating_sub(1) % 8);
            if beam.struck & bit != 0 {
                continue;
            }
            let shielded = stop.is_some_and(|(s, _)| s == id);
            if !shielded && !beam.crosses(origin, (hero.x, hero.y)) {
                continue;
            }
            beam.struck |= bit;
            if shielded {
                let at = (origin.0 + ux * beam.reach, origin.1 + uy * beam.reach);
                sounds.push("shield_block");
                feel.block(id, at, (-ux, -uy), (ux, uy), bullet.kind, 3);
            } else if bullet.traits.blow != 0 {
                if hero.blow(bullet.damage, bullet.traits.blow) {
                    sounds.push("hero_hurt");
                }
            } else if hero.invulnerable == 0 {
                hero.hurt(bullet.damage * BEAM_SHARE / 100);
                sounds.push("hero_hurt");
            }
        }
    } else {
        beam.reach = reach(grid, origin, beam.angle);
    }
    bullet.traits.path.beam = Some(beam);
}

/// A borrowed beam's tick (`bodies`): its warning, then its burn down the
/// line, striking each monster it crosses once. A tower shield facing it
/// turns it, and counts it (`critters::guard_struck`). Who it struck and
/// what each took.
pub(super) fn shine_foes(bullet: &mut Projectile, enemies: &mut [Enemy], grid: &Grid) -> Vec<(usize, u32)> {
    let mut struck = Vec::new();
    let Some(mut beam) = bullet.traits.path.beam else {
        return struck;
    };
    bullet.traits.path.held = false;
    bullet.ttl = bullet.ttl.saturating_sub(1);
    let age = bullet.traits.path.age.saturating_add(1);
    bullet.traits.path.age = age;
    if age > u16::from(beam.warn) + u16::from(beam.burn) {
        bullet.ttl = 0;
        return struck;
    }
    let origin = (bullet.x, bullet.y);
    if beam.burning(age) {
        if age > u16::from(beam.warn) + 1 {
            beam.angle += beam.sweep;
        }
        beam.reach = reach(grid, origin, beam.angle);
        let (ux, uy) = (beam.angle.cos(), beam.angle.sin());
        for (i, enemy) in enemies.iter_mut().enumerate() {
            if enemy.hp == 0 || enemy.buried() || bullet.traits.pierced.contains(&enemy.id) {
                continue;
            }
            let (rx, ry) = (enemy.x - origin.0, enemy.y - origin.1);
            let along = (rx * ux + ry * uy).clamp(0.0, beam.reach);
            if (rx - ux * along).hypot(ry - uy * along) > enemy.radius() + BEAM_HALF {
                continue;
            }
            bullet.traits.pierced.push(enemy.id);
            if critters::blocks(enemy, ux, uy) {
                critters::guard_struck(enemy, bullet.damage);
                continue;
            }
            let dealt = bullet.damage.min(enemy.hp);
            enemy.hp -= dealt;
            struck.push((i, dealt));
        }
    } else {
        beam.reach = reach(grid, origin, beam.angle);
    }
    bullet.traits.path.beam = Some(beam);
    struck
}

/// Who the monsters see this tick: the knights (and what stands in for
/// them), how each is running, the floor, and how many knights stand.
pub(super) struct Sight<'a> {
    pub(super) knights: &'a [(f32, f32)],
    pub(super) paces: &'a [(f32, f32)],
    pub(super) pack: Pack,
    pub(super) depth: u32,
    pub(super) heads: usize,
}

impl Sight<'_> {
    /// The thickness a volley gets on this floor, with this many knights,
    /// from an elite or not.
    pub(super) fn step(&self, elite: bool) -> u8 {
        let floor = DEPTH_STEP[(self.depth.clamp(1, 6) - 1) as usize];
        let heads = HEAD_STEP[self.heads.min(HEAD_STEP.len() - 1)];
        (floor + heads + u8::from(elite)).min(MOST_STEP)
    }
}

/// A monster's signature: ticks between its volleys, the emitters it walks
/// through one volley each (and the shot each throws), and what each shot
/// deals.
#[derive(Clone, Copy, Debug)]
pub struct Signature {
    pub every: u32,
    pub chain: &'static [(Emitter, Shot)],
    pub damage: u32,
}

const fn sig(every: u32, damage: u32, chain: &'static [(Emitter, Shot)]) -> Option<Signature> {
    Some(Signature {
        every,
        chain,
        damage,
    })
}

/// Each kind's signature on each floor; an elite's walks a longer chain.
/// Kinds whose threat is something else (a charge, a hook, a bell, a
/// bomb, a hex) have none.
pub fn signature(kind: EnemyKind, pack: Pack, elite: bool) -> Option<Signature> {
    use Emitter::*;
    use EnemyKind as K;
    use Pack as P;
    use Shot::{Ball, Bone, Ember, Eye, Flame, Ore, Page, Spore, Wisp};
    match (kind, pack, elite) {
        // Bones in a stream; in the Mines they ricochet; in the Archive,
        // torn pages hook in.
        (K::Skeleton, P::Cavern, false) => sig(70, 12, &[(Bounce, Ore)]),
        (K::Skeleton, P::Cavern, true) => sig(60, 12, &[(Bounce, Ore), (Lead, Ore)]),
        (K::Skeleton, P::Archive, false) => sig(70, 12, &[(Curve, Page)]),
        (K::Skeleton, P::Archive, true) => sig(60, 12, &[(Curve, Page), (Darts, Page)]),
        // Everywhere else a skeleton looses thin, fast darts; an elite
        // mixes in a plain stream and a ring that stops and comes again.
        (K::Skeleton, _, false) => sig(60, 12, &[(Darts, Bone)]),
        (K::Skeleton, _, true) => sig(55, 12, &[(Darts, Bone), (Burst, Bone), (Reaim, Wisp)]),
        // The shade: slow ghost rings with a way through; in the Archive a
        // weaving stream; in Dragon Keep rings that stop and come again;
        // in the Unknown an eye that stares a line.
        (K::Wraith, P::Archive, false) => sig(75, 12, &[(Weave, Page)]),
        (K::Wraith, P::Archive, true) => sig(70, 12, &[(Weave, Page), (GapRing, Page)]),
        (K::Wraith, P::Hellforge, false) => sig(80, 12, &[(Reaim, Flame)]),
        (K::Wraith, P::Hellforge, true) => sig(70, 12, &[(Reaim, Flame), (Orbs, Flame)]),
        (K::Wraith, P::Unknown, false) => sig(80, 12, &[(Laser, Eye), (GapRing, Eye)]),
        (K::Wraith, P::Unknown, true) => sig(70, 12, &[(Laser, Eye), (GapRing, Eye), (Reaim, Eye)]),
        (K::Wraith, _, false) => sig(80, 12, &[(GapRing, Wisp)]),
        (K::Wraith, _, true) => sig(70, 12, &[(GapRing, Wisp), (Flower, Wisp)]),
        // The imp: a shotgun where you are going; in Dragon Keep a ring
        // that quickens; in the Fungal Deep spores that burst; in the
        // Unknown hooks.
        // In the Mines it lobs lumps of ore that come down where a knight
        // stood.
        (K::Imp, P::Cavern, false) => sig(70, 12, &[(Lob, Ore)]),
        (K::Imp, P::Cavern, true) => sig(60, 12, &[(Lob, Ore), (Lead, Ore)]),
        (K::Imp, P::Hellforge, false) => sig(70, 12, &[(Surge, Flame)]),
        (K::Imp, P::Hellforge, true) => sig(60, 12, &[(Surge, Flame), (Lob, Ember)]),
        (K::Imp, P::Fungal, false) => sig(75, 12, &[(Split, Spore)]),
        (K::Imp, P::Fungal, true) => sig(65, 12, &[(Split, Spore), (Mines, Spore)]),
        (K::Imp, P::Unknown, false) => sig(70, 12, &[(Curve, Eye)]),
        (K::Imp, P::Unknown, true) => sig(60, 12, &[(Curve, Eye), (Lead, Eye)]),
        (K::Imp, _, false) => sig(70, 12, &[(Lead, Ember)]),
        (K::Imp, _, true) => sig(60, 12, &[(Lead, Ember), (Split, Ember)]),
        // The greater demon: spirals, a sweeping fire beam, a shotgun.
        (K::Demon, P::Unknown, false) => sig(60, 16, &[(GapRing, Eye), (Sweep, Eye), (Surge, Eye)]),
        (K::Demon, P::Unknown, true) => sig(
            55,
            16,
            &[(GapRing, Eye), (Sweep, Eye), (Surge, Eye), (Laser, Eye)],
        ),
        // Slower and lighter than it was (the operator, 2026-10-18: "the
        // demons in lvl 3 are bad"), its health untouched: a volley every
        // three seconds, and see `DEMON_STEP`.
        (K::Demon, _, false) => sig(
            90,
            12,
            &[(TwinSpiral, Flame), (Sweep, Flame), (Drift, Ember)],
        ),
        (K::Demon, _, true) => sig(
            80,
            12,
            &[
                (TwinSpiral, Flame),
                (Sweep, Flame),
                (Rain, Flame),
                (Flower, Flame),
            ],
        ),
        // The mimic spits its coins: they ricochet, or arc down where you
        // stand.
        (K::Mimic, _, _) => sig(80, 10, &[(Bounce, Ball), (Lob, Ball)]),
        // The lair's master: sweeping fire, fire from the roof, spirals,
        // a beam, a shotgun.
        (K::Dragon, _, _) => sig(
            45,
            16,
            &[
                (Sweep, Flame),
                (Rain, Flame),
                (TwinSpiral, Flame),
                (Laser, Flame),
            ],
        ),
        // The Pit Tyrant breathes leads, sends a broken wall and quickens
        // a ring between its slams.
        (K::PitTyrant, _, _) => sig(90, 14, &[(Lead, Flame), (GapWall, Flame), (Surge, Flame)]),
        // The necromancer's orbs: slow and heavy; in the Archive they
        // seek; in the Unknown they hang and pop, or stop and come again.
        (K::Necromancer, P::Archive, _) => sig(75, 14, &[(Seeker, Page)]),
        (K::Necromancer, P::Unknown, _) => sig(85, 14, &[(Mines, Eye), (Reaim, Eye)]),
        (K::Necromancer, _, _) => sig(75, 14, &[(Orbs, Wisp), (Reaim, Wisp)]),
        // The shaman calls fire across the room from the side walls.
        (K::Shaman, P::Fungal, _) => sig(90, 12, &[(Crossfire, Spore)]),
        (K::Shaman, P::Unknown, _) => sig(90, 12, &[(Crossfire, Eye)]),
        (K::Shaman, _, _) => sig(90, 12, &[(Crossfire, Flame)]),
        // Egg sacs that hang and burst.
        (K::Silkmother, P::Fungal, _) => sig(150, 12, &[(Mines, Spore)]),
        (K::Silkmother, _, _) => sig(150, 12, &[(Mines, Page)]),
        // The bell's ring goes out as a ring with a way through it.
        (K::Ringer, P::Archive, _) => sig(120, 10, &[(GapRing, Page)]),
        (K::Ringer, P::Cavern, _) => sig(120, 10, &[(GapRing, Ore)]),
        (K::Ringer, _, _) => sig(120, 10, &[(GapRing, Wisp)]),
        // The shield line pushes a wall of shots with a gap in it.
        (K::Pavise, P::Hellforge, _) => sig(110, 12, &[(GapWall, Flame)]),
        (K::Pavise, P::Unknown, _) => sig(110, 12, &[(GapWall, Eye)]),
        (K::Pavise, _, _) => sig(110, 12, &[(GapWall, Bone)]),
        // A spitter in the Fungal Deep spits spores that burst, and lobs
        // them.
        (K::Spitter, P::Fungal, _) => sig(130, 12, &[(Split, Spore), (Lob, Spore)]),
        // A big slime in the Fungal Deep sheds spore mines.
        (K::Slime, P::Fungal, _) => sig(140, 10, &[(Mines, Spore)]),
        // The Hollow One's void looks out down a sweeping line.
        (K::Hollow, _, _) => sig(140, 14, &[(Sweep, Eye)]),
        // Grave-knights sworn together throw their spirals.
        (K::Oathbound, _, _) => sig(110, 14, &[(TwinSpiral, Eye), (Drift, Eye)]),
        _ => None,
    }
}

/// A monster's volley this tick, if its signature's beat has come; a
/// guardian's from its `.boss` file's named patterns and phases.
pub(super) fn volley(
    enemy: &mut Enemy,
    sight: &Sight,
    grid: &Grid,
    seed: u64,
    bosses: &[Boss],
    shots: &mut Vec<Projectile>,
) {
    let Some((i, at)) = sight.knights.iter().copied().enumerate().min_by(|a, b| {
        (a.1.0 - enemy.x)
            .hypot(a.1.1 - enemy.y)
            .total_cmp(&(b.1.0 - enemy.x).hypot(b.1.1 - enemy.y))
    }) else {
        return;
    };
    let pace = sight.paces.get(i).copied().unwrap_or((0.0, 0.0));
    // A guardian's floor and the party's size are already in its health
    // and its blows; only a full party thickens its patterns.
    let step = if enemy.boss.is_some() {
        BOSS_HEAD_STEP[sight.heads.min(BOSS_HEAD_STEP.len() - 1)]
    } else if enemy.kind == EnemyKind::Demon {
        sight.step(enemy.elite).min(DEMON_STEP)
    } else {
        sight.step(enemy.elite)
    };
    let salt = seed ^ u64::from(enemy.id) << 24;
    let funnel = enemy
        .boss
        .and_then(|i| bosses.get(i as usize))
        .is_some_and(|b| b.id == signatures::ENGINE)
        && grid.room.kind != RoomKind::Wanted;
    let nozzle = (enemy.x - 0.3, enemy.y - 2.2);
    let from = if funnel && grid.shot_stop((enemy.x, enemy.y), nozzle).is_none() {
        nozzle
    } else {
        (enemy.x, enemy.y)
    };
    let aim = |count: u32, kind: Shot, damage: u32, guardian: Option<u32>| Aim {
        from,
        at,
        pace,
        grid,
        count,
        salt,
        step,
        kind,
        damage,
        guardian,
    };
    if let Some(boss) = enemy.boss.and_then(|b| bosses.get(usize::from(b))) {
        if grid.room.kind == RoomKind::Wanted && !outlaw::shooting(enemy.stage) {
            // An outlaw shoots only from its shooting states.
            return;
        }
        let wanted = grid.room.kind == RoomKind::Wanted;
        if !signatures::shooting(enemy, boss, wanted) {
            return;
        }
        let first = shots.len();
        // In its own room a guardian's fight builds stage by stage: each
        // line joins at its `from`, its volleys are a guardian's size, and
        // only its rage quickens it. An outlaw fights as it always has.
        let stage = if wanted {
            bosses::Stage::Rage
        } else {
            boss.stage(enemy.hp, enemy.max_hp)
        };
        let raging = if wanted {
            boss.rage > 0.0 && (enemy.hp as f32) < boss.rage * enemy.max_hp as f32
        } else {
            stage == bosses::Stage::Rage
        };
        let quicken = |every: u32| {
            let every = if raging {
                (every * 2 / 3).max(4)
            } else {
                every.max(4)
            };
            if wanted {
                enemy.beat(every) * outlaw::pattern_pace(enemy.stage)
            } else {
                every
            }
        };
        let size = |emitter: Emitter| (!wanted).then(|| emitter.guardian_count());
        for (index, attack) in boss.attacks.iter().enumerate() {
            let bosses::Pattern::Named(emitter) = attack.pattern else {
                continue;
            };
            if attack.from > stage {
                continue;
            }
            let every = quicken(attack.every);
            let tick = enemy.age + index as u32 * 11;
            if tick.is_multiple_of(every) {
                let kind = attack.bolt.shot(boss.only_in);
                emit(emitter, &aim(tick / every, kind, attack.damage, size(emitter)), shots);
            }
        }
        if let Some(phases) = boss.phases.as_ref().filter(|p| p.from <= stage) {
            let every = quicken(phases.every);
            let tick = enemy.age + 7;
            if tick.is_multiple_of(every) {
                let count = enemy.pattern_volleys;
                let phase = (count / phases.volleys.max(1)) as usize % phases.chain.len().max(1);
                if let Some(&emitter) = phases.chain.get(phase) {
                    let kind = phases.bolt.shot(boss.only_in);
                    emit(emitter, &aim(count, kind, phases.damage, size(emitter)), shots);
                    enemy.pattern_volleys = enemy.pattern_volleys.wrapping_add(1);
                }
            }
        }
        if boss.id == signatures::ENGINE {
            for shot in &mut shots[first..] {
                shot.look = Some(("soot".into(), "coal".into()));
            }
        }
        return;
    }
    if (enemy.kind == EnemyKind::Slime && enemy.stage != 0)
        || (enemy.kind == EnemyKind::Hollow && enemy.stage == 1)
        || (enemy.kind == EnemyKind::PitTyrant && enemy.stage != 0)
    {
        return;
    }
    let Some(sig) = signature(enemy.kind, sight.pack, enemy.elite) else {
        return;
    };
    let starter = match sight.depth {
        1 | 2 => STARTER_PACE[sight.depth as usize - 1],
        _ => 100,
    };
    let every = enemy.beat(sig.every * starter / 100);
    let tick = enemy.age + enemy.id * 13;
    if !tick.is_multiple_of(every) || sig.chain.is_empty() {
        return;
    }
    let count = enemy.pattern_volleys;
    let (emitter, kind) = sig.chain[count as usize % sig.chain.len()];
    emit(emitter, &aim(count, kind, sig.damage, None), shots);
    enemy.pattern_volleys = enemy.pattern_volleys.wrapping_add(1);
}

#[cfg(test)]
#[path = "../../tests/rules/together_patterns__tests.rs"]
mod tests;
