//! Helpers the course files share: the pieces of authoring that are the same on every map.
use super::course::*;

pub fn chord(main: &Line, s0: f32, s1: f32, side: f32, bulge: f32, dive: f32, w: f32, n: usize) -> Vec<Cp> {
    let (a, b) = (main.at(s0), main.at(s1));
    let put = |p: &Sample| { let off = if side > 0. { p.wl } else { p.wr } * 0.5; (p.x - side * off * p.th.sin(), p.y + side * off * p.th.cos(), p.z) };
    let ((ax, ay, az), (bx, by, bz)) = (put(&a), put(&b));
    let len = (bx - ax).hypot(by - ay).max(1.);
    let (nx, ny) = (-(by - ay) / len, (bx - ax) / len);
    (1..=n).map(|i| {
        let q = i as f32 / (n + 1) as f32;
        let sw = (q * std::f32::consts::PI).sin();
        Cp { x: ax + (bx - ax) * q + nx * bulge * sw, y: ay + (by - ay) * q + ny * bulge * sw, z: az + (bz - az) * q - dive * sw, wl: w, wr: w, bank: 0. }
    }).collect()
}

pub fn along(fork: &Fork, q: f32) -> f32 { fork.s0 + (fork.s1 - fork.s0) * q }
