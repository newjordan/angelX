//! What a player's hands are doing, the same on every device: a keyboard, a
//! gamepad and a touch screen all fill one `Hands`. A game turns it into its
//! own command once a tick.

/// The buttons, by bit. Keyboards, gamepads and the touch layout map onto
/// these; a game gives them meaning (the Delve: E interacts, F fires, 1 to 4
/// are spells).
pub mod button {
    pub const UP: u32 = 1 << 0;
    pub const DOWN: u32 = 1 << 1;
    pub const LEFT: u32 = 1 << 2;
    pub const RIGHT: u32 = 1 << 3;
    /// F, the right trigger, the touch fire button.
    pub const FIRE: u32 = 1 << 4;
    /// E, the south face button, the touch use button.
    pub const USE: u32 = 1 << 5;
    pub const SLOT1: u32 = 1 << 6;
    pub const SLOT2: u32 = 1 << 7;
    pub const SLOT3: u32 = 1 << 8;
    pub const SLOT4: u32 = 1 << 9;
    /// Space.
    pub const JUMP: u32 = 1 << 10;
    /// Escape, start.
    pub const MENU: u32 = 1 << 11;
    /// Q, the left shoulder.
    pub const ALT: u32 = 1 << 12;
    /// Shift, the right shoulder.
    pub const SHIFT: u32 = 1 << 13;
    /// Tab, select.
    pub const MAP: u32 = 1 << 14;
    /// Enter.
    pub const OK: u32 = 1 << 15;
    /// The aim stick's directions, for twin-stick games (the Delve aims
    /// with the arrows and the right stick). Held, they make the aim stick
    /// as the four directions make the move stick.
    pub const AIM_UP: u32 = 1 << 16;
    pub const AIM_DOWN: u32 = 1 << 17;
    pub const AIM_LEFT: u32 = 1 << 18;
    pub const AIM_RIGHT: u32 = 1 << 19;
    /// Twelve more buttons a game names for itself, through its controls
    /// JSON (the Delve: bomb, swing, drink, vigil, ultimate, its skills).
    /// No keyboard or pad key sends these unless the game maps one to it.
    pub const X1: u32 = 1 << 20;
    pub const X2: u32 = 1 << 21;
    pub const X3: u32 = 1 << 22;
    pub const X4: u32 = 1 << 23;
    pub const X5: u32 = 1 << 24;
    pub const X6: u32 = 1 << 25;
    pub const X7: u32 = 1 << 26;
    pub const X8: u32 = 1 << 27;
    pub const X9: u32 = 1 << 28;
    pub const X10: u32 = 1 << 29;
    pub const X11: u32 = 1 << 30;
    pub const X12: u32 = 1 << 31;

    /// Every button by its name in a controls JSON: `"KeyW": "UP"`.
    pub const NAMES: [(&str, u32); 32] = [
        ("UP", UP),
        ("DOWN", DOWN),
        ("LEFT", LEFT),
        ("RIGHT", RIGHT),
        ("FIRE", FIRE),
        ("USE", USE),
        ("SLOT1", SLOT1),
        ("SLOT2", SLOT2),
        ("SLOT3", SLOT3),
        ("SLOT4", SLOT4),
        ("JUMP", JUMP),
        ("MENU", MENU),
        ("ALT", ALT),
        ("SHIFT", SHIFT),
        ("MAP", MAP),
        ("OK", OK),
        ("AIM_UP", AIM_UP),
        ("AIM_DOWN", AIM_DOWN),
        ("AIM_LEFT", AIM_LEFT),
        ("AIM_RIGHT", AIM_RIGHT),
        ("X1", X1),
        ("X2", X2),
        ("X3", X3),
        ("X4", X4),
        ("X5", X5),
        ("X6", X6),
        ("X7", X7),
        ("X8", X8),
        ("X9", X9),
        ("X10", X10),
        ("X11", X11),
        ("X12", X12),
    ];

