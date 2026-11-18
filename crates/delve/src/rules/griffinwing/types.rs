//! Griffinwing's bodies: the craft, the foes, the bolts, the things on the
//! rail, the Wyrm, and the little lists the stage draws from. Every number is
//! a whole number on the 30 Hz sim, 256 to the unit (the Forest Road's `Q`).
//! Positions are in rail space: `s` along the rail, `x` across it (right is
//! positive) and `z` above it.
use super::*;

pub const Q: i32 = 256;
pub const fn dm(n: i32) -> i32 {
    n * Q / 10
}
pub const PACE: i32 = 85;
pub const LEAD: i32 = dm(640);
pub const FAR: i32 = dm(340);
pub const NEAR: i32 = dm(150);
pub const LOCK_MAX: i32 = dm(520);
pub const LOCK_MIN: i32 = dm(15);
pub const MAX_LOCKS: usize = 6;
pub const MAX_PER_BODY: u8 = 3;
pub const MAX_FOES: usize = 40;
pub const MAX_PROPS: usize = 40;
pub const MAX_BOLTS: usize = 160;
pub const MAX_CALLS: usize = 4;
pub const MAX_FX: usize = 48;
pub const SHIELD: i32 = 100;
pub const WING: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Wisp,
    Kite,
    Turret,
    Bomber,
    Lamp,
}

impl Kind {
    pub fn radius(self) -> i32 {
        match self {
            Kind::Wisp => dm(9),
            Kind::Kite => dm(8),
            Kind::Turret => dm(16),
            Kind::Bomber => dm(12),
            Kind::Lamp => dm(16),
        }
    }
    pub fn health(self) -> u32 {
        match self {
            Kind::Wisp => 20,
            Kind::Kite => 20,
            Kind::Turret => 60,
            Kind::Bomber => 40,
            Kind::Lamp => 120,
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum St {
    Fly,
    Tell,
    Rest,
    Hunt,
    Latched,
    Flee,
    Hide,
    Dive,
    Shut,
    Open,
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
    pub seats: u8,
    pub st: St,
    pub ticks: u32,
    pub fx: i32,
    pub fz: i32,
    pub on: u8,
    pub flash: u8,
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
    pub fn lockable(&self) -> bool {
        self.alive() && !matches!(self.st, St::Hide | St::Shut | St::Flee)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arm {
    Lance,
    Star,
    Ally,
    Spark,
    Gloam,
    Turned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bolt {
    pub shot: Arm,
    pub owner: u8,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    pub vs: i32,
    pub vx: i32,
    pub vz: i32,
    pub dmg: u16,
    pub life: u16,
    pub aim: u16,
    pub src: u16,
    pub delay: u8,
    pub full: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Thing {
    Gate,
    Ring,
    Arch,
    Ridge,
    Shutter,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prop {
    pub thing: Thing,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    pub w: i32,
    pub h: i32,
    pub len: i32,
    pub taken: u8,
    pub phase: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub who: u8,
    pub line: Line,
    pub ticks: u8,
}

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

pub fn rider_name(id: u8) -> Option<&'static str> {
    match id {
        7 => Some("ASTER"),
        8 => Some("MIRA"),
        9 => Some("TOVIN"),
        10 => Some("NESSA"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flare {
    pub what: Burst,
    pub s: i32,
    pub x: i32,
    pub z: i32,
    pub ticks: u8,
    pub n: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Burst {
    Pop,
    Hit,
    Turn,
    Lock,
    Gate,
    Scrape,
    Fall,
    Hurt,
    Lamp,
    Starfall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    pub foe: u16,
    pub n: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Craft {
    pub x: i32,
    pub z: i32,
    pub vx: i32,
    pub vz: i32,
    pub bank: i32,
    pub lift: i32,
    pub back: i32,
    pub ds: i32,
    pub rx: i32,
    pub rz: i32,
    pub shield: i32,
    pub hurt: u32,
    pub down: u32,
    pub roll: u32,
    pub roll_len: u32,
    pub dir: i8,
    pub rest: u32,
    pub held: u32,
    pub charge: u32,
    pub cool: u32,
    pub locks: Vec<Lock>,
    pub dwell: u32,
    pub meter: i32,
    pub spent: bool,
    pub boost: bool,
    pub brake: bool,
    pub bombs: u8,
    pub keys: u8,
    pub tap_dir: i8,
    pub tap_ticks: u8,
    pub last_x: i8,
    pub chain: u16,
    pub chain_ticks: u32,
    pub pts: u32,
    pub hits: u32,
    pub saved: u16,
    pub gates: u16,
}

impl Craft {
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
    pub fn reflecting(&self, reflect: u32) -> bool {
        self.roll > 0 && self.roll_len - self.roll < reflect
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

pub const METER: i32 = 7200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub joined: u32,
    pub craft: Craft,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wingman {
    pub id: u8,
    pub craft: Craft,
    pub lost: bool,
    pub fire: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Wyrm {
    Rise,
    Idle,
    Tell,
    Coil,
    Roar,
    Hurt,
    Dying,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Beast {
    pub phase: u8,
    pub st: Wyrm,
    pub ticks: u32,
    pub x: i32,
    pub z: i32,
    pub lamps: [u16; 3],
    pub marks: Vec<(i32, i32)>,
    pub gap: i32,
    pub side: i8,
    pub col: i32,
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
            && self.marks.iter().all(|&(x, z)| {
                (-dm(300)..=dm(300)).contains(&x) && (-dm(300)..=dm(300)).contains(&z)
            })
            && (-dm(200)..=dm(200)).contains(&self.gap)
            && (-1..=1).contains(&self.side)
            && (-dm(200)..=dm(200)).contains(&self.col)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Play,
    Retry,
    Victory,
    Tally,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub pts: u32,
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
    pub fn rank(self) -> u32 {
        match self {
            Medal::Bronze => 1,
            Medal::Silver => 2,
            Medal::Gold => 3,
        }
    }
}

pub fn tri(t: u32, period: u32, amp: i32) -> i32 {
    let p = period.max(2);
    let k = (t % p) as i64;
    let half = i64::from(p / 2).max(1);
    let up = if k < half { k } else { i64::from(p) - k };
    ((up * 2 - half) * i64::from(amp) / half) as i32
}
