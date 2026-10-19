//! The town's two residents are painted at 1254 px square and drawn 20 px
//! tall. Shrinking them here, once, at build time, keeps a megabyte of PNG
//! out of the binary and out of every browser's download: the renderer gets
//! the same cropped, resized pixels it used to make on its first frame
//! (`render::overworld::clerks`, whose tests hold the two ways equal).

use std::path::Path;
mod arcade_gate;

/// The residents, by name, and their paintings.
const RESIDENTS: [(&str, &str); 2] = [
    (
        "archivist",
        "../../cockpit/assets/realm/inhabitants/archivist.png",
    ),
    (
        "courier",
        "../../cockpit/assets/realm/inhabitants/courier.png",
    ),
];
/// The sprite's height, in pixels (`clerks::HEIGHT`).
const HEIGHT: u32 = 20;
/// A pixel this opaque is part of the figure.
const SOLID: u8 = 96;

fn main() {
    // A checkout's build reads its own asset folder (`world3d::blockout::
    // landmark_folder`). A portable build (a prebuilt package sets
    // `ANGEL_PORTABLE_BUILD=1`) names no folder, so no shipped binary carries
    // the builder's.
    println!("cargo:rerun-if-env-changed=ANGEL_PORTABLE_BUILD");
    if std::env::var_os("ANGEL_PORTABLE_BUILD").is_none_or(|v| v != "1") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        println!("cargo:rustc-env=ANGEL_SOURCE_DELVE_DIR={dir}");
    }
    println!("cargo:rerun-if-changed=arcade_gate.rs");
    println!("cargo:rerun-if-changed=assets/arcade");
    println!("cargo:rerun-if-changed=src/render/overworld/ink.rs");
    arcade_gate::validate(Path::new("assets/arcade"))
        .unwrap_or_else(|e| panic!("arcade gate:\n{e}"));
    if std::env::var_os("CARGO_FEATURE_ARCADE_REVIEW").is_none() {
        arcade_gate::validate_cells(Path::new("assets/arcade")).unwrap_or_else(|e| panic!("arcade cell gate: {e}; regenerate with the arcade-review feature (docs/ART-ARCADE.md)"));
    }
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    slot_art(&out);
    boss_assets(&out);
    let mut assets = String::from("const FILES: &[(&str,&[u8])] = &[\n");
    for kit in ["knight", "fx"] {
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(format!("assets/arcade/{kit}/manifest.json")).unwrap(),
        )
        .unwrap();
        for animation in manifest["states"].as_object().unwrap().values() {
            for frame in animation["frames"].as_array().unwrap() {
                let file = frame["file"].as_str().unwrap();
                assets.push_str(&format!("({:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/arcade/{kit}/{file}\"))),\n", format!("{kit}/{file}")));
            }
        }
    }
    assets.push_str("];\n");
    std::fs::write(Path::new(&out).join("arcade_assets.rs"), assets).unwrap();
    for (name, path) in RESIDENTS {
        println!("cargo:rerun-if-changed={path}");
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let shrunk = shrink(&bytes);
        std::fs::write(Path::new(&out).join(format!("{name}.rgba")), shrunk)
            .expect("OUT_DIR is writable");
    }
}

/// Resolve swappable sprite slots at build time, including their rebuild inputs.
fn boss_assets(out: &str) {
    let root = Path::new("../../");
    let mut files = std::collections::BTreeSet::new();
    for name in ["boss-library-registry.json", "boss-effect-slots.json"] {
        let path = root.join("scripts").join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (id, entry) in data.as_object().unwrap() {
            if name == "boss-effect-slots.json" {
                for (role, count) in [("projectile", 2), ("tell", 4), ("impact", 5)] {
                    let frames = entry[role].as_array().expect("effect frame array");
                    assert_eq!(frames.len(), count, "{id}/{role}: simulation phase slots");
                    for file in frames { files.insert(format!("{id}/{}", file.as_str().unwrap())); }
                }
                if let Some(frames)=entry["projectile_v2"].as_array() {
                    assert_eq!(frames.len(),2,"{id}: two native flight frames");
                    for file in frames {files.insert(format!("{id}/{}",file.as_str().unwrap()));}
                }
            } else {
                if entry["active"] == false && std::env::var_os("CARGO_FEATURE_BOSS_LIBRARY_REVIEW").is_none() {
                    continue;
                }
                for kit in ["v1", "v2"] {
                    for pose in ["rest", "windup"] {
                        files.insert(format!("{id}/library-{kit}-{pose}.png"));
                        if std::env::var_os("CARGO_FEATURE_BOSS_LIBRARY_REVIEW").is_some() {
                            for i in 1..=entry["study"][kit].as_array().unwrap().len() {
                                files.insert(format!("{id}/library-{kit}-study-{i}-{pose}.png"));
                            }
                        }
                    }
                }
            }
        }
    }
    let mut code = String::from("const BOSS_FILES: &[(&str, &[u8])] = &[\n");
    for file in files {
        let path = root.join("cockpit/assets/dungeon/bosses").join(&file).canonicalize().unwrap();
        println!("cargo:rerun-if-changed={}", path.display());
        code.push_str(&format!("({file:?}, include_bytes!({:?})),\n", path));
    }
    code.push_str("];\n");
    std::fs::write(Path::new(out).join("boss_assets.rs"), code).unwrap();
}

