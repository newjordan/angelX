//! The realm's hero, the knight every player starts as: drawn toward the
//! viewer, across (facing right; flipped for the left) and away, in parts
//! on one sixteen-pixel square so each stage of dress, each step of the
//! stride and each thing held is its own drawing.
//!
//! The steel is the stone bank's cool ramp, navy in its shadows and pale
//! where the light from the upper left catches it; the edge away from the
//! light takes the ramp's own darkest step, never black, and the lit edge
//! has none. One warm cream glint keys the crown, and a cool rim of sky
//! catches the shadowed side of the helm and the shoulder. The seat's
//! colour is the plume, the tabard and the shield's field (`7 8 5`,
//! recoloured per seat as ever); the shield's rim is gold, lit along its
//! top (bronze for the gold seat, so its field stands off its rim).
//! Leather is the timber bank; a bow is one tone of wood on a dark string,
//! so it never crowds the silhouette. Every part is ink rows, read once and
//! kept: a dressed figure is composed once per thread and copied out after.

use std::cell::RefCell;
use std::collections::HashMap;

use super::super::ink::Img;

/// A drawing inked in the structure greys (older armour: a helm in the
/// air, a breastplate on the pit stop's rack) taken to the hero's steel.
pub const STEEL: &[(char, char)] = &[
    ('H', 'W'),
    ('i', 'V'),
    ('h', 'v'),
    ('J', 'U'),
    ('G', 'u'),
    ('g', 'x'),
];

/// Which way the hero is seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum View {
    Toward,
    /// Facing right; the caller flips the picture for the left.
    Across,
    Away,
}

/// What the hero has in hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Held {
    Nothing,
    Bow,
    Crossbow,
    Handgonne,
    Sword,
}

/// One drawing of the hero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dress {
    pub view: View,
    /// The helm on; else the bare head.
    pub helm: bool,
    /// In harness and tabard; else in smallclothes.
    pub harness: bool,
    /// The shield at rest on the arm.
    pub shield: bool,
    /// The stride: 0 standing, 1 the first foot planted, 2 passing, 3 the
    /// other foot planted, 4 passing again. Passing rides a pixel higher.
    pub step: u8,
    pub held: Held,
    /// The held thing in its strike: drawn, levelled, fired, swung.
    pub strike: bool,
}

impl Dress {
    /// Whole, at rest, toward the viewer: the mini-viz's knight.
    pub const WHOLE: Dress = Dress {
        view: View::Toward,
        helm: true,
        harness: true,
        shield: true,
        step: 0,
        held: Held::Nothing,
        strike: false,
    };
}

/// A drawing placed against the figure's top-left.
struct Layer {
    at: (i32, i32),
    rows: &'static [&'static str],
}

/// One way of seeing the hero. The head is the top eight rows, the body
/// the six under it, the legs the last two.
struct Parts {
    helm: &'static [&'static str],
    bare: &'static [&'static str],
    harness: &'static [&'static str],
    small: &'static [&'static str],
    /// Legs in harness: standing, the first foot planted, the other.
    steel: [&'static [&'static str]; 3],
    /// Bare legs, the same three.
    skin: [&'static [&'static str]; 3],
    shield: Layer,
    /// Bow, crossbow, handgonne, sword: at rest, and in the strike.
    held: [[Layer; 2]; 4],
}

fn parts(view: View) -> &'static Parts {
    match view {
        View::Toward => &TOWARD,
        View::Across => &ACROSS,
        View::Away => &AWAY,
    }
}

/// The hero dressed as `d`, sixteen wide with the feet on the bottom row
/// (a passing step is a row taller, its last row clear).
pub fn draw(d: Dress) -> Img {
    thread_local! {
        static DRAWN: RefCell<HashMap<Dress, Img>> = RefCell::new(HashMap::new());
    }
    if let Some(im) = DRAWN.with(|m| m.borrow().get(&d).cloned()) {
        return im;
    }
    let im = compose(d);
    DRAWN.with(|m| m.borrow_mut().insert(d, im.clone()));
    im
}

