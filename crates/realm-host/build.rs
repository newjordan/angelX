//! Build the Delve's browser page into the realm host.
//!
//! The Delve's page is the angel-web shell with the Delve's wasm client
//! (`crates/web/delve`), built for `wasm32-unknown-unknown` by a nested
//! cargo with its own target directory and a clean environment, and
//! embedded: a page and its host always match (`docs/ENGINE.md`). This script
//! used to live in the cockpit's; it moved here so an edit to the Delve or to
//! the browser client reruns only this crate's build, not the cockpit's.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "build_fast.rs"]
mod build_fast;
#[path = "../web/rules_js.rs"]
mod rules_js;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_fast.rs");
    println!("cargo:rerun-if-env-changed=WASM_BINDGEN");
    println!("cargo:rerun-if-env-changed=ANGEL_PORTABLE_BUILD");
    delve_web();
}

/// The `--remap-path-prefix` flags among this build's rustc flags.
fn remap_flags(encoded: &str) -> Vec<String> {
    let mut flags = Vec::new();
    let mut parts = encoded.split('\x1f').filter(|part| !part.is_empty());
    while let Some(part) = parts.next() {
        if part == "--remap-path-prefix" {
            if let Some(map) = parts.next() {
                flags.push(format!("--remap-path-prefix={map}"));
            }
        } else if part.starts_with("--remap-path-prefix=") {
            flags.push(part.to_string());
        }
    }
    flags
}

/// The game's name in the bundle, as `crates/web/build.sh` names it.
const DELVE: &str = "delve";

