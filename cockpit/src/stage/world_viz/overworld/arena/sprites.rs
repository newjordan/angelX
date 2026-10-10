//! The delve's figures and dressing, drawn at the overworld's own scale:
//! sixteen pixels to a tile. The knights are the realm's hero from the kit;
//! the monsters are authored ink rows or shaded blobs in the same hand.
//! A figure's live parts (eyes, fire, a knight's colours, loot) are signal
//! inks so they never dim; everything else is the realm's material banks.

use super::super::ink::{Img, hash};
use super::super::kit::{self, blob};
use crate::drive::together_shooter::{EnemyKind, Shot};

fn rows(rows: &[&str]) -> Img {
    Img::from_rows(rows)
}

/// The mini-viz hero for player one; every other knight wears the blue house
/// of the lists on the same body.
pub(super) fn knight(id: u32) -> Img {
    let im = kit::knight();
    match id {
        1 => im,
        2 => im.recolor(&[('7', '1'), ('8', '0'), ('5', '9')]),
        3 => im.recolor(&[('7', 'A'), ('8', 'l'), ('5', '6')]),
        _ => im.recolor(&[('7', '4'), ('8', 'a'), ('5', 'c')]),
    }
}

/// A knight of the company in their own colours.
pub(crate) fn knight_in(colours: &[(char, char)]) -> Img {
    kit::knight().recolor(colours)
}

/// A fallen knight: the helm on its side, the plume in the dust.
/// A hexed knight: a frog in a tiny helmet, hopping.
pub(super) fn frog(tick: u32) -> Img {
    let hop = (tick / 6).is_multiple_of(2);
    rows(&[
        "....iHi.....",
        ".ww.hKh.ww..",
        "wKwlhhhlwKw.",
        "lCCCCCCCCCl.",
        "lCCCCCCCCCCl",
        ".lCYYYYYYCl.",
        if hop { "..lCYYYYCl.." } else { "llCYYYYYYCll" },
        if hop { "..ll....ll.." } else { "l.lll..lll.l" },
    ])
}

pub(super) fn fallen() -> Img {
    rows(&[
        "...77.......",
        "..7iHHi.....",
        ".77HhhhJ.J..",
        "..hJhJhJJhJ.",
        "...gg.gg.gg.",
    ])
}

/// A statue: the same figure in the structure greys, by brightness.
pub(super) fn stone(im: &Img) -> Img {
    const RAMP: [char; 7] = ['g', 'j', 'G', 'J', 'h', 'i', 'H'];
    let mut out = Img::new(im.w, im.h);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some([r, g, b]) = im.get(x, y) {
                let l = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
                out.put(
                    x,
                    y,
                    RAMP[(l as usize * RAMP.len() / 256).min(RAMP.len() - 1)],
                );
            }
        }
    }
    out
}

