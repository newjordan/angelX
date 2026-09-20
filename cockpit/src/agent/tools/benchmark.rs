//! Deterministic comparisons of caller-supplied paired measurements.
use crate::agent::club::ToolDef;
use crate::agent::harness::Tool;
use serde_json::{Value, json};

pub(crate) struct BenchmarkCompareTool;

fn samples(value: &Value) -> Result<Vec<f64>, String> {
    value
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 256)
        .ok_or("provide 1..256 paired samples per candidate")?
        .iter()
        .map(|v| {
            v.as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1e100)
                .ok_or_else(|| "samples must be finite numbers between 0 and 1e100".into())
        })
        .collect()
}

fn summary(values: &[f64]) -> Value {
    let n = values.len();
    let mean = values.iter().sum::<f64>() / n as f64;
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    let median = (ordered[(n - 1) / 2] + ordered[n / 2]) / 2.0;
    let variance =
        (n > 1).then(|| values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64);
    json!({"n":n, "mean":mean, "median":median, "min":ordered[0], "max":ordered[n-1],
        "p95_nearest_rank":ordered[(0.95 * n as f64).ceil() as usize - 1],
        "sample_stddev":variance.map(f64::sqrt)})
}

fn compare(args: &Value) -> Result<Value, String> {
    let label = |key: &str| {
        args[key]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 256)
            .ok_or_else(|| format!("{key} must identify the measurement (1..256 bytes)"))
    };
    let dataset = label("dataset")?;
    let metric = label("metric")?;
    let direction = args["direction"]
        .as_str()
        .filter(|s| matches!(*s, "minimize" | "maximize"))
        .ok_or("direction must be minimize or maximize")?;
    let baseline = samples(&args["baseline"])?;
    let candidate = samples(&args["candidate"])?;
    if baseline.len() != candidate.len() {
        return Err(
            "baseline and candidate must be matched pairs on the same cases/dataset".into(),
        );
    }
    let before = summary(&baseline);
    let after = summary(&candidate);
    let b = before["mean"].as_f64().unwrap();
    let c = after["mean"].as_f64().unwrap();
    let sign = if direction == "maximize" { 1.0 } else { -1.0 };
    let change = (b > 0.0)
        .then(|| (c - b) / b * 100.0)
        .filter(|v| v.is_finite());
    let mut wins = 0;
    let mut ties = 0;
    for (b, c) in baseline.iter().zip(&candidate) {
        if (c - b) * sign > 0.0 {
            wins += 1;
        }
        if b == c {
            ties += 1;
        }
    }
    Ok(json!({
        "schema":"angel.benchmark-comparison/v1", "source":"caller_supplied_measurements",
        "dataset":dataset, "metric":metric, "direction":direction,
        "baseline":before, "candidate":after,
        "mean_delta":c-b, "mean_change_percent":change,
        "mean_improvement_percent":change.map(|v| v * sign),
        "ratio_of_means_speedup":(direction == "minimize" && c > 0.0 && b > 0.0).then(|| b/c).filter(|v| v.is_finite()),
        "paired_wins":wins, "paired_ties":ties, "paired_losses":baseline.len()-wins-ties,
        "paired_win_percent":100.0 * wins as f64 / baseline.len() as f64,
        "interpretation":"Deterministic arithmetic on supplied matched samples, not independently verified acceptance. Positive improvement is better. Zero baseline makes relative percentages undefined (null). Dataset labels are supplied by the caller; do not mix diagnostic, full-development and official results. No statistical significance or generalization claim."
    }))
}

impl Tool for BenchmarkCompareTool {
    fn name(&self) -> &str {
        "benchmark_compare"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name:self.name().into(),
            description:"Calculate measured benchmark percentages from paired baseline/candidate samples on one named dataset: mean/median/p95, sample standard deviation, signed improvement, ratio-of-means speedup, paired wins/ties/losses. Local deterministic arithmetic; no network, model estimates or acceptance authority. Samples must be nonnegative and matched in order; separate diagnostic, full-development and official cohorts.".into(),
            params:json!({"type":"object", "properties":{
                "dataset":{"type":"string", "maxLength":256},
                "metric":{"type":"string", "maxLength":256, "description":"Metric and unit, e.g. kernel latency (ms)."},
                "direction":{"type":"string", "enum":["minimize","maximize"]},
                "baseline":{"type":"array", "minItems":1, "maxItems":256, "items":{"type":"number","minimum":0,"maximum":1e100}},
                "candidate":{"type":"array", "minItems":1, "maxItems":256, "items":{"type":"number","minimum":0,"maximum":1e100}}
            }, "required":["dataset","metric","direction","baseline","candidate"], "additionalProperties":false}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        compare(args).and_then(|v| {
            serde_json::to_string(&v).map_err(|_| "invalid measurement result".into())
        })
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/benchmark__tests.rs"]
mod tests;
