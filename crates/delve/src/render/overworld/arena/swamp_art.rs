//! Original ink-row pictures for the Swamp of Horrors.
//! Cool mud, rusted bronze and pale living things on black paper. Creature
//! rows are raw: the arena supplies materials, facing and elite stature.

use super::super::ink::{Img, hash, vnoise};
use super::{RockKind, Style};

/// All swamp pictures are authored here rather than borrowed from the realm.
pub const PLACEHOLDER: bool = false;

pub(super) const STYLE: Style = Style {
    rock: RockKind::Slate,
    pool: ('e', 'P'),
};

/// Silted masonry, from wet shadow to the lichen on its upper lip.
pub const WALL_RAMP: &[char] = &['s', 'D', 'F', 'e', 'N', 'm', 'j'];

/// Broken roots, sedge and pebbles leave most of the dry causeway as paper.
pub fn ground(x: i32, y: i32) -> Option<char> {
    let (cx, cy) = (x.div_euclid(9), y.div_euclid(7));
    let h = hash(cx, cy, 831);
    let (px, py) = (x.rem_euclid(9), y.rem_euclid(7));
    match h % 4 {
        0 if py == 4 && (2..=6).contains(&px) => Some(if px < 4 { 'P' } else { 'b' }),
        0 if (px, py) == (3, 3) || (px, py) == (6, 5) => Some('I'),
        1 if (px, py) == (3, 3) || (px, py) == (4, 4) => Some('E'),
        1 if (px, py) == (5, 2) || (px, py) == (5, 3) => Some('N'),
        1 if py == 5 && (3..=6).contains(&px) => Some('D'),
        2 if py == 3 && (3..=5).contains(&px) => Some(if px == 3 { 'j' } else { 'S' }),
        2 if py == 4 && (2..=5).contains(&px) => Some('x'),
        _ if hash(x, y, 834) % 23 == 0 => Some('F'),
        _ => None,
    }
}

/// Thick olive silt, interlocking scum rafts and slow popping bubbles. The
/// broad stationary islands distinguish it from the blue, flowing deep water.
pub fn bog_field(
    cv: &mut Img,
    rect: (i32, i32, i32, i32),
    bog: &dyn Fn(i32, i32) -> bool,
    step: u32,
) {
    let (x0, y0, w, h) = rect;
    for y in y0.max(0)..(y0 + h).min(cv.h) {
        for x in x0.max(0)..(x0 + w).min(cv.w) {
            if !bog(x, y) {
                continue;
            }
            let n = vnoise(x as f32 / 13.0, y as f32 / 7.0, 838);
            let grain = hash(x / 3, y, 839) % 9;
            let c = if n > 0.57 {
                Some(if grain < 2 {
                    'N'
                } else if grain < 5 {
                    'F'
                } else {
                    'D'
                })
            } else if n > 0.34 {
                Some(if grain < 3 { 'I' } else { 'f' })
            } else if y.rem_euclid(5) == 1
                && grain < 5
                && hash(x.div_euclid(8), y.div_euclid(5), 841) % 3 != 0
            {
                Some('s')
            } else {
                None
            };
            if let Some(c) = c {
                cv.put(x, y, c);
            }
            // A bubble belongs to a stationary 19x13 silt pocket; only its
            // tiny rim moves, so the mire never becomes a swimming texture.
            let (bx, by) = (x.div_euclid(19), y.div_euclid(13));
            let h = hash(bx, by, 840);
            let (px, py) = (
                x.rem_euclid(19) - 5 - (h % 8) as i32,
                y.rem_euclid(13) - 4 - (h / 8 % 5) as i32,
            );
            let phase = (step / 2 + h % 7) % 7;
            if phase < 3 && ((py == 0 && px.abs() == 2) || (py.abs() == 1 && px.abs() < 2)) {
                cv.put(x, y, if py < 0 { 'm' } else { 'e' });
            } else if phase == 3 && py == 1 && px.abs() <= 3 {
                cv.put(x, y, 'P');
            }
        }
    }
}

/// Three silhouettes: a fanned sedge tussock, copper bulrushes, and a
/// lightning-broken willow stump. All roots end inside their own tile.
pub fn reeds(cv: &mut Img, x: i32, y: i32, v: u32) {
    let rows: &[&str] = match v % 3 {
        0 => &[
            "........................",
            "........................",
            "........................",
            "...........E............",
            ".......F...El...........",
            ".......EF..El....E......",
            "...F....EF.El...El......",
            "....EF..EF.El..El.......",
            ".....EF..ElEl..El...F...",
            "..F..FEl.ElEl.El...El...",
            "...EF.FElElEl.El..El....",
            "....EF.FElEElEl..El.....",
            ".....EF.ElEElEl.El......",
            "......EFElEElElEl.......",
            "...F...ElEmElElEl...F...",
            "....EF..EmmElElEl..EF...",
            ".....EF.ElmmlElEl.EF....",
            "......EFElmmEElElEF.....",
            ".......FElmmEElEEF......",
            ".....FFElNmmNEElEF......",
            "....FElNNNmNNNEElEF.....",
            "...fFNNmNmNNNFEEEFf.....",
            "..fDFNNNNNFNNFFEDFf.....",
            "..nnDFFFFnFFFFFDnnn.....",
            "...nbIInnnnIInnnn.......",
            "....nnIInnIInnn.........",
            "......nnnnnnn...........",
            "........................",
            "........................",
            "........................",
        ],
        1 => &[
            "........................",
            "............n...........",
            ".......nn..nPn..........",
            "......nPPn.nrn..........",
            "......nron.nPn....nn....",
            "......nPIn.nIn...nPPn...",
            "......nIn..nIn...nron...",
            ".......E....E....nPIn...",
            "...E...El...El...nIn....",
            "...El..El...El....E.....",
            "....El.El...El....El....",
            "....El.El...El..F.El....",
            ".....ElEl...El.EF.El....",
            "..F..ElEl..El.EF..El....",
            "...EF.ElEl.ElEF..El.....",
            "....EFElEl.ElEF..El.....",
            ".....EFElElElEF.El......",
            "......FElElElEFEl.......",
            "....FFElEmmElEElEF......",
            "...fFElNNmmNEEElEFf.....",
            "..fDFNNmNNmNNFEEFf......",
            "..nnDFFNNFFNNFFDnnn.....",
            "...nIPnnnFFFFnIPnn......",
            "....nnnnIIInnnnn........",
            "......nnnnnnn...........",
            "........................",
            "........................",
            "........................",
            "........................",
            "........................",
        ],
        _ => &[
            "........................",
            "........n...............",
            ".......nPn......n.......",
            ".......nPn.....nIn......",
            "...n...nIIn...nIPn......",
            "..nPn..nIPn..nIPn.......",
            "..nIPn..nIPnnIPn........",
            "...nIPn.nIIPIPn.........",
            "....nIPnnIPPPn....n.....",
            ".....nIPnIPPPn...nPn....",
            "......nIIPPPPnnnnIPn....",
            ".......nIPoPPPPPIPn.....",
            ".......nIPPPIPPPnn......",
            ".......nIPPPnnnn........",
            "......nIPoPPn...........",
            "......nIPPPIn...........",
            "......nIPPPIn...........",
            ".....nIPoPPIPn..........",
            ".....nIPPPPIIn..........",
            "....nIPPPnPIIIn.........",
            "...nIPoPn.nPIIIn........",
            "..nIPPPnF..nPIIIn.......",
            ".nIPnnnFFN..nnIIPnn.....",
            ".nnnnFNNNEFFFnnIIPnn....",
            "..nnnDFFnFDFDnnnnnn.....",
            "....nnnnnnnnnnnn........",
            ".......nnnnnn...........",
            "........................",
            "........................",
            "........................",
        ],
    };
    cv.stamp(&Img::from_rows(rows), x - 4, y - 14);
}

