//! Griffinwing's bodies: the craft, the foes, the bolts, the things on the
//! rail, the Wyrm, and the little lists the stage draws from. Every number is
//! a whole number on the 30 Hz sim, 256 to the unit (the Forest Road's `Q`).
//! Positions are in rail space: `s` along the rail, `x` across it (right is
//! positive) and `z` above it.
use super::*;

/// Fixed point: 256 to the unit.
pub const Q: i32 = 256;
/// A length in tenths of a unit, as fixed point.
pub const fn dm(n: i32) -> i32 {
    n * Q / 10
}
/// The rail's pace, per tick: about ten units a second.
pub const PACE: i32 = 85;
/// Where things appear: this far ahead of the formation.
pub const LEAD: i32 = dm(640);
/// The far reticle's distance ahead of its craft, and the near one's.
pub const FAR: i32 = dm(340);
pub const NEAR: i32 = dm(150);
/// The longest and shortest range a lock will take.
pub const LOCK_MAX: i32 = dm(520);
pub const LOCK_MIN: i32 = dm(15);
/// How many locks one release holds, and how many of them one great body
/// (the Wyrm's lamp) may take.
pub const MAX_LOCKS: usize = 6;
pub const MAX_PER_BODY: u8 = 3;
/// The most of each list a session may hold: what `valid` bounds and the
/// script never passes.
pub const MAX_FOES: usize = 40;
pub const MAX_PROPS: usize = 40;
pub const MAX_BOLTS: usize = 160;
pub const MAX_CALLS: usize = 4;
pub const MAX_FX: usize = 48;
/// A craft's shield, full.
pub const SHIELD: i32 = 100;
/// How many tick marks a held count keeps.
pub const WING: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A hooded snuffer on the wing: the small drone.
    Wisp,
    /// A lantern-bat: flies in convoys, or hunts a wingman and latches on.
    Kite,
    /// A hooded lantern tower on a ledge: slow bolts, a long tell.
    Turret,
    /// A dive-bomber: waits behind a ridge, then dives down a locked lane.
    Bomber,
    /// One of the Wyrm's three lamps: a cold-light shield over a flame.
    Lamp,
}

impl Kind {
    /// Its body's size (radius, a unit and a tenth is `dm(11)`): for hits
    /// and for the lock's reach.
    pub fn radius(self) -> i32 {
        match self {
            Kind::Wisp => dm(9),
            Kind::Kite => dm(8),
            Kind::Turret => dm(16),
            Kind::Bomber => dm(12),
            Kind::Lamp => dm(16),
        }
    }
    /// Its health, before the table's seats are counted in.
    pub fn health(self) -> u32 {
        match self {
            Kind::Wisp => 20,
            Kind::Kite => 20,
            Kind::Turret => 60,
            Kind::Bomber => 40,
            Kind::Lamp => 120,
        }
    }
    /// What it is worth when it falls, for the score and the medal.
    pub fn worth(self) -> u32 {
        match self {
            Kind::Wisp => 10,
            Kind::Kite => 20,
            Kind::Turret => 60,
            Kind::Bomber => 40,
            Kind::Lamp => 500,
        }
    }
    pub fn slot(self) -> &'static str {
        match self {
            Kind::Wisp => "griffinwing.wisp",
            Kind::Kite => "griffinwing.kite",
            Kind::Turret => "griffinwing.turret",
            Kind::Bomber => "griffinwing.bomber",
            Kind::Lamp => "griffinwing.lamp",
        }
    }
}

/// What a foe is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum St {
    /// On the wing, on its way.
    Fly,
    /// Its danger footprint is locked and shown; it will fire (or dive) when
    /// the ticks run out.
    Tell,
    /// Between volleys.
    Rest,
    /// A hunting kite, closing on its victim.
    Hunt,
    /// Latched onto a craft's tail.
    Latched,
    /// Thrown off, or its victim gone: away it goes.
    Flee,
    /// Behind a ridge, unseen and untouchable.
    Hide,
    /// Diving down its lane.
    Dive,
    /// A lamp's cold-light shield is closed: nothing touches it.
    Shut,
    /// A lamp's shield is open for the ticks it has left.
    Open,
    /// A lamp that has gone out.
    Broken,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foe {
    pub id: u16,
    pub kind: Kind,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    pub vs: i32,
    pub vx: i32,
    pub vz: i32,
    pub hp: u32,
    pub full: u32,
    /// The seats its health was provisioned for (`Session::provision`).
    pub seats: u8,
    pub st: St,
    /// What the state has left to run (or the age of a flier).
    pub ticks: u32,
    /// Its locked footprint: where the shot or the dive will land.
    pub fx: i32,
    pub fz: i32,
    /// The craft it hunts or hangs on (0: none).
    pub on: u8,
    /// A white flash after a hit, in ticks.
    pub flash: u8,
    /// Its pattern of flight, and its phase in it.
    pub mode: u8,
    pub phase: u8,
}

