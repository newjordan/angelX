//! Tailnet/URL endpoint resolution and fleet discovery / smartness ranking.

use super::*;

pub(crate) struct TailHost {
    pub(crate) ip: String,
    pub(crate) online: bool,
}

pub(crate) fn tailnet_label(dns_name: &str) -> Option<String> {
    let label = dns_name.split('.').next().unwrap_or("").trim();
    (!label.is_empty()).then(|| label.to_lowercase())
}

pub(crate) fn insert_tail_node(
    map: &mut std::collections::HashMap<String, TailHost>,
    node: &serde_json::Value,
) {
    let Some(dns) = node.get("DNSName").and_then(|d| d.as_str()) else {
        return;
    };
    let Some(label) = tailnet_label(dns) else {
        return;
    };
    let ip = node
        .get("TailscaleIPs")
        .and_then(|a| a.as_array())
        .and_then(|a| {
            a.iter()
                .find_map(|s| s.as_str().filter(|s| s.contains('.')))
        });
    let Some(ip) = ip else {
        return;
    };
    let online = node.get("Online").and_then(|o| o.as_bool()).unwrap_or(true);
    map.insert(
        label,
        TailHost {
            ip: ip.to_string(),
            online,
        },
    );
}

pub(crate) fn parse_tailnet(v: &serde_json::Value) -> std::collections::HashMap<String, TailHost> {
    let mut map = std::collections::HashMap::new();
    if let Some(s) = v.get("Self") {
        insert_tail_node(&mut map, s);
    }
    if let Some(peers) = v.get("Peer").and_then(|p| p.as_object()) {
        for node in peers.values() {
            insert_tail_node(&mut map, node);
        }
    }
    map
}

pub(crate) fn live_tailnet_hosts() -> std::collections::HashMap<String, TailHost> {
    let mut command = std::process::Command::new("tailscale");
    command.args(["status", "--json"]);
    tailnet_hosts_from_command(command, Duration::from_secs(3))
}

pub(crate) fn tailnet_hosts_from_command(
    command: std::process::Command,
    timeout: Duration,
) -> std::collections::HashMap<String, TailHost> {
    crate::platform::workspace_store::capture_bounded_command(command, timeout)
        .and_then(|out| serde_json::from_slice::<serde_json::Value>(&out).ok())
        .map(|v| parse_tailnet(&v))
        .unwrap_or_default()
}

pub(crate) fn tailnet_hosts() -> std::collections::HashMap<String, TailHost> {
    if tailnet_resolution_enabled() {
        live_tailnet_hosts()
    } else {
        std::collections::HashMap::new()
    }
}

pub(crate) fn canonical_fleet_box(_host: &str) -> String {
    "local".to_string()
}

pub(crate) fn tailnet_lookup<'a>(
    host: &str,
    tailnet: &'a std::collections::HashMap<String, TailHost>,
) -> Option<&'a TailHost> {
    tailnet.get(&host.trim().to_ascii_lowercase())
}

pub(crate) fn url_from(
    host: &str,
    port: u16,
    fallback_ip: &str,
    tailnet: &std::collections::HashMap<String, TailHost>,
) -> String {
    let ip = tailnet_lookup(host, tailnet)
        .map(|h| h.ip.as_str())
        .unwrap_or(fallback_ip);
    format!("http://{ip}:{port}/v1")
}

pub(crate) fn resolve_club_url(
    label: &str,
    host: &str,
    port: u16,
    fallback_ip: &str,
    tailnet: &std::collections::HashMap<String, TailHost>,
) -> String {
    let up = label.to_uppercase().replace('-', "_");
    if let Ok(u) = std::env::var(format!("ANGEL_{up}_URL"))
        && !u.trim().is_empty()
    {
        return u;
    }
    url_from(host, port, fallback_ip, tailnet)
}

pub(crate) fn host_online(
    host: &str,
    tailnet: &std::collections::HashMap<String, TailHost>,
) -> bool {
    if host.is_empty() || tailnet.is_empty() {
        return true;
    }
    tailnet_lookup(host, tailnet)
        .map(|h| h.online)
        .unwrap_or(false)
}

