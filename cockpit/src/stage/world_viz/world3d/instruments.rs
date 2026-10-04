//! Static instruments and lanterns for the research rooms and realm.
//! Broad silhouettes survive the ordinary terminal's dot resolution. Brass
//! and parchment reflect existing light; steady lamps are scenery, not status.
use super::arch::prim;
use super::math::{V3, v3};
use super::mesh::{Mesh, mat};
use std::f32::consts::TAU;

/// The tower's scrying glass: a solid, faceted lens held by two brass rings.
/// This is steady room furniture, not a busy indicator or simulated asset.
pub(super) fn scrying_globe(mesh: &mut Mesh, centre: V3) {
    let (x, y) = (centre.x, centre.y);
    prim::prism(mesh, x, y, 0.0, 0.18, 0.82, 0.72, 8, 0.0, mat::STONE_DARK);
    prim::prism(
        mesh,
        x,
        y,
        0.18,
        centre.z - 0.84,
        0.34,
        0.27,
        8,
        0.0,
        mat::STONE,
    );
    prim::prism(
        mesh,
        x,
        y,
        centre.z - 0.84,
        centre.z - 0.68,
        0.52,
        0.48,
        8,
        0.0,
        mat::BRASS,
    );

    let radius = 0.76;
    let point = |latitude: f32, longitude: f32| {
        centre
            + v3(
                radius * latitude.cos() * longitude.cos(),
                radius * latitude.cos() * longitude.sin(),
                radius * latitude.sin(),
            )
    };
    // Ten sides and five latitude bands give a round silhouette even in the
    // narrow terminal pane, without buying a dense sphere for every frame.
    for band in 0..5 {
        let a = -std::f32::consts::FRAC_PI_2 + band as f32 * std::f32::consts::PI / 5.0;
        let b = a + std::f32::consts::PI / 5.0;
        for side in 0..10 {
            let c = side as f32 * TAU / 10.0;
            let d = (side + 1) as f32 * TAU / 10.0;
            prim::quad(
                mesh,
                point(a, c),
                point(a, d),
                point(b, d),
                point(b, c),
                mat::CRYSTAL,
            );
        }
    }
    ring(mesh, centre, v3(0.0, 1.0, 0.0), V3::UP, 0.98, 0.09);
    ring(
        mesh,
        centre,
        v3(1.0, 0.0, 0.0),
        v3(0.0, 0.87, 0.50),
        1.02,
        0.08,
    );
}

pub(super) fn telescope(mesh: &mut Mesh, base: V3, muzzle: V3) {
    let axis = (muzzle - base).normalize();
    tube(mesh, base, muzzle, 0.23, 0.34, 8, mat::WOOD);
    for (fraction, radius) in [(0.12, 0.27), (0.58, 0.33), (0.94, 0.39)] {
        let centre = base + (muzzle - base) * fraction;
        tube(
            mesh,
            centre - axis * 0.09,
            centre + axis * 0.09,
            radius,
            radius,
            8,
            mat::BRASS,
        );
    }
    // The objective is an actual dark opening, bounded by a broad brass lip.
    let right = axis.cross(V3::UP).normalize();
    let up = axis.cross(right).normalize();
    prim::poly_fan(
        mesh,
        muzzle + axis * 0.025,
        right,
        up,
        0.29,
        8,
        0.0,
        mat::STONE_DARK,
    );
    tube(mesh, base - axis * 0.52, base, 0.085, 0.13, 6, mat::BRASS);
}

pub(super) fn armillary(mesh: &mut Mesh, centre: V3) {
    ring(mesh, centre, v3(1.0, 0.0, 0.0), V3::UP, 0.48, 0.06);
    ring(mesh, centre, v3(0.0, 1.0, 0.0), V3::UP, 0.48, 0.06);
    ring(
        mesh,
        centre,
        v3(1.0, 0.0, 0.0),
        v3(0.0, 0.80, 0.60),
        0.48,
        0.06,
    );
    prim::prism(
        mesh,
        centre.x,
        centre.y,
        centre.z - 0.13,
        centre.z + 0.13,
        0.14,
        0.10,
        6,
        0.0,
        mat::BRASS,
    );
}

fn ring(mesh: &mut Mesh, centre: V3, right: V3, up: V3, radius: f32, width: f32) {
    let at = |r: f32, theta: f32| centre + right * (r * theta.cos()) + up * (r * theta.sin());
    for i in 0..12 {
        let a = TAU * i as f32 / 12.0;
        let b = TAU * (i + 1) as f32 / 12.0;
        prim::quad(
            mesh,
            at(radius - width, a),
            at(radius, a),
            at(radius, b),
            at(radius - width, b),
            mat::BRASS,
        );
    }
}

