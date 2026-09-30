//! ⠧ v — what the tests and checks say about the code.
//!
//! Every fact here is a measurement: the model's own test runs, the pinned
//! acceptance command, the post-write check, a confirming re-run of the last
//! green, the task's own test files and editable paths. None of them vetoes
//! anything. At a tool result a fact is a route on the warpath tail; at the
//! model's answer it is a stop fact for the checkpoint (`q_stop.rs`).

use super::{Primary, Raise, Route, Sub};
use crate::agent::harness::*;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub(crate) const CELL: char = '⠧';
pub(crate) const RED: Route = Route::new(CELL, '⠁');
pub(crate) const ACCEPTANCE: Route = Route::new(CELL, '⠃');
pub(crate) const POST_WRITE: Route = Route::new(CELL, '⠉');
pub(crate) const RED_STREAK: Route = Route::new(CELL, '⠙');
pub(crate) const FLAKY: Route = Route::new(CELL, '⠑');
pub(crate) const UNTESTED: Route = Route::new(CELL, '⠋');
pub(crate) const TESTS_EDITED: Route = Route::new(CELL, '⠛');
pub(crate) const OUT_OF_SCOPE: Route = Route::new(CELL, '⠓');
pub(crate) const POSTURE: Route = Route::new(CELL, '⠊');
pub(crate) const POSTURE_COMPACT: Route = Route::new(CELL, '⠚');

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "verification",
    surface: "what the tests and checks say about the code",
    subs: &[
        Sub {
            route: RED,
            name: "red",
            signal: "the last test run on this exact code failed (run below)",
            action: "",
            ideas: "- The answer is yours either way; if something still fails, say what.",
            pages: &[
                "Your last test run on this exact code failed, so the task is not finished, and there is budget left to fix it.",
                "Read the failure below, change the code, and run the tests again.",
                "If something outside the code blocks you, such as a missing tool or a broken environment, say what it is and answer again.",
            ],
        },
        Sub {
            route: ACCEPTANCE,
            name: "acceptance",
            signal: "the task's pinned acceptance command is red (receipt below)",
            action: "",
            ideas: "",
            pages: &[
                "The task's operator-pinned acceptance command is still RED.",
                "This is a hard completion contract, not an advisory verifier: fix the reported failure before claiming completion.",
                "Do not redefine, bypass, mask, or replace the command.",
                "If the contract cannot be satisfied, report the concrete blocker.",
            ],
        },
        Sub {
            pages: &[],
            route: POST_WRITE,
            name: "post-write",
            signal: "the project check after this edit failed (diagnostics below)",
            action: "fix the first diagnostic before the next edit",
            ideas: "- A check that compiles is not a check that the behavior holds.",
        },
        Sub {
            route: RED_STREAK,
            name: "red-streak",
            signal: "three test runs in a row failed",
            action: "",
            ideas: "- Separate an implementation defect from a missing prerequisite or a broken \
                    test environment.",
            pages: &[
                "VERIFICATION RECOVERY: 3 consecutive verification failures detected.",
                "Pause speculative edits and inspect the first failing diagnostic.",
                "If errors span multiple functions, types, or borrow lifetimes, stop micro-patching with str_replace and use write_file to rewrite the module cleanly.",
                "Do not re-run tests without changing code.",
                "Report infrastructure failures honestly; never discard unrelated changes or assume a clean baseline exists.",
            ],
        },
        Sub {
            route: FLAKY,
            name: "flaky",
            signal: "a passing run did not hold when re-run on the same code (failing run below)",
            action: "",
            ideas: "- One pass can be luck.",
            pages: &[
                "Your last passing test run did not hold: angelX re-ran it on the same code and it failed.",
                "The solution passes by luck; something depends on randomness, timing, iteration order or state shared between tests or runs.",
                "Find that and fix it so the tests pass every run.",
                "Re-running until green is not a fix.",
            ],
        },
        Sub {
            route: UNTESTED,
            name: "untested",
            signal: "the workspace changed after the last test run",
            action: "",
            ideas: "",
            pages: &[
                "You edited the workspace but have not run a verifier since the latest edit.",
                "Before claiming completion, run the smallest relevant `check`, `run_tests`, `lint`, `fmt --check`, or equivalent repository command.",
                "One relevant green verifier is sufficient; do not follow it with broader or overlapping checks unless the task explicitly requires them.",
                "If verification cannot run, state the concrete blocker and the unverified risk in your final answer.",
                "A real verifier attempt, even when red or unavailable, is sufficient evidence for an honest blocker report.",
            ],
        },
        Sub {
            route: TESTS_EDITED,
            name: "tests-edited",
            signal: "test files that came with the task were changed (paths below)",
            action: "",
            ideas: "- A pass that leans on edited task tests says little about the code.",
            pages: &[
                "You changed test files that came with the task.",
                "Those tests are the task's contract: leave them as they were.",
                "Restore them (for example `git checkout -- <file>`) and keep your fix in the source.",
                "To run tests that are skipped, run a copy or restore the file afterwards.",
                "If the task asked you to change these tests, say so and answer again.",
            ],
        },
        Sub {
            pages: &[],
            route: OUT_OF_SCOPE,
            name: "out-of-scope",
            signal: "an edit landed outside the task's evaluated paths (paths below)",
            action: "move the change into an editable path; the evaluated copy drops this one",
            ideas: "",
        },
        Sub {
            route: POSTURE,
            name: "posture",
            signal: "the verification posture",
            action: "",
            ideas: "",
            pages: &[
                "Verification posture: after editing code, run the smallest relevant verifier.",
                "One conclusive green test or build on the unchanged workspace is enough; do not stack broader, overlapping tests, builds, lint, or vet commands unless the task explicitly requires distinct gates or the first verifier produced a diagnostic that demands one.",
            ],
        },
        Sub {
            route: POSTURE_COMPACT,
            name: "posture-compact",
            signal: "the verification posture, compact core",
            action: "",
            ideas: "",
            pages: &[
                "After editing code, run the smallest relevant verifier and fix failures from its diagnostics.",
                "Respect explicit no-build or external-verification instructions and report pending checks honestly.",
                "Complete all requested deliverables and distinct required gates; do not stack overlapping verifiers on unchanged code.",
            ],
        },
    ],
};

