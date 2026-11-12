//! Timbered adit, 26 by 34. Limestone rows; the cart ore is swapped per region.

pub(crate) const ADIT_W: i32 = 26;
pub(crate) const ADIT_H: i32 = 34;
/// First row of the black tunnel. The lintel is the rows above it.
pub(crate) const MOUTH_ROW: i32 = 4;
/// First row of the floor rails and the ore cart.
pub(crate) const CART_ROW: i32 = 22;

pub(crate) const BASE: [&str; 34] = [
    "kkkkkkkkkkkkkkkkkkkkkkkkkk",
    "kOOOOOOOOOOOOOOOOOOOOOOOOk",
    "kRRRBRRRRRRRBRRRRRRRBRRRRk",
    "kkkkkkkkkkkkkkkkkkkkkkkkkk",
    ".kRrBkkkkkkkkkkkkJkkrBbk..",
    ".kRrBkkkkkkkkkkkkJkkrBbk..",
    ".kRrBkkkkkkkkkkkkOkkrBbk..",
    ".kRrBkkkkKKKKKKKt4tkrBbk..",
    ".kRBBkkkkKKXKKXKkakkrBbk..",
    ".kRrBkkkkKKXKKXKKkkkrBbk..",
    ".kRrBkkkkKKXKKXKKkkkrBbk..",
    ".kRrBkkkkKKXKKXKKkkkrBbk..",
    ".kRrBkkkkKgKKKKgKkkkrBbk..",
    ".kRrBkkkkkgkkkkgkkkkrBbk..",
    ".kRrBkkkkkgkkkkgkkkkrBbk..",
    ".kRBBkkkkkgkkkkgkkkkrBbk..",
    ".kRrBkkkkkjkkkkjkkkkrBbk..",
    ".kRrBkkkkkjkkkkjkkkkrBbk..",
    ".kRrBkkkkjkkkkkkjkkkrBbk..",
    ".kRrBkkkkGkkkkkkGkkkrBbk..",
    ".kRrBkkkkGkkkkkkGkkkrBbk..",
    ".kRrBkkkkGkkkkkkGkkkrBbk..",
    ".kkkkk....J....J...kkkkk..",
    ".........BJBBBBJB.........",
    "..........V.v.VV..........",
    ".......kkVvVkVvVvkk.......",
    ".......kJJJJJJJJJJk.......",
    ".......kGGGGGGGGGGk.......",
    ".......kGGGGGGGGGGk.......",
    ".......kjjjjjjjjjjk.......",
    ".......kkkkkkkkkkkk.......",
    ".......kkJkkkkkkJkk.......",
    ".........kJ....Jk.........",
    ".........BJBBBBJB.........",
];

/// Cart ore that is not limestone: (row, col) and the replacement ink.
/// Limestone keeps the sheet (`V` highlight, `v` main).
pub(crate) fn cart_ink(region: u8, row: usize, col: usize, ch: char) -> char {
    const ORE: [(usize, usize); 11] = [
        (24, 10), (24, 12), (24, 14), (24, 15),
        (25, 9), (25, 10), (25, 11), (25, 13),
        (25, 14), (25, 15), (25, 16),
    ];
    if region == 0 || !ORE.contains(&(row, col)) {
        return ch;
    }
    let hi = matches!(ch, 'V');
    match region {
        1 => if hi { 'g' } else { 'X' },
        2 => if hi { '4' } else { 'a' },
        3 => if hi { 'U' } else { 'u' },
        _ => ch,
    }
}
