use super::*;
/// Whether a successful direct mutation creates behavior that should block an
/// unsupported completion claim. Known prose/data-only edits have no meaningful
/// build/test command, so do not spend model hops inventing one. Unknown,
/// deletion-only, opaque, formatting, integration, and code-mode mutations stay
/// conservative and require verification.
pub(crate) fn mutation_requires_verification(call: &ToolCall) -> bool {
    if !is_mutation_call(call) {
        return false;
    }
    let mut empty = true;
    let mut needs = false;
    crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
        empty = false;
        if !is_prose_only_path(path) {
            needs = true;
            return true;
        }
        false
    });
    empty || needs
}

/// Whether a call is an explicit verification attempt. A red test still
/// counts as an attempt: after seeing real failure evidence the model may report
/// the blocker. The policy prevents unsupported completion claims; it does not
/// loop forever demanding green from a broken environment.
pub(crate) fn is_verification_call(call: &ToolCall) -> bool {
    match call.name.as_str() {
        "run_tests" | "check" | "lint" | "machine_test" => true,
        "fmt" => call.args.get("check").and_then(Value::as_bool) == Some(true),
        "cargo" => {
            let first = call
                .args
                .get("args")
                .and_then(Value::as_str)
                .unwrap_or("")
                .split_whitespace()
                .next();
            matches!(first, Some("test" | "check" | "clippy"))
                || (first == Some("fmt")
                    && call
                        .args
                        .get("args")
                        .and_then(Value::as_str)
                        .is_some_and(|args| args.contains("--check")))
        }
        "shell" => {
            let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
            shell_command_runs_verifier(command)
        }
        _ => false,
    }
}

/// Progress credit also recognizes builds which do not satisfy the separate
/// verify-before-done contract. Keep that distinction out of completion gates.
pub(crate) fn is_progress_verifier_call(name: &str, args: &Value) -> bool {
    let call = ToolCall {
        id: String::new(),
        name: name.into(),
        args: args.clone(),
    };
    if is_verification_call(&call) {
        return true;
    }
    if name == "cargo" {
        return args
            .get("args")
            .and_then(Value::as_str)
            .is_some_and(|s| s.split_whitespace().next() == Some("build"));
    }
    if name != "shell" {
        return false;
    }
    let command = crate::agent::tools::shell::shell_command_arg(args).unwrap_or("");
    if command.contains("||") || shell_has_async_control(command) {
        return false;
    }
    shell_pipeline_segments(shell_status_bearing_segment(command)).any(|segment| {
        let words: Vec<_> = segment
            .split_whitespace()
            .skip_while(|s| shell_env_assignment(s))
            .collect();
        let Some(program) = words.first().map(|s| s.rsplit('/').next().unwrap_or(s)) else {
            return false;
        };
        match program {
            "cargo" => words.get(1) == Some(&"build"),
            "npm" | "pnpm" | "yarn" | "bun" => {
                words.get(1) == Some(&"build")
                    || (words.get(1) == Some(&"run") && words.get(2) == Some(&"build"))
            }
            "cmake" => words.get(1) == Some(&"--build"),
            "make" | "ninja" => {
                words.len() == 1 || words.get(1).is_some_and(|s| matches!(*s, "all" | "build"))
            }
            _ => false,
        }
    })
}

fn shell_env_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn ascii_choice(word: &str, options: &[&str]) -> bool {
    options
        .iter()
        .any(|option| word.eq_ignore_ascii_case(option))
}

/// JavaScript test runners a model calls directly or through a package
/// launcher. `npm test` covers only the package script, so a direct `npx jest`
/// was not a test run at all: DeepSeek ran it about 30 times on polyglot-v1
/// js-forth, and every red run counted as a broken tool instead.
const JS_TEST_RUNNERS: &[&str] = &["jest", "vitest", "mocha", "ava", "jasmine", "tap"];

/// `jest …`, `npx jest …`, `bunx vitest …`, `pnpm exec mocha …`, `yarn jest …`,
/// `npm exec -- jest …`, `./node_modules/.bin/jest …`.
fn runs_js_test_runner(program: &str, words: &[&str]) -> bool {
    let subcommand = words.get(1).copied().unwrap_or("");
    let rest = if ascii_choice(program, &["npx", "bunx", "pnpx"]) {
        &words[1..]
    } else if ascii_choice(program, &["npm", "pnpm", "yarn", "bun"])
        && ascii_choice(subcommand, &["exec", "dlx", "x"])
    {
        &words[2..]
    } else if ascii_choice(program, &["pnpm", "yarn"]) {
        &words[1..]
    } else {
        words
    };
    rest.iter()
        .find(|word| !word.starts_with('-'))
        .map(|word| word.rsplit('/').next().unwrap_or(word))
        .is_some_and(|runner| ascii_choice(runner, JS_TEST_RUNNERS))
}

