//! Book Hop's small sprites, drawn once and kept: the floating tomes, the
//! owls on the shelves, a hanging lantern, a crown, a coin, the strip's
//! little book, a streak's flame and the arrow over a knight.

use super::super::super::ink::Img;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// How tall a tome is, in pixels (a lane is sixteen).
pub(super) const TOME_H: i32 = 14;

/// A cover's three inks, lit to deep.
#[derive(Clone, Copy)]
pub(super) struct Leather {
    pub lit: char,
    pub base: char,
    pub deep: char,
}

/// The covers a tome may wear: oxblood, moss, slate, plum, tan.
pub(super) const COVERS: [Leather; 5] = [
    Leather { lit: '7', base: 'p', deep: 'B' },
    Leather { lit: 'l', base: 'e', deep: 'F' },
    Leather { lit: 'U', base: 'u', deep: 'S' },
    Leather { lit: '[', base: ')', deep: '(' },
    Leather { lit: 'o', base: 'r', deep: 'b' },
];

/// A book lying on the water, `len` pixels long, seen from above and a
/// little in front: its cover lit along the top with a gilt frame and a
/// crest, the spine at the left bound in gilt bands, the pages' edge
/// cream along the front under the cover's lip, all outlined in black so
/// it stands off the ink. `dip` pulls the whole book a pixel under the
/// water's face (a knight just landed on it): the cover darkens.
pub(super) fn tome(len: i32, cover: usize, dip: bool) -> Arc<Img> {
    type Key = (i32, usize, bool);
    static CACHE: OnceLock<Mutex<HashMap<Key, Arc<Img>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (len, cover % COVERS.len(), dip);
    if let Some(im) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return im;
    }
    let im = Arc::new(make_tome(len, COVERS[key.1], dip));
    if let Ok(mut c) = cache.lock() {
        c.insert(key, im.clone());
    }
    im
}

fn make_tome(w: i32, c: Leather, dip: bool) -> Img {
    let mut im = Img::new(w, TOME_H);
    let (lit, base, deep) = if dip { (c.base, c.deep, c.deep) } else { (c.lit, c.base, c.deep) };
    let row = |im: &mut Img, y: i32, x0: i32, x1: i32, ch: char| {
        for x in x0..x1 {
            im.put(x, y, ch);
        }
    };
    // The outline, then the cover's face: lit on the top row, shaded at its
    // front edge, rounded at the corners.
    row(&mut im, 0, 2, w - 2, 'k');
    im.put(1, 1, 'k');
    im.put(w - 2, 1, 'k');
    row(&mut im, 1, 2, w - 2, lit);
    for y in 2..8 {
        im.put(0, y, 'k');
        im.put(w - 1, y, 'k');
        row(&mut im, y, 1, w - 1, base);
    }
    row(&mut im, 8, 1, w - 1, deep);
    im.put(0, 8, 'k');
    im.put(w - 1, 8, 'k');
    // The spine: deep leather bound with two gilt bands.
    for y in 1..9 {
        im.put(1, y, deep);
        im.put(2, y, if y > 1 && y < 8 { '4' } else { deep });
        im.put(3, y, deep);
    }
    // The cover's frame in worn gilt, and a crest in the middle of it.
    let (fx0, fx1) = (6, w - 4);
    for x in fx0..fx1 {
        im.put(x, 3, 'a');
        im.put(x, 6, 'a');
    }
    for y in 3..7 {
        im.put(fx0, y, 'a');
        im.put(fx1 - 1, y, 'a');
    }
    let cx = w / 2 + 1;
    im.put(cx, 3, '6');
    im.put(cx - 1, 4, '4');
    im.put(cx, 4, '6');
    im.put(cx + 1, 4, '4');
    im.put(cx - 1, 5, '4');
    im.put(cx, 5, '4');
    im.put(cx + 1, 5, '4');
    if w >= 40 {
        // A long book's title: two dashes of gilt either side of the crest.
        for x in (fx0 + 3..cx - 4).step_by(2) {
            im.put(x, 4, 'o');
            im.put(x, 5, 'o');
        }
        for x in (cx + 4..fx1 - 3).step_by(2) {
            im.put(x, 4, 'o');
            im.put(x, 5, 'o');
        }
    }
    // The pages' edge along the front, a little shorter than the cover,
    // ruled with fine lines; the cover's lip under them.
    for y in 9..12 {
        im.put(0, y, 'k');
        im.put(w - 1, y, 'k');
        im.put(1, y, deep);
        im.put(2, y, deep);
        im.put(w - 2, y, deep);
        im.put(w - 3, y, deep);
        for x in 3..w - 3 {
            let ink = match y {
                9 => 'c',
                10 if x % 2 == 0 => '$',
                10 => 'c',
                _ => '$',
            };
            im.put(x, y, ink);
        }
    }
    row(&mut im, 12, 1, w - 1, deep);
    im.put(0, 12, 'k');
    im.put(w - 1, 12, 'k');
    row(&mut im, 13, 2, w - 2, 'k');
    im
}

