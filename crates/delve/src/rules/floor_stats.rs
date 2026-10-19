//! A floor's results: what each knight did on it, the honours it earned,
//! and the wish that did the most.
//!
//! - **The tally** (`Tally`) counts every knight's floor as it is played,
//!   room after room: kills, damage dealt and taken, shots and the hits
//!   they landed, falls, friends raised from their skulls, gold picked up.
//!   It starts over on each new floor. It is the server's own: a client is
//!   sent only the results (`wire::PRIVATE`).
//! - **The results** (`Results`) are the tally closed: when the party takes
//!   the stairs down, and when the run is won or falls. A seat sees them as
//!   the first part of the next floor's plate (`floor_plates`), a press
//!   skips them, and a fight never waits on them.
//! - **Honours** go only to what the tally can show: the most kills, the
//!   most damage, the best aim over a fair number of shots, a floor fought
//!   without a scratch, a friend raised, the most gold. The MVP holds the
//!   biggest share of the floor's kills and damage, a friend raised worth
//!   half a share.
//! - **The most impactful wish** compares a knight's pace with their pace
//!   before it: damage dealt for each second stood in a fight, over the
//!   delve before the wish, against the seconds after it until their next
//!   wish. What the knight dealt over their old pace is the wish's doing.
//!   Wishes made together, with no fight between them, are measured as
//!   one. A wish with too little fight on either side is not measured;
//!   when no wish this floor is, the one held while its knight dealt the
//!   most is named for that alone, and says so.

use super::*;

/// Ticks of fight a wish needs on either side of it to be measured.
pub const SETTLE: u64 = 5 * HZ as u64;
/// Shots a knight must loose, and the hits in a hundred they must land,
/// for their aim to be honoured.
pub const AIMED: u32 = 20;
pub const TRUE_AIM: u32 = 50;
/// The most wishes a floor's tally keeps.
const WISHES: usize = 32;
/// Ticks a seat's plate gives the results before the floor's own title.
pub const SHOWN: u8 = 200;

/// One knight's floor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Line {
    pub kills: u32,
    pub damage: u32,
    pub taken: u32,
    pub shots: u32,
    pub hits: u32,
    pub falls: u32,
    /// Fallen friends this knight raised from their skulls.
    pub raised: u32,
    pub gold: u32,
    /// Ticks the knight stood in a fight.
    pub fought: u32,
}

impl Line {
    /// Hits for every hundred shots, when there were shots.
    pub fn aim(&self) -> Option<u32> {
        (self.shots > 0).then(|| (self.hits.min(self.shots) * 100 / self.shots).min(100))
    }
}

/// Wishes a knight made together, and how they fought before and since:
/// damage dealt and ticks stood in a fight.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wished {
    pub knight: u32,
    pub names: Vec<String>,
    /// Over the delve before the first of them.
    pub before: (u64, u64),
    /// From then until the knight's next wish, or the floor's end.
    pub after: (u64, u64),
}

impl Wished {
    /// Damage dealt over the knight's old pace, and the pace's rise in
    /// percent; nothing when either side had too little fight to tell.
    pub fn impact(&self) -> Option<(i64, i64)> {
        let ((bd, bt), (ad, at)) = (self.before, self.after);
        if bt < SETTLE || at < SETTLE || bd == 0 {
            return None;
        }
        let expected = (bd as u128 * at as u128 / bt as u128) as i64;
        let gain = (ad as u128 * bt as u128 * 100 / (bd as u128 * at as u128)) as i64 - 100;
        Some((ad as i64 - expected, gain))
    }

    /// The wishes' names as one: "Twin Bolts", "Twin Bolts and Fork".
    pub fn name(&self) -> String {
        match self.names.as_slice() {
            [] => String::new(),
            [one] => one.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        }
    }
}