fn shell_segment_runs_verifier(segment: &str) -> bool {
    let words = segment.split_whitespace().collect::<Vec<_>>();
    let mut start = 0;
    while words
        .get(start)
        .is_some_and(|word| shell_env_assignment(word))
    {
        start += 1;
    }
    let words = &words[start..];
    let Some(program) = words
        .first()
        .map(|word| word.rsplit('/').next().unwrap_or(word))
    else {
        return false;
    };
    if runs_js_test_runner(program, words) {
        return true;
    }
    // Match argv in place. `is_verification_call` used to lowercase the
    // whole command on every hop; mixed-case `CARGO TEST` still counts.
    if program.eq_ignore_ascii_case("cargo") {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "check", "clippy"]))
            || (words
                .get(1)
                .is_some_and(|word| word.eq_ignore_ascii_case("fmt"))
                && words
                    .iter()
                    .any(|word| word.eq_ignore_ascii_case("--check")))
    } else if program.eq_ignore_ascii_case("node") {
        words
            .iter()
            .skip(1)
            .any(|word| word.eq_ignore_ascii_case("--test"))
    } else if ascii_choice(program, &["npm", "pnpm", "yarn", "bun"]) {
        words
            .get(1)
            .is_some_and(|word| word.eq_ignore_ascii_case("test"))
            || (words
                .get(1)
                .is_some_and(|word| word.eq_ignore_ascii_case("run"))
                && words
                    .get(2)
                    .is_some_and(|word| word.eq_ignore_ascii_case("test")))
    } else if ascii_choice(program, &["pytest", "pytest-3"]) {
        true
    } else if ascii_choice(program, &["python", "python3"]) {
        words.windows(2).any(|pair| {
            pair[0].eq_ignore_ascii_case("-m") && ascii_choice(pair[1], &["pytest", "unittest"])
        })
    } else if program.eq_ignore_ascii_case("uv") {
        words
            .get(1)
            .is_some_and(|word| word.eq_ignore_ascii_case("run"))
            && (words
                .get(2)
                .is_some_and(|word| ascii_choice(word, &["pytest", "pytest-3"]))
                || words.windows(3).any(|triple| {
                    ascii_choice(triple[0], &["python", "python3"])
                        && triple[1].eq_ignore_ascii_case("-m")
                        && triple[2].eq_ignore_ascii_case("pytest")
                }))
    } else if program.eq_ignore_ascii_case("go") {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "build", "vet"]))
    } else if ascii_choice(program, &["make", "just"]) {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "check"]))
    } else if ascii_choice(program, &["mvn", "mvnw"]) {
        words
            .iter()
            .skip(1)
            .any(|word| ascii_choice(word, &["test", "verify", "package"]))
    } else if ascii_choice(program, &["gradle", "gradlew"]) {
        words.iter().skip(1).any(|word| {
            let task = word.rsplit(':').next().unwrap_or(word);
            ascii_choice(task, &["test", "check", "build"])
        })
    } else if program.eq_ignore_ascii_case("dotnet") {
        words
            .get(1)
            .is_some_and(|word| ascii_choice(word, &["test", "build"]))
    } else if program.eq_ignore_ascii_case("ssh") {
        let mut i = 1;
        while i < words.len() {
            let arg = words[i];
            if arg.starts_with('-') {
                if matches!(
                    arg,
                    "-o" | "-p" | "-i" | "-l" | "-F" | "-c" | "-b" | "-E" | "-e" | "-J"
                ) {
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
                break;
            }
        }
        if i < words.len() {
            let remote_cmd = words[i..].join(" ");
            let trimmed = remote_cmd
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .or_else(|| {
                    remote_cmd
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                })
                .unwrap_or(&remote_cmd);
            shell_command_runs_verifier(trimmed)
        } else {
            false
        }
    } else {
        program.eq_ignore_ascii_case("ctest")
    }
}

/// True when a shell list contains an asynchronous `&` control operator rather
/// than `&&` or an `&>` / `>&` redirection. The scanner respects simple shell
/// quoting/escaping; an ampersand printed as data is not process control.
fn shell_has_async_control(command: &str) -> bool {
    let Some(positions) = shell_control_positions(command) else {
        return true;
    };
    positions.into_iter().any(|index| {
        let bytes = command.as_bytes();
        if bytes[index] != b'&' {
            return false;
        }
        let prev = index.checked_sub(1).and_then(|i| bytes.get(i)).copied();
        let next = bytes.get(index + 1).copied();
        prev != Some(b'&') && next != Some(b'&') && prev != Some(b'>') && next != Some(b'>')
    })
}

