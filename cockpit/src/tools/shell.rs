//! `shell` — run the agent's command line under the landlock sandbox. The actual
//! sandbox-exec machinery (`run_sandboxed` and friends) is shared infrastructure
//! and lives in `harness`; this is just the tool wrapper around it.
//!
//! ## Why this runs `bash -o pipefail` and not `sh -c`
//!
//! Agents write `cd cockpit && cargo check 2>&1 | tail -20` constantly — 56% of
//! every command in the experience ledger contains a pipe. In a POSIX shell a
//! pipeline's status is its **last** stage's, so that line reports `tail`'s exit,
//! which is 0 essentially always. This tool is the only place a command's exit
//! status is ever observed, so **every piped build and test command angel has
//! ever run was recorded as a PASS regardless of whether it passed**, and the
//! Repo Dossier, The Cut, and the experience ledger all read that number.
//!
//! `pipefail` is the fix, and it cannot be done in the shell we were using:
//! `/bin/sh` here is `dash`, which rejects `set -o pipefail` outright ("Illegal
//! option"). So the shell moves to `bash`, which has had `pipefail` since 3.0.
//! That is a behavior change for every command angel runs, and the case for it is
//! that it is almost entirely a change agents were already assuming: `[[ … ]]`,
//! arrays, `&>`, `{1..10}`, and `<(…)` are all things models write by reflex and
//! all things that silently failed under dash. `--noprofile --norc` avoids user
//! startup files, but it does not prove executable identity: inherited
//! functions, `BASH_ENV`, and PATH shims can still affect resolution. The
//! harness therefore records raw-shell checks as verification attempts, never
//! as durable green evidence.
//!
//! `pipefail` is not free, and the ledger is honest about the cost rather than
//! papering over it. It faithfully reports a stage killed by its own consumer
//! (`seq 1 100000 | head -2` → 141, because `head` closed the pipe), which is a
//! plumbing artifact and not a failing build. Rather than swap a false PASS for a
//! false FAIL, such a status is recorded as an explicit **no-verdict** — see
//! [`crate::experience::cmd_verdict`], which is the sole authority on what a
//! recorded exit code is worth. Notably this does *not* cost us build verdicts:
//! cargo (like every Rust binary) ignores `SIGPIPE`, so even `cargo check 2>&1 |
//! head -5` reports cargo's true status.

use crate::sandbox::process_owner::OwnedCommandExt;
mod scope;

use crate::club::ToolDef;
use crate::experience::CmdShell;
use crate::harness::{Tool, run_sandboxed_observed_cancellable};
use crate::sandbox::SandboxPolicy;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn task_shell_active() -> bool {
    std::env::var("ANGEL_TASK_ACTIVE")
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("on")
        })
        .unwrap_or(false)
}

/// Sealed benchmark runners can opt into single-command process ownership.
/// A detached compiler outlives the tool receipt, so the next model hop cannot
/// tell whether it is observing the same build and commonly launches another.
fn task_shell_no_detach_active() -> bool {
    task_shell_active()
        && std::env::var("ANGEL_TASK_SHELL_NO_DETACH")
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false)
}

/// Sealed competition runners can make repository control state read-only
/// while still allowing ordinary source edits. A model that stashes its patch
/// can make the final workspace look clean and thereby evade scope/provenance
/// checks, so these tasks admit only Git's inspection commands.
fn task_shell_protect_git_active() -> bool {
    task_shell_active()
        && std::env::var("ANGEL_TASK_SHELL_PROTECT_GIT")
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false)
}

fn unquoted_shell_word(command: &str, wanted: &str) -> bool {
    let mut word = String::new();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let flush = |word: &mut String| {
        let found = word == wanted;
        word.clear();
        found
    };
    for ch in command.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            continue;
        }
        if ch == '\'' && !double {
            if !single && flush(&mut word) {
                return true;
            }
            single = !single;
            continue;
        }
        if ch == '"' && !single {
            if !double && flush(&mut word) {
                return true;
            }
            double = !double;
            continue;
        }
        if single || double {
            continue;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
            word.push(ch);
        } else if flush(&mut word) {
            return true;
        }
    }
    flush(&mut word)
}

fn strip_unquoted_shell_comments(command: &str) -> String {
    let mut clean = String::with_capacity(command.len());
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let mut comment = false;
    let mut token_started = false;
    for ch in command.chars() {
        if comment {
            if ch == '\n' {
                comment = false;
                token_started = false;
                clean.push(ch);
            }
            continue;
        }
        if escaped {
            escaped = false;
            token_started = true;
            clean.push(ch);
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            token_started = true;
            clean.push(ch);
            continue;
        }
        if ch == '\'' && !double {
            single = !single;
            token_started = true;
            clean.push(ch);
            continue;
        }
        if ch == '"' && !single {
            double = !double;
            token_started = true;
            clean.push(ch);
            continue;
        }
        if !single && !double && ch == '#' && !token_started {
            comment = true;
            continue;
        }
        clean.push(ch);
        if !single && !double {
            token_started =
                !(ch.is_whitespace() || matches!(ch, ';' | '|' | '&' | '(' | ')' | '<' | '>'));
        }
    }
    clean
}

/// Remove here-document bodies before policy scanners look for shell control
/// words/operators. The body is stdin data for the foreground command, not
/// shell syntax: Python's `left & right`, documentation containing `timeout`,
/// or a generated script fragment must not be mistaken for a detached process.
/// Keep the declaration line itself so `python <<'PY' &` is still rejected.
fn strip_shell_heredoc_bodies(command: &str) -> String {
    fn declarations(line: &str) -> Vec<(String, bool)> {
        let bytes = line.as_bytes();
        let mut found = Vec::new();
        let mut index = 0usize;
        let mut single = false;
        let mut double = false;
        let mut escaped = false;
        while index < bytes.len() {
            let byte = bytes[index];
            if escaped {
                escaped = false;
                index += 1;
                continue;
            }
            if byte == b'\\' && !single {
                escaped = true;
                index += 1;
                continue;
            }
            if byte == b'\'' && !double {
                single = !single;
                index += 1;
                continue;
            }
            if byte == b'"' && !single {
                double = !double;
                index += 1;
                continue;
            }
            if single
                || double
                || byte != b'<'
                || bytes.get(index + 1) != Some(&b'<')
                || bytes.get(index + 2) == Some(&b'<')
            {
                index += 1;
                continue;
            }
            index += 2;
            let strip_tabs = bytes.get(index) == Some(&b'-');
            if strip_tabs {
                index += 1;
            }
            while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
                index += 1;
            }
            let quote = bytes
                .get(index)
                .copied()
                .filter(|byte| matches!(byte, b'\'' | b'"'));
            if quote.is_some() {
                index += 1;
            }
            let start = index;
            while let Some(byte) = bytes.get(index).copied() {
                if quote.is_some_and(|quote| byte == quote)
                    || (quote.is_none()
                        && (byte.is_ascii_whitespace()
                            || matches!(byte, b';' | b'|' | b'&' | b'<' | b'>' | b'(' | b')')))
                {
                    break;
                }
                index += 1;
            }
            if index > start {
                found.push((line[start..index].to_string(), strip_tabs));
            }
            if quote.is_some() && bytes.get(index) == quote.as_ref() {
                index += 1;
            }
        }
        found
    }

    let mut clean = String::with_capacity(command.len());
    let mut pending = std::collections::VecDeque::<(String, bool)>::new();
    for segment in command.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        if let Some((delimiter, strip_tabs)) = pending.front() {
            let candidate = if *strip_tabs {
                line.trim_start_matches('\t')
            } else {
                line
            };
            if candidate == delimiter {
                pending.pop_front();
            }
            if segment.ends_with('\n') {
                clean.push('\n');
            }
            continue;
        }
        clean.push_str(segment);
        pending.extend(declarations(line));
    }
    clean
}

const TASK_EDITABLE_PATHS_ENV: &str = "ANGEL_TASK_EDITABLE_PATHS_JSON";

