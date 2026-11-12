//! The continuous line in blocks (the sim's `rollers`, the flat kit's `rollers`): a tile of
//! rollerway per piece and way, the goods that ride it, the beacon over the head of a jam; and, for
//! the rail network, the lever and point blade where the rails part and the bay's platform with its
//! board and lantern.
//!
//! Every piece is built facing east in the tile's own frame (16 across, `x` the way the goods go, `z`
//! across it, `y` up) and turned about the tile's middle to the way it faces, as the flat kit's `Pen`
//! turns its pixels. The rollers turn (a pale lug rides each log, four steps a turn); a tile that is
//! stood still stands still.
//!
//! Nothing here is per frame: a tile is made once per `(piece, way, step)` and the world's chunk of
//! them baked once for as long as its tiles are what they were (`world3d::hillside`); a good is one
//! small shared mesh moved to its slot.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use angel_stage::set::Kind;

use super::W;
use crate::kingdom::Piece;
use crate::render::overworld::works_kit::Load;
use crate::world3d::blockout::{Shape, Solid};

/// Where a good rides above the ground on a roller, a chain's trodden way and the raised line of a
/// trestle (pixels).
pub const ROLLER_TOP: f32 = 3.8;
pub const CHAIN_TOP: f32 = 0.7;
pub const TRESTLE_RISE: f32 = 4.8;

fn builder(step: u32) -> W {
    W { w: 16.0, d: 16.0, seed: 9, t: step, work: true, solids: Vec::new(), lamps: Vec::new() }
}

/// `solids` built facing east, turned about the tile's middle to face `dir` (0 north, 1 east, 2 south,
/// 3 west), as `rails::vehicle` turns a carrier about its own origin.
fn faced(mut solids: Vec<Solid>, dir: u8) -> Vec<Solid> {
    let turn = match dir % 4 {
        0 => 90.0f32,
        1 => 0.0,
        2 => 270.0,
        _ => 180.0,
    };
    if turn == 0.0 {
        return solids;
    }
    let (s, c) = turn.to_radians().sin_cos();
    let (s, c) = (s.round(), c.round());
    for solid in &mut solids {
        let (x, z) = (solid.at[0] - 8.0, solid.at[2] - 8.0);
        solid.at[0] = 8.0 + x * c + z * s;
        solid.at[2] = 8.0 - x * s + z * c;
        let square = matches!(solid.shape, Shape::Box | Shape::Cylinder | Shape::Sphere | Shape::Cone | Shape::Frustum | Shape::Gem | Shape::Rock(_) | Shape::Pyramid);
        if square && solid.turn == 0.0 {
            if (turn as i32 / 90) % 2 == 1 {
                solid.size.swap(0, 2);
            }
        } else {
            solid.turn = (solid.turn + turn).rem_euclid(360.0);
        }
    }
    solids
}

/// A bed of log rollers along `x` from `x0` to `x1` at `y0`, its side beams and legs, a lug riding
/// each log when `phase` is `Some` (the rollers turning). A bed along `z` is this one turned.
fn bed(b: &mut W, x0: f32, x1: f32, y0: f32, phase: Option<u8>) {
    for z in [2.2f32, 12.4] {
        b.bx(Kind::Wood, [x0, x1], [y0, y0 + 3.0], [z, z + 1.4]);
    }
    let n = (((x1 - x0) / 4.0).floor() as i32).max(1);
    for i in 0..n {
        let c = x0 + (i as f32 + 0.5) * (x1 - x0) / n as f32;
        b.bx(Kind::Floor, [c - 1.4, c + 1.4], [y0 + 1.0, y0 + 3.6], [3.4, 12.6]);
        // The lug: a pale strip across the log's top that rides round with it.
        let at = match phase {
            Some(p) => ((p as i32 + i) % 4) as f32 * 0.6 - 0.9,
            None => 0.0,
        };
        b.bx(Kind::Road, [c + at - 0.4, c + at + 0.4], [y0 + 3.6, y0 + 3.9], [3.8, 12.2]);
    }
    if y0 > 0.5 {
        return;
    }
    // Low legs under the beams at each end.
    for x in [x0 + 0.8, x1 - 0.8] {
        for z in [2.6f32, 12.8] {
            b.bx(Kind::Trunk, [x - 0.5, x + 0.5], [0.0, 1.0], [z, z + 0.8]);
        }
    }
}