/// Byte offsets of unquoted shell control characters. This deliberately models
/// only the lexical property the verifier gate needs; it is not a shell parser.
/// Unclosed quotes fail closed by returning no status-bearing segment later.
fn shell_control_positions(command: &str) -> Option<Vec<usize>> {
    let bytes = command.as_bytes();
    let mut positions = Vec::new();
    let mut single = false;
    let mut double = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' && !single {
            escaped = true;
            continue;
        }
        match byte {
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            // Command/process substitution has its own nested shell grammar;
            // treating an inner separator as top-level can launder a verifier
            // behind an outer successful command. This lightweight gate does
            // not parse nesting, so fail closed on every unquoted form.
            b'`' if !single => return None,
            // `$()` remains active inside double quotes; treating its inner
            // separators as top-level would let an outer successful command
            // launder a failed verifier. Process substitution is active only
            // while unquoted. Any other unquoted grouping is likewise outside
            // this deliberately small lexer, including function definitions.
            b'$' if !single && bytes.get(index + 1) == Some(&b'(') => return None,
            b'<' | b'>' if !single && !double && bytes.get(index + 1) == Some(&b'(') => {
                return None;
            }
            b'(' | b')' | b'{' | b'}' if !single && !double => return None,
            // Here-doc bodies have their own quoting and line-consumption
            // rules. An apparent `; cargo test` inside the body is data, not a
            // status-bearing command, so reject the construct conservatively.
            b'<' if !single && !double && bytes.get(index + 1) == Some(&b'<') => return None,
            // A shell comment suppresses every apparent command after `#` on
            // that line. Reject unquoted comments rather than duplicate the
            // shell's token-boundary grammar in this evidence classifier.
            b'#' if !single && !double => return None,
            b';' | b'\n' | b'\r' | b'|' | b'&' if !single && !double => positions.push(index),
            _ => {}
        }
    }
    if single || double || escaped {
        None
    } else {
        Some(positions)
    }
}

/// Return the final non-empty sequential shell-list segment. Bash reports only
/// this segment's status across `;` and newline boundaries: `cargo test; echo
/// done` reports `echo`, while `echo prep; cargo test` reports the test. Within
/// the returned segment, `&&` and a pipefail pipeline preserve an earlier
/// verifier failure and are therefore observable.
fn shell_status_bearing_segment(command: &str) -> &str {
    let Some(positions) = shell_control_positions(command) else {
        return "";
    };
    let mut end = command.len();
    for index in positions
        .into_iter()
        .rev()
        .filter(|index| matches!(command.as_bytes()[*index], b';' | b'\n' | b'\r'))
    {
        let candidate = command.get(index + 1..end).unwrap_or("");
        if !candidate.trim().is_empty() {
            return candidate;
        }
        end = index;
    }
    command.get(..end).unwrap_or("")
}

/// Conservatively identify a verifier whose failure can reach the shell status.
/// Substring matching is unsafe (`echo cargo test` is not evidence), an `||`
/// branch can mask failure, and an unjoined background job reports launch rather
/// than completion. Only the final `;`/newline list segment owns the status;
/// mandatory `&&` and pipeline members inside it remain eligible.
/// Scans argv in place so ordinary shell hops do not copy the command.
fn shell_command_runs_verifier(command: &str) -> bool {
    !command.contains("||") && !shell_has_async_control(command) && {
        let segment = shell_status_bearing_segment(command);
        shell_pipeline_segments(segment).any(shell_segment_runs_verifier)
    }
}

/// Unquoted members of a status-bearing pipeline/mandatory-AND expression.
/// The lexical scanner owns separator recognition for every verifier path so a
/// quoted `| cargo test` or `& cargo test` can never manufacture evidence.
fn shell_pipeline_segments(segment: &str) -> impl Iterator<Item = &str> {
    let mut start = 0usize;
    shell_control_positions(segment)
        .unwrap_or_default()
        .into_iter()
        .filter(|index| matches!(segment.as_bytes()[*index], b'|' | b'&'))
        .chain(std::iter::once(segment.len()))
        .map(move |end| {
            let item = &segment[start..end];
            start = end.saturating_add(1).min(segment.len());
            item
        })
}

