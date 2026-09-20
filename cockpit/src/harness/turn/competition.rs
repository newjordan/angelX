use super::classify::board_marker_as_token;
use super::*;
use std::borrow::Cow;
/// True when the operator turn (or handoff/podrace injection) is a ranked
/// competition: submit/score is the only real progress.
pub(crate) fn competition_mode_active(history: &[ChatMsg]) -> bool {
    competition_mode_trigger(history).is_some()
}

/// What engaged competition mode, if anything. Engagement is EXPLICIT only:
/// an env flag or a deliberate launch phrase in the current operator turn (the
/// podrace/handoff loop injections, or the operator literally asking for
/// competition mode).
/// Ambient competition vocabulary ("b200", "leaderboard", "popcorn",
/// "gpu mode", submission talk) never arms it — a fresh session that merely
/// *mentions* a leaderboard is not entering a race, and the tight recon
/// budget that rides along must not gum up ordinary work.
pub(crate) fn competition_mode_trigger(history: &[ChatMsg]) -> Option<&'static str> {
    if env_flag("ANGEL_COMPETITION_MODE", false) {
        return Some("ANGEL_COMPETITION_MODE");
    }
    // GpuComp / popcorn peer reward is itself a competition contract.
    if env_flag("ANGEL_GPU_COMP_LOCAL_MOA", false) {
        return Some("ANGEL_GPU_COMP_LOCAL_MOA");
    }
    // Only the current operator turn may arm this turn. Scanning a history
    // window made a paused competition session sticky: an ordinary follow-up
    // such as "hey, how goes?" inherited an old `competition-loop` marker and
    // the harness answered it with race cadence instead of conversation.
    // Autonomous drivers already inject their launch phrase on every turn; an
    // explicit env flag remains the durable opt-in for operators who want it.
    // Scan ASCII in place so ordinary turns do not copy a multi-KB prompt
    // just to hunt "competition-loop" / "FIRE NOW".
    let blob = history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .map(|message| message.content.as_ref())
        .unwrap_or_default();
    let trigger = [
        // Deliberate launch phrases only: typed by the operator or injected by
        // the podrace/handoff loop drivers — never incidental vocabulary.
        "competition mode",
        "competition-loop",
        "competition loop",
        "handoff rl",
        "handoff-rl",
        "hit it chewy",
        "podrace",
        // Operator fire imperatives: a mid-run "FIRE NOW" is the most
        // deliberate competition arming there is. Before these, the message
        // demanding action was exactly what disarmed the cadence (2026-08-15
        // flock incident: every angry steer reverted the turn to ungoverned
        // conversation mode).
        "fire now",
        "fire one off",
        "fire a submission",
        "submit now",
        "submission lane hot",
        "submit and",
        "points on the board",
        "rolling lanes",
        "when competing",
    ]
    .into_iter()
    .find(|needle| ascii_contains_ignore_case(blob, needle));
    if trigger.is_some() {
        return trigger;
    }
    // The standing /goal is durable operator intent and is re-injected by the
    // harness every turn. A competitive objective keeps the cadence armed
    // across the whole session, so a launch-phrase-free steer ("stop
    // overthinking") can no longer disarm it — while sessions with no goal or
    // a non-competitive goal keep the anti-sticky behavior above.
    let goal = history
        .iter()
        .rev()
        .find(|message| {
            message.role == ChatRole::Harness
                && message.content.contains("[goal — standing objective")
        })
        .map(|message| message.content.as_ref())
        .unwrap_or_default();
    if goal.is_empty() {
        return None;
    }
    [
        "competition",
        "compete",
        "leaderboard",
        "submission",
        "podrace",
        "winning",
    ]
    .into_iter()
    .find(|needle| ascii_contains_ignore_case(goal, needle))
    .map(|_| "competition-goal")
}

