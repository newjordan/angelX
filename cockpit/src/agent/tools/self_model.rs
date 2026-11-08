//! Self-model — the cockpit's **root understanding of its own source tree**.
//!
//! Self-work uses the active checkout's source, including linked worktrees.
//! Unrelated projects do not receive this map at startup. Explicit `self_map`
//! diagnostics remain available read-only without changing the working project.
//!
//! It also hosts the **self-modification safety gate** ([`evaluate_gate`] /
//! [`run_self_gate`]): a self-edit is only acceptable if the crate still builds
//! **and** its test suite is green — the reusable commit/integrate condition for
//! Layer 2 (point the edit tools at the cockpit's own crate, gated by this).

use crate::agent::club::ToolDef;
use crate::agent::harness::{Tool, env_flag};
use crate::agent::tools::build::{TestOutcome, parse_test_result};
use serde_json::Value;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Locating our own source.
// ---------------------------------------------------------------------------

/// Locate the cockpit crate's source root (the dir holding `Cargo.toml` + `src/`).
///
/// Resolution order, most-explicit first:
///   1. `ANGEL_SELF_SRC` if it points at a crate root.
///   2. The shipped resource bundle or relocated source checkout.
///   3. Walk up from the current dir looking for `angelX-cockpit`'s `Cargo.toml`.
///
/// `None` only when the source genuinely can't be found (e.g. a relocated binary
/// with no tree nearby) — self-understanding of source is moot in that case.
pub fn source_root() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ANGEL_SELF_SRC") {
        let p = PathBuf::from(p);
        if is_cockpit_root(&p) {
            return Some(p);
        }
    }
    let shipped = crate::platform::runtime_paths::cockpit_dir();
    if is_cockpit_root(&shipped) {
        return Some(shipped);
    }
    let mut dir = std::env::current_dir().ok();
    while let Some(d) = dir {
        if is_cockpit_root(&d) {
            return Some(d);
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    None
}

/// A dir is the cockpit crate root if it has a `src/main.rs` and a `Cargo.toml`
/// declaring `name = "angelX-cockpit"` (so we don't latch onto a sibling crate).
fn is_cockpit_root(dir: &Path) -> bool {
    if !dir.join("src/main.rs").is_file() {
        return false;
    }
    match std::fs::read_to_string(dir.join("Cargo.toml")) {
        Ok(toml) => toml.contains("name = \"angelX-cockpit\""),
        Err(_) => false,
    }
}

// The projection is hotloaded JavaScript. These focused test helpers exercise
// the same module used by the application rather than retaining a Rust copy.
#[cfg(test)]
fn module_purpose(src: &str) -> String {
    test_projection(serde_json::json!({"op":"analyze", "src":src}))["purpose"]
        .as_str()
        .unwrap()
        .to_string()
}

#[cfg(test)]
struct ModuleSymbols {
    count: usize,
    key_types: Vec<String>,
}

#[cfg(test)]
fn module_symbols(src: &str) -> ModuleSymbols {
    let value = test_projection(serde_json::json!({"op":"analyze", "src":src}));
    ModuleSymbols {
        count: value["count"].as_u64().unwrap() as usize,
        key_types: value["keyTypes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect(),
    }
}

#[cfg(test)]
fn pub_type_name(line: &str) -> Option<String> {
    test_projection(serde_json::json!({"op":"publicType", "line":line}))["name"]
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
fn group_of(stem: &str) -> String {
    test_projection(serde_json::json!({"op":"group", "stem":stem}))["group"]
        .as_str()
        .unwrap()
        .to_string()
}

#[cfg(test)]
fn test_projection(input: Value) -> Value {
    let _guard = crate::tests::env_lock();
    self_map_worker(&self_map_module().unwrap(), &input).unwrap()
}

// ---------------------------------------------------------------------------
// Crate metadata (a tiny line scanner — no toml dependency).
// ---------------------------------------------------------------------------

struct CrateMeta {
    name: String,
    version: String,
    edition: String,
    bin: String,
}

/// Pull `name`/`version`/`edition` (from `[package]`) and the bin name (from
/// `[[bin]]`) out of a Cargo.toml without a TOML parser. Best-effort; missing
/// keys fall back to sensible defaults.
fn parse_crate_meta(toml: &str) -> CrateMeta {
    let mut section = "";
    let mut name = String::new();
    let mut version = String::new();
    let mut edition = String::new();
    let mut bin = String::new();
    for raw in toml.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            section = if line.starts_with("[[bin]]") {
                "bin"
            } else if line.starts_with("[package]") {
                "package"
            } else {
                "other"
            };
            continue;
        }
        let val = |line: &str| -> String {
            line.split_once('=')
                .map(|(_, v)| v.trim().trim_matches('"').to_string())
                .unwrap_or_default()
        };
        match section {
            "package" => {
                if line.starts_with("name") {
                    name = val(line);
                } else if line.starts_with("version") {
                    version = val(line);
                } else if line.starts_with("edition") {
                    edition = val(line);
                }
            }
            "bin" if line.starts_with("name") => bin = val(line),
            _ => {}
        }
    }
    CrateMeta {
        name: if name.is_empty() {
            "angelX-cockpit".into()
        } else {
            name
        },
        version,
        edition,
        bin: if bin.is_empty() { "angel".into() } else { bin },
    }
}

// The host owns discovery and input bounds; the module owns presentation.
#[derive(serde::Serialize)]
struct ModuleInput {
    stem: String,
    rel: String,
    src: String,
}

#[derive(Clone)]
struct SelfContextCache {
    root: PathBuf,
    source_stamp: u64,
    rendered: String,
}

static SELF_CONTEXT_CACHE: OnceLock<Mutex<Option<SelfContextCache>>> = OnceLock::new();

/// One computation at a time. The startup prefetch ([`prefetch_context`]), the
/// bootstrap call and a resume or `/cd` refresh all want the same render; the
/// later callers wait for the first and then read its result instead of each
/// spawning a worker over the whole source tree.
static SELF_CONTEXT_COMPUTE: Mutex<()> = Mutex::new(());

const SELF_MAP_CACHE_SCHEMA: &str = "angel-self-map-cache/1";
const SELF_MAP_CACHE_LIMIT: u64 = 2 * 1024 * 1024;

/// Where worker reports persist between launches. `ANGEL_SELF_MAP_CACHE` names
/// a directory, or `0` turns the persistent cache off; unit tests use none
/// unless they name one, so they never write into the real home.
fn self_map_cache_dir() -> Option<PathBuf> {
    match std::env::var_os("ANGEL_SELF_MAP_CACHE") {
        Some(value) if value == "0" => None,
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ if cfg!(test) => None,
        _ => Some(crate::platform::workspace_store::angel_subdir("self-map")),
    }
}

/// The worker's last report for `root` when it was rendered from exactly these
/// sources and rules (`stamp` covers both, and is what the in-process cache is
/// keyed on). Only the report is kept on disk: the guidance pages are compiled
/// into this binary and are assembled around it on every launch.
fn load_cached_report(root: &Path, stamp: u64) -> Option<Value> {
    let path = crate::platform::workspace_store::workspace_json_path_in(
        &self_map_cache_dir()?,
        root,
    );
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > SELF_MAP_CACHE_LIMIT {
        return None;
    }
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(file, SELF_MAP_CACHE_LIMIT), &mut bytes)
        .ok()?;
    let entry: Value = serde_json::from_slice(&bytes).ok()?;
    if entry.get("schema").and_then(Value::as_str) != Some(SELF_MAP_CACHE_SCHEMA)
        || entry.get("root").and_then(Value::as_str) != root.to_str()
        || entry.get("stamp").and_then(Value::as_str) != Some(format!("{stamp:016x}").as_str())
    {
        return None;
    }
    let report = entry.get("report")?.clone();
    // The same shape checks a fresh worker response passes in `render_modules`.
    let valid = report.get("version").and_then(Value::as_u64) == Some(1)
        && ["full", "compact", "tools", "toolNames"]
            .iter()
            .all(|key| report.get(*key).and_then(Value::as_str).is_some())
        && ["hasTop", "hasTools"]
            .iter()
            .all(|key| report.get(*key).and_then(Value::as_bool).is_some());
    valid.then_some(report)
}