/// Return the literal filesystem targets of shell output redirections. Shell
/// commands need the whole workspace writable so compilers can populate their
/// build trees, but that broad execution capability must not make `> scratch`
/// a second, unscoped file-writing API in sealed competition tasks.
///
/// This is deliberately a narrow preflight rather than a shell parser. It
/// recognizes Bash's output operators, ignores descriptor duplication, strips
/// here-document bodies, and fails closed when the target uses expansion. The
/// post-turn workspace audit remains the authority for indirect writes made by
/// compilers and arbitrary programs.
fn literal_output_redirection_targets(command: &str) -> Result<Vec<String>, String> {
    fn token_boundary(chars: &[char], at: usize) -> bool {
        at == 0
            || chars[at - 1].is_whitespace()
            || matches!(chars[at - 1], ';' | '|' | '&' | '(' | ')' | '<' | '>')
    }

    fn redirection_word(chars: &[char], mut at: usize) -> (String, bool, usize) {
        while chars.get(at).is_some_and(|ch| ch.is_whitespace()) {
            at += 1;
        }
        let mut word = String::new();
        let mut single = false;
        let mut double = false;
        let mut literal = true;
        let mut first = true;
        while let Some(ch) = chars.get(at).copied() {
            if single {
                if ch == '\'' {
                    single = false;
                } else {
                    word.push(ch);
                }
                at += 1;
                continue;
            }
            if double {
                match ch {
                    '"' => double = false,
                    '$' | '`' => {
                        literal = false;
                        word.push(ch);
                    }
                    '\\' => {
                        at += 1;
                        if let Some(escaped) = chars.get(at).copied() {
                            word.push(escaped);
                        } else {
                            literal = false;
                        }
                    }
                    _ => word.push(ch),
                }
                at += 1;
                first = false;
                continue;
            }
            match ch {
                '\'' => single = true,
                '"' => double = true,
                '\\' => {
                    at += 1;
                    if let Some(escaped) = chars.get(at).copied() {
                        word.push(escaped);
                    } else {
                        literal = false;
                    }
                }
                '$' | '`' | '*' | '?' | '[' | '{' | '}' => {
                    literal = false;
                    word.push(ch);
                }
                '~' if first => {
                    literal = false;
                    word.push(ch);
                }
                ch if ch.is_whitespace()
                    || matches!(ch, ';' | '|' | '&' | '(' | ')' | '<' | '>') =>
                {
                    break;
                }
                _ => word.push(ch),
            }
            at += 1;
            first = false;
        }
        if single || double {
            literal = false;
        }
        (word, literal, at)
    }

    let command = strip_shell_heredoc_bodies(command);
    let command = strip_unquoted_shell_comments(&command);
    let chars = command.chars().collect::<Vec<_>>();
    let mut targets = Vec::new();
    let mut at = 0usize;
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let mut conditional = false;
    let mut arithmetic_depth = 0usize;
    while at < chars.len() {
        let ch = chars[at];
        if escaped {
            escaped = false;
            at += 1;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            at += 1;
            continue;
        }
        if ch == '\'' && !double {
            single = !single;
            at += 1;
            continue;
        }
        if ch == '"' && !single {
            double = !double;
            at += 1;
            continue;
        }
        if single || double {
            at += 1;
            continue;
        }

        // `>` is an expression operator inside Bash's conditional and
        // arithmetic forms, not a file redirection.
        if !conditional
            && arithmetic_depth == 0
            && ch == '['
            && chars.get(at + 1) == Some(&'[')
            && token_boundary(&chars, at)
        {
            conditional = true;
            at += 2;
            continue;
        }
        if conditional {
            if ch == ']' && chars.get(at + 1) == Some(&']') {
                conditional = false;
                at += 2;
            } else {
                at += 1;
            }
            continue;
        }
        if ch == '(' && chars.get(at + 1) == Some(&'(') {
            arithmetic_depth += 1;
            at += 2;
            continue;
        }
        if arithmetic_depth > 0 {
            if ch == ')' && chars.get(at + 1) == Some(&')') {
                arithmetic_depth -= 1;
                at += 2;
            } else {
                at += 1;
            }
            continue;
        }

        if ch != '>' {
            at += 1;
            continue;
        }
        // `>(command)` is process substitution. It has no filesystem target;
        // keep scanning its body so a nested `> file` is still found.
        if chars.get(at + 1) == Some(&'(') {
            at += 1;
            continue;
        }

        let mut target_at = at + 1;
        if matches!(chars.get(target_at), Some('>' | '|')) {
            target_at += 1;
        }
        let descriptor_duplication = chars.get(target_at) == Some(&'&');
        if descriptor_duplication {
            target_at += 1;
        }
        while chars.get(target_at).is_some_and(|ch| ch.is_whitespace()) {
            target_at += 1;
        }
        if matches!(
            (chars.get(target_at), chars.get(target_at + 1)),
            (Some('>'), Some('(')) | (Some('<'), Some('('))
        ) {
            // The target is a generated /dev/fd path. Resume at the process
            // substitution so redirections inside it are still inspected.
            at = target_at;
            continue;
        }
        let (target, literal, end) = redirection_word(&chars, target_at);
        if target.is_empty() {
            return Err(
                "sealed task could not determine a shell output redirection target".to_string(),
            );
        }
        if descriptor_duplication && (target == "-" || target.chars().all(|ch| ch.is_ascii_digit()))
        {
            at = end.max(at + 1);
            continue;
        }
        if !literal {
            return Err(format!(
                "sealed task could not determine expanded shell output redirection target {target:?}"
            ));
        }
        targets.push(target);
        at = end.max(at + 1);
    }
    Ok(targets)
}

fn bounded_scratch_path(path: &Path) -> bool {
    if matches!(
        path.to_str(),
        Some("/dev/null" | "/dev/stdout" | "/dev/stderr")
    ) {
        return true;
    }
    let descriptor = |prefix: &Path| {
        path.strip_prefix(prefix).is_ok_and(|suffix| {
            suffix.components().count() == 1
                && suffix
                    .to_str()
                    .is_some_and(|fd| fd.chars().all(|ch| ch.is_ascii_digit()))
        })
    };
    if descriptor(Path::new("/dev/fd")) || descriptor(Path::new("/proc/self/fd")) {
        return true;
    }
    let mut roots = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/dev/shm"),
    ];
    for key in ["TMPDIR", "XDG_RUNTIME_DIR"] {
        if let Some(root) = std::env::var_os(key).map(PathBuf::from) {
            roots.push(root);
        }
    }
    roots
        .iter()
        .any(|root| crate::harness::workspace_relative(root, path).is_ok())
}

fn task_shell_editable_redirect(command: &str, cwd: Option<&Path>) -> Option<String> {
    if !task_shell_active() {
        return None;
    }
    let raw = std::env::var_os(TASK_EDITABLE_PATHS_ENV)?;
    let Some(root) = cwd else {
        return Some(
            "shell command rejected: sealed task has no workspace for output-redirection scope"
                .to_string(),
        );
    };
    let configured = match raw
        .to_str()
        .ok_or_else(|| format!("{TASK_EDITABLE_PATHS_ENV} is not valid UTF-8"))
        .and_then(|raw| {
            serde_json::from_str::<Vec<String>>(raw)
                .map_err(|error| format!("invalid {TASK_EDITABLE_PATHS_ENV}: {error}"))
        }) {
        Ok(configured) => configured,
        Err(error) => return Some(format!("shell command rejected: {error}")),
    };
    let allowed = match configured
        .iter()
        .map(|path| crate::harness::workspace_relative(root, Path::new(path)))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(allowed) => allowed,
        Err(error) => return Some(format!("shell command rejected: {error}")),
    };
    let targets = match literal_output_redirection_targets(command) {
        Ok(targets) => targets,
        Err(error) => {
            return Some(format!(
                "shell command rejected: {error}. Use a literal editable workspace path or a literal /tmp/... path for scratch output."
            ));
        }
    };
    for target in targets {
        let supplied = Path::new(&target);
        if supplied.is_absolute() && bounded_scratch_path(supplied) {
            continue;
        }
        let relative = match crate::harness::workspace_relative(root, supplied) {
            Ok(relative) => relative,
            Err(_) => {
                return Some(format!(
                    "shell command rejected: sealed task rejected output redirection target {target:?} outside its workspace and bounded scratch roots; use a literal /tmp/... path for scratch output"
                ));
            }
        };
        if !allowed.iter().any(|path| {
            path.as_os_str().is_empty() || relative == *path || relative.starts_with(path)
        }) {
            return Some(format!(
                "shell command rejected: sealed task rejected out-of-scope output redirection target {target:?}; editable paths are {configured:?}. Use a literal /tmp/... path for scratch output."
            ));
        }
    }
    None
}

fn shell_words(command: &str) -> Vec<String> {
    let command = strip_shell_heredoc_bodies(command);
    let command = strip_unquoted_shell_comments(&command);
    let mut words = Vec::new();
    let mut word = String::new();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let flush = |word: &mut String, words: &mut Vec<String>| {
        if !word.is_empty() {
            words.push(std::mem::take(word));
        }
    };
    for ch in command.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            continue;
        }
        if ch == '\'' && !double {
            single = !single;
            continue;
        }
        if ch == '"' && !single {
            double = !double;
            continue;
        }
        if !single
            && !double
            && (ch.is_whitespace() || matches!(ch, ';' | '|' | '&' | '(' | ')' | '<' | '>'))
        {
            flush(&mut word, &mut words);
            continue;
        }
        word.push(ch);
    }
    flush(&mut word, &mut words);
    words
}

fn git_subcommand(words: &[String], git_at: usize) -> Option<&str> {
    let mut at = git_at + 1;
    while let Some(word) = words.get(at) {
        match word.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--exec-path" => {
                at += 2;
            }
            "--no-pager"
            | "--paginate"
            | "--bare"
            | "--literal-pathspecs"
            | "--no-literal-pathspecs"
            | "--glob-pathspecs"
            | "--noglob-pathspecs"
            | "--icase-pathspecs"
            | "--no-replace-objects"
            | "--no-optional-locks" => at += 1,
            option
                if option.starts_with("--git-dir=")
                    || option.starts_with("--work-tree=")
                    || option.starts_with("--namespace=")
                    || option.starts_with("--exec-path=") =>
            {
                at += 1;
            }
            "--version" | "--help" => return None,
            subcommand => return Some(subcommand),
        }
    }
    None
}

