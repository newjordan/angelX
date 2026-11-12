//! How she is drawn in glass: an `angel-gpu` scene of her rig, her head in
//! chrome (the mesh shader's `chrome` finish, molten with `liquid`), eyes
//! whose irises burn (`glow`), rose-gold lips. Just her head: that is the
//! foundation.
//!
//! Her regalia is kept for later and drawn only when asked
//! ([`Drawing::regalia`]): gold and cyan rings turning about her head with
//! gems riding them, a halo disc of rings, rays and braille behind her, a
//! jewel on her brow, and around her picture an aura and motes rising.
//!
//! Ids: meshes `MESH_BASE..`, pictures `PICTURE_BASE..` (the scene's own
//! picture first, then the halo's), layers in group [`GROUP`].

use std::sync::Arc;

use angel_stage::scene::{
    AlphaMode, Camera, Mat4, Material, MeshData, MeshRef, Node, Projection, Sampling, Scene, Skin,
    TextureSource, Transform, quat_axis_angle, quat_mul, quat_normalize,
};
use angel_stage::{LAYER_PICTURES, Layer, LayerKind, Particles, Placement, Stroke, StrokeShape};

use crate::mind::Moment;
use crate::model::{
    EYE_IRIS, EYE_PUPIL, EYE_WHITE, Goddess, PartKind, SKIN_FADE, SKIN_LIPS, SKIN_SOLID,
};

/// Her layer group ("FACE").
pub const GROUP: u32 = 0x4641_4345;
/// The picture her scene is drawn into, and the halo disc's.
pub const PICTURE_BASE: u32 = LAYER_PICTURES + 0x4641_0000;
pub const SCENE_PICTURE: u32 = PICTURE_BASE;
pub const HALO_PICTURE: u32 = PICTURE_BASE + 1;
/// The picture a pointing limb is drawn into.
pub const LIMB_PICTURE: u32 = PICTURE_BASE + 3;
/// Her meshes: a part each, then the rings.
pub const MESH_BASE: u32 = 0x4641_0000;
const RING_MESHES: u32 = MESH_BASE + 0x10;
const SHEATH_MESHES: u32 = MESH_BASE + 0x20;
const LIMB_MESH: u32 = MESH_BASE + 0x40;
const LIMB_ID: u64 = 0x4641_0003_0000;

/// Her silver, her lips' rose gold, her eyes' light.
const SILVER: [f32; 3] = [0.92, 0.92, 0.94];
const ROSE_GOLD: [f32; 3] = [1.0, 0.56, 0.48];
const EYE_LIGHT: [f32; 3] = [0.82, 0.95, 1.0];
/// Her eyes as they burn.
const EYE_FIRE: [f32; 3] = [1.0, 0.10, 0.04];

/// The cube in her forehead: its side (meters), how far it drifts in and
/// out of her skull about the skin (meters), how long one drift takes, how
/// fast it turns, and how slowly its colours go round (hue turns a second).
const CUBE_SIDE: f32 = 0.05;
const CUBE_DRIFT: f32 = 0.028;
const CUBE_DRIFT_SECONDS: f32 = 23.0;
const CUBE_TURN: [f32; 3] = [0.13, 0.21, 0.08];
const CUBE_HUE_RATE: f32 = 0.018;

/// Where the jewel on her brow is found: the plane of a circlet about her
/// head (its middle up and back from her eyes, its front tipped down
/// toward them, radians), fitted to her skin, the jewel at its front.
const CIRCLET_UP: f32 = 0.06;
const CIRCLET_BACK: f32 = 0.10;
const CIRCLET_TIP: f32 = 0.30;
const CIRCLET_STANDOFF: f32 = 0.0025;
const CIRCLET_STEPS: usize = 144;

/// Scene node ids: her joints (by the file's node index), her parts, her
/// regalia.
const JOINT_IDS: u64 = 0x4641_0000_0000;
const PART_IDS: u64 = 0x4641_0001_0000;
const CROWN_ID: u64 = 0x4641_0002_0000;

/// The halo disc's picture size.
const HALO_SIZE: u32 = 512;

/// Her camera: how wide it sees (her picture is square), how far back it
/// stands, and where it looks, from the middle of her eyes. Her head and
/// neck alone; with her regalia, wide enough for the halo and rings whole.
const FOV_Y: f32 = 0.215;
const FOV_Y_REGALIA: f32 = 0.335;
const DISTANCE: f32 = 1.85;
const LOOK_AT: [f32; 3] = [0.0, -0.045, -0.07];
const LOOK_AT_REGALIA: [f32; 3] = [0.0, -0.02, -0.07];
/// The camera a little below her eyes: she is looked up to.
const CAMERA_DROP: f32 = 0.07;

/// The rings: radius, tube, tilt about X, tilt about Z, turn speed
/// (radians a second), colour.
const RINGS: [(f32, f32, f32, f32, f32, [f32; 3]); 3] = [
    (0.205, 0.0016, 1.32, 0.10, 0.22, [1.0, 0.80, 0.42]),
    (0.235, 0.0011, 0.32, -0.18, -0.13, [0.55, 0.88, 1.0]),
    (0.265, 0.0009, 0.95, 0.55, 0.08, [1.0, 0.92, 0.75]),
];
const GEMS: usize = 7;

