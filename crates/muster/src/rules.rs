//! The muster's rules that both halves run: a knight's command, how a
//! knight moves, and the bytes each thing is sent as. The server steps
//! knights with `Knight::step`; the client replays its own unacknowledged
//! commands through the same function, so its prediction is the server's
//! answer whenever the commands arrived as sent. Integer math only.

use angel_net::cmd::Command;
use angel_net::input::{Hands, button};

pub const GAME: &str = "muster";
pub const TICK_HZ: u32 = 20;
pub const SNAPSHOT_HZ: u32 = 20;
/// Positions are in sixteenths of a pixel: slow walking is smooth, and a
/// position fits 16 bits across the field.
pub const SUB: i32 = 16;
/// The field is this many pixels square.
pub const FIELD_PX: i32 = 4096;
/// The field in sub-pixels; a position is 0..FIELD.
pub const FIELD: i32 = FIELD_PX * SUB;
/// Seats: everyone gets a knight.
pub const SEATS: u32 = 256;
/// The client draws this many pixels.
pub const VIEW_W: u32 = 320;
pub const VIEW_H: u32 = 180;

/// Sub-pixels a tick at full stick: 3.5 pixels, 70 a second.
pub const KNIGHT_SPEED: i32 = 56;
/// A stick nearer the middle than this is let go (a thumb resting on glass).
pub const DEAD_ZONE: i32 = 16;
/// A wave lasts this many ticks.
pub const WAVE_TICKS: u8 = 15;
/// A knight lifts its own banner from this close.
pub const REACH: i32 = 24 * SUB;
pub const SHEEP: u32 = 60;

/// Entity ids: a knight is its seat; sheep and banners above.
pub const SHEEP_BASE: u32 = 1000;
pub const BANNER_BASE: u32 = 2000;
pub const KIND_KNIGHT: u16 = 1;
pub const KIND_SHEEP: u16 = 2;
pub const KIND_BANNER: u16 = 3;

/// Interest: the view, plus this much so a thing is held before it walks
/// on screen, plus `KEEP_PX` more before it is let go. Cells of the grid
/// are `CELL_PX` across.
pub const MARGIN_PX: i32 = 32;
pub const KEEP_PX: i32 = 48;
pub const CELL_PX: i32 = 128;

/// Command buttons (the muster's own bits, not `input::button`'s).
pub const WAVE: u8 = 1;
pub const BANNER: u8 = 2;

/// Event kinds: the first byte of each event, then the seat.
pub const EV_WAVE: u8 = 1;
pub const EV_PLANT: u8 = 2;
pub const EV_LIFT: u8 = 3;
/// Then the name.
pub const EV_JOIN: u8 = 4;
pub const EV_LEAVE: u8 = 5;

/// One seat's command for one tick: the stick and two buttons, 4 bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MusterCmd {
    pub x: i8,
    pub y: i8,
    pub held: u8,
    pub pressed: u8,
}

impl Command for MusterCmd {
    fn neutral() -> Self {
        MusterCmd::default()
    }
    fn held_only(&self) -> Self {
        MusterCmd { pressed: 0, ..*self }
    }
    fn absorb(&mut self, older: &Self) {
        self.pressed |= older.pressed;
    }
}

impl MusterCmd {
    pub fn from_hands(hands: &Hands) -> MusterCmd {
        let buttons = |bits: u32| -> u8 {
            (u8::from(bits & button::FIRE != 0) * WAVE) | (u8::from(bits & button::USE != 0) * BANNER)
        };
        MusterCmd { x: hands.x, y: hands.y, held: buttons(hands.held), pressed: buttons(hands.pressed) }
    }

    pub fn encode(&self) -> [u8; 4] {
        [self.x as u8, self.y as u8, self.held, self.pressed]
    }

    pub fn decode(bytes: &[u8]) -> Option<MusterCmd> {
        let &[x, y, held, pressed] = bytes else { return None };
        let known = WAVE | BANNER;
        if held & !known != 0 || pressed & !known != 0 {
            return None;
        }
        Some(MusterCmd { x: x as i8, y: y as i8, held, pressed })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Facing {
    #[default]
    Down = 0,
    Up = 1,
    Left = 2,
    Right = 3,
}

impl Facing {
    fn from_bits(b: u8) -> Facing {
        match b & 3 {
            0 => Facing::Down,
            1 => Facing::Up,
            2 => Facing::Left,
            _ => Facing::Right,
        }
    }
}

/// What a step did besides moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deed {
    Wave,
    Plant,
    Lift,
}

/// A knight as both halves know it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Knight {
    pub x: i32,
    pub y: i32,
    pub facing: Facing,
    pub walking: bool,
    pub carrying: bool,
    /// Ticks of waving left.
    pub wave: u8,
}