/// Thin broken ribbons. A whole ribbon drifts one pixel every sixteen ticks.
pub fn mist(x: i32, y: i32, tick: u32) -> Option<char> {
    let drift = (tick / 16 % 512) as i32;
    let xx = x + drift;
    let band = y + (vnoise(xx as f32 / 24.0, y.div_euclid(11) as f32, 846) * 5.0) as i32;
    if band.rem_euclid(11) == 0 && hash(xx.div_euclid(7), y.div_euclid(11), 847) % 5 < 3 {
        Some(if xx.rem_euclid(7) < 2 { 'S' } else { 'x' })
    } else {
        None
    }
}

// ─── Horrors: authored east-facing ink rows ────────────────────────────────

pub fn bog_hand(stage: u8, _t: u32, tick: u32) -> Img {
    let f = tick / 8 % 2;
    let mut im = Img::new(24, 24);
    let mud = Img::from_rows(&[
        ".......FFFFF............",
        "....FFNNmNNNFFF.........",
        "..FNNFDDFFNNNFDF........",
        ".FNFxxs...xDFNFDF.......",
        "..FxSxxFFFDxxFNF........",
        "...FFFFNNNNFFFF.........",
        "......DDFFFF............",
    ]);
    im.stamp(&mud, f as i32, 17);
    if stage == 0 {
        im.stamp(
            &Img::from_rows(&[
                "....DFFNNFFD...",
                "..DFNmNNNNFFD..",
                ".DFNNmNNFFFFFD.",
                "FNFDDDxxxDDFFF.",
                ".DFFFFNNFFFFD..",
            ]),
            4 - f as i32,
            15,
        );
    } else if stage == 1 {
        // Four nails are above the mud for every tick of the warning.
        im.stamp(
            &Img::from_rows(&[
                ".......khk.........",
                "...kGk.kihk........",
                "...kikkihGk..kGk...",
                "kGkkihkihGk..kihk..",
                "kihkihkihGk..kiGk..",
                "kiGkiGkiGjk..khGk..",
                "kGjkjjkGjkk..kjjk..",
                ".kEEkEEkEFk..kEFk..",
                "..FNNmNFFNNFFNFF...",
            ]),
            3,
            11 - f as i32,
        );
    } else {
        let palm: &[&str] = if f == 0 {
            &[
                "............kkk.........",
                ".......kk..khiGk........",
                "......khiK.kijGk..kkk...",
                "...kk.kihK.kijGk.khiGk..",
                "..khiKkihK.khjGk.kijGk..",
                "..kiGKkihK.khjGk.khjGk..",
                "..kiGKkhGK.khjGk.khjK...",
                "..khGKkhGK.khGjk.khGK...",
                "..khGkkhGkkhGjkkkhGK....",
                "..kjGkkhGJhhGGjkhGK.....",
                "...kjGjhiiihGjjhGK..kk..",
                "...kGJhiiiihGGjGK..kGhk.",
                "....kGJhiihGGjjK..kGhk..",
                "....kGJhhGGGjK...kGhk...",
                ".....kGJGGGGjk..kGhk....",
                ".....kGGjjGGGjkGhhk.....",
                "......kGGjjGGGJhhk......",
                ".......kGjjGGJhhk.......",
                ".......kEGjGGjGk........",
                "......kFEGGjGGjk........",
            ]
        } else {
            &[
                "........................",
                "..........kkkk..........",
                "......kk.khiGGk..kkkk...",
                "..kk.khiKkiGjjk.khiGGk..",
                ".khiKkihKkhjkK..kiGjjk..",
                ".kiGKkihKkhjK...khjK....",
                ".khGKkhGKkhjK..khjK.....",
                "..khGKkhGkhGjkkhjK......",
                "..khGkkhGJhhGjkhGK......",
                "...kjGjhiiihGjjGK.......",
                "...kGJhiiiihGGjK....kkk..",
                "....kGJhiihGGjjK...kGhhk.",
                "....kGJhhGGGjjK..kGhhk..",
                ".....kGJGGGGjk..kGhhk...",
                ".....kGGjjGGGjkGhhk.....",
                "......kGGjjGGGJhhk......",
                ".......kGjjGGJhhk.......",
                ".......kEGjGGjGk........",
                "......kFEGGjGGjk........",
                "......kFEGGjGGjk........",
            ]
        };
        im.stamp(&Img::from_rows(palm), 0, 1);
        im.line(7, 21, 15, 21, 'F');
        im.put(11, 20, 'N');
    }
    im
}

