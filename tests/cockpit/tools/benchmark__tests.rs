use super::*;
fn input() -> Value {
    json!({"dataset":"diagnostic fixture", "metric":"latency (ms)", "direction":"minimize", "baseline":[10,20,30], "candidate":[5,10,15]})
}
#[test]
fn latency_reduction_is_distinct_from_speedup_and_win_rate() {
    let v = compare(&input()).unwrap();
    assert_eq!(v["mean_improvement_percent"], 50.0);
    assert_eq!(v["ratio_of_means_speedup"], 2.0);
    assert_eq!(v["paired_win_percent"], 100.0);
    assert_eq!(v["baseline"]["sample_stddev"], 10.0);
    assert_eq!(v["candidate"]["p95_nearest_rank"], 15.0);
    let mut args = input();
    args["direction"] = json!("maximize");
    assert_eq!(compare(&args).unwrap()["mean_improvement_percent"], -50.0);
}
#[test]
fn handles_zeros_ties_and_rejects_unpaired_or_invalid_data() {
    let mut args = input();
    args["baseline"] = json!([0]);
    args["candidate"] = json!([0]);
    let v = compare(&args).unwrap();
    assert!(v["mean_change_percent"].is_null());
    assert!(v["baseline"]["sample_stddev"].is_null());
    assert_eq!(v["paired_ties"], 1);
    for invalid in [
        json!([]),
        json!([-1]),
        json!([1, 2]),
        json!(["2"]),
        json!([1e200]),
    ] {
        args["candidate"] = invalid;
        assert!(compare(&args).is_err());
    }
}
