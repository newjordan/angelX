//! The holdings drawn: the Court of Deeds (its great board, the works
//! ledger, the clerk and the plates), a held floor's steward's hall below,
//! a castle's hall, and the realm's banner wherever it flies.

use super::super::chrome;
use super::super::ink::{Img, text_width};
use super::super::kit::{self, House, Roof, Wall};
use super::super::light::Light;
use super::{NATIVE_H, NATIVE_W, TILE, fire};
use crate::realm::{Spoil, Spoils};
use crate::rules::holdings::{
    self, BAYS, BOARD, CASTLE_PLATE, CHEST, DESK, Deed, Family, Holding, Holdings, LEDGE, LEDGER, Leg, Op,
    Plate, SLOTS, STEWARD_PLATE, TABLE, WALLS, Work,
};
use crate::rules::home::{BUY_HOLD, hold_bar};
use crate::rules::citizens::{self, Bay, BayState, Citizen, Job};
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
        // The Great Hall: tall windows, a gold runner, the high table.
        "greathall" => &[
            "....@.......@...",
            "....6.......6...",
            "..wwww....wwww..",
            "..w66w....w66w..",
            "..wwww....wwww..",
            "................",
            ".rrrrrrrrrrrrrr.",
            ".r666666666666r.",
            ".rrrrrrrrrrrrrr.",
            ".hh6hh6hh6hh6hh.",
            "..BB...BB...BB..",
            "..BB...BB...BB..",
            "................",
            "................",
        ],
        // The Great Forge: its fire behind, the anvil before it.
        "greatforge" => &[
            "......rr........",
            "......rR........",
            "...rrrrrr.......",
            "...rRRRRr.......",
            "...r@77@r.......",
            "..rr@77@rr......",
            "..rR@77@Rr.BBBB.",
            "..rRRRRRRr..hh..",
            "..rrrrrrrr..hh..",
            "...@...@........",
            "................",
            "................",
            "................",
            "................",
        ],
        // The Great Nave: a rose window over the columns and the altar.
        "greatnave" => &[
            ".....rrrr.......",
            "....r6ww6r......",
            "....rw66wr......",
            "....r6ww6r......",
            "....rrrrrr......",
            "................",
            "..JJ........JJ..",
            "..JJ...66...JJ..",
            "..JJ..h66h..JJ..",
            "..JJ..hhhh..JJ..",
            "JJJJJJJJJJJJJJJJ",
            "JJJJJJJJJJJJJJJJ",
            "................",
            "................",
        ],
        // The Bourse: a balance over the counter, coin on it.
        "bourse" => &[
            "......www.......",
            ".......h........",
            "....w..h..w.....",
            "...www.h.www....",
            "................",
            "................",
            ".rrrrrrrrrrrrrr.",
            ".rRRRRRRRRRRRRr.",
            ".r6.66..66..6.r.",
            ".rrrrrrrrrrrrrr.",
            "..BB........BB..",
            "..BB........BB..",
            "................",
            "................",
        ],
        // The Waypoint: a stone door, and the way down glowing from it.
        "waypoint" => &[
            "...JJ......JJ...",
            "...JJ......JJ...",
            "...JJ..33..JJ...",
            "...JJ.3ww3.JJ...",
            "...JJ..33..JJ...",
            "...JJ......JJ...",
            "..JJJJJJJJJJJJ..",
            "................",
            "......33........",
            ".....3333.......",
            "....333333......",
            "...33333333.....",
            "................",
            "................",
        ],
        // The Garrison: kite shields on the wall, spears beside.
        "garrison" => &[
            "................",
            "..hhh....hhh....",
            ".hJJJh..hJJJh...",
            ".hJJJh..hJJJh...",
            ".hJ6Jh..hJ6Jh...",
            ".hJJJh..hJJJh...",
            "..hhh....hhh....",
            "................",
            "....W.W.W.......",
            "....W.W.W.......",
            "....W.W.W.......",
            "....W.W.W.......",
            "................",
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

/// The raiders' war-horn, 7 by 7: a band is on the road.
fn horn() -> Img {
    Img::from_rows(&["......7", ".....77", "7777777", "7777777", ".....77", "......7", "......."])
}

/// The warning over a hall a band is on the road to: plain words, in the
/// trouble red, under the banners.
fn muster_chip(cv: &mut Img, run: &Run, key: &str) {
    let Some(m) = run.home.holdings.muster.as_ref().filter(|m| m.key == key) else {
        return;
    };
    if run.holding_raid.is_some() {
        return;
    }
    let days = if m.days == 1 { "1 DAY".to_string() } else { format!("{} DAYS", m.days) };
    let text = format!("{} RAIDERS IN {days}", m.strength);
    let horn = horn();
    let w = horn.w + 4 + text_width(&text);
    let x = (NATIVE_W - w) / 2;
    chrome::chip(cv, (x - 3, 19, w + 6, 11));
    cv.stamp(&horn, x, 21);
    cv.text(x + horn.w + 4, 21, &text, '7');
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
/// its sign a sealed deed, the realm's banner on the gable, at dusk like
/// the map's other landmarks.
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
    super::super::light::at_dusk(&im)
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
        Plate::Aye => &["....O", "...O.", "..O..", "O.O..", ".O..."],
        Plate::Nay => &["O...O", ".O.O.", "..O..", ".O.O.", "O...O"],
        Plate::Orders => &["OOOO.", "OOOOO", "..O..", "..O..", "..O.."],
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
    // The Foreman's ledge, its top a plank worn by drawings.
    let (gx, gy, gw, gh) = rect_px(LEDGE);
    cv.rect(gx, gy + 2, gw, gh - 2, 'n');
    cv.frame(gx, gy + 2, gw, gh - 2, 'B');
    cv.rect(gx, gy + 2, gw, 2, 'T');
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
        Plate::Aye => "AYE",
        Plate::Nay => "NAY",
        Plate::Orders => "CREW",
        Plate::Collect => "RENTS",
    }
}

