//! The holdings drawn: the Court of Deeds (its great board, the works
//! ledger, the clerk and the plates), a held floor's steward's hall below,
//! a castle's hall, and the realm's banner wherever it flies.

use super::super::ink::{Img, text_width};
use super::super::kit::{self, House, Roof, Wall};
use super::super::light::Light;
use super::{NATIVE_H, NATIVE_W, TILE, fire};
use crate::realm::Spoil;
use crate::rules::holdings::{
    self, BAYS, BOARD, CASTLE_PLATE, CHEST, DESK, Deed, Family, Holding, LEDGER, Op, Plate, SLOTS,
    STEWARD_PLATE, TABLE, WALLS,
};
use crate::rules::home::{BUY_HOLD, hold_bar};
use crate::rules::{RoomKind, Run};

// ── Sprites ──────────────────────────────────────────────────────────────

/// The realm's banner: a gold tower on the deep blue, swallow-tailed. It
/// flies over everything the realm holds.
pub(super) fn banner() -> Img {
    Img::from_rows(&[
        "bbbbbbbbbbbb",
        ".0qqqqqqqq0.",
        ".0qqqqqqqq0.",
        ".0qq6.6.6q0.",
        ".0qq66666q0.",
        ".0qqq666qq0.",
        ".0qqq6.6qq0.",
        ".0qqq666qq0.",
        ".0qq66666q0.",
        ".0qqqqqqqq0.",
        ".0qqqqqqqq0.",
        ".0qqq..qqq0.",
        ".0qq....qq0.",
        ".0q......q0.",
    ])
}

/// A small banner on a pole, for the map.
fn pennon() -> Img {
    Img::from_rows(&[
        "h6666..",
        "hqq6qq.",
        "hq666q.",
        "hqq6qq.",
        "hqq.qq.",
        "h......",
        "h......",
        "h......",
    ])
}

/// A room's picture: its furniture, side-on, 16 by 14.
pub(super) fn room_art(id: &str) -> Img {
    Img::from_rows(match id {
        // A cask on its cradle, tapped.
        "taproom" => &[
            "................",
            "...rrrrrrrrrr...",
            "..rRRRRRRRRRRr..",
            ".rRoRRRRRRRRoRr.",
            ".hhhhhhhhhhhhhh.",
            ".rRoRRRRRRRRoRr.",
            ".rRoRRRRRRRRoRr.",
            ".rRoRRRRRRRRoRr.",
            ".hhhhhhhhhhhhhh.",
            ".rRoRRRRkkRRoRr.",
            "..rRRRRRRkRRRr..",
            "...rrrrrrrrrr...",
            "..BB...TT...BB..",
            "..BB...TT...BB..",
        ],
        // A copper vat with its fire under, and a steam.
        "brewery" => &[
            ".....hh.........",
            "......hh........",
            "....aoooooa.....",
            "...aO6OOOOOa....",
            "...oOOOOOOOo....",
            "...oOOOOOOOo..r.",
            "...aoOOOOOoa..r.",
            "....aooooo...rRr",
            "....BBBBBBB..rRr",
            "...B7p@p7pB..rRr",
            "...BppppppB..rRr",
            "...BBBBBBBB..rRr",
            "...g.......g....",
            "..gg.......gg...",
        ],
        // A bed with a blue coverlet and a pillow.
        "inn" => &[
            "................",
            "................",
            "rr..............",
            "rR..............",
            "rR..............",
            "rRWV............",
            "rRVWqqqqqqqqqqr.",
            "rRqqqQqqqqQqqqR.",
            "rRQqqqqQqqqqQqR.",
            "rRrrrrrrrrrrrrR.",
            "rRBBBBBBBBBBBBR.",
            "rR............R.",
            "rR............R.",
            "................",
        ],
        // An anvil on its block, the hammer resting, an ingot glowing.
        "smithy" => &[
            "..........hh....",
            ".........hHh....",
            "..........b.....",
            "..........b.....",
            ".GGGGGGGGGb.....",
            "jjGGGGGGGGGGj...",
            "...jGGGGGGj.....",
            "....jGGGGj......",
            ".....jGGj...p7..",
            "....jjGGjj..7@p.",
            "...rrrrrrrr.....",
            "...rRRRRRRr.....",
            "...rRRRRRRr.....",
            "...rrrrrrrr.....",
        ],
        // A quiver of fletched shafts, a bow on the peg.
        "fletcher" => &[
            "..7.9.7.........",
            "..7.9.7.....rr..",
            "..h.h.h....r..r.",
            "..h.h.h...r....h",
            ".rRRRRRr..r....h",
            ".rRRRRRr..r....h",
            ".rRoooRr..r....h",
            ".rRRRRRr..r....h",
            ".rRoooRr..r....h",
            ".rRRRRRr..r....h",
            ".rRRRRRr...r..r.",
            ".rrrrrrr....rr..",
            "................",
            "................",
        ],
        // A mail shirt on its stand, a kite shield beside.
        "armory" => &[
            "...hhhhh........",
            "..hiHiHih.......",
            ".hiHiHiHih.QQQQ.",
            ".hHiHiHiHh.Q66Q.",
            "..iHiHiHi..Q66Q.",
            "..HiHiHiH..QQQQ.",
            "..iHiHiHi..QQQQ.",
            "..HiHiHiH...QQ..",
            "..iHiHiHi...QQ..",
            "....rr.......Q..",
            "....rr..........",
            "....rr..........",
            "..rrrrrr........",
            "..rrrrrr........",
        ],
        // An altar with two candles and the Grail's cross.
        "shrine" => &[
            ".......6........",
            "......666.......",
            ".......6........",
            "..w....6....w...",
            "..6....6....6...",
            "..h...666...h...",
            ".JJJJJJJJJJJJJ..",
            ".JhhhhhhhhhhhJ..",
            ".Jh7777777777hJ.",
            ".Jh7666666667hJ.",
            ".Jh7777777777hJ.",
            ".JhhhhhhhhhhhJ..",
            ".JJ.........JJ..",
            ".JJ.........JJ..",
        ],
        // A cot, a basin and the healer's red cross hung above.
        "infirmary" => &[
            "......777.......",
            "......777.......",
            "....7777777.....",
            "....7777777.....",
            "......777.......",
            "......777.......",
            "................",
            "rr..........vv..",
            "rRWVVVVVVVr.vv..",
            "rRVVVVVVVVr.hh..",
            "rRrrrrrrrrr.hh..",
            "rR.......Rr.hh..",
            "rR.......Rr.hh..",
            "................",
        ],
        // The bell in its frame, its rope hanging.
        "belltower" => &[
            "rrrrrrrrrrrrrrrr",
            "rRRRRRRRRRRRRRRr",
            ".r.....oo.....r.",
            ".r....oOOo....r.",
            ".r...oO6OOo...r.",
            ".r...OOOOOO...r.",
            ".r..oOOOOOOo..r.",
            ".r..OOOOOOOO..r.",
            ".r.aOOOOOOOOa.r.",
            ".r......O.....r.",
            ".r......h.....r.",
            ".r......h.....r.",
            ".r......h.....r.",
            "rrr.....P....rrr",
        ],
        // A striped awning over a trestle of goods.
        "stall" => &[
            "7W7W7W7W7W7W7W7W",
            "7W7W7W7W7W7W7W7W",
            ".7.W.7.W.7.W.7..",
            ".r............r.",
            ".r............r.",
            ".r..Ry.66.Ry..r.",
            ".r.RRyy66RRyy.r.",
            ".rrrrrrrrrrrrrr.",
            ".rRRRRRRRRRRRRr.",
            ".r............r.",
            ".r............r.",
            ".r............r.",
            ".r............r.",
            "................",
        ],
        // Stacked coin and the open ledger on a counting board.
        "counting" => &[
            "................",
            "........6.......",
            "...66..666......",
            "..6666.6a6..TTT.",
            "..6a66.666.TT9T.",
            "..6666.6a6.T9TT.",
            "..66a6.666.TTTT.",
            "..6666.6a6.TTTT.",
            "BBBBBBBBBBBBBBBB",
            "BrrrrrrrrrrrrrrB",
            ".B............B.",
            ".B............B.",
            ".B............B.",
            ".B............B.",
        ],
        // Crates and grain sacks, stacked.
        "warehouse" => &[
            "................",
            "................",
            "........PPP.....",
            ".......P$$$P....",
            "rrrrrr.P$$$$P...",
            "rRrrRr.P$$$$P...",
            "rrRRrr.P$$$$P...",
            "rRrrRr..PPPP....",
            "rrrrrrrrrrrrr...",
            "rRrrRrrRrrRrr...",
            "rrRRrrrrRRrrr.$.",
            "rrRRrrrrRRrr.$$$",
            "rRrrRrrRrrRr.$$$",
            "rrrrrrrrrrrr.PPP",
        ],
        // A bunk and the guards' spear rack.
        "barracks" => &[
            "................",
            "................",
            "rr..........h.h.",
            "rRVVVVVr....H.H.",
            "rRqqqqqr....h.h.",
            "rrrrrrrr...hhhhh",
            "rR.....r....h.h.",
            "rRVVVVVr....h.h.",
            "rRqqqqqr....h.h.",
            "rrrrrrrr....h.h.",
            "rr.....rr..rrrrr",
            "rr.....rr..rRRRr",
            "................",
            "................",
        ],
        // A dog house with its dark door, and a bowl.
        "kennels" => &[
            "................",
            "................",
            "......rr........",
            ".....rRRr.......",
            "....rRRRRr......",
            "...rRRRRRRr.....",
            "..rRRRRRRRRr....",
            "...JJJJJJJJ.....",
            "...JhhkkhhJ.....",
            "...JhkkkkhJ.....",
            "...JhkkkkhJ..oo.",
            "...JhkkkkhJ.oOOo",
            "...JJJJJJJJ.oooo",
            "................",
        ],
        // A straw butt with its painted rings, an arrow in the gold.
        "range" => &[
            ".....PPPPPP.....",
            "....P777777P....",
            "...P7TTTTTT7P...",
            "...P7T7777T7P..h",
            "...P7T7667T7Phh.",
            "...P7T7667Th....",
            "...P7T7777T7P...",
            "...P7TTTTTT7P...",
            "....P777777P....",
            ".....PPPPPP.....",
            "......r..r......",
            ".....r....r.....",
            "....r......r....",
            "................",
        ],
        _ => &["................"],
    })
}