/// An owl on the top shelf, twelve by thirteen: frame 0 watching, 1 blinking,
/// 2 wings thrown up cheering.
pub(super) fn owl(frame: u32) -> &'static Img {
    static OWLS: OnceLock<[Img; 3]> = OnceLock::new();
    let owls = OWLS.get_or_init(|| {
        let watch = [
            "B..........B",
            "BB.PPPPPP.BB",
            ".PPPPPPPPPP.",
            "PPWWWPPWWWPP",
            "PPW6kPPk6WPP",
            "PPWWWOOWWWPP",
            "PPPPPOOPPPPP",
            "PooPTTTTPooP",
            "PooTTPPTTooP",
            "PoPTTTTTTPoP",
            ".PPTTTTTTPP.",
            ".PPPTTTTPPP.",
            "..OO....OO..",
        ];
        let blink = [
            "B..........B",
            "BB.PPPPPP.BB",
            ".PPPPPPPPPP.",
            "PPPPPPPPPPPP",
            "PPBBBPPBBBPP",
            "PPPPPOOPPPPP",
            "PPPPPOOPPPPP",
            "PooPTTTTPooP",
            "PooTTPPTTooP",
            "PoPTTTTTTPoP",
            ".PPTTTTTTPP.",
            ".PPPTTTTPPP.",
            "..OO....OO..",
        ];
        let cheer = [
            "B..........B",
            "BB.PPPPPP.BB",
            "oPPPPPPPPPPo",
            "oPWWWPPWWWPo",
            "ooW6kPPk6Woo",
            "oPWWWOOWWWPo",
            ".PPPPOOPPPP.",
            "..PPTTTTPP..",
            "..PTTPPTTP..",
            "..PTTTTTTP..",
            "..PTTTTTTP..",
            "...PTTTTP...",
            "..OO....OO..",
        ];
        [Img::from_rows(&watch), Img::from_rows(&blink), Img::from_rows(&cheer)]
    });
    &owls[(frame % 3) as usize]
}

/// A hanging lantern, seven by nine, its chain above: dark iron, a gold
/// ring, and glass lit in `flame` (or cold when `None`).
pub(super) fn lantern(flame: Option<char>, tick: u32) -> Img {
    let (core, glass) = match flame {
        Some(f) if tick / 3 % 2 == 0 => (f, '6'),
        Some(f) => ('6', f),
        None => ('G', 'j'),
    };
    let mut im = Img::from_rows(&[
        "...j...",
        "..jjj..",
        ".gOOOg.",
        "gOGGGOg",
        "gOGGGOg",
        "gOGGGOg",
        ".gOOOg.",
        "..jjj..",
        "...g...",
    ]);
    for y in 3..6 {
        for x in 2..5 {
            im.put(x, y, if y == 4 && x == 3 { core } else { glass });
        }
    }
    // A cold lantern wears paler iron, so it still shows on the dark case.
    if flame.is_none() {
        return im.recolor(&[('g', 'G'), ('j', 'J')]);
    }
    im
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

/// The strip's little closed book, nine by seven.
pub(super) fn strip_book() -> &'static Img {
    static BOOK: OnceLock<Img> = OnceLock::new();
    BOOK.get_or_init(|| {
        Img::from_rows(&[
            ".kkkkkkk.",
            "k4RRRRRRk",
            "k4RRTRRRk",
            "k4RRRRRRk",
            "kccccccck",
            "kc$c$c$ck",
            ".kkkkkkk.",
        ])
    })
}

/// A flame, two frames, three by five (a streak's).
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

/// The arrow over a knight, in `ink` with a black edge, seven by five.
pub(super) fn arrow(ink: char) -> Img {
    let mut im = Img::from_rows(&["ccccccc", ".ccccc.", "..ccc..", "...c..."]);
    for y in 0..4 {
        for x in 0..7 {
            if im.get(x, y).is_some() {
                im.put(x, y, ink);
            }
        }
    }
    let mut out = Img::new(9, 6);
    out.stamp(&im, 1, 1);
    out.outline_outside('k');
    out
}
