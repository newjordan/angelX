//! The Gravecaller, drawn: a hero of the barrows, half a head over the
//! company and broad with it, in a hooded slate mantle over old plate,
//! a skull for a face with grave-light in its eyes, pauldrons spiked with
//! bone, and a great staff with a headstone for its head. Toward the
//! viewer and away, the staff stands at his side behind him; across, he
//! holds it out before him. Striking or casting, his arm goes up and the
//! staff with it, and the headstone flares with grave-fire. The seat's
//! trim is his tabard and his hem.

use super::super::super::ink::Img;
use super::super::knights::Facing;
use super::{Layer, Look, Pose, View};

/// His canvas: the body is twenty wide, and the staff wants four more
/// each side, so the feet stay centred whichever way he holds it.
pub const W: i32 = 28;
const MARGIN: i32 = 4;

const FRONT: [&str; 22] = [
    "........xxxx........",
    ".......xSuuSx.......",
    "......xSuUUuSx......",
    ".....xSuxkkxuSx.....",
    "....xSxkhiHhkxSx....",
    "....xSxkYhhYkxSx....",
    ".i..xSxkihihkxSx..i.",
    ".hi.xSSxkkkkxSSx.ih.",
    ".gJhJGxSSuuSSxGJhJg.",
    "gJhiHJxSS77SSxgJhiJg",
    "gJiHiJxSS78SSxgJihJg",
    ".gGJGjxSS77SSxjGJGg.",
    ".xSuxSuxS78SxuSxuSx.",
    ".xSuxSIbobbobISxuSx.",
    ".xSuxSuIbhkhbISxuSx.",
    ".gGgxSuxS77SxuSxgGg.",
    ".gjgxSuxS78SxuSxgjg.",
    "...xSuuxS77SxuuSx...",
    "..xSuuuxS78SxuuuSx..",
    "..8777777777777778..",
    "....nbbn....nbbn....",
    "....nnnn....nnnn....",
];

const FRONT_UP: [&str; 22] = [
    "........xxxx........",
    ".......xSuuSx.......",
    "......xSuUUuSx......",
    ".....xSuxkkxuSx.....",
    "....xSxkhiHhkxSxgGg.",
    "....xSxkYhhYkxSxgjgx",
    ".i..xSxkihihkxSxxSux",
    ".hi.xSSxkkkkxSSxxSux",
    ".gJhJGxSSuuSSxGJhJg.",
    "gJhiHJxSS77SSxgJhiJg",
    "gJiHiJxSS78SSxgJihJg",
    ".gGJGjxSS77SSxjGJGg.",
    ".xSuxSuxS78SxuSx....",
    ".xSuxSIbobbobISx....",
    ".xSuxSuIbhkhbISx....",
    ".gGgxSuxS77SxuSx....",
    ".gjgxSuxS78SxuSx....",
    "...xSuuxS77SxuuSx...",
    "..xSuuuxS78SxuuuSx..",
    "..8777777777777778..",
    "....nbbn....nbbn....",
    "....nnnn....nnnn....",
];

const SIDE: [&str; 22] = [
    ".......xxxx.........",
    "......xSuuSx........",
    ".....xSuUUuSx.......",
    "....xSuUuuSkkx......",
    "....xSuUuSkhiHkx....",
    "....xSuUuSkhhYhx....",
    "....xSuuuSkihiHx....",
    "...xSSuuuSxkkkx.....",
    "..xSSgjGGjgSxx......",
    ".xSSgGJhJGjgSx......",
    ".xSSgJhiJGjgSx......",
    ".xSSSgjjjgg7Sx......",
    ".xSSSSuuxS78Sx......",
    ".xSSSSuIbbbSobx.....",
    ".xSSSSuuxS78SxxgGg..",
    ".xSSSSuuxS77Sx.gjg..",
    ".xSSSSuuuxS78Sx.....",
    ".xSSSSuuuxS778Sx....",
    ".xSSSSuuuxS7778Sx...",
    "x8777777777777778...",
    "....nbbbn..nbbbn....",
    "....nnnnn..nnnnn....",
];