/// A floor as it is played (`Run::floor_tally`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tally {
    /// The tick the floor began.
    pub began: u64,
    pub knights: BTreeMap<u32, Line>,
    pub wishes: Vec<Wished>,
    /// Each knight's damage and ticks in a fight over the whole delve:
    /// the pace a wish is measured against.
    pub pace: BTreeMap<u32, (u64, u64)>,
    /// The gold each knight carried at the last tick.
    pub purse: BTreeMap<u32, u32>,
}

impl Tally {
    pub fn line(&mut self, id: u32) -> &mut Line {
        self.knights.entry(id).or_default()
    }

    /// Knight `id` was granted `name`: one with the wish before it when no
    /// fight came between them.
    pub fn wished(&mut self, id: u32, name: &str) {
        let pace = self.pace.get(&id).copied().unwrap_or_default();
        let full = self.wishes.len() >= WISHES;
        match self.wishes.iter_mut().rev().find(|w| w.knight == id) {
            Some(last) if last.after.1 < SETTLE => {
                if last.names.len() < 4 {
                    last.names.push(name.to_string());
                }
            }
            _ if !full => self.wishes.push(Wished {
                knight: id,
                names: vec![name.to_string()],
                before: pace,
                after: (0, 0),
            }),
            _ => {}
        }
    }

    /// A tick of the floor: what each knight dealt, and who stood in a
    /// fight.
    pub fn count(&mut self, id: u32, dealt: u32, fighting: bool) {
        let fought = u64::from(fighting);
        let line = self.line(id);
        line.damage = line.damage.saturating_add(dealt);
        line.fought += u32::from(fighting);
        let pace = self.pace.entry(id).or_default();
        *pace = (pace.0 + u64::from(dealt), pace.1 + fought);
        if let Some(w) = self.wishes.iter_mut().rev().find(|w| w.knight == id) {
            w.after = (w.after.0 + u64::from(dealt), w.after.1 + fought);
        }
    }

    /// A new floor: its lines and wishes start over; the delve's pace and
    /// the purses carry on.
    pub fn next_floor(&mut self, tick: u64) {
        self.began = tick;
        self.knights.clear();
        self.wishes.clear();
    }

    /// A new delve: nothing carries over.
    pub fn new_delve(&mut self, tick: u64) {
        *self = Tally { began: tick, purse: std::mem::take(&mut self.purse), ..Tally::default() };
    }
}

/// How a floor ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    /// The party took the stairs down.
    #[default]
    Cleared,
    Won,
    Fallen,
}

/// An honour, and what it is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Honour {
    MostKills,
    HardestHitter,
    Sharpshooter,
    Untouched,
    Lifesaver,
    DeepPockets,
}

impl Honour {
    pub fn title(self) -> &'static str {
        match self {
            Self::MostKills => "MOST KILLS",
            Self::HardestHitter => "HARDEST HITTER",
            Self::Sharpshooter => "SHARPSHOOTER",
            Self::Untouched => "UNTOUCHED",
            Self::Lifesaver => "LIFESAVER",
            Self::DeepPockets => "DEEP POCKETS",
        }
    }
}

/// A knight's floor on the results board.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Row {
    pub id: u32,
    pub name: String,
    pub line: Line,
    pub honours: Vec<Honour>,
}

/// The wish that did the most this floor.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Impact {
    pub knight: u32,
    pub name: String,
    pub wish: String,
    /// Damage over the knight's old pace (or, unmeasured, all they dealt
    /// with it).
    pub damage: u32,
    /// The pace's rise in percent; none when it could not be measured.
    pub gain: Option<u32>,
}

/// A floor closed (`Run::floor_results`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Results {
    pub depth: u32,
    pub end: End,
    /// Ticks the floor took.
    pub ticks: u64,
    pub rows: Vec<Row>,
    pub mvp: Option<u32>,
    pub wish: Option<Impact>,
    /// The tick the floor was closed.
    pub at: u64,
    /// A finished run's deeds (`Run::deeds`), each an achievement's id, in
    /// the order earned.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deeds: Vec<String>,
    /// Steps a finished run's verdict has stood.
    pub stood: u32,
}

