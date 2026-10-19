//! Griffinwing's stage: the canyon in true 3D through world3d's banded
//! raster, under a chase camera riding behind this seat's griffin (the same
//! trailing eye race3d's griffin has, with a little lag and a shake only on a
//! hit), laid down as ordered dots on black paper; the foes, wingmen and the
//! Wyrm are labelled blocks in it, and the reticle, brackets, footprints and
//! HUD sit over it (docs/ROUNDTABLE-ADVENTURES.md has the contract, the
//! brief the design). `layers` is what a seat sees (384 by 224: the whole
//! picture in the base, which is what the native route draws),
//! `terminal_layers` what the terminal's cell painter draws (the same until
//! the cells want their own), `kingdom` the door on the walking map. Sprites
//! go through the slot art registry like the Forest Road's, never into the
//! arena; art never sets a hitbox.
mod actors;
mod bank;
mod cam;
mod canyon;
mod hud;
mod layout;
mod paint;
mod set;
mod sky;

use super::super::ink::Img;
use super::race3d::dots_with;
use super::roundtable::{self as shell, Look};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Mesh;
use crate::render::world3d::raster::{self, Backdrop, Firelight};
use crate::rules::griffinwing::trig::Det;
use crate::rules::griffinwing::{Craft, Mode, Session, rail, rail::units};
use crate::rules::{Run, griffinwing as rules};
use cam::Cam;

const LOOK: Look = Look {
    name: rules::NAME,
    backdrop: 'q',
    edge: 'z',
    accent: 't',
};

const W: i32 = 384;
const H: i32 = 224;
/// The tone the raster's frame is put on black paper at: the black point, the
/// white point, the gamma and the floor an ink is lifted to. The realm's own
/// night look (race3d's) is darker; a canyon has to be read, so it lifts more:
/// from the black point of nothing, with the white point down at 70, so the
/// rock's shadowed faces land on a dusk blue and not on black, while the
/// griffins, which are lit above it, keep their brightness.
const TONE: (f32, f32, f32, f32) = (0.0, 70.0, 0.8, 100.0);
/// How far the haze reaches over the first two beats and over the roost's: the
/// far walls fade into the dusk by this distance, not to black short of it.
const FOG: (f32, f32) = (110.0, 90.0);

std::thread_local! {
    /// The moving things' mesh, rebuilt each frame into the same buffer, and
    /// the world as the eye has it (`cam.rs`).
    static MOVERS: std::cell::RefCell<Mesh> = std::cell::RefCell::new(Mesh::new());
    static STAGED: std::cell::RefCell<Mesh> = std::cell::RefCell::new(Mesh::new());
}

/// The point of the rail in the world.
fn at(s: f32, x: f32, z: f32) -> V3 {
    let w = rail::rail().world(s, x, z);
    v3(w[0], w[1], w[2])
}

/// The chase camera behind `c`'s griffin: back along the rail, a little up,
/// looking a way down it. It follows the craft across at seven-tenths and
/// leans against its sideways speed, so it trails and lags a little; a hit
/// shakes it, and nothing else does.
fn camera(s: &Session, c: &Craft) -> Cam {
    let su = units(s.craft_s(c));
    let (x, z) = (units(c.x), units(c.z));
    let (lag_x, lag_z) = (units(c.vx) * 5.0, units(c.vz) * 5.0);
    let boost = if c.boost { 1.0 } else { 0.0 };
    let brake = if c.brake { 1.0 } else { 0.0 };
    let back = 7.0 + boost * 0.9 - brake * 0.6;
    let ahead = 12.0 + if s.boss.is_some() { 3.0 } else { 0.0 };
    let (mut ex, mut ez) = ((x - lag_x) * 0.7, z * 0.85 - lag_z * 0.5 + 1.9);
    if c.hurt > 26 && c.down == 0 {
        // A blow: a few ticks of shake, dying away.
        let k = (c.hurt - 26) as f32 * 0.03;
        let t = s.elapsed as f32;
        ex += (t * 2.9).dsin() * k;
        ez += (t * 3.7).dcos() * k;
    }
    let eye = at(su - back, ex, ez);
    // The eye swings into a turn a little before the craft gets there.
    let (lead_x, lead_z) = (units(c.vx) * 8.0, units(c.vz) * 6.0);
    let look = at(su + ahead, x * 0.45 + lead_x, z * 0.8 + 0.4 + lead_z);
    let d = look - eye;
    let flat = (d.x * d.x + d.y * d.y).sqrt().max(1e-3);
    Cam::new(eye, d.y.datan2(d.x), d.z.datan2(flat))
}

