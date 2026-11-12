//! The store's small sprites, drawn once and kept: the powder keg (cool,
//! warm and white-hot), the strip's little keg, a crown, a coin, a flame,
//! the pictures of the title's two plain words.

use super::super::super::ink::Img;
use std::sync::OnceLock;

/// The keg, twelve square: a stave barrel with two iron hoops, lit from the
/// upper left. `heat` 0 is cold wood, 1 warm, 2 red-hot, 3 white.
pub(super) fn keg(heat: u8) -> &'static Img {
    static KEGS: OnceLock<[Img; 4]> = OnceLock::new();
    let kegs = KEGS.get_or_init(|| {
        let cold = Img::from_rows(&[
            "...kkkkkk...",
            "..koOOOOok..",
            ".kOTtOOooRk.",
            ".kRooooooRk.",
            "kjhhhhhhhhgk",
            "koRRRrrrrppk",
            "kRoRRrrrpppk",
            "kRRRRrrrpppk",
            "kjhhhhhhhhgk",
            ".kRRrrrrppk.",
            ".kpppppppbk.",
            "..kkkkkkkk..",
        ]);
        let warm = cold.recolor(&[('R', '@'), ('r', 'R'), ('p', '7'), ('o', '6'), ('O', '6'), ('t', '9')]);
        let hot = cold.recolor(&[('R', '6'), ('r', '@'), ('p', 'R'), ('o', '9'), ('O', '9'), ('t', '9'), ('b', '7'), ('h', '@'), ('g', '7')]);
        let white = cold.recolor(&[
            ('R', '9'), ('r', '9'), ('p', '6'), ('o', '9'), ('O', '9'), ('t', '9'), ('b', '6'), ('h', 'w'), ('g', '6'), ('j', '6'), ('k', '@'),
        ]);
        [cold, warm, hot, white]
    });
    &kegs[usize::from(heat.min(3))]
}

/// The strip's keg, eight square.
pub(super) fn strip_keg() -> &'static Img {
    static KEG: OnceLock<Img> = OnceLock::new();
    KEG.get_or_init(|| {
        Img::from_rows(&[".kkkkkk.", "koOOOOok", "kRRRRRpk", "kjhhhhgk", "kRRrrrpk", "kjhhhhgk", "kpppppbk", ".kkkkkk."])
    })
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

/// A knight sooted, as a dark silhouette in the shape of any sprite: every
/// pixel of it soot black, the edge a light grey so it reads on stone.
pub(super) fn sooted(src: &Img) -> Img {
    let mut out = Img::new(src.w, src.h);
    for y in 0..src.h {
        for x in 0..src.w {
            if src.get(x, y).is_none() {
                continue;
            }
            let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| src.get(x + dx, y + dy).is_none());
            out.put(x, y, if edge { 'h' } else if (x + y) % 2 == 0 { 'k' } else { 'K' });
        }
    }
    out
}