/// True when `cmd` is an `ls` (possibly path-qualified / wrapped) with recursive
/// listing flags. Detects combined short clusters (`-laR`, `-lR`, `-1R`) and
/// long-form `--recursive` / `--recurse`, not only the bare `ls -r` substring.
pub(crate) fn ls_cmd_is_recursive_listing(cmd: &str) -> bool {
    let mut saw_ls = false;
    for part in cmd.split_whitespace() {
        let base = part.rsplit('/').next().unwrap_or(part);
        // Skip env assignments already stripped at the head; mid-command `FOO=1`
        // is rare and not treated as ls.
        if base.contains('=') && !base.starts_with('-') {
            continue;
        }
        if !base.starts_with('-') {
            if base == "ls" {
                saw_ls = true;
            }
            continue;
        }
        if !saw_ls {
            continue;
        }
        if base.starts_with("--") {
            if base == "--recursive"
                || base == "--recurse"
                || base.starts_with("--recursive=")
                || base.starts_with("--recurse=")
            {
                return true;
            }
            continue;
        }
        // Short cluster: any `r` after lowercasing (covers `-R` and accidental `-r`).
        if base.chars().skip(1).any(|c| c == 'r') {
            return true;
        }
    }
    false
}

/// Shell inventory / recursive search that must burn first-write budget even
/// when the argv also names a living-board path.
fn shell_cmd_is_inventory_thrash(cmd: &str) -> bool {
    let cmd = cmd.trim_start();
    // Leading env assignments: `FOO=1 rg …`
    let cmd = {
        let mut rest = cmd;
        while let Some((head, tail)) = rest.split_once(char::is_whitespace) {
            if head.contains('=') && !head.starts_with('-') {
                rest = tail.trim_start();
                continue;
            }
            break;
        }
        rest
    };
    let thrash_substrings = [
        "find ",
        "find\t",
        "find/",
        "rg ",
        "rg\t",
        "grep ",
        "grep\t",
        "egrep ",
        "fgrep ",
        "git grep",
        // Git inventory / history — never a cheap board digest even when argv
        // names living-handoff / board.md (A4: used to launder as wait/poll).
        "git ls-files",
        "git ls-tree",
        "git blame",
        "git log",
        // A8: `git diff` / `git status` naming a board path laundered as legal
        // wait/poll (unlike ls-files/blame/log) and skipped first-write burn.
        "git diff",
        "git status",
        // A4: shortlog / rev-list / reflog naming board paths also laundered.
        "git shortlog",
        "git rev-list",
        "git reflog",
        // A8: commit-walk inventory (not a tip blob digest).
        "git whatchanged",
        "ag ",
        "ag\t",
        "ack ",
        "fd ",
        "fd\t",
        "tree ",
        "locate ",
        // Full binary names (not only `rg ` / `grep ` prefixes).
        "ripgrep ",
        "ripgrep\t",
        "ugrep ",
        "ugrep\t",
    ];
    if thrash_substrings.iter().any(|t| cmd.contains(t)) {
        return true;
    }
    // A8: `git show --stat` / pathspec forms used to launder as board wait while
    // only `git show REV:path` blob digests are legal tip reads.
    if git_show_is_inventory_thrash(cmd) {
        return true;
    }
    // Recursive directory listing — combined short flags (`ls -laR`, `ls -lR`,
    // `ls -1R`) used to miss the bare `ls -r` substring and launder as board wait
    // when argv also named a living-handoff path (A4). Lowercasing maps `-R`→`-r`.
    if ls_cmd_is_recursive_listing(cmd) {
        return true;
    }
    // Command word at the start (or after `sudo` / shell wrappers).
    let first = cmd
        .split(|c: char| c.is_whitespace() || c == '|')
        .map(str::trim)
        .find(|p| !p.is_empty() && !p.contains('='))
        .unwrap_or("");
    let first = first.rsplit('/').next().unwrap_or(first);
    matches!(
        first,
        "find"
            | "rg"
            | "ripgrep"
            | "grep"
            | "egrep"
            | "fgrep"
            | "ugrep"
            | "ag"
            | "ack"
            | "fd"
            | "tree"
            | "locate"
    ) || first == "git"
        && (cmd.contains("grep")
            || cmd.contains("ls-files")
            || cmd.contains("ls-tree")
            || shell_argv_has_token(cmd, "blame")
            || shell_argv_has_token(cmd, "log")
            || shell_argv_has_token(cmd, "diff")
            || shell_argv_has_token(cmd, "status")
            || shell_argv_has_token(cmd, "shortlog")
            || shell_argv_has_token(cmd, "rev-list")
            || shell_argv_has_token(cmd, "reflog")
            || shell_argv_has_token(cmd, "whatchanged")
            || git_show_is_inventory_thrash(cmd))
}

