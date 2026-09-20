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
mod tests {
    use super::*;
    #[test]
    fn comparisons_are_exact_and_do_not_assign_official_rewards() {
        let sample: Measurement = serde_json::from_value(serde_json::json!({
            "value": "0.844070999999999999999", "baseline": "0.844071",
            "dataset": "full_development", "lower_is_better": true,
            "receipt_sha256": "a".repeat(64)
        }))
        .unwrap();
        assert_eq!(sample.comparison(), "better");
        assert_eq!(sample.dataset_label(), "full dev");
        assert!(sample.validate().is_ok());
        let mut higher = sample.clone();
        higher.lower_is_better = false;
        assert_eq!(higher.comparison(), "worse");
    }
    #[test]
    fn missing_comparator_does_not_invent_a_baseline_role() {
        for dataset in ["full_development", "official_report"] {
            let sample: Measurement = serde_json::from_value(serde_json::json!({
                "value": "0.843792", "baseline": null, "dataset": dataset,
                "lower_is_better": true, "receipt_sha256": "a".repeat(64)
            }))
            .unwrap();
            assert_eq!(sample.comparison(), "reported");
        }
    }
    #[test]
    fn noncanonical_and_nonfinite_values_are_rejected() {
        for value in ["NaN", "inf", "0.80", "1e-3", "-0"] {
            assert!(
                serde_json::from_value::<Measurement>(serde_json::json!({
                    "value": value, "baseline": null, "dataset": "diagnostic",
                    "lower_is_better": true, "receipt_sha256": "a".repeat(64)
                }))
                .is_err()
            );
        }
    }
}
