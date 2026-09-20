//! Explicit, model-bound capability profile for a calibrated local vLLM route.
//! This never aliases the wire model or enables official-API reasoning replay.

use super::{ReasoningDialect, is_private_host};

pub(super) const LEVELS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

#[derive(Clone, Debug)]
pub(super) enum ReasoningProfile {
    Unconfigured,
    LocalV4Vllm { model: String },
    Invalid(&'static str),
}

impl ReasoningProfile {
    pub(super) fn from_env(prefix: &str, base: &str, model: Option<&str>) -> Self {
        let value = std::env::var(format!("ANGEL_{prefix}_REASONING_PROFILE")).ok();
        Self::parse(value.as_deref(), base, model)
    }

    pub(super) fn parse(value: Option<&str>, base: &str, model: Option<&str>) -> Self {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None => Self::Unconfigured,
            Some("deepseek-v4-vllm") => {
                let local_http = url::Url::parse(base).ok().is_some_and(|url| {
                    matches!(url.scheme(), "http" | "https")
                        && url.username().is_empty()
                        && url.password().is_none()
                        && is_private_host(base)
                });
                if !local_http {
                    return Self::Invalid(
                        "deepseek-v4-vllm reasoning profile requires an explicit local HTTP endpoint",
                    );
                }
                let Some(model) = model.map(str::trim).filter(|model| !model.is_empty()) else {
                    return Self::Invalid(
                        "deepseek-v4-vllm reasoning profile requires an explicitly pinned served model ID",
                    );
                };
                Self::LocalV4Vllm {
                    model: model.into(),
                }
            }
            Some(_) => Self::Invalid(
                "unknown reasoning profile; supported local profile: deepseek-v4-vllm",
            ),
        }
    }

    pub(super) fn error(&self, model: &str, dialect: ReasoningDialect) -> Option<&'static str> {
        match self {
            Self::Unconfigured => None,
            Self::Invalid(message) => Some(message),
            Self::LocalV4Vllm { model: bound } if bound != model => Some(
                "local DeepSeek reasoning profile belongs to a different served model; reconfigure this route",
            ),
            Self::LocalV4Vllm { .. } if dialect != ReasoningDialect::OpenAiEffort => Some(
                "local DeepSeek reasoning profile requires the openai effort dialect; enable_thinking is not its calibrated control",
            ),
            Self::LocalV4Vllm { .. } => None,
        }
    }

    pub(super) fn levels(&self) -> Option<&'static [&'static str]> {
        matches!(self, Self::LocalV4Vllm { .. }).then_some(LEVELS.as_slice())
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/club/http__local_deepseek__tests.rs"]
mod tests;
