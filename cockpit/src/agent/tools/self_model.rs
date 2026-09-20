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
use crate::agent::tools::nav::is_outline_decl;
use serde_json::Value;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// How many leading lines of a file to scan for its `//!` doc block — doc
/// comments live at the very top by convention, so this bounds the work.
const DOC_SCAN_LINES: usize = 80;

// ---------------------------------------------------------------------------
// Locating our own source.
// ---------------------------------------------------------------------------

/// Locate the cockpit crate's source root (the dir holding `Cargo.toml` + `src/`).
///
/// Resolution order, most-explicit first:
///   1. `ANGEL_SELF_SRC` if it points at a crate root.
///   2. The shipped resource bundle or relocated source checkout.
///   3. Walk up from the current dir looking for `angel0-cockpit`'s `Cargo.toml`.
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
/// declaring `name = "angel0-cockpit"` (so we don't latch onto a sibling crate).
fn is_cockpit_root(dir: &Path) -> bool {
    if !dir.join("src/main.rs").is_file() {
        return false;
    }
    match std::fs::read_to_string(dir.join("Cargo.toml")) {
        Ok(toml) => toml.contains("name = \"angel0-cockpit\""),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Parsing module purpose + symbols out of the live source (pure helpers).
// ---------------------------------------------------------------------------

/// Extract a one-line purpose for a module from its leading `//!` doc block:
/// the first contiguous run of `//!` lines, joined and trimmed to one sentence.
/// `""` when the module has no doc comment (e.g. UI glue files).
pub(crate) fn module_purpose(src: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut started = false;
    for line in src.lines().take(DOC_SCAN_LINES) {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("//!") {
            let rest = rest.trim();
            if rest.is_empty() {
                if started {
                    break; // blank `//!` ends the lead paragraph
                }
                continue;
            }
            started = true;
            parts.push(rest);
        } else if started {
            break; // doc block ended (a code/use/attr line)
        }
    }
    first_sentence(&parts.join(" "))
}

/// First sentence of `s` (up to the first `". "`/trailing `.`), capped to a
/// readable length so one verbose module can't blow the budget.
fn first_sentence(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let mut end = s.len();
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'.' && (i + 1 == bytes.len() || bytes[i + 1] == b' ') {
            end = i + 1;
            break;
        }
    }
    let mut out: String = s[..end].trim().to_string();
    const MAX: usize = 200;
    if out.chars().count() > MAX {
        out = out.chars().take(MAX).collect::<String>() + "…";
    }
    out
}

/// A module's symbol surface: how many top-level items it declares, and the names
/// of its key **public** types/traits (the navigational anchors).
pub(crate) struct ModuleSymbols {
    pub count: usize,
    pub key_types: Vec<String>,
}

/// Count top-level declarations (via the shared outline heuristic) and pull out
/// the names of `pub`/`pub(crate)` `struct`/`enum`/`trait` items.
pub(crate) fn module_symbols(src: &str) -> ModuleSymbols {
    let mut count = 0;
    let mut key_types = Vec::new();
    for line in src.lines() {
        if !is_outline_decl(line) {
            continue;
        }
        count += 1;
        if let Some(name) = pub_type_name(line)
            && key_types.len() < 8
            && !key_types.contains(&name)
        {
            key_types.push(name);
        }
    }
    ModuleSymbols { count, key_types }
}

/// If `line` declares a `pub`/`pub(crate)`/`pub(super)` `struct`/`enum`/`trait`,
/// return its name. `None` otherwise (private items, fns, impls, …).
fn pub_type_name(line: &str) -> Option<String> {
    let mut s = line.trim_start();
    let mut had_pub = false;
    for p in ["pub(crate) ", "pub(super) ", "pub "] {
        if let Some(rest) = s.strip_prefix(p) {
            s = rest.trim_start();
            had_pub = true;
            break;
        }
    }
    if !had_pub {
        return None;
    }
    for kw in ["struct ", "enum ", "trait "] {
        if let Some(rest) = s.strip_prefix(kw) {
            let name: String = rest
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
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
            "angel0-cockpit".into()
        } else {
            name
        },
        version,
        edition,
        bin: if bin.is_empty() { "angel".into() } else { bin },
    }
}

// ---------------------------------------------------------------------------
// Module grouping — stable membership (a structural fact about the tree); the
// descriptions come from the live `//!` docs, so the map stays fresh.
// ---------------------------------------------------------------------------

/// Functional area for a top-level module, by file stem. Unknown modules fall
/// into "Other" so a newly-added module is never silently dropped from the map.
fn group_of(stem: &str) -> &'static str {
    match stem {
        "main" | "app" | "app_control" | "draw" | "term" | "input" | "status_view"
        | "transcript" | "hud" | "glyphs" | "anim" | "loader" | "chart" | "markdown" | "viewer"
        | "media" | "agent_view" | "turn_event_view" | "approval_view" | "artifacts_view"
        | "agentviz" | "terminal_art" | "raytrace" | "world_viz" => "Entry & TUI / display",
        "swarm" | "swarm_delegate" | "deli" | "overwatch" | "club" | "turn" | "session"
        | "bootstrap" | "self_loop" => "Drivers & orchestration",
        "harness" | "lsp" | "mcp" | "code_mode" | "sandbox" | "pty" | "reinforce" | "skills"
        | "approval" | "local_command" => "Agent harness & tooling",
        "memory" | "memory_store" | "librarian" | "compaction" => "Memory & compaction",
        "codex_import" | "openai_codex" | "agent_profile" => "External agents & import",
        _ => "Other",
    }
}