/// Whether one green result is enough completion evidence for the unchanged
/// workspace. Formatting and lint-only checks remain useful but do not suppress
/// a later build/test. Tests, type/build checks, and package builds do.
/// A `cargo test` invocation that exercises the full suite. A positional
/// filter (`cargo test raycast`, `cargo test -- name`) or `--ignored` runs a
/// deliberate slice — targeted evidence, never proof the tree is green, and
/// never grounds to guillotine a run that still has distinct gates pending
/// (the first sol dogfood run lost its fixture check, full suite, and commit
/// to a filtered green arming the post-green guard).
fn cargo_test_is_full(words: &[&str]) -> bool {
    const TARGETING_FLAGS: &[&str] = &[
        "--ignored",
        "--test",
        "--tests",
        "--bench",
        "--benches",
        "--example",
        "--examples",
        "--package",
        "-p",
        "--exclude",
        "--lib",
        "--bin",
        "--bins",
        "--doc",
        "--no-run",
    ];
    words.iter().all(|word| {
        *word == "--"
            || (word.starts_with('-')
                && !TARGETING_FLAGS
                    .iter()
                    .any(|flag| word.eq_ignore_ascii_case(flag)))
    })
}

/// Only the adapter-owned first line carries coverage authority. Diagnostics
/// and path names below it may contain arbitrary strings, including markers.
/// A verifier result that names a *known* coverage gap: the typed check ran,
/// the changed-path inventory was established, and the selected targets left
/// some of those paths uncompiled (`coverage=incomplete`). Unknown coverage —
/// no git, an opaque shell earlier, missing dep-info — is not a gap; the
/// receipt already tells the model to report the scope limitation, and holding
/// the completion gate on it only burns nudges (2026-09-07).
pub(crate) fn verification_result_has_known_gap(result: &str) -> bool {
    let header = result.lines().next().unwrap_or("");
    header.split("; ").any(|part| part == "coverage=incomplete")
}

pub(crate) fn verification_result_covers_changes(result: &str) -> bool {
    let header = result.lines().next().unwrap_or("");
    !header
        .split("; ")
        .any(|part| part == "coverage=incomplete" || part == "coverage=unknown")
}

pub(crate) fn verification_is_completion_sufficient(call: &ToolCall) -> bool {
    match call.name.as_str() {
        // The dedicated tools are the harness's own curated gates and keep
        // their standing contract. Raw `cargo check` below is different: a
        // model-chosen compile probe proves the tree builds, not that the
        // task is done — one arming the wrap-up guard cut run11 off
        // mid-ladder.
        "run_tests" => call
            .args
            .get("args")
            .and_then(Value::as_str)
            .map(|args| args.split_whitespace().collect::<Vec<_>>())
            .is_none_or(|words| cargo_test_is_full(&words)),
        "check" => true,
        "lint" | "fmt" => false,
        "cargo" => call
            .args
            .get("args")
            .and_then(Value::as_str)
            .map(|args| args.split_whitespace().collect::<Vec<_>>())
            .is_some_and(|words| match words.first() {
                Some(&"test") => cargo_test_is_full(&words[1..]),
                _ => false,
            }),
        // A raw shell result proves the shell's final status, not the identity
        // of the executable that produced it. PATH shims, imported bash
        // functions, BASH_ENV, and user-controlled absolute paths can all make
        // a verifier-looking command return zero without running that verifier.
        // Keep shell checks as useful verification attempts, but never let them
        // arm durable-green completion state until the tool boundary can return
        // a typed receipt for an actually pinned direct executable.
        "shell" => false,
        _ => false,
    }
}

fn normalized_verifier_words(raw: &str) -> Option<String> {
    let words = raw.split_whitespace().collect::<Vec<_>>();
    (!words.is_empty()).then(|| words.join(" "))
}

fn cargo_verification_identity(raw: &str) -> Option<String> {
    let normalized = normalized_verifier_words(raw)?;
    let first = normalized.split_whitespace().next()?;
    matches!(first, "test" | "check" | "clippy")
        .then(|| format!("cargo|{normalized}"))
        .or_else(|| {
            (first == "fmt" && normalized.split_whitespace().any(|word| word == "--check"))
                .then(|| format!("cargo|{normalized}"))
        })
}

fn is_plain_shell_words(command: &str) -> bool {
    command.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || ch.is_ascii_whitespace()
            || matches!(
                ch,
                '-' | '_' | '.' | '/' | ':' | '=' | '+' | ',' | '@' | '%'
            )
    })
}

