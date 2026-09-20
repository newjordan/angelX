//! code-mode glue between the registry and the V8 runtime.

use super::interception::Interception;
use super::registry::SeatGrant;
use super::*;

/// Recover the stable counters from a model-facing code-mode receipt. Keeping
/// this parser narrow means the experience ledger never infers orchestration
/// work from prose or console output.
pub(crate) fn code_mode_receipt_metrics(result: &str) -> Option<(usize, u64)> {
    let receipt = result
        .lines()
        .find(|line| line.starts_with("--- code_mode receipt: "))?;
    let field = |name: &str| {
        receipt
            .split_ascii_whitespace()
            .find_map(|part| part.strip_prefix(name))
    };
    Some((
        field("nested_calls=")?.parse().ok()?,
        field("nested_output_bytes=")?.parse().ok()?,
    ))
}

pub(crate) fn code_mode_receipt_is_recipe(result: &str, recipe: &str) -> bool {
    result.lines().any(|line| {
        line.starts_with("--- code_mode receipt: ")
            && line
                .split_ascii_whitespace()
                .any(|part| part == format!("recipe={recipe}"))
    })
}

pub(crate) fn is_code_mode_policy_rejection(result: &str) -> bool {
    [
        "effectful code_mode is quarantined",
        "requires allow_effects=true",
        "nested-call budget exhausted",
        "nested-output budget exhausted",
        "batch has ",
        "script is ",
        "requires exactly one of `script` or `recipe`",
        "unknown code_mode recipe",
        "repo_recon recipe is read-only",
        "repo_recon query is ",
        "execution timed out",
    ]
    .iter()
    .any(|marker| result.contains(marker))
}

const REPO_RECON_MAX_QUERY_BYTES: usize = 32 * 1024;
const REPO_RECON_MAX_TERMS: usize = 8;

fn repo_recon_terms(query: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "about",
        "after",
        "again",
        "against",
        "also",
        "and",
        "are",
        "been",
        "before",
        "being",
        "but",
        "can",
        "code",
        "coding",
        "could",
        "does",
        "doing",
        "each",
        "fix",
        "for",
        "from",
        "have",
        "help",
        "implement",
        "improve",
        "into",
        "its",
        "just",
        "make",
        "more",
        "need",
        "now",
        "only",
        "please",
        "repo",
        "repository",
        "should",
        "some",
        "something",
        "task",
        "than",
        "that",
        "the",
        "their",
        "then",
        "there",
        "these",
        "they",
        "this",
        "those",
        "through",
        "use",
        "using",
        "want",
        "was",
        "were",
        "what",
        "when",
        "where",
        "which",
        "while",
        "with",
        "would",
    ];
    let mut seen = std::collections::HashSet::new();
    let mut ranked = query
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')))
        .enumerate()
        .filter_map(|(position, raw)| {
            let term = raw.trim_matches(|c: char| matches!(c, '_' | '-' | '.' | '/'));
            if term.len() < 3 || term.len() > 96 {
                return None;
            }
            let lower = term.to_ascii_lowercase();
            if STOP.contains(&lower.as_str()) || !seen.insert(lower.clone()) {
                return None;
            }
            let shape = usize::from(term.contains(['/', '_', '-', '.'])) * 32
                + usize::from(term.chars().any(|c| c.is_ascii_uppercase())) * 12
                + term.len().min(24);
            Some((std::cmp::Reverse(shape), position, lower))
        })
        .collect::<Vec<_>>();
    ranked.sort();
    ranked
        .into_iter()
        .take(REPO_RECON_MAX_TERMS)
        .map(|(_, _, term)| term)
        .collect()
}

