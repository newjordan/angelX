//! Presentation-only measurements reuse the competition layer's exact decimals.
use crate::competition::schema::{ComparatorKindV1, ObjectiveComparatorV1, ScoreV1};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Measurement {
    pub(crate) value: ScoreV1,
    pub(crate) baseline: Option<ScoreV1>,
    pub(crate) dataset: Dataset,
    pub(crate) lower_is_better: bool,
    pub(crate) receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Dataset {
    FullDevelopment,
    Diagnostic,
    OfficialReport,
}

impl Measurement {
    pub(crate) fn dataset_label(&self) -> &'static str {
        match self.dataset {
            Dataset::FullDevelopment => "full dev",
            Dataset::Diagnostic => "subset",
            Dataset::OfficialReport => "official report",
        }
    }

    pub(crate) fn comparison(&self) -> &'static str {
        let Some(baseline) = &self.baseline else {
            return match self.dataset {
                Dataset::FullDevelopment => "reported",
                Dataset::Diagnostic => "sample",
                Dataset::OfficialReport => "reported",
            };
        };
        let comparator = ObjectiveComparatorV1 {
            objective_id: "research-display-only".into(),
            version: "1".into(),
            kind: if self.lower_is_better {
                ComparatorKindV1::LowerIsBetter
            } else {
                ComparatorKindV1::HigherIsBetter
            },
        };
        match comparator.compare(&self.value, baseline) {
            Ok(std::cmp::Ordering::Greater) => "better",
            Ok(std::cmp::Ordering::Less) => "worse",
            Ok(std::cmp::Ordering::Equal) => "same",
            Err(_) => "unknown",
        }
    }

    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.receipt_sha256.len() != 64
            || !self
                .receipt_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid measurement receipt reference");
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/research_workspace__measurement__tests.rs"]
mod tests;
