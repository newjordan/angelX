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
#[path = "../../../tests/cockpit/app/cut__verification__tests.rs"]
mod tests;