impl Foe {
    pub fn radius(&self) -> i32 {
        self.kind.radius()
    }
    pub fn alive(&self) -> bool {
        self.hp > 0 && self.st != St::Broken
    }
    /// Whether a lock may take it: alive, in the open and (a lamp) with its
    /// shield open.
    pub fn lockable(&self) -> bool {
        self.alive() && !matches!(self.st, St::Hide | St::Shut | St::Flee)
    }
}

/// What a bolt is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arm {
    /// A craft's rapid twin bolt.
    Lance,
    /// A released lock: the homing starburst.
    Star,
    /// A wingman's bolt.
    Ally,
    /// A snuffer's small spark: slow, and a bolt will shoot it down.
    Spark,
    /// A Gloam bolt: a turret's, a bomber's, the Wyrm's. A roll reflects it.
    Gloam,
    /// A Gloam bolt turned round by a roll.
    Turned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bolt {
    pub shot: Arm,
    /// Whose it is: a seat (1 to 6) or a wingman (7 to 10); 0 is the foes'.
    pub owner: u8,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    pub vs: i32,
    pub vx: i32,
    pub vz: i32,
    pub dmg: u16,
    pub life: u16,
    /// The foe it homes on (0: none), and the one that fired it (0: none).
    pub aim: u16,
    pub src: u16,
    /// Ticks before a released starburst leaves its craft.
    pub delay: u8,
    /// A full charge behind it: a lamp takes only these.
    pub full: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Thing {
    /// A gold bonus gate: fly through it for points.
    Gate,
    /// A hearth ring: fly through it to mend the shield.
    Ring,
    /// A rock arch: thread its opening.
    Arch,
    /// A ridge a bomber hides behind (and a rock to keep clear of).
    Ridge,
    /// A pair of shutters closing on a gap, on a clock.
    Shutter,
}

/// A thing on the rail that stays where it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prop {
    pub thing: Thing,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    /// Half its size across and up (an arch's opening, a ridge's body, a
    /// gate's radius in `w`), and how deep it is along the rail.
    pub w: i32,
    pub h: i32,
    pub len: i32,
    /// The seats that have flown it (a bit each).
    pub taken: u8,
    /// A shutter's place in its cycle, in ticks.
    pub phase: u16,
}

/// A portrait call-out: who says what, for how long it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    /// A seat (1 to 6) or a wingman (7 to 10).
    pub who: u8,
    pub line: Line,
    pub ticks: u8,
}

/// The short lines the riders shout. The stage turns each into words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Line {
    Hello,
    Hold,
    Ring,
    Arch,
    Latched,
    Thanks,
    Lost,
    Turret,
    Roll,
    Convoy,
    Bombers,
    Shutter,
    Wyrm,
    Open,
    Reflect,
    Coils,
    Roar,
    Clear,
    Home,
    Chain,
    Hurt,
    Join,
    Low,
    Roost,
    Ruins,
}

