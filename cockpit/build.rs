//! Compile plugged-in source cartridges into the cockpit, and build the
//! Delve's browser page into it.
//!
//! A cartridge is a folder with `cartridge.toml`; one with `src/mod.rs` is a
//! source cartridge, built in here as a module with `HOOKS`. Folders come from
//! `ANGEL_CARTRIDGES` (a path list; each entry is a cartridge or a folder of
//! them), else `~/.angelX/cartridges` — the same places the cockpit reads
//! config-only cartridges from at startup. See docs/CARTRIDGES.md.
//!
//! The Delve's page is the angel-web shell with the Delve's wasm client
//! (`crates/web/delve`), built for `wasm32-unknown-unknown` by a nested
//! cargo with its own target directory and a clean environment, and
//! embedded: a page and its host always match (`docs/ENGINE.md`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    delve_web();
}

/// The game's name in the bundle, as `crates/web/build.sh` names it.
const DELVE: &str = "delve";

/// Build the Delve's browser bundle and write `delve_web.rs`, which embeds
/// it: the page, the shell, the wasm and the sounds, as
/// `crates/web/build.sh web/delve delve` lays them out.
fn delve_web() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let crates = manifest.parent().expect("the cockpit sits in the repository").join("crates");
    let web = crates.join("web");
    let game = web.join(DELVE);
    let code = out.join("delve_web.rs");
    println!("cargo:rerun-if-changed={}", game.display());
    if !game.join("Cargo.toml").is_file() {
        fs::write(&code, "/// No browser page in this build.\npub(crate) static BUNDLE: Option<(&str, &[(&str, &[u8])])> = None;\n")
            .expect("write delve_web.rs");
        return;
    }
    for dir in [web.join("shell"), web.join("src"), game.clone(), crates.join("delve").join("src"), crates.join("net").join("src")] {
        watch(&dir);
    }
    for file in ["Cargo.toml", "Cargo.lock", "delve/Cargo.toml", "net/Cargo.toml", "web/Cargo.toml"] {
        println!("cargo:rerun-if-changed={}", crates.join(file).display());
    }
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let sysroot = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .expect("could not ask rustc for its sysroot");
    if !sysroot.join("lib/rustlib/wasm32-unknown-unknown").is_dir() {
        panic!(
            "the Delve's browser page needs Rust's wasm32-unknown-unknown target, which this toolchain ({}) does not have: \
             rustup target add wasm32-unknown-unknown",
            sysroot.display()
        );
    }
    let package = fs::read_to_string(game.join("Cargo.toml"))
        .ok()
        .and_then(|toml| {
            toml.lines()
                .find_map(|line| line.trim().strip_prefix("name").map(|rest| rest.trim().trim_start_matches('=').trim().trim_matches('"').to_string()))
        })
        .expect("crates/web/delve/Cargo.toml names its package");
    let target = out.join("delve-web-target");
    // A clean environment: none of this build's flags, profile or target
    // directory reach the wasm build, only where the toolchain and the
    // crates' cache are.
    let mut cargo = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo.env_clear();
    for key in ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "TMPDIR", "TEMP", "TMP", "SystemRoot", "USERPROFILE"] {
        if let Some(value) = env::var_os(key) {
            cargo.env(key, value);
        }
    }
    cargo.env("RUSTC", &rustc);
    cargo
        .arg("build")
        .arg("--manifest-path")
        .arg(crates.join("Cargo.toml"))
        .args(["-p", &package, "--profile", "web", "--target", "wasm32-unknown-unknown", "--locked", "--target-dir"])
        .arg(&target);
    let status = cargo.status().expect("could not run cargo for the Delve's browser page");
    if !status.success() {
        panic!("the Delve's browser page did not build (cargo build -p {package} --profile web --target wasm32-unknown-unknown in crates/)");
    }
    let lib = package.replace('-', "_");
    let wasm = target.join("wasm32-unknown-unknown").join("web").join(format!("{lib}.wasm"));
    let dist = out.join("delve-web");
    let _ = fs::remove_dir_all(&dist);
    fs::create_dir_all(&dist).expect("make the bundle's folder");
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut copy = |name: String, from: &Path| {
        let to = dist.join(&name);
        fs::copy(from, &to).unwrap_or_else(|e| panic!("copy {} into the bundle: {e}", from.display()));
        files.push((name, to));
    };
    copy(format!("{DELVE}.wasm"), &wasm);
    let mut shell: Vec<PathBuf> = fs::read_dir(web.join("shell"))
        .expect("the shell's folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "js") || p.file_name().is_some_and(|n| n == "shell.css"))
        .collect();
    shell.sort();
    for path in shell {
        copy(path.file_name().expect("a file").to_string_lossy().into_owned(), &path);
    }
    // The sounds manifest: {"name": "{{asset:name.ext}}", ...}, inline in
    // the page so the server's asset names reach it. The sounds are the
    // crate's own `sounds/` and the folders its `sounds.list` names (a name
    // prefix, "-" for none, then a folder), as `crates/web/build.sh` takes them.
    let mut folders = vec![(String::new(), game.join("sounds"))];
    let list = game.join("sounds.list");
    println!("cargo:rerun-if-changed={}", list.display());
    if let Ok(text) = fs::read_to_string(&list) {
        for line in text.lines() {
            let mut words = line.split_whitespace();
            let (Some(prefix), Some(folder)) = (words.next(), words.next()) else {
                continue;
            };
            if prefix.starts_with('#') {
                continue;
            }
            let dir = game.join(folder);
            if !dir.is_dir() {
                panic!("{}: no folder {folder}", list.display());
            }
            folders.push((if prefix == "-" { String::new() } else { prefix.to_string() }, dir));
        }
    }
    let mut sounds = Vec::new();
    for (prefix, dir) in folders {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
        paths.sort();
        for path in paths {
            let file = format!("{prefix}{}", path.file_name().expect("a file").to_string_lossy());
            if !file.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
                continue;
            }
            let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s).to_string();
            sounds.push(format!("\"{stem}\":\"{{{{asset:{file}}}}}\""));
            files.push((file, path));
        }
    }
    let title = format!("{}{}", DELVE[..1].to_ascii_uppercase(), &DELVE[1..]);
    let index = fs::read_to_string(web.join("shell").join("index.html"))
        .expect("the shell's page")
        .replace("@@GAME@@", DELVE)
        .replace("@@TITLE@@", &title)
        .replace("@@SOUNDS@@", &format!("{{{}}}", sounds.join(",")));
    let index_path = dist.join("index.html");
    fs::write(&index_path, index).expect("write the bundle's page");
    let entries: String = files
        .iter()
        .map(|(name, path)| format!("    ({name:?}, include_bytes!({:?})),\n", path.display().to_string()))
        .collect();
    fs::write(
        &code,
        format!(
            "/// The Delve's browser page: its `index.html`, then every other file.\npub(crate) static BUNDLE: Option<(&str, &[(&str, &[u8])])> = Some((include_str!({:?}), &[\n{entries}]));\n",
            index_path.display().to_string()
        ),
    )
    .expect("write delve_web.rs");
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
