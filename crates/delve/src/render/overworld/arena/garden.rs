//! The Gnome's Garden, drawn (`nooks::gnome`): the cottage with its cellar
//! hatch and lit windows, the copper kettle out back with its jug, the
//! barrels and the harvest crates, the drying rack and the glowing
//! mushroom bed, lanterns strung over it all. The party gnome, the little
//! blue folk along the walls (they put their hands in their pockets when a
//! knight comes near) and the fairies, who fly a little crooked.
//!
//! Once the party is on the garden is a party: the lanterns chase, the
//! gnome dances with his mug up, the blue folk dance their loops, notes
//! come out of the cottage, and the guests at the gates look like what
//! they are (`guests`). What moves over it all, the lights, the steam, the
//! bites at the hatch, the end of the night, is in `revels`.
use super::super::ink::{Img, hash};
use super::super::material::{self as mat, ramp};
use super::super::{kit, light::Light};
use super::{TILE, at, fire, stand};
use crate::rules::{
    Enemy, EnemyKind, RoomKind, Run,
    nooks::gnome::{self as g, Party},
};

mod flamingo;
mod guests;
mod revels;

pub(super) use flamingo::figure as flamingo;
pub(super) use flamingo::{corpse as flamingo_corpse, fallen_under as flamingo_fallen, is as is_flamingo, over as flamingo_over, owns_fall as flamingo_owns_fall};
#[cfg(test)]
pub(super) use flamingo::sheet as flamingo_sheet;

fn all(_: i32, _: i32) -> bool {
    true
}