/// The raiders' banner: a torn black rag with a red claw, flown where
/// they hold a holding.
pub(super) fn raider_banner() -> Img {
    Img::from_rows(&[
        "nnnnnnnnnnnn",
        ".kkkkkkkkkk.",
        ".kk7kk7kk7k.",
        ".kk7kk7kk7k.",
        ".kkk7k7k7kk.",
        ".kkkk777kkk.",
        ".kkkkk7kkkk.",
        ".kkkkkkkkk..",
        ".kk.kkkkkk..",
        ".k...kkk.k..",
        "......k.....",
    ])
}

/// A small arrow, up for a neighbour's help and down for its harm.
fn pip(up: bool) -> Img {
    if up {
        Img::from_rows(&["..y..", ".yyy.", "yyyyy", "..y..", "..y.."])
    } else {
        Img::from_rows(&["..7..", "..7..", "77777", ".777.", "..7.."])
    }
}

/// What a bay's room feels from its neighbours, summed: its rent change.
fn felt(held: &Holding, i: usize) -> i32 {
    held.rooms
        .get(i)
        .filter(|r| r.standing())
        .map_or(0, |r| holdings::neighbours(&held.rooms, i, &r.id).iter().map(|&(n, _)| n).sum())
}

/// Twice the size, for the bays of a steward's hall.
fn doubled(im: &Img) -> Img {
    let mut out = Img::new(im.w * 2, im.h * 2);
    for y in 0..out.h {
        for x in 0..out.w {
            if let Some(c) = im.get(x / 2, y / 2) {
                out.set(x, y, c);
            }
        }
    }
    out
}

/// A steward, standing, 12 by 13.
pub(super) fn steward_art(id: &str) -> Img {
    Img::from_rows(match id {
        // Wynn the greedy: a fat red coat, a fatter purse.
        "wynn" => &[
            "...IIIII....",
            "...IcccI....",
            "...cKcKc....",
            "...ccpcc....",
            "....ccc.....",
            "..7777777...",
            ".777777777..",
            ".7777O77766.",
            ".77777777aa.",
            ".777777777..",
            "..7777777...",
            "..BB...BB...",
            "..nn...nn...",
        ],
        // Brother Aled: a monk's brown and a tonsure.
        "aled" => &[
            "...bbbbb....",
            "..bcccccb...",
            "...cKcKc....",
            "...ccccc....",
            "....ccc.....",
            "...PPPPP....",
            "..PPPPPPP...",
            "..PPp6pPP...",
            "..PPPPPPP...",
            "..PPPPPPP...",
            "..PPPPPPP...",
            ".PPPPPPPPP..",
            "..nn...nn...",
        ],
        // Gwen the brave: helm, tabard and spear.
        "gwen" => &[
            "..........H.",
            "...hhhh...H.",
            "..hiiiih..r.",
            "..hcKcKh..r.",
            "...cccc...r.",
            "..QQQQQQ..r.",
            ".JQQQ6QQJ.r.",
            ".JQQ666QJcr.",
            ".cQQQ6QQ..r.",
            "..QQQQQQ..r.",
            "..JJ..JJ..r.",
            "..JJ..JJ....",
            "..nn..nn....",
        ],
        // Old Hob the sot: a green jerkin, a red nose, a tankard.
        "hob" => &[
            "...EEEEE....",
            "..EEEEEEE...",
            "...cKcKc....",
            "...cc7cc....",
            "....ccc.....",
            "..mmmmmm.hh.",
            ".mmmmmmmcOO.",
            ".mmmmmmm.OO.",
            ".mmmBBmmm...",
            "..mmmmmm....",
            "..mmmmmm....",
            "..bb..bb....",
            "..nn..nn....",
        ],
        // Mistress Vell the shrewd: a dark blue gown and her ledger.
        "vell" => &[
            "...kkkkk....",
            "..kkcccck...",
            "..kcKcKck...",
            "..kccccck...",
            "....ccc.....",
            "...qqqqq....",
            "..qqqqqqqTT.",
            "..qqQqqqcT9.",
            "..qqqqqqqTT.",
            "..qqqqqqq...",
            ".qqqqqqqqq..",
            ".qqqqqqqqq..",
            "..nn...nn...",
        ],
        _ => &["............"],
    })
}

