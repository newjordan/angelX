//! The time-trial ghost on the road: the saved lap's pod, drawn through the pod's own mesh and
//! laid over the picture a dot at a time, behind whatever is nearer, so it reads as a thing of
//! glass; and a plate saying how far ahead or behind it is.
use super::kit::*;
use super::*;
use super::super::camera::project;

/// The seat the ghost wears the colours of: Cael's blue.
const GHOST_SEAT: u32 = 3;

pub fn draw(im: &mut Img, race: &rules::Session, car: &rules::Car, seat: u32, view: &View3, depth: &[f32]) {
    if race.flow.mode != rules::Mode::TimeTrial || race.flow.host() != Some(seat) { return; }
    let Some(g) = race.flow.ghost else { return };
    let len = race.course().lap();
    // The lap the pod is on, nearest the viewer's: the ghost's own lap-relative distance.
    let d = g.s + len * ((car.d - g.s) / len).round();
    let mut ghost = car.clone();
    ghost.d = d;
    ghost.y = g.y;
    ghost.lift = g.lift;
    ghost.route = g.route;
    ghost.pod = g.pod;
    ghost.yaw = 0.;
    ghost.vy = 0.;
    ghost.v = 40.;
    ghost.boosting = false;
    ghost.finish = None;
    ghost.impact = 0;
    let mut mesh = Mesh::new();
    pods::add(&mut mesh, race, GHOST_SEAT, &ghost, view.pos);
    if !mesh.tris.is_empty() {
        let (rgba, gd) = super::super::garage::shoot(&[&mesh], view, Firelight { gain: 1.9, warm: 0.0 }, 200., Backdrop::Dark([0, 0, 0]));
        let lit = dots_with(rgba.as_raw(), 384, 224, (0., 82., 0.65, 150.));
        for y in 0..224i32 {
            for x in 0..384i32 {
                // Every other dot: the road shows through.
                if (x + y) % 2 != 0 { continue; }
                let i = (y * 384 + x) as usize;
                if !gd[i].is_finite() || gd[i] >= depth[i] { continue; }
                if let Some(c) = lit.get(x, y) { if c != [0, 0, 0] { im.set(x, y, c); } }
            }
        }
    }
    // A tag over it, and the gap in the corner.
    if let Some((x, y, z)) = project(view, point(d - 2.5, g.y, 5.2 + g.lift.max(0.))) {
        if z < 120. && (8.0..376.0).contains(&x) && (40.0..190.0).contains(&y) {
            centred(im, x as i32, y as i32 - 6, "GHOST", 1, 'z');
        }
    }
    let ahead = g.gap > 0.;
    plate(im, 6, 54, 58, 24);
    im.text(12, 58, "GHOST", 't');
    let gap = format!("{}{:.1}", if ahead { "+" } else { "-" }, g.gap.abs());
    im.text(12, 68, &gap, if ahead { 'O' } else { 'y' });
    im.text(12 + text_w(&gap, 1) + 4, 68, if ahead { "AHEAD" } else { "BEHIND" }, 't');
}