fn px_rect((c, r, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (c * TILE, r * TILE, w * TILE, h * TILE)
}

fn px(t: (f32, f32)) -> (i32, i32) {
    at(t.0 * 2.0, t.1 * 2.0)
}

/// Whether the room the party stands in is the garden.
fn in_garden(run: &Run) -> bool {
    run.room().kind == RoomKind::GnomeParty
}

/// The garden's state, while the party stands in it.
fn here(run: &Run) -> Option<&g::Garden> {
    if !in_garden(run) {
        return None;
    }
    run.nook.as_ref().filter(|n| n.room == run.at)?.garden.as_ref()
}

/// The room's last line, if it is the garden's and was said `within`
/// ticks ago: how long ago, who it was said to, and the line.
fn said(run: &Run, within: u64) -> Option<(u64, u32, &str)> {
    let (when, who, line) = run.found.as_ref()?;
    let age = run.tick.saturating_sub(*when);
    (in_garden(run) && age < within).then_some((age, *who, line.as_str()))
}

/// How long the house has stood held or fallen, while its last word is
/// still the room's: the end of the night's own clock.
fn ended(run: &Run, party: Party) -> Option<u64> {
    let line = match party {
        Party::Held => g::HELD_LINE,
        Party::Lost => g::LOST_LINE,
        _ => return None,
    };
    let (when, _, said) = run.found.as_ref()?;
    (said == line).then(|| run.tick.saturating_sub(*when))
}

/// The ticks the house's own cheer has, before the fairies come out of it
/// with their wish (`magic`).
pub(super) const FAIRY_LEAD: u64 = 24;

/// The ticks the lanterns take to go out when the house falls, from both
/// ends of the string to its middle.
const LIGHTS_OUT: f32 = 40.0;

/// The party gnome: the bazaar's gnome, a mug in his hand.
const GNOME: [&str; 17] = [
    "....7.......",
    "...777......",
    "...7777.....",
    "..777777....",
    "..OOOOOO....",
    "..OKOOKO....",
    "..HHHHHH....",
    ".HHHHHHHH.H.",
    ".HHHHHHHHoHo",
    "..EEEEEEooo.",
    ".EEEEEEEEoo.",
    ".EEEEEEEE...",
    "..EE..EE....",
    "..EE..EE....",
    "..II..II....",
    "..II..II....",
    ".nnn..nnn...",
];

/// The gnome at his party: eyes shut with joy, a red nose, the mug up
/// over his hat and its head of foam slopping.
const GNOME_CHEERS: [&str; 17] = [
    "....7....HH.",
    "...777..oHHo",
    "...7777.ooo.",
    "..777777ooo.",
    "..OOOOOO.E..",
    "..OkOOkOE...",
    "..HRRHHE....",
    ".HHHHHHHH...",
    ".HHHHHHHH...",
    "..EEEEEE....",
    ".EEEEEEEE...",
    ".EEEEEEEE...",
    "..EE..EE....",
    "..EE..EE....",
    "..II..II....",
    "..II..II....",
    ".nnn..nnn...",
];

/// Between cheers: the mug at his beard, his nose gone rosy.
const GNOME_SIP: [&str; 17] = [
    "....7.......",
    "...777......",
    "...7777.....",
    "..777777....",
    "..OOOOOO....",
    "..OkOOkO....",
    "..HRRHoHo...",
    ".HHHHHooo...",
    ".HHHHHHooE..",
    "..EEEEEEEE..",
    ".EEEEEEEE...",
    ".EEEEEEEE...",
    "..EE..EE....",
    "..EE..EE....",
    "..II..II....",
    "..II..II....",
    ".nnn..nnn...",
];

/// The night lost: his hat's tip hanging, his eyes down, empty-handed.
const GNOME_GLUM: [&str; 17] = [
    "............",
    "............",
    "...7777.....",
    "..7777777...",
    "..OOOOOO77..",
    "..OOOOOO.7..",
    "..OkOOkO....",
    "..HHzHHH....",
    ".HHHHHHHH...",
    ".HHHHHHHH...",
    "..EEEEEE....",
    ".EEEEEEEE...",
    "..EE..EE....",
    "..EE..EE....",
    "..II..II....",
    "..II..II....",
    ".nnn..nnn...",
];

/// A little blue garden-dweller: a floppy mushroom cap, a big nose,
/// heavy-lidded eyes, a mossy waistcoat. Hands behind the back.
const FOLK: [&str; 15] = [
    "...rrrr...",
    "..rRWRRr..",
    ".rRRRRWRr.",
    "rRWRRRRRRr",
    "..QQQQQQ..",
    "..QhKQhK..",
    "..QQQzzQ..",
    "...QQzQ...",
    "...QQQQ...",
    "..lAAAAl..",
    ".QlAMMAlQ.",
    "..lAAAAl..",
    "...ss.ss..",
    "...ss.ss..",
    "..nn..nn..",
];

/// The same, glancing sideways, a hand gone to a pocket.
const FOLK_SHIFTY: [&str; 15] = [
    "...rrrr...",
    "..rRWRRr..",
    ".rRRRRWRr.",
    "rRWRRRRRRr",
    "..QQQQQQ..",
    "..QQhKhK..",
    "..QQQQzzQ.",
    "...QQQzQ..",
    "...QQQQ...",
    "..lAAAAl..",
    "..lAMMAQ..",
    "..lAAAQQ..",
    "...ss.ss..",
    "...ss.ss..",
    "..nn..nn..",
];

/// The same, glancing the other way: the eyes dart.
const FOLK_DART: [&str; 15] = [
    "...rrrr...",
    "..rRWRRr..",
    ".rRRRRWRr.",
    "rRWRRRRRRr",
    "..QQQQQQ..",
    "..QKhQKh..",
    "..QQQQzzQ.",
    "...QQQzQ..",
    "...QQQQ...",
    "..lAAAAl..",
    "..lAMMAQ..",
    "..lAAAQQ..",
    "...ss.ss..",
    "...ss.ss..",
    "..nn..nn..",
];

/// A blue dweller dancing: both arms up, the cap bouncing, a grin, the
/// knees out.
const FOLK_ARMS_UP: [&str; 15] = [
    "....rrrr....",
    "...rRWRRr...",
    "Q.rRRRRWRr.Q",
    "lrRWRRRRRRrl",
    ".l.QQQQQQ.l.",
    "..lQkQQkQl..",
    "...QQzzQQ...",
    "....QkkQ....",
    "....QQQQ....",
    "...lAAAAl...",
    "...lAMMAl...",
    "...lAAAAl...",
    "...ss..ss...",
    "..ss....ss..",
    ".nn......nn.",
];

/// Dancing, the other beat: one arm up, one on the hip, a heel kicked.
const FOLK_KICK: [&str; 15] = [
    "....rrrr....",
    "...rRWRRr...",
    "..rRRRRWRr.Q",
    ".rRWRRRRRRrl",
    "...QQQQQQ.l.",
    "...QkQQkQl..",
    "...QQzzQQ...",
    "....QkkQ....",
    "....QQQQ....",
    "..QlAAAAl...",
    "...lAMMAl...",
    "...lAAAAl...",
    "....ss.ss...",
    "....ss..sss.",
    "...nn.......",
];

/// A fairy, wings up and wings down.
const FAIRY: [[&str; 6]; 2] = [
    ["W.....W", "VW.T.WV", ".VWtWV.", "...t...", "..T.T..", "......."],
    [".......", "...T...", "VWVtVWV", "W.WtW.W", "..T.T..", "......."],
];

/// A fairy's frame, wings up (0) or down (1): for the fairies' wish
/// (`magic`).
pub(super) fn fairy(frame: usize) -> Img {
    Img::from_rows(&FAIRY[frame % 2])
}

/// Where the fairies circle the cottage, in native pixels.
pub(super) fn fairy_home() -> (i32, i32) {
    let (hx, hy) = px((12.0, 3.0));
    (hx, hy + 30)
}

/// A garden guest as the garden sees it: the enemy drawn as the
/// caterpillar, worm, beetle or moth it is here, or `None` anywhere but
/// the garden and for anything that is not one of its guests. The
/// bestiary keeps its own pictures everywhere else.
pub(super) fn guest(run: &Run, enemy: &Enemy, tick: u32) -> Option<Img> {
    if !in_garden(run) || enemy.boss.is_some() || !guests::is_guest(enemy.kind) {
        return None;
    }
    Some(guests::figure(run, here(run), enemy, tick))
}

/// Whether a fall is the garden's to draw (a guest's, in the garden):
/// its pieces are the guest's own colours, not the bestiary's.
pub(super) fn owns_death(run: &Run, kind: Option<EnemyKind>) -> bool {
    in_garden(run) && kind.is_some_and(guests::is_guest)
}

pub(super) fn scenery(cv: &mut Img, run: &Run, lights: &mut Vec<Light>) {
    let tick = run.tick as u32;
    lawn(cv);
    cottage(cv, lights);
    kettle(cv, lights, tick);
    // The barrels nobody opens, and the crates by the wall.
    let (bx, by, _, _) = px_rect(g::BARRELS);
    cv.stamp(&kit::barrels(), bx + 2, by + 2);
    cv.stamp(&kit::barrels(), bx + 30, by + 4);
    for k in 0..3 {
        let (cx, cy) = (bx + 4 + k * 19, by + 34);
        cv.rect(cx, cy, 17, 11, 'r');
        cv.frame(cx, cy, 17, 11, 'P');
        cv.line(cx, cy + 5, cx + 16, cy + 5, 'P');
    }
    cv.text(bx + 5, by + 47, "HARVEST", 'o');
    herb_rack(cv);
    mushroom_bed(cv, lights, tick);
    // Grass and flowers about the garden.
    for k in 0..26u32 {
        let h = hash(k as i32, 7, 0x6a0e);
        let (x, y) = (16 + (h % 352) as i32, 120 + ((h >> 9) % 92) as i32);
        let ink = ['l', 'A', 'M', 'm'][(h >> 17) as usize % 4];
        cv.line(x, y, x - 1, y - 3, ink);
        cv.line(x + 1, y, x + 2, y - 3, ink);
        if (h >> 21).is_multiple_of(5) {
            cv.put(x, y - 4, ['O', 'T', 'V'][(h >> 23) as usize % 3]);
        }
    }
}

/// The lawn: meadow tufts gathered in clumps on the dark, clumps of
/// flowers, and stepping stones from the round door down past the gnome.
fn lawn(cv: &mut Img) {
    let (w, h) = (cv.w.min(24 * TILE), cv.h.min(14 * TILE));
    for y in TILE + 4..h - TILE {
        for x in TILE + 2..w - TILE - 2 {
            // Only on the bare dark of the floor.
            if cv.get(x, y).is_none_or(|c| c.iter().all(|&v| v < 16))
                && let Some(ch) = mat::ground(mat::Ground::Meadow, x, y)
            {
                cv.put(x, y, ch);
            }
        }
    }
    for k in 0..9 {
        let hsh = hash(k, 3, 0x9a2d);
        let (x, y) = (30 + (hsh % 320) as i32, 128 + ((hsh >> 9) % 80) as i32);
        mat::flowers(cv, x, y, hsh);
    }
    let (dx, _) = px(g::DOOR);
    for (x, y) in [(dx - 3, 87), (dx - 13, 95), (dx - 21, 104)] {
        cv.ellipse(x as f32, y as f32, 4.5, 2.0, 'g');
        cv.ellipse(x as f32 - 0.5, y as f32 - 0.5, 3.5, 1.2, 'j');
        cv.put(x - 2, y - 1, 'G');
    }
}

/// The cottage on the north wall: thatch, timbered plaster, two lit
/// windows, a round door, the cellar hatch below it and a smoking chimney.
fn cottage(cv: &mut Img, lights: &mut Vec<Light>) {
    let (x, y, w, h) = px_rect(g::HOUSE);
    cv.rect(x + 4, y + 24, w - 8, h - 24, 'i');
    cv.frame(x + 4, y + 24, w - 8, h - 24, 'P');
    for bx in [x + 4, x + 30, x + w - 31, x + w - 5] {
        cv.line(bx, y + 24, bx, y + h - 1, 'P');
    }
    cv.line(x + 4, y + 42, x + w - 5, y + 42, 'P');
    mat::lobe(cv, ((x + w / 2) as f32, (y + 16) as f32), ((w / 2) as f32, 13.0), ramp::THATCH, 7301, 2.0, 0.3, &all);
    // The chimney.
    cv.rect(x + w - 22, y - 2, 8, 14, 'g');
    cv.frame(x + w - 22, y - 2, 8, 14, 'X');
    for (k, wx) in [x + 12, x + w - 26].into_iter().enumerate() {
        cv.rect(wx, y + 30, 14, 10, 'T');
        cv.rect(wx + 2, y + 32, 10, 6, '6');
        cv.frame(wx, y + 30, 14, 10, 'P');
        cv.line(wx + 7, y + 30, wx + 7, y + 39, 'P');
        lights.push(fire(wx + 7, y + 36, 46.0, 0.35 + k as f32 * 0.02));
    }
    // The round door, and the cellar hatch where the guests are headed.
    let dx = x + w / 2;
    cv.ellipse(dx as f32, (y + h - 12) as f32, 7.0, 8.0, 'p');
    cv.rect(dx - 7, y + h - 12, 15, 12, 'p');
    cv.put(dx + 4, y + h - 7, 'O');
    let (hx, hy) = hatch();
    cv.rect(hx - 10, hy - 2, 21, 6, 'r');
    cv.frame(hx - 10, hy - 2, 21, 6, 'b');
    cv.line(hx, hy - 2, hx, hy + 3, 'b');
}

/// The cellar hatch's middle, in native pixels.
fn hatch() -> (i32, i32) {
    px((g::DOOR.0, g::DOOR.1 - 0.4))
}

/// Where the copper kettle's pot sits, in native pixels.
fn pot() -> (i32, i32) {
    let (x, y, w, h) = px_rect(g::KETTLE);
    (x + w / 2 - 4, y + h / 2 + 2)
}

/// The kettle's tap over its jug: the tap's column and row, and the row
/// of the jug's mouth under it.
fn drip() -> (i32, i32, i32) {
    let (cx, cy) = pot();
    (cx + 19, cy + 3, cy + 6)
}

/// The middle of the stoneware jug under the kettle's tap.
fn jug() -> (i32, i32) {
    let (cx, cy) = pot();
    (cx + 19, cy + 15)
}

/// The copper kettle out back: a fat pot on a fire, its coiled pipe, and
/// a stoneware jug under the tap, three crosses on its label. It is never
/// for tea.
fn kettle(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (_, y, _, h) = px_rect(g::KETTLE);
    let (cx, cy) = pot();
    super::nooks::logs(cv, cx, y + h - 2, tick);
    lights.push(fire(cx, y + h - 4, 58.0, 0.42));
    // The pot: solid copper, lit on its left, a rivet band round its belly.
    cv.ellipse(cx as f32, cy as f32, 12.0, 11.0, 'R');
    cv.ellipse((cx - 1) as f32, (cy - 1) as f32, 10.0, 9.0, 'o');
    cv.ellipse((cx - 4) as f32, (cy - 4) as f32, 4.0, 3.0, 'O');
    cv.put(cx - 6, cy - 6, 't');
    cv.line(cx - 11, cy + 3, cx + 11, cy + 3, 'p');
    for k in 0..5 {
        cv.put(cx - 8 + k * 4, cy + 3, 'O');
    }
    cv.rect(cx - 3, cy - 15, 6, 5, 'O');
    cv.frame(cx - 3, cy - 15, 6, 5, 'R');
    // The coil, round and round, down to the tap.
    cv.line(cx + 3, cy - 13, cx + 13, cy - 11, 'O');
    for k in 0..4 {
        let ry = cy - 10 + k * 3;
        cv.ellipse((cx + 16) as f32, ry as f32, 4.0, 1.5, 'O');
        cv.line(cx + 14, ry, cx + 18, ry, 'R');
    }
    cv.line(cx + 19, cy, cx + 19, cy + 1, 'O');
    cv.put(cx + 19, cy + 2, 'R');
    cv.put(cx + 20, cy + 1, 'r');
    // The jug: a glazed brown shoulder over a cream belly, its mouth open
    // under the tap, three crosses on it.
    let (jx, jy) = jug();
    let (rx, ry) = (6.0f32, 6.5f32);
    for y in jy - 7..=jy + 7 {
        for x in jx - 7..=jx + 7 {
            let nx = (x as f32 + 0.5 - jx as f32) / rx;
            let ny = (y as f32 + 0.5 - jy as f32) / ry;
            let d = nx * nx + ny * ny;
            if d > 1.0 {
                continue;
            }
            let ink = if d > 0.78 && (nx > 0.0 || ny > 0.0) {
                if ny < -0.3 { 'B' } else { 'o' }
            } else if ny < -0.3 {
                if nx < -0.2 && ny < -0.55 { 'O' } else { 'r' }
            } else if nx > 0.5 {
                't'
            } else {
                'T'
            };
            cv.put(x, y, ink);
        }
    }
    cv.rect(jx - 1, jy - 8, 3, 2, 'r');
    cv.rect(jx - 1, jy - 9, 3, 1, 'o');
    cv.put(jx, jy - 9, 'K');
    cv.line(jx + 6, jy - 4, jx + 7, jy - 1, 'B');
    cv.put(jx + 6, jy, 'B');
    for k in 0..3 {
        let lx = jx - 5 + k * 4;
        for (dx, dy) in [(0, 0), (2, 0), (1, 1), (0, 2), (2, 2)] {
            cv.put(lx + dx, jy + dy, 'p');
        }
    }
}

/// The drying rack: herbs hung on a line between two posts, and a little
/// table of spice jars under it.
fn herb_rack(cv: &mut Img) {
    let (x, y) = px(g::HERBS);
    let (l, r, top) = (x - 22, x + 22, y - 26);
    cv.line(l, top, l, y - 4, 'P');
    cv.line(r, top, r, y - 4, 'P');
    cv.line(l, top + 2, r, top + 2, 'o');
    for k in 0..7 {
        let hx = l + 4 + k * 6;
        cv.line(hx, top + 3, hx - 1, top + 10, if k % 2 == 0 { 'A' } else { 'l' });
        cv.line(hx + 1, top + 3, hx + 2, top + 9, 'M');
    }
    cv.rect(l + 4, y - 12, r - l - 8, 3, 'r');
    for (k, ink) in ['R', 'O', 'y', 'o', 'L'].into_iter().enumerate() {
        let jx = l + 7 + k as i32 * 7;
        cv.rect(jx, y - 17, 4, 5, ink);
        cv.rect(jx, y - 18, 4, 1, 'h');
    }
}

/// Where each mushroom in the bed stands: its foot and its cap's width.
fn caps() -> impl Iterator<Item = (i32, i32, f32)> {
    let (x, y) = px(g::SHROOMS);
    (0..7).map(move |k| {
        let h = hash(k, 13, 0x5e0d);
        (x - 18 + k * 6, y - 8 - (h % 5) as i32, (2 + h % 3) as f32)
    })
}

/// The mushroom bed: dark soil and a crop of caps that glow and breathe
/// (their light is drawn over the room, `revels`).
fn mushroom_bed(cv: &mut Img, lights: &mut Vec<Light>, tick: u32) {
    let (x, y) = px(g::SHROOMS);
    cv.ellipse(x as f32, (y - 6) as f32, 22.0, 7.0, 'b');
    cv.ellipse(x as f32, (y - 7) as f32, 20.0, 5.0, 'I');
    let breathe = (tick / 12) % 4;
    for (mx, my, cap) in caps() {
        cv.line(mx, my, mx, my + 4, 'H');
        cv.ellipse(mx as f32, (my - 1) as f32, cap + 0.5, cap * 0.6, 'Q');
        cv.put(mx - 1, my - 2, 'W');
    }
    lights.push(Light { x: x as f32, y: (y - 8) as f32, r: 52.0, s: 0.28 + breathe as f32 * 0.03, fire: false });
}

/// How many lanterns hang on the string.
const STRING: i32 = 19;

/// The two poles the lanterns are strung between, and the height of
/// their tops (native pixels).
const POLES: (i32, i32, i32) = (30, 354, 112);

/// How far the lantern string has sunk as its lights went out, 0 to 1.
fn droop(run: &Run, party: Party) -> f32 {
    if party != Party::Lost {
        return 0.0;
    }
    ended(run, party).map_or(1.0, |a| super::pfx::ease::out(a as f32 / LIGHTS_OUT))
}

/// The lanterns strung over the garden: where each hangs this tick, the
/// string sunk by `droop`.
fn lanterns(tick: u32, droop: f32) -> impl Iterator<Item = (usize, i32, i32)> {
    let (l, r, top) = POLES;
    let sway = (tick as f32 * 0.045).sin() * 1.2 * (1.0 - droop);
    (0..STRING).map(move |k| {
        let u = (k as f32 + 0.5) / STRING as f32;
        let sag = (u * std::f32::consts::PI).sin() * (10.0 + sway + droop * 8.0);
        (k as usize, l + 6 + k * (r - l - 12) / (STRING - 1), top + 1 + sag.round() as i32)
    })
}

/// A lantern's paper, lit and unlit.
const LANTERN: [(char, char); 5] = [('6', 'a'), ('z', 'q'), ('y', 'l'), ('t', 'r'), ('T', 'o')];

/// Whether lantern `k` is lit, and if so the colour it is lit in.
fn lantern_lit(run: &Run, party: Party, k: usize) -> Option<usize> {
    let tick = run.tick as u32;
    let colour = match party {
        Party::Holding | Party::Held => (k + (tick / 18) as usize) % LANTERN.len(),
        _ => k % LANTERN.len(),
    };
    let mid = (STRING - 1) as f32 / 2.0;
    match party {
        Party::Quiet => Some(colour),
        Party::Holding | Party::Held => {
            // A chase runs down the string; when the party starts, the
            // lights come on from the middle out.
            let started = said(run, 60).filter(|&(_, _, l)| l.starts_with("STAY A WHILE")).map(|(a, _, _)| a as f32);
            if started.is_some_and(|a| (k as f32 - mid).abs() > a * 0.5) {
                return None;
            }
            (!(k as u32 + tick / 4).is_multiple_of(5)).then_some(colour)
        }
        Party::Lost => {
            // Out from both ends to the middle, then all dark.
            let age = ended(run, party).map_or(f32::MAX, |a| a as f32);
            let from_end = (k as f32).min(2.0 * mid - k as f32);
            (age < LIGHTS_OUT && from_end > age / LIGHTS_OUT * (mid + 0.5)).then_some(colour)
        }
    }
}

pub(super) fn figures(cv: &mut Img, run: &Run, garden: Option<&g::Garden>) {
    let tick = run.tick as u32;
    let party = garden.map_or(Party::Quiet, |gd| gd.party);
    let dancing = matches!(party, Party::Holding | Party::Held);
    // The lanterns strung over the garden between two poles, blinking
    // with the party.
    let (l, r, top) = POLES;
    for px_ in [l, r] {
        cv.rect(px_ - 1, top, 2, 30, 'P');
        cv.line(px_ - 1, top, px_ - 1, top + 29, 'r');
        cv.rect(px_ - 2, top - 1, 4, 2, 'o');
        cv.put(px_ - 2, top + 30, 'I');
        cv.put(px_ + 1, top + 30, 'I');
    }
    let mut last = (l, top);
    for (k, lx, ly) in lanterns(tick, droop(run, party)) {
        cv.line(last.0, last.1, lx, ly - 1, 'j');
        last = (lx, ly - 1);
        let lit = lantern_lit(run, party, k);
        let (bright, dark) = LANTERN[lit.unwrap_or(k % LANTERN.len())];
        cv.put(lx, ly, 'j');
        cv.rect(lx - 1, ly + 1, 3, 4, if lit.is_some() { bright } else { dark });
        cv.put(lx - 1, ly + 1, 'j');
        cv.put(lx + 1, ly + 1, 'j');
        if lit.is_some() {
            cv.put(lx, ly + 2, if bright == 'T' { '9' } else { 'T' });
        }
        cv.put(lx, ly + 5, 'j');
    }
    cv.line(last.0, last.1, r, top, 'j');
    // The lights, the steam and the notes go under everyone who stands.
    revels::under(cv, run, garden);
    revels::balloons(cv, run, party);
    // The gnome: waves you over, and dances once the party's on.
    let (gx, gy) = gnome_spot(party);
    let end = ended(run, party);
    let (rows, hop, flip): (&[&str], i32, bool) = match party {
        Party::Quiet => (&GNOME, (tick / 30).is_multiple_of(4) as i32, false),
        Party::Lost => (&GNOME_GLUM, 0, false),
        _ => {
            // A cheer every few beats, his mug up; in between a sip.
            let victory = end.is_some_and(|a| a < 48);
            let beat = tick % 72;
            let rows: &[&str] = if victory || beat < 22 {
                &GNOME_CHEERS
            } else if beat < 40 {
                &GNOME_SIP
            } else {
                &GNOME
            };
            let hop = if victory {
                (super::pfx::ease::bump((end.unwrap_or(0) % 16) as f32 / 16.0) * 7.0) as i32
            } else {
                [0, 2, 3, 2][(tick / 5 % 4) as usize]
            };
            (rows, hop, (tick / 20).is_multiple_of(2) && beat >= 22)
        }
    };
    let gnome = Img::from_rows(rows);
    let gnome = if flip { gnome.flip_h() } else { gnome };
    stand(cv, &gnome, gx, gy - hop);
    if party == Party::Lost {
        revels::spilt_mug(cv, gx + 9, gy);
    }
    if party == Party::Quiet {
        let (gx, gy) = px(g::GNOME);
        super::near_prompt(cv, run, (gx, gy + 34), "STAY A WHILE");
    }
    super::near_prompt(cv, run, { let (x, y) = px(g::HERBS); (x, y + 34) }, "HERBS + SPICES");
    super::near_prompt(cv, run, { let (x, y) = px(g::SHROOMS); (x, y + 34) }, "MUSHROOMS");
    // The little blue folk, along the walls. Shifty when a knight is near;
    // dancing their loops once the party's on.
    for (i, &spot) in g::FOLK.iter().enumerate() {
        let (fx, fy) = px(spot);
        let (im, x, y) = if dancing {
            let a = tick as f32 * 0.055 + i as f32 * 1.7;
            let (ox, oy) = (a.cos() * 7.0, a.sin() * 3.0);
            let beat = (tick as f32 * 0.21 + i as f32 * 1.3).sin();
            let hop = (beat.abs() * 4.0) as i32;
            let rows: &[&str] = if beat > 0.0 { &FOLK_ARMS_UP } else { &FOLK_KICK };
            let im = Img::from_rows(rows);
            // They face the way the loop carries them.
            let im = if a.sin() > 0.0 { im.flip_h() } else { im };
            (im, fx + ox.round() as i32, fy + oy.round() as i32 - hop)
        } else {
            let near = folk_near(run, (fx, fy));
            let rows: &[&str] = if near && party == Party::Quiet {
                if (tick / 11 + i as u32).is_multiple_of(3) { &FOLK_DART } else { &FOLK_SHIFTY }
            } else {
                &FOLK
            };
            let im = Img::from_rows(rows);
            let im = if (fx < 192) == (i % 2 == 0) { im } else { im.flip_h() };
            // Glum once the cellar's lost: slumped a pixel, no bob.
            let bob = if party == Party::Lost { -1 } else { (tick / 40 + i as u32).is_multiple_of(5) as i32 };
            // With a knight near, they edge away from them, a step and back.
            let edge = if near && party == Party::Quiet { sidle(run, (fx, fy), tick + i as u32 * 9) } else { 0 };
            (im, fx + edge, fy - bob)
        };
        stand(cv, &im, x, y);
        // A pipe's smoke from the one by the cottage.
        if i == 0 && !dancing {
            for k in 0..3u32 {
                let rise = ((tick + k * 20) % 60) as i32;
                cv.put(fx + 6 + rise / 12, fy - 14 - rise / 3, if rise < 30 { 'h' } else { 'g' });
            }
        }
    }
}

/// Where the gnome stands: by his door, or, once the night is lost, off
/// to one side of it, sulking.
fn gnome_spot(party: Party) -> (i32, i32) {
    let (x, y) = px(g::GNOME);
    if party == Party::Lost { (x - 30, y + 2) } else { (x, y) }
}

/// How far a blue dweller has edged away from the nearest knight: a
/// pixel or two, and back, never far.
fn sidle(run: &Run, (fx, fy): (i32, i32), t: u32) -> i32 {
    let near = run.players.values().filter(|h| h.hp > 0).map(|h| at(h.x, h.y)).min_by_key(|&(hx, hy)| (hx - fx).pow(2) + (hy - fy).pow(2));
    let away = near.map_or(1, |(hx, _)| if hx < fx { 1 } else { -1 });
    away * [0, 1, 2, 2, 2, 1][(t / 12 % 6) as usize]
}

/// Whether a living knight stands within three tiles of a blue dweller.
fn folk_near(run: &Run, (fx, fy): (i32, i32)) -> bool {
    run.players.values().any(|h| {
        let (hx, hy) = at(h.x, h.y);
        h.hp > 0 && ((hx - fx) as f32).hypot((hy - fy) as f32) < 3.0 * TILE as f32
    })
}

/// Over the figures: the garden's lights and steam, its fairies, what its
/// guests leave when they fall, the bites at the hatch, the house's bar,
/// the moments a knight takes something, and the night's end.
pub(super) fn over(cv: &mut Img, run: &Run, legible: bool) {
    if !in_garden(run) {
        return;
    }
    revels::over(cv, run, here(run), legible);
}

pub(super) fn board(lines: &mut Vec<(String, char)>, garden: Option<&g::Garden>) {
    lines.push(("THE GNOME'S GARDEN".into(), '9'));
    match garden.map_or(Party::Quiet, |gd| gd.party) {
        Party::Quiet => {
            lines.push(("HERBS  MUSHROOMS  PARTY".into(), 'O'));
            lines.push(("ALL LOCALLY GROWN".into(), 'h'));
        }
        Party::Holding => {
            let gd = garden.unwrap();
            lines.push(("HOLD THE CELLAR DOOR!".into(), '6'));
            lines.push((format!("WAVE {}/{}  HOUSE {}%", gd.wave + 1, g::WAVES.len(), gd.house * 100 / g::HOUSE_HP), 'h'));
        }
        Party::Held => {
            lines.push(("THE HOUSE HELD!".into(), 'O'));
            lines.push(("FAIRY WISH: T TO WISH".into(), 'h'));
        }
        Party::Lost => {
            lines.push(("THE PARTY'S OVER".into(), '6'));
            lines.push(("THEY GOT INTO THE CELLAR".into(), 'h'));
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__garden__tests.rs"]
mod tests;
