//! What each class looks like: a whole figure of its own, not a badge on
//! the knight. The Mage in a robe and pointed hat with a staff, the Healer
//! in a cowled habit swinging a censer, the Archer hooded and cloaked with a
//! longbow, the Swordsman in plate with his blade, the Shieldmaiden in mail
//! and a winged helm behind her kite shield, the Gunner in a wide iron hat
//! and buff coat with his handgonne, and the Berserker, a dwarf with two
//! axes.
//!
//! Each class is drawn three ways (toward the viewer, across facing right,
//! and away), as a body and the thing it holds. The held thing has a second
//! drawing for the strike. Walking moves the legs (or the hem of a robe) on
//! the stride. The seat's own colour is the trim: the Mage's hat band and
//! hem, the Healer's scapular, the Archer's feather and belt, the plume and
//! tabard, the shield's field, the Gunner's neckerchief, the dwarf's belt.
//! Those are the `7 8 5` inks, recoloured per seat like the knight's.
//!
//! One kit that is no class has a figure of its own all the same: the
//! Gravecaller, a hero half a head over the rest (`gravecaller`).

use super::super::ink::Img;
use super::knights::Facing;
use crate::rules::{Hero, classes};

mod art;
pub(super) mod bard;
pub(super) mod gravecaller;
pub(super) mod peasant;

pub(super) struct View {
    body: &'static [&'static str],
    item: Layer,
    strike: Layer,
}

pub(super) struct Layer {
    at: (i32, i32),
    rows: &'static [&'static str],
}

pub const W: i32 = 20;
const MARGIN: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Archer,
    Shieldmaiden,
    Gunner,
    Swordsman,
    Berserker,
    Mage,
    Healer,
    Bard,
    Peasant,
    Gravecaller,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Stand,
    Step(u8),
    Strike,
}

impl Look {
    pub const ALL: [Look; 9] = [
        Look::Archer,
        Look::Shieldmaiden,
        Look::Gunner,
        Look::Swordsman,
        Look::Berserker,
        Look::Mage,
        Look::Healer,
        Look::Bard,
        Look::Peasant,
    ];

    pub fn named(name: &str) -> Option<Look> {
        Some(match name {
            "Archer" => Look::Archer,
            "Shieldmaiden" => Look::Shieldmaiden,
            "Gunner" => Look::Gunner,
            "Swordsman" => Look::Swordsman,
            "Berserker" => Look::Berserker,
            "Mage" => Look::Mage,
            "Healer" => Look::Healer,
            "Bard" => Look::Bard,
            "Peasant" => Look::Peasant,
            _ => return None,
        })
    }

    pub fn of(hero: &Hero) -> Option<Look> {
        classes::of(hero)
            .and_then(|c| Look::named(c.name))
            .or_else(|| Look::own(hero.knight.as_deref()?))
    }

    pub fn of_kit(kit: &str) -> Option<Look> {
        classes::CLASSES
            .iter()
            .find(|c| c.kit == kit)
            .and_then(|c| Look::named(c.name))
            .or_else(|| Look::own(kit))
    }

    pub fn own(kit: &str) -> Option<Look> {
        (kit == crate::rules::minions::GRAVECALLER).then_some(Look::Gravecaller)
    }

    pub fn name(self) -> &'static str {
        match self {
            Look::Archer => "Archer",
            Look::Shieldmaiden => "Shieldmaiden",
            Look::Gunner => "Gunner",
            Look::Swordsman => "Swordsman",
            Look::Berserker => "Berserker",
            Look::Mage => "Mage",
            Look::Healer => "Healer",
            Look::Bard => "Bard",
            Look::Peasant => "Peasant",
            Look::Gravecaller => "Gravecaller",
        }
    }

    fn views(self) -> &'static [View; 3] {
        match self {
            Look::Archer => &art::ARCHER,
            Look::Shieldmaiden => &art::SHIELDMAIDEN,
            Look::Gunner => &art::GUNNER,
            Look::Swordsman => &art::SWORDSMAN,
            Look::Berserker => &art::BERSERKER,
            Look::Mage => &art::MAGE,
            Look::Healer => &art::HEALER,
            Look::Bard => bard::views(),
            Look::Peasant => peasant::views(),
            Look::Gravecaller => gravecaller::views(),
        }
    }

    pub fn bodies(self) -> [&'static [&'static str]; 3] {
        let v = self.views();
        [v[0].body, v[1].body, v[2].body]
    }

    fn robed(self) -> bool {
        matches!(self, Look::Mage | Look::Healer | Look::Gravecaller)
    }

    pub fn shielded(self) -> bool {
        self == Look::Shieldmaiden
    }
}

