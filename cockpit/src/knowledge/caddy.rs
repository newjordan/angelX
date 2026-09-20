//! Caddy v0 — the recipes/hazards card between the chamber and the library
//! (`docs/plans/caddy.md`). Knowledge that already exists (the command that
//! produced a verified receipt here, the command that failed and why) must
//! reach the model on hop 1 without growing the chamber. Measured 2026-09-05:
//! the Mac competition loop ran 46 iterations with zero benchmark runs while
//! the working local-verifier recipe and the failure hazard sat in that
//! morning's receipts on the same machine.
//!
//! Storage: bounded atomic JSON-lines snapshots under `~/.angel0/caddy/<repo_key>/`
//! (`recipes.jsonl` / `hazards.jsonl`), keyed by the canonical repository
//! identity so a linked worktree or subdirectory shares its main checkout's
//! bag. Directory resolution mirrors `dossier.rs`: an `ANGEL_CADDY_DIR`
//! override, else the `~/.angel` root.
//!
//! The card is bounded (`ANGEL_CADDY_CARD_BYTES`, default 1536, min 256),
//! each line ≤ 200 chars, and drops from the bottom when over cap: doors
//! first, then oldest hazards, then oldest recipes. `ANGEL_CADDY=0` disables
//! both rendering and write-back. Missing stores render nothing; damaged
//! stores expose degraded health. Writer lock contention returns immediately.
//!
//! M06b (2026-09-09): cohort #6 measured the always-on full card as pure cost
//! (input +1990 tokens, sign p 0.001, wall +6.8 s, recipe precision/recall
//! 1.0 with the card absent) — the model re-read what the card already says.
//! The default is now **cost-aware**: `render_card_for_task` injects only
//! recipes/hazards RELEVANT to this workspace's verifier family (the same
//! ruling as `scripts/cohort-memory.py::recipe_relevance`), never door
//! skills/rituals, ≤ 400 B; when nothing is relevant it injects nothing and
//! ledgers `caddy: skipped (no relevant entry)`. `ANGEL_CADDY_CARD=full`
//! restores the full card for A/B comparison. The on/off/full default
//! resolves from a `[[features]]` receipt file (`ANGEL_FEATURE_DEFAULTS`,
//! else `docs/telemetry/feature-defaults.toml`), so root can retire the card
//! by data — see `docs/reflex/caddy-default.md`.

use crate::agent::club::{ChatMsg, ChatRole, ToolCall};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read as _;

mod storage;
use std::path::{Path, PathBuf};

const DEFAULT_CARD_BYTES: usize = 1536;
const MIN_CARD_BYTES: usize = 256;
const MAX_LINE_CHARS: usize = 200;
const MAX_STORED_COMMAND_CHARS: usize = 200;
const MAX_DIAGNOSTIC_CHARS: usize = 160;
const MAX_ENV_PREFIXES: usize = 8;
const MAX_RECIPES: usize = 5;
const MAX_HAZARDS: usize = 5;
const MAX_DOOR_SKILLS: usize = 6;
const MAX_DOOR_RITUALS: usize = 3;
/// Byte ceiling for the cost-aware (default) card: the relevant entries only.
const RELEVANT_CARD_BYTES: usize = 400;
const DOSSIER_RITUAL_MIN_BELIEF: f64 = 0.6;
/// Ceiling for each jsonl store snapshot (recipes.jsonl / hazards.jsonl).
/// The published file can never exceed this: the writer rewrites the whole
/// snapshot on every append and drops oldest rows (superseded rows with the
/// same command first) until it fits. `tail_bytes` readers also clamp reads
/// to this bound, so growth beyond it is impossible, not merely unlikely.
const STORE_TAIL_BYTES: u64 = 256 * 1024;
/// M05: a recipe stays on the card while unverified for at most this many
/// observed workspace identity transitions; after that it expires from the card
/// (compaction eventually removes it from disk).
pub(crate) const RECIPE_STALE_CHANGE_LIMIT: u32 = 5;
const RESULT_SCAN_BYTES: usize = 8 * 1024;
const SKILL_MD_SCAN_BYTES: usize = 4 * 1024;

/// Tools whose calls can become recipes/hazards.
const TRACKED_TOOLS: &[&str] = &[
    "shell",
    "proc_run",
    "cargo",
    "run_tests",
    "check",
    "lint",
    "machine_test",
];

/// A command that produced a verified result in this repo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Recipe {
    pub ts_ms: u64,
    pub command: String,
    /// `KEY=VAL` prefixes found in the command.
    pub env: Vec<String>,
    pub duration_ms: Option<u64>,
    pub tool: String,
    /// ≤120 chars, e.g. "verified: exit 0".
    pub note: String,
    /// Absent in legacy keyword-derived records. Those records are never
    /// presented as verified evidence after the typed-receipt migration.
    #[serde(default)]
    pub verification: Option<RecipeVerification>,
    /// M05: the short workspace HEAD the recipe was verified on, when known.
    /// Recipes without one (all pre-M05 records) never count as fresh.
    #[serde(default)]
    pub verified_head: Option<String>,
    /// M05: count of workspace identity transitions observed since the last fresh
    /// verified execution of this command. Maintained by the card render;
    /// a fresh verified execution resets it to 0.
    #[serde(default)]
    pub stale_since_changes: u32,
    #[serde(default)]
    pub workspace_state: Option<serde_json::Value>,
    #[serde(default)]
    pub observed_workspace_state: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum RecipeVerification {
    ExecutedVerifierV1,
}

/// A command that failed here, and why.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Hazard {
    pub ts_ms: u64,
    pub command: String,
    /// ≤160 chars.
    pub diagnostic: String,
    pub tool: String,
}

/// M05 memory-health summary for one store file: counts only. Reused by the
/// render paths, the write-back path and the `--task-json` envelope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct StoreHealthSummary {
    pub(crate) malformed_rows: usize,
    pub(crate) invalid_utf8_rows: usize,
    pub(crate) incomplete_tail: usize,
    pub(crate) lock_or_io_failures: usize,
}

impl StoreHealthSummary {
    pub(crate) fn healthy(&self) -> bool {
        self.malformed_rows == 0
            && self.invalid_utf8_rows == 0
            && self.incomplete_tail == 0
            && self.lock_or_io_failures == 0
    }