/// A plate: open and lit (a knight on it), open, or shut (dim).
fn plate(cv: &mut Img, rect: (i32, i32, i32, i32), lit: bool, open: bool, mark: &Img) {
    let (x, y, w, h) = rect_px(rect);
    let (x, y, w, h) = (x + 1, y + 1, w - 2, h - 2);
    chrome::floor_plate(cv, (x, y, w, h), lit, open);
    let ink = match (open, lit) {
        (true, true) => '6',
        (true, false) => 'O',
        (false, _) => 'j',
    };
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

/// The Foreman's head petition, and its room's def, if any.
fn head_petition(h: &holdings::Holdings) -> Option<(&crate::rules::foreman::Petition, Option<&'static holdings::RoomDef>)> {
    h.foreman.queue.first().map(|p| (p, holdings::room_def(&p.room)))
}

/// The Foreman: a brown coat, a leather apron, a rolled drawing under one
/// arm.
fn foreman_art(tick: u32) -> Img {
    let mut im = Img::from_rows(&[
        "..BBBBBB....",
        "...cccc.....",
        "...cKcKc....",
        "...ccccc....",
        "....ccc.....",
        "..PPPPPP.r..",
        ".PPPPPPPPr..",
        ".PPTTTTPP.r.",
        ".PTTTTTTP.r.",
        ".PTTTTTTP...",
        "..PPPPPP....",
        "..BB..BB....",
        "..nn..nn....",
    ]);
    // He blinks over his figures.
    if tick % 64 < 4 {
        im.put(4, 2, 'c');
        im.put(6, 2, 'c');
    }
    im
}

/// The Foreman's sheet: the head petition with its quote and his line,
/// then the rest of his ledge in the small hand. A relic the purse holds
/// rides the price; one it lacks gets its own line in the trouble red.
fn petitions_sheet(cv: &mut Img, h: &holdings::Holdings, treasury: &Spoils) {
    let mut lines: Vec<(String, char)> = Vec::new();
    match head_petition(h) {
        Some((p, def)) => {
            let room = def.map_or_else(|| p.room.clone(), |d| d.name.to_string()).to_uppercase();
            let relic = p
                .relic
                .filter(|&r| treasury.get(r) > 0)
                .map_or_else(String::new, |r| format!(" · {}", r.word().to_uppercase()));
            lines.push((format!("{room} · {gold}G{relic}", gold = p.gold), 'O'));
            if let Some(r) = p.relic.filter(|&r| treasury.get(r) == 0) {
                lines.push((format!("BRING A {}", r.word().to_uppercase()), '7'));
            }
            // His line, kept to the sheet's width.
            let says: String = p.says.chars().take(40).collect();
            let says = says.trim_end();
            lines.push((format!("\"{says}\""), 'h'));
        }
        None => lines.push(("THE LEDGE IS BARE".to_string(), 'G')),
    }
    for p in h.foreman.queue.iter().skip(1) {
        let room = holdings::room_def(&p.room).map_or_else(|| p.room.clone(), |d| d.name.to_string()).to_uppercase();
        let relic = p.relic.map_or_else(String::new, |r| format!(" · {}", r.word().to_uppercase()));
        lines.push((format!("+ {room} · {gold}G{relic}", gold = p.gold), 'G'));
    }
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 8;
    let bh = lines.len() as i32 * 9 + 5;
    let (lx, ly, lw, _) = rect_px(LEDGE);
    let x = (lx + (lw - w) / 2).clamp(2, NATIVE_W - w - 2);
    let y = (ly - bh - 3).max(2);
    chrome::board(cv, (x, y, w, bh), 'B');
    for (i, (text, ink)) in lines.iter().enumerate() {
        let tw = text_width(text);
        cv.text(x + (w - tw) / 2, y + 3 + i as i32 * 9, text, chrome::dusk_mark(*ink));
    }
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
    // The Foreman at his ledge, his petitions above him.
    if !h.foreman.queue.is_empty() || h.owned.iter().any(|w| w.rooms.len() < SLOTS) {
        let f = foreman_art(tick);
        let (lx, ly, _, _) = rect_px(LEDGE);
        cv.stamp(&f, lx + 2, ly - f.h + 4);
        petitions_sheet(cv, h, &run.treasury);
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
    cv.text(x0 + text_width(&count) + 8, by + 4, &name, 'i');
    let stars = held.map_or(0, |w| w.stars);
    for k in 0..5u8 {
        cv.stamp(&star(k < stars), bx + bw - 6 - (5 - i32::from(k)) * 9, by + 4);
    }
    // The King's favour, left of the stars: his crown and the number.
    let favour = format!("{:+}", h.favour);
    let fx = bx + bw - 6 - 5 * 9 - 10 - text_width(&favour);
    let crown = chrome::dusk_icon(&Img::from_rows(&["6.6.6", "66666", "66666", ".....", "66666"]));
    cv.stamp(&crown, fx - 8, by + 5);
    let ink = if h.favour >= holdings::ROYAL { 'O' } else if h.favour <= -holdings::ROYAL { '7' } else { 'h' };
    cv.text(fx, by + 4, &favour, ink);
    // The second line: pictures and numbers.
    let y = by + 15;
    let mut x = x0;
    // Most pressing first: what does not fit before the board's edge (and
    // the favour's word) is left off.
    let edge = bx + bw - 6 - crate::rules::crown::favour_word(h).map_or(0, |w| text_width(w) + 8);
    let mut item = |cv: &mut Img, icon: &Img, text: &str, ink: char| {
        let wide = icon.w + 3 + text_width(text);
        if x + wide > edge {
            return;
        }
        cv.stamp(icon, x, y);
        cv.text(x + icon.w + 3, y, text, chrome::dusk_mark(ink));
        x += wide + 10;
    };
    match held {
        Some(w) => {
            if let Some(m) = h.muster.as_ref().filter(|m| m.key == w.key) {
                item(cv, &horn(), &format!("RAIDERS IN {}", m.days), '7');
            }
            if w.raid > 0 {
                let claw = Img::from_rows(&["7.7.7..", "7.7.7..", ".777...", "..7....", ".......", ".......", "......."]);
                item(cv, &claw, "RAIDED", '7');
            }
            let wall = Img::from_rows(&["JJJJJJJ", "JhJhJhJ", "JJJJJJJ", "JhJhJhJ", "JJJJJJJ", ".......", "......."]);
            let walls = format!("{}/{WALLS}", w.walls);
            item(cv, &wall, &walls, if w.walls == 0 { '7' } else { 'i' });
            let coin = chrome::dusk_icon(&Img::from_rows(&[".666...", "6aaa6..", "6a6a6..", "6aaa6..", ".666...", ".......", "......."]));
            item(cv, &coin, &format!("{}G", w.coffer), '6');
            let rooms = Img::from_rows(&["rrrrrr.", "r.r..r.", "rrrrrr.", "r..r.r.", "rrrrrr.", ".......", "......."]);
            item(cv, &rooms, &format!("{}/{SLOTS}", w.standing()), 'i');
            let hand = Img::from_rows(&["..cc...", "..cc...", ".oooo..", ".oooo..", "..oo...", ".o..o..", "......."]);
            let (crew, ink) = crew_line(h, w);
            item(cv, &hand, &crew, ink);
            match w.steward.as_deref().and_then(holdings::steward) {
                Some(s) => {
                    let face = Img::from_rows(&["kkkk...", "kcck...", "cKcK...", "cccc...", ".cc....", ".......", "......."]);
                    item(cv, &face, &s.way, 'h');
                }
                None => item(cv, &Img::new(0, 7), "NO STEWARD", 'G'),
            }
            if w.fief {
                let crown = chrome::dusk_icon(&Img::from_rows(&["6.6.6..", "66666..", "66666..", ".......", ".......", ".......", "......."]));
                item(cv, &crown, "FIEF", 'h');
            }
        }
        None => match h.offered(deed) {
            Ok(()) => {
                let seal = Img::from_rows(&[".777...", "77777..", "77.77..", "77777..", ".777...", ".......", "......."]);
                item(cv, &seal, &format!("CLAIM {}G", deed.price(false)), '6');
                let crown = chrome::dusk_icon(&Img::from_rows(&["6.6.6..", "66666..", "66666..", ".......", "66666..", ".......", "......."]));
                item(cv, &crown, &format!("FIEF {}G", deed.price(true)), 'O');
            }
            Err(why) => item(cv, &Img::new(0, 7), why, 'G'),
        },
    }
    if let Some(word) = crate::rules::crown::favour_word(h) {
        cv.text(bx + bw - 6 - text_width(word), y, word, if h.favour > 0 { 'O' } else { '7' });
    }
    cross_section(cv, x0 - 2, by + 26, deed, h, tick_of(run));
}

fn tick_of(run: &Run) -> u32 {
    run.tick as u32
}

/// One citizen of the holding, seven pixels wide: a tunic in their own
/// hue, a crate on the shoulder for a hauler, a bright tool-tip when a
/// builder's stroke lands. The legs swap only while walking; at either
/// end of the walk the figure stands and works.
fn folk(citizen: &Citizen, working: bool, back: bool, tick: u32) -> Img {
    let stride = !working && (tick / 6 + citizen.id).is_multiple_of(2);
    let mut im = Img::from_rows(if stride {
        &[
            "..ccc..", "..ccc..", ".RRRRR.", "RRRRRRR", ".RRRRR.", "..RRR..", ".BB.BB.", "BB...BB",
        ][..]
    } else {
        &[
            "..ccc..", "..ccc..", ".RRRRR.", "RRRRRRR", ".RRRRR.", "..RRR..", "..BBB..", "..B.B..",
        ][..]
    });
    if citizen.job == Job::Haul {
        im.rect(4, 0, 3, 3, 'r');
        im.put(5, 1, 'O');
    }
    if citizen.job == Job::Build && working && (tick / 8).is_multiple_of(2) {
        im.put(6, 2, 'H');
    }
    const TUNICS: [char; 4] = ['R', 'O', 'q', '6'];
    let tunic = TUNICS[usize::from(citizen.hue) % TUNICS.len()];
    if tunic != 'R' {
        im = im.recolor(&[('R', tunic)]);
    }
    if back {
        im = im.flip_h();
    }
    im
}

fn key_seed(key: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in key.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// The holding's citizens, from the shared roster: a worker pacing each
/// standing room — the SimTower slice, lived in. Pure ambience from
/// `(holding, tick)`: the picture mints nobody. The building, hauling and
/// mending is the crew's (`crew`), who do it for real. It does dress a
/// bay's figure by the temper of the named villager who keeps that room:
/// grey when they go hungry or glum, a gold cap-dot when content —
/// so a starving village *looks* starved before anyone walks away.
fn cross_section_citizens(cv: &mut Img, x: i32, y: i32, held: &Holding, tick: u32) {
    // Where anyone lives, the ones about are the ledger's own.
    if !held.folk.is_empty() {
        super::holdings_folk::cross_section(cv, x, y, held, tick);
        return;
    }
    let bays: Vec<Bay> = (0..SLOTS)
        .map(|i| match held.rooms.get(i) {
            None => Bay {
                state: BayState::Empty,
                job: Job::Wander,
            },
            Some(built) if !built.standing() => Bay {
                state: BayState::Empty,
                job: Job::Wander,
            },
            Some(built) => Bay {
                state: BayState::Standing,
                job: match holdings::room_def(&built.id).map(|def| def.family) {
                    Some(Family::Forge) => Job::Make,
                    Some(Family::Tavern | Family::Chapel) => Job::Tend,
                    _ => Job::Wander,
                },
            },
        })
        .collect();
    let seed = key_seed(&held.key);
    let floor_y = y + 13;
    for citizen in citizens::roster(seed, &bays).into_iter().filter(|c| c.job != Job::Haul) {
        let (from, span, stride) = if citizen.job == Job::Haul {
            (
                x + 16 + citizen.home as i32 * 30,
                ((citizen.to - citizen.home) * 30) as u32,
                2,
            )
        } else {
            // A short pace inside one's own bay, slower at restful work.
            let (span, stride) = match citizen.job {
                Job::Build => (13, 3),
                Job::Make | Job::Tend => (6, 5),
                _ => (10, 4),
            };
            (x + 16 + citizen.home as i32 * 30, span, stride)
        };
        let phase = citizens::phase(seed, citizen.id, u64::from(tick), span, stride);
        let px = if citizen.job == Job::Haul || citizen.job == Job::Build {
            from + phase.step as i32
        } else {
            // Workers keep mostly to their room's art, at its right shoulder.
            from + 13 + phase.step as i32
        };
        let mut figure = folk(&citizen, phase.working, phase.back, tick);
        if citizen.job != Job::Haul {
            let keeper = held
                .rooms
                .get(citizen.home)
                .and_then(|built| holdings::room_def(&built.id))
                .and_then(|def| held.folk.iter().find(|f| f.trade.family() == Some(def.family)));
            if let Some(soul) = keeper {
                if soul.hunger > 0 || soul.mood <= -2 {
                    figure = figure.recolor(&[('R', 'G'), ('O', 'G'), ('q', 'G'), ('6', 'G')]);
                } else if soul.mood >= 3 {
                    figure.put(3, 0, '6');
                }
            }
        }
        cv.stamp(&figure, px, floor_y);
    }
}

/// One hand of a holding's crew, eight by nine: a smock in its own hue,
/// and the load its work carries — a beam to the room going up, a stone to
/// the walls, a sack to the granary — on the way back from the yard; a
/// hammer's stroke, a trowel's or a sack set down while it works.
fn hand_art(work: Option<Work>, leg: Leg, hue: u8, back: bool, tick: u32) -> Img {
    let walking = !leg.working && work.is_some();
    let stride = walking && (tick / 5 + u32::from(hue)).is_multiple_of(2);
    let mut im = Img::from_rows(if stride {
        &[
            "........", "..ccc...", "..ccc...", ".RRRRR..", "RRRRRRR.", ".RRRRR..", "..RRR...", ".BB.BB..", "BB...BB.",
        ][..]
    } else {
        &[
            "........", "..ccc...", "..ccc...", ".RRRRR..", "RRRRRRR.", ".RRRRR..", "..RRR...", "..BBB...", "..B.B...",
        ][..]
    });
    const SMOCKS: [char; 4] = ['R', 'u', 'P', 'S'];
    let smock = SMOCKS[usize::from(hue) % SMOCKS.len()];
    if smock != 'R' {
        im = im.recolor(&[('R', smock)]);
    }
    let beat = (tick / 7 + u32::from(hue)).is_multiple_of(2);
    match work {
        Some(Work::Build) if leg.laden && !leg.working => {
            // A beam on the shoulder, longer than the hand.
            im.line(0, 2, 7, 2, 'O');
            im.put(0, 2, 'B');
            im.put(7, 2, 'B');
        }
        Some(Work::Build) if leg.working && leg.laden => {
            im.put(6, if beat { 1 } else { 3 }, 'H');
            im.put(5, if beat { 2 } else { 3 }, 'r');
        }
        Some(Work::Mend) if leg.laden && !leg.working => {
            // A dressed stone on the head.
            im.rect(2, 0, 3, 2, 'J');
            im.put(2, 0, 'h');
        }
        Some(Work::Mend) if leg.working && leg.laden => {
            im.rect(5, if beat { 2 } else { 4 }, 2, 2, 'J');
        }
        Some(Work::Stock) if leg.laden && !leg.working => {
            // A sack on the back.
            im.rect(4, 0, 3, 3, 'T');
            im.put(5, 0, 'o');
        }
        Some(Work::Stock) if leg.working && leg.laden => {
            im.rect(5, 6, 3, 3, 'T');
            im.put(6, 6, 'o');
        }
        _ => {}
    }
    if back {
        im = im.flip_h();
    }
    im
}

/// The crew's yard: timber, stone and the granary's sacks, stacked where
/// they wait for the hands — in the first empty bay, or at the gate once
/// every bay is built. `stores` sets how many sacks show.
fn yard_art(stores: u32) -> Img {
    let mut im = Img::new(14, 7);
    // Beams.
    im.line(0, 6, 7, 6, 'r');
    im.line(1, 5, 6, 5, 'o');
    // Stones.
    im.rect(8, 4, 3, 3, 'J');
    im.put(8, 4, 'h');
    im.rect(11, 5, 3, 2, 'G');
    // Sacks, one for each five in the granary, to three.
    for k in 0..(stores.div_ceil(5)).min(3) as i32 {
        im.rect(2 + k * 3, 2 - (k % 2), 3, 3, 'T');
        im.put(3 + k * 3, 2 - (k % 2), 'o');
    }
    im
}

/// The crew in a cross-section: each hand on its trip between where its
/// load waits and the work, the load the work's, landing as the rules land
/// it (`leg`). Timber waits in the yard (the first empty bay, beside the
/// room going up); stone and sacks come in at the gate, the stone set on
/// the gate's walls, the sacks carried into the first bay. Resting, they
/// stand about the yard; raiders hold it, and they are gone.
fn cross_section_crew(cv: &mut Img, x: i32, y: i32, h: &Holdings, held: &Holding, tick: u32) {
    let yard = (held.rooms.len() < SLOTS).then_some(held.rooms.len());
    let bay_x = |b: Option<usize>| b.map_or(x + 2, |i| x + 16 + i as i32 * 30);
    if held.raid == 0 {
        let at = bay_x(yard) + if yard.is_some() { 12 } else { 0 };
        cv.stamp(&yard_art(h.stores), at, y + 13);
    }
    let hands = h.hands(held);
    let work = h.crew_work(held);
    // (the work, where the load waits), in pixels.
    let (to, from) = match work {
        Some(Work::Build) => (bay_x(held.rising()) + 2, bay_x(yard) + 8),
        Some(Work::Mend) => (x + 2, x + 30),
        Some(Work::Stock) => (x + 24, x + 2),
        None => (bay_x(yard) + 8, bay_x(yard) + 8),
    };
    for j in 0..hands {
        let leg = holdings::leg(u64::from(tick), j, hands);
        let hue = (j % 4) as u8;
        let spread = j as i32 * 3;
        let (wx, fx) = (to + spread, from + spread);
        let (px, back) = match work {
            Some(_) => {
                let px = wx as f32 + (fx - wx) as f32 * leg.along;
                // Facing the way they walk: out to the yard, back to the work.
                let out = !leg.laden;
                (px.round() as i32, (fx < wx) == out)
            }
            None => (fx + j as i32 * 5, j % 2 == 1),
        };
        let leg = if work.is_some() { leg } else { Leg { along: 0.0, laden: false, working: true } };
        cv.stamp(&hand_art(work, leg, hue, back, tick), px, y + 12);
    }
}

/// The crew's line on a board: how many hands, at what, and why fewer.
pub(super) fn crew_line(h: &Holdings, held: &Holding) -> (String, char) {
    if held.raid > 0 {
        return ("CREW FLED".to_string(), '7');
    }
    let hands = h.hands(held);
    if hands == 0 {
        return ("RIOT: TOOLS DOWN".to_string(), '7');
    }
    let doing = h.crew_work(held).map_or("RESTING", Work::doing);
    let ink = if hands < held.crew() { '7' } else { 'i' };
    (format!("{hands} {doing}"), ink)
}

/// A holding side-on: the stair (a floor) or the keep's tower (a castle)
/// at the left, then its ten bays as built, building or empty.
pub(super) fn cross_section(cv: &mut Img, x: i32, y: i32, deed: Deed, h: &Holdings, tick: u32) {
    let held = h.holding(&deed.key());
    // The way in: a stair shaft or a tower.
    match deed {
        Deed::Floor(_) => {
            chrome::shaped(cv, (x, y, 12, 22), &[2, 1], chrome::Tone::WELL.rimmed(chrome::COLD_RIM), false);
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
        // A bay: a rounded dark well in a stone rim, its floor a step lit.
        chrome::shaped(cv, (bx, y, 28, 22), &[2, 1], chrome::Tone::WELL.rimmed(chrome::COLD_RIM), false);
        cv.line(bx + 2, y + 20, bx + 25, y + 20, 'j');
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
    if let Some(held) = held {
        cross_section_citizens(cv, x, y, held, tick);
        cross_section_crew(cv, x, y, h, held, tick);
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
        cv.text(lx + lw - 3 - text_width(&n), y, &n, if hot { '7' } else { 'i' });
        cv.text(lx + 12, y + 8, op.source(), 'G');
    }
}

/// The bottom edge's clear stretch (native pixels): right of the tray of
/// arms and potions, left of the spell buttons.
const OPEN_FLOOR: (i32, i32) = (142, 328);

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
        Plate::Aye => match head_petition(h) {
            Some((p, def)) => {
                let relic = p.relic.map_or_else(String::new, |r| format!(" · {}", r.word().to_uppercase()));
                format!("{} — {}G{relic}", def.map_or("A ROOM", |d| d.name).to_uppercase(), p.gold)
            }
            None => "PETITIONS".to_string(),
        },
        Plate::Nay => "REFUSE THE FOREMAN".to_string(),
        // The order the crew works down, its first first.
        Plate::Orders => match h.holding(&deed.key()) {
            Some(w) => {
                let order: Vec<&str> = std::iter::once(w.order)
                    .chain(Work::ALL.into_iter().filter(|&o| o != w.order))
                    .map(Work::word)
                    .collect();
                format!("CREW: {}", order.join(" > "))
            }
            None => "CREW ORDERS".to_string(),
        },
        Plate::Collect => "COLLECT RENTS".to_string(),
    };
    let last = match holdings::plate_open(run, p) {
        Ok(_) if p.tapped() => ("E: TURN".to_string(), 'O'),
        Ok((_, price)) => {
            let what = match (p, h.holding(&deed.key())) {
                (Plate::Collect, _) => format!("HOLD E: +{price}G"),
                (Plate::Orders, Some(w)) => format!("HOLD E: {}", w.order.next().word()),
                _ if price > 0 => format!("HOLD E: {price}G"),
                _ => "HOLD E".to_string(),
            };
            (format!("{what} {}", hold_bar(hero.buying, BUY_HOLD)), 'O')
        }
        Err(why) => (why.to_string(), 'h'),
    };
    let mut lines = vec![(title, '9'), last];
    // A petition's worth: what the room's neighbours would make of it, and
    // the Foreman's own line for it.
    if let Plate::Aye | Plate::Nay = p
        && let Some((petition, def)) = head_petition(h)
        && let Some(def) = def
        && let Some(w) = h.holding(&petition.key)
    {
        for (n, word) in w.preview(def) {
            lines.push((format!("{word} {n:+}"), if n > 0 { 'y' } else { '7' }));
        }
        lines.push((format!("\"{}\"", petition.says), 'h'));
    }
    let w = lines.iter().map(|(s, _)| text_width(s)).max().unwrap_or(0) + 8;
    let bh = lines.len() as i32 * 9 + 5;
    // On the south wall, under every plate and its name: between the tray
    // and the spell buttons when it fits there.
    let x = if w <= OPEN_FLOOR.1 - OPEN_FLOOR.0 {
        OPEN_FLOOR.0 + (OPEN_FLOOR.1 - OPEN_FLOOR.0 - w) / 2
    } else {
        ((NATIVE_W - w) / 2 - 24).max(2)
    };
    let y = NATIVE_H - bh - 1;
    chrome::board(cv, (x, y, w, bh), 'B');
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
        let seized = crate::rules::fey::seized_bay(held, i);
        let open = built.standing() && !used && !seized;
        let mark = Img::from_rows(&["..O..", ".OOO.", "OOOOO", ".OOO.", "..O.."]);
        plate(cv, plate_rect, on(run, plate_rect), open, &mark);
        let seized_by = held.seized.as_ref().filter(|_| seized).map(|z| format!("{} HAS IT", z.who.to_uppercase()));
        let tag = match (def, built.standing(), used) {
            (_, false, _) => "RISING",
            _ if seized_by.is_some() => seized_by.as_deref().unwrap_or(""),
            (_, _, true) => "USED",
            (Some(d), _, _) => d.says,
            (None, _, _) => "",
        };
        let (px, py, pw, ph) = rect_px(plate_rect);
        let tw = text_width(tag);
        let ty = if BAYS[i].1 < 5 { py + ph + 1 } else { py - 9 };
        chrome::chip(cv, (px + (pw - tw) / 2 - 1, ty - 1, tw + 2, 9));
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
    chrome::chip(cv, (px + (pw - w) / 2 - 1, py + ph, w + 2, 9));
    cv.text(px + (pw - w) / 2, py + ph + 1, &tag, if open { 'O' } else { 'G' });
    hall_crew(cv, run, held);
    super::holdings_folk::hall_folk(cv, run, held);
    muster_chip(cv, run, &held.key);
}

/// The crew in a steward's hall, twice the size: timber from the yard by
/// the door to a rising bay's plate, stone from the yard to the door's
/// walls, sacks in at the door to the yard — each hand on the trip the
/// rules count. Raiders in the hall, and the crew hides.
fn hall_crew(cv: &mut Img, run: &Run, held: &Holding) {
    if run.holding_raid.is_some() {
        return;
    }
    let h = &run.home.holdings;
    let room = run.room();
    let door = match (0..4).find(|&d| room.doors[d]).unwrap_or(3) {
        0 => (12 * TILE, 2 * TILE),
        1 => (22 * TILE, 7 * TILE + 8),
        2 => (12 * TILE, 12 * TILE + 8),
        _ => (2 * TILE, 7 * TILE + 8),
    };
    // The yard stands a few strides in from the door, clear of the bays
    // and the steward's table.
    let yard = match (0..4).find(|&d| room.doors[d]).unwrap_or(3) {
        0 => (16 * TILE, 4 * TILE + 8),
        1 => (18 * TILE, 7 * TILE + 8),
        2 => (16 * TILE, 10 * TILE + 4),
        _ => (6 * TILE, 7 * TILE + 8),
    };
    if held.raid == 0 {
        let pile = doubled(&yard_art(h.stores));
        cv.stamp(&pile, yard.0 - pile.w / 2, yard.1 - pile.h);
    }
    let hands = h.hands(held);
    let work = h.crew_work(held);
    let rising = held.rising().map(|i| {
        let (px, py, pw, ph) = rect_px(holdings::bay(i).1);
        (px + pw / 2, py + ph / 2 + 6)
    });
    let (to, from) = match work {
        Some(Work::Build) => (rising.unwrap_or(yard), yard),
        Some(Work::Mend) => (door, yard),
        Some(Work::Stock) => (yard, door),
        None => (yard, yard),
    };
    let tick = run.tick as u32;
    for j in 0..hands {
        let leg = holdings::leg(run.tick, j, hands);
        let spread = (j as i32 - hands as i32 / 2) * 7;
        let (x, y, back, leg) = match work {
            Some(_) => {
                let x = to.0 as f32 + (from.0 - to.0) as f32 * leg.along;
                let y = to.1 as f32 + (from.1 - to.1) as f32 * leg.along;
                (x.round() as i32 + spread, y.round() as i32, (from.0 < to.0) == !leg.laden, leg)
            }
            None => (yard.0 + spread * 2, yard.1 + 4, j % 2 == 1, Leg { along: 0.0, laden: false, working: true }),
        };
        let im = doubled(&hand_art(work, leg, (j % 4) as u8, back, tick));
        cv.stamp(&im, x - im.w / 2, y - im.h);
    }
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
        let mut lines = vec![
            (who, '9'),
            if waiting > 0 {
                (format!("HOLD E: +{waiting}G {}", hold_bar(hero.buying, BUY_HOLD)), 'O')
            } else {
                ("NO RENT YET".to_string(), 'h')
            },
            crew_line(&run.home.holdings, held),
        ];
        // The going rates: this holding's next room of every family, at
        // the list — the knowledge a padded quote is caught by.
        for family in Family::ALL {
            if let Some(def) = held.next_of(family) {
                let relic = def.relic.map_or_else(String::new, |r| format!(" +{}", r.word().to_uppercase()));
                lines.push((format!("{} {}G{relic}", def.name.to_uppercase(), holdings::room_price(&run.home.holdings, def)), 'i'));
            }
        }
        // The village roll: who lives here and how the realm is
        // treating them, then who is merely lodging. The board
        // names the first eight souls and counts the rest.
        for soul in held.folk.iter().take(8) {
            let temper = if soul.hunger > 0 {
                "HUNGRY"
            } else if soul.mood >= 3 {
                "CONTENT"
            } else if soul.mood <= -2 {
                "GLUM"
            } else {
                "SETTLED"
            };
            lines.push((
                format!("{} · {} · {temper}", soul.name.to_uppercase(), soul.trade.word().to_uppercase()),
                if soul.hunger > 0 || soul.mood <= -2 { '7' } else { 'h' },
            ));
        }
        if held.folk.len() > 8 {
            lines.push((format!("+{} MORE SOULS", held.folk.len() - 8), 'h'));
        }
        for guest in &held.visitors {
            lines.push((
                format!("GUEST {} · {} · {}D", guest.name.to_uppercase(), guest.kind.word().to_uppercase(), guest.days),
                'O',
            ));
        }
        lines
    } else if let Some(i) = (0..held.rooms.len().min(SLOTS)).find(|&i| inside(holdings::bay(i).1)) {
        plate = holdings::bay(i).1;
        let built = &held.rooms[i];
        let Some(def) = holdings::room_def(&built.id) else { return };
        let last = if !built.standing() {
            ("STILL RISING".to_string(), 'h')
        } else if let Some(z) = held.seized.as_ref().filter(|_| crate::rules::fey::seized_bay(held, i)) {
            (format!("SEIZED: {} AT THE WORK, {}D", z.who.to_uppercase(), z.days), '3')
        } else if run.holding_used & (1 << i) != 0 {
            ("USED THIS FLOOR".to_string(), 'h')
        } else {
            (format!("HOLD E: {} {}", def.says, hold_bar(hero.buying, BUY_HOLD)), 'O')
        };
        let mut lines = vec![(def.name.to_uppercase(), '9'), last];
        if built.standing() {
            // The villager who keeps this room, if one of its trade
            // lives here (the first of them, as the roster keeps them).
            if let Some(soul) = held.folk.iter().find(|f| f.trade.family() == Some(def.family)) {
                lines.push((format!("{} KEEPS IT", soul.name.to_uppercase()), 'h'));
            }
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
    chrome::board(cv, (x, y, w, bh), 'B');
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
    chrome::board(cv, (x - 4, y - 6, width + 8, 32), 'B');
    cross_section(cv, x, y - 1, deed, h, run.tick as u32);
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
    muster_chip(cv, run, &deed.key());
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
    chrome::board(cv, (x, y, w, bh), 'B');
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
    stood: bool,
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
        // In 3D the posts stand up as blocks (`world3d::lots`).
        if !stood {
            over.stamp(&im, px - im.w / 2, py - im.h);
        }
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
    chrome::panel(over, (cx, cy, w, bh), chrome::Tone::FRAMED);
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
        chrome::chip(over, (lx, py + 2, lw, lh));
        for (i, (text, ink)) in rows.iter().enumerate() {
            over.text(lx + 2, py + 3 + i as i32 * 9, text, chrome::dusk_mark(*ink));
        }
        taken.push((lx, py + 2, lw, lh));
    }
    taken
}
