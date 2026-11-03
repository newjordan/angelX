//! Embed the shipped policy generations (feature `v8`). Every `kind: policy`
//! module of `cockpit/live` is captured by the same rules as the live loader
//! (`bundle.rs` `visit`): recursive `*.js`/`*.bend`, the root `module.json` and
//! `bend-build.json`, dotfiles skipped.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_V8").is_none() {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let live = fs::canonicalize(manifest.join("../../cockpit/live"))
        .expect("the shipped modules live in cockpit/live, beside the crates workspace");
    println!("cargo:rerun-if-changed={}", live.display());
    let mut table =
        String::from("pub(crate) static SHIPPED_LEGS: &[(&str, &[(&str, &str)])] = &[\n");
    let mut dirs: Vec<PathBuf> = fs::read_dir(&live)
        .expect("cockpit/live")
        .map(|entry| entry.expect("live entry").path())
        .filter(|dir| dir.join("module.json").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        let manifest = fs::read_to_string(dir.join("module.json")).expect("module.json");
        let compact: String = manifest.chars().filter(|c| !c.is_whitespace()).collect();
        println!("cargo:rerun-if-changed={}", dir.display());
        if !compact.contains("\"kind\":\"policy\"") {
            continue;
        }
        let id = dir.file_name().and_then(|n| n.to_str()).expect("module id");
        let mut files = Vec::new();
        leg_files(&dir, &dir, &mut files);
        files.sort();
        table.push_str(&format!("    ({id:?}, &[\n"));
        for (name, path) in files {
            println!("cargo:rerun-if-changed={}", path.display());
            table.push_str(&format!(
                "        ({name:?}, include_str!({:?})),\n",
                path.display().to_string()
            ));
        }
        table.push_str("    ]),\n");
    }
    table.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("shipped_legs.rs");
    fs::write(out, table).expect("write shipped_legs.rs");
}

fn leg_files(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(dir).expect("leg dir") {
        let path = entry.expect("leg entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            leg_files(root, &path, files);
        } else if path.extension().is_some_and(|v| v == "js" || v == "bend")
            || path == root.join("module.json")
            || path == root.join("bend-build.json")
        {
            let rel = path
                .strip_prefix(root)
                .expect("relative")
                .to_string_lossy()
                .replace('\\', "/");
            files.push((rel, path));
        }
    }
}
