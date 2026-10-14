//! One seat's command for one tick on the engine (`docs/ENGINE.md`, "The
//! Delve on the engine"): the keys held at the client's sample, and the
//! presses since the one before, so a tap shorter than a tick still acts.
//!
//! The step sees `held` with `pressed` laid over it (`input`). A tick the
//! seat's next command has not come for repeats what was held, without its
//! presses and without the one-shot fields (`vote`, `leave_activity`,
//! `activity_menu`, `skip_plate`). Commands that pile up fold into the next
//! one keeping their presses. The rules' own press latches (`ult_key`,
//! `take_key`...) see these inputs exactly as they saw the old ones.
//!
//! The bytes are the command's own, a few a tick, not a serde tree. Each
//! `Input` is:
//!
//! | bytes | what |
//! |---|---|
//! | 1 | the four axes, two bits each (`move_x`, `move_y`, `aim_x`, `aim_y` from the low bits), each its value + 1 |
//! | varint | the flags (`FLAGS`, in order from bit 0), then bit 11: slots follow, bit 12: a vote follows |
//! | varint, if slots | `play`, `cast << 3`, `skill << 6` |
//! | varint + 1, if a vote | the vote's serial, then its action (`action_code`) |
//!
//! A command is `held` then `pressed`: four bytes while walking and
//! firing, more only when a card, a spell, a skill or a vote is pressed.

use super::Input;
use super::votes::{Action, Command as Vote};
use angel_net::cmd::Command;
use angel_net::codec::{Bytes, put_varint};

/// One seat's command for one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DelveCmd {
    /// The keys down at the sample.
    pub held: Input,
    /// What went down since the sample before, even if let go since.
    pub pressed: Input,
}

/// The two-bit code of an axis that does not exist (-1..=1 are 0..=2).
const NO_AXIS: u8 = 3;
/// The flag bits, in order from bit 0.
const FLAGS: usize = 12;
/// After the flags: the slots byte follows, a vote follows.
const HAS_SLOTS: u64 = 1 << FLAGS;
const HAS_VOTE: u64 = 1 << (FLAGS + 1);
/// Bits a slot value takes in the slots varint (`play`, `cast`, `skill`
/// are each at most 5).
const SLOT_BITS: u32 = 3;
/// The longest a command's bytes can be: two inputs of an axes byte, a
/// two-byte flags varint, a two-byte slots varint, a ten-byte serial and an
/// action byte.
pub const MAX_BYTES: usize = 2 * (1 + 2 + 2 + 10 + 1);

impl DelveCmd {
    /// What the step sees: `held` with `pressed` laid over it.
    pub fn input(&self) -> Input {
        lay_over(self.held, &self.pressed)
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        put_input(out, &self.held);
        put_input(out, &self.pressed);
    }

    /// A command's bytes, or `None` when they are not one: a cut-off or
    /// overlong body, an axis or flag that does not exist, or an input the
    /// rules would refuse (`Input::valid`).
    pub fn decode(bytes: &[u8]) -> Option<DelveCmd> {
        if bytes.len() > MAX_BYTES {
            return None;
        }
        let mut b = Bytes::new(bytes);
        let held = read_input(&mut b)?;
        let pressed = read_input(&mut b)?;
        b.end().ok()?;
        (held.valid() && pressed.valid()).then_some(DelveCmd { held, pressed })
    }
}

impl Command for DelveCmd {
    fn neutral() -> Self {
        DelveCmd::default()
    }

    fn held_only(&self) -> Self {
        DelveCmd {
            held: Input {
                vote: None,
                leave_activity: false,
                activity_menu: false,
                skip_plate: false,
                again: false,
                ..self.held
            },
            pressed: Input::default(),
        }
    }

    fn absorb(&mut self, older: &Self) {
        self.pressed = lay_over(older.pressed, &self.pressed);
    }
}

/// `under` with every field `over` sets laid on top: a flag set in either,
/// an axis or slot `over` gives, `over`'s vote if it has one.
fn lay_over(under: Input, over: &Input) -> Input {
    let axis = |u: i8, o: i8| if o != 0 { o } else { u };
    let slot = |u: u8, o: u8| if o != 0 { o } else { u };
    Input {
        leave_activity: under.leave_activity || over.leave_activity,
        activity_menu: under.activity_menu || over.activity_menu,
        vote: over.vote.or(under.vote),
        move_x: axis(under.move_x, over.move_x),
        move_y: axis(under.move_y, over.move_y),
        aim_x: axis(under.aim_x, over.aim_x),
        aim_y: axis(under.aim_y, over.aim_y),
        fire: under.fire || over.fire,
        dash: under.dash || over.dash,
        bomb: under.bomb || over.bomb,
        swing: under.swing || over.swing,
        play: slot(under.play, over.play),
        cast: slot(under.cast, over.cast),
        vigil: under.vigil || over.vigil,
        ult: under.ult || over.ult,
        take: under.take || over.take,
        interact: under.interact || over.interact,
        skill: slot(under.skill, over.skill),
        skip_plate: under.skip_plate || over.skip_plate,
        again: under.again || over.again,
    }
}

