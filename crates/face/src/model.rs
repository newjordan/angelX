//! The goddess as glass draws her, cut from a rigged avatar (a `.glb` with
//! one skin and ARKit blendshapes): its skeleton, and the parts of her head.
//!
//! - **Skin**: the head and neck of the body mesh, skinned. Its triangles
//!   below the neck's foot are dropped; a band of the neck fades to nothing
//!   through its vertices' alpha (its own group, drawn blended), so she ends
//!   in light, not in a cut.
//! - **Eyes**: rigid, hung from the head joint (their weights split half
//!   and half between the head and the eye joints, the ARKit way: gaze is
//!   the `eyeLook*` shapes, not the joints). Three groups by their picture's
//!   layout: the white, the iris and the pupil, so the iris can glow.
//! - **Lashes, teeth, tongue**: skinned, as authored.
//!
//! Hair, brows and clothes are left out. Blendshapes are applied here on
//! the CPU ([`Part::shaped`]); a part is sent again only when its weights
//! change, which the GPU core takes in place (`Core::upload_mesh`).

use std::collections::HashMap;

use angel_stage::scene::{Mat4, MeshData, Transform, quat_mul, quat_normalize};

use crate::glb::{self, GPrimitive, Gltf};

pub const ARKIT: [&str; 52] = [
    "eyeBlinkLeft",
    "eyeLookDownLeft",
    "eyeLookInLeft",
    "eyeLookOutLeft",
    "eyeLookUpLeft",
    "eyeSquintLeft",
    "eyeWideLeft",
    "eyeBlinkRight",
    "eyeLookDownRight",
    "eyeLookInRight",
    "eyeLookOutRight",
    "eyeLookUpRight",
    "eyeSquintRight",
    "eyeWideRight",
    "jawForward",
    "jawLeft",
    "jawRight",
    "jawOpen",
    "mouthClose",
    "mouthFunnel",
    "mouthPucker",
    "mouthLeft",
    "mouthRight",
    "mouthSmileLeft",
    "mouthSmileRight",
    "mouthFrownLeft",
    "mouthFrownRight",
    "mouthDimpleLeft",
    "mouthDimpleRight",
    "mouthStretchLeft",
    "mouthStretchRight",
    "mouthRollLower",
    "mouthRollUpper",
    "mouthShrugLower",
    "mouthShrugUpper",
    "mouthPressLeft",
    "mouthPressRight",
    "mouthLowerDownLeft",
    "mouthLowerDownRight",
    "mouthUpperUpLeft",
    "mouthUpperUpRight",
    "browDownLeft",
    "browDownRight",
    "browInnerUp",
    "browOuterUpLeft",
    "browOuterUpRight",
    "cheekPuff",
    "cheekSquintLeft",
    "cheekSquintRight",
    "noseSneerLeft",
    "noseSneerRight",
    "tongueOut",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PartKind {
    Skin,
    Eyes,
    Lashes,
    Brows,
    Teeth,
    Tongue,
}

impl PartKind {
    fn material(self) -> &'static str {
        match self {
            PartKind::Skin => "Human.body",
            PartKind::Eyes => "Human.high-poly",
            PartKind::Lashes => "Human.mindfront_eyelashes_02",
            PartKind::Brows => "Human.mindfront_eyebrows_02",
            PartKind::Teeth => "Human.teeth_base",
            PartKind::Tongue => "Human.tongue01",
        }
    }

    pub const ALL: [PartKind; 6] = [
        PartKind::Skin,
        PartKind::Eyes,
        PartKind::Lashes,
        PartKind::Brows,
        PartKind::Teeth,
        PartKind::Tongue,
    ];
}

pub const SKIN_SOLID: usize = 0;
pub const SKIN_LIPS: usize = 1;
pub const SKIN_FADE: usize = 2;
pub const EYE_WHITE: usize = 0;
pub const EYE_IRIS: usize = 1;
pub const EYE_PUPIL: usize = 2;

#[derive(Clone, Debug)]
pub struct Target {
    pub channel: usize,
    pub deltas: Vec<(u32, [f32; 3], [f32; 3])>,
}

#[derive(Clone, Debug)]
pub struct Part {
    pub kind: PartKind,
    pub mesh: MeshData,
    pub targets: Vec<Target>,
    pub rigid_to: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub rest: Transform,
}

