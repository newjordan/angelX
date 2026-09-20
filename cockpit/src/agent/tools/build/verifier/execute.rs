use super::{Kind, PinnedExecutable, Runtime, SandboxPolicy, ToolOutputProgress, plan::Plan};
use crate::agent::tools::build::{CapturedCommand, TestOutcome, tail, timeout_detail};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

pub(super) fn run(
    executable: &PinnedExecutable,
    plan: &Plan,
    context: (&Path, &serde_json::Value),
    policy: &SandboxPolicy,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: Option<Arc<ToolOutputProgress>>,
) -> Result<String, String> {
    let (workspace, args) = context;
    let started = Instant::now();
    let deadline = crate::agent::harness::tool_timeout().map(|duration| started + duration);
    let mut test_counts = None;
    let mut raw_tail = String::new();
    let mut actual_argvs = plan.argvs.clone();
    if plan.runtime == Runtime::Python {
        // -B stops writes, but existing project .pyc files would still load.
        // This absent path is beside the immutable interpreter; no directory
        // is created, and task processes cannot populate it with stale code.
        let cache = executable
            .path
            .parent()
            .ok_or("Python runtime has no parent")?
            .join(".angel-verifier-no-bytecode");
        match std::fs::symlink_metadata(&cache) {
            Ok(_) => return Err("isolated Python bytecode lookup path already exists; refusing stale-cache verification".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(format!("cannot establish an absent isolated Python bytecode lookup path: {error}")),
        }
        for argv in &mut actual_argvs {
            argv.splice(
                0..0,
                [
                    "-B".into(),
                    "-X".into(),
                    format!("pycache_prefix={}", cache.display()),
                ],
            );
        }
    }
    for argv in &actual_argvs {
        let remaining = deadline.map(|at| at.saturating_duration_since(Instant::now()));
        if remaining.is_some_and(|duration| duration.is_zero()) {
            return Ok(format!(
                "{}: timed out before all selected files were checked",
                plan.kind.summary()
            ));
        }
        super::revalidate_executable(executable, plan.runtime.label())?;
        let mut command = crate::agent::sandbox::command(&executable.path, argv, policy)?;
        crate::agent::tools::build::apply_verifier_env(&mut command, args)?;
        command.current_dir(workspace);
        command.env("PATH", "/usr/local/bin:/usr/bin:/bin");
        if plan.runtime == Runtime::Go {
            command
                .env("GOTOOLCHAIN", "local")
                .env("GOPROXY", "off")
                .env("GOSUMDB", "off")
                .env("GOFLAGS", "")
                .env("GOWORK", "off");
        }
        // Python -I -S is authoritative; explicit removal also prevents child
        // subprocesses from inheriting operator/workspace startup overrides.
        for (name, _) in std::env::vars_os() {
            let text = name.to_string_lossy();
            if text.starts_with("NODE_")
                || text.starts_with("PYTHON")
                || text.starts_with("npm_config_")
                || text.starts_with("LD_")
                || text.starts_with("DYLD_")
            {
                command.env_remove(name);
            }
        }
        command
            .env("PYTEST_DISABLE_PLUGIN_AUTOLOAD", "1")
            .env_remove("PYTEST_ADDOPTS")
            .env_remove("PYTEST_PLUGINS");
        let command_started = Instant::now();
        let captured = crate::agent::harness::output_timed_extensible_cancellable_with_progress(
            command,
            remaining,
            cancel,
            progress.clone(),
        )?;
        let out = CapturedCommand {
            checked_sources: None,
            stdout: String::from_utf8_lossy(&captured.output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&captured.output.stderr).into_owned(),
            success: captured.output.status.success(),
            exit: captured.output.status.code(),
            timed_out: captured.timed_out,
            cancelled: captured.cancelled,
            dur_ms: command_started.elapsed().as_millis(),
            timeout_diag: captured.timeout_diag,
        };
        let actual_command =
            serde_json::json!({"executable": executable.path, "argv": argv}).to_string();
        crate::knowledge::experience::record_cmd_event(
            &crate::knowledge::experience::CmdExperience {
                tool: plan.kind.tool(),
                text: &actual_command,
                exit: out.exit,
                timed_out: out.timed_out,
                dur_ms: out.dur_ms,
                bytes_out: out.stdout.len() + out.stderr.len(),
                shell: crate::knowledge::experience::CmdShell::Direct,
            },
            workspace,
        );
        if out.cancelled {
            return Err(format!(
                "cancelled by operator: native {}\n{}",
                plan.kind.tool(),
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if out.timed_out {
            return Ok(format!("{}: {}", plan.kind.summary(), timeout_detail(&out)));
        }
        raw_tail = tail(&format!("{}\n{}", out.stdout, out.stderr), 1500);
        // Python 3.14 unittest, like pytest, exits 5 for an empty selection.
        let python_empty = plan.runtime == Runtime::Python
            && out.exit == Some(5)
            && ((argv.iter().any(|s| s.contains("pytest.main"))
                && pytest_counts(&out.stdout).is_none_or(|counts| counts.ran() == 0))
                || python_counts(&out.stderr).is_some_and(|counts| counts.ran() == 0));
        if !out.success && !python_empty {
            return Err(format!(
                "{} {} failed (exit {})\n{}",
                plan.runtime.label(),
                plan.kind.tool(),
                out.exit
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "signal".into()),
                tail(&format!("{}\n{}", out.stdout, out.stderr), 1500)
            ));
        }
        if plan.kind == Kind::Tests {
            test_counts = match plan.runtime {
                Runtime::Node => node_counts(&out.stdout),
                Runtime::Python => {
                    if argv.iter().any(|s| s.contains("pytest.main")) {
                        pytest_counts(&out.stdout)
                    } else {
                        python_counts(&out.stderr)
                    }
                }
                Runtime::Go => go_counts(&out.stdout),
            };
        }
    }
    let summary = match plan.kind {
        Kind::Check => format!("check: 0 warnings, 0 errors — reward 1.00\nsyntax-only check: {} source file(s); this is not a type/dependency/lint check", plan.checked_files),
        Kind::Tests => match test_counts {
            Some(counts) if counts.ran() > 0 => format!(
                "tests: {} passed, {} failed, {} ignored — reward {:.2}",
                counts.passed, counts.failed, counts.ignored, counts.reward()),
            Some(_) if args["args"].as_str().is_some_and(|value| !value.trim().is_empty()) =>
                "verification inconclusive: no tests ran — reward 0.00".into(),
            // Preserve the existing fail-closed empty-suite verdict when no
            // selection was requested (including forged child pass text).
            Some(_) => "no tests ran — reward 0.00".into(),
            // Child text may contain fake pass summaries. Never attach it to
            // an inconclusive typed receipt: legacy classifiers scan text.
            None => "verification inconclusive: the pinned runner exited successfully without a recognized final nonempty test report".into(),
        },
        Kind::Lint => unreachable!("unsupported lint adapters are declined before dispatch"),
    };
    let attribution = serde_json::json!({
        "schema": "angel-native-verifier/v1", "runtime": plan.runtime.label(),
        "executable": executable.path, "executable_sha256": executable.sha256.get(),
        "argv": actual_argvs, "scope": if plan.kind == Kind::Check { "syntax" } else { "tests" },
    });
    Ok(format!(
        "{summary}\n[verifier attribution] {attribution}\n[raw tail]\n{raw_tail}"
    ))
}

fn node_counts(text: &str) -> Option<TestOutcome> {
    // Only the complete terminal TAP footer belongs to the fixed reporter.
    // Test stdout is earlier in this stream and cannot replace that footer.
    static FOOTER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let regex = FOOTER.get_or_init(|| regex::Regex::new(
        r"(?m)^1\.\.[0-9]+\n# tests ([0-9]+)\n# suites [0-9]+\n# pass ([0-9]+)\n# fail ([0-9]+)\n# cancelled ([0-9]+)\n# skipped ([0-9]+)\n# todo ([0-9]+)\n# duration_ms [0-9.]+\n?\z"
    ).expect("fixed Node TAP footer regex"));
    let captured = regex.captures(text)?;
    let number = |index| captured.get(index)?.as_str().parse::<usize>().ok();
    let total = number(1)?;
    let passed = number(2)?;
    let failed = number(3)?.checked_add(number(4)?)?;
    let ignored = number(5)?.checked_add(number(6)?)?;
    if passed.checked_add(failed)?.checked_add(ignored)? != total {
        return None;
    }
    Some(TestOutcome {
        passed,
        failed,
        ignored,
    })
}

fn python_counts(text: &str) -> Option<TestOutcome> {
    static FOOTER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let regex = FOOTER.get_or_init(|| {
        regex::Regex::new(
            r"(?m)^-{70}\nRan ([0-9]+) tests? in [0-9.]+s\n\n(?:OK(?: \(([^\n]*)\))?|NO TESTS RAN)\n?\z",
        )
        .expect("fixed unittest footer regex")
    });
    let captured = regex.captures(text)?;
    let total = captured.get(1)?.as_str().parse::<usize>().ok()?;
    if text.trim_end().ends_with("NO TESTS RAN") && total != 0 {
        return None;
    }
    let mut ignored = 0usize;
    if let Some(options) = captured.get(2) {
        for option in options.as_str().split(", ") {
            let (name, value) = option.split_once('=')?;
            if !matches!(name, "skipped" | "expected failures") {
                return None;
            }
            ignored = ignored.checked_add(value.parse::<usize>().ok()?)?;
        }
    }
    Some(TestOutcome {
        passed: total.checked_sub(ignored)?,
        failed: 0,
        ignored,
    })
}

#[cfg(test)]
pub(super) fn parsed_node(text: &str) -> Option<TestOutcome> {
    node_counts(text)
}
#[cfg(test)]
pub(super) fn parsed_python(text: &str) -> Option<TestOutcome> {
    python_counts(text)
}

pub(super) fn go_counts(text: &str) -> Option<TestOutcome> {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut ignored = 0usize;
    let mut packages = std::collections::HashMap::new();
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        let action = value["Action"].as_str()?;
        let package = value["Package"].as_str()?;
        if value["Test"].is_string() {
            match action {
                "pass" => passed += 1,
                "fail" => failed += 1,
                "skip" => ignored += 1,
                _ => {}
            }
        } else {
            packages.insert(package.to_string(), action.to_string());
        }
    }
    if packages.is_empty()
        || packages
            .values()
            .any(|a| !matches!(a.as_str(), "pass" | "skip"))
    {
        return None;
    }
    Some(TestOutcome {
        passed,
        failed,
        ignored,
    })
}

pub(super) fn pytest_counts(text: &str) -> Option<TestOutcome> {
    let line = text.lines().last()?.trim().trim_matches('=').trim();
    let (counts, duration) = line.rsplit_once(" in ")?;
    duration.strip_suffix('s')?.parse::<f64>().ok()?;
    let mut outcome = TestOutcome {
        passed: 0,
        failed: 0,
        ignored: 0,
    };
    for count in counts.split(", ") {
        let (value, kind) = count.split_once(' ')?;
        let value = value.parse::<usize>().ok()?;
        match kind {
            "passed" => outcome.passed = outcome.passed.checked_add(value)?,
            "failed" | "error" | "errors" => outcome.failed = outcome.failed.checked_add(value)?,
            "skipped" | "xfailed" | "deselected" => {
                outcome.ignored = outcome.ignored.checked_add(value)?
            }
            _ => return None,
        }
    }
    Some(outcome)
}
