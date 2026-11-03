//! The favours: what E does at each person and place, and what the town
//! says back. The chain of them is the adventure.
//!
//! Five favours earn what the towers keep, and each hand carries one thing
//! at a time, so which favour a knight does in a loop is a choice, and a
//! party splits them:
//!
//!   Pella's scroll, from her at seven the first morning, to Mira at her
//!   nine o'clock well watch, second or third evening  the Swift mask, kept
//!   dry reeds from the bank (twelve hours in the hand) to Nessa's oven
//!   from half past eight to half past ten on the second morning  the Stout mask, kept
//!   Orin's ink pot at seven the first morning, five hours' writing, then
//!   the pass to Tovin on the second evening  the north key, kept
//!   a horseshoe from the forge on the second day, to Aster at the gate
//!   .......................................... the oil in the well, kept
//!   Mira's seal at dawn for a knight with both masks the south seal, kept
//!   the verse (the third evening), then the Third Midnight: three lights.
use super::clock;
use super::folk::{self, Folk, Figure};
use super::ledger::{self, Fact, STOUT, SWIFT};
use super::says::{self, Ctx, Topic};
use super::state::*;
use super::town::{self, TILE};
use super::Session;

/// Hours of Orin's writing.
pub const PASS_HOURS: u32 = 5;
/// How long E is held to light a flame, in ticks.
pub const LIGHT_HOLD: u32 = 24;

impl Session {
    pub fn figure(&self, who: Folk) -> Figure {
        folk::figure(who, self.clock, self.nums.day, self.world.shod)
    }

    pub fn day_index(&self) -> u32 {
        (self.clock / self.nums.day.max(1)).min(clock::DAYS - 1)
    }

    /// The lantern lights by the lamplighter's round, and stays lit unless
    /// the Gloam or the moon's shadow put it out.
    pub fn lantern_scheduled(&self, n: u8) -> bool {
        folk::lamp_lit_at(n, self.nums.day).is_some_and(|at| self.clock % self.nums.day >= at)
    }

    pub fn lantern_lit(&self, n: u8) -> bool {
        self.lantern_scheduled(n) && self.world.snuffed & (1 << n) == 0 && self.completed.is_none()
    }

    /// A flame of the three towers: 0 north, 1 south, 2 the well.
    pub fn flame(&self, i: usize) -> bool {
        self.world.lit_until[i] > self.clock
    }

    pub fn final_hours(&self) -> bool {
        self.clock >= clock::final_start(self.nums.day)
    }

    /// Whether tower `i` has what it needs to take a flame.
    pub fn tower_ready(&self, i: usize) -> bool {
        self.memory.knows(Fact::Verse)
            && self.final_hours()
            && match i {
                0 => self.world.has(NORTH_OPEN),
                1 => self.world.has(SOUTH_OPEN),
                _ => self.memory.has_keep(ledger::OIL),
            }
    }

    /// The seats' count the table is scaled to.
    pub fn seats_peak(&self) -> u32 {
        self.peak.max(1)
    }

    fn ctx(&self) -> Ctx {
        let wait = self.world.ink_given.map_or(0, |t| {
            let ready = t + PASS_HOURS * self.nums.day / 24;
            (ready.saturating_sub(self.clock) * 24).div_ceil(self.nums.day.max(1))
        });
        Ctx {
            loops: self.memory.loops,
            ticks: self.memory.masks.count_ones(),
            wait_hours: wait,
        }
    }

    /// The lines a knight would read for `topic` from `who` just now.
    pub fn words(&self, folk: Folk, topic: Topic) -> Vec<String> {
        says::lines(folk, topic, self.ctx())
    }

    /// Whether `folk` is in the middle of crying a proclamation.
    pub fn cry_active(&self, who: Folk) -> bool {
        self.cry.is_some_and(|(f, _, _)| f == who)
    }

    /// Whether a conversation page `n` exists for what a knight is saying.
    pub fn say_pages(&self, say: &Say) -> usize {
        self.words(say.folk, say.topic).len().div_ceil(2).max(1)
    }

    /// The lit lights, by id and place, in pixels.
    pub fn lights(&self) -> Vec<(u8, (i32, i32))> {
        let mut out = Vec::new();
        for n in 0..6u8 {
            if self.lantern_lit(n) {
                let (tx, ty) = town::LAMPS[usize::from(n)];
                out.push((n, (tx * TILE + 8, ty * TILE)));
            }
        }
        for (i, at) in [town::NORTH_DOOR, town::SOUTH_DOOR, town::WELL].into_iter().enumerate() {
            if self.flame(i) {
                out.push((6 + i as u8, at));
            }
        }
        out
    }
}

