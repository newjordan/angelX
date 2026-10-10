//! Authored, quiet, seed-free practice district. Large profiles and broad local
//! lantern-lit surfaces survive the normal Dotmax grade; no alternate renderer.
use super::arch::{self, prim};
use super::math::{V3, v3};
use super::mesh::{Mesh, mat};
use super::raster::{self, View3};
use crate::drive::chivalry::{Chivalry, Mount, Phase, Place, Visit};

fn cube(m: &mut Mesh, lo: V3, hi: V3, material: u8) {
    prim::boxed(m, lo, hi, material, prim::F_ALL);
}
fn beam(m: &mut Mesh, a: V3, b: V3, radius: f32, material: u8) {
    // A quiet six-sided prism through the same triangle mesh path.
    let axis = (b - a).normalize();
    let reference = if axis.z.abs() < 0.9 {
        v3(0.0, 0.0, 1.0)
    } else {
        v3(0.0, 1.0, 0.0)
    };
    let u = axis.cross(reference).normalize() * radius;
    let v = axis.cross(u).normalize() * radius;
    for i in 0..6 {
        let t0 = std::f32::consts::TAU * i as f32 / 6.0;
        let t1 = std::f32::consts::TAU * (i + 1) as f32 / 6.0;
        let r0 = u * t0.cos() + v * t0.sin();
        let r1 = u * t1.cos() + v * t1.sin();
        prim::quad(m, a + r0, a + r1, b + r1, b + r0, material);
    }
}
fn rail(m: &mut Mesh, a: V3, b: V3) {
    // A rail must be a continuous multi-dot stroke, not isolated dark posts.
    beam(m, a, b, 0.12, mat::STABLE_TIMBER);
}
fn post(m: &mut Mesh, x: f32, y: f32, height: f32) {
    cube(
        m,
        v3(x - 0.13, y - 0.13, 0.0),
        v3(x + 0.13, y + 0.13, height),
        mat::STABLE_TIMBER,
    );
}
// Keep primitive geometry inputs explicit at this small mesh-builder boundary.
#[allow(clippy::too_many_arguments)]
fn cylinder(m: &mut Mesh, x: f32, y: f32, z0: f32, z1: f32, r: f32, sides: usize, material: u8) {
    prim::prism(m, x, y, z0, z1, r, r, sides, 0.0, material);
    prim::disc(m, x, y, z1, r, sides, 0.0, material);
}
fn coat(mount: Mount) -> u8 {
    match mount {
        Mount::Bramble => mat::HORSE_BAY,
        Mount::Cinder => mat::HORSE_CHESTNUT,
        Mount::Mist => mat::HORSE_GREY,
    }
}