/// A short stub of bed along `z` (a side way of a house), from `z0` to `z1`.
fn stub_z(b: &mut W, z0: f32, z1: f32) {
    for x in [5.6f32, 9.4] {
        b.bx(Kind::Wood, [x, x + 1.0], [0.0, 2.8], [z0, z1]);
    }
    b.bx(Kind::Floor, [6.6, 9.4], [1.0, 3.2], [z0, z1]);
    b.bx(Kind::Road, [7.4, 8.6], [3.2, 3.5], [z0 + 0.4, z1 - 0.4]);
}

/// A tile of rollerway of `piece` facing `dir`, built once per `(piece, way, phase, crewed)`: `phase`
/// is the step the rollers are at while the tile is moving goods, none when it stands; `crewed`
/// stands the hand-chain's labourer in his tile.
pub fn tile(piece: Piece, dir: u8, phase: Option<u8>, crewed: bool) -> Arc<Vec<Solid>> {
    static MADE: OnceLock<Mutex<HashMap<(Piece, u8, Option<u8>, bool), Arc<Vec<Solid>>>>> = OnceLock::new();
    let key = (piece, dir % 4, phase, crewed);
    let memory = MADE.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let made = Arc::new(faced(build(piece, phase, crewed), dir));
    if let Ok(mut m) = memory.lock() {
        m.insert(key, made.clone());
    }
    made
}