/// The Court's clerk behind his desk: the royal red, a quill.
fn clerk(tick: u32) -> Img {
    let mut im = Img::from_rows(&[
        "...BBBBB....",
        "..BBBBBBB...",
        "...cKcKc....",
        "...ccccc..W.",
        "....ccc..W..",
        "..RRRRRRcW..",
        ".RRR6RRRR...",
        ".RRRRRRRR...",
        ".RRRRRRRR...",
    ]);
    // The quill scratches.
    if (tick / 10).is_multiple_of(2) {
        im.put(9, 3, 'V');
    }
    im
}

/// The Court of Deeds on the kingdom's map: a dressed-stone counting house,
/// its sign a sealed deed, the realm's banner on the gable.
pub fn land_office_building() -> Img {
    let base = kit::house(&House {
        w: 48,
        h: 38,
        roof_h: 16,
        wall: Wall::Stone,
        roof: Roof::Slate,
        door_glow: false,
        windows: 2,
        lit: true,
        chimney: false,
    });
    let mut im = Img::new(base.w + 6, base.h + 8);
    im.stamp(&base, 0, 8);
    im.stamp(&pennon(), base.w / 2 - 1, 0);
    // The hanging sign: a scroll with a red seal.
    let sign = Img::from_rows(&[
        "r........r",
        "r........r",
        "TTTTTTTTTT",
        "T99999999T",
        "T9hhhh9779",
        "T9hhh99779",
        "TTTTTTTTTT",
    ]);
    im.stamp(&sign, base.w - 6, 26);
    im
}

/// A plate's mark, 5 wide.
fn plate_mark(plate: Plate) -> Img {
    Img::from_rows(match plate {
        Plate::Prev => &["..O..", ".OO..", "OOOOO", ".OO..", "..O.."],
        Plate::Next => &["..O..", "..OO.", "OOOOO", "..OO.", "..O.."],
        Plate::Claim => &[".OOO.", "OOOOO", "OO.OO", "OOOOO", ".OOO."],
        Plate::Fief => &["O.O.O", "OOOOO", "OOOOO", ".....", "OOOOO"],
        Plate::Steward => &["OOO..", "O.OOO", "OOO.O", ".....", "....."],
        Plate::Charter => &["..O..", "OOOOO", ".OOO.", "O...O", "....."],
        Plate::Build(Family::Tavern) => &["OOOO.", "O..OO", "O..O.", "O..OO", "OOOO."],
        Plate::Build(Family::Forge) => &["OOOO.", "OOOOO", "..O..", "..O..", "..O.."],
        Plate::Build(Family::Chapel) => &["..O..", "OOOOO", "..O..", "..O..", "..O.."],
        Plate::Build(Family::Trade) => &[".OOO.", "OO.OO", "O.O.O", "OO.OO", ".OOO."],
        Plate::Build(Family::Barracks) => &["OOOOO", "O.O.O", "OOOOO", ".OOO.", "..O.."],
        Plate::Collect => &["OOOOO", "O...O", "OOOOO", "O.O.O", "OOOOO"],
    })
}

/// A star, lit or not.
fn star(lit: bool) -> Img {
    let im = Img::from_rows(&[
        "...O...",
        "..OOO..",
        "OOOOOOO",
        ".OOOOO.",
        "..OOO..",
        ".OO.OO.",
        "OO...OO",
    ]);
    im.recolor(&[('O', if lit { '6' } else { 'g' })])
}

/// The ledger's little pictures, 7 by 7, one per work of angelX's.
fn op_icon(op: Op) -> Img {
    Img::from_rows(match op {
        Op::Cache => &["..$$...", ".$$$$..", "$$$$$$.", "$$$$$$.", "$$$$$$.", ".PPPP..", "......."],
        Op::Tokens => &[".666...", "6aaa6..", "6a6a6..", "6aaa6..", ".666...", "......."],
        Op::Tests => &["JJJJJJJ", "JhJhJhJ", "JJJJJJJ", "JhJhJhJ", "JJJJJJJ", "......."],
        Op::Commits => &["......h", ".....h.", "rrrrh..", "rRRhr..", "rRRRr..", "rrrrr..", "......."],
        Op::Compactions => &["TTTTTT.", "T9999T.", "T9hh9T.", "T9999T.", "TTTTTT.", "......."],
        Op::Subagents => &["..H....", ".hih...", ".cKc...", "QQQQQ..", "QQ6QQ..", ".J.J...", "......."],
        Op::Loops => &["..6....", ".666...", "..h....", "..h....", "..h....", ".hhh...", "......."],
        Op::Errors => &["...7...", "..777..", "..7.7..", ".7.7.7.", ".7...7.", "7777777", "......."],
    })
}

// ── The Court of Deeds ───────────────────────────────────────────────────

