use super::flywheel_files::{
    FlywheelRowV1, FlywheelStateFilesV1, canonical_score, iso8601_epoch_ms, malformed, read_state,
};
use super::{
    AdapterFailureV1, CompetitionAdapterV1, CompetitionIdentityV1, PersonalSubmissionObservationV1,
};
use crate::competition::board::{
    BOARD_OBSERVATION_SCHEMA_V1, BoardChangeV1, BoardEntryV1, BoardObservationKindV1,
    BoardObservationV1, ObservationCursorV1, ObservationProvenanceV1, ObservationSourceV1,
};
use crate::competition::schema::{
    AdapterCapabilityV1, AdapterIdentityV1, ComparatorKindV1, CompetitionKeyV1,
    ObjectiveComparatorV1,
};
use std::path::Path;

pub(crate) const FLYWHEEL_ADAPTER_ID: &str = "flywheel-shadow";

/// Read-only (`shadow`) `CompetitionAdapterV1` over a flywheel state directory:
/// identity from `board.json`, board and personal observations from the raw
/// `api-rows.json` capture. No method writes; unsupported capabilities are
/// typed failures, never panics, never fabricated rows.
#[derive(Debug)]
pub(crate) struct FlywheelAdapterV1 {
    identity: AdapterIdentityV1,
    competition: CompetitionIdentityV1,
    threshold_bips: i64,
    rows: Vec<FlywheelRowV1>,
    me: Option<String>,
    observed_at_ms: u64,
    raw_rows_sha256: String,
}

impl FlywheelAdapterV1 {
    pub(crate) fn open(state_dir: &Path) -> Result<Self, AdapterFailureV1> {
        Self::open_with_me(state_dir, None)
    }

    pub(crate) fn open_as(state_dir: &Path, me: &str) -> Result<Self, AdapterFailureV1> {
        Self::open_with_me(state_dir, Some(me))
    }

    fn open_with_me(state_dir: &Path, me_override: Option<&str>) -> Result<Self, AdapterFailureV1> {
        let files = read_state(state_dir)?;
        let kind = match files.card.direction.as_str() {
            "higher" => ComparatorKindV1::HigherIsBetter,
            _ => ComparatorKindV1::LowerIsBetter,
        };
        let observed_at_ms = iso8601_epoch_ms(&files.api.ts)
            .ok_or_else(|| malformed("api-rows capture timestamp is not ISO-8601"))?;
        let mut identity_bytes = files.board_bytes.clone();
        identity_bytes.extend_from_slice(&files.rows_bytes);
        let me = me_override
            .map(str::to_string)
            .or(files.card.me.clone())
            .or_else(|| self_me_from_rows(&files));
        let competition = CompetitionKeyV1 {
            platform_id: "yukon".into(),
            competition_id: files.card.benchmark.clone(),
            field_id: "official".into(),
            benchmark_id: files.card.benchmark_id.clone(),
            profile_id: "default".into(),
            hardware_id: state_dir
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("flywheel")
                .into(),
        };
        Ok(Self {
            identity: AdapterIdentityV1 {
                adapter_id: FLYWHEEL_ADAPTER_ID.into(),
                adapter_version: "1".into(),
                runtime_sha256: crate::cut::sha256_hex(&identity_bytes),
                capabilities: [
                    AdapterCapabilityV1::Board,
                    AdapterCapabilityV1::PersonalSubmissions,
                    AdapterCapabilityV1::Results,
                ]
                .into_iter()
                .collect(),
            },
            competition: CompetitionIdentityV1 {
                competition,
                comparator: ObjectiveComparatorV1 {
                    objective_id: "officialScore".into(),
                    version: "1".into(),
                    kind,
                },
            },
            threshold_bips: files.card.threshold_bips,
            rows: files.api.rows,
            me,
            observed_at_ms,
            raw_rows_sha256: crate::cut::sha256_hex(&files.rows_bytes),
        })
    }

    pub(crate) fn threshold_bips(&self) -> i64 {
        self.threshold_bips
    }

    pub(crate) fn competition(&self) -> &CompetitionIdentityV1 {
        &self.competition
    }

    pub(crate) fn me(&self) -> Option<&str> {
        self.me.as_deref()
    }

    pub(crate) fn observed_at_ms(&self) -> u64 {
        self.observed_at_ms
    }

    pub(crate) fn frontier_row(&self) -> Option<&FlywheelRowV1> {
        self.rows
            .iter()
            .filter(|row| row.is_promoted())
            .max_by_key(|row| row.finished_ms())
    }