pub(super) fn enemy(kind: EnemyKind, tick: u32) -> Img {
    let flap = (tick / 4).is_multiple_of(2);
    match kind {
        EnemyKind::Mimic => mimic(tick),
        EnemyKind::PitTyrant => {
            let im = rows(&[
                "TT.........................TT",
                "TOT.......................TOT",
                ".TOT.....................TOT.",
                "..TOT.....XXXXXXXXX.....TOT..",
                "...TTXXXXXjjjjjjjjjXXXXXTT...",
                "....XjjjjjjGGGGGGGjjjjjjX....",
                "...XjjGGGGGGjjjjjGGGGGGjjX...",
                "..XjGGjj77jjjjjjjjj77jjGGjX..",
                "..XjGjjj777jjjjjjj777jjjGjX..",
                ".XjGjjjjjjjjjjjjjjjjjjjjjGjX.",
                ".XjGjjjj@66666@66666@jjjjGjX.",
                ".XjGjjjjj@@@@@@@@@@@jjjjjGjX.",
                "XXjGGjjjjjjjjjjjjjjjjjjjGGjXX",
                "XjjjGGGjjjjgjjjjjgjjjjGGGjjjX",
                "XjGGjjGGGGGGGGGGGGGGGGGjjGGjX",
                "XjGjjjjXXXXXXXXXXXXXXXjjjjGjX",
                "HjGj.....XjjGGjGGjjX.....jGjH",
                "HHj......XjGjjXjjGjX......jHH",
                ".H.......XXHHXXXHHXX.......H.",
            ]);
            // Its breath flickers in its maw.
            if flap {
                im.recolor(&[('6', '@'), ('@', '6')])
            } else {
                im
            }
        }
        EnemyKind::Flesher => rows(&[
            "........llll........",
            ".......lMMMAl.......",
            ".......lM6A6l.......",
            ".......lAkkAl.......",
            ".....llmAAAAmll.....",
            "..JhlMMAAAAAAMMl..WW",
            ".J.lMAA9999AAAAMlWWh",
            ".H.lMA99979999AAMWWh",
            "HH.lMA99999979AAMBh.",
            ".HlMAA97999999AAMl..",
            "..lMA999999999AAml..",
            "..lmA99999799AAAml..",
            "...lmAA99999AAAml...",
            "...lmAAbbbbbAAml....",
            "....lmmAAAAAmml.....",
            if flap {
                ".....lmm...mml......"
            } else {
                "....lmm....lmm......"
            },
            if flap {
                ".....ll.....ll......"
            } else {
                "....ll.....ll......."
            },
        ]),
        EnemyKind::Silkmother => rows(&[
            if flap {
                "J....J.......J....J"
            } else {
                ".J...J.......J...J."
            },
            ".J....J.....J....J.",
            "..J...JXXXXXJ...J..",
            "...JJXjjjjjjjXJJ...",
            "JJ..XjjjjjjjjjX..JJ",
            "..JJXjjj777jjjXJJ..",
            "....Xjjjj7jjjjX....",
            "..JJXjjj777jjjXJJ..",
            "JJ..XXjjjjjjjXX..JJ",
            "...JJ.XXjjjXX.JJ...",
            "..J..J.X666X.J..J..",
            if flap {
                ".J...J..XXX..J...J."
            } else {
                "J....J..XXX..J....J"
            },
        ]),
        EnemyKind::Spiderling => rows(&[
            if flap { "J..J.J..J" } else { ".J.J.J.J." },
            ".J.XjX.J.",
            "..JX6XJ..",
            ".J.XXX.J.",
            if flap { "J.......J" } else { ".J.....J." },
        ]),
        EnemyKind::Bat if flap => rows(&[
            "r...........r",
            "Rr.........rR",
            "oRr.O...O.rRo",
            ".oRrtOOOtrRo.",
            "..o.O7O7O.o..",
            "......O......",
        ]),
        EnemyKind::Bat => rows(&[
            ".............",
            "....O...O....",
            "...rtOOOtr...",
            "..rRO7O7ORr..",
            ".rRo.OOO.oRr.",
            "rRo...O...oRr",
        ]),
        EnemyKind::Skeleton => rows(&[
            "...iHHi....",
            "..iHHHHi...",
            "..HgHHgH...",
            "..iHHHHi...",
            "...hJhJ....",
            "....hh...o.",
            ".iHHHHHi..o",
            ".h.hHh.h..o",
            ".h.HJH.HhHo",
            ".J.hHh....o",
            "...HJH...o.",
            "...h.h.....",
            "..hJ.Jh....",
            "..J...J....",
            ".hJ...Jh...",
        ]),
        EnemyKind::Wraith => {
            let hem: [&str; 3] = if flap {
                ["..u.U.U.u..", "....u.u....", "...u...u..."]
            } else {
                ["...uU.Uu...", "..u..U..u..", ".....u....."]
            };
            let mut body = vec![
                "....uuu....",
                "...uUUUu...",
                "..uUvvvUu..",
                "..UvSSSvU..",
                "..US2S2SU..",
                "..uSSSSSu..",
                ".uUSSSSSUu.",
                ".UvUSSSUvU.",
                "uU.UvSvU.Uu",
                "u..UvSvU..u",
                "...uUSUu...",
                "...uU.Uu...",
            ];
            body.extend(hem);
            rows(&body)
        }
        EnemyKind::Imp => rows(&[
            ".h.......h.",
            ".hR.....Rh.",
            "..RRRRRRR..",
            "..R6RRR6R..",
            "..pRRoRRp..",
            "...pRRRp...",
            ".rRRpRpRRr.",
            "r..RRRRR..r",
            "...pRRRp.o.",
            "...R...R.o.",
            "..pp...pp..",
        ]),
        EnemyKind::Demon => demon(flap),
        EnemyKind::Sapper => rows(&[
            "...pRp.....",
            "..pRRRp....",
            "mMMMMMMm...",
            ".MK7MK7M...",
            "..MMMMM.bBb",
            "..MyyyM.BIB",
            ".mMMMMMmbBb",
            "m.MMMMM.m..",
            if flap { "..MM.MM...." } else { "..M..MM...." },
            if flap { ".mM...Mm..." } else { ".mm...M...." },
        ]),
        EnemyKind::Necromancer => {
            let flame = if (tick / 3).is_multiple_of(2) { '3' } else { '2' };
            let mut im = rows(&[
                "..2...XXX....",
                ".232.XgggX...",
                "..H.XggggX...",
                ".HiHXK2K2X...",
                "..b.XgggXX...",
                "..b.XXgXXX...",
                "..bXXjXXjXX..",
                "..bXjXXXXjXX.",
                "..bXXXjXXXXX.",
                "..bXXjXXXjXX.",
                "..bXXXXjXXXX.",
                "..bXXjXXXjXX.",
                "..bXjXXXXXjX.",
                "..XXXXXXXXXXX",
                ".XjXXXjXXjXXX",
            ]);
            im.put(2, 0, flame);
            im
        }
        EnemyKind::Warboar => rows(&[
            "......bbb.........",
            "....bbBBBbb.......",
            "..bbBBpppBBb......",
            ".bBBppppppBBbb.HH.",
            "bBpppppppppBB7BH..",
            "bBpprppprpppBBBB..",
            "bBpppppppppppBnnb.",
            ".bBpppppppppBBnn..",
            "..bBBppppBBBb.....",
            if flap { "...bb.bb..bb.bb..." } else { "..bb..bb...bb.bb.." },
            if flap { "...nb.nb..nb.nb..." } else { "..nb...nb.nb..nb." },
        ]),
        EnemyKind::Slime if flap => rows(&[
            "....EEEE....",
            "..EElllmEE..",
            ".ElllYYlllE.",
            "ElllYYllllmE",
            "ElKllllKlllE",
            "EllllllllllE",
            "EmlllllllmmE",
            ".EmmmmmmmmE.",
            "..EEEEEEEE..",
        ]),
        EnemyKind::Slime => rows(&[
            "............",
            "...EEEEEE...",
            ".EElllYYlmE.",
            "ElllYYllllmE",
            "ElKllllKlllE",
            "EmlllllllmmE",
            "EEmmmmmmmmEE",
            ".EEEEEEEEEE.",
            "............",
        ]),
        EnemyKind::Goblin => rows(&[
            if flap { "......tTTt..." } else { ".......tTTt.." },
            if flap { ".....tTTTTt.." } else { "......tTTTTt." },
            "....tTO5OTTt.",
            "mMMmtTTTTTTt.",
            ".MK7tTTTOTTt.",
            ".MMMMtTTTTt..",
            "..yy.ottttO..",
            ".mMMMm.......",
            "m.MMM.m......",
            if flap { "..M.M........" } else { "..MM..M......" },
            if flap { ".mm.mm......." } else { ".m...mm......" },
        ]),
        EnemyKind::Hob => {
            let spark = if (tick / 2).is_multiple_of(2) { '6' } else { '5' };
            let mut im = rows(&[
                "..NNN......6.",
                ".NLLLN....65.",
                "NLKLKLN...K..",
                ".LLLLL...KKK.",
                "..NLN...KKKKK",
                ".NLLLNmLKKKK.",
                "N.LLL.N.KKK..",
                "..L.L........",
                ".NN.NN.......",
            ]);
            im.put(11, 0, spark);
            im
        }
        EnemyKind::Shaman => rows(&[
            ".7.H...H.7...",
            "..7HHHHH7....",
            "...HKHKH.....",
            "...HHHHH...O.",
            "....hHh....b.",
            "..rRRRRRr..b.",
            ".rRRoRoRRr.b.",
            "rRRRRRRRRRrb.",
            ".rRRoRoRRr.b.",
            "..rRRRRRr..b.",
            "..rR...Rr..b.",
            "..nn...nn....",
        ]),
        // The Lich: an ice-crowned skull, eyes lit teal, frost in both
        // hands, a slate robe.
        EnemyKind::Lich => rows(&[
            "....W.W.W....",
            "....WWWWW....",
            "...VWWWWWV...",
            if flap {
                "...V3WWW3V..."
            } else {
                "...VwWWWwV..."
            },
            "...VWKKKWV...",
            "....VWKWV....",
            "..uuSVVVSuu..",
            ".uuSSVWVSSuu.",
            "3uSSSVWVSSSu3",
            "wzSSSVWVSSSzw",
            "3.uSSSVSSSu.3",
            "...uSSSSSu...",
            "..uuSSSSSuu..",
            ".uuuSuuuSuuu.",
        ]),
        // The Hollow One: stones floating round a void, a cold light at
        // its heart; its crown of stones turns with `flap`.
        EnemyKind::Hollow => rows(&[
            if flap {
                ".......gjj......."
            } else {
                "........jjg......"
            },
            "......gJhJg......",
            if flap {
                ".jg...gjjjg...gj."
            } else {
                "..gj..gjjjg..jg.."
            },
            "gJhj.........jhJg",
            ".jg....001....gj.",
            "......01110......",
            ".....0123210.....",
            "..gg.0123210.gg..",
            ".gJhg.01110.gJhg.",
            "..gg...000...gg..",
            "......gjjjg......",
            ".....gJhhhJg.....",
            "......gjjjg......",
            "...gj.......jg...",
            "..gJhj.....jhJg..",
            "...gj.......jg...",
        ]),
        // The Hexer: a hooded witch, eyes lit in the hood's dark, her
        // staff's frog-green orb flaring now and then.
        EnemyKind::Hexer => {
            let top = if (tick / 16).is_multiple_of(3) {
                ["........YCY..", ".........CwC.", "...XXX..YCY.."]
            } else {
                ["..........C..", ".........CYC.", "...XXX....C.."]
            };
            rows(&[
                top[0],
                top[1],
                top[2],
                "..XeeeX...b..",
                ".XekkkeX..b..",
                ".XkYkYkX..b..",
                "..XkkkX..Ob..",
                ".EeeeeeEOOb..",
                "EeeEeEeeE.b..",
                ".EeeeeeeE.b..",
                "..EeeeeE..b..",
                "..nn..nn.....",
            ])
        }
        EnemyKind::Ward => rows(&[
            "..EE..",
            if flap { ".E7E7." } else { ".E8E8." },
            ".EEEE.",
            "..lE..",
            ".lEl..",
            "..El..",
            ".lEl..",
            "..El..",
            ".bbbb.",
            "bBBBBb",
        ]),
        EnemyKind::Dummy => kit::quintain(true, tick / 3),
        EnemyKind::Dragon => kit::dragon((tick / 30) % 4 == 3, tick),
        // Bosses draw their own art (arena.rs); this is never reached.
        EnemyKind::Boss => Img::new(1, 1),
    }
}