fn view_of(facing: Facing) -> usize {
    match facing {
        Facing::Down => 0,
        Facing::Side => 1,
        Facing::Up => 2,
    }
}

pub(super) fn figure(look: Look, facing: Facing, pose: Pose, held: bool) -> Img {
    figure_shielded(look, facing, pose, held, false)
}

pub(super) fn figure_shielded(look: Look, facing: Facing, pose: Pose, held: bool, shield: bool) -> Img {
    use super::super::kit::hero::{self as realm, View as Seen};
    if look == Look::Gravecaller {
        return gravecaller::figure(facing, pose, held);
    }
    let view = &look.views()[view_of(facing)];
    let standing = Img::from_rows(view.body).h;
    let body = step(look, facing, Img::from_rows(view.body), pose);
    let layer = (pose == Pose::Strike).then_some(&view.strike).unwrap_or(&view.item);
    let item = held.then(|| Img::from_rows(layer.rows));
    let top = item.as_ref().map_or(0, |_| layer.at.1.min(0));
    let bottom = body.h.max(item.as_ref().map_or(0, |i| layer.at.1 + i.h));
    let mut im = Img::new(W, bottom - top);
    im.stamp(&body, MARGIN, -top);
    if shield {
        let seen = match facing {
            Facing::Down => Seen::Toward,
            Facing::Side => Seen::Across,
            Facing::Up => Seen::Away,
        };
        let (rows, (x, y)) = realm::shield(seen);
        im.stamp(&rows, MARGIN + x, y + standing - 16 - top);
    }
    if let Some(item) = item {
        im.stamp(&item, MARGIN + layer.at.0, layer.at.1 - top);
    }
    im
}

pub(super) fn floating_figure(look: Look, facing: Facing, pose: Pose, held: bool, shield: bool, tick: u64, phase: u32, aim: (f32, f32)) -> Img {
    if look == Look::Gravecaller { return gravecaller::floating_figure(facing, pose, held, tick, phase, aim); }
    let body = figure_shielded(look, facing, pose, false, shield);
    if !held { return body; }
    if look.shielded() {
        let view = &look.views()[view_of(facing)];
        let layer = if pose == Pose::Strike { &view.strike } else { &view.item };
        return super::wear::floating(&body, &Img::from_rows(layer.rows),
            (MARGIN + layer.at.0, layer.at.1), tick, phase, pose == Pose::Strike);
    }
    use std::f32::consts::FRAC_PI_2;
    let strike = pose == Pose::Strike;
    let (item, grip, forward) = match look {
        Look::Archer => {
            let view = &look.views()[0];
            let layer = if strike { &view.strike } else { &view.item };
            (Img::from_rows(layer.rows), if strike { (4, 4) } else { (3, 4) }, 0.0)
        },
        Look::Gunner | Look::Swordsman => {
            let held = if look == Look::Gunner { super::super::kit::hero::Held::Handgonne }
                else { super::super::kit::hero::Held::Sword };
            let (item, grip) = super::super::kit::hero::clock_equipment(held, strike).unwrap();
            (item, grip, 0.0)
        },
        Look::Berserker => (Img::from_rows(&[".vV.", "vWWu", "VWVu", ".uu.", "..B.", "..B.", "..r.", "..B."]), (2, 6), -FRAC_PI_2),
        _ => {
            let item = Img::from_rows(look.views()[1].item.rows);
            let grip = (item.w / 2, item.h * 2 / 3);
            (item, grip, -FRAC_PI_2)
        },
    };
    super::wear::clocked(&body, &item, grip, forward,
        (body.w / 2 + 4, body.h - 7), aim)
}

