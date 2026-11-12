//! The resolution ladder (the operator, 2026-10-28: "over time you increase
//! resolution, thats part of the game"; docs/LADDER.md). A knight starts a
//! run at the game's own grain, sixteen pixels a figure, and as the run goes
//! on climbs to thirty-two, sixty-four and a hundred and twenty-eight: a
//! window that has finer drawings of a figure shows the finest its rung
//! allows (`render::overworld::arena::hd_pack`), and everything else keeps
//! its sixteen-pixel inks.
//!
//! This is the one place that says which rung a knight is on. It is worked
//! out from the run as it stands, never saved or sent: every seat that
//! holds the same snapshot comes to the same rung, and an old save climbs
//! as a new one does. Gameplay never reads it; the grain of the game, the
//! wire and the terminal are as they were.
//!
//! The rule is a knob (`Rule`, `ANGEL_DELVE_LADDER`): by default the floor
//! the party has reached (`Rule::Floor`, `FLOOR_RUNGS`), so the run's first
//! floor is sixteen and the Deep is a hundred and twenty-eight; or the
//! knight's own level, which the realm keeps from run to run
//! (`Rule::Level`, `LEVEL_RUNGS`); or one rung for everyone (`Rule::Fixed`,
//! for shots and tests).

use std::sync::OnceLock;

use super::Run;

/// A rung: the pixels a knight's figure is drawn in, top to toe, at its
/// finest. Sixteen is the game's own grain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rung(u16);

impl Rung {
    /// The game's own grain: what every run starts at.
    pub const BASE: Rung = Rung(16);
    pub const ALL: [Rung; 4] = [Rung(16), Rung(32), Rung(64), Rung(128)];

    /// The rung of `px` pixels, if it is one of the ladder's.
    pub fn of(px: u32) -> Option<Rung> {
        Self::ALL.into_iter().find(|r| u32::from(r.0) == px)
    }

    /// Its pixels, top to toe.
    pub fn px(self) -> u32 {
        u32::from(self.0)
    }

    /// A drawing's pixels to one of the game's at this rung: 1, 2, 4, 8.
    pub fn scale(self) -> u32 {
        self.px() / Self::BASE.px()
    }

    /// The rung above, if there is one.
    pub fn next(self) -> Option<Rung> {
        Self::ALL.into_iter().find(|r| *r > self)
    }
}

impl std::fmt::Display for Rung {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} px", self.0)
    }
}

/// What a knight climbs the ladder by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// The floor the party has reached in this run (`Run::floor`): the
    /// default.
    Floor,
    /// The knight's level, kept by the realm from run to run.
    Level,
    /// Everyone on one rung.
    Fixed(Rung),
}

/// The floor each rung begins on (`Rule::Floor`): the run's first floor
/// at the game's grain, thirty-two from the second floor, sixty-four from
/// the deep below the dragon (floor four, where the dungeon stands up), a
/// hundred and twenty-eight on the Deep's road (floor seven) and below.
pub const FLOOR_RUNGS: [(u32, Rung); 4] = [(0, Rung(16)), (2, Rung(32)), (4, Rung(64)), (7, Rung(128))];

/// The level each rung begins at (`Rule::Level`): the top rung from level 5
/// (`talents::LEVELS`).
pub const LEVEL_RUNGS: [(u32, Rung); 4] = [(0, Rung(16)), (2, Rung(32)), (3, Rung(64)), (5, Rung(128))];

/// The rule in force: `ANGEL_DELVE_LADDER` read once (`floor`, the
/// default; `level`; or a rung's pixels, `16`, `32`, `64`, `128`), unless a
/// test or a shot has set one (`set_rule`).
pub fn rule() -> Rule {
    if let Some(rule) = OVERRIDE.with(std::cell::Cell::get) {
        return rule;
    }
    static FROM_ENV: OnceLock<Rule> = OnceLock::new();
    *FROM_ENV.get_or_init(|| parse_rule(std::env::var("ANGEL_DELVE_LADDER").ok().as_deref()).unwrap_or(Rule::Floor))
}

/// A rule as `ANGEL_DELVE_LADDER` says it.
pub fn parse_rule(text: Option<&str>) -> Option<Rule> {
    match text?.trim() {
        "floor" | "" => Some(Rule::Floor),
        "level" => Some(Rule::Level),
        px => px.strip_suffix("px").unwrap_or(px).trim().parse().ok().and_then(Rung::of).map(Rule::Fixed),
    }
}

std::thread_local! {
    static OVERRIDE: std::cell::Cell<Option<Rule>> = const { std::cell::Cell::new(None) };
}

/// Put `rule` in force on this thread (a shot, a test); `None` gives it
/// back to the environment.
pub fn set_rule(rule: Option<Rule>) {
    OVERRIDE.with(|o| o.set(rule));
}

/// The rung of `value` on `table`: the last whose start it has reached.
fn on(table: &[(u32, Rung)], value: u32) -> Rung {
    table.iter().rev().find(|(from, _)| value >= *from).map_or(Rung::BASE, |(_, r)| *r)
}

/// The level the realm keeps for knight `id`, 1 if it keeps none.
fn level(run: &Run, id: u32) -> u32 {
    run.players
        .get(&id)
        .and_then(|h| run.home.knights.get(&super::talents::profile_key(h)))
        .map_or(1, |p| p.level())
}

/// The rung knight `id` is on, by `rule`.
pub fn rung_by(run: &Run, id: u32, rule: Rule) -> Rung {
    match rule {
        Rule::Floor => on(&FLOOR_RUNGS, run.floor()),
        Rule::Level => on(&LEVEL_RUNGS, level(run, id)),
        Rule::Fixed(rung) => rung,
    }
}

/// The rung knight `id` is on (by the rule in force).
pub fn rung(run: &Run, id: u32) -> Rung {
    rung_by(run, id, rule())
}

/// The rung a window draws knight `id`'s figure at: the rung they are on, but
/// never under the one a drawn-whole look is drawn at (`BODY_RUNG`) for a
/// knight whose mirror wish was drawn whole (`Run::has_mirror_body`): a wished
/// look is not a floor's reward, and its drawings are that rung's.
pub fn drawn_rung(run: &Run, id: u32) -> Rung {
    let rung = rung(run, id);
    if run.has_mirror_body(id) { rung.max(BODY_RUNG) } else { rung }
}

/// The rung a body-maker draws a wished look at: 4 drawing pixels to the
/// game's, a knight 64 rows tall.
pub const BODY_RUNG: Rung = Rung(64);

/// What lies ahead on the ladder for knight `id`: the next rung, and the
/// floor (`Rule::Floor`) or level (`Rule::Level`) it begins at. `None` at
/// the top, or when one rung is fixed for everyone. For a run's end and a
/// rung-up's moment: "the next rung, at floor 4".
pub fn next(run: &Run, id: u32) -> Option<(Rung, Climb)> {
    let rule = rule();
    let now = rung_by(run, id, rule);
    let (table, climb): (&[(u32, Rung)], fn(u32) -> Climb) = match rule {
        Rule::Floor => (&FLOOR_RUNGS, Climb::Floor),
        Rule::Level => (&LEVEL_RUNGS, Climb::Level),
        Rule::Fixed(_) => return None,
    };
    table.iter().find(|(_, r)| *r > now).map(|&(at, r)| (r, climb(at)))
}

/// Where a rung begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Climb {
    Floor(u32),
    Level(u32),
}

#[cfg(test)]
#[path = "../../tests/rules/resolution__tests.rs"]
mod tests;