/// Group display order.
const GROUP_ORDER: &[&str] = &[
    "Drivers & orchestration",
    "Agent harness & tooling",
    "Entry & TUI / display",
    "Memory & compaction",
    "External agents & import",
    "Other",
];

struct ModuleInfo {
    stem: String,
    rel: String, // path relative to crate root, e.g. "src/harness.rs"
    purpose: String,
    symbols: ModuleSymbols,
}

#[derive(Clone)]
struct SelfContextCache {
    root: PathBuf,
    source_stamp: u64,
    rendered: String,
}

static SELF_CONTEXT_CACHE: OnceLock<Mutex<Option<SelfContextCache>>> = OnceLock::new();

/// The seven dependency layers `src/` is grouped into. They are directories,
/// not modules in their own right, so every scan reaches *through* them: the
/// stems one level down are the modules this map has always described, which
/// is what keeps [`group_of`] keyed on bare stems.
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

/// Read every module under `src/`, reaching through the layer directories, plus
/// `src/agent/tools/*.rs`, extracting purpose + symbols. `top_level` is the
/// module set; `tools` is the tool-impl set.
fn scan_modules(root: &Path) -> (Vec<ModuleInfo>, Vec<ModuleInfo>) {
    let (mut top_level, mut tools) = (Vec::new(), Vec::new());
    for (dir, rel_prefix, skip) in scan_roots(root) {
        let scanned = scan_dir(&dir, root, &rel_prefix, &skip);
        if rel_prefix == TOOLS_REL {
            tools = scanned;
        } else {
            top_level.extend(scanned);
        }
    }
    top_level.sort_by(|a, b| a.stem.cmp(&b.stem));
    (top_level, tools)
}

fn scan_dir(dir: &Path, root: &Path, rel_prefix: &str, skip: &[&str]) -> Vec<ModuleInfo> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        // `skip` holds the names this pass must not fold in: the layer dirs when
        // scanning `src/`, each layer's own `mod.rs` facade, and `tools/`, which
        // keeps its own dedicated section below.
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| skip.contains(&name))
        {
            continue;
        }
        // Directory modules (e.g. `src/agent/swarm/`) are read through their
        // `mod.rs` facade so the map keeps listing them after a file→dir split.
        let path = if entry.path().is_dir() && entry.path().join("mod.rs").is_file() {
            entry.path().join("mod.rs")
        } else {
            entry.path()
        };
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let stem = if path.file_name().and_then(|s| s.to_str()) == Some("mod.rs") {
            match path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
            {
                Some(s) => s.to_string(),
                None => continue,
            }
        } else {
            match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            }
        };
        let src = std::fs::read_to_string(&path).unwrap_or_default();
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| format!("{rel_prefix}/{stem}.rs"));
        out.push(ModuleInfo {
            stem,
            rel,
            purpose: module_purpose(&src),
            symbols: module_symbols(&src),
        });
    }
    out.sort_by(|a, b| a.stem.cmp(&b.stem));
    out
}