/// Produce a fixed, trusted program for task-conditioned repository mapping.
/// The task text is reduced to JSON-encoded search terms in Rust; it is never
/// interpolated as executable JavaScript.
fn repo_recon_script(query: &str) -> Result<String, String> {
    if query.len() > REPO_RECON_MAX_QUERY_BYTES {
        return Err(format!(
            "tool error: code_mode repo_recon query is {} B; limit is {REPO_RECON_MAX_QUERY_BYTES} B",
            query.len()
        ));
    }
    let terms = repo_recon_terms(query);
    let pattern = if terms.is_empty() {
        "__angel_no_search_terms__".to_string()
    } else {
        terms
            .iter()
            .map(|term| regex::escape(term))
            .collect::<Vec<_>>()
            .join("|")
    };
    let terms_json = serde_json::to_string(&terms).map_err(|e| e.to_string())?;
    let pattern_json = serde_json::to_string(&pattern).map_err(|e| e.to_string())?;
    Ok(REPO_RECON_PROGRAM
        .replace("__TASK_RECON_TERMS__", &terms_json)
        .replace("__TASK_RECON_PATTERN__", &pattern_json))
}

const REPO_RECON_PROGRAM: &str = r#"
const terms = __TASK_RECON_TERMS__;
const pattern = __TASK_RECON_PATTERN__;
// Batch 1: root listing + content grep only (cheap base).
const first = batch([
  {tool:'list_dir', args:{path:'.'}},
  {tool:'grep', args:{pattern, path:'.', ignore_case:true, context:0}},
]);
const text = (item) => item && item.ok && typeof item.output === 'string' ? item.output : '';
const clipped = (value, limit) => String(value).replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f]/g, '').slice(0, limit);
const rootEntries = text(first[0]).split('\n')
  .filter(entry => entry && entry !== 'off-limits/' && !entry.startsWith('.'))
  .slice(0, 120);
const rootSet = new Set(rootEntries.map(e => e.replace(/\/$/, '')));
const candidates = new Map();
// Prefer implementing libraries over docs/testdata mirrors (coding harness quality).
function implPathBonus(path) {
  const p = String(path).toLowerCase().replace(/\\/g, '/');
  if (/(^|\/)(docs|testdata|fixtures|examples|example|generated|vendor|node_modules|target\/|\.git\/)(\/|$)/.test(p)) {
    return -6;
  }
  if (/(^|\/)(src|lib|pkg|core|internal|machinery|cockpit\/src|crates\/[^/]+\/src)(\/|$)/.test(p)) {
    return 8;
  }
  if (/\.(rs|go|py|ts|tsx|js|jsx|c|cc|cpp|h|hpp|java|kt|swift)$/.test(p)) {
    return 3;
  }
  if (/\.(md|txt|rst|adoc)$/.test(p)) {
    return -3;
  }
  return 0;
}
function note(path, score, sample) {
  path = clipped(path, 512);
  if (!path || path.startsWith('no files fuzzy-match')) return;
  if (path.includes('off-limits/')) return;
  // Path-class bonus once per path (not per hit) so dense docs hits cannot
  // outrank a single implementing-library match.
  const isNew = !candidates.has(path);
  const current = candidates.get(path) || {path, score:0, hits:0, samples:[]};
  current.score += score + (isNew ? implPathBonus(path) : 0);
  if (sample) {
    current.hits += 1;
    if (current.samples.length < 3) current.samples.push(clipped(sample, 240));
  }
  candidates.set(path, current);
}
for (const line of text(first[1]).split('\n')) {
  const match = line.match(/^(.+?):([0-9]+):(.*)$/);
  if (!match) continue;
  const lowerPath = match[1].toLowerCase();
  const pathBonus = terms.reduce((sum, term) => sum + (lowerPath.includes(term) ? 4 : 0), 0);
  note(match[1], 8 + pathBonus, `${match[2]}:${match[3]}`);
}
// Benchmark and dependency manifests: only open existing workspace-root files.
const manifestNames = ['benchmark.json','go.mod','package.json','Cargo.toml','pom.xml','build.gradle','build.gradle.kts'];
const presentManifests = manifestNames.filter(name => rootSet.has(name));
const manifestBatch = presentManifests.length
  ? batch(presentManifests.map(path => ({tool:'read_file', args:{path}})))
  : [];
