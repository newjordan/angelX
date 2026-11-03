//! The Heartglass's 3D heart: the cloud's cells as tiny gems in an
//! `angel-gpu` scene, turning slowly and beating, cycling through the
//! stages with a scan that rebuilds each one slab by slab.
//!
//! Each stage is cut into [`SLABS`] horizontal slabs, one uploaded mesh
//! each, so a stage can come and go without a mesh being sent again: a
//! slab is a node, and a node only needs a new transform. A slab's mesh has
//! up to three parts (mesh groups): the cardiac cells' cores, lit but
//! mostly shining by themselves; the other cells' cores, small, dim and
//! shaded; and a soft translucent shell round each cardiac cell, its glow.
//!
//! Everything here is a pure function of the cloud and the clock, with no
//! window types, so the meshes, the schedule and the scene are tested in a
//! plain test run.

use angel_stage::scene::{
    AlphaMode, Camera, Material, MeshData, MeshRef, Node, Projection, Scene, Transform, mat_point,
    quat_axis_angle,
};

use super::campaign::{Cloud, Group, Stage};

/// Horizontal slabs a stage is cut into for the scan.
pub(crate) const SLABS: usize = 14;
/// The uploaded meshes' ids: `MESH_BASE | stage << 8 | slab`, and the
/// projector ring's.
pub(crate) const MESH_BASE: u32 = 0x4845_0000;
pub(crate) const RING_MESH: u32 = MESH_BASE | 0xF000;
/// Cells further than this from the middle are left out: the cloud is
/// normalized to a 95th-percentile radius of 1, so these are strays that
/// would only shrink the heart in its frame.
pub(crate) const MAX_RADIUS: f32 = 2.6;

/// Half the size of a gem, in the cloud's units: cardiac cores, the other
/// cells' cores, and the cardiac glow shells.
pub(crate) const CARDIAC_R: f32 = 0.0155;
pub(crate) const CONTEXT_R: f32 = 0.0105;
pub(crate) const HALO_R: f32 = 0.038;
/// How much of a lit cardiac core shines by itself; the rest is the key
/// light's shading, which makes the gems glint as the heart turns.
pub(crate) const CARDIAC_EMISSIVE: f32 = 0.82;
pub(crate) const CONTEXT_EMISSIVE: f32 = 0.32;
/// The glow shells' alpha.
pub(crate) const HALO_ALPHA: f32 = 0.11;
/// What is left of a non-cardiac cell's colour: dimmed and greyed, so the
/// heart's own lineages carry the picture.
pub(crate) const CONTEXT_DIM: f32 = 0.42;

/// Seconds a stage is held, and the scan that brings the next one in.
pub(crate) const HOLD: f32 = 7.5;
pub(crate) const SCAN: f32 = 2.6;
/// The turn, radians a second.
pub(crate) const SPIN: f32 = 0.24;
/// The heartbeat: seconds a beat, and how much the heart swells at its
/// strongest (and when a new official score lands).
pub(crate) const BEAT_PERIOD: f32 = 1.05;
pub(crate) const BEAT_SWELL: f32 = 0.022;
pub(crate) const FLARE_SWELL: f32 = 0.07;

/// Where the scan starts and ends, below and above the heart.
pub(crate) const SCAN_FROM: f32 = -1.6;
pub(crate) const SCAN_TO: f32 = 1.5;
/// How far a slab's coming and going is spread round the scan's height.
pub(crate) const SCAN_SOFT: f32 = 0.22;
/// The projector ring's height and size under the heart.
pub(crate) const RING_Y: f32 = -1.32;
pub(crate) const RING_SCALE: f32 = 0.95;

/// One part of a slab's mesh, drawn with its own material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Cardiac,
    Context,
    Halo,
}

/// One slab of a stage as uploaded: its mesh's id, the height it spans,
/// the middle of its cells across, and its parts in group order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slab {
    pub(crate) id: u32,
    pub(crate) band: [f32; 2],
    pub(crate) centre: [f32; 2],
    pub(crate) parts: Vec<Part>,
}

/// One stage as uploaded.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Built {
    pub(crate) label: String,
    pub(crate) predicted: bool,
    pub(crate) board: String,
    pub(crate) cells: usize,
    pub(crate) cardiac: usize,
    pub(crate) slabs: Vec<Slab>,
}

/// A slab's mesh id.
pub(crate) fn slab_mesh_id(stage: usize, slab: usize) -> u32 {
    MESH_BASE | ((stage as u32 & 0xF) << 8) | (slab as u32 & 0xFF)
}