fn rect_px((c, r, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

/// The Court at rest: dressed flags (painted by the world), the board's
/// timber, the ledger wall, the clerk's desk, the strongbox, the King's
/// red runner from the door, and lamps.
pub(super) fn office_scenery(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    // The runner, from the door to the desk.
    for y in 6 * TILE..13 * TILE {
        for x in 11 * TILE + 3..13 * TILE - 3 {
            let edge = x == 11 * TILE + 3 || x == 13 * TILE - 4;
            cv.put(x, y, if edge { '6' } else if (x + y) % 7 == 0 { 'p' } else { '8' });
        }
    }
    // The great board's timber frame.
    let (bx, by, bw, bh) = rect_px(BOARD);
    cv.rect(bx, by, bw, bh, 'n');
    cv.frame(bx, by, bw, bh, 'B');
    cv.frame(bx + 1, by + 1, bw - 2, bh - 2, 'r');
    // The ledger wall: a tall board of its own.
    let (lx, ly, lw, lh) = rect_px(LEDGER);
    cv.rect(lx, ly, lw, lh, 'n');
    cv.frame(lx, ly, lw, lh, 'B');
    cv.frame(lx + 1, ly + 1, lw - 2, lh - 2, 'r');
    // The clerk's desk.
    let (dx, dy, dw, dh) = rect_px(DESK);
    cv.rect(dx, dy + 3, dw, dh - 3, 'r');
    cv.rect(dx, dy + 3, dw, 2, 'R');
    cv.line(dx, dy + dh - 1, dx + dw - 1, dy + dh - 1, 'B');
    for k in 0..3 {
        cv.rect(dx + 10 + k * 14, dy + 1, 8, 3, if k == 1 { '9' } else { 'T' });
    }
    // The strongbox, banded.
    let (cx, cy, cw, ch) = rect_px(CHEST);
    let chest = kit::chest(false);
    for y in [cy + 2, cy + ch - 12] {
        cv.stamp(&chest, cx + (cw - chest.w) / 2, y);
    }
    // Banners on the east wall, lamps by the door.
    let b = banner();
    cv.stamp(&b, 22 * TILE + 2, 2 * TILE);
    cv.stamp(&b, 22 * TILE + 2, 9 * TILE);
    for x in [9 * TILE + 4, 14 * TILE + 10] {
        let lamp = kit::torch(tick / 4, x as u32);
        cv.stamp(&lamp, x - lamp.w / 2, 13 * TILE - lamp.h + 2);
        lights.push(fire(x, 13 * TILE - lamp.h + 4, 54.0, 0.36));
    }
    lights.push(fire(12 * TILE, 5 * TILE, 120.0, 0.3));
    lights.push(fire(3 * TILE, 8 * TILE, 70.0, 0.25));
}

/// Where a plate's tag goes and what it says.
fn plate_tag(plate: Plate) -> &'static str {
    match plate {
        Plate::Prev | Plate::Next => "",
        Plate::Claim => "CLAIM",
        Plate::Fief => "FIEF",
        Plate::Steward => "STEWARD",
        Plate::Charter => "CHARTER",
        Plate::Build(f) => f.name(),
        Plate::Collect => "RENTS",
    }
}

/// A plate: open and lit (a knight on it), open, or shut (dim).
fn plate(cv: &mut Img, rect: (i32, i32, i32, i32), lit: bool, open: bool, mark: &Img) {
    let (x, y, w, h) = rect_px(rect);
    let (x, y, w, h) = (x + 1, y + 1, w - 2, h - 2);
    cv.rect(x, y, w, h, 'X');
    let (frame, ink) = match (open, lit) {
        (true, true) => ('5', '6'),
        (true, false) => ('o', 'O'),
        (false, _) => ('g', 'j'),
    };
    cv.frame(x, y, w, h, frame);
    let m = mark.recolor(&[('O', ink)]);
    cv.stamp(&m, x + (w - m.w) / 2, y + (h - m.h) / 2);
}

fn on(run: &Run, rect: (i32, i32, i32, i32)) -> bool {
    run.players.values().any(|h| {
        h.hp > 0 && {
            let (c, r) = ((h.x / 2.0).floor() as i32, (h.y / 2.0).floor() as i32);
            c >= rect.0 && c < rect.0 + rect.2 && r >= rect.1 && r < rect.1 + rect.3
        }
    })
}

/// The Court's live things: the board, the ledger, the clerk, the plates.
pub(super) fn office(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let h = &run.home.holdings;
    let deed = h.picked();
    board(cv, run, deed);
    ledger(cv, h);
    let c = clerk(tick);
    let (dx, dy, dw, _) = rect_px(DESK);
    cv.stamp(&c, dx + (dw - c.w) / 2, dy - c.h + 4);
    // The board's holding's steward waits by the desk with its accounts.
    if let Some(s) = h.holding(&deed.key()).and_then(|w| w.steward.as_deref()) {
        let im = steward_art(s);
        cv.stamp(&im, dx + dw + 4, dy - im.h + 12);
    }
    // Coin on the strongbox when there is rent to collect.
    let waiting: u32 = h.owned.iter().map(|w| w.coffer).sum();
    if waiting > 0 {
        let (cx, cy, _, _) = rect_px(CHEST);
        for k in 0..(waiting / 100).clamp(1, 6) as i32 {
            cv.rect(cx + 4 + k * 4, cy - 2 - (k % 2), 3, 2, '6');
        }
    }
    for (p, rect) in Plate::all() {
        let open = holdings::plate_open(run, p).is_ok();
        plate(cv, rect, on(run, rect), open, &plate_mark(p));
        let tag = plate_tag(p);
        if !tag.is_empty() {
            let (x, y, w, h) = rect_px(rect);
            let tw = text_width(tag);
            cv.text(x + (w - tw) / 2, y + h + 1, tag, if open { 'h' } else { 'G' });
        }
    }
}

/// The great board: the deed shown, its stars and state, and its ten bays
/// side-on.
fn board(cv: &mut Img, run: &Run, deed: Deed) {
    let h = &run.home.holdings;
    let (bx, by, bw, _) = rect_px(BOARD);
    let held = h.holding(&deed.key());
    let all = holdings::catalogue();
    let at = all.iter().position(|&d| d == deed).unwrap_or(0) + 1;
    let x0 = bx + 6;
    // The arrows' count at the left, the name, the stars at the right.
    let count = format!("{at}/{}", all.len());
    cv.text(x0, by + 4, &count, 'G');
    let name = deed.name().to_uppercase();
    cv.text(x0 + text_width(&count) + 8, by + 4, &name, '9');
    let stars = held.map_or(0, |w| w.stars);
    for k in 0..5u8 {
        cv.stamp(&star(k < stars), bx + bw - 6 - (5 - i32::from(k)) * 9, by + 4);
    }
    // The King's favour, left of the stars: his crown and the number.
    let favour = format!("{:+}", h.favour);
    let fx = bx + bw - 6 - 5 * 9 - 10 - text_width(&favour);
    let crown = Img::from_rows(&["6.6.6", "66666", "66666", ".....", "66666"]);
    cv.stamp(&crown, fx - 8, by + 5);
    let ink = if h.favour >= holdings::ROYAL { '6' } else if h.favour <= -holdings::ROYAL { '7' } else { 'h' };
    cv.text(fx, by + 4, &favour, ink);
    // The second line: pictures and numbers.
    let y = by + 15;
    let mut x = x0;
    let mut item = |cv: &mut Img, icon: &Img, text: &str, ink: char| {
        cv.stamp(icon, x, y);
        cv.text(x + icon.w + 3, y, text, ink);
        x += icon.w + 3 + text_width(text) + 10;
    };
    match held {
        Some(w) => {
            if w.raid > 0 {
                let claw = Img::from_rows(&["7.7.7..", "7.7.7..", ".777...", "..7....", ".......", ".......", "......."]);
                item(cv, &claw, "RAIDED", '7');
            }
            let wall = Img::from_rows(&["JJJJJJJ", "JhJhJhJ", "JJJJJJJ", "JhJhJhJ", "JJJJJJJ", ".......", "......."]);
            let walls = format!("{}/{WALLS}", w.walls);
            item(cv, &wall, &walls, if w.walls == 0 { '7' } else { 'i' });
            let coin = Img::from_rows(&[".666...", "6aaa6..", "6a6a6..", "6aaa6..", ".666...", ".......", "......."]);
            item(cv, &coin, &format!("{}G", w.coffer), '6');
            let rooms = Img::from_rows(&["rrrrrr.", "r.r..r.", "rrrrrr.", "r..r.r.", "rrrrrr.", ".......", "......."]);
            item(cv, &rooms, &format!("{}/{SLOTS}", w.standing()), 'i');
            match w.steward.as_deref().and_then(holdings::steward) {
                Some(s) => {
                    let face = Img::from_rows(&["kkkk...", "kcck...", "cKcK...", "cccc...", ".cc....", ".......", "......."]);
                    item(cv, &face, &s.way, 'h');
                }
                None => item(cv, &Img::new(0, 7), "NO STEWARD", 'G'),
            }
            if w.fief {
                let crown = Img::from_rows(&["6.6.6..", "66666..", "66666..", ".......", ".......", ".......", "......."]);
                item(cv, &crown, "FIEF", 'h');
            }
        }
        None => match h.offered(deed) {
            Ok(()) => {
                let seal = Img::from_rows(&[".777...", "77777..", "77.77..", "77777..", ".777...", ".......", "......."]);
                item(cv, &seal, &format!("CLAIM {}G", deed.price(false)), '6');
                let crown = Img::from_rows(&["6.6.6..", "66666..", "66666..", ".......", "66666..", ".......", "......."]);
                item(cv, &crown, &format!("FIEF {}G", deed.price(true)), 'O');
            }
            Err(why) => item(cv, &Img::new(0, 7), why, 'G'),
        },
    }
    if let Some(word) = crate::rules::crown::favour_word(h) {
        cv.text(bx + bw - 6 - text_width(word), y, word, if h.favour > 0 { '6' } else { '7' });
    }
    cross_section(cv, x0 - 2, by + 26, deed, held, tick_of(run));
}

fn tick_of(run: &Run) -> u32 {
    run.tick as u32
}

/// A holding side-on: the stair (a floor) or the keep's tower (a castle)
/// at the left, then its ten bays as built, building or empty.
pub(super) fn cross_section(cv: &mut Img, x: i32, y: i32, deed: Deed, held: Option<&Holding>, tick: u32) {
    // The way in: a stair shaft or a tower.
    match deed {
        Deed::Floor(_) => {
            cv.rect(x, y, 12, 22, 'k');
            cv.frame(x, y, 12, 22, 'g');
            for k in 0..5 {
                cv.line(x + 2 + k * 2, y + 20 - k * 4, x + 5 + k * 2, y + 20 - k * 4, 'J');
            }
        }
        Deed::Castle(_) => {
            cv.rect(x, y, 12, 22, 'G');
            for k in 0..3 {
                cv.rect(x + k * 4, y - 3, 3, 3, 'G');
            }
            cv.rect(x + 4, y + 13, 4, 9, 'k');
            cv.put(x + 5, y + 6, '6');
        }
    }
    for i in 0..SLOTS as i32 {
        let bx = x + 14 + i * 30;
        cv.rect(bx, y, 28, 22, 'k');
        cv.frame(bx, y, 28, 22, 'g');
        cv.line(bx, y + 21, bx + 27, y + 21, 'j');
        let Some(built) = held.and_then(|w| w.rooms.get(i as usize)) else {
            // An empty bay: just the dark.
            cv.put(bx + 13, y + 10, 'g');
            cv.put(bx + 14, y + 10, 'g');
            continue;
        };
        let art = room_art(&built.id);
        if built.standing() {
            cv.stamp(&art, bx + 6, y + 4);
            let near = held.map_or(0, |w| felt(w, i as usize));
            if near != 0 {
                cv.stamp(&pip(near > 0), bx + 22, y + 2);
            }
        } else {
            // Going up: the room's shape in grey under scaffolding, and the
            // courses laid so far.
            let ghost = art.recolor(&[
                ('r', 'g'), ('R', 'g'), ('o', 'g'), ('O', 'g'), ('h', 'g'), ('H', 'g'), ('i', 'g'),
                ('6', 'g'), ('7', 'g'), ('q', 'g'), ('Q', 'g'), ('T', 'g'), ('9', 'g'), ('W', 'g'),
                ('V', 'g'), ('B', 'g'), ('G', 'g'), ('J', 'g'), ('j', 'g'), ('P', 'g'), ('$', 'g'),
                ('p', 'g'), ('@', 'g'), ('a', 'g'), ('c', 'g'), ('w', 'g'), ('y', 'g'), ('b', 'g'),
                ('k', 'g'), ('n', 'g'), ('8', 'g'),
            ]);
            cv.stamp(&ghost, bx + 6, y + 4);
            for k in [bx + 3, bx + 24] {
                cv.line(k, y + 2, k, y + 20, 'r');
            }
            cv.line(bx + 3, y + 8, bx + 24, y + 8, 'r');
            let done = (built.labour * 22 / holdings::ROOM_LABOUR) as i32;
            cv.rect(bx + 3, y + 18, done, 2, '6');
            if (tick / 8).is_multiple_of(2) {
                cv.put(bx + 3 + done, y + 17, 'P');
            }
        }
    }
}

/// The works ledger on the west wall: what angelX's own work does for the
/// holdings, one short label each, with the count.
fn ledger(cv: &mut Img, h: &holdings::Holdings) {
    let (lx, ly, lw, _) = rect_px(LEDGER);
    let fed = |op: Op| h.feed.get(op.word()).copied().unwrap_or(0);
    for (k, op) in Op::ALL.into_iter().enumerate() {
        let y = ly + 4 + k as i32 * 17;
        let value = match op {
            Op::Cache => h.stores,
            Op::Compactions => h.scrolls,
            Op::Subagents => h.retainers,
            Op::Loops => h.patrols,
            Op::Errors => h.unrest,
            _ => fed(op),
        };
        cv.stamp(&op_icon(op), lx + 3, y);
        let hot = op == Op::Errors && value >= holdings::RESTLESS;
        cv.text(lx + 12, y, op.word(), if hot { '7' } else { 'O' });
        let n = value.to_string();
        cv.text(lx + lw - 3 - text_width(&n), y, &n, if hot { '7' } else { '9' });
        cv.text(lx + 12, y + 8, op.source(), 'G');
    }
}

/// What the plate under the focused knight does, over their head.
pub(super) fn office_boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or(1);
    let Some((p, _)) = holdings::plate_under(run, id) else {
        return;
    };
    let hero = &run.players[&id];
    let h = &run.home.holdings;
    let deed = h.picked();
    let name = deed.name().to_uppercase();
    let title = match p {
        Plate::Prev => "PREVIOUS DEED".to_string(),
        Plate::Next => "NEXT DEED".to_string(),
        Plate::Claim => format!("CLAIM {name}"),
        Plate::Fief => format!("FIEF {name}"),
        Plate::Steward => match holdings::next_steward(h, &deed.key()) {
            Some(s) => format!("{} {}", s.name.to_uppercase(), s.does),
            None => "STEWARDS".to_string(),
        },
        Plate::Charter => match h.holding(&deed.key()).map(Holding::charter) {
            Some(Ok((star, _))) => format!("CHARTER {star} STARS"),
            _ => "CHARTER".to_string(),
        },
        Plate::Build(f) => match h.holding(&deed.key()).and_then(|w| w.next_of(f)) {
            Some(def) => format!("BUILD {}", def.name.to_uppercase()),
            None => format!("BUILD {}", f.name()),
        },
        Plate::Collect => "COLLECT RENTS".to_string(),
    };
    let last = match holdings::plate_open(run, p) {
        Ok(_) if p.tapped() => ("E: TURN".to_string(), 'O'),
        Ok((_, price)) => {
            let what = match p {
                Plate::Collect => format!("HOLD E: +{price}G"),
                _ if price > 0 => format!("HOLD E: {price}G"),
                _ => "HOLD E".to_string(),
            };
            (format!("{what} {}", hold_bar(hero.buying, BUY_HOLD)), 'O')
        }
        Err(why) => (why.to_string(), 'h'),
    };
    let mut lines = vec![(title, '9'), last];
    // Building: what the next bay's neighbours make of it.
    if let Plate::Build(f) = p
        && let Some(w) = h.holding(&deed.key())
        && let Some(def) = w.next_of(f)
    {
        for (n, word) in w.preview(def) {
            lines.push((format!("{word} {n:+}"), if n > 0 { 'y' } else { '7' }));
        }
    }
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 8;
    let bh = lines.len() as i32 * 9 + 5;
    // On the south wall, under every plate and its name, clear of the tray.
    let x = ((NATIVE_W - w) / 2 - 24).max(2);
    let y = NATIVE_H - bh - 1;
    cv.rect(x, y, w, bh, 'K');
    cv.frame(x, y, w, bh, 'B');
    for (i, (text, ink)) in lines.iter().enumerate() {
        let tw = text_width(text);
        cv.text(x + (w - tw) / 2, y + 3 + i as i32 * 9, text, *ink);
    }
}

