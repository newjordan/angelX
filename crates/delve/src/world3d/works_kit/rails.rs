//! The laid lines and what runs on them, in the monolith language: roads
//! worn into the grass, rails with sleepers on ballast, a ropeway's towers
//! with the cable sagging between them and buckets hung from it, the chute
//! on its trestles, the barge's lane; and the ox cart (the ox walking, the
//! wheels turning), the wagon on its bogies, the bucket, the sled, the barge.
//! Each is blocks only (`W::bx` and the shapes), in the tile's own frame (16
//! across, `z` down the tile, `x` across) or the carrier's (built heading
//! east from its rear, `z` across, `y` up). A line's tones keep to the look's
//! ladder (docs/REALM-MONOLITH.md): the road is the brightest ground, a rail
//! brighter than its sleeper, its sleeper brighter than the ballast, so a
//! line reads as a line from the stand-back camera; a carrier is darker than
//! the road it runs on and what it carries is paler, and each good has its
//! own shape (`works_kit::load_solids`).

use std::f32::consts::{FRAC_PI_2, TAU};

use angel_stage::set::Kind;

use super::{Dir, LineKind, W};
use crate::render::overworld::ink::hash;
use crate::render::overworld::works_kit::Load;
use crate::world3d::blockout::{Shape, Solid};

pub const CABLE: f32 = 40.0;

const SAG: [f32; 4] = [0.0, 2.6, 4.2, 2.6];

pub const SPAN: u32 = 4;

fn tile_builder(frame: u32) -> W {
    W { w: 16.0, d: 16.0, seed: 3, t: frame, work: true, solids: Vec::new(), lamps: Vec::new() }
}

