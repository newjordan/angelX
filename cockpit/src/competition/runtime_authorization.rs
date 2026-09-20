use super::journal::canonical_action_key;
use super::runtime_identity::{canonical_sha256, id, sha};
use super::runtime_schema::*;
use super::schema::{ActionIntentV1, LaneIdV1};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const AUTHORIZATION_SCHEMA_V1: &str = "angel.competition-runtime-authorization/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ParityProjectionV1 {
    IdentityV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServiceAuthorizationV1 {
    pub(crate) schema: String,
    pub(crate) preimage: AuthorizationPreimageV1,
    pub(crate) authorization_id: String,
}

impl ReducerContractV1 {
    pub(crate) fn new(
        operation_tag: ServiceOperationTagV1,
        reducer_version_sha256: String,
        read_components: BTreeSet<StateComponentV1>,
        write_selectors: BTreeSet<LeafSelectorV1>,
        rebase_rule: RebaseRuleV1,
    ) -> Result<Self, String> {
        sha(&reducer_version_sha256)?;
        if read_components.is_empty() || write_selectors.is_empty() {
            return Err("reducer selectors cannot be empty".into());
        }
        let (legal_slot, legal_action, result_tag, _) = operation_tag.contract_tuple();
        let selector_contract_sha256 = canonical_sha256(
            "selector contract",
            &(
                RUNTIME_CONTRACT_V1,
                operation_tag,
                &read_components,
                &write_selectors,
                rebase_rule,
            ),
        )?;
        let mut value = Self {
            schema: REDUCER_CONTRACT_SCHEMA_V1.into(),
            operation_tag,
            reducer_version_sha256,
            legal_slot,
            legal_action,
            result_tag,
            read_components,
            write_selectors,
            rebase_rule,
            selector_contract_sha256,
            reducer_contract_id: String::new(),
        };
        value.reducer_contract_id = value.canonical_id()?;
        Ok(value)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        sha(&self.reducer_version_sha256)?;
        let (slot, action, result, _) = self.operation_tag.contract_tuple();
        let selector = canonical_sha256(
            "selector contract",
            &(
                RUNTIME_CONTRACT_V1,
                self.operation_tag,
                &self.read_components,
                &self.write_selectors,
                self.rebase_rule,
            ),
        )?;
        if self.schema != REDUCER_CONTRACT_SCHEMA_V1
            || self.read_components.is_empty()
            || self.write_selectors.is_empty()
            || self.legal_slot != slot
            || self.legal_action != action
            || self.result_tag != result
            || self.selector_contract_sha256 != selector
            || self.reducer_contract_id != self.canonical_id()?
        {
            return Err("persisted reducer contract identity is invalid".into());
        }
        Ok(())
    }

    fn canonical_id(&self) -> Result<String, String> {
        let mut value = self.clone();
        value.reducer_contract_id.clear();
        canonical_sha256("reducer contract", &(RUNTIME_CONTRACT_V1, value))
    }
}

pub(crate) fn reducer_registry_sha256(registry: &[ReducerContractV1]) -> Result<String, String> {
    if registry.is_empty() {
        return Err("reducer registry cannot be empty".into());
    }
    let mut ordered = registry.to_vec();
    for contract in &ordered {
        contract.validate()?;
    }
    ordered.sort_by_key(|contract| contract.operation_tag);
    if ordered
        .windows(2)
        .any(|pair| pair[0].operation_tag == pair[1].operation_tag)
    {
        return Err("reducer registry contains duplicate operation tags".into());
    }
    canonical_sha256("reducer registry", &(RUNTIME_CONTRACT_V1, ordered))
}

impl DispatchBindingV1 {
    fn validate_for(&self, tag: ServiceOperationTagV1) -> Result<(), String> {
        let (required_slot, required_action, _, _) = tag.contract_tuple();
        let (decision, tick, slot, action) = match self {
            Self::Lane {
                decision_sha256,
                tick,
                slot,
                action,
                lane,
                lane_sequence,
                assignment_sha256,
                lease_key_sha256,
                lease_id,
                generation,
                ..
            } => {
                if *lane_sequence == 0 || *generation == 0 {
                    return Err("invalid lane binding counter".into());
                }
                let expected_lane = match required_slot {
                    ServiceSlotV1::FrontierLane => LaneIdV1::FrontierGuard,
                    ServiceSlotV1::DeepCutLane => LaneIdV1::DeepCut,
                    _ => return Err("independent operation cannot use a lane binding".into()),
                };
                if *lane != expected_lane {
                    return Err("lane binding crosses its logical lane".into());
                }
                sha(assignment_sha256)?;
                sha(lease_key_sha256)?;
                id(lease_id, "invalid dispatch lease id")?;
                (decision_sha256, tick, slot, action)
            }
            Self::Independent {
                decision_sha256,
                tick,
                slot,
                action,
                service_sequence,
            } => {
                if *service_sequence == 0
                    || matches!(
                        required_slot,
                        ServiceSlotV1::FrontierLane | ServiceSlotV1::DeepCutLane
                    )
                {
                    return Err("invalid independent service binding".into());
                }
                (decision_sha256, tick, slot, action)
            }
        };
        sha(decision)?;
        if *tick == 0 || *slot != required_slot || *action != required_action {
            return Err("dispatch binding does not match the reducer contract".into());
        }
        Ok(())
    }
}