/// The figure cropped to its solid pixels and resized to `HEIGHT`, nearest
/// neighbour: `[w: u16 LE][h: u16 LE]` then RGBA rows. An empty painting is
/// 0 by 0.
fn shrink(png: &[u8]) -> Vec<u8> {
    let source = image::load_from_memory(png)
        .expect("a resident's PNG")
        .to_rgba8();
    let (mut left, mut top) = source.dimensions();
    let (mut right, mut bottom) = (0, 0);
    for (x, y, pixel) in source.enumerate_pixels() {
        if pixel[3] >= SOLID {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    let mut out = Vec::new();
    if right <= left || bottom <= top {
        out.extend_from_slice(&[0, 0, 0, 0]);
        return out;
    }
    let width = ((right - left) * HEIGHT / (bottom - top)).max(1);
    let cropped = image::imageops::crop_imm(&source, left, top, right - left, bottom - top);
    let small = image::imageops::resize(
        &cropped.to_image(),
        width,
        HEIGHT,
        image::imageops::FilterType::Nearest,
    );
    out.extend_from_slice(&(width as u16).to_le_bytes());
    out.extend_from_slice(&(HEIGHT as u16).to_le_bytes());
    out.extend_from_slice(small.as_raw());
    out
}

/// Only reviewed frame mappings enter the binary. Missing slots retain their
/// original embedded art, including builds without the staging directory.
fn slot_art(out: &str) {
    let root = Path::new("assets/art-slots");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut code = String::from(
        "pub(super) fn bytes(slot: &str, name: &str) -> Option<&'static [u8]> { match (slot, name) {\n",
    );
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let id = dir.file_name().unwrap().to_str().unwrap();
        let map_path = dir.join(format!("{id}.map.json"));
        println!("cargo:rerun-if-changed={}", map_path.display());
        let Ok(data) = std::fs::read(&map_path) else {
            continue;
        };
        let map: serde_json::Value = serde_json::from_slice(&data).expect("slot map JSON");
        if map["runtime"] != true {
            continue;
        }
        for frame in map["pieces"].as_array().expect("slot pieces") {
            if frame["runtime"] != true || frame["approved"] != true {
                continue;
            }
            let name = frame["name"].as_str().expect("slot frame name");
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            );
            let kits = map["embed_kits"].as_array().map(|v| v.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>()).unwrap_or_else(|| vec!["v1"]);
            for kit in kits {
                assert!(matches!(kit, "v1" | "v2"));
                let embedded = if kit == "v1" { name.to_string() } else { format!("v2-{name}") };
                let path = dir.join(kit).join(format!("{name}.png"));
                println!("cargo:rerun-if-changed={}", path.display());
                if path.exists() {
                    let path = std::fs::canonicalize(path).unwrap();
                    code.push_str(&format!("({id:?}, {embedded:?}) => Some(include_bytes!({:?})),\n", path.to_str().unwrap()));
                    let mask = dir.join(kit).join(format!("{name}-seat-mask.png"));
                    println!("cargo:rerun-if-changed={}", mask.display());
                    if mask.exists() {
                        let mask = std::fs::canonicalize(mask).unwrap();
                        code.push_str(&format!("({id:?}, {:?}) => Some(include_bytes!({:?})),\n", format!("{embedded}-seat-mask"), mask.to_str().unwrap()));
                    }
                }
            }
        }
    }
    code.push_str("_ => None, }}\n");
    std::fs::write(Path::new(out).join("slot_art.rs"), code).expect("slot art embedding");
}