const manifests = [];
const dep_hits = [];
for (let index = 0; index < presentManifests.length; index++) {
  const path = presentManifests[index];
  const bodyRaw = text(manifestBatch[index]);
  if (!bodyRaw) continue;
  const body = clipped(bodyRaw, 2500);
  manifests.push({path, role:path === 'benchmark.json' ? 'benchmark-contract' : 'dependency-manifest', preview:body});
  note(path, 2, '');
  if (path === 'benchmark.json') continue; // Command metadata, not dependency upgrade advice.
  const lower = body.toLowerCase();
  for (const term of terms) {
    if (term.length >= 3 && lower.includes(term)) {
      dep_hits.push({manifest:path, term, hint:'prefer version bump in this lockfile over vendoring'});
      note(path, 12, `dep-match:${term}`);
    }
  }
}
// The union grep above already found enough paths for most tasks. Only sparse
// tasks pay for a definition fallback, and all eligible terms share ONE
// workspace scan rather than rereading the repo once per term.
const symbol_map = [];
const definitionFallback = candidates.size < 4;
const defTerms = definitionFallback
  ? terms.filter(term => /^[a-z0-9_$:]+$/i.test(term)).slice(0, 6)
  : [];
const defBatch = defTerms.length
  ? batch([{tool:'defs', args:{names:defTerms, ignore_case:true}}])
  : [];
const definitionLines = text(defBatch[0]).split('\n').filter(Boolean);
for (const term of defTerms) {
  const lines = definitionLines
    .filter(line => line.toLowerCase().includes(term.toLowerCase()))
    .slice(0, 8);
  for (const line of lines) {
    const m = line.match(/^(.+?):([0-9]+)/);
    if (m) note(m[1], 10, line.slice(0, 200));
  }
  if (lines.length) symbol_map.push({term, hits:lines.map(l => clipped(l, 200))});
}
const filenameFallback = candidates.size < 4;
const filenameResults = filenameFallback
  ? batch(terms.slice(0, 4).map(query => ({tool:'file_search', args:{query, limit:12}})))
  : [];
for (let index = 0; index < filenameResults.length; index++) {
  const term = terms[index] || '';
  for (const path of text(filenameResults[index]).split('\n').filter(Boolean).slice(0, 12)) {
    note(path, 3 + (path.toLowerCase().includes(term) ? 3 : 0), '');
  }
}
const ranked = [...candidates.values()].sort((a, b) =>
  (b.score - a.score) || (b.hits - a.hits) || (a.path < b.path ? -1 : a.path > b.path ? 1 : 0)
).slice(0, 8);
const outlines = ranked.length
  ? batch(ranked.map(candidate => ({tool:'outline', args:{path:candidate.path}})))
  : [];
for (let index = 0; index < ranked.length; index++) {
  ranked[index].outline = clipped(text(outlines[index]), 1200);
}
const errors = first.concat(manifestBatch, defBatch, filenameResults, outlines)
  .filter(item => item && !item.ok)
  .map(item => clipped(item.error || 'nested read failed', 240))
  .filter(msg => !/no such file|not found|does not exist/i.test(msg))
  .slice(0, 8);
return {
  schema:'angel-repo-recon/v2',
  snapshot:'preturn-read-only',
  terms,
  root_entries:rootEntries,
  manifests,
  dep_hits:dep_hits.slice(0, 16),
  symbol_map:symbol_map.slice(0, 8),
  candidates:ranked,
  filename_fallback:filenameFallback,
  errors,
};
"#;

// ---------------------------------------------------------------------------
// code-mode glue — bridge between the harness ToolRegistry and the pure V8
// runtime in `code_mode.rs`. On by default; `ANGEL_CODE_MODE=0` disables the
// `code_mode` tool (see `maybe_register_code_mode`). The runtime itself is
// harness-agnostic; this is the only place that knows about both worlds.
// ---------------------------------------------------------------------------

