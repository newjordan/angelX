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

use super::super::ink::Img;
use super::knights::Facing;
use crate::rules::{Hero, classes};

mod art;
pub(super) mod bard;
pub(super) mod peasant;

/// One way of seeing a class: the body, and the held thing at rest and in
/// the strike. Layers are placed against the body's top-left.
pub(super) struct View {
    body: &'static [&'static str],
    item: Layer,
    strike: Layer,
}

pub(super) struct Layer {
    at: (i32, i32),
    rows: &'static [&'static str],
}

/// The figure's canvas width: the sixteen-pixel body with two pixels each
/// side for what it holds, so the feet stay centred whatever is held out.
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
}

/// Where a figure is in its movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Stand,
    /// A walking step: 0 a foot planted, 1 passing, 2 the other foot, 3
    /// passing again.
    Step(u8),
    /// Shooting, swinging or casting: the held thing in its strike.
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

    /// The look of a class, by its name in `classes::CLASSES`.
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

    /// The class a knight is, by hall or by kit; `None` for a kit that is
    /// no class (the posse), who stays the realm's knight.
    pub fn of(hero: &Hero) -> Option<Look> {
        classes::of(hero).and_then(|c| Look::named(c.name))
    }

    /// The class a company kit makes its wearer, for the intro's portraits.
    pub fn of_kit(kit: &str) -> Option<Look> {
        classes::CLASSES
            .iter()
            .find(|c| c.kit == kit)
            .and_then(|c| Look::named(c.name))
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
        }
    }

    /// The bodies the class is drawn from, toward / across / away.
    pub fn bodies(self) -> [&'static [&'static str]; 3] {
        let v = self.views();
        [v[0].body, v[1].body, v[2].body]
    }

    /// A robe to the ground: the hem sways where legs would step.
    fn robed(self) -> bool {
        matches!(self, Look::Mage | Look::Healer)
    }

    /// The shield on her arm is the Shieldmaiden's held thing; it comes
    /// round to the aim when she raises it.
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

/// The class's figure, facing right when across (the caller flips it for
/// left), with its feet on the bottom row and its middle at `W / 2`.
/// `held`: whether the held thing is drawn at all (not while the dwarf's
/// axes spin, not when the shield is thrown or raised).
pub(super) fn figure(look: Look, facing: Facing, pose: Pose, held: bool) -> Img {
    let view = &look.views()[view_of(facing)];
    let body = step(look, facing, Img::from_rows(view.body), pose);
    let layer = (pose == Pose::Strike).then_some(&view.strike).unwrap_or(&view.item);
    let item = held.then(|| Img::from_rows(layer.rows));
    let top = item.as_ref().map_or(0, |_| layer.at.1.min(0));
    let bottom = body.h.max(item.as_ref().map_or(0, |i| layer.at.1 + i.h));
    let mut im = Img::new(W, bottom - top);
    // What is held at the back is drawn first: the archer's bow across
    // his cloak, the staff beside the robe.
    im.stamp(&body, MARGIN, -top);
    if let Some(item) = item {
        im.stamp(&item, MARGIN + layer.at.0, layer.at.1 - top);
    }
    im
}

