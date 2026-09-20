use super::*;

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "angel-cut-group-{}-{}",
            std::process::id(),
            super::super::next_seq(),
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn package(&self, dir: &str, complete: bool) {
        self.write(
            &format!("{dir}/Cargo.toml"),
            "[package]\nname='fixture'\nversion='0.1.0'\n",
        );
        if complete {
            self.write(&format!("{dir}/src/main.rs"), "fn main() {}\n");
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn completed(plan: VerifyPlan, exit: i32) -> Machine {
    Machine::Ran {
        cmd: plan.cmd,
        dir: plan.dir,
        source: plan.source,
        exit: Some(exit),
        timed_out: false,
        dur_ms: 7,
        err: if exit == 0 {
            String::new()
        } else {
            "fixture compile error".into()
        },
    }
}

fn controls() -> (crate::tests::TestEnvGuard, crate::tests::TestEnvGuard) {
    (
        crate::tests::TestEnvGuard::unset("ANGEL_CUT_VERIFY"),
        crate::tests::TestEnvGuard::set("ANGEL_DOSSIER", "0"),
    )
}

#[test]
fn same_crate_batch_shares_one_execution_but_next_hop_checks_again() {
    let _lock = crate::tests::env_lock();
    let _controls = controls();
    let fixture = Fixture::new();
    fixture.package("app", true);
    let mut group = PostWriteVerification::default();
    let mut verdicts = TurnVerdicts::default();
    let first = group
        .check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |plan| {
            completed(plan, 0)
        })
        .unwrap();
    let second = group
        .check_with(&fixture.0, "app/src/helper.rs", &mut verdicts, |_| {
            panic!("same completed batch must not execute twice")
        })
        .unwrap();
    assert_eq!(first.id, second.id);
    assert!(first.id.starts_with(run_namespace()));
    assert_eq!(run_namespace().len(), 32);
    assert_eq!(first.plan_id, second.plan_id);
    assert!(!first.shared);
    assert!(second.shared);
    assert_eq!(
        verdicts.seen.len(),
        1,
        "one training observation per execution"
    );
    assert_eq!(verdicts.reward(), Some(1.0));
    group.begin_batch();
    let third = group
        .check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |plan| {
            completed(plan, 101)
        })
        .unwrap();
    assert_ne!(first.id, third.id);
    assert_eq!(verdicts.seen.len(), 2);
    assert_eq!(verdicts.reward(), Some(0.0));
}

#[test]
fn repeated_identical_failure_defers_until_the_turn_boundary() {
    let _lock = crate::tests::env_lock();
    let _controls = controls();
    let fixture = Fixture::new();
    fixture.package("app", true);
    let mut group = PostWriteVerification::default();
    let mut verdicts = TurnVerdicts::default();
    let mut executions = 0;

    for _ in 0..REPEATED_FAILURE_LIMIT {
        group.begin_batch();
        let receipt = group
            .check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |plan| {
                executions += 1;
                completed(plan, 101)
            })
            .unwrap();
        assert!(matches!(
            receipt.machine,
            Machine::Ran {
                exit: Some(101),
                ..
            }
        ));
    }

    group.begin_batch();
    let deferred = group
        .check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |_| {
            panic!("the third identical failure must not launch another compiler")
        })
        .unwrap();
    assert!(matches!(
        deferred.machine,
        Machine::Skipped {
            reason: REPEATED_FAILURE_DEFERRED
        }
    ));
    assert_eq!(executions, REPEATED_FAILURE_LIMIT);
    assert_eq!(verdicts.reward(), None, "a deferred check is not a verdict");

    let final_receipts = group.finish_with(&mut verdicts, |plan| {
        executions += 1;
        completed(plan, 0)
    });
    assert_eq!(executions, REPEATED_FAILURE_LIMIT + 1);
    assert_eq!(final_receipts.len(), 1);
    assert_eq!(final_receipts[0].scope, "turn-boundary");
    assert!(final_receipts[0].machine.passed());
}

