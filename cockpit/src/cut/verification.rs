//! One verifier execution per concrete plan in a fully dispatched tool batch.
//! Only incomplete default Cargo scaffolds survive between batches; completed
//! verdicts never form a cross-hop success cache.

use super::{Machine, TurnVerdicts, Verify, VerifyPlan};
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::Path;
use std::sync::atomic::AtomicBool;

#[derive(Clone)]
pub(crate) struct VerificationReceipt {
    pub(crate) machine: Machine,
    id: String,
    plan_id: String,
    skipped_plan: Option<VerifyPlan>,
    scope: &'static str,
    pub(crate) shared: bool,
}

impl VerificationReceipt {
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let mut value = self.machine.to_json();
        value["verification_id"] = self.id.clone().into();
        value["verification_plan_sha256"] = self.plan_id.clone().into();
        value["shared"] = self.shared.into();
        value["verification_scope"] = self.scope.into();
        if let Some(plan) = &self.skipped_plan {
            // A pending/cancelled row still identifies the concrete obligation;
            // a digest alone cannot explain an abandoned scaffold later.
            value["cmd"] = plan.cmd.clone().into();
            value["dir"] = plan.dir.clone().into();
            value["source"] = plan.source.into();
        }
        value
    }
}

#[derive(Default)]
pub(crate) struct PostWriteVerification {
    batch: HashMap<VerifyPlan, VerificationReceipt>,
    deferred: Vec<VerifyPlan>,
    repeated_failures: HashMap<VerifyPlan, RepeatedFailure>,
}

#[derive(Clone)]
struct RepeatedFailure {
    fingerprint: String,
    count: usize,
}

const REPEATED_FAILURE_LIMIT: usize = 2;
const REPEATED_FAILURE_DEFERRED: &str =
    "repeated-identical-failure; mandatory turn-boundary check pending";

impl PostWriteVerification {
    /// Called after dispatch, before processing this batch's mutation results.
    pub(crate) fn begin_batch(&mut self) {
        self.batch.clear();
    }

    pub(crate) fn check(
        &mut self,
        workspace: &Path,
        target: &str,
        cancel: &AtomicBool,
        verdicts: &mut TurnVerdicts,
    ) -> Option<VerificationReceipt> {
        // Keep the existing unit-test no-spawn contract. The production binary
        // fixture is mandatory acceptance; planner tests use check_with below.
        if cfg!(test) {
            return None;
        }
        self.check_with(workspace, target, verdicts, |plan| {
            super::run_verify(workspace, plan, Some(cancel))
        })
    }

    fn check_with(
        &mut self,
        workspace: &Path,
        target: &str,
        verdicts: &mut TurnVerdicts,
        run: impl FnOnce(VerifyPlan) -> Machine,
    ) -> Option<VerificationReceipt> {
        let plan = match super::resolve_verify(workspace, &[target.to_string()]) {
            Verify::Off => return None,
            Verify::Skip(reason) => {
                return Some(receipt(None, Machine::Skipped { reason }));
            }
            Verify::Run(plan) => plan,
        };
        if let Some(cached) = self.batch.get(&plan) {
            let mut shared = cached.clone();
            shared.shared = true;
            return Some(shared);
        }
        verdicts.invalidate(&plan);
        let machine = if self
            .repeated_failures
            .get(&plan)
            .is_some_and(|failure| failure.count >= REPEATED_FAILURE_LIMIT)
        {
            if !self.deferred.contains(&plan) {
                self.deferred.push(plan.clone());
            }
            Machine::Skipped {
                reason: REPEATED_FAILURE_DEFERRED,
            }
        } else if incomplete_cargo_scaffold(workspace, &plan)
            && (self.deferred.contains(&plan) || self.deferred.len() < super::MAX_PENDING)
        {
            if !self.deferred.contains(&plan) {
                self.deferred.push(plan.clone());
            }
            Machine::Skipped {
                reason: "incomplete-cargo-scaffold; mandatory check pending",
            }
        } else {
            self.deferred.retain(|pending| pending != &plan);
            run(plan.clone())
        };
        if let Some(fingerprint) = repeatable_failure_fingerprint(&machine) {
            self.repeated_failures
                .entry(plan.clone())
                .and_modify(|failure| {
                    if failure.fingerprint == fingerprint {
                        failure.count = failure.count.saturating_add(1);
                    } else {
                        failure.fingerprint.clone_from(&fingerprint);
                        failure.count = 1;
                    }
                })
                .or_insert(RepeatedFailure {
                    fingerprint,
                    count: 1,
                });
        } else if !matches!(&machine, Machine::Skipped { reason } if *reason == REPEATED_FAILURE_DEFERRED)
        {
            self.repeated_failures.remove(&plan);
        }
        verdicts.observe(&machine);
        let receipt = receipt(Some(&plan), machine);
        // Bound retained diagnostics even for a very large multi-file patch.
        // Overflow may repeat a check, but must never drop verification scope.
        if self.batch.len() >= super::MAX_PENDING {
            self.batch.clear();
        }
        self.batch.insert(plan, receipt.clone());
        Some(receipt)
    }