    pub(crate) fn from_read_health(health: &storage::ReadHealth) -> Self {
        Self {
            malformed_rows: health.malformed_rows,
            invalid_utf8_rows: health.invalid_utf8_rows,
            incomplete_tail: usize::from(health.unterminated_tail),
            lock_or_io_failures: usize::from(health.io_error.is_some()),
        }
    }
}

/// M05: caddy health for this workspace's stores (recipes + hazards), counts
/// only — one read each, no extra lock acquisition.
pub(crate) fn memory_health(workspace: &Path) -> StoreHealthSummary {
    let mut summary = memory_health_in(&caddy_dir(), workspace);
    summary.lock_or_io_failures += WRITE_FAILURES.swap(0, std::sync::atomic::Ordering::Relaxed);
    summary.lock_or_io_failures += crate::knowledge::experience::take_store_failures();
    summary
}

/// [`memory_health`] with the store directory injected (testable).
pub(crate) fn memory_health_in(dir: &Path, workspace: &Path) -> StoreHealthSummary {
    let repo_dir = dir.join(crate::platform::workspace_store::repo_identity(workspace).key);
    let recipes = storage::load::<Recipe>(&repo_dir.join("recipes.jsonl")).health;
    let hazards = storage::load::<Hazard>(&repo_dir.join("hazards.jsonl")).health;
    let mut summary = StoreHealthSummary::from_read_health(&recipes);
    let hazard_summary = StoreHealthSummary::from_read_health(&hazards);
    summary.malformed_rows += hazard_summary.malformed_rows;
    summary.invalid_utf8_rows += hazard_summary.invalid_utf8_rows;
    summary.incomplete_tail += hazard_summary.incomplete_tail;
    summary.lock_or_io_failures += hazard_summary.lock_or_io_failures;
    for name in ["recipes.jsonl", "hazards.jsonl"] {
        if let Some(status) = storage::probe_writer(&repo_dir, name) {
            summary.lock_or_io_failures += 1;
            let health = StoreHealthSummary {
                lock_or_io_failures: 1,
                ..Default::default()
            };
            record_health_event(workspace, name, &status, &health);
        }
    }
    if !summary.healthy() {
        record_health_event(
            workspace,
            "stores",
            &storage::WriteStatus::CorruptInput,
            &summary,
        );
    }
    summary
}

/// M05 staleness ruling for one stored recipe against the workspace head.
/// `Fresh` = verified on this head; `UnverifiedSince(head)` = render with a
/// staleness label; `Expired` = drop from the card.
enum StaleMark {
    Fresh,
    UnverifiedSince(String),
    Expired,
}

fn workspace_state(workspace: &Path) -> serde_json::Value {
    crate::agent::harness::trace_schema::workspace_state(Some(workspace))
}

fn stale_mark(recipe: &Recipe, state: &serde_json::Value) -> StaleMark {
    if recipe.stale_since_changes >= RECIPE_STALE_CHANGE_LIMIT {
        return StaleMark::Expired;
    }
    if recipe.workspace_state.as_ref() == Some(state)
        && state["tree_sha256"].as_str().is_some_and(|s| s.len() == 64)
    {
        return StaleMark::Fresh;
    }
    StaleMark::UnverifiedSince(recipe.verified_head.clone().unwrap_or_default())
}

/// The situation the card is dealt into: repo identity, base state, languages,
/// and whether a competition board is armed.
#[derive(Clone, Debug)]
pub(crate) struct Lie {
    pub repo_key: String,
    pub head: String,
    pub dirty_files: usize,
    pub langs: Vec<String>,
    pub board_armed: bool,
}

/// Whether the caddy is on (`ANGEL_CADDY`, default **on**).
fn enabled() -> bool {
    match std::env::var("ANGEL_CADDY") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t.is_empty() || v == "0" || t == "false" || t == "no" || t == "off")
        }
        // No explicit flag: the receipt file decides (default on).
        Err(_) => caddy_feature_default().as_deref() != Some("off"),
    }
}

/// How the card is dealt. `Relevant` (cost-aware, the M06b default), `Full`
/// (the pre-M06b card, kept for A/B), or `Off` (retired by receipt or flag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardMode {
    Off,
    Relevant,
    Full,
}

/// Card-mode resolution: explicit `ANGEL_CADDY_CARD` (full|relevant|off) wins,
/// else the `[[features]]` receipt (`on`→Relevant, `full`→Full, `off`→Off),
/// else Relevant. `ANGEL_CADDY=0` still kills everything.
pub(crate) fn card_mode() -> CardMode {
    if let Ok(value) = std::env::var("ANGEL_CADDY_CARD") {
        return match value.trim().to_ascii_lowercase().as_str() {
            "full" => CardMode::Full,
            "off" | "0" | "none" => CardMode::Off,
            _ => CardMode::Relevant,
        };
    }
    // An explicit ANGEL_CADDY flag outranks the receipt: `0` retires here,
    // and any truthy value re-enables even a receipt that retired the card.
    if std::env::var_os("ANGEL_CADDY").is_some() {
        return if enabled() {
            CardMode::Relevant
        } else {
            CardMode::Off
        };
    }
    match caddy_feature_default().as_deref() {
        Some("full") => CardMode::Full,
        Some("off") => CardMode::Off,
        _ => CardMode::Relevant,
    }
}

/// `default = "on"|"full"|"off"` for `name = "caddy"` from the features
/// receipt (`ANGEL_FEATURE_DEFAULTS`, else `docs/telemetry/feature-defaults.toml`
/// relative to the cwd). `None` when the file or entry is absent — the
/// caller then keeps the built-in default. Parse the table structurally so an
/// unrelated feature, key order, or inline comment cannot change Caddy policy.
fn caddy_feature_default() -> Option<String> {
    let path = std::env::var_os("ANGEL_FEATURE_DEFAULTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/telemetry/feature-defaults.toml"));
    let body = std::fs::read_to_string(path).ok()?;
    let receipt: toml::Value = toml::from_str(&body).ok()?;
    receipt
        .get("features")?
        .as_array()?
        .iter()
        .rev()
        .find(|feature| feature.get("name").and_then(toml::Value::as_str) == Some("caddy"))?
        .get("default")?
        .as_str()
        .map(str::to_owned)
}