/// Best effort: a cache that cannot be written only costs the next launch a render.
fn store_cached_report(root: &Path, stamp: u64, report: &Value) {
    let Some(dir) = self_map_cache_dir() else {
        return;
    };
    let Some(root_text) = root.to_str() else {
        return;
    };
    let entry = serde_json::json!({
        "schema": SELF_MAP_CACHE_SCHEMA,
        "root": root_text,
        "stamp": format!("{stamp:016x}"),
        "report": report,
    });
    let Ok(bytes) = serde_json::to_vec(&entry) else {
        return;
    };
    if bytes.len() as u64 > SELF_MAP_CACHE_LIMIT {
        return;
    }
    let path = crate::platform::workspace_store::workspace_json_path_in(&dir, root);
    let _ = crate::platform::workspace_store::write_atomic(&path, &bytes);
}

/// Start rendering the self-map for `workspace` on its own thread so a launch in
/// an angelX checkout overlaps the render with terminal and route setup. The
/// bootstrap call waits on [`SELF_CONTEXT_COMPUTE`] and reads the result; the
/// output is exactly what it would have computed itself.
pub fn prefetch_context(workspace: &Path) {
    if !env_flag("ANGEL_SELF_MODEL", true) {
        return;
    }
    let workspace = workspace.to_path_buf();
    // A launch without the thread just renders in the bootstrap call.
    let _ = std::thread::Builder::new()
        .name("angel-self-map-warm".into())
        .spawn(move || {
            let _ = self_context(&workspace);
        });
}