pub(super) fn line(kind: LineKind, links: u8, post: bool, span: u32, frame: u32) -> Vec<Solid> {
    let mut b = tile_builder(frame);
    let (n, e, s, w) = (links & 1 != 0, links & 2 != 0, links & 4 != 0, links & 8 != 0);
    let along_x = e || w || links == 0;
    let arm = |half: f32| {
        let mut v = vec![([8.0 - half, 8.0 + half], [8.0 - half, 8.0 + half])];
        if n {
            v.push(([8.0 - half, 8.0 + half], [0.0, 8.0]));
        }
        if s {
            v.push(([8.0 - half, 8.0 + half], [8.0, 16.0]));
        }
        if e {
            v.push(([8.0, 16.0], [8.0 - half, 8.0 + half]));
        }
        if w {
            v.push(([0.0, 8.0], [8.0 - half, 8.0 + half]));
        }
        v
    };
    match kind {
        LineKind::Cart => {
            for (x, z) in arm(4.6) {
                b.bx(Kind::Road, x, [0.0, 1.4], z);
            }
            for (x, z) in arm(4.6) {
                if x[1] - x[0] > z[1] - z[0] {
                    for off in [-2.4f32, 2.4] {
                        b.bx(Kind::Mud, x, [1.4, 1.8], [8.0 + off - 0.5, 8.0 + off + 0.5]);
                    }
                } else {
                    for off in [-2.4f32, 2.4] {
                        b.bx(Kind::Mud, [8.0 + off - 0.5, 8.0 + off + 0.5], [1.4, 1.8], z);
                    }
                }
            }
            for k in 0..7 {
                let v = hash(k, links as i32, 31);
                let side = (v % 4) as u8;
                let t = 1.5 + ((v >> 3) % 13) as f32;
                let (x, z) = match side {
                    0 if !n => (t, 3.0),
                    1 if !e => (13.0, t),
                    2 if !s => (t, 13.0),
                    3 if !w => (3.0, t),
                    _ => (5.0 + (v % 7) as f32, 5.0 + ((v >> 4) % 7) as f32),
                };
                let kind = if v % 3 == 0 { Kind::Hedge } else { Kind::Rock };
                b.bx(kind, [x, x + 1.4], [0.8, 2.2], [z, z + 1.4]);
            }
        }
        LineKind::Tram => {
            for (x, z) in arm(4.2) {
                b.bx(Kind::Sand, x, [0.0, 1.2], z);
            }
            let across = |b: &mut W, horizontal: bool, at: f32| {
                if horizontal {
                    b.bx(Kind::Trunk, [at, at + 1.8], [1.2, 2.2], [3.0, 13.0]);
                } else {
                    b.bx(Kind::Trunk, [3.0, 13.0], [1.2, 2.2], [at, at + 1.8]);
                }
            };
            let lone = links == 0;
            for at in [1.0f32, 5.0] {
                if lone || w {
                    across(&mut b, true, at);
                }
                if n {
                    across(&mut b, false, at);
                }
            }
            for at in [9.0f32, 13.0] {
                if lone || e {
                    across(&mut b, true, at);
                }
                if s {
                    across(&mut b, false, at);
                }
            }
            for off in [-2.6f32, 2.6] {
                let (lo, hi) = (8.0 + off - 0.5, 8.0 + off + 0.5);
                if lone {
                    b.bx(Kind::Road, [0.0, 16.0], [2.2, 3.4], [lo, hi]);
                }
                if w {
                    b.bx(Kind::Road, [0.0, 11.0], [2.2, 3.4], [lo, hi]);
                }
                if e {
                    b.bx(Kind::Road, [5.0, 16.0], [2.2, 3.4], [lo, hi]);
                }
                if n {
                    b.bx(Kind::Road, [lo, hi], [2.2, 3.4], [0.0, 11.0]);
                }
                if s {
                    b.bx(Kind::Road, [lo, hi], [2.2, 3.4], [5.0, 16.0]);
                }
            }
            if links.count_ones() == 1 {
                let (bx, bz) = match links {
                    1 => ([3.5, 12.5], [0.4, 1.8]),
                    2 => ([14.2, 15.6], [3.5, 12.5]),
                    4 => ([3.5, 12.5], [14.2, 15.6]),
                    _ => ([0.4, 1.8], [3.5, 12.5]),
                };
                b.bx(Kind::Trunk, bx, [1.0, 4.4], bz);
            }
        }
        LineKind::Chute => {
            for (x, z) in arm(4.0) {
                b.bx(Kind::Floor, x, [5.0, 6.0], z);
            }
            for (x, z) in arm(4.0) {
                if along_x {
                    b.bx(Kind::Floor, x, [6.0, 8.0], [z[0], z[0] + 1.0]);
                    b.bx(Kind::Floor, x, [6.0, 8.0], [z[1] - 1.0, z[1]]);
                } else {
                    b.bx(Kind::Floor, [x[0], x[0] + 1.0], [6.0, 8.0], z);
                    b.bx(Kind::Floor, [x[1] - 1.0, x[1]], [6.0, 8.0], z);
                }
            }
            for (lx, lz) in [(4.0f32, 4.0f32), (12.0, 12.0)] {
                b.bx(Kind::Trunk, [lx - 0.7, lx + 0.7], [0.0, 5.0], [lz - 0.7, lz + 0.7]);
                b.bx(Kind::Trunk, [lx - 2.6, lx + 2.6], [2.4, 3.2], [lz - 0.5, lz + 0.5]);
            }
        }
        LineKind::Rope => {
            let h = |k: u32| CABLE - SAG[(k % SPAN) as usize];
            let (here, before, after) = (h(span), h(span + SPAN - 1), h(span + 1));
            for (k, (x, z)) in arm(0.35).into_iter().enumerate() {
                let first = x[0] < 8.0 - 0.5 || z[0] < 8.0 - 0.5;
                let y = if k == 0 { here } else if first { (before + here) / 2.0 } else { (here + after) / 2.0 };
                b.bx(Kind::Road, x, [y - 0.5, y + 0.6], z);
            }
            if post {
                let top = CABLE;
                for (lx, lz) in [(5.2f32, 6.2f32), (10.8, 6.2), (5.2, 9.8), (10.8, 9.8)] {
                    b.bx(Kind::Path, [lx - 0.6, lx + 0.6], [1.0, top - 3.0], [lz - 0.6, lz + 0.6]);
                }
                for y in [10.0f32, 22.0] {
                    b.bx(Kind::Floor, [4.8, 11.2], [y, y + 0.9], [5.8, 6.8]);
                    b.bx(Kind::Floor, [4.8, 11.2], [y, y + 0.9], [9.2, 10.2]);
                    b.bx(Kind::Floor, [4.8, 5.8], [y, y + 0.9], [5.8, 10.2]);
                    b.bx(Kind::Floor, [10.2, 11.2], [y, y + 0.9], [5.8, 10.2]);
                }
                b.bx(Kind::Road, [3.0, 13.0], [top - 3.2, top - 1.8], [6.4, 9.6]);
                b.bx(Kind::Path, [6.0, 10.0], [top - 1.8, top - 0.2], [7.0, 9.0]);
                b.wheel(Kind::Metal, 8.0, top - 0.6, 8.0, 1.8, 0.5, 4, 1.2);
                b.bx(Kind::Stone, [3.8, 12.2], [0.0, 1.6], [4.8, 11.2]);
                b.bx(Kind::Rock, [6.8, 9.2], [top - 9.0, top - 5.5], [7.2, 8.8]);
            }
        }
        LineKind::Barge => {
            for (x, z) in arm(6.0) {
                b.bx(Kind::Water, x, [0.0, 0.4], z);
            }
            for k in 0..3 {
                let sh = ((frame + k * 3) % 8) as f32;
                b.bx(Kind::Plaster, [sh * 1.8, sh * 1.8 + 2.0], [0.4, 0.7], [4.0 + k as f32 * 3.4, 4.6 + k as f32 * 3.4]);
            }
            if post {
                b.bx(Kind::Trunk, [1.0, 2.6], [0.0, 7.0], [2.0, 3.6]);
                b.bx(Kind::Thatch, [0.6, 3.0], [5.0, 5.8], [1.6, 4.0]);
            }
        }
        LineKind::Mule => {
            for (x, z) in arm(2.8) {
                b.bx(Kind::Path, x, [0.0, 0.8], z);
            }
            for k in 0..4 {
                let side = if k % 2 == 0 { -1.0f32 } else { 1.0 };
                let u = 2.0 + k as f32 * 3.6;
                let (x, z) = if along_x { (u, 8.0 + side * 0.9) } else { (8.0 + side * 0.9, u) };
                b.bx(Kind::Mud, [x, x + 1.4], [0.8, 1.1], [z, z + 1.0]);
            }
            for k in 0..4 {
                let v = hash(k, links as i32, 47);
                let (x, z) = (1.0 + (v % 13) as f32, 1.0 + ((v >> 4) % 13) as f32);
                let off_way = (x - 8.0).abs() > 3.4 && (z - 8.0).abs() > 3.4 || (along_x && (z - 8.0).abs() > 3.4) || (!along_x && (x - 8.0).abs() > 3.4);
                if off_way {
                    b.bx(if v % 3 == 0 { Kind::Hedge } else { Kind::Rock }, [x, x + 1.2], [0.0, 1.8], [z, z + 1.2]);
                }
            }
        }
    }
    b.solids
}

