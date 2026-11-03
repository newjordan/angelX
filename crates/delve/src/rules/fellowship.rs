//! The fellowship at a way out. A doorway, or the stairs down, moves the
//! party only when every knight who counts stands in that same one: a knight
//! who is alive, here, awake and not turned to stone. Fallen, away and
//! absent knights (`Run::away`, `Hero::absent`) never hold the party back,
//! and a knight alone goes at once.
//!
//! Knights who reach a way first wait in it, and may walk back out whenever
//! they like: nothing locks or pushes them, and leaving stops counting them.
//! While some wait and the rest have yet to come, the waiting see
//! "WAITING FOR FELLOWSHIP" and their friends are shown where they are
//! awaited (`Run::fellowship_line`). A retreat to the kingdom stays a seat's
//! own decision (`exits`), and the boss door, the guardian's lock and the
//! Winding Stair's hold work as before.
//!
//! "if fellowship is waiting for people there should be a blinking arrow and
//! ping if you're in the minority to join your fellowship" (the operator,
//! 2026-10-19). Once most of the party waits in one way, each knight who
//! counts and is not there is called: a blinking arrow on their own screen
//! points to where the rest wait (`Run::fellowship_call`), and a bell rings
//! for them alone when the call begins and every `CALL_EVERY` after, until
//! they join or the wait ends (`Run::call_fellowship`).

use super::*;

/// The stairs, as a way out beside the four doorways (north, east, south,
/// west: `0..=3`).
pub const STAIRS: u8 = 4;

/// The light home after a slain dragon: the whole party steps in together.
pub const LIGHT: u8 = 5;

/// What each way is called, by number.
const WAYS: [&str; 6] = ["THE NORTH DOOR", "THE EAST DOOR", "THE SOUTH DOOR", "THE WEST DOOR", "THE STAIRS", "THE LIGHT HOME"];

/// How often a called knight hears the bell again: four seconds.
pub const CALL_EVERY: u64 = 4 * HZ as u64;

/// The bell that calls a knight to the fellowship: the new circle's bell.
pub const CALL_SOUND: &str = "circle_toll";

/// The knights waiting in a way out for the rest of the party.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Each waiting knight's seat and the way it stands in.
    pub waiting: Vec<(u32, u8)>,
    /// The knights called to join: most of the party waits in one way and
    /// these, who count, do not stand in it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub called: Vec<u32>,
    /// When each called knight last heard the bell, by tick.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub rung: BTreeMap<u32, u64>,
}

impl State {
    pub(super) fn clear(&mut self) {
        self.waiting.clear();
        self.called.clear();
    }

    /// Knights `ways` (a seat and a way each) wait.
    pub(super) fn wait(&mut self, ways: impl Iterator<Item = (u32, u8)>) {
        self.waiting.extend(ways);
    }

    /// Whether nobody waits or is called (a save then leaves the state out).
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty() && self.called.is_empty() && self.rung.is_empty()
    }

    /// Whether knight `id` stands waiting.
    pub fn is_waiting(&self, id: u32) -> bool {
        self.waiting.iter().any(|&(seat, _)| seat == id)
    }
}

impl Run {
    /// Whether any knight waits in a way out for the rest of the party.
    pub fn waiting_for_fellowship(&self) -> bool {
        !self.fellowship.waiting.is_empty()
    }

    /// What the screen of seat `focus` says of the wait, and whether that
    /// seat is among those waiting (its words flash): "WAITING FOR
    /// FELLOWSHIP" to a knight who stands in a way, and to the rest where
    /// their friends wait.
    pub fn fellowship_line(&self, focus: Option<u32>) -> Option<(String, bool)> {
        let &(_, way) = self.fellowship.waiting.first()?;
        if focus.is_some_and(|id| self.fellowship.is_waiting(id)) {
            return Some(("WAITING FOR FELLOWSHIP".into(), true));
        }
        let place = WAYS[usize::from(way).min(WAYS.len() - 1)];
        Some((format!("FRIENDS WAIT AT {place}"), false))
    }

    /// Whether knight `id` counts for the fellowship: alive, here, awake and
    /// not turned to stone.
    pub(super) fn counts_in_fellowship(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.hp > 0 && !h.stone && !h.absent) && !self.away(id)
    }

    /// The way the most knights wait in (the first, on a tie), and how many.
    fn gathered(&self) -> Option<(u8, usize)> {
        let mut best: Option<(u8, usize)> = None;
        for &(_, way) in &self.fellowship.waiting {
            let n = self.fellowship.waiting.iter().filter(|&&(_, w)| w == way).count();
            if best.is_none_or(|(_, most)| n > most) {
                best = Some((way, n));
            }
        }
        best
    }

    /// Who the fellowship calls after this tick, and their bells: when the
    /// knights waiting in one way outnumber the rest who count, each of the
    /// rest is called, and hears the bell at once and every `CALL_EVERY`.
    /// A knight who joins, or a wait that ends, stops the bell; a new call
    /// rings at once.
    pub(super) fn call_fellowship(&mut self) {
        let counted: Vec<u32> = self.players.keys().copied().filter(|&id| self.counts_in_fellowship(id)).collect();
        let called: Vec<u32> = match self.gathered() {
            Some((way, n)) if n * 2 > counted.len() => counted
                .into_iter()
                .filter(|&id| !self.fellowship.waiting.contains(&(id, way)))
                .collect(),
            _ => Vec::new(),
        };
        let tick = self.tick;
        let state = &mut self.fellowship;
        state.rung.retain(|id, _| called.contains(id));
        for &id in &called {
            if state.rung.get(&id).is_none_or(|&at| tick.saturating_sub(at) >= CALL_EVERY) {
                state.rung.insert(id, tick);
                self.seat_sounds.push((id, CALL_SOUND));
            }
        }
        state.called = called;
    }

    /// Where the fellowship waits for seat `focus`, when it is called: the
    /// middle of the knights gathered there, in the room's units.
    pub fn fellowship_call(&self, focus: Option<u32>) -> Option<(f32, f32)> {
        let id = focus?;
        if !self.fellowship.called.contains(&id) {
            return None;
        }
        let (way, _) = self.gathered()?;
        let there: Vec<(f32, f32)> = self
            .fellowship
            .waiting
            .iter()
            .filter(|&&(_, w)| w == way)
            .filter_map(|(seat, _)| self.players.get(seat).map(|h| (h.x, h.y)))
            .collect();
        let n = there.len().max(1) as f32;
        (!there.is_empty()).then(|| {
            let (sx, sy) = there.iter().fold((0.0, 0.0), |(ax, ay), &(x, y)| (ax + x, ay + y));
            (sx / n, sy / n)
        })
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_fellowship__tests.rs"]
mod tests;