/// Stable semantic identity for a verifier invocation. Tool-call IDs and
/// harmless whitespace are excluded. Raw shell commands keep a separate
/// identity from curated verifier tools: the same command text can resolve to
/// a function or PATH shim, so cross-wrapper cache reuse would cross a trust
/// boundary even when the visible argv looks identical.
pub(crate) fn verification_identity(call: &ToolCall) -> Option<String> {
    if !is_verification_call(call) {
        return None;
    }
    let extra = || {
        call.args
            .get("args")
            .and_then(Value::as_str)
            .and_then(normalized_verifier_words)
    };
    match call.name.as_str() {
        "run_tests" => Some(format!(
            "cargo|test{}",
            extra().map(|args| format!(" {args}")).unwrap_or_default()
        )),
        "check" => Some(format!(
            "cargo|check{}",
            extra().map(|args| format!(" {args}")).unwrap_or_default()
        )),
        "lint" => Some(format!(
            "cargo|clippy{}",
            extra().map(|args| format!(" {args}")).unwrap_or_default()
        )),
        "fmt" => Some("cargo|fmt --check".to_string()),
        "cargo" => call
            .args
            .get("args")
            .and_then(Value::as_str)
            .and_then(cargo_verification_identity),
        "shell" => {
            let command = crate::agent::tools::shell::shell_command_arg(&call.args)?;
            let command = if is_plain_shell_words(command) {
                normalized_verifier_words(command)?
            } else {
                command.trim().to_string()
            };
            Some(format!("shell|{command}"))
        }
        _ => Some(format!("{}|{}", call.name, call.args)),
    }
}

pub(super) fn cached_verification_result(call: &ToolCall, outcome: VerificationOutcome) -> String {
    format!(
        "[reused verifier result: {}] The semantically identical `{}` verifier already ran against this exact workspace state; it was not executed again. Edit the workspace, choose a different relevant verifier, or finish from the existing evidence.",
        outcome.as_str(),
        call.name
    )
}

pub(super) fn sufficient_verification_result(call: &ToolCall) -> String {
    format!(
        "[skipped redundant verifier: passed] This exact `{}` invocation already passed against this unchanged workspace state; it was not executed again. Different tools or arguments require their own result.",
        call.name
    )
}

pub(crate) fn should_activate_final_mile(
    max_hops: Option<usize>,
    completed_hops: usize,
    reserve_hops: usize,
    mutation_seen: bool,
    already_active: bool,
) -> bool {
    reserve_hops > 0
        && mutation_seen
        && !already_active
        && max_hops.is_some_and(|max| max.saturating_sub(completed_hops) <= reserve_hops)
}

pub(crate) fn should_force_final_mile_answer(
    max_hops: Option<usize>,
    completed_hops: usize,
    answer_hops: usize,
    final_mile_active: bool,
) -> bool {
    answer_hops > 0
        && final_mile_active
        && max_hops.is_some_and(|max| max.saturating_sub(completed_hops) <= answer_hops)
}

/// Merge the operator interrupt with an absolute turn deadline for one
/// blocking provider/tool operation. Long-running implementations already
/// honor an `AtomicBool`; this gives them one cooperative signal without
/// mutating the operator-owned flag or leaving a watchdog alive after the
/// operation returns.
pub(crate) fn with_turn_deadline_cancel<R>(
    operator_cancel: &AtomicBool,
    deadline: Option<Instant>,
    operation: impl FnOnce(&AtomicBool) -> R,
) -> R {
    crate::agent::tools::http_transport::with_deadline(deadline, || {
        let Some(deadline) = deadline else {
            return operation(operator_cancel);
        };
        let effective_cancel =
            AtomicBool::new(operator_cancel.load(Ordering::Acquire) || Instant::now() >= deadline);
        let _activity_owner =
            super::super::exec::link_child_owner(&effective_cancel, operator_cancel);
        struct ProcessOwnerTransfer(usize, usize);
        impl Drop for ProcessOwnerTransfer {
            fn drop(&mut self) {
                crate::agent::tools::proc::transfer_owned(self.0, self.1);
            }
        }
        // Dispatch can launch background work under this operation-local flag.
        // Rebind it before the flag disappears, including early return/unwind.
        let _process_owner = ProcessOwnerTransfer(
            &effective_cancel as *const _ as usize,
            operator_cancel as *const _ as usize,
        );
        if effective_cancel.load(Ordering::Acquire) {
            return operation(&effective_cancel);
        }

        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            struct WatcherGuard<'a> {
                done: &'a AtomicBool,
                thread: std::thread::Thread,
            }
            impl Drop for WatcherGuard<'_> {
                fn drop(&mut self) {
                    self.done.store(true, Ordering::Release);
                    self.thread.unpark();
                }
            }

            let watcher = scope.spawn(|| {
                while !done.load(Ordering::Acquire) {
                    if operator_cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
                        effective_cancel.store(true, Ordering::Release);
                        return;
                    }
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    std::thread::park_timeout(remaining.min(Duration::from_millis(25)));
                }
            });
            let _watcher_guard = WatcherGuard {
                done: &done,
                thread: watcher.thread().clone(),
            };
            operation(&effective_cancel)
        })
    })
}

