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
use super::folk::{self, CAST, Folk, Figure};
use super::ledger::{self, Fact, NORTH_KEY, SOUTH_SEAL, STOUT, SWIFT};
use super::says::{self, Ctx, Sign, Topic};
use super::state::*;
use super::town;
use super::Session;

/// What E may mean where a knight stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Folk(Folk),
    Ink,
    Reeds,
    North,
    South,
    Well,
    /// The Moonwell Bell on its post by the well.
    Bell,
    Lantern(u8),
}

/// Hours of Orin's writing.
pub const PASS_HOURS: u32 = 5;
/// Hours reeds are kept in the hand before they are dry.
pub const DRY_HOURS: u32 = 12;
/// Hours a finished pass keeps its ink: after them it smudges.
pub const PASS_FRESH: u32 = 2;
/// How long E is held to light a flame, in ticks.
pub const LIGHT_HOLD: u32 = 24;
/// The windows, in half-hours into a day (06:00 is 0): Pella sorts the post
/// 07:00 to 08:30 on the first morning; Orin takes the pot back in the same
/// hour; Nessa's oven wants its reeds 08:30 to 10:30 on the second day; Mira's
/// pen is fresh 06:30 to 09:00; Tovin airs the keys 17:00 to 20:00 on the
/// second day; Nessa's iron cart is at the forge 06:30 to 08:00 on the second
/// day.
pub const POST: (u32, u32) = (2, 5);
pub const OVEN: (u32, u32) = (5, 9);
pub const PEN: (u32, u32) = (1, 6);
pub const KEYS: (u32, u32) = (22, 28);
pub const IRON: (u32, u32) = (1, 4);

fn dist2(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).pow(2) + (a.1 - b.1).pow(2)
}

impl Session {
    pub fn phase_is_play(&self) -> bool {
        self.phase == Phase::Play
    }

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

    /// Ticks a flame burns once lit: tighter with every extra seat.
    pub fn burn(&self) -> u32 {
        let hour = self.nums.day / 24;
        (hour * 9 / 2).saturating_sub(hour * (self.seats_peak() - 1) / 4).max(hour * 5 / 2)
    }

    /// The thing nearest a knight's feet that E could mean.
    pub fn nearest_use(&self, who: u32) -> Option<Use> {
        let k = &self.seats.get(&who)?.knight;
        let me = k.px();
        let mut best: Option<(i32, Use)> = None;
        let mut offer = |d2: i32, reach: i32, u: Use| {
            if d2 <= reach * reach && best.is_none_or(|(b, _)| d2 < b) {
                best = Some((d2, u));
            }
        };
        for f in CAST {
            let fig = self.figure(f);
            if fig.visible {
                offer(dist2(me, (fig.x, fig.y)), 26, Use::Folk(f));
            }
        }
        offer(dist2(me, town::INK_POT), 22, Use::Ink);
        offer(dist2(me, town::REED_STACK), 22, Use::Reeds);
        offer(dist2(me, town::NORTH_DOOR), 26, Use::North);
        offer(dist2(me, town::SOUTH_DOOR), 26, Use::South);
        offer(dist2(me, town::WELL), 38, Use::Well);
        offer(dist2(me, town::BELL_POST), 22, Use::Bell);
        for (n, &(tx, ty)) in town::LAMPS.iter().enumerate() {
            let at = (tx * town::TILE + 8, ty * town::TILE + 4);
            offer(dist2(me, at), 18, Use::Lantern(n as u8));
        }
        best.map(|(_, u)| u)
    }