/// Card byte cap (`ANGEL_CADDY_CARD_BYTES`, default 1536, min 256).
pub(crate) fn card_cap() -> usize {
    crate::agent::harness::env_usize("ANGEL_CADDY_CARD_BYTES", DEFAULT_CARD_BYTES)
        .max(MIN_CARD_BYTES)
}

/// Store root: `ANGEL_CADDY_DIR`, else `~/.angel0/caddy` (mirrors
/// `dossier.rs`'s `ANGEL_DOSSIER_DIR` resolution).
fn caddy_dir() -> PathBuf {
    match std::env::var("ANGEL_CADDY_DIR") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::platform::workspace_store::angel_subdir("caddy"),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Classify the situation from things already computed. Git state comes from
/// one bounded spawn per probe (300 ms deadline); unavailable probes report
/// partial metadata and retain the `no-git`/0 fallback. Languages come from markers.
pub(crate) fn read_lie(workspace: &Path) -> Lie {
    let repo_key = crate::platform::workspace_store::repo_identity(workspace).key;
    let probe = |args: &[&str], step| {
        let mut command = std::process::Command::new("git");
        command.args(args).current_dir(workspace);
        crate::platform::workspace_store::identity_probe(command, step)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    };
    let head = probe(&["rev-parse", "--short", "HEAD"], "caddy_head")
        .map(|out| first_line(&out).to_string())
        .unwrap_or_else(|| "no-git".to_string());
    // `git status --porcelain` prints nothing on a clean tree, and the probe
    // treats empty output as None — both a clean repo and a missing git are 0.
    let dirty_files = probe(&["status", "--porcelain"], "caddy_status")
        .map(|out| out.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    // Shared shallow scan (root + one level), so a repo whose crate lives under
    // a subdirectory (angel0: cockpit/Cargo.toml) is no longer read as `js`.
    let langs = crate::platform::workspace_lang::lang_names(
        &crate::platform::workspace_lang::detect(workspace),
    );
    Lie {
        repo_key,
        head,
        dirty_files,
        langs,
        board_armed: board_armed(workspace),
    }
}

/// Whether a competition board is armed. `goal.rs` has no competition
/// accessor, so this mirrors the one the turn classifier uses
/// (`turn/competition.rs::competition_mode_trigger`): the explicit
/// `ANGEL_COMPETITION_MODE` / `ANGEL_GPU_COMP_LOCAL_MOA` env flags, else an
/// Active `/goal` whose text carries competition vocabulary.
fn board_armed(workspace: &Path) -> bool {
    if crate::agent::harness::env_flag("ANGEL_COMPETITION_MODE", false)
        || crate::agent::harness::env_flag("ANGEL_GPU_COMP_LOCAL_MOA", false)
    {
        return true;
    }
    match crate::drive::goal::load_for(workspace) {
        Some(goal) if goal.status == crate::drive::goal::GoalStatus::Active => {
            let text = goal.text.to_ascii_lowercase();
            [
                "competition",
                "compete",
                "leaderboard",
                "submission",
                "podrace",
                "winning",
            ]
            .iter()
            .any(|needle| text.contains(needle))
        }
        _ => false,
    }
}

/// The recipes/hazards card for `workspace`, or "" when disabled or nothing
/// is known. Starts with a blank-line separator so callers can push it
/// straight after another block.
pub(crate) fn render_card(workspace: &Path, cap_bytes: usize) -> String {
    render_card_in(&caddy_dir(), workspace, cap_bytes)
}

/// The card as this task earns it: `Full` renders the pre-M06b card;
/// `Relevant` injects only entries whose verifier family matches this
/// workspace's languages (≤ [`RELEVANT_CARD_BYTES`]); `Off`/no-match inject
/// nothing, with the skip ledgered.
pub(crate) fn render_card_for_task(workspace: &Path, cap_bytes: usize) -> String {
    if !enabled() {
        return String::new();
    }
    match card_mode() {
        CardMode::Off => String::new(),
        CardMode::Full => render_card(workspace, cap_bytes),
        CardMode::Relevant => {
            render_relevant_card_in(&caddy_dir(), workspace, cap_bytes.min(RELEVANT_CARD_BYTES))
        }
    }
}

/// Verifier families implied by the workspace's detected languages — the
/// workspace-side half of the cohort's relevance ruling. A `check`/`lint`
/// tool call is never a test verifier, so its family rule stays in
/// [`recipe_relevance`] (only `run_tests` and `cargo test` match).
fn families_for_langs(langs: &[String]) -> Vec<&'static str> {
    let mut families = Vec::new();
    for lang in langs {
        let family = match lang.to_ascii_lowercase().as_str() {
            "rust" => "cargo-test",
            "js" | "javascript" | "node" | "typescript" | "ts" => "node-test",
            "python" => "python-test",
            "go" => "go-test",
            _ => continue,
        };
        if !families.contains(&family) {
            families.push(family);
        }
    }
    families
}

/// Runtimes a `run_tests`/`check`/`lint` recipe may name for `family` and
/// still be the same verifier family (port of `RELEVANCE_TABLE`).
fn allowed_runtimes(family: &str) -> &'static [&'static str] {
    match family {
        "node-test" => &["node", "auto"],
        "python-test" => &["python", "pytest", "auto"],
        "cargo-test" => &["cargo", "rust", "auto"],
        "go-test" => &["go"],
        _ => &[],
    }
}

/// Typed verifier tools whose stored command is `name {json}` (not a shell line).
fn typed_verifier_tool(name: &str) -> bool {
    matches!(name, "run_tests" | "cargo" | "check" | "lint")
}

/// Leading tool name of a stored typed-verifier command (`name {json}`).
fn split_typed_verifier(command: &str) -> Option<(&str, &str)> {
    let (name, rest) = command.split_once(' ')?;
    typed_verifier_tool(name).then_some((name, rest))
}