/// The seven dependency layers `src/` is grouped into. They are directories,
/// not modules in their own right, so every scan reaches *through* them: the
/// stems one level down are the modules this map has always described, which
/// is what keeps the report keyed on bare stems.
const LAYERS: &[&str] = &[
    "agent",
    "app",
    "drive",
    "knowledge",
    "platform",
    "stage",
    "ui",
];

/// The concrete `Tool` impls, relative to the crate root. They render as their
/// own section, so every other scan skips them.
const TOOLS_REL: &str = "src/agent/tools";

/// Every directory the module map reads, as `(dir, rel_prefix, skip)`. One list
/// so [`scan_modules`] and [`module_source_stamp`] cannot drift apart — they
/// held separate copies of this before, and the layering silently broke both.
fn scan_roots(root: &Path) -> Vec<(PathBuf, String, Vec<&'static str>)> {
    let src = root.join("src");
    // Top level keeps only the true root modules; the layer dirs are reached below.
    let mut roots = vec![(src.clone(), "src".to_string(), LAYERS.to_vec())];
    for layer in LAYERS {
        let mut skip = vec!["mod.rs"];
        if *layer == "agent" {
            skip.push("tools");
        }
        roots.push((src.join(layer), format!("src/{layer}"), skip));
    }
    roots.push((root.join(TOOLS_REL), TOOLS_REL.to_string(), vec!["mod.rs"]));
    roots
}

/// Cheap invalidation key for the files [`scan_modules`] reads. Resuming a
/// session should not reread the entire cockpit source tree when none of those
/// files changed, but the injected map must still refresh after a live edit.
fn module_source_stamp(root: &Path) -> u64 {
    let mut paths = Vec::new();
    for (dir, _rel_prefix, skip) in scan_roots(root) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| skip.contains(&name))
            {
                continue;
            }
            let path = if entry.path().is_dir() && entry.path().join("mod.rs").is_file() {
                entry.path().join("mod.rs")
            } else {
                entry.path()
            };
            if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                paths.push(path);
            }
        }
    }
    paths.sort_unstable();
    paths.dedup();

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for path in paths {
        path.strip_prefix(root).unwrap_or(&path).hash(&mut hasher);
        if let Ok(metadata) = std::fs::metadata(&path) {
            metadata.len().hash(&mut hasher);
            metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
                .hash(&mut hasher);
        }
    }
    hasher.finish()
}

const SELF_MAP_SOURCE_LIMIT: u64 = 2 * 1024 * 1024;
const SELF_MAP_TOTAL_LIMIT: usize = 12 * 1024 * 1024;
const SELF_MAP_INPUT_LIMIT: usize = 16 * 1024 * 1024;
const SELF_MAP_OUTPUT_LIMIT: usize = 1024 * 1024;

/// Use the shared confined reader; on Linux it anchors resolution to the source
/// directory descriptor. The worker receives source bytes, not file paths.
fn read_source(path: &Path, root: &Path, total: &mut usize) -> Result<Option<String>, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "source is outside src/")?;
    let bytes = match crate::agent::harness::confined_read_limited(
        root,
        relative,
        SELF_MAP_SOURCE_LIMIT as usize,
    ) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Err("self-map source exceeds its byte limit".into()),
        Err(_) => return Ok(None),
    };
    *total = total.saturating_add(bytes.len());
    if bytes.len() as u64 > SELF_MAP_SOURCE_LIMIT || *total > SELF_MAP_TOTAL_LIMIT {
        return Err("self-map source exceeds its byte limit".into());
    }
    Ok(String::from_utf8(bytes).ok())
}

fn scan_modules(root: &Path) -> Result<(Vec<ModuleInput>, Vec<ModuleInput>), String> {
    let source_root = root.join("src");
    let (mut top, mut tools, mut total) = (Vec::new(), Vec::new(), 0);
    for (dir, rel_prefix, skip) in scan_roots(root) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries {
            let entry = entry.map_err(|e| format!("scan source: {e}"))?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| skip.contains(&name))
            {
                continue;
            }
            let path = if entry.path().is_dir() && entry.path().join("mod.rs").is_file() {
                entry.path().join("mod.rs")
            } else {
                entry.path()
            };
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let stem = if path.file_name().and_then(|s| s.to_str()) == Some("mod.rs") {
                path.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
            } else {
                path.file_stem().and_then(|s| s.to_str())
            };
            let Some(stem) = stem else { continue };
            if top.len() + tools.len() >= 1024 {
                return Err("self-map source count exceeds its limit".into());
            }
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| format!("{rel_prefix}/{stem}.rs"));
            let input = ModuleInput {
                stem: stem.into(),
                rel,
                src: read_source(&path, &source_root, &mut total)?.unwrap_or_default(),
            };
            if rel_prefix == TOOLS_REL {
                tools.push(input)
            } else {
                top.push(input)
            }
        }
    }
    Ok((top, tools))
}

