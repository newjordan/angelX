//! The teams' pictures: a pair of barded horses at the gallop and the chariot behind them, drawn
//! from shapes (the lists' destrier twice, and a car with a spoked wheel and a driver), side-on
//! for the straights and head-on or from behind for the ends. Each yard runs in its own cloth.
//! The base pictures are made once for a yard, a coat, a view and a stride, and kept.
use super::super::super::ink::Img;
use super::super::joust::{Coat, destrier};
use crate::rules::chariot::Heading;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// What a team wears: the cloth over its horses and on its driver, the trim, and the ink it is
/// dotted in on the map and the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Livery {
    pub cloth: char,
    pub trim: char,
    pub banner: char,
}

/// A yard's colours. A yard the realm does not know runs in the keep's grey.
pub(super) fn livery(house: &str) -> Livery {
    let (cloth, trim, banner) = match house {
        "qwen" => ('z', 'Q', 'z'),
        "grok" => ('A', 'l', 'A'),
        "glm" => ('5', '4', '5'),
        "deepseek" => ('1', 'q', '1'),
        "tower" => ('7', '8', 'R'),
        "marsh" => ('2', '0', '2'),
        _ => ('h', 'G', 'h'),
    };
    Livery { cloth, trim, banner }
}

/// The ink of a team's mark on the map: its banner colour.
pub(super) fn banner_ink(house: &str) -> char {
    livery(house).banner
}

/// A coat by team: bay, grey, chestnut, black, dun, palomino.
fn coat_of(team: u8) -> Coat {
    let (hide, dark, mane) = [('R', 'r', 'n'), ('i', 'h', 'G'), ('o', 'P', 'b'), ('X', 'g', 'K'), ('P', 'B', 'n'), ('T', 'o', 'P')][usize::from(team) % 6];
    Coat { hide, dark, mane }
}

/// The ink a shade down: the far horse of a pair runs darker.
fn darker(c: char) -> char {
    match c {
        'R' => 'r',
        'r' => 'b',
        'i' => 'h',
        'h' => 'j',
        'o' => 'P',
        'P' => 'B',
        'X' => 'g',
        'g' => 'K',
        'T' => 'o',
        'B' => 'b',
        _ => 'n',
    }
}

/// How many ticks of the gait the feet take: the picture's own stride.
pub(super) const STRIDE: u32 = 4;

/// The view a team is seen from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum View {
    Side,
    Head,
    Rear,
}

pub(super) fn view_of(h: Heading) -> View {
    match h {
        Heading::South => View::Head,
        Heading::North => View::Rear,
        _ => View::Side,
    }
}

type Key = (Livery, u8, u8, View, bool);

fn cache() -> &'static Mutex<HashMap<Key, Img>> {
    static C: OnceLock<Mutex<HashMap<Key, Img>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The team as `view` shows it, at `frame` of the gait; the whip arm up when `whip`. Facing east
/// for the side view (flip it for west). The driver's head is a two-by-two mark at `driver_at`
/// for a class's own head to be set on.
pub(super) fn team(l: Livery, team: u8, frame: u32, view: View, whip: bool) -> Img {
    let key = (l, team % 6, (frame % STRIDE) as u8, view, whip);
    if let Some(im) = cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return im;
    }
    let im = match view {
        View::Side => side(l, coat_of(team), frame % STRIDE, whip),
        View::Head => ahead(l, coat_of(team), frame % 2, false),
        View::Rear => ahead(l, coat_of(team), frame % 2, true),
    };
    if let Ok(mut c) = cache().lock() {
        c.insert(key, im.clone());
    }
    im
}

/// Where the driver's two-by-two head sits in a picture of `view` (before any flip), and the
/// picture's width, for dressing it.
pub(super) fn driver_at(view: View) -> (i32, i32) {
    match view {
        View::Side => (11, 6),
        View::Head => (19, 3),
        View::Rear => (19, 9),
    }
}

fn thick(im: &mut Img, a: (f32, f32), b: (f32, f32), ink: char) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0) as i32 + 1;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        im.put((a.0 + (b.0 - a.0) * t).round() as i32, (a.1 + (b.1 - a.1) * t).round() as i32, ink);
    }
}