/// The colour a cell is drawn in: its lineage's own for a cardiac cell,
/// dimmed and greyed toward a cool slate for the rest.
pub(crate) fn cell_colour(group: Option<&Group>, cardiac: bool) -> [f32; 4] {
    let rgb = group
        .map_or([200, 200, 210], |g| g.rgb)
        .map(|c| f32::from(c) / 255.0);
    if cardiac {
        return [rgb[0], rgb[1], rgb[2], 1.0];
    }
    let grey = rgb[0] * 0.30 + rgb[1] * 0.55 + rgb[2] * 0.15;
    let slate = [0.55, 0.62, 0.78];
    let mix = |c: f32, s: f32| ((c * 0.35 + grey * 0.35 + s * 0.30) * CONTEXT_DIM).clamp(0.0, 1.0);
    [
        mix(rgb[0], slate[0]),
        mix(rgb[1], slate[1]),
        mix(rgb[2], slate[2]),
        1.0,
    ]
}

/// Add one gem (an octahedron with smooth normals, six vertices) of half
/// size `r` at `at` in `colour`.
fn push_gem(mesh: &mut MeshData, at: [f32; 3], r: f32, colour: [f32; 4]) {
    let base = mesh.positions.len() as u32;
    const AXES: [[f32; 3]; 6] = [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    for axis in AXES {
        mesh.positions.push([
            at[0] + axis[0] * r,
            at[1] + axis[1] * r,
            at[2] + axis[2] * r,
        ]);
        mesh.normals.push(axis);
        mesh.uvs.push([0.5, 0.5]);
        mesh.colors.push(colour);
    }
    // The eight faces, counter-clockwise from outside: (+x, +y, +z) and its
    // mirrors, flipped where an odd number of axes is negative.
    let (px, nx, py, ny, pz, nz) = (base, base + 1, base + 2, base + 3, base + 4, base + 5);
    mesh.indices.extend([
        px, py, pz, //
        nx, pz, py, //
        px, pz, ny, //
        nx, ny, pz, //
        px, nz, py, //
        nx, py, nz, //
        px, ny, nz, //
        nx, nz, ny, //
    ]);
}

/// The stage's cells cut into slabs: their meshes, ready to upload, and
/// what each is. A slab with no cells has no mesh and is left out.
pub(crate) fn stage_meshes(cloud: &Cloud, index: usize) -> Option<(Built, Vec<(u32, MeshData)>)> {
    let stage: &Stage = cloud.stages.get(index)?;
    let kept: Vec<_> = stage
        .points
        .iter()
        .filter(|p| p.pos.iter().map(|v| v * v).sum::<f32>() <= MAX_RADIUS * MAX_RADIUS)
        .collect();
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for p in &kept {
        lo = lo.min(p.pos[1]);
        hi = hi.max(p.pos[1]);
    }
    if !(lo.is_finite() && hi.is_finite()) {
        return Some((
            Built {
                label: stage.label.clone(),
                predicted: stage.predicted,
                board: stage.board.clone(),
                cells: 0,
                cardiac: 0,
                slabs: Vec::new(),
            },
            Vec::new(),
        ));
    }
    let span = (hi - lo).max(1e-3);
    let slab_of = |y: f32| (((y - lo) / span * SLABS as f32) as usize).min(SLABS - 1);
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); SLABS];
    for (i, p) in kept.iter().enumerate() {
        buckets[slab_of(p.pos[1])].push(i);
    }
    let mut built = Built {
        label: stage.label.clone(),
        predicted: stage.predicted,
        board: stage.board.clone(),
        cells: kept.len(),
        cardiac: kept.iter().filter(|p| p.cardiac).count(),
        slabs: Vec::new(),
    };
    let mut meshes = Vec::new();
    for (s, bucket) in buckets.iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }
        let mut mesh = MeshData::default();
        let mut parts = Vec::new();
        let mut centre = [0.0f32; 2];
        for &i in bucket {
            centre[0] += kept[i].pos[0];
            centre[1] += kept[i].pos[2];
        }
        centre = centre.map(|c| c / bucket.len() as f32);
        for part in [Part::Cardiac, Part::Context, Part::Halo] {
            let first = mesh.indices.len() as u32;
            for &i in bucket {
                let p = kept[i];
                let group = cloud.groups.get(usize::from(p.group));
                match part {
                    Part::Cardiac if p.cardiac => {
                        push_gem(&mut mesh, p.pos, CARDIAC_R, cell_colour(group, true))
                    }
                    Part::Context if !p.cardiac => {
                        push_gem(&mut mesh, p.pos, CONTEXT_R, cell_colour(group, false))
                    }
                    Part::Halo if p.cardiac => {
                        push_gem(&mut mesh, p.pos, HALO_R, cell_colour(group, true))
                    }
                    _ => {}
                }
            }
            let count = mesh.indices.len() as u32 - first;
            if count > 0 {
                mesh.groups.push((first, count));
                parts.push(part);
            }
        }
        let id = slab_mesh_id(index, s);
        built.slabs.push(Slab {
            id,
            band: [
                lo + span * s as f32 / SLABS as f32,
                lo + span * (s + 1) as f32 / SLABS as f32,
            ],
            centre,
            parts,
        });
        meshes.push((id, mesh));
    }
    Some((built, meshes))
}