pub(super) fn fit(look: Look, hero: &Hero, figure: &Img) -> crate::rules::looks::Body {
    if look == Look::Gravecaller { return gravecaller::fit(figure, hero); }
    let face = super::knights::facing(hero.aim_x, hero.aim_y);
    let height = look.views()[view_of(face)].body.len() as i32;
    let top = figure.h - height;
    let margin = (figure.w - (W - MARGIN * 2)) / 2;
    let view = match face { Facing::Down => "front", Facing::Side => "right", Facing::Up => "back" };
    let name = match look {
        Look::Archer => "archer", Look::Shieldmaiden => "shieldmaiden", Look::Gunner => "gunner",
        Look::Swordsman => "swordsman", Look::Berserker => "berserker", Look::Mage => "mage",
        Look::Healer => "healer", Look::Bard => "bard", Look::Peasant => "peasant", Look::Gravecaller => unreachable!(),
    };
    let lift = i32::from(matches!(stride(hero), Some(1 | 3)) && pose(hero) != Pose::Strike);
    let mut fit = crate::rules::looks::profile(&format!("{name}_{view}")).shifted(margin, top - lift);
    let layer = if pose(hero) == Pose::Strike { &look.views()[view_of(face)].strike } else { &look.views()[view_of(face)].item };
    let grips: Vec<_> = layer.rows.iter().enumerate().flat_map(|(y,row)| row.chars().enumerate()
        .filter(|(_,c)| matches!(c, 'O' | 'o' | 't' | 'T'))
        .map(move |(x,_)| (x as i32, y as i32))).collect();
    if !grips.is_empty() {
        let (gx,gy) = grips.iter().fold((0,0), |(x,y),(a,b)| (x+a,y+b));
        fit.right.anchor = (margin + layer.at.0 + gx / grips.len() as i32,
            top - lift + layer.at.1 + gy / grips.len() as i32);
    }
    if hero.shielding {
        fit.left.anchor = match face { Facing::Down => (margin + 8, figure.h - 4), Facing::Side => (margin + 12, figure.h - 6), Facing::Up => (margin + 6, figure.h - 8) };
    }
    fit
}

fn step(look: Look, facing: Facing, body: Img, pose: Pose) -> Img {
    let Pose::Step(phase) = pose else {
        return body;
    };
    let phase = phase % 4;
    let mut out = body.clone();
    let legs = if look.robed() { 2 } else { LEGS };
    let first = body.h - legs;
    let mid = body.w / 2;
    for y in first..body.h {
        for x in 0..body.w {
            out.clear(x, y);
        }
    }
    let planted = phase % 2 == 0;
    let first_foot = phase == 0;
    for y in first..body.h {
        for x in 0..body.w {
            let Some(c) = body.get(x, y) else { continue };
            let back = x < mid;
            let (dx, dy) = if !planted {
                (0, 0)
            } else if look.robed() {
                let out = if first_foot { -1 } else { 1 };
                if y < body.h - 1 || first_foot == back { (out, 0) } else { (0, 0) }
            } else if facing == Facing::Side {
                (if back { -1 } else { 1 }, 0)
            } else if first_foot == back {
                (0, -1)
            } else {
                (0, 0)
            };
            out.set(x + dx, (y + dy).max(first), c);
        }
    }
    if !planted {
        let mut lifted = Img::new(out.w, out.h + 1);
        lifted.stamp(&out, 0, 0);
        return lifted;
    }
    out
}

const LEGS: i32 = 2;

pub fn pose(hero: &Hero) -> Pose {
    if hero.swing > 0 || hero.fire_cooldown > 0 {
        Pose::Strike
    } else {
        stride(hero).map_or(Pose::Stand, Pose::Step)
    }
}

pub(super) fn stride(hero: &Hero) -> Option<u8> {
    (hero.stride > 0.0).then(|| ((hero.stride / 0.85) as u8).min(3))
}