    /// A button's bit by its controls-JSON name.
    pub fn named(name: &str) -> Option<u32> {
        NAMES.iter().find(|(n, _)| *n == name).map(|&(_, bit)| bit)
    }
}

/// One sample of a player's input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hands {
    /// Buttons down at the sample.
    pub held: u32,
    /// Buttons that went down since the previous sample, even if already let
    /// go: a tap shorter than a tick is never lost.
    pub pressed: u32,
    /// The stick, -127..=127 each way. Up is negative y, as on screen.
    /// An analog stick gives where it leans at the sample. The direction
    /// buttons give the corners and edges of every one held at any moment
    /// since the previous sample, so a tap shorter than a tick still walks
    /// that tick. A key let go mid-tick walks the tick out: what a client
    /// drew of the tick ahead (`View::hands`) is then what the server
    /// steps, and nothing drawn is taken back.
    pub x: i8,
    pub y: i8,
    /// The aim stick, for twin-stick games, made the same way as the move
    /// stick: an analog right stick where it leans, else the aim buttons
    /// (`AIM_UP`...) held at any moment since the previous sample.
    pub aim_x: i8,
    pub aim_y: i8,
}

impl Hands {
    /// The stick from the direction buttons, for keyboards and d-pads.
    pub fn stick_from_buttons(held: u32) -> (i8, i8) {
        sticks(held, [button::LEFT, button::RIGHT, button::UP, button::DOWN])
    }

    /// The aim stick from the aim buttons, for keyboards.
    pub fn aim_from_buttons(held: u32) -> (i8, i8) {
        sticks(held, [button::AIM_LEFT, button::AIM_RIGHT, button::AIM_UP, button::AIM_DOWN])
    }
}

/// A stick from four direction buttons: left, right, up, down.
fn sticks(held: u32, [left, right, up, down]: [u32; 4]) -> (i8, i8) {
    let axis = |neg: u32, pos: u32| -> i8 {
        match (held & neg != 0, held & pos != 0) {
            (true, false) => -127,
            (false, true) => 127,
            _ => 0,
        }
    };
    (axis(left, right), axis(up, down))
}

/// Samples raw input events into `Hands`, one sample a tick, keeping every
/// press that happened between two samples.
#[derive(Default)]
pub struct Sampler {
    held: u32,
    pressed: u32,
    /// Held at any moment since the last sample.
    swept: u32,
    stick: Option<(i8, i8)>,
    aim: Option<(i8, i8)>,
}

impl Sampler {
    pub fn down(&mut self, buttons: u32) {
        self.pressed |= buttons & !self.held;
        self.held |= buttons;
        self.swept |= buttons;
    }

    pub fn up(&mut self, buttons: u32) {
        self.held &= !buttons;
    }

    /// Replace the held set wholesale (a gamepad or touch poll), noting
    /// what went down.
    pub fn set_held(&mut self, held: u32) {
        self.pressed |= held & !self.held;
        self.held = held;
        self.swept |= held;
    }

    /// An analog stick, or `None` to take the stick from the direction buttons.
    pub fn stick(&mut self, stick: Option<(i8, i8)>) {
        self.stick = stick;
    }

    /// An analog aim stick (a pad's right stick), or `None` to take the aim
    /// from the aim buttons.
    pub fn aim(&mut self, aim: Option<(i8, i8)>) {
        self.aim = aim;
    }

    /// This tick's sample; presses start over.
    pub fn sample(&mut self) -> Hands {
        let hands = self.peek();
        self.pressed = 0;
        self.swept = self.held;
        hands
    }

    /// The hands now, without taking a sample: the presses stay for the
    /// next one. For drawing, between two command ticks.
    pub fn peek(&self) -> Hands {
        let (x, y) = self.stick.unwrap_or_else(|| Hands::stick_from_buttons(self.swept));
        let (aim_x, aim_y) = self.aim.unwrap_or_else(|| Hands::aim_from_buttons(self.swept));
        Hands { held: self.held, pressed: self.pressed, x, y, aim_x, aim_y }
    }

