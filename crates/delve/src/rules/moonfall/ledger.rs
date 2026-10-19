//! The Ledger: the party's notebook, written by the town itself.
//!
//! What any knight sees is written for all of them: a person's routine as
//! it is witnessed, a request as it is heard, a fact as it comes to pass. A
//! tick goes beside a person when their favour is done. None of it is lost
//! when the bell rewinds the town; that is the whole point of it. The
//! masks the favours earn are kept here too.
use super::clock;
use super::folk::{self, Folk, NONE};
use serde::{Deserialize, Serialize};

/// What the party has learned that opens a door.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Fact {
    /// Orin lost his ink pot in the reeds.
    OrinLost,
    /// Nessa's oven wants dry reeds.
    NessaCold,
    /// Pella has a scroll for Mira.
    PellaScroll,
    /// Tovin gives no key without a pass.
    TovinPass,
    /// Mira seals the south door for a knight with the favours done.
    MiraSeal,
    /// The herald cried a rider is coming on the second evening.
    AsterComing,
    /// The rider has come, and his horse has lost a shoe.
    AsterLame,
    /// Mira said the verse of the three lights.
    Verse,
}

pub const FACTS: [Fact; 8] = [
    Fact::OrinLost,
    Fact::NessaCold,
    Fact::PellaScroll,
    Fact::TovinPass,
    Fact::MiraSeal,
    Fact::AsterComing,
    Fact::AsterLame,
    Fact::Verse,
];

impl Fact {
    fn bit(self) -> u16 {
        1 << (self as u16)
    }
    /// The line the Ledger keeps for it, on the page of whoever told it.
    pub fn text(self) -> &'static str {
        match self {
            Self::OrinLost => "LOST HIS INK POT IN THE REED BANK. TAKES IT BACK AT SEVEN THE FIRST MORNING, AND WRITES A PASS IN FIVE HOURS, FRESH FOR TWO.",
            Self::NessaCold => "OVEN AND FORGE WANT DRY REEDS, TWELVE HOURS IN THE HAND, BETWEEN HALF PAST EIGHT AND TEN ON THE SECOND MORNING. PAYS IN THE STOUT MASK.",
            Self::PellaScroll => "A SCROLL FOR MIRA, FROM HER AT SEVEN THE FIRST MORNING. MIRA TAKES IT AT THE NINE O'CLOCK WELL WATCH, THE SECOND OR THIRD EVENING. PAYS IN THE SWIFT MASK.",
            Self::TovinPass => "NO KEY WITHOUT ORIN'S PASS. KEY DAY IS THE SECOND DAY, FIVE TO EIGHT IN THE EVENING. THE KEY KEEPS.",
            Self::MiraSeal => "SEALS THE SOUTH DOOR AT DAWN FOR A KNIGHT WHO WEARS BOTH MASKS. THE SEAL KEEPS.",
            Self::AsterComing => "A LANTERN RIDER COMES FROM THE EAST ON THE SECOND DAY, TOWARD DUSK.",
            Self::AsterLame => "HIS HORSE HAS LOST A SHOE. THE IRON CART IS AT THE FORGE FROM HALF PAST SIX TO EIGHT ON THE SECOND DAY.",
            Self::Verse => "THREE LIGHTS, ONE MIDNIGHT: NORTH, SOUTH AND THE WELL TOGETHER.",
        }
    }
    /// Whose page it is written on.
    pub fn page(self) -> Folk {
        match self {
            Self::OrinLost => Folk::Orin,
            Self::NessaCold => Folk::Nessa,
            Self::PellaScroll | Self::AsterComing => Folk::Pella,
            Self::TovinPass => Folk::Tovin,
            Self::MiraSeal | Self::Verse => Folk::Mira,
            Self::AsterLame => Folk::Aster,
        }
    }
}

/// The two masks, and what a knight keeps.
pub const SWIFT: u8 = 1;
pub const STOUT: u8 = 2;