const SELF_MAP_FILES: &[&str] = &["scripts/runtime/self-map.mjs", "lib/self-map/report.mjs"];

/// Pin both the entry point and its analysis rules for one request. Their
/// exact contents also invalidate the compact-context cache on an edit.
struct SelfMapModule {
    snapshot: crate::platform::worker_snapshot::Snapshot,
    fingerprint: u64,
}

fn self_map_module() -> Result<SelfMapModule, String> {
    let root = crate::platform::runtime_paths::root();
    let snapshot =
        crate::platform::worker_snapshot::Snapshot::capture(&root, SELF_MAP_FILES, "self-map")?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for path in SELF_MAP_FILES {
        let bytes = crate::platform::worker_snapshot::read(&snapshot.dir.join(path), "self-map")?;
        if bytes.len() > 64 * 1024 {
            return Err("self-map module exceeds its byte limit".into());
        }
        path.hash(&mut hasher);
        bytes.hash(&mut hasher);
    }
    Ok(SelfMapModule {
        snapshot,
        fingerprint: hasher.finish(),
    })
}

fn self_map_worker(module: &SelfMapModule, request: &Value) -> Result<Value, String> {
    let node = crate::agent::tools::runtime_missing::resolve("node")
        .map_err(|_| "Node.js unavailable".to_string())?;
    let input = serde_json::to_vec(request).map_err(|e| format!("self-map request: {e}"))?;
    let mut command = Command::new(node);
    command
        .arg(module.snapshot.dir.join(SELF_MAP_FILES[0]))
        .env_clear();
    let context = crate::agent::tools::http_transport::context();
    let output = crate::platform::worker::run_cancellable(
        command,
        &input,
        std::time::Duration::from_secs(10),
        SELF_MAP_INPUT_LIMIT,
        SELF_MAP_OUTPUT_LIMIT,
        || context.interrupted(),
    )
    .map_err(|e| format!("self-map module: {e}"))?;
    let result: Value = serde_json::from_slice(&output).map_err(|_| "invalid self-map response")?;
    if result.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("unsupported self-map response".into());
    }
    Ok(result)
}

