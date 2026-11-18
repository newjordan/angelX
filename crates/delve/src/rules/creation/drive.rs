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

pub const WINDUP: u64 = 8;
pub const STRIKE: u64 = 6;
pub const RECOVER: u64 = 6;
pub const ATTACK_TICKS: u64 = WINDUP + STRIKE + RECOVER;

pub const SETTLE: u64 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blow {
    pub windup: u64,
    pub strike: u64,
    pub recover: u64,
    pub settle: u64,
}

impl Blow {
    pub const FIXED: Blow = Blow { windup: WINDUP, strike: STRIKE, recover: RECOVER, settle: 0 };

    pub const fn clip(self) -> u64 {
        self.windup + self.strike + self.recover
    }

    pub const fn ticks(self) -> u64 {
        self.clip() + self.settle
    }
}

impl Default for Blow {
    fn default() -> Self {
        Self::FIXED
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shot {
    pub ticks: u64,
    pub event: u64,
    pub settle: u64,
}

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

pub const FOE_REACH: f32 = 9.0;
pub const FOE_EVERY: u64 = 75;
pub const FOE_TELL: u64 = 14;
pub const FOE_DAMAGE: u16 = 6;
const FOES: usize = 4;

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

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn nth(i: usize) -> Way {
        Self::ALL[i % 8]
    }

    pub fn short(self) -> &'static str {
        ["s", "sw", "w", "nw", "n", "ne", "e", "se"][self.index()]
    }

    pub fn from_short(name: &str) -> Option<Way> {
        Self::ALL.into_iter().find(|w| w.short() == name)
    }

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

    pub fn axes(self) -> (i8, i8) {
        [(0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1), (1, 0), (1, 1)][self.index()]
    }

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriveInput {
    pub dx: i8,
    pub dy: i8,
    pub attack: bool,
    pub interact: bool,
    pub run: bool,
    pub cast: bool,
    pub fall: bool,
}

pub const RUN_SPEED: f32 = 1.6;

pub struct Ground<'a> {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hurt {
    pub foe: usize,
    pub damage: u16,
    pub tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Cast,
    Hit,
    Fell,
    Rose,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Struck(Hit),
    Missed,
    Spoke(u8),
    Alone,
    Cast,
    Hurt(Hurt),
    Fell,
    Rose,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drive {
    pub x: f32,
    pub y: f32,
    pub facing: i8,
    pub moving: bool,
    pub vertical: bool,
    pub way: Way,
    pub running: bool,
    pub walked: f32,
    pub attack_at: Option<u64>,
    pub strikes: u32,
    pub last_hit: Option<Hit>,
    pub spoke: Option<(u64, u8)>,
    pub born: u64,
    pub timing: Timing,
    pub act: Option<(Act, u64)>,
    pub hurt: Option<Hurt>,
    pub hurts: u32,
    foe_clock: [u16; FOES],
    speed: f32,
    reach: f32,
    damage: u16,
    talks: u8,
}

impl Drive {
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

    pub fn blow_age(&self, tick: u64) -> Option<u64> {
        let blow = self.timing.blow;
        let a = tick.checked_sub(self.attack_at?)?;
        (blow.windup..blow.ticks()).contains(&a).then(|| a - blow.windup)
    }

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

    fn act_span(&self, act: Act) -> Option<(u64, u64)> {
        let shot = match act {
            Act::Cast => self.timing.cast,
            Act::Hit => self.timing.hit,
            Act::Fell | Act::Rose => self.timing.death,
        }?;
        Some((shot.ticks, shot.settle))
    }

    pub fn acting(&self, tick: u64) -> Option<(Act, u64)> {
        let (act, at) = self.act?;
        let since = tick.checked_sub(at)?;
        let (clip, settle) = self.act_span(act)?;
        match act {
            Act::Fell => Some((act, since)),
            _ => (since < clip + settle).then_some((act, since)),
        }
    }

    fn act_rooted(&self, tick: u64) -> bool {
        match self.acting(tick) {
            Some((Act::Fell, _)) => true,
            Some((act, since)) => self.act_span(act).is_some_and(|(clip, _)| since < clip),
            None => false,
        }
    }

    pub fn settling(&self, tick: u64) -> bool {
        match self.acting(tick) {
            Some((Act::Fell, _)) | None => false,
            Some((act, since)) => self.act_span(act).is_some_and(|(clip, _)| since >= clip),
        }
    }

    pub fn down(&self, tick: u64) -> bool {
        matches!(self.acting(tick), Some((Act::Fell, _)))
    }

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
            self.act = None;
        }
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
