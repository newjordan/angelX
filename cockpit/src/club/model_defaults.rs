//! Measured defaults only. Exact model IDs avoid applying measurements to a new revision.
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::OnceLock;

const EMBEDDED: &str = include_str!("../../../docs/telemetry/model-calibration.toml");

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Entry {
    pub model: String,
    pub default_effort: Option<String>,
    pub stream_stall_secs: Option<u64>,
    #[serde(rename = "turn_idle_secs")]
    pub _turn_idle_secs: Option<u64>,
    #[serde(default)]
    pub date: String,
    pub receipt: String,
    pub liveness_receipt: Option<String>,
    pub reason: String,
}

#[derive(Deserialize)]
struct Table {
    #[serde(default)]
    date: String,
    #[serde(default)]
    models: Vec<Entry>,
}

fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let table: Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut entries = table.models;
    for entry in &mut entries {
        if entry.date.is_empty() {
            entry.date.clone_from(&table.date);
        }
        if entry.date.is_empty() || entry.receipt.is_empty() || entry.reason.is_empty() {
            return Err("calibration requires date, receipt and reason".into());
        }
        if entry.default_effort.as_deref().is_some_and(|v| {
            !matches!(
                v,
                "auto" | "none" | "low" | "medium" | "high" | "max" | "xhigh" | "ultra"
            )
        }) {
            return Err("invalid calibration effort".into());
        }
    }
    Ok(entries)
}

fn load(path: Option<&Path>) -> Result<Vec<Entry>, String> {
    let mut entries = parse(EMBEDDED)?;
    if let Some(path) = path {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                for entry in parse(&text)? {
                    entries.retain(|old| !old.model.eq_ignore_ascii_case(&entry.model));
                    entries.push(entry);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(entries)
}

pub(crate) fn entry(model: &str) -> Option<Entry> {
    static TABLE: OnceLock<Vec<Entry>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let path = std::env::var_os("HOME")
                .map(|p| Path::new(&p).join(".angel0/model-calibration.toml"));
            load(path.as_deref()).unwrap_or_else(|_| {
                eprintln!("[angel] invalid runtime model calibration; using embedded defaults");
                parse(EMBEDDED).expect("valid embedded model calibration")
            })
        })
        .iter()
        .find(|e| e.model.eq_ignore_ascii_case(model))
        .cloned()
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn resolve<T>(explicit: Option<T>, table: Option<T>, provider: T, date: &str) -> (T, String) {
    if let Some(value) = explicit {
        (value, "env".into())
    } else if let Some(value) = table {
        (value, format!("table {date}"))
    } else {
        (provider, "provider".into())
    }
}

pub(crate) fn budgets(model: &str, namespace: &str) -> Value {
    let entry = entry(model);
    let date = entry.as_ref().map_or("", |e| e.date.as_str());
    let prefix = if namespace.starts_with("ANGEL_") {
        namespace.to_string()
    } else {
        format!("ANGEL_{}", namespace.to_ascii_uppercase())
    };
    let (effort, effort_source) = resolve(
        env(&format!("{prefix}_REASONING_EFFORT")).or_else(|| env("ANGEL_REASONING_EFFORT")),
        entry.as_ref().and_then(|e| e.default_effort.clone()),
        "auto".into(),
        date,
    );
    let (stall, stall_source) = resolve(
        env("ANGEL_STREAM_STALL_SECS").and_then(|s| s.parse::<u64>().ok()),
        entry.as_ref().and_then(|e| e.stream_stall_secs),
        45,
        date,
    );
    let (idle, idle_source) = resolve(
        env("ANGEL_TURN_IDLE_TIMEOUT_SECS").and_then(|s| s.parse::<u64>().ok()),
        None,
        0,
        date,
    );
    json!({"reasoning_effort": effort, "reasoning_effort_source": effort_source,
        "stream_stall_secs": stall, "stream_stall_source": stall_source,
        "turn_idle_timeout_secs": idle, "turn_idle_timeout_source": idle_source,
        "calibration_receipt": entry.as_ref().map(|e| &e.receipt),
        "calibration_liveness_receipt": entry.as_ref().and_then(|e| e.liveness_receipt.as_ref()),
        "calibration_notes": entry.as_ref().map(|e| &e.reason)})
}

pub(crate) fn summary(club: &dyn super::Club) -> String {
    let b = club.resolved_model_defaults();
    format!(
        "effort={}[{}] stream-stall={}s[{}] turn-idle={}s[{}]",
        club.reasoning_effort().unwrap_or_else(|| "auto".into()),
        b["reasoning_effort_source"].as_str().unwrap_or("provider"),
        b["stream_stall_secs"],
        b["stream_stall_source"].as_str().unwrap_or("provider"),
        b["turn_idle_timeout_secs"],
        b["turn_idle_timeout_source"].as_str().unwrap_or("provider")
    )
}

#[cfg(test)]
#[path = "../../../tests/cockpit/club/model_defaults__tests.rs"]
mod tests;