/// What draws her, with what it has sent.
pub struct Drawing {
    pub goddess: Arc<Goddess>,
    /// Draw her regalia (rings, halo disc, gems, the jewel on her brow, and
    /// the aura and motes about her picture). Off: just her head.
    pub regalia: bool,
    /// The jewel on her brow, in the head joint's space (fitted once).
    jewel: [f32; 3],
    /// Each part's mesh as last sent (its weights' signature).
    sent: Vec<Option<u64>>,
    rings_sent: bool,
}

impl Drawing {
    pub fn new(goddess: Arc<Goddess>) -> Self {
        let parts = goddess.parts.len();
        let mut d = Self {
            goddess,
            regalia: false,
            jewel: [0.0; 3],
            sent: vec![None; parts],
            rings_sent: false,
        };
        d.jewel = d.jewel_at();
        d
    }

    /// Forget what was sent: everything goes again (a new window).
    pub fn reset(&mut self) {
        self.sent.iter_mut().for_each(|s| *s = None);
        self.rings_sent = false;
    }

    /// Where the camera stands and what it looks at.
    pub fn camera_at(&self) -> ([f32; 3], [f32; 3]) {
        let e = self.goddess.eyes_at;
        let at = if self.regalia {
            LOOK_AT_REGALIA
        } else {
            LOOK_AT
        };
        let target = [e[0] + at[0], e[1] + at[1], e[2] + at[2]];
        let eye = [target[0], e[1] - CAMERA_DROP, target[2] + DISTANCE];
        (eye, target)
    }

    /// How many meters her picture spans top to bottom (her camera's view
    /// at her face): what turns pixels into meters for her limbs.
    pub fn meters_across(&self) -> f32 {
        let fov = if self.regalia { FOV_Y_REGALIA } else { FOV_Y };
        2.0 * DISTANCE * (fov * 0.5).tan()
    }

    /// Where you are from her eyes: yaw toward her left, pitch down.
    pub fn viewer(&self) -> [f32; 2] {
        let (cam, _) = self.camera_at();
        let e = self.goddess.eyes_at;
        let (dx, dy, dz) = (cam[0] - e[0], cam[1] - e[1], cam[2] - e[2]);
        [dx.atan2(dz), (-dy).atan2((dx * dx + dz * dz).sqrt())]
    }

    /// The meshes to send before drawing `moment`: the parts whose shapes
    /// moved since they were last sent, and the rings the first time.
    pub fn meshes(&mut self, moment: &Moment) -> Vec<(u32, MeshData)> {
        let mut out = Vec::new();
        for (i, part) in self.goddess.parts.iter().enumerate() {
            let sig = part.signature(&moment.weights);
            if self.sent[i] != Some(sig) {
                self.sent[i] = Some(sig);
                out.push((MESH_BASE + i as u32, part.shaped(&moment.weights)));
            }
        }
        if self.regalia && !self.rings_sent {
            self.rings_sent = true;
            for (k, ring) in RINGS.iter().enumerate() {
                out.push((RING_MESHES + k as u32, torus(ring.0, ring.1, 160, 10)));
                out.push((
                    SHEATH_MESHES + k as u32,
                    torus(ring.0, ring.1 * 5.0, 160, 12),
                ));
            }
        }
        out
    }

    /// The pictures her scene names, straight RGBA, by id (with her
    /// regalia, the halo disc's). Sent once.
    pub fn pictures(&self) -> Vec<(u32, u32, u32, Vec<u8>)> {
        if !self.regalia {
            return Vec::new();
        }
        let (w, h, rgba) = halo_picture();
        vec![(HALO_PICTURE, w, h, rgba)]
    }