fn build(piece: Piece, phase: Option<u8>, crewed: bool) -> Vec<Solid> {
    let mut b = builder(u32::from(phase.unwrap_or(0)));
    match piece {
        Piece::Roller => bed(&mut b, 0.0, 16.0, 0.0, phase),
        Piece::Chain => {
            // A trodden way between two staked ropes: packed earth, a stake at each end and the middle.
            b.bx(Kind::Path, [0.0, 16.0], [0.0, CHAIN_TOP], [3.2, 12.8]);
            for x in [0.6f32, 8.0, 15.4] {
                for z in [3.0f32, 12.6] {
                    b.bx(Kind::Trunk, [x - 0.4, x + 0.4], [0.7, 3.8], [z, z + 0.8]);
                }
            }
            for z in [3.2f32, 12.6] {
                b.bx(Kind::Cloth, [0.0, 16.0], [3.0, 3.5], [z, z + 0.4]);
            }
            b.bx(Kind::Road, [11.0, 12.2], [CHAIN_TOP, CHAIN_TOP + 0.3], [7.4, 8.6]);
            if crewed {
                // The labourer standing in the way, passing goods hand to hand: his arms swing between the
                // two sides of it, a pace a step, and he bobs a pixel on the odd ones.
                let bob = if phase.is_some_and(|p| p % 2 == 1) { 0.5 } else { 0.0 };
                let swing = phase.is_some_and(|p| (p / 2) % 2 == 0);
                b.bx(Kind::Trunk, [6.8, 8.2], [0.7, 5.6], [6.6, 9.4]);
                b.bx(Kind::Cloth, [6.6, 8.6], [5.6 - bob, 10.6 - bob], [6.2, 9.8]);
                b.bx(Kind::Bone, [6.9, 8.3], [10.6 - bob, 13.0 - bob], [7.0, 9.0]);
                let (hi, lo) = if swing { (9.4, 7.4) } else { (7.4, 9.4) };
                b.bx(Kind::Cloth, [8.4, 11.8], [hi - bob, hi + 1.0 - bob], [7.6, 8.4]);
                b.bx(Kind::Cloth, [4.2, 7.6], [lo - bob, lo + 1.0 - bob], [7.6, 8.4]);
            } else {
                // Nobody at it: a stool by the way.
                b.bx(Kind::Floor, [7.0, 9.4], [CHAIN_TOP, CHAIN_TOP + 2.2], [6.6, 8.8]);
            }
        }
        Piece::Splitter | Piece::Merge | Piece::Gate => {
            // The ways in and out: stubs of bed, and the house of boards over their meeting.
            let (ins, outs): (&[&str], &[&str]) = match piece {
                Piece::Merge => (&["w", "n", "s"], &["e"]),
                _ => (&["w"], &["e", "n", "s"]),
            };
            for way in ins.iter().chain(outs.iter()) {
                match *way {
                    "w" => bed(&mut b, 0.0, 4.4, 0.0, None),
                    "e" => bed(&mut b, 11.6, 16.0, 0.0, None),
                    "n" => stub_z(&mut b, 0.0, 4.2),
                    _ => stub_z(&mut b, 11.8, 16.0),
                }
            }
            b.bx(Kind::Wood, [3.4, 12.6], [0.0, 5.4], [3.4, 12.6]);
            b.bx(Kind::Floor, [3.8, 12.2], [0.0, 5.0], [3.8, 12.2]);
            let (roof, rise) = match piece {
                Piece::Splitter => (Kind::Roof, 4.4),
                Piece::Merge => (Kind::Slate, 4.0),
                _ => (Kind::Thatch, 4.4),
            };
            b.gable(roof, [2.6, 13.4], [2.6, 13.4], 5.4, rise, false);
            match piece {
                Piece::Splitter => {
                    // A wedge on the ridge that parts the flow.
                    b.bx(Kind::Road, [7.0, 9.0], [5.4 + rise, 5.4 + rise + 1.2], [7.4, 8.6]);
                }
                Piece::Merge => {
                    b.bx(Kind::Road, [5.0, 6.0], [5.4 + rise - 0.6, 5.4 + rise + 0.8], [5.0, 11.0]);
                }
                _ => {
                    // The gate's flap and bar across the front way: front first.
                    b.bx(Kind::Road, [12.6, 13.6], [0.6, 4.6], [4.4, 11.6]);
                    b.bx(Kind::Metal, [10.8, 12.6], [4.2, 5.0], [4.6, 11.4]);
                }
            }
        }
        Piece::Trestle => {
            // The line under (a quarter turn to the right of the way it faces, which is south in this frame),
            // and over it on trestles the line that runs the way it faces.
            for x in [2.2f32, 12.4] {
                b.bx(Kind::Wood, [x, x + 1.4], [0.0, 3.0], [0.0, 16.0]);
            }
            for i in 0..4 {
                let c = 2.0 + i as f32 * 4.0;
                let at = match phase {
                    Some(p) => ((p as i32 + i) % 4) as f32 * 0.6 - 0.9,
                    None => 0.0,
                };
                b.bx(Kind::Floor, [3.6, 12.4], [1.0, 3.4], [c - 1.4, c + 1.4]);
                b.bx(Kind::Road, [4.0, 12.0], [3.4, 3.7], [c + at - 0.4, c + at + 0.4]);
            }
            // Four legs of the trestle and a cross-beam each side.
            for x in [0.8f32, 14.6] {
                for z in [2.0f32, 13.2] {
                    b.bx(Kind::Trunk, [x, x + 0.9], [0.0, TRESTLE_RISE + 1.0], [z, z + 0.9]);
                }
                b.bx(Kind::Trunk, [x, x + 0.9], [TRESTLE_RISE - 0.6, TRESTLE_RISE + 0.2], [2.0, 14.1]);
            }
            bed(&mut b, 0.0, 16.0, TRESTLE_RISE, phase);
        }
    }
    b.solids
}