/// The model's last red test run, kept for the stop checkpoint's `⠧⠁` fact.
pub(crate) struct RedRun {
    /// The workspace it failed on; the fact applies only to that same code.
    pub(crate) workspace: Option<u64>,
    pub(crate) label: String,
    pub(crate) tail: String,
}

impl RedRun {
    pub(crate) fn fact(&self) -> Raise {
        Raise::new(RED, format!("Last run, {}:\n{}", self.label, self.tail))
    }
}

/// Test runs that failed in a row; the third throws `⠧⠙`, once per streak.
#[derive(Default)]
pub(crate) struct RedStreak {
    failures: usize,
}

impl RedStreak {
    pub(crate) fn observe(&mut self, outcome: VerificationOutcome) -> Option<Raise> {
        match outcome {
            VerificationOutcome::Failed => self.failures += 1,
            VerificationOutcome::Passed => self.failures = 0,
            _ => return None,
        }
        (outcome == VerificationOutcome::Failed && self.failures == 3)
            .then(|| Raise::new(RED_STREAK, "3 test runs in a row failed".to_string()))
    }
}

/// Where the model last ran a test of any kind — a typed verifier or a test
/// run inside a shell command. `⠧⠋` means the workspace moved after it: a fact
/// about what the model saw, not the attested verdict the reward ledger keeps
/// (a shell green is never attested there).
#[derive(Default)]
pub(crate) struct Untested {
    last_attempt: Option<u64>,
    edited_since: bool,
}

impl Untested {
    pub(crate) fn note_edit(&mut self) {
        self.edited_since = true;
    }

    pub(crate) fn note_attempt(&mut self, workspace: Option<u64>) {
        self.last_attempt = workspace;
        self.edited_since = false;
    }

    /// A declared code edit since the last test attempt, or bytes that moved
    /// since it (an opaque shell edit), prose-only edits aside.
    pub(crate) fn fact(
        &self,
        initial: Option<u64>,
        current: Option<u64>,
        prose_only: Option<u64>,
    ) -> Option<Raise> {
        let moved = match (initial, current) {
            (Some(initial), Some(current)) => {
                current != initial
                    && self.last_attempt != Some(current)
                    && prose_only != Some(current)
            }
            _ => false,
        };
        (self.edited_since || moved).then(|| Raise::new(UNTESTED, None))
    }
}