// ── A held floor's steward's hall ────────────────────────────────────────

/// The steward's hall below: the realm's banners, the bays as built, the
/// steward at the table with the coffer, each bay's boon on its plate.
pub(super) fn hall(cv: &mut Img, run: &Run) {
    let Some(held) = holdings::held_floor(&run.home.holdings, run.dungeon.pack) else {
        return;
    };
    let room = run.room();
    let b = if held.raid > 0 { raider_banner() } else { banner() };
    let w = room.cols as i32 * TILE;
    for x in [3 * TILE + 2, w - 4 * TILE + 2] {
        cv.stamp(&b, x, 1);
    }
    for i in 0..SLOTS {
        let (solid, plate_rect) = holdings::bay(i);
        let (sx, sy, sw, sh) = rect_px(solid);
        let built = held.rooms.get(i);
        let Some(built) = built else {
            // An empty bay: a dim outline on the floor.
            let (c, r) = BAYS[i];
            let (x, y) = (c * TILE + 2, if r < 5 { r * TILE + 2 } else { (r + 1) * TILE + 2 });
            for k in (0..44).step_by(4) {
                cv.put(x + k, y, 'g');
                cv.put(x + k, y + 27, 'g');
            }
            continue;
        };
        let def = holdings::room_def(&built.id);
        let art = doubled(&room_art(&built.id));
        if built.standing() {
            cv.stamp(&art, sx + (sw - art.w) / 2, sy + sh - art.h);
        } else {
            let ghost = art.recolor(&[('r', 'g'), ('R', 'g'), ('O', 'g'), ('o', 'g'), ('6', 'g'), ('h', 'g')]);
            cv.stamp(&ghost, sx + (sw - art.w) / 2, sy + sh - art.h);
            cv.line(sx + 2, sy, sx + 2, sy + sh - 1, 'r');
            cv.line(sx + sw - 3, sy, sx + sw - 3, sy + sh - 1, 'r');
            cv.line(sx + 2, sy + 6, sx + sw - 3, sy + 6, 'r');
        }
        let used = run.holding_used & (1 << i) != 0;
        let open = built.standing() && !used;
        let mark = Img::from_rows(&["..O..", ".OOO.", "OOOOO", ".OOO.", "..O.."]);
        plate(cv, plate_rect, on(run, plate_rect), open, &mark);
        let tag = match (def, built.standing(), used) {
            (_, false, _) => "RISING",
            (_, _, true) => "USED",
            (Some(d), _, _) => d.says,
            (None, _, _) => "",
        };
        let (px, py, pw, ph) = rect_px(plate_rect);
        let tw = text_width(tag);
        let ty = if BAYS[i].1 < 5 { py + ph + 1 } else { py - 9 };
        cv.rect(px + (pw - tw) / 2 - 1, ty - 1, tw + 2, 9, 'K');
        cv.text(px + (pw - tw) / 2, ty, tag, if open { 'O' } else { 'G' });
        // A neighbour's help or harm, over the furniture's corner.
        let near = felt(held, i);
        if near != 0 {
            cv.stamp(&pip(near > 0), sx + sw - 7, sy + 1);
        }
    }
    // The steward behind the table, the coffer at its end.
    let (tx, ty, tw, _) = rect_px(TABLE);
    if let Some(s) = held.steward.as_deref() {
        let im = steward_art(s);
        cv.stamp(&im, tx + tw / 2 - im.w / 2 + 4, ty - im.h + 7);
    }
    cv.rect(tx - 6, ty + 4, tw + 12, 10, 'r');
    cv.rect(tx - 6, ty + 4, tw + 12, 2, 'R');
    cv.rect(tx - 4, ty + 14, 2, 2, 'B');
    cv.rect(tx + tw + 2, ty + 14, 2, 2, 'B');
    let chest = kit::chest(held.coffer > 0);
    cv.stamp(&chest, tx - 6, ty - 3);
    let waiting: u32 = run.home.holdings.owned.iter().map(|w| w.coffer).sum();
    let open = waiting > 0;
    plate(cv, STEWARD_PLATE, on(run, STEWARD_PLATE), open, &plate_mark(Plate::Collect));
    let tag = if open { format!("RENTS {waiting}G") } else { "NO RENT".to_string() };
    let (px, py, pw, ph) = rect_px(STEWARD_PLATE);
    let w = text_width(&tag);
    cv.rect(px + (pw - w) / 2 - 1, py + ph, w + 2, 9, 'K');
    cv.text(px + (pw - w) / 2, py + ph + 1, &tag, if open { 'O' } else { 'G' });
}

