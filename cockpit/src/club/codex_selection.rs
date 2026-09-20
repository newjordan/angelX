//! ChatGPT OAuth selection: explicit env/CLI pins > Codex config > built-in
//! fallback. `ANGEL_OPENAI_MODEL` pins the model; `ANGEL_OPENAI_REASONING_EFFORT`
//! pins effort before the generic `ANGEL_REASONING_EFFORT` alias. Blank pins
//! are absent. Config uses `model` and `model_reasoning_effort` from CODEX_HOME.
//! Cache validation never changes a requested model or effort. Missing cache
//! refuses env/config pins; only the wholly built-in fallback can proceed
//! without a cached support claim. Legacy `ultra` is normalized to wire `xhigh`.
//! Source `env` also denotes explicit route controls after construction.
use crate::openai_codex::{CodexModelInfo, OPENAI_LUNA_EFFORT, OPENAI_LUNA_MODEL};

#[derive(Clone, Debug)]
pub(crate) struct Selection {
    pub model: String,
    pub effort: String,
    pub model_source: &'static str,
    pub effort_source: &'static str,
    pub error: Option<String>,
}

fn pick(env: Option<String>, config: Option<String>, fallback: &str) -> (String, &'static str) {
    let clean = |s: String| (!s.trim().is_empty()).then(|| s.trim().to_string());
    if let Some(value) = env.and_then(clean) {
        (value, "env")
    } else if let Some(value) = config.and_then(clean) {
        (value, "codex-config")
    } else {
        (fallback.into(), "fallback")
    }
}

pub(crate) fn resolve(
    model_env: Option<String>,
    effort_env: Option<String>,
    model_config: Option<String>,
    effort_config: Option<String>,
    catalog: &[CodexModelInfo],
) -> Selection {
    let (model, model_source) = pick(model_env, model_config, OPENAI_LUNA_MODEL);
    let (effort, effort_source) = pick(effort_env, effort_config, OPENAI_LUNA_EFFORT);
    let effort = effort.to_ascii_lowercase();
    // Preserve the established legacy spelling at the input boundary only.
    let effort = if effort == "ultra" {
        "xhigh".into()
    } else {
        effort
    };
    let spec = catalog.iter().find(|candidate| candidate.slug == model);
    let error = if spec.is_none() && (model_source != "fallback" || !catalog.is_empty()) {
        Some(format!(
            "OpenAI model {model:?} ({model_source}) is not supported by models_cache.json; supported models: [{}]",
            catalog
                .iter()
                .map(|m| m.slug.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    } else if let Some(spec) = spec {
        (!spec.supported_reasoning_levels.iter().any(|level| level.effort == effort || (level.effort == "ultra" && effort == "xhigh"))).then(|| format!(
            "OpenAI effort {effort:?} ({effort_source}) is not supported for {model}; supported efforts: [{}]",
            spec.supported_reasoning_levels.iter().map(|l| l.effort.as_str()).collect::<Vec<_>>().join(", ")
        ))
    } else if effort_source != "fallback" {
        Some(format!(
            "OpenAI effort {effort:?} ({effort_source}) cannot be validated without models_cache.json; supported efforts: []"
        ))
    } else {
        None
    };
    Selection {
        model,
        effort,
        model_source,
        effort_source,
        error,
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/club/codex_selection__tests.rs"]
mod tests;
