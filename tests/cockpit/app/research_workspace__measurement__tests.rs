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
