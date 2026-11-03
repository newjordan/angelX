//! The sampler's glTF model, made in code: a brass orrery-lantern on a
//! slate plinth. Three arms turn on a hub, each holding a glowing gem
//! that bobs; an ember ring ticks round the hub; a glass dome breathes
//! over it; the plinth's top is a texture. Two clips: `orbit` (the arms
//! by linear rotation, the gems by cubic spline, the ring by step, the
//! dome by scale) and `tick` (the ring alone).

use super::glb::{self, Gltf};
use std::f32::consts::{PI, TAU};

/// A mesh's arrays before they go into the file.
#[derive(Default)]
struct Shape {
    positions: Vec<f32>,
    normals: Vec<f32>,
    colors: Vec<f32>,
    uvs: Vec<f32>,
    indices: Vec<u32>,
}

impl Shape {
    fn vertex(&mut self, p: [f32; 3], n: [f32; 3]) -> u32 {
        self.positions.extend(p);
        self.normals.extend(n);
        (self.positions.len() / 3 - 1) as u32
    }

    /// Into the file as a mesh of one primitive with `material`.
    fn add(&self, g: &mut Gltf, material: usize) -> usize {
        let mut attributes = vec![("POSITION", g.floats(&self.positions, "VEC3"))];
        if !self.normals.is_empty() {
            attributes.push(("NORMAL", g.floats(&self.normals, "VEC3")));
        }
        if !self.colors.is_empty() {
            attributes.push(("COLOR_0", g.floats(&self.colors, "VEC4")));
        }
        if !self.uvs.is_empty() {
            attributes.push(("TEXCOORD_0", g.floats(&self.uvs, "VEC2")));
        }
        let indices = g.indices(&self.indices);
        g.mesh(&attributes, Some(indices), Some(material))
    }
}

/// A prism or a tapered column round y: `sides` faces, flat-shaded when
/// `flat`, from `r0` at y 0 to `r1` at `h`, capped.
fn column(sides: u32, r0: f32, r1: f32, h: f32, flat: bool) -> Shape {
    let mut s = Shape::default();
    let slope = (r0 - r1) / h;
    for i in 0..sides {
        let (a0, a1) = (i as f32 / sides as f32 * TAU, (i + 1) as f32 / sides as f32 * TAU);
        let normal = |a: f32| {
            let n = [a.sin(), slope, a.cos()];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            [n[0] / l, n[1] / l, n[2] / l]
        };
        let (n0, n1) = if flat { (normal((a0 + a1) / 2.0), normal((a0 + a1) / 2.0)) } else { (normal(a0), normal(a1)) };
        let b0 = s.vertex([a0.sin() * r0, 0.0, a0.cos() * r0], n0);
        let b1 = s.vertex([a1.sin() * r0, 0.0, a1.cos() * r0], n1);
        let t1 = s.vertex([a1.sin() * r1, h, a1.cos() * r1], n1);
        let t0 = s.vertex([a0.sin() * r1, h, a0.cos() * r1], n0);
        s.indices.extend([b0, b1, t1, b0, t1, t0]);
    }
    for (y, r, up) in [(h, r1, 1.0f32), (0.0, r0, -1.0)] {
        let centre = s.vertex([0.0, y, 0.0], [0.0, up, 0.0]);
        for i in 0..sides {
            let (a0, a1) = (i as f32 / sides as f32 * TAU, (i + 1) as f32 / sides as f32 * TAU);
            let p0 = s.vertex([a0.sin() * r, y, a0.cos() * r], [0.0, up, 0.0]);
            let p1 = s.vertex([a1.sin() * r, y, a1.cos() * r], [0.0, up, 0.0]);
            if up > 0.0 {
                s.indices.extend([centre, p0, p1]);
            } else {
                s.indices.extend([centre, p1, p0]);
            }
        }
    }
    s
}

