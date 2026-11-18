//! The carter's yard, the castle's stockyard and the founding camp as
//! solids of the monolith kit, each the same place as its flat piece
//! (`render::overworld::works_kit::yards`). Front toward `+z`.

use angel_stage::set::Kind;

use super::W;
use crate::render::overworld::works_kit::Load;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stance {
    Stand,
    Graze,
    Lie,
}

fn ox(b: &mut W, hide: Kind, cx: f32, z0: f32, stance: Stance, tail: f32) {
    let lie = stance == Stance::Lie;
    let (y0, y1) = if lie { (0.4, 5.4) } else { (3.8, 9.8) };
    b.bx(hide, [cx - 2.8, cx + 2.8], [y0, y1], [z0, z0 + 8.0]);
    b.bx(hide, [cx - 2.0, cx + 2.0], [y1, y1 + 1.5], [z0 + 2.5, z0 + 6.0]);
    if !lie {
        for (lx, lz) in [(-2.6, 0.5), (1.5, 0.5), (-2.6, 6.3), (1.5, 6.3)] {
            b.bx(Kind::Trunk, [cx + lx, cx + lx + 1.1], [0.0, 3.8], [z0 + lz, z0 + lz + 1.2]);
        }
    }
    let (hy0, hy1) = match stance {
        Stance::Graze => (2.4, 6.0),
        Stance::Lie => (3.6, 7.4),
        Stance::Stand => (6.0, 9.6),
    };
    b.bx(hide, [cx - 1.5, cx + 1.5], [hy0, hy1], [z0 + 7.5, z0 + 10.8]);
    b.bx(Kind::Bone, [cx - 1.0, cx + 1.0], [hy0, hy0 + 2.0], [z0 + 10.8, z0 + 11.8]);
    for side in [-1.0f32, 1.0] {
        let hx = cx + side * 2.0;
        b.bx(Kind::Bone, [hx - 0.5, hx + 0.5], [hy1 - 0.4, hy1 + 0.5], [z0 + 8.0, z0 + 9.2]);
        b.bx(Kind::Bone, [hx + side * 0.8 - 0.4, hx + side * 0.8 + 0.4], [hy1 + 0.5, hy1 + 1.8], [z0 + 8.2, z0 + 9.0]);
    }
    b.bx(Kind::Trunk, [cx - 0.4 + tail, cx + 0.4 + tail], [if lie { 0.6 } else { 3.0 }, y1 - 0.6], [z0 - 1.0, z0]);
}

fn cart(b: &mut W, x0: f32, z: f32, loaded: bool) {
    b.bx(Kind::Floor, [x0, x0 + 18.0], [4.6, 6.2], [z, z + 7.0]);
    for zz in [z, z + 6.3] {
        b.bx(Kind::Floor, [x0, x0 + 18.0], [6.2, 10.0], [zz, zz + 0.7]);
    }
    b.bx(Kind::Floor, [x0, x0 + 0.8], [6.2, 10.0], [z, z + 7.0]);
    for zz in [z - 1.2, z + 7.0] {
        b.wheel(Kind::Floor, x0 + 8.0, 4.6, zz + 0.6, 4.6, 0.3, 8, 1.2);
    }
    b.bx(Kind::Floor, [x0 + 17.0, x0 + 30.0], [5.0, 6.0], [z + 3.0, z + 4.0]);
    b.bx(Kind::Floor, [x0 + 28.0, x0 + 29.2], [0.0, 5.0], [z + 2.8, z + 4.2]);
    if loaded {
        b.heap(Load::Crate, x0 + 8.0, z + 3.5, 9.0, 6.2, 5.0);
    }
}