#[test]
fn one_patch_covers_distinct_crates_and_per_file_syntax_plans() {
    let _lock = crate::tests::env_lock();
    let _controls = controls();
    let fixture = Fixture::new();
    fixture.package("a", true);
    fixture.package("b", true);
    let patch = serde_json::json!({"diff": "*** Begin Patch\n*** Add File: a/src/main.rs\n+fn main() {}\n*** Add File: a/src/helper.rs\n+pub fn f() {}\n*** Add File: b/src/main.rs\n+fn main() { bad(); }\n*** Add File: x.js\n+let x = 1;\n*** Add File: y.js\n+let y = 2;\n*** Add File: z.py\n+value = 3\n*** End Patch\n"});
    let paths = super::super::mutation_targets("apply_patch", &patch);
    assert_eq!(paths.len(), 6);
    let mut group = PostWriteVerification::default();
    let mut verdicts = TurnVerdicts::default();
    let mut plans = Vec::new();
    let mut receipts = Vec::new();
    for path in &paths {
        let record = group
            .check_with(&fixture.0, path, &mut verdicts, |plan| {
                plans.push(plan.clone());
                let exit = if plan.dir == "b" { 101 } else { 0 };
                completed(plan, exit)
            })
            .unwrap();
        receipts.push((path.clone(), record.to_json()));
    }
    assert_eq!(plans.len(), 5, "two crates and three distinct file checks");
    assert_eq!(
        receipts.len(),
        6,
        "every file retains its own authored attachment"
    );
    assert_eq!(
        receipts[0].1["verification_id"],
        receipts[1].1["verification_id"]
    );
    assert_ne!(
        receipts[0].1["verification_id"],
        receipts[2].1["verification_id"]
    );
    assert_eq!(receipts[2].1["exit"], 101);
    assert_eq!(receipts[2].1["source"], "default");
    assert_eq!(verdicts.reward(), Some(0.0));
}

#[test]
fn manifest_first_hops_defer_until_complete_without_repeated_checks() {
    let _lock = crate::tests::env_lock();
    let _controls = controls();
    let fixture = Fixture::new();
    fixture.package("app", false);
    let mut group = PostWriteVerification::default();
    let mut verdicts = TurnVerdicts::default();
    for path in ["app/Cargo.toml", "app/src/support.rs"] {
        group.begin_batch();
        let record = group
            .check_with(&fixture.0, path, &mut verdicts, |_| {
                panic!("known incomplete scaffold must defer")
            })
            .unwrap();
        assert!(record.machine.inline_note().unwrap().contains("mandatory"));
        assert_eq!(record.to_json()["cmd"], "cargo check");
        assert_eq!(record.to_json()["dir"], "app");
        assert_eq!(record.to_json()["source"], "default");
        assert_eq!(verdicts.reward(), None);
    }
    assert_eq!(group.deferred.len(), 1);
    fixture.write("app/src/main.rs", "fn main() {}\n");
    group.begin_batch();
    group.check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |plan| {
        completed(plan, 0)
    });
    assert!(group.deferred.is_empty());
    assert_eq!(verdicts.reward(), Some(1.0));
    assert!(
        group
            .finish_with(&mut verdicts, |_| panic!("already checked"))
            .is_empty()
    );
}

#[test]
fn unfinished_scaffold_is_checked_at_answer_and_cancelled_group_stays_unverified() {
    let _lock = crate::tests::env_lock();
    let _controls = controls();
    let fixture = Fixture::new();
    fixture.package("app", false);
    for cancelled in [false, true] {
        let mut group = PostWriteVerification::default();
        let mut verdicts = TurnVerdicts::default();
        group.check_with(&fixture.0, "app/Cargo.toml", &mut verdicts, |_| {
            panic!("incomplete scaffold must defer")
        });
        let records = group.finish_with(&mut verdicts, |plan| {
            if cancelled {
                Machine::Skipped {
                    reason: "cancelled",
                }
            } else {
                completed(plan, 101)
            }
        });
        assert_eq!(records.len(), 1);
        assert_eq!(verdicts.reward(), if cancelled { None } else { Some(0.0) });
        assert_eq!(verdicts.pending_note().is_some(), cancelled);
    }
}