    pub(crate) fn personal_best(&self) -> Option<&FlywheelRowV1> {
        let lower = self.competition.comparator.kind == ComparatorKindV1::LowerIsBetter;
        self.rows
            .iter()
            .filter(|row| self.owns(row) && row.official_score.is_some())
            .fold(None, |selected: Option<&FlywheelRowV1>, row| {
                let better = match selected {
                    None => true,
                    Some(best_row) => {
                        let (candidate, best) = (
                            row.official_score.unwrap(),
                            best_row.official_score.unwrap(),
                        );
                        if lower {
                            candidate < best
                        } else {
                            candidate > best
                        }
                    }
                };
                Some(match better {
                    true => row,
                    false => selected.expect("better implies a previous selection"),
                })
            })
    }

    pub(crate) fn personal_in_flight(&self) -> Vec<(String, String)> {
        self.rows
            .iter()
            .filter(|row| self.owns(row) && row.in_flight())
            .map(|row| (row.id.clone(), row.status.clone().unwrap_or_default()))
            .collect()
    }

    pub(crate) fn personal_submission_ids(&self) -> std::collections::BTreeSet<String> {
        self.rows
            .iter()
            .filter(|row| self.owns(row))
            .map(|row| row.id.clone())
            .collect()
    }

    fn owns(&self, row: &FlywheelRowV1) -> bool {
        self.me.as_deref() == Some(row.solver_username.as_str())
    }

    fn entry(
        &self,
        row: &FlywheelRowV1,
        score: f64,
        personal: bool,
    ) -> Result<BoardEntryV1, AdapterFailureV1> {
        Ok(BoardEntryV1 {
            entry_id: row.id.clone(),
            participant_id: row.solver_username.clone(),
            submission_id: Some(row.id.clone()),
            rank: None,
            score: canonical_score(score)?,
            personal,
            source: None,
        })
    }
}

fn self_me_from_rows(files: &FlywheelStateFilesV1) -> Option<String> {
    let sub = files
        .card
        .my_best
        .as_ref()
        .and_then(|best| best.sub.as_deref())?;
    files
        .api
        .rows
        .iter()
        .find(|row| row.id == sub)
        .map(|row| row.solver_username.clone())
}

impl CompetitionAdapterV1 for FlywheelAdapterV1 {
    fn identity(&self) -> &AdapterIdentityV1 {
        &self.identity
    }

    fn identify_competition(&mut self) -> Result<CompetitionIdentityV1, AdapterFailureV1> {
        Ok(self.competition.clone())
    }

    fn fetch_board(
        &mut self,
        _cursor: Option<&ObservationCursorV1>,
    ) -> Result<BoardObservationV1, AdapterFailureV1> {
        let mut changes = Vec::new();
        let mut latest_event_ms = 0u64;
        for row in self.rows.iter().filter(|row| row.is_promoted()) {
            let score = row
                .official_score
                .expect("promoted rows are scored by is_promoted");
            changes.push(BoardChangeV1::Upsert {
                entry: Box::new(self.entry(row, score, self.owns(row))?),
            });
            latest_event_ms = latest_event_ms.max(row.finished_ms());
        }
        if changes.is_empty() {
            return Err(malformed("no promoted rows with official scores"));
        }
        Ok(BoardObservationV1 {
            schema: BOARD_OBSERVATION_SCHEMA_V1.into(),
            observation_id: format!("flywheel-board-{}", &self.raw_rows_sha256[..16]),
            competition: self.competition.competition.clone(),
            kind: BoardObservationKindV1::Full,
            cursor: ObservationCursorV1 {
                opaque: None,
                sequence: None,
            },
            changes,
            provenance: ObservationProvenanceV1 {
                adapter: self.identity.clone(),
                source: ObservationSourceV1::LiveApi,
                observed_at_ms: self.observed_at_ms,
                platform_event_at_ms: (latest_event_ms > 0).then_some(latest_event_ms),
                raw_sha256: self.raw_rows_sha256.clone(),
            },
        })
    }

    fn fetch_personal_submissions(
        &mut self,
        _cursor: Option<&ObservationCursorV1>,
    ) -> Result<PersonalSubmissionObservationV1, AdapterFailureV1> {
        let me = self
            .me
            .clone()
            .ok_or_else(|| AdapterFailureV1::unsupported("flywheel.me"))?;
        let mut entries = Vec::new();
        for row in self
            .rows
            .iter()
            .filter(|row| row.solver_username == me && row.official_score.is_some())
        {
            let score = row.official_score.expect("scored row");
            entries.push(self.entry(row, score, true)?);
        }
        let raw = crate::cut::sha256_hex(
            entries
                .iter()
                .map(|entry| entry.entry_id.as_str())
                .collect::<Vec<_>>()
                .join("\n")
                .as_bytes(),
        );
        Ok(PersonalSubmissionObservationV1 {
            competition: self.competition.competition.clone(),
            cursor: ObservationCursorV1::default(),
            entries,
            observed_at_ms: self.observed_at_ms,
            raw_sha256: raw,
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/competition/adapters__flywheel_tests.rs"]
mod tests;
