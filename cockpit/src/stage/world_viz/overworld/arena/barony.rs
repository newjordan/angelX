//! King Brannoc's barony, drawn: the King and his dwarves about the
//! mine-head; the gate-hall of Caer Dwfn rising out of its ruin as the
//! dwarves build it, stage by stage; the hall inside, with the King on his
//! seat, the war table of the forges, the ledger, and the Great Forge, cold;
//! and below, a forge-hall from ruin to rebuilt to burning.

use super::super::ink::{Img, hash, text_width};
use super::super::kit;
use super::{TILE, at, fire, home, stand};
use crate::drive::together_realm::Spoils;
use crate::drive::together_shooter::barony::{
    self, ANVILS, CAMP_FIRE, CAMP_TENT, Court, FORGE_PLATE, FORGES, FURNACE, GREAT_FORGE,
    HALL_PLATE, LECTERN, LECTERN_PLATE, PILLARS, SEAT_DAIS, TABLE_PLATE, WAR_TABLE,
};
use crate::drive::together_shooter::folk::{self, Figure, Folk, Pose};
use crate::drive::together_shooter::world::OLD_HALL;
use crate::drive::together_shooter::{RoomKind, Run};
use crate::stage::world_viz::overworld::light::Light;

// ── The King and his dwarves ─────────────────────────────────────────────

/// King Brannoc Onehorn: one bone horn on his helm (the other snapped to a
/// stub), a red beard braided to his belt, mail, a round shield with the
/// foaming tankard, a notched axe.
pub(super) const KING: [&str; 20] = [
    "..T...................",
    ".Tt...................",
    ".tT...................",
    ".ttTGhhhG.............",
    "..tGhiHhhG............",
    "..GhiHhhhhGt..........",
    "..GjjjjjjjjG..........",
    ".hh.OKOOKOO...........",
    "hHiH.OOooO............",
    "hi.HRROOOORR..GGGGG...",
    "hiHbpRRrRRRRpGpppppG..",
    ".hhbpRRRRrRRpGpHiHpG..",
    "...bjRRrRRRRjGpoooOG..",
    "...bjjRRRRrjjGpOOOoG..",
    "...bGjjRRRjjGGpOOOpG..",
    "...bOjjjOjjjjOGpppG...",
    "...b.bbbObbbb..GGG....",
    "...b.jjjjjjjj.........",
    "...b..nnn.nnn.........",
    "....nnnnn.nnnnn.......",
];

/// The King on his seat.
const SEATED: [&str; 16] = [
    "..T..........",
    ".Tt..........",
    ".tT..........",
    ".ttTGhhhG....",
    "..tGhiHhhG...",
    "..GhiHhhhhGt.",
    "..GjjjjjjjjG.",
    "...hOKOOKOO..",
    "..hHOOOooO...",
    "...HRROOOORR.",
    "..jjpRRrRRRRp",
    "..jjpRRRRrRRp",
    "..OjjRRrRRRRj",
    "...jjjRRRRrjj",
    "...jjjjjjjjj.",
    "...nnn...nnn.",
];

const MINER: [&str; 12] = [
    ".....hh.....",
    "..666hJh....",
    ".Jhh5h.h....",
    ".JhhhhJ.h...",
    ".OKOOKO..b..",
    "IrrrrrrI.b..",
    "IrrIrrrrb...",
    "GGrrrrGGb...",
    "GGGGGGGGO...",
    ".bbbObbb....",
    ".n....n.....",
    "nn...nn.....",
];

/// A miner coming up with a sack of ore on his back.
const PORTER: [&str; 12] = [
    "..ttt.......",
    ".tTttt......",
    ".tttttJjjJ..",
    ".ttt.jjjjjj.",
    "..tt.OKOOK..",
    "...tIIIIIII.",
    "...IIhIIIIO.",
    "..PPPhIIhPP.",
    "..PPPPPPPPP.",
    "...bbObbb...",
    "...n...n....",
    "..nn..nn....",
];

/// A builder with a dressed stone on his shoulder (empty-handed, the top
/// three rows are not drawn).
const BUILDER: [&str; 13] = [
    "...VVVVV....",
    "...VWVVVv...",
    "...vvvvv....",
    "..uJhhhJu...",
    ".OhhhhhhhO..",
    ".OOKOOKOO...",
    ".hhhhhhhh...",
    ".hHhhHhhh...",
    ".jjhhhhjjr..",
    ".jjjjjjjjR..",
    "..bbObbb....",
    "..n....n....",
    ".nn...nn....",
];

const GUARD: [&str; 13] = [
    ".........H..",
    ".........i..",
    "...JhhG..b..",
    "..JhiHhG.b..",
    "..GjjjjG.b..",
    "..OKOOKO.b..",
    "..ppRRppOb..",
    ".GpppGRRbb..",
    ".pHiHpjjjb..",
    ".pOOOpjjjb..",
    ".GpppGbbb...",
    "...n...n....",
    "..nn..nn....",
];

/// A smith at the anvil, the hammer up (and, swung, down).
const SMITH: [&str; 12] = [
    ".......HH...",
    ".......Hh...",
    ".......b....",
    "..IIII.b....",
    ".IIIIIIb....",
    ".OKOOKOO....",
    ".hhhhhhO....",
    ".hHhhhh.....",
    "OBBBBBB.....",
    ".BBBBBB.....",
    ".BBpBBB.....",
    ".nn..nn.....",
];