/// The side view: 64 by 38, facing east. Two barded horses abreast, the far one a shade darker
/// and a stride ahead, a pole to the car, the car's wheel in spokes, and the driver on it.
fn side(l: Livery, c: Coat, frame: u32, whip: bool) -> Img {
    let mut im = Img::new(64, 38);
    let far = Coat { hide: darker(c.hide), dark: darker(darker(c.hide)), mane: c.mane };
    let horse = |c: Coat, frame: u32| super::super::resized(&destrier(c, Some(frame), Some((l.cloth, l.trim))), 0.7);
    // The far horse a stride out of step, a little up and ahead.
    im.stamp(&horse(far, (frame + 2) % 4), 27, 1);
    // The pole, and the yoke it hangs from.
    thick(&mut im, (30.0, 25.0), (17.0, 27.0), 'B');
    thick(&mut im, (30.0, 26.0), (17.0, 28.0), 'b');
    im.stamp(&horse(c, frame), 27, 4);
    // The car: a tub in the team's cloth, trimmed, with a rail rising to the front.
    im.rect(4, 19, 16, 8, l.cloth);
    im.rect(4, 19, 16, 1, l.trim);
    im.rect(4, 26, 16, 1, 'b');
    im.rect(5, 21, 14, 1, darker(l.cloth));
    thick(&mut im, (19.0, 19.0), (19.0, 13.0), 'O');
    thick(&mut im, (14.0, 13.0), (19.0, 13.0), 'O');
    // The wheel: a ring, a hub and the spokes turning with the gait.
    let (wx, wy) = (11.0f32, 29.0f32);
    im.ellipse(wx, wy, 6.5, 6.5, 'B');
    im.ellipse(wx, wy, 5.0, 5.0, 'n');
    for k in 0..4 {
        let a = (k as f32 * 45.0 + frame as f32 * 11.0).to_radians();
        thick(&mut im, (wx - a.cos() * 5.0, wy - a.sin() * 5.0), (wx + a.cos() * 5.0, wy + a.sin() * 5.0), 'o');
    }
    im.rect(wx as i32 - 1, wy as i32 - 1, 2, 2, 'O');
    // The driver: legs in the tub, a tabard, the head a mark for a class's own, a plume.
    im.rect(10, 12, 5, 8, l.cloth);
    im.rect(10, 12, 5, 1, 'O');
    im.rect(11, 12, 1, 8, l.trim);
    im.rect(11, 6, 2, 2, 'v');
    im.put(11, 5, 'h');
    im.put(12, 4, l.trim);
    im.put(13, 3, l.trim);
    // The reins run forward to the horses; the whip arm is back and up when the team is lashed.
    thick(&mut im, (15.0, 14.0), (40.0, 13.0), 'G');
    if whip {
        thick(&mut im, (10.0, 13.0), (5.0, 8.0), 'v');
        thick(&mut im, (5.0, 8.0), (1.0, 3.0), 'J');
        im.put(0, 2, 'H');
    } else {
        thick(&mut im, (10.0, 14.0), (8.0, 17.0), 'v');
    }
    im.outline_outside('k');
    im
}

/// One horse seen head-on at `cx`, or from behind: a chest (or a rump), the head and its mane,
/// forelegs in the gait, in the team's cloth over the shoulders.
fn front_horse(im: &mut Img, l: Livery, c: Coat, cx: i32, frame: u32, rear: bool) {
    let (hide, dark) = (c.hide, c.dark);
    // The legs first, a stride apart.
    let (a, b) = if frame % 2 == 0 { (38, 41) } else { (41, 38) };
    for (dx, bottom) in [(-3, a), (3, b)] {
        im.rect(cx + dx - 1, 28, 3, bottom - 28, if dx < 0 { dark } else { hide });
        im.rect(cx + dx - 1, bottom, 3, 2, 'k');
    }
    if rear {
        // From behind: the rump in cloth, the tail hanging, ears forward of the croup.
        im.ellipse(cx as f32, 24.0, 7.5, 8.0, hide);
        im.ellipse(cx as f32, 22.0, 7.0, 6.0, l.cloth);
        im.rect(cx - 7, 25, 15, 1, l.trim);
        im.rect(cx - 1, 20, 3, 14, c.mane);
        im.rect(cx - 2, 32, 5, 3, c.mane);
    } else {
        // Head-on: the chest in cloth, the neck, the head and its blaze, the ears, the mane.
        im.ellipse(cx as f32, 25.0, 7.5, 8.0, hide);
        im.ellipse(cx as f32, 25.0, 6.5, 6.5, l.cloth);
        im.rect(cx - 6, 21, 13, 1, l.trim);
        im.rect(cx - 2, 15, 5, 8, hide);
        im.ellipse(cx as f32, 12.0, 3.2, 5.0, hide);
        im.ellipse(cx as f32, 16.0, 2.2, 2.2, dark);
        im.rect(cx - 1, 8, 2, 8, 'H');
        im.rect(cx - 3, 5, 2, 3, hide);
        im.rect(cx + 2, 5, 2, 3, hide);
        im.rect(cx - 3, 12, 1, 1, 'k');
        im.rect(cx + 3, 12, 1, 1, 'k');
        im.rect(cx - 4, 10, 1, 8, c.mane);
        im.put(cx, 17, 'k');
    }
}

