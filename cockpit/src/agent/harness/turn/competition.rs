use super::classify::board_marker_as_token;
use super::*;
use std::borrow::Cow;

/// The shipped Bend leg that owns these decisions. The native bodies below stay
/// as the fallback and the parity oracle.
const LEG: &str = "competition-mode";

/// Deliberate launch phrases only, in the order the native scan tries them:
/// typed by the operator or injected by the podrace/handoff loop drivers — never
/// incidental vocabulary. The leg answers with an index into this table, so no
/// phrase text crosses into Bend.
const LAUNCH_PHRASES: &[&str] = &[
    "competition mode",
    "competition-loop",
    "competition loop",
    "handoff rl",
    "handoff-rl",
    "hit it chewy",
    "podrace",
    // Operator fire imperatives: a mid-run "FIRE NOW" is the most deliberate
    // competition arming there is (2026-08-15 flock incident).
    "fire now",
    "fire one off",
    "fire a submission",
    "submit now",
    "submission lane hot",
    "submit and",
    "points on the board",
    "rolling lanes",
    "when competing",
];

/// The vocabulary that makes a standing `/goal` competitive.
const GOAL_PHRASES: &[&str] = &[
    "competition",
    "compete",
    "leaderboard",
    "submission",
    "podrace",
    "winning",
];

/// Command words that are inventory or search rather than a board digest.
const INVENTORY_WORDS: &[&str] = &[
    "find", "rg", "ripgrep", "grep", "egrep", "fgrep", "ugrep", "ag", "ack", "fd", "tree",
    "locate",
];

/// The `git` inventory subcommands, in the native scan's order. The first three
/// are plain substrings there (so `git grep`, `git ls-files`, `git ls-tree`
/// count); the rest are whole argv words.
const GIT_ARG_WORDS: &[&str] = &[
    "grep",
    "ls-files",
    "ls-tree",
    "blame",
    "log",
    "diff",
    "status",
    "shortlog",
    "rev-list",
    "reflog",
    "whatchanged",
];
const GIT_ARG_SUBSTRING_COUNT: usize = 3;

/// `git show` listing / summary flags: history inventory, never a tip blob read.
const SHOW_LIST_FLAGS: &[&str] = &[
    "--stat",
    "--name-only",
    "--name-status",
    "--numstat",
    "--shortstat",
    "--dirstat",
    "--summary",
    "--raw",
];

/// Shell inventory: searches, listings and git history, including the full
/// binary names and the `find`/`rg`/`fd` tab forms.
const THRASH_SUBSTRINGS: &[&str] = &[
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
    "git ls-files",
    "git ls-tree",
    "git blame",
    "git log",
    "git diff",
    "git status",
    "git shortlog",
    "git rev-list",
    "git reflog",
    "git whatchanged",
    "ag ",
    "ag\t",
    "ack ",
    "fd ",
    "fd\t",
    "tree ",
    "locate ",
    "ripgrep ",
    "ripgrep\t",
    "ugrep ",
    "ugrep\t",
];

/// True when the operator turn (or handoff/podrace injection) is a ranked
/// competition: submit/score is the only real progress.
pub(crate) fn competition_mode_active(history: &[ChatMsg]) -> bool {
    competition_mode_trigger_live(history).is_some()
}

