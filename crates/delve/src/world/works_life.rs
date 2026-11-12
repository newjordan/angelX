//! The site is alive (the Works, `world::works`): where the masons and the
//! hodmen are, how the crane stands and turns with a stone on its rope, the
//! dust where a stone just landed, the mason's marks. All pure functions of
//! the castle, how many stones stand and the time, in the castle's own frame
//! (metres, `x` east, `y` north, `z` up), so the 3D view and the flat view
//! show the same crew doing the same work.

use super::works::{Castle, Work};

/// Where the cutting benches stand in the bailey's south end, west of the
/// gate's mouth: the masons tap at them.
pub const BANKERS: [[f32; 2]; 3] = [[-8.6, -11.6], [-6.6, -11.6], [-4.6, -11.6]];

/// Where cut stone is stacked before it goes up: on the open ground west of
/// the keep, south of the crane's foot (the tower's own corner stands where it
/// was first drawn, a pocket no one can walk to).
pub const YARD: [f32; 2] = [-12.2, -2.4];

/// The crane's jib, metres.
const JIB: f32 = 4.4;

/// Which kind of working folk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Taps a block at a banker.
    Mason,
    /// Carries a hod of stone or mortar from the yard to the crane's foot and back.
    Hodman,
}

/// One of the crew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Folk {
    pub kind: Kind,
    /// Where they stand, metres.
    pub at: [f32; 2],
    /// Facing east (else west) on the screen.
    pub east: bool,
    /// The step or the swing they are on: a whole number that moves.
    pub frame: u32,
    /// A hodman loaded.
    pub carrying: bool,
}

/// The crew with `laid` stones standing at time `seconds`; none once the
/// castle stands or before it is begun.
pub fn folk(castle: &Castle, laid: u32, seconds: f32) -> Vec<Folk> {
    if laid >= castle.len() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (k, &at) in BANKERS.iter().enumerate() {
        // Each taps to their own beat, stood a little back from the bench.
        let beat = ((seconds * 3.0 + k as f32 * 0.37) as u32) % 2;
        out.push(Folk { kind: Kind::Mason, at: [at[0], at[1] - 0.9], east: k % 2 == 0, frame: beat, carrying: false });
    }
    // The hodmen walk the yard to the crane's foot and back, each in their
    // own time; loaded going, empty returning.
    let Some(c) = crane(castle, laid, seconds) else { return out };
    let foot = [c.at[0], c.at[1] - 1.4];
    for k in 0..3 {
        let phase = (seconds * 0.16 + k as f32 / 3.0).fract();
        let (to, t) = if phase < 0.5 { (true, phase * 2.0) } else { (false, (phase - 0.5) * 2.0) };
        let s = if to { t } else { 1.0 - t };
        let at = [YARD[0] + (foot[0] - YARD[0]) * s + (k as f32 - 1.0) * 0.5, YARD[1] + (foot[1] - YARD[1]) * s];
        out.push(Folk { kind: Kind::Hodman, at, east: to == (foot[0] >= YARD[0]), frame: (seconds * 4.0) as u32 + k, carrying: to });
    }
    out
}

/// The crane as it stands and turns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crane {
    /// The mast's foot, metres.
    pub at: [f32; 2],
    /// The mast's top, metres up.
    pub top: f32,
    /// The jib's turn: radians, `0` east, counter-clockwise from above.
    pub yaw: f32,
    /// How far the jib reaches out, metres.
    pub reach: f32,
    /// Where the stone hangs: the jib's tip, and how far down it hangs.
    pub rope: f32,
    /// The stone's middle, metres, on its rope.
    pub stone: [f32; 3],
    /// How far the treadwheel has turned, radians.
    pub wheel: f32,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The crane of the lift being laid, with a stone on its rope turning from
