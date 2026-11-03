//! glTF assets: files made in code (the sampler's own writer), read whole
//! and broken every way the reader must refuse, the node tree's maths
//! against hand-computed matrices, clip sampling, and the `asset` and
//! `play` ops through the store as a program sends them.

#[path = "../../examples/glass_sampler/glb.rs"]
mod glb;
#[path = "../../examples/glass_sampler/model.rs"]
mod model;

use super::glass_gltf::*;
use super::testing::FakeEngine;
use super::*;
use angel_glass::scene::{mat_point, AlphaMode, Camera, MeshRef, Scene, TextureSource, Transform};
use glb::Gltf;

/// Base64, as a program sends its payloads.
pub(super) fn b64(bytes: &[u8]) -> String {
    glb::base64(bytes)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn near3(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|k| near(a[k], b[k]))
}

/// A unit quad in XY, two triangles, with normals, uvs, colours and a
/// material, hung one node below a root.
fn quad_file() -> Gltf {
    let mut g = Gltf::default();
    let pos = g.floats(&[0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 0.], "VEC3");
    let nor = g.floats(&[0., 0., 1., 0., 0., 1., 0., 0., 1., 0., 0., 1.], "VEC3");
    let uv = g.floats(&[0., 1., 1., 1., 1., 0., 0., 0.], "VEC2");
    let col = g.floats(&[1., 0., 0., 1., 0., 1., 0., 1., 0., 0., 1., 1., 1., 1., 1., 0.5], "VEC4");
    let idx = g.indices(&[0, 1, 2, 0, 2, 3]);
    let mat = g.material(r#"{"pbrMetallicRoughness":{"baseColorFactor":[0.5,0.25,1,1],"metallicFactor":0,"roughnessFactor":1}}"#.into());
    let mesh = g.mesh(&[("POSITION", pos), ("NORMAL", nor), ("TEXCOORD_0", uv), ("COLOR_0", col)], Some(idx), Some(mat));
    let child = g.node(format!(r#"{{"mesh":{mesh},"translation":[1,0,0]}}"#));
    let root = g.node(format!(r#"{{"children":[{child}],"translation":[0,2,0]}}"#));
    g.add_scene(&[root]);
    g
}

fn code(result: Result<Loaded, super::glass_scene::WireError>) -> (&'static str, String) {
    match result {
        Ok(_) => ("OK", String::new()),
        Err(e) => (e.code, e.message),
    }
}

// ------------------------------------------------------------ sound files

#[test]
fn a_glb_made_in_code_loads_its_meshes_materials_and_tree() {
    let loaded = load(&quad_file().glb()).expect("the quad loads");
    let a = &loaded.asset;
    assert_eq!(a.nodes.len(), 2);
    assert_eq!(a.scenes, vec![vec![1]]);
    assert_eq!(a.nodes[0].parent, Some(1));
    assert_eq!(a.nodes[1].children, vec![0]);
    assert_eq!(a.prims.len(), 1);
    assert_eq!((a.vertices, a.triangles), (4, 2));
    let mesh = &a.prims[0].mesh;
    assert_eq!(mesh.positions[2], [1.0, 1.0, 0.0]);
    assert_eq!(mesh.normals.len(), 4);
    assert_eq!(mesh.uvs[0], [0.0, 1.0]);
    assert_eq!(mesh.colors[3], [1.0, 1.0, 1.0, 0.5]);
    assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
    let m = &a.materials[a.prims[0].material];
    assert!(m.material.lit, "a glTF material is lit");
    assert_eq!(m.material.tint, [0.5, 0.25, 1.0, 1.0]);
    assert_eq!(m.material.alpha, AlphaMode::Opaque);
    assert_eq!(a.bytes, 4 * VERTEX_BYTES + 6 * INDEX_BYTES);
}

#[test]
fn a_gltf_with_its_buffer_as_a_data_uri_loads_the_same() {
    let file = quad_file();
    let from_glb = load(&file.glb()).expect("glb");
    let from_gltf = load(&file.gltf()).expect("gltf");
    assert_eq!(from_glb.asset.prims[0].mesh, from_gltf.asset.prims[0].mesh);
    assert_eq!(from_glb.asset.nodes.len(), from_gltf.asset.nodes.len());
}

#[test]
fn a_base_colour_texture_is_decoded_and_its_uv_set_taken() {
    let mut g = quad_file();
    let rgba: Vec<u8> = (0..4 * 4).flat_map(|i| [i as u8 * 16, 0, 255 - i as u8 * 16, 255]).collect();
    let tex = g.png_texture(&glb::png(4, 4, &rgba), true);
    g.materials[0] = format!(r#"{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":{tex}}}}},"alphaMode":"BLEND","doubleSided":true}}"#);
    let loaded = load(&g.glb()).expect("textured quad");
    assert_eq!(loaded.images.len(), 1);
    assert_eq!((loaded.images[0].width, loaded.images[0].height), (4, 4));
    assert_eq!(&loaded.images[0].rgba[..4], &[0, 0, 255, 255]);
    let m = &loaded.asset.materials[0];
    assert_eq!(m.image, Some(0));
    assert_eq!(m.material.sampling, angel_glass::scene::Sampling::Nearest);
    assert_eq!(m.material.alpha, AlphaMode::Blend);
    assert!(m.material.double_sided);
    assert_eq!(m.pbr.base_texture, Some((tex, 0)));
}

#[test]
fn materials_map_metallic_roughness_and_emissive_onto_the_lit_path() {
    let shiny = Pbr { base: [0.8, 0.6, 0.4, 1.0], metallic: 1.0, roughness: 0.0, ..Default::default() };
    let m = lit_material(&shiny, angel_glass::scene::Sampling::Linear, false);
    assert!(near(m.tint[0], 0.4) && near(m.tint[1], 0.3) && near(m.tint[2], 0.2), "a smooth metal is darkened by half: {:?}", m.tint);
    let rough = Pbr { base: [0.8, 0.6, 0.4, 1.0], metallic: 1.0, roughness: 1.0, ..Default::default() };
    assert_eq!(lit_material(&rough, angel_glass::scene::Sampling::Linear, false).tint, [0.8, 0.6, 0.4, 1.0]);
    let glow = Pbr { base: [0.0, 0.0, 0.0, 1.0], emissive: [1.0, 0.5, 0.0], ..Default::default() };
    let m = lit_material(&glow, angel_glass::scene::Sampling::Linear, true);
    assert_eq!(m.emissive, 1.0);
    assert_eq!(m.tint, [1.0, 0.5, 0.0, 1.0], "the emissive colour is what shows");
    assert_eq!(m.alpha, AlphaMode::Blend);
    // The file's numbers are kept for a physically based path.
    let mut g = quad_file();
    g.materials[0] = r#"{"pbrMetallicRoughness":{"metallicFactor":0.25,"roughnessFactor":0.75},"emissiveFactor":[0.1,0.2,0.3],"alphaMode":"MASK","alphaCutoff":0.4}"#.into();
    let pbr = load(&g.glb()).expect("pbr").asset.materials[0].pbr.clone();
    assert_eq!((pbr.metallic, pbr.roughness, pbr.emissive), (0.25, 0.75, [0.1, 0.2, 0.3]));
    assert!(pbr.masked);
    assert_eq!(pbr.alpha_cutoff, Some(0.4));
}

#[test]
fn a_primitive_without_normals_takes_flat_ones() {
    let mut g = Gltf::default();
    let pos = g.floats(&[0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.], "VEC3");
    let idx = g.indices(&[0, 1, 2, 0, 3, 1]);
    let mesh = g.mesh(&[("POSITION", pos)], Some(idx), None);
    g.node(format!(r#"{{"mesh":{mesh}}}"#));
    let a = load(&g.glb()).expect("flat").asset;
    let mesh = &a.prims[0].mesh;
    assert_eq!(mesh.positions.len(), 6, "each triangle has its own three vertices");
    assert!(near3(mesh.normals[0], [0.0, 0.0, 1.0]));
    // (0,0,0), (0,0,1), (1,0,0): counter-clockwise seen from +y.
    assert!(near3(mesh.normals[3], [0.0, 1.0, 0.0]));
    assert_eq!(a.vertices, 6);
}

#[test]
fn strips_and_fans_become_triangle_lists_and_points_are_skipped() {
    let mut g = Gltf::default();
    let pos = g.floats(&[0., 0., 0., 1., 0., 0., 0., 1., 0., 1., 1., 0., 2., 0., 0.], "VEC3");
    let nor = g.floats(&[0., 0., 1.].repeat(5), "VEC3");
    let strip = glb::primitive(&[("POSITION", pos), ("NORMAL", nor)], None, None, Some(5));
    let fan = glb::primitive(&[("POSITION", pos), ("NORMAL", nor)], None, None, Some(6));
    let points = glb::primitive(&[("POSITION", pos)], None, None, Some(0));
    let mesh = g.mesh_json(format!(r#"{{"primitives":[{strip},{fan},{points}]}}"#));
    g.node(format!(r#"{{"mesh":{mesh}}}"#));
    let a = load(&g.glb()).expect("strip and fan").asset;
    assert_eq!(a.prims.len(), 2);
    // glTF's strip: triangle i is p(i), p(i + 1 + i % 2), p(i + 2 - i % 2).
    assert_eq!(a.prims[0].mesh.indices, vec![0, 1, 2, 1, 3, 2, 2, 3, 4], "a strip keeps every triangle's winding");
    assert_eq!(a.prims[1].mesh.indices, vec![1, 2, 0, 2, 3, 0, 3, 4, 0]);
    assert_eq!(a.skipped.primitives, 1);
}

#[test]
fn the_samplers_model_loads_with_step_linear_and_cubic_spline_keys() {
    let (file, summary) = model::lantern();
    let loaded = load(&file).expect("the sampler's model");
    let a = &loaded.asset;
    assert_eq!(a.nodes.len(), summary.nodes);
    assert_eq!(a.triangles, summary.triangles);
    assert_eq!((a.clips.len(), loaded.images.len()), (summary.clips, 1));
    assert_eq!(summary.clips, 2);
    assert!((a.clips[0].duration - 6.0).abs() < 1e-6);
    let kinds: Vec<Interp> = a.clips[0].channels.iter().map(|c| c.interp).collect();
    for kind in [Interp::Linear, Interp::Step, Interp::CubicSpline] {
        assert!(kinds.contains(&kind), "{kind:?} in {kinds:?}");
    }
    // The first gem's spline runs through a sine's quarter points with the
    // sine's own slopes, so between them it stays on the sine.
    let gem = a.clips[0].channels.iter().find(|c| c.interp == Interp::CubicSpline).unwrap();
    for t in [0.4f32, 1.1, 2.6, 5.2] {
        let want = 0.16 * (std::f32::consts::TAU / 6.0 * t).sin();
        assert!((gem.sample(t)[1] - want).abs() < 0.005, "at {t}: {} against {want}", gem.sample(t)[1]);
    }
    assert!(a.materials.iter().any(|m| m.material.alpha == AlphaMode::Blend && m.material.double_sided), "the glass dome");
    assert!(a.materials.iter().any(|m| m.material.emissive > 0.5), "the gems glow");
}

// ------------------------------------------------------------ the tree

#[test]
fn the_tree_places_nodes_by_hand_computed_matrices() {
    // Parent: moved (1, 0, 0), a quarter turn about z, scaled 2. Child:
    // moved (1, 0, 0) under it. The child's origin is the parent's local
    // (1, 0, 0): scaled to (2, 0, 0), turned to (0, 2, 0), moved to (1, 2, 0).
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let mut g = Gltf::default();
    let child = g.node(r#"{"translation":[1,0,0],"scale":[1,3,1]}"#.into());
    let parent = g.node(format!(r#"{{"children":[{child}],"translation":[1,0,0],"rotation":[0,0,{h},{h}],"scale":[2,2,2]}}"#));
    g.add_scene(&[parent]);
    let asset = std::sync::Arc::new(load(&g.glb()).expect("tree").asset);
    let mut scene = Scene::new(Camera::default());
    scene.nodes.push(angel_glass::scene::Node::empty(7));
    scene.nodes[0].transform = Transform::from_translation([0.0, 0.0, -5.0]);
    let inst = instantiate(&asset, 1, 0, 7, 0, 100, &mut scene.nodes).expect("hang");
    let worlds = scene.world_matrices();
    let child_world = worlds[inst.places[child].unwrap() as usize].expect("placed");
    assert!(near3(mat_point(&child_world, [0.0, 0.0, 0.0]), [1.0, 2.0, -5.0]));
    // The child's own +y, scaled 3 then 2, turned to -x: (0, 1, 0) lands
    // at (1, 2, -5) + (-6, 0, 0).
    assert!(near3(mat_point(&child_world, [0.0, 1.0, 0.0]), [-5.0, 2.0, -5.0]));
    // +x of the child is the parent's +x scaled 2, turned to +y.
    assert!(near3(mat_point(&child_world, [1.0, 0.0, 0.0]), [1.0, 4.0, -5.0]));
}

#[test]
fn a_node_matrix_decomposes_into_its_translation_rotation_and_scale() {
    // Column-major: T(3, 4, 5) * Rz(90) * S(1, 2, 3).
    let m = [0.0, 1.0, 0.0, 0.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 3.0, 4.0, 5.0, 1.0];
    let t = decompose(&m).expect("finite");
    assert!(near3(t.translation, [3.0, 4.0, 5.0]));
    assert!(near3(t.scale, [1.0, 2.0, 3.0]));
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let q = t.rotation;
    assert!(near(q[2].abs(), h) && near(q[3].abs(), h) && near(q[0], 0.0) && near(q[1], 0.0), "{q:?}");
    // Rebuilt, it is the same matrix.
    let back = t.matrix();
    for c in 0..4 {
        for r in 0..4 {
            assert!(near(back[c][r], m[c * 4 + r]), "{back:?}");
        }
    }
    // A mirror keeps its handedness in the scale.
    let mirror = [-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let t = decompose(&mirror).unwrap();
    assert!(near3(t.scale, [-1.0, 1.0, 1.0]));
    assert!(decompose(&[f32::NAN; 16]).is_none());
}

#[test]
fn cyclic_doubly_parented_and_deep_trees_are_refused() {
    let tree = |nodes: Vec<String>| {
        let g = Gltf { nodes, ..Default::default() };
        code(load(&g.glb()))
    };
    let (c, m) = tree(vec![r#"{"children":[0]}"#.into()]);
    assert_eq!(c, "EINVAL");
    assert!(m.contains("cycles"), "{m}");
    let (c, m) = tree(vec![r#"{"children":[1]}"#.into(), r#"{"children":[0]}"#.into()]);
    assert_eq!(c, "EINVAL");
    assert!(m.contains("cycle"), "{m}");
    let (c, m) = tree(vec![r#"{"children":[2]}"#.into(), r#"{"children":[2]}"#.into(), "{}".into()]);
    assert_eq!(c, "EINVAL");
    assert!(m.contains("two parents"), "{m}");
    let (c, m) = tree(vec![r#"{"children":[9]}"#.into()]);
    assert_eq!(c, "EINVAL");
    assert!(m.contains("out of range"), "{m}");
    let chain = |n: usize| (0..n).map(|i| if i + 1 < n { format!(r#"{{"children":[{}]}}"#, i + 1) } else { "{}".into() }).collect();
    assert_eq!(tree(chain(ASSET_DEPTH)).0, "OK");
    let (c, m) = tree(chain(ASSET_DEPTH + 1));
    assert_eq!(c, "EINVAL");
    assert!(m.contains("deeper"), "{m}");
    let (c, _) = tree((0..=ASSET_NODES).map(|_| "{}".to_string()).collect());
    assert_eq!(c, "EFBIG");
    // A scene root that has a parent, and a default scene past the end.
    let mut g = Gltf { nodes: vec![r#"{"children":[1]}"#.into(), "{}".into()], ..Default::default() };
    g.add_scene(&[1]);
    assert!(code(load(&g.glb())).1.contains("not a root"));
    let mut g = Gltf::default();
    g.node("{}".into());
    g.scene = Some(3);
    assert_eq!(code(load(&g.glb())).0, "EINVAL");
}

// -------------------------------------------------------- broken files

#[test]
fn every_truncation_of_a_glb_is_refused_and_none_panics() {
    let whole = quad_file().glb();
    for len in 0..whole.len() {
        let (c, _) = code(load(&whole[..len]));
        assert_ne!(c, "OK", "a glb cut at {len} of {} bytes", whole.len());
    }
    assert_eq!(code(load(&whole)).0, "OK");
    // A .gltf cut short is not JSON.
    let text = quad_file().gltf();
    for len in [0, 1, text.len() / 3, text.len() / 2, text.len() - 1] {
        assert_ne!(code(load(&text[..len])).0, "OK", "a gltf cut at {len}");
    }
}

#[test]
fn chunk_and_header_lengths_that_lie_are_refused() {
    let whole = quad_file().glb();
    let put = |at: usize, v: u32| {
        let mut b = whole.clone();
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        b
    };
    let json_len = u32::from_le_bytes(whole[12..16].try_into().unwrap()) as usize;
    let bin_at = 20 + json_len;
    for (what, bytes) in [
        ("JSON chunk past the file", put(12, u32::MAX - 3)),
        ("JSON chunk as long as the file", put(12, whole.len() as u32)),
        ("BIN chunk past the file", put(bin_at, u32::MAX)),
        ("total under the header", put(8, 4)),
        ("total past the data", put(8, whole.len() as u32 + 4)),
        ("version 1", put(4, 1)),
        ("first chunk BIN", put(16, 0x004E_4942)),
    ] {
        let (c, m) = code(load(&bytes));
        assert!(c == "EBADF" || c == "EINVAL", "{what}: {c} {m}");
    }
    // The BIN chunk cut short of the buffer's byteLength.
    let mut short = whole.clone();
    let bin_len = u32::from_le_bytes(short[bin_at..bin_at + 4].try_into().unwrap());
    short[bin_at..bin_at + 4].copy_from_slice(&(bin_len - 8).to_le_bytes());
    let total = short.len() as u32 - 8;
    short[8..12].copy_from_slice(&total.to_le_bytes());
    short.truncate(short.len() - 8);
    let (c, m) = code(load(&short));
    assert_eq!(c, "EBADF", "{m}");
    assert!(m.contains("shorter than its byteLength"), "{m}");
}

#[test]
fn huge_accessor_counts_are_refused_before_anything_is_read() {
    let with_count = |count: u64, which: usize| {
        let mut g = quad_file();
        let text = g.accessors[which].clone();
        let at = text.find("\"count\":").unwrap() + 8;
        let end = at + text[at..].find(',').unwrap();
        g.accessors[which] = format!("{}{count}{}", &text[..at], &text[end..]);
        code(load(&g.glb()))
    };
    let (c, m) = with_count(4_000_000_000, 0);
    assert_eq!(c, "EFBIG", "{m}");
    let (c, m) = with_count(u64::MAX, 0);
    assert_eq!(c, "EFBIG", "{m}");
    // Within the caps but past the bytes its view has.
    let (c, m) = with_count(5, 0);
    assert_eq!(c, "EBADF", "{m}");
    assert!(m.contains("runs past"), "{m}");
    // The indices, past the cap and past their view.
    let (c, _) = with_count(4_000_000_000, 4);
    assert_eq!(c, "EFBIG");
    let (c, _) = with_count(9, 4);
    assert_eq!(c, "EBADF");
    // A stride that lies, a view past its buffer.
    let mut g = quad_file();
    g.views[0] = g.views[0].replace(r#""byteLength":48"#, r#""byteLength":48,"byteStride":4"#);
    assert_eq!(code(load(&g.glb())).0, "EINVAL");
    let mut g = quad_file();
    g.views[0] = g.views[0].replace(r#""byteLength":48"#, r#""byteLength":4000000"#);
    assert_eq!(code(load(&g.glb())).0, "EBADF");
}

#[test]
fn indices_and_references_out_of_range_are_refused() {
    let refused = |g: &Gltf, needle: &str| {
        let (c, m) = code(load(&g.glb()));
        assert_eq!(c, "EINVAL", "{needle}: {m}");
        assert!(m.contains(needle), "{needle} not in {m}");
    };
    // A vertex index past the four vertices.
    let mut g = quad_file();
    let end = g.bin.len();
    g.bin[end - 12..end - 10].copy_from_slice(&7u16.to_le_bytes());
    let (c, m) = code(load(&g.glb()));
    assert_eq!(c, "EINVAL", "{m}");
    assert!(m.contains("out of range of its 4 vertices"), "{m}");
    // POSITION naming an accessor that is not there (the gltf crate's own
    // validator would index past its list here).
    let mut g = quad_file();
    g.meshes[0] = g.meshes[0].replace(r#""POSITION":0"#, r#""POSITION":99"#);
    refused(&g, "accessor 99 is out of range");
    let mut g = quad_file();
    g.meshes[0] = g.meshes[0].replace(r#""material":0"#, r#""material":5"#);
    refused(&g, "material 5, out of range");
    let mut g = quad_file();
    g.nodes[0] = g.nodes[0].replace(r#""mesh":0"#, r#""mesh":3"#);
    refused(&g, "mesh 3, out of range");
    let mut g = quad_file();
    g.accessors[0] = g.accessors[0].replace(r#""bufferView":0"#, r#""bufferView":40"#);
    refused(&g, "buffer view 40 is out of range");
    let mut g = quad_file();
    g.materials[0] = r#"{"pbrMetallicRoughness":{"baseColorTexture":{"index":4}}}"#.into();
    refused(&g, "texture 4, out of range");
    // Animation channels naming a node or a sampler that is not there.
    let mut g = quad_file();
    let t = g.floats(&[0.0, 1.0], "SCALAR");
    let v = g.floats(&[0.0; 6], "VEC3");
    g.animation(format!(r#"{{"channels":[{{"sampler":0,"target":{{"node":12,"path":"translation"}}}}],"samplers":[{{"input":{t},"output":{v}}}]}}"#));
    refused(&g, "node 12, out of range");
    let mut g = quad_file();
    let t = g.floats(&[0.0, 1.0], "SCALAR");
    let v = g.floats(&[0.0; 6], "VEC3");
    g.animation(format!(r#"{{"channels":[{{"sampler":3,"target":{{"node":0,"path":"translation"}}}}],"samplers":[{{"input":{t},"output":{v}}}]}}"#));
    refused(&g, "sampler 3, out of range");
    // A sparse index past the accessor.
    let mut g = quad_file();
    let si = g.view(&[9u8, 0, 0, 0]);
    let sv = g.view(&[0u8; 12]);
    g.accessors[0] = g.accessors[0].replace(
        r#""type":"VEC3""#,
        &format!(r#""type":"VEC3","sparse":{{"count":1,"indices":{{"bufferView":{si},"componentType":5125}},"values":{{"bufferView":{sv}}}}}"#),
    );
    refused(&g, "sparse index out of range");
}

#[test]
fn json_that_would_grow_past_its_size_is_refused_unparsed() {
    let mut text = String::from(r#"{"asset":{"version":"2.0"},"nodes":["#);
    for i in 0..=JSON_STRUCTURES {
        if i > 0 {
            text.push(',');
        }
        text.push_str("{}");
    }
    text.push_str("]}");
    let (c, m) = code(load(text.as_bytes()));
    assert_eq!(c, "EFBIG", "{m}");
    // Braces inside strings are not counted.
    let quoted = format!(r#"{{"asset":{{"version":"2.0","generator":"{}"}}}}"#, "{".repeat(JSON_STRUCTURES + 5));
    assert_eq!(code(load(quoted.as_bytes())).0, "OK");
    // Deep nesting stops at the JSON reader's depth, not the stack.
    let deep = format!(r#"{{"asset":{{"version":"2.0"}},"extras":{}{}}}"#, "[".repeat(10_000), "]".repeat(10_000));
    assert_eq!(code(load(deep.as_bytes())).0, "EBADF");
}

#[test]
fn foreign_buffers_versions_and_required_extensions_are_refused() {
    let mut g = quad_file();
    let text = String::from_utf8(g.gltf()).unwrap();
    let start = text.find("data:").unwrap();
    let end = start + text[start..].find('"').unwrap();
    let external = format!("{}model.bin{}", &text[..start], &text[end..]);
    let (c, m) = code(load(external.as_bytes()));
    assert_eq!(c, "EINVAL");
    assert!(m.contains("not a base64 data: uri"), "{m}");
    g.extra.push(r#""extensionsRequired":["KHR_draco_mesh_compression"]"#.into());
    assert!(code(load(&g.glb())).1.contains("requires 1 extension"));
    let v1 = r#"{"asset":{"version":"1.0"}}"#;
    assert_eq!(code(load(v1.as_bytes())).0, "EINVAL");
    assert_eq!(code(load(b"PNG not a model")).0, "EBADF");
    // An image named by an external uri is skipped, the model kept.
    let mut g = quad_file();
    g.images.push(r#"{"uri":"wood.png"}"#.into());
    g.textures.push(r#"{"source":0}"#.into());
    g.materials[0] = r#"{"pbrMetallicRoughness":{"baseColorTexture":{"index":0}}}"#.into();
    let a = load(&g.glb()).expect("kept").asset;
    assert_eq!(a.skipped.images, 1);
    assert_eq!(a.materials[0].image, None);
}

#[test]
fn fuzzed_glbs_never_panic() {
    let mut file = quad_file();
    let t = file.floats(&[0.0, 0.5, 1.0], "SCALAR");
    let v = file.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0], "VEC3");
    file.animation(format!(r#"{{"channels":[{{"sampler":0,"target":{{"node":0,"path":"translation"}}}}],"samplers":[{{"input":{t},"output":{v}}}]}}"#));
    let whole = file.glb();
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut loaded = 0;
    for _ in 0..3000 {
        let mut b = whole.clone();
        for _ in 0..(next() % 6 + 1) {
            let at = (next() as usize) % b.len();
            b[at] = match next() % 4 {
                0 => 0xff,
                1 => 0,
                2 => b[at].wrapping_add(1),
                _ => next() as u8,
            };
        }
        if load(&b).is_ok() {
            loaded += 1;
        }
    }
    // Most single-byte flips land in the float data and still load.
    assert!(loaded > 0);
}

#[test]
fn caps_on_vertices_primitives_and_keys_hold() {
    // One primitive past the window's per-mesh vertex cap, refused from its
    // count alone.
    let mut g = Gltf::default();
    g.view(&[0u8; 12]);
    g.accessor(format!(r#"{{"bufferView":0,"componentType":5126,"count":{},"type":"VEC3"}}"#, angel_glass::scene::MAX_MESH_VERTICES + 1));
    let mesh = g.mesh(&[("POSITION", 0)], None, None);
    g.node(format!(r#"{{"mesh":{mesh}}}"#));
    let (c, m) = code(load(&g.glb()));
    assert_eq!(c, "EFBIG", "{m}");
    // Primitives sharing one big accessor until the asset's cap is reached.
    let n = 262_143usize;
    let mut g = Gltf::default();
    let pos = g.floats(&vec![0.0; n * 3], "VEC3");
    let nor = g.floats(&[0.0, 0.0, 1.0].repeat(n), "VEC3");
    let prim = glb::primitive(&[("POSITION", pos), ("NORMAL", nor)], None, None, None);
    let fits = ASSET_VERTICES / n;
    let mesh = g.mesh_json(format!(r#"{{"primitives":[{}]}}"#, vec![prim; fits + 1].join(",")));
    g.node(format!(r#"{{"mesh":{mesh}}}"#));
    let (c, m) = code(load(&g.glb()));
    assert_eq!(c, "EFBIG", "{m}");
    // Too many primitives, and too many keyframe values.
    let mut g = quad_file();
    let prim = glb::primitive(&[("POSITION", 0)], Some(4), None, None);
    g.meshes[0] = format!(r#"{{"primitives":[{}]}}"#, vec![prim; ASSET_PRIMITIVES + 1].join(","));
    assert_eq!(code(load(&g.glb())).0, "EFBIG");
    let mut g = quad_file();
    g.view(&[0u8; 4]);
    let view = g.views.len() - 1;
    let t = g.accessor(format!(r#"{{"bufferView":{view},"componentType":5126,"count":{},"type":"SCALAR"}}"#, ASSET_KEYS));
    g.animation(format!(r#"{{"channels":[{{"sampler":0,"target":{{"node":0,"path":"scale"}}}}],"samplers":[{{"input":{t},"output":{t}}}]}}"#));
    assert_eq!(code(load(&g.glb())).0, "EFBIG");
}

// ------------------------------------------------------------- sampling

fn channel(path: Path, interp: Interp, times: &[f32], values: &[[f32; 4]]) -> Channel {
    Channel { node: 0, path, interp, times: times.to_vec(), values: values.to_vec() }
}

#[test]
fn clips_sample_step_linear_cubic_spline_and_slerp() {
    let lin = channel(Path::Translation, Interp::Linear, &[0.0, 2.0], &[[0.0; 4], [4.0, -2.0, 8.0, 0.0]]);
    assert_eq!(lin.sample(-1.0), [0.0; 4], "held before the first key");
    assert_eq!(lin.sample(0.5), [1.0, -0.5, 2.0, 0.0]);
    assert_eq!(lin.sample(9.0), [4.0, -2.0, 8.0, 0.0], "held after the last");
    let step = channel(Path::Scale, Interp::Step, &[0.0, 1.0, 2.0], &[[1.0; 4], [2.0; 4], [3.0; 4]]);
    assert_eq!(step.sample(0.99)[0], 1.0);
    assert_eq!(step.sample(1.0)[0], 2.0);
    assert_eq!(step.sample(1.5)[0], 2.0);
    // Cubic spline, keys at 0 and 2 (span 2): v0 = 0, out-tangent b0 = 1,
    // v1 = 1, in-tangent a1 = 0. At u = 0.5: h00 = 0.5, h10 = 0.125,
    // h01 = 0.5, h11 = -0.125, so p = 0.125 * 2 * 1 + 0.5 * 1 = 0.75.
    let cubic = channel(
        Path::Translation,
        Interp::CubicSpline,
        &[0.0, 2.0],
        &[[0.0; 4], [0.0; 4], [1.0, 0.0, 0.0, 0.0], [0.0; 4], [1.0, 0.0, 0.0, 0.0], [0.0; 4]],
    );
    assert!(near(cubic.sample(1.0)[0], 0.75), "{:?}", cubic.sample(1.0));
    assert!(near(cubic.sample(2.0)[0], 1.0));
    // A quarter turn about z, sampled halfway: an eighth turn.
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let rot = channel(Path::Rotation, Interp::Linear, &[0.0, 1.0], &[[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, h, h]]);
    let q = rot.sample(0.5);
    let e = (std::f32::consts::PI / 8.0).sin();
    assert!(near(q[2], e) && near(q[3], (std::f32::consts::PI / 8.0).cos()), "{q:?}");
}

#[test]
fn playback_loops_runs_out_and_keeps_its_place_through_changes() {
    let mut p = Playback::new(ClipSel::One(0));
    assert_eq!(p.local(500, 2.0), (0.0, false), "unstarted, it holds");
    p.since = Some(1000);
    assert_eq!(p.local(2500, 2.0).0, 1.5);
    assert!(near(p.local(3500, 2.0).0, 0.5), "it loops");
    p.rebase(3500);
    p.speed = 2.0;
    p.since = Some(3500);
    assert!(near(p.local(4000, 2.0).0, 1.5), "a new speed carries on from where it was");
    p.looping = false;
    let (t, ended) = p.local(9000, 2.0);
    assert_eq!((t, ended), (2.0, true));
    assert!(!p.moving(9000, 2.0), "a clip that ran out wakes nothing");
    p.paused = true;
    assert!(!p.moving(4000, 2.0));
}

// -------------------------------------------------------------- the wire

/// A store and an engine, fed `glass;` bodies as a program writes them.
pub(super) struct Rig {
    pub(super) intake: Intake,
    engine: FakeEngine,
    splitter: Splitter,
}

impl Rig {
    pub(super) fn new() -> Self {
        let mut intake = Intake::new(angel_glass::LAYER_PICTURES);
        intake.set_cell((8, 16));
        Self { intake, engine: FakeEngine::new(), splitter: Splitter::default() }
    }

    pub(super) fn send(&mut self, body: &str) -> String {
        let bytes = format!("\x1b_glass;{body}\x1b\\").into_bytes();
        let mut pieces = Vec::new();
        self.splitter.split(&bytes, &mut pieces);
        for piece in pieces {
            if let Piece::Media(m) = piece {
                self.intake.handle(m, &mut self.engine);
            }
        }
        String::from_utf8_lossy(&self.intake.take_replies()).into_owned()
    }
}

/// A clip on the quad file: node 0 moves from x 0 to x 4 over 2 seconds.
fn animated_file() -> Gltf {
    let mut g = quad_file();
    let t = g.floats(&[0.0, 2.0], "SCALAR");
    let v = g.floats(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0], "VEC3");
    g.animation(format!(r#"{{"channels":[{{"sampler":0,"target":{{"node":0,"path":"translation"}}}}],"samplers":[{{"input":{t},"output":{v}}}]}}"#));
    g
}

fn scene_json(asset: u32) -> String {
    b64(format!(r#"{{"v":1,"scene":{{"nodes":[{{"id":5,"at":[0,0,-3],"color":[1,1,1,0.5],"mesh":{{"kind":"gltf","asset":{asset}}}}}]}}}}"#).as_bytes())
}

#[test]
fn an_asset_goes_up_in_pieces_and_answers_with_what_it_holds() {
    let mut rig = Rig::new();
    let mut file = animated_file();
    let rgba = [255u8, 0, 0, 255].repeat(4);
    let tex = file.png_texture(&glb::png(2, 2, &rgba), false);
    file.materials[0] = format!(r#"{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":{tex}}}}}}}"#);
    let b64 = glb::base64(&file.glb());
    let third = b64.len() / 3 / 4 * 4;
    assert_eq!(rig.send(&format!("asset;id=3;m=1;r=1;{}", &b64[..third])), "", "only the last piece answers");
    assert_eq!(rig.send(&format!("asset;m=1;{}", &b64[third..third * 2])), "");
    let reply = rig.send(&format!("asset;m=0;{}", &b64[third * 2..]));
    assert_eq!(
        reply,
        "\x1b_glass;r=1;OK;nodes=2,meshes=1,primitives=1,vertices=4,triangles=2,materials=1,textures=1,clips=1,seconds=2\x1b\\"
    );
    // A scene hangs it from node 5; the node's colour tints every material.
    let reply = rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=2;{}", scene_json(3)));
    assert!(reply.contains("r=2;OK"), "{reply}");
    let scene = rig.intake.expansion.scene(9).expect("scene");
    assert_eq!(scene.nodes.len(), 3, "the host, then the asset's two nodes");
    assert!(scene.nodes[1].id >= EXPANDED && scene.nodes[2].id >= EXPANDED);
    let drawn = scene.nodes.iter().find(|n| n.mesh.is_some()).expect("the quad's node");
    assert_eq!(drawn.material.tint[3], 0.5);
    assert_eq!(drawn.material.alpha, AlphaMode::Blend);
    let Some(MeshRef::Uploaded(mesh_id)) = drawn.mesh else { panic!("an uploaded mesh") };
    assert!(matches!(drawn.material.source, angel_glass::scene::TextureSource::Picture(_)), "the texture went in as a picture");
    let uploads = rig.intake.expansion.take_mesh_uploads();
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].0, mesh_id);
    assert_eq!(uploads[0].1.positions.len(), 4);
    // Sent again, the scene does not send the asset's meshes again.
    rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;{}", scene_json(3)));
    assert!(rig.intake.expansion.take_mesh_uploads().is_empty());
    // Its world: the asset's root hangs from node 5 at z -3.
    let worlds = scene_worlds(&rig, 9);
    assert!(worlds.iter().flatten().any(|m| near3(mat_point(m, [0.0; 3]), [1.0, 2.0, -3.0])), "the quad's node");
}

#[test]
fn a_node_shows_a_program_picture_on_its_mesh() {
    let mut rig = Rig::new();
    let quad = r#"{"v":1,"scene":{"nodes":[{"id":1,"mesh":"cube","picture":4,"color":[1,1,1,0.5]},{"id":2,"mesh":"quad","picture":4,"sampling":"nearest"}]}}"#;
    let b64 = |s: &str| glb::base64(s.as_bytes());
    // Not uploaded yet: refused by name.
    let reply = rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=1;{}", b64(quad)));
    assert!(reply.contains("EINVAL") && reply.contains("no picture 4"), "{reply}");
    assert!(rig.send("picture;id=4;f=24;s=1;v=1;r=2;////").contains("r=2;OK"));
    let reply = rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=3;{}", b64(quad)));
    assert!(reply.contains("r=3;OK"), "{reply}");
    let scene = rig.intake.expansion.scene(9).expect("scene");
    let TextureSource::Picture(first) = scene.nodes[0].material.source else { panic!("a picture on the cube") };
    assert_eq!(scene.nodes[0].material.sampling, angel_glass::scene::Sampling::Linear);
    assert_eq!(scene.nodes[0].material.tint[3], 0.5);
    assert_eq!(scene.nodes[1].material.sampling, angel_glass::scene::Sampling::Nearest);
    // The same id sent again (a video's next frame) still shows on the
    // next scene sent.
    assert!(rig.send("picture;id=4;f=24;s=1;v=1;r=4;AAAA").contains("r=4;OK"));
    rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;{}", b64(quad)));
    let scene = rig.intake.expansion.scene(9).expect("scene");
    assert!(matches!(scene.nodes[0].material.source, TextureSource::Picture(_)), "still {first}'s place");
    for bad in [r#""picture":-1"#, r#""picture":"4""#, r#""picture":4,"sampling":"cubic""#] {
        let json = format!(r#"{{"v":1,"scene":{{"nodes":[{{"id":1,"mesh":"cube",{bad}}}]}}}}"#);
        let reply = rig.send(&format!("scene;id=9;s=8;v=8;px=0,0,8,8;r=5;{}", b64(&json)));
        assert!(reply.contains("EINVAL"), "{bad}: {reply}");
    }
}

fn scene_worlds(rig: &Rig, id: u32) -> Vec<Option<angel_glass::scene::Mat4>> {
    rig.intake.expansion.scene(id).expect("scene").world_matrices()
}

fn quad_x(rig: &Rig) -> f32 {
    let scene = rig.intake.expansion.scene(9).expect("scene");
    scene.nodes.iter().find(|n| n.mesh.is_some()).expect("quad").transform.translation[0]
}

#[test]
fn play_drives_a_clip_on_the_fake_clock_and_sleeps_when_still() {
    let mut rig = Rig::new();
    rig.send(&format!("asset;id=3;{}", glb::base64(&animated_file().glb())));
    rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;{}", scene_json(3)));
    assert_eq!(rig.intake.animation_deadline(), None, "nothing plays yet");
    assert_eq!(rig.send("play;id=9;r=4"), "\x1b_glass;r=4;OK;seconds=2\x1b\\");
    rig.intake.set_animation_clock(1000);
    assert_eq!(rig.intake.animation_deadline(), Some(1000), "a new play wants a tick at once");
    assert!(rig.intake.tick_animations(1000));
    assert_eq!(quad_x(&rig), 0.0);
    rig.intake.set_display_frame(16);
    assert_eq!(rig.intake.animation_deadline(), Some(1016), "one display frame on");
    rig.intake.tick_animations(1500);
    assert!(near(quad_x(&rig), 1.0), "a quarter of the way: {}", quad_x(&rig));
    // It loops by default: 2.5 s in is 0.5 s in.
    rig.intake.tick_animations(3500);
    assert!(near(quad_x(&rig), 1.0));
    // Paused, it holds and wakes nothing.
    rig.send("play;id=9;pause=1");
    rig.intake.tick_animations(3600);
    assert!(near(quad_x(&rig), 1.0));
    assert_eq!(rig.intake.animation_deadline(), None);
    // Seek and speed while paused, then resume: it carries on from there.
    rig.send("play;id=9;t=1;speed=0.5;pause=0");
    rig.intake.tick_animations(4000);
    assert!(near(quad_x(&rig), 2.0));
    rig.intake.tick_animations(5000);
    assert!(near(quad_x(&rig), 3.0), "half speed: {}", quad_x(&rig));
    // Once through, it ends on the last pose and sleeps.
    rig.send("play;id=9;loop=0;speed=1");
    rig.intake.tick_animations(5000);
    rig.intake.tick_animations(9000);
    assert!(near(quad_x(&rig), 4.0));
    assert_eq!(rig.intake.animation_deadline(), None);
    // Stop goes back to the file's own pose.
    rig.send("play;id=9;stop=1");
    assert!(near(quad_x(&rig), 1.0), "the rest pose: {}", quad_x(&rig));
    // The scene sent again while playing keeps its place.
    rig.send("play;id=9");
    rig.intake.tick_animations(10_000);
    rig.intake.tick_animations(11_000);
    assert!(near(quad_x(&rig), 2.0));
    rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;{}", scene_json(3)));
    assert!(near(quad_x(&rig), 2.0), "posed as it was, not at rest: {}", quad_x(&rig));
}

#[test]
fn asset_and_play_errors_name_themselves() {
    let mut rig = Rig::new();
    let cases = [
        ("asset;r=1;AAAA", "EINVAL:asset needs id="),
        ("asset;id=1073741824;r=1;AAAA", "EINVAL:id must be under"),
        ("asset;id=1;r=1;!!!!", "EBADF:payload is not base64"),
        ("asset;id=1;t=q;r=1;AAAA", "EINVAL:t=q is not d or f"),
        ("asset;id=1;r=1;AAAA", "EBADF:the asset is neither"),
        ("scene;id=2;s=8;v=8;px=0,0,8,8;r=1;", "EBADF"),
        ("play;id=4;r=1", "EINVAL:no scene 4"),
    ];
    for (body, want) in cases {
        let reply = rig.send(body);
        assert!(reply.contains(want), "{body}: {reply}");
    }
    let missing = rig.send(&format!("scene;id=2;s=8;v=8;px=0,0,8,8;r=1;{}", scene_json(77)));
    assert!(missing.contains("EINVAL:no asset 77 was uploaded"), "{missing}");
    rig.send(&format!("asset;id=1;{}", glb::base64(&quad_file().glb())));
    rig.send(&format!("scene;id=2;s=8;v=8;px=0,0,8,8;{}", scene_json(1)));
    assert!(rig.send("play;id=2;r=1").contains("EINVAL:asset 1 has no animations"));
    rig.send(&format!("asset;id=3;{}", glb::base64(&animated_file().glb())));
    rig.send(&format!("scene;id=4;s=8;v=8;px=0,0,8,8;{}", scene_json(3)));
    assert!(rig.send("play;id=4;clip=2;r=1").contains("EINVAL:asset 3 has 1 clips"));
    assert!(rig.send("play;id=4;node=6;r=1").contains("EINVAL:no glTF asset hangs from node 6"));
    assert!(rig.send("play;id=4;speed=40;r=1").contains("EINVAL:speed= must be"));
    // A node id in the window's own range is refused on the wire.
    let reserved = glb::base64(br#"{"v":1,"scene":{"nodes":[{"id":9223372036854775808}]}}"#);
    assert!(rig.send(&format!("scene;id=5;s=8;v=8;px=0,0,8,8;r=1;{reserved}")).contains("below 2^63"));
}

#[test]
fn assets_are_capped_in_count_and_removed_by_id() {
    let mut rig = Rig::new();
    let b64 = glb::base64(&quad_file().glb());
    for id in 0..super::glass_dialect::ASSET_COUNT {
        assert!(rig.send(&format!("asset;id={id};r=1;{b64}")).contains("OK"));
    }
    let full = rig.send(&format!("asset;id=999;r=1;{b64}"));
    assert!(full.contains("ENOMEM"), "{full}");
    // The same id replaces, and is not counted twice.
    assert!(rig.send(&format!("asset;id=0;r=1;{b64}")).contains("OK"));
    rig.send("remove;asset=0-9");
    assert!(rig.send(&format!("asset;id=999;r=1;{b64}")).contains("OK"));
    assert!(rig.send(&format!("scene;id=1;s=8;v=8;px=0,0,8,8;r=1;{}", scene_json(5))).contains("no asset 5"));
}

#[test]
fn an_asset_from_a_path_follows_the_file_medium_rules() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("glass-asset-test-{}.glb", std::process::id()));
    std::fs::write(&path, quad_file().glb()).expect("write");
    let mut rig = Rig::new();
    let reply = rig.send(&format!("asset;id=1;t=f;r=1;{}", glb::base64(path.to_string_lossy().as_bytes())));
    let _ = std::fs::remove_file(&path);
    assert!(reply.contains("OK;nodes=2"), "{reply}");
    for refused in ["/proc/self/environ", "/dev/zero", "/sys/kernel/notes", "/nonexistent/model.glb"] {
        let reply = rig.send(&format!("asset;id=2;t=f;r=1;{}", glb::base64(refused.as_bytes())));
        assert!(reply.contains("EBADF"), "{refused}: {reply}");
    }
}

// ------------------------------------------------------------- skins

/// Where the Mixamo characters are kept on the machine that has them
/// (`GLASS_MIXAMO`, else `~/models/mixamo`); they are not in the repository.
fn mixamo_dir() -> std::path::PathBuf {
    std::env::var_os("GLASS_MIXAMO").map_or_else(
        || std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join("models/mixamo"),
        Into::into,
    )
}

/// The skinned vertices of every skinned node of scene `id`: each node's
/// mesh's positions through its joints (angel_stage's own maths, which the
/// mesh shader mirrors).
fn skinned_points(rig: &Rig, id: u32, uploads: &[(u32, std::sync::Arc<angel_glass::scene::MeshData>)]) -> Vec<[f32; 3]> {
    use angel_glass::scene::{mat_point, skin_matrix};
    let scene = rig.intake.expansion.scene(id).expect("scene");
    let worlds = scene.world_matrices();
    let mut out = Vec::new();
    for (i, node) in scene.nodes.iter().enumerate() {
        let (Some(_), Some(MeshRef::Uploaded(mesh)), Some(world)) = (&node.skin, node.mesh, worlds[i]) else { continue };
        let palette = scene.skin_matrices(i).expect("every joint placed");
        let data = &uploads.iter().find(|(m, _)| *m == mesh).expect("its mesh went up").1;
        for (k, p) in data.positions.iter().enumerate() {
            out.push(mat_point(&skin_matrix(&palette, data.joints[k], data.weights[k], &world), *p));
        }
    }
    out
}

/// The box `points` fill: its least and most corners.
fn bounds(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

#[test]
#[ignore = "reads the Mixamo characters from $GLASS_MIXAMO (else ~/models/mixamo); skips any that is not there"]
fn the_mixamo_characters_load_bend_at_rest_and_play() {
    // (file, skins, clips, base colour pictures decoded: a PNG, a JPEG
    // both materials share, none, none)
    let files = [("Michelle.glb", 1, 2, 1usize), ("Soldier.glb", 2, 4, 1), ("Xbot.glb", 1, 7, 0), ("man.glb", 1, 10, 0)];
    let mut seen = 0;
    for (k, (file, skins, clips, textures)) in files.into_iter().enumerate() {
        let path = mixamo_dir().join(file);
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipped: {} is not there", path.display());
            continue;
        };
        seen += 1;
        let loaded = load(&bytes).unwrap_or_else(|e| panic!("{file}: {} {}", e.code, e.message));
        let a = &loaded.asset;
        assert_eq!(a.skins.len(), skins, "{file} skins");
        assert_eq!(a.clips.len(), clips, "{file} clips");
        assert!(loaded.images.len() >= textures, "{file}: {} textures decoded", loaded.images.len());
        assert!(a.skins.iter().all(|s| s.joints.len() <= ASSET_JOINTS && s.inverse_bind.len() == s.joints.len()));
        assert!(a.prims.iter().all(|p| !p.mesh.joints.is_empty()), "{file}: every primitive is skinned");
        // Through the dialect, as a program sends it: read from its path,
        // hung in a scene, at rest, then playing.
        let mut rig = Rig::new();
        let id = 10 + k as u32;
        let reply = rig.send(&format!("asset;id={id};t=f;r=1;{}", glb::base64(path.to_string_lossy().as_bytes())));
        assert!(reply.contains("r=1;OK"), "{file}: {reply}");
        eprintln!("{file}: {reply:?}");
        let scene = format!(r#"{{"v":1,"scene":{{"nodes":[{{"id":1,"mesh":{{"kind":"gltf","asset":{id}}},"chrome":1,"liquid":0.3}}]}}}}"#);
        let reply = rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=2;{}", glb::base64(scene.as_bytes())));
        assert!(reply.contains("r=2;OK"), "{file}: {reply}");
        let uploads = rig.intake.expansion.take_mesh_uploads();
        let skinned = rig.intake.expansion.scene(9).expect("scene").nodes.iter().filter(|n| n.skin.is_some()).count();
        assert!(skinned >= skins, "{file}: {skinned} skinned nodes");
        // At rest the figure stands about a person's height, feet near the
        // floor: its joints and inverse bind matrices agree.
        let rest = skinned_points(&rig, 9, &uploads);
        let (lo, hi) = bounds(&rest);
        eprintln!("{file}: at rest {lo:?}..{hi:?}");
        let height = hi[1] - lo[1];
        assert!((1.2..2.3).contains(&height), "{file}: {height} tall at rest");
        assert!(lo[1].abs() < 0.3, "{file}: its feet at {}", lo[1]);
        // Every material is the host's chrome.
        let s = rig.intake.expansion.scene(9).expect("scene");
        assert!(s.nodes.iter().filter(|n| n.mesh.is_some()).all(|n| n.material.chrome == 1.0 && n.material.liquid == 0.3));
        // Playing: two moments apart bend it differently, and stay sound.
        assert!(rig.send("play;id=9;clip=0;r=3").contains("r=3;OK"));
        rig.intake.tick_animations(1000);
        rig.intake.tick_animations(1400);
        let one = skinned_points(&rig, 9, &uploads);
        rig.intake.tick_animations(2100);
        let two = skinned_points(&rig, 9, &uploads);
        assert!(one.iter().chain(&two).flatten().all(|v| v.is_finite()));
        let moved = one.iter().zip(&two).map(|(a, b)| (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f32::max)).fold(0.0, f32::max);
        eprintln!("{file}: the most a vertex moved in 0.7 s: {moved}");
        assert!(moved > 0.01, "{file}: the clip bends it ({moved})");
        let (lo, hi) = bounds(&two);
        assert!(hi[1] - lo[1] < 3.0 && lo[1] > -1.0, "{file}: still a figure while it plays: {lo:?}..{hi:?}");
        // Every clip, at a few moments: sound, and still a figure (a dance
        // may bounce and travel, never fly apart).
        let mut clock = 3000;
        for clip in 0..clips {
            assert!(rig.send(&format!("play;id=9;clip={clip};r=4")).contains("r=4;OK"), "{file} clip {clip}");
            for _ in 0..3 {
                clock += 377;
                rig.intake.tick_animations(clock);
                let points = skinned_points(&rig, 9, &uploads);
                assert!(points.iter().flatten().all(|v| v.is_finite()), "{file} clip {clip}");
                let (lo, hi) = bounds(&points);
                let size = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f32::max);
                assert!((0.4..3.5).contains(&size), "{file} clip {clip}: {lo:?}..{hi:?}");
            }
        }
    }
    eprintln!("{seen} of {} characters read", files.len());
    if seen < files.len() {
        return;
    }
    // The performance bar's scene: three figures and sixty shapes, sent
    // again whole every display frame. What the intake spends on each.
    let mut rig = Rig::new();
    for (k, file) in ["Xbot.glb", "Michelle.glb", "Soldier.glb"].iter().enumerate() {
        let path = mixamo_dir().join(file);
        rig.send(&format!("asset;id={};t=f;{}", k + 1, glb::base64(path.to_string_lossy().as_bytes())));
    }
    let frame = |t: f32| {
        let mut nodes: Vec<String> = (1..=3)
            .map(|k| format!(r#"{{"id":{k},"at":[{},0,0],"mesh":{{"kind":"gltf","asset":{k}}},"chrome":1,"liquid":0.4}}"#, k as f32 - 2.0))
            .collect();
        for i in 0..60 {
            let a = i as f32 * 0.1 + t;
            nodes.push(format!(r#"{{"id":{},"at":[{:.3},{:.3},-2],"mesh":{{"kind":"sphere","segments":32}},"chrome":1,"liquid":0.8}}"#, 100 + i, a.sin() * 3.0, a.cos() + 1.5));
        }
        glb::base64(format!(r#"{{"v":1,"scene":{{"nodes":[{}]}}}}"#, nodes.join(",")).as_bytes())
    };
    rig.send(&format!("scene;id=1;px=0,0,1920,1080;s=1920;v=1080;{}", frame(0.0)));
    for k in 1..=3 {
        rig.send(&format!("play;id=1;node={k}"));
    }
    let frames: Vec<String> = (0..120).map(|i| frame(i as f32 / 60.0)).collect();
    let began = std::time::Instant::now();
    for (i, f) in frames.iter().enumerate() {
        rig.send(&format!("scene;id=1;px=0,0,1920,1080;s=1920;v=1080;{f}"));
        rig.intake.tick_animations(1000 + i as u64 * 16);
        let _ = rig.intake.expansion.dirty_scenes();
    }
    let each = began.elapsed().as_secs_f64() * 1000.0 / frames.len() as f64;
    eprintln!("a 63-node scene with three figures, sent again, ticked and taken: {each:.3} ms a frame");
}

/// Two joints and a triangle they bend: joint A (node 0) at the origin,
/// joint B (node 1) a unit up from it, the mesh (node 2) skinned by them,
/// all under a root (node 3). Bound where they stand: A's inverse bind is
/// the identity, B's a unit down. Vertex 0 (1, 1, 0) follows B, vertex 1
/// (-1, 1, 0) both by half, vertex 2 (0, 2, 0) A. One clip turns B a
/// quarter about z over a second.
fn skinned_file(joints_u16: bool) -> Gltf {
    let mut g = Gltf::default();
    let pos = g.floats(&[1., 1., 0., -1., 1., 0., 0., 2., 0.], "VEC3");
    let nor = g.floats(&[0., 0., 1., 0., 0., 1., 0., 0., 1.], "VEC3");
    let joints: [u16; 12] = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    let (bytes, component): (Vec<u8>, u32) = if joints_u16 {
        (joints.iter().flat_map(|j| j.to_le_bytes()).collect(), 5123)
    } else {
        (joints.iter().map(|j| *j as u8).collect(), 5121)
    };
    let view = g.view(&bytes);
    let jnt = g.accessor(format!(r#"{{"bufferView":{view},"componentType":{component},"count":3,"type":"VEC4"}}"#));
    let wgt = g.floats(&[1., 0., 0., 0., 0.5, 0.5, 0., 0., 1., 0., 0., 0.], "VEC4");
    let idx = g.indices(&[0, 1, 2]);
    let mesh = g.mesh(&[("POSITION", pos), ("NORMAL", nor), ("JOINTS_0", jnt), ("WEIGHTS_0", wgt)], Some(idx), None);
    let ibm: Vec<f32> = [IDENTITY, translate(0.0, -1.0, 0.0)].concat();
    let view = g.view(&ibm.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>());
    let ibm = g.accessor(format!(r#"{{"bufferView":{view},"componentType":5126,"count":2,"type":"MAT4"}}"#));
    let a = g.node(r#"{"children":[1]}"#.into());
    g.node(r#"{"translation":[0,1,0]}"#.into());
    let skinned = g.node(format!(r#"{{"mesh":{mesh},"skin":0}}"#));
    let root = g.node(format!(r#"{{"children":[{a},{skinned}]}}"#));
    g.add_scene(&[root]);
    g.extra.push(format!(r#""skins":[{{"joints":[0,1],"inverseBindMatrices":{ibm}}}]"#));
    let t = g.floats(&[0.0, 1.0], "SCALAR");
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let q = g.floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, h, h], "VEC4");
    g.animation(format!(r#"{{"channels":[{{"sampler":0,"target":{{"node":1,"path":"rotation"}}}}],"samplers":[{{"input":{t},"output":{q}}}]}}"#));
    g
}

const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];

fn translate(x: f32, y: f32, z: f32) -> [f32; 16] {
    [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., x, y, z, 1.]
}

#[test]
fn a_skin_is_read_with_its_joints_weights_and_inverse_bind_matrices() {
    for wide in [false, true] {
        let loaded = load(&skinned_file(wide).glb()).expect("the skinned file loads");
        let a = &loaded.asset;
        assert_eq!(a.skins.len(), 1);
        assert_eq!(a.skins[0].joints, vec![0, 1]);
        assert_eq!(a.skins[0].inverse_bind[0], angel_glass::scene::IDENTITY_MATRIX);
        assert_eq!(a.skins[0].inverse_bind[1][3], [0.0, -1.0, 0.0, 1.0], "column-major, as glTF keeps them");
        assert_eq!(a.nodes[2].skin, Some(0));
        let mesh = &a.prims[0].mesh;
        assert_eq!(mesh.joints, vec![[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 0, 0]]);
        assert_eq!(mesh.weights[1], [0.5, 0.5, 0.0, 0.0]);
        assert_eq!(a.bytes, 3 * VERTEX_BYTES + 3 * INDEX_BYTES + 3 * SKIN_BYTES, "a skinned vertex costs its joints too");
    }
    // A skin with no inverse bind matrices binds at the identity.
    let mut g = skinned_file(false);
    g.extra = vec![r#""skins":[{"joints":[0,1]}]"#.into()];
    let a = load(&g.glb()).expect("loads").asset;
    assert!(a.skins[0].inverse_bind.iter().all(|m| *m == angel_glass::scene::IDENTITY_MATRIX));
}

/// The triangle's three points in the world, through its skin.
fn bent(rig: &Rig) -> Vec<[f32; 3]> {
    let scene = rig.intake.expansion.scene(9).expect("scene");
    let worlds = scene.world_matrices();
    let at = scene.nodes.iter().position(|n| n.skin.is_some()).expect("a skinned node");
    let palette = scene.skin_matrices(at).expect("its joints are placed");
    let world = worlds[at].expect("placed");
    let points = [[1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [0.0, 2.0, 0.0]];
    let joints = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 0, 0]];
    let weights = [[1.0, 0.0, 0.0, 0.0], [0.5, 0.5, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]];
    (0..3).map(|k| mat_point(&angel_glass::scene::skin_matrix(&palette, joints[k], weights[k], &world), points[k])).collect()
}

#[test]
fn a_skinned_asset_rests_in_its_bind_pose_and_bends_as_its_clip_turns_a_joint() {
    let mut rig = Rig::new();
    assert!(rig.send(&format!("asset;id=3;r=1;{}", glb::base64(&skinned_file(false).glb()))).contains("r=1;OK"));
    rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;{}", scene_json(3)));
    // At rest it is its bind pose: the triangle as the file gave it, where
    // the host node (z -3) puts it.
    let rest = bent(&rig);
    for (got, want) in rest.iter().zip([[1.0, 1.0, -3.0], [-1.0, 1.0, -3.0], [0.0, 2.0, -3.0]]) {
        assert!(near3(*got, want), "rest: {got:?} != {want:?}");
    }
    // The skinned node's joints are the asset's nodes as this scene has them.
    let scene = rig.intake.expansion.scene(9).expect("scene");
    let skin = scene.nodes.iter().find_map(|n| n.skin.clone()).expect("a skin");
    assert!(skin.joints.iter().all(|id| scene.nodes.iter().any(|n| n.id == *id)));
    // A second in, B has turned a quarter: by hand, vertex 0 goes (1, 1)
    // -> (1, 0) -> (0, 1) -> (0, 2); vertex 1 is half of (-1, 1) and half
    // of (0, 0); vertex 2 stays with A.
    rig.send("play;id=9;loop=0");
    rig.intake.tick_animations(1000);
    rig.intake.tick_animations(2000);
    let turned = bent(&rig);
    for (got, want) in turned.iter().zip([[0.0, 2.0, -3.0], [-0.5, 0.5, -3.0], [0.0, 2.0, -3.0]]) {
        assert!(near3(*got, want), "turned: {got:?} != {want:?}");
    }
    // Stopped, it rests in its bind pose again.
    rig.send("play;id=9;stop=1");
    assert_eq!(bent(&rig), rest);
}

#[test]
fn skins_are_capped_and_their_errors_name_themselves() {
    let skins = |json: String| {
        let mut g = skinned_file(false);
        g.extra = vec![json];
        code(load(&g.glb()))
    };
    let joints = |n: usize| (0..n).map(|i| (i % 2).to_string()).collect::<Vec<_>>().join(",");
    // 256 joints a skin is the most; one more is refused by name.
    assert_eq!(skins(format!(r#""skins":[{{"joints":[{}]}}]"#, joints(ASSET_JOINTS))).0, "OK");
    let (c, m) = skins(format!(r#""skins":[{{"joints":[{}]}}]"#, joints(ASSET_JOINTS + 1)));
    assert_eq!(c, "EFBIG");
    assert!(m.contains("skin 0 has 257 joints; the cap is 256"), "{m}");
    // 64 skins an asset.
    let many = |n: usize| format!(r#""skins":[{}]"#, vec![r#"{"joints":[0,1]}"#; n].join(","));
    assert_eq!(skins(many(ASSET_SKINS)).0, "OK");
    let (c, m) = skins(many(ASSET_SKINS + 1));
    assert_eq!(c, "EFBIG");
    assert!(m.contains("capped at 64 skins"), "{m}");
    for (json, needle) in [
        (r#""skins":[{"joints":[]}]"#, "skin 0 has no joints"),
        (r#""skins":[{"joints":[0,9]}]"#, "skin 0 names joint node 9, out of range"),
        (r#""skins":[{"joints":[0],"inverseBindMatrices":0}]"#, "is not 4x4 float matrices"),
        (r#""skins":[{"joints":[0,1,1],"inverseBindMatrices":7}]"#, "fewer than its 3 joints"),
        (r#""skins":[{"joints":[0]}]"#, "names joint 1, past skin 0's 1 joints"),
    ] {
        let (c, m) = skins(json.into());
        assert_eq!(c, "EINVAL", "{json}: {m}");
        assert!(m.contains(needle), "{json}: {m}");
    }
    let mut g = skinned_file(false);
    g.nodes[2] = r#"{"mesh":0,"skin":4}"#.into();
    assert!(code(load(&g.glb())).1.contains("node 2 names skin 4, out of range"));
    // Joints without weights, and the other way round.
    for (drop, needle) in [("WEIGHTS_0", "JOINTS_0 without WEIGHTS_0"), ("JOINTS_0", "WEIGHTS_0 without JOINTS_0")] {
        let mut g = skinned_file(false);
        let attrs = if drop == "WEIGHTS_0" { r#""POSITION":0,"NORMAL":1,"JOINTS_0":2"# } else { r#""POSITION":0,"NORMAL":1,"WEIGHTS_0":3"# };
        g.meshes[0] = format!(r#"{{"primitives":[{{"attributes":{{{attrs}}},"indices":4}}]}}"#);
        assert!(code(load(&g.glb())).1.contains(needle), "{drop}");
    }
}

#[test]
fn other_uv_sets_and_extensions_used_but_not_required_are_taken() {
    // A base colour texture on TEXCOORD_2 of three sets: its uvs are that
    // set's.
    let mut g = quad_file();
    let sets: Vec<usize> = (0..3).map(|s| g.floats(&[s as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], "VEC2")).collect();
    let tex = g.png_texture(&glb::png(1, 1, &[255, 0, 0, 255]), false);
    let attrs: Vec<(String, usize)> = sets.iter().enumerate().map(|(s, a)| (format!("TEXCOORD_{s}"), *a)).collect();
    let mut all: Vec<(&str, usize)> = vec![("POSITION", 0), ("NORMAL", 1)];
    all.extend(attrs.iter().map(|(n, a)| (n.as_str(), *a)));
    g.meshes[0] = format!(r#"{{"primitives":[{}]}}"#, glb::primitive(&all, Some(4), Some(0), None));
    g.materials[0] = format!(
        r#"{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":{tex},"texCoord":2}}}},"extensions":{{"KHR_materials_specular":{{"specularFactor":0.5}},"KHR_materials_ior":{{"ior":1.4}}}}}}"#
    );
    g.extra.push(r#""extensionsUsed":["KHR_materials_specular","KHR_materials_ior","EXT_glass_unknown"]"#.into());
    let a = load(&g.glb()).expect("an extension used but not required is passed over").asset;
    assert_eq!(a.prims[0].mesh.uvs[0], [2.0, 0.0], "TEXCOORD_2's uvs");
}

// ------------------------------------------------- faces, edges, finish

#[test]
fn faces_show_uploaded_pictures_and_a_missing_one_is_refused_by_name() {
    let mut rig = Rig::new();
    let b64 = |s: &str| glb::base64(s.as_bytes());
    let cube = r#"{"v":1,"scene":{"nodes":[{"id":1,"mesh":{"kind":"roundcube","radius":0.2},"color":[1,0.5,0.5,1],"faces":[4,null,5,4],"edge":{"color":[0,1,1,1],"glow":0.9}},{"id":2,"mesh":"cube","faces":[5]}]}}"#;
    let reply = rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=1;{}", b64(cube)));
    assert!(reply.contains("EINVAL:no picture 4 was uploaded"), "{reply}");
    assert!(rig.send("picture;id=4;f=24;s=1;v=1;r=2;////").contains("r=2;OK"));
    let reply = rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=3;{}", b64(cube)));
    assert!(reply.contains("EINVAL:no picture 5 was uploaded"), "{reply}");
    assert!(rig.send("picture;id=5;f=24;s=1;v=1;r=4;AAAA").contains("r=4;OK"));
    let reply = rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=5;{}", b64(cube)));
    assert!(reply.contains("r=5;OK"), "{reply}");
    let scene = rig.intake.expansion.scene(9).expect("scene");
    let round = &scene.nodes[0];
    assert_eq!(round.mesh, Some(MeshRef::RoundCube { radius: 200, segments: 4 }));
    assert_eq!(round.face_materials.len(), 7, "six faces and the edge");
    let picture = |m: &angel_glass::scene::Material| match m.source {
        TextureSource::Picture(id) => Some(id),
        TextureSource::None => None,
    };
    let (four, five) = (picture(&round.face_materials[0]).expect("face 0"), picture(&round.face_materials[2]).expect("face 2"));
    assert_ne!(four, five);
    assert_eq!(picture(&round.face_materials[3]), Some(four), "the same picture on two faces");
    assert_eq!(picture(&round.face_materials[1]), None, "null: the node's own");
    assert!(round.face_materials[..6].iter().all(|m| m.tint == [1.0, 0.5, 0.5, 1.0]), "tinted by the node's colour");
    assert!(round.face_materials[..6].iter().filter(|m| picture(m).is_some()).all(|m| m.sampling == angel_glass::scene::Sampling::Linear));
    assert_eq!((round.face_materials[6].tint, round.face_materials[6].glow), ([0.0, 1.0, 1.0, 1.0], 0.9));
    let cube = &scene.nodes[1];
    assert_eq!(cube.face_materials.len(), 6);
    assert_eq!(picture(&cube.face_materials[0]), Some(five));
    // A colour tween moves the faces with the node; the edge keeps its own.
    assert!(rig.send("tween;id=9;node=1;color=0,0,1,1;dur=0;r=6").contains("OK"));
    rig.intake.tick_animations(1000);
    rig.intake.tick_animations(1100);
    let round = &rig.intake.expansion.scene(9).expect("scene").nodes[0];
    assert!(round.face_materials[..6].iter().all(|m| m.tint == [0.0, 0.0, 1.0, 1.0]), "{:?}", round.face_materials[0].tint);
    assert_eq!(round.face_materials[6].tint, [0.0, 1.0, 1.0, 1.0]);
}

#[test]
fn a_chrome_host_draws_its_asset_in_its_own_material_and_a_liquid_one_keeps_the_files() {
    let mut rig = Rig::new();
    let mut file = animated_file();
    let rgba = [255u8, 0, 0, 255].repeat(4);
    let tex = file.png_texture(&glb::png(2, 2, &rgba), false);
    file.materials[0] = format!(r#"{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":{tex}}}}},"doubleSided":true}}"#);
    rig.send(&format!("asset;id=3;{}", glb::base64(&file.glb())));
    let hang = |finish: &str| {
        glb::base64(format!(r#"{{"v":1,"scene":{{"nodes":[{{"id":5,"color":[0.9,0.9,1,1],{finish},"mesh":{{"kind":"gltf","asset":3}}}}]}}}}"#).as_bytes())
    };
    assert!(rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=1;{}", hang(r#""chrome":1,"liquid":0.4"#))).contains("OK"));
    let drawn = |rig: &Rig| rig.intake.expansion.scene(9).expect("scene").nodes.iter().find(|n| n.mesh.is_some()).expect("drawn").material;
    let m = drawn(&rig);
    assert_eq!((m.chrome, m.liquid, m.tint), (1.0, 0.4, [0.9, 0.9, 1.0, 1.0]), "the host's chrome, tinted by its colour");
    assert_eq!(m.source, TextureSource::None, "the file's texture is overridden");
    assert!(m.double_sided, "it keeps its own sides");
    assert!(rig.send(&format!("scene;id=9;s=64;v=64;px=0,0,64,64;r=2;{}", hang(r#""liquid":0.4"#))).contains("OK"));
    let m = drawn(&rig);
    assert_eq!((m.chrome, m.liquid), (0.0, 0.4));
    assert!(matches!(m.source, TextureSource::Picture(_)), "liquid alone keeps the file's material");
}