/// Whether a started call ran tests, for [`Untested`]. `self_authored_only`
/// is the opt-in guard: a green that only ran tests written this turn is not a
/// test of the code.
pub(crate) fn is_test_attempt(
    registry: &ToolRegistry,
    call: &ToolCall,
    result: &str,
    self_authored_only: bool,
) -> bool {
    !self_authored_only
        && (verification_outcome(call, result).is_some()
            || is_progress_verifier_call(&call.name, &call.args)
            || shell_runs_tests_anywhere(call)
            || registry
                .routed_execution(call, result)
                .is_some_and(|entry| {
                    entry.outcome.verification != VerificationOutcome::NotApplicable
                }))
}

/// The model's last passing test run and the workspace it passed on.
pub(crate) struct GreenRun {
    pub(crate) call: ToolCall,
    /// The workspace it passed on; the re-run happens only on that same code.
    pub(crate) workspace: Option<u64>,
    /// `run_tests` standing in for a shell run angelX will not repeat as-is.
    pub(crate) substitute: bool,
}

/// Extra runs of the model's last passing test at its first stop:
/// `ANGEL_CONFIRM_GREEN_RUNS` (0-5) when set; unset, see
/// [`confirm_green_by_chance`]. It repeats only what the model already chose
/// to run. Opt-in on the evidence: a two-seed polyglot-v1 A/B on DeepSeek V4.1
/// Flash (272 tasks per arm) solved 267 with it at 2 against 269 without, for
/// 21% more agent time; 255 greens re-run, one flaky pass caught
/// (cpp-robot-name).
pub(crate) fn confirm_green_extra_runs(_competition: bool) -> usize {
    std::env::var("ANGEL_CONFIRM_GREEN_RUNS")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map_or(0, |runs| runs.min(5))
}

/// Extra confirming runs when `ANGEL_CONFIRM_GREEN_RUNS` is unset but the code
/// the model wrote draws on chance (see [`edits_depend_on_chance`]). Two runs
/// of a test that fails half the time catch it three times in four.
pub(crate) const CHANCE_CONFIRM_GREEN_RUNS: usize = 2;

/// Whether the unset-`ANGEL_CONFIRM_GREEN_RUNS` default applies: task mode,
/// where nobody reviews the answer, and never in competition. An explicit
/// value, including 0, is the operator's choice and stands.
pub(crate) fn confirm_green_by_chance(task_active: bool, competition: bool) -> bool {
    task_active && !competition && std::env::var_os("ANGEL_CONFIRM_GREEN_RUNS").is_none()
}

/// Calls whose presence makes a passing test run a sample rather than a proof:
/// random numbers, clocks, threads. A plain substring scan over the source the
/// model edited; a false hit costs two test re-runs.
const CHANCE_MARKERS: &[&str] = &[
    // C and C++
    "rand(",
    "random_device",
    "mt19937",
    "_distribution<",
    "std::chrono",
    "std::thread",
    "std::async",
    // Rust
    "rand::",
    "thread_rng",
    "SystemTime",
    "Instant::now",
    "thread::spawn",
    "tokio::spawn",
    // Python
    "import random",
    "from random",
    "random.",
    "uuid",
    "time.time",
    "datetime.now",
    "threading",
    "asyncio",
    // JavaScript and TypeScript
    "Math.random",
    "crypto.random",
    "Date.now",
    "new Date(",
    "setTimeout",
    "setInterval",
    // Go and Java
    "math/rand",
    "time.Now",
    "go func",
    "new Random(",
    "ThreadLocalRandom",
    "currentTimeMillis",
];

/// Whether any non-test source file the model edited draws on chance.
/// polyglot-v1 cpp-robot-name: Grok 4.7 wrote a name generator that reused a
/// released name about half the time; one green run was accepted in 5 of 14
/// angelX runs. Other harnesses caught it only when their first run happened to
/// fail.
pub(crate) fn edits_depend_on_chance(
    workspace: &Path,
    edited: &std::collections::BTreeSet<String>,
) -> bool {
    edited
        .iter()
        .filter(|path| !is_test_path(path))
        .any(|path| {
            let path = Path::new(path);
            let file = if path.is_absolute() {
                path.to_path_buf()
            } else {
                workspace.join(path)
            };
            std::fs::read_to_string(file)
                .is_ok_and(|source| CHANCE_MARKERS.iter().any(|marker| source.contains(marker)))
        })
}