/// The ink of a good: what it is made of, as the loads' heaps are (`works_kit::load_solids`).
fn kind_of(load: Load) -> Kind {
    match load {
        Load::Stone | Load::Rubble => Kind::Rock,
        Load::Ore | Load::Slate | Load::Coal => Kind::Slate,
        Load::Ashlar => Kind::Road,
        Load::Brick | Load::Clay => Kind::Brick,
        Load::Iron | Load::Lead | Load::Glass => Kind::Metal,
        Load::Planks => Kind::Sand,
        Load::Timber => Kind::Wood,
        Load::Hides => Kind::Path,
        Load::Cloth => Kind::Cloth,
        Load::Wool => Kind::Bone,
        Load::Lime | Load::Flour => Kind::Plaster,
        Load::Sand => Kind::Sand,
        Load::Mortar => Kind::Path,
        Load::Charcoal => Kind::Mud,
        Load::Grain => Kind::Crop,
        Load::Hemp => Kind::Hedge,
        Load::Rope | Load::Bread => Kind::Thatch,
        Load::Water => Kind::Water,
        Load::Ale | Load::Crate => Kind::Floor,
    }
}

/// A good riding the line, on its skid, centred on the origin: a crate of its colour, a pale lid, a
/// dark skid under it; long along `x` when `along_x`, else along `z`.
pub fn good(load: Load, along_x: bool) -> Arc<Vec<Solid>> {
    static MADE: OnceLock<Mutex<HashMap<(Load, bool), Arc<Vec<Solid>>>>> = OnceLock::new();
    let memory = MADE.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&(load, along_x)).cloned()) {
        return hit;
    }
    let (lx, lz) = if along_x { (2.7, 2.0) } else { (2.0, 2.7) };
    let mut solids = vec![
        Solid::block(Kind::Trunk, [-lx, -lz, lx, lz], 0.0, 0.5),
        Solid::block(kind_of(load), [-lx + 0.2, -lz + 0.2, lx - 0.2, lz - 0.2], 0.5, 3.0),
    ];
    // A sack or a barrel reads rounder than a block: its top corners cut by a lid a little smaller.
    solids.push(Solid::block(Kind::Bone, [-lx + 0.8, -lz + 0.8, lx - 0.8, lz - 0.8], 3.5, 0.4));
    let made = Arc::new(solids);
    if let Ok(mut m) = memory.lock() {
        m.insert((load, along_x), made.clone());
    }
    made
}

/// The beacon over the head of a jam, or the tag over a wagon held up for the track (`amber`): a post
/// with a lamp that blinks, as the flat view's red ring and amber tag do.
pub fn beacon(amber: bool, on: bool) -> Arc<Vec<Solid>> {
    static MADE: OnceLock<Mutex<HashMap<(bool, bool), Arc<Vec<Solid>>>>> = OnceLock::new();
    let memory = MADE.get_or_init(Default::default);
    if let Some(hit) = memory.lock().ok().and_then(|m| m.get(&(amber, on)).cloned()) {
        return hit;
    }
    let lamp = match (amber, on) {
        (_, false) => Kind::Door,
        (true, true) => Kind::Window,
        (false, true) => Kind::Fire,
    };
    let solids = vec![Solid::block(Kind::Trunk, [-0.5, -0.5, 0.5, 0.5], 0.0, 9.0), Solid::block(lamp, [-1.6, -1.6, 1.6, 1.6], 9.0, 3.2)];
    let made = Arc::new(solids);
    if let Ok(mut m) = memory.lock() {
        m.insert((amber, on), made.clone());
    }
    made
}

/// The way a rail tile's lever leans: 0 north, 1 east, 2 south, 3 west, or none for a lever standing
/// upright.
pub type Throw = Option<u8>;