pub(crate) fn final_mile_rejects_inspection(
    active: bool,
    verification_outstanding: bool,
    calls: &[ToolCall],
) -> bool {
    active
        && verification_outstanding
        && !calls
            .iter()
            .any(|call| is_mutation_call(call) || is_verification_call(call))
}

/// Outcome quality for a verifier result. This is intentionally distinct from
/// "the tool returned": several verifier tools return a useful red report as an
/// `Ok`, while older/generic command adapters may omit the process status. Only
/// `Passed` is suitable as durable green evidence.
/// A passing verifier is completion evidence only after the agent has changed
/// the workspace. A baseline-green suite on an unchanged checkout proves the
/// starting point is healthy, not that the requested repair was implemented.
pub(crate) fn is_completion_green(outcome: VerificationOutcome, mutation_seen: bool) -> bool {
    mutation_seen && outcome == VerificationOutcome::Passed
}

/// Whether this result is strong enough to release the edit-scoped
/// verify-before-done gate. A definite raw-shell failure still lets the model
/// disclose a blocker, but an untyped zero exit cannot: it is inconclusive
/// precisely because the shell has not proved which verifier executable ran.
pub(super) fn verification_attempt_releases_gate(
    call: &ToolCall,
    outcome: VerificationOutcome,
) -> bool {
    outcome != VerificationOutcome::NotApplicable
        && (call.name != "shell" || outcome != VerificationOutcome::Inconclusive)
}

fn labeled_count(result: &str, label: &str) -> Option<usize> {
    let words: Vec<&str> = result
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    words.windows(2).find_map(|pair| {
        (pair[1] == label)
            .then(|| pair[0].parse::<usize>().ok())
            .flatten()
    })
}

/// Classify a verifier without trusting model prose or equating dispatch success
/// with a green build. `None` means the call was not an accepted verification
/// attempt (including an approval denial).
pub(crate) fn verification_outcome(call: &ToolCall, result: &str) -> Option<VerificationOutcome> {
    if !is_verification_call(call)
        || result.starts_with("action capsule denied")
        || is_hook_blocked_result(result)
    {
        return None;
    }
    if is_error_result(result) {
        return Some(VerificationOutcome::Failed);
    }
    if result.starts_with("verification inconclusive:") {
        return Some(VerificationOutcome::Inconclusive);
    }
    // Typed verifier adapters own these headers. Later attribution may contain
    // arbitrary filenames/argv such as "9 passed" or "0 errors"; it must not
    // turn an unsupported runner or empty suite into green evidence.
    let typed_summary = matches!(call.name.as_str(), "run_tests" | "check" | "lint");
    let verdict_text = if typed_summary {
        result.lines().next().unwrap_or("")
    } else {
        result
    };
    if typed_summary {
        if result.starts_with("verification inconclusive:") {
            return Some(VerificationOutcome::Inconclusive);
        }
        if result.starts_with("no tests ran") {
            return Some(VerificationOutcome::Failed);
        }
        let expected = match call.name.as_str() {
            "run_tests" => "tests:",
            "check" => "check:",
            _ => "lint:",
        };
        if !verdict_text.starts_with(expected) {
            return Some(VerificationOutcome::Inconclusive);
        }
    }
    if ascii_contains_ignore_case(verdict_text, "timed out") {
        return Some(VerificationOutcome::Failed);
    }
    // The fallback still executes custom project runners, but their counts
    // are observations, not an authenticated verifier verdict. This also
    // covers the previous numeric-reward + `(unlabeled: ...)` format and YOLO
    // receipts; neither may become a green task event or a Caddy recipe.
    if typed_summary && ascii_contains_ignore_case(verdict_text, "unlabeled") {
        return Some(VerificationOutcome::Inconclusive);
    }
    let outcome = match call.name.as_str() {
        "run_tests" => match (
            labeled_count(verdict_text, "passed"),
            labeled_count(verdict_text, "failed"),
        ) {
            (Some(passed), Some(0)) if passed > 0 => VerificationOutcome::Passed,
            (_, Some(failed)) if failed > 0 => VerificationOutcome::Failed,
            _ if ascii_contains_ignore_case(result, "no tests ran") => VerificationOutcome::Failed,
            _ => VerificationOutcome::Inconclusive,
        },
        "check" | "lint" => match labeled_count(verdict_text, "errors") {
            Some(0) => VerificationOutcome::Passed,
            Some(_) => VerificationOutcome::Failed,
            None => VerificationOutcome::Inconclusive,
        },
        "fmt" => {
            if ascii_contains_ignore_case(result, "already rustfmt-clean") {
                VerificationOutcome::Passed
            } else if ascii_contains_ignore_case(result, "needs formatting") {
                VerificationOutcome::Failed
            } else {
                VerificationOutcome::Inconclusive
            }
        }
        "shell" => {
            // ShellTool converts a trustworthy non-zero shell status into a
            // dispatch error, handled above. A zero status is still not green
            // verifier evidence: the string command carries no typed proof of
            // the executable identity (function/PATH/BASH_ENV shadowing).
            VerificationOutcome::Inconclusive
        }
        "machine_test" => {
            if ascii_contains_ignore_case(result, "\"event\": \"released\"")
                && ascii_contains_ignore_case(result, "\"status\": \"done\"")
            {
                VerificationOutcome::Passed
            } else if ascii_contains_ignore_case(result, "\"event\": \"released\"") {
                VerificationOutcome::Failed
            } else {
                VerificationOutcome::Inconclusive
            }
        }
        "cargo" => {
            let args = call.args.get("args").and_then(Value::as_str).unwrap_or("");
            let first = args.split_whitespace().next();
            match first {
                Some("test") => {
                    let parsed = crate::agent::harness::parse_test_result(result);
                    if parsed.all_passed() {
                        VerificationOutcome::Passed
                    } else if parsed.ran() > 0
                        || ascii_contains_ignore_case(result, "could not compile")
                    {
                        VerificationOutcome::Failed
                    } else {
                        VerificationOutcome::Inconclusive
                    }
                }
                Some("check" | "clippy") => {
                    if ascii_contains_ignore_case(result, "error: could not compile") {
                        VerificationOutcome::Failed
                    } else if ascii_contains_ignore_case(result, "[cargo verdict: pass]")
                        || ascii_contains_ignore_case(result, "finished ")
                    {
                        VerificationOutcome::Passed
                    } else {
                        VerificationOutcome::Inconclusive
                    }
                }
                Some("fmt") if args.contains("--check") => {
                    if ascii_contains_ignore_case(result, "[cargo verdict: pass]")
                        || result.trim().is_empty()
                    {
                        VerificationOutcome::Passed
                    } else if ascii_contains_ignore_case(result, "diff in") {
                        VerificationOutcome::Failed
                    } else {
                        VerificationOutcome::Inconclusive
                    }
                }
                _ => VerificationOutcome::Inconclusive,
            }
        }
        _ => VerificationOutcome::Inconclusive,
    };
    Some(outcome)
}

