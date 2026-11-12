//! Her limbs, for pointing: her own left arm with its hand pointing (the
//! index finger out, the others and the thumb curled to the palm), and
//! sometimes her left leg with its foot pointed, toe first. Each is cut
//! from the avatar's body by the joint that pulls each vertex most, posed
//! on the CPU once, and laid out along +X from its root (the shoulder, the
//! hip: `x = 0`) to its tip, so it can be drawn stretched to any length,
//! hand and foot kept true, its root dissolving into nothing.

use angel_stage::scene::{
    Mat4, MeshData, mat_point, quat_axis_angle, quat_mul, quat_normalize, skin_matrix,
};

use crate::glb::{GPrimitive, Gltf};
use crate::model::{Joint, rest_worlds};

/// Which limb.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LimbKind {
    Arm,
    Leg,
}

/// A limb, posed, at rest length, in its own frame: +X from root to tip,
/// +Y up across it (the thumb's side, the front of the leg), +Z toward the
/// eye.
#[derive(Clone, Debug)]
pub struct Limb {
    pub kind: LimbKind,
    pub mesh: MeshData,
    /// Where its stretchable length ends (the wrist, the ankle), and where
    /// its tip is (the index fingertip, the toe), along X.
    pub reach: f32,
    pub tip: f32,
    /// The farthest it reaches across X: half the strip it needs.
    pub radius: f32,
}

/// How long a stretch of the root dissolves (meters).
const ROOT_FADE: f32 = 0.09;

/// The joints of each limb, root first, and its wrist (or ankle) and tip.
struct Chain {
    joints: &'static [&'static str],
    root: &'static str,
    end: &'static str,
}

const ARM: Chain = Chain {
    joints: &[
        "LeftArm",
        "LeftForeArm",
        "LeftHand",
        "LeftHandIndex1",
        "LeftHandIndex2",
        "LeftHandIndex3",
        "LeftHandMiddle1",
        "LeftHandMiddle2",
        "LeftHandMiddle3",
        "LeftHandRing1",
        "LeftHandRing2",
        "LeftHandRing3",
        "LeftHandPinky1",
        "LeftHandPinky2",
        "LeftHandPinky3",
        "LeftHandThumb1",
        "LeftHandThumb2",
        "LeftHandThumb3",
    ],
    root: "LeftArm",
    end: "LeftHand",
};

const LEG: Chain = Chain {
    joints: &["LeftUpLeg", "LeftLeg", "LeftFoot", "LeftToeBase"],
    root: "LeftUpLeg",
    end: "LeftFoot",
};

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}
/// `v` with its part along `along` (a unit vector) taken out, made unit.
fn across(v: [f32; 3], along: [f32; 3]) -> [f32; 3] {
    let k = dot(v, along);
    unit([
        v[0] - along[0] * k,
        v[1] - along[1] * k,
        v[2] - along[2] * k,
    ])
}
fn turn3(m: &Mat4, d: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * d[0] + m[1][0] * d[1] + m[2][0] * d[2],
        m[0][1] * d[0] + m[1][1] * d[1] + m[2][1] * d[2],
        m[0][2] * d[0] + m[1][2] * d[1] + m[2][2] * d[2],
    ]
}
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A joint's world rotation at rest: its chain's rotations composed.
fn rest_rotation(joints: &[Option<Joint>], mut i: usize) -> [f32; 4] {
    let mut chain = Vec::new();
    while let Some(j) = joints.get(i).and_then(|j| j.as_ref()) {
        chain.push(j.rest.rotation);
        match j.parent {
            Some(p) if chain.len() < 256 => i = p,
            _ => break,
        }
    }
    chain.iter().rev().fold([0.0, 0.0, 0.0, 1.0], |acc, &q| {
        quat_normalize(quat_mul(acc, quat_normalize(q)))
    })
}