/// What the rails add to a tile of track: a switch's lever and point blade, a bay's platform with its
/// board and lantern. Built facing east and turned like the pieces; the lever and blade lean toward the
/// way thrown.
pub fn rail_extras(switch: bool, throw: Throw, bay: Option<(u8, bool, u8)>) -> Vec<Solid> {
    let mut solids = Vec::new();
    if switch {
        let mut b = builder(0);
        // The stand beside the rails where they part (the tile's far corner), the post and the lever.
        b.bx(Kind::Stone, [12.2, 15.0], [0.0, 1.2], [12.2, 15.0]);
        b.bx(Kind::Floor, [12.9, 14.3], [1.2, 6.0], [12.9, 14.3]);
        let (dx, dz) = match throw {
            Some(0) => (0.0, -1.0f32),
            Some(1) => (1.0, 0.0),
            Some(2) => (0.0, 1.0),
            Some(3) => (-1.0, 0.0),
            _ => (0.0, 0.0),
        };
        let tip = [13.6 + dx * 3.0, 9.0, 13.6 + dz * 3.0];
        let n = 5;
        for k in 0..=n {
            let t = k as f32 / n as f32;
            let p = [13.6 + (tip[0] - 13.6) * t, 6.0 + (tip[1] - 6.0) * t, 13.6 + (tip[2] - 13.6) * t];
            b.bx(Kind::Road, [p[0] - 0.6, p[0] + 0.6], [p[1] - 0.6, p[1] + 0.6], [p[2] - 0.6, p[2] + 0.6]);
        }
        b.bx(Kind::Bone, [tip[0] - 1.3, tip[0] + 1.3], [tip[1], tip[1] + 2.0], [tip[2] - 1.3, tip[2] + 1.3]);
        // The point blade: a pale rail laid from the middle of the tile toward the way thrown, over the rail.
        if throw.is_some() {
            let (cx, cz) = (8.0 + dx * 3.2, 8.0 + dz * 3.2);
            let (hx, hz) = if dx != 0.0 { (2.8, 0.6) } else { (0.6, 2.8) };
            let side = if dx != 0.0 { (0.0, 2.6) } else { (2.6, 0.0) };
            b.bx(Kind::Bone, [cx + side.0 - hx, cx + side.0 + hx], [3.4, 4.6], [cz + side.1 - hz - 0.3, cz + side.1 + hz + 0.3]);
        }
        solids.extend(b.solids);
    }
    if let Some((side, held, tag)) = bay {
        solids.extend(faced(platform(held, tag), side));
    }
    solids
}

/// A bay's platform in the east-facing frame (the building to the east): a deck of boards along the
/// building's side of the rail, a post at the north end with a board marked by the bay's `tag` and a
/// lantern that is lit while a wagon works the bay.
fn platform(held: bool, tag: u8) -> Vec<Solid> {
    let mut b = builder(0);
    b.bx(Kind::Wood, [10.2, 16.0], [0.0, 2.4], [1.6, 14.4]);
    b.bx(Kind::Floor, [10.2, 16.0], [2.4, 3.0], [1.6, 14.4]);
    for z in [4.0f32, 7.0, 10.0, 13.0] {
        b.bx(Kind::Trunk, [10.2, 16.0], [3.0, 3.2], [z, z + 0.4]);
    }
    // The post at the north end, the board on it, marks on the board by the bay's name.
    b.bx(Kind::Trunk, [10.4, 11.4], [3.0, 11.0], [1.8, 2.8]);
    b.bx(Kind::Floor, [10.2, 14.2], [7.0, 10.4], [1.4, 3.2]);
    for k in 0..3 {
        let on = tag >> k & 1 == 1;
        b.bx(if on { Kind::Bone } else { Kind::Door }, [10.8 + k as f32 * 1.2, 11.8 + k as f32 * 1.2], [7.6, 9.8], [3.2, 3.5]);
    }
    b.bx(if held { Kind::Window } else { Kind::Trunk }, [10.2, 11.6], [11.0, 12.8], [1.6, 3.0]);
    b.solids
}