pub(super) fn carters_yard(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let (t, work) = (b.t, b.work);
    b.bx(Kind::Path, [2.0, w - 2.0], [0.0, 0.4], [3.0, d - 3.0]);
    b.bx(Kind::Floor, [1.0, w - 1.0], [0.0, 15.0], [3.0, 4.6]);
    for x in [1.0, 17.0, 33.0, w - 3.0] {
        b.bx(Kind::Floor, [x, x + 2.0], [0.0, 15.0], [4.4, 6.0]);
        b.bx(Kind::Floor, [x, x + 2.0], [0.0, 13.5], [9.0, 10.8]);
        b.bx(Kind::Floor, [x, x + 1.2], [0.0, 7.0], [6.0, 15.0]);
    }
    b.bx(Kind::Floor, [0.0, w], [13.5, 15.0], [9.0, 10.8]);
    b.wedge(Kind::Roof, [0.0, w], [2.0, 11.4], 15.0, 5.0);
    b.bx(Kind::Thatch, [3.0, 16.5], [0.4, 0.9], [6.0, 13.5]);
    b.bx(Kind::Thatch, [19.0, 32.5], [0.4, 0.9], [6.0, 13.5]);
    let swing = |k: u32| if work && (t + k) / 2 % 2 == 1 { 1.4 } else { 0.0 };
    let (a, c) = (swing(0), swing(2));
    if work {
        ox(b, Kind::Trunk, 10.0, 6.5, Stance::Stand, a);
        ox(b, Kind::Sand, 26.0, 6.5, Stance::Stand, c);
    } else {
        ox(b, Kind::Trunk, 10.0, 6.5, Stance::Lie, 0.0);
        ox(b, Kind::Sand, 26.0, 6.5, Stance::Graze, 0.0);
    }
    for x in [5.0, 21.0] {
        b.bx(Kind::Thatch, [x, x + 10.0], [10.5, 13.5], [4.6, 5.8]);
    }
    b.bx(Kind::Door, [34.5, w - 3.5], [6.0, 12.0], [4.6, 5.2]);
    b.wheel(Kind::Trunk, 38.0, 8.5, 6.0, 2.6, 0.0, 0, 1.0);
    b.bx(Kind::Floor, [41.0, 46.0], [9.0, 10.4], [5.4, 6.6]);
    for x in [41.8, 44.8] {
        b.bx(Kind::Floor, [x, x + 0.8], [5.0, 9.0], [5.6, 6.4]);
    }
    let sway = if work && (t / 2) % 2 == 0 { 0.8 } else { 0.0 };
    b.bx(Kind::Metal, [36.0, 36.6], [12.8, 13.6], [10.4, 11.0]);
    b.bx(Kind::Metal, [44.0, 44.6], [12.8, 13.6], [10.4, 11.0]);
    b.bx(Kind::Plaster, [35.0 + sway, 46.0 + sway], [8.0, 12.8], [10.8, 11.8]);
    b.wheel(Kind::Floor, 40.5 + sway, 10.4, 12.2, 1.6, 0.0, 6, 0.4);
    b.bx(Kind::Metal, [14.7, 15.1], [12.6, 13.6], [10.2, 10.6]);
    b.bx(Kind::Metal, [13.6, 16.2], [9.0, 9.6], [9.4, 11.4]);
    b.bx(if work { Kind::Window } else { Kind::Door }, [13.8, 16.0], [9.6, 12.4], [9.6, 11.2]);
    if work {
        b.lamp([15.0, 10.5, 12.5], 24.0, 0.5);
    }
    cart(b, 3.0, 20.0, work && (t / 4) % 2 == 1);
    b.bx(Kind::Floor, [34.0, w - 1.0], [0.0, 1.4], [19.0, 21.0]);
    b.wheel(Kind::Floor, 38.5, 5.4, 20.0, 5.2, 0.5, 8, 1.2);
    b.wheel(Kind::Floor, 44.0, 5.0, 18.6, 5.0, 0.0, 8, 1.2);
    b.bx(Kind::Floor, [34.0, w - 1.5], [0.0, 3.0], [24.0, 28.0]);
    b.bx(Kind::Water, [35.0, w - 2.5], [2.4, 3.1], [24.8, 27.2]);
}

fn lift(b: &W) -> f32 {
    if b.work { ((b.turn().cos() + 1.0) * 5.0).round() } else { 8.0 }
}

