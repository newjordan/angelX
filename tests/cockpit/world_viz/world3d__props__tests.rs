use super::*;
use crate::stage::world_viz::world3d::raster::{self, View3};

const SQUARE: &str = "\
# a stone slab with a brass corner
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
usemtl stone
f 1 2 3 4
usemtl brass
f 1 2 3
";

#[test]
fn a_prop_parses_into_tagged_triangles() {
    let mesh = parse(SQUARE).unwrap();
    assert_eq!(mesh.tris.len(), 3, "a quad fans into two triangles, then one more");
    assert_eq!(mesh.tris[0].mat, mat::STONE);
    assert_eq!(mesh.tris[2].mat, mat::BRASS);
    // A floor-facing face takes its surface coordinates from x and y.
    assert_eq!(mesh.tris[0].uv[1], [1.0, 0.0]);
}

#[test]
fn a_sliver_face_is_dropped() {
    let mesh = parse("v 0 0 0\nv 1 0 0\nv 2 0 0\nv 0 1 0\nf 1 2 3\nf 1 2 4\n").unwrap();
    assert_eq!(mesh.tris.len(), 1, "the collinear face has no facing");
}

#[test]
fn a_bad_prop_says_where() {
    assert!(parse("v 0 0 0\nf 1 2 3\n").unwrap_err().contains("line 2"));
    assert!(parse("usemtl marble\n").unwrap_err().contains("marble"));
    assert!(parse("v 0 0\n").unwrap_err().contains("three numbers"));
}

fn library() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/realm/props/library");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "obj"))
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_prop_parses_stands_on_the_ground_and_stays_small() {
    let bundled = PROPS.iter().map(|(n, t)| (n.to_string(), t.to_string()));
    for (name, text) in bundled.chain(library()) {
        let mesh = parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!mesh.tris.is_empty(), "{name} is empty");
        assert!(mesh.tris.len() <= 3000, "{name} has {} faces", mesh.tris.len());
        let low = mesh
            .tris
            .iter()
            .flat_map(|t| t.v)
            .map(|p| p.z)
            .fold(f32::MAX, f32::min);
        assert!(low.abs() < 0.01, "{name} floats or sinks: lowest point {low}");
        for tri in &mesh.tris {
            assert!(tri.normal.length() > 0.9, "{name}: degenerate face {:?}", tri.v);
        }
    }
    for (name, _) in PROPS {
        assert!(prop(name).is_some(), "{name} is listed but does not load");
    }
}

/// `ANGEL_PROP_SHOTS=<dir>`: each prop on a patch of ground at dusk, from a
/// three-quarter view, at a large size and at mini-pane size.
#[test]
#[ignore]
fn write_prop_shots() {
    let Some(dir) = std::env::var_os("ANGEL_PROP_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    // `ANGEL_PROP_DIR=<dir>` renders loose `.obj` files instead, for trying a
    // conversion before it is bundled.
    let mut props: Vec<(String, Arc<Mesh>)> = PROPS
        .iter()
        .map(|(name, _)| (name.to_string(), prop(name).unwrap()))
        .chain(library().into_iter().map(|(name, text)| (name, Arc::new(parse(&text).unwrap()))))
        .collect();
    if let Some(loose) = std::env::var_os("ANGEL_PROP_DIR") {
        props.clear();
        let mut files: Vec<_> = std::fs::read_dir(loose).unwrap().flatten().map(|e| e.path()).collect();
        files.sort();
        for path in files.into_iter().filter(|p| p.extension().is_some_and(|e| e == "obj")) {
            let text = std::fs::read_to_string(&path).unwrap();
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            props.push((name, Arc::new(parse(&text).unwrap())));
        }
    }
    for (name, prop) in props {
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for p in prop.tris.iter().flat_map(|t| t.v) {
            lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        let size = (hi.x - lo.x).max(hi.y - lo.y).max(hi.z - lo.z);
        let mut scene = Mesh::new();
        let r = size * 1.6;
        scene.push_quad(v3(-r, -r, 0.0), v3(r, -r, 0.0), v3(r, r, 0.0), v3(-r, r, 0.0), mat::PATH);
        scene.merge(Mesh { tris: prop.tris.clone() });
        for (label, w, h) in [("large", 480, 360), ("mini", 160, 120)] {
            let distance = size * 1.9;
            // Front three-quarter: a converted prop faces -y.
            let heading = std::f32::consts::FRAC_PI_2 + 0.5;
            let view = View3 {
                pos: v3(
                    -heading.cos() * distance,
                    -heading.sin() * distance,
                    (hi.z * 0.75).max(0.6),
                ),
                heading_rad: heading,
                pitch: -0.18,
                fov_rad: 0.95,
            };
            let frame = raster::render_scene(&scene, &view, w, h);
            frame.save(dir.join(format!("{name}-{label}.png"))).unwrap();
        }
    }
}

/// `cargo test --bin angel print_room_budgets -- --ignored --nocapture`
#[test]
#[ignore]
fn print_room_budgets() {
    use crate::stage::world_viz::world3d::interior;
    for index in 0..8u8 {
        eprintln!("{index} {:<14} {} tris", interior::hero(index), interior::interior_mesh(index).tris.len());
    }
}

#[test]
fn placed_sculptures_keep_their_face_counts_feet_extents_and_material_identity() {
    for &(name, text) in PROPS {
        let raw = parse(text).unwrap();
        let finish = prop(name).unwrap();
        assert_eq!(
            finish.tris.len(),
            raw.tris.len(),
            "{name}: no added frame work"
        );
        let bounds = |mesh: &Mesh| {
            let mut low = [f32::MAX; 3];
            let mut high = [f32::MIN; 3];
            for p in mesh.tris.iter().flat_map(|t| t.v) {
                for (k, n) in [p.x, p.y, p.z].into_iter().enumerate() {
                    low[k] = low[k].min(n);
                    high[k] = high[k].max(n);
                }
            }
            (low, high)
        };
        assert_eq!(bounds(&raw), bounds(&finish), "{name}: placement footprint");
        assert!(finish.tris.iter().all(|t| t.normal.length() > 0.99));
        if name == "dragon_skull" {
            assert!(finish.tris.iter().any(|t| t.mat == mat::BONE));
        }
        if name == "knight_statue" {
            assert!(finish.tris.iter().any(|t| t.mat == mat::ARMOR));
        }
    }
}