/// What the towers keep for the party once earned: the north key, the
/// south seal and the oil in the well's sconce.
pub const NORTH_KEY: u8 = 1;
pub const SOUTH_SEAL: u8 = 2;
pub const OIL: u8 = 4;

/// Everything the party keeps across the rewind.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// A bit for each `Fact`.
    pub facts: u16,
    /// For each villager, a bit for each routine line witnessed.
    pub routines: [u32; 6],
    /// A bit for each villager whose favour is done.
    pub ticks: u8,
    /// The masks earned.
    pub masks: u8,
    /// The keys, the seal and the oil.
    pub keeps: u8,
    /// Times the bell has rewound the town.
    pub loops: u32,
}

impl Memory {
    pub fn knows(&self, f: Fact) -> bool {
        self.facts & f.bit() != 0
    }
    /// Learn a fact: true when it is new.
    pub fn learn(&mut self, f: Fact) -> bool {
        let new = !self.knows(f);
        self.facts |= f.bit();
        new
    }
    pub fn witnessed(&self, folk: Folk, routine: u8) -> bool {
        routine != NONE && routine < 32 && self.routines[folk.index()] & (1 << routine) != 0
    }
    pub fn witness(&mut self, folk: Folk, routine: u8) -> bool {
        if routine == NONE || routine >= 32 || self.witnessed(folk, routine) {
            return false;
        }
        self.routines[folk.index()] |= 1 << routine;
        true
    }
    pub fn ticked(&self, folk: Folk) -> bool {
        self.ticks & (1 << folk.index()) != 0
    }
    pub fn tick(&mut self, folk: Folk) -> bool {
        let new = !self.ticked(folk);
        self.ticks |= 1 << folk.index();
        new
    }
    /// How much the Ledger holds: a count that only grows, for the "new"
    /// mark on the Ledger key.
    pub fn written(&self) -> u32 {
        self.facts.count_ones()
            + self.routines.iter().map(|r| r.count_ones()).sum::<u32>()
            + self.ticks.count_ones()
            + self.masks.count_ones()
            + self.keeps.count_ones()
    }
    pub fn has_mask(&self, mask: u8) -> bool {
        self.masks & mask != 0
    }
    pub fn has_keep(&self, keep: u8) -> bool {
        self.keeps & keep != 0
    }
    pub fn valid(&self) -> bool {
        self.facts < (1 << FACTS.len())
            && self.ticks < 64
            && self.masks <= 3
            && self.keeps <= 7
            && self.loops <= 100_000
            && self.routines.iter().all(|&r| r >> 24 == 0)
    }
    pub fn repair(&mut self) {
        self.facts &= (1 << FACTS.len()) - 1;
        self.ticks &= 63;
        self.masks &= 3;
        self.keeps &= 7;
        self.loops = self.loops.min(100_000);
        for r in &mut self.routines {
            *r &= 0x00ff_ffff;
        }
    }
}

/// A line of a page: when, and what.
pub struct Line {
    pub time: String,
    pub text: String,
    /// Happening now (the stop the clock stands in).
    pub now: bool,
}

/// One person's page.
pub struct Page {
    pub folk: Folk,
    pub ticked: bool,
    /// What they ask, if the party has heard it.
    pub asks: Vec<&'static str>,
    pub routine: Vec<Line>,
    /// Facts written on this page.
    pub notes: Vec<&'static str>,
}