    /// A normal answer closes any scaffold group, even if no entrypoint was
    /// ever written. Cancellation/deadline reaches run_verify's existing skip
    /// contract and therefore cannot earn a reward from an earlier pass.
    pub(crate) fn finish(
        &mut self,
        workspace: &Path,
        cancel: &AtomicBool,
        verdicts: &mut TurnVerdicts,
    ) -> Vec<VerificationReceipt> {
        if cfg!(test) {
            return Vec::new();
        }
        self.finish_with(verdicts, |plan| {
            super::run_verify(workspace, plan, Some(cancel))
        })
    }

    fn finish_with(
        &mut self,
        verdicts: &mut TurnVerdicts,
        mut run: impl FnMut(VerifyPlan) -> Machine,
    ) -> Vec<VerificationReceipt> {
        std::mem::take(&mut self.deferred)
            .into_iter()
            .map(|plan| {
                let machine = run(plan.clone());
                self.repeated_failures.remove(&plan);
                verdicts.observe(&machine);
                let mut receipt = receipt(Some(&plan), machine);
                receipt.scope = "turn-boundary";
                receipt
            })
            .collect()
    }
}

fn repeatable_failure_fingerprint(machine: &Machine) -> Option<String> {
    let Machine::Ran {
        exit,
        timed_out: false,
        err,
        ..
    } = machine
    else {
        return None;
    };
    if exit.is_some_and(|code| code == 0) {
        return None;
    }
    serde_json::to_vec(&(exit, err))
        .ok()
        .map(|bytes| super::sha256_hex(&bytes))
}

fn receipt(plan: Option<&VerifyPlan>, machine: Machine) -> VerificationReceipt {
    let skipped_plan = if matches!(&machine, Machine::Skipped { .. }) {
        plan.cloned()
    } else {
        None
    };
    let plan_id = plan
        .map(|plan| {
            super::sha256_hex(
                &serde_json::to_vec(&(plan.cmd.as_str(), plan.dir.as_str(), plan.source))
                    .expect("verifier identity serializes"),
            )
        })
        .unwrap_or_default();
    VerificationReceipt {
        machine,
        id: format!(
            "{}:{}:{}",
            run_namespace(),
            std::process::id(),
            super::next_seq()
        ),
        plan_id,
        skipped_plan,
        scope: "completed-tool-batch",
        shared: false,
    }
}

fn run_namespace() -> &'static str {
    static NAMESPACE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAMESPACE.get_or_init(|| {
        // RandomState has independently seeded per-instance hash keys. Include
        // wall time as well; retained receipts must not collide after PID reuse.
        // This is an accounting nonce, not an authentication token.
        let random = std::collections::hash_map::RandomState::new();
        let seed = (std::process::id(), std::time::SystemTime::now());
        format!(
            "{:016x}{:016x}",
            random.hash_one((0u8, seed)),
            random.hash_one((1u8, seed))
        )
    })
}