/// Walking: the last two rows (the legs) step together and apart.
fn stride(rows: &[&str], t: u32) -> Img {
    let mut im = Img::from_rows(rows);
    if (t / 6).is_multiple_of(2) {
        return im;
    }
    let n = rows.len() as i32;
    let w = im.w;
    for y in n - 2..n {
        for x in 0..w {
            im.clear(x, y);
        }
    }
    let legs = Img::from_rows(&["...n.n......", "..nn.nn....."]);
    im.stamp(&legs, (w - 12) / 2 + 1, n - 2);
    im
}

/// One of the folk, as they look this tick.
pub(super) fn figure(f: &Figure, tick: u32) -> Img {
    let im = match (f.folk, f.pose) {
        (Folk::King, Pose::Seated) => Img::from_rows(&SEATED),
        (Folk::King, _) => Img::from_rows(&KING),
        (Folk::Guard, _) => {
            // Guards shift their weight now and then.
            let mut im = Img::from_rows(&GUARD);
            if (tick / 45 + f.x as u32).is_multiple_of(5) {
                im = im.flip_h();
            }
            im
        }
        (Folk::Miner, Pose::Walk) if f.loaded => stride(&PORTER, f.t),
        (Folk::Miner, Pose::Walk) => stride(&MINER, f.t),
        (Folk::Miner, _) => Img::from_rows(&PORTER),
        (Folk::Builder, Pose::Work) => {
            // Setting a stone: the mallet comes down.
            let mut im = Img::from_rows(&BUILDER[3..]);
            if (f.t / 8).is_multiple_of(2) {
                im.line(9, 2, 11, 0, 'r');
                im.put(11, 0, 'G');
            } else {
                im.line(9, 5, 11, 5, 'r');
                im.put(11, 5, 'G');
            }
            im
        }
        (Folk::Builder, _) if f.loaded => stride(&BUILDER, f.t),
        (Folk::Builder, _) => stride(&BUILDER[3..], f.t),
        (Folk::Carter, _) => carter(f.loaded, f.t),
        (Folk::Smith, _) => {
            let mut im = Img::from_rows(&SMITH);
            if (f.t / 9).is_multiple_of(2) {
                // The hammer down on the anvil.
                for y in 0..4 {
                    for x in 6..10 {
                        im.clear(x, y);
                    }
                }
                im.line(7, 6, 10, 8, 'b');
                im.put(10, 8, 'H');
                im.put(11, 8, 'H');
            }
            im
        }
    };
    if f.east || matches!(f.folk, Folk::King | Folk::Guard) {
        im
    } else {
        im.flip_h()
    }
}

/// A carter pushing a handcart east: full of ore going out, empty back.
fn carter(loaded: bool, t: u32) -> Img {
    let mut im = Img::new(30, 14);
    let cart = kit::handcart();
    im.stamp(&cart, 12, 14 - cart.h);
    if loaded {
        for k in 0..6 {
            im.rect(
                14 + k * 2,
                14 - cart.h - 2 + (k % 2),
                2,
                2,
                if k % 2 == 0 { 'J' } else { 'R' },
            );
        }
    }
    let dwarf = stride(&MINER, t);
    im.stamp(&dwarf, 0, 14 - dwarf.h);
    im
}

// ── The mine-head: the camp, the gate-hall rising ───────────────────────

/// The King's camp before his hall stands: a tent of the tankard's red
/// and timber, a fire, a stack of ore sacks.
fn camp(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (tx, ty) = at(CAMP_TENT.0 * 2.0, CAMP_TENT.1 * 2.0);
    let tent = Img::from_rows(&[
        "..........b...........",
        "..........bpp.........",
        "..........bppp........",
        "..........b...........",
        ".........rRr..........",
        "........rRRRr.........",
        ".......rRRoRRr........",
        "......rRRRoRRRr.......",
        ".....rRRRRoRRRRr......",
        "....rRRRRRoRRRRRr.....",
        "...rRRRRRRoRRRRRRr....",
        "..rRRRRRRkkkRRRRRRr...",
        ".rRRRRRRkkkkkRRRRRRr..",
        "rRRRRRRRkkkkkRRRRRRRr.",
        "rRRRRRRkkkkkkkRRRRRRr.",
        "bbbbbbbbbbbbbbbbbbbbbb",
    ]);
    cv.stamp(&tent, tx - tent.w / 2, ty - tent.h + 1);
    let (fx, fy) = at(CAMP_FIRE.0 * 2.0, CAMP_FIRE.1 * 2.0);
    let im = kit::brazier(tick / 4, 61);
    cv.rect(fx - 5, fy - 1, 11, 2, 'j');
    cv.stamp(&im, fx - im.w / 2, fy - im.h + 1);
    lights.push(fire(fx, fy - 4, 46.0, 0.42));
}

/// The barony's stockpile: ore sacks and a heap, growing with the miners.
fn stockpile(cv: &mut Img, run: &Run) {
    if !run.home.barony.holds_workings() {
        return;
    }
    let (sx, sy) = at(
        barony::STOCKPILE_AT.0 * 2.0 + 1.0,
        barony::STOCKPILE_AT.1 * 2.0 + 1.0,
    );
    let n = run.home.barony.miners().min(6) as i32;
    for k in 0..n + 2 {
        let (x, y) = (sx - 10 + (k % 4) * 6, sy - (k / 4) * 4);
        cv.rect(x, y - 4, 5, 5, 't');
        cv.put(x + 2, y - 5, 'T');
        cv.line(x, y, x + 4, y, 'P');
    }
}