const SIDE_UP: [&str; 22] = [
    ".......xxxx.........",
    "......xSuuSx........",
    ".....xSuUUuSx.......",
    "....xSuUuuSkkx......",
    "....xSuUuSkhiHkx....",
    "....xSuUuSkhhYhxgGg.",
    "....xSuuuSkihiHxgjg.",
    "...xSSuuuSxkkkxxSux.",
    "..xSSgjGGjgSxxxSux..",
    ".xSSgGJhJGjgSxSux...",
    ".xSSgJhiJGjgSxx.....",
    ".xSSSgjjjgg7Sx......",
    ".xSSSSuuxS78Sx......",
    ".xSSSSuIbbbSobx.....",
    ".xSSSSuuxS78Sx......",
    ".xSSSSuuxS77Sx......",
    ".xSSSSuuuxS78Sx.....",
    ".xSSSSuuuxS778Sx....",
    ".xSSSSuuuxS7778Sx...",
    "x8777777777777778...",
    "....nbbbn..nbbbn....",
    "....nnnnn..nnnnn....",
];

const BACK: [&str; 22] = [
    "........xxxx........",
    ".......xSuuSx.......",
    "......xSuUUuSx......",
    ".....xSuUuuuuSx.....",
    "....xSuUuuuuuuSx....",
    "....xSuUuuuuuSSx....",
    ".i..xSuuuuuuuSSx..i.",
    ".hi.xSSuuuuuSSSx.ih.",
    ".gJhJGxSSuuSSxGJhJg.",
    "gJhiHJxSuuuuSxgJhiJg",
    "gJiHiJxSuuuuSxgJihJg",
    ".gGJGjxSuuuuSxjGJGg.",
    ".xSuxSuxuuuuxuSxuSx.",
    ".xSuxSIbbbbbbISxuSx.",
    ".xSuxSuIbbbbbISxuSx.",
    ".gGgxSuxuuuuxuSxgGg.",
    ".gjgxSuxuuuuxuSxgjg.",
    "...xSuuxuuuuxuuSx...",
    "..xSuuuxuuuuxuuuSx..",
    "..8777777777777778..",
    "....nbbn....nbbn....",
    "....nnnn....nnnn....",
];

const BACK_UP: [&str; 22] = [
    "........xxxx........",
    ".......xSuuSx.......",
    "......xSuUUuSx......",
    ".....xSuUuuuuSx.....",
    ".gGgxSuUuuuuuuSx....",
    "xgjgxSuUuuuuuSSx....",
    "xSuSxSuuuuuuuSSx..i.",
    "xSuSxSSuuuuuSSSx.ih.",
    ".gJhJGxSSuuSSxGJhJg.",
    "gJhiHJxSuuuuSxgJhiJg",
    "gJiHiJxSuuuuSxgJihJg",
    ".gGJGjxSuuuuSxjGJGg.",
    "....xSuxuuuuxuSxuSx.",
    "....xSIbbbbbbISxuSx.",
    "....xSuIbbbbbISxuSx.",
    "....xSuxuuuuxuSxgGg.",
    "....xSuxuuuuxuSxgjg.",
    "...xSuuxuuuuxuuSx...",
    "..xSuuuxuuuuxuuuSx..",
    "..8777777777777778..",
    "....nbbn....nbbn....",
    "....nnnn....nnnn....",
];

const STAFF: [&str; 26] = [
    ".....C...",
    "....CYC..",
    "...eCYCe.",
    "....xxx..",
    "...xSUSx.",
    "..xSUVUSx",
    "..SVVVVVS",
    "..SUuVuuS",
    "..SuuVuuS",
    "..xSSSSSx",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....nbn..",
];

const RAISED: [&str; 25] = [
    "..e..C..e",
    "...CeYeC.",
    ".eCYYWYYC",
    "...CYWYC.",
    "..e.CYC.e",
    "....xxx..",
    "...xSUSx.",
    "..xSUVUSx",
    "..SVVVVVS",
    "..SUuVuuS",
    "..SuuVuuS",
    "..xSSSSSx",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....IpI..",
    "....BpB..",
    "....nbn..",
];

/// Where the staff stands against the body's top-left, at rest and
/// raised, and whether it is behind him.
fn staff_at(facing: Facing, raised: bool) -> ((i32, i32), bool) {
    let lift = if raised { -12 } else { -4 };
    match facing {
        Facing::Down => ((13, lift), true),
        Facing::Side => ((13, lift), false),
        Facing::Up => ((-3, lift), true),
    }
}

static VIEWS: [View; 3] = [
    View { body: &FRONT, item: Layer { at: (13, -4), rows: &STAFF }, strike: Layer { at: (13, -12), rows: &RAISED } },
    View { body: &SIDE, item: Layer { at: (13, -4), rows: &STAFF }, strike: Layer { at: (13, -12), rows: &RAISED } },
    View { body: &BACK, item: Layer { at: (-3, -4), rows: &STAFF }, strike: Layer { at: (-3, -12), rows: &RAISED } },
];