/// Side profile faces +x. Staggered far legs leave four separate feet even at
/// dot scale. Neck rises out of the shoulder; muzzle, ears, mane and tail break
/// the silhouette rather than adding small details inside an anonymous block.
fn horse(mount: Mount, selected: bool, tended: bool) -> Mesh {
    let mut m = Mesh::new();
    let body = coat(mount);
    // Authored world-space ear-tip height; this is scene geometry, not pi.
    #[allow(clippy::approx_constant)]
    const EAR_TIP_Z: f32 = 3.14;
    prim::battered(&mut m, 0.0, 0.0, 1.15, 2.03, 1.10, 0.40, 1.02, 0.45, body);
    prim::quad(
        &mut m,
        v3(-1.10, -0.40, 1.15),
        v3(-1.10, 0.40, 1.15),
        v3(1.10, 0.40, 1.15),
        v3(1.10, -0.40, 1.15),
        body,
    );
    prim::quad(
        &mut m,
        v3(-1.02, -0.45, 2.03),
        v3(1.02, -0.45, 2.03),
        v3(1.02, 0.45, 2.03),
        v3(-1.02, 0.45, 2.03),
        body,
    );
    beam(&mut m, v3(0.70, 0.0, 1.72), v3(1.25, 0.0, 2.66), 0.37, body);
    cube(&mut m, v3(1.18, -0.27, 2.40), v3(2.02, 0.27, 2.85), body);
    // Dark muzzle and a visible mane edge, not a silver rim around every face.
    cube(
        &mut m,
        v3(1.89, -0.28, 2.42),
        v3(2.12, 0.28, 2.68),
        mat::TACK_LEATHER,
    );
    beam(
        &mut m,
        v3(0.43, 0.0, 1.94),
        v3(1.03, 0.0, 2.85),
        0.13,
        mat::TACK_LEATHER,
    );
    for y in [-0.20, 0.20] {
        beam(&mut m, v3(1.27, y, 2.77), v3(1.17, y, EAR_TIP_Z), 0.10, body);
    }
    for (x, y, foot) in [
        (-0.85, -0.34, -1.09),
        (-0.55, 0.34, -0.46),
        (0.63, -0.34, 0.50),
        (0.93, 0.34, 1.20),
    ] {
        beam(&mut m, v3(x, y, 1.28), v3(foot, y, 0.18), 0.105, body);
        cube(
            &mut m,
            v3(foot - 0.13, y - 0.12, 0.02),
            v3(foot + 0.16, y + 0.12, 0.23),
            mat::TACK_LEATHER,
        );
    }
    beam(
        &mut m,
        v3(-1.02, 0.0, 1.92),
        v3(-1.52, 0.0, 0.82),
        0.14,
        mat::TACK_LEATHER,
    );
    let cloth = if selected {
        mat::CLOTH_GOLD
    } else {
        mat::CLOTH_BLUE
    };
    // Broad hanging saddlecloth on both flanks: selection is a visible surface,
    // not the old tiny glowing cube, and still works when the coat is grey.
    cube(&mut m, v3(-0.72, -0.49, 1.32), v3(0.43, 0.49, 2.09), cloth);
    cube(
        &mut m,
        v3(-0.41, -0.36, 2.08),
        v3(0.19, 0.36, 2.22),
        mat::TACK_LEATHER,
    );
    if tended {
        cube(
            &mut m,
            v3(0.14, -0.51, 1.50),
            v3(0.29, -0.49, 1.86),
            mat::BRASS,
        );
    }
    m
}
fn lantern(m: &mut Mesh, x: f32, y: f32, z: f32) {
    beam(m, v3(x, y, z + 0.2), v3(x, y, z + 0.65), 0.045, mat::BRASS);
    cube(
        m,
        v3(x - 0.18, y - 0.18, z - 0.02),
        v3(x + 0.18, y + 0.18, z + 0.35),
        mat::WINDOW,
    );
}
fn hay(m: &mut Mesh, x: f32, y: f32) {
    cube(
        m,
        v3(x - 0.70, y - 0.54, 0.02),
        v3(x + 0.70, y + 0.54, 1.15),
        mat::HAY,
    );
    // Two broad dark binding straps continue across the top and near face.
    for dx in [-0.38, 0.38] {
        cube(
            m,
            v3(x + dx - 0.045, y - 0.56, 0.04),
            v3(x + dx + 0.045, y - 0.54, 1.17),
            mat::TACK_LEATHER,
        );
        cube(
            m,
            v3(x + dx - 0.045, y - 0.54, 1.15),
            v3(x + dx + 0.045, y + 0.54, 1.18),
            mat::TACK_LEATHER,
        );
    }
}
fn tack(m: &mut Mesh, x: f32, y: f32) {
    // Freestanding saddle horse, with shaped seat, cantle and hanging stirrups.
    for dx in [-0.44, 0.44] {
        beam(
            m,
            v3(x + dx, y - 0.35, 0.02),
            v3(x + dx, y, 1.85),
            0.09,
            mat::STABLE_TIMBER,
        );
        beam(
            m,
            v3(x + dx, y + 0.35, 0.02),
            v3(x + dx, y, 1.85),
            0.09,
            mat::STABLE_TIMBER,
        );
    }
    rail(m, v3(x - 0.68, y, 1.85), v3(x + 0.68, y, 1.85));
    cube(
        m,
        v3(x - 0.46, y - 0.42, 1.83),
        v3(x + 0.46, y + 0.42, 2.00),
        mat::TACK_LEATHER,
    );
    cube(
        m,
        v3(x - 0.51, y - 0.44, 1.92),
        v3(x - 0.31, y + 0.44, 2.22),
        mat::TACK_LEATHER,
    );
    cube(
        m,
        v3(x + 0.31, y - 0.35, 1.96),
        v3(x + 0.49, y + 0.35, 2.13),
        mat::TACK_LEATHER,
    );
    for dx in [-0.30, 0.30] {
        beam(
            m,
            v3(x + dx, y - 0.45, 1.89),
            v3(x + dx, y - 0.53, 1.03),
            0.05,
            mat::BRASS,
        );
        rail(
            m,
            v3(x + dx - 0.13, y - 0.53, 1.02),
            v3(x + dx + 0.13, y - 0.53, 1.02),
        );
    }
}
fn trough(m: &mut Mesh, x: f32, y: f32) {
    cube(
        m,
        v3(x - 0.85, y - 0.38, 0.0),
        v3(x + 0.85, y + 0.38, 0.58),
        mat::STABLE_TIMBER,
    );
    cube(
        m,
        v3(x - 0.70, y - 0.26, 0.59),
        v3(x + 0.70, y + 0.26, 0.62),
        mat::WATER,
    );
}
fn ground(m: &mut Mesh) {
    cube(
        m,
        v3(-50.0, -40.0, -0.14),
        v3(50.0, 60.0, -0.05),
        mat::GRASS,
    );
}
fn stable_inside(state: &Chivalry) -> Mesh {
    let mut m = Mesh::new();
    cube(
        &mut m,
        v3(-5.2, -7.0, -0.12),
        v3(5.4, 15.2, 0.0),
        mat::STABLE_FLOOR,
    );
    // An actual roofed interior: opaque ceiling, side walls and back wall.
    cube(
        &mut m,
        v3(-5.4, -7.0, 0.0),
        v3(-5.2, 15.4, 4.35),
        mat::STABLE_WALL,
    );
    cube(
        &mut m,
        v3(5.4, -7.0, 0.0),
        v3(5.6, 15.4, 4.35),
        mat::STABLE_WALL,
    );
    cube(
        &mut m,
        v3(-5.4, 15.2, 0.0),
        v3(5.6, 15.4, 4.35),
        mat::STABLE_WALL,
    );
    cube(
        &mut m,
        v3(-5.4, -7.0, 4.20),
        v3(5.6, 15.4, 4.40),
        mat::STABLE_WALL,
    );
    for y in [-1.8, 3.7, 8.2, 12.7] {
        cube(
            &mut m,
            v3(-5.2, y - 0.14, 3.95),
            v3(5.4, y + 0.14, 4.2),
            mat::STABLE_TIMBER,
        );
    }
    // Selected mount is staged nearest the aisle; only two profiles dominate
    // the view, with the third recessed. No roster or game state is altered.
    for i in 0..3 {
        let mount = Mount::ALL[(state.selected.index() + i) % 3];
        let y = 2.4 + i as f32 * 4.5;
        m.merge(
            horse(mount, i == 0, state.tended[mount.index()])
                .scaled(1.0)
                .rotated_z(0.0)
                .translated(v3(-1.8, y, 0.0)),
        );
        // Partitions BEHIND each horse leave its head and feet unobscured.
        for x in [-4.6, 0.7] {
            post(&mut m, x, y + 1.95, 2.25);
        }
        for z in [0.80, 1.40] {
            rail(&mut m, v3(-4.6, y + 1.95, z), v3(0.7, y + 1.95, z));
        }
        // Open, hinged stall gate runs along the aisle rather than across the
        // foreground horse. Its latch post and two bars are still tangible.
        post(&mut m, 0.7, y - 1.75, 2.25);
        for z in [0.8, 1.4] {
            rail(&mut m, v3(0.7, y - 1.75, z), v3(0.7, y - 0.35, z));
        }
        lantern(&mut m, -4.5, y + 1.9, 2.85);
    }
    hay(&mut m, 3.3, 9.0);
    tack(&mut m, 3.3, 5.8);
    trough(&mut m, -3.4, -0.3);
    lantern(&mut m, 3.9, 4.5, 2.85);
    m
}
fn paddock(m: &mut Mesh) {
    for x in [-5.7, 5.7] {
        for y in [-4.0, -1.3, 1.4] {
            post(m, x, y, 1.6);
        }
        for z in [0.65, 1.25] {
            rail(m, v3(x, -4.0, z), v3(x, 1.4, z));
        }
    }
    for x in [-5.7, -1.9, 1.9, 5.7] {
        post(m, x, -4.0, 1.6);
    }
    for z in [0.65, 1.25] {
        rail(m, v3(-5.7, -4.0, z), v3(-1.9, -4.0, z));
        rail(m, v3(1.9, -4.0, z), v3(5.7, -4.0, z));
    }
}
fn stable_outside(state: &Chivalry) -> Mesh {
    let mut m = Mesh::new();
    ground(&mut m);
    cube(
        &mut m,
        v3(-5.5, -7.0, -0.045),
        v3(5.5, 2.0, -0.01),
        mat::PATH,
    );
    cube(
        &mut m,
        v3(-4.8, 2.0, 0.0),
        v3(-3.0, 9.2, 3.3),
        mat::STABLE_WALL,
    );
    cube(
        &mut m,
        v3(1.3, 2.0, 0.0),
        v3(3.2, 9.2, 3.3),
        mat::STABLE_WALL,
    );
    cube(
        &mut m,
        v3(-3.0, 8.9, 0.0),
        v3(1.3, 9.2, 3.3),
        mat::STABLE_WALL,
    );
    cube(
        &mut m,
        v3(-3.0, 2.0, 2.75),
        v3(1.3, 2.3, 3.3),
        mat::STABLE_TIMBER,
    );
    // Bold pitched terracotta roof and filled gables distinguish a barn from
    // the open lists. An opening with half-height gates shows lit depth inside.
    for y in [1.65, 9.5] {
        m.push_tri(
            [v3(-5.15, y, 3.3), v3(3.55, y, 3.3), v3(-0.8, y, 5.1)],
            [[0.0, 0.0], [8.7, 0.0], [4.35, 1.8]],
            mat::STABLE_WALL,
        );
    }
    m.push_quad(
        v3(-5.15, 1.65, 3.3),
        v3(-0.8, 1.65, 5.1),
        v3(-0.8, 9.5, 5.1),
        v3(-5.15, 9.5, 3.3),
        mat::STABLE_ROOF,
    );
    m.push_quad(
        v3(-0.8, 1.65, 5.1),
        v3(3.55, 1.65, 3.3),
        v3(3.55, 9.5, 3.3),
        v3(-0.8, 9.5, 5.1),
        mat::STABLE_ROOF,
    );
    for x in [-3.0, 1.3] {
        post(&mut m, x, 1.9, 2.8);
        lantern(&mut m, x, 1.55, 2.0);
    }
    for z in [0.65, 1.25] {
        rail(&mut m, v3(-3.0, 1.85, z), v3(-1.45, 1.85, z));
        rail(&mut m, v3(1.3, 1.85, z), v3(1.3, 0.45, z));
    }
    hay(&mut m, -1.7, 4.0);
    trough(&mut m, 3.7, -0.6);
    paddock(&mut m);
    m.merge(
        horse(state.selected, true, state.tended[state.selected.index()])
            .scaled(1.0)
            .rotated_z(0.0)
            .translated(v3(-1.6, -0.6, 0.0)),
    );
    // One neighboring cottage and a tree imply the village, not another busy
    // wall of small buildings competing with the stable and paddock.
    m.merge(
        arch::cottage(71)
            .scaled(1.25)
            .rotated_z(0.0)
            .translated(v3(-10.0, 7.0, 0.0)),
    );
    m.merge(
        arch::tree(73)
            .scaled(1.0)
            .rotated_z(0.0)
            .translated(v3(-9.0, 12.0, 0.0)),
    );
    m
}