impl Knight {
    pub fn new(x: i32, y: i32) -> Knight {
        Knight { x, y, carrying: true, ..Knight::default() }
    }

    /// One tick of `cmd`. `banner` is where this knight's own banner
    /// stands, if planted. At most one deed a tick: a banner before a wave.
    pub fn step(&mut self, cmd: &MusterCmd, banner: Option<(i32, i32)>) -> Option<Deed> {
        let axis = |s: i8| -> i32 {
            let s = i32::from(s);
            if s.abs() < DEAD_ZONE { 0 } else { s * KNIGHT_SPEED / 127 }
        };
        let (mut dx, mut dy) = (axis(cmd.x), axis(cmd.y));
        let len2 = i64::from(dx * dx + dy * dy);
        let max2 = i64::from(KNIGHT_SPEED * KNIGHT_SPEED);
        if len2 > max2 {
            // A diagonal goes no faster than straight.
            let len = len2.isqrt() as i32;
            dx = dx * KNIGHT_SPEED / len;
            dy = dy * KNIGHT_SPEED / len;
        }
        self.x = (self.x + dx).clamp(0, FIELD - 1);
        self.y = (self.y + dy).clamp(0, FIELD - 1);
        self.walking = dx != 0 || dy != 0;
        if self.walking {
            self.facing = if dx.abs() > dy.abs() {
                if dx < 0 { Facing::Left } else { Facing::Right }
            } else if dy < 0 {
                Facing::Up
            } else {
                Facing::Down
            };
        }
        self.wave = self.wave.saturating_sub(1);
        if cmd.pressed & BANNER != 0 {
            if self.carrying {
                self.carrying = false;
                return Some(Deed::Plant);
            }
            if let Some((bx, by)) = banner
                && (bx - self.x).abs() <= REACH
                && (by - self.y).abs() <= REACH
            {
                self.carrying = true;
                return Some(Deed::Lift);
            }
        }
        if cmd.pressed & WAVE != 0 {
            self.wave = WAVE_TICKS;
            return Some(Deed::Wave);
        }
        None
    }

    /// Five bytes: x, y (16 bits each), then facing, walking, carrying and
    /// the wave's ticks left packed in one.
    pub fn state(&self) -> [u8; 5] {
        let [x0, x1] = (self.x as u16).to_le_bytes();
        let [y0, y1] = (self.y as u16).to_le_bytes();
        let pose = self.facing as u8 | u8::from(self.walking) << 2 | u8::from(self.carrying) << 3 | self.wave.min(15) << 4;
        [x0, x1, y0, y1, pose]
    }

    pub fn from_state(b: &[u8]) -> Option<Knight> {
        let &[x0, x1, y0, y1, pose] = b else { return None };
        Some(Knight {
            x: i32::from(u16::from_le_bytes([x0, x1])),
            y: i32::from(u16::from_le_bytes([y0, y1])),
            facing: Facing::from_bits(pose),
            walking: pose & 4 != 0,
            carrying: pose & 8 != 0,
            wave: pose >> 4,
        })
    }
}

/// A sheep or banner's place and pose: x, y, then facing and walking.
pub fn place_state(x: i32, y: i32, facing: Facing, walking: bool) -> [u8; 5] {
    let [x0, x1] = (x as u16).to_le_bytes();
    let [y0, y1] = (y as u16).to_le_bytes();
    [x0, x1, y0, y1, facing as u8 | u8::from(walking) << 2]
}

/// (x, y, facing, walking) from `place_state`'s bytes.
pub fn read_place(b: &[u8]) -> Option<(i32, i32, Facing, bool)> {
    let &[x0, x1, y0, y1, pose] = b else { return None };
    Some((
        i32::from(u16::from_le_bytes([x0, x1])),
        i32::from(u16::from_le_bytes([y0, y1])),
        Facing::from_bits(pose),
        pose & 4 != 0,
    ))
}