/// Re-run the model's last passing test call up to `extra` more times on the
/// unchanged workspace, judged exactly as the turn judges any tool result.
/// `Ok(runs)` when every run passed; `Err((run, output))` at the first that did
/// not. Runs stop once they have taken `ANGEL_CONFIRM_GREEN_SECS` (default 60),
/// so a slow suite is not repeated at length. polyglot-v1 cpp-robot-name
/// passed about one run in four; a single green run was accepted and the
/// grader's run failed.
pub(crate) fn confirm_green_run(
    registry: &ToolRegistry,
    call: &ToolCall,
    extra: usize,
    cancel: &AtomicBool,
) -> Result<usize, (usize, String)> {
    let budget = Duration::from_secs(env_usize("ANGEL_CONFIRM_GREEN_SECS", 60) as u64);
    let started = Instant::now();
    let mut runs = 0;
    while runs < extra && started.elapsed() < budget && !cancel.load(Ordering::Relaxed) {
        runs += 1;
        let output = match registry.dispatch_with_cancel(&call.name, &call.args, Some(cancel)) {
            Ok(output) => output,
            Err(error) => format!("tool error: {error}"),
        };
        let executed =
            turn_event_outcome(call, &output, false).execution == ExecutionOutcome::Succeeded;
        if !executed || verification_outcome(call, &output) == Some(VerificationOutcome::Failed) {
            return Err((runs, output));
        }
    }
    Ok(runs)
}

/// A shell test run angelX will not re-run as-is: an `a || b` fallback can turn
/// a failing test green (`./test || ctest` passes when CTest has nothing
/// registered). Recognised when the command without its fallbacks is a test run.
pub(crate) fn test_run_behind_fallback(call: &ToolCall) -> bool {
    if call.name != "shell" {
        return false;
    }
    let command = crate::agent::tools::shell::shell_command_arg(&call.args).unwrap_or("");
    let Some((primary, _)) = command.split_once("||") else {
        return false;
    };
    let primary: String = primary
        .chars()
        .filter(|ch| !matches!(ch, '(' | ')' | '{' | '}'))
        .collect();
    let args = serde_json::json!({"command": primary.trim()});
    let probe = ToolCall {
        id: String::new(),
        name: "shell".into(),
        args: args.clone(),
    };
    is_verification_call(&probe) || is_progress_verifier_call("shell", &args)
}

/// A short label for a test call: its tool, and its command when it has one.
pub(crate) fn run_label(call: &ToolCall) -> String {
    let detail = match call.name.as_str() {
        "shell" => crate::agent::tools::shell::shell_command_arg(&call.args)
            .unwrap_or("")
            .to_string(),
        "cargo" => call
            .args
            .get("args")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };
    if detail.is_empty() {
        format!("`{}`", call.name)
    } else {
        format!(
            "`{}: {}`",
            call.name,
            detail.chars().take(120).collect::<String>()
        )
    }
}

/// The last `limit` characters of `text`, trailing whitespace trimmed.
pub(crate) fn tail_chars(text: &str, limit: usize) -> String {
    let text = text.trim_end();
    let skip = text.chars().count().saturating_sub(limit);
    text.chars().skip(skip).collect()
}

/// How much of a failing run's output the ledger keeps.
pub(crate) const RUN_TAIL_CHARS: usize = 1_500;

/// Test files tracked at `HEAD` that now differ from it. `None` outside Git.
pub(crate) fn changed_test_files(root: &Path) -> Option<Vec<String>> {
    Some(
        crate::agent::harness::workspace_state::changed_tracked_paths(root)?
            .into_iter()
            .filter(|path| is_test_path(path))
            .collect(),
    )
}

/// `⠧⠛` when a test file that came with the task changed during this turn;
/// files already changed before it are not the model's doing.
pub(crate) fn tests_edited_fact(root: &Path, changed_at_start: &[String]) -> Option<Raise> {
    let changed = changed_test_files(root)?
        .into_iter()
        .filter(|path| !changed_at_start.contains(path))
        .collect::<Vec<_>>();
    (!changed.is_empty()).then(|| Raise::new(TESTS_EDITED, changed.join("\n")))
}

