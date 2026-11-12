//! Process-local withdrawal of pure live policy overrides. Captured local
//! dependencies belong to the immutable Program; already-pinned Programs keep
//! them. These controls never remove the host's shipped/native fallback.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyControlAction {
    Withdraw,
    Enable,
}

/// Immediate publication receipt, distinct from a Kernel drain operation.
/// Generations include code-free disabled tombstones and never reset on enable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyControl {
    pub contract: String,
    pub action: PolicyControlAction,
    pub generations: BTreeMap<String, u64>,
}

impl Registry {
    pub(super) fn is_policy_control(&self, id: &str) -> bool {
        self.disabled_policies.contains(id)
            || self
                .modules
                .get(id)
                .and_then(|module| module.active.as_ref())
                .is_some_and(|bundle| bundle.manifest.kind == "policy")
    }

    fn pure_policy(bundle: &Bundle) -> Result<(), String> {
        if bundle.manifest.kind != "policy" {
            return Err("cannot mix policy overrides and Kernel system controls".into());
        }
        // API1 policy evaluation has no Kernel resource-import or export
        // protocol. Do not silently claim those declarations participate in a
        // dependency-aware withdrawal. Local require dependencies are captured.
        if !bundle.manifest.imports.is_empty() || !bundle.manifest.exports.is_empty() {
            return Err("policy control/1 supports captured local dependencies, not Kernel imports or exports".into());
        }
        Ok(())
    }

    pub(super) fn withdraw_policies(
        &mut self,
        expected: Vec<(String, u64)>,
    ) -> Result<Value, String> {
        if self.kernel.withdrawal_pending() || expected.is_empty() || expected.len() > 64 {
            return Err("policy control busy or invalid transaction size".into());
        }
        let mut generations = BTreeMap::new();
        for (id, generation) in expected {
            if generations.contains_key(&id) {
                return Err("policy withdrawal is duplicated".into());
            }
            let module = self.modules.get(&id).ok_or("policy is not active")?;
            let bundle = module.active.as_ref().ok_or("policy is not active")?;
            Self::pure_policy(bundle)?;
            if module.generation != generation || self.disabled_modules.contains(&id) {
                return Err("policy generation changed or is disabled".into());
            }
            let next = generation
                .checked_add(1)
                .ok_or("policy generation exhausted")?;
            generations.insert(id, next);
        }
        // Every check precedes publication. Tombstones retain only bounded
        // identity/generation metadata, not active, previous or captured code.
        for (id, generation) in &generations {
            self.modules.insert(
                id.clone(),
                Module {
                    generation: *generation,
                    ..Module::default()
                },
            );
            self.disabled_modules.insert(id.clone());
            self.disabled_policies.insert(id.clone());
        }
        self.publish_policy_control(PolicyControlAction::Withdraw, generations)
    }

    pub(super) fn enable_policies(&mut self, ids: BTreeSet<String>) -> Result<Value, String> {
        if ids.iter().any(|id| !self.disabled_policies.contains(id)) {
            return Err("cannot mix policy overrides and Kernel system controls".into());
        }
        // The common enable preflight checks distinct IDs, suppression and the
        // retained module bound. Every policy has its original tombstone.
        let mut prepared = Vec::new();
        let mut generations = BTreeMap::new();
        for id in ids {
            let module = self.modules.get(&id).ok_or("policy tombstone is missing")?;
            let generation = module
                .generation
                .checked_add(1)
                .ok_or("policy generation exhausted")?;
            let bundle = Arc::new(Bundle::read(&self.root.join(&id), &id)?);
            Self::pure_policy(&bundle)?;
            if let Some(error) = &bundle.build_error {
                return Err(error.clone());
            }
            let value = evaluate(&bundle, &bundle.manifest.probe, self.evaluator)
                .and_then(|value| validate("policy", value))?;
            (self.validator)("policy", &value)?;
            generations.insert(id.clone(), generation);
            prepared.push((id, bundle, value, generation));
        }
        // No capture, probe or host validation runs after the first mutation.
        for (id, bundle, value, generation) in prepared {
            self.modules.insert(
                id.clone(),
                Module {
                    captured: Some(Arc::clone(&bundle)),
                    active: Some(bundle),
                    cached: Some(value),
                    generation,
                    ..Module::default()
                },
            );
            self.disabled_modules.remove(&id);
            self.disabled_policies.remove(&id);
        }
        self.publish_policy_control(PolicyControlAction::Enable, generations)
    }

    fn publish_policy_control(
        &mut self,
        action: PolicyControlAction,
        generations: BTreeMap<String, u64>,
    ) -> Result<Value, String> {
        let status = PolicyControl {
            contract: "policy-control/1".into(),
            action,
            generations,
        };
        // Serialization is infallible for this fixed string/enum/integer shape.
        let response = serde_json::to_value(&status).expect("policy control receipt serialization");
        self.policy_control = Some(status);
        self.revision += 1;
        Ok(response)
    }
}