/// Shell-free word split of a recipe's `args` (a compound command never
/// establishes a verifier family — port of `command_words`).
fn simple_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for ch in command.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                started = true;
            }
            None if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            None => {
                current.push(ch);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Vec::new();
    }
    if started {
        words.push(current);
    }
    // Port of the cohort's `command_words`: a shell prefix of a compound
    // command never establishes a verifier family.
    if words.iter().any(|word| {
        word.contains(';')
            || word.contains('|')
            || word.contains('&')
            || word.contains('`')
            || word.contains("$(")
    }) {
        return Vec::new();
    }
    words
}

/// The same ruling as `scripts/cohort-memory.py::recipe_relevance`, against
/// the workspace's verifier families instead of one sealed verifier command:
/// a tracked tool call whose runtime (or, for `cargo`, a leading `test`
/// argument) matches the family. Never a substring match.
pub(crate) fn recipe_relevance(command: &str, families: &[&str]) -> Option<&'static str> {
    let (name, raw) = split_typed_verifier(command)?;
    let args: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let extra = args.get("args").and_then(|v| v.as_str()).unwrap_or("");
    let words = simple_words(extra);
    if !extra.trim().is_empty() && words.is_empty() {
        // Compound shell args never establish relevance.
        return None;
    }
    for family in families {
        if name == "cargo" {
            if *family == "cargo-test" && words.first().map(String::as_str) == Some("test") {
                return Some("typed-equivalent");
            }
        } else if name == "check" || name == "lint" {
            // Syntax/check/lint is not a test verifier (RELEVANCE_TABLE's
            // family=None row) — never relevant.
            continue;
        } else {
            let runtime = args
                .get("runtime")
                .and_then(|v| v.as_str())
                .unwrap_or("auto");
            if allowed_runtimes(family).contains(&runtime) {
                return Some("typed-equivalent");
            }
        }
    }
    None
}

/// Whether a stored hazard's command belongs to one of the workspace's
/// verifier families: tool-prefixed hazards reuse the recipe ruling; raw
/// shell hazards match on their program word (never a substring).
fn hazard_relevant(command: &str, families: &[&str]) -> bool {
    if split_typed_verifier(command).is_some() {
        return recipe_relevance(command, families).is_some();
    }
    let program = command.split_whitespace().next().unwrap_or("");
    families.iter().any(|family| {
        let programs: &[&str] = match *family {
            "node-test" => &["node", "npm", "npx", "yarn", "pnpm", "bun"],
            "python-test" => &["python", "python3", "pytest", "pip", "pip3"],
            "cargo-test" => &["cargo", "rustc"],
            "go-test" => &["go"],
            _ => &[],
        };
        programs.contains(&program)
    })
}

fn observe_workspace(repo_dir: &Path, workspace: &Path) -> serde_json::Value {
    let mut state = crate::agent::harness::trace_schema::startup_workspace_state(workspace);
    if state["tree_sha256"].as_str().is_some_and(|s| s.len() == 64) {
        // Timing/coverage annotations are not part of a complete verification
        // identity; preserve equality with the exact state stored by verifiers.
        if let Some(object) = state.as_object_mut() {
            for key in ["startup_index", "startup_walk_ms", "startup_walk_budget_ms"] {
                object.remove(key);
            }
        }
        let report =
            storage::append::<Recipe>(repo_dir, "recipes.jsonl", &[], Some(&state.to_string()));
        expose_report(workspace, "recipes.jsonl", &report);
    }
    state
}

fn staleness_label(recipe: &Recipe, state: &serde_json::Value) -> String {
    match stale_mark(recipe, state) {
        StaleMark::Fresh => String::new(),
        StaleMark::UnverifiedSince(head) if head.is_empty() => "[unverified legacy] ".into(),
        StaleMark::UnverifiedSince(head) => format!("[unverified-since {head}] "),
        StaleMark::Expired => "[expired] ".into(),
    }
}

fn record_store_health(
    workspace: &Path,
    recipes: &storage::ReadHealth,
    hazards: &storage::ReadHealth,
) {
    for (file, health) in [("recipes.jsonl", recipes), ("hazards.jsonl", hazards)] {
        let summary = StoreHealthSummary::from_read_health(health);
        if !summary.healthy() {
            record_health_event(
                workspace,
                file,
                &storage::WriteStatus::CorruptInput,
                &summary,
            );
        }
    }
}

fn live_recipes(rows: Vec<Recipe>, state: &serde_json::Value) -> Vec<Recipe> {
    rows.into_iter()
        .filter(|recipe| {
            recipe.verification.is_some()
                && !matches!(stale_mark(recipe, state), StaleMark::Expired)
        })
        .collect()
}

fn identity_header(lie: &Lie) -> String {
    let mut header = format!(
        "[caddy · {} · HEAD {} · {} dirty",
        short_key(&lie.repo_key),
        lie.head,
        lie.dirty_files
    );
    if !lie.langs.is_empty() {
        header.push_str(&format!(" · {}", lie.langs.join(",")));
    }
    header
}

fn recipe_card_line(recipe: &Recipe, count: usize, state: &serde_json::Value) -> String {
    let duration = recipe
        .duration_ms
        .map(|ms| format!("{}s", ms / 1000))
        .unwrap_or_else(|| "?".to_string());
    let mut note = recipe.note.clone();
    if count > 1 {
        note.push_str(&format!(" · records ×{count}"));
    }
    bound_line(&format!(
        "- {}{} — {} — {} — saved {}",
        staleness_label(recipe, state),
        recipe.command,
        duration,
        note,
        iso_day(recipe.ts_ms)
    ))
}

fn hazard_card_line(hazard: &Hazard) -> String {
    bound_line(&format!(
        "- {}: {} — {}",
        hazard.command,
        hazard.diagnostic,
        iso_day(hazard.ts_ms)
    ))
}

fn take_newest_recipes(mut recipes: Vec<(Recipe, usize)>) -> Vec<(Recipe, usize)> {
    recipes.sort_by_key(|recipe| std::cmp::Reverse(recipe.0.ts_ms));
    recipes.truncate(MAX_RECIPES);
    recipes
}

fn take_newest_hazards(mut hazards: Vec<Hazard>) -> Vec<Hazard> {
    hazards.sort_by_key(|hazard| std::cmp::Reverse(hazard.ts_ms));
    hazards.truncate(MAX_HAZARDS);
    hazards
}