impl Line {
    /// What a rider's line says, in two or three plain words, never a wall of
    /// text: `me` is the knight the line is shown to (the one who is calling),
    /// `human` whether the caller is a knight (a rider calls it otherwise).
    pub fn says(self, me: bool, human: bool) -> &'static str {
        match self {
            Line::Hello => "WING UP! TAP F TO FIRE",
            Line::Hold => "HOLD F, SWEEP, LET GO!",
            Line::Ring => "HEARTH RINGS MEND YOU",
            Line::Arch => "ARCH AHEAD! THREAD IT",
            Line::Latched => {
                if me {
                    "KITE ON ME! ROLL IT OFF"
                } else if human {
                    "COVER ME! BRAKE, SHOOT IT"
                } else {
                    "GET HIM OFF ME!"
                }
            }
            Line::Thanks => "THANKS! NICE SHOT!",
            Line::Lost => "I AM GOING DOWN!",
            Line::Turret => "TURRET! ROLL TO REFLECT",
            Line::Roll => "SPACE ROLLS: BOLTS TURN",
            Line::Convoy => "KITE CONVOY! SWEEP IT",
            Line::Bombers => "BOMBERS BEHIND THE RIDGE!",
            Line::Shutter => "SHUTTERS! SPUR OR BRAKE",
            Line::Wyrm => "THE GLOAM WYRM!",
            Line::Open => "LAMP OPEN! CHARGE IT!",
            Line::Reflect => "ROLL INTO ITS SPIT!",
            Line::Coils => "COILS! FIND THE GAP",
            Line::Roar => "THE ROAR! LEAVE THE LIGHT",
            Line::Clear => "BEAT CLEAR! HEARTHS AHEAD",
            Line::Home => "IT FALLS! TO THE BEACON!",
            Line::Chain => "WHAT A CHAIN!",
            Line::Hurt => "OUCH!",
            Line::Join => "A RIDER JOINS THE WING",
            Line::Low => "SHIELD LOW! FIND A RING",
            Line::Roost => "THE WYRM ROOST AHEAD",
            Line::Ruins => "THE RUINED CAUSEWAY",
        }
    }
}

/// A rider's name, for the wingmen of the Round Table (7 to 10): the empty
/// seats' riders; `None` for a knight's seat.
pub fn rider_name(id: u8) -> Option<&'static str> {
    match id {
        7 => Some("ASTER"),
        8 => Some("MIRA"),
        9 => Some("TOVIN"),
        10 => Some("NESSA"),
        _ => None,
    }
}

/// A flash, a pop or a ring for the stage to draw: nothing in the sim reads
/// it, and it is gone in a few ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flare {
    pub what: Burst,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    pub ticks: u8,
    /// A number the picture wants (a hit counter, a seat).
    pub n: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Burst {
    /// A foe popped into cold smoke.
    Pop,
    /// A bolt landed.
    Hit,
    /// A bolt turned round.
    Turn,
    /// A bracket snapped on.
    Lock,
    /// A gate or ring taken.
    Gate,
    /// Scraped rock.
    Scrape,
    /// A craft went down.
    Fall,
    /// A craft's shield was struck.
    Hurt,
    /// A lamp's shield opened or broke.
    Lamp,
    /// The screen-clearing bomb.
    Starfall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    pub foe: u16,
    pub n: u8,
}

/// One craft: a knight's griffin, or a wingman's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Craft {
    pub x: i32,
    pub z: i32,
    pub vx: i32,
    pub vz: i32,
    /// How far it leans, from a lagging blend of its sideways speed (Q is
    /// full), and how far it climbs or dives.
    pub bank: i32,
    pub lift: i32,
    /// Its place along the rail against the formation's: forward on a
    /// boost, back on a brake.
    pub back: i32,
    /// The distance it moved along the rail this tick.
    pub ds: i32,
    /// The reticle, across and up from the craft at the far plane.
    pub rx: i32,
    pub rz: i32,
    pub shield: i32,
    /// Ticks left unhurt (after a hit, a return).
    pub hurt: u32,
    /// Ticks left falling, before it rises again.
    pub down: u32,
    /// The roll: ticks left, how long it began as, its way, and the rest.
    pub roll: u32,
    pub roll_len: u32,
    pub dir: i8,
    pub rest: u32,
    /// The fire key: ticks held, charge gathered, rest after a shot.
    pub held: u32,
    pub charge: u32,
    pub cool: u32,
    pub locks: Vec<Lock>,
    /// How long the reticle has dwelt on a lock's body.
    pub dwell: u32,
    /// The spur: its meter, whether it is spent, and what it did this tick.
    pub meter: i32,
    pub spent: bool,
    pub boost: bool,
    pub brake: bool,
    pub bombs: u8,
    /// The keys held last tick (a bit each), for presses, and the double tap.
    pub keys: u8,
    pub tap_dir: i8,
    pub tap_ticks: u8,
    pub last_x: i8,
    /// The hit counter and how long before it lapses.
    pub chain: u16,
    pub chain_ticks: u32,
    /// What this craft has done: points, bolts landed, wingmen saved.
    pub pts: u32,
    pub hits: u32,
    pub saved: u16,
    pub gates: u16,
}