#[derive(Clone, Debug)]
pub struct Goddess {
    pub joints: Vec<Option<Joint>>,
    pub skin_joints: Vec<usize>,
    pub inverse_bind: Vec<Mat4>,
    pub parts: Vec<Part>,
    pub channels: Vec<String>,
    pub ponytail: Vec<usize>,
    pub into_head: Mat4,
    pub eye_centres: [[f32; 3]; 2],
    pub mouth: [f32; 3],
    pub forward: [f32; 3],
    pub forehead: [f32; 3],
    pub forehead_out: [f32; 3],
    pub arm: Option<crate::limb::Limb>,
    pub leg: Option<crate::limb::Limb>,
    pub neck: usize,
    pub head: usize,
    pub chest: usize,
    pub eyes_at: [f32; 3],
    pub head_at: [f32; 3],
    pub neck_at: [f32; 3],
}

const CUT_BELOW_NECK: f32 = 0.035;
const FADE_ABOVE_NECK: f32 = 0.085;

const MOUTH_BELOW: std::ops::Range<f32> = 0.05..0.105;
const MOUTH_HALF_WIDTH: f32 = 0.034;
const MOUTH_BEHIND: f32 = 0.03;
const LIP_RED: f32 = 0.42;

const SCULPT_NARROW: f32 = 0.085;
const SCULPT_SPAN: f32 = 0.065;
const CHEEKBONES_BELOW_EYES: f32 = 0.04;

const IRIS_CENTRES: [[f32; 2]; 2] = [[0.293, 0.707], [0.707, 0.293]];
const IRIS_RADIUS: f32 = 0.118;
const PUPIL_RADIUS: f32 = 0.045;

impl Goddess {
    pub fn from_glb(bytes: &[u8]) -> Result<Self, String> {
        let file = glb::read(bytes)?;
        Self::from_gltf(&file)
    }

