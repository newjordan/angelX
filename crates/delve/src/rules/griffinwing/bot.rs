//! A bot's hands are the module's too (`Session::bot_with` asks it what a
//! scripted knight presses): this is only how well it flies.

/// How well a bot flies: a sure hand, or a novice's (late on the roll, loose
/// on the aim, letting go of a charge too soon, missing a reflect now and
/// then): the tuning rounds fly both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Sure,
    Novice,
}
