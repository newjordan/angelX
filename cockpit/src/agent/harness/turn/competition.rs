use super::classify::board_marker_as_token;
use super::*;
use std::borrow::Cow;

const LEG: &str = "competition-mode";

const LAUNCH_PHRASES: &[&str] = &[
    "competition mode",
    "competition-loop",
    "competition loop",
    "handoff rl",
    "handoff-rl",
    "hit it chewy",
    "podrace",
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

const GOAL_PHRASES: &[&str] = &[
    "competition",
    "compete",
    "leaderboard",
    "submission",
    "podrace",
    "winning",
];

const INVENTORY_WORDS: &[&str] = &[
    "find", "rg", "ripgrep", "grep", "egrep", "fgrep", "ugrep", "ag", "ack", "fd", "tree",
    "locate",
];

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

pub(crate) fn competition_mode_active(history: &[ChatMsg]) -> bool {
    competition_mode_trigger_live(history).is_some()
}

#[cfg(test)]
pub(crate) fn competition_mode_active_native(history: &[ChatMsg]) -> bool {
    competition_mode_trigger(history).is_some()
}

#[cfg(test)]
pub(crate) fn competition_mode_trigger(history: &[ChatMsg]) -> Option<&'static str> {
    if env_flag("ANGEL_COMPETITION_MODE", false) {
        return Some("ANGEL_COMPETITION_MODE");
    }
    let blob = operator_turn_text(history);
    let trigger = LAUNCH_PHRASES
        .iter()
        .copied()
        .find(|needle| ascii_contains_ignore_case(blob, needle));
    if trigger.is_some() {
        return trigger;
    }
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

fn operator_turn_text(history: &[ChatMsg]) -> &str {
    history
        .iter()
        .rev()
        .find(|message| message.role == ChatRole::User)
        .map(|message| message.content.as_ref())
        .unwrap_or_default()
}

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

fn shell_cmd_is_inventory_thrash_live(cmd: &str) -> bool {
    let input = shell_input(strip_env_prefix(cmd));
    let (verdict, _source) = crate::agent::harness::live_policy::decide_shipped(
        LEG,
        &input,
        |_verdict: &Verdict| Ok(()),
    );
    verdict.hit
}

#[cfg(test)]
pub(crate) fn ls_cmd_is_recursive_listing(cmd: &str) -> bool {
    super::super::comp_watch::recursive_listing(cmd)
}

fn shell_cmd_is_inventory_thrash(cmd: &str) -> bool {
    super::super::comp_watch::competition_inventory(cmd)
}

#[cfg(test)]
fn git_show_is_inventory_thrash(cmd: &str) -> bool {
    let facts = git_facts(cmd);
    if !facts.has_git || !facts.has_show {
        return false;
    }
    facts.list_flag || facts.pathspec || !facts.blob
}

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

pub(crate) fn classify_shell_hay_needs_lower(cmd: &str) -> bool {
    cmd.split_whitespace().any(|token| {
        let token = token.trim_matches(|c| matches!(c, '"' | '\'' | '`'));
        if token.is_empty() || token.contains('/') || token.contains('\\') || token.contains('.') {
            return false;
        }
        token.as_bytes().iter().any(|b| b.is_ascii_uppercase())
    })
}

pub(crate) fn classify_shell_hay(cmd: &str) -> Cow<'_, str> {
    if classify_shell_hay_needs_lower(cmd) {
        Cow::Owned(cmd.to_ascii_lowercase())
    } else {
        Cow::Borrowed(cmd)
    }
}

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

#[cfg(test)]
pub(crate) fn is_competition_outcome_call(call: &ToolCall) -> bool {
    let (name, hay, is_shell) = competition_call_text(call);
    is_competition_outcome_hay(name, &hay, is_shell)
}

fn is_competition_outcome_hay(name: &str, hay: &str, is_shell: bool) -> bool {
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
    if crate::agent::harness::cartridges::boards().any(|board| {
        ["status", "submissions", "list", "submit"]
            .iter()
            .any(|verb| hay.contains(&format!("{board} {verb}")))
    }) {
        return true;
    }

    if is_shell && (shell_argv_has_token(hay, "submit") || shell_argv_has_token(hay, "score")) {
        return true;
    }
    let _ = name;
    false
}

#[cfg(test)]
pub(crate) fn is_competition_board_state_call(call: &ToolCall) -> bool {
    let (name, hay_body, is_shell) = competition_call_text(call);
    is_competition_board_state_hay(name, &hay_body, is_shell)
}

fn is_competition_board_state_hay(name: &str, hay_body: &str, is_shell: bool) -> bool {
    if is_inventory_or_recon_thrash(name, hay_body, is_shell) {
        return false;
    }
    hay_names_board_meta(name) || hay_names_board_meta(hay_body)
}

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

fn is_inventory_or_recon_thrash(name: &str, hay_body: &str, is_shell: bool) -> bool {
    if is_shell {
        return shell_cmd_is_inventory_thrash_live(hay_body);
    }
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
    if name.eq_ignore_ascii_case("code_mode")
        && (ascii_contains_ignore_case(hay_body, "repo_recon")
            || ascii_contains_ignore_case(hay_body, "\"recipe\":\"repo_recon\"")
            || ascii_contains_ignore_case(hay_body, "\"recipe\": \"repo_recon\""))
    {
        return true;
    }
    false
}

pub(crate) fn is_competition_wait_or_progress_call(call: &ToolCall) -> bool {
    let (name, hay, is_shell) = competition_call_text(call);
    is_competition_outcome_hay(name, &hay, is_shell)
        || is_competition_board_state_hay(name, &hay, is_shell)
}

pub(crate) fn is_free_form_recon(call: &ToolCall) -> bool {
    !is_product_mutation_call(call) && !is_competition_wait_or_progress_call(call)
}

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

pub(crate) fn hop_budget_classify_applied(competition: bool, hop_path_active: bool) -> bool {
    competition || hop_path_active
}

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

#[cfg(test)]
pub(crate) fn shell_cmd_is_inventory_thrash_native(cmd: &str) -> bool {
    shell_cmd_is_inventory_thrash(cmd)
}

#[cfg(test)]
pub(crate) fn launch_phrases() -> &'static [&'static str] {
    LAUNCH_PHRASES
}

#[cfg(test)]
pub(crate) fn goal_phrases() -> &'static [&'static str] {
    GOAL_PHRASES
}

#[cfg(test)]
pub(crate) fn git_show_is_inventory_thrash_native(cmd: &str) -> bool {
    git_show_is_inventory_thrash(cmd)
}

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