/// Weed-hung plate, a slit visor and a rust-notched executioner's sword.
pub fn drowned(stage: u8, _t: u32, tick: u32) -> Img {
    let f = (tick / 8 % 2) as i32;
    let mut im = Img::new(28, 28);
    let body = Img::from_rows(&[
        ".........kkkkkk........",
        "........kSUUuSxk.......",
        ".......kSPoUSSSxk......",
        ".......kSUuSSSSSSk.....",
        ".......kSxkkkk2k2k.....",
        ".......kPxUuSSSxk......",
        "........kPPSxSxk.......",
        "....kkkkkSoPPkkk.......",
        "..kkSPoSSSUuSSSPkk.....",
        ".kSPoUUSxSUUUSxSPPk....",
        ".kSUUSSxFSSSSxFFSPk....",
        ".kSSSSxFEUUUSSxFSPk....",
        "..kSSxFElSUSSxFESk.....",
        "...kSxFElSPPPxFESk.....",
        "....kFFElIPoIFFEkk.....",
        "....kFEFkPPPIkFEk......",
        ".....kFEkSSSxkFEk......",
        ".....kFEkSxxk.FE.......",
    ]);
    let lean = i32::from(stage == 2);
    im.stamp(&body, 1 + lean, 5 + lean);
    if stage == 0 {
        let legs = Img::from_rows(if f == 0 {
            &[
                "...kSUxk...kSxk.....",
                "..kSPPxk...kSxk.....",
                "..kSSxk....kSPPk....",
                ".kSSxxk....kSSxxk...",
                "..FFFxxSSxFFFxx.....",
            ]
        } else {
            &[
                "....kSxk..kSUxk.....",
                "....kSxk..kSPPxk....",
                "...kSPPk...kSSxk....",
                "..kSSxxk...kSSxxk...",
                ".FFFxxFFFxSSxxF.....",
            ]
        });
        im.stamp(&legs, 3, 23);
        im.stamp(
            &Img::from_rows(&[
                ".......kSk",
                "......kUvk",
                ".....kUuPk",
                "....kUuPk.",
                "...kUuPk..",
                "..kUuPk...",
                ".kUuPk....",
                "kPIPk.....",
                ".kPk......",
                ".kIk......",
            ]),
            18,
            10 + f,
        );
    } else if stage == 1 {
        // Both hands raised; a broad blade stays overhead throughout windup.
        im.stamp(
            &Img::from_rows(&[
                "..................kkk.....",
                "...............kkkUvPk....",
                "............kkkUUUuPk.....",
                ".........kkkUUUuPPkk......",
                "......kkkUUUuPPkk.........",
                ".....kPPUuPPkk............",
                ".....kIPkkkk..............",
                "....kShPk.................",
                "....kSUhk.................",
                ".....kSxhk................",
            ]),
            1,
            0,
        );
        im.line(8, 8, 9, 15, 'S');
        im.line(9, 8, 10, 14, 'U');
        im.line(7, 23, 6, 26, 'S');
        im.line(15, 23, 17, 26, 'x');
        im.line(5, 27, 9, 27, 'F');
        im.line(15, 27, 20, 27, 'F');
        im.put(20, 5 + f * 2, 'U');
        im.put(24, 8 + f, 'S');
    } else {
        // Weight pitched forward, both hands bearing down on the stuck hilt.
        im.stamp(
            &Img::from_rows(&[
                "kSUhk.......",
                ".kSUhhkkkk..",
                "..kSPPhhIPk.",
                "....kkkkIPk.",
                ".......kPk..",
                "......kUPk..",
                "......kUPk..",
                "......kUPk..",
                "......kUPk..",
                "......kUPk..",
                ".....FFNFF..",
                "...FFNNNFFFD",
            ]),
            15,
            16,
        );
        im.line(8, 24, 6, 27, 'S');
        im.line(15, 24, 17, 27, 'x');
        im.put(24, 26, 'm');
    }
    im
}

pub fn wisp(stage: u8, _t: u32, tick: u32) -> Img {
    // Four flickers fit the arena's 48-tick pose cache exactly.
    let f = tick / 12 % 4;
    let rows: &[&str] = match stage {
        1 => &[
            "..........u.....",
            ".........uU.....",
            "...S....uUvS....",
            "...US..uU3vS....",
            "...vUS.U33US....",
            "S..UvUSU33U.S...",
            "US..U3U333vSU...",
            "UvS.U33333vUU...",
            ".UvUU3v33v33US..",
            "..U3vvsvvsv3UvS.",
            "..Uv3sk3sk33UUS.",
            ".Uv33vvv33333US.",
            "SUv333vs3vv33vUS",
            ".SUv33vsvv33vUS.",
            "..SUv333333vUS..",
            "...SUvv33vvUS...",
            "....SUvvvUSS....",
            ".....SUvUS......",
            "....SUUS........",
            "...SS...........",
        ],
        2 => &[
            ".......S....",
            "......Su....",
            "......uUS...",
            ".....SUUS...",
            ".....SUvUS..",
            "....SUvvUS..",
            "....UvsvsUS.",
            "....USkSkUS.",
            "....SUvvUS..",
            ".....SUUS...",
            "......SuS...",
            ".....SS.....",
        ],
        _ => &[
            "........S....",
            ".......Su....",
            ".......uUS...",
            "......SU3S...",
            ".....SU33S...",
            ".....U33US...",
            "....SU333US..",
            "...SU3333vUS.",
            "...UvsvvsvUS.",
            "...USk3Sk3US.",
            "...Uv33333US.",
            "....Uv3svUS..",
            "....SU33US...",
            ".....SUUS....",
            "....SUUS.....",
            "...SS........",
        ],
    };
    let src = Img::from_rows(rows);
    let mut im = Img::new(16, 20);
    im.stamp(
        &src,
        (16 - src.w) / 2,
        if stage == 1 { 0 } else { 1 + (f % 2) as i32 },
    );
    if stage == 1 && f % 2 == 1 {
        im.clear(3, 2);
        im.put(4, 2, 'U');
        im.put(4, 1, 'S');
        im.clear(0, 12);
        im.put(1, 11, 'u');
    }
    // Four flickers: the tail curls across, and one crown tongue leans east.
    if f >= 2 {
        im.put(10, if stage == 1 { 0 } else { 2 }, 'U');
        im.put(11, if stage == 1 { 1 } else { 3 }, 'u');
        im.clear(3, 17);
        im.put(8, 18, 'S');
    }
    im
}