/// A greater demon: charcoal hide over molten seams, ribbed wings behind.
fn demon(flap: bool) -> Img {
    let mut im = Img::new(27, 25);
    let lift = i32::from(flap);
    for (x0, dir) in [(9, -1), (17, 1)] {
        for i in 0..9 {
            let h = 13 - i;
            let ink = if i % 3 == 0 { 'p' } else { 'b' };
            im.line(x0 + dir * i, 9 - h / 2 - lift, x0 + dir * i, 9 + h / 3, ink);
        }
    }
    let mut body = blob(
        27,
        25,
        &[
            (13.5, 13.0, 5.6, 6.4),
            (13.5, 6.0, 3.8, 3.3),
            (10.0, 20.5, 2.3, 3.6),
            (17.0, 20.5, 2.3, 3.6),
            (8.4, 12.5, 2.1, 4.2),
            (18.6, 12.5, 2.1, 4.2),
        ],
        &['K', 'X', 'g', 'j', 'G'],
        0.3,
        23,
        3.0,
    );
    body.outline_inside('K');
    im.stamp(&body, 0, 0);
    im.line(11, 4, 9, 0, 'h');
    im.line(16, 4, 18, 0, 'h');
    im.put(9, 0, 'i');
    im.put(18, 0, 'i');
    im.put(12, 6, '5');
    im.put(15, 6, '5');
    for (x, y) in [(13, 11), (14, 12), (13, 13), (12, 14), (15, 14), (14, 16)] {
        im.put(x, y, 'R');
    }
    im.put(13, 8, 'o');
    im.put(14, 8, 'o');
    im
}