fn render_modules(root: &Path, module: &SelfMapModule) -> Result<Value, String> {
    let (top, tools) = scan_modules(root)?;
    let has_top = !top.is_empty();
    let has_tools = !tools.is_empty();
    let request = serde_json::json!({"op":"render", "top":top, "tools":tools});
    let result = self_map_worker(module, &request)?;
    for key in ["full", "compact", "tools", "toolNames"] {
        if result.get(key).and_then(Value::as_str).is_none() {
            return Err(format!("invalid self-map {key} response"));
        }
    }
    for key in ["hasTop", "hasTools"] {
        if result.get(key).and_then(Value::as_bool).is_none() {
            return Err(format!("invalid self-map {key} response"));
        }
    }
    if result["hasTop"].as_bool() != Some(has_top)
        || result["hasTools"].as_bool() != Some(has_tools)
    {
        return Err("inconsistent self-map source counts".into());
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Map generation.
// ---------------------------------------------------------------------------

/// The fixed build/test/run reference (the operator + agent contract): its
/// `⠰⠛` pages, the crate root and binary beside them as data.
fn build_test_run(root: &Path, meta: &CrateMeta) -> String {
    use crate::agent::harness::book::d56_replies as replies;
    format!(
        "{} root={root} bin={bin}\n",
        crate::agent::harness::book::d46_recovery::run(&[
            replies::MAP_BUILD,
            replies::MAP_COMMANDS,
            replies::MAP_BINARY,
            replies::MAP_GIT_ROOT,
        ]),
        root = root.display(),
        bin = meta.bin,
    )
}

/// The full, never-stale self-model: crate identity, build/test/run, the grouped
/// module map (purpose + symbol counts + key public types), and the safety note.
#[cfg_attr(not(test), allow(dead_code))]
pub fn generate_self_model() -> String {
    let root = match source_root() {
        Some(r) => r,
        None => {
            return format!(
                "self-model unavailable: could not locate the angelX-cockpit source tree.\n{}",
                crate::agent::harness::book::d56_replies::SELF_SRC_ROOT.cells()
            );
        }
    };
    generate_self_model_at(&root).unwrap_or_else(|error| format!("self-model unavailable: {error}"))
}

fn generate_self_model_at(root: &Path) -> Result<String, String> {
    let toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let meta = parse_crate_meta(&toml);
    let module = self_map_module()?;
    let report = render_modules(root, &module)?;

    use crate::agent::harness::book::d46_recovery::run;
    use crate::agent::harness::book::d56_replies as replies;
    // The map's prose is `⠰⠛` (and the safety contract `⠰⠓`); the crate's
    // identity, root, groups and modules are the data between the pages.
    let mut s = String::new();
    s.push_str(&format!(
        "{} name={name} ver={ver} ed={ed}\n\n",
        run(&[replies::MAP_TITLE, replies::MAP_HARNESS, replies::MAP_LIVE]),
        name = meta.name,
        ver = if meta.version.is_empty() {
            "?".into()
        } else {
            meta.version.clone()
        },
        ed = if meta.edition.is_empty() {
            "?".into()
        } else {
            meta.edition.clone()
        },
    ));
    s.push_str(&build_test_run(root, &meta));
    s.push('\n');

    s.push_str(&replies::MAP_MODULES.cells());
    s.push('\n');
    s.push_str(report["full"].as_str().unwrap());
    if report["hasTools"].as_bool() == Some(true) {
        s.push('\n');
        s.push_str(&replies::MAP_TOOLS.cells());
        s.push('\n');
        s.push_str(report["tools"].as_str().unwrap());
    }

    s.push('\n');
    s.push_str(&replies::SELF_SAFETY.cells());
    s.push('\n');
    s.push('\n');
    s.push_str(&replies::MAP_FOOTER.cells());
    s.push('\n');
    Ok(s)
}

/// Locate Angel source within the active project, never the installed binary's
/// build tree. Stop at the nearest Git boundary (or the workspace without Git).
fn workspace_source_root(workspace: &Path) -> Option<PathBuf> {
    let workspace = workspace.canonicalize().ok()?;
    let boundary = workspace
        .ancestors()
        .find(|dir| crate::agent::harness::is_git_root(dir))
        .unwrap_or(&workspace);
    for dir in workspace
        .ancestors()
        .take_while(|dir| dir.starts_with(boundary))
    {
        for candidate in [dir.to_path_buf(), dir.join("cockpit")] {
            if let Ok(root) = candidate.canonicalize()
                && root.starts_with(boundary)
                && is_cockpit_root(&root)
            {
                return Some(root);
            }
        }
    }
    None
}

/// Condensed architecture context for self-work in the active Angel checkout.
/// An installed source pin does not opt an unrelated workspace into self-work.
pub fn self_context(workspace: &Path) -> String {
    if !env_flag("ANGEL_SELF_MODEL", true) {
        return String::new();
    }
    let Some(root) = workspace_source_root(workspace) else {
        return String::new();
    };
    let _compute = SELF_CONTEXT_COMPUTE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let module = match self_map_module() {
        Ok(module) => module,
        Err(error) => return self_context_unavailable(&error),
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    module_source_stamp(&root).hash(&mut hasher);
    module.fingerprint.hash(&mut hasher);
    let source_stamp = hasher.finish();
    let cache = SELF_CONTEXT_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(cached) = cache.lock()
        && let Some(cached) = cached.as_ref()
        && cached.root == root
        && cached.source_stamp == source_stamp
    {
        return cached.rendered.clone();
    }
    let report = match load_cached_report(&root, source_stamp) {
        Some(report) => report,
        None => {
            let report = match render_modules(&root, &module) {
                Ok(report) => report,
                Err(error) => return self_context_unavailable(&error),
            };
            // Only a report the checks below accept is worth keeping.
            if report["hasTop"].as_bool() == Some(true) {
                store_cached_report(&root, source_stamp, &report);
            }
            report
        }
    };
    if report["hasTop"].as_bool() != Some(true) {
        return self_context_unavailable("no source modules found");
    }
    // Guidance remains native; the worker supplies only the source summary.
    let mut s = format!(
        "\n\n{}\n{}",
        crate::agent::harness::book::n_environment::SELF_MODEL.cells(),
        report["compact"].as_str().unwrap()
    );
    if report["hasTools"].as_bool() == Some(true) {
        s.push_str(&format!(
            "{} {}\n",
            crate::agent::harness::book::d3_roles::pages(
                crate::agent::harness::book::n_environment::SELF_MODEL,
                [8]
            ),
            report["toolNames"].as_str().unwrap()
        ));
    }
    if s.len() >= 16 * 1024 {
        return self_context_unavailable("self-map context exceeds its byte limit");
    }
    if let Ok(mut cached) = cache.lock() {
        *cached = Some(SelfContextCache {
            root,
            source_stamp,
            rendered: s.clone(),
        });
    }
    s
}

fn self_context_unavailable(error: &str) -> String {
    format!(
        "\n\n{}\n[self-map unavailable: {error}]\n",
        crate::agent::harness::book::n_environment::SELF_MODEL.cells()
    )
}

// ---------------------------------------------------------------------------
// The self_map tool.
// ---------------------------------------------------------------------------

/// `self_map` — the agent's on-demand view of its OWN source structure.
pub struct SelfMapTool {
    /// A headless opt-in pins read authority; it must never use ambient fallback.
    read_only_root: Option<Result<PathBuf, String>>,
}

impl SelfMapTool {
    pub fn new() -> Self {
        SelfMapTool {
            read_only_root: None,
        }
    }

    pub fn for_workspace(workspace: &Path) -> Self {
        match workspace_source_root(workspace).or_else(source_root) {
            Some(root) => Self::read_only(root),
            None => Self {
                read_only_root: Some(Err(
                    "Angel source is unavailable for self-diagnostics".into()
                )),
            },
        }
    }

    pub fn read_only(root: PathBuf) -> Self {
        let pinned = root
            .canonicalize()
            .ok()
            .filter(|root| is_cockpit_root(root));
        SelfMapTool {
            read_only_root: Some(pinned.ok_or_else(|| {
                format!(
                    "invalid ANGEL_SELF_SRC pin: {} is not an angelX-cockpit crate root",
                    root.display()
                )
            })),
        }
    }

    /// No source probing or context injection unless the operator opts in.
    pub fn register_headless(registry: &mut crate::agent::harness::ToolRegistry) {
        if let Some(pin) = std::env::var_os("ANGEL_SELF_SRC")
            .filter(|pin| !pin.to_string_lossy().trim().is_empty())
        {
            registry.register(Box::new(Self::read_only(PathBuf::from(pin))));
        }
    }
}

impl Default for SelfMapTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SelfMapTool {
    fn name(&self) -> &str {
        "self_map"
    }
    fn def(&self) -> ToolDef {
        if self.read_only_root.is_some() {
            return ToolDef {
                name: "self_map".to_string(),
                description: "Read-only Angel harness source diagnostics, for an explicit self-work request. This is not the current project map. No args: source architecture map. module: one module's doc and symbol outline. Does not grant source editing or persist SELF.md; write=true is rejected. ⠣⠛".to_string(),
                params: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "module": { "type": "string", "description": "⠣⠛⠃" }
                    },
                    "additionalProperties": false
                }),
            };
        }
        ToolDef {
            name: "self_map".to_string(),
            description: "Your OWN source structure (angelX-cockpit). No args: the full map — \
                          crate identity, build/test/run, every module's purpose + key public \
                          types, and the self-modification safety contract. `module=\"<name>\"`: \
                          that module's doc + symbol outline. `write=true`: also write SELF.md \
                          to the crate root. ⠣⠛"
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "module": {
                        "type": "string",
                        "description": "⠣⠛⠉"
                    },
                    "write": {
                        "type": "boolean",
                        "description": "⠣⠛⠙"
                    }
                }
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let root = if let Some(root) = &self.read_only_root {
            if args.get("write").and_then(Value::as_bool) == Some(true) {
                return Err("self_map is read-only; write=true is not permitted".to_string());
            }
            let root = root.as_ref().map_err(Clone::clone)?;
            if !is_cockpit_root(root) {
                return Err(format!(
                    "invalid ANGEL_SELF_SRC pin: {} is not an angelX-cockpit crate root",
                    root.display()
                ));
            }
            root.clone()
        } else {
            source_root().ok_or_else(|| {
                format!(
                    "could not locate the angelX-cockpit source tree\n{}",
                    crate::agent::harness::book::d56_replies::SELF_SRC.cells()
                )
            })?
        };
        if let Some(module) = args.get("module").and_then(|v| v.as_str()) {
            return module_outline(&root, module);
        }
        let map = generate_self_model_at(&root)?;
        if args.get("write").and_then(|v| v.as_bool()) == Some(true) {
            let target = root.join("SELF.md");
            return crate::agent::harness::hardlink_result(|| {
                crate::agent::harness::confined_write(&root, Path::new("SELF.md"), map.as_bytes())?;
                Ok(format!("wrote {}\n\n{map}", target.display()))
            });
        }
        Ok(map)
    }
}