/// The bronze reed crown sits between broken antlers. A beard of drowned
/// roots, a tarnished chain and a vast ragged mantle give him a royal heft.
const KING: &[&str] = &[
    "...n..............n.........n...",
    "...nP....n........nP.......Pn...",
    ".n.nP....Pn.......nP..n...Pn....",
    "..nPPn...Pn...P...nPPnP..Pn.....",
    "...nPPn.nP...PtP...nPPnnPn......",
    "....nPPnP...PtPtP...nPPPn.......",
    ".....nPPP..PtPtPtP..nPn.........",
    "......nPPn.PoPoPoP.nPn..........",
    ".......nPnnPPPPPPnnPnn..........",
    "......nFFFFFkGGhhhhGGkFFF.......",
    ".....nNmmmFFkGhhhhhhGjkFF.......",
    "....nNmmmNFFkGhk2kGh22kFFn......",
    "...nNmmmNNFFkGjhGkhhhGhkFEn.....",
    "..nNmmmNLNFFFkGjGkhhihGhkNEn....",
    ".nNmmmmNNFFFFkGGhhjjGGkkFNNEn...",
    ".nmmmNNNFFFFFFkGhjkkkhGkFmNNn...",
    "nmmmNNNFFFFFFFFkGhjGjjkFFNNNNn..",
    "nmmNNNFFFoFFFFFkIGjGjkkFFNNNNn..",
    "nmNNNFFFFFPFFFFkIPjGjPkFPENmNn..",
    "nFFEENNmNEFoFFkIPnGjPnFoFEENNn..",
    "nFFEENNNNEFFPFnIPnnPnFPFFEEENn..",
    "nFFEENNNNEFFFoFnIFFPnoFFFEEFFn..",
    ".nFEDNNNNEEFFFPtFFFtPFFEEDEFn...",
    ".nkGjDNNNEEFFnInPoPFFFFEjGDn....",
    "..khGkNNDEEFnIPnFFFFFFkGhGk.....",
    "..kjGjkNN.nInIPnInFFFkjGjk......",
    "...kkk.NNn.PnIPnIPnFF.kkk.......",
    "......nFFnIPnIPnIPnFFn..........",
    ".....nIPnnIPn..nIPnnIPn.........",
    "....nIPn.nIPn...nIPn.nIPn.......",
    "..nnIPn..nIPnn..nIPnn.nIPnn.....",
    ".nnnnnn.nnnnnnn.nnnnnn.nnnnnn...",
];

pub fn mire_king(raged: bool, tick: u32) -> Img {
    let mut im = Img::new(32, 32);
    let f = tick / 12 % 2;
    im.stamp(&Img::from_rows(KING), 0, 0);
    if f != 0 {
        // One shoulder heaves; weeds and the lowest beard root sway.
        im.put(27, 17, 'm');
        im.put(28, 18, 'N');
        im.put(15, 26, 'P');
        im.put(14, 26, 'n');
        im.clear(5, 1);
        im.put(6, 2, 'P');
    }
    if raged {
        im = im.recolor(&[('2', '4'), ('m', 'l'), ('N', 'E')]);
        // The brow lowers and the jaw parts; roots wrench through the robe.
        im.line(15, 10, 17, 10, 'k');
        im.line(20, 10, 23, 10, 'k');
        im.line(18, 15, 22, 15, 'k');
        im.put(19, 15, 'i');
        im.put(22, 15, 'h');
        im.line(4, 22, 1, 26, 'P');
        im.line(27, 23, 30, 27, 'P');
        im.put(2, 25, 'o');
        im.put(29, 26, 'o');
    }
    im
}

// ─── Blanchefleur ────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorsePose {
    Stand,
    Graze,
    Gallop(u32),
    Frightened,
}

/// Fill a small hand-authored silhouette; unlike the establishing Brush,
/// sprite planes stay in their exact named inks.
fn horse_plane(im: &mut Img, points: &[(i32, i32)], c: char) {
    let ymin = points.iter().map(|p| p.1).min().unwrap();
    let ymax = points.iter().map(|p| p.1).max().unwrap();
    for y in ymin..=ymax {
        let mut xs = Vec::new();
        for i in 0..points.len() {
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
                xs.push(a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1));
            }
        }
        xs.sort_unstable();
        for pair in xs.chunks_exact(2) {
            im.line(pair[0], y, pair[1], y, c);
        }
    }
}

