//! The town's two residents are painted at 1254 px square and drawn 20 px
//! tall. Shrinking them here, once, at build time, keeps a megabyte of PNG
//! out of the binary and out of every browser's download: the renderer gets
//! the same cropped, resized pixels it used to make on its first frame
//! (`render::overworld::clerks`, whose tests hold the two ways equal).

use std::path::{Path, PathBuf};
mod build_watch;
mod arcade_gate;
mod frontier_art_build;
mod village_art_build;

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

/// Write only when the content changed, so a build-script re-run that
/// produces the same generated source does not dirty the library build.
fn write_if_changed(path: &Path, bytes: &[u8]) {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return;
    }
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn main() {
    // A house's private table (`rules/private_table.rs`, left out of the
    // public tree by release/public-exclude.txt): where the file is, rude
    // wishes are granted as asked. Before the rules-only return below, so
    // the rules worker is built the same way.
    println!("cargo:rustc-check-cfg=cfg(private_table)");
    println!("cargo:rerun-if-changed=src/rules/private_table.rs");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    if std::path::Path::new(&manifest).join("src/rules/private_table.rs").exists() {
        println!("cargo:rustc-cfg=private_table");
    }
    // A rules-only worker embeds no renderer or image assets.
    if std::env::var_os("CARGO_FEATURE_RENDERER").is_none() {
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }
    factory_sounds();
    // A checkout's build reads its own asset folder (`world3d::blockout::
    // landmark_folder`). A portable build (a prebuilt package sets
    // `ANGEL_PORTABLE_BUILD=1`) names no folder, so no shipped binary carries
    // the builder's.
    println!("cargo:rerun-if-env-changed=ANGEL_PORTABLE_BUILD");
    if std::env::var_os("ANGEL_PORTABLE_BUILD").is_none_or(|v| v != "1") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
        println!("cargo:rustc-env=ANGEL_SOURCE_DELVE_DIR={dir}");
    }
    let mut watched: Vec<PathBuf> = ["build.rs", "build_watch.rs", "arcade_gate.rs", "assets/arcade", "src/render/overworld/ink.rs"]
        .into_iter().map(PathBuf::from).collect();
    arcade_gate::validate_watched(Path::new("assets/arcade"), |path| watched.push(path.to_path_buf()))
        .unwrap_or_else(|e| panic!("arcade gate:\n{e}"));
    if std::env::var_os("CARGO_FEATURE_ARCADE_REVIEW").is_none() {
        arcade_gate::validate_cells(Path::new("assets/arcade")).unwrap_or_else(|e| panic!("arcade cell gate: {e}; regenerate with the arcade-review feature (docs/ART-ARCADE.md)"));
    }
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    kingdom_art(&out, &mut watched);
    frontier_art_build::build(&out, &mut watched);
    village_art_build::build(&out, &mut watched);
    slot_art(&out, &mut watched);
    boss_assets(&out, &mut watched);
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
    write_if_changed(&Path::new(&out).join("arcade_assets.rs"), assets.as_bytes());
    for (name, path) in RESIDENTS {
        watched.push(PathBuf::from(path));
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let shrunk = shrink(&bytes);
        write_if_changed(&Path::new(&out).join(format!("{name}.rgba")), &shrunk);
    }
    let cwd = std::env::current_dir().expect("build script cwd");
    for path in build_watch::minimal_paths(&cwd, watched) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// Resolve swappable sprite slots at build time, including their rebuild inputs.
