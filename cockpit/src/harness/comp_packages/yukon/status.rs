//! Authenticated, exact-ID Yukon observations. No tool prose or inferred wins.
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Serialize)]
pub(crate) struct SubmissionStatus {
    pub(crate) id: String,
    pub(crate) benchmark_id: String,
    pub(crate) status: String,
    pub(crate) promotion_status: Option<String>,
    pub(crate) promoted_source_ref: Option<String>,
    pub(crate) submission_commit_sha: Option<String>,
    pub(crate) official_score: Option<String>,
    pub(crate) fetched_at_ms: u128,
}

impl SubmissionStatus {
    pub(crate) fn receipt_note(&self) -> String {
        format!(
            "authenticated Yukon exact-ID response; benchmark={}; promotion={}; promoted_source={}; fetched_at_ms={}; frontier win not established",
            self.benchmark_id,
            self.promotion_status.as_deref().unwrap_or("not reported"),
            self.promoted_source_ref
                .as_deref()
                .unwrap_or("not reported"),
            self.fetched_at_ms,
        )
    }
}

pub(crate) fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}

/// Same credential precedence as the installed official Yukon client. Secrets
/// stay in memory, never in returned errors, command arguments or receipts.
fn credentials() -> Result<(String, String), String> {
    let mut config = Value::Null;
    if let Some(home) = std::env::var_os("HOME") {
        for name in ["yukon", "hilbert"] {
            let path = std::path::PathBuf::from(&home)
                .join(".config")
                .join(name)
                .join("config.json");
            let Ok(file) = std::fs::File::open(path) else {
                continue;
            };
            let mut bytes = Vec::new();
            if file.take(65_537).read_to_end(&mut bytes).is_ok()
                && bytes.len() <= 65_536
                && let Ok(value) = serde_json::from_slice::<Value>(&bytes)
                && value.is_object()
            {
                config = value;
                break;
            }
        }
    }
    let env = |names: &[&str]| names.iter().find_map(|name| std::env::var(name).ok());
    let base = env(&["YUKON_API_URL", "HILBERT_API_URL"])
        .or_else(|| config["apiBaseUrl"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "https://yukon-api-dev.fly.dev".into());
    let url = url::Url::parse(&base).map_err(|_| "invalid Yukon API URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Yukon status requires an HTTPS API URL without credentials/query/fragment".into(),
        );
    }
    let token = env(&[
        "YUKON_API_TOKEN",
        "HILBERT_API_TOKEN",
        "SUPABASE_ACCESS_TOKEN",
    ])
    .or_else(|| config["token"].as_str().map(str::to_owned))
    .filter(|value| !value.trim().is_empty())
    .ok_or("Yukon is not logged in")?;
    Ok((base.trim_end_matches('/').into(), token))
}

pub(crate) fn fetch(benchmark: &str, id: &str) -> Result<SubmissionStatus, String> {
    if !valid_uuid(benchmark) || !valid_uuid(id) {
        return Err("Yukon status requires full lowercase benchmark and submission UUIDs".into());
    }
    let (base, token) = credentials()?;
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .redirects(0)
        .build();
    let response = agent
        .get(&format!("{base}/api/submissions/{id}"))
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(status, _) => format!("Yukon status HTTP {status}"),
            _ => "Yukon status transport failed".into(),
        })?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Yukon status response read failed")?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err("Yukon status response exceeded 1 MiB".into());
    }
    let body: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid Yukon status JSON")?;
    let row = body
        .get("submission")
        .ok_or("missing Yukon submission object")?;
    if row["id"].as_str() != Some(id) || row["benchmarkId"].as_str() != Some(benchmark) {
        return Err("Yukon response submission/benchmark identity mismatch".into());
    }
    let status = bounded_label(&row["status"]).ok_or("invalid Yukon submission status")?;
    let promotion_status = optional_label(&row["promotionStatus"])?;
    let promoted_source_ref = optional_commit(&row["promotedSourceRef"])?;
    let submission_commit_sha = optional_commit(&row["submissionCommitSha"])?;
    // Platform acceptance and source promotion remain independent facts.
    if promotion_status.as_deref() == Some("promoted")
        && (status != "accepted" || promoted_source_ref.is_none())
    {
        return Err("inconsistent Yukon promotion receipt".into());
    }
    let official_score = match &row["officialScore"] {
        Value::Null => None,
        Value::Number(number) => Some(number.to_string()),
        Value::String(value)
            if value.len() <= 128
                && !value.is_empty()
                && value.parse::<f64>().is_ok_and(f64::is_finite) =>
        {
            Some(value.clone())
        }
        _ => return Err("invalid Yukon official score".into()),
    };
    Ok(SubmissionStatus {
        id: id.into(),
        benchmark_id: benchmark.into(),
        status,
        promotion_status,
        promoted_source_ref,
        submission_commit_sha,
        official_score,
        fetched_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    })
}

fn bounded_label(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || matches!(b, b'_' | b'-'))
        })
        .map(str::to_owned)
}

fn optional_label(value: &Value) -> Result<Option<String>, String> {
    if value.is_null() {
        Ok(None)
    } else {
        bounded_label(value)
            .map(Some)
            .ok_or_else(|| "invalid Yukon promotion status".into())
    }
}

fn optional_commit(value: &Value) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .filter(|s| {
            matches!(s.len(), 40 | 64)
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        .map(|s| Some(s.to_owned()))
        .ok_or_else(|| "invalid Yukon source reference".into())
}

pub(crate) fn run_cli(benchmark: &str, id: &str) -> Result<(), String> {
    let receipt = fetch(benchmark, id)?;
    println!(
        "{}",
        serde_json::json!({
            "schema":"angel.yukon-status/v1", "source":"authenticated-yukon-api",
            "submission":receipt, "frontier_win":null,
        })
    );
    Ok(())
}