pub(super) fn castle_yard(b: &mut W) {
    let (w, d) = (b.w, b.d);
    b.fence(Kind::Floor, [2.0, 17.0], 4.0, 7.0);
    b.fence(Kind::Floor, [30.0, w - 2.0], 4.0, 7.0);
    for x in [17.0, 29.0] {
        b.bx(Kind::Floor, [x, x + 1.6], [0.0, 10.0], [3.2, 4.8]);
    }
    b.bx(Kind::Floor, [20.0, 20.8], [0.0, 6.6], [4.0, 10.0]);
    b.bx(Kind::Floor, [20.0, 20.8], [2.0, 2.8], [4.0, 10.0]);
    b.bx(Kind::Stone, [0.5, 5.0], [0.0, 2.0], [4.5, 8.0]);
    b.bx(Kind::Floor, [1.6, 2.8], [0.0, 37.0], [5.4, 6.6]);
    for k in 0..6 {
        let wave = if b.work { (b.turn() - k as f32 * 0.8).sin() * 0.9 } else { 0.0 };
        b.bx(Kind::Cloth, [2.8 + k as f32 * 1.5, 4.3 + k as f32 * 1.5], [29.0 + wave, 35.0 + wave - (k as f32 * 0.45)], [5.7, 6.3]);
    }
    for l in 0..5 {
        let inset = if l % 2 == 1 { 0.4 } else { 0.0 };
        let kind = if l == 4 { Kind::Plaster } else { Kind::Stone };
        b.bx(kind, [5.0 + inset, 19.0 - inset], [l as f32 * 2.6, l as f32 * 2.6 + 2.6], [8.0, 15.0]);
    }
    let (mx, mz) = (41.0, 13.0);
    b.bx(Kind::Floor, [mx - 6.0, mx + 7.0], [0.0, 1.6], [mz - 1.0, mz + 3.4]);
    b.bx(Kind::Floor, [mx, mx + 2.4], [1.6, 42.0], [mz, mz + 2.4]);
    for k in 1..6 {
        let f = k as f32 / 6.0;
        b.bx(Kind::Floor, [mx - 6.0 + f * 5.0, mx - 4.8 + f * 5.0], [1.6 + f * 9.0, 2.8 + f * 9.0], [mz + 0.4, mz + 2.0]);
    }
    b.bx(Kind::Floor, [24.0, mx + 2.4], [40.0, 42.0], [mz + 0.2, mz + 2.2]);
    for k in 1..8 {
        let f = k as f32 / 8.0;
        b.bx(Kind::Floor, [mx - f * 11.0, mx - f * 11.0 + 1.4], [26.0 + f * 14.0, 27.4 + f * 14.0], [mz + 0.6, mz + 1.8]);
    }
    let by = 18.0 + lift(b);
    b.bx(Kind::Cloth, [24.4, 25.0], [by + 6.0, 40.0], [mz + 0.9, mz + 1.5]);
    b.bx(Kind::Plaster, [21.5, 28.0], [by, by + 5.5], [mz - 1.2, mz + 3.4]);
    b.bx(Kind::Cloth, [22.2, 27.4], [by + 5.5, by + 6.2], [mz + 0.7, mz + 1.7]);
    b.bx(Kind::Floor, [20.0, 36.0], [1.8, 3.0], [mz - 3.0, mz + 5.6]);
    for x in [21.5, 27.0, 32.5] {
        b.bx(Kind::Trunk, [x, x + 1.6], [0.0, 1.8], [mz - 3.6, mz + 6.2]);
    }
    b.bx(Kind::Stone, [23.0, 33.0], [3.0, 8.0], [mz - 2.0, mz + 4.0]);
    for x in [35.0, w - 4.0] {
        b.bx(Kind::Trunk, [x, x + 2.0], [0.0, 1.4], [d - 14.0, d - 6.0]);
    }
    for l in 0..3 {
        let inset = if l == 1 { 1.0 } else { 0.0 };
        b.bx(Kind::Floor, [34.0 + inset, w - 1.0], [1.4 + l as f32 * 3.0, 4.2 + l as f32 * 3.0], [d - 14.0, d - 6.0]);
    }
    for r in 0..4 {
        let (a, c) = if r % 2 == 0 { (6.0, 3.0) } else { (3.0, 6.0) };
        b.bx(Kind::Plaster, [2.0, 2.0 + a], [r as f32 * 2.4, r as f32 * 2.4 + 2.4], [d - 12.0, d - 8.0]);
        b.bx(Kind::Plaster, [2.0 + a, 2.0 + a + c], [r as f32 * 2.4, r as f32 * 2.4 + 2.4], [d - 12.0, d - 8.0]);
    }
    for x in [14.0, 26.0] {
        b.bx(Kind::Floor, [x, x + 1.0], [0.0, 10.0], [d - 11.0, d - 10.0]);
    }
    b.bx(Kind::Door, [13.0, 27.0], [6.0, 14.0], [d - 11.4, d - 10.4]);
    for k in 0..4 {
        b.bx(Kind::Bone, [15.0 + k as f32 * 2.8, 15.5 + k as f32 * 2.8], [8.0, 12.5], [d - 10.2, d - 9.8]);
    }
    b.cyl(Kind::Floor, 31.0, d - 9.0, 3.4, 0.0, 5.6);
    b.cyl(Kind::Path, 31.0, d - 9.0, 2.9, 5.6, 0.4);
    b.cyl(Kind::Metal, 31.0, d - 9.0, 3.5, 2.0, 0.6);
    b.bx(Kind::Floor, [33.0, 33.6], [5.0, 12.0], [d - 9.6, d - 9.0]);
    b.bx(Kind::Metal, [32.2, 34.4], [11.6, 12.2], [d - 9.8, d - 8.8]);
}