/// Where a seat's knight first stands: a sunflower spiral around the
/// muster point, 22 pixels apart, so a crowd starts in one view.
pub fn spawn_point(seat: u32) -> (i32, i32) {
    const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
    const SPACING_PX: f64 = 22.0;
    let r = SPACING_PX * (f64::from(seat) + 0.5).sqrt();
    let a = f64::from(seat) * GOLDEN_ANGLE;
    let centre = f64::from(FIELD / 2);
    ((centre + r * a.cos() * f64::from(SUB)) as i32, (centre + r * a.sin() * f64::from(SUB)) as i32)
}

/// A knight's colour, by seat: 16 that tell apart on a dark field.
pub const PALETTE: [u32; 16] = [
    0xFFC0504D, 0xFF4F81BD, 0xFF9BBB59, 0xFF8064A2, 0xFFF79646, 0xFF4BACC6, 0xFFD4B84A, 0xFFC77CA8, 0xFF7F9FB5,
    0xFFA9714B, 0xFF6FB98F, 0xFFB5525E, 0xFF6474C8, 0xFFC9C36A, 0xFF8FB0D9, 0xFFD98F6A,
];

pub fn colour_of(seat: u32) -> u8 {
    (seat % PALETTE.len() as u32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_reads_back_and_nonsense_does_not() {
        let cmd = MusterCmd { x: -127, y: 40, held: WAVE, pressed: WAVE | BANNER };
        assert_eq!(MusterCmd::decode(&cmd.encode()), Some(cmd));
        assert_eq!(MusterCmd::decode(&[0, 0, 0]), None);
        assert_eq!(MusterCmd::decode(&[0, 0, 0x80, 0]), None);
    }

    #[test]
    fn a_knight_walks_no_faster_on_the_diagonal_and_stays_on_the_field() {
        let mut k = Knight::new(1000, 1000);
        k.step(&MusterCmd { x: 127, y: 0, ..Default::default() }, None);
        assert_eq!((k.x, k.facing, k.walking), (1000 + KNIGHT_SPEED, Facing::Right, true));
        let mut d = Knight::new(1000, 1000);
        d.step(&MusterCmd { x: 127, y: 127, ..Default::default() }, None);
        let moved = (d.x - 1000).pow(2) + (d.y - 1000).pow(2);
        assert!(moved <= KNIGHT_SPEED * KNIGHT_SPEED, "{moved}");
        let mut edge = Knight::new(0, FIELD - 1);
        edge.step(&MusterCmd { x: -127, y: 127, ..Default::default() }, None);
        assert_eq!((edge.x, edge.y), (0, FIELD - 1));
        // A resting thumb does not walk.
        let mut still = Knight::new(5, 5);
        still.step(&MusterCmd { x: 10, y: -12, ..Default::default() }, None);
        assert!(!still.walking);
    }

    #[test]
    fn the_banner_is_planted_then_lifted_only_within_reach() {
        let mut k = Knight::new(5000, 5000);
        let press = MusterCmd { pressed: BANNER, ..Default::default() };
        assert_eq!(k.step(&press, None), Some(Deed::Plant));
        assert!(!k.carrying);
        assert_eq!(k.step(&press, Some((5000 + REACH + 1, 5000))), None);
        assert_eq!(k.step(&press, Some((5000 + REACH, 5000))), Some(Deed::Lift));
        assert!(k.carrying);
    }

    #[test]
    fn a_state_reads_back() {
        let k = Knight { x: 65_535, y: 7, facing: Facing::Left, walking: true, carrying: true, wave: 9 };
        assert_eq!(Knight::from_state(&k.state()), Some(k));
        assert_eq!(read_place(&place_state(3, 4, Facing::Up, true)), Some((3, 4, Facing::Up, true)));
    }

    #[test]
    fn spawn_points_are_on_the_field_and_apart() {
        let points: Vec<_> = (0..SEATS).map(spawn_point).collect();
        for (i, a) in points.iter().enumerate() {
            assert!((0..FIELD).contains(&a.0) && (0..FIELD).contains(&a.1));
            for b in &points[i + 1..] {
                let d2 = i64::from(a.0 - b.0).pow(2) + i64::from(a.1 - b.1).pow(2);
                assert!(d2 > i64::from(10 * SUB).pow(2), "{a:?} {b:?}");
            }
        }
    }
}
