//! Controls for the explicit OpenAI API-key Responses seat. OAuth and other
//! Responses providers retain their existing endpoint-managed behavior.
use crate::agent::club::{OutputBudgetPolicy, OutputBudgetSource};
use crate::agent::harness::formation_budget::{self, Reservation};

pub(super) struct ApiControls {
    pub(super) error: Option<String>,
}

impl ApiControls {
    pub(super) fn effort(&self, model: &str) -> (Option<String>, &'static str) {
        let explicit = crate::agent::club::reasoning_env_var("ANGEL_OPENAI_API_REASONING_EFFORT")
            .or_else(|| crate::agent::club::reasoning_env_var("ANGEL_REASONING_EFFORT"));
        let (value, source) = if let Some(value) = explicit {
            (Some(value), "env")
        } else if let Some(value) =
            crate::agent::club::model_defaults::entry(model).and_then(|entry| entry.default_effort)
        {
            (Some(value), "table")
        } else {
            (None, "provider")
        };
        (
            value
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty() && value != "auto"),
            source,
        )
    }

    pub(super) fn output_budget(&self) -> (OutputBudgetPolicy, Option<String>) {
        for (key, source) in [
            (
                "ANGEL_OPENAI_API_MAX_TOKENS",
                OutputBudgetSource::PerClubEnv,
            ),
            ("ANGEL_CLUB_MAX_TOKENS", OutputBudgetSource::GlobalEnv),
        ] {
            if let Some(tokens) = crate::agent::club::max_tokens_env(key) {
                return (
                    OutputBudgetPolicy::Explicit { tokens, source },
                    Some(key.into()),
                );
            }
        }
        (OutputBudgetPolicy::ProviderNative, None)
    }
}

/// Same conservative output fitting and reservation contract as HttpClub,
/// using Responses' max_output_tokens field. The Attempt owns settlement.
pub(super) fn fit_and_reserve(
    mut bytes: Vec<u8>,
    route: &str,
) -> Result<(Vec<u8>, Option<Reservation>), String> {
    let Some(budget) = formation_budget::current() else {
        return Ok((bytes, None));
    };
    let mut body: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("formation request is not JSON: {e}"))?;
    let mut requested = body["max_output_tokens"].as_u64().unwrap_or(0);
    if let Some(available) = budget.available_tokens() {
        requested = body["max_output_tokens"].as_u64().unwrap_or(1024);
        let room = available.saturating_sub((bytes.len() as u64).saturating_add(320));
        let share = budget.share_output(requested).unwrap_or(requested);
        body["max_output_tokens"] = serde_json::json!(
            requested
                .min(room.max(formation_budget::SHARE_FLOOR))
                .min(share.max(1))
                .max(1)
        );
        bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    }
    let output = body["max_output_tokens"].as_u64().unwrap_or(0);
    let reservation = budget.reserve_fitted(
        route,
        (bytes.len() as u64).saturating_add(256),
        output,
        requested,
    )?;
    Ok((bytes, Some(reservation)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_and_explicit_effort_report_their_source() {
        let _guard = crate::tests::env_lock();
        let _global = crate::tests::TestEnvGuard::unset("ANGEL_REASONING_EFFORT");
        let _api = crate::tests::TestEnvGuard::unset("ANGEL_OPENAI_API_REASONING_EFFORT");
        crate::agent::club::resync_reasoning_effort_env_from_env();
        let controls = ApiControls { error: None };
        assert_eq!(controls.effort("grok-4.6").1, "table");
        {
            let _explicit =
                crate::tests::TestEnvGuard::set("ANGEL_OPENAI_API_REASONING_EFFORT", "high");
            crate::agent::club::resync_reasoning_effort_env_from_env();
            assert_eq!(controls.effort("grok-4.6"), (Some("high".into()), "env"));
        }
        crate::agent::club::resync_reasoning_effort_env_from_env();
    }
}
