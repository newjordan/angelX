//! The sky: a dome of geometry painted from the sky table, so it needs no
//! backdrop pass and the far land can sit in front of it by depth alone.
use super::geo::Soup;
use super::noise::hash;
use super::palette::*;
use super::route::{Look, Route};
use crate::render::world3d::math::{V3, v3};
use crate::render::world3d::mesh::Tri;
use std::f32::consts::{PI, TAU};

/// Beyond the farthest land.
pub const RADIUS: f32 = 1500.;
/// How far either side of the heading the dome is built.
const CONE: f32 = 1.35;
const RINGS: [f32; 10] = [-0.16, 0.0, 0.06, 0.14, 0.26, 0.42, 0.65, 0.95, 1.25, 1.5];
const SEGS: usize = 11;

fn sky_index(look: Look) -> usize { match look { Look::Dune => 0, Look::Forge | Look::Ash => 1, Look::Deep => 2, Look::Castle => 3 } }

fn dir(elev: f32, az: f32) -> V3 { v3(elev.cos() * az.cos(), elev.cos() * az.sin(), elev.sin()) }

pub fn dome(out: &mut Vec<Tri>, r: &dyn Route, eye: V3, heading: f32, look: Look, ticks: u32) {
    let mut g = Soup::new();
    let sky = sky_index(look);
    let at = |e: f32, k: usize| {
        let az = heading - CONE + 2. * CONE * k as f32 / SEGS as f32;
        // The smoke and the clouds drift round the compass, slowly.
        (eye + dir(e, az) * RADIUS, sky_uv(sky, e, az + ticks as f32 * 0.00022))
    };
    for w in RINGS.windows(2) {
        for k in 0..SEGS {
            let (p00, t00) = at(w[0], k);
            let (p01, t01) = at(w[0], k + 1);
            let (p10, t10) = at(w[1], k);
            let (p11, t11) = at(w[1], k + 1);
            g.tri_uv(p00, p01, p11, [t00, t01, t11], SKY);
            g.tri_uv(p00, p11, p10, [t00, t11, t10], SKY);
        }
    }
    // Stars, where the sky is clear; and the moon where there is one.
    let near = |az: f32| { let d = (az - heading).rem_euclid(TAU); (if d > PI { d - TAU } else { d }).abs() < CONE };
    let smoky = matches!(look, Look::Forge | Look::Ash);
    let speck = |g: &mut Soup, i: u32, el: f32, az: f32, size: f32, mat: u8| {
        let c = eye + dir(el, az) * (RADIUS - 24.);
        let (r, u) = (c - eye, v3(-az.sin(), az.cos(), 0.));
        let up = r.normalize().cross(u).normalize();
        let s = size * (0.7 + hash(i as i32, 3, 77));
        g.tri_uv(c - u * s, c + u * s, c + up * s, [special_uv((mat - 128) as usize); 3], SKY);
    };
    if look == Look::Deep {
        // A cave has no stars: the roof is flecked with crystal light, cyan and violet, that comes and goes.
        for i in 0..70u32 {
            let az = hash(i as i32, 1, 79) * TAU;
            let el = 0.05 + hash(i as i32, 2, 79).powf(0.7) * 1.3;
            if !near(az) || (ticks / 9 + i) % 7 == 0 { continue; }
            speck(&mut g, i, el, az, 6., [CYAN_GLOW, CYAN_DIM, VIOLET_GLOW, CYAN_GLOW, VIOLET_DIM][(i % 5) as usize]);
        }
    } else if !smoky {
        // The chase lens sees only the low band of the sky: most of the stars are put there.
        for i in 0..150u32 {
            let az = hash(i as i32, 1, 77) * TAU;
            let el = 0.07 + hash(i as i32, 2, 77).powf(if look == Look::Castle { 1.3 } else { 0.6 }) * 1.35;
            // The bright ones twinkle.
            if !near(az) || (look == Look::Castle && (ticks / 6 + i) % 5 == 0) { continue; }
            speck(&mut g, i, el, az, 6., STAR);
        }
    }
    // The moon and the castle are far off, so they hardly turn as the road does: they keep a fifth of
    // their place in the compass and the rest of the lens' heading, left and right of ahead, so a night
    // on this road always has its moon and its keep in the sky the lens sees.
    let pan = panorama(r, heading);
    let moon_az = pan + MOON_SIDE;
    if look == Look::Castle && near(moon_az) {
        // Low and large: the chase lens sees only a thin band of sky over the horizon.
        let el = 0.105;
        // In front of the dome by more than the dome's chords stand off the sphere (eleven metres).
        let c = eye + dir(el, moon_az) * (RADIUS - 40.);
        let rr = (c - eye).normalize();
        let u = v3(-moon_az.sin(), moon_az.cos(), 0.);
        let up = rr.cross(u).normalize();
        // Two halos and the disc, a ring of the moon's pale light on the night about it.
        for (k, (rad, mat)) in [(0.24 * RADIUS, MOON_HALO_FAR), (0.14 * RADIUS, MOON_HALO), (0.062 * RADIUS, MOON)].into_iter().enumerate() {
            let c = c + rr * (8. - 4. * k as f32);
            for i in 0..20 {
                let (a0, a1) = (i as f32 / 20. * TAU, (i + 1) as f32 / 20. * TAU);
                g.tri_uv(c, c + (u * a0.cos() + up * a0.sin()) * rad, c + (u * a1.cos() + up * a1.sin()) * rad, [special_uv((mat - 128) as usize); 3], SKY);
            }
        }
        // The shade of a sea on the moon's face: two dim bands.
        for (dy, w) in [(-0.012f32, 0.050f32), (0.018, 0.034)] {
            let (cc, rad) = (c + up * (dy * RADIUS) + rr * -6., w * RADIUS);
            g.quad(cc - u * rad, cc + u * rad, cc + u * rad + up * (0.005 * RADIUS), cc - u * rad + up * (0.005 * RADIUS), MOON_HALO);
        }
    }
    if look == Look::Castle { skyline(&mut g, eye, heading, pan); }
    out.extend(g.t);
}