pub(super) fn council_map(mesh: &mut Mesh) {
    // A parchment planning board, with a brass bezel and an abstract island.
    // It is room dressing; actual reports and images occupy the foreground Stage.
    prim::disc(mesh, 0.0, 0.0, 0.925, 1.38, 12, 0.0, mat::PARCHMENT);
    prim::annulus(mesh, 0.0, 0.0, 0.935, 1.30, 1.47, 12, 0.0, mat::BRASS);
    prim::quad(
        mesh,
        v3(-0.80, -0.12, 0.94),
        v3(-0.18, -0.62, 0.94),
        v3(0.48, -0.29, 0.94),
        v3(0.73, 0.35, 0.94),
        mat::FOLIAGE,
    );
    prim::quad(
        mesh,
        v3(-0.80, -0.12, 0.94),
        v3(0.73, 0.35, 0.94),
        v3(0.24, 0.69, 0.94),
        v3(-0.38, 0.47, 0.94),
        mat::FOLIAGE,
    );
    for (x, y) in [(-0.30, 0.15), (0.22, -0.14), (0.37, 0.35)] {
        prim::prism(mesh, x, y, 0.94, 1.05, 0.055, 0.045, 5, 0.0, mat::BRASS);
    }
}

/// A low survey dial on the observatory lawn. Its four brass points and
/// triangular gnomon are reflected scenery, never live progress or a clock.
pub(super) fn survey_dial(mesh: &mut Mesh) {
    prim::prism(
        mesh,
        0.0,
        0.0,
        0.0,
        0.18,
        0.72,
        0.62,
        8,
        0.0,
        mat::STONE_DARK,
    );
    prim::prism(mesh, 0.0, 0.0, 0.18, 0.92, 0.28, 0.22, 6, 0.0, mat::STONE);
    prim::disc(mesh, 0.0, 0.0, 0.94, 0.86, 8, 0.0, mat::PARCHMENT);
    prim::annulus(mesh, 0.0, 0.0, 0.95, 0.76, 0.91, 8, 0.0, mat::BRASS);
    for i in 0..4 {
        let angle = i as f32 * TAU / 4.0;
        let axis = v3(angle.cos(), angle.sin(), 0.0);
        let side = v3(-angle.sin(), angle.cos(), 0.0);
        let centre = v3(0.0, 0.0, 0.96);
        prim::tri(
            mesh,
            [
                centre + axis * 0.72,
                centre + side * 0.15,
                centre - side * 0.15,
            ],
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            mat::BRASS,
        );
    }
    prim::tri(
        mesh,
        [
            v3(0.0, -0.35, 0.97),
            v3(0.0, 0.35, 0.97),
            v3(0.0, 0.35, 1.46),
        ],
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        mat::BRASS,
    );
}

/// A hooded bridge lantern. Steady warm glazing shares the outdoor windows'
/// lighting; it neither flickers nor schedules another world frame.
pub(super) fn bridge_lantern(mesh: &mut Mesh) {
    prim::boxed(
        mesh,
        v3(-0.07, -0.07, 0.0),
        v3(0.07, 0.07, 0.72),
        mat::WOOD,
        prim::F_SIDES_TOP,
    );
    prim::boxed(
        mesh,
        v3(-0.23, -0.23, 0.72),
        v3(0.23, 0.23, 1.18),
        mat::WINDOW,
        prim::F_SIDES,
    );
    prim::boxed(
        mesh,
        v3(-0.30, -0.30, 1.18),
        v3(0.30, 0.30, 1.30),
        mat::BRASS,
        prim::F_SIDES_TOP,
    );
}

/// Tapered tube between two arbitrary points — the one primitive the
/// architecture library has no equivalent for, because every exterior mass it
/// builds is axis-aligned and a telescope is not.
pub(super) fn tube(
    mesh: &mut Mesh,
    from: V3,
    to: V3,
    r0: f32,
    r1: f32,
    sides: usize,
    material: u8,
) {
    let axis = (to - from).normalize();
    if axis.length() < 0.5 {
        return;
    }
    let helper = if axis.z.abs() > 0.9 {
        v3(1.0, 0.0, 0.0)
    } else {
        V3::UP
    };
    let u = axis.cross(helper).normalize();
    let v = axis.cross(u).normalize();
    let sides = sides.max(3);
    let ring =
        |centre: V3, r: f32, angle: f32| centre + u * (r * angle.cos()) + v * (r * angle.sin());
    for i in 0..sides {
        let a0 = TAU * i as f32 / sides as f32;
        let a1 = TAU * (i + 1) as f32 / sides as f32;
        prim::quad(
            mesh,
            ring(from, r0, a0),
            ring(from, r0, a1),
            ring(to, r1, a1),
            ring(to, r1, a0),
            material,
        );
    }
    for i in 0..sides {
        let a0 = TAU * i as f32 / sides as f32;
        let a1 = TAU * (i + 1) as f32 / sides as f32;
        prim::tri(
            mesh,
            [to, ring(to, r1, a0), ring(to, r1, a1)],
            [[0.0, 0.0], [r1, 0.0], [r1, r1]],
            material,
        );
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/world3d__instruments__tests.rs"]
mod tests;