/// What the bay or the steward under the focused knight gives.
pub(super) fn hall_boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let id = focus.unwrap_or(1);
    let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0) else {
        return;
    };
    let Some(held) = holdings::held_floor(&run.home.holdings, run.dungeon.pack) else {
        return;
    };
    let tile = ((hero.x / 2.0).floor() as i32, (hero.y / 2.0).floor() as i32);
    let inside = |(c, r, w, h): (i32, i32, i32, i32)| tile.0 >= c && tile.0 < c + w && tile.1 >= r && tile.1 < r + h;
    let mut plate = STEWARD_PLATE;
    let lines: Vec<(String, char)> = if inside(STEWARD_PLATE) {
        let waiting: u32 = run.home.holdings.owned.iter().map(|w| w.coffer).sum();
        let who = held.steward.as_deref().and_then(holdings::steward).map_or("THE STEWARD".to_string(), |s| s.name.to_uppercase());
        vec![
            (who, '9'),
            if waiting > 0 {
                (format!("HOLD E: +{waiting}G {}", hold_bar(hero.buying, BUY_HOLD)), 'O')
            } else {
                ("NO RENT YET".to_string(), 'h')
            },
        ]
    } else if let Some(i) = (0..held.rooms.len().min(SLOTS)).find(|&i| inside(holdings::bay(i).1)) {
        plate = holdings::bay(i).1;
        let built = &held.rooms[i];
        let Some(def) = holdings::room_def(&built.id) else { return };
        let last = if !built.standing() {
            ("STILL RISING".to_string(), 'h')
        } else if run.holding_used & (1 << i) != 0 {
            ("USED THIS FLOOR".to_string(), 'h')
        } else {
            (format!("HOLD E: {} {}", def.says, hold_bar(hero.buying, BUY_HOLD)), 'O')
        };
        let mut lines = vec![(def.name.to_uppercase(), '9'), last];
        if built.standing() {
            for (n, word) in holdings::neighbours(&held.rooms, i, def.id) {
                lines.push((format!("{word} {n:+}"), if n > 0 { 'y' } else { '7' }));
            }
        }
        lines
    } else {
        return;
    };
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 8;
    let bh = lines.len() as i32 * 9 + 5;
    // On the south wall, like the Court's: under the knight's feet and
    // clear of the way-on marker and the tray.
    let (px, _, pw, _) = rect_px(plate);
    let x = (px + pw / 2 - w / 2).clamp(24, NATIVE_W - w - 96);
    let y = NATIVE_H - bh - 1;
    cv.rect(x, y, w, bh, 'K');
    cv.frame(x, y, w, bh, 'B');
    for (i, (text, ink)) in lines.iter().enumerate() {
        let tw = text_width(text);
        cv.text(x + (w - tw) / 2, y + 3 + i as i32 * 9, text, *ink);
    }
}