pub(crate) fn env_first(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

pub(crate) const DEEPSEEK_API_MODEL_OPTIONS: &[&str] = &["deepseek-v4-pro", "deepseek-flash"];

pub(crate) const GLM_API_MODEL_OPTIONS: &[&str] = &["glm-5.3", "glm-5.3-flash", "glm-5.2"];

pub(crate) const KIMI_API_MODEL_OPTIONS: &[&str] = &["kimi-k3", "kimi-k3.1"];

pub(crate) const KIMI_CODE_MODEL_OPTIONS: &[&str] = &["k3", "k3.1-pro", "k3.1-pro-highspeed"];

pub(crate) fn is_kimi_code_url(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    (url.contains("kimi.ai") || url.contains("kimi.com")) && url.contains("/coding")
}

pub(crate) const OPENROUTER_API_MODEL_OPTIONS: &[&str] = &[
    "stealth/union-alpha",
    "stealth/ox-alpha",
    "thinkingmachines/inkling:free",
    "thinkingmachines/inkling-small:free",
    "nvidia/nemotron-3-ultra-550b-a55b:free",
    "openrouter/free",
    "poolside/laguna-s-2.1:free",
    "cohere/north-mini-code:free",
    "z-ai/glm-5.2:free",
    "nvidia/nemotron-3.5-lightning:free",
];

pub(crate) fn resolve_deepseek_model_alias(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "deepseek-chat"
        | "deepseek-reasoner"
        | "deepseek-v4-flash"
        | "deepseek-v4-flash-vision-exp" => DEEPSEEK_API_MODEL_OPTIONS[1].to_string(),
        "deepseek-v4-pro" => DEEPSEEK_API_MODEL_OPTIONS[0].to_string(),
        "deepseek-flash" => DEEPSEEK_API_MODEL_OPTIONS[1].to_string(),
        _ => trimmed.to_string(),
    }
}

pub(crate) fn resolve_glm_model_alias(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "glm-flash" | "glmflash" | "glm-5.3-flash" => "glm-5.3-flash".to_string(),
        _ => trimmed.to_string(),
    }
}

pub(crate) fn resolve_kimi_model_alias(raw: &str) -> String {
    resolve_kimi_model_alias_for(raw, false)
}

pub(crate) fn resolve_kimi_model_alias_for(raw: &str, kimi_code: bool) -> String {
    let trimmed = raw.trim();
    if kimi_code {
        return match trimmed.to_ascii_lowercase().as_str() {
            "k3.1" | "kimi-3.1" | "kimi3.1" | "kimi-k3.1" | "k3.1-pro" => "k3.1-pro".to_string(),
            "k3.1-highspeed" | "kimi-k3.1-highspeed" | "k3.1-pro-highspeed" => {
                "k3.1-pro-highspeed".to_string()
            }
            "kimi-k3" | "k3" => "k3".to_string(),
            _ => trimmed.to_string(),
        };
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "k3.1" | "kimi-3.1" | "kimi3.1" | "kimi-k3.1" => "kimi-k3.1".to_string(),
        _ => trimmed.to_string(),
    }
}

pub(crate) fn resolve_openrouter_model_alias(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "union-alpha" | "union_alpha" | "union alpha" | "union" | "stealth/union-alpha" => {
            "stealth/union-alpha".to_string()
        }
        "ox-alpha" | "ox_alpha" | "ox alpha" | "ox" | "stealth/ox-alpha" => {
            "stealth/ox-alpha".to_string()
        }
        "tencent/hy3:free" | "tencent/hy3-preview:free" => {
            OPENROUTER_API_MODEL_OPTIONS[0].to_string()
        }
        _ => trimmed.to_string(),
    }
}

pub(crate) fn api_club_enabled(alias: &str) -> bool {
    let family = if alias.starts_with("glm-") {
        "glm"
    } else if alias.starts_with("kimi-") || alias == "k3" || alias.starts_with("k3-") || alias.starts_with("k3.") {
        "kimi"
    } else if alias == "deepseek-flash" {
        "deepseek"
    } else if alias.starts_with("claude-") || alias == "opus" {
        "anthropic"
    } else if alias == "grok-api" {
        "grok"
    } else if alias == "openai-api" {
        "openai"
    } else {
        alias
    };
    let Ok(list) = std::env::var("ANGEL_API_CLUBS") else {
        return true;
    };
    let list = list.trim();
    if list.is_empty() || list.eq_ignore_ascii_case("none") {
        return false;
    }
    if list == "*" || list.eq_ignore_ascii_case("all") {
        return true;
    }
    list.split(',').any(|entry| {
        let entry = entry.trim();
        entry == "*" || entry.eq_ignore_ascii_case("all") || entry.eq_ignore_ascii_case(family)
    })
}