pub(super) fn camp(b: &mut W) {
    let (w, d) = (b.w, b.d);
    let _ = d;
    b.bx(Kind::Path, [0.5, w - 0.5], [0.0, 0.4], [1.0, 15.0]);
    for k in 0..6 {
        let x = 1.5 + k as f32 * 2.2;
        b.bx(if k % 2 == 0 { Kind::Plaster } else { Kind::Cloth }, [x, x + 2.2], [0.0, 4.6], [2.0, 10.0]);
    }
    for (k, inset) in [0.0f32, 1.6, 3.2, 4.8].into_iter().enumerate() {
        let (y0, y1) = (4.6 + k as f32 * 3.4, 8.0 + k as f32 * 3.4);
        let kind = if k % 2 == 0 { Kind::Cloth } else { Kind::Plaster };
        b.bx(kind, [0.8 + inset, 14.8 - inset], [y0, y1], [1.4 + inset * 0.6, 10.6 - inset * 0.6]);
    }
    b.bx(Kind::Door, [6.2, 9.8], [0.0, 4.2], [9.8, 10.4]);
    b.bx(Kind::Floor, [7.4, 8.4], [17.0, 27.0], [5.4, 6.4]);
    for k in 0..4 {
        let wave = if b.work { (b.turn() - k as f32 * 0.9).sin() * 0.7 } else { 0.0 };
        b.bx(Kind::Cloth, [8.4 + k as f32 * 1.5, 9.9 + k as f32 * 1.5], [22.0 + wave, 26.0 + wave - k as f32 * 0.7], [5.7, 6.1]);
    }
    b.bx(Kind::Floor, [1.0, 4.4], [0.0, 3.4], [11.6, 15.0]);
    b.ball(Kind::Plaster, 6.6, 2.4, 12.8, 1.8, 2.4, 1.6);
    let (fx, fz) = (11.2, 12.8);
    for k in 0..6 {
        let a = k as f32 / 6.0 * std::f32::consts::TAU;
        b.bx(Kind::Rock, [fx + a.cos() * 2.6 - 0.7, fx + a.cos() * 2.6 + 0.7], [0.0, 1.2], [fz + a.sin() * 2.2 - 0.7, fz + a.sin() * 2.2 + 0.7]);
    }
    for (dx, dz) in [(-2.2, -1.8), (2.2, -1.8), (0.0, 2.4)] {
        b.bx(Kind::Floor, [fx + dx - 0.35, fx + dx + 0.35], [0.0, 3.6], [fz + dz - 0.35, fz + dz + 0.35]);
        b.bx(Kind::Floor, [fx + dx * 0.5 - 0.35, fx + dx * 0.5 + 0.35], [3.6, 7.6], [fz + dz * 0.5 - 0.35, fz + dz * 0.5 + 0.35]);
    }
    b.cyl(Kind::Metal, fx, fz, 1.9, 3.0, 2.8);
    b.cyl(Kind::Rock, fx, fz, 1.6, 5.6, 0.3);
    if b.work {
        b.fire(fx, 0.4, fz, 2.6);
        b.smoke(fx, 8.0, fz, 8.0, 3);
    } else {
        b.bx(Kind::Brick, [fx - 1.0, fx + 1.0], [0.3, 0.9], [fz - 0.8, fz + 0.8]);
    }
}