fn cartwheel(b: &mut W, cx: f32, cy: f32, cz: f32, r: f32, a: f32, n: u32, spokes: bool) {
    let block = (r * 0.38).clamp(0.9, 1.5);
    for k in 0..n {
        let t = a + k as f32 / n as f32 * TAU;
        let (x, y) = (cx + t.cos() * r, cy + t.sin() * r);
        let kind = if k == 0 { Kind::Trunk } else { Kind::Floor };
        b.bx(kind, [x - block, x + block], [(y - block).max(0.0), (y + block).max(0.5)], [cz - 0.6, cz + 0.6]);
    }
    if spokes {
        for s in 0..4 {
            let t = a + s as f32 * FRAC_PI_2;
            let (x, y) = (cx + t.cos() * r * 0.5, cy + t.sin() * r * 0.5);
            b.bx(Kind::Wood, [x - 0.6, x + 0.6], [(y - 0.6).max(0.0), (y + 0.6).max(0.5)], [cz - 0.4, cz + 0.4]);
        }
    }
    b.bx(Kind::Metal, [cx - 0.9, cx + 0.9], [(cy - 0.9).max(0.0), cy + 0.9], [cz - 1.0, cz + 1.0]);
}

fn ox(b: &mut W, x0: f32, step: u32) {
    b.bx(Kind::Mud, [x0, x0 + 11.0], [4.8, 9.6], [-2.8, 2.8]);
    b.bx(Kind::Mud, [x0 + 6.5, x0 + 11.5], [9.6, 11.4], [-2.4, 2.4]);
    b.bx(Kind::Mud, [x0 + 11.0, x0 + 14.0], [6.8, 10.4], [-1.8, 1.8]);
    b.bx(Kind::Wood, [x0 + 14.0, x0 + 16.4], [6.4, 9.0], [-1.4, 1.4]);
    b.bx(Kind::Bone, [x0 + 12.4, x0 + 13.6], [10.4, 11.8], [-2.6, 2.6]);
    let swing = if (step / 2) % 2 == 0 { 1.0f32 } else { -1.0 };
    for (lx, lz, side) in [(x0 + 0.8, -2.4f32, 1.0f32), (x0 + 0.8, 1.2, -1.0), (x0 + 8.6, -2.4, -1.0), (x0 + 8.6, 1.2, 1.0)] {
        let lift = if swing * side > 0.0 { 1.3 } else { 0.0 };
        b.bx(Kind::Trunk, [lx, lx + 1.8], [lift, 4.9], [lz, lz + 1.2]);
    }
    b.bx(Kind::Trunk, [x0 - 1.4, x0 + 0.1], [4.6, 8.8], [-0.3, 0.3]);
    b.bx(Kind::Floor, [x0 + 10.6, x0 + 12.0], [10.4, 11.4], [-4.6, 4.6]);
}

