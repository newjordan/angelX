//! The world around the Knight Racers' road: real terrain, set pieces and sky.
//!
//! A frame never builds the world. The road is built in blocks, the land in
//! chunks, each once, at a level of detail by distance; a frame gathers the
//! ones in view into one mesh, and only the sky is made fresh.
mod ambient;
mod cache;
mod dynamic;
mod frame;
mod geo;
mod index;
mod look;
mod noise;
pub mod palette;
mod pieces;
mod pit;
mod setpieces;
mod props;
mod road;
mod roadtex;
mod roadtex_glass;
mod roadtex_moat;
mod roadtex_wash;
pub mod route;
mod scenery;
mod sky;
mod sky_wash;
mod terrain;
mod tex;
mod trackside;
#[cfg(test)]
mod frame_time;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod shots;
#[cfg(test)]
mod polish;

use frame::row;
use route::Route;
use crate::render::world3d::math::V3;
use crate::render::world3d::mesh::{Mesh, Tri};

pub const TRIANGLE_BUDGET: usize = 32_000;

const ROAD_BEHIND: f32 = 70.;
const ROAD_AHEAD: f32 = 340.;
const HORIZON: f32 = 1150.;
const KEEP: f32 = 1.0;

pub fn fog_for(look: route::Look) -> f32 {
    match look { route::Look::Deep => 230., route::Look::Wash => 210., route::Look::Forge | route::Look::Ash => 190., route::Look::Castle => 170., _ => 130. }
}

pub fn dye(im: &mut super::Img, rgba: &[u8]) {
    const VIOLET: [char; 6] = ['d', '(', ')', '[', ']', '^'];
    for y in 18..176 {
        for x in 0..384 {
            let i = ((y * 384 + x) * 4) as usize;
            let (r, g, b) = (rgba[i] as i32, rgba[i + 1] as i32, rgba[i + 2] as i32);
            if !(b > r + 10 && r > g + 14) { continue; }
            let Some(now) = im.get(x, y) else { continue };
            if now == [0, 0, 0] { continue; }
            let tone = (now[0].max(now[1]).max(now[2]) as f32 / 215.).clamp(0., 1.);
            im.put(x, y, VIOLET[((tone * 5.) + 0.5) as usize]);
        }
    }
}

pub fn haze(look: route::Look) -> [u8; 3] { palette::haze_of(look) }

const VAULT_SEE: f32 = 170.;