/// Cut `kind` from the avatar's meshes (`sources`: the body, and what it
/// wears over the parts the body leaves out: MPFB deletes the skin under
/// clothes, so a leg is the trousers' and a foot the body's).
pub fn cut(
    file: &Gltf,
    sources: &[&GPrimitive],
    joints: &[Option<Joint>],
    skin_joints: &[usize],
    inverse_bind: &[Mat4],
    kind: LimbKind,
) -> Option<Limb> {
    let chain = match kind {
        LimbKind::Arm => &ARM,
        LimbKind::Leg => &LEG,
    };
    let node = |name: &str| file.node(name);
    let rest = rest_worlds(joints);
    let at = |name: &str| -> Option<[f32; 3]> {
        rest.get(node(name)?)?.map(|m| [m[3][0], m[3][1], m[3][2]])
    };
    // Its pose: each turn a world-axis rotation in the rest frame, the
    // joint's local rotation then conj(W_parent) * turn * W_joint (turns so
    // made compose down a chain as they should).
    let mut turns: Vec<(usize, [f32; 4])> = Vec::new();
    // Turn joint `name` by `angle` so its bone (toward `next`) swings
    // toward `toward`.
    let curl =
        |name: &str, toward: [f32; 3], angle: f32, next: &str| -> Option<(usize, [f32; 4])> {
            let dir = unit(sub(at(next)?, at(name)?));
            let axis = unit(cross(dir, toward));
            Some((node(name)?, quat_axis_angle(axis, angle)))
        };
    match kind {
        LimbKind::Arm => {
            let hand = at("LeftHand")?;
            let d = unit(sub(at("LeftHandMiddle1")?, hand));
            let span = unit(sub(at("LeftHandIndex1")?, at("LeftHandPinky1")?));
            let mut palm = unit(cross(d, span));
            if dot(sub(at("LeftHandThumb2")?, hand), palm) < 0.0 {
                palm = [-palm[0], -palm[1], -palm[2]];
            }
            for (finger, angles) in [
                ("Middle", [1.25, 1.45, 1.05]),
                ("Ring", [1.3, 1.5, 1.05]),
                ("Pinky", [1.35, 1.45, 1.0]),
            ] {
                let names = [
                    format!("LeftHand{finger}1"),
                    format!("LeftHand{finger}2"),
                    format!("LeftHand{finger}3"),
                ];
                turns.extend(curl(&names[0], palm, angles[0], &names[1]));
                turns.extend(curl(&names[1], palm, angles[1], &names[2]));
                // The last joint has no child: it bends as the one before it.
                let dir = unit(sub(at(&names[2])?, at(&names[1])?));
                turns.push((
                    node(&names[2])?,
                    quat_axis_angle(unit(cross(dir, palm)), angles[2]),
                ));
            }
            turns.extend(curl("LeftHandIndex1", palm, 0.06, "LeftHandIndex2"));
            // The wrist turned so the pointing finger runs on along the arm.
            let finger = unit(sub(at("LeftHandIndex3")?, at("LeftHandIndex1")?));
            let arm = unit(sub(hand, at("LeftArm")?));
            let bend = dot(finger, arm).clamp(-1.0, 1.0).acos();
            if bend > 1e-3 {
                turns.push((node("LeftHand")?, quat_axis_angle(unit(cross(finger, arm)), bend)));
            }
            turns.extend(curl("LeftHandThumb1", palm, 0.35, "LeftHandThumb2"));
            turns.extend(curl("LeftHandThumb2", palm, 0.55, "LeftHandThumb3"));
        }
        LimbKind::Leg => {
            let down = unit(sub(at("LeftFoot")?, at("LeftLeg")?));
            turns.extend(curl("LeftFoot", down, 1.0, "LeftToeBase"));
            let dir = unit(sub(at("LeftToeBase")?, at("LeftFoot")?));
            turns.push((
                node("LeftToeBase")?,
                quat_axis_angle(unit(cross(dir, down)), 0.35),
            ));
        }
    }
    let mut posed: Vec<Option<Joint>> = joints.to_vec();
    for (i, turn) in turns {
        let Some(j) = posed[i].as_mut() else { continue };
        let wp = j
            .parent
            .map_or([0.0, 0.0, 0.0, 1.0], |p| rest_rotation(joints, p));
        let wj = rest_rotation(joints, i);
        j.rest.rotation = quat_normalize(quat_mul(
            quat_mul([-wp[0], -wp[1], -wp[2], wp[3]], turn),
            wj,
        ));
    }
    let worlds = rest_worlds(&posed);
    let palette: Vec<Mat4> = skin_joints
        .iter()
        .zip(inverse_bind)
        .map(|(&j, ib)| {
            worlds
                .get(j)
                .copied()
                .flatten()
                .map_or(angel_stage::scene::IDENTITY_MATRIX, |w| {
                    angel_stage::scene::mat_mul(&w, ib)
                })
        })
        .collect();
    let posed_at = |name: &str| -> Option<[f32; 3]> {
        worlds
            .get(node(name)?)?
            .map(|m| [m[3][0], m[3][1], m[3][2]])
    };

    // Its vertices: those the limb's joints pull most.
    let members: Vec<usize> = chain.joints.iter().filter_map(|n| node(n)).collect();
    let mine = |body: &GPrimitive, v: usize| -> bool {
        let (j, w) = (body.joints.get(v), body.weights.get(v));
        let (Some(j), Some(w)) = (j, w) else {
            return false;
        };
        let best = (0..4).max_by(|&a, &b| w[a].total_cmp(&w[b])).unwrap_or(0);
        skin_joints
            .get(j[best] as usize)
            .is_some_and(|n| members.contains(n))
    };

    // Its frame, posed: X from root to wrist (ankle), Y up across it.
    let root = posed_at(chain.root)?;
    let x = unit(sub(posed_at(chain.end)?, root));
    let y = match kind {
        LimbKind::Arm => across(sub(posed_at("LeftHandThumb2")?, posed_at("LeftHand")?), x),
        LimbKind::Leg => across([0.0, 0.0, 1.0], x),
    };
    let z = cross(x, y);
    let frame = |p: [f32; 3]| {
        let d = sub(p, root);
        [dot(d, x), dot(d, y), dot(d, z)]
    };
    let frame_dir = |d: [f32; 3]| unit([dot(d, x), dot(d, y), dot(d, z)]);

    let mut mesh = MeshData::default();
    let mut solid = Vec::new();
    let mut band = Vec::new();
    let mut tris: Vec<(f32, [u32; 3])> = Vec::new();
    for body in sources {
        let mut remap = vec![u32::MAX; body.positions.len()];
        for tri in body.indices.chunks_exact(3) {
            if !tri.iter().all(|&v| mine(body, v as usize)) {
                continue;
            }
            let mut at_x = [0.0f32; 3];
            for (k, &v) in tri.iter().enumerate() {
                let v = v as usize;
                if remap[v] == u32::MAX {
                    let m = skin_matrix(
                        &palette,
                        body.joints[v],
                        body.weights[v],
                        &angel_stage::scene::IDENTITY_MATRIX,
                    );
                    let p = frame(mat_point(&m, body.positions[v]));
                    let n = frame_dir(turn3(
                        &m,
                        body.normals.get(v).copied().unwrap_or([0.0, 0.0, 1.0]),
                    ));
                    remap[v] = mesh.positions.len() as u32;
                    mesh.positions.push(p);
                    mesh.normals.push(n);
                    mesh.colors.push([1.0; 4]);
                }
                at_x[k] = mesh.positions[remap[v] as usize][0];
            }
            let low = at_x.iter().copied().fold(f32::INFINITY, f32::min);
            tris.push((
                low,
                [
                    remap[tri[0] as usize],
                    remap[tri[1] as usize],
                    remap[tri[2] as usize],
                ],
            ));
        }
    }
    // Its root dissolves from where its mesh begins.
    let start = mesh
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min)
        .max(-0.05);
    for (c, p) in mesh.colors.iter_mut().zip(&mesh.positions) {
        c[3] = smooth((p[0] - start) / ROOT_FADE);
    }
    for (low, ids) in tris {
        if low < start + ROOT_FADE {
            band.extend(ids)
        } else {
            solid.extend(ids)
        }
    }
    if solid.is_empty() {
        eprintln!("angelface: her {kind:?} has no triangles of its own");
        return None;
    }
    // The whole of it, then the band that dissolves (when it has one).
    mesh.groups = vec![(0, solid.len() as u32)];
    if !band.is_empty() {
        mesh.groups.push((solid.len() as u32, band.len() as u32));
    }
    mesh.indices = solid.into_iter().chain(band).collect();
    let reach = dot(sub(posed_at(chain.end)?, root), x);
    let tip = mesh
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let radius = mesh
        .positions
        .iter()
        .map(|p| p[1].abs().max(p[2].abs()))
        .fold(0.0f32, f32::max);
    if let Err(why) = mesh.validate() {
        eprintln!("angelface: her {kind:?} could not be cut: {why}");
        return None;
    }
    Some(Limb {
        kind,
        mesh,
        reach,
        tip,
        radius,
    })
}

impl Limb {
    /// The limb stretched so its tip lies at `tip` (meters from its root):
    /// its length between root and wrist (ankle) drawn out, the hand
    /// (foot) beyond kept as it is. Never shorter than at rest.
    pub fn stretched(&self, tip: f32) -> MeshData {
        let k = (1.0 + (tip - self.tip) / self.reach.max(1e-3)).max(1.0);
        let mut mesh = self.mesh.clone();
        for (p, n) in mesh.positions.iter_mut().zip(mesh.normals.iter_mut()) {
            if p[0] <= 0.0 {
                continue;
            }
            if p[0] <= self.reach {
                p[0] *= k;
                *n = unit([n[0] / k, n[1], n[2]]);
            } else {
                p[0] += (k - 1.0) * self.reach;
            }
        }
        mesh
    }

    /// Its tip once stretched to `tip`: `tip`, or its rest tip if longer.
    pub fn tip_at(&self, tip: f32) -> f32 {
        tip.max(self.tip)
    }
}