/// Fine-legged and unburdened. Ivory H and i roll through grey flanks to
/// blue-grey underplanes; timber T warms just the forelock and mane.
pub fn white_horse(pose: HorsePose) -> Img {
    let mut im = Img::new(56, 48);
    let gait = if let HorsePose::Gallop(f) = pose {
        Some(f % 4)
    } else {
        None
    };
    let lift = if matches!(gait, Some(1 | 3)) { -2 } else { 0 };
    let frightened = pose == HorsePose::Frightened;
    let graze = pose == HorsePose::Graze;
    // Tail originates under the croup, with a full root and loose pale ends.
    let tail: &[(i32, i32)] = if gait.is_some() {
        &[
            (10, 22),
            (6, 22),
            (2, 24),
            (0, 28),
            (3, 27),
            (7, 26),
            (11, 26),
        ]
    } else {
        &[
            (11, 22),
            (7, 23),
            (5, 27),
            (2, 31),
            (2, 35),
            (5, 33),
            (9, 28),
            (12, 26),
        ]
    };
    horse_plane(&mut im, tail, 'G');
    if gait.is_some() {
        im.line(8, 23, 2, 26, 'i');
        im.line(6, 25, 1, 28, 'h');
    } else {
        im.line(9, 24, 4, 31, 'i');
        im.line(7, 28, 3, 34, 'h');
    }
    let legs: [Vec<(i32, i32)>; 4] = match gait {
        Some(0) => [
            vec![(14, 29), (10, 37), (5, 46)],
            vec![(34, 29), (39, 36), (48, 44)],
            vec![(18, 30), (13, 38), (8, 47)],
            vec![(38, 29), (44, 36), (53, 45)],
        ],
        Some(1) => [
            vec![(14, 27), (19, 35), (14, 43)],
            vec![(34, 27), (39, 33), (33, 40)],
            vec![(18, 28), (23, 37), (18, 47)],
            vec![(38, 27), (44, 33), (38, 42)],
        ],
        Some(2) => [
            vec![(14, 29), (21, 37), (29, 46)],
            vec![(34, 29), (28, 38), (20, 46)],
            vec![(18, 30), (25, 38), (34, 47)],
            vec![(38, 29), (31, 39), (24, 47)],
        ],
        Some(_) => [
            vec![(14, 27), (9, 35), (16, 42)],
            vec![(34, 27), (29, 34), (35, 41)],
            vec![(18, 28), (13, 36), (20, 47)],
            vec![(38, 27), (32, 35), (39, 43)],
        ],
        None if frightened => [
            vec![(14, 29), (12, 37), (9, 47)],
            vec![(34, 29), (31, 37), (28, 47)],
            vec![(18, 30), (17, 38), (14, 47)],
            vec![(38, 29), (41, 36), (36, 42)],
        ],
        None => [
            vec![(14, 29), (13, 37), (12, 47)],
            vec![(34, 29), (33, 38), (32, 47)],
            vec![(18, 30), (20, 37), (18, 47)],
            vec![(38, 29), (39, 38), (38, 47)],
        ],
    };
    let leg = |im: &mut Img, pts: &[(i32, i32)], near: bool| {
        let (a, b, c) = (pts[0], pts[1], pts[2]);
        horse_plane(
            im,
            &[
                (a.0 - 2, a.1 - 2),
                (a.0 + 2, a.1 - 2),
                (b.0 + 1, b.1 + 1),
                (b.0 - 1, b.1 + 1),
            ],
            if near { 'h' } else { 'S' },
        );
        for dx in -1..=1 {
            im.line(
                b.0 + dx,
                b.1,
                c.0 + dx,
                c.1 - 1,
                if near { 'h' } else { 'S' },
            );
        }
        im.line(a.0 - 1, a.1, b.0, b.1, if near { 'i' } else { 'U' });
        im.line(b.0, b.1, c.0, c.1 - 1, if near { 'i' } else { 'u' });
        im.put(b.0 + 1, b.1, 'G');
        im.rect(c.0 - 1, c.1 - 1, 4, 2, 'g');
        im.line(c.0, c.1 - 1, c.0 + 2, c.1 - 1, 'j');
    };
    for pts in &legs[..2] {
        leg(&mut im, pts, false);
    }
    // The shoulder and neck form one continuous silhouette with the barrel.
    let mut skin = Img::new(56, 48);
    skin.ellipse(23.0, 26.5 + lift as f32, 16.0, 7.0, 'h');
    skin.ellipse(14.0, 26.0 + lift as f32, 6.0, 6.0, 'h');
    skin.ellipse(35.0, 26.0 + lift as f32, 7.0, 7.0, 'h');
    let neck: &[(i32, i32)] = if graze {
        &[
            (31, 22),
            (37, 23),
            (41, 28),
            (44, 33),
            (49, 39),
            (46, 42),
            (41, 37),
            (36, 32),
            (31, 31),
        ]
    } else if frightened {
        &[
            (27, 25),
            (31, 17),
            (34, 9),
            (37, 4),
            (42, 5),
            (44, 10),
            (41, 18),
            (42, 25),
            (38, 32),
            (31, 32),
        ]
    } else {
        &[
            (27, 25),
            (31, 18),
            (35, 11),
            (38, 7),
            (43, 8),
            (45, 13),
            (43, 20),
            (43, 26),
            (39, 32),
            (31, 32),
        ]
    };
    let translated: Vec<_> = neck.iter().map(|&(x, y)| (x, y + lift)).collect();
    horse_plane(&mut skin, &translated, 'h');
    let head: &[(i32, i32)] = if graze {
        &[
            (43, 33),
            (46, 34),
            (48, 38),
            (52, 42),
            (52, 45),
            (48, 46),
            (45, 42),
            (42, 38),
        ]
    } else if frightened {
        &[
            (37, 4),
            (42, 4),
            (46, 7),
            (52, 8),
            (53, 10),
            (51, 12),
            (46, 12),
            (42, 10),
            (40, 14),
            (36, 12),
        ]
    } else {
        &[
            (38, 7),
            (43, 7),
            (47, 10),
            (53, 12),
            (54, 15),
            (51, 17),
            (46, 15),
            (43, 14),
            (41, 18),
            (37, 15),
        ]
    };
    let translated: Vec<_> = head.iter().map(|&(x, y)| (x, y + lift)).collect();
    horse_plane(&mut skin, &translated, 'h');
    // Ears communicate mood even in the small pasture drawing.
    let ears: &[&[(i32, i32)]] = if graze {
        &[
            &[(43, 34), (42, 29), (45, 33)],
            &[(46, 36), (47, 31), (48, 37)],
        ]
    } else if frightened {
        &[
            &[(38, 6), (33, 2), (37, 2), (42, 6)],
            &[(40, 5), (37, 1), (40, 2), (44, 7)],
        ]
    } else {
        &[
            &[(39, 9), (38, 2), (40, 3), (42, 9)],
            &[(43, 10), (44, 3), (46, 4), (46, 11)],
        ]
    };
    for ear in ears {
        let p: Vec<_> = ear.iter().map(|&(x, y)| (x, y + lift)).collect();
        horse_plane(&mut skin, &p, 'i');
    }
    for y in 0..48 {
        for x in 0..56 {
            if !skin.solid(x, y) {
                continue;
            }
            let yy = y - lift;
            let c = if yy >= 21 && x < 39 {
                // A broad lit top, rounded haunch and cool belly, with a quiet
                // muscular shoulder. These are planes rather than stipple.
                let volume =
                    1.0 - ((x - 21) as f32 / 18.0).powi(2) - ((yy as f32 - 23.5) / 8.0).powi(2);
                if volume > 0.74 {
                    'H'
                } else if volume > 0.48 {
                    'i'
                } else if volume > 0.16 {
                    'h'
                } else if volume > -0.10 {
                    'J'
                } else {
                    'G'
                }
            } else if graze {
                if x < 43 {
                    'i'
                } else if yy < 41 {
                    'H'
                } else {
                    'h'
                }
            } else {
                // Neck highlight follows its diagonal, so it flows into the back.
                let mid = 39 - (yy - 12).max(0) / 3;
                if x < mid - 2 {
                    'h'
                } else if x <= mid + 2 {
                    'H'
                } else if x <= mid + 5 {
                    'i'
                } else {
                    'h'
                }
            };
            skin.put(x, y, c);
        }
    }
    im.stamp(&skin, 0, 0);
    for pts in &legs[2..] {
        leg(&mut im, pts, true);
    }
    // Curved haunch, shoulder muscle and a few separated mane locks.
    im.line(11, 26 + lift, 12, 29 + lift, 'i');
    im.line(12, 29 + lift, 15, 30 + lift, 'J');
    im.line(35, 27 + lift, 34, 30 + lift, 'G');
    im.line(36, 27 + lift, 35, 30 + lift, 'h');
    if graze {
        im.line(38, 27, 43, 33, 'G');
        im.line(39, 27, 44, 33, 'i');
        im.put(46, 38, 'k');
        im.put(47, 38, 'H');
        im.put(51, 43, 'g');
        im.line(49, 45, 52, 45, 'j');
    } else {
        let (topx, topy) = if frightened { (36, 7) } else { (37, 10) };
        for i in 0..4 {
            let x = topx - i;
            let y = topy + i * 3 + lift;
            im.line(x, y, x - 2, y + 4, 'G');
            im.line(x + 1, y, x - 1, y + 4, if i == 0 { 'T' } else { 'i' });
        }
        let (ex, ey) = if frightened { (44, 7) } else { (45, 11) };
        im.put(ex, ey + lift, 'k');
        im.put(ex + 1, ey - 1 + lift, 'H');
        im.put(
            if frightened { 51 } else { 52 },
            if frightened { 9 } else { 14 } + lift,
            'g',
        );
        im.line(
            if frightened { 48 } else { 49 },
            if frightened { 11 } else { 16 } + lift,
            if frightened { 51 } else { 52 },
            if frightened { 11 } else { 16 } + lift,
            'j',
        );
        im.put(topx + 2, topy - 2 + lift, 'T');
    }
    im.outline_outside('k');
    im
}