pub(super) fn saddle(look: Look, seat: u32) -> (Img, Img) {
    let (head, coat): (&[&str], &[&str]) = match look {
        Look::Mage => (
            &["..u....", "..uS...", "..SSu..", ".uSSSu.", ".u8777.", "xSSSSSx", "..Sgtk.", "..Sott.", "...ot.."],
            &["xSuS7", "xSuS7", "SSuS7", "SSuS7", "SSuSS", "SSuSS", "SuSSS", "87777"],
        ),
        Look::Peasant => (
            &[".ooooo.", ".otttt.", ".otttt.", ".ooooo."],
            &["otttO", "otttO", "otttO", "otttO", "OOOOO", "OOOOO", "OOOOO", "OOOOO"],
        ),
        Look::Bard => (
            &[".44444.", ".4uuu4.", ".4u4u4.", ".4uuu4.", ".44444."],
            &["44444", "4uuu4", "4u4u4", "4uuu4", "4u4u4", "4uuu4", "4uuu4", "44444"],
        ),
        Look::Healer => (
            &["..JhJ..", ".Jhiih.", ".hiJot.", ".hiJtk.", ".hhJot.", "..JhJ.."],
            &["hhJh7", "hhJ77", "hJh77", "hhJh7", "OOOOO", "hhJhh", "hhhhh", "JJJJG"],
        ),
        Look::Archer => (
            &["...l...", "..lEl7.", ".lEEEl7", ".lEEoo8", "lEEotk.", ".lEott.", "..lEo.."],
            &["lEPPP", "lEPPP", "lEPP8", "lEPPP", "lB755", "lPPPP", "lPPPP", "lPPPP"],
        ),
        Look::Gunner => (
            &["..gGG..", ".gJhhJ.", "XgGGGGX", "..gotk.", "..gott.", "...7t.."],
            &["rr777", "rRo87", "rRoRb", "RRORR", "RRRRb", "rbbbb", "RRRRR", "RRRRR"],
        ),
        Look::Shieldmaiden => (
            &["H.iHHh.", "iiHhhhJ", ".OhJJJ.", ".Oottk.", ".O.ott.", ".O..o.."],
            &["jjGJJ", "jJJGJ", "jJJGJ", "jjJGJ", "jj7jj", "jjB4B", "jjjjj", "jjjjj"],
        ),
        Look::Swordsman => (
            &[".7.....", ".77iH..", "..iHHh.", "..iHhhJ", "..hgggg", "..hhJhJ", "..JhhhG"],
            &["h7777", "J7557", "h7557", "h7777", "hBB4B", "87778", "h7777", "h7777"],
        ),
        Look::Berserker => (
            &[".T...T.", ".TiHHiT", "..iHhH.", "..Jjjjt", "..ptkt.", ".rRpOt.", ".rRRoRR"],
            &["RRoRR", "RRRoR", "hRRoR", "JRRrR", "hRoRR", "B8777", "jGGjG", "jjjjj"],
        ),
        Look::Gravecaller => (
            &["..xxx..", ".xSuSx.", "xSuUuxk", "xSuxhiY", "xSuxhkh", "xSSxihi", ".xSSxkx"],
            &["gJhJ7", "gJhJ7", "SSuS7", "SSuS8", "IbbbB", "SSuS7", "SSuS7", "88777"],
        ),
    };
    let trim = super::knights::seat_colours(seat);
    let steel = steel(look);
    (
        Img::from_rows(head).recolor(steel).recolor(trim),
        Img::from_rows(coat).recolor(steel).recolor(trim),
    )
}

fn steel(look: Look) -> &'static [(char, char)] {
    match look {
        Look::Swordsman | Look::Shieldmaiden | Look::Berserker => {
            &[('H', 'W'), ('i', 'V'), ('h', 'v'), ('J', 'U'), ('g', 's')]
        }
        Look::Gunner => &[('h', 'v'), ('J', 'U'), ('G', 'u'), ('g', 'x'), ('X', 'S')],
        _ => &[],
    }
}

pub(super) fn dress_rider(im: &mut Img, (sx, sy): (i32, i32), look: Look, seat: u32) {
    let (head, coat) = saddle(look, seat);
    for y in sy - 15..=sy {
        for x in sx - 3..=sx + 2 {
            im.clear(x, y);
        }
    }
    im.stamp(&coat, sx - 2, sy - 7);
    im.stamp(&head, sx - 3, sy - 7 - head.h);
}

pub(super) fn dress_driver(im: &mut Img, (x, y): (i32, i32), facing: Facing, flip: bool, look: Look, seat: u32) {
    let x = if flip { im.w - 2 - x } else { x };
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        im.clear(x + dx, y + dy);
    }
    let head = driver_head(look, facing, seat);
    let head = if flip { head.flip_h() } else { head };
    im.stamp(&head, x - 1, y - 2);
}