    pub fn from_gltf(file: &Gltf) -> Result<Self, String> {
        let skin_node = file
            .nodes
            .iter()
            .find(|n| n.skin.is_some())
            .ok_or("no skinned mesh")?;
        let skin = file
            .skins
            .get(skin_node.skin.unwrap_or(0))
            .ok_or("a node's skin is not in the file")?;
        let parents = file.parents();
        let mesh_nodes: Vec<bool> = file.nodes.iter().map(|n| n.mesh.is_some()).collect();
        let joints: Vec<Option<Joint>> = file
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (!mesh_nodes[i]).then(|| Joint {
                    name: n.name.clone(),
                    parent: parents[i],
                    rest: Transform {
                        translation: n.translation,
                        rotation: n.rotation,
                        scale: n.scale,
                    },
                })
            })
            .collect();
        let find = |name: &str| {
            file.node(name)
                .ok_or_else(|| format!("the rig has no {name} joint"))
        };
        let (neck, head, chest) = (find("Neck")?, find("Head")?, find("Spine2")?);
        let rest_world = rest_worlds(&joints);
        let at = |i: usize| rest_world[i].map_or([0.0; 3], |m| [m[3][0], m[3][1], m[3][2]]);
        let (head_at, neck_at) = (at(head), at(neck));

        let mut channels: Vec<String> = ARKIT.iter().map(|s| s.to_string()).collect();
        let mut parts = Vec::new();
        let eyes_at = primitive_of(file, PartKind::Eyes).map(|p| {
            let sum = p
                .positions
                .iter()
                .fold([0.0f32; 3], |a, q| [a[0] + q[0], a[1] + q[1], a[2] + q[2]]);
            let n = p.positions.len().max(1) as f32;
            [sum[0] / n, sum[1] / n, sum[2] / n]
        });
        for kind in PartKind::ALL {
            let Some(prim) = primitive_of(file, kind) else {
                if kind == PartKind::Skin {
                    return Err("the avatar has no body mesh".into());
                }
                continue;
            };
            let targets = |prim: &GPrimitive,
                           remap: &dyn Fn(u32) -> Option<u32>,
                           xf: &dyn Fn([f32; 3]) -> [f32; 3],
                           channels: &mut Vec<String>| {
                prim.targets
                    .iter()
                    .filter_map(|t| {
                        let deltas: Vec<_> = t
                            .deltas
                            .iter()
                            .filter_map(|&(v, d, n)| remap(v).map(|v| (v, xf(d), xf(n))))
                            .collect();
                        if deltas.is_empty() {
                            return None;
                        }
                        let channel = match channels.iter().position(|c| *c == t.name) {
                            Some(c) => c,
                            None => {
                                channels.push(t.name.clone());
                                channels.len() - 1
                            }
                        };
                        Some(Target { channel, deltas })
                    })
                    .collect::<Vec<_>>()
            };
            match kind {
                PartKind::Skin => {
                    let skin_picture = picture_of(file, kind);
                    let eyes =
                        eyes_at.unwrap_or([head_at[0], head_at[1] + 0.09, head_at[2] + 0.11]);
                    let lip = |tri: &[u32]| -> bool {
                        let Some((w, h, rgba)) = &skin_picture else {
                            return false;
                        };
                        let mid = tri.iter().fold([0.0f32; 3], |a, &i| {
                            let q = prim.positions[i as usize];
                            [a[0] + q[0] / 3.0, a[1] + q[1] / 3.0, a[2] + q[2] / 3.0]
                        });
                        let below = eyes[1] - mid[1];
                        if !MOUTH_BELOW.contains(&below)
                            || (mid[0] - eyes[0]).abs() > MOUTH_HALF_WIDTH
                            || mid[2] < eyes[2] - MOUTH_BEHIND
                        {
                            return false;
                        }
                        let uv = tri.iter().fold([0.0f32; 2], |a, &i| {
                            let u = prim.uvs.get(i as usize).copied().unwrap_or([0.0; 2]);
                            [a[0] + u[0] / 3.0, a[1] + u[1] / 3.0]
                        });
                        let x = ((uv[0].clamp(0.0, 1.0) * *w as f32) as u32).min(w - 1);
                        let y = ((uv[1].clamp(0.0, 1.0) * *h as f32) as u32).min(h - 1);
                        let at = ((y * w + x) * 4) as usize;
                        let c = |k: usize| rgba[at + k] as f32 / 255.0;
                        c(0) - (c(1) + c(2)) / 2.0 > LIP_RED
                    };
                    let (mesh, keep) = cut_skin(
                        prim,
                        neck_at[1] - CUT_BELOW_NECK,
                        neck_at[1] + FADE_ABOVE_NECK,
                        &lip,
                    );
                    let remap = |v: u32| keep.get(&v).copied();
                    let t = targets(prim, &remap, &|d| d, &mut channels);
                    parts.push(Part {
                        kind,
                        mesh,
                        targets: t,
                        rigid_to: None,
                    });
                }
                PartKind::Eyes => {
                    let at = skin
                        .joints
                        .iter()
                        .position(|&j| j == head)
                        .ok_or("the head is not a joint of the skin")?;
                    let into_head = skin.inverse_bind[at];
                    let mesh = eye_mesh(prim, &into_head);
                    let t = targets(prim, &|v| Some(v), &|d| turn(&into_head, d), &mut channels);
                    parts.push(Part {
                        kind,
                        mesh,
                        targets: t,
                        rigid_to: Some(head),
                    });
                }
                _ => {
                    let mesh = skinned_mesh(prim);
                    let t = targets(prim, &|v| Some(v), &|d| d, &mut channels);
                    parts.push(Part {
                        kind,
                        mesh,
                        targets: t,
                        rigid_to: None,
                    });
                }
            }
        }
        for part in &parts {
            part.mesh
                .validate()
                .map_err(|e| format!("{:?}: {e}", part.kind))?;
        }
        let ponytail = ["PonytailRoot", "Ponytail1", "Ponytail2", "Ponytail3"]
            .iter()
            .filter_map(|n| file.node(n))
            .collect();
        let into_head = skin
            .joints
            .iter()
            .position(|&j| j == head)
            .map(|at| skin.inverse_bind[at])
            .ok_or("the head is not a joint of the skin")?;
        let eyes = eyes_at.unwrap_or([head_at[0], head_at[1] + 0.09, head_at[2] + 0.11]);
        for part in parts.iter_mut().filter(|p| p.rigid_to.is_none()) {
            sculpt(&mut part.mesh, &mut part.targets, eyes);
        }
        let landmarks = Landmarks::find(file, &parts, eyes, &into_head);
        let body = primitive_of(file, PartKind::Skin).ok_or("the avatar has no body mesh")?;
        let clothes = file
            .materials
            .iter()
            .position(|m| m == "Human.female_casualsuit01")
            .and_then(|m| {
                file.meshes
                    .iter()
                    .flat_map(|x| &x.primitives)
                    .find(|p| p.material == Some(m))
            });
        let sources: Vec<&GPrimitive> = std::iter::once(body).chain(clothes).collect();
        let limb = |kind| {
            crate::limb::cut(
                file,
                &sources,
                &joints,
                &skin.joints,
                &skin.inverse_bind,
                kind,
            )
        };
        let (arm, leg) = (
            limb(crate::limb::LimbKind::Arm),
            limb(crate::limb::LimbKind::Leg),
        );
        Ok(Self {
            joints,
            skin_joints: skin.joints.clone(),
            inverse_bind: skin.inverse_bind.clone(),
            parts,
            channels,
            ponytail,
            into_head,
            eye_centres: landmarks.eye_centres,
            mouth: landmarks.mouth,
            forward: landmarks.forward,
            forehead: landmarks.forehead,
            forehead_out: landmarks.forehead_out,
            arm,
            leg,
            neck,
            head,
            chest,
            eyes_at: eyes_at.unwrap_or([head_at[0], head_at[1] + 0.09, head_at[2] + 0.11]),
            head_at,
            neck_at,
        })
    }

    pub fn channel(&self, name: &str) -> Option<usize> {
        self.channels.iter().position(|c| c == name)
    }

    pub fn neutral(&self) -> Vec<f32> {
        vec![0.0; self.channels.len()]
    }

    pub fn channel_map(&self) -> HashMap<&str, usize> {
        self.channels
            .iter()
            .enumerate()
            .map(|(i, c)| (c.as_str(), i))
            .collect()
    }
}