/// His drawings as the classes' are kept: the body at rest, the staff at
/// rest and raised.
pub(super) fn views() -> &'static [View; 3] {
    &VIEWS
}

/// The Gravecaller whole, facing right when across (the caller flips him
/// for left), his feet on the bottom row and his middle at `W / 2`.
/// `held`: whether the staff is drawn at all. Striking with it, his arm is
/// up and the staff raised over his head.
pub(in super::super) fn figure(facing: Facing, pose: Pose, held: bool) -> Img {
    let raised = pose == Pose::Strike;
    let rows: &[&str] = match (facing, raised) {
        (Facing::Down, false) => &FRONT,
        (Facing::Down, true) => &FRONT_UP,
        (Facing::Side, false) => &SIDE,
        (Facing::Side, true) => &SIDE_UP,
        (Facing::Up, false) => &BACK,
        (Facing::Up, true) => &BACK_UP,
    };
    let body = super::step(Look::Gravecaller, facing, Img::from_rows(rows), pose);
    let ((sx, sy), behind) = staff_at(facing, raised);
    let staff = held.then(|| Img::from_rows(if raised { &RAISED[..] } else { &STAFF[..] }));
    let top = staff.as_ref().map_or(0, |_| sy.min(0));
    let bottom = body.h.max(staff.as_ref().map_or(0, |s| sy + s.h));
    let mut im = Img::new(W, bottom - top);
    if let Some(staff) = staff.as_ref().filter(|_| behind) {
        im.stamp(staff, MARGIN + sx, sy - top);
    }
    im.stamp(&body, MARGIN, -top);
    if let Some(staff) = staff.as_ref().filter(|_| !behind) {
        im.stamp(staff, MARGIN + sx, sy - top);
    }
    im
}

pub(super) fn floating_figure(facing: Facing, pose: Pose, held: bool, _tick: u64, _phase: u32, aim: (f32, f32)) -> Img {
    let body = figure(facing, pose, false);
    if !held { return body; }
    let staff = Img::from_rows(&STAFF);
    super::super::wear::clocked(&body, &staff, (4, staff.h / 2), -std::f32::consts::FRAC_PI_2,
        (body.w / 2 + 5, body.h - 9), aim)
}

/// Where the headstone of his staff is, against the figure's feet (its
/// middle at `W / 2`), facing right: the grave-fire's place when he casts.
pub(in super::super) fn staff_head(facing: Facing, raised: bool) -> (i32, i32) {
    let ((sx, sy), _) = staff_at(facing, raised);
    let rows = FRONT.len() as i32;
    (MARGIN + sx + 5 - W / 2, sy + if raised { 2 } else { 1 } - rows)
}

/// His hood, plate and feet, excluding the staff's headroom.
pub(super) fn fit(figure: &Img, hero: &crate::rules::Hero) -> crate::rules::looks::Body {
    let top = figure.h - 22;
    let face = super::super::knights::facing(hero.aim_x, hero.aim_y);
    let view = match face { Facing::Down => "front", Facing::Side => "right", Facing::Up => "back" };
    let pose = super::pose(hero);
    let lift = i32::from(matches!(super::stride(hero), Some(1 | 3)) && pose != Pose::Strike);
    let margin = MARGIN + (figure.w - W) / 2;
    let mut fit = crate::rules::looks::profile(&format!("gravecaller_{view}")).shifted(margin, top - lift);
    let ((sx,sy),_) = staff_at(face, pose == Pose::Strike);
    // Hand meets the staff's grip, not the canvas edge beside its headstone.
    fit.right.anchor = (margin + sx + 4, top - lift + sy + 14);
    fit
}

pub(in super::super) fn floating_staff_head(facing: Facing, raised: bool, aim: (f32, f32)) -> (i32, i32) {
    let pose = if raised { Pose::Strike } else { Pose::Stand };
    let body = figure(facing, pose, false);
    let staff = Img::from_rows(&STAFF);
    let (x, y) = super::super::wear::clock_point(&body, &staff, (4, staff.h / 2),
        -std::f32::consts::FRAC_PI_2, (body.w / 2 + 5, body.h - 9), aim, (5, 1));
    (x - body.w / 2, y - body.h + 1)
}