/// Execute a `code_mode` tool call: run the model's `script` in the V8 isolate
/// with every bindable registry tool exposed as a host function. Nested tool
/// calls re-enter the registry through `dispatch` (with the same pre/post hooks),
/// and are deliberately *not* run through the central output cap — a script that
/// reads→edits→writes a file needs the full content. The script's final return
/// value (capped upstream like any tool result) is what re-enters history.
pub(crate) fn run_code_mode_tool(
    registry: &ToolRegistry,
    hooks: &Hooks,
    args: &Value,
    event_context: Option<(&ToolEventId, &mpsc::Sender<TurnEvent>)>,
    cancel: Option<&AtomicBool>,
) -> String {
    let supplied_script = args
        .get("script")
        .and_then(Value::as_str)
        .filter(|script| !script.is_empty());
    let supplied_recipe = args
        .get("recipe")
        .and_then(Value::as_str)
        .filter(|recipe| !recipe.is_empty());
    // Both given: the call is unambiguous (an explicit script wins over the
    // fixed recipe), so run the script and say so instead of failing the
    // exclusivity check the model kept retrying. Only neither-given still
    // errors: there is genuinely nothing to run.
    let script_recipe_notice = match (supplied_script, supplied_recipe) {
        (Some(_), Some(_)) => Some("[code_mode: both script and recipe given; ran script]"),
        _ => None,
    };
    let effective_recipe = if supplied_script.is_some() {
        None
    } else {
        supplied_recipe
    };
    if supplied_script.is_none() && effective_recipe.is_none() {
        return "tool error: code_mode requires exactly one of `script` or `recipe`".to_string();
    }
    let recipe = effective_recipe.unwrap_or("script");
    let owned_script;
    let script = match effective_recipe {
        Some("repo_recon") => {
            if args
                .get("allow_effects")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return "tool error: code_mode repo_recon recipe is read-only".to_string();
            }
            let query = match args.get("query").and_then(Value::as_str) {
                Some(query) => query,
                None => {
                    return "tool error: code_mode repo_recon recipe requires a `query` string"
                        .to_string();
                }
            };
            owned_script = match repo_recon_script(query) {
                Ok(script) => script,
                Err(error) => return error,
            };
            owned_script.as_str()
        }
        Some(other) => return format!("tool error: unknown code_mode recipe `{other}`"),
        None => supplied_script.expect("validated script/recipe exclusivity"),
    };
    let max_script_bytes = if crate::yolo::enabled() && recipe != "repo_recon" {
        usize::MAX
    } else {
        env_usize("ANGEL_CODE_MODE_MAX_SCRIPT_BYTES", 65_536).max(1)
    };
    if script.len() > max_script_bytes {
        return format!(
            "tool error: code_mode script is {} B; limit is {max_script_bytes} B",
            script.len()
        );
    }
    // Read-only reconnaissance is the safe/default use case. A script may
    // reach mutation or execution tools only when its outer call makes that
    // broader capability explicit, so an approval preview and persisted tool
    // call cannot disguise effects behind an opaque program.
    let allow_effects = if recipe == "repo_recon" {
        false
    } else {
        args.get("allow_effects")
            .and_then(Value::as_bool)
            .unwrap_or_else(crate::yolo::code_effects_allowed)
    };
    // Full YOLO and Smart YOLO both unlock effectful code_mode; Full still
    // lifts script/timeout/call caps elsewhere. Env pin remains the ablation path.
    if allow_effects
        && !crate::yolo::code_effects_allowed()
        && !env_flag("ANGEL_CODE_MODE_EFFECTS", false)
    {
        return "tool error: effectful code_mode is quarantined; operator must set \
                ANGEL_CODE_MODE_EFFECTS=1 in addition to allow_effects=true \
                (or enable /yolos / /yolo)"
            .to_string();
    }
    // Generic scripts may orchestrate long build matrices. The trusted preturn
    // recipe is latency-sensitive and gets a much smaller non-disableable cap.
    let timeout_ms = if recipe == "repo_recon" {
        env_usize("ANGEL_TASK_RECON_TIMEOUT_MS", 15_000).clamp(100, 60_000) as u64
    } else if crate::yolo::enabled() {
        // The V8 API requires a finite watchdog duration. One year is an
        // effectively unbounded operator session while retaining a final
        // process-safety escape for a permanently wedged isolate.
        365 * 24 * 60 * 60 * 1_000
    } else {
        match env_usize("ANGEL_CODE_MODE_TIMEOUT_MS", 300_000) as u64 {
            0 => 86_400_000,
            n => n,
        }
    };
    let heap_mb = env_usize("ANGEL_CODE_MODE_HEAP_MB", 256);
    let tool_names = registry.bindable_tool_names();
    let allowed_tools = tool_names
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    // §3.2.3 (Def 26/27): this seat's policy is a table consulted at each nested
    // invocation rather than a branch — bound once here for the run and released by
    // `_seat` on every exit path, including an unwind. `allow_effects` decides the
    // table, so flipping it mid-run would bind the next call with no reload.
    const CODE_MODE_SEAT: &str = "code_mode";
    let seat_table = if allow_effects {
        Interception::empty()
    } else {
        // The seat's read-only predicate, stated once, becomes its deny list.
        Interception::deny_tools(
            tool_names
                .iter()
                .filter(|name| !is_code_mode_repo_read(name.as_str())),
        )
    };
    let _seat = SeatGrant::install(registry, CODE_MODE_SEAT, seat_table);
    let generic_max_calls = env_usize("ANGEL_CODE_MODE_MAX_CALLS", 48).max(1);
    let generic_max_nested_output_bytes =
        env_usize("ANGEL_CODE_MODE_MAX_NESTED_OUTPUT_BYTES", 8 * 1024 * 1024).max(1);
    // v2 recon batches manifests + defs + outlines; 24 nested calls is still
    // far cheaper than a paid multi-hop grep loop (Roll 09 lesson).
    let (max_calls, max_nested_output_bytes) = if recipe == "repo_recon" {
        (
            generic_max_calls.min(env_usize("ANGEL_TASK_RECON_MAX_CALLS", 24).clamp(1, 32)),
            generic_max_nested_output_bytes.min(
                env_usize("ANGEL_TASK_RECON_MAX_NESTED_OUTPUT_BYTES", 4 * 1024 * 1024)
                    .clamp(1, 4 * 1024 * 1024),
            ),
        )
    } else if crate::yolo::enabled() {
        (usize::MAX, usize::MAX)
    } else {
        (generic_max_calls, generic_max_nested_output_bytes)
    };
    let nested_calls = std::sync::atomic::AtomicUsize::new(0);
    let nested_output_bytes = std::sync::atomic::AtomicUsize::new(0);

    // Nested dispatch: hook-checked, error-returning (so JS sees a throw), and
    // uncapped per call for full-content in-script processing, but bounded by a
    // cumulative byte budget so a loop cannot turn the isolate into an
    // unaccounted output sink.
    let invoke = |name: &str, a: &Value| -> Result<String, String> {
        if cancel.is_some_and(|cancel| cancel.load(std::sync::atomic::Ordering::Acquire)) {
            return Err("code_mode cancelled".to_string());
        }
        // Synthetic handle ops — not registry tools. Scripts park/rehydrate bulk
        // without materializing it as the code_mode return value.
        if name == "__handle_put" {
            return code_mode_handle_put(a);
        }
        if name == "__handle_get" {
            return code_mode_handle_get(a);
        }
        if !allowed_tools.contains(name) {
            return Err(format!(
                "code_mode tool is not in the bound capability set: {name}"
            ));
        }
        if let Some(policy) = registry
            .effective_interception(Some(CODE_MODE_SEAT))
            .consult(name)
            .denial()
        {
            return Err(format!(
                "code_mode effectful tool `{name}` requires allow_effects=true ({policy})"
            ));
        }
        let child_sequence = match nested_calls.fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| (n < max_calls).then_some(n + 1),
        ) {
            Ok(sequence) => sequence.saturating_add(1),
            Err(_) => {
                return Err(format!(
                    "code_mode nested-call budget exhausted ({max_calls} calls)"
                ));
            }
        };
        let child_id = event_context
            .map(|(outer_id, _)| ToolEventId(format!("{}:{child_sequence}", outer_id.0)));
        if let (Some((_, event_tx)), Some(child_id)) = (event_context, child_id.as_ref()) {
            let _ = event_tx.send(TurnEvent::ToolCall {
                id: child_id.clone(),
                name: name.to_string(),
                args_summary: summarize_args(a),
            });
        }
        let dispatched = if !hooks.is_empty() {
            hooks.pre_tool_use(name, a).map_or_else(
                || registry.dispatch_with_cancel_in_seat(CODE_MODE_SEAT, name, a, cancel),
                Err,
            )
        } else {
            registry.dispatch_with_cancel_in_seat(CODE_MODE_SEAT, name, a, cancel)
        };
        if let Ok(ref out) = dispatched
            && !hooks.is_empty()
        {
            hooks.post_tool_use(name, a, out);
        }
        let r = dispatched.and_then(|out| {
            let produced = out.len();
            let before =
                nested_output_bytes.fetch_add(produced, std::sync::atomic::Ordering::Relaxed);
            if before.saturating_add(produced) > max_nested_output_bytes {
                Err(format!(
                    "code_mode nested-output budget exhausted ({max_nested_output_bytes} B)"
                ))
            } else {
                Ok(out)
            }
        });
        if let (Some((_, event_tx)), Some(child_id)) = (event_context, child_id.as_ref()) {
            let event_result = r
                .as_ref()
                .map(|result| result.to_string())
                .unwrap_or_else(|error| format!("tool error: {error}"));
            let call = ToolCall {
                id: child_id.0.clone(),
                name: name.to_string(),
                args: a.clone(),
            };
            let _ = event_tx.send(TurnEvent::ToolResult {
                id: child_id.clone(),
                name: name.to_string(),
                summary: if event_result.starts_with("tool error:") {
                    crate::secrets::redact_str(&event_result).into_owned()
                } else {
                    summarize_result(&event_result)
                },
                outcome: registry.executed_outcome(&call, &event_result, false),
            });
        }
        r
    };

    // Reuse the ordinary harness scheduler. Mixed batches recover concurrency
    // inside maximal footprint-safe runs, with every effect/conflict retained
    // as a strict declaration-order barrier.
    let batch_scheduler = |specs: &[(String, Value)]| {
        let calls = specs
            .iter()
            .enumerate()
            .map(|(index, (name, args))| ToolCall {
                id: format!("code-mode-{index}"),
                name: name.clone(),
                args: args.clone(),
            })
            .collect::<Vec<_>>();
        batch_segments_with(&calls, false)
    };

    let outcome = crate::code_mode::run(
        script,
        &tool_names,
        &invoke,
        &batch_scheduler,
        std::time::Duration::from_millis(timeout_ms),
        heap_mb,
    );
    let receipt = format!(
        "code_mode receipt: nested_calls={} nested_output_bytes={} allow_effects={allow_effects} recipe={recipe} max_calls={max_calls} max_nested_output_bytes={max_nested_output_bytes} timeout_ms={timeout_ms} policy_denials={}",
        nested_calls.load(std::sync::atomic::Ordering::Relaxed),
        nested_output_bytes.load(std::sync::atomic::Ordering::Relaxed),
        registry.policy_denials().len(),
    );
    let body = match outcome {
        Ok(out) => {
            let result = if out.console.is_empty() {
                out.result
            } else {
                let truncation = if out.console_truncated {
                    " (byte limit reached)"
                } else {
                    ""
                };
                format!(
                    "{}\n--- console ({} line(s){truncation}) ---\n{}",
                    out.result,
                    out.console.len(),
                    out.console.join("\n")
                )
            };
            // Large programmatic returns stay addressable under a handle so the
            // root sees the strategy-level code_mode receipt, not bulk evidence.
            let min_offload = code_mode_offload_min_bytes();
            let identity = format!("code_mode|{recipe}");
            if let Some(handle_receipt) = maybe_offload_root_body(
                &result,
                HandleKind::CodeMode,
                "code_mode",
                &identity,
                min_offload,
            ) {
                format!("{handle_receipt}\n--- {receipt}")
            } else {
                format!("{result}\n--- {receipt}")
            }
        }
        Err(e) => format!("tool error: {e}\n--- {receipt}"),
    };
    // The both-given recovery notice rides the top of the result so the model
    // learns the vocabulary fix without spending an error hop on it.
    match script_recipe_notice {
        Some(notice) => format!("{notice}\n{body}"),
        None => body,
    }
}