/// The native engagement answer, kept for the parity oracle.
#[cfg(test)]
pub(crate) fn competition_mode_active_native(history: &[ChatMsg]) -> bool {
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
///
/// The native scan, kept unchanged as this leg's parity oracle.
#[cfg(test)]
pub(crate) fn competition_mode_trigger(history: &[ChatMsg]) -> Option<&'static str> {
    if env_flag("ANGEL_COMPETITION_MODE", false) {
        return Some("ANGEL_COMPETITION_MODE");
    }
    // Only the current operator turn may arm this turn. Scanning a history
    // window made a paused competition session sticky: an ordinary follow-up
    // such as "hey, how goes?" inherited an old `competition-loop` marker and
    // the harness answered it with race cadence instead of conversation.
    // Autonomous drivers already inject their launch phrase on every turn; an
    // explicit env flag remains the durable opt-in for operators who want it.
    // Scan ASCII in place so ordinary turns do not copy a multi-KB prompt
    // just to hunt "competition-loop" / "FIRE NOW".
    let blob = operator_turn_text(history);
    // Table order is the scan order: the first matching phrase is the trigger.
    let trigger = LAUNCH_PHRASES
        .iter()
        .copied()
        .find(|needle| ascii_contains_ignore_case(blob, needle));
    if trigger.is_some() {
        return trigger;
    }
    // The standing /goal is durable operator intent and is re-injected by the
    // harness every turn. A competitive objective keeps the cadence armed
    // across the whole session, so a launch-phrase-free steer ("stop
    // overthinking") can no longer disarm it — while sessions with no goal or
    // a non-competitive goal keep the anti-sticky behavior above.
    let goal = goal_text(history);
    if goal.is_empty() {
        return None;
    }
    GOAL_PHRASES
        .iter()
        .copied()
        .find(|needle| ascii_contains_ignore_case(goal, needle))
        .map(|_| "competition-goal")
}

/// The current operator turn's text, or empty when there is none.
fn operator_turn_text(history: &[ChatMsg]) -> &str {
    history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .map(|message| message.content.as_ref())
        .unwrap_or_default()
}

/// The standing goal block the harness re-injects, or empty when it published
/// none.
fn goal_text(history: &[ChatMsg]) -> &str {
    history
        .iter()
        .rev()
        .find(|message| {
            message.role == ChatRole::Harness
                && message.content.contains(crate::drive::goal::GOAL_BLOCK_HEADER)
        })
        .map(|message| message.content.as_ref())
        .unwrap_or_default()
}

/// Everything the trigger decision reads: the env flag, the launch phrase flags
/// and the competitive-goal vocabulary flags, each in table order.
struct TriggerFacts {
    env: bool,
    phrases: Vec<bool>,
    goal_present: bool,
    goal_phrases: Vec<bool>,
}

fn trigger_facts(history: &[ChatMsg]) -> TriggerFacts {
    let blob = operator_turn_text(history);
    let goal = goal_text(history);
    TriggerFacts {
        env: env_flag("ANGEL_COMPETITION_MODE", false),
        phrases: LAUNCH_PHRASES
            .iter()
            .map(|needle| ascii_contains_ignore_case(blob, needle))
            .collect(),
        goal_present: !goal.is_empty(),
        goal_phrases: GOAL_PHRASES
            .iter()
            .map(|needle| ascii_contains_ignore_case(goal, needle))
            .collect(),
    }
}

impl TriggerFacts {
    fn input(&self) -> serde_json::Value {
        serde_json::json!({
            "op": "trigger",
            "env": self.env,
            "phrases": self.phrases,
            "goal_present": self.goal_present,
            "goal_phrases": self.goal_phrases,
        })
    }
}

/// The leg answers with an enum index; the trigger string never leaves Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TriggerKind {
    Env,
    Phrase,
    Goal,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TriggerDecision {
    trigger: TriggerKind,
    index: Option<usize>,
}

impl TriggerDecision {
    #[cfg(test)]
    fn of(trigger: Option<&'static str>) -> Self {
        match trigger {
            None => Self {
                trigger: TriggerKind::None,
                index: None,
            },
            Some("ANGEL_COMPETITION_MODE") => Self {
                trigger: TriggerKind::Env,
                index: None,
            },
            Some("competition-goal") => Self {
                trigger: TriggerKind::Goal,
                index: None,
            },
            Some(phrase) => Self {
                trigger: TriggerKind::Phrase,
                index: LAUNCH_PHRASES.iter().position(|needle| *needle == phrase),
            },
        }
    }