    fn note(&mut self, who: u32, n: Note) {
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.note = Some((n, 90));
        }
    }

    /// Everyone's: the Ledger wrote, or an hour is near.
    pub fn announce(&mut self, n: Note) {
        match n {
            Note::Wrote(..) | Note::Learned(_) => self.pending.push("card_pickup"),
            Note::Ticked(_) => self.pending.push("wish_grant"),
            Note::Remind(..) => self.pending.push("play_card"),
            _ => {}
        }
        self.news.push((n, 120));
        if self.news.len() > 3 {
            self.news.remove(0);
        }
    }

    fn take(&mut self, who: u32, item: Item) {
        let clock = self.clock;
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.held = item;
            s.knight.held_at = clock;
        }
        self.note(who, Note::Got(item));
    }

    fn held(&self, who: u32) -> Item {
        self.seats.get(&who).map_or(Item::Empty, |s| s.knight.held)
    }

    /// The part of the day by the clock, for small talk.
    fn band(&self) -> u8 {
        says::band(clock::stamp(self.clock, self.nums.day).hour)
    }

    /// Half-hours since the first dawn.
    fn half(&self) -> u32 {
        clock::half(self.clock, self.nums.day)
    }

    /// Whether it is the `day` (0 to 2) and within `w` half-hours of its dawn.
    pub fn within(&self, day: Option<u32>, w: (u32, u32)) -> bool {
        let h = self.half();
        day.is_none_or(|d| h / 48 == d) && (w.0..w.1).contains(&(h % 48))
    }

    /// Whether the reeds in a knight's hand are dry.
    pub fn dry(&self, k: &Knight) -> bool {
        k.held == Item::Reeds && self.clock >= k.held_at + DRY_HOURS * self.nums.day / 24
    }

    /// E pressed: do the one thing it means here.
    pub fn press_use(&mut self, who: u32) {
        match self.nearest_use(who) {
            Some(Use::Folk(f)) => self.talk(who, f),
            Some(Use::Ink) => {
                if self.held(who) != Item::Empty {
                    self.note(who, Note::Sign(Sign::HandsFull));
                } else if !self.memory.knows(Fact::OrinLost) || self.world.has(INK_TAKEN) {
                    self.note(who, Note::Sign(Sign::NoInk));
                } else {
                    self.world.set(INK_TAKEN);
                    self.take(who, Item::Ink);
                }
            }
            Some(Use::Reeds) => {
                if self.held(who) != Item::Empty {
                    self.note(who, Note::Sign(Sign::HandsFull));
                } else if !self.memory.knows(Fact::NessaCold) {
                    self.note(who, Note::Sign(Sign::NoInk));
                } else {
                    self.take(who, Item::Reeds);
                }
            }
            Some(Use::North) => self.door(who, true),
            Some(Use::South) => self.door(who, false),
            Some(Use::Well) => {
                let sign = if !self.memory.has_keep(ledger::OIL) {
                    Sign::WellIdle
                } else if self.final_hours() && !self.memory.knows(Fact::Verse) {
                    Sign::TowerNeedsVerse
                } else {
                    Sign::WellOiled
                };
                self.note(who, Note::Sign(sign));
            }
            Some(Use::Bell) => {
                // E at the post is the bell's own menu, as R is.
                if self.completed.is_none() && self.call.is_none() {
                    if let Some(s) = self.seats.get_mut(&who) {
                        s.knight.panel = Panel::Bell(0);
                        s.knight.say = None;
                    }
                }
            }
            Some(Use::Lantern(n)) => self.relight(who, n),
            None => {}
        }
    }

    fn relight(&mut self, who: u32, n: u8) {
        if self.lantern_scheduled(n) && self.world.snuffed & (1 << n) != 0 {
            self.world.snuffed &= !(1 << n);
            self.pending.push("spoil_pickup");
            self.note(who, Note::Sign(Sign::Relit));
        }
    }

    /// The tower doors open to the key and the seal the party keeps.
    fn door(&mut self, who: u32, north: bool) {
        let (flag, keep) = if north { (NORTH_OPEN, NORTH_KEY) } else { (SOUTH_OPEN, SOUTH_SEAL) };
        if self.world.has(flag) {
            if self.final_hours() {
                if !self.memory.knows(Fact::Verse) {
                    self.note(who, Note::Sign(Sign::TowerNeedsVerse));
                }
                return;
            }
            self.note(who, Note::Sign(Sign::TowerIdle));
        } else if self.memory.has_keep(keep) {
            self.world.set(flag);
            self.note(who, Note::Sign(if north { Sign::NorthOpened } else { Sign::SouthOpened }));
        } else {
            self.note(who, Note::Sign(if north { Sign::NorthLocked } else { Sign::SouthLocked }));
        }
    }

    /// E held: at a tower that is ready, light it when the hold is full.
    pub fn hold_use(&mut self, who: u32) -> Option<u8> {
        let target = self.nearest_use(who);
        let i = match target {
            Some(Use::North) => 0,
            Some(Use::South) => 1,
            Some(Use::Well) => 2,
            _ => {
                if let Some(s) = self.seats.get_mut(&who) {
                    s.knight.hold = 0;
                }
                return None;
            }
        };
        if !self.tower_ready(i) {
            if let Some(s) = self.seats.get_mut(&who) {
                s.knight.hold = 0;
            }
            return None;
        }
        let full = {
            let k = &mut self.seats.get_mut(&who)?.knight;
            k.hold += 1;
            k.hold >= LIGHT_HOLD
        };
        if full {
            if let Some(s) = self.seats.get_mut(&who) {
                s.knight.hold = 0;
            }
            if !self.flame(i) {
                let burn = self.burn();
                self.world.lit_until[i] = self.clock + burn;
                self.announce(Note::Lit(i as u8));
                return Some(i as u8);
            }
        }
        None
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

    /// Start a conversation: choose what is said, and do what it means.
    fn talk(&mut self, who: u32, to: Folk) {
        let held = self.held(who);
        let band = self.band();
        let day_ticks = self.nums.day;
        let hour = clock::stamp(self.clock, day_ticks).hour;
        let me = self.seats[&who].knight.clone();
        let topic = match to {
            Folk::Mira => {
                if held == Item::Scroll {
                    // She reads the post aloud at the well on the second and
                    // third evenings.
                    if self.day_index() >= 1 && self.figure(Folk::Mira).routine == 10 {
                        self.give_away(who);
                        self.keep_mask(who, SWIFT, Mask::Swift, Folk::Pella);
                        Topic::Thanks
                    } else {
                        Topic::Nag(2)
                    }
                } else if !self.memory.knows(Fact::MiraSeal) {
                    self.learn(Fact::MiraSeal);
                    Topic::Request
                } else if self.cry_active(Folk::Mira) {
                    Topic::Cry(0)
                } else if self.memory.has_keep(SOUTH_SEAL) {
                    Topic::Done
                } else if self.within(None, PEN) {
                    if self.memory.has_mask(SWIFT) && self.memory.has_mask(STOUT) {
                        self.memory.keeps |= SOUTH_SEAL;
                        self.note(who, Note::Sign(Sign::SealKept));
                        if self.memory.tick(Folk::Mira) {
                            self.announce(Note::Ticked(Folk::Mira));
                        }
                        Topic::Give
                    } else {
                        Topic::Nag(0)
                    }
                } else if self.memory.has_mask(SWIFT) && self.memory.has_mask(STOUT) {
                    Topic::Nag(1)
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Pella => {
                if !self.memory.knows(Fact::PellaScroll) {
                    self.learn(Fact::PellaScroll);
                    Topic::Request
                } else if self.cry_active(Folk::Pella) {
                    Topic::Cry(self.cry_number())
                } else if self.memory.has_mask(SWIFT) {
                    Topic::Done
                } else if held == Item::Scroll {
                    Topic::Nag(0)
                } else if !self.within(Some(0), POST) {
                    Topic::Nag(1)
                } else if held == Item::Empty {
                    self.take(who, Item::Scroll);
                    Topic::Give
                } else {
                    self.note(who, Note::Sign(Sign::HandsFull));
                    Topic::Hello(band)
                }
            }
            Folk::Nessa => {
                if !self.memory.knows(Fact::NessaCold) {
                    self.learn(Fact::NessaCold);
                    Topic::Request
                } else if held == Item::Reeds {
                    if !self.dry(&me) {
                        Topic::Nag(2)
                    } else if !self.within(Some(1), OVEN) {
                        Topic::Nag(3)
                    } else {
                        self.give_away(who);
                        self.keep_mask(who, STOUT, Mask::Stout, Folk::Nessa);
                        Topic::Thanks
                    }
                } else if self.shoe_day() {
                    if !self.memory.knows(Fact::AsterLame) {
                        Topic::Nag(0)
                    } else if self.world.shod.is_some() {
                        Topic::Done
                    } else if held == Item::Empty {
                        self.take(who, Item::Shoe);
                        Topic::Give
                    } else {
                        self.note(who, Note::Sign(Sign::HandsFull));
                        Topic::Hello(band)
                    }
                } else if self.memory.has_mask(STOUT) {
                    if self.memory.knows(Fact::AsterLame) && !self.memory.has_keep(ledger::OIL) {
                        Topic::Nag(1)
                    } else {
                        Topic::Done
                    }
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Orin => {
                if !self.memory.knows(Fact::OrinLost) {
                    self.learn(Fact::OrinLost);
                    Topic::Request
                } else if held == Item::Ink {
                    if self.within(Some(0), POST) {
                        self.give_away(who);
                        self.world.ink_given = Some(self.clock);
                        Topic::Thanks
                    } else {
                        Topic::Nag(1)
                    }
                } else if let Some(t) = self.world.ink_given {
                    let ready = t + PASS_HOURS * day_ticks / 24;
                    if self.clock < ready {
                        Topic::Nag(0)
                    } else if self.memory.ticked(Folk::Orin) && self.memory.has_keep(NORTH_KEY) {
                        Topic::Done
                    } else if self.clock > ready + PASS_FRESH * day_ticks / 24 {
                        Topic::Nag(2)
                    } else if held == Item::Empty {
                        self.take(who, Item::Pass);
                        if self.memory.tick(Folk::Orin) {
                            self.announce(Note::Ticked(Folk::Orin));
                        }
                        Topic::Give
                    } else {
                        self.note(who, Note::Sign(Sign::HandsFull));
                        Topic::Hello(band)
                    }
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Tovin => {
                if !self.memory.knows(Fact::TovinPass) {
                    self.learn(Fact::TovinPass);
                    Topic::Request
                } else if held == Item::Pass {
                    if self.within(Some(1), KEYS) {
                        self.give_away(who);
                        self.memory.keeps |= NORTH_KEY;
                        self.note(who, Note::Sign(Sign::KeyKept));
                        if self.memory.tick(Folk::Tovin) {
                            self.announce(Note::Ticked(Folk::Tovin));
                        }
                        Topic::Thanks
                    } else {
                        Topic::Nag(1)
                    }
                } else if self.memory.has_keep(NORTH_KEY) {
                    Topic::Done
                } else if hour >= 20 || hour < 6 {
                    Topic::Hello(3)
                } else {
                    Topic::Hello(band)
                }
            }
            Folk::Aster => {
                if held == Item::Shoe && self.world.shod.is_none() {
                    self.give_away(who);
                    self.world.shod = Some(self.clock);
                    Topic::Thanks
                } else if self.memory.has_keep(ledger::OIL) {
                    Topic::Done
                } else if self.world.shod.is_some() {
                    Topic::Give
                } else {
                    if self.memory.learn(Fact::AsterLame) {
                        self.announce(Note::Learned(Fact::AsterLame));
                    }
                    Topic::Request
                }
            }
        };
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.say = Some(Say { folk: to, topic, page: 0, age: 0 });
        }
    }

    fn give_away(&mut self, who: u32) {
        if let Some(s) = self.seats.get_mut(&who) {
            s.knight.held = Item::Empty;
        }
    }

    /// A mask is earned and kept, and the person's tick goes in the Ledger.
    fn keep_mask(&mut self, who: u32, bit: u8, mask: Mask, ticked: Folk) {
        self.memory.masks |= bit;
        if self.memory.tick(ticked) {
            self.announce(Note::Ticked(ticked));
        }
        self.note(who, Note::Mask(mask));
    }

    fn learn(&mut self, f: Fact) {
        if self.memory.learn(f) {
            self.announce(Note::Learned(f));
        }
    }

    /// Whether `folk` is in the middle of crying a proclamation.
    pub fn cry_active(&self, who: Folk) -> bool {
        self.cry.is_some_and(|(f, _, _)| f == who)
    }

    fn cry_number(&self) -> u8 {
        match self.cry {
            Some((_, Topic::Cry(n), _)) => n,
            _ => 0,
        }
    }

    /// The second day's iron cart: Nessa's rack of shoes is out.
    pub fn shoe_day(&self) -> bool {
        self.within(Some(1), IRON) && self.figure(Folk::Nessa).arrived
    }

    /// Whether a conversation page `n` exists for what a knight is saying.
    pub fn say_pages(&self, say: &Say) -> usize {
        self.words(say.folk, say.topic).len().div_ceil(2).max(1)
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