/// The realm's banners in the first hall of a floor it holds.
pub(super) fn start_banners(cv: &mut Img, run: &Run) {
    let Some(held) = holdings::held_floor(&run.home.holdings, run.dungeon.pack).filter(|_| run.room().kind == RoomKind::Start) else {
        return;
    };
    let b = if held.raid > 0 { raider_banner() } else { banner() };
    let w = run.room().cols as i32 * TILE;
    for x in [4 * TILE, w - 5 * TILE] {
        cv.stamp(&b, x, 1);
    }
}

// ── A castle's hall ──────────────────────────────────────────────────────

/// A model castle's hall: its house's flags, the realm's banner once it is
/// ours, its bays on the wall as a cross-section, its steward, and the deed
/// plate (buy it, or collect).
pub(super) fn castle(cv: &mut Img, run: &Run) {
    let Some(i) = holdings::castle_here(run) else {
        return;
    };
    let deed = Deed::Castle(i);
    let house = crate::world::houses::get(crate::world::houses::HouseId(i));
    let h = &run.home.holdings;
    let held = h.holding(&deed.key());
    let flag = kit::flag(house.field, house.charge);
    for x in [20, NATIVE_W - 30] {
        cv.stamp(&flag, x, 18);
    }
    if let Some(w) = held {
        let b = if w.raid > 0 { raider_banner() } else { banner() };
        for x in [64, NATIVE_W - 76] {
            cv.stamp(&b, x, 18);
        }
    }
    // The tapestry: the castle's bays as built.
    let width = 14 + SLOTS as i32 * 30;
    let (x, y) = crate::rules::raids::TAPESTRY;
    cv.rect(x - 4, y - 6, width + 8, 32, 'K');
    cv.frame(x - 4, y - 6, width + 8, 32, 'B');
    cross_section(cv, x, y - 1, deed, held, run.tick as u32);
    if let Some(s) = held.and_then(|w| w.steward.as_deref()) {
        let im = steward_art(s);
        let (px, py, _, _) = rect_px(CASTLE_PLATE);
        cv.stamp(&im, px - 18, py - 4);
    }
    let gold = run.treasury.get(Spoil::Gold);
    let open = if held.is_some() {
        h.owned.iter().any(|w| w.coffer > 0)
    } else {
        holdings::buy_open(h, deed, false, gold, &run.marks).is_ok()
    };
    let mark = if held.is_some() { plate_mark(Plate::Collect) } else { plate_mark(Plate::Claim) };
    plate(cv, CASTLE_PLATE, on(run, CASTLE_PLATE), open, &mark);
}

/// The castle's deed plate, read.
pub(super) fn castle_boards(cv: &mut Img, run: &Run, focus: Option<u32>) {
    let Some(i) = holdings::castle_here(run) else {
        return;
    };
    let id = focus.unwrap_or(1);
    let Some(hero) = run.players.get(&id).filter(|h| h.hp > 0) else {
        return;
    };
    let deed = Deed::Castle(i);
    let h = &run.home.holdings;
    let held = h.holding(&deed.key());
    let tile = ((hero.x / 2.0).floor() as i32, (hero.y / 2.0).floor() as i32);
    let (c, r, w, hh) = CASTLE_PLATE;
    let standing = tile.0 >= c && tile.0 < c + w && tile.1 >= r && tile.1 < r + hh;
    let lines: Vec<(String, char)> = if standing {
        if held.is_some() {
            let waiting: u32 = h.owned.iter().map(|w| w.coffer).sum();
            vec![
                ("COLLECT RENTS".to_string(), '9'),
                if waiting > 0 {
                    (format!("HOLD E: +{waiting}G {}", hold_bar(hero.buying, BUY_HOLD)), 'O')
                } else {
                    ("NO RENT YET".to_string(), 'h')
                },
            ]
        } else {
            let last = match holdings::buy_open(h, deed, false, run.treasury.get(Spoil::Gold), &run.marks) {
                Ok(price) => (format!("HOLD E: {price}G {}", hold_bar(hero.buying, BUY_HOLD)), 'O'),
                Err(why) => (why.to_string(), 'h'),
            };
            vec![("CLAIM THIS CASTLE".to_string(), '9'), last]
        }
    } else if held.is_none() {
        // Walking in, the deed is for sale: say so once, quietly.
        vec![(match h.offered(deed) {
            Ok(()) => format!("FOR SALE {}G", deed.price(false)),
            Err(why) => why.to_string(),
        }, 'h')]
    } else {
        return;
    };
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 8;
    let bh = lines.len() as i32 * 9 + 5;
    let (px, py, pw, _) = rect_px(CASTLE_PLATE);
    // Beside the plate, clear of the knight on it and the way out.
    let x = (px + pw + 10).min(NATIVE_W - w - 2);
    let y = py - 2;
    cv.rect(x, y, w, bh, 'K');
    cv.frame(x, y, w, bh, 'B');
    for (i, (text, ink)) in lines.iter().enumerate() {
        let tw = text_width(text);
        cv.text(x + (w - tw) / 2, y + 3 + i as i32 * 9, text, *ink);
    }
}

