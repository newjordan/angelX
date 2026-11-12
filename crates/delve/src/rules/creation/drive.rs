//! The Test drive's rules: a made creature walking and striking on a fixture floor.
//!
//! Pure data. It reads a floor only through [`Ground`], never touches a `Run`, and a strike is
//! windup, then strike, then recovery, so the frames the art supplies (rest, windup, strike) land
//! on named ticks. Nothing here can reach the live fight.
//!
//! The blow's three spans are the library's fixed 8, 6 and 6 ticks ([`Blow::FIXED`]) unless the host
//! says the art brings clips with timing of their own ([`Timing`], set by the host from the set it
//! shows): then the blow lands on the clip's own impact frame, the spans are the clip's, and a
//! creature that has a cast, a hit and a death clip can play them too (the cast key, a training foe
//! that strikes back on a timer, the fall key). The fixed timeline is what every older set plays,
//! to the tick.

use super::Stats;

/// Ticks at 30 Hz: the windup frame, the strike frame (the blow lands on its first tick), the recovery.
pub const WINDUP: u64 = 8;
pub const STRIKE: u64 = 6;
pub const RECOVER: u64 = 6;
pub const ATTACK_TICKS: u64 = WINDUP + STRIKE + RECOVER;

/// Ticks the last frame of a clip melts into the standing pose over, on a set that has its own clips.
pub const SETTLE: u64 = 6;

/// How long the blow's three spans are, and the settle after them (none, on the fixed timeline).
/// The creature is rooted through windup and strike, free in recovery and settle; the blow lands on
/// the first tick of the strike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blow {
    pub windup: u64,
    pub strike: u64,
    pub recover: u64,
    pub settle: u64,
}

impl Blow {
    /// The library's own: 8, 6, 6 and no settle.
    pub const FIXED: Blow = Blow { windup: WINDUP, strike: STRIKE, recover: RECOVER, settle: 0 };

    /// Ticks of the clip itself (windup, strike and recovery).
    pub const fn clip(self) -> u64 {
        self.windup + self.strike + self.recover
    }

    /// Ticks until the creature may strike again.
    pub const fn ticks(self) -> u64 {
        self.clip() + self.settle
    }
}

impl Default for Blow {
    fn default() -> Self {
        Self::FIXED
    }
}

/// A one-shot clip's own timing: ticks it plays, the tick of its event (a cast's release), the settle after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shot {
    pub ticks: u64,
    pub event: u64,
    pub settle: u64,
}

/// What a set's art says about time. A set without its own clips has the fixed blow and no cast,
/// hit or death: its drive is what it always was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub blow: Blow,
    pub cast: Option<Shot>,
    pub hit: Option<Shot>,
    pub death: Option<Shot>,
}

impl Timing {
    pub const FIXED: Timing = Timing { blow: Blow::FIXED, cast: None, hit: None, death: None };
}

/// A foe in this reach, still, strikes back every `FOE_EVERY` ticks (a creature with a hit clip only).
pub const FOE_REACH: f32 = 9.0;
pub const FOE_EVERY: u64 = 75;
/// The last ticks before the blow, the foe's tell.
pub const FOE_TELL: u64 = 14;
pub const FOE_DAMAGE: u16 = 6;
/// Foes that keep a clock (a floor has two).
const FOES: usize = 4;

/// Which way a creature faces: the eight compass points, in the order the pipeline's frame sets name
/// their directions (clockwise on the screen from the camera: south is toward us, east is to the
/// right). The drive faces where it walks; a set with only the two old ways (east, south) is shown
/// through [`Way::flat`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Way {
    #[default]
    South,
    SouthWest,
    West,
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
}

impl Way {
    pub const ALL: [Way; 8] = [Way::South, Way::SouthWest, Way::West, Way::NorthWest, Way::North, Way::NorthEast, Way::East, Way::SouthEast];

    /// Position in [`Way::ALL`] (south 0, then round by the clock: south-west 1 ... south-east 7).
    pub fn index(self) -> usize {
        self as usize
    }