/// A sphere of radius `r`, `rings` from pole to pole, the upper half only
/// when `dome`.
fn sphere(r: f32, segments: u32, rings: u32, dome: bool) -> Shape {
    let mut s = Shape::default();
    let last = if dome { rings / 2 } else { rings };
    for j in 0..=last {
        let v = j as f32 / rings as f32 * PI;
        for i in 0..=segments {
            let u = i as f32 / segments as f32 * TAU;
            let n = [v.sin() * u.sin(), v.cos(), v.sin() * u.cos()];
            s.vertex([n[0] * r, n[1] * r, n[2] * r], n);
        }
    }
    let row = segments + 1;
    for j in 0..last {
        for i in 0..segments {
            let a = j * row + i;
            let b = a + row;
            s.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    s
}

/// A square bar `thick` across, reaching `length` along +x from the
/// origin: an arm.
fn arm(length: f32, thick: f32) -> Shape {
    let mut s = Shape::default();
    let (x0, x1, t) = (0.0, length, thick / 2.0);
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([1.0, 0.0, 0.0], [[x1, -t, t], [x1, -t, -t], [x1, t, -t], [x1, t, t]]),
        ([-1.0, 0.0, 0.0], [[x0, -t, -t], [x0, -t, t], [x0, t, t], [x0, t, -t]]),
        ([0.0, 1.0, 0.0], [[x0, t, t], [x1, t, t], [x1, t, -t], [x0, t, -t]]),
        ([0.0, -1.0, 0.0], [[x0, -t, -t], [x1, -t, -t], [x1, -t, t], [x0, -t, t]]),
        ([0.0, 0.0, 1.0], [[x0, -t, t], [x1, -t, t], [x1, t, t], [x0, t, t]]),
        ([0.0, 0.0, -1.0], [[x1, -t, -t], [x0, -t, -t], [x0, t, -t], [x1, t, -t]]),
    ];
    for (n, corners) in faces {
        let base = s.positions.len() as u32 / 3;
        for c in corners {
            s.vertex(c, n);
        }
        s.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    s
}

/// An octahedron, no normals: the reader makes its faces flat.
fn gem(r: f32) -> Shape {
    let mut s = Shape::default();
    let p = [[r, 0.0, 0.0], [-r, 0.0, 0.0], [0.0, r * 1.4, 0.0], [0.0, -r * 1.4, 0.0], [0.0, 0.0, r], [0.0, 0.0, -r]];
    for v in p {
        s.positions.extend(v);
    }
    s.indices = vec![2, 4, 0, 2, 0, 5, 2, 5, 1, 2, 1, 4, 3, 0, 4, 3, 5, 0, 3, 1, 5, 3, 4, 1];
    s
}

/// A ring round y: radius `big`, tube `small`.
fn torus(big: f32, small: f32, segments: u32, sides: u32) -> Shape {
    let mut s = Shape::default();
    for i in 0..=segments {
        let u = i as f32 / segments as f32 * TAU;
        for j in 0..=sides {
            let v = j as f32 / sides as f32 * TAU;
            let n = [v.cos() * u.sin(), v.sin(), v.cos() * u.cos()];
            let c = [u.sin() * big, 0.0, u.cos() * big];
            s.vertex([c[0] + n[0] * small, n[1] * small, c[2] + n[2] * small], n);
        }
    }
    let row = sides + 1;
    for i in 0..segments {
        for j in 0..sides {
            let a = i * row + j;
            let b = a + row;
            s.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    s
}

/// A flat square on y 0, facing up, uv over it.
fn plate(half: f32) -> Shape {
    let mut s = Shape::default();
    for (x, z, u, v) in [(-half, half, 0.0, 1.0), (half, half, 1.0, 1.0), (half, -half, 1.0, 0.0), (-half, -half, 0.0, 0.0)] {
        s.vertex([x, 0.0, z], [0.0, 1.0, 0.0]);
        s.uvs.extend([u, v]);
    }
    s.indices = vec![0, 1, 2, 0, 2, 3];
    s
}

/// The plinth's top: rings and spokes, pixel art, sampled nearest.
fn compass(side: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    let c = side as f32 / 2.0;
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = (dx * dx + dy * dy).sqrt() / c;
            let a = dy.atan2(dx);
            let ring = ((r * 9.0).fract() < 0.12) && r < 0.95;
            let spoke = (a * 6.0 / PI).rem_euclid(1.0) < 0.06 && r > 0.25 && r < 0.9;
            let base = if r > 0.97 { [30, 34, 46] } else { [44, 52, 70] };
            let px = if ring {
                [90, 230, 255]
            } else if spoke {
                [255, 170, 80]
            } else {
                base
            };
            rgba.extend([px[0], px[1], px[2], 255]);
        }
    }
    rgba
}

fn quat_y(angle: f32) -> [f32; 4] {
    [0.0, (angle / 2.0).sin(), 0.0, (angle / 2.0).cos()]
}

fn list(values: &[f32]) -> String {
    values.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(",")
}

/// What the file holds, for the sampler's readout.
pub struct Summary {
    pub nodes: usize,
    pub triangles: usize,
    pub clips: usize,
}

/// The model as a `.glb`, and what it holds.
pub fn lantern() -> (Vec<u8>, Summary) {
    let mut g = Gltf::default();
    let slate = g.material(r#"{"pbrMetallicRoughness":{"baseColorFactor":[0.36,0.4,0.5,1],"metallicFactor":0,"roughnessFactor":0.9}}"#.into());
    let brass = g.material(r#"{"pbrMetallicRoughness":{"baseColorFactor":[0.95,0.72,0.34,1],"metallicFactor":0.9,"roughnessFactor":0.45}}"#.into());
    let glows: Vec<usize> = [[0.3, 0.95, 1.0], [1.0, 0.35, 0.75], [1.0, 0.8, 0.3]]
        .iter()
        .map(|c| {
            g.material(format!(
                r#"{{"pbrMetallicRoughness":{{"baseColorFactor":[{},1]}},"emissiveFactor":[{}]}}"#,
                list(c),
                list(&[c[0] * 0.8, c[1] * 0.8, c[2] * 0.8])
            ))
        })
        .collect();
    let ember = g.material(r#"{"pbrMetallicRoughness":{"baseColorFactor":[1,0.45,0.15,1]},"emissiveFactor":[0.9,0.4,0.12]}"#.into());
    let glass = g.material(r#"{"pbrMetallicRoughness":{"baseColorFactor":[0.65,0.85,1,0.22],"metallicFactor":0,"roughnessFactor":0.1},"alphaMode":"BLEND","doubleSided":true}"#.into());
    let texture = g.png_texture(&glb::png(64, 64, &compass(64)), true);
    let face = g.material(format!(r#"{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":{texture}}},"metallicFactor":0,"roughnessFactor":1}}}}"#));

    let mut plinth = column(6, 1.15, 1.0, 0.35, true);
    // Vertex colours: the plinth darker at its foot.
    for chunk in plinth.positions.chunks(3) {
        let lift = 0.55 + chunk[1] / 0.35 * 0.45;
        plinth.colors.extend([lift, lift, lift, 1.0]);
    }
    let plinth_mesh = plinth.add(&mut g, slate);
    let top_mesh = plate(0.86).add(&mut g, face);
    let column_mesh = column(16, 0.2, 0.12, 1.25, false).add(&mut g, brass);
    let hub_mesh = sphere(0.24, 20, 12, false).add(&mut g, brass);
    let arm_mesh = arm(0.82, 0.07).add(&mut g, brass);
    let gem_meshes: Vec<usize> = glows.iter().map(|m| gem(0.15).add(&mut g, *m)).collect();
    let ring_mesh = torus(0.5, 0.035, 48, 10).add(&mut g, ember);
    let dome_mesh = sphere(0.4, 28, 16, true).add(&mut g, glass);
    let triangles = g
        .accessors
        .iter()
        .filter(|a| a.contains(r#""componentType":5123"#) || a.contains(r#""componentType":5125"#))
        .filter_map(|a| a.split(r#""count":"#).nth(1)?.split(',').next()?.parse::<usize>().ok())
        .sum::<usize>()
        / 3;

    // Nodes: the gems, the arms that hold them, the ring, the dome, the
    // hub that turns them, the column, the plinth, the root.
    let mut gems = Vec::new();
    for mesh in &gem_meshes {
        gems.push(g.node(format!(r#"{{"mesh":{mesh},"translation":[0.98,0,0]}}"#)));
    }
    let mut arms = Vec::new();
    for (k, gem) in gems.iter().enumerate() {
        let q = quat_y(k as f32 * TAU / 3.0);
        arms.push(g.node(format!(r#"{{"mesh":{arm_mesh},"rotation":[{}],"children":[{gem}]}}"#, list(&q))));
    }
    let ring = g.node(format!(r#"{{"mesh":{ring_mesh}}}"#));
    let dome = g.node(format!(r#"{{"mesh":{dome_mesh},"translation":[0,0.02,0]}}"#));
    let hub_children: Vec<String> = arms.iter().chain([&ring, &dome]).map(|n| n.to_string()).collect();
    let hub = g.node(format!(r#"{{"mesh":{hub_mesh},"translation":[0,1.62,0],"children":[{}]}}"#, hub_children.join(",")));
    let column_node = g.node(format!(r#"{{"mesh":{column_mesh},"translation":[0,0.35,0]}}"#));
    let top = g.node(format!(r#"{{"mesh":{top_mesh},"translation":[0,0.352,0]}}"#));
    let plinth_node = g.node(format!(r#"{{"mesh":{plinth_mesh}}}"#));
    let root = g.node(format!(r#"{{"children":[{plinth_node},{top},{column_node},{hub}]}}"#));
    let scene = g.add_scene(&[root]);
    g.scene = Some(scene);

    // Clip 0, `orbit`, six seconds.
    let keys = [0.0f32, 1.5, 3.0, 4.5, 6.0];
    let hub_t = g.floats(&keys, "SCALAR");
    let turns: Vec<f32> = keys.iter().flat_map(|t| quat_y(t / 6.0 * TAU)).collect();
    let hub_r = g.floats(&turns, "VEC4");
    let mut channels = vec![format!(r#"{{"sampler":0,"target":{{"node":{hub},"path":"rotation"}}}}"#)];
    let mut samplers = vec![format!(r#"{{"input":{hub_t},"output":{hub_r},"interpolation":"LINEAR"}}"#)];
    // Each gem bobs on a sine, a third of a turn apart: a cubic spline
    // through its quarter points, with the sine's own slope as tangents.
    for (k, gem) in gems.iter().enumerate() {
        let (amp, w, phase) = (0.16f32, TAU / 6.0, k as f32 * TAU / 3.0);
        let mut out = Vec::new();
        for t in keys {
            let slope = amp * w * (w * t + phase).cos();
            let y = amp * (w * t + phase).sin();
            out.extend([0.0, slope, 0.0, 0.98, y, 0.0, 0.0, slope, 0.0]);
        }
        let o = g.floats(&out, "VEC3");
        channels.push(format!(r#"{{"sampler":{},"target":{{"node":{gem},"path":"translation"}}}}"#, samplers.len()));
        samplers.push(format!(r#"{{"input":{hub_t},"output":{o},"interpolation":"CUBICSPLINE"}}"#));
    }
    // The ring ticks back a twelfth of a turn every half second.
    let ticks: Vec<f32> = (0..=12).map(|i| i as f32 * 0.5).collect();
    let tick_t = g.floats(&ticks, "SCALAR");
    let tick_r = g.floats(&ticks.iter().flat_map(|t| quat_y(-t / 6.0 * TAU)).collect::<Vec<_>>(), "VEC4");
    channels.push(format!(r#"{{"sampler":{},"target":{{"node":{ring},"path":"rotation"}}}}"#, samplers.len()));
    samplers.push(format!(r#"{{"input":{tick_t},"output":{tick_r},"interpolation":"STEP"}}"#));
    // The dome breathes.
    let dome_t = g.floats(&[0.0, 3.0, 6.0], "SCALAR");
    let dome_s = g.floats(&[1.0, 1.0, 1.0, 1.08, 1.12, 1.08, 1.0, 1.0, 1.0], "VEC3");
    channels.push(format!(r#"{{"sampler":{},"target":{{"node":{dome},"path":"scale"}}}}"#, samplers.len()));
    samplers.push(format!(r#"{{"input":{dome_t},"output":{dome_s}}}"#));
    g.animation(format!(r#"{{"channels":[{}],"samplers":[{}]}}"#, channels.join(","), samplers.join(",")));
    // Clip 1, `tick`: the ring alone.
    g.animation(format!(
        r#"{{"channels":[{{"sampler":0,"target":{{"node":{ring},"path":"rotation"}}}}],"samplers":[{{"input":{tick_t},"output":{tick_r},"interpolation":"STEP"}}]}}"#
    ));
    let summary = Summary { nodes: g.nodes.len(), triangles, clips: g.animations.len() };
    (g.glb(), summary)
}
