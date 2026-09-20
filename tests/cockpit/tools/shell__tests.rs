use super::*;
use crate::knowledge::experience::{
    CmdExperience, VERDICT_FAIL, VERDICT_NONE, VERDICT_PASS, cmd_verdict,
};
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
    let _editable = crate::tests::TestEnvGuard::set(TASK_EDITABLE_PATHS_ENV, "[\"submission\"]");
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
    let mut registry = crate::agent::harness::ToolRegistry::new();
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
    let parsed = crate::agent::tools::runtime_missing::RuntimeMissing::decode(&error).unwrap();
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
    if crate::platform::workspace_lang::resolve_on_path("python").is_none()
        && crate::platform::workspace_lang::resolve_on_path("python3").is_some()
    {
        if crate::platform::workspace_lang::runtime_shims_dir().is_some() {
            match tool.call(&serde_json::json!({
                "command": "python -c 'import sys; print(sys.version_info[0])'"
            })) {
                Ok(out) => assert!(out.contains('3'), "{out}"),
                Err(err) => eprintln!(
                    "SKIP a_command_not_found_names_the_missing_program_and_its_sibling python shim: node/runtime-shim capability absent ({err})"
                ),
            }
        }
        let hint = crate::platform::workspace_lang::missing_program_hint("python -m unittest -v")
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
    let call = crate::agent::club::ToolCall {
        id: "killed".into(),
        name: "shell".into(),
        args: serde_json::json!({"command":"kill -KILL $$"}),
    };
    let outcome =
        crate::agent::harness::turn_event_outcome(&call, &format!("tool error: {error}"), false);
    assert_eq!(
        outcome.execution,
        crate::agent::harness::ExecutionOutcome::Failed
    );
    assert_eq!(
        outcome.verification,
        crate::agent::harness::VerificationOutcome::Inconclusive
    );
    let kill = crate::agent::sandbox::process_owner::KillReceipt::from_error(&error).unwrap();
    assert_eq!(kill.signal, Some(9));
    assert_eq!(kill.reason, "signal_death");
    assert_eq!(kill.owner, "unknown_external");
    crate::agent::harness::note_tool_outcome(
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
    let ledger = crate::agent::harness::tool_ledger_snapshot();
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
        let call = crate::agent::club::ToolCall {
            id: signal.into(),
            name: "shell".into(),
            args,
        };
        let outcome = crate::agent::harness::turn_event_outcome(
            &call,
            &format!("tool error: {error}"),
            false,
        );
        assert_eq!(
            outcome.execution,
            crate::agent::harness::ExecutionOutcome::Failed
        );
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
    use crate::agent::harness::Tool;
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
        read_only.contains("read-only") || read_only.contains("cannot run a command that writes"),
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
