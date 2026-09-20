//! Reactive coeffects — a component's *advertisement* derived from what is live.
//!
//! Transfer of arXiv 2608.25512 §3.2 (Def 19/21/22). A component declares a
//! specification `d ⊆ K` over a coeffect context `Σ = (K ⇀ V)`; every change to
//! `Σ` is classified **activating / deactivating / neutral** *for that spec*, and
//! that classification is what moves the component in or out of service. Nothing
//! needs to be told which components a change touched: the reaction is computed
//! from the declaration. §5.1.3 contributes the one case a value comparison
//! cannot see — **provider identity**: a replaced provider that provides an equal
//! value is still a change, so a binding carries a `uid` beside its value.
//!
//! Applied here to tool advertisement: `ToolRegistry` used to answer "which
//! schemas do I advertise?" from an imperative activation set (`activate(names,
//! max)`), so a provider that went away left its schemas behind until something
//! remembered to retract them. Now a tool declares what it needs, the registry
//! binds providers into the store, and the advertised set is *derived* at read
//! time from `intent ∩ satisfied`. Dropping a provider reclassifies every
//! dependent as deactivating and the schemas leave with it — no reload, no
//! restart, and no room for a stale schema to reach a provider request.
//!
//! §6.6 caveat: key identity is nominal, so keys are namespaced by construction
//! (`provider:mcp:web`) rather than trusting a bare name two components could
//! collide on.

#![allow(dead_code)]

use std::collections::BTreeMap;

/// A key in the coeffect context `Σ`.
///
/// `pub` because a tool declares its spec in the [`crate::harness::registry::Tool`]
/// trait; the crate is a binary, so this widens nothing outside the process.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Key(String);

impl Key {
    /// A provider's liveness — bound while it is registered, released when it
    /// leaves. `provider:mcp:web`.
    pub fn provider(id: &str) -> Self {
        Self(format!("provider:{id}"))
    }

    /// A capability the workspace offers (a binary on PATH, a service reachable).
    pub fn capability(id: &str) -> Self {
        Self(format!("capability:{id}"))
    }

    /// A resource binding — a workspace root, a session, a checkout.
    pub fn resource(id: &str) -> Self {
        Self(format!("resource:{id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a binding holds. Only presence and readiness matter for activation; the
/// payload is what a consumer reads *once* it is activated.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Value {
    Flag(bool),
    Count(usize),
    Text(String),
}

impl Value {
    /// A binding that is present but not usable does not satisfy a spec:
    /// `Flag(false)` and `Count(0)` mean "declared, not ready", and the gap
    /// between the two is exactly what makes a live retraction expressible.
    fn ready(&self) -> bool {
        !matches!(self, Value::Flag(false) | Value::Count(0))
    }
}

/// One binding: the provider's **identity** (§5.1.3) beside its value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Binding {
    uid: u64,
    value: Value,
}

impl Binding {
    pub fn uid(&self) -> u64 {
        self.uid
    }

    fn ready(&self) -> bool {
        self.value.ready()
    }
}

/// `d ⊆ K` — the keys a component declares it needs (Def 21).
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Requirement {
    keys: Vec<Key>,
}

impl Requirement {
    pub fn any(keys: impl IntoIterator<Item = Key>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }

    /// The empty spec is satisfied by every context — the identity coeffect, the
    /// shape an ordinary always-available tool declares.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn contains(&self, key: &Key) -> bool {
        self.keys.iter().any(|candidate| candidate == key)
    }
}

/// Def 22: the three-way classification of a change against one spec.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Classification {
    Activating,
    Deactivating,
    Neutral,
}

/// What one change to `Σ` does to one declared spec.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reaction {
    pub classification: Classification,
    /// §5.1.3: the binding was replaced by one with a different identity. The
    /// value may be equal; the provider is not, so the dependent must re-derive
    /// rather than keep serving from what it resolved before.
    pub identity_changed: bool,
}

impl Reaction {
    /// Whether anything must be re-derived. A neutral reaction with stable
    /// identity is a *retune*: the paper's Def 27 property that a consumer's
    /// metadata can change under a live binding with no reload.
    pub fn reloads(self) -> bool {
        self.classification != Classification::Neutral || self.identity_changed
    }

    pub fn is_neutral(self) -> bool {
        !self.reloads()
    }
}

/// One committed change to `Σ`, carrying enough context to classify it.
#[derive(Clone, Debug)]
pub struct Change {
    key: Key,
    before: Option<Binding>,
    after: Option<Binding>,
}

