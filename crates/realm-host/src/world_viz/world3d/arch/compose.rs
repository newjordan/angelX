//! Composed sample scenes — drop one call in and get a full staged vista.

use super::super::math::{V3, v3};
use super::super::mesh::Mesh;
use super::castle::{curtain_wall, gatehouse, keep, rookery_dressing, round_tower};
use super::land::{Strip, TerrainOpts, ground_z, terrain_patch_with, tree};
use super::library::library;
use super::prim;
use super::rng::{Rng, sub_seed};
use super::village::{bridge, chapel, cottage, market_stall, well};
use super::works::{forge, observatory};

pub const COURT_EXTENT: f32 = 28.0;

pub fn court_terrain() -> TerrainOpts {
    TerrainOpts {
        cell: 2.9,
        relief: 0.30,
        path: Some(Strip::along_y(0.0, 1.20)),
        river: Some(Strip::along_x(-22.0, 1.90)),
        water_depth: 0.34,
    }
}

pub fn court_ground_z(seed: u64, x: f32, y: f32) -> f32 {
    ground_z(seed, x, y, &court_terrain())
}

pub fn court_scene(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x43_4F_55_52));
    let mut m = Mesh::new();

    let opts = court_terrain();
    m.merge(super::realm_land::terrain(seed));

    let place = |out: &mut Mesh, part: Mesh, x: f32, y: f32, yaw: f32| {
        let z = ground_z(seed, x, y, &opts) - 0.12;
        out.merge(part.rotated_z(yaw).translated(v3(x, y, z)));
    };

    let court = 11.0_f32;
    let gate_half = 4.05_f32;
    let run = court - gate_half;
    let run_cx = (court + gate_half) * 0.5;

    place(&mut m, keep(sub_seed(seed, 1)), 0.0, 1.5, 0.0);
    place(&mut m, gatehouse(sub_seed(seed, 2)), 0.0, -court, 0.0);

    let north = curtain_wall(court * 2.0, sub_seed(seed, 3));

    place(&mut m, north, 0.0, court, std::f32::consts::PI);
    for (x, yaw) in [
        (court, std::f32::consts::FRAC_PI_2),
        (-court, -std::f32::consts::FRAC_PI_2),
    ] {
        place(&mut m, curtain_wall(5.0, sub_seed(seed, 4)), x, -8.5, yaw);
        place(&mut m, curtain_wall(13.0, sub_seed(seed, 54)), x, 4.5, yaw);
        let z = court_ground_z(seed, x, -4.0) - 0.12;
        prim::boxed(
            &mut m,
            v3(x - 0.4, -6.0, z + 3.1),
            v3(x + 0.4, -2.0, z + 3.45),
            super::super::mesh::mat::STONE_DARK,
            prim::F_SIDES_TOP,
        );
    }
    let stub = curtain_wall(run, sub_seed(seed, 5));
    place(&mut m, stub.clone(), run_cx, -court, 0.0);
    place(&mut m, stub, -run_cx, -court, 0.0);

    for (i, &(sx, sy)) in [(-1.0_f32, -1.0_f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .enumerate()
    {
        let mut t = round_tower(5.6, 1.6, sub_seed(seed, 10 + i as u64));
        if (sx, sy) == (-1.0, 1.0) {
            t.merge(rookery_dressing(5.6, 1.6, sub_seed(seed, 10 + i as u64)));
        }
        place(&mut m, t, sx * court, sy * court, 0.0);
    }

    place(&mut m, well(), 0.0, -5.5, 0.0);
    place(
        &mut m,
        market_stall(sub_seed(seed, 20)),
        -6.5,
        -5.0,
        r.jitter(0.35),
    );
    place(
        &mut m,
        market_stall(sub_seed(seed, 21)),
        6.0,
        -6.2,
        r.jitter(0.35),
    );

    place(&mut m, library(sub_seed(seed, 30)), -20.0, 6.0, 0.0);
    place(
        &mut m,
        chapel(sub_seed(seed, 31)),
        17.0,
        7.0,
        -0.30 + r.jitter(0.12),
    );
    place(
        &mut m,
        observatory(sub_seed(seed, 32)),
        18.5,
        -11.5,
        0.72 + r.jitter(0.06),
    );
    place(&mut m, bridge(9.0), 0.0, -22.0, std::f32::consts::FRAC_PI_2);

    let mut dial = Mesh::new();
    super::super::instruments::survey_dial(&mut dial);
    place(&mut m, dial, 22.0, -12.4, 0.0);

    let bridge_z = court_ground_z(seed, 0.0, -22.0) - 0.12;
    let deck_z = 1.30 + 0.50 * (1.0 - (3.0_f32 / 4.5).powi(2));
    for x in [-1.16, 1.16] {
        let mut lamp = Mesh::new();
        super::super::instruments::bridge_lantern(&mut lamp);
        m.merge(lamp.translated(v3(x, -19.0, bridge_z + deck_z + 0.58)));
    }

    let hamlet = [
        (-7.6_f32, -14.4_f32),
        (7.4, -13.2),
        (-8.6, -18.6),
        (8.8, -19.6),
    ];
    for (i, &(x, y)) in hamlet.iter().enumerate() {
        let yaw = if x < 0.0 {
            std::f32::consts::FRAC_PI_2
        } else {
            -std::f32::consts::FRAC_PI_2
        } + r.jitter(0.20);
        place(&mut m, cottage(sub_seed(seed, 40 + i as u64)), x, y, yaw);
    }
    m.merge(super::super::chivalry::village_dressing());

    place(&mut m, forge(sub_seed(seed, 44)), 10.8, -16.4, 0.66);

    for (i, &(x, y)) in [(-23.2_f32, -6.0_f32), (-25.3, -1.2)].iter().enumerate() {
        place(
            &mut m,
            tree(sub_seed(seed, 60 + i as u64)),
            x,
            y,
            r.range(0.0, std::f32::consts::TAU),
        );
    }

    let blocked = |x: f32, y: f32| -> bool {
        (x.abs() < 14.0 && y.abs() < 14.0)
            || x.abs() < 2.6
            || (y + 22.0).abs() < 3.5
            || ((x + 20.0).abs() < 8.0 && (y - 6.0).abs() < 6.5)
            || ((x - 17.0).abs() < 5.5 && (y - 7.0).abs() < 4.5)
            || ((x - 18.5).abs() < 5.0 && (y + 11.5).abs() < 5.0)
            || (x.abs() < 12.5 && y < -11.0 && y > -22.5)
            || (x < -11.0 && y > -20.0)
            || (x > 12.0 && y < -12.0 && y > -22.0)
            || (x > 17.0 && y > 20.0)
            || super::realm_land::scenery_excluded(x, y)
    };
    let mut planted = 0;
    let mut attempts = 0;
    while planted < 12 && attempts < 200 {
        attempts += 1;
        let x = r.range(-25.0, 25.0);
        let y = r.range(-25.0, 25.0);
        if blocked(x, y) {
            continue;
        }
        place(
            &mut m,
            tree(sub_seed(seed, 100 + planted as u64)),
            x,
            y,
            r.range(0.0, std::f32::consts::TAU),
        );
        planted += 1;
    }

    m.merge(super::realm_dressing::dressing(seed));

    m
}

pub fn hamlet_scene(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x48_41_4D_4C));
    let mut m = Mesh::new();
    let opts = TerrainOpts {
        cell: 2.2,
        relief: 0.26,
        path: Some(Strip::along_x(0.0, 1.10)),
        river: None,
        water_depth: 0.0,
    };
    m.merge(terrain_patch_with(26.0, 18.0, seed, &opts));

    let place = |out: &mut Mesh, part: Mesh, x: f32, y: f32, yaw: f32| {
        let z = ground_z(seed, x, y, &opts) - 0.12;
        out.merge(part.rotated_z(yaw).translated(v3(x, y, z)));
    };

    for i in 0..4 {
        let x = -7.5 + i as f32 * 5.0;
        let sy = if i % 2 == 0 { -1.0_f32 } else { 1.0 };
        let yaw = if sy < 0.0 { 0.0 } else { std::f32::consts::PI };
        place(
            &mut m,
            cottage(sub_seed(seed, 200 + i as u64)),
            x + r.jitter(0.4),
            sy * 4.2,
            yaw + r.jitter(0.15),
        );
    }
    place(&mut m, well(), 1.2, -1.9, 0.0);
    place(&mut m, tree(sub_seed(seed, 210)), -10.5, 5.5, 0.0);
    place(&mut m, tree(sub_seed(seed, 211)), 10.0, -5.8, 0.0);
    m
}

pub fn scene_bounds(m: &Mesh) -> Option<(V3, V3)> {
    prim::bbox(m)
}

#[cfg(test)]
#[path = "../../../../tests/world_viz/world3d__arch__compose__district_tests.rs"]
mod district_tests;