pub(crate) fn optional_sota_http_club(
    alias: &str,
    label: &str,
    url_keys: &[&str],
    default_url: &str,
    model_keys: &[&str],
    default_model: Option<&str>,
    key_keys: &[&str],
) -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    if !api_club_enabled(alias) {
        return None;
    }
    let key = env_first(key_keys)?;
    let model = env_first(model_keys).or_else(|| default_model.map(str::to_string))?;
    let deepseek_seat = default_url.trim_end_matches('/') == "https://api.deepseek.com/v1";
    let model = if deepseek_seat {
        resolve_deepseek_model_alias(&model)
    } else if default_url.contains("openrouter.ai") {
        resolve_openrouter_model_alias(&model)
    } else if default_url.contains("z.ai") || default_url.contains("bigmodel.cn") {
        resolve_glm_model_alias(&model)
    } else if default_url.contains("moonshot.ai") {
        let actual = env_first(url_keys).unwrap_or_else(|| default_url.to_string());
        resolve_kimi_model_alias_for(&model, is_kimi_code_url(&actual))
    } else {
        model
    };
    let url = env_first(url_keys).unwrap_or_else(|| default_url.to_string());
    let label = if deepseek_seat
        || default_url.contains("openrouter.ai")
        || default_url.contains("z.ai")
        || default_url.contains("bigmodel.cn")
        || default_url.contains("api.meta.ai")
        || default_url.contains("moonshot.ai")
    {
        model.clone()
    } else {
        label.to_string()
    };
    let mut http = HttpClub::new(label, url, model, Some(key));
    if deepseek_seat {
        http = http.with_provider_contract(ProviderContract::DeepSeek);
        if std::env::var("ANGEL_DEEPSEEK_API")
            .is_ok_and(|api| api.trim().eq_ignore_ascii_case("messages"))
        {
            http = http.with_deepseek_messages();
        }
    } else if default_url.contains("z.ai") || default_url.contains("bigmodel.cn") {
        http = http.with_env_namespace("GLM");
    } else if alias == "openrouter" {
        http = http.with_env_namespace("OPENROUTER");
    }
    let club: Arc<dyn Club> = Arc::new(http.sota_tuned());
    Some((alias.to_string(), club, Arc::new(AtomicBool::new(true))))
}

pub(crate) const ANTHROPIC_OPUS_55_MODEL: &str = "claude-opus-5-5";

const ANTHROPIC_DEFAULT_MODEL: &str = "claude-sonnet-5-5";

fn anthropic_messages_club(
    alias: &str,
    model: String,
) -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    if !api_club_enabled("anthropic") {
        return None;
    }
    let key = env_first(&["ANGEL_ANTHROPIC_KEY", "ANTHROPIC_API_KEY"])?;
    let url = env_first(&["ANGEL_ANTHROPIC_URL"])
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    let http = HttpClub::new(model.clone(), url, model, Some(key))
        .with_env_namespace("ANTHROPIC")
        .with_provider_contract(ProviderContract::Anthropic)
        .with_anthropic_messages(env_first(&["ANGEL_ANTHROPIC_WORKSPACE_ID"]));
    Some((
        alias.to_string(),
        Arc::new(http.sota_tuned()),
        Arc::new(AtomicBool::new(true)),
    ))
}

pub(crate) fn optional_anthropic_http_club() -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let model = env_first(&["ANGEL_ANTHROPIC_MODEL"])
        .unwrap_or_else(|| ANTHROPIC_DEFAULT_MODEL.to_string());
    anthropic_messages_club("anthropic", model)
}

pub(crate) fn optional_anthropic_http_clubs() -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let Some(primary) = optional_anthropic_http_club() else {
        return Vec::new();
    };
    let configured = primary.1.model_identity();
    let mut links = vec![primary];
    if configured
        .as_deref()
        .is_some_and(|model| model.eq_ignore_ascii_case(ANTHROPIC_OPUS_55_MODEL))
    {
        return links;
    }
    if let Some(opus) =
        anthropic_messages_club(ANTHROPIC_OPUS_55_MODEL, ANTHROPIC_OPUS_55_MODEL.to_string())
    {
        links.push(opus);
    }
    links
}

pub(crate) const CARBON_SEAT: &str = "carbon";

pub(crate) fn optional_carbon_http_club() -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    if !api_club_enabled(CARBON_SEAT) {
        return None;
    }
    let key = env_first(&["ANGEL_CARBON_KEY", "GEMINI_API_KEY"])?;
    let model = env_first(&["ANGEL_CARBON_MODEL"]).unwrap_or_else(|| "gemini-4-carbon".to_string());
    let url = env_first(&["ANGEL_CARBON_URL"])
        .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta/openai".to_string());
    let http = HttpClub::new(CARBON_SEAT, url, model, Some(key)).with_gemini_thought_signatures();
    Some((
        CARBON_SEAT.to_string(),
        Arc::new(http.sota_tuned()),
        Arc::new(AtomicBool::new(true)),
    ))
}