pub(crate) fn turn_event_outcome(call: &ToolCall, result: &str, denied: bool) -> ToolOutcome {
    // Only recognized dispatch headers describe execution. Successful output
    // may discuss cancellation, print test names, or explain what to do "if
    // interrupted"; scanning that body fabricated cancelled/panicked events.
    let header = result.lines().next().unwrap_or("").trim();
    let header = header.trim_start_matches("tool error: ");
    let starts = |prefix: &str| {
        header
            .get(..prefix.len())
            .is_some_and(|text| text.eq_ignore_ascii_case(prefix))
    };
    let execution = if denied
        || result.starts_with("action capsule denied")
        || is_hook_blocked_result(result)
    {
        ExecutionOutcome::Denied
    } else if starts("worker panicked")
        || starts("tool panicked")
        || (is_error_result(result) && starts(&format!("{} panicked:", call.name)))
    {
        ExecutionOutcome::Panicked
    } else if (is_error_result(result)
        && crate::agent::sandbox::process_owner::KillReceipt::from_error(result)
            .is_some_and(|kill| kill.reason == "cancelled"))
        || [
            "cancelled by ",
            "canceled by ",
            "interrupted by ",
            "execution cancelled before spawn",
            "shell command cancelled",
            "cargo command cancelled",
            "cargo test cancelled",
            "cargo clippy cancelled",
            "cargo check cancelled",
            "cargo fmt cancelled",
            "code_mode cancelled",
            "process wait cancelled",
            "process launch cancelled",
            "queued machine test cancelled by operator",
        ]
        .into_iter()
        .any(starts)
    {
        ExecutionOutcome::Cancelled
    } else if ascii_contains_ignore_case(result, "[timed out after ") || is_error_result(result) {
        ExecutionOutcome::Failed
    } else {
        ExecutionOutcome::Succeeded
    };
    let verification = verification_outcome(call, result).unwrap_or_else(|| {
        if (is_verification_call(call) && execution != ExecutionOutcome::Denied)
            || (matches!(
                call.name.as_str(),
                "shell" | "proc_run" | "proc_status" | "proc_wait" | "proc_stop"
            ) && matches!(
                execution,
                ExecutionOutcome::Failed | ExecutionOutcome::Cancelled | ExecutionOutcome::Panicked
            ))
        {
            VerificationOutcome::Inconclusive
        } else {
            VerificationOutcome::NotApplicable
        }
    });
    ToolOutcome {
        execution,
        verification,
    }
}

