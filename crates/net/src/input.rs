//! What a player's hands are doing, the same on every device: a keyboard, a
//! gamepad and a touch screen all fill one `Hands`. A game turns it into its
//! own command once a tick.

pub mod button {
    pub const UP: u32 = 1 << 0;
    pub const DOWN: u32 = 1 << 1;
    pub const LEFT: u32 = 1 << 2;
    pub const RIGHT: u32 = 1 << 3;
    pub const FIRE: u32 = 1 << 4;
    pub const USE: u32 = 1 << 5;
    pub const SLOT1: u32 = 1 << 6;
    pub const SLOT2: u32 = 1 << 7;
    pub const SLOT3: u32 = 1 << 8;
    pub const SLOT4: u32 = 1 << 9;
    pub const JUMP: u32 = 1 << 10;
    pub const MENU: u32 = 1 << 11;
    pub const ALT: u32 = 1 << 12;
    pub const SHIFT: u32 = 1 << 13;
    pub const MAP: u32 = 1 << 14;
    pub const OK: u32 = 1 << 15;
    pub const AIM_UP: u32 = 1 << 16;
    pub const AIM_DOWN: u32 = 1 << 17;
    pub const AIM_LEFT: u32 = 1 << 18;
    pub const AIM_RIGHT: u32 = 1 << 19;
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

    pub fn named(name: &str) -> Option<u32> {
        if name == "TEAM" { return Some(OK | SHIFT); }
        NAMES.iter().find(|(n, _)| *n == name).map(|&(_, bit)| bit)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hands {
    pub held: u32,
    pub pressed: u32,
    pub x: i8,
    pub y: i8,
    pub aim_x: i8,
    pub aim_y: i8,
}

impl Hands {
    pub fn stick_from_buttons(held: u32) -> (i8, i8) {
        sticks(held, [button::LEFT, button::RIGHT, button::UP, button::DOWN])
    }

    pub fn aim_from_buttons(held: u32) -> (i8, i8) {
        sticks(held, [button::AIM_LEFT, button::AIM_RIGHT, button::AIM_UP, button::AIM_DOWN])
    }
}

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

#[derive(Default)]
pub struct Sampler {
    held: u32,
    pressed: u32,
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

    pub fn set_held(&mut self, held: u32) {
        self.pressed |= held & !self.held;
        self.held = held;
        self.swept |= held;
    }

    pub fn stick(&mut self, stick: Option<(i8, i8)>) {
        self.stick = stick;
    }

    pub fn aim(&mut self, aim: Option<(i8, i8)>) {
        self.aim = aim;
    }

    pub fn sample(&mut self) -> Hands {
        let hands = self.peek();
        self.pressed = 0;
        self.swept = self.held;
        hands
    }

    pub fn peek(&self) -> Hands {
        let (x, y) = self.stick.unwrap_or_else(|| Hands::stick_from_buttons(self.swept));
        let (aim_x, aim_y) = self.aim.unwrap_or_else(|| Hands::aim_from_buttons(self.swept));
        Hands { held: self.held, pressed: self.pressed, x, y, aim_x, aim_y }
    }

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
        s.up(button::AIM_LEFT | button::AIM_DOWN);
        assert_eq!(s.sample().aim_x, -127);
        assert_eq!(s.sample().aim_x, 0);
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