fn mounted(mount: Mount, selected: bool, running: bool, raised: bool) -> Mesh {
    let mut m = horse(mount, selected, false);
    let cloth = if selected {
        mat::CLOTH_GOLD
    } else {
        mat::CLOTH_BLUE
    };
    cube(
        &mut m,
        v3(-0.38, -0.30, 2.15),
        v3(0.18, 0.30, 3.02),
        mat::ARMOR,
    );
    cylinder(&mut m, -0.10, 0.0, 3.04, 3.50, 0.29, 6, mat::ARMOR);
    cube(&mut m, v3(-0.39, -0.36, 2.20), v3(0.14, -0.31, 2.92), cloth);
    let hand = v3(0.44, -0.35, 2.85);
    beam(&mut m, v3(-0.10, -0.25, 2.9), hand, 0.12, mat::ARMOR);
    beam(
        &mut m,
        hand,
        if running {
            v3(4.45, -0.35, 2.62)
        } else {
            v3(1.70, -0.35, 5.0)
        },
        0.065,
        mat::CLOTH_GOLD,
    );
    if raised {
        beam(
            &mut m,
            v3(-0.10, 0.24, 2.85),
            v3(0.20, 0.50, 4.13),
            0.12,
            mat::ARMOR,
        );
        m.push_quad(
            v3(0.20, 0.5, 4.13),
            v3(1.0, 0.5, 4.13),
            v3(1.0, 0.5, 3.64),
            v3(0.20, 0.5, 3.78),
            cloth,
        );
    }
    m
}
fn lists(state: &Chivalry) -> Mesh {
    let mut m = Mesh::new();
    ground(&mut m);
    cube(
        &mut m,
        v3(-7.0, -4.0, -0.045),
        v3(7.0, 13.0, -0.01),
        mat::STABLE_FLOOR,
    );
    for x in [-7.0, 7.0] {
        for y in [-4.0, 0.25, 4.5, 8.75, 13.0] {
            post(&mut m, x, y, 1.8);
        }
        for z in [0.65, 1.40] {
            rail(&mut m, v3(x, -4.0, z), v3(x, 13.0, z));
        }
    }
    for y in [-4.0, 13.0] {
        for x in [-7.0, -3.5, 0.0, 3.5, 7.0] {
            post(&mut m, x, y, 1.8);
        }
        for z in [0.65, 1.40] {
            rail(&mut m, v3(-7.0, y, z), v3(7.0, y, z));
        }
    }
    // The continuous central tilt rail divides two open lanes.
    for y in [-0.6, 3.5, 7.5, 11.6] {
        post(&mut m, 0.0, y, 1.65);
    }
    for z in [0.75, 1.35] {
        rail(&mut m, v3(0.0, -0.6, z), v3(0.0, 11.6, z));
    }
    for (x, material) in [(-6.5, mat::CLOTH_GOLD), (6.5, mat::CLOTH_BLUE)] {
        post(&mut m, x, 10.5, 4.0);
        m.push_quad(
            v3(x, 10.5, 3.95),
            v3(x + 1.0, 10.5, 3.95),
            v3(x + 1.0, 10.5, 2.7),
            v3(x, 10.5, 2.7),
            material,
        );
    }
    let t = &state.tournament;
    let running = t.phase == Phase::Running;
    let finished = t.phase == Phase::Finished;
    let (player_y, rival_y) = if running {
        (
            3.2 + t.played() as f32 * 0.45,
            7.6 - t.played() as f32 * 0.45,
        )
    } else {
        (1.0, 8.5)
    };
    let (player_wins, rival_wins) = (
        finished && t.result() != "LOSS",
        finished && t.result() != "WIN",
    );
    for (x, y, mount, selected, winner, yaw) in [
        (
            -2.7,
            player_y,
            t.mount,
            true,
            player_wins,
            std::f32::consts::FRAC_PI_2,
        ),
        (
            2.7,
            rival_y,
            Mount::Mist,
            false,
            rival_wins,
            -std::f32::consts::FRAC_PI_2,
        ),
    ] {
        let z: f32 = if winner { 0.55 } else { 0.0 };
        if finished {
            cube(
                &mut m,
                v3(x - 1.15, y - 1.65, 0.0),
                v3(x + 1.15, y + 2.25, z.max(0.18)),
                mat::STABLE_TIMBER,
            );
            cube(
                &mut m,
                v3(x - 1.12, y - 1.68, 0.16),
                v3(x + 1.12, y - 1.65, z.max(0.18)),
                if selected {
                    mat::CLOTH_GOLD
                } else {
                    mat::CLOTH_BLUE
                },
            );
        }
        m.merge(
            mounted(mount, selected, running, winner)
                .scaled(1.0)
                .rotated_z(yaw)
                .translated(v3(x, y, z)),
        );
    }
    for i in 0..t.played() {
        cube(
            &mut m,
            v3(-0.16, 0.6 + i as f32 * 1.2, 1.38),
            v3(0.16, 1.12 + i as f32 * 1.2, 1.62),
            mat::CLOTH_GOLD,
        );
    }
    if finished {
        // Static presentation of the actual score result. A draw raises both;
        // a loss raises the rival. No inferred victory or outcome animation.
        for x in [-1.3, 1.3] {
            post(&mut m, x, 12.4, 4.0);
        }
        m.push_quad(
            v3(-1.3, 12.4, 3.9),
            v3(1.3, 12.4, 3.9),
            v3(1.3, 12.4, 2.35),
            v3(-1.3, 12.4, 2.35),
            if t.result() == "LOSS" {
                mat::CLOTH_BLUE
            } else {
                mat::CLOTH_GOLD
            },
        );
    }
    for (x, material) in [
        (-4.6, mat::CLOTH_GOLD),
        (3.7, mat::CLOTH_BLUE),
        (5.2, mat::CLOTH_BLUE),
    ] {
        cube(
            &mut m,
            v3(x - 0.20, 13.6, 0.2),
            v3(x + 0.20, 13.95, 1.2),
            material,
        );
        cylinder(&mut m, x, 13.77, 1.23, 1.61, 0.21, 6, mat::ARMOR);
    }
    m
}