pub fn white_coat() -> (char, char, char) {
    ('H', 'h', 'i')
}

// ─── Bog-timber prison and marsh sign ─────────────────────────────────────

pub fn pen(open: bool, tick: u32) -> Img {
    let mut im = Img::new(64, 64);
    // Rear lintel is high above her face; two front rails sit below it.
    im.stamp(
        &Img::from_rows(&[
            "....nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn....",
            "..nnIPPPPPPPoPPPPPPPPPPPPPPPPPPoPPPPPPPPPPPPPPPPPPPPnn..",
            ".nIPPPIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIPn.",
            "..nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn..",
        ]),
        4,
        5,
    );
    for x in [3, 57] {
        im.rect(x - 1, 7, 7, 57, 'n');
        im.rect(x, 9, 4, 54, 'I');
        im.line(x, 10, x, 61, 'o');
        im.line(x + 2, 10, x + 2, 62, 'P');
        im.line(x + 3, 16, x + 3, 54, 'b');
        for y in [12, 16, 33, 37, 54, 58] {
            im.line(x - 2, y, x + 5, y - 1, 'P');
            im.line(x - 2, y - 1, x + 4, y - 2, 't');
        }
        im.put(x + 1, 24, 'b');
        im.put(x + 2, 25, 'b');
    }
    if open {
        // Gate seen edge-on at the left hinge, its diagonal rails foreshortened.
        for y in [34, 54] {
            for dy in 0..4 {
                im.line(8, y + dy, 19, y - 8 + dy, if dy == 0 { 'o' } else { 'I' });
            }
        }
        im.rect(17, 25, 4, 31, 'n');
        im.line(18, 26, 18, 55, 'P');
    } else {
        for x in [16, 29, 42] {
            im.rect(x, 33, 3, 30, 'n');
            im.line(x, 34, x, 62, 'P');
        }
        for y in [34, 54] {
            im.rect(7, y, 50, 5, 'n');
            im.line(8, y, 56, y, 'o');
            im.line(8, y + 1, 56, y + 1, 'P');
            im.line(9, y + 3, 55, y + 3, 'I');
        }
        im.line(9, 52, 53, 39, 'b');
        im.line(9, 53, 53, 40, 'P');
        // The only luminous ornament is the seal that holds the gate shut.
        im.stamp(
            &Img::from_rows(&[
                "..SSS..", ".SuuuS.", ".Su.uS.", "kP444Pk", "k44644k", "k44k44k", "k44k44k",
                ".kPPPk.",
            ]),
            29,
            33,
        );
        if tick / 12 % 2 == 1 {
            im.put(32, 37, '5');
        }
    }
    im.line(1, 63, 9, 63, 'n');
    im.line(55, 63, 63, 63, 'n');
    im
}