    fn back(self) -> Option<&'static str> {
        match self.trigger {
            TriggerKind::Env => Some("ANGEL_COMPETITION_MODE"),
            TriggerKind::Goal => Some("competition-goal"),
            TriggerKind::Phrase => self
                .index
                .and_then(|index| LAUNCH_PHRASES.get(index))
                .copied(),
            TriggerKind::None => None,
        }
    }

    /// The leg may not report a phrase the turn did not match, skip an earlier
    /// one, or engage on an unset flag: the native scan takes the first hit in
    /// table order, after the env flag.
    fn check(&self, facts: &TriggerFacts) -> Result<(), String> {
        if self.index.is_some() && self.trigger != TriggerKind::Phrase {
            return Err("live competition-mode leg reported a phrase index for no phrase".into());
        }
        match self.trigger {
            TriggerKind::None => {
                if facts.env {
                    return Err("live competition-mode leg ignored the env flag".into());
                }
                if facts.phrases.iter().any(|hit| *hit) {
                    return Err("live competition-mode leg ignored a launch phrase".into());
                }
                if facts.goal_present && facts.goal_phrases.iter().any(|hit| *hit) {
                    return Err("live competition-mode leg ignored a competitive goal".into());
                }
                Ok(())
            }
            TriggerKind::Env => {
                if !facts.env {
                    return Err("live competition-mode leg engaged on an unset env flag".into());
                }
                Ok(())
            }
            TriggerKind::Goal => {
                if !facts.goal_present || !facts.goal_phrases.iter().any(|hit| *hit) {
                    return Err("live competition-mode leg engaged on a non-competitive goal".into());
                }
                Ok(())
            }
            TriggerKind::Phrase => {
                let index = self
                    .index
                    .ok_or("live competition-mode leg reported a phrase without its index")?;
                let hit = facts.phrases.get(index).copied().ok_or_else(|| {
                    format!("live competition-mode leg named phrase {index} beyond the table")
                })?;
                if !hit {
                    return Err(format!(
                        "live competition-mode leg named phrase {index}, which the turn did not match"
                    ));
                }
                if facts.phrases[..index].iter().any(|earlier| *earlier) {
                    return Err(format!(
                        "live competition-mode leg skipped an earlier launch phrase for {index}"
                    ));
                }
                Ok(())
            }
        }
    }
}

/// The routed engagement answer: the shipped leg decides.
pub(crate) fn competition_mode_trigger_live(history: &[ChatMsg]) -> Option<&'static str> {
    let facts = trigger_facts(history);
    let input = facts.input();
    let (decision, _source) = crate::agent::harness::live_policy::decide_shipped(
        LEG,
        &input,
        |decision: &TriggerDecision| decision.check(&facts),
    );
    decision.back()
}

/// Every other decision in this leg is a boolean; the adapter flattens the leg's
/// `Hit`/`Clear` constructor into `hit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Verdict {
    hit: bool,
}

impl Verdict {
    #[cfg(test)]
    fn of(hit: bool) -> Self {
        Self { hit }
    }
}

/// One whitespace-split argv word, classified the way the native listing scan
/// reads it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LsWord {
    Ls,
    Word,
    Recursive,
    Plain,
}

impl LsWord {
    fn kind(self) -> &'static str {
        match self {
            Self::Ls => "ls",
            Self::Word => "word",
            Self::Recursive => "recursive",
            Self::Plain => "plain",
        }
    }
}

fn ls_words(cmd: &str) -> Vec<LsWord> {
    cmd.split_whitespace()
        .map(|part| {
            let base = part.rsplit('/').next().unwrap_or(part);
            if base.contains('=') && !base.starts_with('-') {
                return LsWord::Plain;
            }
            if !base.starts_with('-') {
                return if base == "ls" { LsWord::Ls } else { LsWord::Word };
            }
            if base.starts_with("--") {
                let recursive = base == "--recursive"
                    || base == "--recurse"
                    || base.starts_with("--recursive=")
                    || base.starts_with("--recurse=");
                return if recursive {
                    LsWord::Recursive
                } else {
                    LsWord::Plain
                };
            }
            if base.chars().skip(1).any(|c| c == 'r') {
                LsWord::Recursive
            } else {
                LsWord::Plain
            }
        })
        .collect()
}