/// Fit the cap by dropping from the bottom, never splitting a line: doors
/// first (full card only), then the oldest hazards, then the oldest recipes.
/// A section that loses its last entry loses its header too.
fn fit_card(
    header: &str,
    recipes: &[(Recipe, usize)],
    mut recipe_lines: Vec<String>,
    mut hazard_lines: Vec<String>,
    mut doors: Option<String>,
    state: &serde_json::Value,
    cap_bytes: usize,
) -> String {
    loop {
        let mut lines = vec![header.to_string()];
        if !recipe_lines.is_empty() {
            lines.push(
                if recipes
                    .iter()
                    .all(|(r, _)| matches!(stale_mark(r, state), StaleMark::Fresh))
                {
                    "recipes (verified on this workspace):"
                } else {
                    "recipes (historical; source unbound; rerun before relying):"
                }
                .to_string(),
            );
            lines.extend(recipe_lines.iter().cloned());
        }
        if !hazard_lines.is_empty() {
            lines.push("hazards (what failed here and why):".to_string());
            lines.extend(hazard_lines.iter().cloned());
        }
        if let Some(doors) = doors.as_ref() {
            lines.push(doors.clone());
        }
        let card = format!("\n\n{}\n", lines.join("\n"));
        if card.len() <= cap_bytes {
            return card;
        }
        if doors.is_some() {
            doors = None;
        } else if !hazard_lines.is_empty() {
            hazard_lines.pop();
        } else if !recipe_lines.is_empty() {
            recipe_lines.pop();
        } else {
            let header_only = format!("\n\n{header}\n");
            return if header_only.len() <= cap_bytes {
                header_only
            } else {
                String::new()
            };
        }
    }
}

/// Cost-aware card: only relevant entries, no doors, hard 400 B ceiling.
fn render_relevant_card_in(dir: &Path, workspace: &Path, cap_bytes: usize) -> String {
    let repo_dir = dir.join(crate::platform::workspace_store::repo_identity(workspace).key);
    let mut loaded_recipes = storage::load::<Recipe>(&repo_dir.join("recipes.jsonl"));
    let loaded_hazards = storage::load::<Hazard>(&repo_dir.join("hazards.jsonl"));
    record_store_health(workspace, &loaded_recipes.health, &loaded_hazards.health);
    // Empty stores need neither a language/status scan nor a content identity.
    // Still expose corrupt-store health above and retain the ordinary skip ledger.
    if loaded_recipes.rows.is_empty() && loaded_hazards.rows.is_empty() {
        record_skip(workspace);
        return String::new();
    }
    let lie = read_lie(workspace);
    let families = families_for_langs(&lie.langs);
    let state = if loaded_recipes
        .rows
        .iter()
        .any(|recipe| recipe.verification.is_some())
    {
        let state = observe_workspace(&repo_dir, workspace);
        // Observation can advance persisted staleness counters.
        loaded_recipes = storage::load::<Recipe>(&repo_dir.join("recipes.jsonl"));
        state
    } else {
        serde_json::json!({"tree_sha256":"unbound"})
    };
    let mut recipes = dedup_newest(live_recipes(loaded_recipes.rows, &state));
    recipes.retain(|(recipe, _)| {
        !families.is_empty() && recipe_relevance(&recipe.command, &families).is_some()
    });
    let mut hazards = loaded_hazards.rows;
    hazards.retain(|hazard| !families.is_empty() && hazard_relevant(&hazard.command, &families));
    if recipes.is_empty() && hazards.is_empty() {
        record_skip(workspace);
        return String::new();
    }
    let recipes = take_newest_recipes(recipes);
    let hazards = take_newest_hazards(hazards);
    let mut header = identity_header(&lie);
    header.push(']');
    let recipe_lines: Vec<String> = recipes
        .iter()
        .map(|(recipe, count)| recipe_card_line(recipe, *count, &state))
        .collect();
    let hazard_lines: Vec<String> = hazards.iter().map(hazard_card_line).collect();
    fit_card(
        &header,
        &recipes,
        recipe_lines,
        hazard_lines,
        None,
        &state,
        cap_bytes,
    )
}

/// Dedup recipes by command, keeping the newest stored record (the same
/// fold `render_card_in` performs), as (recipe, record count) pairs.
fn dedup_newest(recipes: Vec<Recipe>) -> Vec<(Recipe, usize)> {
    let mut by_command: HashMap<String, (Recipe, usize)> = HashMap::new();
    for recipe in recipes {
        match by_command.get_mut(&recipe.command) {
            Some(slot) => {
                slot.1 += 1;
                if recipe.ts_ms >= slot.0.ts_ms {
                    slot.0 = recipe;
                }
            }
            None => {
                by_command.insert(recipe.command.clone(), (recipe, 1));
            }
        }
    }
    by_command.into_values().collect()
}

/// Ledger receipt for the cost-aware skip: the store held entries, none were
/// relevant, so nothing was injected.
fn record_skip(workspace: &Path) {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if !crate::knowledge::experience::enabled() {
        return;
    }
    crate::knowledge::experience::append_jsonl(
        &crate::knowledge::experience::ledger_path(),
        &serde_json::json!({
            "kind": "caddy_card",
            "v": 1,
            "ts": now_ms() / 1000,
            // Fast empty-card checks can occur twice in the same millisecond,
            // or concurrently in processes sharing a store. Preserve distinct
            // invocation provenance instead of emitting byte-identical events.
            "pid": std::process::id(),
            "seq": SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            "ts_ms": now_ms(),
            "repo": crate::platform::workspace_store::repo_identity(workspace).key,
            "outcome": {
                "card": "skipped",
                "reason": "caddy: skipped (no relevant entry)",
            },
        }),
    );
}