impl Part {
    pub fn shaped(&self, weights: &[f32]) -> MeshData {
        let mut mesh = self.mesh.clone();
        let mut moved = false;
        for t in &self.targets {
            let w = weights.get(t.channel).copied().unwrap_or(0.0);
            if !(w.is_finite() && w.abs() > 1e-4) {
                continue;
            }
            moved = true;
            for &(v, d, n) in &t.deltas {
                let v = v as usize;
                if let (Some(p), Some(q)) = (mesh.positions.get_mut(v), mesh.normals.get_mut(v)) {
                    for k in 0..3 {
                        p[k] += w * d[k];
                        q[k] += w * n[k];
                    }
                }
            }
        }
        if moved {
            for n in &mut mesh.normals {
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if l > 1e-12 {
                    *n = [n[0] / l, n[1] / l, n[2] / l];
                }
            }
        }
        mesh
    }

    pub fn signature(&self, weights: &[f32]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for t in &self.targets {
            let w = weights.get(t.channel).copied().unwrap_or(0.0);
            let q = if w.is_finite() {
                (w * 400.0).round() as i64
            } else {
                0
            };
            h = (h ^ (q as u64).wrapping_add(t.channel as u64 * 0x9E37))
                .wrapping_mul(0x0100_0000_01b3);
        }
        h
    }
}

pub fn rest_worlds(joints: &[Option<Joint>]) -> Vec<Option<Mat4>> {
    let mut out: Vec<Option<Mat4>> = vec![None; joints.len()];
    for i in 0..joints.len() {
        world_of(joints, i, &mut out, 0);
    }
    out
}

fn world_of(
    joints: &[Option<Joint>],
    i: usize,
    out: &mut [Option<Mat4>],
    depth: usize,
) -> Option<Mat4> {
    if let Some(m) = out[i] {
        return Some(m);
    }
    if depth > 256 {
        return None;
    }
    let joint = joints[i].as_ref()?;
    let local = joint.rest.matrix();
    let m = match joint.parent {
        Some(p) => angel_stage::scene::mat_mul(&world_of(joints, p, out, depth + 1)?, &local),
        None => local,
    };
    out[i] = Some(m);
    Some(m)
}

fn turn(m: &Mat4, d: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * d[0] + m[1][0] * d[1] + m[2][0] * d[2],
        m[0][1] * d[0] + m[1][1] * d[1] + m[2][1] * d[2],
        m[0][2] * d[0] + m[1][2] * d[1] + m[2][2] * d[2],
    ]
}

struct Landmarks {
    eye_centres: [[f32; 3]; 2],
    mouth: [f32; 3],
    forward: [f32; 3],
    forehead: [f32; 3],
    forehead_out: [f32; 3],
}