impl Change {
    /// Classify this change against one declared spec (Def 22 + §5.1.3).
    ///
    /// A key the spec does not mention is **neutral by independence** (§3.4:
    /// distinct keys are independent outright) — that is what keeps an unrelated
    /// provider's arrival from reloading anything.
    pub fn react(&self, requirement: &Requirement) -> Reaction {
        let neutral = Reaction {
            classification: Classification::Neutral,
            identity_changed: false,
        };
        if !requirement.contains(&self.key) {
            return neutral;
        }
        let was = self.before.as_ref().is_some_and(Binding::ready);
        let now = self.after.as_ref().is_some_and(Binding::ready);
        match (was, now) {
            (false, true) => Reaction {
                classification: Classification::Activating,
                identity_changed: false,
            },
            (true, false) => Reaction {
                classification: Classification::Deactivating,
                identity_changed: false,
            },
            (true, true) => Reaction {
                classification: Classification::Neutral,
                identity_changed: self.before.as_ref().map(Binding::uid)
                    != self.after.as_ref().map(Binding::uid),
            },
            (false, false) => neutral,
        }
    }

    /// Whether the change concerns a key this spec names **at all** — boundness as well
    /// as readiness.
    ///
    /// A surface that tells *unknown* (never mentioned: keeps working) from *declined*
    /// (declared absent: suspends) must re-derive on the unbound → declined step, which
    /// the readiness comparison in `react` reports as neutral. Consumers whose only
    /// question is "is this spec satisfied" keep using `react`, where that neutrality is
    /// exactly right.
    pub fn touches(&self, requirement: &Requirement) -> bool {
        requirement.contains(&self.key)
    }
}

/// `Σ = (K ⇀ V)` with a version that moves on every change.
///
/// The version is the cache key that lets a *derived* advertisement be memoized
/// without going stale — the imperative cache this replaces was invalidated by
/// hand at each call site that mutated the tool set.
#[derive(Clone, Default, Debug)]
pub struct CoeffectStore {
    bindings: BTreeMap<Key, Binding>,
    next_uid: u64,
    version: u64,
}

impl CoeffectStore {
    /// Bind a key to a value with a **fresh identity**. Rebinding an occupied key
    /// is the §5.1.3 replacement: the dependent is re-derived even when the new
    /// value equals the old one.
    pub fn bind(&mut self, key: Key, value: Value) -> Change {
        self.next_uid += 1;
        let binding = Binding {
            uid: self.next_uid,
            value,
        };
        self.commit(key, Some(binding))
    }

    /// Change a live binding's value **keeping its identity** — a retune under a
    /// still-live provider, which must not invalidate anything downstream.
    pub fn retune(&mut self, key: Key, value: Value) -> Change {
        let uid = match self.bindings.get(&key) {
            Some(existing) => existing.uid,
            None => {
                self.next_uid += 1;
                self.next_uid
            }
        };
        self.commit(key, Some(Binding { uid, value }))
    }

    /// Release a key: the provider is gone. A no-op change when the key was not
    /// bound, so a double release is harmless.
    pub fn release(&mut self, key: &Key) -> Change {
        self.commit(key.clone(), None)
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// `σ ⊧ d` — every declared key is bound and ready. The empty spec holds.
    pub fn satisfied(&self, requirement: &Requirement) -> bool {
        requirement
            .keys()
            .iter()
            .all(|key| self.bindings.get(key).is_some_and(Binding::ready))
    }

    /// The key is bound but **not ready** — a source that was declared absent rather
    /// than never mentioned. The distinction is load-bearing for a live host: an
    /// unbound key is *unknown* (a surface keeps working as it did), while a
    /// declined one is a positive statement that the feed is gone.
    pub(crate) fn declined(&self, key: &Key) -> bool {
        self.bindings
            .get(key)
            .is_some_and(|binding| !binding.ready())
    }

    /// Every key of the spec that is declined, in declared order (owned, so a
    /// derived caller can outlive the spec value it was computed from).
    pub(crate) fn declined_keys(&self, requirement: &Requirement) -> Vec<Key> {
        requirement
            .keys()
            .iter()
            .filter(|key| self.declined(key))
            .cloned()
            .collect()
    }

    fn commit(&mut self, key: Key, after: Option<Binding>) -> Change {
        let before = self.bindings.get(&key).cloned();
        // The version moves only when a *derivation* could change. A retune that
        // keeps presence, readiness, and provider identity is invisible to every
        // declared spec, so it neither moves the version nor invalidates a memo
        // built on it — the "no reload" property, stated where it is enforced.
        if satisfaction_key(before.as_ref()) != satisfaction_key(after.as_ref()) {
            self.version += 1;
        }
        match &after {
            Some(binding) => {
                self.bindings.insert(key.clone(), binding.clone());
            }
            None => {
                self.bindings.remove(&key);
            }
        }
        Change { key, before, after }
    }
}

/// The part of a binding a derivation can depend on: present, ready, and — per
/// §5.1.3 — *which* provider it is. Equal keys mean no declared spec can tell the
/// two contexts apart, so nothing derived from the store needs re-deriving.
fn satisfaction_key(binding: Option<&Binding>) -> (bool, bool, u64) {
    match binding {
        Some(binding) => (true, binding.ready(), binding.uid),
        None => (false, false, 0),
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/harness/coeffect__tests.rs"]
mod tests;