/// M05: a classified caddy memory-health event (corrupt line, truncated
/// tail, lock/IO failure) reaches the experience ledger as counts and the
/// `--task` stderr — never a silent skip.
fn record_health_event(
    workspace: &Path,
    file: &str,
    status: &storage::WriteStatus,
    health: &StoreHealthSummary,
) {
    eprintln!(
        "[caddy] memory-health {file}: {:?}; malformed={} invalid-utf8={} incomplete-tail={} lock/io={}",
        status,
        health.malformed_rows,
        health.invalid_utf8_rows,
        health.incomplete_tail,
        health.lock_or_io_failures
    );
    if !crate::knowledge::experience::enabled() {
        return;
    }
    crate::knowledge::experience::append_jsonl(
        &crate::knowledge::experience::ledger_path(),
        &serde_json::json!({
            "kind": "caddy_health",
            "v": 1,
            "ts": now_ms() / 1000,
            "repo": crate::platform::workspace_store::repo_identity(workspace).key,
            "file": file,
            "class": if matches!(status, storage::WriteStatus::Busy) { crate::agent::harness::tool_errors::ToolErrorClass::Transient } else { crate::agent::harness::tool_errors::ToolErrorClass::Environment },
            "outcome": {
                "write": format!("{:?}", status),
                "malformed_rows": health.malformed_rows,
                "invalid_utf8_rows": health.invalid_utf8_rows,
                "incomplete_tail": health.incomplete_tail,
                "lock_or_io_failures": health.lock_or_io_failures,
            },
        }),
    );
}