fn boss_assets(out: &str, watched: &mut Vec<PathBuf>) {
    let root = Path::new("../../");
    let mut files = std::collections::BTreeSet::new();
    for name in ["boss-library-registry.json", "boss-effect-slots.json"] {
        let path = root.join("scripts").join(name);
        watched.push(path.to_path_buf());
        let data: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        for (id, entry) in data.as_object().unwrap() {
            if name == "boss-effect-slots.json" {
                for (role, count) in [("projectile", 2), ("tell", 4), ("impact", 5)] {
                    let frames = entry[role].as_array().expect("effect frame array");
                    assert_eq!(frames.len(), count, "{id}/{role}: simulation phase slots");
                    for file in frames { files.insert(format!("{id}/{}", file.as_str().unwrap())); }
                }
            } else {
                if entry["active"] == false && std::env::var_os("CARGO_FEATURE_BOSS_LIBRARY_REVIEW").is_none() {
                    continue;
                }
                for pose in ["rest", "windup"] {
                    files.insert(format!("{id}/library-v1-{pose}.png"));
                    if std::env::var_os("CARGO_FEATURE_BOSS_LIBRARY_REVIEW").is_some() {
                        for i in 1..=entry["study"]["v1"].as_array().unwrap().len() {
                            files.insert(format!("{id}/library-v1-study-{i}-{pose}.png"));
                        }
                    }
                }
            }
        }
    }
    let mut code = String::from("const BOSS_FILES: &[(&str, &[u8])] = &[\n");
    for file in files {
        let path = root.join("cockpit/assets/dungeon/bosses").join(&file).canonicalize().unwrap();
        watched.push(path.to_path_buf());
        code.push_str(&format!("({file:?}, include_bytes!({:?})),\n", path));
    }
    code.push_str("];\n");
    write_if_changed(&Path::new(out).join("boss_assets.rs"), code.as_bytes());
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
fn slot_art(out: &str, watched: &mut Vec<PathBuf>) {
    let root = Path::new("assets/art-slots");
    watched.push(root.to_path_buf());
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
        // A missing map must not be watched: cargo counts that as changed
        // and would rebuild the Delve on every build. The slot root is
        // already watched, so a map that arrives later still rebuilds.
        if !map_path.is_file() {
            continue;
        }
        watched.push(map_path.to_path_buf());
        let data = std::fs::read(&map_path).expect("slot map");
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
            // The first kit's frames only (`v1`); the second kit's were cut
            // on 2026-10-20 and archived.
            {
                let kit = "v1";
                let embedded = name.to_string();
                // Only paths that exist are watched: cargo counts a watched
                // path that is missing as changed, which rebuilt the Delve
                // (and the browser page's wasm with it) on every build. The
                // kit folder is watched when it is there, so a piece or mask
                // that arrives later still rebuilds. The slot root is already
                // watched, so a kit folder that is not there yet is noticed
                // when it appears, without watching the missing path.
                let kit_dir = dir.join(kit);
                if kit_dir.exists() {
                    watched.push(kit_dir.to_path_buf());
                }
                let path = kit_dir.join(format!("{name}.png"));
                if path.exists() {
                    watched.push(path.to_path_buf());
                    let path = std::fs::canonicalize(path).unwrap();
                    code.push_str(&format!("({id:?}, {embedded:?}) => Some(include_bytes!({:?})),\n", path.to_str().unwrap()));
                    let mask = dir.join(kit).join(format!("{name}-seat-mask.png"));
                    if mask.exists() {
                        watched.push(mask.to_path_buf());
                        let mask = std::fs::canonicalize(mask).unwrap();
                        code.push_str(&format!("({id:?}, {:?}) => Some(include_bytes!({:?})),\n", format!("{embedded}-seat-mask"), mask.to_str().unwrap()));
                    }
                }
            }
        }
    }
    code.push_str("_ => None, }}\n");
    write_if_changed(&Path::new(out).join("slot_art.rs"), code.as_bytes());
}