pub(crate) fn optional_glm_http_clubs() -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    const URL_KEYS: &[&str] = &["ANGEL_GLM_URL", "GLM_API_URL"];
    const MODEL_KEYS: &[&str] = &["ANGEL_GLM_MODEL", "GLM_MODEL"];
    let selected_url = env_first(URL_KEYS).unwrap_or_else(|| default_glm_url().to_string());
    let key_keys = glm_key_env_order(&selected_url);

    let configured = optional_sota_http_club(
        "glm",
        GLM_API_MODEL_OPTIONS[0],
        URL_KEYS,
        default_glm_url(),
        MODEL_KEYS,
        Some(GLM_API_MODEL_OPTIONS[0]),
        key_keys,
    );
    let configured_model = configured
        .as_ref()
        .and_then(|(_, club, _)| club.model_identity());

    let mut links = Vec::with_capacity(GLM_API_MODEL_OPTIONS.len());
    if let Some(link) = configured {
        links.push(link);
    }
    for model in GLM_API_MODEL_OPTIONS {
        if configured_model
            .as_deref()
            .is_some_and(|configured| configured.eq_ignore_ascii_case(model))
        {
            continue;
        }
        if let Some(link) = optional_sota_http_club(
            model,
            model,
            URL_KEYS,
            default_glm_url(),
            &[],
            Some(model),
            key_keys,
        ) {
            links.push(link);
        }
    }
    links
}

pub(crate) fn glm_key_env_order(url: &str) -> &'static [&'static str] {
    const ZAI_KEYS: &[&str] = &[
        "ZAI_API_KEY",
        "ANGEL_GLM_KEY",
        "GLM_API_KEY",
        "ZHIPU_API_KEY",
        "BIGMODEL_API_KEY",
    ];
    const BIGMODEL_KEYS: &[&str] = &[
        "ZHIPU_API_KEY",
        "BIGMODEL_API_KEY",
        "ANGEL_GLM_KEY",
        "GLM_API_KEY",
        "ZAI_API_KEY",
    ];
    const GENERIC_KEYS: &[&str] = &[
        "ANGEL_GLM_KEY",
        "GLM_API_KEY",
        "ZAI_API_KEY",
        "ZHIPU_API_KEY",
        "BIGMODEL_API_KEY",
    ];

    let host = url.to_ascii_lowercase();
    if host.contains("api.z.ai") {
        ZAI_KEYS
    } else if host.contains("bigmodel.cn") {
        BIGMODEL_KEYS
    } else {
        GENERIC_KEYS
    }
}

pub(crate) fn optional_kimi_http_clubs() -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    const URL_KEYS: &[&str] = &["ANGEL_KIMI_URL", "KIMI_API_URL", "MOONSHOT_API_URL"];
    const MODEL_KEYS: &[&str] = &["ANGEL_KIMI_MODEL", "KIMI_MODEL", "MOONSHOT_MODEL"];
    const KEY_KEYS: &[&str] = &["ANGEL_KIMI_KEY", "KIMI_API_KEY", "MOONSHOT_API_KEY"];

    let options = if env_first(URL_KEYS).is_some_and(|url| is_kimi_code_url(&url)) {
        KIMI_CODE_MODEL_OPTIONS
    } else {
        KIMI_API_MODEL_OPTIONS
    };
    let configured = optional_sota_http_club(
        "kimi",
        "kimi-k3",
        URL_KEYS,
        "https://api.moonshot.ai/v1",
        MODEL_KEYS,
        Some(options[0]),
        KEY_KEYS,
    );
    let configured_model = configured
        .as_ref()
        .and_then(|(_, club, _)| club.model_identity());

    let mut links = Vec::with_capacity(options.len());
    if let Some(link) = configured {
        links.push(link);
    }
    for model in options {
        if configured_model
            .as_deref()
            .is_some_and(|configured| configured.eq_ignore_ascii_case(model))
        {
            continue;
        }
        if let Some(link) = optional_sota_http_club(
            model,
            model,
            URL_KEYS,
            "https://api.moonshot.ai/v1",
            &[],
            Some(model),
            KEY_KEYS,
        ) {
            links.push(link);
        }
    }
    links
}

pub(crate) fn optional_openrouter_http_clubs() -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    const URL_KEYS: &[&str] = &["ANGEL_OPENROUTER_URL", "OPENROUTER_BASE_URL"];
    const MODEL_KEYS: &[&str] = &["ANGEL_OPENROUTER_MODEL", "OPENROUTER_MODEL"];
    const KEY_KEYS: &[&str] = &["ANGEL_OPENROUTER_KEY", "OPENROUTER_API_KEY"];
    const URL: &str = "https://openrouter.ai/api/v1";

    let configured = optional_sota_http_club(
        "openrouter",
        OPENROUTER_API_MODEL_OPTIONS[0],
        URL_KEYS,
        URL,
        MODEL_KEYS,
        None,
        KEY_KEYS,
    );
    configured.into_iter().collect()
}

