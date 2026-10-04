//! Compile plugged-in source cartridges into the cockpit.
//!
//! A cartridge is a folder with `cartridge.toml`; one with `src/mod.rs` is a
//! source cartridge, built in here as a module with `HOOKS`. Folders come from
//! `ANGEL_CARTRIDGES` (a path list; each entry is a cartridge or a folder of
//! them), else `~/.angelX/cartridges` — the same places the cockpit reads
//! config-only cartridges from at startup. See docs/CARTRIDGES.md.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
        watch(&folder);
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
    fs::write(out, code).expect("write cartridges.rs");
    println!("cargo:rustc-env=ANGEL_COMPILED_CARTRIDGES={}", ids.join(";"));
}

/// Every cartridge folder under `roots`, in order (as `config::folders`).
fn folders(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in roots {
        println!("cargo:rerun-if-changed={}", root.display());
        if root.join("cartridge.toml").is_file() {
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
        dirs.sort();
        found.extend(dirs);
    }
    found
}

/// Rebuild when any file of a compiled cartridge changes.
fn watch(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target" || n == ".git") {
                continue;
            }
            watch(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