/// The realm's pennon over every castle it holds, on the kingdom's map.
pub(super) fn castle_pennons(cv: &mut Img, run: &Run, origin: (i32, i32)) {
    for w in &run.home.holdings.owned {
        let Some(Deed::Castle(i)) = w.deed() else { continue };
        let (tx, ty, tw, _) = crate::render::overworld::castles::footprint(crate::world::houses::HouseId(i));
        let (x, y) = (tx * TILE + tw * TILE / 2 - origin.0, ty * TILE - 18 - origin.1);
        if x < -16 || y < -16 || x > NATIVE_W + 16 || y > NATIVE_H + 16 {
            continue;
        }
        cv.stamp(&pennon(), x, y);
    }
}

// ── The King's herald, in the kingdom ────────────────────────────────────

/// The King's herald: the royal red and gold, a feathered cap, the card in
/// his hand.
fn herald_art(tick: u32) -> Img {
    let mut im = Img::from_rows(&[
        ".......w....",
        "...RRRRw....",
        "..RRRRRR....",
        "...cKcKc....",
        "...ccccc....",
        "..7766677...",
        ".c7766677TT.",
        ".c7766677T9.",
        "..7766677TT.",
        "..7766677...",
        "..7777777...",
        "...hh.hh....",
        "...hh.hh....",
        "..nnn.nnn...",
    ]);
    // He lifts the card now and then.
    if (tick / 40).is_multiple_of(3) {
        im.put(10, 5, 'T');
    }
    im
}

/// An answer post: a timber post with a hanging board.
fn post_art(pick: usize, lit: bool) -> Img {
    let board = if lit { '6' } else if pick == 0 { 'O' } else { 'h' };
    let mut im = Img::from_rows(&[
        "....rr....",
        "rrrrrrrrrr",
        "..r....r..",
        ".BBBBBBBB.",
        ".BxxxxxxB.",
        ".BxxxxxxB.",
        ".BBBBBBBB.",
        "....rr....",
        "....rr....",
        "....rr....",
        "...rrrr...",
    ]);
    for (x, y) in [(3, 4), (4, 4), (5, 4), (6, 4), (3, 5), (4, 5), (5, 5), (6, 5)] {
        im.put(x, y, board);
    }
    // The answer's number, A or B, in the board's dark.
    im.put(if pick == 0 { 4 } else { 5 }, 4, 'K');
    im
}

/// The herald, his posts and, near him, his card: the King's choice with
/// its two answers and their prices. The rectangles it took, for the
/// plaques.
pub(super) fn herald(
    over: &mut Img,
    run: &Run,
    geo: &super::super::kingdom::Geography,
    origin: (i32, i32),
    focus: Option<u32>,
    legible: bool,
) -> Vec<(i32, i32, i32, i32)> {
    use crate::rules::crown::{self, CARDS};
    let mut taken = Vec::new();
    let Some(card) = run.home.holdings.card else {
        return taken;
    };
    let Some(((hx, hy), posts)) = crown::herald(geo) else {
        return taken;
    };
    let tick = run.tick as u32;
    let feet = |(tx, ty): (i32, i32)| (tx * TILE + TILE / 2 - origin.0, (ty + 1) * TILE - origin.1);
    let (x, y) = feet((hx, hy));
    if x < -120 || y < -120 || x > NATIVE_W + 120 || y > NATIVE_H + 120 {
        return taken;
    }
    let c = &CARDS[usize::from(card)];
    let id = focus.unwrap_or(1);
    let on = crown::post_under(run, id);
    let gold = run.treasury.get(Spoil::Gold);
    for (pick, &p) in posts.iter().enumerate() {
        let (px, py) = feet(p);
        let lit = on == Some(pick);
        let im = post_art(pick, lit);
        over.stamp(&im, px - im.w / 2, py - im.h);
        taken.push((px - im.w / 2, py - im.h, im.w, im.h));
    }
    let im = herald_art(tick);
    over.stamp(&im, x - im.w / 2, y - im.h);
    taken.push((x - im.w / 2, y - im.h, im.w, im.h));
    if !legible {
        return taken;
    }
    let hero = run.players.get(&id);
    let near = hero.is_some_and(|h| {
        (h.x - (hx as f32 + 0.5) * crate::rules::TILE_UNITS)
            .hypot(h.y - (hy as f32 + 0.5) * crate::rules::TILE_UNITS)
            < 12.0 * crate::rules::TILE_UNITS
    });
    if !near {
        // From afar, only the card's corner shows over his head.
        over.rect(x - 4, y - im.h - 9, 8, 6, 'T');
        over.frame(x - 4, y - im.h - 9, 8, 6, 'o');
        return taken;
    }
    // The card over his head: who comes, in a few words.
    let lines = [(c.title, '9'), (c.line, 'h')];
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 10;
    let bh = lines.len() as i32 * 9 + 6;
    let (cx, cy) = ((x - w / 2).clamp(2, NATIVE_W - w - 2), y - im.h - bh - 4);
    over.rect(cx, cy, w, bh, 'K');
    over.frame(cx, cy, w, bh, '6');
    over.frame(cx + 1, cy + 1, w - 2, bh - 2, 'a');
    for (i, (text, ink)) in lines.iter().enumerate() {
        over.text(cx + (w - text_width(text)) / 2, cy + 4 + i as i32 * 9, text, *ink);
    }
    taken.push((cx, cy, w, bh));
    // Each answer under its post: its words and price, and the hold.
    for (pick, &p) in posts.iter().enumerate() {
        let choice = &c.picks[pick];
        let (px, py) = feet(p);
        let open = crown::open(choice, gold).is_ok();
        let words = if choice.gold > 0 { format!("{} {}G", choice.label, choice.gold) } else { choice.label.to_string() };
        let mut rows = vec![(words, if open { 'O' } else { 'G' })];
        if on == Some(pick) && let Some(h) = hero {
            rows.push(if open {
                (format!("HOLD E {}", hold_bar(h.buying, BUY_HOLD)), '6')
            } else {
                ("PURSE SHORT".to_string(), 'h')
            });
        }
        let lw = rows.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 4;
        let lh = rows.len() as i32 * 9 + 2;
        let lx = if pick == 0 { px - lw + 6 } else { px - 6 };
        let lx = lx.clamp(2, NATIVE_W - lw - 2);
        over.rect(lx, py + 2, lw, lh, 'K');
        for (i, (text, ink)) in rows.iter().enumerate() {
            over.text(lx + 2, py + 3 + i as i32 * 9, text, *ink);
        }
        taken.push((lx, py + 2, lw, lh));
    }
    taken
}