    /// `ALL[i % 8]`: a turntable counts round without end.
    pub fn nth(i: usize) -> Way {
        Self::ALL[i % 8]
    }

    /// The frame sets' own name for it: `s sw w nw n ne e se`.
    pub fn short(self) -> &'static str {
        ["s", "sw", "w", "nw", "n", "ne", "e", "se"][self.index()]
    }

    pub fn from_short(name: &str) -> Option<Way> {
        Self::ALL.into_iter().find(|w| w.short() == name)
    }

    /// The way the hands face: right is east and down the floor (toward the camera) is south.
    /// No arrow, no way.
    pub fn from_axes(dx: i8, dy: i8) -> Option<Way> {
        match (dx.signum(), dy.signum()) {
            (0, 1) => Some(Way::South),
            (-1, 1) => Some(Way::SouthWest),
            (-1, 0) => Some(Way::West),
            (-1, -1) => Some(Way::NorthWest),
            (0, -1) => Some(Way::North),
            (1, -1) => Some(Way::NorthEast),
            (1, 0) => Some(Way::East),
            (1, 1) => Some(Way::SouthEast),
            _ => None,
        }
    }

    /// The arrows that point this way (the inverse of `from_axes`): right is +x, toward the camera +y.
    pub fn axes(self) -> (i8, i8) {
        [(0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1), (1, 0), (1, 1)][self.index()]
    }

    /// The old art's two ways that stand for this one, and whether the art is mirrored: north and south
    /// are the front walk, every way with a left part is the east walk turned about.
    pub fn flat(self) -> (Way, bool) {
        match self {
            Way::South | Way::North => (Way::South, false),
            Way::East | Way::NorthEast | Way::SouthEast => (Way::East, false),
            Way::West | Way::NorthWest | Way::SouthWest => (Way::East, true),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pose {
    #[default]
    Rest,
    Windup,
    Strike,
}

/// One tick of a player's hands, in the game's own keys: the arrows or WASD move, F strikes, E meets,
/// R casts (the game's ultimate key) and X falls and gets up again (the bomb key).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriveInput {
    pub dx: i8,
    pub dy: i8,
    pub attack: bool,
    pub interact: bool,
    /// Shift held: a run, a little faster, drawn with the set's run clip where it has one.
    pub run: bool,
    /// Cast (R): plays the set's cast clip, where it has one.
    pub cast: bool,
    /// Fall (X): plays the set's death clip and stays down; pressed again, it gets up.
    pub fall: bool,
}

/// How much faster a run is than a walk.
pub const RUN_SPEED: f32 = 1.6;

/// What the floor says: where feet may stand, where the training foes and the comparison knight are.
pub struct Ground<'a> {
    /// Where a creature first stands.
    pub spawn: (f32, f32),
    pub walkable: &'a dyn Fn(f32, f32) -> bool,
    pub foes: &'a [(f32, f32)],
    pub friend: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub foe: usize,
    pub damage: u16,
    pub tick: u64,
}

/// A training foe's blow on the creature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hurt {
    pub foe: usize,
    pub damage: u16,
    pub tick: u64,
}