fn far(b: &mut W, kind: LineKind, load: Option<Load>) {
    let step = b.t;
    match kind {
        LineKind::Cart => {
            b.bx(Kind::Wood, [0.0, 12.0], [3.2, 6.6], [-3.4, 3.4]);
            for z in [-5.2f32, 4.2] {
                b.bx(Kind::Floor, [2.0, 9.8], [0.0, 7.6], [z, z + 1.0]);
            }
            b.bx(Kind::Wood, [12.8, 19.0], [4.2, 5.2], [-0.6, 0.6]);
            b.bx(Kind::Mud, [18.0, 29.5], [4.8, 11.4], [-2.8, 2.8]);
            b.bx(Kind::Mud, [29.0, 34.4], [6.4, 10.4], [-1.8, 1.8]);
            let lift = if (step / 2) % 2 == 0 { 1.3f32 } else { 0.0 };
            b.bx(Kind::Trunk, [18.8, 20.6], [lift, 4.9], [-2.4, 2.4]);
            b.bx(Kind::Trunk, [26.6, 28.4], [1.3 - lift, 4.9], [-2.4, 2.4]);
            if let Some(load) = load {
                b.heap(load, 6.2, 0.0, 11.0, 6.6, 7.0);
            }
        }
        LineKind::Tram => {
            b.bx(Kind::Metal, [0.0, 22.0], [2.6, 4.8], [-3.6, 3.6]);
            b.bx(Kind::Wood, [0.5, 21.5], [4.8, 7.0], [-3.6, 3.6]);
            for z in [-4.6f32, 3.6] {
                b.bx(Kind::Metal, [2.0, 20.0], [0.0, 4.4], [z, z + 1.0]);
            }
            if let Some(load) = load {
                b.heap(load, 11.0, 0.0, 19.0, 7.0, 7.0);
            }
        }
        LineKind::Rope => {
            b.bx(Kind::Metal, [0.6, 7.4], [CABLE - 2.0, CABLE + 1.4], [-1.2, 1.2]);
            b.bx(Kind::Metal, [3.5, 4.5], [CABLE - 11.0, CABLE - 2.0], [-0.4, 0.4]);
            b.frus(Kind::Wood, 4.0, 0.0, 5.0, CABLE - 20.0, 9.0);
            if let Some(load) = load {
                b.heap(load, 4.0, 0.0, 7.0, CABLE - 11.0, 5.0);
            }
        }
        LineKind::Chute => {
            b.bx(Kind::Trunk, [0.0, 16.0], [0.0, 1.0], [-2.8, 2.8]);
            b.bx(Kind::Wood, [0.5, 15.5], [1.0, 3.6], [-3.0, 3.0]);
            if let Some(load) = load {
                b.heap(load, 8.0, 0.0, 13.0, 2.0, 8.0);
            }
        }
        LineKind::Barge => {
            b.bx(Kind::Wood, [4.0, 50.0], [0.0, 5.8], [-6.2, 6.2]);
            b.wedge(Kind::Wood, [50.0, 58.0], [-5.6, 5.6], 0.0, 4.2);
            b.bx(Kind::Trunk, [30.0, 31.2], [4.2, 26.0], [-0.6, 0.6]);
            b.bx(Kind::Floor, [-9.0, 3.0], [3.0, 4.0], [-0.5, 0.5]);
            if let Some(load) = load {
                b.heap(load, 40.0, 0.0, 16.0, 5.8, 9.0);
            }
        }
        LineKind::Mule => {
            b.bx(Kind::Rock, [0.0, 11.0], [5.0, 10.0], [-2.4, 2.4]);
            b.bx(Kind::Rock, [9.6, 17.4], [8.2, 12.6], [-1.6, 1.6]);
            b.bx(Kind::Rock, [12.4, 13.4], [11.6, 14.8], [-1.4, 1.4]);
            b.bx(Kind::Trunk, [-1.8, 0.1], [4.8, 9.0], [-0.3, 0.3]);
            b.bx(Kind::Floor, [2.8, 8.2], [5.6, 9.8], [-4.6, 4.6]);
            let lift = if (step / 2) % 2 == 0 { 1.0f32 } else { 0.0 };
            b.bx(Kind::Trunk, [1.0, 2.2], [lift, 5.0], [-2.0, 2.0]);
            b.bx(Kind::Trunk, [11.0, 12.2], [1.0 - lift, 5.0], [-2.0, 2.0]);
            if let Some(load) = load {
                b.heap(load, 5.5, 0.0, 8.0, 10.0, 5.0);
            }
        }
    }
}