/// `git show HEAD:path` is a legal board blob digest. Listing / pathspec forms
/// (`--stat`, `--name-only`, `REV -- path`) are history inventory and must burn
/// first-write budget and count as anti-spin thrash (A8).
fn git_show_is_inventory_thrash(cmd: &str) -> bool {
    if !shell_argv_has_token(cmd, "show") || !shell_argv_has_token(cmd, "git") {
        return false;
    }
    // Listing / summary flags — never a pure tip blob read.
    const LIST_FLAGS: &[&str] = &[
        "--stat",
        "--name-only",
        "--name-status",
        "--numstat",
        "--shortstat",
        "--dirstat",
        "--summary",
        "--raw",
    ];
    if LIST_FLAGS.iter().any(|f| cmd.contains(f)) {
        return true;
    }
    // Pathspec form: `git show REV -- path` (commit + diff), not `REV:path`.
    if cmd.contains(" -- ") {
        return true;
    }
    // Blob digests use a single `rev:path` argv word (HEAD:board.md).
    let has_blob = cmd.split_whitespace().any(|part| {
        if part.starts_with('-') {
            return false;
        }
        match part.split_once(':') {
            Some((rev, path)) => {
                !rev.is_empty()
                    && !path.is_empty()
                    && !rev.contains('/') // skip URLs / host:path
                    && !part.contains("://")
            }
            None => false,
        }
    });
    // show without a blob form is commit/history inventory when a board path is
    // named as a separate path argument (or a plain commit dump).
    !has_blob
}

/// True when `token` appears as its own shell argv word (split on whitespace and
/// common shell punctuation), not as a path/file substring (`src/score.rs`,
/// `submit_form.html`, prose "still validating").
/// Compares ASCII case in place so classify hops do not copy each argv word.
pub(crate) fn shell_argv_has_token(command: &str, token: &str) -> bool {
    command
        .split(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '/' | '\\'
                        | '|'
                        | '&'
                        | ';'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '<'
                        | '>'
                        | '`'
                        | '"'
                        | '\''
                        | '='
                        | ','
                        | ':'
                )
        })
        .any(|part| {
            let part = part.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
            part.eq_ignore_ascii_case(token)
                || part
                    .strip_prefix("--")
                    .is_some_and(|flag| flag.eq_ignore_ascii_case(token))
        })
}

/// Product payload keys. Classifiers must not serialize or scan these —
/// they are megabyte write/patch bodies, and source text must not launder
/// as submit/wait (A8 leftover).
pub(crate) fn is_competition_payload_key(tool: &str, key: &str) -> bool {
    match key {
        "content" | "contents" | "old" | "new" | "old_string" | "new_string" | "replacement"
        | "patch" | "diff" => true,
        "input" => tool.eq_ignore_ascii_case("apply_patch"),
        "script" | "body" => tool.eq_ignore_ascii_case("code_mode"),
        _ => false,
    }
}

fn push_classify_lower(hay: &mut String, s: &str) {
    hay.reserve(s.len().saturating_add(1));
    hay.push(' ');
    hay.extend(s.chars().map(|c| c.to_ascii_lowercase()));
}

