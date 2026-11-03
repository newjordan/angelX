//! Compile plugged-in source cartridges into the cockpit.
//!
//! A cartridge is a folder with `cartridge.toml`; one with `src/mod.rs` is a
//! source cartridge, built in here as a module with `HOOKS`. Folders come from
//! `ANGEL_CARTRIDGES` (a path list; each entry is a cartridge or a folder of
//! them), else `~/.angelX/cartridges` — the same places the cockpit reads
//! config-only cartridges from at startup. See docs/CARTRIDGES.md.
//!
//! Browser assets live in the independent `angel-web-bundle` dependency.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Write only when the content changed, so a build-script re-run that
/// produces the same generated source does not dirty the cockpit build.
fn write_if_changed(path: &Path, bytes: &[u8]) {
    if fs::read(path).is_ok_and(|old| old == bytes) {
        return;
    }
    fs::write(path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ANGEL_CARTRIDGES");
    println!("cargo:rerun-if-env-changed=HOME");
    let roots: Vec<PathBuf> = match env::var_os("ANGEL_CARTRIDGES") {
        Some(list) => env::split_paths(&list).collect(),
        None => env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".angelX").join("cartridges"))
            .into_iter()
            .collect(),
    };
    let mut modules = String::new();
    let mut entries = Vec::new();
    let mut ids = Vec::new();
    for folder in folders(&roots) {
        if !folder.join("src").join("mod.rs").is_file() {
            continue;
        }
        let folder = fs::canonicalize(&folder).expect("cartridge folder");
        // Rust tracks this module and its includes through dep-info. The build
        // script only generates its path; cartridge notes, bins and tests must
        // not rerun native compilation and browser packaging.
        let name = format!(
            "cartridge_{}",
            folder
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
                .collect::<String>()
        );
        let toml = folder.join("cartridge.toml");
        let source = folder.join("src").join("mod.rs");
        // Removing the entry module changes the generated cartridge registry.
        println!("cargo:rerun-if-changed={}", source.display());
        modules.push_str(&format!("#[path = {:?}]\npub(crate) mod {name};\n", source));
        entries.push(format!(
            "    (include_str!({:?}), &{name}::HOOKS as &dyn Hooks),\n",
            toml
        ));
        ids.push(format!("{name}={}", folder.display()));
    }
    let code = format!(
        "{modules}\n/// Source cartridges compiled into this build: their toml and hooks.\npub(crate) static COMPILED: &[(&str, &dyn Hooks)] = &[\n{}];\n",
        entries.concat()
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("cartridges.rs");
    write_if_changed(&out, code.as_bytes());
    println!("cargo:rustc-env=ANGEL_COMPILED_CARTRIDGES={}", ids.join(";"));
    build_paths();
}

/// Where this build ran, for a checkout's own runs: the source tree's resources
/// and the cargo that built it. A portable build (a prebuilt package sets
/// `ANGEL_PORTABLE_BUILD=1`) names neither, so no shipped binary carries the
/// builder's folders.
fn build_paths() {
    println!("cargo:rerun-if-env-changed=ANGEL_PORTABLE_BUILD");
    if env::var_os("ANGEL_PORTABLE_BUILD").is_some_and(|v| v == "1") {
        return;
    }
    if let Some(dir) = env::var_os("CARGO_MANIFEST_DIR") {
        println!("cargo:rustc-env=ANGEL_SOURCE_COCKPIT_DIR={}", PathBuf::from(dir).display());
    }
    if let Some(cargo) = env::var_os("CARGO") {
        println!("cargo:rustc-env=ANGEL_BUILD_CARGO={}", PathBuf::from(cargo).display());
    }
}

/// Every cartridge folder under `roots`, in order (as `config::folders`).
fn folders(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    // Not the roots themselves: cargo watches a folder through all its
    // depth, so any write in any cartridge (a config-only one's notes, its
    // .git) rebuilt the cockpit. Each cartridge's toml is watched here and its
    // entry module above; Rust tracks the other source dependencies. A source
    // cartridge plugged in later is compiled in by the next rebuild
    // (`touch cockpit/build.rs`).
    for root in roots {
        if root.join("cartridge.toml").is_file() {
            println!("cargo:rerun-if-changed={}", root.join("cartridge.toml").display());
            found.push(root.clone());
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.join("cartridge.toml").is_file())
            .collect();
        for dir in &dirs {
            println!("cargo:rerun-if-changed={}", dir.join("cartridge.toml").display());
        }
        dirs.sort();
        found.extend(dirs);
    }
    found
}