impl Landmarks {
    fn find(file: &Gltf, parts: &[Part], eyes: [f32; 3], into_head: &Mat4) -> Self {
        let head = |p: [f32; 3]| angel_stage::scene::mat_point(into_head, p);
        let unit = |v: [f32; 3]| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
            [v[0] / l, v[1] / l, v[2] / l]
        };
        let mean = |pts: &mut dyn Iterator<Item = [f32; 3]>| {
            let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
            for p in pts {
                sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
                n += 1.0;
            }
            (n > 0.0).then(|| [sum[0] / n, sum[1] / n, sum[2] / n])
        };
        let eye_prim = primitive_of(file, PartKind::Eyes);
        let side = |left: bool| {
            eye_prim
                .and_then(|p| {
                    mean(
                        &mut p
                            .positions
                            .iter()
                            .copied()
                            .filter(|q| (q[0] > eyes[0]) == left),
                    )
                })
                .unwrap_or([
                    eyes[0] + if left { 0.032 } else { -0.032 },
                    eyes[1],
                    eyes[2],
                ])
        };
        let eye_centres = [head(side(true)), head(side(false))];
        let skin = parts.iter().find(|p| p.kind == PartKind::Skin);
        let mouth = skin
            .and_then(|s| {
                let (first, count) = *s.mesh.groups.get(SKIN_LIPS)?;
                let tris = s
                    .mesh
                    .indices
                    .get(first as usize..(first + count) as usize)?;
                mean(&mut tris.iter().map(|&i| s.mesh.positions[i as usize]))
            })
            .unwrap_or([eyes[0], eyes[1] - 0.078, eyes[2] + 0.005]);
        let centre = [eyes[0], eyes[1], eyes[2] - 0.09];
        let toward = unit([0.0, 0.8, 0.6]);
        let forehead = skin
            .and_then(|s| {
                s.mesh
                    .positions
                    .iter()
                    .filter(|p| (p[0] - eyes[0]).abs() < 0.012)
                    .max_by(|a, b| {
                        let d = |p: &[f32; 3]| {
                            (p[0] - centre[0]) * toward[0]
                                + (p[1] - centre[1]) * toward[1]
                                + (p[2] - centre[2]) * toward[2]
                        };
                        d(a).total_cmp(&d(b))
                    })
                    .copied()
            })
            .unwrap_or([eyes[0], eyes[1] + 0.085, eyes[2] - 0.03]);
        let out = unit([
            forehead[0] - centre[0],
            forehead[1] - centre[1],
            forehead[2] - centre[2],
        ]);
        Self {
            eye_centres,
            mouth: head(mouth),
            forward: unit(turn(into_head, [0.0, 0.0, 1.0])),
            forehead: head(forehead),
            forehead_out: unit(turn(into_head, out)),
        }
    }
}

fn sculpt(mesh: &mut MeshData, targets: &mut [Target], eyes: [f32; 3]) {
    let top = eyes[1] - CHEEKBONES_BELOW_EYES;
    let keep = |y: f32| 1.0 - SCULPT_NARROW * smooth((top - y) / SCULPT_SPAN);
    let factors: Vec<f32> = mesh.positions.iter().map(|p| keep(p[1])).collect();
    for (p, k) in mesh.positions.iter_mut().zip(&factors) {
        p[0] = eyes[0] + (p[0] - eyes[0]) * k;
    }
    for n in &mut mesh.normals {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
        *n = [n[0] / l, n[1] / l, n[2] / l];
    }
    for t in targets {
        for d in &mut t.deltas {
            let k = factors.get(d.0 as usize).copied().unwrap_or(1.0);
            d.1[0] *= k;
        }
    }
}

fn picture_of(file: &Gltf, kind: PartKind) -> Option<(u32, u32, Vec<u8>)> {
    let material = file.materials.iter().position(|m| m == kind.material())?;
    let image = (*file.material_images.get(material)?)?;
    decode_png(file.images.get(image)?)
}

pub fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let px = (w * h) as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..px * 4].to_vec(),
        png::ColorType::Rgb => buf[..px * 3]
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf[..px * 2]
            .chunks_exact(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => buf[..px].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((w, h, rgba))
}

fn primitive_of(file: &Gltf, kind: PartKind) -> Option<&GPrimitive> {
    let material = file.materials.iter().position(|m| m == kind.material())?;
    file.meshes
        .iter()
        .flat_map(|m| &m.primitives)
        .find(|p| p.material == Some(material))
}

fn skinned_mesh(p: &GPrimitive) -> MeshData {
    MeshData {
        positions: p.positions.clone(),
        normals: p.normals.clone(),
        uvs: p.uvs.clone(),
        colors: Vec::new(),
        indices: p.indices.clone(),
        groups: Vec::new(),
        joints: p.joints.clone(),
        weights: p.weights.clone(),
    }
}

