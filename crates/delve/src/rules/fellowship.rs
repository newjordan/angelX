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

pub const STAIRS: u8 = 4;

pub const LIGHT: u8 = 5;

const WAYS: [&str; 6] = ["THE NORTH DOOR", "THE EAST DOOR", "THE SOUTH DOOR", "THE WEST DOOR", "THE STAIRS", "THE LIGHT HOME"];

pub const CALL_EVERY: u64 = 4 * HZ as u64;

pub const CALL_SOUND: &str = "circle_toll";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub waiting: Vec<(u32, u8)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub called: Vec<u32>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub rung: BTreeMap<u32, u64>,
}

impl State {
    pub(super) fn clear(&mut self) {
        self.waiting.clear();
        self.called.clear();
    }

    pub(super) fn wait(&mut self, ways: impl Iterator<Item = (u32, u8)>) {
        self.waiting.extend(ways);
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty() && self.called.is_empty() && self.rung.is_empty()
    }

    pub fn is_waiting(&self, id: u32) -> bool {
        self.waiting.iter().any(|&(seat, _)| seat == id)
    }
}

impl Run {
    pub fn waiting_for_fellowship(&self) -> bool {
        !self.fellowship.waiting.is_empty()
    }

    pub fn fellowship_line(&self, focus: Option<u32>) -> Option<(String, bool)> {
        let &(_, way) = self.fellowship.waiting.first()?;
        if focus.is_some_and(|id| self.fellowship.is_waiting(id)) {
            return Some(("WAITING FOR FELLOWSHIP".into(), true));
        }
        let place = WAYS[usize::from(way).min(WAYS.len() - 1)];
        Some((format!("FRIENDS WAIT AT {place}"), false))
    }

    pub(super) fn counts_in_fellowship(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| h.hp > 0 && !h.stone && !h.absent) && !self.away(id) && !self.caves.inside(id)
    }

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
