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

pub const NEAR: f32 = 4.0;
pub const LONG_HOLD: u16 = 2 * HZ as u16;
pub const STEADY: u16 = 3 * HZ as u16 / 2;
pub const JOLT: u32 = 4;
pub const ECHO: u32 = 8 * HZ;
pub const CHORD_HOLD: (u32, u32) = (75, 30);
pub const STILL_TICKS: (u32, u32) = (4 * HZ, 3 * HZ / 2);
pub const VEIL_TICKS: (u32, u32) = (4 * HZ, 3 * HZ / 2);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wits {
    #[serde(default)]
    pub foe: u32,
    #[serde(default)]
    pub held: u16,
    #[serde(default)]
    pub steady: u16,
    #[serde(default)]
    pub flinch: u8,
}

impl Wits {
    pub fn calm(&self) -> bool {
        *self == Wits::default()
    }
}

pub(super) type Knights = Vec<(u32, f32, f32)>;

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
        .chain(
            run.squire_field.companions.values()
                .filter(|s| s.targetable())
                .map(|s| (s.actor.x, s.actor.y)),
        )
        .chain(run.minions.decoys())
        .chain(run.squire_hall.decoys(run))
        .chain(super::countryside_squires::decoys(&run.countryside_squires))
        .chain(run.squire_decoys())
        .collect();
    let mut seen = Vec::new();
    peasant::sights(run.tick, &run.scarecrows, enemy, &knights, &decoys, &mut seen);
    run.minions.roared(enemy, &mut seen);
    super::countryside_squires::roared(&run.countryside_squires, enemy, &mut seen);
    run.squire_academy.roared(enemy, &mut seen);
    if let Some((door, guests)) = run.garden_lure()
        && guests.contains(&enemy.id)
    {
        seen.clear();
        seen.push(door);
    }
    seen.into_iter().min_by(|a, b| {
        (a.0 - enemy.x)
            .hypot(a.1 - enemy.y)
            .total_cmp(&(b.0 - enemy.x).hypot(b.1 - enemy.y))
    })
}

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