/// A test or spec file by name: `*_test.*`, `*.test.*`, `*.spec.*`,
/// `test_*.py`, or anything under a `test`/`tests`/`spec`/`__tests__` directory.
pub(crate) fn is_test_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    let name = path
        .rsplit('/')
        .next()
        .unwrap_or(&path)
        .to_ascii_lowercase();
    let stem = name.split('.').next().unwrap_or(&name);
    path.split('/').rev().skip(1).any(|dir| {
        matches!(
            dir.to_ascii_lowercase().as_str(),
            "test" | "tests" | "spec" | "specs" | "__tests__"
        )
    }) || name.contains(".test.")
        || name.contains(".spec.")
        || stem.ends_with("_test")
        || stem.ends_with("_spec")
        || stem.ends_with("test") && stem.len() > 4 && name.ends_with(".java")
        || stem.starts_with("test_")
}

/// The paths a task declares as its editable surface, when it declares one:
/// the sealed-task allowlist `ANGEL_TASK_EDITABLE_PATHS_JSON` (whose file-tool
/// edits are already refused outside it, so this catches shell edits), else a
/// Yukon `benchmark.json` in the workspace (`editablePaths` and
/// `optionalEditablePaths`, and every track's for a schema-v2 manifest). Edits
/// elsewhere are not evaluated.
pub(crate) fn task_edit_scope(workspace: &Path) -> Option<Vec<String>> {
    if let Ok(raw) = std::env::var("ANGEL_TASK_EDITABLE_PATHS_JSON") {
        let scope: Vec<String> = serde_json::from_str::<Vec<String>>(&raw)
            .ok()?
            .into_iter()
            .map(|path| path.trim().trim_matches('/').to_string())
            .filter(|path| !path.is_empty())
            .collect();
        return (!scope.is_empty()).then_some(scope);
    }
    let text = std::fs::read_to_string(workspace.join("benchmark.json")).ok()?;
    let manifest: Value = serde_json::from_str(&text).ok()?;
    let mut scope = Vec::new();
    let mut take = |value: &Value| {
        for key in ["editablePaths", "optionalEditablePaths"] {
            if let Some(list) = value.get(key).and_then(Value::as_array) {
                scope.extend(
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(|path| path.trim_matches('/').to_string()),
                );
            }
        }
    };
    take(&manifest);
    if let Some(tracks) = manifest.get("tracks").and_then(Value::as_array) {
        tracks.iter().for_each(&mut take);
    }
    scope.sort();
    scope.dedup();
    (!scope.is_empty()).then_some(scope)
}

/// `⠧⠓` when a mutation lands outside the declared edit scope, once per path
/// per turn. polyglot-v1 rust-doubly-linked-list went green locally on edits
/// to Cargo.toml and src/pre_implemented.rs that the evaluator discards.
pub(crate) fn out_of_scope_fact(
    scope: Option<&[String]>,
    workspace: &Path,
    call: &ToolCall,
    noted: &mut std::collections::HashSet<String>,
) -> Option<Raise> {
    let scope = scope?;
    let mut outside = Vec::new();
    crate::knowledge::cut::for_each_mutation_target_path(&call.name, &call.args, |path| {
        let relative = Path::new(path)
            .strip_prefix(workspace)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| std::path::PathBuf::from(path));
        let relative = relative
            .to_string_lossy()
            .trim_start_matches("./")
            .to_string();
        let inside = scope
            .iter()
            .any(|allowed| relative == *allowed || relative.starts_with(&format!("{allowed}/")));
        if !inside && noted.insert(relative.clone()) {
            outside.push(relative);
        }
        false
    });
    (!outside.is_empty()).then(|| {
        Raise::new(
            OUT_OF_SCOPE,
            format!(
                "Outside the editable paths: {}\nEditable paths: {}",
                outside.join(", "),
                scope.join(", ")
            ),
        )
    })
}

/// One run of the task's pinned acceptance command.
#[derive(Debug)]
pub(crate) struct TaskAcceptResult {
    pub(crate) passed: bool,
    pub(crate) result_class: &'static str,
    pub(crate) summary: String,
    pub(crate) elapsed_ms: u128,
    /// The end of a failing run's output; empty when it passed.
    pub(crate) output_tail: String,
}

