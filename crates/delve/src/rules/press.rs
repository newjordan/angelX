//! Co-op pressure: a party of knights is never an easier delve than one.
//!
//! Monster health stays as it is. A bigger party meets more pressure
//! instead (the head-count tables live in `stakes`), and these rules keep
//! the party from answering it for free:
//!
//!  - **Spread aggro.** Each monster hunts its own knight (by its id across
//!    the knights standing) and turns on whoever last hurt it. It still
//!    sees decoys (Phantasm's images, a hireling) and any knight who walks
//!    within `NEAR` of it, so a blade can always draw it.
//!  - **Holds wear off.** A monster held still for `LONG_HOLD` ticks in a
//!    row (a stun, a Grand Chord, a Stillhour) shrugs off any long hold for
//!    `STEADY` ticks after it is free. A hit's jolt still lands.
//!  - **Party-wide ultimates don't chain.** A second Grand Chord, Stillhour
//!    or Lady's Veil within `ECHO` ticks of the last holds for the short
//!    time below, not the full one.

use super::*;

/// A knight this near a monster is always in its sight, whoever it hunts.
pub const NEAR: f32 = 4.0;
/// Ticks held still in a row that make a long hold.
pub const LONG_HOLD: u16 = 2 * HZ as u16;
/// Ticks after a long hold in which another long hold slides off.
pub const STEADY: u16 = 3 * HZ as u16 / 2;
/// The longest hold that still lands on a steady monster: a hit's jolt.
pub const JOLT: u32 = 4;
/// Ticks after one party-wide control ultimate in which the next is short.
pub const ECHO: u32 = 8 * HZ;
/// A Grand Chord's stun, a Stillhour's life and a Lady's Veil's wings, in
/// full and as an echo.
pub const CHORD_HOLD: (u32, u32) = (75, 30);
pub const STILL_TICKS: (u32, u32) = (4 * HZ, 3 * HZ / 2);
pub const VEIL_TICKS: (u32, u32) = (4 * HZ, 3 * HZ / 2);

/// What a monster has in mind: who it hunts, and how it stands up to being
/// held still.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wits {
    /// The knight that last hurt it (0: none yet); it turns on them.
    #[serde(default)]
    pub foe: u32,
    /// Ticks it has been held still in a row.
    #[serde(default)]
    pub held: u16,
    /// Ticks left in which a long hold slides off it.
    #[serde(default)]
    pub steady: u16,
    /// Ticks until a hit may jolt it still again (`feel`).
    #[serde(default)]
    pub flinch: u8,
}

impl Wits {
    /// Nothing in mind yet: left off the wire.
    pub fn calm(&self) -> bool {
        *self == Wits::default()
    }
}

/// The knights a monster may hunt this tick: `(id, x, y)` of each one
/// standing and seen.
pub(super) type Knights = Vec<(u32, f32, f32)>;

/// What `enemy` sees this tick, into `out`: its own knight (the one that
/// last hurt it, or its share of the party), any knight within `NEAR`, and
/// every decoy. Every monster's own aim then picks the nearest of these.
pub(super) fn sights(
    enemy: &Enemy,
    knights: &[(u32, f32, f32)],
    decoys: &[(f32, f32)],
    out: &mut Vec<(f32, f32)>,
) {
    out.clear();
    if let Some(&(own, ..)) = knights
        .iter()
        .find(|k| k.0 == enemy.wits.foe)
        .or_else(|| knights.get(enemy.id as usize % knights.len().max(1)))
    {
        let reach = NEAR + enemy.radius();
        for &(id, x, y) in knights {
            if id == own || (x - enemy.x).hypot(y - enemy.y) <= reach {
                out.push((x, y));
            }
        }
    }
    out.extend_from_slice(decoys);
}

/// A current target for a directional preview, using the simulation's sight
/// filter, assigned aggro, invisibility and decoys without advancing the run.
pub fn warning_target(run: &Run, enemy: &Enemy) -> Option<(f32, f32)> {
    let knights: Knights = run
        .players
        .iter()
        .filter(|(_, h)| h.targetable() && !h.has_rune(runes::RuneKind::Invisibility))
        .map(|(&id, h)| (id, h.x, h.y))
        .collect();
    let decoys: Vec<_> = run
        .phantoms
        .iter()
        .map(|p| (p.x, p.y))
        .chain(
            run.hireling
                .iter()
                .filter(|h| !h.down())
                .map(|h| (h.x, h.y)),
        )
        .collect();
    let mut seen = Vec::new();
    peasant::sights(run.tick, &run.scarecrows, enemy, &knights, &decoys, &mut seen);
    seen.into_iter().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    })
}

/// One tick of a held monster's wits, before it acts: a long hold slides
/// off a steady monster, and a long hold ending makes it steady. True
/// while it is still held this tick.
pub(super) fn held(enemy: &mut Enemy) -> bool {
    let wits = &mut enemy.wits;
    wits.flinch = wits.flinch.saturating_sub(1);
    if wits.steady > 0 {
        wits.steady -= 1;
        if enemy.frozen > JOLT {
            enemy.frozen = 0;
        }
    }
    if enemy.frozen == 0 {
        wits.held = 0;
        return false;
    }
    enemy.frozen -= 1;
    wits.held = wits.held.saturating_add(1);
    if enemy.frozen == 0 && wits.held >= LONG_HOLD {
        wits.steady = STEADY;
    }
    true
}

impl Run {
    /// A party-wide control ultimate cast now: its hold, full or short as
    /// the last one was long ago or not. `full` and `echo` are the pair.
    pub(super) fn control_hold(&mut self, (full, echo): (u32, u32)) -> u32 {
        let short = self
            .feel
            .control
            .is_some_and(|at| self.tick.saturating_sub(at) < u64::from(ECHO));
        self.feel.control = Some(self.tick);
        if short {
            self.cues.push("ult_echo".into());
            echo
        } else {
            full
        }
    }
}