pub fn plaque_icon() -> Img {
    Img::from_rows(&[
        "......k..k......",
        ".....kHkkHk.....",
        "....kTHHHhk.....",
        "...khiHHkHihk...",
        "...kiHHHHHHihk..",
        "..khiHHHHHihGk..",
        "..kiHHHhhkkkk...",
        ".khiHHhGk.......",
        ".kiHHhGk...E....",
        ".khiHGk...El....",
        "..kGGk.E..El..F.",
        "...kk..El.El.EF.",
        "....F..ElElElEF.",
        ".....FFElElEEF..",
        "......FNNNFFF...",
        ".......DFFFD....",
    ])
}

/// The contract the art team's drawings drop into: sizes, the realm palette,
/// and signal inks (the glowing bank) only where the swamp's art may glow.
#[cfg(test)]
mod tests {
    use super::super::super::ink::{BLACK, is_signal, palette_index};
    use super::*;

    fn inked(img: &Img) -> usize {
        (0..img.h)
            .flat_map(|y| (0..img.w).filter_map(move |x| img.get(x, y)))
            .count()
    }

    /// In the palette, within `w` by `h`, something drawn, and at most
    /// `glow` signal pixels.
    fn check(what: &str, img: &Img, (w, h): (i32, i32), glow: usize) {
        assert!(
            img.w > 0 && img.h > 0 && img.w <= w && img.h <= h,
            "{what}: {}x{} over {w}x{h}",
            img.w,
            img.h
        );
        assert!(inked(img) > 0, "{what}: nothing drawn");
        let mut signal = 0;
        for y in 0..img.h {
            for x in 0..img.w {
                if let Some(c) = img.get(x, y) {
                    assert!(
                        palette_index(c).is_some(),
                        "{what}: off-palette pixel at {x},{y}"
                    );
                    signal += usize::from(is_signal(c));
                }
            }
        }
        assert!(
            signal <= glow,
            "{what}: {signal} signal pixels, at most {glow}"
        );
    }

    const TICKS: [u32; 6] = [0, 4, 8, 12, 24, 47];