/// The gate-hall's ruin, as it fell: broken walls of dressed stone, its
/// doorway's arch still standing, rubble inside. `cleared` (0 to 1) of the
/// rubble has been carried away.
fn ruin(cv: &mut Img, cleared: f32) {
    let (hc, hr, hw, hh) = OLD_HALL;
    let (hx, hy, w2, h2) = (hc * TILE, hr * TILE, hw * TILE, hh * TILE);
    let t = 9;
    for x in hx..hx + w2 {
        for y in hy..hy + h2 {
            let wall = x < hx + t || x >= hx + w2 - t || y < hy + t || y >= hy + h2 - t;
            if !wall {
                let flag = (x - hx) % 12 == 0 || (y - hy) % 10 == 0;
                if flag && hash(x / 12, y / 10, 78) % 3 != 0 {
                    cv.put(x, y, 'g');
                }
                continue;
            }
            let span = if x < hx + t || x >= hx + w2 - t {
                (y - hy) / 14
            } else {
                (x - hx) / 14
            };
            let fallen = hash(span, i32::from(x < hx + t) + 2 * i32::from(y < hy + t), 79) % 4 == 0;
            if fallen {
                if hash(x, y, 80) % 3 == 0 {
                    cv.put(
                        x,
                        y,
                        if hash(x, y, 82).is_multiple_of(2) {
                            'G'
                        } else {
                            'J'
                        },
                    );
                }
                continue;
            }
            let course = (y - hy) / 4;
            let joint = (y - hy) % 4 == 0 || (x + course * 5) % 8 == 0;
            let lit = (y - hy) % 4 == 1;
            cv.put(
                x,
                y,
                if joint {
                    'G'
                } else if lit {
                    'i'
                } else {
                    'h'
                },
            );
        }
    }
    let dx = hx + w2 / 2;
    for y in hy + h2 - t..hy + h2 {
        for x in dx - 8..dx + 8 {
            cv.put(x, y, 'k');
        }
    }
    for k in 0..24 {
        let a = std::f32::consts::PI * (k as f32 / 23.0);
        for r in [10.0f32, 11.0, 12.0] {
            let (x, y) = (
                dx as f32 - a.cos() * r,
                (hy + h2 - t) as f32 - a.sin() * (r - 3.0),
            );
            cv.put(x as i32, y as i32, if r == 10.0 { 'i' } else { 'J' });
        }
    }
    let heaps = ((1.0 - cleared) * 10.0).round() as i32;
    for k in 0..heaps {
        let (rx, ry) = (
            hx + 10 + (hash(k, 1, 81) % (w2 as u32 - 24)) as i32,
            hy + 10 + (hash(k, 2, 81) % (h2 as u32 - 24)) as i32,
        );
        cv.rect(rx, ry, 5, 3, if k % 2 == 0 { 'J' } else { 'G' });
        cv.put(rx + 1, ry - 1, 'h');
    }
}

/// Scaffolding against a wall face: poles and planks.
fn scaffold(cv: &mut Img, x0: i32, x1: i32, y0: i32, y1: i32) {
    for x in (x0..x1).step_by(18) {
        cv.line(x, y0, x, y1, 'r');
    }
    for y in (y0..y1).step_by(10) {
        cv.line(x0, y, x1, y, 'o');
        cv.line(x0, y + 1, x1, y + 1, 'b');
    }
}