fn openai_gpt6_family(model: &str) -> Option<&'static str> {
    ["gpt-6-astra", "gpt-6-sol", "gpt-6-luna"]
        .into_iter()
        .find(|base| {
            model == *base
                || model
                    .strip_prefix(&format!("{base}-"))
                    .is_some_and(|suffix| {
                        suffix.len() == 10
                            && suffix.bytes().enumerate().all(|(i, ch)| {
                                if i == 4 || i == 7 {
                                    ch == b'-'
                                } else {
                                    ch.is_ascii_digit()
                                }
                            })
                    })
        })
}

pub(crate) fn openai_api_uses_responses(
    base: &str,
    model: &str,
    transport: Option<&str>,
) -> Result<bool, String> {
    match transport
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("auto")
    {
        "responses" => return Ok(true),
        "chat" => return Ok(false),
        "auto" => {}
        value => {
            return Err(format!(
                "ANGEL_OPENAI_API_TRANSPORT={value:?} is invalid; use auto, responses or chat"
            ));
        }
    }
    let official = url::Url::parse(base).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("api.openai.com")
            && url.port_or_known_default() == Some(443)
            && url.path().trim_end_matches('/') == "/v1"
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none()
    });
    Ok(official && openai_gpt6_family(model).is_some())
}

pub(crate) fn optional_openai_api_http_club() -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    optional_openai_api_for_launch(None)
}

pub(crate) fn optional_openai_api_for_launch(
    model: Option<&str>,
) -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    if !api_club_enabled("openai-api") {
        return None;
    }
    let key = env_first(&["ANGEL_OPENAI_KEY", "OPENAI_API_KEY"])?;
    let model_cli = model.is_some();
    let model = model
        .map(str::to_owned)
        .or_else(|| env_first(&["ANGEL_OPENAI_API_MODEL", "OPENAI_MODEL"]))?;
    let base = env_first(&["ANGEL_OPENAI_API_URL", "OPENAI_BASE_URL"])
        .unwrap_or_else(|| "https://api.openai.com/v1".into());
    let transport = env_first(&["ANGEL_OPENAI_API_TRANSPORT"]);
    let selection = openai_api_uses_responses(&base, &model, transport.as_deref());
    if selection == Ok(false) {
        let club = HttpClub::new("openai-api", base, model, Some(key)).sota_tuned();
        let club = if model_cli {
            club.with_cli_model()
        } else {
            club
        };
        return Some((
            "openai-api".into(),
            Arc::new(club),
            Arc::new(AtomicBool::new(true)),
        ));
    }
    let levels = if openai_gpt6_family(&model) == Some("gpt-6-astra") {
        vec!["low", "medium", "high", "xhigh", "max"]
    } else {
        vec!["none", "low", "medium", "high", "xhigh", "max"]
    };
    let club = crate::agent::openai_codex::CodexClub::api_key_seat(
        "openai-api",
        model,
        format!("{}/responses", base.trim_end_matches('/')),
        key,
        None,
        levels.into_iter().map(str::to_string).collect(),
        RouteMetadata::default(),
    )
    .with_openai_api_controls(selection.err())
    .sota_tuned();
    let club = if model_cli {
        club.with_cli_model()
    } else {
        club
    };
    Some((
        "openai-api".into(),
        Arc::new(club),
        Arc::new(AtomicBool::new(true)),
    ))
}

pub(crate) const META_REASONING_LEVELS: &[&str] = &["minimal", "low", "medium", "high", "xhigh"];

pub(crate) fn optional_meta_http_club() -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let model = env_first(&["ANGEL_META_MODEL"]).unwrap_or_else(|| "muse-spark-1.3".to_string());
    meta_http_club("meta", model, &["ANGEL_META_MODEL"])
}

pub(crate) const META_API_MODEL_OPTIONS: &[&str] = &["muse-wonder-1.0"];

pub(crate) fn optional_meta_http_clubs() -> Vec<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    let configured = optional_meta_http_club();
    let configured_model = configured
        .as_ref()
        .and_then(|(_, club, _)| club.model_identity());
    let mut links: Vec<_> = configured.into_iter().collect();
    for model in META_API_MODEL_OPTIONS {
        if configured_model
            .as_deref()
            .is_some_and(|configured| configured.eq_ignore_ascii_case(model))
        {
            continue;
        }
        links.extend(meta_http_club(model, model.to_string(), &[]));
    }
    links
}