// Factory attack metadata names the very same sounds the game bundle ships.
// Build an index, not an authoring path in catalog/save/wire data.
fn factory_sounds() {
    fn collect(dir: &Path, prefix: &str, picks: &mut Vec<String>) {
        println!("cargo:rerun-if-changed={}", dir.display());
        for entry in std::fs::read_dir(dir).expect("game sound directory") {
            let path = entry.expect("game sound entry").path();
            if path.is_dir() {
                collect(
                    &path,
                    &format!("{prefix}{}/", path.file_name().unwrap().to_str().unwrap()),
                    picks,
                );
            } else if path.extension().is_some_and(|ext| ext == "mp3") {
                let pick = format!("{prefix}{}", path.file_stem().unwrap().to_str().unwrap());
                picks.push(pick.replace('/', "-"));
                picks.push(pick);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cockpit/assets/dungeon/sfx");
    let mut picks = Vec::new();
    collect(&root, "", &mut picks);
    picks.sort();
    picks.dedup();
    let out = std::env::var_os("OUT_DIR").unwrap();
    std::fs::write(
        Path::new(&out).join("factory_sounds.rs"),
        format!("const SOUNDS: &[&str] = &{picks:?};"),
    )
    .unwrap();
}

// Runtime-sized portraits keep the original ImageGen paintings in the source
// library without making every joined browser download twelve full masters.
fn kingdom_art(out: &str, watched: &mut Vec<PathBuf>) {
    let ore_root=PathBuf::from("../../cockpit/assets/kingdom/monsters");
    for (manifest_name,suffix) in [("ore-enemies-manifest.json",""),("ore-enemies-windup-manifest.json","-wind")] {
        let ore_manifest=ore_root.join(manifest_name);
        watched.push(ore_manifest.clone());
        let manifest:serde_json::Value=serde_json::from_slice(&std::fs::read(&ore_manifest).expect("ore manifest")).expect("valid ore manifest");
        let atlas_path=ore_root.join(manifest["atlas"].as_str().unwrap());
        watched.push(atlas_path.clone());
        let atlas=image::open(&atlas_path).expect("ore atlas").into_rgba8();
        for entry in manifest["assets"].as_array().unwrap() {
            let id=entry["id"].as_str().unwrap();
            let r=entry["rect"].as_array().unwrap();
            let rect:[u32;4]=std::array::from_fn(|i|r[i].as_u64().unwrap() as u32);
            let crop=image::imageops::crop_imm(&atlas,rect[0],rect[1],rect[2],rect[3]).to_image();
            // Raised hammer and opened prism arms extend the silhouette;
            // preserve torso size instead of shrinking the whole body.
            let size=match (suffix,id) {("-wind","iron-ore-sentinel")=>44,("-wind","crystal-prism-stalker")=>40,_=>36};
            let pixels=frontier_art_build::prepare(&crop,size,false);
            write_if_changed(&Path::new(out).join(format!("ore-{id}{suffix}.rgba")),pixels.as_raw());
        }
    }
    for (folder, names) in [
        ("items", &[
            "iron-oathblade", "ember-edge", "mooncrystal-staff", "hearth-wardstone", "prospectors-pick", "runebound-aegis",
        ][..]),
        ("monsters", &[
            "ironroot-colossus", "cindermaw", "verdigris-mantis", "amethyst-crowned-wyrm", "mossback-briarhorn", "chalkwing-gargoyle",
        ][..]),
    ] {
        for name in names {
            let path = PathBuf::from(format!("../../cockpit/assets/kingdom/{folder}/{name}.png"));
            watched.push(path.clone());
            let source = image::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            if folder == "monsters" && matches!(*name, "ironroot-colossus" | "cindermaw" | "amethyst-crowned-wyrm") {
                // Combat uses the visible body, not the portrait's generous
                // transparent canvas. Keep this a build asset conversion so
                // source paintings and the browser download stay small.
                let wind_path = PathBuf::from(format!("../../cockpit/assets/kingdom/monsters/combat/{name}-wind.png"));
                watched.push(wind_path.clone());
                let poses = [source.to_rgba8(), image::open(&wind_path).expect("claim wind-up image").to_rgba8()];
                let crops = poses.map(|rgba| {
                    let (mut x0,mut y0,mut x1,mut y1) = (rgba.width(),rgba.height(),0,0);
                    for (x,y,_) in rgba.enumerate_pixels().filter(|(_,_,p)| p[3] >= 128) {
                        x0=x0.min(x); y0=y0.min(y); x1=x1.max(x+1); y1=y1.max(y+1);
                    }
                    image::imageops::crop_imm(&rgba,x0,y0,x1-x0,y1-y0).to_image()
                });
                let extent = crops.iter().map(|p|p.width().max(p.height())).max().unwrap();
                // Rest and raised-arm poses share one scale and floor anchor.
                for (pose,crop) in crops.into_iter().enumerate() {
                    let (w,h) = ((crop.width()*60/extent).max(1),(crop.height()*60/extent).max(1));
                    let body = image::imageops::resize(&crop,w,h,image::imageops::FilterType::Lanczos3);
                    let mut combat = image::RgbaImage::new(64,64);
                    image::imageops::overlay(&mut combat,&body,i64::from((64-body.width())/2),i64::from(64-body.height()));
                    let suffix = if pose == 0 { "" } else { "-wind" };
                    write_if_changed(&Path::new(out).join(format!("claim-{name}{suffix}.rgba")),combat.as_raw());
                }
            }
            let rgba=source.to_rgba8();
            let (mut x0,mut y0,mut x1,mut y1)=(rgba.width(),rgba.height(),0,0);
            for (x,y,p) in rgba.enumerate_pixels() { if p[3]>=32 {x0=x0.min(x);y0=y0.min(y);x1=x1.max(x+1);y1=y1.max(y+1);} }
            if x1>x0 && y1>y0 {
                let crop=image::imageops::crop_imm(&rgba,x0,y0,x1-x0,y1-y0).to_image();
                let extent=crop.width().max(crop.height());
                let gallery=image::imageops::resize(&crop,(crop.width()*120/extent).max(1),(crop.height()*120/extent).max(1),image::imageops::FilterType::Lanczos3);
                let mut canvas=image::RgbaImage::new(128,128);
                image::imageops::overlay(&mut canvas,&gallery,i64::from((128-gallery.width())/2),i64::from(124-gallery.height()));
                write_if_changed(&Path::new(out).join(format!("kingdom-gallery-{name}.rgba")),canvas.as_raw());
            }
            let image = source.resize(48, 48, image::imageops::FilterType::Lanczos3).into_rgba8();
            let mut canvas = image::RgbaImage::new(48,48);
            image::imageops::overlay(&mut canvas, &image, i64::from((48-image.width())/2), i64::from(48-image.height()));
            write_if_changed(&Path::new(out).join(format!("kingdom-{name}.rgba")), canvas.as_raw());
        }
    }
}