fn compose(d: Dress) -> Img {
    let p = parts(d.view);
    let mut im = Img::new(16, 16);
    let legs = if d.harness { &p.steel } else { &p.skin };
    let foot = match d.step {
        1 => 1,
        3 => 2,
        _ => 0,
    };
    im.stamp(&Img::from_rows(legs[foot]), 0, 14);
    im.stamp(
        &Img::from_rows(if d.harness { p.harness } else { p.small }),
        0,
        8,
    );
    im.stamp(&Img::from_rows(if d.helm { p.helm } else { p.bare }), 0, 0);
    if d.shield {
        im.stamp(&Img::from_rows(p.shield.rows), p.shield.at.0, p.shield.at.1);
    }
    let held = match d.held {
        Held::Nothing => None,
        Held::Bow => Some(&p.held[0]),
        Held::Crossbow => Some(&p.held[1]),
        Held::Handgonne => Some(&p.held[2]),
        Held::Sword => Some(&p.held[3]),
    };
    if let Some([rest, strike]) = held {
        let layer = if d.strike { strike } else { rest };
        im.stamp(&Img::from_rows(layer.rows), layer.at.0, layer.at.1);
    }
    if matches!(d.step, 2 | 4) {
        let mut lifted = Img::new(16, 17);
        lifted.stamp(&im, 0, 0);
        return lifted;
    }
    im
}

/// The operator's own knight in the mini-viz: the hero with a frost plume,
/// a sea-green tabard and a blue shield.
pub fn visitor() -> Img {
    let p = parts(View::Toward);
    let mut im = Img::new(16, 16);
    im.stamp(&Img::from_rows(p.steel[0]), 0, 14);
    im.stamp(
        &Img::from_rows(p.harness).recolor(&[('7', '2'), ('8', '0'), ('5', 'w')]),
        0,
        8,
    );
    im.stamp(
        &Img::from_rows(p.helm).recolor(&[('7', 'w'), ('8', '3')]),
        0,
        0,
    );
    im.stamp(
        &Img::from_rows(p.shield.rows).recolor(&[
            ('7', '1'),
            ('5', 'w'),
            ('6', '3'),
            ('4', '2'),
            ('a', '0'),
        ]),
        p.shield.at.0,
        p.shield.at.1,
    );
    im
}

/// The helm as it flies off: the head's top rows, cut to the helm.
pub fn helmet() -> Img {
    let head = Img::from_rows(TOWARD.helm);
    let mut im = Img::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            if let Some(c) = head.get(x + 4, y) {
                im.set(x, y, c);
            }
        }
    }
    im
}

// ─── the drawings ────────────────────────────────────────────────────────────