/// One module's doc purpose + a line-numbered symbol outline. `module` is a stem
/// (`harness`) or a `src`-relative-ish path (`tools/nav`); it must resolve to a
/// `.rs` file under `src/` — no path escapes.
fn module_outline(root: &Path, module: &str) -> Result<String, String> {
    let module = module.trim().trim_end_matches(".rs");
    let relative = module.strip_prefix("src/").unwrap_or(module);
    if relative.is_empty()
        || Path::new(relative).components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!("invalid module name: {module:?}"));
    }
    let source_root = root
        .join("src")
        .canonicalize()
        .map_err(|e| format!("locate source root: {e}"))?;
    // `src/` is grouped into layer directories, so accept both the qualified
    // path (`agent/harness/registry`) and the bare one (`harness/registry`) the
    // map has always used.
    let mut candidates = vec![
        source_root.join(format!("{relative}.rs")),
        source_root.join(relative).join("mod.rs"),
    ];
    for layer in LAYERS {
        candidates.push(source_root.join(layer).join(format!("{relative}.rs")));
        candidates.push(source_root.join(layer).join(relative).join("mod.rs"));
    }
    let candidate = candidates
        .iter()
        .find(|p| p.is_file())
        .ok_or_else(|| format!("no such module under src/: {module}"))?;
    let path = candidate
        .canonicalize()
        .map_err(|e| format!("resolve {module}: {e}"))?;
    if !path.starts_with(&source_root) {
        return Err(format!("module resolves outside src/: {module}"));
    }
    let mut total = 0;
    let src = read_source(&path, &source_root, &mut total)?
        .ok_or_else(|| format!("could not read {module} as UTF-8 source"))?;
    let rel = path
        .strip_prefix(&source_root)
        .map(|p| Path::new("src").join(p).to_string_lossy().to_string())
        .unwrap_or_else(|_| module.to_string());
    let result = self_map_worker(
        &self_map_module()?,
        &serde_json::json!({"op":"outline", "rel":rel, "src":src}),
    )?;
    result
        .get("outline")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "invalid self-map outline response".to_string())
}

