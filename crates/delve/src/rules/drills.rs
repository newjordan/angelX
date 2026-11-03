//! A kit's drill: the one rule that makes it play apart from the kits that
//! carry the same arm. Bow, crossbow, handgonne and the Knight's Blade are
//! each worn by kits with no spells of their own, so each family has one
//! knight drilled past the rest:
//!
//! - **The Unbroken Loop** (the Loop Knight, bow): arrows loosed without a
//!   rest build the beat, and the beat quickens the string. A rest of
//!   `BEAT_LIFE` loses all of it.
//! - **The Measured Bolt** (the Competition Knight, crossbow): a bolt after
//!   a full `MEASURE` of rest strikes `MEASURED` hard. A hurried bolt is a
//!   plain one, and starts the measure again.
//! - **The Purse** (the Money Knight, handgonne): the gold they carry
//!   weighs the balls, `PURSE_MOST` percent at most. Spend it and the
//!   gonne goes light again.
//! - **The Clean Copy** (the Composer, Knight's Blade): every `CLEAN_EVERY`th
//!   stroke of an unbroken chain cuts all the way round, half again as hard.
//!
//! A drill goes with the kit's own arm: a knight who picks up another, forges
//! one, or wishes themselves a shape has left it behind until the arm is back.
//! Clocks are fixed ticks on the hero's replicated `Stance`, kept by the
//! shared fire path, so a client's predicted shots match the host's. No dice.

use super::*;

/// How many beats the Loop Knight can hold.
pub const BEAT_MOST: u32 = 6;
/// Percent quicker the string draws for each beat held.
pub const BEAT_RATE: u32 = 8;
/// Ticks without an arrow before the beat is lost.
pub const BEAT_LIFE: u32 = 24;
/// Ticks of rest that make the next bolt a measured one: twice the
/// crossbow's own rearm, quickened as the rearm is.
pub const MEASURE: u32 = 46;
/// A measured bolt's damage, in percent.
pub const MEASURED: u32 = 250;
/// Gold carried for each percent the purse adds.
pub const PURSE_STEP: u32 = 5;
/// The most the purse adds, in percent.
pub const PURSE_MOST: u32 = 50;
/// The stroke of a chain that is the clean copy.
pub const CLEAN_EVERY: u32 = 3;
/// Ticks without a stroke before the chain is lost.
pub const STROKE_LIFE: u32 = 30;
/// A clean stroke's damage, in percent.
pub const CLEAN_HIT: u32 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drill {
    Loop,
    Measured,
    Purse,
    Clean,
}

/// The drill and the arm that carries it, for the kit `id`.
pub fn of_kit(id: &str) -> Option<(Drill, &'static str)> {
    match id {
        "loop" => Some((Drill::Loop, "bow")),
        "competition" => Some((Drill::Measured, "crossbow")),
        "money" => Some((Drill::Purse, "handgonne")),
        "composer" => Some((Drill::Clean, "knights-blade")),
        _ => None,
    }
}

/// The drill knight `h` is holding to, with their kit's arm in hand.
pub fn of(h: &Hero) -> Option<Drill> {
    if h.shape.is_some() {
        return None;
    }
    let (drill, arm) = of_kit(h.knight.as_deref()?)?;
    (h.arm.as_deref() == Some(arm)).then_some(drill)
}

/// What a drill lends one pull of a plain arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pull {
    /// The pull's damage, in percent.
    pub damage: u32,
    pub empowered: bool,
}

/// What the purse adds for `gold` carried, in percent.
pub fn purse(gold: u32) -> u32 {
    (gold / PURSE_STEP).min(PURSE_MOST)
}

/// Count a pull of a plain arm: the host and a predicting client both come
/// through here (`Hero::loose`).
pub(super) fn pull(h: &mut Hero) -> Pull {
    let mut pull = Pull { damage: 100, empowered: false };
    match of(h) {
        Some(Drill::Loop) => {
            h.hero_kit.beat = (h.hero_kit.beat + 1).min(BEAT_MOST);
            h.hero_kit.beat_life = BEAT_LIFE;
        }
        Some(Drill::Measured) => {
            if h.hero_kit.measure == 0 {
                pull = Pull { damage: MEASURED, empowered: true };
            }
            h.hero_kit.measure = h.cooldown(MEASURE);
        }
        Some(Drill::Purse) => pull.damage += purse(h.carried.get(Spoil::Gold)),
        Some(Drill::Clean) | None => {}
    }
    pull
}

/// Count a stroke of the blade: true on the clean one.
pub(super) fn stroke(h: &mut Hero) -> bool {
    if of(h) != Some(Drill::Clean) {
        return false;
    }
    h.hero_kit.strokes += 1;
    h.hero_kit.stroke_life = STROKE_LIFE;
    let clean = h.hero_kit.strokes >= CLEAN_EVERY;
    if clean {
        h.hero_kit.strokes = 0;
    }
    clean
}

#[cfg(test)]
#[path = "../../tests/rules/drills__tests.rs"]
mod tests;