/// The projector ring under the heart: two thin rings, a ring of ticks and
/// four brighter arcs, flat in XZ and facing up, radius about 1. White, with
/// the strength in the vertex alpha: the material's tint gives it its
/// colour.
pub(crate) fn ring_mesh() -> MeshData {
    let mut mesh = MeshData::default();
    let band =
        |mesh: &mut MeshData, r0: f32, r1: f32, a0: f32, a1: f32, alpha: f32, segments: u32| {
            for i in 0..segments {
                let (t0, t1) = (
                    a0 + (a1 - a0) * i as f32 / segments as f32,
                    a0 + (a1 - a0) * (i + 1) as f32 / segments as f32,
                );
                let base = mesh.positions.len() as u32;
                for (r, t) in [(r0, t0), (r1, t0), (r1, t1), (r0, t1)] {
                    mesh.positions.push([r * t.cos(), 0.0, r * t.sin()]);
                    mesh.normals.push([0.0, 1.0, 0.0]);
                    mesh.uvs.push([0.5, 0.5]);
                    mesh.colors.push([1.0, 1.0, 1.0, alpha]);
                }
                // Facing up (+Y): counter-clockwise seen from above.
                mesh.indices
                    .extend([base, base + 3, base + 2, base, base + 2, base + 1]);
            }
        };
    let tau = std::f32::consts::TAU;
    band(&mut mesh, 0.985, 1.0, 0.0, tau, 0.85, 96);
    band(&mut mesh, 0.80, 0.806, 0.0, tau, 0.45, 72);
    band(&mut mesh, 1.07, 1.075, 0.0, tau, 0.30, 96);
    for k in 0..4 {
        let a = tau * k as f32 / 4.0 + 0.2;
        band(&mut mesh, 1.02, 1.05, a, a + 0.62, 0.8, 12);
    }
    for k in 0..72 {
        let a = tau * k as f32 / 72.0;
        let long = k % 6 == 0;
        band(
            &mut mesh,
            0.86,
            if long { 0.95 } else { 0.91 },
            a - 0.006,
            a + 0.006,
            if long { 0.75 } else { 0.4 },
            1,
        );
    }
    mesh
}

/// Where the cycle is: the stage coming in (`to`), the one it replaces
/// (`from`, `None` while the first one builds), and how far the scan has
/// come (0 to 1; 1 is settled).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Moment {
    pub(crate) from: Option<usize>,
    pub(crate) to: usize,
    pub(crate) k: f32,
}

impl Moment {
    /// Whether a scan is running.
    pub(crate) fn scanning(&self) -> bool {
        self.k < 1.0
    }
}

/// The cycle through `n` stages at `t` seconds: each held [`HOLD`] seconds
/// after a [`SCAN`] that builds it over the last. The first stage builds
/// from nothing. `None` with no stages.
pub(crate) fn moment(t: f32, n: usize) -> Option<Moment> {
    if n == 0 {
        return None;
    }
    let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
    let slot = HOLD + SCAN;
    let i = (t / slot).floor() as u64;
    let u = t - i as f32 * slot;
    let to = (i % n as u64) as usize;
    let from = (i > 0 && n > 1).then(|| ((i - 1) % n as u64) as usize);
    let k = if (i == 0 || n > 1) && u < SCAN {
        (u / SCAN).clamp(0.0, 1.0)
    } else {
        1.0
    };
    Some(Moment { from, to, k })
}

pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let k = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// The scan's height at `k` (0 to 1).
pub(crate) fn scan_height(k: f32) -> f32 {
    SCAN_FROM + (SCAN_TO - SCAN_FROM) * smoothstep(0.0, 1.0, k)
}

/// How much of a slab spanning `band` is there, 0 to 1: the incoming
/// stage's slabs grow in as the scan passes them, the outgoing one's go.
pub(crate) fn presence(band: [f32; 2], scan: f32, incoming: bool) -> f32 {
    let mid = (band[0] + band[1]) * 0.5;
    let passed = smoothstep(mid - SCAN_SOFT, mid + SCAN_SOFT, scan);
    if incoming { passed } else { 1.0 - passed }
}

/// The heartbeat at `t`: a lub and a softer dub each beat, 0 to about 1.
pub(crate) fn beat(t: f32) -> f32 {
    let p = (t / BEAT_PERIOD).rem_euclid(1.0);
    let lub = p.min(1.0 - p);
    let pulse = |d: f32, w: f32| (-(d / w) * (d / w)).exp();
    pulse(lub, 0.045) + 0.55 * pulse(p - 0.24, 0.05)
}

/// The heart's trace at `t`, as a monitor draws it: a small P wave, the
/// sharp QRS spike on the beat's lub ([`beat`]'s peak) and a T wave on its
/// dub. About -0.3 to 1.
pub(crate) fn ecg(t: f32) -> f32 {
    let p = (t / BEAT_PERIOD).rem_euclid(1.0);
    let d = if p > 0.5 { p - 1.0 } else { p };
    let g = |at: f32, width: f32, height: f32| {
        height * (-((d - at) / width) * ((d - at) / width)).exp()
    };
    g(-0.16, 0.035, 0.14)
        + g(-0.025, 0.010, -0.14)
        + g(0.0, 0.011, 1.0)
        + g(0.028, 0.012, -0.30)
        + g(0.24, 0.055, 0.26)
}

/// How long a newly landed official score's flare lasts, and how fast it
/// fades.
pub(crate) const FLARE_SECONDS: f32 = 8.0;
pub(crate) const FLARE_FADE: f32 = 1.8;

/// A newly landed official score's flare: when it began (the panel's
/// clock) and the row of the board it landed on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flare {
    pub(crate) at: f32,
    pub(crate) row: usize,
}

impl Flare {
    /// How strong it is at `t`: up within a few frames, then fading.
    pub(crate) fn strength(&self, t: f32) -> f32 {
        let age = t - self.at;
        if !(0.0..FLARE_SECONDS).contains(&age) {
            return 0.0;
        }
        (age / 0.15).min(1.0) * (-(age / FLARE_FADE)).exp()
    }

    /// Whether it has run its course by `t`. Its first frame is not: it
    /// starts from nothing.
    pub(crate) fn over(&self, t: f32) -> bool {
        t - self.at >= FLARE_SECONDS
    }
}

/// The camera: a little above the heart, looking slightly down at it, the
/// heart's top and the ring's near edge both inside its view.
pub(crate) fn camera() -> Camera {
    let pitch = quat_axis_angle([1.0, 0.0, 0.0], -0.10);
    Camera {
        transform: Transform {
            translation: [0.0, 0.32, 4.8],
            rotation: pitch,
            scale: [1.0; 3],
        },
        projection: Projection::Perspective {
            fov_y: 38.0_f32.to_radians(),
            near: 0.5,
            far: 30.0,
        },
        clear: Some([0.0; 4]),
    }
}

/// Where world point `p` lands in a target `size` pixels, as fractions of
/// its width and height (0, 0 top left), or `None` behind the eye.
pub(crate) fn project(p: [f32; 3], size: (u32, u32)) -> Option<[f32; 2]> {
    let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
    let vp = camera().view_projection(aspect)?;
    let clip = mat_point(&vp, p);
    let w = vp[0][3] * p[0] + vp[1][3] * p[1] + vp[2][3] * p[2] + vp[3][3];
    if w <= 1e-6 {
        return None;
    }
    // `mat_point` leaves the perspective divide to its caller.
    Some([(clip[0] / w + 1.0) * 0.5, (1.0 - clip[1] / w) * 0.5])
}

/// The ring's colour: cyan for an observed stage, gold for ours.
pub(crate) const RING_OBSERVED: [f32; 3] = [0.35, 0.85, 1.0];
pub(crate) const RING_OURS: [f32; 3] = [1.0, 0.78, 0.30];