/// The words for a note: what the Ledger or the town says to the table.
pub fn note_text(n: Note) -> String {
    match n {
        Note::Wrote(f, id) => {
            let time = folk::routine_time(f, id).map_or(String::new(), |t| {
                format!("{:02}:{:02} ", (6 + t / 2) % 24, (t % 2) * 30)
            });
            format!("LEDGER: {} {}{}", f.short(), time, folk::routine_text(f, id))
        }
        Note::Learned(fact) => format!("LEDGER: {} - {}", fact.page().short(), fact.text()),
        Note::Ticked(f) => format!("LEDGER: {} TICKED", f.short()),
        Note::Remind(f, id) => {
            let text = if f == Folk::Aster {
                folk::aster_text(id)
            } else {
                folk::routine_text(f, id)
            };
            format!("REMINDER IN ONE HOUR: {} {}", f.short(), text)
        }
        Note::Sign(sign) => says::sign(sign).to_string(),
        Note::Got(item) => format!("GOT {}", item.name()),
        Note::Mask(Mask::Swift) => "SWIFT MASK EARNED. Z TO WEAR.".into(),
        Note::Mask(Mask::Stout) => "STOUT MASK EARNED. Z TO WEAR.".into(),
        Note::Mask(Mask::Bare) => String::new(),
        Note::Candle(n) => format!("A CANDLE GOES OUT. {n} LEFT."),
        Note::Snuffed => "SNUFFED OUT. THE WELL CARRIES YOU BACK.".into(),
        Note::Lit(0) => "THE NORTH TOWER BURNS!".into(),
        Note::Lit(1) => "THE SOUTH TOWER BURNS!".into(),
        Note::Lit(_) => "THE MOONWELL BURNS!".into(),
        Note::Out => "A LIGHT GOES OUT. RELIGHT IT (E).".into(),
        Note::Ring(id) => format!("KNIGHT {id} PULLS THE BELL ROPE"),
    }
}

/// The Ledger page `page` (0 the town's, 1 to 6 the folk's) as plain lines,
/// for the terminal, which cannot read the picture's small print.
pub fn page_lines(s: &Session, page: u8) -> Vec<String> {
    let st = clock::stamp(s.shown_clock(), s.nums.day);
    let wrap = |text: &str, n: usize| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for word in text.split_whitespace() {
            match out.last_mut() {
                Some(line) if line.len() + 1 + word.len() <= n => {
                    line.push(' ');
                    line.push_str(word);
                }
                _ => out.push(word.to_string()),
            }
        }
        out
    };
    let mut out = Vec::new();
    if page == 0 {
        out.push(format!("THE LEDGER  DAY {} {}  LOOP {}", st.day, st.text(), s.memory.loops + 1));
        let keeps = [
            ("NORTH TOWER", ledger::NORTH_KEY, "KEY KEPT", "NEEDS TOVIN'S KEY"),
            ("SOUTH TOWER", ledger::SOUTH_SEAL, "SEAL KEPT", "NEEDS MIRA'S SEAL"),
            ("THE MOONWELL", ledger::OIL, "OIL KEPT", "NEEDS ASTER'S OIL"),
        ];
        for (name, bit, yes, no) in keeps {
            out.push(format!("{name}: {}", if s.memory.has_keep(bit) { yes } else { no }));
        }
        out.push(format!("FAVOURS TICKED: {} OF 6", s.memory.ticks.count_ones()));
        let masks = format!(
            "MASKS: {}{}",
            if s.memory.has_mask(SWIFT) { "SWIFT " } else { "" },
            if s.memory.has_mask(STOUT) { "STOUT" } else { "" }
        );
        out.push(if masks.len() > 7 { masks } else { "MASKS: NONE YET".into() });
        if s.memory.knows(Fact::Verse) {
            out.extend(wrap(Fact::Verse.text(), 60));
        }
        return out;
    }
    let Some(who) = Folk::from_index(usize::from(page) - 1) else {
        return out;
    };
    let p = ledger::page(&s.memory, who, s.shown_clock(), s.nums.day);
    out.push(format!("{}  {}  {}", who.name(), who.title(), if p.ticked { "[DONE]" } else { "[ ]" }));
    for a in &p.asks {
        out.extend(wrap(a, 60).into_iter().map(|l| format!("> {l}")));
    }
    for a in &p.notes {
        out.extend(wrap(a, 60).into_iter().map(|l| format!("* {l}")));
    }
    for l in &p.routine {
        out.push(format!("{}{} {}", if l.now { "> " } else { "  " }, l.time, l.text));
    }
    if out.len() == 1 {
        out.push("NOTHING WRITTEN YET. GO AND SEE WHAT THEY DO, AND ASK.".into());
    }
    out
}