fn append_classification_value(tool: &str, value: &Value, hay: &mut String) {
    match value {
        Value::Null => {}
        Value::Bool(b) => {
            hay.push(' ');
            hay.push_str(if *b { "true" } else { "false" });
        }
        Value::Number(n) => {
            hay.push(' ');
            hay.push_str(&n.to_string());
        }
        Value::String(s) => push_classify_lower(hay, s),
        Value::Array(items) => {
            for item in items {
                append_classification_value(tool, item, hay);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                if is_competition_payload_key(tool, k) {
                    continue;
                }
                push_classify_lower(hay, k);
                append_classification_value(tool, v, hay);
            }
        }
    }
}

/// Borrow the one remaining classify string when args are a single
/// non-payload string (the usual `path`). Shell still needs a built hay.
/// Classify hops must not copy that path.
pub(crate) fn classify_hay_borrow(call: &ToolCall) -> Option<&str> {
    if call.name.eq_ignore_ascii_case("shell") {
        return None;
    }
    if call.name.eq_ignore_ascii_case("apply_patch") {
        return classify_apply_patch_borrow(call);
    }
    let Value::Object(map) = &call.args else {
        return None;
    };
    let mut found = None;
    for (key, value) in map {
        if is_competition_payload_key(call.name.as_str(), key) {
            continue;
        }
        match value {
            Value::Null => {}
            Value::String(s) => {
                if found.is_some() {
                    return None;
                }
                found = Some(s.as_str());
            }
            _ => return None,
        }
    }
    found.filter(|s| !s.is_empty())
}

/// Borrow the single apply_patch target path from the diff header.
/// Multi-file patches still build a hay.
pub(crate) fn classify_apply_patch_borrow(call: &ToolCall) -> Option<&str> {
    if !call.name.eq_ignore_ascii_case("apply_patch") {
        return None;
    }
    let top = mutation_arg_path(&call.args);
    let found = call
        .args
        .get("diff")
        .and_then(Value::as_str)
        .and_then(crate::cut::apply_patch_single_target_path);
    match (top, found) {
        (Some(top), Some(found)) if top == found => Some(top),
        (Some(top), None) => Some(top),
        (None, Some(found)) => Some(found),
        _ => None,
    }
}

/// Shell command text, or non-payload arg strings — one hay shared by
/// competition classifiers on the hop hot path (A7). Mutation payloads
/// (write content, patch hunks, code_mode scripts) stay out of the hay
/// so ordinary hops do not allocate or scan megabyte bodies.
///
/// True when a shell command must be lowercased for classify/preflight
/// `contains` needles. Path-like tokens (`src/x`, `LIVING_HANDOFF.md`)
/// already match case-insensitively — do not copy those hops.
pub(crate) fn classify_shell_hay_needs_lower(cmd: &str) -> bool {
    cmd.split_whitespace().any(|token| {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if token.is_empty() || token.contains('/') || token.contains('\\') || token.contains('.') {
            return false;
        }
        token.as_bytes().iter().any(|b| b.is_ascii_uppercase())
    })
}

/// Borrow an already-lowercase (or path-only-uppercase) shell command.
/// Mixed-case program words still fold so `GIT DIFF` / `SUBMIT` match.
pub(crate) fn classify_shell_hay(cmd: &str) -> Cow<'_, str> {
    if classify_shell_hay_needs_lower(cmd) {
        Cow::Owned(cmd.to_ascii_lowercase())
    } else {
        Cow::Borrowed(cmd)
    }
}

/// The tool name is borrowed. A single remaining path is borrowed.
/// A lowercase / path-cased shell command is borrowed.
/// Classify hops must not copy those to lowercase.
pub(crate) fn competition_call_text(call: &ToolCall) -> (&str, Cow<'_, str>, bool) {
    let name = call.name.as_str();
    if name.eq_ignore_ascii_case("shell") {
        let cmd = crate::tools::shell::shell_command_arg(&call.args).unwrap_or("");
        (name, classify_shell_hay(cmd), true)
    } else if let Some(borrowed) = classify_hay_borrow(call) {
        (name, Cow::Borrowed(borrowed), false)
    } else {
        let mut hay = String::new();
        append_classification_value(name, &call.args, &mut hay);
        // apply_patch rarely has a top-level path; keep the extracted
        // targets so meta-only patches still classify as board wait.
        if name.eq_ignore_ascii_case("apply_patch") {
            if let Some(path) = mutation_arg_path(&call.args) {
                push_classify_lower(&mut hay, path);
            }
            crate::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
                push_classify_lower(&mut hay, path.trim());
                false
            });
        }
        (name, Cow::Owned(hay), false)
    }
}