fn ls_tokens_json(words: &[LsWord]) -> Vec<serde_json::Value> {
    words
        .iter()
        .map(|word| serde_json::json!({"kind": word.kind()}))
        .collect()
}

fn ls_input(words: &[LsWord]) -> serde_json::Value {
    serde_json::json!({"op": "ls_recursive", "tokens": ls_tokens_json(words)})
}

/// `ls` with a recursive flag, decided by the shipped leg.
pub(crate) fn ls_cmd_is_recursive_listing_live(cmd: &str) -> bool {
    let words = ls_words(cmd);
    let input = ls_input(&words);
    let (verdict, _source) = crate::agent::harness::live_policy::decide_shipped(
        LEG,
        &input,
        |_verdict: &Verdict| Ok(()),
    );
    verdict.hit
}

/// The `git show` scans, as features for the leg.
struct GitFacts {
    has_git: bool,
    has_show: bool,
    list_flag: bool,
    pathspec: bool,
    blob: bool,
}

impl GitFacts {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "has_git": self.has_git,
            "has_show": self.has_show,
            "list_flag": self.list_flag,
            "pathspec": self.pathspec,
            "blob": self.blob,
        })
    }
}

/// A single `rev:path` argv word with both halves non-empty, no `/` in the
/// revision and no URL scheme: the legal tip blob form.
fn git_show_names_a_blob(cmd: &str) -> bool {
    cmd.split_whitespace().any(|part| {
        if part.starts_with('-') {
            return false;
        }
        match part.split_once(':') {
            Some((rev, path)) => {
                !rev.is_empty()
                    && !path.is_empty()
                    && !rev.contains('/')
                    && !part.contains("://")
            }
            None => false,
        }
    })
}

fn git_facts(cmd: &str) -> GitFacts {
    GitFacts {
        has_git: shell_argv_has_token(cmd, "git"),
        has_show: shell_argv_has_token(cmd, "show"),
        list_flag: SHOW_LIST_FLAGS.iter().any(|flag| cmd.contains(flag)),
        pathspec: cmd.contains(" -- "),
        blob: git_show_names_a_blob(cmd),
    }
}

#[cfg(test)]
fn git_show_input(facts: &GitFacts) -> serde_json::Value {
    serde_json::json!({"op": "git_show", "git": facts.json()})
}

/// Leading env assignments are not the command word.
fn strip_env_prefix(cmd: &str) -> &str {
    let mut rest = cmd.trim_start();
    while let Some((head, tail)) = rest.split_once(char::is_whitespace) {
        if head.contains('=') && !head.starts_with('-') {
            rest = tail.trim_start();
            continue;
        }
        break;
    }
    rest
}

/// Where a shell command's first non-assignment word sits in the tables below.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FirstWord {
    Inventory,
    Git,
    Other,
}

impl FirstWord {
    fn kind(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Git => "git",
            Self::Other => "other",
        }
    }
}

fn shell_first_word(cmd: &str) -> &str {
    let first = cmd
        .split(|c: char| c.is_whitespace() || c == '|')
        .map(str::trim)
        .find(|part| !part.is_empty() && !part.contains('='))
        .unwrap_or("");
    first.rsplit('/').next().unwrap_or(first)
}

fn first_word(cmd: &str) -> FirstWord {
    let word = shell_first_word(cmd);
    if INVENTORY_WORDS.contains(&word) {
        FirstWord::Inventory
    } else if word == "git" {
        FirstWord::Git
    } else {
        FirstWord::Other
    }
}

/// The `git` inventory subcommands as flags: the first three are substrings in
/// the native scan (`git grep`, `git ls-files`, `git ls-tree`), the rest are
/// whole argv words.
fn git_arg_flags(cmd: &str) -> Vec<bool> {
    GIT_ARG_WORDS
        .iter()
        .enumerate()
        .map(|(index, word)| {
            if index < GIT_ARG_SUBSTRING_COUNT {
                cmd.contains(word)
            } else {
                shell_argv_has_token(cmd, word)
            }
        })
        .collect()
}

