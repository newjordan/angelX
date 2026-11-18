//! A bot's hands are the module's too (`Session::bot_with` asks it what a
//! scripted knight presses): this is only how well it flies.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Sure,
    Novice,
}