pub(super) fn driver_head(look: Look, facing: Facing, seat: u32) -> Img {
    let rows: &[&str] = match (look, facing) {
        (Look::Peasant, _) => &["oooo", "ottt", "ottt", "oooo", "...."],
        (Look::Bard, _) => &["4444", "4..4", "4444", "4..4", "4444"],
        (Look::Mage, Facing::Down) => &["..x.", ".xS.", "xS7S", ".tt.", "...."],
        (Look::Mage, Facing::Side) => &[".x..", "..S.", "SS7S", ".gt.", "...."],
        (Look::Mage, Facing::Up) => &["..x.", ".xS.", "x7SS", ".SS.", "...."],
        (Look::Healer, Facing::Down) => &[".hh.", "hiih", "htth", ".77.", "...."],
        (Look::Healer, Facing::Side) => &[".hh.", "hiih", "hJot", ".h7.", "...."],
        (Look::Healer, Facing::Up) => &[".hh.", "hiih", "hiih", ".77.", "...."],
        (Look::Archer, Facing::Down) => &["..l7", ".lEl", "lEEl", ".tt.", "...."],
        (Look::Archer, Facing::Side) => &[".l.7", ".lEl", "lEEo", ".Et.", "...."],
        (Look::Archer, Facing::Up) => &["7.l.", "lEl.", "lEEl", ".EE.", "...."],
        (Look::Gunner, Facing::Down) => &["....", ".GG.", "GhhG", ".tt.", ".77."],
        (Look::Gunner, Facing::Side) => &["....", ".GG.", "GhhG", ".gt.", ".77."],
        (Look::Gunner, Facing::Up) => &["....", ".GG.", "GhhG", ".RR.", ".77."],
        (Look::Shieldmaiden, Facing::Down) => &["....", "HhhH", ".hh.", "OttO", "...."],
        (Look::Shieldmaiden, Facing::Side) => &["....", "HhhH", "Ohh.", "O.tk", "...."],
        (Look::Shieldmaiden, Facing::Up) => &["....", "HhhH", ".hh.", ".OO.", "...."],
        (Look::Swordsman, Facing::Down) => &[".7..", ".77.", ".hh.", ".gg.", "...."],
        (Look::Swordsman, Facing::Side) => &["7...", "77h.", ".hhH", ".hgg", "...."],
        (Look::Swordsman, Facing::Up) => &[".7..", ".77.", ".hh.", ".hh.", "...."],
        (Look::Berserker, Facing::Down) => &["....", "ThhT", ".tt.", ".RR.", "...."],
        (Look::Berserker, Facing::Side) => &["....", "ThhT", ".pt.", ".RRt", "...."],
        (Look::Berserker, Facing::Up) => &["....", "ThhT", ".RR.", ".RR.", "...."],
        (Look::Gravecaller, Facing::Down) => &[".xx.", "xuSx", "xhhx", "xYYx", "...."],
        (Look::Gravecaller, Facing::Side) => &[".xx.", "xuSx", "xxhh", "xxhY", "...."],
        (Look::Gravecaller, Facing::Up) => &[".xx.", "xuSx", "xSSx", "xSSx", "...."],
    };
    Img::from_rows(rows).recolor(steel(look)).recolor(super::knights::seat_colours(seat))
}

pub(super) fn class_of(colour: &str) -> Option<super::room_scene::ClassShot> {
    use crate::rules::classes::{ARCANE, HOLY, SONG};
    match colour {
        c if c == ARCANE => Some(super::room_scene::ClassShot::Arcane),
        c if c == HOLY => Some(super::room_scene::ClassShot::Holy),
        c if c == SONG => Some(super::room_scene::ClassShot::Song),
        _ => None,
    }
}

pub(super) fn shot(cv: &mut Img, (x, y): (i32, i32), (vx, vy): (f32, f32), colour: &str, tick: u64) -> bool {
    use crate::rules::classes::{ARCANE, HOLY, SONG};
    if colour == SONG {
        bard::note_shot(cv, (x, y), (vx, vy), tick);
        return true;
    }
    let len = vx.hypot(vy).max(1e-3);
    let (ux, uy) = (vx / len, vy / len);
    let (px, py) = (-uy, ux);
    let behind = |k: f32, side: f32| {
        (
            x - (ux * k - px * side).round() as i32,
            y - (uy * k - py * side).round() as i32,
        )
    };
    let phase = (tick / 2 % 4) as i32;
    match colour {
        c if c == ARCANE => {
            for (k, ink) in [(6, 'S'), (5, 'q'), (4, 'Q'), (3, 'Q'), (2, 'z')] {
                let wave = [0.0, 0.7, 0.0, -0.7][((k as i32 + phase) % 4) as usize];
                let (tx, ty) = behind(k as f32, wave);
                cv.put(tx, ty, ink);
            }
            for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                cv.put(x + dx, y + dy, 'Q');
            }
            for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                cv.put(x + dx, y + dy, 'z');
            }
            cv.put(x, y, '9');
            let (mx, my) = [(2, -1), (1, 2), (-2, 1), (-1, -2)][phase as usize];
            cv.put(x + mx, y + my, 'V');
            cv.put(x - mx, y - my, 'z');
            true
        }
        c if c == HOLY => {
            for (k, ink) in [(6, 's'), (5, 'P'), (3, 'o'), (2, '@')] {
                let drift = if (k + phase) % 2 == 0 { 0.6 } else { -0.6 };
                let (tx, ty) = behind(k as f32, drift);
                cv.put(tx, ty, ink);
            }
            let rays: [(i32, i32); 4] = if phase % 2 == 0 {
                [(0, -2), (-2, 0), (2, 0), (0, 2)]
            } else {
                [(-1, -1), (1, -1), (-1, 1), (1, 1)]
            };
            for (dx, dy) in rays {
                cv.put(x + dx, y + dy, 'c');
            }
            for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                cv.put(x + dx, y + dy, '6');
            }
            cv.put(x, y, '9');
            true
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/overworld__class_looks_tests.rs"]
mod tests;