/// The gate-hall at the mine-head: a ruin, the rubble going, its walls
/// rising whole, its roof, its great door; then finished, the door open on
/// warm light and the King's banner over it.
pub(super) fn gate_hall(cv: &mut Img, run: &Run, lights: &mut Vec<Light>, tick: u32) {
    let b = &run.home.barony;
    if b.here() && b.court != Court::Sworn {
        camp(cv, lights, tick);
    }
    stockpile(cv, run);
    let Some(p) = b.progress("hall") else {
        ruin(cv, 0.0);
        return;
    };
    let (hc, hr, hw, hh) = OLD_HALL;
    let (hx, hy, w, h) = (hc * TILE, hr * TILE, hw * TILE, hh * TILE);
    // A quarter at a time: the rubble cleared, the walls, the roof, the
    // front and its door.
    let stage = |from: u32| ((p.saturating_sub(from)) as f32 / 250.0).min(1.0);
    if p < 250 {
        ruin(cv, stage(0));
        scaffold(cv, hx + 4, hx + w - 4, hy + h - 26, hy + h - 2);
        return;
    }
    let roof_h = 34;
    // The walls, risen course by course from the foot.
    let walls = stage(250);
    let wall_top = hy + roof_h;
    let risen = wall_top + ((1.0 - walls) * (h - roof_h) as f32) as i32;
    for y in risen..hy + h {
        for x in hx..hx + w {
            let course = (y - hy) / 4;
            let joint = (y - hy) % 4 == 0 || (x + course * 5) % 10 == 0;
            cv.put(
                x,
                y,
                if joint {
                    'G'
                } else if (y - hy) % 4 == 1 {
                    'i'
                } else {
                    'h'
                },
            );
        }
    }
    // The roof, laid from the eaves up: slate in its courses.
    let roof = stage(500);
    if roof > 0.0 {
        let laid = ((1.0 - roof) * roof_h as f32) as i32;
        for y in hy + laid..hy + roof_h {
            let inset = (roof_h - (y - hy)) / 3;
            for x in hx - 3 + inset..hx + w + 3 - inset {
                let slate = (x + (y / 3) * 2) % 6 == 0 || y % 3 == 0;
                cv.put(x, y, if slate { 'x' } else { 'S' });
            }
        }
        cv.line(hx - 3, hy + roof_h, hx + w + 2, hy + roof_h, 'g');
    }
    // The front: the great door and the King's banner over it.
    let front = stage(750);
    let (dx, dy) = (hx + w / 2, hy + h);
    if front > 0.0 || p >= 1000 {
        let open = p >= 1000;
        let door_h = 20;
        for y in dy - door_h..dy {
            for x in dx - 9..dx + 9 {
                let arch = {
                    let (ex, ey) = ((x - dx) as f32 / 9.0, (y - (dy - door_h + 9)) as f32 / 9.0);
                    y >= dy - door_h + 9 || ex * ex + ey * ey <= 1.0
                };
                if !arch {
                    continue;
                }
                let ink = if open {
                    if hash(x, y, 91).is_multiple_of(4) {
                        '@'
                    } else {
                        '8'
                    }
                } else if (x - dx).abs() == 0 || (y - dy) % 5 == 0 {
                    'b'
                } else {
                    'B'
                };
                cv.put(x, y, ink);
            }
        }
        if open {
            lights.push(fire(dx, dy - 8, 52.0, 0.4));
        }
        if front >= 0.5 || p >= 1000 {
            banner(cv, dx - 8, hy + roof_h + 1);
        }
    }
    if roof >= 1.0 {
        // A parapet along the eaves, crenellated: a dwarf's hall is a fort.
        for x in (hx - 2..hx + w + 2).step_by(6) {
            cv.rect(x, hy + roof_h - 4, 4, 4, 'h');
            cv.line(x, hy + roof_h - 4, x + 3, hy + roof_h - 4, 'i');
        }
    }
    if front >= 1.0 || p >= 1000 {
        // Two stout pillars flanking the door, and a band of runes over it.
        for px in [dx - 16, dx + 12] {
            cv.rect(px, dy - 26, 5, 26, 'i');
            cv.line(px, dy - 26, px, dy - 1, 'H');
            cv.rect(px - 1, dy - 28, 7, 3, 'J');
        }
        for k in 0..7 {
            cv.put(dx - 9 + k * 3, dy - 23, if k % 2 == 0 { 'O' } else { 'o' });
        }
    }
    if p < 1000 {
        scaffold(cv, hx + 2, hx + w - 2, risen.max(hy + 4), hy + h - 2);
    } else {
        // Braziers either side of the door, lit.
        for (k, x) in [(0u32, dx - 22), (1, dx + 21)] {
            let im = kit::brazier(tick / 4, 70 + k);
            cv.stamp(&im, x - im.w / 2, dy - im.h + 1);
            lights.push(fire(x, dy - im.h + 3, 44.0, 0.34));
        }
    }
}

/// The King's banner: the foaming tankard on the red of Caer Dwfn.
fn banner(cv: &mut Img, x: i32, y: i32) {
    let im = Img::from_rows(&[
        "bbbbbbbbbbbbbbbb",
        ".RRRRRRRRRRRRRR.",
        ".RRRHiHiHiHRRRR.",
        ".RRHiHiHiHiHRRR.",
        ".RROOOOOOOOORRR.",
        ".RROOOOOOOOOooR.",
        ".RROOOoOOOOORoR.",
        ".RROOOoOOOOORoR.",
        ".RROOOOOOOOOooR.",
        ".RROOOOOOOOORRR.",
        ".RRROOOOOOORRRR.",
        ".RRRRRRRRRRRRRR.",
        ".RRRRRRRRRRRRRR.",
        ".RRRR......RRRR.",
        ".RRR........RRR.",
        ".RR..........RR.",
    ]);
    cv.stamp(&im, x, y);
}

// ── Inside the King's Hall ───────────────────────────────────────────────

