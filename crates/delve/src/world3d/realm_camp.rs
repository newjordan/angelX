//! The blank-slate realm's two places in the monolith language
//! (`crate::world::plan`; docs/REALM-MONOLITH.md "Scale", for every castle but
//! this one): the tiny castle just begun, small enough to see whole, and a
//! camp where a model house's castle will be built.
//!
//! - **The Keep begun** (`Kit::KeepBegun`, eight tiles by five): a low
//!   foundation course, a squat tower half raised with a ragged top and
//!   scaffolding up it, a stump of a twin, a back wall and front stubs with a
//!   gap for the gate (a timber frame in it), a heap of cut stone, a banner
//!   and a brazier at the gate.
//! - **A camp** (`Kit::Camp`, seven tiles by four): two cloth tents, a fire
//!   ringed with stones and crossed with logs, a barrel, a banner on its pole.
//!   No wall and no gate. The fire is the camp's lamp.
//!
//! The monolith look is monotone: a camp's cloth is the look's cloth. The
//! house's own colours are the flat map's (`render::overworld::camps`).

use angel_stage::set::Kind;

use super::realm_mono::Mono;
use super::realm_place::Stood;

/// The Keep's banner pole, in the Keep's own frame, for a ground `w` wide
/// and `d` deep: over the squat tower's front corner.
fn keep_pole(w: f32, d: f32) -> (f32, f32) {
    let (hx, hz) = (w / 2.0 - 1.0, d / 2.0 - 1.0);
    (-hx + 8.0 + 34.0 - 6.0, -hz + 8.0 + 34.0 - 6.0)
}

/// Where the Keep's banner flies, as `[x0, z0, x1, z1]` in the kingdom's
/// pixels, for the Keep over `foot` (`[x0, z0, x1, z1]`, facing south as it
/// is built): the realm's look dyes it in the Keep's colours
/// (`realm_look::camp_zones`).
pub fn keep_banner_box(foot: [f32; 4]) -> [f32; 4] {
    let (cx, cz) = ((foot[0] + foot[2]) / 2.0, (foot[1] + foot[3]) / 2.0);
    let (bx, bz) = keep_pole(foot[2] - foot[0], foot[3] - foot[1]);
    [cx + bx, cz + bz - 1.0, cx + bx + 16.0, cz + bz + 1.0]
}

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