    /// The circlet's plane: its middle, the way to her left, the way to
    /// its front (tipped down), and its normal.
    fn circlet_frame(&self) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
        let e = self.goddess.eyes_at;
        let c = [e[0], e[1] + CIRCLET_UP, e[2] - CIRCLET_BACK];
        let (s, k) = CIRCLET_TIP.sin_cos();
        (c, [1.0, 0.0, 0.0], [0.0, -s, k], [0.0, -k, -s])
    }

    /// How far her skin reaches from the circlet's middle, in its plane,
    /// at each of `CIRCLET_STEPS` angles (0 her front, turning to her
    /// left): the band hugs her head at that, plus its standoff.
    fn circlet_reach(&self) -> Vec<f32> {
        let (c, u, v, w) = self.circlet_frame();
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let mut reach = vec![0.0f32; CIRCLET_STEPS];
        if let Some(skin) = self.goddess.parts.iter().find(|p| p.kind == PartKind::Skin) {
            for p in &skin.mesh.positions {
                let q = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
                if dot(q, w).abs() > 0.007 {
                    continue;
                }
                let (x, z) = (dot(q, u), dot(q, v));
                let a = x.atan2(z).rem_euclid(std::f32::consts::TAU);
                let i = ((a / std::f32::consts::TAU * CIRCLET_STEPS as f32) as usize)
                    .min(CIRCLET_STEPS - 1);
                reach[i] = reach[i].max((x * x + z * z).sqrt());
            }
        }
        // Fill angles no vertex fell in from their neighbours, then smooth.
        let known: Vec<usize> = (0..CIRCLET_STEPS).filter(|&i| reach[i] > 0.0).collect();
        if known.is_empty() {
            return vec![0.09; CIRCLET_STEPS];
        }
        let filled: Vec<f32> = (0..CIRCLET_STEPS)
            .map(|i| {
                if reach[i] > 0.0 {
                    return reach[i];
                }
                let near = known.iter().min_by_key(|&&k| {
                    let d = (k as i64 - i as i64).unsigned_abs() as usize;
                    d.min(CIRCLET_STEPS - d)
                });
                near.map_or(0.09, |&k| reach[k])
            })
            .collect();
        (0..CIRCLET_STEPS)
            .map(|i| {
                let mut top = 0.0f32;
                for d in 0..5 {
                    top = top.max(filled[(i + CIRCLET_STEPS + d - 2) % CIRCLET_STEPS]);
                }
                top + CIRCLET_STANDOFF
            })
            .collect()
    }

    /// Where the circlet's jewel sits on her brow, in the head joint's space.
    fn jewel_at(&self) -> [f32; 3] {
        let (c, _, v, _) = self.circlet_frame();
        let r = self.circlet_reach()[0] + 0.003;
        angel_stage::scene::mat_point(
            &self.goddess.into_head,
            [c[0] + v[0] * r, c[1] + v[1] * r, c[2] + v[2] * r],
        )
    }

    /// Her scene at `moment`, `t` seconds on her clock.
    pub fn scene(&self, moment: &Moment, t: f64) -> Scene {
        let g = &self.goddess;
        let (eye, target) = self.camera_at();
        let mut scene = Scene::new(Camera {
            transform: Transform {
                translation: eye,
                rotation: look_at(eye, target),
                scale: [1.0; 3],
            },
            projection: Projection::Perspective {
                fov_y: if self.regalia { FOV_Y_REGALIA } else { FOV_Y },
                near: 0.05,
                far: 20.0,
            },
            clear: Some([0.0; 4]),
        });
        // The rig, posed.
        let posed = g.posed(&moment.head);
        for (i, joint) in g.joints.iter().enumerate() {
            let (Some(joint), Some(transform)) = (joint, posed[i]) else {
                continue;
            };
            let mut node = Node::empty(JOINT_IDS + i as u64);
            node.parent = joint.parent.map(|p| JOINT_IDS + p as u64);
            node.transform = transform;
            scene.nodes.push(node);
        }
        let skin = Skin {
            joints: g
                .skin_joints
                .iter()
                .map(|&j| JOINT_IDS + j as u64)
                .collect::<Vec<_>>()
                .into(),
            inverse_bind: Arc::from(g.inverse_bind.clone()),
        };
        let p = moment.presence;
        let liquid = moment.liquid.clamp(0.0, 1.0);
        for (i, part) in g.parts.iter().enumerate() {
            let mesh = MeshRef::Uploaded(MESH_BASE + i as u32);
            let mut node = Node::new(PART_IDS + i as u64, mesh, chrome(SILVER, liquid));
            match part.rigid_to {
                Some(j) => node.parent = Some(JOINT_IDS + j as u64),
                None => node.skin = Some(skin.clone()),
            }
            match part.kind {
                PartKind::Skin => {
                    let solid = chrome(SILVER, liquid);
                    let lips = Material {
                        lit: true,
                        chrome: 0.7,
                        liquid,
                        ..Material::color([ROSE_GOLD[0], ROSE_GOLD[1], ROSE_GOLD[2], 1.0])
                    };
                    let fade = Material {
                        alpha: AlphaMode::Blend,
                        ..chrome(SILVER, liquid)
                    };
                    node.face_materials = vec![solid, lips, fade];
                    debug_assert_eq!((SKIN_SOLID, SKIN_LIPS, SKIN_FADE), (0, 1, 2));
                }
                PartKind::Eyes => {
                    let white = Material {
                        chrome: 1.0,
                        ..Material::color([0.10, 0.11, 0.15, 1.0])
                    };
                    let fire = moment.heat.max(moment.laser);
                    let burn = (0.55 + 0.45 * moment.glow).max(fire).clamp(0.0, 1.0);
                    let light: [f32; 3] =
                        std::array::from_fn(|k| EYE_LIGHT[k] + (EYE_FIRE[k] - EYE_LIGHT[k]) * fire);
                    let iris = Material {
                        lit: true,
                        emissive: 1.0,
                        glow: burn,
                        ..Material::color([light[0], light[1], light[2], 1.0])
                    };
                    let pupil = Material {
                        chrome: 0.7,
                        ..Material::color([0.02, 0.02, 0.03, 1.0])
                    };
                    node.face_materials = vec![white, iris, pupil];
                    debug_assert_eq!((EYE_WHITE, EYE_IRIS, EYE_PUPIL), (0, 1, 2));
                }
                PartKind::Lashes => node.material = chrome([0.10, 0.11, 0.14], 0.0),
                PartKind::Brows => node.material = chrome([0.42, 0.40, 0.40], 0.0),
                PartKind::Teeth => node.material = chrome([0.88, 0.90, 0.95], 0.0),
                PartKind::Tongue => {
                    node.material = Material {
                        lit: true,
                        emissive: 0.15,
                        ..Material::color([0.30, 0.10, 0.16, 1.0])
                    }
                }
            }
            scene.nodes.push(node);
        }
        // The cube in her forehead: turning, drifting in and out of her skull,
        // its faces going round the colours a little apart from each other.
        let drift = (t as f32 / CUBE_DRIFT_SECONDS * std::f32::consts::TAU).sin();
        let f = g.forehead;
        let o = g.forehead_out;
        let at = [
            f[0] + o[0] * CUBE_DRIFT * drift,
            f[1] + o[1] * CUBE_DRIFT * drift,
            f[2] + o[2] * CUBE_DRIFT * drift,
        ];
        let tt = t as f32;
        let turn = quat_normalize(quat_mul(
            quat_mul(
                quat_axis_angle([1.0, 0.0, 0.0], tt * CUBE_TURN[0]),
                quat_axis_angle([0.0, 1.0, 0.0], tt * CUBE_TURN[1]),
            ),
            quat_axis_angle([0.0, 0.0, 1.0], tt * CUBE_TURN[2]),
        ));
        let mut cube = Node::new(CROWN_ID + 50, MeshRef::Cube, Material::default());
        cube.parent = Some(JOINT_IDS + g.head as u64);
        cube.transform = Transform {
            translation: at,
            rotation: turn,
            scale: [CUBE_SIDE * (0.4 + 0.6 * p); 3],
        };
        cube.face_materials = (0..6)
            .map(|i| {
                let [r, gg, b] = hsv((tt * CUBE_HUE_RATE + i as f32 * 0.085).fract(), 0.82, 1.0);
                Material {
                    lit: true,
                    emissive: 0.55,
                    glow: 0.3,
                    ..Material::color([r, gg, b, 1.0])
                }
            })
            .collect();
        scene.nodes.push(cube);
        if !self.regalia {
            return scene;
        }
        // A jewel on her brow, where a circlet would hold it: it rides her head.
        let mut jewel = Node::new(
            CROWN_ID + 41,
            MeshRef::Gem,
            Material {
                lit: true,
                emissive: 1.0,
                glow: (0.5 + 0.5 * moment.glow).min(1.0),
                ..Material::color([EYE_LIGHT[0], EYE_LIGHT[1], EYE_LIGHT[2], 1.0])
            },
        );
        jewel.parent = Some(JOINT_IDS + g.head as u64);
        jewel.transform = Transform {
            translation: self.jewel,
            scale: [0.0065, 0.0105, 0.004],
            ..Transform::IDENTITY
        };
        scene.nodes.push(jewel);
        // Her regalia: a crown point at her head, not turning with it.
        let crown_at = [g.head_at[0], g.eyes_at[1] + 0.03, g.head_at[2] - 0.01];
        let grow = 0.6 + 0.4 * p;
        let mut crown = Node::empty(CROWN_ID);
        crown.transform = Transform {
            translation: crown_at,
            scale: [grow; 3],
            ..Transform::IDENTITY
        };
        scene.nodes.push(crown);
        let t32 = t as f32;
        let spin = moment.spin;
        // The halo disc, behind her head, facing you.
        let mut disc = Node::new(
            CROWN_ID + 1,
            MeshRef::Quad,
            Material {
                source: TextureSource::Picture(HALO_PICTURE),
                tint: [1.0, 1.0, 1.0, (0.55 + 0.35 * moment.voice) * p],
                sampling: Sampling::Linear,
                alpha: AlphaMode::Blend,
                double_sided: true,
                ..Material::default()
            },
        );
        disc.parent = Some(CROWN_ID);
        disc.transform = Transform {
            translation: [0.0, 0.02, -0.17],
            rotation: quat_axis_angle([0.0, 0.0, 1.0], t32 * 0.035 * spin),
            scale: [0.58, 0.58, 1.0],
        };
        scene.nodes.push(disc);
        for (k, &(_, _, tilt_x, tilt_z, speed, colour)) in RINGS.iter().enumerate() {
            let turn = quat_normalize(quat_mul(
                quat_mul(
                    quat_axis_angle(
                        [0.0, 0.0, 1.0],
                        tilt_z + 0.05 * (t32 * 0.11 + k as f32).sin(),
                    ),
                    quat_axis_angle([1.0, 0.0, 0.0], tilt_x),
                ),
                quat_axis_angle([0.0, 0.0, 1.0], t32 * speed * spin),
            ));
            let bright = (0.75 + 0.25 * moment.glow).min(1.0);
            let mut ring = Node::new(
                CROWN_ID + 2 + k as u64,
                MeshRef::Uploaded(RING_MESHES + k as u32),
                Material::color([
                    colour[0] * bright,
                    colour[1] * bright,
                    colour[2] * bright,
                    p,
                ]),
            );
            ring.material.alpha = AlphaMode::Blend;
            ring.parent = Some(CROWN_ID);
            ring.transform = Transform {
                rotation: turn,
                ..Transform::IDENTITY
            };
            scene.nodes.push(ring);
            // Its light: a wider, faint sheath about it.
            let mut sheath = Node::new(
                CROWN_ID + 8 + k as u64,
                MeshRef::Uploaded(SHEATH_MESHES + k as u32),
                Material {
                    alpha: AlphaMode::Blend,
                    ..Material::color([
                        colour[0],
                        colour[1],
                        colour[2],
                        (0.10 + 0.08 * moment.voice) * p,
                    ])
                },
            );
            sheath.parent = Some(CROWN_ID + 2 + k as u64);
            scene.nodes.push(sheath);
            if k == 0 {
                // Gems riding the first ring.
                for n in 0..GEMS {
                    let a = n as f32 / GEMS as f32 * std::f32::consts::TAU;
                    let mut gem = Node::new(
                        CROWN_ID + 16 + n as u64,
                        MeshRef::Gem,
                        Material {
                            chrome: 0.85,
                            glow: 0.5 * p,
                            ..Material::color([1.0, 0.9, 0.7, 1.0])
                        },
                    );
                    gem.parent = Some(CROWN_ID + 2);
                    gem.transform = Transform {
                        translation: [a.cos() * RINGS[0].0, a.sin() * RINGS[0].0, 0.0],
                        rotation: quat_axis_angle([0.3, 1.0, 0.2], t32 * 1.3 + n as f32),
                        scale: [0.011, 0.016, 0.011],
                    };
                    scene.nodes.push(gem);
                }
            }
        }
        scene
    }
}