/// Advertises the `code_mode` capability. Execution is intercepted in
/// [`dispatch_with_hooks`] (the runtime needs registry re-entry the tool can't
/// reach), so [`Tool::call`] here is only a defensive fallback.
pub struct CodeModeTool {
    description: String,
}

impl CodeModeTool {
    /// Build the tool, embedding the names of the tools that will be bound as
    /// host functions so the model knows the in-script API up front.
    pub fn new(tool_names: &[String]) -> Self {
        let read_api = tool_names
            .iter()
            .filter(|name| is_code_mode_repo_read(name))
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let description = format!(
            "Run a JavaScript (ES2020) program in a sandboxed V8 isolate to \
             orchestrate tools programmatically — loops, conditionals, batching, \
             and filtering in ONE turn instead of many tool round-trips. Each \
             cockpit tool is a global function taking one object argument and \
             returning its text output (throwing on error); calls are synchronous \
             (`await` is optional). `return` your final value (objects are \
             JSON-stringified). `console.log(...)` is byte-capped. There is no \
             network or filesystem except through these tools, which keep their \
             usual sandboxing. Nested calls and their cumulative output are \
             hard-budgeted and reported in an orchestration receipt. Read-only \
             tools are available by default. Mutation or execution additionally \
             requires both outer `allow_effects: true` and operator-controlled \
             `ANGEL_CODE_MODE_EFFECTS=1`; it is quarantined by default. Tool outputs inside \
             the script are not individually truncated. \
             Default repository-read tools: {read_api}. Also `tool(name, args)` \
             dispatches within the operator-authorized capability set, and \
             `batch([{{tool, args}}, ...])` runs maximal footprint-safe \
             segments concurrently around strict effect/conflict barriers → an \
             ordered array of {{ok, output}} (per-item errors captured). \
             Handle intermediates with `handle_put(body, {{producer?, identity?}})` \
             → opaque `hnd_…` and `handle_get(id, {{offset?, max_bytes?}})` for a \
             capped slice — keep bulk out of the script return value. \
             Use `recipe:'repo_recon', query:'the task'` for the fixed, read-only \
             task-conditioned repository map, or fan out custom inspection. Example: \
             `const r = batch(files.map(f => ({{tool:'outline', args:{{path:f}}}}))); \
             return r.filter(x => !x.ok).length + ' failing inspections';`"
        );
        Self { description }
    }
}