/// What the scene shows besides the stages: the clock, the beat's
/// strength now (with a flare after a new official score), and whether the
/// stage in front is ours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    pub(crate) t: f32,
    /// 0 to 1: how strong the flare of a newly landed official score is.
    pub(crate) flare: f32,
}

fn material(part: Part) -> Material {
    match part {
        Part::Cardiac => Material::lit([1.0; 4], CARDIAC_EMISSIVE),
        Part::Context => Material::lit([0.85, 0.9, 1.0, 1.0], CONTEXT_EMISSIVE),
        Part::Halo => Material {
            alpha: AlphaMode::Blend,
            ..Material::color([1.0, 1.0, 1.0, HALO_ALPHA])
        },
    }
}

/// The scene at `pose`: the stages `built` in their cycle, turning on a
/// beating root over the projector ring, and the scan ring while a stage
/// builds. With no stages, the ring alone idles.
pub(crate) fn scene(built: &[Built], pose: Pose) -> Scene {
    let t = pose.t;
    let flare = pose.flare.clamp(0.0, 1.0);
    let swell = 1.0 + BEAT_SWELL * beat(t) * (1.0 + 2.0 * flare) + FLARE_SWELL * flare;
    let mut nodes = Vec::with_capacity(2 * SLABS + 4);
    let mut root = Node::empty(1);
    root.transform = Transform {
        translation: [0.0, 0.0, 0.0],
        rotation: quat_axis_angle([0.0, 1.0, 0.0], t * SPIN),
        scale: [swell; 3],
    };
    nodes.push(root);
    let now = moment(t, built.len());
    let ours = now.is_some_and(|m| built[m.to].predicted);
    let ring_rgb = if ours { RING_OURS } else { RING_OBSERVED };
    if let Some(m) = now {
        let scan = scan_height(m.k);
        let mut id = 100u64;
        let mut place = |stage: &Built, incoming: bool, nodes: &mut Vec<Node>| {
            for slab in &stage.slabs {
                id += 1;
                let there = if m.scanning() {
                    presence(slab.band, scan, incoming)
                } else if incoming {
                    1.0
                } else {
                    0.0
                };
                if there < 0.02 {
                    continue;
                }
                let s = there.powf(0.7);
                let mut node = Node::new(id, MeshRef::Uploaded(slab.id), material(slab.parts[0]));
                node.parent = Some(1);
                node.face_materials = slab.parts.iter().map(|&p| material(p)).collect();
                // Narrowed toward the slab's own middle, across only: the
                // scan rebuilds a stage in layers, as a printer would.
                node.transform = Transform {
                    translation: [slab.centre[0] * (1.0 - s), 0.0, slab.centre[1] * (1.0 - s)],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [s, 1.0, s],
                };
                nodes.push(node);
            }
        };
        if m.scanning()
            && let Some(from) = m.from
        {
            place(&built[from], false, &mut nodes);
        }
        place(&built[m.to], true, &mut nodes);
        if m.scanning() {
            // The scan ring, riding up through the heart.
            let mut hoop = Node::new(
                20,
                MeshRef::Uploaded(RING_MESH),
                Material {
                    alpha: AlphaMode::Blend,
                    double_sided: true,
                    ..Material::color([0.75, 0.95, 1.0, 0.9])
                },
            );
            hoop.parent = Some(1);
            let fade = smoothstep(0.0, 0.08, m.k) * (1.0 - smoothstep(0.9, 1.0, m.k));
            hoop.material.tint[3] = 0.9 * fade;
            hoop.transform = Transform {
                translation: [0.0, scan, 0.0],
                rotation: quat_axis_angle([0.0, 1.0, 0.0], -t * 1.3),
                scale: [1.18, 1.0, 1.18],
            };
            nodes.push(hoop);
        }
    }
    // The projector ring: not on the beating root, turning the other way.
    let mut ring = Node::new(
        10,
        MeshRef::Uploaded(RING_MESH),
        Material {
            alpha: AlphaMode::Blend,
            double_sided: true,
            ..Material::color([ring_rgb[0], ring_rgb[1], ring_rgb[2], 0.8 + 0.2 * flare])
        },
    );
    ring.transform = Transform {
        translation: [0.0, RING_Y, 0.0],
        rotation: quat_axis_angle([0.0, 1.0, 0.0], -t * 0.11),
        scale: [RING_SCALE; 3],
    };
    nodes.push(ring);
    Scene {
        camera: camera(),
        nodes,
        look: None,
    }
}