/// The hall inside: flagstones and a red runner up to the dais, pillars,
/// braziers, the King's banner behind his seat, the war table, the ledger
/// on its lectern, and at the east end the Great Forge: the crown of the
/// forges, cold until the first fire is brought home.
pub(super) fn kings_hall(cv: &mut Img, run: &Run, lights: &mut Vec<Light>, tick: u32) {
    let b = &run.home.barony;
    // The runner from the door to the dais.
    for y in 4 * TILE..13 * TILE {
        for x in 11 * TILE + 2..13 * TILE - 2 {
            let edge = x == 11 * TILE + 2 || x == 13 * TILE - 3;
            cv.put(
                x,
                y,
                if edge {
                    'O'
                } else if (x + y) % 9 == 0 {
                    'p'
                } else {
                    '8'
                },
            );
        }
    }
    // The dais: steps of pale stone, the seat of dressed stone on it, the
    // banner on the wall behind.
    let (c, r, w, h) = SEAT_DAIS;
    let (x0, y0) = (c * TILE, r * TILE);
    for k in 0..3 {
        cv.rect(
            x0 - 4 + k * 4,
            y0 + h * TILE - 6 - k * 6,
            w * TILE + 8 - k * 8,
            6,
            ['J', 'h', 'i'][k as usize],
        );
    }
    cv.rect(x0 + 18, y0 - 4, 28, 26, 'G');
    cv.rect(x0 + 20, y0 - 2, 24, 22, 'h');
    cv.line(x0 + 18, y0 - 4, x0 + 45, y0 - 4, 'i');
    banner(cv, x0 + 24, 1);
    // Pillars of dressed stone.
    for &(pc, pr) in &PILLARS {
        let (px, py) = (pc * TILE, pr * TILE);
        cv.rect(px + 2, py - 10, 12, 26, 'h');
        cv.rect(px + 1, py - 12, 14, 3, 'i');
        cv.rect(px + 1, py + 13, 14, 3, 'J');
        cv.line(px + 4, py - 9, px + 4, py + 12, 'G');
    }
    // Braziers along the walls.
    for (k, (bx, by)) in [
        (2 * TILE, 7 * TILE),
        (21 * TILE, 11 * TILE),
        (8 * TILE, TILE + 4),
        (16 * TILE, TILE + 4),
    ]
    .into_iter()
    .enumerate()
    {
        let im = kit::brazier(tick / 4, 80 + k as u32);
        cv.stamp(&im, bx - im.w / 2, by);
        lights.push(fire(bx, by + 3, 54.0, 0.36));
    }
    // The war table and its map of the forges.
    war_table(cv, b);
    // The ledger on its lectern.
    let (lc, lr, _, _) = LECTERN;
    let (lx, ly) = (lc * TILE, lr * TILE);
    cv.rect(lx + 6, ly + 6, 4, 10, 'B');
    cv.rect(lx + 1, ly + 1, 14, 7, 'b');
    cv.rect(lx + 2, ly + 2, 5, 5, 'T');
    cv.rect(lx + 9, ly + 2, 5, 5, 'T');
    cv.line(lx + 3, ly + 4, lx + 6, ly + 4, 'P');
    cv.line(lx + 10, ly + 4, lx + 13, ly + 4, 'P');
    // The Great Forge: cold, its mouth black, until the first fire.
    great_forge(cv, b, lights, tick);
    // The great door, inside: light from the mine-head.
    let (ec, er, ew, _) = barony::HALL_EXIT;
    for y in er * TILE + 4..(er + 2) * TILE {
        for x in ec * TILE..(ec + ew) * TILE {
            cv.put(x, y, if (x + y) % 3 == 0 { 'P' } else { 'I' });
        }
    }
    home::plate_marked(cv, LECTERN_PLATE, false, &book_mark());
    home::plate_marked(cv, TABLE_PLATE, false, &anvil_mark());
}

fn book_mark() -> Img {
    Img::from_rows(&["OO.OO", "OO.OO", "OO.OO", ".OOO."])
}

fn anvil_mark() -> Img {
    Img::from_rows(&["OOOOO", ".OOO.", "..O..", ".OOO."])
}

/// Where each forge sits on the war table's map (pixels from its corner):
/// the Great Forge at the head, the halls below it.
const MAP_AT: [(i32, i32); 6] = [(10, 22), (22, 26), (34, 22), (46, 26), (54, 18), (32, 8)];

fn war_table(cv: &mut Img, b: &barony::Barony) {
    let (c, r, w, h) = WAR_TABLE;
    let (x0, y0, tw, th) = (c * TILE, r * TILE, w * TILE, h * TILE);
    cv.rect(x0, y0 + 2, tw, th - 4, 'B');
    cv.rect(x0 + 2, y0 + 4, tw - 4, th - 9, 'T');
    cv.line(x0, y0 + th - 3, x0 + tw - 1, y0 + th - 3, 'b');
    // The chain: each forge joined to the Great Forge.
    let crown = MAP_AT[5];
    for &(mx, my) in &MAP_AT[..5] {
        cv.line(x0 + mx, y0 + my - 4, x0 + crown.0, y0 + crown.1, 'P');
    }
    for (i, f) in FORGES.iter().enumerate() {
        let (mx, my) = MAP_AT[i];
        let (x, y) = (x0 + mx, y0 + my - 4);
        let ink = if b.is_lit(f.id) {
            '6'
        } else if f.work.is_some_and(|w| b.labour(w).is_some()) || (i == 0 && b.holds_workings()) {
            'o'
        } else {
            'b'
        };
        cv.rect(x - 1, y - 1, 3, 3, ink);
        if i == 5 {
            cv.frame(x - 3, y - 3, 7, 7, 'p');
        }
    }
}