fn enclosed(r: &dyn Route, eye: V3, heading: f32) -> bool {
    let Some(n) = index::of(r).nearest(eye.x, eye.y, 30.) else { return false };
    if n.dist >= n.edge || eye.z - n.h > 14. { return false; }
    let inside = r.pieces().iter().any(|p| matches!(p.kind, route::Kind::Tunnel | route::Kind::CrystalVault) && n.s >= p.s0 + 20. && n.s <= p.s1 - VAULT_SEE);
    let forked = r.forks().iter().any(|f| f.s0 <= n.s + VAULT_SEE && f.s1 >= n.s - 30.);
    let (_, _, th) = r.pose(n.s);
    let turn = (heading - th + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    inside && !forked && turn.abs() < 1.1
}

pub fn point(r: &dyn Route, s: f32, lat: f32, up: f32) -> V3 { row(r, s).pt(lat, up) }

pub fn build(r: &dyn Route, s: f32) -> Mesh { build_at(r, s, route::ticks()) }

pub fn build_at(r: &dyn Route, s: f32, ticks: u32) -> Mesh { gather(r, s, ticks, &mut Tally::default()) }

#[derive(Default, Debug, Clone, Copy)]
pub struct Tally { pub sky: usize, pub road: usize, pub props: usize, pub pieces: usize, pub hazards: usize, pub land: usize }

pub fn gather(r: &dyn Route, s: f32, ticks: u32, tally: &mut Tally) -> Mesh {
    let eye = point(r, s - 27., 0., 7.);
    let aim = point(r, s + 28., 0., 1.5);
    let d = aim - eye;
    gather_view(r, s, ticks, eye, d.y.atan2(d.x), KEEP, tally)
}

pub fn gather_view(r: &dyn Route, s: f32, ticks: u32, eye: V3, heading: f32, keep: f32, tally: &mut Tally) -> Mesh {
    let fwd = (heading.cos(), heading.sin());
    let mut tris: Vec<Tri> = Vec::with_capacity(16_384);
    let vaulted = enclosed(r, eye, heading);
    if !vaulted { sky::dome(&mut tris, r, eye, heading, r.zone(s + 60.).0, ticks); }
    tally.sky = tris.len();

    let lap = r.lap();
    let lines = 1 + r.forks().len();
    for line in 0..lines as u8 {
        let (n, first, len) = road::blocks(r, line);
        let s_here = if line == 0 { s } else { s.rem_euclid(lap) };
        let (k0, k1) = (((s_here - ROAD_BEHIND - first) / len).floor() as i32, ((s_here + ROAD_AHEAD - first) / len).floor() as i32);
        let (k0, k1) = if line == 0 { (k0, k1) } else { (k0.max(0), k1.min(n - 1)) };
        if k0 > k1 { continue; }
        for k in k0..=k1 {
            let away = (first + (k as f32 + 0.5) * len - s_here).abs();
            let lod = if away < 120. { 0 } else if away < 230. { 1 } else { 2 };
            let n = tris.len();
            tris.extend_from_slice(&road::block(r, line, k, lod));
            tally.road += tris.len() - n;
            if line == 0 { tris.extend_from_slice(&props::block(r, k)); }
        }
        if line > 0 { tris.extend_from_slice(&props::fork(r, line)); }
        let n = tris.len();
        for (i, p) in pieces::list(r, line).iter().enumerate() {
            for seg in 0..pieces::segments(p) {
                let (w0, w1) = pieces::window(p, seg);
                let (a0, a1) = (w0.max(p.s0), w1.min(p.s1));
                let (behind, ahead) = pieces::reach(p.kind);
                if [-lap, 0., lap].iter().any(|o| a1 + o >= s - behind && a0 + o <= s + ahead) {
                    tris.extend_from_slice(&pieces::piece(r, line, i, seg));
                }
            }
        }
        tally.pieces += tris.len() - n;
    }
    tally.props = tris.len() - tally.sky - tally.road - tally.pieces;
    terrain::flow_all(&mut tris[tally.sky..], ticks);

    let n = tris.len();
    dynamic::hazards(&mut tris, r, s, ticks);
    dynamic::strips(&mut tris, r, s, ticks);
    pit::arms(&mut tris, r, s, ticks);
    ambient::air(&mut tris, r, s, eye, heading, ticks);
    tally.hazards = tris.len() - n;
    let n = tris.len();

    if vaulted { return Mesh { tris }; }
    let c = terrain::CHUNK;
    let (cx0, cx1) = (((eye.x - HORIZON) / c).floor() as i32, ((eye.x + HORIZON) / c).ceil() as i32);
    let (cy0, cy1) = (((eye.y - HORIZON) / c).floor() as i32, ((eye.y + HORIZON) / c).ceil() as i32);
    for cy in cy0..cy1 {
        for cx in cx0..cx1 {
            let (mx, my) = ((cx as f32 + 0.5) * c - eye.x, (cy as f32 + 0.5) * c - eye.y);
            let dist = mx.hypot(my);
            if dist > HORIZON + 60. { continue; }
            if dist > 130. {
                let spread = (70. / dist).min(1.).asin();
                if (mx * fwd.0 + my * fwd.1) / dist < (keep + spread).min(3.0).cos() { continue; }
            }
            let road_here = cache::flag(r.key(), cx, cy, || terrain::on_road(r, cx, cy));
            let reach = ((mx.abs() - c * 0.5).max(0.)).hypot((my.abs() - c * 0.5).max(0.));
            let at = tris.len();
            tris.extend_from_slice(&terrain::chunk(r, cx, cy, terrain::lod_for(reach + 48., road_here)));
            terrain::flow(&mut tris[at..], ticks);
        }
    }
    tally.land = tris.len() - n;
    Mesh { tris }
}
