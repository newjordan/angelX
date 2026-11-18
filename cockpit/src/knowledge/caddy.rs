//! Caddy v0 — the recipes/hazards card between the chamber and the library
//! (`docs/plans/caddy.md`). Knowledge that already exists (the command that
//! produced a verified receipt here, the command that failed and why) must
//! reach the model on hop 1 without growing the chamber. Measured 2026-09-05:
//! the Mac competition loop ran 46 iterations with zero benchmark runs while
//! the working local-verifier recipe and the failure hazard sat in that
//! morning's receipts on the same machine.
//!
//! Storage: bounded atomic JSON-lines snapshots under `~/.angelX/caddy/<repo_key>/`
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
const RELEVANT_CARD_BYTES: usize = 400;
const DOSSIER_RITUAL_MIN_BELIEF: f64 = 0.6;
const STORE_TAIL_BYTES: u64 = 256 * 1024;
pub(crate) const RECIPE_STALE_CHANGE_LIMIT: u32 = 5;
const RESULT_SCAN_BYTES: usize = 8 * 1024;
const SKILL_MD_SCAN_BYTES: usize = 4 * 1024;

const TRACKED_TOOLS: &[&str] = &["shell", "proc_run", "cargo", "run_tests", "check", "lint"];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Recipe {
    pub ts_ms: u64,
    pub command: String,
    pub env: Vec<String>,
    pub duration_ms: Option<u64>,
    pub tool: String,
    pub note: String,
    #[serde(default)]
    pub verification: Option<RecipeVerification>,
    #[serde(default)]
    pub verified_head: Option<String>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Hazard {
    pub ts_ms: u64,
    pub command: String,
    pub diagnostic: String,
    pub tool: String,
}

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

pub(crate) fn memory_health(workspace: &Path) -> StoreHealthSummary {
    let mut summary = memory_health_in(&caddy_dir(), workspace);
    summary.lock_or_io_failures += WRITE_FAILURES.swap(0, std::sync::atomic::Ordering::Relaxed);
    summary.lock_or_io_failures += crate::knowledge::experience::take_store_failures();
    summary
}

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

#[derive(Clone, Debug)]
pub(crate) struct Lie {
    pub repo_key: String,
    pub head: String,
    pub dirty_files: usize,
    pub langs: Vec<String>,
    pub board_armed: bool,
}

fn enabled() -> bool {
    match std::env::var("ANGEL_CADDY") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t.is_empty() || v == "0" || t == "false" || t == "no" || t == "off")
        }
        Err(_) => caddy_feature_default().as_deref() != Some("off"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardMode {
    Off,
    Relevant,
    Full,
}

pub(crate) fn card_mode() -> CardMode {
    if let Ok(value) = std::env::var("ANGEL_CADDY_CARD") {
        return match value.trim().to_ascii_lowercase().as_str() {
            "full" => CardMode::Full,
            "off" | "0" | "none" => CardMode::Off,
            _ => CardMode::Relevant,
        };
    }
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

pub(crate) fn card_cap() -> usize {
    crate::agent::harness::env_usize("ANGEL_CADDY_CARD_BYTES", DEFAULT_CARD_BYTES)
        .max(MIN_CARD_BYTES)
}

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

pub(crate) fn read_lie(workspace: &Path) -> Lie {
    let repo_key = crate::platform::workspace_store::repo_identity(workspace).key;
    let probe = |args: &[&str], step| {
        let mut command = std::process::Command::new("git");
        command
            .args(crate::agent::harness::GIT_NO_WORKSPACE_EXEC)
            .args(args)
            .current_dir(workspace);
        crate::platform::workspace_store::identity_probe(command, step)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    };
    let head = probe(&["rev-parse", "--short", "HEAD"], "caddy_head")
        .map(|out| first_line(&out).to_string())
        .unwrap_or_else(|| "no-git".to_string());
    let dirty_files = probe(&["status", "--porcelain"], "caddy_status")
        .map(|out| out.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
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

fn board_armed(workspace: &Path) -> bool {
    if crate::agent::harness::env_flag("ANGEL_COMPETITION_MODE", false) {
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

pub(crate) fn render_card(workspace: &Path, cap_bytes: usize) -> String {
    render_card_in(&caddy_dir(), workspace, cap_bytes)
}

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

fn allowed_runtimes(family: &str) -> &'static [&'static str] {
    match family {
        "node-test" => &["node", "auto"],
        "python-test" => &["python", "pytest", "auto"],
        "cargo-test" => &["cargo", "rust", "auto"],
        "go-test" => &["go"],
        _ => &[],
    }
}

fn typed_verifier_tool(name: &str) -> bool {
    matches!(name, "run_tests" | "cargo" | "check" | "lint")
}

fn split_typed_verifier(command: &str) -> Option<(&str, &str)> {
    let (name, rest) = command.split_once(' ')?;
    typed_verifier_tool(name).then_some((name, rest))
}

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

pub(crate) fn recipe_relevance(command: &str, families: &[&str]) -> Option<&'static str> {
    let (name, raw) = split_typed_verifier(command)?;
    let args: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let extra = args.get("args").and_then(|v| v.as_str()).unwrap_or("");
    let words = simple_words(extra);
    if !extra.trim().is_empty() && words.is_empty() {
        return None;
    }
    for family in families {
        if name == "cargo" {
            if *family == "cargo-test" && words.first().map(String::as_str) == Some("test") {
                return Some("typed-equivalent");
            }
        } else if name == "check" || name == "lint" {
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

fn identity_header(
    lie: &Lie,
    recipes_degraded: bool,
    hazards_degraded: bool,
    host: Option<&str>,
) -> String {
    use crate::agent::harness::book::d456_knowledge as book;
    let mut pages = vec![book::CADDY_IDENTITY];
    if recipes_degraded {
        pages.push(book::CADDY_RECIPES_DEGRADED);
    }
    if hazards_degraded {
        pages.push(book::CADDY_HAZARDS_DEGRADED);
    }
    let mut header = format!(
        "{} repo={} head={} dirty={}",
        crate::agent::harness::book::d46_recovery::run(&pages),
        short_key(&lie.repo_key),
        lie.head,
        lie.dirty_files
    );
    if !lie.langs.is_empty() {
        header.push_str(&format!(" langs={}", lie.langs.join(",")));
    }
    if let Some(host) = host.filter(|host| !host.is_empty()) {
        header.push_str(&format!(" host={host}"));
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
    let command = hazard.command.lines().next().unwrap_or("").trim();
    let command = bound_chars(command, MAX_HAZARD_COMMAND_CHARS);
    bound_line(&format!(
        "- {} — `{}` ({})",
        hazard.diagnostic.trim(),
        command,
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

fn fit_card(
    header: &str,
    recipes: &[(Recipe, usize)],
    mut recipe_lines: Vec<String>,
    mut hazard_lines: Vec<String>,
    mut doors: Option<String>,
    state: &serde_json::Value,
    cap_bytes: usize,
) -> String {
    use crate::agent::harness::book::{d3_roles::pages, d456_knowledge::CADDY};
    loop {
        let mut headings = Vec::new();
        let mut body = Vec::new();
        if !recipe_lines.is_empty() {
            headings.push(
                if recipes
                    .iter()
                    .all(|(r, _)| matches!(stale_mark(r, state), StaleMark::Fresh))
                {
                    1
                } else {
                    2
                },
            );
            body.push("recipes:".to_string());
            body.extend(recipe_lines.iter().cloned());
        }
        if !hazard_lines.is_empty() {
            headings.push(3);
            body.push("hazards:".to_string());
            body.extend(hazard_lines.iter().cloned());
        }
        let mut lines = Vec::new();
        if !headings.is_empty() {
            lines.push(pages(CADDY, headings));
        }
        lines.push(header.to_string());
        lines.extend(body);
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

fn render_relevant_card_in(dir: &Path, workspace: &Path, cap_bytes: usize) -> String {
    let repo_dir = dir.join(crate::platform::workspace_store::repo_identity(workspace).key);
    let mut loaded_recipes = storage::load::<Recipe>(&repo_dir.join("recipes.jsonl"));
    let loaded_hazards = storage::load::<Hazard>(&repo_dir.join("hazards.jsonl"));
    record_store_health(workspace, &loaded_recipes.health, &loaded_hazards.health);
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
    let header = identity_header(&lie, false, false, None);
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

fn render_card_in(dir: &Path, workspace: &Path, cap_bytes: usize) -> String {
    if !enabled() {
        return String::new();
    }
    let lie = read_lie(workspace);
    let repo_dir = dir.join(&lie.repo_key);
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
    let recipes = live_recipes(loaded_recipes.rows, &state);
    let hazards = loaded_hazards.rows;
    if recipes.is_empty() && hazards.is_empty() && !recipes_degraded && !hazards_degraded {
        return String::new();
    }
    let recipes = take_newest_recipes(dedup_newest(recipes));
    let hazards = take_newest_hazards(hazards);

    let header = identity_header(
        &lie,
        recipes_degraded,
        hazards_degraded,
        Some(crate::platform::workspace_lang::host_runtimes()),
    );

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

fn doors_line(lie: &Lie, workspace: &Path) -> Option<String> {
    let mut parts = Vec::new();
    if lie.board_armed {
        let skills = door_skills(lie);
        if !skills.is_empty() {
            parts.push(format!("skills={}", skills.join(",")));
        }
    }
    let rituals = dossier_rituals(workspace, &lie.repo_key);
    if !rituals.is_empty() {
        parts.push(format!(
            "rituals={}",
            serde_json::to_string(&rituals).unwrap_or_default()
        ));
    }
    (!parts.is_empty()).then(|| {
        bound_line(&format!(
            "{} {}",
            crate::agent::harness::book::d456_knowledge::CADDY_DOORS.cells(),
            parts.join(" ")
        ))
    })
}

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

fn command_of(call: &ToolCall) -> Option<(String, Vec<String>)> {
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

fn bound_bytes(text: &str, max_bytes: usize) -> &str {
    let mut cut = text.len().min(max_bytes);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    &text[..cut]
}

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

const MAX_HAZARD_COMMAND_CHARS: usize = 72;

fn bound_line(line: &str) -> String {
    bound_chars(line, MAX_LINE_CHARS)
}

fn short_key(key: &str) -> String {
    match key.rsplit_once('-') {
        Some((name, _)) if !name.is_empty() => name.to_string(),
        _ => key.to_string(),
    }
}

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