fn shaft_mouth(b: &mut W) {
    b.bx(Kind::Path, [0.5, 15.5], [0.0, 0.4], [0.5, 15.5]);
    b.bx(Kind::Stone, [2.0, 14.0], [0.0, 2.2], [9.0, 12.0]);
    b.bx(Kind::Stone, [2.0, 14.0], [0.0, 2.2], [3.0, 6.0]);
    b.bx(Kind::Stone, [2.0, 5.0], [0.0, 2.2], [6.0, 9.0]);
    b.bx(Kind::Stone, [11.0, 14.0], [0.0, 2.2], [6.0, 9.0]);
    b.bx(Kind::Door, [5.0, 11.0], [0.0, 0.6], [6.0, 9.0]);
}

fn lean(b: &mut W, (x0, z0): (f32, f32), (x1, z1): (f32, f32), (y0, y1): (f32, f32)) {
    let steps = 5;
    for k in 0..steps {
        let (f, g) = (k as f32 / steps as f32, (k + 1) as f32 / steps as f32);
        let (x, z) = (x0 + (x1 - x0) * f, z0 + (z1 - z0) * f);
        b.bx(Kind::Floor, [x - 0.6, x + 0.6], [y0 + (y1 - y0) * f, y0 + (y1 - y0) * g + 0.3], [z - 0.6, z + 0.6]);
    }
}

fn bucket_y(b: &W, lo: f32, hi: f32) -> f32 {
    if b.work { lo + (((b.turn() + 0.9).cos() + 1.0) / 2.0 * (hi - lo)).round() } else { hi }
}

pub(super) fn shaft_hand(b: &mut W) {
    shaft_mouth(b);
    lean(b, (2.0, 13.0), (8.0, 8.0), (0.0, 21.0));
    lean(b, (14.0, 13.0), (8.0, 8.0), (0.0, 21.0));
    lean(b, (8.0, 2.0), (8.0, 8.0), (0.0, 21.0));
    b.bx(Kind::Metal, [6.8, 9.2], [20.0, 23.0], [6.8, 9.2]);
    let by = bucket_y(b, 3.0, 8.0);
    b.bx(Kind::Cloth, [7.7, 8.3], [by + 3.0, 20.0], [7.7, 8.3]);
    b.cyl(Kind::Floor, 8.0, 7.6, 1.8, by, 3.0);
    for x in [0.8, 12.0] {
        b.bx(Kind::Floor, [x, x + 3.2], [0.0, 2.2], [12.5, 15.0]);
    }
}

pub(super) fn shaft_windlass(b: &mut W) {
    shaft_mouth(b);
    for x in [2.0, 13.0] {
        b.bx(Kind::Floor, [x, x + 1.4], [0.0, 17.0], [6.5, 8.0]);
    }
    b.bx(Kind::Floor, [1.0, 15.4], [17.0, 18.6], [6.0, 8.5]);
    b.bx(Kind::Floor, [3.4, 13.0], [9.0, 13.0], [6.6, 7.8]);
    let a = if b.work { b.turn() } else { 0.6 };
    b.bx(Kind::Floor, [14.0 + a.cos() * 0.8, 15.4 + a.cos() * 0.8], [11.0 + a.sin() * 2.5, 12.4 + a.sin() * 2.5], [6.8, 7.8]);
    let by = bucket_y(b, 3.0, 8.0);
    b.bx(Kind::Cloth, [7.7, 8.3], [by + 3.0, 9.0], [7.0, 7.6]);
    b.cyl(Kind::Floor, 8.0, 7.3, 1.8, by, 3.0);
}