/// the yard to the place the last stone went, at time `seconds`.
pub fn crane(castle: &Castle, laid: u32, seconds: f32) -> Option<Crane> {
    let f = castle.falsework.iter().find(|f| f.kind == Work::Crane && f.appears <= laid && laid < f.gone)?;
    let [x0, y0, _, x1, y1, z1] = f.bx.map(|v| f32::from(v) / 100.0);
    let at = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
    let top = z1 + 0.9;
    // Where the last stone went: the middle of it, metres.
    let aim = if laid > 0 {
        let c = castle.stones[(laid - 1) as usize].corners();
        [0, 1, 2].map(|k| c.iter().map(|p| p[k]).sum::<f32>() / 8.0)
    } else {
        [at[0], at[1] + 3.0, 0.3]
    };
    let to_yard = [YARD[0] - at[0], YARD[1] - at[1]];
    let to_aim = [aim[0] - at[0], aim[1] - at[1]];
    let yaw_yard = to_yard[1].atan2(to_yard[0]);
    let mut yaw_aim = to_aim[1].atan2(to_aim[0]);
    // Turn the short way.
    while yaw_aim - yaw_yard > std::f32::consts::PI {
        yaw_aim -= std::f32::consts::TAU;
    }
    while yaw_aim - yaw_yard < -std::f32::consts::PI {
        yaw_aim += std::f32::consts::TAU;
    }
    let p = (seconds / 9.0).fract();
    // The cycle: stone made fast in the yard; hoisted; the jib turns; lowered
    // onto the wall; the jib comes back empty.
    let (yaw, rise, lower) = if p < 0.18 {
        (yaw_yard, 0.0, 0.0)
    } else if p < 0.42 {
        (yaw_yard, smooth((p - 0.18) / 0.24), 0.0)
    } else if p < 0.62 {
        (yaw_yard + (yaw_aim - yaw_yard) * smooth((p - 0.42) / 0.20), 1.0, 0.0)
    } else if p < 0.78 {
        (yaw_aim, 1.0, smooth((p - 0.62) / 0.16))
    } else {
        (yaw_aim + (yaw_yard - yaw_aim) * smooth((p - 0.78) / 0.22), 1.0, 1.0)
    };
    let dist = (to_aim[0].hypot(to_aim[1])).min(JIB);
    let reach = if p < 0.62 { JIB.min(to_yard[0].hypot(to_yard[1])).max(2.0) } else { dist.max(2.0) };
    // The stone's height: on the ground in the yard, up under the jib,
    // down to where the last stone went.
    let hang = top - 0.7;
    let stone_z = if lower > 0.0 && p < 0.78 { hang + (aim[2] + 0.3 - hang) * lower } else if p >= 0.78 { hang } else { 0.3 + (hang - 0.3) * rise };
    // On the empty return the hook is up and no stone hangs on it: the stone
    // is shown as the next, in the yard, ready.
    let tip = [at[0] + reach * yaw.cos(), at[1] + reach * yaw.sin()];
    let stone = if p >= 0.78 { [YARD[0], YARD[1], 0.3] } else { [tip[0], tip[1], stone_z] };
    Some(Crane { at, top, yaw, reach, rope: top - stone_z, stone, wheel: seconds * 1.4 })
}

/// A puff of dust where a stone just landed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puff {
    /// Where, metres.
    pub at: [f32; 3],
    /// How wide, metres.
    pub radius: f32,
    /// How thick, 0 to 1.
    pub thick: f32,
}

/// The dust of the last few stones laid, each a puff that spreads and thins
/// as the stones set after it pile up.
pub fn dust(castle: &Castle, laid: u32) -> Vec<Puff> {
    let mut out = Vec::new();
    let span = 10u32;
    for back in 0..span.min(laid) {
        let i = laid - 1 - back;
        let c = castle.stones[i as usize].corners();
        let mid = [0, 1, 2].map(|k| c.iter().map(|p| p[k]).sum::<f32>() / 8.0);
        let top = c.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        let age = back as f32 / span as f32;
        out.push(Puff { at: [mid[0], mid[1], top + 0.1 + 0.5 * age], radius: 0.35 + 0.9 * age, thick: (1.0 - age) * 0.55 });
    }
    out
}

/// A mason's mark: one of a few cut glyphs, by who set the stone. Each is
/// strokes on a three by three grid, `(x0, y0, x1, y1)` in thirds.
pub fn mark_strokes(seat: u8) -> &'static [(f32, f32, f32, f32)] {
    const MARKS: [&[(f32, f32, f32, f32)]; 6] = [
        // The realm's own masons: a plain cross.
        &[(0.5, 0.1, 0.5, 0.9), (0.1, 0.5, 0.9, 0.5)],
        // A V.
        &[(0.1, 0.9, 0.5, 0.1), (0.5, 0.1, 0.9, 0.9)],
        // A triangle's two sides and its foot.
        &[(0.1, 0.9, 0.9, 0.9), (0.1, 0.9, 0.5, 0.1), (0.5, 0.1, 0.9, 0.9)],
        // A Z.
        &[(0.1, 0.1, 0.9, 0.1), (0.9, 0.1, 0.1, 0.9), (0.1, 0.9, 0.9, 0.9)],
        // A bar and a dot's stem.
        &[(0.5, 0.1, 0.5, 0.9), (0.2, 0.9, 0.8, 0.9)],
        // A diamond's slant.
        &[(0.1, 0.5, 0.5, 0.1), (0.5, 0.1, 0.9, 0.5), (0.9, 0.5, 0.5, 0.9)],
    ];
    MARKS[usize::from(seat) % MARKS.len()]
}

/// The first course's stones of stage `stage`, the places its pegs go: the
/// lowest-set stones of the lift, corners and every few along its runs.
pub fn peg_places(castle: &Castle, stage: usize) -> Vec<[f32; 2]> {
    let Some(s) = castle.stages.get(stage).filter(|s| s.count > 0) else { return Vec::new() };
    let range = s.first..(s.first + s.count).min(castle.len());
    let low = range
        .clone()
        .map(|i| castle.stones[i as usize].aabb()[2])
        .min()
        .unwrap_or(0);
    let mut out = Vec::new();
    for i in range {
        let b = castle.stones[i as usize].aabb();
        if b[2] != low {
            continue;
        }
        // The corner of each course-one stone, so the line of the wall is pegged.
        if i % 4 == 0 {
            out.push([f32::from(b[0]) / 100.0, f32::from(b[1]) / 100.0]);
        }
    }
    out
}