impl Results {
    /// The tally closed, for the party in `players`.
    pub fn of(tally: &Tally, players: &BTreeMap<u32, Hero>, depth: u32, end: End, tick: u64) -> Results {
        let mut rows: Vec<Row> = players
            .iter()
            .filter(|(_, h)| !h.absent)
            .map(|(&id, h)| Row {
                id,
                name: h.shown_name(),
                line: tally.knights.get(&id).copied().unwrap_or_default(),
                honours: Vec::new(),
            })
            .collect();
        let party = rows.len();
        // Each honour goes to whoever tops it, ties shared; the "most"
        // honours only in a party, where there is someone to beat.
        let mut honour = |honour: Honour, value: &dyn Fn(&Line) -> Option<u64>, alone: bool| {
            if party < 2 && !alone {
                return;
            }
            let best = rows.iter().filter_map(|r| value(&r.line)).max().filter(|&v| v > 0);
            if let Some(best) = best {
                for row in rows.iter_mut().filter(|r| value(&r.line) == Some(best)) {
                    row.honours.push(honour);
                }
            }
        };
        honour(Honour::MostKills, &|l| Some(u64::from(l.kills)), false);
        honour(Honour::HardestHitter, &|l| Some(u64::from(l.damage)), false);
        honour(Honour::Sharpshooter, &|l| l.aim().filter(|&a| l.shots >= AIMED && a >= TRUE_AIM).map(u64::from), true);
        honour(Honour::Untouched, &|l| (l.taken == 0 && u64::from(l.fought) >= SETTLE).then_some(1), true);
        honour(Honour::Lifesaver, &|l| Some(u64::from(l.raised)), true);
        honour(Honour::DeepPockets, &|l| Some(u64::from(l.gold)), false);
        let mvp = (party >= 2).then(|| mvp(&rows)).flatten();
        let wish = impact(tally, players);
        Results {
            depth,
            end,
            ticks: tick.saturating_sub(tally.began),
            rows,
            mvp,
            wish,
            at: tick,
            deeds: Vec::new(),
            stood: 0,
        }
    }

    pub fn row(&self, id: u32) -> Option<&Row> {
        self.rows.iter().find(|r| r.id == id)
    }
}

/// The knight with the biggest share of the floor's kills and damage, each
/// counted as a share of the party's, and half a share for the friends
/// they raised; none on a floor where nobody did anything.
fn mvp(rows: &[Row]) -> Option<u32> {
    let total = |f: fn(&Line) -> u32| rows.iter().map(|r| u64::from(f(&r.line))).sum::<u64>();
    let (kills, damage, raised) = (total(|l| l.kills), total(|l| l.damage), total(|l| l.raised));
    let share = |part: u32, whole: u64| if whole == 0 { 0 } else { u64::from(part) * 1000 / whole };
    rows.iter()
        .map(|r| (share(r.line.kills, kills) + share(r.line.damage, damage) + share(r.line.raised, raised) / 2, r.line.damage, std::cmp::Reverse(r.id), r.id))
        .filter(|s| s.0 > 0)
        .max()
        .map(|s| s.3)
}