/// Which sky: the moon's dusk over the gorge and the causeway, the roost's
/// smoke, and a warm dawn for the way home (painted over what the raster
/// leaves bare, `sky.rs`).
fn sky_of(s: &Session) -> sky::Sky {
    match (s.stage, s.mode) {
        (_, Mode::Victory | Mode::Tally) => sky::Sky::Dawn,
        (2, _) => sky::Sky::Smoke,
        _ => sky::Sky::Dusk,
    }
}

/// The 3D scene alone, as dots, and the depth under it.
fn dots(s: &Session, seat: u32) -> (Img, Vec<f32>, Cam) {
    let c = s
        .seats
        .get(&seat)
        .map(|x| x.craft.clone())
        .unwrap_or_else(|| {
            s.seats
                .values()
                .next()
                .map_or_else(|| Craft::new(0, 0), |x| x.craft.clone())
        });
    let cam = camera(s, &c);
    let su = units(s.craft_s(&c));
    let (rgba, depth) = MOVERS.with(|cell| {
        let mut movers = cell.borrow_mut();
        actors::movers(&mut movers, s, seat, cam.eye);
        STAGED.with(|staged| {
            let mut staged = staged.borrow_mut();
            staged.tris.clear();
            for chunk in canyon::near(su) {
                cam.stage(&mut staged, chunk);
            }
            cam.stage(&mut staged, &movers);
            raster::render_meshes_painted(
                &[&staged],
                &cam.view(),
                W as usize,
                H as usize,
                Firelight::STEADY,
                if s.stage == 2 { FOG.1 } else { FOG.0 },
                Backdrop::Dark([0, 0, 0]),
                paint::painted(s.stage),
            )
        })
    });
    let mut im = dots_with(rgba.as_raw(), W, H, TONE);
    sky::paint(&mut im, &depth, &cam.sky_view(), sky_of(s), s.elapsed);
    set::paint(&mut im, &depth, &cam, s, seat);
    (im, depth, cam)
}

/// The whole picture, for `seat`.
fn scene(run: &Run, s: &Session, seat: u32) -> Img {
    let (mut im, depth, cam) = dots(s, seat);
    hud::paint(
        &mut im,
        &hud::Ctx {
            s,
            run,
            seat,
            cam: &cam,
            depth: &depth,
        },
    );
    im
}

pub(super) fn layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    let Some(s) = run.griffinwing.as_ref() else {
        return shell::stub_layers(run, focus, &LOOK);
    };
    let seat = focus
        .filter(|id| s.seats.contains_key(id))
        .or_else(|| s.seats.keys().next().copied());
    let Some(seat) = seat else {
        return shell::stub_layers(run, focus, &LOOK);
    };
    (scene(run, s, seat), Img::new(W, H))
}

pub(super) fn terminal_layers(run: &Run, focus: Option<u32>) -> (Img, Img) {
    layers(run, focus)
}

/// The door on the walking map: the shell's post and board with a pair of
/// gold wings on the board.
pub(super) fn kingdom(im: &mut Img, _run: &Run, origin: (i32, i32)) {
    shell::stub_kingdom(im, origin, rules::ENTRY, &LOOK);
    let (x, y) = (
        rules::ENTRY.0 * 16 + 8 - origin.0,
        rules::ENTRY.1 * 16 + 8 - origin.1,
    );
    if (-12..396).contains(&x) && (-24..248).contains(&y) {
        let wings = Img::from_rows(&[
            "k.............k",
            "kk...kkkkk...kk",
            "kOk.kOOOOOk.kOk",
            ".kOkOOOHOOOkOk.",
            "..kOOOOOOOOOk..",
            "...kOOOoOOOk...",
            "....kkOOOkk....",
            ".......k.......",
        ]);
        im.stamp(&wings, x - 7, y - 10);
    }
}

#[cfg(test)]
#[path = "../../../../tests/render/arena__griffinwing__tests.rs"]
mod tests;
