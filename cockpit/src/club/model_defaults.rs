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
mod tests {
    use super::*;
    #[test]
    fn l01_calibration_cannot_install_an_idle_stop() {
        let _guard = crate::tests::env_lock();
        let _unset = crate::tests::TestEnvGuard::unset("ANGEL_TURN_IDLE_TIMEOUT_SECS");
        for model in ["grok-4.6", "unknown"] {
            assert_eq!(budgets(model, "GROK")["turn_idle_timeout_secs"], 0);
        }
        let _cap = crate::tests::TestEnvGuard::set("ANGEL_TURN_IDLE_TIMEOUT_SECS", "17");
        assert_eq!(budgets("grok-4.6", "GROK")["turn_idle_timeout_secs"], 17);
    }

    #[test]
    fn model_defaults_precedence_and_unknown() {
        assert_eq!(
            resolve(Some(0), Some(240), 45, "2026-09-08"),
            (0, "env".into())
        );
        assert_eq!(
            resolve(None, Some(240), 45, "2026-09-08"),
            (240, "table 2026-09-08".into())
        );
        assert_eq!(resolve(None, None::<u64>, 45, ""), (45, "provider".into()));
        assert!(
            !parse(EMBEDDED)
                .unwrap()
                .iter()
                .any(|e| e.model == "grok-4.7")
        );
    }
    #[test]
    fn model_defaults_toml_override() {
        let _guard = crate::tests::env_lock();
        let path = std::env::current_dir()
            .unwrap()
            .join(format!(".model-defaults-{}.toml", std::process::id()));
        std::fs::write(&path, "date='2026-09-09'\n[[models]]\nmodel='grok-4.6'\ndefault_effort='high'\nreceipt='test'\nreason='test'\n").unwrap();
        let entries = load(Some(&path)).unwrap();
        std::fs::remove_file(&path).unwrap();
        let grok = entries.iter().find(|e| e.model == "grok-4.6").unwrap();
        assert_eq!(grok.default_effort.as_deref(), Some("high"));
        assert_eq!(grok.date, "2026-09-09");
        assert!(entries.iter().any(|e| e.model == "deepseek-v4-flash"));
        assert!(parse("[[models]]\nmodel='oops'\n").is_err());
    }
    #[test]
    fn model_defaults_http_wire_and_explicit_precedence() {
        let _guard = crate::tests::env_lock();
        struct ResetEffortCache;
        impl Drop for ResetEffortCache {
            fn drop(&mut self) {
                super::super::http::resync_reasoning_effort_env_from_env();
            }
        }
        let _reset = ResetEffortCache;
        let _global = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
        let _driver = crate::tests::TestEnvGuard::unset("ANGEL_GROK_REASONING_EFFORT");
        super::super::http::resync_reasoning_effort_env_from_env();
        let club = super::super::HttpClub::new("grok", "http://127.0.0.1:1/v1", "grok-4.6", None);
        let body = club.build_body(&[], &[], false).unwrap();
        assert_eq!(body["reasoning_effort"], "low");
        use super::super::Club;
        assert_eq!(
            club.resolved_model_defaults()["reasoning_effort_source"],
            "table 2026-09-09"
        );
        let body = club
            .build_body_with_effort(&[], &[], false, Some("high"))
            .unwrap();
        assert_eq!(body["reasoning_effort"], "high");
        let _explicit = crate::tests::TestEnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "high");
        super::super::http::resync_reasoning_effort_env_from_env();
        assert_eq!(
            club.build_body(&[], &[], false).unwrap()["reasoning_effort"],
            "high"
        );
    }

    #[test]
    fn model_defaults_budget_recording() {
        let _guard = crate::tests::env_lock();
        let _stall = crate::tests::TestEnvGuard::set("ANGEL_STREAM_STALL_SECS", "0");
        let _global = crate::tests::TestEnvGuard::set("ANGEL_REASONING_EFFORT", "high");
        let _driver = crate::tests::TestEnvGuard::set("ANGEL_GROK_REASONING_EFFORT", "max");
        let b = budgets("grok-4.6", "ANGEL_GROK");
        assert_eq!(b["stream_stall_secs"], 0);
        assert_eq!(b["stream_stall_source"], "env");
        assert_eq!(b["reasoning_effort"], "max");
        assert_eq!(b["reasoning_effort_source"], "env");
    }
}