fn meta_http_club(
    alias: &str,
    model: String,
    model_keys: &[&str],
) -> Option<(String, Arc<dyn Club>, Arc<AtomicBool>)> {
    if std::env::var("ANGEL_META_API").is_ok_and(|api| api.trim().eq_ignore_ascii_case("chat")) {
        return optional_sota_http_club(
            alias,
            &model,
            &["ANGEL_META_URL"],
            "https://api.meta.ai/v1",
            model_keys,
            Some(&model),
            &["ANGEL_META_KEY", "META_API_KEY"],
        );
    }
    if !api_club_enabled("meta") {
        return None;
    }
    let key = env_first(&["ANGEL_META_KEY", "META_API_KEY"])?;
    let base =
        env_first(&["ANGEL_META_URL"]).unwrap_or_else(|| "https://api.meta.ai/v1".to_string());
    let effort = env_first(&["ANGEL_META_REASONING_EFFORT"])
        .map(|effort| effort.to_ascii_lowercase())
        .filter(|effort| META_REASONING_LEVELS.contains(&effort.as_str()))
        .or_else(|| {
            super::model_defaults::entry(&model)
                .and_then(|entry| entry.default_effort)
                .filter(|effort| META_REASONING_LEVELS.contains(&effort.as_str()))
        });
    let club = crate::agent::openai_codex::CodexClub::api_key_seat(
        model.clone(),
        model,
        format!("{}/responses", base.trim_end_matches('/')),
        key,
        effort,
        META_REASONING_LEVELS
            .iter()
            .map(|level| level.to_string())
            .collect(),
        RouteMetadata::default(),
    )
    .with_reasoning_summary(
        env_first(&["ANGEL_META_REASONING_SUMMARY"])
            .map(|level| level.to_ascii_lowercase())
            .filter(|level| ["auto", "concise", "detailed"].contains(&level.as_str()))
            .unwrap_or_else(|| "detailed".to_string()),
    )
    .with_reasoning_replay(crate::agent::harness::env_flag("ANGEL_META_REPLAY", true))
    .with_prompt_cache_key()
    .sota_tuned();
    Some((
        alias.to_string(),
        Arc::new(club),
        Arc::new(AtomicBool::new(true)),
    ))
}

pub(crate) fn openrouter_configured() -> bool {
    api_club_enabled("openrouter")
        && env_first(&["ANGEL_OPENROUTER_MODEL", "OPENROUTER_MODEL"]).is_some()
        && env_first(&["ANGEL_OPENROUTER_KEY", "OPENROUTER_API_KEY"]).is_some()
}

pub(crate) fn default_glm_url() -> &'static str {
    if env_first(&[
        "ANGEL_GLM_KEY",
        "GLM_API_KEY",
        "ZHIPU_API_KEY",
        "BIGMODEL_API_KEY",
    ])
    .is_none()
        && env_first(&["ZAI_API_KEY"]).is_some()
    {
        "https://api.z.ai/api/coding/paas/v4"
    } else {
        "https://open.bigmodel.cn/api/paas/v4"
    }
}

