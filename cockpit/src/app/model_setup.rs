//! Read-only onboarding guidance for model connections.

const USAGE: &str = "usage: /connect [grok|openai|glm|deepseek|openrouter|local]";

fn policy_text() -> String {
    let api = match std::env::var("ANGEL_API_CLUBS") {
        Ok(value) if value.trim().is_empty() => "empty (none)".to_string(),
        Ok(value) if value.trim().eq_ignore_ascii_case("none") => "none".to_string(),
        Ok(value) => format!("{value} (allowlist)"),
        Err(_) => "unset (all configured API families)".to_string(),
    };
    let consult = match std::env::var("ANGEL_ALLOW_SOTA_CONSULT") {
        Ok(value) if value.trim().eq_ignore_ascii_case("0") => "0 (disabled)".to_string(),
        Ok(value) => format!("{value} (enabled when truthy)"),
        Err(_) => "unset (enabled)".to_string(),
    };
    let solo = if crate::agent::tools::solo::solo_mode_active() {
        "on (consults restricted)"
    } else {
        "off"
    };
    format!("policy: ANGEL_API_CLUBS={api}; ANGEL_ALLOW_SOTA_CONSULT={consult}; /solo={solo}")
}

pub(crate) fn connect_text(provider: Option<&str>) -> String {
    let Some(provider) = provider.map(str::trim).filter(|value| !value.is_empty()) else {
        return format!(
            "{USAGE}\n\
             Setup instructions; run /model after configuring and restarting.\n\
             providers: grok · openai · glm · deepseek · openrouter · local\n\
             {}\n\
             after setup: /model · /think · /status",
            policy_text()
        );
    };

    let key = provider.to_ascii_lowercase();
    let body = match key.as_str() {
        "grok" => {
            "Grok account OAuth\n\
            1. Install the Grok CLI and run: grok login --oauth\n\
            2. Default credential store: ~/.grok/auth.json\n\
            3. Optional paths: ANGEL_GROK_OAUTH_FILE or ANGEL_GROK_HOME/GROK_HOME\n\
            4. Optional model: ANGEL_GROK_MODEL or GROK_MODEL (default: grok-4.6)\n\
            API-key alternative: set XAI_API_KEY or ANGEL_GROK_KEY for the separate\n\
            `grok-api` route, with ANGEL_GROK_API_URL and ANGEL_GROK_API_MODEL/GROK_API_MODEL\n\
            as needed (API model selection is separate from OAuth model selection).\n\
            OAuth and API routes are separate; this command does not test readiness."
        }
        "openai" => {
            "OpenAI/ChatGPT account OAuth\n\
            1. Sign in with the Codex CLI: codex login\n\
            2. Default credential store: ~/.codex/auth.json\n\
            3. Override the store with CODEX_HOME; choose a model with ANGEL_OPENAI_MODEL\n\
            API-key alternative: set ANGEL_OPENAI_KEY/OPENAI_API_KEY and an explicit\n\
            ANGEL_OPENAI_API_MODEL/OPENAI_MODEL for the separate `openai-api` route.\n\
            The cockpit reuses the Codex login. Use /model after setup to choose a catalog\n\
            entry and /think to choose its supported reasoning level."
        }
        "glm" => {
            "GLM API connection\n\
            Configure ANGEL_GLM_URL and ANGEL_GLM_MODEL when the defaults do not fit.\n\
            Credentials use ANGEL_GLM_KEY, GLM_API_KEY, ZHIPU_API_KEY, BIGMODEL_API_KEY,\n\
            or the URL-specific ZAI_API_KEY. Values are read by the provider adapter and\n\
            are never displayed by /connect."
        }
        "deepseek" => {
            "DeepSeek API connection\n\
            Optional endpoint: ANGEL_DEEPSEEK_URL (default: https://api.deepseek.com/v1).\n\
            Optional model: ANGEL_DEEPSEEK_MODEL or DEEPSEEK_MODEL.\n\
            Credential keys: ANGEL_DEEPSEEK_KEY or DEEPSEEK_API_KEY. Set these outside the\n\
            composer; /connect only prints the names of supported settings."
        }
        "openrouter" => {
            "OpenRouter API connection\n\
            Set ANGEL_OPENROUTER_URL/OPENROUTER_BASE_URL, ANGEL_OPENROUTER_MODEL/OPENROUTER_MODEL,\n\
            and ANGEL_OPENROUTER_KEY/OPENROUTER_API_KEY. OpenRouter requires an explicit\n\
            model and credential before its catalog is offered. /connect does not test it."
        }
        "local" => {
            "Local model connection\n\
            Configure ANGEL_LOCAL_URL and ANGEL_LOCAL_MODEL, or a named endpoint such as\n\
            ANGEL_SPARK_URL, ANGEL_GEMMA_URL, or ANGEL_TURBO_URL. ANGEL_SCAN=1\n\
            enables opt-in fleet discovery; local endpoints may not require credentials.\n\
            Use /model to select a discovered route after it is configured."
        }
        _ => return format!("unknown provider {provider:?}\n{USAGE}"),
    };
    format!("{body}\n\nnext: /model · /think · /status")
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/model_setup__tests.rs"]
mod tests;