/// What a creature is in the middle of, besides walking and striking: a clip of the set's own, played once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Cast,
    /// A training foe struck it.
    Hit,
    /// It fell (the death clip) and lies there.
    Fell,
    /// It gets up (the death clip backwards).
    Rose,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Struck(Hit),
    Missed,
    /// The friend answered; the number picks the line.
    Spoke(u8),
    Alone,
    /// The cast began.
    Cast,
    /// A training foe struck it.
    Hurt(Hurt),
    Fell,
    Rose,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    /// The feet, in run units.
    pub x: f32,
    pub y: f32,
    /// -1 faces left, 1 right; the front-facing art mirrors to it.
    pub facing: i8,
    pub moving: bool,
    /// The last walk went up or down the floor, not along it.
    pub vertical: bool,
    /// Which of the eight ways it faces: where the arrows last pointed (south, toward the camera, at the start).
    /// Sets with eight ways draw it; the two-way art keeps to `facing` and `vertical`.
    pub way: Way,
    /// The walk in progress is a run.
    pub running: bool,
    pub walked: f32,
    pub attack_at: Option<u64>,
    pub strikes: u32,
    pub last_hit: Option<Hit>,
    pub spoke: Option<(u64, u8)>,
    pub born: u64,
    /// How the set's art times a blow and its other clips (the fixed blow where it has none).
    pub timing: Timing,
    /// The clip it is playing besides a blow (a cast, a hit, a fall, a rise) and the tick it began.
    pub act: Option<(Act, u64)>,
    /// The last blow a training foe landed on it, and how many have.
    pub hurt: Option<Hurt>,
    pub hurts: u32,
    /// Ticks each foe has had it in reach since its last blow.
    foe_clock: [u16; FOES],
    speed: f32,
    reach: f32,
    damage: u16,
    talks: u8,
}

impl Drive {
    /// A creature of these derived stats, standing at `at`. Speed 100 is about eight run units a second.
    pub fn new(at: (f32, f32), stats: &Stats, tick: u64) -> Self {
        Self {
            x: at.0,
            y: at.1,
            facing: 1,
            moving: false,
            vertical: false,
            way: Way::South,
            running: false,
            walked: 0.0,
            attack_at: None,
            strikes: 0,
            last_hit: None,
            spoke: None,
            born: tick,
            timing: Timing::FIXED,
            act: None,
            hurt: None,
            hurts: 0,
            foe_clock: [0; FOES],
            speed: 0.28 * f32::from(stats.speed.max(30)) / 100.0,
            reach: 7.5,
            damage: stats.melee.max(1),
            talks: 0,
        }
    }

    pub fn damage(&self) -> u16 {
        self.damage
    }

    /// The art's pose on this tick.
    pub fn pose(&self, tick: u64) -> Pose {
        let blow = self.timing.blow;
        match self.attack_at.map(|at| tick.saturating_sub(at)) {
            Some(a) if a < blow.windup => Pose::Windup,
            Some(a) if a < blow.windup + blow.strike => Pose::Strike,
            _ => Pose::Rest,
        }
    }

    pub fn attacking(&self, tick: u64) -> bool {
        self.attack_at.is_some_and(|at| tick < at + self.timing.blow.ticks())
    }

    /// Age of the blow in ticks, while it is landing.
    pub fn blow_age(&self, tick: u64) -> Option<u64> {
        let blow = self.timing.blow;
        let a = tick.checked_sub(self.attack_at?)?;
        (blow.windup..blow.ticks()).contains(&a).then(|| a - blow.windup)
    }

    /// The host says how the set's art times its clips. It takes hold between blows and clips, so a
    /// look flipped in the middle of one finishes it as it began; a clip the new set lacks ends.
    pub fn retime(&mut self, timing: Timing, tick: u64) {
        if self.timing == timing {
            return;
        }
        if self.act.is_some() && self.acting(tick).is_none() {
            self.act = None;
        }
        if !self.attacking(tick) && self.act.is_none() {
            self.timing = timing;
        } else if let Some((act, _)) = self.act {
            let lacks = match act {
                Act::Cast => timing.cast.is_none(),
                Act::Hit => timing.hit.is_none(),
                Act::Fell | Act::Rose => timing.death.is_none(),
            };
            if lacks {
                self.act = None;
                self.timing = timing;
            }
        }
    }

    /// Ticks the act's clip plays and the settle after it, in the timing the drive holds.
    fn act_span(&self, act: Act) -> Option<(u64, u64)> {
        let shot = match act {
            Act::Cast => self.timing.cast,
            Act::Hit => self.timing.hit,
            Act::Fell | Act::Rose => self.timing.death,
        }?;
        Some((shot.ticks, shot.settle))
    }