pub(crate) struct SlotMeta {
    pub(crate) agent: usize,
    pub(crate) slot: usize,
    pub(crate) model: String,
    pub(crate) n_params: Option<u64>,
    pub(crate) is_swarm: bool,
    pub(crate) auto: bool,
    pub(crate) available: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub(crate) struct Surface {
    host: String,
    ip: String,
    port: u16,
    url: String,
    model_id: String,
}

pub(crate) fn scan_ports() -> Vec<u16> {
    if let Ok(s) = std::env::var("ANGEL_SCAN_PORTS") {
        let v: Vec<u16> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if !v.is_empty() {
            return v;
        }
    }
    vec![8080, 8000, 8001, 8081, 1234, 11434, 30000]
}

pub(crate) fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| {
            matches!(
                v.trim(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(default)
}

pub(crate) fn tailnet_resolution_enabled() -> bool {
    env_flag("ANGEL_TAILNET_RESOLVE", false)
}

pub(crate) fn fleet_scan_enabled() -> bool {
    env_flag("ANGEL_SCAN", false)
}

pub(crate) fn hydra_discovery_enabled() -> bool {
    env_flag("ANGEL_HYDRA_DISCOVER", false)
}

pub(crate) fn hydra_publish_enabled() -> bool {
    env_flag("ANGEL_HYDRA_PUBLISH", false)
}

fn model_serving_host_excluded(host: &str, ip: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    let ip = ip.trim().to_ascii_lowercase();
    let Ok(excluded) = std::env::var("ANGEL_MODEL_SERVING_EXCLUDE_HOSTS") else {
        return false;
    };
    excluded.split(',').any(|value| {
        let value = value.trim();
        !value.is_empty() && (value.eq_ignore_ascii_case(&host) || value.eq_ignore_ascii_case(&ip))
    })
}

pub(crate) fn model_serving_target_allowed(host: &str, ip: &str) -> bool {
    !model_serving_host_excluded(host, ip)
}

pub(crate) fn hydra_base() -> String {
    std::env::var("ANGEL_HYDRA_URL").unwrap_or_else(|_| "http://127.0.0.1:17876".to_string())
}

pub(crate) fn scan_interval() -> Option<Duration> {
    let secs = std::env::var("ANGEL_SCAN_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(45);
    (secs > 0).then(|| Duration::from_secs(secs))
}

pub(crate) fn scan_concurrency(targets: usize) -> usize {
    let configured = std::env::var("ANGEL_SCAN_CONCURRENCY")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(32)
        .min(256);
    targets.min(configured)
}

pub(crate) fn parse_endpoint_host_port(endpoint: &str) -> Option<(String, u16)> {
    let rest = endpoint.trim().strip_prefix("http://")?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix("/v1").unwrap_or(rest);
    let (host, port) = rest.rsplit_once(':')?;
    if host.is_empty() || host.contains('/') {
        return None;
    }
    let port: u16 = port.parse().ok()?;
    Some((host.to_string(), port))
}

pub(crate) fn parse_hydra_chat_targets(v: &serde_json::Value) -> Vec<(String, String, u16)> {
    let Some(surfaces) = v.get("surfaces").and_then(|s| s.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for s in surfaces {
        let chat = s
            .get("roles")
            .and_then(|r| r.as_array())
            .map(|r| r.iter().any(|x| x.as_str() == Some("chat")))
            .unwrap_or(false);
        if !chat {
            continue;
        }
        let Some((ip, port)) = s
            .get("endpoint")
            .and_then(|e| e.as_str())
            .and_then(parse_endpoint_host_port)
        else {
            continue;
        };
        let host = s.get("host_id").and_then(|h| h.as_str()).unwrap_or("");
        out.push((canonical_fleet_box(host), ip, port));
    }
    out
}

pub(crate) fn hydra_chat_targets() -> Vec<(String, String, u16)> {
    if !hydra_discovery_enabled() {
        return Vec::new();
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout_read(Duration::from_millis(1500))
        .timeout_write(Duration::from_millis(800))
        .build();
    let url = format!("{}/surfaces/list", hydra_base().trim_end_matches('/'));
    let Ok(resp) = agent
        .post(&url)
        .set("content-type", "application/json")
        .send_string("{}")
    else {
        return Vec::new();
    };
    let Ok(v) = resp.into_json::<serde_json::Value>() else {
        return Vec::new();
    };
    parse_hydra_chat_targets(&v)
}

pub(crate) fn scan_fleet() -> Vec<Surface> {
    if !fleet_scan_enabled() {
        return Vec::new();
    }
    let tailnet = live_tailnet_hosts();
    let ports = scan_ports();
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout_read(Duration::from_millis(1500))
        .timeout_write(Duration::from_millis(800))
        .user_agent(concat!("angelX-cockpit-scan/", env!("CARGO_PKG_VERSION")))
        .build();
    let mut targets: Vec<(String, String, u16)> = Vec::new();
    let mut queued: std::collections::HashSet<(String, u16)> = std::collections::HashSet::new();
    for (host, h) in &tailnet {
        if !h.online {
            continue;
        }
        if !model_serving_target_allowed(host, &h.ip) {
            continue;
        }
        for &port in &ports {
            if queued.insert((h.ip.clone(), port)) {
                targets.push((host.clone(), h.ip.clone(), port));
            }
        }
    }
    for (host, ip, port) in hydra_chat_targets() {
        if !model_serving_target_allowed(&host, &ip) {
            continue;
        }
        if queued.insert((ip.clone(), port)) {
            targets.push((host, ip, port));
        }
    }
    if targets.is_empty() {
        return Vec::new();
    }
    let found: Mutex<Vec<Surface>> = Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = scan_concurrency(targets.len());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let agent = &agent;
            let found = &found;
            let next = &next;
            let targets = &targets;
            scope.spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((host, ip, port)) = targets.get(i) else {
                        break;
                    };
                    let url = format!("http://{ip}:{port}/v1");
                    let Ok(resp) = agent.get(&format!("{url}/models")).call() else {
                        continue;
                    };
                    let Ok(v) = resp.into_json::<serde_json::Value>() else {
                        continue;
                    };
                    let model_id = v
                        .pointer("/data/0/id")
                        .and_then(|x| x.as_str())
                        .or_else(|| v.pointer("/models/0/name").and_then(|x| x.as_str()))
                        .map(|s| s.to_string());
                    let Some(model_id) = model_id else {
                        continue;
                    };
                    if let Ok(mut g) = found.lock() {
                        g.push(Surface {
                            host: canonical_fleet_box(host),
                            ip: ip.clone(),
                            port: *port,
                            url,
                            model_id,
                        });
                    }
                }
            });
        }
    });
    found.into_inner().unwrap_or_default()
}

pub(crate) fn short_model_label(model_id: &str) -> String {
    let base = model_id
        .rsplit('/')
        .next()
        .unwrap_or(model_id)
        .trim_end_matches(".gguf")
        .trim_end_matches(".GGUF");
    let mut s = base.to_lowercase();
    if s.len() > 24 {
        let mut end = 24;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        while s.ends_with(['-', '_', '.']) {
            s.pop();
        }
    }
    if s.is_empty() { "model".to_string() } else { s }
}