/// A crypt's headstone: a rounded slab with a cross cut in it.
pub(super) fn tomb(v: u32) -> Img {
    let lean = (hash(v as i32, 3, 71) % 3) as f32 - 1.0;
    let mut im = blob(
        16,
        16,
        &[(8.0 + lean * 0.4, 6.5, 5.2, 5.6), (8.0, 10.5, 5.4, 4.8)],
        &['s', 'x', 'S', 'u', 'U', 'v'],
        0.5,
        v,
        1.6,
    );
    im.outline_inside('k');
    im.line(8, 4, 8, 10, 'x');
    im.line(6, 6, 10, 6, 'x');
    im.line(2, 15, 13, 15, 'S');
    im
}

/// A bookcase in the Drowned Archive: a timber frame, three shelves of
/// spines in the realm's muted inks, each case shelved its own way.
pub(super) fn shelf(v: u32) -> Img {
    const SPINES: [char; 8] = ['r', 'B', 'q', 'E', 'o', 'p', 'R', 'u'];
    let mut im = Img::new(16, 16);
    im.rect(0, 0, 16, 15, 'b');
    im.line(1, 1, 14, 1, 'P');
    for shelf in 0..3 {
        let y0 = 2 + shelf * 4;
        for x in 1..15 {
            let spine = SPINES[(hash(x, shelf + v as i32, 61) % SPINES.len() as u32) as usize];
            let short = hash(x, shelf, 62 + v).is_multiple_of(5);
            for y in y0 + i32::from(short)..y0 + 3 {
                im.put(x, y, spine);
            }
        }
        im.line(1, y0 + 3, 14, y0 + 3, 'I');
    }
    im.line(0, 15, 15, 15, 'n');
    im
}

/// A mushroom of the Fungal Deep, grown tall as a knight: a spotted cap
/// over a pale stem. Each one its own colour.
pub(super) fn mushroom(v: u32) -> Img {
    let (cap, rim, spot) = match v % 3 {
        0 => ('p', 'B', 'T'),
        1 => ('u', 'S', 'v'),
        _ => ('o', 'r', 't'),
    };
    let mut im = Img::new(16, 16);
    im.rect(6, 8, 4, 7, 'T');
    im.line(6, 8, 6, 14, 't');
    im.line(5, 15, 10, 15, 'O');
    for y in 0..9 {
        for x in 0..16 {
            let (dx, dy) = ((x as f32 - 7.5) / 7.5, (y as f32 - 8.0) / 7.0);
            if dx * dx + dy * dy > 1.0 || y > 7 {
                continue;
            }
            let edge = dx * dx + dy * dy > 0.7;
            let dotted = hash(x / 2, y / 2, 63 + v).is_multiple_of(4) && !edge;
            im.put(x, y, if dotted { spot } else if edge { rim } else { cap });
        }
    }
    im.line(1, 8, 14, 8, 't');
    im
}

/// A pillar of the Unknown: dark stone with one eye in it, that looks,
/// and now and then blinks.
pub(super) fn eye_pillar(v: u32, frame: u32) -> Img {
    let mut im = Img::new(16, 16);
    im.rect(2, 0, 12, 16, 'X');
    im.line(2, 0, 2, 15, 'g');
    im.line(13, 0, 13, 15, 'K');
    for y in (2..16).step_by(5) {
        im.line(3, y, 12, y, 'K');
    }
    let blink = hash(v as i32, frame as i32, 64).is_multiple_of(9);
    if blink {
        im.line(5, 8, 10, 8, 'g');
    } else {
        im.rect(5, 6, 6, 4, 'h');
        im.rect(7, 6, 2, 4, '2');
        im.put(7, 7, 'K');
        im.put(8, 8, 'K');
        im.put(5, 6, 'X');
        im.put(10, 9, 'X');
    }
    im
}

