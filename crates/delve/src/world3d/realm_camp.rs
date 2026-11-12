//! The blank-slate realm's places in the monolith language
//! (`crate::world::plan`; docs/REALM-MONOLITH.md "Scale", for every castle but
//! this one): a camp where a model house's castle will be built, and the
//! Keep's site, which stands nothing here.
//!
//! - **The Keep** (`Kit::KeepBegun`): the great tower on its site, raised
//!   stone by stone as the run says (`world3d::works_site`, the Works). The
//!   plan keeps the site as a build so the ground under it is the open floor,
//!   but this kit stands no blocks of its own: the stones are the run's.
//! - **A camp** (`Kit::Camp`, seven tiles by four): two cloth tents, a fire
//!   ringed with stones and crossed with logs, a barrel, a banner on its pole.
//!   No wall and no gate. The fire is the camp's lamp.
//!
//! The monolith look is monotone: a camp's cloth is the look's cloth. The
//! house's own colours are the flat map's (`render::overworld::camps`).

use angel_stage::set::Kind;

use super::realm_mono::Mono;
use super::realm_place::Stood;

/// Where a camp's cloth stands, as `[x0, z0, x1, z1]` in the kingdom's pixels,
/// for the camp over `foot` (`[x0, z0, x1, z1]`; a camp always faces south,
/// to its stand): its two tents together, and the thin strip its banner flies
/// over. The realm's look dyes them in the house's colours
/// (`realm_look::camp_zones`), nothing round them.
pub fn camp_cloth_boxes(foot: [f32; 4]) -> [[f32; 4]; 2] {
    let (cx, cz) = ((foot[0] + foot[2]) / 2.0, (foot[1] + foot[3]) / 2.0);
    let (hx, hz) = ((foot[2] - foot[0]) / 2.0 - 2.0, (foot[3] - foot[1]) / 2.0 - 2.0);
    [
        [cx - hx + 10.0, cz - hz + 4.0, cx + 24.0, cz - hz + 41.0],
        [cx + hx - 21.0, cz - hz + 8.5, cx + hx - 4.5, cz - hz + 11.5],
    ]
}

/// The Keep's site as a place: nothing stands from the plan (the great tower
/// is raised stone by stone from the run's state, `world3d::works_site`).
pub fn keep_begun(_m: &mut Mono, _y0: f32, _s: &mut Stood) {}

/// A camp: two tents, a fire, a barrel, a banner on its pole.
pub fn camp(m: &mut Mono, y0: f32, s: &mut Stood) {
    let (hx, hz) = (m.w / 2.0 - 2.0, m.d / 2.0 - 2.0);
    // The larger tent, back left; the smaller, nearer the middle.
    m.put("pyramid", Kind::Cloth, [-hx + 30.0, y0, -hz + 24.0], [40.0, 30.0 - y0, 34.0], 0.0);
    m.block(Kind::Door, [-hx + 27.0, -hx + 33.0], [y0, 13.0], [-hz + 41.0, -hz + 41.4]);
    m.put("pyramid", Kind::Cloth, [8.0, y0, -hz + 18.0], [32.0, 24.0 - y0, 28.0], 0.0);
    m.block(Kind::Door, [5.5, 10.5], [y0, 10.0], [-hz + 31.5, -hz + 31.9]);
    // The ridge poles' tips.
    for (x, z, h) in [(-hx + 30.0, -hz + 24.0, 33.0), (8.0, -hz + 18.0, 27.0)] {
        m.block(Kind::Wood, [x - 0.7, x + 0.7], [h - 4.0, h + 3.0], [z - 0.7, z + 0.7]);
    }
    // The fire, in the middle of the front: a ring of stones, crossed logs, a flame.
    let (fx, fz) = (-6.0, hz - 10.0);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let (sx, sz) = (fx + a.cos() * 6.5, fz + a.sin() * 5.0);
        m.block(Kind::Stone, [sx - 1.6, sx + 1.6], [y0, 2.5], [sz - 1.6, sz + 1.6]);
    }
    for turn in [25.0f32, -25.0] {
        m.put("box", Kind::Trunk, [fx, 0.0, fz], [11.0, 2.4, 2.4], turn);
    }
    m.put("cone-6", Kind::Fire, [fx, 1.5, fz], [5.0, 8.0, 5.0], 0.0);
    s.lamp(m, [fx, 11.0, fz], 48.0, 0.85, 2.0);
    // A barrel and a bedroll by the larger tent.
    m.put("cylinder-12", Kind::Wood, [-hx + 4.0, y0, hz - 14.0], [7.0, 9.0 - y0, 7.0], 0.0);
    // (Timber, not cloth: only the tents and the banner are dyed in the house's colours.)
    m.block(Kind::Wood, [-hx + 14.0, -hx + 26.0], [y0, 2.5], [hz - 12.0, hz - 7.0]);
    // The banner on its pole at the right edge.
    let (bx, bz) = (hx - 6.0, -hz + 10.0);
    // A pole a pixel and a bit across, so the banner is seen hung from it, not floating.
    m.block(Kind::Wood, [bx - 1.3, bx + 1.3], [y0, 44.0], [bz - 1.3, bz + 1.3]);
    m.block(Kind::Cloth, [bx - 15.0, bx - 1.3], [32.0, 43.0], [bz - 0.4, bz + 0.4]);
}