pub(crate) fn should_nudge_final_verification(
    policy_enabled: bool,
    verification_needed: bool,
    nudges: usize,
    max_nudges: usize,
) -> bool {
    policy_enabled && verification_needed && nudges < max_nudges
}

/// Whether the verification gate is allowed to interrupt the operator at all.
///
/// Both YOLO profiles opted out of modals while keeping this gate (see `yolo`),
/// so they take the silent hold rather than a prompt they turned off.
pub(crate) fn verification_gate_prompts() -> bool {
    matches!(
        crate::platform::yolo::profile(),
        crate::platform::yolo::Profile::Guarded
    )
}

/// Ask the operator whether to let an unverified completion stand.
///
/// The gate retracts an answer the provider already streamed. That used to be
/// announced by a routine strip murmur, which the pane folds into a `(N notes)`
/// counter — so work stalled with no stated reason and no way for the operator
/// to overrule it. Raising the first denial of a turn as a modal makes holding
/// the answer a decision someone made.
///
/// Only the Guarded posture prompts: `yolo` documents that completion and
/// verification gates survive both YOLO profiles, so an unrestricted run keeps
/// today's behavior rather than gaining a modal it opted out of. A headless
/// harness, a dropped UI, and an unanswered prompt all resolve to "hold", so
/// silence can never release a completion claim.
pub(super) fn operator_releases_unverified_completion(workspace: &Path) -> bool {
    if !verification_gate_prompts() {
        return false;
    }
    let decision = crate::agent::approval::ask(
        crate::agent::approval::ApprovalScope::TurnGate("unverified completion".to_string()),
        &format!(
            "The agent edited {} and claimed completion without running a verifier since \
             the last edit.\n\nApprove to accept the answer as it stands; deny to retract it \
             and make the agent verify first.",
            workspace.display()
        ),
    );
    matches!(decision, crate::agent::approval::Decision::Approve)
}

pub(crate) fn accepted_unverified_completion(
    policy_enabled: bool,
    verification_outstanding: bool,
    nudges: usize,
    max_nudges: usize,
) -> bool {
    verification_outstanding
        && !should_nudge_final_verification(
            policy_enabled,
            verification_outstanding,
            nudges,
            max_nudges,
        )
}

#[cfg(test)]
pub(crate) fn looks_like_deferred_action_only(answer: &str) -> bool {
    let text = answer.trim();
    // GLM and other deliberators dump multi-KB "plans" with zero tools; the old
    // 700-char cap let those through and burned whole days of wall clock.
    if text.is_empty() || text.len() > 4_000 {
        return false;
    }
    // Scan in place so a text-only hop does not copy the whole answer
    // just to hunt "I'll analyze" / "submitted".
    const INTENT: &[&str] = &[
        "let me ",
        "i'll ",
        "i will ",
        "i need to ",
        "i’m ",
        "i am ",
        "i'm going to ",
        "i plan to ",
        "i should ",
        "we should ",
        "next i ",
        "starting with ",
        "starting ",
        "beginning ",
        "start by ",
        "first, i'll ",
        "first i’ll ",
        "first i ",
        "i’ll ",
        "before i can ",
        "my approach",
        "step 1",
        "plan:",
        "here's the plan",
        "here is the plan",
    ];
    if !INTENT
        .iter()
        .any(|needle| ascii_contains_ignore_case(text, needle))
    {
        return false;
    }
    const WORK: &[&str] = &[
        "inspect",
        "check",
        "look at",
        "read",
        "recon",
        "pull the lay",
        "locat",
        "brief",
        "council",
        "delegate",
        "discuss",
        "figure out",
        "analyz",
        "investigat",
        "research",
        "explore",
        "understand",
        "review the",
        "map the",
        "think through",
        "consider",
    ];
    if !WORK
        .iter()
        .any(|needle| ascii_contains_ignore_case(text, needle))
    {
        return false;
    }
    const CLOSURE: &[&str] = &[
        "done",
        "completed",
        "fixed",
        "changed",
        "verified",
        "the answer is",
        "i found",
        "i checked",
        "submitted",
        "submission id",
        "zero submissions",
    ];
    !CLOSURE
        .iter()
        .any(|needle| ascii_contains_ignore_case(text, needle))
}
