//! A guardian's health, under its name plate (`wanted::nameplates`): a
//! dark track in the kit's grey rim, filled with its health. A tick under
//! the bar where its fight turns to its middle (`bosses::MIDDLE_FROM`) and
//! one where it rages (its own `rage`), dim once passed; the pale bite a
//! blow took, draining after it (`Enemy::drain`), its edge lit while a
//! hit's flash lasts (`Enemy::hurt`); and for the two guardians whose
//! fight is a rule of its own, the fill in the state those rules read:
//! Cinderjaw's iron, open furnace and molten rage (`furnace`), the Relic
//! Liege's ward and its cracking (`court`).
//!
//! All of it reads the run, so a friend's mirror and a browser guest's
//! picture show what the host's does.

use super::super::ink::{Img, hash};
use crate::rules::bosses::{Boss, LAST_FROM, MIDDLE_FROM, Stage};
use crate::rules::{Enemy, court, furnace};

/// The track's height inside its rim.
const TRACK_H: i32 = 4;
/// What a guardian's fill says of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    /// Its opening and middle: red.
    Plain,
    /// Its rage: hot orange.
    Raging,
    /// Cinderjaw's iron, letting a blow through at `furnace::ARMOR_TAKES`.
    Armored,
    /// Cinderjaw's furnace open after its breath: its core takes the most.
    Venting,
    /// Cinderjaw past its rage: no iron left.
    Molten,
    /// The Relic Liege's ward, whole.
    Warded,
    /// The Relic Liege's ward broken in its rage.
    Cracked,
}

/// The state guardian `e` (of `boss`) is in, read off the rules.
pub(super) fn state(e: &Enemy, boss: &Boss) -> State {
    if furnace::is(boss) {
        if furnace::venting(e, boss) {
            State::Venting
        } else if furnace::molten(e, boss) {
            State::Molten
        } else if furnace::armored(e, boss) {
            State::Armored
        } else {
            State::Plain
        }
    } else if court::is(boss) {
        if court::cracked(e, boss) {
            State::Cracked
        } else if court::ward(e, boss) < 100 {
            State::Warded
        } else {
            State::Plain
        }
    } else if boss.stage(e.hp, e.max_hp) == Stage::Rage {
        State::Raging
    } else {
        State::Plain
    }
}

/// The fill's body and the lit row along its top, at `tick`.
fn inks(state: State, tick: u64) -> (char, char) {
    match state {
        State::Plain => ('7', 'R'),
        State::Raging | State::Molten | State::Cracked => ('@', '5'),
        State::Armored => ('J', 'h'),
        // The open core beats, so the party sees the moment to strike.
        State::Venting if tick / 4 % 2 == 0 => ('5', '6'),
        State::Venting => ('@', '6'),
        State::Warded => ('U', 'v'),
    }
}

/// Where its rage begins, as a share of its health (`Boss::stage`).
fn rage_from(boss: &Boss) -> f32 {
    if boss.rage > 0.0 { boss.rage } else { LAST_FROM }
}

/// The bar under a plate `w` wide from `x0`, whose foot is row `foot`.
///
/// `fill` (0 to 1) is how much of it is shown yet: a guardian's entrance
/// fills its bar from empty (`wanted::nameplates`), with a lit leading
/// edge, and its notches come as the fill passes them.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(cv: &mut Img, e: &Enemy, boss: &Boss, (x0, foot, w): (i32, i32, i32), tick: u64, delivered: bool, fill: f32) {
    let max = e.max_hp.max(1) as f32;
    let fill = fill.clamp(0.0, 1.0);
    let share = (e.hp as f32 / max).clamp(0.0, 1.0) * fill;
    let (x, y, span) = (x0 + 1, foot + 2, w - 2);
    // The track and its rim, as every health bar wears them.
    super::marks::bar(cv, x, y, span, TRACK_H, 0.0, 'k', delivered);
    let px = |s: f32| (s.clamp(0.0, 1.0) * span as f32).ceil() as i32;
    let lit = px(share);
    // What the blows took, pale, still draining.
    if e.drain > e.hp {
        let bite = px(e.drain as f32 / max);
        cv.rect(x + lit, y, bite - lit, TRACK_H, if e.hurt > 0 { 'T' } else { '$' });
    }
    let state = state(e, boss);
    let (body, top) = inks(state, tick);
    cv.rect(x, y, lit, TRACK_H, body);
    cv.rect(x, y, lit, 1, top);
    match state {
        // Rivets along the iron.
        State::Armored => {
            for k in (3..lit - 1).step_by(6) {
                cv.put(x + k, y + 2, 'G');
            }
        }
        // Embers running in the molten fill.
        State::Molten => {
            for k in 0..lit {
                if hash(k, (tick / 3) as i32, 61) % 7 == 0 {
                    cv.put(x + k, y + 1 + (k & 1), '7');
                }
            }
        }
        // The broken ward's cracks.
        State::Cracked => {
            for k in (5..lit - 1).step_by(9) {
                cv.put(x + k, y, 'k');
                cv.put(x + k + 1, y + 1, 'k');
                cv.put(x + k, y + 2, 'k');
                cv.put(x + k + 1, y + 3, 'k');
            }
        }
        _ => {}
    }
    // A hit's flash: the edge it bit to, lit; and while the bar fills,
    // its leading edge.
    if (e.hurt > 0 || fill < 1.0) && lit > 0 {
        cv.rect(x + lit - 1, y, 1, TRACK_H, 'c');
    }
    // Where its fight turns: a notch through the track and a tick under
    // the bar, grey for its middle and brass for its rage, dim once passed.
    for (from, ahead) in [(MIDDLE_FROM, 'h'), (rage_from(boss), 'O')] {
        if fill < from {
            continue;
        }
        let mx = x + px(from) - 1;
        let ink = if share < from { 'g' } else { ahead };
        cv.rect(mx, y + 1, 1, TRACK_H - 1, 'k');
        cv.put(mx, y + TRACK_H + 1, ink);
        cv.put(mx, y + TRACK_H + 2, ink);
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__guardian_bar__tests.rs"]
mod tests;