/// Iron bars across a two-tile doorway; `across` when the doorway runs
/// east–west along a north or south wall, so the bars stand upright.
pub(super) fn portcullis(across: bool) -> Img {
    let (w, h) = if across { (32, 16) } else { (16, 32) };
    let mut im = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (along, depth) = if across { (x, y) } else { (y, x) };
            let ink = if along % 4 == 1 {
                if depth % 5 == 0 { 'h' } else { 'J' }
            } else if depth % 5 == 2 {
                'G'
            } else {
                continue;
            };
            im.put(x, y, ink);
        }
    }
    // The jamb stones the bars run between.
    if across {
        im.rect(0, 0, 1, h, 'U');
        im.rect(w - 1, 0, 1, h, 'U');
    } else {
        im.rect(0, 0, w, 1, 'U');
        im.rect(0, h - 1, w, 1, 'U');
    }
    im
}

/// Two tiles of steps going down into the dark; a grate lies over them
/// while the room is still being fought.
pub(super) fn stairs(open: bool) -> Img {
    const RAMP: [char; 8] = ['v', 'U', 'u', 'S', 'x', 's', 'K', 'K'];
    let mut im = Img::new(32, 32);
    for y in 0..32 {
        let inset = y / 4;
        let ink = RAMP[inset as usize];
        if y % 4 == 0 {
            im.line(inset, y, 31 - inset, y, ink);
        } else {
            im.put(inset, y, ink);
            im.put(31 - inset, y, ink);
        }
    }
    im.frame(0, 0, 32, 32, 'u');
    if !open {
        for k in (2..32).step_by(4) {
            im.line(k, 0, k, 31, 'G');
            im.line(0, k, 31, k, 'j');
        }
    }
    im
}

/// The kit's brazier: blazing while the room is fought, embers once clear.
pub(super) fn brazier(blazing: bool, tick: u32, seed: u32) -> Img {
    let mut im = kit::brazier(tick, seed);
    if !blazing {
        for y in 0..5 {
            for x in 0..im.w {
                im.clear(x, y);
            }
        }
        im = im.recolor(&[('@', 'a')]);
        im.put(3 + (tick % 2) as i32, 5, '@');
    }
    im
}

/// A sapper's keg on the floor: its fuse sparks, faster and red once it
/// is armed.
pub(super) fn keg(fuse: u32, armed: bool, tick: u32) -> Img {
    let mut im = rows(&[
        "...6...",
        "...o...",
        ".bBBBb.",
        "bBIBIBb",
        "bBBBBBb",
        "bBIBIBb",
        ".bBBBb.",
    ]);
    let quick = if armed { 2 } else { 5 };
    let lit = (tick / quick).is_multiple_of(2);
    im.put(3, 0, if armed && lit { '7' } else if lit { '6' } else { '5' });
    if fuse < 12 && lit {
        im.put(2, 0, '6');
        im.put(4, 0, '6');
    }
    im
}