// ---------------------------------------------------------------------------
// Layer 2 — the self-modification safety gate.
// ---------------------------------------------------------------------------

/// Verdict of the build+test gate: a self-edit is only acceptable when `passed`.
/// The Layer-2 commit/integrate condition — consumed live by the `/self` loop
/// (see [`crate::drive::self_loop`] and `docs/SELF_MODEL.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct GateVerdict {
    /// The crate built AND its test suite is green (≥1 test ran, none failed).
    pub passed: bool,
    /// Reward in [0,1]: 0 if the build broke, else the fraction of tests passing.
    pub reward: f32,
    /// Human-readable one-line summary.
    pub summary: String,
}

/// Decide the gate from a build result + parsed test outcome. **Pure** (no
/// process spawning) so it's unit-testable: the commit/integrate condition is
/// "builds AND all tests pass". A build failure is a hard 0 — never integrate a
/// non-compiling tree.
pub fn evaluate_gate(build_ok: bool, test: &TestOutcome) -> GateVerdict {
    if !build_ok {
        return GateVerdict {
            passed: false,
            reward: 0.0,
            summary: "REJECTED — the crate does not build".to_string(),
        };
    }
    let passed = test.all_passed();
    let summary = if passed {
        format!("GREEN — builds; {} tests pass", test.passed)
    } else if test.ran() == 0 {
        "REJECTED — builds but no tests ran (cannot confirm green)".to_string()
    } else {
        format!(
            "REJECTED — builds but {} of {} tests failed",
            test.failed,
            test.ran()
        )
    };
    GateVerdict {
        passed,
        reward: test.reward(),
        summary,
    }
}

/// [`evaluate_gate`] plus the anti-reward-hack **regression guard** the loop
/// applies to acceptance commands: a green whose pass-count fell below the
/// pre-edit baseline means tests were deleted or disabled to fake a pass —
/// rejected even at reward 1.0. Pure, like `evaluate_gate`.
pub fn evaluate_gate_with_baseline(
    build_ok: bool,
    test: &TestOutcome,
    baseline_passed: Option<usize>,
) -> GateVerdict {
    let mut v = evaluate_gate(build_ok, test);
    if v.passed
        && let Some(b) = baseline_passed
        && test.passed < b
    {
        v.passed = false;
        v.summary = format!(
            "REJECTED — green, but {} passed < baseline {} (tests deleted/disabled?)",
            test.passed, b
        );
    }
    v
}

/// Run the build+test gate against `workspace`: `cargo build` then `cargo test`,
/// parsed into a [`GateVerdict`]. This is the runtime side of the Layer-2 gate —
/// the condition under which a self-edit (in the workspace or an integrated
/// worktree) may be committed. Heavy (spawns cargo), so it's not exercised in unit
/// tests; [`evaluate_gate`] carries the logic and is.
#[allow(dead_code)] // doc-facing entry; the /self loop calls the baseline-aware variant
pub fn run_self_gate(workspace: &Path) -> GateVerdict {
    run_self_gate_with_baseline(workspace, None).0
}

