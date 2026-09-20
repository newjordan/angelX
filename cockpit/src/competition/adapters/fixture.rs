use super::*;
use crate::competition::board::{
    BoardObservationV1, RawBoardJournalV1, RawObservationReceiptV1, RawObservationStatusV1,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FixtureCallV1 {
    Identify,
    FetchBoard,
    FetchPersonal,
    ObserveSource,
    Compare,
}

pub(crate) struct ScriptedAdapterV1 {
    identity: AdapterIdentityV1,
    competition: CompetitionIdentityV1,
    board: VecDeque<Result<BoardObservationV1, AdapterFailureV1>>,
    personal: VecDeque<Result<PersonalSubmissionObservationV1, AdapterFailureV1>>,
    source: VecDeque<Result<SourceAccessObservationV1, AdapterFailureV1>>,
    calls: std::sync::Mutex<Vec<FixtureCallV1>>,
    shadow: bool,
}

impl ScriptedAdapterV1 {
    pub(crate) fn new(
        identity: AdapterIdentityV1,
        competition: CompetitionIdentityV1,
        board: Vec<Result<BoardObservationV1, AdapterFailureV1>>,
    ) -> Self {
        Self {
            identity,
            competition,
            board: board.into(),
            personal: VecDeque::new(),
            source: VecDeque::new(),
            calls: std::sync::Mutex::new(Vec::new()),
            shadow: false,
        }
    }

    pub(crate) fn shadow(mut self) -> Self {
        self.shadow = true;
        self
    }

    pub(crate) fn with_personal(
        mut self,
        observations: Vec<Result<PersonalSubmissionObservationV1, AdapterFailureV1>>,
    ) -> Self {
        self.personal = observations.into();
        self
    }

    pub(crate) fn with_source(
        mut self,
        observations: Vec<Result<SourceAccessObservationV1, AdapterFailureV1>>,
    ) -> Self {
        self.source = observations.into();
        self
    }

    pub(crate) fn calls(&self) -> Vec<FixtureCallV1> {
        self.calls.lock().expect("fixture calls").clone()
    }

    fn note(&self, call: FixtureCallV1) {
        self.calls.lock().expect("fixture calls").push(call);
    }
}

impl CompetitionAdapterV1 for ScriptedAdapterV1 {
    fn identity(&self) -> &AdapterIdentityV1 {
        &self.identity
    }

    fn identify_competition(&mut self) -> Result<CompetitionIdentityV1, AdapterFailureV1> {
        self.note(FixtureCallV1::Identify);
        Ok(self.competition.clone())
    }

    fn fetch_board(
        &mut self,
        _cursor: Option<&ObservationCursorV1>,
    ) -> Result<BoardObservationV1, AdapterFailureV1> {
        self.note(FixtureCallV1::FetchBoard);
        self.board.pop_front().unwrap_or_else(|| {
            Err(AdapterFailureV1 {
                class: AdapterFailureClassV1::Invariant,
                retry: RetryPolicyV1::Never,
                detail_sha256: crate::cut::sha256_hex(b"fixture board exhausted"),
                provenance_sha256: crate::cut::sha256_hex(b"scripted-adapter/v1"),
            })
        })
    }

    fn fetch_personal_submissions(
        &mut self,
        _cursor: Option<&ObservationCursorV1>,
    ) -> Result<PersonalSubmissionObservationV1, AdapterFailureV1> {
        self.note(FixtureCallV1::FetchPersonal);
        self.personal
            .pop_front()
            .unwrap_or_else(|| Err(AdapterFailureV1::unsupported("personal_submissions")))
    }

    fn observe_source_access(
        &mut self,
        _cursor: Option<&ObservationCursorV1>,
    ) -> Result<SourceAccessObservationV1, AdapterFailureV1> {
        self.note(FixtureCallV1::ObserveSource);
        self.source
            .pop_front()
            .unwrap_or_else(|| Err(AdapterFailureV1::unsupported("source_access")))
    }

    fn compare_scores(
        &self,
        comparator: &ObjectiveComparatorV1,
        candidate: &ScoreV1,
        baseline: &ScoreV1,
    ) -> Result<Ordering, AdapterFailureV1> {
        self.note(FixtureCallV1::Compare);
        comparator
            .compare(candidate, baseline)
            .map_err(|_| AdapterFailureV1::unsupported("adapter_defined_score_comparison"))
    }
}

#[derive(Default)]
pub(crate) struct MemoryRawBoardJournalV1 {
    observations: Vec<BoardObservationV1>,
    digests: BTreeMap<String, String>,
}

impl MemoryRawBoardJournalV1 {
    pub(crate) fn observations(&self) -> &[BoardObservationV1] {
        &self.observations
    }
}

impl RawBoardJournalV1 for MemoryRawBoardJournalV1 {
    fn persist_raw(
        &mut self,
        observation: &BoardObservationV1,
    ) -> Result<RawObservationReceiptV1, String> {
        let digest = observation.provenance.raw_sha256.clone();
        if let Some(existing) = self.digests.get(&observation.observation_id) {
            if existing != &digest {
                return Err("observation id reused with different raw digest".to_string());
            }
            let journal_sequence = self
                .observations
                .iter()
                .position(|seen| seen.observation_id == observation.observation_id)
                .unwrap_or_default() as u64;
            return Ok(RawObservationReceiptV1 {
                observation_id: observation.observation_id.clone(),
                raw_sha256: digest,
                journal_sequence,
                status: RawObservationStatusV1::AlreadyPresent,
            });
        }
        let journal_sequence = self.observations.len() as u64;
        self.digests
            .insert(observation.observation_id.clone(), digest.clone());
        self.observations.push(observation.clone());
        Ok(RawObservationReceiptV1 {
            observation_id: observation.observation_id.clone(),
            raw_sha256: digest,
            journal_sequence,
            status: RawObservationStatusV1::Persisted,
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/adapters__fixture__tests.rs"]
mod tests;