pub(super) fn vehicle(kind: LineKind, dir: Dir, load: Option<Load>, frame: u32, near: bool) -> Vec<Solid> {
    let mut b = W { w: 16.0, d: 16.0, seed: 5, t: frame, work: true, solids: Vec::new(), lamps: Vec::new() };
    let a = b.turn();
    if !near {
        far(&mut b, kind, load);
    } else {
    match kind {
        LineKind::Cart => {
            b.bx(Kind::Floor, [0.0, 12.0], [3.2, 4.4], [-3.4, 3.4]);
            for z in [-3.6f32, 2.8] {
                b.bx(Kind::Wood, [0.0, 12.0], [4.4, 6.4], [z, z + 0.8]);
                for x in [0.6f32, 11.4] {
                    b.bx(Kind::Trunk, [x - 0.4, x + 0.4], [4.4, 7.8], [z, z + 0.8]);
                }
            }
            b.bx(Kind::Wood, [-0.8, 0.2], [4.4, 6.8], [-3.4, 3.4]);
            b.bx(Kind::Wood, [11.8, 12.8], [4.4, 7.4], [-3.4, 3.4]);
            b.bx(Kind::Metal, [5.2, 6.6], [3.2, 4.2], [-5.0, 5.0]);
            cartwheel(&mut b, 5.9, 3.8, 5.0, 3.8, a, 6, true);
            cartwheel(&mut b, 5.9, 3.8, -5.0, 3.8, a, 6, false);
            b.bx(Kind::Wood, [12.8, 19.0], [4.2, 5.2], [-0.6, 0.6]);
            ox(&mut b, 18.0, frame);
            if let Some(load) = load {
                b.heap(load, 6.2, 0.0, 11.0, 4.4, 8.0);
            }
        }
        LineKind::Tram => {
            b.bx(Kind::Metal, [0.0, 22.0], [2.6, 3.6], [-3.6, 3.6]);
            b.bx(Kind::Floor, [0.5, 21.5], [3.6, 4.8], [-3.4, 3.4]);
            for z in [-3.6f32, 2.8] {
                b.bx(Kind::Wood, [0.5, 21.5], [4.8, 7.0], [z, z + 0.8]);
                for x in [1.0f32, 21.0] {
                    b.bx(Kind::Metal, [x - 0.4, x + 0.4], [4.8, 8.0], [z, z + 0.8]);
                }
            }
            b.bx(Kind::Wood, [0.0, 0.8], [4.8, 7.4], [-3.4, 3.4]);
            b.bx(Kind::Wood, [21.2, 22.0], [4.8, 7.4], [-3.4, 3.4]);
            b.bx(Kind::Trunk, [-1.6, 0.0], [3.0, 4.2], [-2.8, 2.8]);
            b.bx(Kind::Trunk, [22.0, 23.6], [3.0, 4.2], [-2.8, 2.8]);
            for x in [4.2f32, 17.8] {
                b.bx(Kind::Metal, [x - 1.2, x + 1.2], [1.8, 3.0], [-4.6, 4.6]);
                cartwheel(&mut b, x, 2.3, 4.2, 2.3, a, 5, false);
                cartwheel(&mut b, x, 2.3, -4.2, 2.3, a, 5, false);
            }
            b.bx(Kind::Trunk, [10.0, 10.8], [4.8, 10.0], [3.8, 4.4]);
            if let Some(load) = load {
                b.heap(load, 11.0, 0.0, 19.0, 4.8, 8.5);
            }
        }
        LineKind::Rope => {
            let sway = (b.turn().sin() * 1.2).round();
            b.bx(Kind::Metal, [0.6, 7.4], [CABLE - 2.0, CABLE + 1.4], [-1.2, 1.2]);
            cartwheel(&mut b, 2.0, CABLE + 0.4, 0.0, 1.6, a, 4, false);
            cartwheel(&mut b, 6.0, CABLE + 0.4, 0.0, 1.6, a, 4, false);
            b.bx(Kind::Metal, [3.5 + sway * 0.3, 4.5 + sway * 0.3], [CABLE - 11.0, CABLE - 2.0], [-0.4, 0.4]);
            let (bx, by) = (4.0 + sway, CABLE - 20.0);
            b.frus(Kind::Wood, bx, 0.0, 5.0, by, 9.0);
            b.cyl(Kind::Metal, bx, 0.0, 5.1, by + 2.0, 0.7);
            b.cyl(Kind::Metal, bx, 0.0, 4.7, by + 6.0, 0.7);
            if let Some(load) = load {
                b.heap(load, bx, 0.0, 7.0, by + 9.0, 5.0);
            }
        }
        LineKind::Chute => {
            b.bx(Kind::Trunk, [0.0, 16.0], [0.0, 1.0], [-2.8, -1.8]);
            b.bx(Kind::Trunk, [0.0, 16.0], [0.0, 1.0], [1.8, 2.8]);
            b.bx(Kind::Floor, [0.5, 15.5], [1.0, 2.0], [-3.0, 3.0]);
            for z in [-3.0f32, 2.4] {
                b.bx(Kind::Wood, [0.5, 15.5], [2.0, 3.6], [z, z + 0.6]);
            }
            b.bx(Kind::Wood, [0.0, 0.8], [2.0, 4.6], [-3.0, 3.0]);
            b.bx(Kind::Metal, [15.6, 16.8], [2.0, 3.2], [-0.8, 0.8]);
            if let Some(load) = load {
                b.heap(load, 8.0, 0.0, 13.0, 2.0, 8.0);
            }
        }
        LineKind::Barge => {
            let bob = (b.turn().sin()).round() * 0.4;
            b.bx(Kind::Wood, [4.0, 50.0], [bob, 4.2 + bob], [-5.6, 5.6]);
            b.wedge(Kind::Wood, [50.0, 58.0], [-5.6, 5.6], bob, 4.2);
            b.wedge(Kind::Wood, [-4.0, 4.0], [-5.6, 5.6], bob, 4.2);
            for z in [-6.2f32, 5.2] {
                b.bx(Kind::Floor, [2.0, 54.0], [4.2 + bob, 5.8 + bob], [z, z + 1.0]);
            }
            b.bx(Kind::Floor, [-1.0, 0.0], [4.2 + bob, 5.6 + bob], [-5.2, 5.2]);
            b.bx(Kind::Trunk, [30.0, 31.2], [4.2 + bob, 26.0 + bob], [-0.6, 0.6]);
            b.bx(Kind::Trunk, [30.2, 31.0], [22.0 + bob, 23.0 + bob], [-6.0, 6.0]);
            b.bx(Kind::Cloth, [30.0, 31.2], [20.6 + bob, 22.0 + bob], [-5.2, 5.2]);
            b.bx(Kind::Floor, [-9.0, 3.0], [3.0 + bob, 4.0 + bob], [-0.5, 0.5]);
            b.bx(Kind::Floor, [-9.6, -8.6], [1.0 + bob, 4.6 + bob], [-1.2, 1.2]);
            b.cyl(Kind::Thatch, 6.0, 3.2, 1.6, 4.2 + bob, 1.0);
            if let Some(load) = load {
                b.heap(load, 40.0, 0.0, 16.0, 4.2 + bob, 9.0);
                b.heap(load, 18.0, 0.0, 14.0, 4.2 + bob, 7.0);
            }
            let w = (frame % 4) as f32;
            for z in [-5.0f32, 4.0] {
                b.bx(Kind::Plaster, [55.0 + w, 58.0 + w], [0.2, 0.7], [z, z + 1.0]);
            }
        }
        LineKind::Mule => {
            b.bx(Kind::Rock, [0.0, 11.0], [5.4, 10.0], [-2.4, 2.4]);
            b.bx(Kind::Rock, [9.6, 13.2], [8.2, 12.6], [-1.6, 1.6]);
            b.bx(Kind::Rock, [12.6, 16.4], [8.6, 11.6], [-1.4, 1.4]);
            b.bx(Kind::Bone, [15.6, 17.4], [8.6, 10.2], [-1.0, 1.0]);
            for z in [-1.4f32, 0.6] {
                b.bx(Kind::Rock, [12.4, 13.4], [11.6, 14.8], [z, z + 0.8]);
            }
            b.bx(Kind::Trunk, [-1.8, 0.1], [4.8, 9.0], [-0.3, 0.3]);
            let swing = if (frame / 2) % 2 == 0 { 1.0f32 } else { -1.0 };
            for (lx, lz, side) in [(0.8, -2.2f32, 1.0f32), (0.8, 1.2, -1.0), (8.6, -2.2, -1.0), (8.6, 1.2, 1.0)] {
                let lift = if swing * side > 0.0 { 1.1 } else { 0.0 };
                b.bx(Kind::Trunk, [lx, lx + 1.3], [lift, 5.5], [lz, lz + 1.0]);
            }
            b.bx(Kind::Cloth, [2.2, 8.8], [10.0, 10.8], [-3.0, 3.0]);
            for z in [-4.8f32, 2.4] {
                b.bx(Kind::Floor, [2.6, 8.4], [5.4, 10.0], [z, z + 2.4]);
                b.bx(Kind::Wood, [2.6, 8.4], [9.2, 10.0], [z, z + 2.4]);
            }
            if let Some(load) = load {
                b.heap(load, 5.5, 0.0, 8.0, 10.8, 6.0);
            }
        }
    }
    }
    let turn = match dir {
        Dir::East => 0.0,
        Dir::West => 180.0,
        Dir::North => 90.0,
        Dir::South => 270.0,
    };
    if turn != 0.0 {
        let (s, c) = (turn as f32).to_radians().sin_cos();
        for solid in &mut b.solids {
            let (x, z) = (solid.at[0], solid.at[2]);
            solid.at[0] = x * c + z * s;
            solid.at[2] = -x * s + z * c;
            let square = matches!(solid.shape, Shape::Box | Shape::Cylinder | Shape::Sphere | Shape::Cone | Shape::Frustum | Shape::Gem | Shape::Rock(_));
            if square && solid.turn == 0.0 {
                if (turn as i32 / 90) % 2 == 1 {
                    solid.size.swap(0, 2);
                }
            } else {
                solid.turn = (solid.turn + turn).rem_euclid(360.0);
            }
        }
    }
    b.solids
}