impl Craft {
    /// A craft at its slot, shield full and three bombs.
    pub fn new(x: i32, z: i32) -> Craft {
        Craft {
            x,
            z,
            vx: 0,
            vz: 0,
            bank: 0,
            lift: 0,
            back: 0,
            ds: PACE,
            rx: 0,
            rz: 0,
            shield: SHIELD,
            hurt: 0,
            down: 0,
            roll: 0,
            roll_len: 0,
            dir: 1,
            rest: 0,
            held: 0,
            charge: 0,
            cool: 0,
            locks: Vec::new(),
            dwell: 0,
            meter: METER,
            spent: false,
            boost: false,
            brake: false,
            bombs: 3,
            keys: 0,
            tap_dir: 0,
            tap_ticks: 0,
            last_x: 0,
            chain: 0,
            chain_ticks: 0,
            pts: 0,
            hits: 0,
            saved: 0,
            gates: 0,
        }
    }
    pub fn flying(&self) -> bool {
        self.down == 0
    }
    pub fn valid(&self) -> bool {
        let far = |v: i32, m: i32| (-m..=m).contains(&v);
        far(self.x, dm(160))
            && far(self.z, dm(120))
            && far(self.vx, 4 * Q)
            && far(self.vz, 4 * Q)
            && far(self.bank, 2 * Q)
            && far(self.lift, 2 * Q)
            && far(self.back, dm(80))
            && (0..=4 * Q).contains(&self.ds)
            && far(self.rx, dm(200))
            && far(self.rz, dm(200))
            && (0..=SHIELD).contains(&self.shield)
            && self.hurt <= 600
            && self.down <= 600
            && self.roll <= 120
            && self.roll_len <= 120
            && [-1, 1].contains(&self.dir)
            && self.rest <= 240
            && self.held <= 36_000
            && self.charge <= 240
            && self.cool <= 240
            && self.locks.len() <= MAX_LOCKS
            && self.locks.iter().map(|l| usize::from(l.n)).sum::<usize>() <= MAX_LOCKS
            && self.dwell <= 600
            && (0..=METER).contains(&self.meter)
            && self.bombs <= 9
            && (-1..=1).contains(&self.tap_dir)
            && (-1..=1).contains(&self.last_x)
            && self.chain_ticks <= 600
    }
}

/// The spur's meter, full.
pub const METER: i32 = 7200;

/// One knight at the table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    /// The tick of the run this knight sat down at (0: from the start).
    pub joined: u32,
    pub craft: Craft,
}

/// A rider of the Round Table who flies the wing when a seat is empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wingman {
    /// 7 to 10: after the seats.
    pub id: u8,
    pub craft: Craft,
    /// Out of it for the rest of the beat.
    pub lost: bool,
    pub fire: u32,
}

/// The Wyrm's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wyrm {
    /// Rising out of the roost.
    Rise,
    /// Between attacks.
    Idle,
    /// An attack's footprints are locked and shown.
    Tell,
    /// The coils sweep the window.
    Coil,
    /// The roar: a column of cold light.
    Roar,
    /// A lamp went out: it reels.
    Hurt,
    /// The last lamp is out.
    Dying,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Beast {
    /// The lamp it guards now (0 to 2): the phase.
    pub phase: u8,
    pub st: Wyrm,
    pub ticks: u32,
    /// Its head, across and up, and its lamps' foes.
    pub x: i32,
    pub z: i32,
    pub lamps: [u16; 3],
    /// The locked footprints of the attack to come (across, up).
    pub marks: Vec<(i32, i32)>,
    /// The coil's gap, across, and which side it sweeps from.
    pub gap: i32,
    pub side: i8,
    /// The roar's column, across.
    pub col: i32,
    /// The attack the tell is for (0 spit, 1 coil, 2 roar), and how many
    /// attacks it has made.
    pub atk: u8,
    pub seq: u8,
}

impl Beast {
    pub fn valid(&self) -> bool {
        self.phase <= 2
            && self.atk <= 2
            && self.ticks <= 3_000
            && (-dm(200)..=dm(200)).contains(&self.x)
            && (-dm(200)..=dm(200)).contains(&self.z)
            && self.marks.len() <= 8
            && self
                .marks
                .iter()
                .all(|&(x, z)| (-dm(300)..=dm(300)).contains(&x) && (-dm(300)..=dm(300)).contains(&z))
            && (-dm(200)..=dm(200)).contains(&self.gap)
            && (-1..=1).contains(&self.side)
            && (-dm(200)..=dm(200)).contains(&self.col)
    }
}