    #[test]
    fn swamp_poses_animate_and_the_hall_king_matches_his_boss_portrait() {
        assert!(!PLACEHOLDER);
        for stage in 0..3 {
            assert_ne!(bog_hand(stage, 0, 0), bog_hand(stage, 0, 8));
            let frames: std::collections::HashSet<_> = (0..4)
                .map(|f| wisp(stage, 0, f * 12).content_hash())
                .collect();
            assert_eq!(frames.len(), 4, "four flame flickers in stage {stage}");
        }
        assert_ne!(drowned(0, 0, 0), drowned(0, 0, 8));
        for raged in [false, true] {
            assert_ne!(mire_king(raged, 0), mire_king(raged, 12));
        }
        let poses: std::collections::HashSet<_> =
            [HorsePose::Stand, HorsePose::Graze, HorsePose::Frightened]
                .into_iter()
                .chain((0..4).map(HorsePose::Gallop))
                .map(|pose| {
                    let im = white_horse(pose);
                    assert!(
                        (0..im.w).any(|x| im.get(x, im.h - 1).is_some()),
                        "hoof anchor: {pose:?}"
                    );
                    im.content_hash()
                })
                .collect();
        assert_eq!(poses.len(), 7);
        assert_eq!(
            white_horse(HorsePose::Gallop(0)),
            white_horse(HorsePose::Gallop(4))
        );
        let boss = crate::rules::bosses::builtin()
            .into_iter()
            .find(|b| b.id == crate::rules::swamp::KING)
            .unwrap();
        let portrait = Img::from_rows(&boss.art.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(portrait, mire_king(false, 0));
    }

    /// Every authored state at six native pixels per pixel. Also writes the
    /// three full establishing paintings separately, for composition review.
    #[test]
    #[ignore]
    fn write_swamp_art_sheet() {
        let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cells = Vec::<(String, Img)>::new();
        let mut add = |name: String, im: Img| cells.push((name, im));
        let mut dry = Img::new(64, 48);
        for y in 0..48 {
            for x in 0..64 {
                if let Some(c) = ground(x, y) {
                    dry.put(x, y, c);
                }
            }
        }
        add("DRY CAUSEWAY".into(), dry);
        for step in [0, 2, 4, 6, 8, 10, 12] {
            let mut im = Img::new(64, 48);
            bog_field(&mut im, (0, 0, 64, 48), &|_, _| true, step);
            add(format!("BOG {step}"), im);
        }
        for v in 0..3 {
            let mut im = Img::new(64, 48);
            reeds(&mut im, 24, 24, v);
            add(format!("BLOCK {v}"), im);
        }
        for tick in [0, 48] {
            let mut im = Img::new(64, 48);
            for y in 0..48 {
                for x in 0..64 {
                    if let Some(c) = mist(x, y, tick) {
                        im.put(x, y, c);
                    }
                }
            }
            add(format!("MIST {tick}"), im);
        }
        add("MARSH EMBLEM".into(), plaque_icon());
        let mut inks = Img::new(56, 24);
        for (i, &c) in WALL_RAMP.iter().enumerate() {
            inks.rect(i as i32 * 8, 0, 8, 8, c);
        }
        let (hide, dark, mane) = white_coat();
        for (i, c) in [hide, dark, mane].into_iter().enumerate() {
            inks.rect(i as i32 * 16, 16, 16, 8, c);
        }
        add("WALL / COAT".into(), inks);
        for stage in 0..3 {
            for f in 0..2 {
                add(format!("HAND {stage} / {f}"), bog_hand(stage, 20, f * 8));
            }
        }
        for stage in 0..3 {
            for f in 0..2 {
                add(format!("DROWN {stage} / {f}"), drowned(stage, 20, f * 8));
            }
        }
        for stage in 0..3 {
            for f in 0..4 {
                add(format!("WISP {stage} / {f}"), wisp(stage, 20, f * 12));
            }
        }
        for raged in [false, true] {
            for f in 0..2 {
                add(
                    format!("KING {} / {f}", if raged { "R" } else { "C" }),
                    mire_king(raged, f * 12),
                );
            }
        }
        for (name, pose) in [
            ("STAND", HorsePose::Stand),
            ("GRAZE", HorsePose::Graze),
            ("FRIGHTENED", HorsePose::Frightened),
        ] {
            add(name.into(), white_horse(pose));
        }
        for f in 0..4 {
            add(format!("GALLOP {f}"), white_horse(HorsePose::Gallop(f)));
        }
        for open in [false, true] {
            for tick in [0, 12] {
                add(
                    format!("PEN {} / {}", if open { "OPEN" } else { "SHUT" }, tick / 12),
                    pen(open, tick),
                );
            }
            let mut im = Img::new(64, 64);
            im.stamp(
                &white_horse(if open {
                    HorsePose::Stand
                } else {
                    HorsePose::Frightened
                }),
                4,
                10,
            );
            im.stamp(&pen(open, 0), 0, 0);
            add(format!("MARE / {}", if open { "OPEN" } else { "SHUT" }), im);
        }
        let rows = cells.len().div_ceil(8) as i32;
        let mut sheet = Img::black(640, 32 + rows * 84);
        sheet.text(8, 6, "SWAMP OF HORRORS - INK STUDIES", 'i');
        sheet.text(
            8,
            18,
            "BLANCHEFLEUR / THE MIRE KING / NATIVE PIXELS X6",
            'U',
        );
        for (i, (name, im)) in cells.iter().enumerate() {
            let (x, y) = ((i % 8) as i32 * 80, 32 + (i / 8) as i32 * 84);
            sheet.rect(x + 1, y + 1, 78, 82, 's');
            sheet.rect(x + 2, y + 12, 76, 69, 'k');
            sheet.text(x + 3, y + 3, name, 'h');
            sheet.stamp(im, x + (80 - im.w) / 2, y + 79 - im.h);
        }
        image::save_buffer(
            dir.join("swamp-art-sheet.png"),
            &sheet.rgba_scaled(6),
            sheet.w as u32 * 6,
            sheet.h as u32 * 6,
            image::ColorType::Rgba8,
        )
        .unwrap();
        for depth in 1..=3 {
            let im =
                super::super::plates::painting(crate::rules::floor_plates::Kind::Swamp, depth, 27);
            image::save_buffer(
                dir.join(format!("swamp-plate-{depth}.png")),
                &im.rgba_scaled(3),
                im.w as u32 * 3,
                im.h as u32 * 3,
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    }

    #[test]
    fn the_ground_the_bog_the_reeds_and_the_mist_keep_to_the_dark_palette() {
        let (mut drawn, mut misted) = (0, 0);
        for y in 0..64 {
            for x in 0..64 {
                if let Some(ch) = ground(x, y) {
                    let c = super::super::super::ink::ink(ch).expect("an ink");
                    assert!(!is_signal(c), "dry ground never glows");
                    drawn += 1;
                }
                if let Some(ch) = mist(x, y, 9) {
                    assert!(!is_signal(super::super::super::ink::ink(ch).unwrap()));
                    misted += 1;
                }
            }
        }
        assert!(
            drawn < 64 * 64 * 30 / 100,
            "dry ground is marks on black paper: {drawn}"
        );
        assert!(
            misted < 64 * 64 * 15 / 100,
            "mist is wisps, not a wall: {misted}"
        );
        // The bog paints only bog, never glowing.
        let mut cv = Img::new(96, 64);
        let bog = |x: i32, y: i32| (16..64).contains(&x) && (16..48).contains(&y);
        bog_field(&mut cv, (0, 0, 96, 64), &bog, 3);
        let mut wet = 0;
        for y in 0..64 {
            for x in 0..96 {
                if let Some(c) = cv.get(x, y) {
                    assert!(bog(x, y), "the bog stays on the bog: {x},{y}");
                    assert!(c != BLACK && !is_signal(c));
                    wet += 1;
                }
            }
        }
        assert!(wet > 48 * 32 / 4, "the bog reads: {wet}");
        // A clump of reeds keeps to its tile and a little above it.
        let mut cv = Img::new(64, 64);
        for v in [0u32, 7, 99, 1234] {
            reeds(&mut cv, 24, 32, v);
        }
        for y in 0..64 {
            for x in 0..64 {
                if cv.get(x, y).is_some() {
                    assert!(
                        (24 - 4..24 + 20).contains(&x) && (32 - 14..32 + 16).contains(&y),
                        "reeds at {x},{y}"
                    );
                }
            }
        }
        assert_eq!(WALL_RAMP.len(), 7);
        assert!(
            WALL_RAMP
                .iter()
                .all(|&ch| !is_signal(super::super::super::ink::ink(ch).unwrap()))
        );
    }

    #[test]
    fn the_horrors_and_the_king_fit_their_frames_and_glow_only_as_allowed() {
        for stage in 0..3u8 {
            for &tick in &TICKS {
                for t in [0, 10, 20] {
                    check("bog hand", &bog_hand(stage, t, tick), (24, 24), 0);
                    // The visor and the blade's edge.
                    check("drowned knight", &drowned(stage, t, tick), (28, 28), 32);
                    check("wisp", &wisp(stage, t, tick), (16, 20), 64);
                }
            }
        }
        for raged in [false, true] {
            for &tick in &TICKS {
                check("the Mire King", &mire_king(raged, tick), (32, 32), 12);
            }
        }
    }

    #[test]
    fn the_white_horse_her_pen_and_the_plaque_fit() {
        for pose in [HorsePose::Stand, HorsePose::Graze, HorsePose::Frightened]
            .into_iter()
            .chain((0..4).map(HorsePose::Gallop))
        {
            // Her eye and the tack's trim; never the coat (below).
            check("the white horse", &white_horse(pose), (56, 48), 40);
        }
        let (hide, dark, mane) = white_coat();
        for ch in [hide, dark, mane] {
            assert!(
                !is_signal(super::super::super::ink::ink(ch).expect("an ink")),
                "a coat never glows"
            );
        }
        for open in [false, true] {
            for &tick in &TICKS {
                // Its lock glows while it holds: state, not dressing.
                check("the pen", &pen(open, tick), (64, 64), 32);
            }
        }
        check("the plaque icon", &plaque_icon(), (16, 16), 4);
    }
}
