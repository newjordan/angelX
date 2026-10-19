//! Lanternward's fixed ground: whole-number math, the five rooms' tiles, and
//! what a body may stand on. Nothing here moves; the session owns what does.
//!
//! Everything is integer. A position is in pixels times `Q` (256), a tile is
//! 16 pixels, an angle is a whole degree (0 east, 90 south, the screen's way
//! round), a direction is a vector of length `Q`.

pub const Q: i32 = 256;
pub const TILE: i32 = 16;
/// One tile in `Q` pixels, a power of two: `x >> TILE_SHIFT` is the column.
pub const TILE_SHIFT: i32 = 12;
pub const ROOMS: usize = 5;

/// The integer square root, rounded down.
pub fn isqrt(n: i64) -> i64 {
    if n < 2 {
        return n.max(0);
    }
    let mut x = 1i64 << (64 - n.leading_zeros()).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// How long `(dx, dy)` is, in its own units.
pub fn dist(dx: i32, dy: i32) -> i32 {
    isqrt(i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy)) as i32
}

/// `(dx, dy)` as a vector of length `Q`; nothing for no length. Short
/// vectors are scaled up first, so `(1, 1)` is as true a direction as
/// `(1000, 1000)`.
pub fn unit(dx: i32, dy: i32) -> (i32, i32) {
    let big = i64::from(dx.unsigned_abs().max(dy.unsigned_abs()));
    if big == 0 {
        return (0, 0);
    }
    let shift = (24 - (64 - big.leading_zeros() as i32)).max(0);
    let (sx, sy) = (i64::from(dx) << shift, i64::from(dy) << shift);
    let l = isqrt(sx * sx + sy * sy).max(1);
    (
        (sx * i64::from(Q) / l) as i32,
        (sy * i64::from(Q) / l) as i32,
    )
}

/// The dot product of two `Q` vectors, in `Q`.
pub fn dot(a: (i32, i32), b: (i32, i32)) -> i32 {
    ((i64::from(a.0) * i64::from(b.0) + i64::from(a.1) * i64::from(b.1)) / i64::from(Q)) as i32
}

/// Sine of a whole-degree angle in `Q` (Bhaskara's rational, good to a
/// fifth of a percent).
pub fn sin_q(deg: i32) -> i32 {
    let d = deg.rem_euclid(360);
    let (d, sign) = if d >= 180 { (d - 180, -1) } else { (d, 1) };
    let p = d * (180 - d);
    sign * (Q * 4 * p + (40500 - p) / 2) / (40500 - p)
}

pub fn cos_q(deg: i32) -> i32 {
    sin_q(deg + 90)
}

/// The `Q` direction of a whole-degree angle.
pub fn dir(deg: i32) -> (i32, i32) {
    (cos_q(deg), sin_q(deg))
}

/// The whole-degree angle of `(dx, dy)`, 0 for no length.
pub fn angle_of(dx: i32, dy: i32) -> i32 {
    let u = unit(dx, dy);
    if u == (0, 0) {
        return 0;
    }
    // The whole degree whose direction lies closest to `u`: of those that
    // point its way, the one with the least sideways error.
    let mut best = (i32::MAX, 0);
    for d in 0..360 {
        let c = dir(d);
        if dot(c, u) <= 0 {
            continue;
        }
        let side = (c.0 * u.1 - c.1 * u.0).abs();
        if side < best.0 {
            best = (side, d);
        }
    }
    best.1
}

/// The turn from angle `a` to angle `b`, -180 to 180.
pub fn turn(a: i32, b: i32) -> i32 {
    (b - a + 180).rem_euclid(360) - 180
}

/// A tile's column or row of a position in `Q` pixels.
pub fn tile_of(v: i32) -> i32 {
    v >> TILE_SHIFT
}

/// The middle of tile `t`, in `Q` pixels.
pub fn middle(t: i32) -> i32 {
    t * TILE * Q + TILE * Q / 2
}

/// One room's picture of itself: its floor plan and whether the Gloam holds
/// it dark. Letters: `#` wall, `.` floor, `=` pit, `V` bars (bolts pass),
/// `@` a seat's start, `C` a resting brazier, `Q` the way on; the rest are
/// things that stand on the floor (`props::Pk::of`).
pub struct Plan {
    pub name: &'static str,
    pub rows: &'static [&'static str],
    pub dark: bool,
}

impl Plan {
    pub fn w(&self) -> i32 {
        self.rows[0].len() as i32
    }
    pub fn h(&self) -> i32 {
        self.rows.len() as i32
    }
    /// The letter at a tile; wall beyond the edge.
    pub fn at(&self, tx: i32, ty: i32) -> u8 {
        if tx < 0 || ty < 0 || ty >= self.h() {
            return b'#';
        }
        self.rows[ty as usize]
            .as_bytes()
            .get(tx as usize)
            .copied()
            .unwrap_or(b'#')
    }
    /// Every tile with this letter, row by row.
    pub fn find(&self, c: u8) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for (y, row) in self.rows.iter().enumerate() {
            for (x, &b) in row.as_bytes().iter().enumerate() {
                if b == c {
                    out.push((x as i32, y as i32));
                }
            }
        }
        out
    }
}

/// Walls and bars stop a body for good; the rest is for the props.
pub fn wall(c: u8) -> bool {
    matches!(c, b'#' | b'V')
}

/// The closet a secret hides, as tiles `(x0, y0, x1, y1)`, drawn black until
/// it is opened.
pub fn closet(room: usize) -> Option<(i32, i32, i32, i32)> {
    match room {
        1 => Some((28, 13, 28, 13)),
        2 => Some((21, 14, 23, 14)),
        3 => Some((30, 12, 30, 12)),
        _ => None,
    }
}