    /// The clip it is playing besides a blow, and the ticks since it began: while the clip plays and
    /// through its settle (a fall never ends until it gets up).
    pub fn acting(&self, tick: u64) -> Option<(Act, u64)> {
        let (act, at) = self.act?;
        let since = tick.checked_sub(at)?;
        let (clip, settle) = self.act_span(act)?;
        match act {
            Act::Fell => Some((act, since)),
            _ => (since < clip + settle).then_some((act, since)),
        }
    }

    /// Whether the clip holds it where it stands: a cast, a flinch or a rise while it plays (free in the settle), a fall always.
    fn act_rooted(&self, tick: u64) -> bool {
        match self.acting(tick) {
            Some((Act::Fell, _)) => true,
            Some((act, since)) => self.act_span(act).is_some_and(|(clip, _)| since < clip),
            None => false,
        }
    }

    /// The clip of its act has played and it is melting into the stance: free to walk off.
    pub fn settling(&self, tick: u64) -> bool {
        match self.acting(tick) {
            Some((Act::Fell, _)) | None => false,
            Some((act, since)) => self.act_span(act).is_some_and(|(clip, _)| since >= clip),
        }
    }

    /// It lies where it fell.
    pub fn down(&self, tick: u64) -> bool {
        matches!(self.acting(tick), Some((Act::Fell, _)))
    }

    /// The foes about to strike, and how far into the tell each is, `0..1`.
    pub fn tells(&self) -> Vec<(usize, f32)> {
        if self.timing.hit.is_none() {
            return Vec::new();
        }
        self.foe_clock
            .iter()
            .enumerate()
            .map(|(i, &c)| (i, c))
            .filter(|&(_, c)| u64::from(c) + FOE_TELL > FOE_EVERY && c > 0)
            .map(|(i, c)| (i, (u64::from(c) + FOE_TELL - FOE_EVERY) as f32 / FOE_TELL as f32))
            .collect()
    }

    pub fn step(&mut self, input: DriveInput, tick: u64, ground: &Ground) -> Vec<Event> {
        let mut events = Vec::new();
        let blow = self.timing.blow;
        if self.attack_at.is_some_and(|at| tick >= at + blow.ticks()) {
            self.attack_at = None;
        }
        if self.act.is_some() && self.acting(tick).is_none() {
            self.act = None;
        }
        // The fall key: down, and once it has lain a moment, up again.
        if input.fall
            && let Some(death) = self.timing.death
        {
            match self.act {
                Some((Act::Fell, at)) if tick.saturating_sub(at) >= death.ticks => {
                    self.act = Some((Act::Rose, tick));
                    events.push(Event::Rose);
                }
                None => {
                    self.act = Some((Act::Fell, tick));
                    self.attack_at = None;
                    events.push(Event::Fell);
                }
                _ => {}
            }
        }
        let busy = self.attacking(tick);
        // Rooted through windup and strike, free again in recovery; a clip of its own roots it as it plays.
        let clip_rooted = self.act_rooted(tick);
        let rooted = self.attack_at.is_some_and(|at| tick < at + blow.windup + blow.strike) || clip_rooted;
        self.moving = false;
        self.running = false;
        if !rooted && (input.dx != 0 || input.dy != 0) {
            let (mut vx, mut vy) = (f32::from(input.dx), f32::from(input.dy));
            let n = vx.hypot(vy);
            let speed = if input.run { self.speed * RUN_SPEED } else { self.speed };
            vx = vx / n * speed;
            vy = vy / n * speed;
            // Feet are a small box; each axis slides along a wall on its own.
            let fits = |x: f32, y: f32| {
                [(-1.6, 0.0), (1.6, 0.0), (0.0, 0.0), (0.0, -1.0)]
                    .iter()
                    .all(|(ox, oy)| (ground.walkable)(x + ox, y + oy))
            };
            let start = (self.x, self.y);
            if fits(self.x + vx, self.y) {
                self.x += vx;
            }
            if fits(self.x, self.y + vy) {
                self.y += vy;
            }
            self.moving = (self.x, self.y) != start;
            self.walked += (self.x - start.0).hypot(self.y - start.1);
            if input.dx != 0 {
                self.facing = input.dx.signum();
            }
            // It faces where the hands point, a wall or no wall.
            if let Some(way) = Way::from_axes(input.dx, input.dy) {
                self.way = way;
            }
            if self.moving {
                self.vertical = input.dx == 0;
                self.running = input.run;
            }
        }
        if input.attack && !busy && !clip_rooted {
            self.attack_at = Some(tick);
            // A new blow ends the settle of a clip before it.
            self.act = None;
        }
        // A cast waits for a blow's windup and strike to be done, and ends the recovery of one.
        if input.cast
            && self.timing.cast.is_some()
            && !clip_rooted
            && !self.attack_at.is_some_and(|at| tick < at + blow.windup + blow.strike)
        {
            self.act = Some((Act::Cast, tick));
            self.attack_at = None;
            events.push(Event::Cast);
        }
        if self.attack_at == Some(tick.saturating_sub(blow.windup)) && tick >= blow.windup {
            let reach = self.reach;
            let near = ground
                .foes
                .iter()
                .enumerate()
                .map(|(i, f)| (i, (f.0 - self.x).hypot(f.1 - self.y)))
                .filter(|(_, d)| *d <= reach)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match near {
                Some((foe, _)) => {
                    self.strikes += 1;
                    let hit = Hit {
                        foe,
                        damage: self.damage,
                        tick,
                    };
                    self.last_hit = Some(hit);
                    events.push(Event::Struck(hit));
                }
                None => events.push(Event::Missed),
            }
        }
        if self.timing.hit.is_some() {
            events.extend(self.foes_answer(tick, ground).map(Event::Hurt));
        }
        if input.interact {
            match ground.friend {
                Some(f) if (f.0 - self.x).hypot(f.1 - self.y) <= 9.0 => {
                    self.talks = self.talks.wrapping_add(1);
                    self.spoke = Some((tick, self.talks));
                    events.push(Event::Spoke(self.talks));
                }
                _ => events.push(Event::Alone),
            }
        }
        events
    }