/// Short advertised `code_mode` blurb for bounded / competition / tight-window
/// hops. The full isolate/API essay stays on the default interactive set.
pub(crate) fn lean_code_mode_description() -> &'static str {
    "Run JavaScript in a sandboxed V8 isolate to batch tool calls in one hop. \
     Bound tools such as read_file are globals (object in, text out). `return` the result. \
     `recipe:'repo_recon', query:'task'` maps the repo. Nested mutation needs \
     allow_effects and ANGEL_CODE_MODE_EFFECTS=1. Use handle_put/handle_get for bulk."
}

/// Short advertised `code_mode` param blurbs for lean hops. Keys, enum, and
/// default stay identical to the full schema; the never-executable /
/// effects-gate essay does not.
pub(crate) fn lean_code_mode_params() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "script": { "type": "string", "description": "JavaScript; call tools, return the result" },
            "recipe": {
                "type": "string",
                "enum": ["repo_recon"],
                "description": "built-in recipe; exclusive with script"
            },
            "query": { "type": "string", "description": "task text for repo_recon" },
            "allow_effects": {
                "type": "boolean",
                "default": false,
                "description": "request nested mutation"
            }
        },
        "required": []
    })
}

impl Tool for CodeModeTool {
    fn name(&self) -> &str {
        "code_mode"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "code_mode".to_string(),
            description: self.description.clone(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "script": {
                        "type": "string",
                        "description": "JavaScript to execute. Call bound tools as \
                            functions; `return` the final value."
                    },
                    "recipe": {
                        "type": "string",
                        "enum": ["repo_recon"],
                        "description": "Trusted built-in orchestration recipe. Mutually exclusive with `script`."
                    },
                    "query": {
                        "type": "string",
                        "description": "Task text used to condition `repo_recon`; encoded as data, never executable code."
                    },
                    "allow_effects": {
                        "type": "boolean",
                        "default": false,
                        "description": "Request nested mutation or execution tools; also requires operator ANGEL_CODE_MODE_EFFECTS=1."
                    }
                },
                "required": []
            }),
        }
    }

    fn call(&self, _args: &Value) -> Result<String, String> {
        // Real execution goes through dispatch_with_hooks; reaching here means a
        // caller bypassed it (e.g. a direct registry.dispatch in a test).
        Err("code_mode must be dispatched via the turn loop (dispatch_with_hooks)".to_string())
    }
}

