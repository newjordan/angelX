use super::profile::DeepCutProfileIdentityV1;
use super::schema::{ActionIntentV1, CompetitionKeyV1, LaneIdV1, ObjectiveComparatorV1, ScoreV1};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) const RUNTIME_CONTRACT_V1: &str = "angel.competition-runtime-contract/v1";
pub(crate) const REDUCER_CONTRACT_SCHEMA_V1: &str = "angel.competition-runtime-reducer-contract/v1";
pub(crate) const RUNTIME_ACTION_INTENT_V1: &str = "angel.competition-runtime-action-intent/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ServiceSlotV1 {
    FrontierLane,
    DeepCutLane,
    Observer,
    Context,
    SubmissionPump,
    WorkerControl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ServiceActionV1 {
    ObserveBoard,
    #[serde(rename = "service-lane:frontier-guard")]
    ServiceFrontierLane,
    #[serde(rename = "service-lane:deep-cut")]
    ServiceDeepCutLane,
    ServiceContext,
    PumpSubmissions,
    InspectWorkers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ServiceOperationTagV1 {
    BoardObserve,
    BoardRefresh,
    BoardRemediate,
    CandidateReplay,
    CandidatePort,
    CandidateExplore,
    DeepCutRegisterWorker,
    DeepCutRegisterEpisode,
    DeepCutTerminal,
    DeepCutRollover,
    Context,
    SubmissionPlan,
    SubmissionEffect,
    SubmissionReconcile,
    OfficialResult,
    WorkerInspect,
    WorkerNudge,
    WorkerReplace,
    WorkerCheckpoint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ServiceOperationV1 {
    Board {
        operation_tag: ServiceOperationTagV1,
        service_intent_id: String,
        adapter_id: String,
        request_sha256: String,
    },
    Candidate {
        operation_tag: ServiceOperationTagV1,
        service_intent_id: String,
        work_id: String,
        board_decision_sha256: String,
        lineage_input_sha256: String,
    },
    DeepCut {
        operation_tag: ServiceOperationTagV1,
        service_intent_id: String,
        work_id: String,
        profile_sha256: String,
        episode_input_sha256: String,
    },
    Context {
        service_intent_id: String,
        dossier_revision: u64,
        context_head_sha256: String,
    },
    Submission {
        operation_tag: ServiceOperationTagV1,
        service_intent_id: String,
        journal_head_sha256: String,
        repository_sha256: String,
        spool_sha256: String,
    },
    OfficialResult {
        service_intent_id: String,
        join: Box<OfficialJoinV1>,
    },
    WorkerControl {
        operation_tag: ServiceOperationTagV1,
        service_intent_id: String,
        lease_key_sha256: String,
        lease_id: String,
        generation: u64,
        checkpoint_revision: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OfficialJoinV1 {
    pub(crate) episode_id: String,
    pub(crate) candidate_id: String,
    pub(crate) submission_id: String,
    pub(crate) comparable_base_id: String,
    pub(crate) comparable_base_score: ScoreV1,
    pub(crate) competition: CompetitionKeyV1,
    pub(crate) objective: ObjectiveComparatorV1,
    pub(crate) profile: DeepCutProfileIdentityV1,
    pub(crate) hardware_id: String,
    pub(crate) adapter_receipt_id: String,
    pub(crate) adapter_event_id: String,
    pub(crate) parents: OfficialParentsV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum OfficialParentsV1 {
    Initial {
        revision: u64,
    },
    Correction {
        next_revision: u64,
        current_revision: u64,
        current_result_id: String,
        current_binding_id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ServiceResultTagV1 {
    #[serde(rename = "board_evidence")]
    Board,
    #[serde(rename = "candidate_evidence")]
    Candidate,
    #[serde(rename = "deep_cut_evidence")]
    DeepCut,
    #[serde(rename = "context_evidence")]
    Context,
    #[serde(rename = "submission_evidence")]
    Submission,
    #[serde(rename = "official_evidence")]
    Official,
    #[serde(rename = "worker_evidence")]
    Worker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StateComponentV1 {
    Board,
    Candidates,
    Episodes,
    Dossier,
    Submissions,
    Rewards,
    Patterns,
    Journal,
    Leases,
    Scheduler,
    Director,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LeafSelectorV1 {
    BoardObservation,
    BoardCanonical,
    CandidateWorkItem,
    EpisodeWorkItem,
    DossierHead,
    DossierAnchors,
    SubmissionWorkItem,
    OfficialResult,
    RewardBinding,
    PatternAggregate,
    ActionJournal,
    LeaseWorkItem,
    SchedulerSlot,
    DirectorHealth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RebaseRuleV1 {
    ExactReadSetUnchanged,
    InsertAbsentMapKey { component: StateComponentV1 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReducerContractV1 {
    pub(crate) schema: String,
    pub(crate) operation_tag: ServiceOperationTagV1,
    pub(crate) reducer_version_sha256: String,
    pub(crate) legal_slot: ServiceSlotV1,
    pub(crate) legal_action: ServiceActionV1,
    pub(crate) result_tag: ServiceResultTagV1,
    pub(crate) read_components: BTreeSet<StateComponentV1>,
    pub(crate) write_selectors: BTreeSet<LeafSelectorV1>,
    pub(crate) rebase_rule: RebaseRuleV1,
    pub(crate) selector_contract_sha256: String,
    pub(crate) reducer_contract_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "binding", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DispatchBindingV1 {
    Lane {
        decision_sha256: String,
        tick: u64,
        slot: ServiceSlotV1,
        action: ServiceActionV1,
        lane: LaneIdV1,
        lane_sequence: u64,
        assignment_sha256: String,
        lease_key_sha256: String,
        lease_id: String,
        generation: u64,
        checkpoint_revision: u64,
    },
    Independent {
        decision_sha256: String,
        tick: u64,
        slot: ServiceSlotV1,
        action: ServiceActionV1,
        service_sequence: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthorizationPreimageV1 {
    pub(crate) runtime_contract: String,
    pub(crate) registry_sha256: String,
    pub(crate) reducer_contract_id: String,
    pub(crate) reducer_version_sha256: String,
    pub(crate) operation_sha256: String,
    pub(crate) service_intent_id: String,
    pub(crate) expected_result_tag: ServiceResultTagV1,
    pub(crate) selector_contract_sha256: String,
    pub(crate) dispatch: DispatchBindingV1,
    pub(crate) action_intent: ActionIntentV1,
    pub(crate) pre_runtime_revision: u64,
    pub(crate) pre_runtime_sha256: String,
    pub(crate) pre_director_sha256: String,
    pub(crate) pre_scheduler_sha256: String,
    pub(crate) typed_input_evidence_sha256: String,
}