/// Shell/platform calls that count as competition progress (submit or score).
///
/// A8: bare substrings like `score` / `submit` / `validating` used to match
/// ordinary recon (`read_file src/score.rs`, write_file content "still
/// validating") and launder those hops as competition wait — they no longer
/// burned first-write budget and did not count as action starvation.
#[cfg(test)]
pub(crate) fn is_competition_outcome_call(call: &ToolCall) -> bool {
    let (name, hay, is_shell) = competition_call_text(call);
    is_competition_outcome_hay(name, &hay, is_shell)
}

fn is_competition_outcome_hay(name: &str, hay: &str, is_shell: bool) -> bool {
    // Multi-word / CLI-specific patterns — safe on any tool shape.
    const STRONG: &[&str] = &[
        "leaderboard",
        "submissions",
        "hilbert status",
        "hilbert submissions",
        "hilbert list",
        "hilbert submit",
        "yukon submissions",
        "yukon submit",
        "popcorn status",
        "popcorn-cli status",
        "popcorn submit",
        "--mode benchmark",
        "--mode leaderboard",
    ];
    if STRONG.iter().any(|needle| hay.contains(needle)) {
        return true;
    }

    // Bare submit/score only as shell argv words — never path fragments or prose.
    if is_shell && (shell_argv_has_token(hay, "submit") || shell_argv_has_token(hay, "score")) {
        return true;
    }
    // Non-shell tools: only the strong multi-word markers above.
    let _ = name;
    false
}

/// Cheap board-state reads that must not burn the pre-edit inspection budget.
/// Shared-slot competitions force the model to re-derive tip/handoff/slot status;
/// counting those as "inspection thrash" made legal wait moves illegal.
#[cfg(test)]
pub(crate) fn is_competition_board_state_call(call: &ToolCall) -> bool {
    let (name, hay_body, is_shell) = competition_call_text(call);
    is_competition_board_state_hay(name, &hay_body, is_shell)
}

fn is_competition_board_state_hay(name: &str, hay_body: &str, is_shell: bool) -> bool {
    // Inventory / content-search thrash is never a "board digest", even if a
    // handoff path is named. A4: bare `grep` (not only `grep -r`), `git grep`,
    // `ag`, and `fd` used to launder workspace recon as legal wait.
    //
    // Native tools (`grep`, `find_files`, …) are the same class of thrash:
    // only shell digests (`cat`/`head`) and direct `read_file` of board paths
    // are legal wait/poll. Searching *through* a handoff path is recon.
    if is_inventory_or_recon_thrash(name, hay_body, is_shell) {
        return false;
    }
    // Name and body are searched separately so we do not allocate `name + hay`.
    hay_names_board_meta(name) || hay_names_board_meta(hay_body)
}

/// True when `hay` names a living-board digest token (`LIVING_HANDOFF.md`,
/// `/tmp/living…`). Product identifiers (`living_handoff_parser.rs`) do not
/// match. Used by board-wait classify and by git-diff preflight so a
/// board-path diff cannot launder runner escalation.
pub(crate) fn hay_names_board_meta(hay: &str) -> bool {
    const MARKERS: &[&str] = &[
        "living_handoff",
        "living-handoff",
        "living handoff",
        "board tip",
        "board_tip",
        "board-tip",
        "slot status",
        "slot_status",
        "in-flight",
        "in_flight",
        "handoff head",
        "handoff_head",
        "/tmp/living",
        "living-log",
        "living_log",
        "handoff.md",
        "board.md",
        "tip.md",
        "slot.json",
        "submissions.json",
    ];
    MARKERS.iter().any(|m| board_marker_as_token(hay, m))
}