/// `handle_put` from inside a code_mode script: store body, return opaque id.
fn code_mode_handle_put(args: &Value) -> Result<String, String> {
    if !handle_store_enabled() {
        return Err("handle store disabled (ANGEL_HANDLE_STORE=0)".into());
    }
    let body = args.get("body").and_then(Value::as_str).unwrap_or("");
    let producer = args
        .get("producer")
        .and_then(Value::as_str)
        .unwrap_or("code_mode");
    let identity = args
        .get("identity")
        .and_then(Value::as_str)
        .unwrap_or("code_mode|put");
    let receipt = session_put(
        body,
        PutMeta {
            kind: HandleKind::CodeMode,
            producer,
            identity: Some(identity),
            paths: &[],
            include_preview: false,
        },
    )
    .ok_or_else(|| "handle_put failed (store full or empty body policy)".to_string())?;
    Ok(receipt.handle.as_str().to_string())
}

/// `handle_get` from inside a code_mode script: capped disclose.
fn code_mode_handle_get(args: &Value) -> Result<String, String> {
    let handle = args
        .get("handle")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "handle_get requires handle".to_string())?;
    if !handle.starts_with("hnd_") {
        return Err(format!("invalid handle id: {handle}"));
    }
    let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let max_bytes = args
        .get("max_bytes")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or_else(|| HandleStoreLimits::from_env().max_disclose_bytes);
    let slice = session_disclose(handle, offset, max_bytes)?;
    Ok(slice.render())
}