/// The Keep as the blank-slate plan has it, begun.
pub fn keep_begun(m: &mut Mono, y0: f32, s: &mut Stood) {
    let (hx, hz) = (m.w / 2.0 - 1.0, m.d / 2.0 - 1.0);
    // The foundation: one low course over the whole ground.
    m.block(Kind::Stone, [-hx, hx], [y0, 2.5], [-hz, hz]);
    // The back wall, partly raised: bays of different heights.
    let back = [-hz, -hz + 9.0];
    for (i, (x0, x1, h)) in [(-hx, -hx + 24.0, 21.0), (-hx + 24.0, -hx + 52.0, 15.0), (hx - 50.0, hx - 24.0, 18.0), (hx - 24.0, hx, 11.0)]
        .into_iter()
        .enumerate()
    {
        m.block(Kind::Stone, [x0, x1], [2.5, h], back);
        if i % 2 == 0 {
            m.block(Kind::Stone, [x0, x1 - 6.0], [h, h + 3.0], [back[0] + 1.0, back[1] - 1.0]);
        }
    }
    // The squat tower, half raised: a square body with a stepped, ragged top.
    let t = 34.0;
    let tx = [-hx + 8.0, -hx + 8.0 + t];
    let tz = [-hz + 8.0, -hz + 8.0 + t];
    m.block(Kind::Stone, [tx[0] - 2.0, tx[1] + 2.0], [2.5, 8.0], [tz[0] - 2.0, tz[1] + 2.0]);
    m.block(Kind::Stone, tx, [8.0, 50.0], tz);
    // The top course stops short: three raw lumps at different heights.
    for (x, z, w, d, h) in [(0.0, 0.0, 14.0, 12.0, 9.0), (16.0, 2.0, 10.0, 10.0, 5.0), (4.0, 18.0, 12.0, 8.0, 3.0)] {
        m.block(Kind::Stone, [tx[0] + x, tx[0] + x + w], [50.0, 50.0 + h], [tz[0] + z, tz[0] + z + d]);
    }
    // A slit and a dark door at its foot.
    m.block(Kind::Door, [tx[0] + t / 2.0 - 1.0, tx[0] + t / 2.0 + 1.0], [26.0, 36.0], [tz[1] - 0.2, tz[1] + 0.25]);
    m.block(Kind::Door, [tx[0] + t / 2.0 - 3.5, tx[0] + t / 2.0 + 3.5], [2.5, 15.0], [tz[1] - 0.2, tz[1] + 0.25]);
    // Scaffolding up the front and the east face: poles, planks, a ladder.
    let sc = [tx[1] + 3.0, tz[1] + 3.0];
    for (px, pz) in [(tx[0] - 3.0, sc[1]), (tx[1] + 3.0, sc[1]), (sc[0], tz[0] + 4.0)] {
        m.block(Kind::Wood, [px - 1.0, px + 1.0], [2.5, 62.0], [pz - 1.0, pz + 1.0]);
    }
    for y in [14.0, 28.0, 42.0] {
        m.block(Kind::Wood, [tx[0] - 3.5, sc[0] + 1.0], [y, y + 1.4], [sc[1] - 2.0, sc[1] + 2.0]);
        m.block(Kind::Wood, [sc[0] - 2.0, sc[0] + 2.0], [y, y + 1.4], [tz[0] + 2.0, sc[1] + 2.0]);
    }
    // The hoist on the tower's back: a mast and a boom out over the wall.
    m.block(Kind::Wood, [tx[0] + 4.0, tx[0] + 6.0], [50.0, 74.0], [tz[0] + 4.0, tz[0] + 6.0]);
    m.block(Kind::Wood, [tx[0] + 5.0, tx[0] + 22.0], [72.0, 73.6], [tz[0] + 4.5, tz[0] + 5.5]);
    // Its twin: a stump with a ragged top.
    let sx = [hx - 40.0, hx - 12.0];
    let sz = [-hz + 6.0, -hz + 34.0];
    m.block(Kind::Stone, [sx[0] - 1.5, sx[1] + 1.5], [2.5, 7.0], [sz[0] - 1.5, sz[1] + 1.5]);
    m.block(Kind::Stone, sx, [7.0, 24.0], sz);
    m.block(Kind::Stone, [sx[0], sx[0] + 12.0], [24.0, 29.0], [sz[0], sz[0] + 10.0]);
    m.block(Kind::Stone, [sx[0] + 14.0, sx[1]], [24.0, 27.0], [sz[0] + 12.0, sz[1]]);
    // The front: stubs either side of a gap for the gate, and a timber frame in the gap.
    let front = [hz - 9.0, hz];
    m.block(Kind::Stone, [-hx, -13.0], [2.5, 13.0], front);
    m.block(Kind::Stone, [-hx, -26.0], [13.0, 16.0], [front[0] + 1.0, front[1] - 1.0]);
    m.block(Kind::Stone, [13.0, hx], [2.5, 9.0], front);
    // The side stubs: a few courses.
    m.block(Kind::Stone, [-hx, -hx + 8.0], [2.5, 7.0], [-hz + 40.0, hz - 10.0]);
    m.block(Kind::Stone, [hx - 8.0, hx], [2.5, 5.0], [-hz + 36.0, hz - 22.0]);
    for x in [-11.0f32, 11.0] {
        m.block(Kind::Wood, [x - 1.5, x + 1.5], [2.5, 28.0], [front[1] - 5.0, front[1] - 2.0]);
    }
    m.block(Kind::Wood, [-14.0, 14.0], [28.0, 31.0], [front[1] - 5.5, front[1] - 1.5]);
    m.block(Kind::Door, [-9.5, 9.5], [2.5, 26.0], [front[1] - 3.2, front[1] - 3.0]);
    // A heap of cut stone before the right stub, and a few blocks by the gate.
    for (x, z, w, d, h) in [(hx - 26.0, hz - 22.0, 16.0, 12.0, 6.0), (hx - 22.0, hz - 20.0, 9.0, 7.0, 11.0), (hx - 12.0, hz - 18.0, 8.0, 6.0, 4.0), (-24.0, hz - 14.0, 7.0, 5.0, 4.5)] {
        m.block(Kind::Stone, [x, x + w], [2.5, 2.5 + h], [z, z + d]);
    }
    // The banner on its pole, over the tower: the Keep's own.
    let (bx, bz) = keep_pole(m.w, m.d);
    m.block(Kind::Wood, [bx - 0.8, bx + 0.8], [50.0, 88.0], [bz - 0.8, bz + 0.8]);
    m.block(Kind::Cloth, [bx + 0.8, bx + 15.0], [78.0, 88.0], [bz - 0.4, bz + 0.4]);
    // The brazier at the gate, burning for the night's work.
    let at = m.brazier(-18.0, hz - 2.0, 2.5);
    s.lamp(m, at, 56.0, 0.8, 3.0);
}

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
