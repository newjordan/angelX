use super::director::CompetitionDirectorStateV1;
use super::migration_qualification::QualificationPermitV1;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum MigrationQualificationMarkerV1 {
    NativeV1,
    QualificationRequired {
        migration_id: String,
        source_sha256: String,
        target_sha256: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MigratedDirectorStateV1 {
    pub(super) marker: MigrationQualificationMarkerV1,
    pub(super) director: CompetitionDirectorStateV1,
}

impl MigratedDirectorStateV1 {
    pub(super) fn release(
        self,
        permit: QualificationPermitV1,
    ) -> Result<CompetitionDirectorStateV1, String> {
        match &self.marker {
            MigrationQualificationMarkerV1::QualificationRequired { migration_id, .. }
                if migration_id == permit.migration_id() =>
            {
                Ok(self.director)
            }
            _ => Err("migration state lacks matching qualification authority".into()),
        }
    }

    #[cfg(test)]
    pub(super) fn commit_health_drift_for_test(&mut self) -> Result<(), String> {
        let mut director = self.director.clone();
        let next = director.clone();
        director.commit(
            next,
            super::schema::DirectorHealthStateV1::Retrying,
            Some("unrelated health drift"),
            "full_board_refresh",
            50,
        )?;
        self.director = director;
        Ok(())
    }
}