/// Register `code_mode` over the registry's current tools. **On by default**
/// (`ANGEL_CODE_MODE=0` disables it) — it's a first-class capability for this
/// system (coding + mass testing). Registration is cheap; V8 only initialises on
/// the first actual `code_mode` call. Call this *last*, after every other tool is
/// registered, so the bound-tool list baked into the description is complete.
pub(crate) fn maybe_register_code_mode(r: &mut ToolRegistry) {
    if env_flag("ANGEL_CODE_MODE", true) {
        let names = r.bindable_tool_names();
        r.register(Box::new(CodeModeTool::new(&names)));
    }
}

#[cfg(test)]
mod recipe_tests {
    use super::*;

    #[test]
    fn code_mode_repo_recon_query_is_encoded_as_data() {
        let script =
            repo_recon_script("needle'); return shell({command:'touch escaped'}); // NeedleWidget")
                .unwrap();
        assert!(!script.contains("shell("));
        assert!(!script.contains("touch escaped"));
        assert!(script.contains("needlewidget"));
        assert!(script.len() < 16 * 1024);
    }

    #[test]
    fn code_mode_repo_recon_terms_are_bounded_and_deterministic() {
        let query = "repair cockpit/src/harness/turn.rs repeated read search telemetry telemetry";
        let first = repo_recon_terms(query);
        assert_eq!(first, repo_recon_terms(query));
        assert!(first.len() <= REPO_RECON_MAX_TERMS);
        assert!(first.contains(&"cockpit/src/harness/turn.rs".to_string()));
    }

    #[test]
    fn code_mode_repo_recon_drops_task_fluff_stopwords() {
        let terms = repo_recon_terms(
            "please help implement improve task only just something MutationThrashNudge",
        );
        for stop in [
            "please",
            "help",
            "implement",
            "improve",
            "task",
            "only",
            "just",
            "something",
        ] {
            assert!(
                !terms.iter().any(|t| t == stop),
                "stopword {stop} should not be a recon term, got {terms:?}"
            );
        }
        assert!(
            terms
                .iter()
                .any(|t| t.contains("mutation") || t.contains("thrash") || t.contains("nudge")),
            "identifier should survive fluff strip: {terms:?}"
        );
    }

    #[test]
    fn code_mode_repo_recon_program_prefers_impl_paths() {
        let script = repo_recon_script("MutationThrashNudge thrash").unwrap();
        assert!(
            script.contains("implPathBonus"),
            "recon program should rank implementing libraries above docs/testdata"
        );
        assert!(script.contains("docs|testdata|fixtures"));
        assert!(script.contains("src|lib|pkg|core"));
    }
}