fn task_shell_git_redirect(command: &str) -> Option<String> {
    if !task_shell_protect_git_active() {
        return None;
    }
    const READ_ONLY: &[&str] = &[
        "annotate",
        "blame",
        "cat-file",
        "describe",
        "diff",
        "diff-tree",
        "for-each-ref",
        "grep",
        "log",
        "ls-files",
        "ls-tree",
        "merge-base",
        "name-rev",
        "rev-list",
        "rev-parse",
        "shortlog",
        "show",
        "show-ref",
        "status",
        "version",
        "whatchanged",
    ];
    let words = shell_words(command);
    for (at, word) in words.iter().enumerate() {
        if Path::new(word).file_name().and_then(|name| name.to_str()) != Some("git") {
            continue;
        }
        if git_subcommand(&words, at).is_some_and(|subcommand| !READ_ONLY.contains(&subcommand)) {
            return Some(
                "shell command rejected: this sealed task protects repository control state. \
                 Git is inspection-only: use status/diff/log/show/grep/ls-files/rev-parse, and \
                 edit source files directly. Do not stash, add, reset, restore, checkout, clean, \
                 commit, switch, fetch, merge, rebase, or push."
                    .to_string(),
            );
        }
    }
    None
}

fn task_shell_poll_redirect(command: &str) -> Option<String> {
    if !task_shell_no_detach_active() {
        return None;
    }
    let command = strip_shell_heredoc_bodies(command);
    let command = strip_unquoted_shell_comments(&command);
    unquoted_shell_word(&command, "sleep").then(|| {
        "shell command rejected: this sealed task does not spend a tool hop sleeping or polling. \
         Run the foreground build/benchmark directly and let it own its wait, or do other useful \
         work before taking one later status snapshot."
            .to_string()
    })
}

fn last_unquoted_background_operator(command: &str) -> Option<usize> {
    let chars = command.char_indices().collect::<Vec<_>>();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    let mut last = None;
    for (index, (byte_index, ch)) in chars.iter().copied().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && !single {
            escaped = true;
            continue;
        }
        if ch == '\'' && !double {
            single = !single;
            continue;
        }
        if ch == '"' && !single {
            double = !double;
            continue;
        }
        if single || double || ch != '&' {
            continue;
        }
        let prev = index
            .checked_sub(1)
            .and_then(|at| chars.get(at))
            .map(|(_, ch)| *ch);
        let next = chars.get(index + 1).map(|(_, ch)| *ch);
        if prev == Some('&')
            || next == Some('&')
            || prev == Some('>')
            || next == Some('>')
            || prev == Some('|')
        {
            continue;
        }
        last = Some(byte_index);
    }
    last
}

fn task_shell_detach_redirect(command: &str) -> Option<String> {
    if !task_shell_no_detach_active() {
        return None;
    }
    let command = strip_shell_heredoc_bodies(command);
    let command = strip_unquoted_shell_comments(&command);
    let detached_program = ["nohup", "disown", "setsid"]
        .iter()
        .any(|word| unquoted_shell_word(&command, word));
    let unmanaged_background = last_unquoted_background_operator(&command)
        .is_some_and(|at| !unquoted_shell_word(&command[at + 1..], "wait"));
    if !detached_program && !unmanaged_background {
        return None;
    }
    Some(
        "shell command rejected: this sealed task requires process ownership. Run builds and \
         benchmarks in the foreground; do not use nohup/disown/setsid or leave an `&` job \
         without `wait`. The runner supplies a long foreground tool deadline, so detached \
         polling is unnecessary. \
         (Opt out: ANGEL_TASK_SHELL_NO_DETACH=0.)"
            .to_string(),
    )
}

// ---------------------------------------------------------------------------
// Shell selection — resolved once, recorded on every command it runs.
// ---------------------------------------------------------------------------

/// The bash argv that keeps a pipeline's failing status *and* stays hermetic.
/// `--noprofile --norc` are no-ops for a non-interactive `-c` shell, but they
/// state the intent and cost nothing.
const BASH_ARGV: &[&str] = &["--noprofile", "--norc", "-o", "pipefail", "-c"];
/// The same for a non-bash shell that still understands `-o pipefail` (zsh, ksh).
const PIPEFAIL_ARGV: &[&str] = &["-o", "pipefail", "-c"];
/// Last resort: a plain POSIX shell. A pipeline's exit is then its last stage's,
/// so piped commands record **no verdict** rather than a fabricated one.
const POSIX_ARGV: &[&str] = &["-c"];

/// How the shell tool invokes a command line. Resolved once per process by
/// [`shell_invocation`] and stamped onto every ledger row it produces, so a
/// consumer never has to *assume* whether a status was trustworthy.
struct ShellInvocation {
    /// The shell program (`bash`, or whatever survived the probe).
    program: String,
    /// Argv before the command text.
    argv: &'static [&'static str],
    /// Whether `program argv` propagates a failing pipeline stage into the exit
    /// status. When false, a piped command's status is not evidence.
    pipefail: bool,
}

