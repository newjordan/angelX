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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Key(String);

impl Key {
    pub fn provider(id: &str) -> Self {
        Self(format!("provider:{id}"))
    }

    pub fn capability(id: &str) -> Self {
        Self(format!("capability:{id}"))
    }

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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Value {
    Flag(bool),
    Count(usize),
    Text(String),
}

impl Value {
    fn ready(&self) -> bool {
        !matches!(self, Value::Flag(false) | Value::Count(0))
    }
}

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Classification {
    Activating,
    Deactivating,
    Neutral,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reaction {
    pub classification: Classification,
    pub identity_changed: bool,
}

impl Reaction {
    pub fn reloads(self) -> bool {
        self.classification != Classification::Neutral || self.identity_changed
    }

    pub fn is_neutral(self) -> bool {
        !self.reloads()
    }
}

#[derive(Clone, Debug)]
pub struct Change {
    key: Key,
    before: Option<Binding>,
    after: Option<Binding>,
}

impl Change {
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

    pub fn touches(&self, requirement: &Requirement) -> bool {
        requirement.contains(&self.key)
    }
}

#[derive(Clone, Default, Debug)]
pub struct CoeffectStore {
    bindings: BTreeMap<Key, Binding>,
    next_uid: u64,
    version: u64,
}

impl CoeffectStore {
    pub fn bind(&mut self, key: Key, value: Value) -> Change {
        self.next_uid += 1;
        let binding = Binding {
            uid: self.next_uid,
            value,
        };
        self.commit(key, Some(binding))
    }

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

    pub fn release(&mut self, key: &Key) -> Change {
        self.commit(key.clone(), None)
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn satisfied(&self, requirement: &Requirement) -> bool {
        requirement
            .keys()
            .iter()
            .all(|key| self.bindings.get(key).is_some_and(Binding::ready))
    }

    pub(crate) fn declined(&self, key: &Key) -> bool {
        self.bindings
            .get(key)
            .is_some_and(|binding| !binding.ready())
    }

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

fn satisfaction_key(binding: Option<&Binding>) -> (bool, bool, u64) {
    match binding {
        Some(binding) => (true, binding.ready(), binding.uid),
        None => (false, false, 0),
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/harness/coeffect__tests.rs"]
mod tests;