/// [`run_self_gate`] with the pre-edit pass-count baseline threaded through to
/// the regression guard. The `/self` loop's verify step — a done-claim on a
/// self-modification run must survive this before integration is even offered.
/// Returns the verdict plus bounded actionable failure lines (compiler errors
/// on a broken build, failing tests on a red suite) — the setback the next
/// iteration is shown so it can fix the cause instead of re-claiming done;
/// empty on green.
pub fn run_self_gate_with_baseline(
    workspace: &Path,
    baseline_passed: Option<usize>,
) -> (GateVerdict, String) {
    let mut build_cmd = Command::new("cargo");
    build_cmd.arg("build").current_dir(workspace);
    let mut test_cmd = Command::new("cargo");
    test_cmd.arg("test").current_dir(workspace);
    run_self_gate_commands(build_cmd, test_cmd, baseline_passed)
}

fn run_self_gate_commands(
    build_cmd: Command,
    test_cmd: Command,
    baseline_passed: Option<usize>,
) -> (GateVerdict, String) {
    // `/self` verification runs off-thread, but a raw `Command::output` can
    // still wedge that worker forever: a noisy Cargo fills one pipe while the
    // parent waits, and a hung descendant keeps `verify_inflight` set. The
    // verifier budget stays unlimited. The capture still drains both pipes
    // and reaps the group when the child exits.
    let build = match crate::agent::harness::output_timed_captured(build_cmd, None) {
        Ok(capture) => capture,
        Err(error) => return (evaluate_gate(false, &TestOutcome::default()), error),
    };
    if let Some(issue) = self_gate_capture_issue(&build, "build") {
        return (
            evaluate_gate(false, &TestOutcome::default()).with_note(&issue),
            capture_failure_detail(&String::from_utf8_lossy(&build.output.stderr), &issue),
        );
    }
    if !build.output.status.success() {
        return (
            evaluate_gate(false, &TestOutcome::default()),
            crate::drive::loop_ctl::failure_detail(&String::from_utf8_lossy(&build.output.stderr)),
        );
    }
    let test = match crate::agent::harness::output_timed_captured(test_cmd, None) {
        Ok(capture) => capture,
        Err(e) => {
            return (
                evaluate_gate(false, &TestOutcome::default()).with_note(&e.to_string()),
                String::new(),
            );
        }
    };
    let stdout = String::from_utf8_lossy(&test.output.stdout);
    let combined = format!("{stdout}\n{}", String::from_utf8_lossy(&test.output.stderr));
    if let Some(issue) = self_gate_capture_issue(&test, "test") {
        // An incomplete prefix cannot establish even the parsed test reward.
        return (
            evaluate_gate(true, &TestOutcome::default()).with_note(&issue),
            capture_failure_detail(&combined, &issue),
        );
    }
    let mut verdict =
        evaluate_gate_with_baseline(true, &parse_test_result(&stdout), baseline_passed);
    if !test.output.status.success() {
        // Retain counts/reward from a complete red suite, but Cargo itself
        // must also succeed before any passing summary grants acceptance.
        if verdict.passed {
            verdict.summary =
                "REJECTED — builds, but cargo test exited nonzero or was terminated".to_string();
        } else {
            verdict = verdict.with_note("cargo test exited nonzero or was terminated");
        }
        verdict.passed = false;
    }
    let detail = if verdict.passed {
        String::new()
    } else {
        crate::drive::loop_ctl::failure_detail(&combined)
    };
    (verdict, detail)
}

fn self_gate_capture_issue(
    capture: &crate::agent::harness::exec::TimedCapture,
    stage: &str,
) -> Option<String> {
    if capture.timed_out {
        Some(format!(
            "cargo {stage} stopped after its execution deadline"
        ))
    } else if capture.cancelled {
        Some(format!("cargo {stage} was cancelled"))
    } else if !capture.output_complete() {
        Some(format!(
            "cargo {stage} output is incomplete (missing EOF or capture byte limit)"
        ))
    } else {
        None
    }
}

fn capture_failure_detail(output: &str, issue: &str) -> String {
    let detail = crate::drive::loop_ctl::failure_detail(output);
    if detail.is_empty() {
        issue.to_string()
    } else {
        format!("{issue}\n{detail}")
    }
}

impl GateVerdict {
    fn with_note(mut self, note: &str) -> Self {
        self.summary = format!("{} ({note})", self.summary);
        self
    }
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/self_model__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/self_model__capture_tests.rs"]
mod capture_tests;
