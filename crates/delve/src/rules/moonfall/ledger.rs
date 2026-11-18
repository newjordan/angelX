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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Fact {
    OrinLost,
    NessaCold,
    PellaScroll,
    TovinPass,
    MiraSeal,
    AsterComing,
    AsterLame,
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

pub const SWIFT: u8 = 1;
pub const STOUT: u8 = 2;

pub const NORTH_KEY: u8 = 1;
pub const SOUTH_SEAL: u8 = 2;
pub const OIL: u8 = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    pub facts: u16,
    pub routines: [u32; 6],
    pub ticks: u8,
    pub masks: u8,
    pub keeps: u8,
    pub loops: u32,
}

impl Memory {
    pub fn knows(&self, f: Fact) -> bool {
        self.facts & f.bit() != 0
    }
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

pub struct Line {
    pub time: String,
    pub text: String,
    pub now: bool,
}

pub struct Page {
    pub folk: Folk,
    pub ticked: bool,
    pub asks: Vec<&'static str>,
    pub routine: Vec<Line>,
    pub notes: Vec<&'static str>,
}

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
}