// ---------------------------------------------------------------------------
// Map generation.
// ---------------------------------------------------------------------------

/// The fixed build/test/run reference (the operator + agent contract).
fn build_test_run(root: &Path, meta: &CrateMeta) -> String {
    format!(
        "## Build · test · run\n\
         Crate root: `{root}` (run cargo here).\n\
         - build:  `cargo build`\n\
         - check:  `cargo check`\n\
         - test:   `cargo test`  ← the self-modification gate (must stay green)\n\
         - lint:   `cargo clippy`\n\
         - run:    `cargo run` (practice agent) · `ANGEL_BRAIN_KEY=<key> cargo run` (live fleet)\n\
         Binary `{bin}` from `src/main.rs`. The git root is the parent dir, so git \
         worktrees for isolated self-edits land beside the crate.\n",
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
            return "self-model unavailable: could not locate the angel0-cockpit source tree \
                    (set ANGEL_SELF_SRC to the crate root)."
                .to_string();
        }
    };
    generate_self_model_at(&root)
}

fn generate_self_model_at(root: &Path) -> String {
    let toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let meta = parse_crate_meta(&toml);
    let (top, tools) = scan_modules(root);

    let mut s = String::new();
    s.push_str(&format!(
        "# angel0 cockpit — self-model (`{name}` v{ver}, edition {ed})\n\n\
         A terminal-first Rust/ratatui agent harness: a `Bag` of model `Club`s driven \
         through a tool-using `run_turn` loop, with workspace-confined file tools, \
         git-worktree delegation, and a verifiable-reward (RLVR) substrate. This map is \
         generated from the live tree + module doc comments, so it tracks the code.\n\n",
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

    s.push_str("## Modules (src/) — grouped, with key public types\n");
    for group in GROUP_ORDER {
        let mut members: Vec<&ModuleInfo> =
            top.iter().filter(|m| group_of(&m.stem) == *group).collect();
        if members.is_empty() {
            continue;
        }
        members.sort_by(|a, b| a.stem.cmp(&b.stem));
        s.push_str(&format!("\n### {group}\n"));
        for m in members {
            s.push_str(&module_line(m));
        }
    }

    if !tools.is_empty() {
        s.push_str(
            "\n### Tool implementations (src/agent/tools/)\n\
             Concrete `Tool` impls grouped by capability; the `Tool` trait, `ToolRegistry`, \
             and `run_turn` loop live in `src/agent/harness/`.\n",
        );
        for m in &tools {
            s.push_str(&module_line(m));
        }
    }

    s.push_str(
        "\n## Self-modification safety\n\
         On Linux, file tools are confined at operation time by descriptor-relative \
         `openat2`/`openat` helpers; outbound symlinks and post-validation swaps are \
         rejected. The interactive workspace is the launch directory or explicit selection. \
         To edit THIS crate, point the workspace at its checkout (see \
         `docs/SELF_MODEL.md`) and gate every change on the build+test gate \
         (`tools::self_model::run_self_gate`): a self-edit is only acceptable if the crate \
         still builds AND `cargo test` is green. Prefer an isolated git worktree \
         (`delegate`/`integrate`) for risky edits; everything is reversible via git. \
         Self-modification is opt-in/break-fix behavior, not startup posture: use it only \
         for an explicit user request or a concrete cockpit failure, then return to normal \
         task work once the failure is handled.\n",
    );
    s.push_str("\n_Generated by `self_map` from the live tree — re-run it after edits._\n");
    s
}

/// One module's line in the map: path, purpose, symbol count, key types.
fn module_line(m: &ModuleInfo) -> String {
    let purpose = if m.purpose.is_empty() {
        "(no module doc — see outline)".to_string()
    } else {
        m.purpose.clone()
    };
    let types = if m.symbols.key_types.is_empty() {
        String::new()
    } else {
        format!("  [{}]", m.symbols.key_types.join(", "))
    };
    format!(
        "- `{rel}` ({n} symbols) — {purpose}{types}\n",
        rel = m.rel,
        n = m.symbols.count,
        purpose = purpose,
        types = types,
    )
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
    let source_stamp = module_source_stamp(&root);
    let cache = SELF_CONTEXT_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(cached) = cache.lock()
        && let Some(cached) = cached.as_ref()
        && cached.root == root
        && cached.source_stamp == source_stamp
    {
        return cached.rendered.clone();
    }
    let (top, tools) = scan_modules(&root);
    if top.is_empty() {
        return String::new();
    }

    let mut s = String::from(
        "\n\n# Self-model (your own source)\n\
         You are `angel0-cockpit`, a Rust/ratatui agent harness, and THIS is a map of your \
         OWN code. Build/test/run from the crate root: `cargo build` · `cargo test` \
         (the self-modification gate — keep it green) · `cargo run`. Call `self_map` for \
         the full structure (key types, symbol counts) or `self_map({\"module\":\"<name>\"})` \
         for one module's outline. This self-model is capability context, not a standing \
         objective: do normal task work by default. Inspect or change your own code only when \
         the user explicitly asks. To change your own code safely, \
         see the `self-modify` skill: edit in an isolated worktree, and integrate only when \
         build+test pass.\n\n\
         Modules (purpose from live doc comments):\n",
    );
    for group in GROUP_ORDER {
        let members: Vec<&ModuleInfo> =
            top.iter().filter(|m| group_of(&m.stem) == *group).collect();
        if members.is_empty() {
            continue;
        }
        s.push_str(&format!("- **{group}**: "));
        let items: Vec<String> = members
            .iter()
            .map(|m| {
                let p = first_clause(&m.purpose);
                if p.is_empty() {
                    m.stem.clone()
                } else {
                    format!("{} ({})", m.stem, p)
                }
            })
            .collect();
        s.push_str(&items.join("; "));
        s.push('\n');
    }
    if !tools.is_empty() {
        let names: Vec<&str> = tools.iter().map(|m| m.stem.as_str()).collect();
        s.push_str(&format!(
            "- **Tool impls (src/agent/tools/)**: {}\n",
            names.join(", ")
        ));
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

/// A very short clause (first ~8 words) of a purpose line, for the compact
/// injected list where the full sentence would be too long.
fn first_clause(purpose: &str) -> String {
    if purpose.is_empty() {
        return String::new();
    }
    let words: Vec<&str> = purpose.split_whitespace().take(8).collect();
    let mut out = words.join(" ");
    out = out.trim_end_matches([',', '.', ';', ':']).to_string();
    out
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
                    "invalid ANGEL_SELF_SRC pin: {} is not an angel0-cockpit crate root",
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
                description: "Read-only Angel harness source diagnostics, for an explicit self-work request. This is not the current project map. No args: source architecture map. module: one module's doc and symbol outline. Does not grant source editing or persist SELF.md; write=true is rejected.".to_string(),
                params: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "module": { "type": "string", "description": "source module stem, e.g. agent/harness/registry (the layer prefix is optional)" }
                    },
                    "additionalProperties": false
                }),
            };
        }
        ToolDef {
            name: "self_map".to_string(),
            description: "Your OWN source structure (angel0-cockpit). No args: the full map — \
                          crate identity, build/test/run, every module's purpose + key public \
                          types, and the self-modification safety contract. `module=\"<name>\"`: \
                          that module's doc + symbol outline. `write=true`: also write SELF.md \
                          to the crate root. Use this to understand or plan changes to yourself."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "module": {
                        "type": "string",
                        "description": "a module stem (e.g. \"harness\", \"swarm\", \"tools/nav\") for its outline only"
                    },
                    "write": {
                        "type": "boolean",
                        "description": "also persist the full map to SELF.md at the crate root"
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
                    "invalid ANGEL_SELF_SRC pin: {} is not an angel0-cockpit crate root",
                    root.display()
                ));
            }
            root.clone()
        } else {
            source_root().ok_or_else(|| {
                "could not locate the angel0-cockpit source tree (set ANGEL_SELF_SRC)".to_string()
            })?
        };
        if let Some(module) = args.get("module").and_then(|v| v.as_str()) {
            return module_outline(&root, module);
        }
        let map = generate_self_model_at(&root);
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
    let src = std::fs::read_to_string(&path).map_err(|e| format!("read {module}: {e}"))?;
    let purpose = module_purpose(&src);
    let mut out = String::new();
    let rel = path
        .strip_prefix(&source_root)
        .map(|p| Path::new("src").join(p).to_string_lossy().to_string())
        .unwrap_or_else(|_| module.to_string());
    out.push_str(&format!("# {rel}\n"));
    if !purpose.is_empty() {
        out.push_str(&format!("{purpose}\n"));
    }
    out.push_str("\nSymbols:\n");
    let mut n = 0;
    for (i, line) in src.lines().enumerate() {
        if is_outline_decl(line) {
            out.push_str(&format!("{}: {}\n", i + 1, line.trim_end()));
            n += 1;
            if n >= 400 {
                out.push_str("…[truncated at 400 symbols]\n");
                break;
            }
        }
    }
    if n == 0 {
        out.push_str("(no top-level symbols)\n");
    }
    Ok(out)
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
    // `/self` verification runs off-thread, but a raw `Command::output` can
    // still wedge that worker forever: a noisy Cargo fills one pipe while the
    // parent waits, and a hung descendant keeps `verify_inflight` set. Keep
    // the ordinary verifier budget unlimited while inheriting the harness's
    // concurrent drains, idle-hang ceiling, and process-group cleanup.
    let build = crate::agent::harness::output_timed(build_cmd, None);
    let build_ok = matches!(&build, Ok((o, false)) if o.status.success());
    if !build_ok {
        let detail = match &build {
            Ok((o, timed_out)) => {
                let detail =
                    crate::drive::loop_ctl::failure_detail(&String::from_utf8_lossy(&o.stderr));
                if *timed_out && detail.is_empty() {
                    "cargo build stopped after its execution deadline".to_string()
                } else {
                    detail
                }
            }
            Err(e) => e.clone(),
        };
        return (evaluate_gate(false, &TestOutcome::default()), detail);
    }
    let mut test_cmd = Command::new("cargo");
    test_cmd.arg("test").current_dir(workspace);
    let test_out = crate::agent::harness::output_timed(test_cmd, None);
    let (stdout, combined) = match &test_out {
        Ok((o, false)) => {
            let out = String::from_utf8_lossy(&o.stdout).into_owned();
            let combined = format!("{out}\n{}", String::from_utf8_lossy(&o.stderr));
            (out, combined)
        }
        Ok((o, true)) => {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            let detail = crate::drive::loop_ctl::failure_detail(&combined);
            return (
                evaluate_gate(true, &TestOutcome::default())
                    .with_note("cargo test stopped after its execution deadline"),
                if detail.is_empty() {
                    "cargo test stopped after its execution deadline".to_string()
                } else {
                    detail
                },
            );
        }
        Err(e) => {
            return (
                evaluate_gate(false, &TestOutcome::default()).with_note(&e.to_string()),
                String::new(),
            );
        }
    };
    let verdict = evaluate_gate_with_baseline(true, &parse_test_result(&stdout), baseline_passed);
    let detail = if verdict.passed {
        String::new()
    } else {
        crate::drive::loop_ctl::failure_detail(&combined)
    };
    (verdict, detail)
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