/// Conservative preflight, not a replacement for Cargo's parser/verification.
/// Only a small, valid package manifest without any declared or conventional
/// target can defer the default `cargo check`. Virtual workspaces, explicit
/// targets, malformed/large manifests and unknown layouts run normally.
fn incomplete_cargo_scaffold(workspace: &Path, plan: &VerifyPlan) -> bool {
    if plan.source != "default" || plan.cmd != "cargo check" {
        return false;
    }
    let path = Path::new(&plan.dir).join("Cargo.toml");
    let Ok(Some(bytes)) =
        crate::harness::confined_read_limited_no_symlinks(workspace, &path, 65_536)
    else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return false;
    };
    let Ok(manifest) = text.parse::<toml::Value>() else {
        return false;
    };
    if manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .is_none()
        || ["lib", "bin", "example", "test", "bench", "workspace"]
            .iter()
            .any(|key| manifest.get(key).is_some())
    {
        return false;
    }
    implicit_targets_provably_absent(workspace, Path::new(&plan.dir))
}

/// Constant-count, descriptor-relative metadata checks. Any symlink, unusual
/// target layout, or ambiguous error declines deferral. In particular, never
/// list `src/bin` through a pathname that can be swapped outside the workspace.
#[cfg(unix)]
fn implicit_targets_provably_absent(workspace: &Path, crate_dir: &Path) -> bool {
    use std::ffi::{CStr, CString};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Component;

    fn directory(parent: &std::fs::File, name: &CStr) -> std::io::Result<Option<std::fs::File>> {
        // SAFETY: parent owns a live directory descriptor, name is NUL-terminated,
        // and O_NOFOLLOW forbids symlinks for every individually opened component.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY,
            )
        };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(libc::ENOENT) {
                Ok(None)
            } else {
                Err(error)
            };
        }
        // SAFETY: openat returned a new owned descriptor.
        Ok(Some(unsafe { std::fs::File::from_raw_fd(fd) }))
    }

    let Ok(mut root) = std::fs::File::open(workspace) else {
        return false;
    };
    for component in crate_dir.components() {
        let Component::Normal(part) = component else {
            if component == Component::CurDir {
                continue;
            }
            return false;
        };
        let Ok(name) = CString::new(part.as_bytes()) else {
            return false;
        };
        let Ok(Some(next)) = directory(&root, &name) else {
            return false;
        };
        root = next;
    }
    let source = match directory(&root, c"src") {
        Ok(Some(source)) => source,
        Ok(None) => return true,
        Err(_) => return false,
    };
    for name in [c"main.rs", c"lib.rs", c"bin"] {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: source owns its descriptor, the name is a valid C string, and
        // stat points to writable storage. No returned fields are read. Leaf
        // symlinks count as present rather than being followed or read.
        let found = unsafe {
            libc::fstatat(
                source.as_raw_fd(),
                name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if found == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
            return false;
        }
    }
    true
}

#[cfg(not(unix))]
fn implicit_targets_provably_absent(_workspace: &Path, _crate_dir: &Path) -> bool {
    false
}

/// Deferred checks have no new authored row. Record their own execution once,
/// linked by plan identity to the earlier explicitly unverified authored rows.
pub(crate) fn record_final_verification(
    workspace: &Path,
    hop: usize,
    route: &crate::club::RouteIdentity,
    receipt: &VerificationReceipt,
) {
    if cfg!(test) || !super::CutCfg::from_env().enabled {
        return;
    }
    super::append_row(&serde_json::json!({
        "v": super::SCHEMA_V,
        "kind": "verification",
        "ts": super::now_secs(),
        "session": std::process::id(),
        "seq": super::next_seq(),
        "hop": hop,
        "repo": crate::experience::repo_value_for(workspace),
        "route_driver": route.driver,
        "model": route.model,
        "machine": receipt.to_json(),
    }));
}

#[cfg(test)]
mod tests {
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
}