impl TaskAcceptResult {
    /// The `⠧⠃` / `⠧⠑` fact for a run that did not pass.
    pub(crate) fn fact(&self) -> Option<Raise> {
        if self.passed {
            return None;
        }
        let route = if self.result_class == "flaky" {
            FLAKY
        } else {
            ACCEPTANCE
        };
        let evidence = if self.output_tail.is_empty() {
            self.summary.clone()
        } else {
            format!(
                "{}\nFailing run output:\n{}",
                self.summary, self.output_tail
            )
        };
        // The summary and the failing run's output are what the model reads.
        Some(Raise::inline(route, evidence))
    }
}

/// Run the task acceptance command until it has passed
/// `ANGEL_TASK_ACCEPT_REPEATS` times in a row (default 3) or failed once. One
/// green run is not proof: a solution that depends on randomness, timing or
/// state shared between tests can pass by luck. On polyglot-v1
/// cpp-robot-name, a `reset()` that released old names passed about one run
/// in four, and the single acceptance run happened to be one of them. Repeats
/// stop once the proof has taken `ANGEL_TASK_ACCEPT_REPEAT_SECS` (default 60)
/// in total, so a slow suite is not run three times. A failure after an earlier
/// pass comes back as `flaky`, with the failing output.
pub(crate) fn run_task_accept(command: &str, workspace: &Path) -> TaskAcceptResult {
    let repeats = env_usize("ANGEL_TASK_ACCEPT_REPEATS", 3).clamp(1, 10);
    let repeat_budget = Duration::from_secs(env_usize("ANGEL_TASK_ACCEPT_REPEAT_SECS", 60) as u64);
    let started = Instant::now();
    let mut result = run_task_accept_once(command, workspace);
    let mut runs = 1;
    while result.passed && runs < repeats && started.elapsed() < repeat_budget {
        let next = run_task_accept_once(command, workspace);
        runs += 1;
        if !next.passed {
            return TaskAcceptResult {
                passed: false,
                result_class: "flaky",
                summary: format!(
                    "task acceptance is nondeterministic: it passed {} run(s), then failed on run \
                     {runs} of {repeats} ({}).",
                    runs - 1,
                    next.summary
                ),
                elapsed_ms: started.elapsed().as_millis(),
                output_tail: next.output_tail,
            };
        }
        result = next;
    }
    if result.passed && runs > 1 {
        result.summary = format!("{} ({runs} consecutive runs)", result.summary);
    }
    result.elapsed_ms = started.elapsed().as_millis();
    result
}

fn run_task_accept_once(command: &str, workspace: &Path) -> TaskAcceptResult {
    let started = Instant::now();
    let timeout =
        Duration::from_secs(env_usize("ANGEL_TASK_ACCEPT_TIMEOUT_SECS", 120).clamp(5, 600) as u64);
    let Ok((output, timed_out)) =
        crate::agent::harness::exec::sandboxed_workspace_sh(command, workspace, workspace)
            .and_then(|process| crate::agent::harness::exec::output_timed(process, Some(timeout)))
    else {
        return TaskAcceptResult {
            passed: false,
            result_class: "spawn_error",
            summary: "task acceptance command could not start".to_string(),
            elapsed_ms: started.elapsed().as_millis(),
            output_tail: String::new(),
        };
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    let result = crate::agent::tools::build::parse_test_result(&combined);
    let libtest = combined.contains("test result:");
    let passed = output.status.success()
        && !timed_out
        && (!libtest || (result.passed > 0 && result.failed == 0));
    let status = if timed_out {
        "timed out".to_string()
    } else {
        output
            .status
            .code()
            .map(|code| format!("exit {code}"))
            .unwrap_or_else(|| "terminated by signal".to_string())
    };
    TaskAcceptResult {
        passed,
        result_class: if timed_out {
            "timeout"
        } else if passed {
            "passed"
        } else {
            "failed"
        },
        summary: if libtest {
            format!(
                "task acceptance {status}: {} passed / {} failed",
                result.passed, result.failed
            )
        } else {
            format!("task acceptance {status}")
        },
        elapsed_ms: started.elapsed().as_millis(),
        output_tail: if passed {
            String::new()
        } else {
            tail_chars(&combined, RUN_TAIL_CHARS)
        },
    }
}
