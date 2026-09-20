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
mod tests {
    use super::*;
    use crate::competition::board::{
        BOARD_OBSERVATION_SCHEMA_V1, BoardObservationKindV1, ObservationProvenanceV1,
        ObservationSourceV1,
    };
    use crate::competition::schema::{AdapterCapabilityV1, ComparatorKindV1};

    fn key() -> CompetitionKeyV1 {
        CompetitionKeyV1 {
            platform_id: "fixture".into(),
            competition_id: "adapter-contract".into(),
            field_id: "field".into(),
            benchmark_id: "bench".into(),
            profile_id: "profile".into(),
            hardware_id: "hardware".into(),
        }
    }

    fn identity() -> AdapterIdentityV1 {
        AdapterIdentityV1 {
            adapter_id: "scripted".into(),
            adapter_version: "1".into(),
            runtime_sha256: crate::cut::sha256_hex(b"fixture-runtime"),
            capabilities: [
                AdapterCapabilityV1::Board,
                AdapterCapabilityV1::PersonalSubmissions,
                AdapterCapabilityV1::SourceAccess,
            ]
            .into_iter()
            .collect(),
        }
    }

    fn comparator(kind: ComparatorKindV1) -> ObjectiveComparatorV1 {
        ObjectiveComparatorV1 {
            objective_id: "score".into(),
            version: "1".into(),
            kind,
        }
    }

    fn board(id: &str) -> BoardObservationV1 {
        BoardObservationV1 {
            schema: BOARD_OBSERVATION_SCHEMA_V1.into(),
            observation_id: id.into(),
            competition: key(),
            kind: BoardObservationKindV1::Full,
            cursor: ObservationCursorV1::default(),
            changes: Vec::new(),
            provenance: ObservationProvenanceV1 {
                adapter: identity(),
                source: ObservationSourceV1::Fixture,
                observed_at_ms: 1,
                platform_event_at_ms: None,
                raw_sha256: crate::cut::sha256_hex(id.as_bytes()),
            },
        }
    }

    fn scripted(observation: BoardObservationV1) -> ScriptedAdapterV1 {
        ScriptedAdapterV1::new(
            identity(),
            CompetitionIdentityV1 {
                competition: key(),
                comparator: comparator(ComparatorKindV1::HigherIsBetter),
            },
            vec![Ok(observation)],
        )
    }

    #[test]
    fn shared_fixture_contract_matrix() {
        let personal = PersonalSubmissionObservationV1 {
            competition: key(),
            cursor: ObservationCursorV1::default(),
            entries: Vec::new(),
            observed_at_ms: 1,
            raw_sha256: crate::cut::sha256_hex(b"personal"),
        };
        let source = SourceAccessObservationV1 {
            competition: key(),
            cursor: ObservationCursorV1::default(),
            claims: Vec::new(),
            observed_at_ms: 1,
            raw_sha256: crate::cut::sha256_hex(b"source"),
        };
        let mut adapter = scripted(board("board"))
            .with_personal(vec![Ok(personal)])
            .with_source(vec![Ok(source)]);
        assert_eq!(adapter.identify_competition().unwrap().competition, key());
        assert_eq!(adapter.fetch_board(None).unwrap().observation_id, "board");
        assert!(adapter.fetch_personal_submissions(None).is_ok());
        assert!(adapter.observe_source_access(None).is_ok());
        let one = ScoreV1::new("1").unwrap();
        let two = ScoreV1::new("2").unwrap();
        assert_eq!(
            adapter
                .compare_scores(&comparator(ComparatorKindV1::HigherIsBetter), &two, &one)
                .unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            adapter
                .compare_scores(&comparator(ComparatorKindV1::LowerIsBetter), &one, &two)
                .unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn shadow_adapter_is_read_only_and_matches_canonical_fixture_digest() {
        let expected = board("shadow");
        let digest = expected.provenance.raw_sha256.clone();
        let mut adapter = scripted(expected).shadow();
        let actual = adapter.fetch_board(None).unwrap().provenance.raw_sha256;
        assert!(adapter.shadow);
        assert_eq!(actual, digest);
        assert_eq!(adapter.calls(), vec![FixtureCallV1::FetchBoard]);
    }
}