/// The hero toward the viewer.
const TOWARD: Parts = Parts {
    helm: &[
        ".......87.......",
        "......8777......",
        ".....uV78Vu.....",
        "....uVTWVVvx....",
        "....vWVVVvUz....",
        "....vsssssSz....",
        "....vVWVvUUx....",
        "....xvVvUUux....",
    ],
    bare: &[
        "................",
        "................",
        ".....bBBBBb.....",
        "....bBRRRRBb....",
        "....BRttttRB....",
        "....Rtkttktb....",
        "....rotTttob....",
        ".....oOttOo.....",
    ],
    harness: &[
        "..xVWvxuUxvVvz..",
        "..uVvvU778uVUz..",
        "..uvUUv778vUUx..",
        "..xuUuv778uvx...",
        "...WuxBB4BBzx...",
        "......8778......",
    ],
    small: &[
        "...oOTttttOOo...",
        "..otTOtTttOoOr..",
        "..otOottttooOr..",
        "..rO.oOttOo.or..",
        "...t.HHhHHh.t...",
        "......HhhH......",
    ],
    steel: [
        &["......Vu.vu.....", ".....xux.xux...."],
        &[".....xux.vu.....", ".........xux...."],
        &["......Vu.xux....", ".....xux........"],
    ],
    skin: [
        &["......tO.tO.....", ".....rOr.rOr...."],
        &[".....rOr.tO.....", ".........rOr...."],
        &["......tO.rOr....", ".....rOr........"],
    ],
    shield: Layer {
        at: (0, 9),
        rows: &[
            ".64444", ".47774", ".47574", ".47574", ".a777a", "..a7a.", "...a..",
        ],
    },
    held: [
        // the bow
        [
            Layer {
                at: (11, 5),
                rows: &[
                    "..r.", ".r.g", "r..g", "r..g", "r..g", "r..g", "r..g", ".r.g", "..r.",
                ],
            },
            Layer {
                at: (8, 9),
                rows: &["r.....r", ".r...r.", "..rrr..", "...j..."],
            },
        ],
        // the crossbow
        [
            Layer {
                at: (10, 11),
                rows: &["uV...", ".Brr.", "..Br."],
            },
            Layer {
                at: (8, 10),
                rows: &["uVVVVu", "..rB.."],
            },
        ],
        // the handgonne
        [
            Layer {
                at: (10, 11),
                rows: &["Br..", ".Bx.", "..xu"],
            },
            Layer {
                at: (9, 11),
                rows: &[".Bx.", "..xu", ".6@6", "..6."],
            },
        ],
        // the sword
        [
            Layer {
                at: (11, 5),
                rows: &[
                    "..W.", ".WV.", ".WV.", ".WV.", ".WV.", "4444", ".B..", ".4..",
                ],
            },
            Layer {
                at: (11, 2),
                rows: &[
                    ".W..", ".WV.", ".WV.", ".WV.", ".WV.", ".WV.", "4444", ".B..", ".4..",
                ],
            },
        ],
    ],
};

/// The hero across, facing right.
const ACROSS: Parts = Parts {
    helm: &[
        "................",
        "......8777......",
        "....877uVWu.....",
        "....87uVTWVx....",
        ".....vWVVVvUx...",
        ".....vVVVsssx...",
        ".....vVWVvUUx...",
        ".....xvVvUUx....",
    ],
    bare: &[
        "................",
        "................",
        "......bBBBb.....",
        ".....bBRRRBb....",
        ".....BRRtttb....",
        ".....BRttktt....",
        ".....bRotTtt....",
        "......rotto.....",
    ],
    harness: &[
        "....uxvVvvUx....",
        "....uVvU778Ux...",
        "....uvUU778vx...",
        "....xuUu778ux...",
        ".....xBBB4Bzx...",
        "......87788.....",
    ],
    small: &[
        ".....oOTttO.....",
        "....otTOttOr....",
        "....otOottOr....",
        "....rOooOtor....",
        ".....rHHhHt.....",
        "......HhhH......",
    ],
    steel: [
        &["......Vuvu......", ".....xuxxux....."],
        &[".....u...Vu.....", "....xx....xux..."],
        &[".....Vu..ux.....", "....xux...xxx..."],
    ],
    skin: [
        &["......tOoO......", ".....rOrrOr....."],
        &[".....o...tO.....", "....rr....rOr..."],
        &[".....tO..or.....", "....rOr...rrr..."],
    ],
    shield: Layer {
        at: (2, 9),
        rows: &[
            "6444.", "47774", "47574", "47574", "a777a", ".a7a.", "..a..",
        ],
    },
    held: [
        // the bow
        [
            Layer {
                at: (12, 5),
                rows: &[
                    ".r.", "r.g", "r.g", "r.g", "r.g", "r.g", "r.g", ".rg", "..r",
                ],
            },
            Layer {
                at: (10, 5),
                rows: &[
                    "...r.", "..r..", ".j.r.", "j..r.", "j..r.", "j..r.", ".j.r.", "..r..", "...r.",
                ],
            },
        ],
        // the crossbow
        [
            Layer {
                at: (10, 10),
                rows: &["uV...", ".Brr.", "..BuV", "....u"],
            },
            Layer {
                at: (9, 8),
                rows: &[".....u", ".....V", "BBrrrV", ".....V", ".....u"],
            },
        ],
        // the handgonne
        [
            Layer {
                at: (10, 11),
                rows: &["Br...", ".Bxu.", "...xu"],
            },
            Layer {
                at: (9, 9),
                rows: &[".....6.", "BrxuuV@", ".....6."],
            },
        ],
        // the sword
        [
            Layer {
                at: (11, 4),
                rows: &[
                    "..W.", ".WV.", ".WV.", ".WV.", ".WV.", "4444", ".B..", ".4..",
                ],
            },
            Layer {
                at: (7, 9),
                rows: &["....J....", "4BBJHHHHW", "....J...."],
            },
        ],
    ],
};