/// Shell inventory thrash **or** native recon tools that must never count as
/// competition board wait (even when args name a living-handoff path).
fn is_inventory_or_recon_thrash(name: &str, hay_body: &str, is_shell: bool) -> bool {
    if is_shell {
        return shell_cmd_is_inventory_thrash(hay_body);
    }
    // Structured workspace search / symbol tools (native + LSP).
    // A4: `lsp_workspace_symbol` / `lsp_symbols` used to launder as board wait
    // when the query or path merely *mentioned* a handoff marker.
    const INVENTORY_TOOLS: &[&str] = &[
        "grep",
        "find_files",
        "file_search",
        "outline",
        "defs",
        "semantic_read",
        "repo_search",
        "list_dir",
        "lsp_workspace_symbol",
        "lsp_symbols",
        "lsp_references",
        "lsp_definition",
        // A8: native git inventory mirrors shell `git diff|status|log` thrash.
        // Path args naming a living-handoff used to launder as board wait
        // (no first-write burn, no anti-spin count).
        "git_diff",
        "git_status",
        "git_log",
    ];
    if INVENTORY_TOOLS
        .iter()
        .any(|tool| name.eq_ignore_ascii_case(tool))
    {
        return true;
    }
    // code_mode repo_recon is a multi-grep inventory recipe, not a board poll.
    if name.eq_ignore_ascii_case("code_mode")
        && (ascii_contains_ignore_case(hay_body, "repo_recon")
            || ascii_contains_ignore_case(hay_body, "\"recipe\":\"repo_recon\"")
            || ascii_contains_ignore_case(hay_body, "\"recipe\": \"repo_recon\""))
    {
        return true;
    }
    false
}

/// Competition wait/poll: outcome (submit/score/status) or board-state digest.
/// These never burn first-write budget and never count as action starvation.
pub(crate) fn is_competition_wait_or_progress_call(call: &ToolCall) -> bool {
    // One hay allocation for both classifiers (hop hot path).
    let (name, hay, is_shell) = competition_call_text(call);
    is_competition_outcome_hay(name, &hay, is_shell)
        || is_competition_board_state_hay(name, &hay, is_shell)
}

/// Free-form recon that spends the pre-edit inspection budget.
pub(crate) fn burns_first_write_budget(call: &ToolCall) -> bool {
    !is_first_write_progress_call(call) && !is_competition_wait_or_progress_call(call)
}

/// Single-pass hop flags for first-write / competition budget (A7: avoid three
/// separate `calls.iter().any(...)` walks that re-serialize args each time).
pub(crate) fn hop_budget_flags(calls: &[ToolCall]) -> (bool, bool, bool, bool) {
    let mut mutation = false;
    let mut outcome = false;
    let mut wait = false;
    let mut burns = false;
    for call in calls {
        let progress = is_first_write_progress_call(call);
        mutation |= progress;
        let (name, hay, is_shell) = competition_call_text(call);
        let is_outcome = is_competition_outcome_hay(name, &hay, is_shell);
        outcome |= is_outcome;
        let is_wait = is_outcome || is_competition_board_state_hay(name, &hay, is_shell);
        wait |= is_wait;
        burns |= !progress && !is_wait;
    }
    (mutation, outcome, wait, burns)
}

/// True when the hop loop needs competition/first-write hay.
/// Default cockpit (no competition, no first-write budget, no in-flight slot)
/// must not serialize args to hunt board/submit markers.
pub(crate) fn hop_budget_classify_applied(
    competition: bool,
    first_write_limit: usize,
    hop_path_active: bool,
) -> bool {
    competition || first_write_limit > 0 || hop_path_active
}

/// Hop-loop flags. When classify is off, every batch is spin-countable and
/// calls are not walked — mutation/wait/outcome stay unused on that path.
pub(crate) fn hop_budget_flags_for_loop(
    apply: bool,
    calls: &[ToolCall],
) -> (bool, bool, bool, bool) {
    if apply {
        hop_budget_flags(calls)
    } else {
        (false, false, false, true)
    }
}