fn shell_input(cmd: &str) -> serde_json::Value {
    serde_json::json!({
        "op": "shell_thrash",
        "thrash_hit": THRASH_SUBSTRINGS.iter().any(|needle| cmd.contains(needle)),
        "ls": ls_tokens_json(&ls_words(cmd)),
        "git": git_facts(cmd).json(),
        "first": first_word(cmd).kind(),
        "git_args": git_arg_flags(cmd),
    })
}

/// Shell inventory / recursive search, decided by the shipped leg. Every branch
/// answers the same boolean, so the leg's scan order cannot change the answer.
fn shell_cmd_is_inventory_thrash_live(cmd: &str) -> bool {
    let input = shell_input(strip_env_prefix(cmd));
    let (verdict, _source) = crate::agent::harness::live_policy::decide_shipped(
        LEG,
        &input,
        |_verdict: &Verdict| Ok(()),
    );
    verdict.hit
}

/// True when `cmd` is an `ls` (possibly path-qualified / wrapped) with recursive
/// listing flags. Detects combined short clusters (`-laR`, `-lR`, `-1R`) and
/// long-form `--recursive` / `--recurse`, not only the bare `ls -r` substring.
/// The leg's parity oracle: test builds only.
#[cfg(test)]
pub(crate) fn ls_cmd_is_recursive_listing(cmd: &str) -> bool {
    super::super::comp_watch::recursive_listing(cmd)
}

/// Shell inventory and recursive search burn first-write budget even if the
/// command also names a board path.
fn shell_cmd_is_inventory_thrash(cmd: &str) -> bool {
    super::super::comp_watch::competition_inventory(cmd)
}

