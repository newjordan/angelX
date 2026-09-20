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

/// Half-extent of [`court_scene`]'s ground patch, in tiles. A camera
/// outside `±COURT_EXTENT` stands off the edge of the world and sees haze
/// where the ground should be.
pub(crate) const COURT_EXTENT: f32 = 28.0;

/// The ground options [`court_scene`] builds its terrain with. Exposed so
/// a camera can stand *on* the same surface the scene was displaced by
/// ([`court_ground_z`]) instead of floating at z = 0.
pub(crate) fn court_terrain() -> TerrainOpts {
    TerrainOpts {
        cell: 2.9,
        relief: 0.30,
        path: Some(Strip::along_y(0.0, 1.20)),
        river: Some(Strip::along_x(-22.0, 1.90)),
        water_depth: 0.34,
    }
}

/// Ground height under a point of [`court_scene`] built with `seed` —
/// the vantage staging plants the eye on this, so a camera on the north rise
/// really does look down into the court.
pub(crate) fn court_ground_z(seed: u64, x: f32, y: f32) -> f32 {
    ground_z(seed, x, y, &court_terrain())
}

/// The reference vista: a walled castle court on a rolling terrain patch, with
/// the library in the west meadow, a chapel and a star-tower on the east rise,
/// a hamlet and a smithy strung along the approach road, and a stone bridge
/// over the river.
///
/// **Footprint 56 × 56 tiles**, centred on the origin, ground plane at z ≈ 0.
/// 4787 triangles, including district roads, posterns and stable yard.
/// The whole-scene budget is 5k.
///
/// # Landmark anchors, for camera staging
/// (World vista law: frame a mass from across open ground, never from a
/// doorway. Bearings below are world bearings — +x is east, +y is north, and
/// the moon hangs in the **south-west**, so a south or west face is lit and an
/// east or north face is dark.)
///
/// | landmark | anchor | faces | tall | staged for |
/// |---|---|---|---|---|
/// | keep | `(0, 1.5)` | porch −y | 5.9 | Keep |
/// | gatehouse | `(0, -11)` | approach −y | 7.4 | Gatehouse, Round Table |
/// | NW drum + rookery | `(-11, 11)` | **lit limb west** | 8.6 | Rookery |
/// | library | `(-20, 6)` | **flank + portico −y** | 9.8 | Scriptorium |
/// | forge | `(10.8, -16.4)` | **open mouth SE** | 4.4 | Smithy |
/// | chapel | `(17, 7)` | entrance −y | 6.9 | Chapel |
/// | observatory | `(18.5, -11.5)` | **slit SE** | 7.6 | Observatory |
/// | bridge | `(0, -22)` | spans the river along y | 2.4 | the travel sweep |
///
/// # Why the plan is what it is
/// - The **library lies broadside to the southern meadow** rather than
///   end-on to the court, because a 13-tile hall seen down its gable is a
///   6-tile blank and seen across its flank is a ladder of lit bays. It is the
///   only mass in the realm long enough to need a 23-tile standoff, and the
///   meadow south of it is the only ground that gives one.
/// - The **hamlet stands ~7.5 tiles off the road centre line**, not ~5.5. The
///   travel sweep runs up the east verge, and at the old spacing a cottage
///   gable passed within 1.7 tiles of the camera — a wall in the face, which
///   is the one thing the vista law forbids outright.
/// - The **forge sits outboard of the hamlet** on the east, so its mouth has a
///   clear line down to the river bank and the castle's south-east drum stacks
///   up behind it.
pub(crate) fn court_scene(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x43_4F_55_52)); // "COUR"
    let mut m = Mesh::new();

    // ---- ground -----------------------------------------------------------
    let opts = court_terrain();
    m.merge(super::realm_land::terrain(seed));

    // Sit a part on the ground, sunk slightly so no seam shows.
    let place = |out: &mut Mesh, part: Mesh, x: f32, y: f32, yaw: f32| {
        let z = ground_z(seed, x, y, &opts) - 0.12;
        out.merge(part.rotated_z(yaw).translated(v3(x, y, z)));
    };

    // ---- the castle court -------------------------------------------------
    let court = 11.0_f32;
    let gate_half = 4.05_f32;
    let run = court - gate_half; // south wall run length
    let run_cx = (court + gate_half) * 0.5;

    place(&mut m, keep(sub_seed(seed, 1)), 0.0, 1.5, 0.0);
    place(&mut m, gatehouse(sub_seed(seed, 2)), 0.0, -court, 0.0);

    let north = curtain_wall(court * 2.0, sub_seed(seed, 3));

    place(&mut m, north, 0.0, court, std::f32::consts::PI);
    // Ground-level posterns make the lateral district paths pass through
    // real openings: four tiles clear, centered at y=-4 on each side wall.
    // The low lintel retains the castle silhouette without closing the road.
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
        // The **north-west** drum is the rookery: same tower, dressed. It has
        // to be that corner and no other. The moon sits in the south-west, so
        // a corner tower's lit faces point along the two walls that meet at it
        // — and for the north-east drum those both point *into* the court,
        // where no camera can stand. Only the north-west drum shows a moonlit
        // limb to open ground, and a dovecote band nobody can see the light on
        // is a band nobody can see.
        if (sx, sy) == (-1.0, 1.0) {
            t.merge(rookery_dressing(5.6, 1.6, sub_seed(seed, 10 + i as u64)));
        }
        place(&mut m, t, sx * court, sy * court, 0.0);
    }

    // ---- court furniture --------------------------------------------------
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

    // ---- outside the walls ------------------------------------------------
    // Broadside to the southern meadow: the long flank, its window ladder and
    // the portico all face −y, and the 13-tile hall fits between the court's
    // west wall and the edge of the ground patch only at yaw 0.
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

    // The hamlet stands well off the road: the travel sweep runs the east
    // verge and needs its standoff (see the doc comment).
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
    // The smithy, outboard of the hamlet, mouth turned down the river bank.
    place(&mut m, forge(sub_seed(seed, 44)), 10.8, -16.4, 0.66);

    // A deliberate copse in front of the library. The tree scatter below is
    // rejection-sampled and keeps out of the staged corridors on purpose, so
    // the Scriptorium's foreground would otherwise be 25 tiles of bare meadow
    // — the one thing a 23-tile standoff cannot supply for itself is depth.
    for (i, &(x, y)) in [(-23.2_f32, -6.0_f32), (-25.3, -1.2)].iter().enumerate() {
        place(
            &mut m,
            tree(sub_seed(seed, 60 + i as u64)),
            x,
            y,
            r.range(0.0, std::f32::consts::TAU),
        );
    }

    // ---- scattered trees --------------------------------------------------
    // Rejection sampling with a hard attempt cap — bounded, and deterministic
    // because the only entropy is the seeded stream.
    // Trees are scenery, and scenery must never stand in a staged sightline —
    // a trunk 3 tiles off the eye is a door close-up with bark on it. The
    // blocked set is therefore the buildings *plus* the meadow corridors the
    // vantages look down (`world3d::VANTAGES`).
    let blocked = |x: f32, y: f32| -> bool {
        (x.abs() < 14.0 && y.abs() < 14.0)                       // the court
            || x.abs() < 2.6                                     // the road
            || (y + 22.0).abs() < 3.5                            // the river
            || ((x + 20.0).abs() < 8.0 && (y - 6.0).abs() < 6.5) // the library
            || ((x - 17.0).abs() < 5.5 && (y - 7.0).abs() < 4.5) // the chapel
            || ((x - 18.5).abs() < 5.0 && (y + 11.5).abs() < 5.0) // the star-tower
            || (x.abs() < 12.5 && y < -11.0 && y > -22.5)        // hamlet + forge
            || (x < -11.0 && y > -20.0)                          // west meadow: the
            //   Scriptorium's sightline south of the library and the Rookery's
            //   north of it are one continuous corridor down the west side.
            || (x > 12.0 && y < -12.0 && y > -22.0) // Observatory / Smithy corridor
            || (x > 17.0 && y > 20.0) // stable yard plus crown/root clearance
            || super::realm_land::scenery_excluded(x, y) // keep district paths open
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

/// A single hamlet row — four cottages, a well and a pair of trees on a small
/// terrain patch. Handy as a cheap mid-ground vista (~1.3k tris).
///
/// **Footprint 26 × 18 tiles**, centred on the origin, road running along +x.
pub(crate) fn hamlet_scene(seed: u64) -> Mesh {
    let mut r = Rng::new(sub_seed(seed, 0x48_41_4D_4C)); // "HAML"
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

/// Bounds of a composed scene, for camera framing. Re-exported convenience so
/// callers do not need [`prim`] directly.
pub(crate) fn scene_bounds(m: &Mesh) -> Option<(V3, V3)> {
    prim::bbox(m)
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/world_viz/world3d__arch__compose__district_tests.rs"]
mod district_tests;
