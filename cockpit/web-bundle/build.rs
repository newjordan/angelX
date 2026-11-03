//! Build and losslessly embed the Delve browser page independently of host edits.
//! Normal builds retain every asset. Nested Cargo, remaps, profile selection and
//! explicit development-only skip policy are shared with the cockpit.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../build_fast.rs"]
mod build_fast;
#[path = "../bundle_bytes.rs"]
mod bundle_bytes;
#[path = "../../crates/web/rules_js.rs"]
mod rules_js;
mod pack_asset;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=WASM_BINDGEN");
    println!("cargo:rerun-if-env-changed=ANGEL_DEV_SKIP_WEB_BUNDLE");
    println!("cargo:rerun-if-env-changed=ANGEL_DELVE_WEB");
    println!("cargo:rerun-if-env-changed=ANGEL_PORTABLE_BUILD");
    let mut inputs = Vec::new();
    delve_web(&mut inputs);
    for path in build_fast::minimal_watch_paths(inputs) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
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

fn write_if_changed(path: &Path, bytes: &[u8]) {
    if fs::read(path).is_ok_and(|old| old == bytes) {
        return;
    }
    fs::write(path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn empty_web_bundle(code: &Path) {
    write_if_changed(
        code,
        "/// No browser page in this build.\n#[inline(never)]\npub fn embedded() -> Option<(&'static str, &'static [(&'static str, &'static [u8])])> { None }\n\
         static PACKED: &[(&str, usize)] = &[];\n\
         static SOUNDS: &str = \"\";\nstatic BESIDE: &[(&str, &str)] = &[];\n".as_bytes(),
    );
}

/// Build the Delve's browser bundle and write `delve_web.rs`, which embeds
/// it: the page, the shell, the wasm and the sounds, as
/// `crates/web/build.sh web/delve delve` lays them out. The music is not
/// embedded: `sounds.list` marks its folder `beside`, and the host reads it
/// from disk (`together_web::bundle`).
fn delve_web(inputs: &mut Vec<PathBuf>) {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .parent().expect("web-bundle sits in the cockpit").to_path_buf();
    let crates = manifest.parent().expect("the cockpit sits in the repository").join("crates");
    let web = crates.join("web");
    let game = web.join(DELVE);
    let code = out.join("delve_web.rs");
    let requested = env::var("ANGEL_DEV_SKIP_WEB_BUNDLE").ok();
    let lane_skip = env::var("ANGEL_DELVE_WEB").ok().as_deref() == Some("0");
    let skip = build_fast::skip_web_bundle(
        if lane_skip { Some("1") } else { requested.as_deref() },
        &env::var("PROFILE").unwrap_or_default(),
        &env::var("OUT_DIR").unwrap_or_default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    if skip {
        empty_web_bundle(&code);
        return;
    }
    // A missing directory must not be watched: cargo treats that as changed
    // on every build and would rerun this script (and its nested wasm build).
    if build_fast::should_watch(&game) {
        inputs.push(game.clone());
    }
    if !game.join("Cargo.toml").is_file() {
        empty_web_bundle(&code);
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
        watch(&dir, inputs);
    }
    for file in ["Cargo.toml", "Cargo.lock", "delve/Cargo.toml", "net/Cargo.toml", "web/Cargo.toml", "stage/Cargo.toml", "gpu/Cargo.toml"] {
        let path = crates.join(file);
        if build_fast::should_watch(&path) {
            inputs.push(path);
        }
    }
    // Blaze's RUSTC is a build-script wrapper whose protocol needs private
    // environment state. The isolated WASM Cargo below deliberately clears
    // that state, so use the actual pinned toolchain, never the wrapper there.
    println!("cargo:rerun-if-env-changed=RA_JIT_SCRIPT_RUSTC");
    println!("cargo:rerun-if-env-changed=RUSTUP_TOOLCHAIN");
    // Blaze can wrap an already wrapped script and overwrite its own real-
    // compiler hint with the alias. The launcher supplies an absolute pinned
    // toolchain; prefer that executable before querying either script alias.
    let rustc = env::var_os("RUSTUP_TOOLCHAIN")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| path.join("bin").join(format!("rustc{}", env::consts::EXE_SUFFIX)))
        .filter(|path| path.is_file())
        .map(|path| path.into_os_string())
        .or_else(|| env::var_os("RA_JIT_SCRIPT_RUSTC"))
        .or_else(|| env::var_os("RUSTC"))
        .unwrap_or_else(|| "rustc".into());
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
    // The nested wasm build's target directory is a gigabyte of fingerprints,
    // `.d` files and cache, none of which is part of what this script produces.
    // It sits beside OUT_DIR, not in it: a daemon that keeps this run re-runs
    // the script in a sandbox and hashes OUT_DIR to decide whether the run can
    // be trusted (ra-jit/blaze), and a target directory re-created by a second
    // run never hashes the same.
    let target = out
        .parent()
        .expect("OUT_DIR sits in a package's build directory")
        .join("delve-web-target");
    let profile = build_fast::nested_web_profile(
        &env::var("PROFILE").unwrap_or_default(),
        &env::var("OUT_DIR").unwrap_or_default(),
    );
    let mut cargo = wasm_cargo(&sysroot);
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
    watch_nested_inputs(&wasm.with_extension("d"), &crates, &target, inputs);
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
        watch(path, inputs);
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
    let repo = manifest.parent().expect("the cockpit sits in the repository");
    let mut folders = vec![(String::new(), game.join("sounds"))];
    let mut beside = Vec::new();
    let list = game.join("sounds.list");
    if list.is_file() {
        inputs.push(list.clone());
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
        inputs.push(dir.clone());
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
    let renderers = renderers(&crates, &sysroot, &target, &out.join("delve-web-render"), &mut files, inputs);
    let title = format!("{}{}", DELVE[..1].to_ascii_uppercase(), &DELVE[1..]);
    let index = fs::read_to_string(web.join("shell").join("index.html"))
        .expect("the shell's page")
        .replace("@@GAME@@", DELVE)
        .replace("@@TITLE@@", &title)
        .replace("@@RENDERERS@@", &renderers);
    let index_path = dist.join("index.html");
    fs::write(&index_path, index).expect("write the bundle's page");
    // Keep every byte of the same page without carrying uncompressed wasm in
    // the native image. Decoding happens only when a browser door is opened.
    let packed_dir = out.join("delve-web-packed");
    fs::create_dir_all(&packed_dir).expect("make packed bundle folder");
    let mut packed_entries = Vec::new();
    let mut entries = String::new();
    // A file under OUT_DIR is named through `env!("OUT_DIR")`, not by its
    // absolute path. A daemon that keeps this run re-runs the script in a
    // sandbox and compares what it wrote (ra-jit/blaze): a generated file
    // holding the absolute name of its own OUT_DIR differs on every such
    // run, and the package would fall back to plain cargo forever. Files
    // outside OUT_DIR (the sounds in the repository) keep their real path.
    let literal = |path: &Path| match path.strip_prefix(&out) {
        Ok(below) => format!(
            "concat!(env!(\"OUT_DIR\"), \"/{}\")",
            below.to_string_lossy().replace('\\', "/")
        ),
        Err(_) => format!("{:?}", path.display().to_string()),
    };
    for (index, (name, path)) in files.iter().enumerate() {
        let plain = fs::read(path).expect("read browser asset");
        let packed_path = packed_dir.join(format!("{index}.zlib"));
        let original_len = pack_asset::pack_asset(&plain, &packed_path)
            .expect("pack and verify browser asset");
        let stored_path = if let Some(original_len) = original_len {
            packed_entries.push(format!("({name:?}, {original_len})"));
            packed_path
        } else {
            path.clone()
        };
        entries.push_str(&format!("    ({name:?}, include_bytes!({})),\n", literal(&stored_path)));
    }
    write_if_changed(
        &code,
        format!(
            "/// The Delve's browser page: its `index.html`, then every other file.\n\
             #[inline(never)]\n\
             pub fn embedded() -> Option<(&'static str, &'static [(&'static str, &'static [u8])])> {{\n\
                 Some((include_str!({}), &[\n{entries}]))\n\
             }}\n\
             /// Lossless packed assets: (name, exact decoded byte length).\n\
             static PACKED: &[(&str, usize)] = &[{}];\n\
             /// The page's sounds manifest without its braces, for the embedded sounds.\n\
             static SOUNDS: &str = {:?};\n\
             /// Sound folders that ship beside the binary: (name prefix, folder in the repository).\n\
             static BESIDE: &[(&str, &str)] = &[{}];\n",
            literal(&index_path),
            packed_entries.join(", "),
            sounds.join(","),
            beside.join(", ")
        ).as_bytes(),
    );
}

/// A cargo for the browser's wasm: a clean environment, none of this build's
/// flags, profile or target directory reach it, only where the toolchain and
/// the crates' cache are (and so it never waits on the outer build's lock).
fn wasm_cargo(sysroot: &Path) -> Command {
    let mut cargo = build_fast::wasm_cargo(sysroot);
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
fn renderers(crates: &Path, sysroot: &Path, target: &Path, work: &Path, files: &mut Vec<(String, PathBuf)>, inputs: &mut Vec<PathBuf>) -> String {
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
        let mut cargo = wasm_cargo(sysroot);
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
        watch_nested_inputs(&wasm.with_extension("d"), crates, target, inputs);
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

/// Cargo already watches directories recursively. Missing paths would make
/// every build dirty, so only register existing inputs.
fn watch(path: &Path, inputs: &mut Vec<PathBuf>) {
    if build_fast::should_watch(path) {
        inputs.push(path.to_path_buf());
    }
}

/// Use nested Cargo's real input inventory rather than guessing which sprite
/// registries, generated art slots or build scripts the browser consumed.
/// Exclude nested outputs: watching those makes every launch dirty.
fn watch_nested_inputs(dep_file: &Path, workspace: &Path, target: &Path, inputs: &mut Vec<PathBuf>) {
    let info = fs::read_to_string(dep_file).expect("nested Cargo writes aggregate dep-info");
    let target = target.canonicalize().expect("nested target exists");
    for input in build_fast::nested_dependency_paths(&info).expect("valid nested Cargo dep-info") {
        let source = workspace.join(input);
        let path = source.canonicalize().unwrap_or_else(|error| {
            panic!("nested Cargo dep-info input {} from {} cannot be resolved: {error}; \
                the input may have been removed during the build or contain a tab/newline \
                that Cargo's aggregate dep-info cannot represent; refusing to omit a freshness input",
                source.display(), dep_file.display())
        });
        if !path.starts_with(&target) {
            watch(&path, inputs);
        }
    }
}
