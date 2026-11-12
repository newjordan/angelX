//! What an open floor keeps of what happened on it: the monsters that fell
//! leave their remains where they fell, and a knight who goes down leaves a
//! cairn there, flying their colours. Both are the floor's own state
//! (`layout::Open::traces`) and are drawn where they lie (`arena/chronicle`);
//! no rule reads them. A new floor starts bare.

use super::*;

/// The most traces a floor keeps. The oldest remains give way first; a
/// cairn is kept until its knight has lain down `CAIRNS_EACH` times more.
pub const TRACES: usize = 120;
/// How many cairns one knight has on a floor.
pub const CAIRNS_EACH: usize = 3;
/// Remains are not laid within this many units of two older ones.
const CROWD: f32 = 1.1;
/// The longest name a cairn keeps, in letters.
pub const NAME_LEN: usize = 12;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Trace {
    /// A monster's remains, where it fell on tick `born`.
    Remains { kind: EnemyKind, x: f32, y: f32, born: u64 },
    /// Where knight `knight` went down on tick `born`.
    Cairn { knight: u32, name: String, x: f32, y: f32, born: u64 },
}

impl Trace {
    pub fn at(&self) -> (f32, f32) {
        match self {
            Trace::Remains { x, y, .. } | Trace::Cairn { x, y, .. } => (*x, *y),
        }
    }

    pub fn born(&self) -> u64 {
        match self {
            Trace::Remains { born, .. } | Trace::Cairn { born, .. } => *born,
        }
    }
}

/// Whether a floor's traces are within the bounds a save or a wire frame
/// may carry.
pub fn valid(traces: &[Trace]) -> bool {
    traces.len() <= TRACES
        && traces.iter().all(|t| {
            let (x, y) = t.at();
            x.is_finite()
                && y.is_finite()
                && match t {
                    Trace::Cairn { name, .. } => name.chars().count() <= NAME_LEN,
                    Trace::Remains { .. } => true,
                }
        })
}

/// Whether a monster leaves remains: it has a body (`gore::body`), or is one
/// of the great ones. A spirit, a prop and a ward leave none.
pub fn leaves_remains(kind: EnemyKind) -> bool {
    gore::body(kind).is_some()
        || matches!(kind, EnemyKind::Boss | EnemyKind::Dragon | EnemyKind::PitTyrant | EnemyKind::Broodmarrow)
}

impl Run {
    /// The tick's dead, `(kind, x, y)` each, lay their remains on the open
    /// map they fell on.
    pub(super) fn leave_remains(&mut self, fallen: &[(EnemyKind, f32, f32)]) {
        if !self.open_map() {
            return;
        }
        let born = self.tick.max(1);
        let Some(open) = self.dungeon.open.as_mut() else {
            return;
        };
        for &(kind, x, y) in fallen.iter().filter(|f| leaves_remains(f.0)) {
            let crowd = open
                .traces
                .iter()
                .filter(|t| matches!(t, Trace::Remains { .. }) && {
                    let (tx, ty) = t.at();
                    (tx - x).hypot(ty - y) < CROWD
                })
                .count();
            if crowd >= 2 {
                continue;
            }
            Self::lay(open, Trace::Remains { kind, x, y, born });
        }
    }

    /// Knight `id` has gone down where they stand: a cairn goes up.
    pub(super) fn leave_cairn(&mut self, id: u32) {
        if !self.open_map() {
            return;
        }
        let Some(hero) = self.players.get(&id) else {
            return;
        };
        let name: String = hero.shown_name().to_uppercase().chars().take(NAME_LEN).collect();
        let (x, y) = (hero.x, hero.y);
        let born = self.tick.max(1);
        let Some(open) = self.dungeon.open.as_mut() else {
            return;
        };
        // Their oldest cairns give way to the newest.
        let mine = |t: &Trace| matches!(t, Trace::Cairn { knight, .. } if *knight == id);
        while open.traces.iter().filter(|t| mine(t)).count() >= CAIRNS_EACH {
            if let Some(oldest) = open.traces.iter().position(mine) {
                open.traces.remove(oldest);
            } else {
                break;
            }
        }
        Self::lay(open, Trace::Cairn { knight: id, name, x, y, born });
    }

    /// Add a trace; at the cap the oldest remains go first.
    fn lay(open: &mut layout::Open, trace: Trace) {
        while open.traces.len() >= TRACES {
            match open.traces.iter().position(|t| matches!(t, Trace::Remains { .. })) {
                Some(oldest) => open.traces.remove(oldest),
                None => open.traces.remove(0),
            };
        }
        open.traces.push(trace);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/chronicle__tests.rs"]
mod tests;
