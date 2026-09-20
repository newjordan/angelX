use super::runtime_identity::{id, sha};
use super::runtime_schema::{
    OfficialJoinV1, OfficialParentsV1, ServiceOperationTagV1, ServiceOperationV1,
    ServiceResultTagV1,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BoardOutcomeV1 {
    Observed { outcome_sha256: String },
    Failed { failure_sha256: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DeepCutEvidenceV1 {
    RegisterWorker { worker_sha256: String },
    RegisterEpisode { episode_sha256: String },
    Terminal { terminal_sha256: String },
    Rollover { rollover_sha256: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SubmissionEvidenceV1 {
    Plan { plan_sha256: String },
    Effect { effect_sha256: String },
    Reconcile { reconcile_sha256: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ServiceResultV1 {
    Board {
        service_intent_id: String,
        operation_tag: ServiceOperationTagV1,
        raw_sha256: String,
        outcome: BoardOutcomeV1,
    },
    Candidate {
        service_intent_id: String,
        operation_tag: ServiceOperationTagV1,
        batch_sha256: String,
    },
    DeepCut {
        service_intent_id: String,
        evidence: DeepCutEvidenceV1,
    },
    Context {
        service_intent_id: String,
        events_sha256: String,
        anchors_sha256: String,
    },
    Submission {
        service_intent_id: String,
        evidence: SubmissionEvidenceV1,
    },
    Official {
        service_intent_id: String,
        result_sha256: String,
        reward_input_sha256: String,
    },
    Worker {
        service_intent_id: String,
        operation_tag: ServiceOperationTagV1,
        control_sha256: String,
        lease_receipt_sha256: String,
    },
}

impl DeepCutEvidenceV1 {
    fn operation_tag(&self) -> ServiceOperationTagV1 {
        match self {
            Self::RegisterWorker { .. } => ServiceOperationTagV1::DeepCutRegisterWorker,
            Self::RegisterEpisode { .. } => ServiceOperationTagV1::DeepCutRegisterEpisode,
            Self::Terminal { .. } => ServiceOperationTagV1::DeepCutTerminal,
            Self::Rollover { .. } => ServiceOperationTagV1::DeepCutRollover,
        }
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::RegisterWorker { worker_sha256 } => sha(worker_sha256),
            Self::RegisterEpisode { episode_sha256 } => sha(episode_sha256),
            Self::Terminal { terminal_sha256 } => sha(terminal_sha256),
            Self::Rollover { rollover_sha256 } => sha(rollover_sha256),
        }
    }
}

impl SubmissionEvidenceV1 {
    fn operation_tag(&self) -> ServiceOperationTagV1 {
        match self {
            Self::Plan { .. } => ServiceOperationTagV1::SubmissionPlan,
            Self::Effect { .. } => ServiceOperationTagV1::SubmissionEffect,
            Self::Reconcile { .. } => ServiceOperationTagV1::SubmissionReconcile,
        }
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::Plan { plan_sha256 } => sha(plan_sha256),
            Self::Effect { effect_sha256 } => sha(effect_sha256),
            Self::Reconcile { reconcile_sha256 } => sha(reconcile_sha256),
        }
    }
}

impl OfficialJoinV1 {
    pub(crate) fn validate(&self) -> Result<(), String> {
        for (value, label) in [
            (&self.episode_id, "invalid official episode id"),
            (&self.candidate_id, "invalid official candidate id"),
            (&self.submission_id, "invalid official submission id"),
            (&self.comparable_base_id, "invalid comparable base id"),
            (&self.hardware_id, "invalid official hardware id"),
            (&self.adapter_receipt_id, "invalid adapter receipt id"),
            (&self.adapter_event_id, "invalid adapter event id"),
        ] {
            id(value, label)?;
        }
        self.competition.validate().map_err(str::to_string)?;
        self.objective.validate().map_err(str::to_string)?;
        self.profile.validate()?;
        if self.hardware_id != self.competition.hardware_id {
            return Err("official hardware does not match the competition key".into());
        }
        match &self.parents {
            OfficialParentsV1::Initial { revision: 1 } => Ok(()),
            OfficialParentsV1::Correction {
                next_revision,
                current_revision,
                current_result_id,
                current_binding_id,
            } if current_revision.checked_add(1) == Some(*next_revision) => {
                id(current_result_id, "invalid current official result id")?;
                sha(current_binding_id)
            }
            _ => Err("official result parents or revision are invalid".into()),
        }
    }
}

impl ServiceResultV1 {
    pub(crate) fn tag(&self) -> ServiceResultTagV1 {
        match self {
            Self::Board { .. } => ServiceResultTagV1::Board,
            Self::Candidate { .. } => ServiceResultTagV1::Candidate,
            Self::DeepCut { .. } => ServiceResultTagV1::DeepCut,
            Self::Context { .. } => ServiceResultTagV1::Context,
            Self::Submission { .. } => ServiceResultTagV1::Submission,
            Self::Official { .. } => ServiceResultTagV1::Official,
            Self::Worker { .. } => ServiceResultTagV1::Worker,
        }
    }

    pub(crate) fn validate_for(&self, operation: &ServiceOperationV1) -> Result<(), String> {
        operation.validate()?;
        let (intent, result_operation) = match self {
            Self::Board {
                service_intent_id,
                operation_tag,
                raw_sha256,
                outcome,
            } => {
                sha(raw_sha256)?;
                match outcome {
                    BoardOutcomeV1::Observed { outcome_sha256 } => sha(outcome_sha256)?,
                    BoardOutcomeV1::Failed { failure_sha256 } => sha(failure_sha256)?,
                }
                (service_intent_id, Some(*operation_tag))
            }
            Self::Candidate {
                service_intent_id,
                operation_tag,
                batch_sha256,
            } => {
                sha(batch_sha256)?;
                (service_intent_id, Some(*operation_tag))
            }
            Self::DeepCut {
                service_intent_id,
                evidence,
            } => {
                evidence.validate()?;
                (service_intent_id, Some(evidence.operation_tag()))
            }
            Self::Context {
                service_intent_id,
                events_sha256,
                anchors_sha256,
            } => {
                sha(events_sha256)?;
                sha(anchors_sha256)?;
                (service_intent_id, None)
            }
            Self::Submission {
                service_intent_id,
                evidence,
            } => {
                evidence.validate()?;
                (service_intent_id, Some(evidence.operation_tag()))
            }
            Self::Official {
                service_intent_id,
                result_sha256,
                reward_input_sha256,
            } => {
                sha(result_sha256)?;
                sha(reward_input_sha256)?;
                (service_intent_id, None)
            }
            Self::Worker {
                service_intent_id,
                operation_tag,
                control_sha256,
                lease_receipt_sha256,
            } => {
                sha(control_sha256)?;
                sha(lease_receipt_sha256)?;
                (service_intent_id, Some(*operation_tag))
            }
        };
        id(intent, "invalid result service intent")?;
        if intent != operation.service_intent_id()
            || self.tag() != operation.tag().contract_tuple().2
            || result_operation.is_some_and(|tag| tag != operation.tag())
        {
            return Err("service result does not match its exact operation".into());
        }
        Ok(())
    }
}
