//! The belfry's small sprites, drawn once and kept: the great bell, the
//! strip's little bell, a crown, a coin, a roosting bat, a rope's end.

use super::super::super::ink::Img;
use std::sync::OnceLock;

/// The great bell, `GREAT_W` by `GREAT_H`, bronze lit from the upper left:
/// a crown with its yoke, a long shoulder flaring to a thick lip, two bands
/// cast round it, a dark mouth and the clapper's ball hanging in it. Its
/// pivot is the middle of the top row.
pub(super) const GREAT_W: i32 = 30;
pub(super) const GREAT_H: i32 = 34;

pub(super) fn great() -> &'static Img {
    static BELL: OnceLock<Img> = OnceLock::new();
    BELL.get_or_init(|| {
        let mut im = Img::new(GREAT_W, GREAT_H);
        let cx = GREAT_W as f32 / 2.0 - 0.5;
        let body = GREAT_H - 8;
        // The half-width at each row: a round crown, sides widening slowly,
        // then the waist flaring out to the lip.
        let half = |y: i32| -> f32 {
            if y < 3 {
                return 2.5 + y as f32 * 0.6;
            }
            let t = ((y - 3) as f32 / (body - 3) as f32).clamp(0.0, 1.0);
            let flare = ((t - 0.58) / 0.42).clamp(0.0, 1.0);
            let flare = flare * flare * (3.0 - 2.0 * flare);
            (4.4 + 3.2 * t + 7.0 * flare).min(14.4)
        };
        for y in 0..body {
            let w = half(y);
            for x in 0..GREAT_W {
                let d = x as f32 - cx;
                if d.abs() > w {
                    continue;
                }
                // Lit from the upper left: a bright band down the left, the
                // right in shade, the very edge dark.
                let u = d / w;
                let ink = if u < -0.8 {
                    'p'
                } else if u < -0.55 {
                    't'
                } else if u < -0.25 {
                    'O'
                } else if u < 0.15 {
                    'o'
                } else if u < 0.5 {
                    'r'
                } else if u < 0.8 {
                    'R'
                } else {
                    'p'
                };
                im.put(x, y, ink);
            }
        }
        // Bands cast round it, each a lit line over a dark one.
        for by in [9, 13, 19] {
            let w = half(by);
            for x in 0..GREAT_W {
                let d = x as f32 - cx;
                if d.abs() <= w {
                    im.put(x, by, if d < -w * 0.25 { 'T' } else if d < w * 0.4 { 'O' } else { 'o' });
                    im.put(x, by + 1, if d.abs() > w * 0.8 { 'p' } else { 'b' });
                }
            }
        }
        // The lip: a thick rim rolled out, lit on top, a dark under-curve.
        for y in body..body + 4 {
            let w = half(body - 1) + 0.6 + (y - body) as f32 * 0.1;
            for x in 0..GREAT_W {
                let d = x as f32 - cx;
                if d.abs() <= w {
                    let ink = if y == body { 'T' } else if y == body + 3 { 'p' } else if d < -w * 0.45 { 'O' } else if d < w * 0.35 { 'o' } else { 'r' };
                    im.put(x, y, ink);
                }
            }
        }
        // The mouth: dark, with the clapper's ball in it.
        let mw = half(body - 1) - 0.4;
        for x in 0..GREAT_W {
            let d = x as f32 - cx;
            if d.abs() <= mw {
                im.put(x, body + 4, 'n');
                if d.abs() <= mw - 1.5 {
                    im.put(x, body + 5, 'k');
                }
            }
        }
        im.rect(GREAT_W / 2 - 2, body + 3, 4, 4, 't');
        im.rect(GREAT_W / 2 - 1, body + 3, 2, 1, 'T');
        im.rect(GREAT_W / 2 - 2, body + 6, 4, 1, 'p');
        // The yoke at the crown: an iron bar, its loop and bolts.
        im.rect(GREAT_W / 2 - 6, 0, 12, 2, 'g');
        im.rect(GREAT_W / 2 - 6, 0, 12, 1, 'J');
        im.put(GREAT_W / 2 - 5, 1, 'h');
        im.put(GREAT_W / 2 + 4, 1, 'h');
        im.rect(GREAT_W / 2 - 1, 0, 2, 4, 'j');
        im
    })
}

/// The strip's little bell, nine by nine, two frames: still, and swung (its
/// lip kicked a pixel to one side).
pub(super) fn strip(swung: bool) -> &'static Img {
    static BELLS: OnceLock<[Img; 2]> = OnceLock::new();
    let bells = BELLS.get_or_init(|| {
        let rows = [
            "...rRr...",
            "..oOOOr..",
            "..oOtOr..",
            ".oOtOOOr.",
            ".oOtOOOr.",
            ".oOOOOOr.",
            "oOOOOOOOr",
            "RTTTTTTTR",
            "...TtT...",
        ];
        // Swung: the body from its waist down shifts a pixel right.
        let swung_rows: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(y, r)| if y >= 5 { format!(".{}", &r[..8]) } else { (*r).to_string() })
            .collect();
        let refs: Vec<&str> = swung_rows.iter().map(String::as_str).collect();
        [Img::from_rows(&rows), Img::from_rows(&refs)]
    });
    &bells[usize::from(swung)]
}

/// The leader's crown, seven by four.
pub(super) fn crown() -> &'static Img {
    static CROWN: OnceLock<Img> = OnceLock::new();
    CROWN.get_or_init(|| Img::from_rows(&["T.T.T.T", "TtTtTtT", "tOOOOOt", ".nnnnn."]))
}

/// A gold coin, five square, for the pot.
pub(super) fn coin() -> &'static Img {
    static COIN: OnceLock<Img> = OnceLock::new();
    COIN.get_or_init(|| Img::from_rows(&[".444.", "46TO4", "4TOa4", "4OaO4", ".444."]))
}

/// A bat hanging by its feet from a rafter, folded: five by five. The
/// second frame has it shuffling.
pub(super) fn roost(frame: u32) -> &'static Img {
    static ROOSTS: OnceLock<[Img; 2]> = OnceLock::new();
    let r = ROOSTS.get_or_init(|| {
        [
            Img::from_rows(&[".n.n.", "nnnnn", "nbnbn", ".n7n.", "..n.."]),
            Img::from_rows(&["n.n.n", ".nnn.", "nbnbn", ".n7n.", "..n.."]),
        ]
    });
    &r[(frame % 2) as usize]
}

/// A flame, two frames, three by five (the lanterns').
pub(super) fn flame(frame: u32) -> &'static Img {
    static FLAMES: OnceLock<[Img; 2]> = OnceLock::new();
    let f = FLAMES.get_or_init(|| {
        [
            Img::from_rows(&[".6.", "656", "6@6", "6@6", ".R."]),
            Img::from_rows(&["..6", ".66", "6@6", "6@6", ".R."]),
        ]
    });
    &f[(frame % 2) as usize]
}

/// A bat of the loft's crowd, hanging by its feet in black against the
/// night, a red eye and a lit wing-edge: seven by six; the second frame has
/// its wings thrown out.
pub(super) fn crowd(wild: bool) -> &'static Img {
    static CROWD: OnceLock<[Img; 2]> = OnceLock::new();
    let c = CROWD.get_or_init(|| {
        [
            Img::from_rows(&[".k...k.", ".kk.kk.", "gkkkkkg", ".kk7kk.", "..kkk..", "...k..."]),
            Img::from_rows(&["k.....k", "kk...kk", "kkkkkkk", ".kk7kk.", "..kkk..", "...k..."]),
        ]
    });
    &c[usize::from(wild)]
}