pub(crate) fn composition(state: &Chivalry, visit: Visit) -> (Mesh, View3) {
    let s = visit.station.min(3) as usize;
    let (mesh, eye, target) = match (visit.place, visit.inside) {
        (Place::Stables, true) => (
            stable_inside(state),
            station(Place::Stables, visit.station),
            [
                v3(0.0, 2.6, 1.40),
                v3(-1.0, 3.2, 1.45),
                v3(-1.0, 7.0, 1.45),
                v3(-1.0, 11.3, 1.45),
            ][s],
        ),
        (Place::Stables, false) => (
            stable_outside(state),
            v3(7.2 + s as f32 * 0.35, -9.0, 5.4),
            v3(-1.2, 2.0, 1.3),
        ),
        (Place::Tournament, _) => (
            lists(state),
            station(Place::Tournament, visit.station),
            [
                v3(0.0, 4.5, -0.65),
                v3(-0.2, 5.0, 0.8),
                v3(0.0, 6.5, 0.8),
                v3(0.0, 8.6, 0.5),
            ][s],
        ),
    };
    let delta = target - eye;
    let view = View3 {
        pos: eye,
        heading_rad: delta.y.atan2(delta.x),
        pitch: (delta.z / (delta.x * delta.x + delta.y * delta.y).sqrt()).atan(),
        fov_rad: 0.84,
    };
    (mesh, view)
}