/// `git show HEAD:path` is a legal board blob digest. Listing / pathspec forms
/// (`--stat`, `--name-only`, `REV -- path`) are history inventory and must burn
/// first-write budget and count as anti-spin thrash (A8). The leg's parity
/// oracle: test builds only.
#[cfg(test)]
fn git_show_is_inventory_thrash(cmd: &str) -> bool {
    let facts = git_facts(cmd);
    if !facts.has_git || !facts.has_show {
        return false;
    }
    // Listing / summary flags and the `REV -- path` pathspec form are history; a
    // show with no `REV:path` blob word is a commit dump.
    facts.list_flag || facts.pathspec || !facts.blob
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
        .and_then(crate::knowledge::cut::apply_patch_single_target_path);
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
        let cmd = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
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
            crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
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
        "popcorn status",
        "popcorn-cli status",
        "popcorn submit",
        "--mode benchmark",
        "--mode leaderboard",
    ];
    if STRONG.iter().any(|needle| hay.contains(needle)) {
        return true;
    }
    // Each plugged-in board CLI's own status, listing and submit commands.
    if crate::agent::harness::cartridges::boards().any(|board| {
        ["status", "submissions", "list", "submit"]
            .iter()
            .any(|verb| hay.contains(&format!("{board} {verb}")))
    }) {
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
        return shell_cmd_is_inventory_thrash_live(hay_body);
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

/// Free-form recon: inspection that is neither board wait nor progress.
pub(crate) fn is_free_form_recon(call: &ToolCall) -> bool {
    !is_product_mutation_call(call) && !is_competition_wait_or_progress_call(call)
}

/// Single-pass competition hop flags: mutation, outcome, board wait, free-form
/// recon (A7: one walk instead of three that re-serialize args each time).
pub(crate) fn hop_budget_flags(calls: &[ToolCall]) -> (bool, bool, bool, bool) {
    let mut mutation = false;
    let mut outcome = false;
    let mut wait = false;
    let mut burns = false;
    for call in calls {
        let progress = is_product_mutation_call(call);
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

/// True when the hop loop needs competition hay.
/// Default cockpit (no competition, no in-flight slot) must not serialize args
/// to hunt board/submit markers.
pub(crate) fn hop_budget_classify_applied(competition: bool, hop_path_active: bool) -> bool {
    competition || hop_path_active
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

/// The native shell verdict. `shell_cmd_is_inventory_thrash` stays private
/// because only the classify hop calls it; the parity table needs the oracle.
#[cfg(test)]
pub(crate) fn shell_cmd_is_inventory_thrash_native(cmd: &str) -> bool {
    shell_cmd_is_inventory_thrash(cmd)
}

/// The launch phrase table, so a parity table covers every entry.
#[cfg(test)]
pub(crate) fn launch_phrases() -> &'static [&'static str] {
    LAUNCH_PHRASES
}

/// The competitive-goal vocabulary, so a parity table covers every entry.
#[cfg(test)]
pub(crate) fn goal_phrases() -> &'static [&'static str] {
    GOAL_PHRASES
}

/// The native `git show` verdict, for the leg's parity table.
#[cfg(test)]
pub(crate) fn git_show_is_inventory_thrash_native(cmd: &str) -> bool {
    git_show_is_inventory_thrash(cmd)
}

/// The engagement decision through a pinned generation, or through the
/// production route (newest live generation, else the shipped one) when no test
/// program is handed over.
#[cfg(test)]
pub(crate) fn trigger_decided(
    leg: Option<angel_live::Program>,
    history: &[ChatMsg],
) -> (Option<&'static str>, crate::agent::harness::live_policy::LegSource) {
    let facts = trigger_facts(history);
    let input = facts.input();
    let (decision, source) = match leg {
        Some(program) => crate::agent::harness::live_policy::decide_with(
            Some(program),
            LEG,
            &input,
            |decision: &TriggerDecision| decision.check(&facts),
            || TriggerDecision::of(competition_mode_trigger(history)),
        ),
        None => crate::agent::harness::live_policy::decide_shipped(LEG, &input, |decision: &TriggerDecision| {
            decision.check(&facts)
        }),
    };
    (decision.back(), source)
}

#[cfg(test)]
pub(crate) fn ls_decided(
    leg: Option<angel_live::Program>,
    cmd: &str,
) -> (bool, crate::agent::harness::live_policy::LegSource) {
    let words = ls_words(cmd);
    let input = ls_input(&words);
    let (verdict, source) = match leg {
        Some(program) => crate::agent::harness::live_policy::decide_with(
            Some(program),
            LEG,
            &input,
            |_verdict: &Verdict| Ok(()),
            || Verdict::of(ls_cmd_is_recursive_listing(cmd)),
        ),
        None => crate::agent::harness::live_policy::decide_shipped(LEG, &input, |_verdict: &Verdict| Ok(())),
    };
    (verdict.hit, source)
}

#[cfg(test)]
pub(crate) fn git_show_decided(
    leg: Option<angel_live::Program>,
    cmd: &str,
) -> (bool, crate::agent::harness::live_policy::LegSource) {
    let facts = git_facts(cmd);
    let input = git_show_input(&facts);
    let (verdict, source) = match leg {
        Some(program) => crate::agent::harness::live_policy::decide_with(
            Some(program),
            LEG,
            &input,
            |_verdict: &Verdict| Ok(()),
            || Verdict::of(git_show_is_inventory_thrash(cmd)),
        ),
        None => crate::agent::harness::live_policy::decide_shipped(LEG, &input, |_verdict: &Verdict| Ok(())),
    };
    (verdict.hit, source)
}

#[cfg(test)]
pub(crate) fn shell_decided(
    leg: Option<angel_live::Program>,
    cmd: &str,
) -> (bool, crate::agent::harness::live_policy::LegSource) {
    let input = shell_input(strip_env_prefix(cmd));
    let (verdict, source) = match leg {
        Some(program) => crate::agent::harness::live_policy::decide_with(
            Some(program),
            LEG,
            &input,
            |_verdict: &Verdict| Ok(()),
            || Verdict::of(shell_cmd_is_inventory_thrash(cmd)),
        ),
        None => crate::agent::harness::live_policy::decide_shipped(LEG, &input, |_verdict: &Verdict| Ok(())),
    };
    (verdict.hit, source)
}