/// A fan's box floating down on a little parachute in Fortune's red and
/// cream, its stripes fluttering.
pub(super) fn fan_box(tick: u32) -> Img {
    let flutter = (tick / 6).is_multiple_of(2);
    let (a, b) = if flutter { ('7', '9') } else { ('9', '7') };
    let canopy = |row: &str| -> String {
        row.chars()
            .enumerate()
            .map(|(i, c)| match c {
                '*' if i % 2 == 0 => a,
                '*' => b,
                c => c,
            })
            .collect()
    };
    let inked: Vec<String> = [
        "....*****....",
        "..*********..",
        ".***********.",
        "*************",
        "8.8.......8.8",
        ".h.........h.",
        "..h.......h..",
        "...h.....h...",
        "....h...h....",
        "....OO5OO....",
        "....Oo5oO....",
        "....55555....",
        "....Oo5oO....",
        "....II5II....",
    ]
    .iter()
    .map(|row| canopy(row))
    .collect();
    rows(&inked.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Lady Tallow: sitting, trotting (two steps), or hissing; a coin in her
/// mouth when she's fetching one home. Drawn facing right.
pub(super) fn tallow(moving: bool, hissing: bool, carrying: bool, tick: u32) -> Img {
    let mut im = if hissing {
        rows(&[
            ".......5.5.",
            ".......555.",
            "h..iii.iHi.",
            "hhiHHHiHHkH",
            ".iHHHHHHH77",
            "..iHHHHHHi.",
            "..i.i..i.i.",
        ])
    } else if moving && (tick / 5).is_multiple_of(2) {
        rows(&[
            ".......5.5.",
            ".......555.",
            "h......iHi.",
            "hh....iHHkH",
            ".hi..iHHHHi",
            "..iHHHHHHi.",
            "..iHHHHHHi.",
            "..i.i..i.i.",
        ])
    } else if moving {
        rows(&[
            ".......5.5.",
            ".......555.",
            ".h.....iHi.",
            ".hh...iHHkH",
            "..hi.iHHHHi",
            "..iHHHHHHi.",
            "..iHHHHHHi.",
            "...i.ii.i..",
        ])
    } else {
        rows(&[
            "......5.5.",
            "......555.",
            "......iHi.",
            ".....iHHHi",
            ".....HkHkH",
            "h....iHHHi",
            "hh..iHHHi.",
            ".h.iHHHHHi",
            ".hiHHHHHHi",
            "..iiHHiiHi",
        ])
    };
    if carrying && !hissing {
        let x = im.w - 1;
        im.put(x, 4, '5');
        im.put(x, 5, '6');
    }
    im
}

/// The Flesher's hook, its barb red.
pub(super) fn hook() -> Img {
    rows(&["..WW.", ".W..W", "....W", "7..W.", "7WW.."])
}

/// A hob's bomb in flight.
pub(super) fn bomb(tick: u32) -> Img {
    let mut im = rows(&["..6.", ".KK.", "KKKK", ".KK."]);
    if (tick / 2).is_multiple_of(2) {
        im.put(2, 0, '5');
    }
    im
}

/// A shot drawn along its flight in native pixels: the head at `(x, y)`.
pub(super) fn shot(cv: &mut Img, (x, y): (i32, i32), (vx, vy): (f32, f32), kind: Shot) {
    let length = vx.hypot(vy).max(1e-3);
    let (ux, uy) = (vx / length, vy / length);
    let along = |cv: &mut Img, inks: &[char]| {
        // Tail first, so the head always shows.
        for (k, &ink) in inks.iter().enumerate().rev() {
            let k = k as f32;
            cv.put(
                x - (ux * k).round() as i32,
                y - (uy * k).round() as i32,
                ink,
            );
        }
    };
    let blot = |cv: &mut Img, core: char, rim: char| {
        for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
            cv.put(x + dx, y + dy, rim);
        }
        cv.put(x, y, core);
    };
    match kind {
        // A thrown blade, spinning: its turn follows where it is.
        Shot::Blade => {
            let turn = ((x + y).rem_euclid(8)) / 2;
            let arms: [(i32, i32); 2] = match turn {
                0 => [(1, 0), (0, 1)],
                1 => [(1, 1), (1, -1)],
                2 => [(0, 1), (1, 0)],
                _ => [(1, -1), (1, 1)],
            };
            for (ax, ay) in arms {
                for k in 1..=3 {
                    let ink = if k == 3 { 'W' } else { 'v' };
                    cv.put(x + ax * k, y + ay * k, ink);
                    cv.put(x - ax * k, y - ay * k, ink);
                }
            }
            cv.put(x, y, '3');
        }
        Shot::Arrow => along(cv, &['W', 'o', 'o', 'o', '2']),
        Shot::Bolt => along(cv, &['V', 'R', 'R', '2']),
        Shot::Ball => {
            blot(cv, 'W', 'J');
            cv.put(x + 1, y + 1, 'g');
        }
        Shot::Bone => {
            // A tumbling bone: knuckles at both ends, across its flight.
            let (px, py) = (-uy, ux);
            for k in -2..=2 {
                let ink = if k == -2 || k == 2 { 'H' } else { 'h' };
                cv.put(
                    x + (px * k as f32).round() as i32,
                    y + (py * k as f32).round() as i32,
                    ink,
                );
            }
            cv.put(x, y, 'H');
        }
        Shot::Orb => blot(cv, 'w', '2'),
        // The Lich's frost: a pale star of ice.
        Shot::Frost => {
            blot(cv, 'w', 'W');
            for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
                cv.put(x + dx, y + dy, 'z');
            }
        }
        // The Hexer's bolt: a frog-green glow.
        Shot::Hex => {
            blot(cv, 'w', 'Y');
            for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                cv.put(x + dx, y + dy, 'C');
            }
        }
        Shot::Ember => blot(cv, '6', '7'),
        // Pilgrim's Arrow: a long shaft of light, its head ablaze.
        Shot::Sacred => {
            along(cv, &['w', '6', '6', '5', '5', '4', '4', '@', 'a']);
            let (px, py) = (-uy, ux);
            for k in [-1.0f32, 1.0] {
                cv.put(
                    x - (ux * 1.0 - px * k).round() as i32,
                    y - (uy * 1.0 - py * k).round() as i32,
                    '6',
                );
                cv.put(
                    x - (ux * 6.0 - px * k * 1.5).round() as i32,
                    y - (uy * 6.0 - py * k * 1.5).round() as i32,
                    '5',
                );
            }
        }
        // A heat-seeker: a steel nose, a red band, smoke behind.
        Shot::Missile => along(cv, &['H', 'J', '7', 'J', 'G', 'g', 'g']),
    }
}

/// A checked rune weapon changes colour and silhouette, never game rules.
/// Its colours are signal inks: a forged shot is a player's live state.
pub(super) fn forged_shot(cv: &mut Img, x: i32, y: i32, colour: &str, shape: &str) {
    let (core, rim) = match colour {
        "ember" => ('6', '7'),
        "ice" => ('w', '3'),
        "gold" => ('6', '4'),
        "venom" => ('3', '2'),
        "void" => ('1', '0'),
        "rose" => ('9', '7'),
        _ => ('w', 'c'),
    };
    let points: &[(i32, i32)] = match shape {
        "arrow" => &[(0, -2), (-1, -1), (0, -1), (1, -1), (0, 0), (0, 1), (0, 2)],
        "shard" | "blade" => &[(0, -3), (0, -2), (0, -1), (0, 0), (0, 1), (0, 2), (0, 3)],
        "star" => &[
            (0, -2),
            (0, -1),
            (-2, 0),
            (-1, 0),
            (0, 0),
            (1, 0),
            (2, 0),
            (0, 1),
            (0, 2),
        ],
        "wave" => &[(-2, 1), (-1, 0), (0, -1), (1, 0), (2, 1)],
        _ => &[(0, -1), (-1, 0), (0, 0), (1, 0), (0, 1)],
    };
    for &(dx, dy) in points {
        cv.put(x + dx, y + dy, rim);
    }
    cv.put(x, y, core);
}