impl AuthorizationPreimageV1 {
    fn validate(
        &self,
        registry: &[ReducerContractV1],
        operation: &ServiceOperationV1,
    ) -> Result<(), String> {
        operation.validate()?;
        let operation_tag = operation.tag();
        let contract = registry
            .iter()
            .find(|entry| entry.operation_tag == operation_tag)
            .ok_or_else(|| "operation reducer is unavailable".to_string())?;
        contract.validate()?;
        let operation_sha256 = operation.canonical_sha256()?;
        if self.runtime_contract != RUNTIME_CONTRACT_V1
            || self.registry_sha256 != reducer_registry_sha256(registry)?
            || self.reducer_contract_id != contract.reducer_contract_id
            || self.reducer_version_sha256 != contract.reducer_version_sha256
            || self.operation_sha256 != operation_sha256
            || self.service_intent_id != operation.service_intent_id()
            || self.expected_result_tag != contract.result_tag
            || self.selector_contract_sha256 != contract.selector_contract_sha256
        {
            return Err("authorization preimage differs from compiled authority".into());
        }
        self.dispatch.validate_for(operation_tag)?;
        self.action_intent.validate().map_err(str::to_string)?;
        let (_, _, _, journal_kind) = operation_tag.contract_tuple();
        if canonical_action_key(&self.action_intent).map_err(|error| error.to_string())?
            != self.action_intent.action_key
            || self.action_intent.kind != journal_kind
            || self.action_intent.subject_id != self.service_intent_id
            || self.action_intent.payload_sha256 != operation_sha256
            || self.action_intent.intent_version != RUNTIME_ACTION_INTENT_V1
        {
            return Err("action intent is not bound to the exact service operation".into());
        }
        if let ServiceOperationV1::OfficialResult { join, .. } = operation
            && self.action_intent.competition != join.competition
        {
            return Err("official action competition does not match its join".into());
        }
        for digest in [
            &self.pre_runtime_sha256,
            &self.pre_director_sha256,
            &self.pre_scheduler_sha256,
            &self.typed_input_evidence_sha256,
        ] {
            sha(digest)?;
        }
        Ok(())
    }
}

impl ServiceAuthorizationV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        registry: &[ReducerContractV1],
        operation: &ServiceOperationV1,
        dispatch: DispatchBindingV1,
        action_intent: ActionIntentV1,
        pre_runtime_revision: u64,
        pre_runtime_sha256: String,
        pre_director_sha256: String,
        pre_scheduler_sha256: String,
        typed_input_evidence_sha256: String,
    ) -> Result<Self, String> {
        operation.validate()?;
        let registry_sha256 = reducer_registry_sha256(registry)?;
        let contract = registry
            .iter()
            .find(|entry| entry.operation_tag == operation.tag())
            .ok_or_else(|| "operation reducer is unavailable".to_string())?;
        let preimage = AuthorizationPreimageV1 {
            runtime_contract: RUNTIME_CONTRACT_V1.into(),
            registry_sha256,
            reducer_contract_id: contract.reducer_contract_id.clone(),
            reducer_version_sha256: contract.reducer_version_sha256.clone(),
            operation_sha256: operation.canonical_sha256()?,
            service_intent_id: operation.service_intent_id().into(),
            expected_result_tag: contract.result_tag,
            selector_contract_sha256: contract.selector_contract_sha256.clone(),
            dispatch,
            action_intent,
            pre_runtime_revision,
            pre_runtime_sha256,
            pre_director_sha256,
            pre_scheduler_sha256,
            typed_input_evidence_sha256,
        };
        preimage.validate(registry, operation)?;
        let authorization_id = canonical_sha256("runtime authorization", &preimage)?;
        Ok(Self {
            schema: AUTHORIZATION_SCHEMA_V1.into(),
            preimage,
            authorization_id,
        })
    }

    pub(crate) fn validate(
        &self,
        registry: &[ReducerContractV1],
        operation: &ServiceOperationV1,
    ) -> Result<(), String> {
        self.preimage.validate(registry, operation)?;
        if self.schema != AUTHORIZATION_SCHEMA_V1
            || self.authorization_id != canonical_sha256("runtime authorization", &self.preimage)?
        {
            return Err("runtime authorization identity mismatch".into());
        }
        Ok(())
    }
}