/// The walk, four drawings to a footstep: a foot planted, passing, the
/// other foot, passing again. Across, the legs open on each planted foot
/// and close as they pass; toward or away, one foot lifts and then the
/// other. A robe's hem sways to one side and then the other instead.
/// Passing, the figure rides a pixel higher.
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
    // The first foot is the one at the back of the picture (its left).
    let first_foot = phase == 0;
    for y in first..body.h {
        for x in 0..body.w {
            let Some(c) = body.get(x, y) else { continue };
            let back = x < mid;
            let (dx, dy) = if !planted {
                (0, 0)
            } else if look.robed() {
                // The hem swings toward the foot that steps out from under it.
                let out = if first_foot { -1 } else { 1 };
                if y < body.h - 1 || first_foot == back { (out, 0) } else { (0, 0) }
            } else if facing == Facing::Side {
                // Across: a stride.
                (if back { -1 } else { 1 }, 0)
            } else if first_foot == back {
                // Toward or away: the one foot up.
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

/// How many rows from the feet up the legs take, for a figure in hose.
const LEGS: i32 = 2;

/// The pose a knight is in: striking while a shot or swing is fresh, a
/// step on the stride while they move, else standing.
pub fn pose(hero: &Hero) -> Pose {
    if hero.swing > 0 || hero.fire_cooldown > 0 {
        Pose::Strike
    } else {
        stride(hero).map_or(Pose::Stand, Pose::Step)
    }
}

/// Where a moving knight is in their stride, 0 to 3: the stride runs 0 to
/// 3.4 units between footsteps (`rules::spectacle`), a quarter to each
/// drawing of the walk. `None` standing.
pub(super) fn stride(hero: &Hero) -> Option<u8> {
    (hero.stride > 0.0).then(|| ((hero.stride / 0.85) as u8).min(3))
}

// ─── in the saddle and the cab ───────────────────────────────────────────────

/// A class on horseback, drawn across facing right, for a rider the lists
/// draw at the knight's own size: the head (headgear and face; the chin on
/// the last row, its middle column over the rider's middle) and the coat,
/// five wide and eight tall from the shoulders down. In the seat's trim.
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
    };
    let trim = super::knights::seat_colours(seat);
    let steel = steel(look);
    (
        Img::from_rows(head).recolor(steel).recolor(trim),
        Img::from_rows(coat).recolor(steel).recolor(trim),
    )
}

/// The small drawings (in the saddle, in the cab) were inked in the
/// structure greys; a class's helm, hat and plate take the stone bank's
/// cool steel instead, as on foot. Mail and wool keep their greys.
fn steel(look: Look) -> &'static [(char, char)] {
    match look {
        Look::Swordsman | Look::Shieldmaiden | Look::Berserker => {
            &[('H', 'W'), ('i', 'V'), ('h', 'v'), ('J', 'U'), ('g', 's')]
        }
        Look::Gunner => &[('h', 'v'), ('J', 'U'), ('G', 'u'), ('g', 'x'), ('X', 'S')],
        _ => &[],
    }
}

/// Dress a rider drawn in the lists' way in their class: `(sx, sy)` is the
/// rider's waist (the coat's bottom middle); the helm, plume and tabard
/// already there are taken off first.
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

/// Put a class's head on a small driver whose two-pixel head (helm over
/// face) has its top-left at `(x, y)` in the unflipped sheet; `flip` when
/// the sheet was turned to run west.
pub(super) fn dress_driver(im: &mut Img, (x, y): (i32, i32), facing: Facing, flip: bool, look: Look, seat: u32) {
    let x = if flip { im.w - 2 - x } else { x };
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        im.clear(x + dx, y + dy);
    }
    let head = driver_head(look, facing, seat);
    let head = if flip { head.flip_h() } else { head };
    im.stamp(&head, x - 1, y - 2);
}

/// A class's head on a chariot's driver, small as the cab: four wide, the
/// face on the last row but one, its second column over the driver's.
/// Toward the viewer, across facing right, or away, as the team runs.
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
    };
    Img::from_rows(rows).recolor(steel(look)).recolor(super::knights::seat_colours(seat))
}

/// Which class's own shot `colour` names, if it names one.
pub(super) fn class_of(colour: &str) -> Option<super::room_scene::ClassShot> {
    use crate::rules::classes::{ARCANE, HOLY};
    match colour {
        c if c == ARCANE => Some(super::room_scene::ClassShot::Arcane),
        c if c == HOLY => Some(super::room_scene::ClassShot::Holy),
        _ => None,
    }
}

/// A class's own shot at `(x, y)` flying along `(vx, vy)`, if `colour` is
/// one (`classes::ARCANE`, `classes::HOLY`): the Mage's arcane bolt, a pale
/// orb with a wavering cold tail and motes about it; the Healer's spark, a
/// warm twinkling star trailing embers. `false` for any other look.
pub(super) fn shot(cv: &mut Img, (x, y): (i32, i32), (vx, vy): (f32, f32), colour: &str, tick: u64) -> bool {
    use crate::rules::classes::{ARCANE, HOLY};
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
            // The tail first, waving as it goes, so the orb sits over it.
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
            // Two motes turning round it.
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
            // A star that turns between a cross and a saltire.
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