#[test]
fn mutation_after_pass_cannot_reuse_reward_when_check_is_unverified() {
    let _lock = crate::tests::env_lock();
    let _controls = controls();
    let fixture = Fixture::new();
    fixture.package("app", true);
    for reason in ["cancelled", "backoff-after-timeout", "spawn-failed"] {
        let mut group = PostWriteVerification::default();
        let mut verdicts = TurnVerdicts::default();
        group.check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |plan| {
            completed(plan, 0)
        });
        group.begin_batch();
        group.check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |_| {
            Machine::Skipped { reason }
        });
        assert_eq!(verdicts.reward(), None, "{reason}");
    }
    let mut group = PostWriteVerification::default();
    let mut verdicts = TurnVerdicts::default();
    group.check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |plan| {
        completed(plan, 0)
    });
    group.begin_batch();
    group.check_with(&fixture.0, "app/src/main.rs", &mut verdicts, |plan| {
        let mut machine = completed(plan, 0);
        if let Machine::Ran { timed_out, .. } = &mut machine {
            *timed_out = true;
        }
        machine
    });
    assert_eq!(
        verdicts.reward(),
        None,
        "timeout cannot retain a prior pass"
    );
}

#[test]
fn explicit_verifier_and_opt_out_override_scaffold_deferral() {
    let _lock = crate::tests::env_lock();
    let _controls = controls();
    let fixture = Fixture::new();
    fixture.package("app", false);
    let mut group = PostWriteVerification::default();
    let mut verdicts = TurnVerdicts::default();
    {
        let _verify = crate::tests::TestEnvGuard::set("ANGEL_CUT_VERIFY", "cargo check");
        let record = group
            .check_with(&fixture.0, "app/Cargo.toml", &mut verdicts, |plan| {
                completed(plan, 101)
            })
            .unwrap();
        assert_eq!(record.to_json()["source"], "env");
        assert_eq!(record.to_json()["dir"], ".");
        assert!(group.deferred.is_empty());
    }
    let _verify = crate::tests::TestEnvGuard::set("ANGEL_CUT_VERIFY", "0");
    assert!(
        group
            .check_with(&fixture.0, "app/Cargo.toml", &mut verdicts, |_| panic!(
                "explicit opt-out must remain off"
            ))
            .is_none()
    );
}

#[test]
fn scaffold_preflight_does_not_hide_malformed_or_explicit_target_errors() {
    let fixture = Fixture::new();
    let plan = VerifyPlan {
        cmd: "cargo check".into(),
        dir: ".".into(),
        source: "default",
    };
    for manifest in [
        "invalid = [",
        "[workspace]\nmembers=[]\n",
        "[package]\nname='x'\n[lib]\npath='missing.rs'\n",
        "[package]\nname='x'\n[[bin]]\nname='x'\npath='missing.rs'\n",
    ] {
        fixture.write("Cargo.toml", manifest);
        assert!(!incomplete_cargo_scaffold(&fixture.0, &plan), "{manifest}");
    }
    fixture.write("Cargo.toml", &" ".repeat(65_537));
    assert!(!incomplete_cargo_scaffold(&fixture.0, &plan));
}

#[cfg(unix)]
#[test]
fn scaffold_preflight_declines_manifest_parent_and_source_symlinks() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let outside = Fixture::new();
    outside.package("crate", false);
    let plan = VerifyPlan {
        cmd: "cargo check".into(),
        dir: ".".into(),
        source: "default",
    };
    symlink(
        outside.0.join("crate/Cargo.toml"),
        fixture.0.join("Cargo.toml"),
    )
    .unwrap();
    assert!(
        !incomplete_cargo_scaffold(&fixture.0, &plan),
        "manifest alias is not read"
    );
    std::fs::remove_file(fixture.0.join("Cargo.toml")).unwrap();
    fixture.package(".", false);
    symlink(outside.0.join("crate"), fixture.0.join("src")).unwrap();
    assert!(
        !incomplete_cargo_scaffold(&fixture.0, &plan),
        "source alias is not traversed"
    );
    symlink(outside.0.join("crate"), fixture.0.join("aliased-crate")).unwrap();
    let parent_alias = VerifyPlan {
        dir: "aliased-crate".into(),
        ..plan
    };
    assert!(
        !incomplete_cargo_scaffold(&fixture.0, &parent_alias),
        "manifest parent alias is not traversed"
    );
}