/// A treasure chest that has teeth: the lid gapes and snaps on its hop.
pub(super) fn mimic(tick: u32) -> Img {
    if (tick / 6) % 6 < 3 {
        rows(&[
            ".kkkkkkkkkk.",
            "k4rRRRRRRr4k",
            "kH.H.H.H.H.k",
            "k.7777777..k",
            "k7787887877k",
            "kH.H.H.H.H.k",
            "k4BBB55BBB4k",
            "k4BBBBBBBB4k",
            ".kkkkkkkkkk.",
        ])
    } else {
        rows(&[
            ".kkkkkkkkkk.",
            "k4rRRRRRRr4k",
            "k4R7RRRR7R4k",
            "kHHHHHHHHHHk",
            "k4BBB55BBB4k",
            "k4BBBBBBBB4k",
            ".kkkkkkkkkk.",
        ])
    }
}

/// A floor plate set with spike holes, at rest (scenery).
pub(super) fn spike_plate() -> Img {
    let mut im = Img::new(32, 32);
    im.frame(1, 1, 30, 30, 'S');
    for y in (5..28).step_by(6) {
        for x in (5..28).step_by(6) {
            im.put(x, y, 'x');
            im.put(x + 1, y, 'S');
        }
    }
    im
}

/// The spikes themselves: tips rattling in their holes, or fully up.
pub(super) fn spikes_out(up: bool, tick: u32) -> Img {
    let mut im = Img::new(32, 32);
    for (i, y) in (5..28).step_by(6).enumerate() {
        for (j, x) in (5..28).step_by(6).enumerate() {
            if up {
                im.put(x, y - 3, 'W');
                im.put(x, y - 2, 'V');
                im.put(x, y - 1, 'v');
                im.put(x + 1, y - 1, 'U');
                im.put(x, y, 'v');
            } else if (tick / 2 + (i + j) as u32).is_multiple_of(2) {
                im.put(x, y - 1, 'v');
            }
        }
    }
    im
}

/// A wall vent's grille, and its glow while it draws breath.
pub(super) fn vent(glow: u32) -> Img {
    let mut im = Img::new(6, 10);
    im.frame(0, 0, 6, 10, 'J');
    im.rect(1, 1, 4, 8, 'K');
    for y in (1..9).step_by(2) {
        let ink = match glow {
            0 => 'G',
            1..=10 => '8',
            11..=20 => '7',
            _ => '@',
        };
        im.rect(1, y, 4, 1, ink);
    }
    im
}

/// Where a rock will land: a closing ring of warning, the rock above it.
pub(super) fn falling_rock(fall: u32, total: u32) -> (Img, i32) {
    let k = fall as f32 / total.max(1) as f32;
    let r = 3.0 + k * 9.0;
    let mut im = Img::new(28, 16);
    let steps = 28;
    for i in 0..steps {
        if i % 2 == 0 {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            im.put(
                14 + (a.cos() * r).round() as i32,
                8 + (a.sin() * r * 0.5).round() as i32,
                if k < 0.35 { '@' } else { '7' },
            );
        }
    }
    // The rock drops from above, faster as it lands.
    let height = (k * k * 60.0) as i32;
    (im, height)
}

pub(super) fn rock() -> Img {
    rows(&[
        "..jGGj..", ".jGJJGj.", "jGJhJGGj", "jGJJGGgj", ".jGGGgj.", "..jjjj..",
    ])
}

pub(super) fn dust(left: u32) -> Img {
    let mut im = Img::new(20, 10);
    let spread = 12 - left as i32;
    for (dx, dy) in [(-1, 0), (1, 0), (-1, -1), (1, -1), (0, -1), (-1, 1), (1, 1)] {
        im.put(
            10 + dx * spread * 3 / 4,
            5 + dy * (spread / 3),
            if left > 6 { 'J' } else { 'G' },
        );
    }
    im
}

/// A sprite turned a quarter clockwise, about its own box.
pub(super) fn quarter_turn(im: &Img) -> Img {
    let mut out = Img::new(im.h, im.w);
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                out.set(im.h - 1 - y, x, c);
            }
        }
    }
    out
}

/// The kite shield, raised.
pub(super) fn kite_shield() -> Img {
    rows(&[
        ".hHHHh.", "hHi4iHh", "Hi474iH", "Hi474iH", ".Hi4iH.", "..HiH..", "...H...",
    ])
}

