use super::runtime_schema::*;
use super::schema::ActionKindV1;
use super::schema_validation::{validate_id, validate_sha256};
use serde::Serialize;

pub(super) fn canonical_sha256<T: Serialize>(label: &str, value: &T) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| crate::knowledge::cut::sha256_hex(&bytes))
        .map_err(|error| format!("encode {label}: {error}"))
}

pub(super) fn id(value: &str, label: &'static str) -> Result<(), String> {
    validate_id(value, label).map_err(str::to_string)
}

pub(super) fn sha(value: &str) -> Result<(), String> {
    validate_sha256(value).map_err(str::to_string)
}

impl ServiceOperationTagV1 {
    pub(crate) const ALL: [Self; 19] = [
        Self::BoardObserve,
        Self::BoardRefresh,
        Self::BoardRemediate,
        Self::CandidateReplay,
        Self::CandidatePort,
        Self::CandidateExplore,
        Self::DeepCutRegisterWorker,
        Self::DeepCutRegisterEpisode,
        Self::DeepCutTerminal,
        Self::DeepCutRollover,
        Self::Context,
        Self::SubmissionPlan,
        Self::SubmissionEffect,
        Self::SubmissionReconcile,
        Self::OfficialResult,
        Self::WorkerInspect,
        Self::WorkerNudge,
        Self::WorkerReplace,
        Self::WorkerCheckpoint,
    ];

    pub(crate) fn contract_tuple(
        self,
    ) -> (
        ServiceSlotV1,
        ServiceActionV1,
        ServiceResultTagV1,
        ActionKindV1,
    ) {
        use ActionKindV1 as J;
        use ServiceActionV1 as A;
        use ServiceOperationTagV1 as O;
        use ServiceResultTagV1 as R;
        use ServiceSlotV1 as S;
        match self {
            O::BoardObserve | O::BoardRefresh | O::BoardRemediate => {
                (S::Observer, A::ObserveBoard, R::Board, J::ObserveBoard)
            }
            O::CandidateReplay | O::CandidatePort => (
                S::FrontierLane,
                A::ServiceFrontierLane,
                R::Candidate,
                J::AcquireSource,
            ),
            O::CandidateExplore => (
                S::FrontierLane,
                A::ServiceFrontierLane,
                R::Candidate,
                J::VerifyCandidate,
            ),
            O::DeepCutRegisterWorker => (
                S::DeepCutLane,
                A::ServiceDeepCutLane,
                R::DeepCut,
                J::DispatchWorker,
            ),
            O::DeepCutRegisterEpisode => (
                S::DeepCutLane,
                A::ServiceDeepCutLane,
                R::DeepCut,
                J::StartEpisode,
            ),
            O::DeepCutTerminal | O::DeepCutRollover => (
                S::DeepCutLane,
                A::ServiceDeepCutLane,
                R::DeepCut,
                J::PublishCheckpoint,
            ),
            O::Context => (
                S::Context,
                A::ServiceContext,
                R::Context,
                J::PublishCheckpoint,
            ),
            O::SubmissionPlan | O::SubmissionEffect => (
                S::SubmissionPump,
                A::PumpSubmissions,
                R::Submission,
                J::SubmitCandidate,
            ),
            O::SubmissionReconcile => (
                S::SubmissionPump,
                A::PumpSubmissions,
                R::Submission,
                J::ReconcileSubmission,
            ),
            O::OfficialResult => (
                S::SubmissionPump,
                A::PumpSubmissions,
                R::Official,
                J::BindOfficialResult,
            ),
            O::WorkerInspect | O::WorkerNudge | O::WorkerReplace => (
                S::WorkerControl,
                A::InspectWorkers,
                R::Worker,
                J::DispatchWorker,
            ),
            O::WorkerCheckpoint => (
                S::WorkerControl,
                A::InspectWorkers,
                R::Worker,
                J::PublishCheckpoint,
            ),
        }
    }
}