/// Can `program argv…` actually run? Probes with a trivial command, so an
/// unknown flag (`--noprofile` on a shell that has never heard of it) or a
/// missing binary is discovered here rather than corrupting a real command.
fn probe(program: &str, argv: &[&str]) -> bool {
    std::process::Command::new(program)
        .args(argv)
        .arg("exit 0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status_owned()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Pick the shell, preferring one that tells the truth about pipelines.
///
/// `ANGEL_SHELL` names the program (default `bash`); `ANGEL_SHELL_PIPEFAIL=0`
/// opts out of pipefail entirely, which restores the old execution semantics —
/// and, because the choice is recorded, makes every subsequent piped command
/// record `no_verdict` instead of a laundered pass. There is no configuration
/// that both lies and claims not to.
fn shell_invocation() -> &'static ShellInvocation {
    static SHELL: OnceLock<ShellInvocation> = OnceLock::new();
    SHELL.get_or_init(|| {
        let program = std::env::var("ANGEL_SHELL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "bash".to_string());
        if crate::experience::env_flag("ANGEL_SHELL_PIPEFAIL", true) {
            for argv in [BASH_ARGV, PIPEFAIL_ARGV] {
                if probe(&program, argv) {
                    return ShellInvocation {
                        program,
                        argv,
                        pipefail: true,
                    };
                }
            }
        }
        if probe(&program, POSIX_ARGV) {
            return ShellInvocation {
                program,
                argv: POSIX_ARGV,
                pipefail: false,
            };
        }
        // Nothing usable at `program` — fall back to the shell POSIX guarantees.
        ShellInvocation {
            program: "sh".to_string(),
            argv: POSIX_ARGV,
            pipefail: false,
        }
    })
}

// ---------------------------------------------------------------------------
// Shell tool — runs the command line under the landlock sandbox.
// ---------------------------------------------------------------------------

pub(crate) struct ShellTool {
    policy: SandboxPolicy,
    cwd: Option<PathBuf>,
    read_only: bool,
    confined_scratch: bool,
    mutation_targets: std::sync::Arc<crate::tools::build::MutationTargets>,
}

impl ShellTool {
    pub(crate) fn with_mutation_targets(
        mut self,
        targets: std::sync::Arc<crate::tools::build::MutationTargets>,
    ) -> Self {
        self.mutation_targets = targets;
        self
    }

    /// A shell scoped to `dir` (cwd + writable) — used inside worktrees.
    pub(crate) fn in_dir(dir: PathBuf) -> Self {
        let mut policy = SandboxPolicy::permissive();
        // A linked worktree's git state lives outside `dir`; without these
        // roots `git commit` dies on index.lock and the agent can never
        // complete a task's protocol.
        policy
            .writable_roots
            .extend(SandboxPolicy::git_worktree_roots(&dir));
        policy.writable_roots.push(dir.clone());
        Self {
            policy,
            cwd: Some(dir),
            read_only: false,
            confined_scratch: false,
            mutation_targets: std::sync::Arc::new(crate::tools::build::MutationTargets::default()),
        }
    }
    /// Capture-enabled recovery cannot grant all of /tmp or inherit YOLO.
    pub(crate) fn confined_in_dir(dir: PathBuf) -> Self {
        Self {
            policy: SandboxPolicy {
                writable_roots: vec![dir.clone()],
                allow_network: false,
                enforce: true,
                mandatory: true,
                sealed_reads: Vec::new(),
                deny_reads: Vec::new(),
            },
            cwd: Some(dir),
            read_only: false,
            confined_scratch: true,
            mutation_targets: std::sync::Arc::new(crate::tools::build::MutationTargets::default()),
        }
    }
    /// A shell scoped to `dir` as cwd, with workspace writes blocked.
    pub(crate) fn read_only_in_dir(dir: PathBuf) -> Self {
        Self {
            policy: SandboxPolicy::read_only(),
            cwd: Some(dir),
            read_only: true,
            confined_scratch: false,
            mutation_targets: std::sync::Arc::new(crate::tools::build::MutationTargets::default()),
        }
    }

    /// Run `command` under the resolved shell inside the sandbox.
    ///
    /// This is the only site where a shell command's exit status comes into
    /// existence. [`Tool::call`] deliberately hands the model nothing but the
    /// output, so the status — the number the whole experience ledger is built
    /// on, and the number this module's bug was about — is observable only here.
    /// That is precisely why the tests drive this rather than `call`.
    #[cfg(test)]
    fn observe(
        &self,
        command: &str,
    ) -> Result<(crate::harness::ExecObservation, &'static ShellInvocation), String> {
        self.observe_with_cancel(command, None, &self.scope(&serde_json::json!({}))?)
    }

    fn observe_with_cancel(
        &self,
        command: &str,
        cancel: Option<&std::sync::atomic::AtomicBool>,
        scope: &scope::ExecutionScope,
    ) -> Result<(crate::harness::ExecObservation, &'static ShellInvocation), String> {
        let confined_command;
        let command = if self.confined_scratch {
            // Constant shell text only; no path is interpolated. The child
            // starts in the owned workspace and may only write beneath it.
            confined_command = format!(
                "export TMPDIR=\"$PWD/.angel-experiment-tmp\" TMP=\"$PWD/.angel-experiment-tmp\" TEMP=\"$PWD/.angel-experiment-tmp\" XDG_CACHE_HOME=\"$PWD/.angel-experiment-tmp/cache\"; {command}"
            );
            confined_command.as_str()
        } else {
            command
        };
        let shell = shell_invocation();
        let mut argv: Vec<&str> = shell.argv.to_vec();
        argv.push(command);
        // Capability evidence is recorded before execution, including calls
        // which fail after a partial write. A scoped/inspection call cannot
        // erase uncertainty from an earlier unrestricted writable command.
        if let Some(paths) = &scope.paths {
            self.mutation_targets.record_scoped_paths(paths);
        } else {
            self.mutation_targets.mark_opaque();
        }
        let obs = run_sandboxed_observed_cancellable(
            &shell.program,
            &argv,
            self.cwd.as_deref(),
            &scope.policy,
            cancel,
        )?;
        Ok((obs, shell))
    }
}

/// A command that tried to elevate or drive a system package manager. Under
/// `no_new_privs` these can never succeed, and models that watch them fail
/// tend to conclude "installs are impossible here" and abandon the task —
/// so the failure carries a pointer at the install paths that do work.
fn privileged_install_attempt(command: &str) -> bool {
    command.split_whitespace().any(|token| {
        let bin = token.rsplit('/').next().unwrap_or(token);
        matches!(
            bin,
            "sudo"
                | "doas"
                | "su"
                | "apt"
                | "apt-get"
                | "dnf"
                | "yum"
                | "pacman"
                | "zypper"
                | "apk"
        )
    })
}

/// Piped/bootstrap installers commonly write a CLI payload under
/// `~/.local/share` and install skills/configuration beside it. Those locations
/// are intentionally outside the ordinary workspace sandbox, so retrying the
/// same script (or chmod/copy variants) cannot make progress.
fn user_home_installer_attempt(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    (lower.contains("curl") || lower.contains("wget"))
        && (lower.contains("install.sh") || lower.contains("| sh") || lower.contains("| bash"))
}

fn looks_like_sandbox_write_denial(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    [
        "permission denied",
        "operation not permitted",
        "read-only file system",
        "failure writing output",
        "eacces",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// The command tried to start its own sandbox inside Angel's confinement.
/// macOS refuses a nested `sandbox-exec`/`sandbox_apply` (SwiftPM does this
/// for manifest and plugin evaluation); bubblewrap cannot create namespaces
/// under the default Landlock backend. Neither is a defect in the model's
/// change, so say exactly which side must move.
fn looks_like_nested_sandbox_denial(output: &str) -> Option<&'static str> {
    let lower = output.to_ascii_lowercase();
    if lower.contains("sandbox_apply: operation not permitted")
        || lower.contains("sandbox-exec: sandbox_apply")
    {
        return Some(SANDBOX_NESTED_SEATBELT_HINT);
    }
    let nested_bwrap = lower.lines().any(|line| {
        let line = line.trim().trim_start_matches("[stderr] ");
        line.starts_with("bwrap: ")
            && (line.contains("no permissions to create new namespace")
                || line.contains("creating new namespace failed")
                || line.contains("setting up uid map")
                || line.contains("setting up gid map")
                || line.contains("can't bind mount /oldroot/dev/"))
    });
    nested_bwrap.then_some(SANDBOX_NESTED_BWRAP_HINT)
}

const SANDBOX_NESTED_SEATBELT_HINT: &str = "[sandbox: this command tried to start its own macOS \
sandbox-exec inside Angel's confinement, which the kernel refuses (sandbox_apply: Operation not \
permitted). Angel already confines writes, so run SwiftPM with `--disable-sandbox` (swift build/test \
--disable-sandbox …) or give the tool the equivalent flag; the code you changed did not cause this.]";

const SANDBOX_NESTED_BWRAP_HINT: &str = "[sandbox: this command's own bubblewrap sandbox could not \
start inside Angel's confinement. That is a harness/launch setting, not your change: the operator \
must launch with ANGEL_SANDBOX_BACKEND=bwrap on a namespace-capable host. Do not retry the same \
command or disable the project's sandbox; report the blocker.]";

const SANDBOX_INSTALL_HINT: &str = "[sandbox: privilege escalation is disabled (no_new_privs), \
so sudo/system package managers cannot work here — this is not a missing password. Self-serve \
user-level instead: pip/npm/cargo installs, or download a static binary into ~/.local/bin \
(writable, on PATH). Network is available. The operator can lift confinement with \
ANGEL_SANDBOX=0 or /yolo.]";

const SANDBOX_USER_INSTALL_HINT: &str = "[sandbox: this bootstrap installer tried to write user \
state/configuration outside the sanctioned install roots. Retrying, chmod, or copy workarounds \
cannot widen Landlock. Use an already-installed binary, or ask the operator to run the installer \
outside the cockpit / enable `/yolo on` for this explicit install.]";

/// Read the canonical shell argument, accepting `cmd` and `script` only as
/// compatibility aliases for recovered/provider tool calls (and code_mode
/// scripts) that use the wrong vocabulary. Keep `command` authoritative when
/// several appear so the advertised API stays deterministic.
pub(crate) fn shell_command_arg(args: &Value) -> Option<&str> {
    args.get("command")
        .and_then(Value::as_str)
        .or_else(|| args.get("cmd").and_then(Value::as_str))
        .or_else(|| args.get("script").and_then(Value::as_str))
}

impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }
    fn def(&self) -> ToolDef {
        // The model is told `pipefail` is on, because it changes what it should
        // write: a pipeline whose early stage is *expected* to fail needs an
        // explicit `|| true`. Saying "bash" also licenses the bash-isms models
        // reach for anyway and that used to fail silently under dash.
        let description = if crate::yolo::enabled() {
            if task_shell_active() {
                "Run a shell command (bash -c, with `pipefail` set). YOLO is active for this \
                 headless coding task: writes/network are unrestricted, but default cwd is the \
                 task workspace — start with local `ls`/`find .`/`tests` there. Do not inventory \
                 `/`, `$HOME`, or unrelated repos; prefer `read_file`/`grep` inside the workspace. \
                 Returns combined stdout+stderr."
            } else {
                "Run an unrestricted operator-approved shell command (bash -c, with `pipefail` set). \
             YOLO is active: filesystem writes, network access, subprocesses, and command \
             duration are not confined by the harness; returns combined stdout+stderr."
            }
        } else if crate::yolo::smart_enabled() {
            "Run a shell command (bash -c, with `pipefail` set) for operator-approved powerful \
             coding. YOLO SMART is active: workspace shell/write batches do not wait for \
             interactive approval — act decisively with tools (edit, build, test, fix). Writes \
             stay Landlock-confined to the workspace; tool timeouts and hooks still apply; \
             returns combined stdout+stderr. Prefer concrete tool-backed code changes over \
             status prose."
        } else if self.read_only {
            "Run a read-only shell command (bash -c, with `pipefail` set) inside the sandbox. \
             The filesystem is readable, but writes to the workspace are blocked; returns \
             combined stdout+stderr."
        } else {
            "Run a shell command (bash -c, with `pipefail` set, so a pipeline reports a failing \
             stage rather than its last stage) inside the sandbox. Writes are confined to the \
             workspace; returns combined stdout+stderr. Network is available, but privilege \
             escalation is disabled, so sudo and system package managers cannot install \
             anything. Simple pip/npm/cargo or static-binary installs can use the sanctioned \
             cache/bin roots; bootstrap scripts that also modify user config/state require \
             explicit `/yolo on` or an operator-side install."
        };
        ToolDef {
            name: "shell".to_string(),
            description: format!(
                "{description} Omit scope options for builds, benchmarks, installs and background jobs: they need scratch writes. read_only and write_paths restrict the entire process tree, including temporary files and network; write_paths is not an output-file list. Act on actual tool evidence: preserve the earliest prerequisite failure, check usable input before dependent measurements, use allowed scratch, and discover optional dependencies or authorized reference paths from real errors and permissions. Do not score failed input as zero performance. Explicit read_only/write_paths remain authoritative; request a user-visible scope change if they block needed work, never omit or auto-remove them. {}{}Never use shell `sleep` to poll a job or submission; keep doing \
                 useful work and use one later status snapshot. Competition submission status \
                 arrives from the harness watcher.",
                if task_shell_no_detach_active() {
                    "This sealed task requires foreground process ownership: run long builds \
                     directly; nohup/disown/setsid and unmanaged `&` jobs are rejected. "
                } else {
                    ""
                },
                if task_shell_protect_git_active() {
                    "This sealed task permits read-only Git inspection only; edit files directly \
                     and do not stash/add/reset/restore/checkout/clean/commit/switch/fetch/merge/\
                     rebase/push. "
                } else {
                    ""
                }
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "shell command. Judge from actual exit status and output; quoted text, a quiet success, and a valid `$url` are not empty-input or zero-frame evidence." },
                    "read_only": { "type": "boolean", "description": "Enforce no filesystem writes except /dev/null and no network; default false. Explicit restrictions stay authoritative; request a user-visible scope change instead of omitting them." },
                    "write_paths": { "type": "array", "maxItems": 128, "items": { "type": "string" }, "description": "Optional strict edit restriction, NOT an output-file list. Only listed existing source files are writable; temporary files, new files and network are denied, including in child processes. Omit for builds, benchmarks, installs and background jobs. Empty means read-only. A denial under this grant is a permission failure, not zero performance." }
                },
                "required": ["command"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        let command = shell_command_arg(args).ok_or("missing 'command'")?;
        let scope = self.scope(args)?;
        if let Some(redirect) = task_shell_editable_redirect(command, self.cwd.as_deref()) {
            return Err(redirect);
        }
        if let Some(redirect) = task_shell_git_redirect(command) {
            return Err(redirect);
        }
        if let Some(redirect) = task_shell_poll_redirect(command) {
            return Err(redirect);
        }
        if let Some(redirect) = task_shell_detach_redirect(command) {
            return Err(redirect);
        }
        // Board submissions carry the harness's own attribution, never the
        // model's copy of someone else's (`submit_identity`).
        let stamped = crate::tools::submit_identity::stamp(command, self.cwd.as_deref())?;
        let command = stamped.as_ref().map_or(command, |s| s.command.as_str());
        let (mut obs, shell) = self.observe_with_cancel(command, cancel, &scope)?;
        if stamped.is_some() {
            crate::tools::submit_identity::journal_execution(
                "shell",
                command,
                self.cwd.as_deref(),
                obs.exit,
                &obs.output,
            );
        }
        if let Some(stamped) = &stamped {
            obs.output = format!("{}\n{}", stamped.notice, obs.output);
        }
        // Experience ledger: what ran, where, and how it went (exit status is
        // observed here and nowhere else). The shell we ran it under rides along,
        // because without it `exit` cannot be told from the exit of whatever
        // happened to sit last in a pipeline. Best-effort; never alters the
        // output returned to the model.
        let folder = self
            .cwd
            .clone()
            .unwrap_or_else(crate::harness::current_dir_workspace);
        let experience = crate::experience::CmdExperience {
            tool: "shell",
            text: command,
            exit: obs.exit,
            timed_out: obs.timed_out,
            dur_ms: obs.dur_ms,
            bytes_out: obs.output.len(),
            shell: CmdShell::Shell {
                name: &shell.program,
                pipefail: shell.pipefail,
            },
        };
        crate::experience::record_cmd_event(&experience, &folder);
        if obs.cancelled {
            // The shared cancellation flag also covers a turn deadline.
            let mut message = "shell command cancelled".to_string();
            if !obs.output.trim().is_empty() {
                message.push('\n');
                message.push_str(&obs.output);
            }
            return Err(obs
                .kill
                .as_ref()
                .map_or_else(|| message.clone(), |kill| kill.error(&message)));
        }

        if obs.exit.is_none() {
            let message = format!(
                "shell worker killed or vanished (exit unknown); verification inconclusive\n{}",
                obs.output
            );
            return Err(obs
                .kill
                .as_ref()
                .map_or_else(|| message.clone(), |kill| kill.error(&message)));
        }

        // The ledger used to know a command failed while the model only saw its
        // stdout/stderr as an ordinary successful tool result.  That split-brain
        // contract let long loops build fresh "findings" on top of dozens of
        // failed commands.  Make the same canonical verdict visible at the tool
        // boundary: definite failures are tool errors, while a SIGPIPE-style
        // no-verdict remains usable but is explicitly labelled as inconclusive.
        let (verdict, reason) = crate::experience::cmd_verdict(&experience);
        match verdict {
            crate::experience::VERDICT_FAIL => {
                let code = obs
                    .exit
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "signal".to_string());
                let mut message = format!("shell command failed (exit {code})");
                if !obs.output.trim().is_empty() {
                    message.push('\n');
                    message.push_str(&obs.output);
                } else {
                    message.push_str(
                        "\n[no output on stdout or stderr; exit status above is the only diagnostic]",
                    );
                }
                if let Some(paths) = &scope.paths {
                    if paths.is_empty() {
                        message.push_str("\n[effective shell scope: filesystem read-only; network disabled. An empty write_paths array selects this scope even with read_only=false. This result alone does not establish a host DNS, disk or permissions failure.]");
                    } else {
                        message.push_str("\n[effective shell scope: writes only to write_paths; temporary-file creation and network disabled for the entire process tree. For builds or benchmarks, omit write_paths to use the workspace's normal permissions.]");
                    }
                }
                if obs.exit == Some(127)
                    && let Some(error) =
                        crate::tools::runtime_missing::from_command_not_found(&obs.output)
                {
                    return Err(error);
                }
                // 127 = command not found: say which program and what exists.
                if obs.exit == Some(127)
                    && let Some(hint) = crate::workspace_lang::missing_program_hint(command)
                {
                    message.push('\n');
                    message.push_str(&hint);
                }
                if let Some(hint) =
                    (scope.paths.is_none() && self.policy.enforce && !crate::yolo::enabled())
                        .then(|| looks_like_nested_sandbox_denial(&obs.output))
                        .flatten()
                {
                    message.push('\n');
                    message.push_str(hint);
                } else if scope.paths.is_none()
                    && !self.read_only
                    && self.policy.enforce
                    && !crate::yolo::enabled()
                    && privileged_install_attempt(command)
                {
                    message.push('\n');
                    message.push_str(SANDBOX_INSTALL_HINT);
                } else if scope.paths.is_none()
                    && !self.read_only
                    && self.policy.enforce
                    && !crate::yolo::enabled()
                    && user_home_installer_attempt(command)
                    && looks_like_sandbox_write_denial(&obs.output)
                {
                    message.push('\n');
                    message.push_str(SANDBOX_USER_INSTALL_HINT);
                }
                if scope.paths.is_none()
                    && self.policy.enforce
                    && !crate::yolo::enabled()
                    && looks_like_sandbox_write_denial(&obs.output)
                {
                    let workspace = self.cwd.as_deref().unwrap_or_else(|| Path::new("."));
                    message.push_str(&format!(
                        "\n[workspace boundary: current workspace is {}. Creating a different home-level project or editing a symlink target outside the granted roots is not permitted by this session. Restart in the intended project or use an explicitly operator-approved scope change; retrying cp/chmod does not change the grant. No successful host change may be claimed without checking the actual destination.]",
                        workspace.display()
                    ));
                }
                Err(message)
            }
            crate::experience::VERDICT_NONE => {
                let code = obs
                    .exit
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let why = reason.unwrap_or("untrusted status");
                let mut output = obs.output;
                if !output.is_empty() && !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str(&format!(
                    "[shell verdict unavailable: exit {code}, reason={why}]"
                ));
                Ok(output)
            }
            _ => Ok(obs.output),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experience::{CmdExperience, VERDICT_FAIL, VERDICT_NONE, VERDICT_PASS, cmd_verdict};
    use std::path::Path;

    #[test]
    fn sealed_task_shell_rejects_detached_work_but_allows_joined_parallelism() {
        let _guard = crate::tests::env_lock();
        let _active = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
        let _sealed = crate::tests::TestEnvGuard::set("ANGEL_TASK_SHELL_NO_DETACH", "1");

        for command in [
            "nohup swift build -c release >build.log 2>&1 &",
            "(swift build -c release &) ; sleep 28",
            "setsid ./benchmark >bench.log 2>&1 &",
            "make all & echo $! > build.pid",
            "make all & # do not wait for it",
            "wait; make all &",
            "sleep 30; ps aux | grep benchmark",
        ] {
            assert!(
                task_shell_detach_redirect(command).is_some()
                    || task_shell_poll_redirect(command).is_some(),
                "must reject {command:?}"
            );
        }
        for command in [
            "swift build -c release",
            "make shard-a & make shard-b & wait",
            "cargo test 2>&1 | tail -20",
            "cargo test |& tee test.log",
            "cmd_a && cmd_b",
            "timeout 1700 python3 search.py",
            "gtimeout 1750 lake build Challenge.Modexp.Submission.Solution",
            "printf '%s\\n' 'nohup & setsid'",
            "printf '%s\\n' 'timeout 1700 benchmark'",
            "grep -n \"sleep 30\" benchmark.sh",
            "python3 - <<'PY'\nmask = left & right\nprint('timeout 1700 benchmark')\nPY",
            "cat <<EOF\nnohup benchmark &\nsleep 30\nEOF",
        ] {
            assert!(
                task_shell_detach_redirect(command).is_none()
                    && task_shell_poll_redirect(command).is_none(),
                "must allow {command:?}"
            );
        }
        assert!(
            task_shell_detach_redirect("python3 - <<'PY' &\nprint('still detached')\nPY").is_some(),
            "a background operator on the heredoc declaration remains shell control syntax"
        );
    }

    #[test]
    fn sealed_task_shell_rejects_git_control_mutations_but_allows_inspection() {
        let _guard = crate::tests::env_lock();
        let _active = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
        let _sealed = crate::tests::TestEnvGuard::set("ANGEL_TASK_SHELL_PROTECT_GIT", "1");

        for command in [
            "git stash && swift test; git stash pop",
            "/usr/bin/git -C . reset --hard HEAD",
            "command git -c advice.detachedHead=false checkout main",
            "git \"clean\" -fd",
            "git add src && git commit -m candidate",
            "git branch scratch",
            "git submodule update --init",
            "git $MUTATING_SUBCOMMAND",
        ] {
            assert!(
                task_shell_git_redirect(command).is_some(),
                "must reject {command:?}"
            );
        }
        for command in [
            "git status --short",
            "git -C . diff -- src",
            "/usr/bin/git --no-pager log -3 --oneline",
            "git rev-parse HEAD && git ls-files",
            "printf '%s\\n' 'git stash'",
            "grep -n \"git stash\" README.md",
        ] {
            assert!(
                task_shell_git_redirect(command).is_none(),
                "must allow {command:?}"
            );
        }
    }

    #[test]
    fn sealed_task_shell_finds_literal_output_redirections_without_confusing_shell_syntax() {
        assert_eq!(
            literal_output_redirection_targets(
                "cat > .scratch_gen.py <<'EOF'\nprint('a > b')\nEOF\nprintf ok 2>> 'logs/error log'",
            )
            .unwrap(),
            vec![".scratch_gen.py", "logs/error log"]
        );
        for command in [
            "cargo test 2>&1 | tail -20",
            "printf '%s\\n' 'literal > text'",
            "[[ $left > $right ]] && echo ordered",
            "(( score > best )) && echo improved",
            "diff <(printf old) >(printf new)",
            "printf ok > >(cat)",
        ] {
            assert!(
                literal_output_redirection_targets(command)
                    .unwrap()
                    .is_empty(),
                "must not invent a file target for {command:?}"
            );
        }
        assert!(literal_output_redirection_targets("printf ok > \"$target\"").is_err());
    }

    #[test]
    fn sealed_task_shell_rejects_out_of_scope_redirection_before_it_writes() {
        let _guard = crate::tests::env_lock();
        let _active = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
        let _editable =
            crate::tests::TestEnvGuard::set(TASK_EDITABLE_PATHS_ENV, "[\"submission\"]");
        let dir = scratch("editable-redirection");
        std::fs::create_dir_all(dir.join("submission")).unwrap();
        let tool = ShellTool::in_dir(dir.clone());

        let error = tool
            .call(&serde_json::json!({
                "command": "cat > .scratch_gen.py <<'EOF'\nprint('scratch')\nEOF"
            }))
            .expect_err("out-of-scope heredoc target must fail before Bash starts");
        assert!(error.contains("out-of-scope output redirection"), "{error}");
        assert!(!dir.join(".scratch_gen.py").exists());

        tool.call(&serde_json::json!({
            "command": "printf retained > submission/candidate.txt"
        }))
        .expect("an editable target remains available to the shell");
        assert_eq!(
            std::fs::read_to_string(dir.join("submission/candidate.txt")).unwrap(),
            "retained"
        );

        let scratch_path =
            std::env::temp_dir().join(format!("angel-shell-redirection-{}", std::process::id()));
        tool.call(&serde_json::json!({
            "command": format!("printf transient > {}", scratch_path.display())
        }))
        .expect("bounded scratch output remains available");
        assert_eq!(std::fs::read_to_string(&scratch_path).unwrap(), "transient");

        let _ = std::fs::remove_file(scratch_path);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn privileged_install_attempts_are_recognized() {
        for cmd in [
            "sudo apt-get install -y ffmpeg",
            "apt install ffmpeg",
            "DEBIAN_FRONTEND=noninteractive apt-get -y install ffmpeg",
            "/usr/bin/sudo dnf install ffmpeg",
            "yes | pacman -S ffmpeg",
        ] {
            assert!(privileged_install_attempt(cmd), "should match: {cmd}");
        }
        for cmd in [
            "pip install --user imageio-ffmpeg",
            "npm install esbuild",
            "cargo install ripgrep",
            "curl -L https://example.com/ffmpeg.tar.xz | tar -xJ -C ~/.local/bin",
            "echo apturl",
        ] {
            assert!(!privileged_install_attempt(cmd), "should not match: {cmd}");
        }
    }

    #[test]
    fn bootstrap_install_write_denials_are_recognized_without_guessing_network_failures() {
        assert!(user_home_installer_attempt(
            "curl -fsSL https://api.example.test/tool/install.sh | sh"
        ));
        assert!(user_home_installer_attempt(
            "wget -qO- https://example.test/bootstrap | bash"
        ));
        assert!(!user_home_installer_attempt("cargo install ripgrep"));
        assert!(looks_like_sandbox_write_denial(
            "curl: (23) Failure writing output to destination"
        ));
        assert!(looks_like_sandbox_write_denial(
            "EACCES: permission denied, open '/home/user/.config/tool'"
        ));
        assert_eq!(
            looks_like_nested_sandbox_denial(
                "error: 'gemma-mini': Invalid manifest\nsandbox-exec: sandbox_apply: Operation not permitted\nerror: ExitCode(rawValue: 1)"
            ),
            Some(SANDBOX_NESTED_SEATBELT_HINT)
        );
        assert_eq!(
            looks_like_nested_sandbox_denial(
                "local-candidate-build: building under bubblewrap\n[stderr] bwrap: Can't bind mount /oldroot/dev/zero on /newroot/dev/zero: No such file or directory"
            ),
            Some(SANDBOX_NESTED_BWRAP_HINT)
        );
        assert_eq!(
            looks_like_nested_sandbox_denial(
                "bwrap: No permissions to create new namespace, likely because the kernel does not allow non-privileged user namespaces"
            ),
            Some(SANDBOX_NESTED_BWRAP_HINT)
        );
        assert_eq!(
            looks_like_nested_sandbox_denial(
                "error: candidate does not compile\nOperation not permitted while writing target/"
            ),
            None
        );
        assert!(!looks_like_sandbox_write_denial(
            "curl: (22) The requested URL returned error: 404"
        ));
    }

    /// Run `command` exactly the way the tool does and return the ledger row it
    /// would produce: `(exit, verdict, reason)`. Goes through [`ShellTool::observe`]
    /// so a regression in shell selection is caught by these tests rather than by
    /// six months of silently false evidence.
    fn row(dir: &Path, command: &str) -> (Option<i32>, &'static str, Option<&'static str>) {
        let tool = ShellTool::in_dir(dir.to_path_buf());
        let (obs, shell) = tool.observe(command).expect("shell ran");
        let exp = CmdExperience {
            tool: "shell",
            text: command,
            exit: obs.exit,
            timed_out: obs.timed_out,
            dur_ms: obs.dur_ms,
            bytes_out: obs.output.len(),
            shell: CmdShell::Shell {
                name: &shell.program,
                pipefail: shell.pipefail,
            },
        };
        let (verdict, reason) = cmd_verdict(&exp);
        (obs.exit, verdict, reason)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("angel-shell-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn shell_tool_honors_registry_cancel_authority() {
        let dir = scratch("cancel");
        let mut registry = crate::harness::ToolRegistry::new();
        registry.register(Box::new(ShellTool::in_dir(dir.clone())));
        registry
            .dispatch(
                "shell",
                &serde_json::json!({"command": "printf runner-warm"}),
            )
            .expect("warm shell runner");

        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let trigger = std::sync::Arc::clone(&cancel);
        let setter = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            trigger.store(true, std::sync::atomic::Ordering::Release);
        });
        let started = std::time::Instant::now();
        let error = registry
            .dispatch_with_cancel(
                "shell",
                &serde_json::json!({"command": "sleep 30 & wait"}),
                Some(cancel.as_ref()),
            )
            .expect_err("operator cancellation must fail the active tool call");
        setter.join().unwrap();

        assert!(error.contains("shell command cancelled"), "{error}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "shell cancellation took {:?}",
            started.elapsed()
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Write a crate at `dir` that does not compile, and return its path.
    fn broken_crate(dir: &Path) {
        std::fs::create_dir_all(dir.join("src")).unwrap();
        // `[workspace]` detaches it from any enclosing workspace; no dependencies,
        // so `cargo check` needs no network.
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"brokencrate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            "fn main() { let _x: i32 = \"not an integer\"; oops_undefined(); }\n",
        )
        .unwrap();
    }

    /// The bug, in the exact shape agents write it. A broken build behind a
    /// `| tail -20` recorded `tail`'s 0 — a PASS — for the entire history of the
    /// ledger. It must now be a failure, and it must never again be a pass.
    #[test]
    fn a_broken_build_behind_a_pipe_is_not_a_pass() {
        // Neighboring self-gate tests temporarily put a fake cargo on PATH.
        // Keep all invocations here bound to the real compiler environment.
        let _env = crate::tests::env_lock();
        let dir = scratch("brokencrate");
        broken_crate(&dir);

        // Ground truth: unpiped, this build fails (cargo exits 101).
        let (bare_exit, bare_verdict, _) = row(&dir, "cargo check");
        if bare_exit.is_none() || bare_verdict == VERDICT_NONE {
            return; // no usable cargo in this environment — nothing to assert
        }
        assert_eq!(bare_verdict, VERDICT_FAIL, "the crate really is broken");

        // The real-world case, verbatim.
        let (exit, verdict, _) = row(&dir, "cargo check 2>&1 | tail -20");
        assert_ne!(
            verdict, VERDICT_PASS,
            "a broken build behind `| tail -20` must never record a pass (got exit {exit:?})"
        );
        assert_eq!(
            verdict, VERDICT_FAIL,
            "`tail` drains the pipe, so cargo is never SIGPIPE'd and its true status survives"
        );
        assert_eq!(exit, bare_exit, "the pipeline reports cargo's own status");

        // `| head -N` truncates, but cargo (like every Rust binary) ignores
        // SIGPIPE, so the build verdict survives that too.
        let (_, verdict, _) = row(&dir, "cargo check 2>&1 | head -5");
        assert_ne!(verdict, VERDICT_PASS, "still not a pass through `| head`");

        // And the same pipelines over a crate that DOES build must still pass —
        // the fix must not manufacture failures.
        std::fs::write(dir.join("src/main.rs"), "fn main() { println!(\"ok\"); }\n").unwrap();
        for cmd in ["cargo check", "cargo check 2>&1 | tail -20"] {
            assert_eq!(row(&dir, cmd).1, VERDICT_PASS, "{cmd} on a good crate");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `pipefail` is on, so an early stage's failure reaches the record instead of
    /// being masked by whatever happened to run last.
    #[test]
    fn a_failing_stage_is_not_masked_by_a_succeeding_one() {
        let dir = scratch("pipefail");
        // The canonical shape: the thing under judgement fails, the plumbing
        // after it succeeds. Under plain `sh` every one of these recorded a 0.
        for cmd in [
            "false | true",
            "exit 3 | cat",
            "(exit 7) | tail -20",
            "echo hi; false | wc -l",
        ] {
            let (exit, verdict, reason) = row(&dir, cmd);
            assert_eq!(
                verdict, VERDICT_FAIL,
                "`{cmd}` must record a failure (exit {exit:?}, reason {reason:?})"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_definite_nonzero_exit_is_a_tool_error_not_ordinary_output() {
        let dir = scratch("call-failure-visible");
        let tool = ShellTool::in_dir(dir.clone());
        let err = tool
            .call(&serde_json::json!({
                "command": "printf 'compile failed\\n'; exit 7"
            }))
            .expect_err("a definite command failure must cross the tool boundary as Err");
        assert!(err.contains("exit 7"), "{err}");
        assert!(err.contains("compile failed"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_write_paths_stays_read_only_and_reports_effective_scope() {
        let _guard = crate::tests::env_lock();
        let dir = scratch("empty-write-paths-scope");
        let tool = ShellTool::in_dir(dir.clone());
        let error = tool
            .call(&serde_json::json!({
                "command": "printf denied > scope-probe.txt",
                "read_only": false,
                "write_paths": []
            }))
            .expect_err("an explicit empty grant must never become writable");
        assert!(!dir.join("scope-probe.txt").exists());
        assert!(
            error.contains("effective shell scope: filesystem read-only; network disabled"),
            "{error}"
        );
        std::fs::write(dir.join("benchmark.log"), "").unwrap();
        let error = tool
            .call(&serde_json::json!({
                "command": "mktemp ./benchmark-scratch.XXXXXX > benchmark.log",
                "write_paths": ["benchmark.log"]
            }))
            .expect_err("a log-only grant must not permit benchmark scratch files");
        assert!(error.contains("omit write_paths"), "{error}");
        assert!(error.contains("entire process tree"), "{error}");
        let normal_error = tool
            .call(&serde_json::json!({"command": "exit 7"}))
            .expect_err("retain ordinary command failure");
        assert!(
            !normal_error.contains("effective shell scope:"),
            "{normal_error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn headless_yolo_preserves_explicit_scope_and_leaves_omission_unrestricted() {
        let _guard = crate::tests::env_lock();
        let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
        let _task = crate::tests::TestEnvGuard::set("ANGEL_TASK_ACTIVE", "1");
        let dir = scratch("yolo-scope");
        let tool = ShellTool::in_dir(dir.clone());
        assert!(tool.scope(&serde_json::json!({})).unwrap().paths.is_none());
        for mut args in [
            serde_json::json!({"read_only":true}),
            serde_json::json!({"write_paths":[]}),
        ] {
            let scope = tool.scope(&args).unwrap();
            assert_eq!(scope.paths, Some(Vec::new()));
            assert!(scope.policy.mandatory);
            args["command"] = "printf forbidden > blocked.txt".into();
            assert!(tool.call(&args).is_err());
            assert!(!dir.join("blocked.txt").exists());
        }
        assert!(
            tool.scope(
                &serde_json::json!({"read_only":true,"write_paths":["not-yet-created/output.txt"]})
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn runtime_missing_shell_error_has_the_shared_onboarding_hint() {
        let _env = crate::tests::env_lock();
        let dir = scratch("runtime-missing");
        let tool = ShellTool::in_dir(dir.clone());
        let error = tool
            .call(&serde_json::json!({
                "command": "PATH=/nonexistent-runtime-path python3 -m unittest",
            }))
            .expect_err("missing interpreter must be a tool error");
        let parsed = crate::tools::runtime_missing::RuntimeMissing::decode(&error).unwrap();
        assert_eq!(parsed.runtime, "python3");
        assert!(parsed.hint.contains("ANGEL_PYTHON_BIN"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_command_not_found_names_the_missing_program_and_its_sibling() {
        let dir = scratch("call-127-hint");
        let tool = ShellTool::in_dir(dir.clone());
        let err = tool
            .call(&serde_json::json!({
                "command": "cd . && zz-no-such-program-angel -m unittest -v 2>&1"
            }))
            .expect_err("exit 127 is a definite failure");
        assert!(err.contains("exit 127"), "{err}");
        assert!(
            err.contains("`zz-no-such-program-angel` is not on PATH"),
            "the hint must name the missing program: {err}"
        );
        assert!(err.contains("host runtimes:"), "{err}");
        // The measured 2026-09-07 case: `python` on a python3-only host. The
        // child PATH carries the runtime shim, so the command that used to exit
        // 127 now runs python3 — and the hint text for the bare name still
        // names the sibling when asked directly.
        if crate::workspace_lang::resolve_on_path("python").is_none()
            && crate::workspace_lang::resolve_on_path("python3").is_some()
        {
            if crate::workspace_lang::runtime_shims_dir().is_some() {
                match tool.call(&serde_json::json!({
                    "command": "python -c 'import sys; print(sys.version_info[0])'"
                })) {
                    Ok(out) => assert!(out.contains('3'), "{out}"),
                    Err(err) => eprintln!(
                        "SKIP a_command_not_found_names_the_missing_program_and_its_sibling python shim: node/runtime-shim capability absent ({err})"
                    ),
                }
            }
            let hint = crate::workspace_lang::missing_program_hint("python -m unittest -v")
                .expect("bare python is absent from this process's PATH");
            assert!(hint.contains("use `python3` instead"), "{hint}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lifecycle_signal_killed_shell_returns_failed_inconclusive_receipt() {
        let _lock = crate::tests::env_lock();
        let _env = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE", "0");
        let dir = scratch("signal-killed");
        let tool = ShellTool::in_dir(dir.clone());
        let error = tool
            .call(&serde_json::json!({"command":"kill -KILL $$"}))
            .unwrap_err();
        assert!(error.contains("signal 9"), "{error}");
        let call = crate::club::ToolCall {
            id: "killed".into(),
            name: "shell".into(),
            args: serde_json::json!({"command":"kill -KILL $$"}),
        };
        let outcome =
            crate::harness::turn_event_outcome(&call, &format!("tool error: {error}"), false);
        assert_eq!(outcome.execution, crate::harness::ExecutionOutcome::Failed);
        assert_eq!(
            outcome.verification,
            crate::harness::VerificationOutcome::Inconclusive
        );
        let kill = crate::sandbox::process_owner::KillReceipt::from_error(&error).unwrap();
        assert_eq!(kill.signal, Some(9));
        assert_eq!(kill.reason, "signal_death");
        assert_eq!(kill.owner, "unknown_external");
        crate::harness::note_tool_outcome(
            1,
            "shell",
            &call.args,
            &format!("tool error: {error}"),
            "failed",
            true,
            None,
            Some("inconclusive"),
            None,
            error.len(),
        );
        let ledger = crate::harness::tool_ledger_snapshot();
        assert_eq!(ledger.last().unwrap()["status"], "killed");
        eprintln!("LIFECYCLE_KILL_RECEIPT signal=9 execution=Failed verification=Inconclusive");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn t06c_stdin_eof_and_signal_names() {
        let _lock = crate::tests::env_lock();
        let _env = crate::tests::TestEnvGuard::set("ANGEL_EXPERIENCE", "0");
        let dir = scratch("t06c-stdin");
        let tool = ShellTool::in_dir(dir.clone());
        for yolo in ["0", "1"] {
            let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", yolo);
            let started = std::time::Instant::now();
            let out = tool.call(&serde_json::json!({"command":"cat && python3 -c 'import sys; assert sys.stdin.read() == \"\"' && echo stdin-eof", "read_only":false})).unwrap();
            assert!(out.contains("stdin-eof"), "{out}");
            // `call` has waited for the child status and drained its output.
            // This marker is emitted only after both readers observe EOF;
            // scheduler latency (including cold helper startup) is not EOF.
            assert_eq!(out.lines().filter(|line| *line == "stdin-eof").count(), 1);
            eprintln!(
                "T06C_STDIN_RECEIPT yolo={yolo} eof=true elapsed_ms={}",
                started.elapsed().as_millis()
            );
        }
        for signal in ["TERM", "KILL", "SEGV"] {
            let args = serde_json::json!({"command":format!("ulimit -c 0; kill -{signal} $$")});
            let error = tool.call(&args).unwrap_err();
            assert!(error.contains(&format!("killed by SIG{signal}")), "{error}");
            assert!(
                error.contains(&format!("reason=signal:SIG{signal}")),
                "{error}"
            );
            let call = crate::club::ToolCall {
                id: signal.into(),
                name: "shell".into(),
                args,
            };
            let outcome =
                crate::harness::turn_event_outcome(&call, &format!("tool error: {error}"), false);
            assert_eq!(outcome.execution, crate::harness::ExecutionOutcome::Failed);
            eprintln!("T06C_SIGNAL_RECEIPT reason=signal:SIG{signal} execution=Failed");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_no_verdict_exit_is_labelled_without_becoming_a_false_failure() {
        let dir = scratch("call-no-verdict-visible");
        let tool = ShellTool::in_dir(dir.clone());
        let out = tool
            .call(&serde_json::json!({
                "command": "seq 1 200000 | head -2"
            }))
            .expect("SIGPIPE plumbing remains a non-failing tool result");
        assert!(out.contains("shell verdict unavailable"), "{out}");
        assert!(out.contains("reason=sigpipe"), "{out}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The blast radius, pinned. `pipefail` faithfully reports a producer killed
    /// by its own consumer closing the pipe — which is plumbing, not a verdict.
    /// These must be recorded as an explicit non-verdict, NOT as a failure: the
    /// point of the fix is to stop laundering one process's status into another's,
    /// and inventing a build failure out of `head(1)` would be the same sin.
    #[test]
    fn a_stage_killed_by_sigpipe_yields_no_verdict_not_a_failure() {
        let dir = scratch("sigpipe");
        for cmd in ["yes | head -1", "seq 1 200000 | head -2"] {
            let (exit, verdict, reason) = row(&dir, cmd);
            assert_eq!(exit, Some(141), "`{cmd}` is 128+SIGPIPE under pipefail");
            assert_eq!(verdict, VERDICT_NONE, "`{cmd}` is not a failing command");
            assert_eq!(reason, Some("sigpipe"));
        }
        // Commands that simply succeed through a pipe are untouched by all this.
        for cmd in ["echo hi | tail -20", "true | true", "ls | wc -l"] {
            assert_eq!(row(&dir, cmd).1, VERDICT_PASS, "{cmd}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The shell we resolve must actually propagate pipeline failure; if it ever
    /// silently degrades to a POSIX `sh`, the ledger says so instead of lying.
    #[test]
    fn the_resolved_shell_reports_whether_it_can_be_trusted() {
        let shell = shell_invocation();
        assert!(
            shell.pipefail,
            "no shell on this box propagates pipeline failure (resolved `{}`); piped commands \
             will correctly record `no_verdict`, but the execution fix is inert",
            shell.program
        );
        // The claim on the tin is the claim in the record.
        let dir = scratch("trust");
        assert_eq!(row(&dir, "false | true").1, VERDICT_FAIL);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shell_guidance_and_real_denials_do_not_police_legitimate_commands() {
        use crate::harness::Tool;
        let _env = crate::tests::env_lock();
        let dir = scratch("flow01-guidance");
        let tool = ShellTool::in_dir(dir.clone());
        let def = tool.def();
        for needle in [
            "earliest prerequisite failure",
            "usable input before dependent measurements",
            "allowed scratch",
            "authorized reference paths",
            "zero performance",
            "user-visible scope change",
            "never omit or auto-remove",
        ] {
            assert!(
                def.description.contains(needle),
                "missing guidance {needle:?} in {}",
                def.description
            );
        }
        let params = def.params.to_string();
        assert!(params.contains("valid `$url`"));
        assert!(params.contains("quiet success"));
        assert!(params.contains("quoted text"));
        assert!(params.contains("permission failure, not zero performance"));
        assert!(params.contains("request a user-visible scope change instead of omitting"));
        assert!(!params.to_lowercase().contains("omit read_only"));

        let empty_grant = tool
            .call(&serde_json::json!({
                "command": "printf denied > scope-probe.txt",
                "read_only": false,
                "write_paths": []
            }))
            .expect_err("empty write_paths remains a real denial");
        assert!(!dir.join("scope-probe.txt").exists());
        assert!(
            empty_grant.contains("effective shell scope: filesystem read-only; network disabled"),
            "{empty_grant}"
        );

        let read_only = tool
            .call(&serde_json::json!({
                "command": "printf denied > readonly-probe.txt",
                "read_only": true
            }))
            .expect_err("explicit read_only remains authoritative");
        assert!(!dir.join("readonly-probe.txt").exists());
        assert!(
            read_only.contains("read-only")
                || read_only.contains("cannot run a command that writes"),
            "{read_only}"
        );

        let quoted = tool
            .call(&serde_json::json!({
                "command": "printf '%s\\n' 'ffmpeg -i missing.mp4'"
            }))
            .expect("quoted ffmpeg text is not a path-existence gate");
        assert!(quoted.contains("ffmpeg -i missing.mp4"), "{quoted}");

        let url = tool
            .call(&serde_json::json!({
                "command": r#"url=https://example.test/clip.mp4; printf 'ffmpeg -i "%s"\n' "$url""#
            }))
            .expect("a valid $url is not empty-input evidence");
        assert!(url.contains("https://example.test/clip.mp4"), "{url}");
        assert!(!url.contains("empty URL"), "{url}");

        let quiet = tool
            .call(&serde_json::json!({ "command": "true" }))
            .expect("quiet success is not zero-frame evidence");
        assert!(!quiet.contains("zero-frame"), "{quiet}");
        assert!(!quiet.contains("empty URL"), "{quiet}");

        let authorized = tool
            .call(&serde_json::json!({ "command": "printf authorized\\n" }))
            .expect("authorized external shell command retains existing behavior");
        assert!(authorized.contains("authorized"), "{authorized}");

        let ffmpeg = tool.call(&serde_json::json!({
            "command": "ffmpeg -hide_banner -loglevel error -f lavfi -i color=c=black:s=2x2:d=0.04 -f null -"
        }));
        match ffmpeg {
            Ok(out) => {
                assert!(!out.contains("empty URL"), "{out}");
                assert!(!out.contains("zero-frame"), "{out}");
            }
            Err(err) => {
                assert!(!err.contains("empty URL"), "{err}");
                assert!(
                    err.contains("exit")
                        || err.contains("not found")
                        || err.contains("No such file")
                        || err.contains("ffmpeg"),
                    "ffmpeg must fail with an actual tool error, not a guessed gate: {err}"
                );
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;

    #[test]
    fn d02_regular_redirection_keeps_numeric_and_dash_file_targets() {
        assert_eq!(
            literal_output_redirection_targets("printf ok > 123; printf ok > -").unwrap(),
            vec!["123", "-"]
        );
        assert!(
            literal_output_redirection_targets("printf ok 2>&1; printf ok 2>&-")
                .unwrap()
                .is_empty()
        );
    }
}