/// The Sanctuary's wishing dais: three stone steps, an arcane ring turning
/// on its top, a writing table with the Scroll of One Wish open on it,
/// candles at its corners and motes rising off the scroll. `wishing`: a
/// wish is being forged, and the ring burns and turns faster.
pub(super) fn dais(tick: u32, wishing: bool) -> Img {
    const W: i32 = 56;
    const H: i32 = 44;
    let mut im = Img::new(W, H);
    let cx = W / 2;
    // Three steps, widest at the foot, each with a lit lip and a shadowed riser.
    for (k, (half, top)) in [(27, 36), (23, 31), (19, 26)].into_iter().enumerate() {
        let bottom = top + 6;
        im.rect(cx - half, top, half * 2, bottom - top, 'u');
        im.line(cx - half, top, cx + half - 1, top, 'v');
        im.line(cx - half, bottom - 1, cx + half - 1, bottom - 1, 'S');
        im.put(cx - half, top, 'U');
        im.put(cx + half - 1, top, 'U');
        // Worn joints along each riser.
        for x in (cx - half + 3 + k as i32..cx + half - 2).step_by(7) {
            im.put(x, top + 3, 'S');
        }
    }
    // The ring on the dais top: an ellipse of runes, turning.
    let speed = if wishing { 0.22 } else { 0.05 };
    let turn = tick as f32 * speed;
    let (rx, ry, ry0) = (17.0, 3.2, 28.5);
    for i in 0..72 {
        let a = i as f32 / 72.0 * std::f32::consts::TAU;
        let (x, y) = (cx as f32 + a.cos() * rx, ry0 + a.sin() * ry);
        let ink = if wishing { '2' } else { '1' };
        im.put(x.round() as i32, y.round() as i32, ink);
    }
    for k in 0..8 {
        // A rune: a short upright stroke with a tick, riding the ring.
        let a = turn + k as f32 / 8.0 * std::f32::consts::TAU;
        let (x, y) = (
            (cx as f32 + a.cos() * rx).round() as i32,
            (ry0 + a.sin() * ry).round() as i32,
        );
        let bright = if wishing { '3' } else { '2' };
        im.put(x, y - 1, bright);
        im.put(x, y - 2, bright);
        im.put(x + if k % 2 == 0 { 1 } else { -1 }, y - 2, '1');
    }
    // The writing table: a timber top on turned legs.
    im.rect(cx - 11, 17, 22, 2, 'B');
    im.line(cx - 11, 17, cx + 10, 17, 'o');
    for x in [cx - 9, cx + 8] {
        im.line(x, 19, x, 27, 'I');
        im.put(x, 23, 'p');
    }
    im.line(cx - 9, 25, cx + 8, 25, 'b');
    // The scroll, open on the table: parchment between two rolled ends.
    im.rect(cx - 7, 10, 14, 7, '$');
    im.line(cx - 7, 10, cx + 6, 10, 'c');
    for (y, from, to) in [(12, -5, 3), (13, -5, 4), (15, -5, 1)] {
        im.line(cx + from, y, cx + to, y, 'J');
    }
    for x in [cx - 8, cx + 7] {
        im.line(x, 9, x, 17, 'o');
        im.put(x, 9, 'O');
        im.put(x, 17, 'r');
    }
    // A wax seal at the scroll's foot.
    im.put(cx + 3, 15, '7');
    im.put(cx + 4, 15, '7');
    im.put(cx + 3, 16, '8');
    // Candles at the corners of the top step.
    for x in [cx - 17, cx + 16] {
        im.line(x, 21, x, 26, 'c');
        im.put(
            x,
            20,
            if (tick / 4 + x as u32).is_multiple_of(3) {
                '6'
            } else {
                '5'
            },
        );
        im.put(x, 19, '@');
    }
    // Motes rising off the scroll, a few at a time.
    let motes = if wishing { 7 } else { 3 };
    for m in 0..motes {
        let life = 24;
        let age = ((tick + m * 11) % life) as i32;
        let drift = ((m as f32 * 1.7 + tick as f32 * 0.07).sin() * 4.0) as i32;
        let (x, y) = (cx - 4 + (m as i32 * 5) % 9 + drift, 9 - age / 3);
        if y >= 0 {
            im.put(x, y, if age < 12 { '3' } else { '2' });
        }
    }
    im
}

/// A Sanctuary's privy: a timber booth under a peaked roof, a crescent
/// moon cut in its door, an iron latch. Its door is its bottom middle.
pub(super) fn privy() -> Img {
    rows(&[
        "..............nn..............",
        "............nnIInn............",
        "..........nnIBBBBInn..........",
        "........nnIBBpppBBBInn........",
        "......nnIBBppppppppBBInn......",
        "....nnIBBBBBBBBBBBBBBBBInn....",
        "..nnnnnnnnnnnnnnnnnnnnnnnnnn..",
        "...bIIIIIIIIIIIIIIIIIIIIIIb...",
        "...bBpBBpBBnnnnnnnnBBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRr..RrnBpBBpBb...",
        "...bBpBBpBnrRR..rRrnBpBBpBb...",
        "...bBpBBpBnrRR..rRrnBpBBpBb...",
        "...bBpBBpBnrRRr..RrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRoonBpBBpBb...",
        "...bBpBBpBnrRRrrRoonBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bBpBBpBnrRRrrRRrnBpBBpBb...",
        "...bIIIIIInnnnnnnnnnIIIIIIb...",
        "..bbbbbbbbbbbbbbbbbbbbbbbbbb..",
    ])
}

/// Hung on a shut privy door: OCCUPIED, in the 5x7 hand on a dark board.
pub(super) fn occupied() -> Img {
    let word = "OCCUPIED";
    let w = super::super::ink::text_width(word) + 4;
    let mut im = Img::new(w, 11);
    im.rect(0, 0, w, 11, 'b');
    im.frame(0, 0, w, 11, 'I');
    im.text(2, 2, word, 'o');
    im
}

/// A morningstar: a spiked iron ball (drawn by a Codex Sol agent).
pub(super) fn morningstar() -> Img {
    rows(&[
        "....J....",
        "..J.J.j..",
        "...Jjg...",
        "..JUjjg..",
        "Jjjjjgggg",
        "..jjggg..",
        "...ggg...",
        "..g.g.g..",
        "....g....",
    ])
}
