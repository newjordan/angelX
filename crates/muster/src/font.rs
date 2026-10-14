//! A 3×5 pixel font for name tags and the count: capitals, digits and a
//! little punctuation; small letters draw as capitals.

/// A glyph's five rows of three bits, top row first, left pixel highest.
const fn glyph(rows: [u8; 5]) -> u16 {
    ((rows[0] as u16) << 12) | ((rows[1] as u16) << 9) | ((rows[2] as u16) << 6) | ((rows[3] as u16) << 3) | rows[4] as u16
}

const LETTERS: [u16; 26] = [
    glyph([0b010, 0b101, 0b111, 0b101, 0b101]),
    glyph([0b110, 0b101, 0b110, 0b101, 0b110]),
    glyph([0b011, 0b100, 0b100, 0b100, 0b011]),
    glyph([0b110, 0b101, 0b101, 0b101, 0b110]),
    glyph([0b111, 0b100, 0b110, 0b100, 0b111]),
    glyph([0b111, 0b100, 0b110, 0b100, 0b100]),
    glyph([0b011, 0b100, 0b101, 0b101, 0b011]),
    glyph([0b101, 0b101, 0b111, 0b101, 0b101]),
    glyph([0b111, 0b010, 0b010, 0b010, 0b111]),
    glyph([0b001, 0b001, 0b001, 0b101, 0b010]),
    glyph([0b101, 0b101, 0b110, 0b101, 0b101]),
    glyph([0b100, 0b100, 0b100, 0b100, 0b111]),
    glyph([0b101, 0b111, 0b111, 0b101, 0b101]),
    glyph([0b110, 0b101, 0b101, 0b101, 0b101]),
    glyph([0b010, 0b101, 0b101, 0b101, 0b010]),
    glyph([0b110, 0b101, 0b110, 0b100, 0b100]),
    glyph([0b010, 0b101, 0b101, 0b110, 0b011]),
    glyph([0b110, 0b101, 0b110, 0b101, 0b101]),
    glyph([0b011, 0b100, 0b010, 0b001, 0b110]),
    glyph([0b111, 0b010, 0b010, 0b010, 0b010]),
    glyph([0b101, 0b101, 0b101, 0b101, 0b111]),
    glyph([0b101, 0b101, 0b101, 0b101, 0b010]),
    glyph([0b101, 0b101, 0b111, 0b111, 0b101]),
    glyph([0b101, 0b101, 0b010, 0b101, 0b101]),
    glyph([0b101, 0b101, 0b010, 0b010, 0b010]),
    glyph([0b111, 0b001, 0b010, 0b100, 0b111]),
];

const DIGITS: [u16; 10] = [
    glyph([0b111, 0b101, 0b101, 0b101, 0b111]),
    glyph([0b010, 0b110, 0b010, 0b010, 0b111]),
    glyph([0b110, 0b001, 0b010, 0b100, 0b111]),
    glyph([0b110, 0b001, 0b010, 0b001, 0b110]),
    glyph([0b101, 0b101, 0b111, 0b001, 0b001]),
    glyph([0b111, 0b100, 0b110, 0b001, 0b110]),
    glyph([0b011, 0b100, 0b110, 0b101, 0b010]),
    glyph([0b111, 0b001, 0b010, 0b010, 0b010]),
    glyph([0b010, 0b101, 0b010, 0b101, 0b010]),
    glyph([0b010, 0b101, 0b011, 0b001, 0b110]),
];

const UNKNOWN: u16 = glyph([0b110, 0b001, 0b010, 0b000, 0b010]);

pub fn glyph_of(c: char) -> u16 {
    match c.to_ascii_uppercase() {
        c @ 'A'..='Z' => LETTERS[(c as u8 - b'A') as usize],
        c @ '0'..='9' => DIGITS[(c as u8 - b'0') as usize],
        ' ' => 0,
        '-' => glyph([0, 0, 0b111, 0, 0]),
        '.' => glyph([0, 0, 0, 0, 0b010]),
        '!' => glyph([0b010, 0b010, 0b010, 0, 0b010]),
        ':' => glyph([0, 0b010, 0, 0b010, 0]),
        '\'' => glyph([0b010, 0b010, 0, 0, 0]),
        '/' => glyph([0b001, 0b001, 0b010, 0b100, 0b100]),
        '_' => glyph([0, 0, 0, 0, 0b111]),
        _ => UNKNOWN,
    }
}

/// Glyphs are 3 wide with a column between.
pub const ADVANCE: i32 = 4;
pub const HEIGHT: i32 = 5;

/// The width of `text` in pixels.
pub fn width(text: &str) -> i32 {
    (text.chars().count() as i32 * ADVANCE - 1).max(0)
}