/// How far the moon stands to the right of the lens' heading (radians), and the castle to its left: clear of the
/// plates and the minimap that sit over the sky's upper corners.
const MOON_SIDE: f32 = 0.20;
const CASTLE_SIDE: f32 = -0.32;

/// Where the moon hangs in the course's own compass: round the course's own bearing, ahead of the grid.
pub fn moon_azimuth(r: &dyn Route) -> f32 { r.pose(260.).2 + 0.38 }

/// The bearing the sky's far scenery is hung from this frame: the lens' heading, with a fifth of how far
/// the course's own compass would have turned it, so it drifts across the sky as the road bends.
fn panorama(r: &dyn Route, heading: f32) -> f32 {
    let d = (moon_azimuth(r) - heading + PI).rem_euclid(TAU) - PI;
    heading + d * 0.2
}

/// The far side of the world's night: a ridge of hills, a town's roofs with lit windows, a harbour
/// of sheds and masts, and a castle with a keep and towers, set on the horizon a kilometre off as
/// dark shapes against the moon's glow. One face each, turned to the eye, so a whole city is a few
/// hundred triangles; and only what is in the lens is built.
fn skyline(g: &mut Soup, eye: V3, heading: f32, pan: f32) {
    const D: f32 = 1120.;
    let z0 = eye.z - 60.;
    let seen = |az: f32| { let d = (az - heading).rem_euclid(TAU); (if d > PI { d - TAU } else { d }).abs() < CONE + 0.1 };
    // One block of the skyline: `w` wide, `h` high, a roof of `roof` metres, `win` lit windows, at distance `dist`.
    let put_at = |g: &mut Soup, az: f32, dist: f32, w: f32, h: f32, roof: f32, win: u32, seed: i32, mat: u8| {
        if !seen(az) { return; }
        let c = eye + v3(az.cos(), az.sin(), 0.) * dist;
        let (u, cz) = (v3(-az.sin(), az.cos(), 0.), z0);
        let (a, b) = (c - u * (w * 0.5), c + u * (w * 0.5));
        g.quad(v3(a.x, a.y, cz), v3(b.x, b.y, cz), v3(b.x, b.y, cz + h), v3(a.x, a.y, cz + h), mat);
        if roof > 0. { g.tri(v3(a.x, a.y, cz + h), v3(b.x, b.y, cz + h), c + v3(0., 0., cz - c.z + h + roof), mat); }
        for k in 0..win {
            let (fx, fz) = (hash(seed, k as i32, 51) - 0.5, 0.25 + 0.6 * hash(seed, k as i32, 52));
            let p = c + u * (fx * w * 0.8);
            let q = v3(p.x, p.y, cz + h * fz);
            g.quad(q - u * 2.2, q + u * 2.2, q + u * 2.2 + v3(0., 0., 4.), q - u * 2.2 + v3(0., 0., 4.), if k % 3 == 0 { WINDOW_DIM } else { WINDOW_HOT });
        }
    };
    let put = |g: &mut Soup, az: f32, w: f32, h: f32, roof: f32, win: u32, seed: i32| put_at(g, az, D, w, h, roof, win, seed, SKYLINE);
    // The ridge: far hills across the whole lens, a shade lighter than the town, a day's ride behind it.
    for i in 0..48 {
        let az = pan - 1.45 + i as f32 * 0.062;
        let hgt = 26. + 46. * hash(i, 8, 61) + 30. * (az * 2.3).sin().abs();
        put_at(g, az, D + 160., 88., hgt, 0., 0, i + 400, HILL_FAR);
    }
    let ck = pan + CASTLE_SIDE;
    // The town: houses and steeples, close set, lit; it runs both sides of the castle and leaves its place.
    for i in 0..30 {
        let az = pan - 1.3 + i as f32 * 0.068;
        if (az - ck).abs() < 0.34 { continue; }
        let steeple = i % 7 == 3;
        put(g, az, if steeple { 18. } else { 28. + 30. * hash(i, 4, 61) }, if steeple { 60. } else { 22. + 30. * hash(i, 5, 61) }, if steeple { 40. } else { 14. + 10. * hash(i, 6, 61) }, 2, i + 200);
    }
    // The harbour: sheds, with a mast or two rising from them and a crane, away to the far side.
    for i in 0..7 {
        let az = pan + 0.95 + i as f32 * 0.085;
        put(g, az, 50. + 40. * hash(i, 1, 61), 12. + 16. * hash(i, 2, 61), 0., 1, i + 100);
        if i % 3 == 1 { put(g, az + 0.02, 3., 70. + 30. * hash(i, 3, 61), 0., 0, i); put(g, az + 0.02, 34., 3., 0., 0, i); }
    }
    // The castle: a curtain wall with its battlements, a tower at each end and two between, and the keep
    // over all of them, a hundred and forty metres across the face and crowned with a spire and a pennant.
    put(g, ck, 640., 58., 0., 0, 300);
    for k in 0..26 { let d = (k as f32 - 12.5) * 0.0178; put(g, ck + d, 14., 70., 0., 0, 0); }
    for (d, h, w, roof) in [(-0.26f32, 120., 54., 48.), (-0.135, 96., 46., 38.), (0.135, 104., 46., 40.), (0.26, 126., 56., 50.)] {
        put(g, ck + d, w, h, roof, 3, 310 + (d * 100.) as i32);
        // The crown of a tower: three merlons.
        for m in [-0.3f32, 0., 0.3] { put(g, ck + d + m * w / D, 7., h + 8., 0., 0, 0); }
    }
    put(g, ck, 150., 215., 74., 7, 330);
    for m in 0..6 { put(g, ck + (m as f32 - 2.5) * 22. / D, 10., 224., 0., 0, 0); }
    // The keep's face: a lit hall high up and the dark door under it; pennants on the spire and a tower.
    if seen(ck) {
        let c = eye + v3(ck.cos(), ck.sin(), 0.) * (D - 6.);
        let u = v3(-ck.sin(), ck.cos(), 0.);
        // The hall's lit stone face high on the keep, with its window, and a tall slit of dark below.
        g.quad(c - u * 22. + v3(0., 0., z0 + 150. - c.z), c + u * 22. + v3(0., 0., z0 + 150. - c.z), c + u * 22. + v3(0., 0., z0 + 188. - c.z), c - u * 22. + v3(0., 0., z0 + 188. - c.z), KEEP_LIT);
        g.quad(c - u * 5. + v3(0., 0., z0 + 160. - c.z + 0.2), c + u * 5. + v3(0., 0., z0 + 160. - c.z + 0.2), c + u * 5. + v3(0., 0., z0 + 176. - c.z), c - u * 5. + v3(0., 0., z0 + 176. - c.z), WINDOW_HOT);
    }
    for (az, top) in [(ck, 215. + 74. + 34.), (ck + 0.26, 126. + 50. + 30.)] {
        if !seen(az) { continue; }
        let c = eye + v3(az.cos(), az.sin(), 0.) * (D - 4.);
        let u = v3(-az.sin(), az.cos(), 0.);
        let t = v3(c.x, c.y, z0 + top);
        g.tri(t, t + u * 26. + v3(0., 0., -5.), t + v3(0., 0., -11.), PENNANT);
        g.quad(t - u * 1.2, t + u * 1.2, t + u * 1.2 + v3(0., 0., 22.), t - u * 1.2 + v3(0., 0., 22.), SKYLINE);
    }
}
