//! Embed the shipped policy generations (feature `v8`). Every `kind: policy`
//! module of `cockpit/live` is captured by the same rules as the live loader
//! (`bundle.rs` `visit`): recursive `*.js`/`*.bend`/`*.json`, including the
//! module and Bend manifests, with dotfiles skipped.
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Read;
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
        println!("cargo:rerun-if-changed={}", dir.display());
        let id = dir.file_name().and_then(|n| n.to_str()).expect("module id");
        println!("cargo:rerun-if-changed={}", dir.join("module.json").display());
        let files = capture_policy_sources(&dir)
            .unwrap_or_else(|error| panic!("shipped policy {id}: {error}"));
        let Some(files) = files else {
            continue;
        };
        table.push_str(&format!("    ({id:?}, &[\n"));
        for (name, (path, body)) in files {
            println!("cargo:rerun-if-changed={}", path.display());
            table.push_str(&format!("        ({name:?}, {body:?}),\n"));
        }
        table.push_str("    ]),\n");
    }
    table.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("shipped_legs.rs");
    fs::write(out, table).expect("write shipped_legs.rs");
}

/// Select only from two matching manifest snapshots, then require the full
/// captured policy generation to contain those same manifest bytes. Non-policy
/// modules stop here and do not inherit the shipped-policy capture budget.
pub(crate) fn capture_policy_sources(
    root: &Path,
) -> Result<Option<BTreeMap<String, (PathBuf, String)>>, String> {
    let manifest_path = root.join("module.json");
    let first = fs::read_to_string(&manifest_path).map_err(|error| error.to_string())?;
    let second = fs::read_to_string(&manifest_path).map_err(|error| error.to_string())?;
    let Some(selected_manifest) = stable_policy_manifest(&first, &second)? else {
        return Ok(None);
    };
    let files = capture_sources(root)?;
    if !captured_manifest_matches(&selected_manifest, &files) {
        return Err("module changed while its generation was being captured".into());
    }
    Ok(Some(files))
}

pub(crate) fn stable_policy_manifest(first: &str, second: &str) -> Result<Option<String>, String> {
    if first != second {
        return Err("module manifest changed while its policy status was being selected".into());
    }
    let compact: String = first.chars().filter(|c| !c.is_whitespace()).collect();
    Ok(compact
        .contains("\"kind\":\"policy\"")
        .then(|| first.to_owned()))
}

pub(crate) fn captured_manifest_matches(
    selected: &str,
    files: &BTreeMap<String, (PathBuf, String)>,
) -> bool {
    files
        .get("module.json")
        .is_some_and(|(_, captured)| captured == selected)
}

// Keep these limits and traversal rules aligned with bundle::{inputs, visit, capture}.
const MAX_FILES: usize = 64;
const MAX_BYTES: usize = 1024 * 1024;

#[cfg(test)]
pub(crate) fn capture_paths(root: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    Ok(capture_sources(root)?
        .into_iter()
        .map(|(name, (path, _))| (name, path))
        .collect())
}

fn capture_sources(root: &Path) -> Result<BTreeMap<String, (PathBuf, String)>, String> {
    let first = capture_once(root)?;
    if first != capture_once(root)? {
        return Err("module changed while its generation was being captured".into());
    }
    Ok(first)
}

fn capture_once(root: &Path) -> Result<BTreeMap<String, (PathBuf, String)>, String> {
    if !fs::symlink_metadata(root)
        .map_err(|e| e.to_string())?
        .is_dir()
    {
        return Err("module must be a directory".into());
    }
    let mut files = BTreeMap::new();
    visit(root, root, 0, &mut files, &mut 0)?;
    let mut sources = BTreeMap::new();
    let mut bytes = 0;
    for (name, path) in files {
        let mut body = String::new();
        fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take((MAX_BYTES.saturating_sub(bytes) + 1) as u64)
            .read_to_string(&mut body)
            .map_err(|e| e.to_string())?;
        bytes += body.len();
        if bytes > MAX_BYTES {
            return Err(format!(
                "module generation exceeds {} KiB",
                MAX_BYTES / 1024
            ));
        }
        sources.insert(name, (path, body));
    }
    Ok(sources)
}

fn visit(
    root: &Path,
    dir: &Path,
    depth: usize,
    files: &mut BTreeMap<String, PathBuf>,
    entries: &mut usize,
) -> Result<(), String> {
    if depth > 8 {
        return Err("module dependency directory is too deep".into());
    }
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        *entries += 1;
        if *entries > 256 {
            return Err("module dependency tree exceeds 256 entries".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if kind.is_dir() {
            visit(root, &path, depth + 1, files, entries)?;
        } else if path
            .extension()
            .is_some_and(|v| v == "js" || v == "bend" || v == "json")
            || path == root.join("module.json")
            || path == root.join("bend-build.json")
        {
            // Register even rejected inputs so repairing an existing file reruns capture.
            println!("cargo:rerun-if-changed={}", path.display());
            if !kind.is_file() {
                return Err("module inputs must be regular files".into());
            }
            if files.len() >= MAX_FILES {
                return Err("too many module files".into());
            }
            let name = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("module path is not UTF-8")?
                .replace('\\', "/");
            files.insert(name, path);
        }
    }
    Ok(())
}