pub fn plan(room: usize) -> &'static Plan {
    &PLANS[room.min(ROOMS - 1)]
}

pub static PLANS: [Plan; ROOMS] = [
    Plan { name: "THE STAIR", rows: STAIR, dark: false },
    Plan { name: "THE LAMP HALL", rows: LAMP, dark: false },
    Plan { name: "THE ARROW GALLERY", rows: GALLERY, dark: false },
    Plan { name: "THE KEY VAULT", rows: VAULT, dark: true },
    Plan { name: "THE WARDEN", rows: WARDEN, dark: false },
];

const STAIR: &[&str] = &[
    "######################",
    "#..............==..\"\"#",
    "#.@.@.@.@.@.@..==..\"\"#",
    "#.C..F.........==....#",
    "#..x....\"\"\"\"...==.o..#",
    "#.......\"\"\"\"...==....#",
    "#..s...........==....#",
    "#.....S........==..c.#",
    "#..\"\"\".........==....#",
    "#..\"\"\".....o...==....#",
    "#..............==....#",
    "#.....o.....S..==..o.#",
    "#..o.........o.==....#",
    "#..............==....#",
    "#..............==....#",
    "#########GG###########",
    "#########QQ###########",
    "######################",
];

const LAMP: &[&str] = &[
    "##############################",
    "#..........#.c.#......########",
    "#.oo.......#111#......#VbV####",
    "#.......S.............##V#####",
    "#....B.....................###",
    "#.......o..................###",
    "#....................B.....###",
    "#..........................###",
    "#.E.....\"\"..............S..GQ#",
    "#.......\"\"........\"\"\"......GQ#",
    "#.................\"\"\"......###",
    "#...s.S....................###",
    "#.............B....S.......###",
    "#.C..............o.......oN2c#",
    "#.@.@.@.@.@.@..............###",
    "##############################",
];

const GALLERY: &[&str] = &[
    "######################################",
    "#..o.............#.................###",
    "#......s......o..#.................GQ#",
    "#.@.@.........Y..#...#.............GQ#",
    "#................#...#.............###",
    "#................#................VK##",
    "#.@.@....X....P..1.................###",
    "#................1.................###",
    "#................1...............T.###",
    "#.@.@..\"\"\"\"......#.....#.....#.....###",
    "#......\"\"\"\"......#.....#.....#.....###",
    "#.C....\"\"\"\".o..o.#..o.....#......T.###",
    "#................#.................###",
    "######################~###############",
    "#####################.c.##############",
    "######################################",
];

const VAULT: &[&str] = &[
    "################################",
    "#..........##.c..##..........###",
    "#..........##....##..........###",
    "#..........###LL###..........###",
    "#..o.....S..........S........###",
    "#............................###",
    "#............................###",
    "#............................###",
    "#..s.B..................B....MQ#",
    "#.............c..............MQ#",
    "#./..........................###",
    "#......S.................S...###",
    "#............................2c#",
    "#.............S.......S......###",
    "#.C.......................o..###",
    "#.@.@.@.@.@.@....o...........###",
    "#............................###",
    "################################",
];

const WARDEN: &[&str] = &[
    "##############################",
    "#............................#",
    "#.o............*...........o.#",
    "#............................#",
    "#............................#",
    "#...B..........W.........B...#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#.......f............f.......#",
    "#............................#",
    "#..........#......#..........#",
    "#............................#",
    "#............................#",
    "#.......f............f.......#",
    "#............................#",
    "#...B....................B...#",
    "#..........#......#..........#",
    "#............................#",
    "#............................#",
    "#............................#",
    "#.o........@.@.@.@.@.@.....o.#",
    "#..............C.............#",
    "##############################",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plans_are_rectangles_walled_in() {
        for (i, p) in PLANS.iter().enumerate() {
            let w = p.rows[0].len();
            assert!(w >= 20 && p.rows.len() >= 14, "room {i} is a room");
            for (y, row) in p.rows.iter().enumerate() {
                assert_eq!(row.len(), w, "room {i} row {y}");
            }
            assert!(p.rows[0].bytes().all(|b| b == b'#'), "room {i} top wall");
            assert!(
                p.rows.last().unwrap().bytes().all(|b| b == b'#'),
                "room {i} bottom wall"
            );
            assert!(
                p.find(b'@').len() >= 6,
                "room {i} has a start for every seat"
            );
        }
    }

    #[test]
    fn the_math_is_whole_numbers_that_agree_with_geometry() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(99), 9);
        assert_eq!(isqrt(100), 10);
        assert_eq!(dist(3 * Q, 4 * Q), 5 * Q);
        assert_eq!(unit(5 * Q, 0), (Q, 0));
        assert_eq!(unit(0, 0), (0, 0));
        for d in (0..360).step_by(5) {
            let (c, s) = dir(d);
            let l = dist(c, s);
            assert!((l - Q).abs() <= 3, "{d}: {l}");
        }
        assert_eq!(sin_q(90), Q);
        assert_eq!(sin_q(270), -Q);
        assert!(sin_q(0).abs() <= 1 && sin_q(180).abs() <= 1);
        for d in (0..360).step_by(7) {
            let (c, s) = dir(d);
            assert!(turn(angle_of(c, s), d).abs() <= 2, "{d}");
        }
        assert_eq!(turn(350, 10), 20);
        assert_eq!(turn(10, 350), -20);
    }
}