pub(super) fn shaft_gin(b: &mut W) {
    shaft_mouth(b);
    let (cx, cz) = (8.0, 10.0);
    b.cyl(Kind::Path, cx, cz, 7.0, 0.0, 0.3);
    b.cyl(Kind::Floor, cx, cz, 1.2, 0.3, 11.0);
    b.cone(Kind::Thatch, cx, cz, 3.0, 11.0, 4.0);
    let a = if b.work { b.turn() } else { 2.4 };
    for k in 1..7 {
        let f = k as f32 * 1.0;
        b.bx(Kind::Floor, [cx + a.cos() * f - 0.5, cx + a.cos() * f + 0.5], [6.0, 7.2], [cz + a.sin() * f * 0.8 - 0.5, cz + a.sin() * f * 0.8 + 0.5]);
    }
    let (hx, hz) = (cx + a.cos() * 6.5, cz + a.sin() * 5.2);
    b.bx(Kind::Floor, [hx - 2.5, hx + 2.5], [3.0, 6.5], [hz - 1.3, hz + 1.3]);
    b.bx(Kind::Floor, [hx + 1.8, hx + 3.6], [5.0, 8.0], [hz - 0.7, hz + 0.7]);
    for (lx, lz) in [(-2.2, -1.0), (1.4, -1.0), (-2.2, 0.4), (1.4, 0.4)] {
        b.bx(Kind::Trunk, [hx + lx, hx + lx + 0.8], [0.0, 3.0], [hz + lz, hz + lz + 0.8]);
    }
    b.bx(Kind::Floor, [2.0, 3.2], [0.0, 12.0], [2.0, 3.2]);
    b.bx(Kind::Floor, [6.0, 7.2], [0.0, 12.0], [2.0, 3.2]);
    b.bx(Kind::Floor, [1.6, 7.6], [12.0, 13.4], [1.8, 3.4]);
    let by = bucket_y(b, 4.0, 8.0);
    b.bx(Kind::Cloth, [4.4, 5.0], [by + 3.0, 12.0], [2.4, 3.0]);
    b.cyl(Kind::Floor, 4.7, 2.7, 1.5, by, 3.0);
}

pub(super) fn shaft_water(b: &mut W) {
    shaft_mouth(b);
    for x in [1.0, 6.0] {
        b.bx(Kind::Floor, [x, x + 1.4], [0.0, 14.0], [5.0, 6.4]);
    }
    b.bx(Kind::Floor, [0.4, 8.0], [14.0, 15.6], [4.6, 6.8]);
    b.bx(Kind::Floor, [2.0, 6.4], [8.0, 11.5], [5.2, 6.4]);
    let by = bucket_y(b, 3.0, 8.0);
    b.bx(Kind::Cloth, [4.0, 4.6], [by + 3.0, 8.0], [5.5, 6.0]);
    b.cyl(Kind::Floor, 4.3, 5.7, 1.5, by, 3.0);
    let a = if b.work { b.turn() / 4.0 } else { 0.4 };
    b.wheel(Kind::Floor, 11.5, 6.4, 6.0, 5.0, a, 8, 1.8);
    b.bx(Kind::Floor, [7.0, 11.0], [10.0, 10.8], [5.6, 6.6]);
    b.bx(Kind::Floor, [9.0, 15.4], [13.0, 14.2], [5.2, 7.2]);
    b.bx(Kind::Water, [9.4, 15.0], [14.0, 14.5], [5.8, 6.6]);
    if b.work {
        b.bx(Kind::Water, [14.4, 15.2], [3.0 + (b.t % 2) as f32, 13.4], [5.8, 6.6]);
    }
    b.bx(Kind::Water, [9.0, 15.4], [0.0, 0.8], [9.0, 13.0]);
}

pub(super) fn shaft_ley(b: &mut W) {
    shaft_mouth(b);
    for (k, x) in [(0u32, 1.0f32), (1, 12.0)] {
        b.bx(Kind::Stone, [x, x + 3.0], [2.2, 17.0], [6.0, 9.0]);
        let lit = b.work && (b.t / 2 + k) % 2 == 0;
        b.bx(if b.work { Kind::Window } else { Kind::Slate }, [x + 0.3, x + 2.7], [17.0, 22.0], [6.3, 8.7]);
        let _ = lit;
    }
    if b.work {
        for k in 0..6 {
            let y = 2.0 + k as f32 * 4.0;
            b.bx(Kind::Window, [7.2, 8.8], [y, y + 3.0], [7.2, 8.8]);
        }
        let by = 3.0 + ((b.t * 3) % 22) as f32;
        b.bx(Kind::Plaster, [5.6, 10.4], [by, by + 3.5], [6.2, 9.8]);
        b.lamp([8.0, 14.0, 8.0], 34.0, 0.9);
    } else {
        b.bx(Kind::Plaster, [5.6, 10.4], [2.2, 5.5], [6.2, 9.8]);
    }
}