impl ServiceOperationV1 {
    pub(crate) fn tag(&self) -> ServiceOperationTagV1 {
        match self {
            Self::Board { operation_tag, .. }
            | Self::Candidate { operation_tag, .. }
            | Self::DeepCut { operation_tag, .. }
            | Self::Submission { operation_tag, .. }
            | Self::WorkerControl { operation_tag, .. } => *operation_tag,
            Self::Context { .. } => ServiceOperationTagV1::Context,
            Self::OfficialResult { .. } => ServiceOperationTagV1::OfficialResult,
        }
    }

    pub(crate) fn service_intent_id(&self) -> &str {
        match self {
            Self::Board {
                service_intent_id, ..
            }
            | Self::Candidate {
                service_intent_id, ..
            }
            | Self::DeepCut {
                service_intent_id, ..
            }
            | Self::Context {
                service_intent_id, ..
            }
            | Self::Submission {
                service_intent_id, ..
            }
            | Self::OfficialResult {
                service_intent_id, ..
            }
            | Self::WorkerControl {
                service_intent_id, ..
            } => service_intent_id,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        use ServiceOperationTagV1 as O;
        id(self.service_intent_id(), "invalid runtime service intent")?;
        match self {
            Self::Board {
                operation_tag,
                adapter_id,
                request_sha256,
                ..
            } => {
                if !matches!(
                    operation_tag,
                    O::BoardObserve | O::BoardRefresh | O::BoardRemediate
                ) {
                    return Err("board operation tag crosses its closed family".into());
                }
                id(adapter_id, "invalid board adapter id")?;
                sha(request_sha256)
            }
            Self::Candidate {
                operation_tag,
                work_id,
                board_decision_sha256,
                lineage_input_sha256,
                ..
            } => {
                if !matches!(
                    operation_tag,
                    O::CandidateReplay | O::CandidatePort | O::CandidateExplore
                ) {
                    return Err("candidate operation tag crosses its closed family".into());
                }
                id(work_id, "invalid candidate work id")?;
                sha(board_decision_sha256)?;
                sha(lineage_input_sha256)
            }
            Self::DeepCut {
                operation_tag,
                work_id,
                profile_sha256,
                episode_input_sha256,
                ..
            } => {
                if !matches!(
                    operation_tag,
                    O::DeepCutRegisterWorker
                        | O::DeepCutRegisterEpisode
                        | O::DeepCutTerminal
                        | O::DeepCutRollover
                ) {
                    return Err("deep-cut operation tag crosses its closed family".into());
                }
                id(work_id, "invalid deep-cut work id")?;
                sha(profile_sha256)?;
                sha(episode_input_sha256)
            }
            Self::Context {
                dossier_revision,
                context_head_sha256,
                ..
            } => {
                if *dossier_revision == 0 {
                    return Err("context revision must be nonzero".into());
                }
                sha(context_head_sha256)
            }
            Self::Submission {
                operation_tag,
                journal_head_sha256,
                repository_sha256,
                spool_sha256,
                ..
            } => {
                if !matches!(
                    operation_tag,
                    O::SubmissionPlan | O::SubmissionEffect | O::SubmissionReconcile
                ) {
                    return Err("submission operation tag crosses its closed family".into());
                }
                sha(journal_head_sha256)?;
                sha(repository_sha256)?;
                sha(spool_sha256)
            }
            Self::OfficialResult { join, .. } => join.validate(),
            Self::WorkerControl {
                operation_tag,
                lease_key_sha256,
                lease_id,
                generation,
                ..
            } => {
                if !matches!(
                    operation_tag,
                    O::WorkerInspect | O::WorkerNudge | O::WorkerReplace | O::WorkerCheckpoint
                ) {
                    return Err("worker operation tag crosses its closed family".into());
                }
                sha(lease_key_sha256)?;
                id(lease_id, "invalid worker lease id")?;
                if *generation == 0 {
                    return Err("worker generation must be nonzero".into());
                }
                Ok(())
            }
        }
    }

    pub(crate) fn canonical_sha256(&self) -> Result<String, String> {
        self.validate()?;
        canonical_sha256("service operation", &(RUNTIME_CONTRACT_V1, self))
    }
}