/// Head-on or from behind: 40 by 44, two horses abreast and the car behind (head-on) or before
/// (from behind) them.
fn ahead(l: Livery, c: Coat, frame: u32, rear: bool) -> Img {
    let mut im = Img::new(40, 44);
    let far = Coat { hide: darker(c.hide), dark: darker(darker(c.hide)), mane: c.mane };
    let car = |im: &mut Img, top: i32| {
        // The wheels either side, the tub, the rail.
        for x in [1, 34] {
            im.rect(x, top + 6, 5, 14, 'B');
            im.rect(x + 1, top + 7, 3, 12, 'n');
            im.rect(x + 2, top + 11, 1, 4, 'O');
        }
        im.rect(6, top + 4, 28, 14, l.cloth);
        im.rect(6, top + 4, 28, 2, l.trim);
        im.rect(6, top + 16, 28, 2, 'b');
        im.rect(7, top + 9, 26, 1, darker(l.cloth));
    };
    let driver = |im: &mut Img, top: i32| {
        im.rect(17, top - 4, 6, 9, l.cloth);
        im.rect(17, top - 4, 6, 1, 'O');
        im.rect(19, top - 4, 1, 9, l.trim);
        im.rect(19, top - 10, 2, 2, 'v');
        im.put(19, top - 11, 'h');
        im.put(20, top - 12, l.trim);
    };
    if rear {
        front_horse(&mut im, l, far, 11, frame + 1, true);
        front_horse(&mut im, l, c, 29, frame, true);
        car(&mut im, 17);
        driver(&mut im, 19);
    } else {
        car(&mut im, 2);
        driver(&mut im, 5);
        front_horse(&mut im, l, far, 11, frame + 1, false);
        front_horse(&mut im, l, c, 29, frame, false);
    }
    im.outline_outside('k');
    im
}

// ─── the crowd ───────────────────────────────────────────────────────────────

/// One spectator painted at `(x, y)` (the top-left of its 7 by 10): `pose` 0 standing, 1 swaying,
/// 2 cheering with both arms up, 3 on the toes with arms up (the top of a jump). `cloth` is the
/// coat, `skin` the face, `cap` the hat.
pub(super) fn paint_spectator(im: &mut Img, x: i32, y: i32, cloth: char, skin: char, cap: char, pose: u8) {
    const STAND: [&str; 10] = [".......", "..ccc..", "..sss..", "..sss..", ".bbbbb.", ".bbbbb.", ".bbbbb.", ".bbbbb.", "..b.b..", "..d.d.."];
    const SWAY: [&str; 10] = [".......", "...ccc.", "...sss.", "...sss.", "..bbbbb", ".bbbbb.", ".bbbbb.", ".bbbbb.", "..b.b..", "..d.d.."];
    const CHEER: [&str; 10] = ["s.....s", "s.ccc.s", "b.sss.b", "bbsssbb", ".bbbbb.", ".bbbbb.", ".bbbbb.", ".bbbbb.", "..b.b..", "..d.d.."];
    const JUMP: [&str; 10] = ["s.....s", "s.ccc.s", "b.sss.b", "bbsssbb", ".bbbbb.", ".bbbbb.", ".bbbbb.", "..bbb..", "..b.b..", "..d.d.."];
    let rows = match pose {
        0 => &STAND,
        1 => &SWAY,
        2 => &CHEER,
        _ => &JUMP,
    };
    for (j, row) in rows.iter().enumerate() {
        for (i, k) in row.bytes().enumerate() {
            let ink = match k {
                b'c' => cap,
                b's' => skin,
                b'b' => cloth,
                b'd' => 'n',
                _ => continue,
            };
            im.put(x + i as i32, y + j as i32, ink);
        }
    }
}

/// A spectator as its own picture.
pub(super) fn spectator(cloth: char, skin: char, cap: char, pose: u8) -> Img {
    let mut im = Img::new(7, 10);
    paint_spectator(&mut im, 0, 0, cloth, skin, cap, pose);
    im
}