    /// Everything let go (the tab was hidden, the window lost focus).
    pub fn release_all(&mut self) {
        self.held = 0;
        self.swept = 0;
        self.stick = None;
        self.aim = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_between_two_samples_is_kept_once() {
        let mut s = Sampler::default();
        s.down(button::FIRE);
        s.up(button::FIRE);
        let first = s.sample();
        assert_eq!(first.held, 0);
        assert_eq!(first.pressed, button::FIRE);
        assert_eq!(s.sample().pressed, 0);
    }

    #[test]
    fn a_held_key_presses_once_and_stays_held() {
        let mut s = Sampler::default();
        s.down(button::RIGHT);
        let a = s.sample();
        s.down(button::RIGHT);
        let b = s.sample();
        assert_eq!((a.held, a.pressed, a.x), (button::RIGHT, button::RIGHT, 127));
        assert_eq!((b.held, b.pressed, b.x), (button::RIGHT, 0, 127));
    }

    #[test]
    fn a_direction_let_go_between_samples_walks_that_tick_and_no_more() {
        let mut s = Sampler::default();
        s.down(button::LEFT);
        s.up(button::LEFT);
        assert_eq!(s.peek().x, -127, "drawn ahead");
        let tap = s.sample();
        assert_eq!((tap.held, tap.x), (0, -127), "walked");
        assert_eq!(s.sample().x, 0);
        s.down(button::RIGHT);
        s.sample();
        s.up(button::RIGHT);
        assert_eq!(s.sample().x, 127, "the tick it was let go in");
        assert_eq!(s.sample().x, 0);
        s.down(button::RIGHT);
        s.release_all();
        assert_eq!(s.sample().x, 0, "letting go of everything stops at once");
    }

    #[test]
    fn the_aim_buttons_make_the_aim_stick_and_leave_the_move_stick_alone() {
        let mut s = Sampler::default();
        s.down(button::AIM_LEFT | button::AIM_DOWN);
        let h = s.sample();
        assert_eq!((h.aim_x, h.aim_y, h.x, h.y), (-127, 127, 0, 0));
        // Let go mid-tick, the aim holds through that tick, as the move does.
        s.up(button::AIM_LEFT | button::AIM_DOWN);
        assert_eq!(s.sample().aim_x, -127);
        assert_eq!(s.sample().aim_x, 0);
        // A tap of an aim key shorter than a tick still aims that tick.
        s.down(button::AIM_RIGHT);
        s.up(button::AIM_RIGHT);
        let tap = s.sample();
        assert_eq!((tap.aim_x, tap.aim_y, tap.held), (127, 0, 0));
        assert_eq!(s.sample().aim_x, 0);
    }

    #[test]
    fn an_analog_aim_stick_wins_over_the_aim_buttons_until_let_go() {
        let mut s = Sampler::default();
        s.down(button::AIM_UP);
        s.aim(Some((90, -40)));
        let h = s.sample();
        assert_eq!((h.aim_x, h.aim_y), (90, -40));
        s.aim(None);
        assert_eq!(s.sample().aim_y, -127);
        s.aim(Some((10, 10)));
        s.release_all();
        assert_eq!((s.sample().aim_x, s.sample().aim_y), (0, 0));
    }

    #[test]
    fn every_bit_has_one_name() {
        let mut all = 0u32;
        for (name, bit) in button::NAMES {
            assert_eq!(bit.count_ones(), 1, "{name}");
            assert_eq!(all & bit, 0, "{name} is named twice");
            assert_eq!(button::named(name), Some(bit));
            all |= bit;
        }
        assert_eq!(all, u32::MAX);
        assert_eq!(button::named("X13"), None);
    }

    #[test]
    fn an_analog_stick_wins_over_the_direction_buttons() {
        let mut s = Sampler::default();
        s.down(button::LEFT);
        s.stick(Some((40, -90)));
        let h = s.sample();
        assert_eq!((h.x, h.y), (40, -90));
        s.stick(None);
        assert_eq!(s.sample().x, -127);
    }
}