    /// The training foe nearest the creature, if it is in reach, strikes back: it keeps a clock of the ticks
    /// it has had the creature in reach, and at `FOE_EVERY` it lands a blow, which plays the hit clip. Only
    /// one foe answers at a time (the others' clocks stand at nothing), so two foes in reach never land
    /// a blow on the heels of one another. A creature lying down is left alone, and a blow waits for the
    /// creature to finish its own windup, strike, cast or flinch.
    fn foes_answer(&mut self, tick: u64, ground: &Ground) -> Option<Hurt> {
        let blow = self.timing.blow;
        let lying = self.down(tick);
        let guarded = self.attack_at.is_some_and(|at| tick < at + blow.windup + blow.strike) || self.act_rooted(tick);
        let nearest = ground
            .foes
            .iter()
            .enumerate()
            .take(FOES)
            .map(|(i, f)| (i, (f.0 - self.x).hypot(f.1 - self.y)))
            .filter(|(_, d)| *d <= FOE_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
            .filter(|_| !lying);
        for (i, clock) in self.foe_clock.iter_mut().enumerate() {
            if Some(i) != nearest {
                *clock = 0;
            }
        }
        let mut landed = None;
        if let Some(i) = nearest {
            // A blow that is due waits for the creature to be free; it is not lost.
            if u64::from(self.foe_clock[i]) < FOE_EVERY || !guarded {
                self.foe_clock[i] += 1;
            }
            if u64::from(self.foe_clock[i]) >= FOE_EVERY && !guarded {
                self.foe_clock[i] = 0;
                landed = Some(Hurt { foe: i, damage: FOE_DAMAGE, tick });
            }
        }
        if let Some(hurt) = landed {
            self.hurt = Some(hurt);
            self.hurts += 1;
            self.act = Some((Act::Hit, tick));
            self.attack_at = None;
        }
        landed
    }
}