fn expose_report(workspace: &Path, file: &str, report: &storage::WriteReport) {
    let mut health = StoreHealthSummary::from_read_health(&report.read_health);
    if !matches!(
        report.status,
        storage::WriteStatus::Published
            | storage::WriteStatus::Unchanged
            | storage::WriteStatus::CorruptInput
    ) {
        health.lock_or_io_failures += 1;
        WRITE_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    if !health.healthy() {
        record_health_event(workspace, file, &report.status, &health);
    }
}

static WRITE_FAILURES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// [`render_card`] with the store directory injected (testable).
fn render_card_in(dir: &Path, workspace: &Path, cap_bytes: usize) -> String {
    if !enabled() {
        return String::new();
    }
    let lie = read_lie(workspace);
    let repo_dir = dir.join(&lie.repo_key);
    // No stored verification can become stale in an empty recipe store.
    // Avoid indexing the entire workspace just to render an empty card.
    let has_verification = storage::load::<Recipe>(&repo_dir.join("recipes.jsonl"))
        .rows
        .iter()
        .any(|recipe| recipe.verification.is_some());
    let state = if has_verification {
        observe_workspace(&repo_dir, workspace)
    } else {
        serde_json::json!({"tree_sha256":"unbound"})
    };
    let loaded_recipes = storage::load::<Recipe>(&repo_dir.join("recipes.jsonl"));
    let loaded_hazards = storage::load::<Hazard>(&repo_dir.join("hazards.jsonl"));
    record_store_health(workspace, &loaded_recipes.health, &loaded_hazards.health);
    let recipes_degraded = loaded_recipes.health.degraded();
    let hazards_degraded = loaded_hazards.health.degraded();
    // Recipes verified on a different workspace identity render as
    // `unverified-since <short-head>`; recipes unverified for
    // RECIPE_STALE_CHANGE_LIMIT workspace changes expire from the card.
    let recipes = live_recipes(loaded_recipes.rows, &state);
    let hazards = loaded_hazards.rows;
    if recipes.is_empty() && hazards.is_empty() && !recipes_degraded && !hazards_degraded {
        return String::new();
    }
    // V1 receipts have no execution-time source identity. Keep them as historical
    // hints only; stored row counts and save dates do not prove fresh execution.
    let recipes = take_newest_recipes(dedup_newest(recipes));
    let hazards = take_newest_hazards(hazards);

    let mut header = identity_header(&lie);
    if recipes_degraded {
        header.push_str(" · recipes degraded");
    }
    if hazards_degraded {
        header.push_str(" · hazards degraded");
    }
    let runtimes = crate::platform::workspace_lang::host_runtimes();
    if !runtimes.is_empty() {
        header.push_str(&format!(" · host: {runtimes}"));
    }
    header.push(']');

    let recipe_lines: Vec<String> = recipes
        .iter()
        .map(|(recipe, count)| recipe_card_line(recipe, *count, &state))
        .collect();
    let hazard_lines: Vec<String> = hazards.iter().map(hazard_card_line).collect();
    fit_card(
        &header,
        &recipes,
        recipe_lines,
        hazard_lines,
        doors_line(&lie, workspace),
        &state,
        cap_bytes,
    )
}

/// `doors: skills … · dossier rituals: …` — names to open, never inline
/// library content.
fn doors_line(lie: &Lie, workspace: &Path) -> Option<String> {
    let mut parts = Vec::new();
    if lie.board_armed {
        let skills = door_skills(lie);
        if !skills.is_empty() {
            parts.push(format!("skills {}", skills.join(", ")));
        }
    }
    let rituals = dossier_rituals(workspace, &lie.repo_key);
    if !rituals.is_empty() {
        parts.push(format!("dossier rituals: {}", rituals.join("; ")));
    }
    (!parts.is_empty()).then(|| bound_line(&format!("doors: {}", parts.join(" · "))))
}

/// ≤6 skill names from `~/.angel0/skills` + `cockpit/skills` whose SKILL.md
/// description mentions one of the repo's languages or
/// "competition"/"benchmark".
fn door_skills(lie: &Lie) -> Vec<String> {
    let user_dir = std::env::var_os("ANGEL_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::platform::workspace_store::angel_subdir("skills"));
    let bundled_dir = std::env::var_os("ANGEL_BUNDLED_SKILLS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::platform::runtime_paths::cockpit_dir().join("skills"));
    let mut needles: Vec<String> = lie
        .langs
        .iter()
        .map(|lang| lang.to_ascii_lowercase())
        .collect();
    needles.extend(["competition", "benchmark"].map(String::from));
    let mut names = Vec::new();
    for dir in [user_dir, bundled_dir] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut batch: Vec<_> = entries.flatten().collect();
        batch.sort_by_key(|e| e.file_name());
        for entry in batch {
            if names.len() >= MAX_DOOR_SKILLS {
                return names;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let skill_md = entry.path().join("SKILL.md");
            let Some(body) = read_bounded(&skill_md, SKILL_MD_SCAN_BYTES as u64) else {
                continue;
            };
            let description = body
                .lines()
                .find_map(|l| l.trim().strip_prefix("description:"))
                .map(str::trim)
                .unwrap_or_default();
            if description.is_empty() {
                continue;
            }
            let lowered = description.to_ascii_lowercase();
            if needles.iter().any(|needle| lowered.contains(needle)) {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.truncate(MAX_DOOR_SKILLS);
    names
}

/// ≤3 dossier fact texts with belief ≥ 0.6 (the card's own gate — looser
/// than the injected dossier block's 0.70, because these are doors to open,
/// not assertions).
fn dossier_rituals(workspace: &Path, repo_key: &str) -> Vec<String> {
    let _ = workspace;
    let dir = match std::env::var("ANGEL_DOSSIER_DIR") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => crate::platform::workspace_store::angel_subdir("dossier"),
    };
    let Some(raw) = read_bounded(&dir.join(format!("{repo_key}.json")), 256 * 1024) else {
        return Vec::new();
    };
    let Ok(artifact) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    artifact["facts"]
        .as_array()
        .map(|facts| {
            facts
                .iter()
                .filter(|fact| fact["belief"].as_f64().unwrap_or(0.0) >= DOSSIER_RITUAL_MIN_BELIEF)
                .filter_map(|fact| fact["text"].as_str())
                .map(str::to_string)
                .take(MAX_DOOR_RITUALS)
                .collect()
        })
        .unwrap_or_default()
}

/// Fold the turn's tool history back into the store: every tracked call
/// paired with its result becomes a hazard (failure markers) or a recipe
/// (success + benchmark/verify vocabulary). Idempotent per (command, day).
/// `ChatMsg` carries no per-call timing, so `duration_ms` stays `None` in v0.
pub(crate) fn write_back_from_history(workspace: &Path, history: &[ChatMsg]) -> (usize, usize) {
    if !enabled() || history.is_empty() {
        return (0, 0);
    }
    let now = now_ms();
    let mut recipes: Vec<Recipe> = Vec::new();
    let mut hazards: Vec<Hazard> = Vec::new();
    let mut results: HashMap<&str, &ChatMsg> = HashMap::new();
    for message in history {
        if message.role == ChatRole::Tool
            && let Some(id) = message.tool_call_id.as_deref()
        {
            results.entry(id).or_insert(message);
        }
    }
    for message in history {
        if message.role != ChatRole::Assistant {
            continue;
        }
        for call in message.tool_calls.iter() {
            if !TRACKED_TOOLS
                .iter()
                .any(|tool| call.name.eq_ignore_ascii_case(tool))
            {
                continue;
            }
            let Some(result) = results.get(call.id.as_str()).copied() else {
                continue;
            };
            let recipe_call = result
                .tool_receipt
                .as_ref()
                .filter(|_| verified_recipe_call(call, result))
                .and_then(|r| r.routing.as_ref())
                .and_then(|r| r.routed_call.as_ref())
                .unwrap_or(call);
            let Some((command, env)) = command_of(recipe_call) else {
                continue;
            };
            let head = bound_bytes(&result.content, RESULT_SCAN_BYTES);
            if let Some(diagnostic) = hazard_diagnostic(head) {
                hazards.push(Hazard {
                    ts_ms: now,
                    command: bound_chars(&command, MAX_STORED_COMMAND_CHARS),
                    diagnostic,
                    tool: call.name.clone(),
                });
            } else if verified_recipe_call(call, result) {
                let state = result
                    .tool_receipt
                    .as_ref()
                    .and_then(|r| r.workspace_state.clone());
                let head = state
                    .as_ref()
                    .and_then(|s| s["head"].as_str())
                    .map(|h| h.chars().take(12).collect());
                recipes.push(Recipe {
                    ts_ms: result
                        .tool_receipt
                        .as_ref()
                        .map_or(now, |r| r.recorded_at_ms),
                    command: bound_chars(&command, MAX_STORED_COMMAND_CHARS),
                    env,
                    duration_ms: None,
                    tool: call.name.clone(),
                    note: "verified: ok".to_string(),
                    verification: Some(RecipeVerification::ExecutedVerifierV1),
                    verified_head: head,
                    stale_since_changes: 0,
                    workspace_state: state.clone(),
                    observed_workspace_state: state,
                });
            }
        }
    }
    if recipes.is_empty() && hazards.is_empty() {
        return (0, 0);
    }
    let repo_dir = caddy_dir().join(crate::platform::workspace_store::repo_identity(workspace).key);
    let recipe_count = append_new(&repo_dir, "recipes.jsonl", &recipes, workspace);
    let hazard_count = append_new(&repo_dir, "hazards.jsonl", &hazards, workspace);
    (recipe_count, hazard_count)
}

/// Command text for a call: `args["command"]` when present (with its leading
/// `KEY=VAL` prefixes parsed out), else the tool name + a bounded head of the
/// args JSON.
fn command_of(call: &ToolCall) -> Option<(String, Vec<String>)> {
    // Describe only executed fields. Verifier tools additionally select a
    // runtime; Cargo ignores that field. An unused `command` property must
    // never become the supposedly verified recipe's description.
    if typed_verifier_tool(&call.name) {
        let mut args = serde_json::json!({
            "args": call.args.get("args").and_then(|value| value.as_str()).unwrap_or("")
        });
        if call.name != "cargo" {
            args["runtime"] = call
                .args
                .get("runtime")
                .and_then(|value| value.as_str())
                .unwrap_or("auto")
                .into();
        }
        if let Some(entrypoint) = call
            .args
            .get("entrypoint")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            args["entrypoint"] = entrypoint.into();
        }
        if let Some(dir) = call.args.get("dir") {
            args["dir"] = dir.clone();
        }
        let env = call
            .args
            .get("env")
            .and_then(serde_json::Value::as_object)
            .map(|env| {
                env.iter()
                    .filter_map(|(key, value)| value.as_str().map(|value| format!("{key}={value}")))
                    .collect()
            })
            .unwrap_or_default();
        // Scrub decoded arguments before embedding their JSON in prose. A
        // newline immediately before a key becomes literal `\n` here, which
        // defeats a later word-boundary secret matcher on the command string.
        crate::platform::secrets::redact_value(&mut args);
        return Some((format!("{} {}", call.name, args), env));
    }
    if let Some(command) = call.args.get("command").and_then(|v| v.as_str()) {
        let command = command.trim();
        if command.is_empty() {
            return None;
        }
        return Some((command.to_string(), env_prefixes(command)));
    }
    let head = serde_json::to_string(&call.args).unwrap_or_default();
    let head = bound_chars(&head, 160);
    Some((format!("{} {}", call.name, head), Vec::new()))
}

/// Leading `KEY=VAL` assignments before the command word.
fn env_prefixes(command: &str) -> Vec<String> {
    let mut env = Vec::new();
    for token in command.split_whitespace().take(MAX_ENV_PREFIXES) {
        if token.starts_with('-') || !token.contains('=') {
            break;
        }
        let (key, _) = token.split_once('=').unwrap_or((token, ""));
        if key.is_empty() {
            break;
        }
        if token.chars().count() <= MAX_STORED_COMMAND_CHARS {
            env.push(token.to_string());
        }
    }
    env
}

/// Failure markers and the ≤160-char diagnostic after the first one found.
fn hazard_diagnostic(result: &str) -> Option<String> {
    if let Some(rest) = result.strip_prefix("tool error:") {
        return Some(bound_chars(rest.trim_start(), MAX_DIAGNOSTIC_CHARS));
    }
    for marker in ["[timed out", "killed", "exit 1", "exit 2"] {
        if let Some(at) = result.find(marker) {
            let rest = &result[at + marker.len()..];
            return Some(bound_chars(rest.trim_start(), MAX_DIAGNOSTIC_CHARS));
        }
    }
    None
}

/// Only the dispatch boundary can attach this transient receipt. Match the
/// complete call as well as its ID, so reused IDs cannot bless different args.
/// Raw shell text and asynchronous machine/process launches are not proofs.
pub(crate) fn verified_recipe_call(call: &ToolCall, result: &ChatMsg) -> bool {
    use crate::agent::harness::{ExecutionOutcome, VerificationOutcome};
    (matches!(call.name.as_str(), "cargo" | "run_tests" | "check" | "lint")
        || (matches!(call.name.as_str(), "shell" | "proc_run")
            && result
                .tool_receipt
                .as_ref()
                .and_then(|r| r.routing.as_ref())
                .and_then(|r| r.routed_call.as_ref())
                .is_some_and(|c| matches!(c.name.as_str(), "run_tests" | "check" | "lint"))))
        && result.tool_receipt.as_ref().is_some_and(|receipt| {
            receipt.call.id == call.id
                && receipt.call.name == call.name
                && receipt.call.args == call.args
                && receipt.outcome.execution == ExecutionOutcome::Succeeded
                && receipt.outcome.verification == VerificationOutcome::Passed
        })
}

/// Publish a bounded snapshot; report contention/corruption/IO instead of
/// conflating rejected persistence with a successful no-op. Every published
/// recipe snapshot also refreshes M05 staleness counters against the
/// workspace head.
fn append_new<T: Serialize + for<'de> Deserialize<'de> + GetCommand>(
    repo_dir: &Path,
    file: &str,
    entries: &[T],
    workspace: &Path,
) -> usize {
    let state = workspace_state(workspace).to_string();
    let report = storage::append(repo_dir, file, entries, Some(&state));
    expose_report(workspace, file, &report);
    if let storage::WriteStatus::PreservedTemporary(path)
    | storage::WriteStatus::FailedTemporaryCleanup { path, .. } = &report.status
    {
        eprintln!(
            "[caddy] preserved staging file at {}; inspect and move it before retrying Caddy writes",
            path.display()
        );
    }
    if !matches!(
        report.status,
        storage::WriteStatus::Unchanged | storage::WriteStatus::Published
    ) {
        eprintln!(
            "[caddy] {file}: {:?}; persisted new records={}, {}",
            report.status,
            report.written,
            report.read_health.summary()
        );
    }
    if report.skipped_new > 0 {
        eprintln!(
            "[caddy] {file}: {} new records omitted by bounded retention/admission",
            report.skipped_new
        );
    }
    report.written
}

trait GetCommand {
    fn command(&self) -> &str;
}

impl GetCommand for Recipe {
    fn command(&self) -> &str {
        &self.command
    }
}

impl GetCommand for Hazard {
    fn command(&self) -> &str {
        &self.command
    }
}

#[cfg(test)]
fn load_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    storage::load(path).rows
}

/// Strict UTF-8 view for existing bounded-tail callers. Corrupt bytes are
/// reported by storage::load and are never rewritten into replacement text.
#[cfg(test)]
fn read_tail(path: &Path, max_bytes: u64) -> Option<String> {
    let (bytes, _, _) = storage::tail_bytes(path, max_bytes).ok()?;
    String::from_utf8(bytes).ok()
}

fn read_bounded(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let mut bytes = Vec::new();
    let take = max_bytes.min(len);
    (&mut file).take(take).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// A byte-limited view rounded down to a UTF-8 boundary.
fn bound_bytes(text: &str, max_bytes: usize) -> &str {
    let mut cut = text.len().min(max_bytes);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}

/// UTF-8-safe char truncation, including an explicit truncation marker. A
/// scalar-value limit also bounds bytes to at most four times that limit.
fn bound_chars(text: &str, max_chars: usize) -> String {
    if text.char_indices().nth(max_chars).is_none() {
        return text.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let cut = text.char_indices().nth(max_chars - 1).unwrap().0;
    format!("{}…", &text[..cut])
}

/// One card line: assembled, then hard-bounded to MAX_LINE_CHARS.
fn bound_line(line: &str) -> String {
    bound_chars(line, MAX_LINE_CHARS)
}

fn short_key(key: &str) -> String {
    match key.rsplit_once('-') {
        Some((name, _)) if !name.is_empty() => name.to_string(),
        _ => key.to_string(),
    }
}

/// `YYYY-MM-DD` for a unix-ms timestamp (Howard Hinnant's civil date).
fn iso_day(ms: u64) -> String {
    let days = (ms / 86_400_000) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/caddy__staleness_tests.rs"]
mod staleness_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/caddy__storage_regression_tests.rs"]
mod storage_regression_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/caddy__m05_lifecycle_tests.rs"]
mod m05_lifecycle_tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/caddy__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/cockpit/app/caddy__t04c_recipe_tests.rs"]
mod t04c_recipe_tests;