fn cut_skin(
    p: &GPrimitive,
    cut: f32,
    fade_top: f32,
    lip: &dyn Fn(&[u32]) -> bool,
) -> (MeshData, HashMap<u32, u32>) {
    let y = |i: u32| p.positions[i as usize][1];
    let mut solid = Vec::new();
    let mut lips = Vec::new();
    let mut band = Vec::new();
    for tri in p.indices.chunks_exact(3) {
        let low = tri.iter().map(|&i| y(i)).fold(f32::INFINITY, f32::min);
        if low < cut {
            continue;
        }
        if low < fade_top {
            band.extend_from_slice(tri);
        } else if lip(tri) {
            lips.extend_from_slice(tri);
        } else {
            solid.extend_from_slice(tri);
        }
    }
    let mut keep: HashMap<u32, u32> = HashMap::new();
    let mut mesh = MeshData::default();
    let mut index = |old: u32, mesh: &mut MeshData| -> u32 {
        *keep.entry(old).or_insert_with(|| {
            let o = old as usize;
            mesh.positions.push(p.positions[o]);
            mesh.normals
                .push(p.normals.get(o).copied().unwrap_or([0.0, 0.0, 1.0]));
            mesh.uvs.push(p.uvs.get(o).copied().unwrap_or([0.0; 2]));
            let k = smooth((p.positions[o][1] - cut) / (fade_top - cut));
            mesh.colors.push([1.0, 1.0, 1.0, k]);
            mesh.joints.push(p.joints.get(o).copied().unwrap_or([0; 4]));
            mesh.weights
                .push(p.weights.get(o).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]));
            (mesh.positions.len() - 1) as u32
        })
    };
    let mut groups = Vec::new();
    for list in [&solid, &lips, &band] {
        groups.push((mesh.indices.len() as u32, list.len() as u32));
        for &i in list.iter() {
            let n = index(i, &mut mesh);
            mesh.indices.push(n);
        }
    }
    mesh.groups = groups;
    (mesh, keep)
}

fn eye_mesh(p: &GPrimitive, into_head: &Mat4) -> MeshData {
    let positions = p
        .positions
        .iter()
        .map(|&q| angel_stage::scene::mat_point(into_head, q))
        .collect();
    let normals = p
        .normals
        .iter()
        .map(|&n| {
            let t = turn(into_head, n);
            let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt().max(1e-12);
            [t[0] / l, t[1] / l, t[2] / l]
        })
        .collect();
    let mut groups: [Vec<u32>; 3] = Default::default();
    for tri in p.indices.chunks_exact(3) {
        let uv = tri.iter().fold([0.0f32; 2], |a, &i| {
            let u = p.uvs.get(i as usize).copied().unwrap_or([0.0; 2]);
            [a[0] + u[0] / 3.0, a[1] + u[1] / 3.0]
        });
        let d = IRIS_CENTRES
            .iter()
            .map(|c| ((uv[0] - c[0]).powi(2) + (uv[1] - c[1]).powi(2)).sqrt())
            .fold(f32::INFINITY, f32::min);
        let g = if d < PUPIL_RADIUS {
            EYE_PUPIL
        } else if d < IRIS_RADIUS {
            EYE_IRIS
        } else {
            EYE_WHITE
        };
        groups[g].extend_from_slice(tri);
    }
    let mut indices = Vec::new();
    let mut ranges = Vec::new();
    for g in &groups {
        ranges.push((indices.len() as u32, g.len() as u32));
        indices.extend_from_slice(g);
    }
    MeshData {
        positions,
        normals,
        uvs: p.uvs.clone(),
        colors: Vec::new(),
        indices,
        groups: ranges,
        joints: Vec::new(),
        weights: Vec::new(),
    }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub fn local_turn(
    rest: [f32; 4],
    rest_parent_world: [f32; 4],
    parent_turn: [f32; 4],
    world_turn: [f32; 4],
) -> [f32; 4] {
    let wp = quat_normalize(rest_parent_world);
    let wj = quat_mul(wp, quat_normalize(rest));
    let inv = |q: [f32; 4]| [-q[0], -q[1], -q[2], q[3]];
    let t = quat_mul(quat_mul(inv(parent_turn), world_turn), parent_turn);
    quat_normalize(quat_mul(quat_mul(inv(wp), t), wj))
}
