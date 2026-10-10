//! The court's practice-district dressing: torch posts, a hay stand and a
//! line of flags, added to the ordinary 3D court. The stables and the lists
//! themselves are rooms of the world, walked in the Delve.
use super::arch::prim;
use super::math::v3;
use super::mesh::{Mesh, mat};

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