/// Short terminal panes widen only this local lens to keep ears and feet.
pub(crate) fn lens(w: usize, h: usize, zoom: f32) -> f32 {
    let aspect = h as f32 / w.max(1) as f32;
    (zoom * 0.80).clamp(0.6, 1.0) * (0.70 / aspect.max(0.1)).clamp(1.0, 1.5)
}

pub(crate) fn frame(
    state: &Chivalry,
    visit: Visit,
    w: usize,
    h: usize,
    gaze: [f32; 4],
) -> image::RgbaImage {
    let (w, h) = (w.min(512), h.min(320));
    if w == 0 || h == 0 {
        return image::RgbaImage::new(w as u32, h as u32);
    }
    let (mesh, mut view) = composition(state, visit);
    // Preserve the existing operator gaze/sway inputs; only authored camera
    // stations and this scene's default lens have moved.
    view.pos += v3(gaze[0] * 0.08, 0.0, 0.0);
    view.heading_rad += gaze[1];
    view.pitch += gaze[2];
    view.fov_rad = lens(w, h, gaze[3]);
    raster::render_scene(&mesh, &view, w, h)
}

/// Additive warm village dressing on the *existing* ordinary court mesh.
pub(crate) fn village_dressing() -> Mesh {
    let mut m = Mesh::new();
    // Low-poly facing accents, rather than full stable props: the old court
    // retains its <5000-triangle budget. This addition is only 43 triangles.
    for (x, y) in [(-9.2, -18.4), (-5.7, -18.4), (-2.2, -18.4)] {
        prim::quad(
            &mut m,
            v3(x - 0.07, y, 0.0),
            v3(x + 0.07, y, 0.0),
            v3(x + 0.07, y, 1.85),
            v3(x - 0.07, y, 1.85),
            mat::WOOD,
        );
        prim::quad(
            &mut m,
            v3(x - 0.18, y, 1.5),
            v3(x + 0.18, y, 1.5),
            v3(x + 0.18, y, 1.8),
            v3(x - 0.18, y, 1.8),
            mat::FIRE,
        );
        prim::quad(
            &mut m,
            v3(x - 0.24, y, 1.8),
            v3(x + 0.24, y, 1.8),
            v3(x + 0.18, y, 1.92),
            v3(x - 0.18, y, 1.92),
            mat::WOOD,
        );
        prim::quad(
            &mut m,
            v3(x - 0.7, y - 0.7, 0.01),
            v3(x + 0.7, y - 0.7, 0.01),
            v3(x + 0.7, y + 0.7, 0.01),
            v3(x - 0.7, y + 0.7, 0.01),
            mat::TORCH_POOL,
        );
    }
    for x in [-9.2, -2.2] {
        prim::quad(
            &mut m,
            v3(x - 0.55, -18.1, 0.43),
            v3(x + 0.55, -18.1, 0.43),
            v3(x + 0.55, -17.65, 0.43),
            v3(x - 0.55, -17.65, 0.43),
            mat::WOOD,
        );
        prim::quad(
            &mut m,
            v3(x - 0.28, -18.1, 0.43),
            v3(x + 0.28, -18.1, 0.43),
            v3(x + 0.28, -18.1, 0.62),
            v3(x - 0.28, -18.1, 0.62),
            mat::HAY,
        );
    }
    // Five hanging flags share a rope and its two poles, not five tall posts.
    for x in [-9.0, -4.2] {
        prim::quad(
            &mut m,
            v3(x - 0.07, -20.0, 0.0),
            v3(x + 0.07, -20.0, 0.0),
            v3(x + 0.07, -20.0, 3.5),
            v3(x - 0.07, -20.0, 3.5),
            mat::WOOD,
        );
    }
    prim::quad(
        &mut m,
        v3(-9.0, -20.0, 3.39),
        v3(-4.2, -20.0, 3.39),
        v3(-4.2, -20.0, 3.44),
        v3(-9.0, -20.0, 3.44),
        mat::WOOD,
    );
    for (i, x) in [-9.0, -7.8, -6.6, -5.4, -4.2].into_iter().enumerate() {
        prim::tri(
            &mut m,
            [
                v3(x, -20.0, 3.39),
                v3(x + 0.75, -20.0, 3.39),
                v3(x + 0.38, -20.0, 2.7),
            ],
            [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
            if i % 2 == 0 {
                mat::CLOTH_GOLD
            } else {
                mat::CLOTH_BLUE
            },
        );
    }
    m
}

pub(crate) fn scene(state: &Chivalry, visit: Visit) -> Mesh {
    composition(state, visit).0
}
/// Connected, prop-clear aisle and outer-arena camera stations.
pub(crate) fn station(place: Place, index: u8) -> V3 {
    let s = index.min(3) as usize;
    match place {
        Place::Stables => [
            v3(1.4, -5.4, 2.15),
            v3(2.6, -1.8, 2.05),
            v3(2.6, 2.6, 2.1),
            v3(2.2, 7.0, 2.1),
        ][s],
        Place::Tournament => [
            v3(10.5, -7.0, 3.8),
            v3(8.2, -1.0, 2.8),
            v3(8.0, 2.3, 2.8),
            v3(8.0, 6.2, 2.8),
        ][s],
    }
}