/// The Great Forge at the hall's east end: a stone furnace taller than the
/// hall's doors, its chimney into the rock. Cold: a black mouth, a chain
/// across it. With the first fire won, an ember lantern hangs by it.
fn great_forge(cv: &mut Img, b: &barony::Barony, lights: &mut Vec<Light>, tick: u32) {
    let (c, r, w, h) = GREAT_FORGE;
    let (x0, y0, fw, fh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    for y in y0..y0 + fh {
        let inset = if y < y0 + 30 { (y0 + 30 - y) / 4 } else { 0 };
        for x in x0 + inset..x0 + fw - inset {
            let course = (y - y0) / 5;
            let joint = (y - y0) % 5 == 0 || (x + course * 7) % 11 == 0;
            cv.put(x, y, if joint { 'g' } else { 'j' });
        }
    }
    // The mouth, facing the hall.
    let (mx, my) = (x0 + 6, y0 + 46);
    for y in my..my + 30 {
        for x in mx..mx + fw - 18 {
            let (ex, ey) = (
                (x - mx - (fw - 18) / 2) as f32 / 18.0,
                (y - my - 10) as f32 / 10.0,
            );
            if y >= my + 10 || ex * ex + ey * ey <= 1.0 {
                cv.put(x, y, 'k');
            }
        }
    }
    // A chain across the cold mouth.
    for x in (mx..mx + fw - 18).step_by(3) {
        cv.put(x, my + 18 + ((x - mx) / 6) % 2, 'G');
    }
    // The first fire, won from Cinderjaw, kept in a lantern for this
    // forge.
    if b.has_done("first-fire") {
        let (lx, ly) = (x0 - 6, y0 + 50);
        cv.line(lx, ly - 12, lx, ly - 4, 'b');
        cv.rect(lx - 3, ly - 4, 7, 8, 'b');
        let flame = if (tick / 5).is_multiple_of(2) {
            '@'
        } else {
            '6'
        };
        cv.rect(lx - 2, ly - 3, 5, 6, '8');
        cv.rect(lx - 1, ly - 2, 3, 4, flame);
        lights.push(fire(lx, ly, 36.0, 0.5));
    }
}

// ── A forge-hall below ───────────────────────────────────────────────────

/// A forge-hall of Caer Dwfn below: its furnace and anvils. Found in ruin
/// once the King holds the halls; rising course by course while it is
/// rebuilt; whole and cold, waiting for fire; then burning, smoke and
/// sparks out of its chimney.
pub(super) fn forge(cv: &mut Img, run: &Run, lights: &mut Vec<Light>, tick: u32) {
    let b = &run.home.barony;
    let Some(f) = barony::forge_of(run.dungeon.pack) else {
        return;
    };
    let progress = f.work.and_then(|w| b.progress(w));
    let lit = b.is_lit(f.id);
    let (c, r, w, h) = FURNACE;
    let (x0, y0, fw, fh) = (c * TILE, r * TILE, w * TILE, h * TILE);
    // How much of the furnace stands: a broken stump, then courses.
    let standing = match progress {
        None => 0.3,
        Some(p) => 0.3 + 0.7 * p as f32 / 1000.0,
    };
    let top = y0 + fh - (standing * fh as f32) as i32;
    for y in top..y0 + fh {
        let inset = if y < y0 + 20 { (y0 + 20 - y) / 3 } else { 0 };
        for x in x0 + inset..x0 + fw - inset {
            // A ruin's courses are broken along the top.
            if progress.is_none() && y < top + 6 && hash(x / 5, 0, 97) % 3 == 0 {
                continue;
            }
            let course = (y - y0) / 4;
            let joint = (y - y0) % 4 == 0 || (x + course * 5) % 9 == 0;
            cv.put(
                x,
                y,
                if joint {
                    'X'
                } else if (y - y0) % 4 == 1 {
                    'J'
                } else {
                    'G'
                },
            );
        }
    }
    // The chimney up into the rock, once the furnace stands to its top.
    if progress.is_some_and(|p| p >= 1000) {
        let (cx0, cw) = (x0 + fw / 2 - 9, 18);
        for y in TILE..y0 + 4 {
            for x in cx0..cx0 + cw {
                let joint = y % 4 == 0 || (x + (y / 4) * 5) % 9 == 0;
                cv.put(x, y, if joint { 'X' } else { 'G' });
            }
        }
        cv.rect(cx0 - 2, TILE - 3, cw + 4, 3, 'J');
    }
    // The mouth: black when cold, burning when lit.
    let (mx, my) = (x0 + fw / 2, y0 + fh - 4);
    for y in my - 18..my {
        for x in mx - 10..mx + 10 {
            let (ex, ey) = ((x - mx) as f32 / 10.0, (y - (my - 10)) as f32 / 10.0);
            if y >= top && (y >= my - 10 || ex * ex + ey * ey <= 1.0) {
                let ink = if lit {
                    match hash(x, y + (tick / 3) as i32, 99) % 5 {
                        0 => '6',
                        1 | 2 => '@',
                        _ => '7',
                    }
                } else {
                    'k'
                };
                cv.put(x, y, ink);
            }
        }
    }
    if lit {
        lights.push(fire(mx, my - 8, 90.0, 0.6));
        // Smoke out of the chimney, and sparks.
        let smoke = kit::smoke(tick);
        cv.stamp(&smoke, mx - smoke.w / 2, TILE - smoke.h + 2);
    }
    if progress.is_none() {
        // Rubble round the ruin.
        for k in 0..12 {
            let (rx, ry) = (
                x0 - 16 + (hash(k, 3, 98) % (fw as u32 + 32)) as i32,
                y0 + fh + 2 + (hash(k, 4, 98) % 10) as i32,
            );
            cv.rect(rx, ry, 4, 3, if k % 2 == 0 { 'G' } else { 'g' });
        }
    } else if progress.is_some_and(|p| p < 1000) {
        scaffold(cv, x0 - 4, x0 + fw + 4, top.max(y0 + 2), y0 + fh - 2);
    }
    // Bellows on the side, once rebuilt.
    if progress == Some(1000) {
        cv.rect(x0 + fw + 2, y0 + fh - 16, 10, 8, 'B');
        cv.line(x0 + fw + 2, y0 + fh - 12, x0 + fw, y0 + fh - 12, 'b');
    }
    // The anvils on their stumps.
    for &(ac, ar) in &ANVILS {
        let (ax, ay) = (ac * TILE, ar * TILE);
        cv.rect(ax + 4, ay + 8, 8, 8, 'B');
        cv.rect(ax + 1, ay + 4, 14, 4, 'G');
        cv.rect(ax + 3, ay + 2, 10, 2, 'J');
        cv.line(ax + 1, ay + 4, ax + 14, ay + 4, 'h');
    }
    home::plate_marked(cv, FORGE_PLATE, false, &anvil_mark());
}

// ── Figures and boards ───────────────────────────────────────────────────

/// The folk in sight, and sparks off a smith's anvil.
pub(super) fn folk(cv: &mut Img, run: &Run) {
    let tick = run.tick as u32;
    let room = run.room().kind;
    let pack = run.dungeon.pack;
    for f in folk::figures(&run.home, room, pack, run.tick) {
        let im = figure(&f, tick);
        let (x, y) = at(f.x, f.y);
        stand(cv, &im, x, y);
        if f.folk == Folk::Smith && (f.t / 9).is_multiple_of(2) && f.t % 9 < 4 {
            let sx = if f.east { x + 6 } else { x - 6 };
            for k in 0..4 {
                cv.put(sx + (k - 2) * 2, y - 8 - (f.t % 9) as i32 - k % 2, '6');
            }
        }
    }
}

/// Wrap words to boards' lines of at most `width` characters.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let last = lines.last_mut().expect("one line");
        if !last.is_empty() && last.len() + 1 + word.len() > width {
            lines.push(word.to_string());
        } else {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    lines
}

/// A board with a price in spoils (short ones in red) under its lines, and
/// the hold bar.
fn price_board(
    lines: &[(String, char)],
    price: &Spoils,
    treasury: &Spoils,
    buying: Option<u32>,
) -> Img {
    let mut rows = lines.to_vec();
    rows.push((String::new(), 'h'));
    if buying.is_some() {
        rows.push((String::new(), 'h'));
    }
    let mut im = home::board(&rows);
    let row_y = 3 + lines.len() as i32 * 9;
    let items: Vec<(Img, String, bool)> = crate::drive::together_realm::Spoil::ALL
        .into_iter()
        .filter(|&s| price.get(s) > 0)
        .map(|s| {
            (
                home::spoil_mark(s),
                price.get(s).to_string(),
                treasury.get(s) >= price.get(s),
            )
        })
        .collect();
    let width: i32 = items
        .iter()
        .map(|(m, n, _)| m.w + 2 + text_width(n) + 7)
        .sum::<i32>()
        - 7;
    let mut x = (im.w - width) / 2;
    for (mark, n, have) in &items {
        im.stamp(mark, x, row_y + (7 - mark.h) / 2);
        x += mark.w + 2;
        x = im.text(x, row_y, n, if *have { 'H' } else { '7' }) + 7;
    }
    if let Some(buying) = buying {
        let bar_y = row_y + 9;
        let label = "HOLD F";
        let lw = text_width(label);
        let bar_w = 40;
        let start = (im.w - (lw + 4 + bar_w)) / 2;
        let after = im.text(start, bar_y, label, 'O');
        im.frame(after + 4, bar_y + 1, bar_w, 5, 'b');
        let hold = crate::drive::together_shooter::home::BUY_HOLD;
        let fill = (buying.min(hold) * (bar_w as u32 - 2) / hold) as i32;
        if fill > 0 {
            im.rect(after + 5, bar_y + 2, fill, 3, '5');
        }
    }
    im
}

fn standing_on(run: &Run, plate: (i32, i32, i32, i32)) -> Option<u32> {
    barony::on_plate(run, plate).map(|id| run.players.get(&id).map_or(0, |h| h.buying))
}

/// Over everything in the barony's rooms: the board of the plate a knight
/// stands on.
pub(super) fn boards(cv: &mut Img, run: &Run) {
    let b = &run.home.barony;
    let room = run.room().kind;
    let place = |cv: &mut Img, board: &Img, plate: (i32, i32, i32, i32), above: bool| {
        let (c, r, w, h) = plate;
        let x = (c * TILE + w * TILE / 2 - board.w / 2).clamp(2, cv.w - board.w - 2);
        let y = if above {
            (r * TILE - board.h - 6).max(2)
        } else {
            ((r + h) * TILE + 4).min(cv.h - board.h - 2)
        };
        cv.stamp(board, x, y);
    };
    // The King's mission, at the plate before him.
    if b.court >= Court::Met
        && let Some(_) = standing_on(run, barony::king_plate(b))
        && barony::king_at(b).is_some_and(|(r, _)| r == room)
    {
        let mut lines = vec![("KING BRANNOC OF CAER DWFN".to_string(), '9')];
        match b
            .mission
            .as_ref()
            .and_then(|m| barony::mission(&m.id).map(|d| (m, d)))
        {
            Some((pinned, m)) => {
                lines.push((m.title.to_uppercase(), '5'));
                for l in wrap(m.asks, 38) {
                    lines.push((l.to_uppercase(), 'h'));
                }
                lines.push((
                    format!(
                        "{}/{}   PAYS {}",
                        pinned.have,
                        m.need,
                        m.reward().label().to_uppercase()
                    ),
                    'O',
                ));
            }
            None => lines.push(("NO MISSION NOW: BUILD, AND RELIGHT".into(), 'h')),
        }
        let board = home::board(&lines);
        place(cv, &board, barony::king_plate(b), false);
        return;
    }
    match room {
        RoomKind::MineHead => {
            if let Some(buying) = standing_on(run, HALL_PLATE)
                && b.court >= Court::Met
            {
                let def = barony::work("hall").expect("the hall");
                let board = match b.progress("hall") {
                    None => price_board(
                        &[
                            (def.name.to_uppercase(), '9'),
                            (def.says.to_uppercase(), 'h'),
                        ],
                        &def.cost(),
                        &run.treasury,
                        Some(buying),
                    ),
                    Some(p) if p < 1000 => home::board(&[
                        (def.name.to_uppercase(), '9'),
                        (format!("{}% BUILT", p / 10), '5'),
                        (format!("{} DWARVES AT WORK", barony::crew(b)), 'h'),
                    ]),
                    Some(_) => {
                        home::board(&[(def.name.to_uppercase(), '9'), ("IT STANDS".into(), '5')])
                    }
                };
                place(cv, &board, HALL_PLATE, false);
            }
        }
        RoomKind::KingsHall => {
            if standing_on(run, LECTERN_PLATE).is_some() {
                let l = &b.ledger;
                let mut lines = vec![
                    ("THE BARON'S LEDGER".to_string(), '9'),
                    (
                        format!("PAID IN   {}", l.paid_in.label().to_uppercase()),
                        'h',
                    ),
                    (
                        format!("PAID BACK {}", l.paid_back.label().to_uppercase()),
                        '5',
                    ),
                    (format!("MINERS AT WORK {}", b.miners()), 'h'),
                ];
                for e in l.lines.iter().rev().take(3) {
                    lines.push((
                        format!(
                            "{} {}",
                            if e.paid_in { "IN " } else { "OUT" },
                            e.what.to_uppercase()
                        ),
                        'O',
                    ));
                }
                let board = home::board(&lines);
                place(cv, &board, LECTERN_PLATE, false);
            } else if standing_on(run, TABLE_PLATE).is_some() {
                let mut lines = vec![("THE FORGES OF CAER DWFN".to_string(), '9')];
                for (i, f) in FORGES.iter().enumerate() {
                    let state = if b.is_lit(f.id) {
                        "BURNING".to_string()
                    } else if let Some(p) = f.work.and_then(|w| b.progress(w)) {
                        if p >= 1000 {
                            "REBUILT, COLD".into()
                        } else {
                            format!("REBUILDING {}%", p / 10)
                        }
                    } else if i == 0 && b.holds_workings() {
                        "FOUND, IN RUIN".into()
                    } else if i == 5 {
                        "THE CROWN: COLD".into()
                    } else {
                        "LOST".into()
                    };
                    lines.push((
                        format!("{}: {state}", f.name.to_uppercase()),
                        if b.is_lit(f.id) { '5' } else { 'h' },
                    ));
                }
                let board = home::board(&lines);
                place(cv, &board, TABLE_PLATE, true);
            }
        }
        RoomKind::Forge => {
            let Some(f) = barony::forge_of(run.dungeon.pack) else {
                return;
            };
            let Some(buying) = standing_on(run, FORGE_PLATE) else {
                return;
            };
            let def = f.work.and_then(barony::work);
            let board = match (def.and_then(|d| b.progress(d.id)), b.is_lit(f.id)) {
                (_, true) => home::board(&[
                    (format!("{} BURNS", f.name.to_uppercase()), '5'),
                    (f.makes.to_uppercase(), 'h'),
                ]),
                (None, _) => {
                    let def = def.expect("rebuildable");
                    price_board(
                        &[
                            (format!("{}: IN RUIN", f.name.to_uppercase()), '9'),
                            (def.says.to_uppercase(), 'h'),
                        ],
                        &def.cost(),
                        &run.treasury,
                        Some(buying),
                    )
                }
                (Some(p), _) if p < 1000 => home::board(&[
                    (f.name.to_uppercase(), '9'),
                    (format!("{}% REBUILT", p / 10), '5'),
                    (format!("{} DWARVES AT WORK", barony::crew(b)), 'h'),
                ]),
                (Some(_), _) => price_board(
                    &[
                        (format!("{}: COLD", f.name.to_uppercase()), '9'),
                        ("RELIGHT IT WITH FIRE FROM BELOW".into(), 'h'),
                    ],
                    &f.fire(),
                    &run.treasury,
                    Some(buying),
                ),
            };
            place(cv, &board, FORGE_PLATE, false);
        }
        _ => {}
    }
}

/// The plates of the mine-head: the King's (once met) and the ruin's.
pub(super) fn plates(cv: &mut Img, run: &Run) {
    let b = &run.home.barony;
    if run.room().kind == RoomKind::MineHead && b.court >= Court::Met {
        if b.labour("hall").is_none() {
            let lit = barony::on_plate(run, HALL_PLATE).is_some();
            home::plate_marked(cv, HALL_PLATE, lit, &hammer_mark());
        }
        if b.court != Court::Sworn {
            let lit = barony::on_plate(run, barony::KING_PLATE).is_some();
            home::plate_marked(cv, barony::KING_PLATE, lit, &crown_mark());
        }
    }
    if run.room().kind == RoomKind::KingsHall && b.court == Court::Sworn {
        let lit = barony::on_plate(run, barony::SEAT_PLATE).is_some();
        home::plate_marked(cv, barony::SEAT_PLATE, lit, &crown_mark());
        for (plate, mark) in [(LECTERN_PLATE, book_mark()), (TABLE_PLATE, anvil_mark())] {
            if barony::on_plate(run, plate).is_some() {
                home::plate_marked(cv, plate, true, &mark);
            }
        }
    }
    if run.room().kind == RoomKind::Forge && barony::on_plate(run, FORGE_PLATE).is_some() {
        home::plate_marked(cv, FORGE_PLATE, true, &anvil_mark());
    }
}

fn hammer_mark() -> Img {
    Img::from_rows(&["OOOO.", "OOOO.", ".O...", ".O...", ".O..."])
}

fn crown_mark() -> Img {
    Img::from_rows(&["O.O.O", "OOOOO", "OOOOO", "....."])
}