/// The floor's most impactful wish among the party in `players`: the most
/// damage over its knight's old pace; or, when none could be measured, the
/// one held while its knight dealt the most.
pub fn impact(tally: &Tally, players: &BTreeMap<u32, Hero>) -> Option<Impact> {
    let name = |id: u32| players.get(&id).map_or_else(String::new, |h| h.shown_name());
    let here = || tally.wishes.iter().filter(|w| players.get(&w.knight).is_some_and(|h| !h.absent));
    let measured = here()
        .filter_map(|w| w.impact().map(|(extra, gain)| (extra, gain, w)))
        .filter(|&(extra, gain, _)| extra > 0 && gain > 0)
        .max_by_key(|&(extra, ..)| extra);
    if let Some((extra, gain, w)) = measured {
        return Some(Impact {
            knight: w.knight,
            name: name(w.knight),
            wish: w.name(),
            damage: extra.min(i64::from(u32::MAX)) as u32,
            gain: Some(gain.min(i64::from(u32::MAX)) as u32),
        });
    }
    let held = here().filter(|w| w.after.0 > 0).max_by_key(|w| w.after.0)?;
    Some(Impact {
        knight: held.knight,
        name: name(held.knight),
        wish: held.name(),
        damage: held.after.0.min(u64::from(u32::MAX)) as u32,
        gain: None,
    })
}

impl Run {
    /// The floor's tick: what each knight dealt (`feel::tick_feel`), who
    /// stood in a fight, the gold they picked up.
    pub(super) fn tally_floor(&mut self, dealt: &BTreeMap<u32, u32>) {
        if self.dungeon.depth == 0 || !self.active() {
            return;
        }
        let fighting = self.fight_on();
        for (&id, hero) in &self.players {
            if hero.absent {
                continue;
            }
            let standing = hero.hp > 0 && !hero.away;
            self.floor_tally.count(id, dealt.get(&id).copied().unwrap_or(0), fighting && standing);
            let gold = hero.carried.get(Spoil::Gold);
            let was = self.floor_tally.purse.insert(id, gold).unwrap_or(gold);
            if gold > was {
                let line = self.floor_tally.line(id);
                line.gold = line.gold.saturating_add(gold - was);
            }
        }
    }

    /// The floor is behind the party: its results, for the plate.
    pub(super) fn close_floor(&mut self, end: End) {
        self.floor_results = Some(Results::of(&self.floor_tally, &self.players, self.dungeon.depth, end, self.tick));
    }

    /// The run is won or has fallen: the floor it ended on is closed.
    pub(super) fn close_run(&mut self) {
        if !self.over() || self.dungeon.depth == 0 {
            return;
        }
        self.close_floor(if self.phase == Phase::Won { End::Won } else { End::Fallen });
        if let Some(results) = &mut self.floor_results {
            results.deeds = self.deeds.clone();
        }
    }

    /// A step of a finished run's verdict: its results count it.
    pub(super) fn stand_verdict(&mut self) {
        if self.over()
            && let Some(results) = self.floor_results.as_mut().filter(|r| r.end != End::Cleared)
        {
            results.stood = results.stood.saturating_add(1);
        }
    }

    /// How long a finished run's verdict stands with nobody asking to go
    /// (`again`): longer when it has deeds to show.
    pub fn verdict_hold(&self) -> u32 {
        let deeds = self.floor_results.as_ref().filter(|r| r.end != End::Cleared).map_or(0, |r| r.deeds.len());
        super::again::VERDICT_HOLD + if deeds > 0 { DEEDS_HOLD } else { 0 }
    }

    /// The results a seat is shown now, and for how many ticks they have
    /// shown: a floor's in the plate's first part, a run's at its verdict.
    pub fn results_showing(&self, seat: Option<u32>) -> Option<(&Results, u32)> {
        let results = self.floor_results.as_ref()?;
        if results.end != End::Cleared {
            if !self.over() {
                return None;
            }
            // The verdict stands a moment on the room before its results.
            return results.stood.checked_sub(VERDICT_FIRST).map(|age| (results, age));
        }
        let plate = self.floor_plate(seat)?;
        (plate.age < plate.results).then_some((results, u32::from(plate.age)))
    }
}

/// Ticks the verdict stands on the room before its results come up, and
/// the longer it stands with deeds to show.
pub const VERDICT_FIRST: u32 = 3 * HZ / 2;
pub const DEEDS_HOLD: u32 = 8 * HZ;

#[cfg(test)]
#[path = "../../tests/rules/together_floor_stats__tests.rs"]
mod tests;