/// Where her eyes and mouth are on her picture, in its pixels (`size`
/// square, y down), and the way her mouth pours on it (a unit vector), for
/// what she shoots and spills. `None` if her head cannot be placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchors {
    pub eyes: [[f32; 2]; 2],
    pub mouth: [f32; 2],
    pub pours: [f32; 2],
}

impl Drawing {
    /// Her eyes and mouth on the picture `scene` draws at `size`.
    pub fn anchors(&self, scene: &Scene, size: (u32, u32)) -> Option<Anchors> {
        let g = &self.goddess;
        let head_id = JOINT_IDS + g.head as u64;
        let index = scene.nodes.iter().position(|n| n.id == head_id)?;
        let head = scene.world_matrices().get(index).copied().flatten()?;
        let vp = scene
            .camera
            .view_projection(size.0 as f32 / size.1.max(1) as f32)?;
        let on_picture = |local: [f32; 3]| -> Option<[f32; 2]> {
            let w = angel_stage::scene::mat_point(&head, local);
            let clip: [f32; 4] = std::array::from_fn(|r| {
                vp[0][r] * w[0] + vp[1][r] * w[1] + vp[2][r] * w[2] + vp[3][r]
            });
            (clip[3] > 1e-6).then(|| {
                [
                    (clip[0] / clip[3] * 0.5 + 0.5) * size.0 as f32,
                    (0.5 - clip[1] / clip[3] * 0.5) * size.1 as f32,
                ]
            })
        };
        let eyes = [on_picture(g.eye_centres[0])?, on_picture(g.eye_centres[1])?];
        let mouth = on_picture(g.mouth)?;
        let m = g.mouth;
        let ahead = on_picture([
            m[0] + g.forward[0] * 0.06,
            m[1] + g.forward[1] * 0.06 - 0.03,
            m[2] + g.forward[2] * 0.06,
        ])?;
        let d = [ahead[0] - mouth[0], ahead[1] - mouth[1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let pours = if l > 1e-3 {
            [d[0] / l, d[1] / l]
        } else {
            [0.0, 1.0]
        };
        Some(Anchors { eyes, mouth, pours })
    }
}

impl Drawing {
    /// Her `kind` of limb stretched so its tip lies `length` meters from
    /// its root, as the mesh to send (once a gesture).
    pub fn limb_mesh(&self, kind: crate::limb::LimbKind, length: f32) -> Option<(u32, MeshData)> {
        let limb = self.limb(kind)?;
        Some((LIMB_MESH, limb.stretched(length)))
    }

    fn limb(&self, kind: crate::limb::LimbKind) -> Option<&crate::limb::Limb> {
        match kind {
            crate::limb::LimbKind::Arm => self.goddess.arm.as_ref(),
            crate::limb::LimbKind::Leg => self.goddess.leg.as_ref(),
        }
    }

    /// How far across a limb reaches (meters): the strip it needs is twice
    /// this, with a little room.
    pub fn limb_radius(&self, kind: crate::limb::LimbKind) -> Option<f32> {
        self.limb(kind).map(|l| l.radius)
    }

    /// The scene of a limb stretched to `length` (its mesh sent with
    /// [`Drawing::limb_mesh`]), lying along the picture, seen side on and
    /// flat (orthographic) at `ppm` pixels a meter, its tip `tip_room`
    /// pixels in from the right edge of a picture `size` pixels.
    pub fn limb_scene(
        &self,
        kind: crate::limb::LimbKind,
        length: f32,
        size: (u32, u32),
        ppm: f32,
        tip_room: f32,
        liquid: f32,
    ) -> Option<Scene> {
        let limb = self.limb(kind)?;
        let (w, h) = (size.0 as f32 / ppm, size.1 as f32 / ppm);
        let tip = limb.tip_at(length);
        let centre_x = tip + tip_room / ppm - w / 2.0;
        let mut scene = Scene::new(Camera {
            transform: Transform::from_translation([centre_x, 0.0, 3.0]),
            projection: Projection::Orthographic {
                height: h,
                near: 0.1,
                far: 10.0,
            },
            clear: Some([0.0; 4]),
        });
        let mut node = Node::new(
            LIMB_ID,
            MeshRef::Uploaded(LIMB_MESH),
            chrome(SILVER, liquid),
        );
        node.face_materials = vec![
            chrome(SILVER, liquid),
            Material {
                alpha: AlphaMode::Blend,
                ..chrome(SILVER, liquid)
            },
        ];
        scene.nodes.push(node);
        Some(scene)
    }
}

/// Hue, saturation, value (each 0..1) as linear RGB.
fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let k = |n: f32| {
        let k = (n + h * 6.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [k(5.0), k(3.0), k(1.0)]
}

/// The rainbow's bands, outside in.
const RAINBOW: [[f32; 3]; 7] = [
    [1.0, 0.16, 0.16],
    [1.0, 0.55, 0.10],
    [1.0, 0.92, 0.18],
    [0.25, 0.95, 0.35],
    [0.20, 0.65, 1.0],
    [0.35, 0.30, 1.0],
    [0.72, 0.30, 1.0],
];

/// Where a limb's picture (`size` pixels, its tip `tip_room` pixels in from
/// its right end) lies so its tip reaches `target`, bursting in from
/// `entry`: its rectangle (in the same pixels, before its turn) and its
/// turn (radians, clockwise on screen). `along` slides it back toward
/// where it came from (negative) or on past the target (positive), pixels.
pub fn limb_rect(entry: [f32; 2], target: [f32; 2], size: (u32, u32), tip_room: f32, along: f32) -> ([f32; 4], f32) {
    let (dx, dy) = (target[0] - entry[0], target[1] - entry[1]);
    let l = (dx * dx + dy * dy).sqrt().max(1.0);
    let dir = [dx / l, dy / l];
    let (w, h) = (size.0 as f32, size.1 as f32);
    let centre = [
        target[0] - dir[0] * (w / 2.0 - tip_room) + dir[0] * along,
        target[1] - dir[1] * (w / 2.0 - tip_room) + dir[1] * along,
    ];
    ([centre[0] - w / 2.0, centre[1] - h / 2.0, w, h], dir[1].atan2(dir[0]))
}

/// The way her rainbow is thrown, on screen: from the corner she leans out
/// of toward the window's middle, a little upward, so it arcs out over the
/// terminal before it falls.
pub fn pour_toward(corner: [f32; 2], middle: [f32; 2]) -> [f32; 2] {
    let d = [
        middle[0] - corner[0],
        middle[1] - corner[1] - 0.45 * (middle[0] - corner[0]).abs(),
    ];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
    [d[0] / l, d[1] / l]
}

/// A glowing line from `a` to `b` (window pixels) as a stroke layer.
fn beam(
    id: u64,
    z: i32,
    a: [f32; 2],
    b: [f32; 2],
    width: f32,
    glow: f32,
    color: [f32; 4],
) -> Layer {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let pad = width.max(1.0);
    let mut layer = Layer::new(
        id,
        z,
        Placement::Pixels([
            (a[0] + b[0]) / 2.0 - len / 2.0,
            (a[1] + b[1]) / 2.0 - pad / 2.0,
            len,
            pad,
        ]),
        LayerKind::Stroke(Stroke {
            shape: StrokeShape::Line,
            color,
            width,
            glow,
        }),
    );
    layer.angle = dy.atan2(dx);
    layer.additive = true;
    layer
}

/// Her spectacle's layers, in window pixels at `z`: as her eyes heat, a red
/// light gathers in them; as they fire, two beams from `eyes` to `target`,
/// white at their heart, and sparks thrown where they strike; as she
/// pours, a rainbow ribbon thrown out of `mouth` along `pours` (a unit
/// vector on screen: out into the window) and falling in an arc, its end
/// thrown into droplets. `scale` is her picture's size over 400 pixels;
/// `t` her clock.
pub fn spectacle_layers(
    moment: &Moment,
    eyes: [[f32; 2]; 2],
    mouth: [f32; 2],
    pours: [f32; 2],
    target: [f32; 2],
    scale: f32,
    t: f64,
    z: i32,
) -> Vec<Layer> {
    let Some(s) = moment.spectacle else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut id = 100u64;
    let mut next = || {
        id += 1;
        id
    };
    let tt = t as f32;
    // Gathering light in her eyes.
    if moment.heat > 0.01 {
        for e in eyes {
            let r = (8.0 + 30.0 * moment.heat + 10.0 * moment.laser) * scale;
            let flick = 0.85 + 0.15 * (tt * 31.0).sin();
            let mut glow = Layer::new(
                next(),
                z,
                Placement::Pixels([e[0] - r, e[1] - r, 2.0 * r, 2.0 * r]),
                LayerKind::Glow([1.0, 0.12, 0.05, 0.9 * moment.heat * flick]),
            );
            glow.additive = true;
            out.push(glow);
        }
    }
    // The beams, and where they strike.
    if moment.laser > 0.01 {
        let pulse = 0.75 + 0.25 * (tt * 47.0).sin().abs();
        for (k, e) in eyes.iter().enumerate() {
            let jitter = [
                (tt * 37.0 + k as f32).sin() * 3.0 * scale,
                (tt * 29.0 + k as f32 * 2.0).cos() * 3.0 * scale,
            ];
            let to = [
                target[0] + jitter[0] + (k as f32 - 0.5) * 10.0 * scale,
                target[1] + jitter[1],
            ];
            out.push(beam(
                next(),
                z,
                *e,
                to,
                7.0 * scale * pulse,
                14.0 * scale,
                [1.0, 0.08, 0.04, 0.95 * moment.laser],
            ));
            out.push(beam(
                next(),
                z + 1,
                *e,
                to,
                2.2 * scale,
                3.0 * scale,
                [1.0, 0.92, 0.85, moment.laser],
            ));
        }
        let r = 46.0 * scale * pulse;
        let mut flash = Layer::new(
            next(),
            z,
            Placement::Pixels([target[0] - r, target[1] - r, 2.0 * r, 2.0 * r]),
            LayerKind::Glow([1.0, 0.45, 0.1, 0.9 * moment.laser]),
        );
        flash.additive = true;
        out.push(flash);
        let since = (s - crate::mind::FIRE_AT as f32).max(0.0);
        for (k, age) in [since % 0.45, (since + 0.22) % 0.45]
            .into_iter()
            .enumerate()
        {
            let mut sparks = Layer::new(
                next(),
                z + 1,
                Placement::Pixels([target[0] - 4.0, target[1] - 4.0, 8.0, 8.0]),
                LayerKind::Particles(Particles {
                    color: [1.0, 0.7, 0.25, 1.0],
                    count: 28,
                    size: 3.0 * scale.max(0.6),
                    life: 0.6,
                    rise: 420.0 * scale,
                    sway: 900.0 * scale,
                    seed: 0x5A50 + k as u32 + (since / 0.45) as u32 * 7,
                    burst: Some(age),
                }),
            );
            sparks.additive = true;
            out.push(sparks);
        }
    }
    // The rainbow.
    if moment.pour > 0.01 {
        let poured = (s - crate::mind::POUR_AT as f32).max(0.0);
        // How long a stretch of falling the ribbon shows: it grows as she
        // pours, then its head lets go and falls away.
        let reach = (poured / 0.7).min(1.0) * 1.15;
        let speed = 560.0 * scale;
        let gravity = 780.0 * scale;
        let point = |tau: f32, wobble: f32| -> [f32; 2] {
            let side = [-pours[1], pours[0]];
            [
                mouth[0] + pours[0] * speed * tau + side[0] * wobble,
                mouth[1] + pours[1] * speed * tau + 0.5 * gravity * tau * tau + side[1] * wobble,
            ]
        };
        let band_w = 4.2 * scale * moment.pour;
        let segments = 18;
        for (b, colour) in RAINBOW.iter().enumerate() {
            let offset = (b as f32 - 3.0) * band_w;
            for i in 0..segments {
                let (u0, u1) = (i as f32 / segments as f32, (i + 1) as f32 / segments as f32);
                let wob = |u: f32| {
                    offset * (1.0 + 0.6 * u) + (u * 9.0 - tt * 11.0).sin() * 3.5 * scale * u
                };
                let (a, c) = (point(u0 * reach, wob(u0)), point(u1 * reach, wob(u1)));
                let fade = moment.pour * (1.0 - 0.55 * u1);
                out.push(beam(
                    next(),
                    z,
                    a,
                    c,
                    band_w * 1.15,
                    3.0 * scale,
                    [colour[0], colour[1], colour[2], 0.92 * fade],
                ));
            }
        }
        // Droplets off its end.
        let end = point(reach, 0.0);
        for (k, colour) in RAINBOW.iter().enumerate() {
            let age = (poured + k as f32 * 0.07) % 0.7;
            let mut drops = Layer::new(
                next(),
                z + 1,
                Placement::Pixels([end[0] - 6.0, end[1] - 6.0, 12.0, 12.0]),
                LayerKind::Particles(Particles {
                    color: [colour[0], colour[1], colour[2], moment.pour],
                    count: 5,
                    size: 4.0 * scale.max(0.6),
                    life: 0.7,
                    rise: 160.0 * scale,
                    sway: 700.0 * scale,
                    seed: 0x7A1B + k as u32 + ((poured + k as f32 * 0.07) / 0.7) as u32 * 13,
                    burst: Some(age),
                }),
            );
            drops.additive = true;
            out.push(drops);
        }
    }
    out
}

/// A chrome surface tinted `tint`, `liquid` molten.
fn chrome(tint: [f32; 3], liquid: f32) -> Material {
    Material {
        chrome: 1.0,
        liquid,
        ..Material::color([tint[0], tint[1], tint[2], 1.0])
    }
}

/// The rotation that turns a camera at `eye` (looking down its -Z) to look
/// at `target`, its up kept up.
pub fn look_at(eye: [f32; 3], target: [f32; 3]) -> [f32; 4] {
    let f = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
    let l = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt().max(1e-9);
    let f = [f[0] / l, f[1] / l, f[2] / l];
    let yaw = (-f[0]).atan2(-f[2]);
    let pitch = f[1].clamp(-1.0, 1.0).asin();
    quat_normalize(quat_mul(
        quat_axis_angle([0.0, 1.0, 0.0], yaw),
        quat_axis_angle([1.0, 0.0, 0.0], pitch),
    ))
}

/// A torus about the Z axis (the ring in the XY plane), `radius` to its
/// tube's middle, `tube` thick.
pub fn torus(radius: f32, tube: f32, around: u32, across: u32) -> MeshData {
    let mut m = MeshData::default();
    for i in 0..=around {
        let a = i as f32 / around as f32 * std::f32::consts::TAU;
        let (ca, sa) = (a.cos(), a.sin());
        for j in 0..=across {
            let b = j as f32 / across as f32 * std::f32::consts::TAU;
            let (cb, sb) = (b.cos(), b.sin());
            m.positions.push([
                (radius + tube * cb) * ca,
                (radius + tube * cb) * sa,
                tube * sb,
            ]);
            m.normals.push([cb * ca, cb * sa, sb]);
            m.uvs
                .push([i as f32 / around as f32, j as f32 / across as f32]);
        }
    }
    let row = across + 1;
    for i in 0..around {
        for j in 0..across {
            let (a, b, c, d) = (
                i * row + j,
                (i + 1) * row + j,
                (i + 1) * row + j + 1,
                i * row + j + 1,
            );
            m.indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    m
}

/// The halo disc's picture: straight RGBA, `HALO_SIZE` square. A warm
/// light behind her head, rings, a dashed ring, a band of braille cells,
/// and faint rays: drawn once.
pub fn halo_picture() -> (u32, u32, Vec<u8>) {
    let n = HALO_SIZE as usize;
    let mut rgba = vec![0u8; n * n * 4];
    let gold = [1.0f32, 0.84, 0.52];
    let cyan = [0.55f32, 0.90, 1.0];
    let px = 2.0 / n as f32;
    let line = |r: f32, at: f32, width: f32| 1.0 - smooth(((r - at).abs() - width * 0.5) / px);
    for yy in 0..n {
        for xx in 0..n {
            let x = (xx as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let y = (yy as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r = (x * x + y * y).sqrt();
            if r > 1.0 {
                continue;
            }
            let a = y.atan2(x);
            let mut light = [0.0f32; 3];
            let mut alpha = 0.0f32;
            let mut add = |c: [f32; 3], k: f32| {
                if k > 0.0 {
                    for i in 0..3 {
                        light[i] += c[i] * k;
                    }
                    alpha += k;
                }
            };
            // The light behind her head.
            add([1.0, 0.93, 0.8], 0.42 * (-((r / 0.42).powi(2))).exp());
            // Rays.
            let ray = (a * 18.0).cos().max(0.0).powf(40.0)
                * smooth((r - 0.32) / 0.1)
                * (1.0 - smooth((r - 0.7) / 0.25));
            add(gold, 0.18 * ray);
            // Rings.
            add(gold, 0.95 * line(r, 0.62, 0.006));
            add(gold, 0.35 * line(r, 0.655, 0.003));
            let dash = ((a / std::f32::consts::TAU * 72.0).fract() < 0.55) as u8 as f32;
            add(gold, 0.6 * dash * line(r, 0.76, 0.012));
            add(cyan, 0.4 * line(r, 0.965, 0.003));
            // A band of braille, 48 cells of 2 x 4 dots.
            let cells = 48.0;
            let u = (a / std::f32::consts::TAU + 0.5) * cells;
            let cell = u.floor() as u32;
            let (cu, cv) = (u.fract(), (r - 0.83) / 0.09);
            if (0.0..1.0).contains(&cv) {
                let bits = hash(cell.wrapping_mul(2654435761) ^ 0x2C3B) & 0xFF;
                let (col, rowf) = ((cu * 2.0).floor() as u32, (cv * 4.0).floor() as u32);
                let bit = col.min(1) * 4 + rowf.min(3);
                if bits & (1 << bit) != 0 {
                    let (du, dv) = ((cu * 2.0).fract() - 0.5, (cv * 4.0).fract() - 0.5);
                    let d = (du * du * 1.4 + dv * dv).sqrt();
                    add(
                        if cell.is_multiple_of(5) { cyan } else { gold },
                        0.85 * (1.0 - smooth((d - 0.2) / 0.08)),
                    );
                }
            }
            if alpha <= 0.0 {
                continue;
            }
            let k = alpha.min(1.0);
            let i = (yy * n + xx) * 4;
            for c in 0..3 {
                rgba[i + c] = ((light[c] / alpha).min(1.0) * 255.0) as u8;
            }
            rgba[i + 3] = (k * 255.0) as u8;
        }
    }
    (HALO_SIZE, HALO_SIZE, rgba)
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// Her glass layers for a picture of her placed at `place` (window pixels
/// or cells), turned by `angle` radians about its middle (clockwise on
/// screen): the picture, at `z`. With her regalia, an aura one under it and
/// motes rising one over, placed in the same units.
pub fn layers(place: Placement, z: i32, moment: &Moment, regalia: bool, angle: f32) -> Vec<Layer> {
    let p = moment.presence;
    if p <= 0.001 {
        return Vec::new();
    }
    if !regalia {
        let mut picture = Layer::new(2, z, place, LayerKind::Picture(SCENE_PICTURE));
        picture.opacity = p;
        picture.angle = angle;
        return vec![picture];
    }
    let (rect, unit): ([f32; 4], fn([f32; 4]) -> Placement) = match place {
        Placement::Pixels(r) => (r, Placement::Pixels),
        Placement::Cells(r) => (r, Placement::Cells),
    };
    let [x, y, w, h] = rect;
    let mut out = Vec::new();
    let aura_w = w * 0.95;
    let mut aura = Layer::new(
        1,
        z - 1,
        unit([x + (w - aura_w) / 2.0, y + h * 0.08, aura_w, h * 0.95]),
        LayerKind::Glow([
            0.75,
            0.86,
            1.0,
            (0.16 + 0.14 * moment.voice + 0.06 * moment.glow) * p,
        ]),
    );
    aura.additive = true;
    out.push(aura);
    let mut picture = Layer::new(2, z, place, LayerKind::Picture(SCENE_PICTURE));
    picture.opacity = p;
    picture.angle = angle;
    out.push(picture);
    let mut motes = Layer::new(
        3,
        z + 1,
        unit([x + w * 0.12, y + h * 0.25, w * 0.76, h * 0.75]),
        LayerKind::Particles(Particles {
            color: [1.0, 0.9, 0.7, 0.55 * p],
            count: 36,
            size: 2.0,
            life: 6.0,
            rise: h * 0.09,
            sway: 6.0,
            seed: 0x4641,
            burst: None,
        }),
    );
    motes.additive = true;
    out.push(motes);
    out
}

/// The world matrix type, for hosts that place things by her.
pub type World = Mat4;