/// How the run stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Flying a beat.
    Play,
    /// Every knight is down: the beat starts over in a moment.
    Retry,
    /// The Wyrm is down: the warm fly-through.
    Victory,
    /// The end tally, and the way home.
    Tally,
}

/// What the party has banked, and what the beat in hand has made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub pts: u32,
    /// What the medal reads: the worth of what was taken (kills, gates,
    /// rescues), without the chain and reflect bonuses a score adds.
    pub base: u32,
    pub offered: u32,
    pub kills: u32,
    pub saved: u16,
    pub lost: u16,
    pub gates: u16,
}

impl Tally {
    pub fn valid(&self) -> bool {
        self.pts <= 1_000_000
            && self.base <= 1_000_000
            && self.offered <= 1_000_000
            && self.kills <= 10_000
            && self.saved <= 1_000
            && self.lost <= 1_000
            && self.gates <= 1_000
    }
    pub fn bank(&mut self, beat: &Tally) {
        self.pts += beat.pts;
        self.base += beat.base;
        self.offered += beat.offered;
        self.kills += beat.kills;
        self.saved += beat.saved;
        self.lost += beat.lost;
        self.gates += beat.gates;
    }
}

/// The medal at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Medal {
    Bronze,
    Silver,
    Gold,
}

impl Medal {
    pub fn name(self) -> &'static str {
        match self {
            Medal::Bronze => "BRONZE",
            Medal::Silver => "SILVER",
            Medal::Gold => "GOLD",
        }
    }
    /// How many gold pieces the purse adds for it.
    pub fn rank(self) -> u32 {
        match self {
            Medal::Bronze => 1,
            Medal::Silver => 2,
            Medal::Gold => 3,
        }
    }
}

// ─── the little arithmetic the sim shares ───────────────────────────────────

/// A whole-number length of the vector `d`, rounded down.
pub fn norm(d: (i32, i32, i32)) -> i64 {
    let n = i64::from(d.0).pow(2) + i64::from(d.1).pow(2) + i64::from(d.2).pow(2);
    // A correctly rounded square root, the same on every machine.
    (n as f64).sqrt() as i64
}

/// `d` turned to a length of `len`, along the same way (straight ahead for
/// a zero vector).
pub fn scale3(d: (i32, i32, i32), len: i32) -> (i32, i32, i32) {
    let m = norm(d);
    if m == 0 {
        return (len, 0, 0);
    }
    let f = |v: i32| (i64::from(v) * i64::from(len) / m) as i32;
    (f(d.0), f(d.1), f(d.2))
}

/// A triangle wave through `-amp..=amp` over `period` ticks.
pub fn tri(t: u32, period: u32, amp: i32) -> i32 {
    let p = period.max(2);
    let k = (t % p) as i64;
    let half = i64::from(p / 2).max(1);
    let up = if k < half { k } else { i64::from(p) - k };
    ((up * 2 - half) * i64::from(amp) / half) as i32
}

/// Whether the segment from `a` to `b` passes within `r` of `p`: the swept
/// test every bolt, dive and latch is judged by (no free 3D collision, only
/// this, in rail space).
pub fn near(a: (i32, i32, i32), b: (i32, i32, i32), p: (i32, i32, i32), r: i32) -> bool {
    let d = (
        i64::from(b.0) - i64::from(a.0),
        i64::from(b.1) - i64::from(a.1),
        i64::from(b.2) - i64::from(a.2),
    );
    let w = (
        i64::from(p.0) - i64::from(a.0),
        i64::from(p.1) - i64::from(a.1),
        i64::from(p.2) - i64::from(a.2),
    );
    let len2 = d.0 * d.0 + d.1 * d.1 + d.2 * d.2;
    let t = if len2 == 0 {
        0
    } else {
        (w.0 * d.0 + w.1 * d.1 + w.2 * d.2).clamp(0, len2)
    };
    let c = |dv: i64, wv: i64| {
        if len2 == 0 { wv } else { wv - dv * t / len2 }
    };
    let (ex, ey, ez) = (c(d.0, w.0), c(d.1, w.1), c(d.2, w.2));
    ex * ex + ey * ey + ez * ez <= i64::from(r) * i64::from(r)
}

/// How far two points are apart across and up (the plane's own distance).
pub fn flat(ax: i32, az: i32, bx: i32, bz: i32) -> i64 {
    norm((0, ax - bx, az - bz))
}