/// A person's page as it stands.
pub fn page(mem: &Memory, folk: Folk, clock: u32, day: u32) -> Page {
    let mut asks = Vec::new();
    let mut notes = Vec::new();
    for f in FACTS {
        if mem.knows(f) && f.page() == folk {
            match f {
                Fact::Verse | Fact::AsterComing => notes.push(f.text()),
                _ => asks.push(f.text()),
            }
        }
    }
    // The routine as witnessed, in the order of the day, each line once.
    let stamp_now = clock::half(clock, day) % 48;
    let mut routine: Vec<(u32, u8)> = (0u8..24)
        .filter(|&id| mem.witnessed(folk, id))
        .filter_map(|id| folk::routine_time(folk, id).map(|t| (t, id)))
        .collect();
    routine.sort();
    let lines = routine
        .iter()
        .enumerate()
        .map(|(n, &(t, id))| {
            let next = routine.get(n + 1).map_or(48, |r| r.0);
            Line {
                time: format!("{:02}:{:02}", (6 + t / 2) % 24, (t % 2) * 30),
                text: folk::routine_text(folk, id).to_string(),
                now: (t..next).contains(&stamp_now),
            }
        })
        .collect();
    Page { folk, ticked: mem.ticked(folk), asks, routine: lines, notes }
}

/// Appointments the Ledger reminds the party of: a routine line known,
/// the hour before it begins. (folk, routine id, on which day or any).
pub const APPOINTMENTS: [(Folk, u8, Option<u32>); 6] = [
    (Folk::Pella, 2, None),
    (Folk::Pella, 4, None),
    (Folk::Mira, 4, None),
    (Folk::Mira, 10, None),
    (Folk::Tovin, 4, None),
    (Folk::Aster, 0, Some(1)),
];

/// The appointments that fall in the hour starting at `clock`: those
/// known to the party whose stop is `ahead` ticks off. Returns
/// (folk, routine).
pub fn reminders(mem: &Memory, clock: u32, day: u32) -> Vec<(Folk, u8)> {
    let hour = day / 24;
    let table = folk::timetable(day);
    let mut out = Vec::new();
    for &(who, id, only) in &APPOINTMENTS {
        let known = mem.witnessed(who, id) || (who == Folk::Aster && mem.knows(Fact::AsterComing));
        if !known {
            continue;
        }
        for leg in table.legs[who.index()].iter().filter(|l| l.routine == id) {
            if only.is_some_and(|d| leg.at / day != d) {
                continue;
            }
            if leg.at.saturating_sub(hour) == clock {
                out.push((who, id));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_learned_is_written_once_and_ticks_count_the_others() {
        let mut m = Memory::default();
        assert!(m.learn(Fact::OrinLost) && !m.learn(Fact::OrinLost));
        assert!(m.witness(Folk::Mira, 1) && !m.witness(Folk::Mira, 1));
        assert!(!m.witness(Folk::Mira, NONE));
        assert!(m.tick(Folk::Pella) && m.tick(Folk::Nessa) && !m.tick(Folk::Nessa));
        assert!(m.valid());
        m.facts = u16::MAX;
        assert!(!m.valid());
        m.repair();
        assert!(m.valid());
    }

    #[test]
    fn a_page_lists_the_witnessed_routine_in_the_order_of_the_day() {
        let mut m = Memory::default();
        m.witness(Folk::Mira, 4);
        m.witness(Folk::Mira, 1);
        m.learn(Fact::MiraSeal);
        let p = page(&m, Folk::Mira, 3000 / 24 * 3 / 4, 3000);
        assert_eq!(p.routine.len(), 2);
        assert!(p.routine[0].time < p.routine[1].time);
        assert!(p.routine[0].now, "06:45 is the stop the porch stands in");
        assert_eq!(p.asks.len(), 1);
    }

    #[test]
    fn a_reminder_pops_an_hour_before_a_known_appointment_and_not_for_the_unknown() {
        let d = 3000;
        let mut m = Memory::default();
        assert!(reminders(&m, 0, d).is_empty());
        m.witness(Folk::Mira, 4);
        let leg = folk::timetable(d).legs[Folk::Mira.index()]
            .iter()
            .find(|l| l.routine == 4)
            .map(|l| l.at)
            .unwrap();
        assert_eq!(reminders(&m, leg - d / 24, d), vec![(Folk::Mira, 4)]);
        assert!(reminders(&m, leg - d / 24 + 1, d).is_empty());
    }
}