/// Build the Delve's browser bundle and write `delve_web.rs`, which embeds
/// it: the page, the shell, the wasm and the sounds, as
/// `crates/web/build.sh web/delve delve` lays them out. The music is not
/// embedded: `sounds.list` marks its folder `beside`, and the host reads it
/// from disk (`together_web::bundle`).
fn delve_web() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let crates = manifest.parent().expect("the realm host sits in the crates folder").to_path_buf();
    let web = crates.join("web");
    let game = web.join(DELVE);
    let code = out.join("delve_web.rs");
    // A missing directory must not be watched: cargo treats that as changed
    // on every build and would rerun this script (and its nested wasm build).
    if build_fast::should_watch(&game) {
        println!("cargo:rerun-if-changed={}", game.display());
    }
    if !game.join("Cargo.toml").is_file() {
        fs::write(
            &code,
            "/// No browser page in this build.\npub static BUNDLE: Option<(&str, &[(&str, &[u8])])> = None;\n\
             pub static SOUNDS: &str = \"\";\npub static BESIDE: &[(&str, &str)] = &[];\n",
        )
            .expect("write delve_web.rs");
        return;
    }
    for dir in [
        web.join("shell"),
        web.join("src"),
        game.clone(),
        crates.join("delve").join("src"),
        crates.join("net").join("src"),
        crates.join("stage").join("src"),
        web.join("render"),
        crates.join("gpu"),
    ] {
        watch(&dir);
    }
    for file in ["Cargo.toml", "Cargo.lock", "delve/Cargo.toml", "net/Cargo.toml", "web/Cargo.toml", "stage/Cargo.toml", "gpu/Cargo.toml"] {
        let path = crates.join(file);
        if build_fast::should_watch(&path) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
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
    let profile = build_fast::nested_web_profile(
        &env::var("PROFILE").unwrap_or_default(),
        &env::var("OUT_DIR").unwrap_or_default(),
    );
    let mut cargo = wasm_cargo(&rustc);
    cargo
        .arg("build")
        .arg("--manifest-path")
        .arg(crates.join("Cargo.toml"))
        .args(["-p", &package, "--profile", profile, "--target", "wasm32-unknown-unknown", "--locked", "--target-dir"])
        .arg(&target);
    let status = cargo.status().expect("could not run cargo for the Delve's browser page");
    if !status.success() {
        panic!("the Delve's browser page did not build (cargo build -p {package} --profile {profile} --target wasm32-unknown-unknown in crates/)");
    }
    let lib = package.replace('-', "_");
    let wasm = target.join("wasm32-unknown-unknown").join(profile).join(format!("{lib}.wasm"));
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
        // The shell's scripts, its styles (shell.css and the frames' skin.css)
        // and its pixel hand (knight.ttf), as `crates/web/build.sh` takes them.
        .filter(|p| p.extension().is_some_and(|e| e == "js" || e == "css" || e == "ttf"))
        .collect();
    shell.sort();
    for path in shell {
        copy(path.file_name().expect("a file").to_string_lossy().into_owned(), &path);
    }
    // The page's rules modules (`rules.js`): the Bend modules the game's
    // `rules.list` names, run by the page as the host runs them in V8, as
    // `crates/web/build.sh` writes them.
    let (rules, read) = rules_js::rules_js(&game);
    for path in &read {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let rules_path = dist.join("rules.js");
    fs::write(&rules_path, rules).expect("write the bundle's rules modules");
    files.push(("rules.js".to_string(), rules_path));
    // The sounds manifest: {"name": "{{asset:name.ext}}", ...}, inline in
    // the page so the server's asset names reach it. The sounds are the
    // crate's own `sounds/` and the folders its `sounds.list` names (a name
    // prefix, "-" for none, then a folder), as `crates/web/build.sh` takes them.
    // A folder marked `beside` stays out of the binary: only its place in the
    // repository is written down (`BESIDE`), and the host reads it from disk
    // and adds its files to the manifest when it opens its door.
    let repo = crates.parent().expect("the crates folder sits in the repository");
    let mut folders = vec![(String::new(), game.join("sounds"))];
    let mut beside = Vec::new();
    let list = game.join("sounds.list");
    if list.is_file() {
        println!("cargo:rerun-if-changed={}", list.display());
    }
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
            let prefix = if prefix == "-" { String::new() } else { prefix.to_string() };
            if words.next() == Some("beside") {
                let canonical = |p: &Path| fs::canonicalize(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                let place = canonical(&dir)
                    .strip_prefix(canonical(repo))
                    .unwrap_or_else(|_| panic!("{}: {folder} is outside the repository", list.display()))
                    .to_string_lossy()
                    .replace('\\', "/");
                beside.push(format!("({prefix:?}, {place:?})"));
                continue;
            }
            folders.push((prefix, dir));
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
    let renderers = renderers(&crates, &rustc, &target, &out.join("delve-web-render"), &mut files);
    let title = format!("{}{}", DELVE[..1].to_ascii_uppercase(), &DELVE[1..]);
    let index = fs::read_to_string(web.join("shell").join("index.html"))
        .expect("the shell's page")
        .replace("@@GAME@@", DELVE)
        .replace("@@TITLE@@", &title)
        .replace("@@RENDERERS@@", &renderers);
    let index_path = dist.join("index.html");
    fs::write(&index_path, index).expect("write the bundle's page");
    let entries: String = files
        .iter()
        .map(|(name, path)| format!("    ({name:?}, include_bytes!({:?})),\n", path.display().to_string()))
        .collect();
    fs::write(
        &code,
        format!(
            "/// The Delve's browser page: its `index.html`, then every other file.\n\
             pub static BUNDLE: Option<(&str, &[(&str, &[u8])])> = Some((include_str!({:?}), &[\n{entries}]));\n\
             /// The page's sounds manifest without its braces, for the sounds in `BUNDLE`.\n\
             pub static SOUNDS: &str = {:?};\n\
             /// Sound folders that ship beside the binary: (name prefix, folder in the repository).\n\
             pub static BESIDE: &[(&str, &str)] = &[{}];\n",
            index_path.display().to_string(),
            sounds.join(","),
            beside.join(", ")
        ),
    )
    .expect("write delve_web.rs");
}

/// A cargo for the browser's wasm: a clean environment, none of this build's
/// flags, profile or target directory reach it, only where the toolchain and
/// the crates' cache are (and so it never waits on the outer build's lock).
fn wasm_cargo(rustc: &std::ffi::OsStr) -> Command {
    let mut cargo = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo.env_clear();
    for key in ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "TMPDIR", "TEMP", "TMP", "SystemRoot", "USERPROFILE"] {
        if let Some(value) = env::var_os(key) {
            cargo.env(key, value);
        }
    }
    cargo.env("RUSTC", rustc);
    // A portable build is portable in the page too: the Delve names no build
    // folder there either (`crates/delve/build.rs`).
    if let Some(value) = env::var_os("ANGEL_PORTABLE_BUILD") {
        cargo.env("ANGEL_PORTABLE_BUILD", value);
    }
    // Path remaps are the one flag the page shares: a portable build maps the
    // builder's folders out of the wasm too.
    let remaps = remap_flags(&env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default());
    if !remaps.is_empty() {
        cargo.env("CARGO_ENCODED_RUSTFLAGS", remaps.join("\x1f"));
    }
    cargo
}

/// The GPU renderer (`crates/web/render`), as `crates/web/build.sh` builds it:
/// twice, WebGPU only and with wgpu's WebGL2 backend too, each through the
/// wasm-bindgen CLI into `render-<build>.js` and `render-<build>_bg.wasm`,
/// which join `files` (served as /a/<hash>.<ext>). Returns the page's
/// `renderers` JSON. The CLI is `$WASM_BINDGEN` or `wasm-bindgen` on PATH and
/// must be the version the crate pins; without it, or at another, the bundle
/// has no renderer (`{}`), with a warning, and the page draws with the shell's
/// own presenter as it did before.
fn renderers(crates: &Path, rustc: &std::ffi::OsStr, target: &Path, work: &Path, files: &mut Vec<(String, PathBuf)>) -> String {
    let pinned = fs::read_to_string(crates.join("web").join("render").join("Cargo.toml"))
        .ok()
        .and_then(|toml| {
            toml.lines().find_map(|line| {
                let rest = line.trim_start().strip_prefix("wasm-bindgen")?.trim_start().strip_prefix('=')?;
                Some(rest.trim().trim_matches('"').trim_start_matches('=').to_string())
            })
        });
    let Some(pinned) = pinned else {
        println!("cargo:warning=crates/web/render/Cargo.toml pins no wasm-bindgen: the browser page has no GPU renderer");
        return "{}".into();
    };
    let bindgen = env::var_os("WASM_BINDGEN").filter(|v| !v.is_empty()).unwrap_or_else(|| "wasm-bindgen".into());
    let version = Command::new(&bindgen)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().nth(1).map(str::to_string));
    if version.as_deref() != Some(pinned.as_str()) {
        println!(
            "cargo:warning=no wasm-bindgen {pinned} (set WASM_BINDGEN; {}): the browser page has no GPU renderer and draws with its own presenter",
            version.map_or("none found".to_string(), |v| format!("found {v}"))
        );
        return "{}".into();
    }
    let _ = fs::remove_dir_all(work);
    fs::create_dir_all(work).expect("make the renderer's folder");
    let mut entries = Vec::new();
    for build in ["webgpu", "webgl"] {
        let profile = build_fast::nested_web_profile(
            &env::var("PROFILE").unwrap_or_default(),
            &env::var("OUT_DIR").unwrap_or_default(),
        );
        let mut cargo = wasm_cargo(rustc);
        cargo
            .arg("build")
            .arg("--manifest-path")
            .arg(crates.join("Cargo.toml"))
            .args(["-p", "angel-web-render", "--profile", profile, "--target", "wasm32-unknown-unknown", "--features", build, "--locked", "--target-dir"])
            .arg(target);
        let status = cargo.status().expect("could not run cargo for the browser renderer");
        if !status.success() {
            panic!("the browser renderer did not build (cargo build -p angel-web-render --profile {profile} --target wasm32-unknown-unknown --features {build} in crates/)");
        }
        let wasm = target.join("wasm32-unknown-unknown").join(profile).join("angel_web_render.wasm");
        let status = Command::new(&bindgen)
            .args(["--target", "web", "--no-typescript", "--out-dir"])
            .arg(work)
            .args(["--out-name", &format!("render-{build}")])
            .arg(&wasm)
            .status()
            .expect("could not run wasm-bindgen");
        if !status.success() {
            panic!("wasm-bindgen did not make the {build} renderer's glue");
        }
        for name in [format!("render-{build}.js"), format!("render-{build}_bg.wasm")] {
            files.push((name.clone(), work.join(name)));
        }
        entries.push(format!(
            "\"{build}\":{{\"js\":\"{{{{asset:render-{build}.js}}}}\",\"wasm\":\"{{{{asset:render-{build}_bg.wasm}}}}\"}}"
        ));
    }
    format!("{{{}}}", entries.join(","))
}

/// Rebuild when any file under `dir` changes (cargo watches a folder only by
/// its own mtime). A path that is not there is not watched: cargo counts a
/// missing `rerun-if-changed` as changed and would rerun this script on every
/// build.
fn watch(dir: &Path) {
    if !dir.exists() {
        return;
    }
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