/// The hero away from the viewer.
const AWAY: Parts = Parts {
    helm: &[
        ".......78.......",
        "......7778......",
        ".....u7778u.....",
        "....uV787Vvx....",
        "....vTV8VvUz....",
        "....vVVVvvUz....",
        "....uvVvvUux....",
        "....xuvvUUux....",
    ],
    bare: &[
        "................",
        "................",
        ".....bBBBBb.....",
        "....bBRRRRBb....",
        "....BRRRRRRB....",
        "....bRRRRRRb....",
        "....bBRRRRBb....",
        ".....bortob.....",
    ],
    harness: &[
        "..xVWvuUUuvVvz..",
        "..uVv777778VUz..",
        "..uvU777778UUx..",
        "..xuU777878ux...",
        "...WxBBBBBBzx...",
        "......8778......",
    ],
    small: &[
        "...oOTttttOOo...",
        "..otTOtttttOOr..",
        "..otOottttooOr..",
        "..rO.oOttOo.or..",
        "...t.HHHHHh.t...",
        "......HhhH......",
    ],
    steel: [
        &["......Vu.vu.....", ".....xux.xux...."],
        &[".....xux.vu.....", ".........xux...."],
        &["......Vu.xux....", ".....xux........"],
    ],
    skin: [
        &["......tO.tO.....", ".....rOr.rOr...."],
        &[".....rOr.tO.....", ".........rOr...."],
        &["......tO.rOr....", ".....rOr........"],
    ],
    shield: Layer {
        at: (0, 9),
        rows: &[
            ".aaaaa", ".aPPPa", ".aPbPa", ".aPbPa", ".aPPPa", "..aPa.", "...a..",
        ],
    },
    held: [
        // the bow
        [
            Layer {
                at: (11, 5),
                rows: &[".r.", "r.g", "r.g", "r.g", "r.g", "r.g", ".rg", "..r"],
            },
            Layer {
                at: (8, 3),
                rows: &["r.....r", ".r...r.", "..rrr.."],
            },
        ],
        // the crossbow
        [
            Layer {
                at: (11, 10),
                rows: &[".uV", ".rB", "rB."],
            },
            Layer {
                at: (8, 4),
                rows: &["uVVVVu", "..Br..", "..rB.."],
            },
        ],
        // the handgonne
        [
            Layer {
                at: (11, 10),
                rows: &["..ux", ".xB.", "rB.."],
            },
            Layer {
                at: (10, 5),
                rows: &["..6.", ".6@6", "..ux", ".xB."],
            },
        ],
        // the sword
        [
            Layer {
                at: (11, 5),
                rows: &[
                    "..W.", ".WV.", ".WV.", ".WV.", ".WV.", "4444", ".B..", ".4..",
                ],
            },
            Layer {
                at: (11, 1),
                rows: &[
                    ".W..", ".WV.", ".WV.", ".WV.", ".WV.", ".WV.", "4444", ".B..",
                ],
            },
        ],
    ],
};