pub(crate) fn parse_param_billions(model_id: &str) -> Option<f64> {
    let s = model_id.to_ascii_lowercase();
    let b = s.as_bytes();
    let mut best: Option<f64> = None;
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        if i < b.len()
            && b[i] == b'b'
            && let Ok(n) = s[start..i].parse::<f64>()
            && n > 0.0
            && n < 2000.0
        {
            best = Some(best.map_or(n, |m: f64| m.max(n)));
        }
    }
    best
}

pub(crate) fn known_model_billions(model_l: &str) -> Option<f64> {
    if model_l.contains("gemma4") {
        return Some(26.0);
    }
    if model_l.contains("longcat-2.0")
        || model_l.contains("longcat2.0")
        || model_l.contains("longcat")
    {
        return Some(520.0);
    }
    None
}

pub(crate) fn model_smartness(model_id: &str, n_params: Option<u64>, is_swarm: bool) -> u64 {
    let ml = model_id.to_ascii_lowercase();
    let base: f64 = n_params
        .map(|n| n as f64)
        .or_else(|| known_model_billions(&ml).map(|b| b * 1e9))
        .or_else(|| parse_param_billions(&ml).map(|b| b * 1e9))
        .unwrap_or(0.5e9);
    let mut score = base;
    if is_swarm {
        score *= 1.5;
    }
    if [
        "coder",
        "opus",
        "qwopus",
        "r1",
        "deepseek-r",
        "siq",
        "qwq",
        "think",
        "reason",
    ]
    .iter()
    .any(|k| ml.contains(k))
    {
        score *= 1.1;
    }
    score as u64
}

pub(crate) fn smartest_available(agents: &mut [Agent], slot_meta: &[SlotMeta]) -> Option<usize> {
    let best = slot_meta
        .iter()
        .filter(|m| m.auto && m.available.load(Ordering::Relaxed))
        .max_by_key(|m| model_smartness(&m.model, m.n_params, m.is_swarm))?;
    if let Some(a) = agents.get_mut(best.agent)
        && best.slot < a.slots.len()
    {
        a.active = best.slot;
    }
    Some(best.agent)
}

pub(crate) fn spawn_fleet_discovery(
    brain: Option<(String, u16)>,
    mut known: std::collections::HashSet<(String, u16)>,
    key: Option<String>,
    tx: std::sync::mpsc::Sender<(Agent, String)>,
) {
    if !fleet_scan_enabled() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("fleet-discovery".into())
        .spawn(move || {
            loop {
                if !fleet_scan_enabled() {
                    return;
                }
                let surfaces = scan_fleet();
                for s in &surfaces {
                    if !known.insert((s.ip.clone(), s.port)) {
                        continue;
                    }
                    let label = short_model_label(&s.model_id);
                    let club: Arc<dyn Club> = Arc::new(
                        HttpClub::new(
                            label.clone(),
                            s.url.clone(),
                            s.model_id.clone(),
                            key.clone(),
                        )
                        .follow_backend(),
                    );
                    let agent = Agent {
                        name: s.host.clone(),
                        slots: vec![Slot {
                            label,
                            club,
                            available: Arc::new(AtomicBool::new(true)),
                        }],
                        active: 0,
                    };
                    if tx.send((agent, s.model_id.clone())).is_err() {
                        return;
                    }
                }
                publish_surfaces_to_hydra(&surfaces, &brain);
                let Some(delay) = scan_interval() else {
                    return;
                };
                std::thread::sleep(delay);
            }
        });
}

fn publish_surfaces_to_hydra(surfaces: &[Surface], brain: &Option<(String, u16)>) {
    if surfaces.is_empty() || !hydra_publish_enabled() {
        return;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout_read(Duration::from_millis(1500))
        .timeout_write(Duration::from_millis(800))
        .build();
    let url = format!("{}/surfaces/register", hydra_base().trim_end_matches('/'));
    for s in surfaces {
        let is_brain = brain
            .as_ref()
            .map(|(ip, p)| *ip == s.ip && *p == s.port)
            .unwrap_or(false);
        let mut roles = vec!["chat"];
        if is_brain {
            roles.push("brain");
        }
        let body = serde_json::json!({
            "surface_id": format!("{}:{}", s.host, s.port),
            "host_id": s.host,
            "endpoint": s.url,
            "model_id": s.model_id,
            "roles": roles,
            "status": "verified",
            "probe_type": "openai_models",
            "source": "cockpit-scan",
            "owner": "cockpit",
        });
        if let Ok(bytes) = serde_json::to_vec(&body) {
            let _ = agent
                .post(&url)
                .set("content-type", "application/json")
                .send_bytes(&bytes);
        }
    }
}