fn flags(input: &Input) -> [bool; FLAGS] {
    [
        input.fire,
        input.dash,
        input.bomb,
        input.swing,
        input.vigil,
        input.ult,
        input.take,
        input.interact,
        input.leave_activity,
        input.activity_menu,
        input.skip_plate,
        input.again,
    ]
}

fn put_input(out: &mut Vec<u8>, input: &Input) {
    let code = |axis: i8| (axis.clamp(-1, 1) + 1) as u8;
    out.push(code(input.move_x) | code(input.move_y) << 2 | code(input.aim_x) << 4 | code(input.aim_y) << 6);
    let mut bits = flags(input)
        .iter()
        .enumerate()
        .fold(0u64, |bits, (i, &on)| bits | u64::from(on) << i);
    let slots = u64::from(input.play)
        | u64::from(input.cast) << SLOT_BITS
        | u64::from(input.skill) << (2 * SLOT_BITS);
    if slots != 0 {
        bits |= HAS_SLOTS;
    }
    if input.vote.is_some() {
        bits |= HAS_VOTE;
    }
    put_varint(out, bits);
    if slots != 0 {
        put_varint(out, slots);
    }
    if let Some(vote) = input.vote {
        put_varint(out, vote.serial);
        out.push(action_code(vote.action));
    }
}

fn read_input(b: &mut Bytes) -> Option<Input> {
    let axes = b.byte().ok()?;
    let axis = |shift: u32| -> Option<i8> {
        let code = axes >> shift & 3;
        (code != NO_AXIS).then_some(code as i8 - 1)
    };
    let bits = b.varint().ok()?;
    if bits >> (FLAGS + 2) != 0 {
        return None;
    }
    let on = |i: usize| bits >> i & 1 == 1;
    let slots = if bits & HAS_SLOTS != 0 {
        let slots = b.varint().ok()?;
        if slots == 0 || slots >> (3 * SLOT_BITS) != 0 {
            return None;
        }
        slots
    } else {
        0
    };
    let slot = |at: u32| (slots >> (at * SLOT_BITS) & ((1 << SLOT_BITS) - 1)) as u8;
    let vote = if bits & HAS_VOTE != 0 {
        let serial = b.varint().ok()?;
        Some(Vote { serial, action: action_of(b.byte().ok()?)? })
    } else {
        None
    };
    Some(Input {
        move_x: axis(0)?,
        move_y: axis(2)?,
        aim_x: axis(4)?,
        aim_y: axis(6)?,
        fire: on(0),
        dash: on(1),
        bomb: on(2),
        swing: on(3),
        vigil: on(4),
        ult: on(5),
        take: on(6),
        interact: on(7),
        leave_activity: on(8),
        activity_menu: on(9),
        skip_plate: on(10),
        again: on(11),
        play: slot(0),
        cast: slot(1),
        skill: slot(2),
        vote,
    })
}

/// A ballot action as one byte: the six moves, then clear and untried,
/// then a rating of n stars as 8 + n.
fn action_code(action: Action) -> u8 {
    match action {
        Action::Toggle => 0,
        Action::Close => 1,
        Action::Up => 2,
        Action::Down => 3,
        Action::Left => 4,
        Action::Right => 5,
        Action::Clear => 6,
        Action::Untried => 7,
        // `Input::valid` keeps n to 1..=5 on both ends.
        Action::Rate(n) => 8u8.saturating_add(n),
    }
}

fn action_of(code: u8) -> Option<Action> {
    Some(match code {
        0 => Action::Toggle,
        1 => Action::Close,
        2 => Action::Up,
        3 => Action::Down,
        4 => Action::Left,
        5 => Action::Right,
        6 => Action::Clear,
        7 => Action::Untried,
        9..=13 => Action::Rate(code - 8),
        _ => return None,
    })
}

#[cfg(test)]
#[path = "../../tests/rules/together_command__tests.rs"]
mod tests;
