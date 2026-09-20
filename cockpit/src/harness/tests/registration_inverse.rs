//! Revertible registration — the witness the runtime does not check.
//!
//! `harness::registration` gives every registration an inverse and a scope to
//! accumulate them; *that an inverse really reverts its own effect* is an
//! obligation on the provider rather than a property the runtime verifies
//! (arXiv 2608.25512 §5.1.1, §6.1). These tests are that witness, and they use
//! real resources — a scratch tree and a live child process — so a missing or
//! double-fired inverse shows up as a leaked directory or an orphan process
//! instead of a passing assertion about a counter.

use super::*;
use crate::harness::registration::{Disposable, RegistrationScope};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What a provider installs: a scratch tree and a live child process. The
/// inverse kills and reaps the child, then removes the tree — the same shape an
/// MCP server's teardown has.
struct ProbeResource {
    dir: PathBuf,
    child: Mutex<Child>,
    reverts: Arc<AtomicUsize>,
}

impl ProbeResource {
    fn spawn(tag: &str) -> Arc<Self> {
        let dir = scratch(&format!("registration-inverse-{tag}"));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        std::fs::write(dir.join("marker"), b"installed").expect("marker file");
        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("child process");
        Arc::new(Self {
            dir,
            child: Mutex::new(child),
            reverts: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn dir_exists(&self) -> bool {
        self.dir.exists()
    }

    /// A reaped child reports an exit status instead of `Ok(None)`; an orphaned
    /// one keeps running past the test that leaked it.
    fn child_reaped(&self) -> bool {
        let mut child = self.child.lock().unwrap_or_else(|e| e.into_inner());
        matches!(child.try_wait(), Ok(Some(_)))
    }

    fn reverts(&self) -> usize {
        self.reverts.load(Ordering::SeqCst)
    }

    fn inverse(self: &Arc<Self>) -> impl FnOnce() + Send + Sync + 'static {
        let this = Arc::clone(self);
        move || {
            {
                let mut child = this.child.lock().unwrap_or_else(|e| e.into_inner());
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = std::fs::remove_dir_all(&this.dir);
            this.reverts.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// A provider whose only job is to be registered and retracted.
struct ProbeTool {
    name: String,
}

impl Tool for ProbeTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name.clone(),
            description: "registration probe".to_string(),
            params: serde_json::json!({"type": "object", "properties": {}}),
        }
    }

    fn call(&self, _args: &Value) -> Result<String, String> {
        Ok("probe ok".to_string())
    }
}

fn probe(name: &str) -> Box<dyn Tool> {
    Box::new(ProbeTool {
        name: name.to_string(),
    })
}

fn advertised(registry: &ToolRegistry, name: &str) -> bool {
    registry.defs().iter().any(|def| def.name == name)
}

// --- the inverse itself -----------------------------------------------------

#[test]
fn an_inverse_fires_at_most_once() {
    let _lock = crate::tests::env_lock();
    let resource = ProbeResource::spawn("once");
    let disposable = Disposable::new(resource.inverse());
    assert!(disposable.is_armed());

    assert!(disposable.fire(), "the first fire runs the inverse");
    assert!(!disposable.fire(), "the second fire is a no-op");
    assert!(!disposable.is_armed());
    assert_eq!(resource.reverts(), 1, "the inverse ran exactly once");
    assert!(!resource.dir_exists());
    assert!(resource.child_reaped(), "the child is reaped, not orphaned");
}

#[test]
fn dropping_a_live_inverse_still_recovers() {
    let _lock = crate::tests::env_lock();
    let resource = ProbeResource::spawn("drop");
    {
        let _held = Disposable::new(resource.inverse());
        assert!(resource.dir_exists(), "installed before the drop");
    }
    assert_eq!(resource.reverts(), 1, "unwinding still reverts the effect");
    assert!(!resource.dir_exists());
    assert!(resource.child_reaped());
}

// --- scope semantics (LIFO, cascade, guard) ---------------------------------

#[test]
fn a_scope_fires_inverses_last_registered_first() {
    let scope = RegistrationScope::new();
    let order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
    for label in ["first", "second", "third"] {
        let slot = Arc::clone(&order);
        scope.track_inverse(move || {
            slot.lock().unwrap_or_else(|e| e.into_inner()).push(label);
        });
    }
    assert_eq!(scope.len(), 3);

    assert_eq!(scope.dispose(), 3);
    assert_eq!(
        *order.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["third", "second", "first"],
        "inverses accumulate LIFO"
    );
    assert_eq!(scope.dispose(), 0, "a second dispose fires nothing");
    assert!(scope.is_empty());
}

#[test]
fn a_nested_scope_disposes_with_its_parent() {
    let parent = RegistrationScope::new();
    let fired: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

    let child = RegistrationScope::new();
    for label in ["child-first", "child-second"] {
        let slot = Arc::clone(&fired);
        child.track_inverse(move || {
            slot.lock().unwrap_or_else(|e| e.into_inner()).push(label);
        });
    }
    let parent_slot = Arc::clone(&fired);
    parent.track_inverse(move || {
        parent_slot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push("parent");
    });
    parent.track(Arc::new(child.into_disposable()));

    assert_eq!(parent.dispose(), 2, "the parent owns the child's recovery");
    assert_eq!(
        *fired.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["child-second", "child-first", "parent"],
        "the nested scope reverts as one effect, in its own LIFO order"
    );
}

#[test]
fn a_partial_registration_keeps_only_the_inverses_it_accumulated() {
    // §5.1.1 Algorithm 1 consults a guard before each step: once it trips, only
    // the inverses accumulated so far remain. A provider that fails halfway
    // through installing itself must leave nothing behind.
    let scope = RegistrationScope::new();
    let fired = Arc::new(AtomicUsize::new(0));

    let first = Arc::clone(&fired);
    scope.track_inverse(move || {
        first.fetch_add(1, Ordering::SeqCst);
    });
    let guard_ok = false; // the second step never happens
    if guard_ok {
        let second = Arc::clone(&fired);
        scope.track_inverse(move || {
            second.fetch_add(1, Ordering::SeqCst);
        });
    }

    assert_eq!(scope.dispose(), 1, "only the accumulated inverse runs");
    assert_eq!(fired.load(Ordering::SeqCst), 1);
}

// --- registry integration ---------------------------------------------------

#[test]
fn unregister_retracts_schema_resources_and_activation() {
    let _lock = crate::tests::env_lock();
    let resource = ProbeResource::spawn("unregister");
    let mut registry = ToolRegistry::new();
    registry.register_tracked_deferred(probe("probe_provider"), resource.inverse());
    registry.enable_tool_search();

    assert!(
        !advertised(&registry, "probe_provider"),
        "deferred stays hidden"
    );
    assert!(
        registry
            .dispatch("probe_provider", &serde_json::json!({}))
            .is_ok(),
        "a deferred provider is still dispatchable"
    );
    registry.tool_activations().activate(["probe_provider"], 8);
    assert_eq!(registry.tracked_registrations(), 1);

    assert!(
        registry.unregister("probe_provider"),
        "the provider was registered"
    );
    assert!(
        registry
            .dispatch("probe_provider", &serde_json::json!({}))
            .is_err(),
        "a retracted provider is gone from dispatch"
    );
    assert!(!advertised(&registry, "probe_provider"));
    assert_eq!(registry.tracked_registrations(), 0);
    assert_eq!(
        registry.activated_schema_failures(&registry.defs()),
        0,
        "retraction clears the activation set with the schema"
    );
    assert_eq!(resource.reverts(), 1);
    assert!(!resource.dir_exists(), "no leaked scratch tree");
    assert!(resource.child_reaped(), "no orphaned child process");

    assert!(
        !registry.unregister("probe_provider"),
        "a second retraction is a no-op"
    );
}

#[test]
fn dispose_registrations_unloads_tracked_providers_and_keeps_builtins() {
    let _lock = crate::tests::env_lock();
    let resource = ProbeResource::spawn("dispose-all");
    let mut registry = ToolRegistry::new();
    registry.register(probe("builtin_probe"));
    registry.register_tracked(probe("tracked_probe"), resource.inverse());

    assert_eq!(
        registry.dispose_registrations(),
        1,
        "one tracked inverse ran"
    );
    assert!(
        advertised(&registry, "builtin_probe"),
        "untracked tools stay"
    );
    assert!(!advertised(&registry, "tracked_probe"));
    assert_eq!(registry.tracked_registrations(), 0);
    assert_eq!(resource.reverts(), 1);
    assert!(!resource.dir_exists());
    assert!(resource.child_reaped());
    assert_eq!(
        registry.dispose_registrations(),
        0,
        "nothing left to unload"
    );
}

#[test]
fn independent_registrations_recover_in_either_order() {
    // §3.4 Definition 42 with Theorem 43: effects that commute may be reverted
    // in any order. Two providers on distinct keys are the commuting case.
    let _lock = crate::tests::env_lock();
    for reverse in [false, true] {
        let alpha = ProbeResource::spawn(&format!("order-alpha-{reverse}"));
        let beta = ProbeResource::spawn(&format!("order-beta-{reverse}"));
        let mut registry = ToolRegistry::new();
        let first = registry.register_tracked(probe("probe_alpha"), alpha.inverse());
        let second = registry.register_tracked(probe("probe_beta"), beta.inverse());

        if reverse {
            second.fire();
            first.fire();
        } else {
            first.fire();
            second.fire();
        }

        for resource in [&alpha, &beta] {
            assert_eq!(resource.reverts(), 1, "each inverse ran exactly once");
            assert!(!resource.dir_exists(), "recovery holds in either order");
            assert!(resource.child_reaped());
        }
    }
}
