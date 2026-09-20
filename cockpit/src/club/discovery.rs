//! Tailnet/URL endpoint resolution and fleet discovery / smartness ranking.

use super::*;

// ---------------------------------------------------------------------------
// Tailnet-dynamic fleet resolution is opt-in. When enabled, resolve club URLs
// from `tailscale status --json` by the host's DNSName label. A host that's
// absent or offline is not auto-selected. An explicit `ANGEL_<LABEL>_URL`
// always wins, and the ordinary fallback is loopback rather than a private
// deployment address.
// ---------------------------------------------------------------------------

/// A tailnet peer: its IPv4 (100.x) and whether tailscale reports it online.
pub(crate) struct TailHost {
    pub(crate) ip: String,
    pub(crate) online: bool,
}

/// The unique tailnet name = the first DNSName label, lowercased.
/// `"atlas-1.tail-example.ts.net."` → `"atlas-1"`.
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
    // First IPv4 (the 100.x); skip the IPv6 that follows it.
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
    // `Self` omits Online (it's the local node) → treat as online.
    let online = node.get("Online").and_then(|o| o.as_bool()).unwrap_or(true);
    map.insert(
        label,
        TailHost {
            ip: ip.to_string(),
            online,
        },
    );
}

/// Parse a `tailscale status --json` document → tailnet-label → host. Pure.
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

/// Query the live tailnet. Callers must gate this behind an explicit opt-in.
/// Empty map if tailscale is missing or unreadable.
pub(crate) fn live_tailnet_hosts() -> std::collections::HashMap<String, TailHost> {
    let mut command = std::process::Command::new("tailscale");
    command.args(["status", "--json"]);
    tailnet_hosts_from_command(command, Duration::from_secs(3))
}

/// Parse one best-effort tailnet-status command behind a fixed whole-tree
/// deadline. Keeping this boundary command-shaped makes the hang behavior
/// regression-testable without replacing the operator's `tailscale` binary.
pub(crate) fn tailnet_hosts_from_command(
    command: std::process::Command,
    timeout: Duration,
) -> std::collections::HashMap<String, TailHost> {
    crate::workspace_store::capture_bounded_command(command, timeout)
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

/// Environment variable listing the extra host labels that fold into the
/// cockpit's `spark` box — typically the peer's tailnet name and its Hydra
/// host id — as a comma-separated, case-insensitive list. The source ships no
/// hardware hostnames: without this variable only the literal `spark` label
/// resolves that box and every other label is looked up verbatim.
pub(crate) const SPARK_HOST_ALIASES_ENV: &str = "ANGEL_SPARK_HOST";

/// The `spark` box label plus every configured alias, lowercased and deduped.
pub(crate) fn spark_host_aliases() -> Vec<String> {
    let mut aliases = vec!["spark".to_string()];
    if let Ok(raw) = std::env::var(SPARK_HOST_ALIASES_ENV) {
        for alias in raw.split(',') {
            let alias = alias.trim().to_ascii_lowercase();
            if !alias.is_empty() && !aliases.contains(&alias) {
                aliases.push(alias);
            }
        }
    }
    aliases
}

/// Stable bag-box name for a tailnet / Hydra host label. Any label listed in
/// `ANGEL_SPARK_HOST` canonicalizes to `spark`, which is what lets a live
/// `:8001` surface fold into the spark tab instead of appearing as a stray box
/// the operator never tabs onto. Every other label is returned trimmed and
/// lowercased.
pub(crate) fn canonical_fleet_box(host: &str) -> String {
    canonical_fleet_box_with(host, &spark_host_aliases())
}

/// Pure form of [`canonical_fleet_box`]; `aliases` includes `spark` itself.
pub(crate) fn canonical_fleet_box_with(host: &str, aliases: &[String]) -> String {
    let label = host.trim().to_ascii_lowercase();
    if aliases.contains(&label) {
        "spark".to_string()
    } else {
        label
    }
}

fn fleet_box_lookup_keys(host: &str) -> Vec<String> {
    let aliases = spark_host_aliases();
    let canonical = canonical_fleet_box_with(host, &aliases);
    let mut keys = vec![canonical.clone()];
    if canonical == "spark" {
        for alias in aliases {
            if !keys.contains(&alias) {
                keys.push(alias);
            }
        }
    } else if !host.trim().is_empty() {
        let raw = host.trim().to_ascii_lowercase();
        if raw != canonical {
            keys.push(raw);
        }
    }
    keys
}

/// Resolve a box host against a parsed tailnet, honoring Spark aliases.
pub(crate) fn tailnet_lookup<'a>(
    host: &str,
    tailnet: &'a std::collections::HashMap<String, TailHost>,
) -> Option<&'a TailHost> {
    fleet_box_lookup_keys(host)
        .into_iter()
        .find_map(|key| tailnet.get(&key))
}

/// Build `http://<ip>:<port>/v1`, resolving `<ip>` from the tailnet by host (else
/// the caller-provided fallback). Pure (no env) so it's unit-testable.
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

/// A club's endpoint URL: explicit `ANGEL_<LABEL>_URL` wins, else the
/// opt-in tailnet-resolved address, else the caller-provided fallback.
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

/// Is `host` usable for auto-selection? Online per tailscale; if resolution is
/// disabled or unavailable (empty map), assume the fallback/env route is usable.
pub(crate) fn host_online(
    host: &str,
    tailnet: &std::collections::HashMap<String, TailHost>,
) -> bool {
    // An empty host means the box is IP-pinned (no tailnet name lookup), so it's
    // always seedable-online; the background prober gives the real verdict.
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

/// Exact model ids currently published by DeepSeek's OpenAI-compatible API.
/// Keep this list provider-authored: local nicknames belong in the resolver
/// below and must never be forwarded in a request body. `deepseek-flash` is the
/// live V4.1 Flash id (native multimodal); `deepseek-v4-pro` is the text-only
/// Pro seat, whose service and billing are unchanged.
pub(crate) const DEEPSEEK_API_MODEL_OPTIONS: &[&str] = &["deepseek-v4-pro", "deepseek-flash"];

/// GLM routes exposed by the ordinary cockpit when Z.ai credentials are
/// configured. Keep the flagship first: it is the configurable/default `glm`
/// seat. GLM-5.3-Flash and the preceding generation remain selectable beside it.
pub(crate) const GLM_API_MODEL_OPTIONS: &[&str] = &["glm-5.3", "glm-5.3-flash", "glm-5.2"];

/// OpenRouter free seats the ordinary cockpit exposes when a key is configured.
/// Keep the default first: Ox Alpha is the `openrouter` bag slot and the
/// cheap-MoA breadth route. Remaining ids are extra selectable `sota` routes.
/// Provider-authored slugs only — local nicknames belong in the resolver below.
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

/// Resolve retired DeepSeek ids locally while preserving explicit unknown ids.
/// DeepSeek's V4 migration mapped both legacy endpoints
/// (`deepseek-chat`/`deepseek-reasoner`) onto V4 Flash with thinking selected
/// separately, then retired them on 2026-07-24. V4.1 Flash now ships as
/// `deepseek-flash`; the older `deepseek-v4-flash` and the experimental
/// `deepseek-v4-flash-vision-exp` ids still alias it upstream but must never be
/// the request body's model. Matching is exact — the Spark-local V4 serve
/// (`deepseek-v4-flash-dspark`) and every custom/unknown pin pass through
/// untouched.
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

/// Local GLM nicknames. Provider-authored ids pass through unchanged; unknown
/// explicit pins stay forward-compatible for private gateways.
pub(crate) fn resolve_glm_model_alias(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "glm-flash" | "glmflash" | "glm-5.3-flash" => "glm-5.3-flash".to_string(),
        _ => trimmed.to_string(),
    }
}

/// Map OpenRouter model aliases onto current defaults. Explicit
/// paid pins and unknown ids pass through unchanged.
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

/// Saved credentials are not permission to add a provider to the working bag.
/// OAuth clubs and local fleet endpoints are registered separately. API-key
/// providers require an exact, explicit comma-separated ANGEL_API_CLUBS entry;
/// no wildcard, inferred opt-in from a key, or automatic free catalog.
pub(crate) fn api_club_enabled(alias: &str) -> bool {
    let family = if alias.starts_with("glm-") {
        "glm"
    } else if alias == "deepseek-flash" {
        "deepseek"
    } else {
        alias
    };
    std::env::var("ANGEL_API_CLUBS").is_ok_and(|list| {
        list.split(',')
            .any(|entry| entry.trim().eq_ignore_ascii_case(family))
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
    // The two built-in DeepSeek seats share this provider URL. Canonicalize
    // retired ids before constructing the club so neither request bodies nor
    // route identity can leak an upstream-invalid alias. A custom/unknown id is
    // intentionally preserved for private gateways and future provider models.
    let deepseek_seat = default_url.trim_end_matches('/') == "https://api.deepseek.com/v1";
    let model = if deepseek_seat {
        resolve_deepseek_model_alias(&model)
    } else if default_url.contains("openrouter.ai") {
        resolve_openrouter_model_alias(&model)
    } else if default_url.contains("z.ai") || default_url.contains("bigmodel.cn") {
        resolve_glm_model_alias(&model)
    } else {
        model
    };
    let url = env_first(url_keys).unwrap_or_else(|| default_url.to_string());
    // OpenRouter clubs wear the request model as their label so a free slug
    // (or a paid pin) is what the bag and MoA matcher see, not a stale default.
    // DeepSeek must likewise label its configured wire model, not the seat's
    // default: otherwise a Flash request is displayed and attributed as Pro.
    // GLM does the same: pinning `ANGEL_GLM_MODEL=glm-5.3-flash` must make the
    // `glm` seat advertise that id, or `ANGEL_DRIVER=glm-5.3-flash` misses it.
    let label = if deepseek_seat
        || default_url.contains("openrouter.ai")
        || default_url.contains("z.ai")
        || default_url.contains("bigmodel.cn")
    {
        model.clone()
    } else {
        label.to_string()
    };
    // A remote SOTA link feeds the MoA. OpenRouter free seats can still
    // truncate output or rate-limit, so keep the SOTA network-tuned behavior.
    let mut http = HttpClub::new(label, url, model, Some(key));
    if deepseek_seat {
        // This seat reads the operator's configured DeepSeek namespace
        // (`ANGEL_DEEPSEEK_*`), so it *is* the provider's route even when
        // ANGEL_DEEPSEEK_URL points at a local forwarder/observer that relays
        // the real API. Provider-private reasoning replay and the provider's
        // declared capabilities follow the contract, not the URL's host.
        http = http.with_provider_contract(ProviderContract::DeepSeek);
    } else if default_url.contains("z.ai") || default_url.contains("bigmodel.cn") {
        // All GLM catalog seats share ANGEL_GLM_* controls, including a
        // custom gateway/model pin. The display label remains the model id.
        http = http.with_env_namespace("GLM");
    } else if alias == "openrouter" {
        // The configured seat keeps its controls when its model is the label
        // or its endpoint is an operator-selected compatible provider.
        http = http.with_env_namespace("OPENROUTER");
    }
    let club: Arc<dyn Club> = Arc::new(http.sota_tuned());
    Some((alias.to_string(), club, Arc::new(AtomicBool::new(true))))
}

/// Build the configurable GLM seat plus every other built-in GLM catalog
/// choice. The configured model is never duplicated, even when it is one of
/// the built-ins.
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

/// Bind credentials to the selected GLM provider before consulting generic
/// aliases. Operators commonly retain both Zhipu and Z.ai keys while moving an
/// `ANGEL_GLM_URL` between the two services; a stale generic key must not
/// silently shadow the provider-specific credential for that URL.
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

/// An explicitly enabled, explicitly pinned OpenRouter route only. A retained
/// key must not populate the cockpit or its failover bench with speculative
/// free models. Historical model metadata remains available for old receipts.
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

/// Per-slot metadata captured while the bag is assembled, used to pick the
/// smartest *available* model for the brain. `auto` is false for the practice
/// swing and most cloud tabs so the auto-pick never silently jumps to them.
/// LongCat is the exception when configured: the operator explicitly asked for
/// it as the main driver, and `ANGEL_DRIVER` can still override that.
pub(crate) struct SlotMeta {
    pub(crate) agent: usize,
    pub(crate) slot: usize,
    pub(crate) model: String,
    /// Backend-reported parameter count when known (llama.cpp `/v1/models`
    /// `meta.n_params`); `None` falls back to parsing the model id.
    pub(crate) n_params: Option<u64>,
    pub(crate) is_swarm: bool,
    pub(crate) auto: bool,
    pub(crate) available: Arc<AtomicBool>,
}

/// A live model surface discovered by scanning the tailnet: a backend that
/// answered `GET /v1/models`. The model id + param count feed the smartest-
/// available driver heuristic and the Hydra surface registration.
#[derive(Debug, Clone)]
pub(crate) struct Surface {
    /// tailnet label of the host serving it (e.g. `atlas-1`, `compute-a`).
    host: String,
    ip: String,
    port: u16,
    /// `http://<ip>:<port>/v1` — the OpenAI-compatible base.
    url: String,
    model_id: String,
}

/// Ports the fleet is known to serve models on, probed on every online peer
/// during discovery. Override with `ANGEL_SCAN_PORTS` (comma-separated).
pub(crate) fn scan_ports() -> Vec<u16> {
    if let Ok(s) = std::env::var("ANGEL_SCAN_PORTS") {
        let v: Vec<u16> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        if !v.is_empty() {
            return v;
        }
    }
    vec![
        8093, 8080, 8000, 8001, 8002, 18888, 8081, 8011, 8090, 8092, 8360, 11434, 30000,
    ]
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

pub(crate) fn atlas_model_serving_enabled() -> bool {
    env_flag("ANGEL_ATLAS_MODEL_SERVING", false)
}

pub(crate) fn model_serving_target_allowed(host: &str, _ip: &str) -> bool {
    let h = host.trim().to_ascii_lowercase();
    let atlas = h == "atlas" || h == "atlas-1" || h.starts_with("atlas-");
    !atlas || atlas_model_serving_enabled()
}

/// The local Hydra head's base URL (`ANGEL_HYDRA_URL` overrides).
pub(crate) fn hydra_base() -> String {
    std::env::var("ANGEL_HYDRA_URL").unwrap_or_else(|_| "http://127.0.0.1:17876".to_string())
}

/// How often the discovery sweep re-runs. `ANGEL_SCAN_INTERVAL_SECS` overrides;
/// `0` = sweep once at launch (the old behavior). The default keeps up with an
/// operator who swaps checkpoints mid-session: a model pulled up on any rig —
/// or registered with Hydra — becomes selectable within a minute.
pub(crate) fn scan_interval() -> Option<Duration> {
    let secs = std::env::var("ANGEL_SCAN_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(45);
    (secs > 0).then(|| Duration::from_secs(secs))
}

/// Bound the OS-thread burst used by one discovery sweep. Tailnets can contain
/// many non-model peers, and probing every peer x port with one thread each can
/// otherwise create hundreds of threads every 45 seconds. Model inference
/// concurrency is independent of this control-plane limit.
pub(crate) fn scan_concurrency(targets: usize) -> usize {
    let configured = std::env::var("ANGEL_SCAN_CONCURRENCY")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(32)
        .min(256);
    targets.min(configured)
}

/// Parse a Hydra surface endpoint (`http://<host>:<port>[/v1]`) → (host, port).
/// Local fleet surfaces carry an explicit plain-http host:port; anything else
/// (https cloud URLs, missing port) is not a scannable fleet endpoint → `None`.
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

/// Pure parse of a Hydra `/surfaces/list` reply → `(host label, ip, port)`
/// probe targets. Only `roles:["chat",…]` surfaces qualify — gen/OCR/embedding
/// surfaces registered on the same head aren't chat clubs. The registry's
/// `model_id` is deliberately ignored: a row is a *candidate*, and the live
/// `/models` probe answers with what the endpoint serves right now.
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
        let host = s
            .get("host_id")
            .and_then(|h| h.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let host = if host.is_empty() {
            ip.clone()
        } else {
            canonical_fleet_box(&host)
        };
        out.push((host, ip, port));
    }
    out
}

/// Chat surfaces registered with the LOCAL Hydra head, as probe targets for the
/// fleet sweep. This is the read half of the Hydra loop (the cockpit already
/// publishes what it scans): registering a model surface with Hydra — by
/// modeld, a script, or another agent — is enough to make it selectable here,
/// including localhost-bound heads the tailnet port sweep can't see. Empty on
/// any transport/parse failure (a head that's down just contributes nothing),
/// and unless `ANGEL_HYDRA_DISCOVER=1`.
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

/// Discover live model surfaces: every ONLINE tailnet peer × candidate port,
/// plus every chat surface registered with the local Hydra head, probed with
/// `GET http://<ip>:<port>/v1/models` on short timeouts — all probes concurrent
/// so the whole sweep costs ~one timeout regardless of fleet size. A port that
/// answers with at least one model is a surface; a peer that's down, a closed
/// port, a stale registry row, or a non-JSON reply is simply skipped. Empty
/// unless `ANGEL_SCAN=1`.
pub(crate) fn scan_fleet() -> Vec<Surface> {
    if !fleet_scan_enabled() {
        return Vec::new();
    }
    let tailnet = live_tailnet_hosts();
    let ports = scan_ports();
    // Short, fixed timeouts so one closed/black-holed port can't stall launch.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(800))
        .timeout_read(Duration::from_millis(1500))
        .timeout_write(Duration::from_millis(800))
        .user_agent(concat!("angel0-cockpit-scan/", env!("CARGO_PKG_VERSION")))
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
    // Hydra-registered chat surfaces join the same sweep and the same liveness
    // bar — this also covers endpoints the tailnet sweep can't reach (loopback
    // heads on this rig, off-tailnet boxes, nonstandard ports).
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
                    // OpenAI shape: data[].id ; llama.cpp also fills models[].name.
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

/// A short, human label for a discovered model id: drop the path and extension,
/// lowercase, clamp length. `/home/x/qworld/nvfp4_model` → `nvfp4_model`;
/// `Qwen3.6-27B-MTP-pi-tune-Q4_K_M.gguf` → `qwen3.6-27b-mtp-pi-tune`.
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

/// Largest `<N>B`/`<N>b` parameter token in a model id, in billions.
/// `Qwen3.6-27B-…` → 27.0 ; `siq-1-35b-turbo` → 35.0 ; `gemma4` → None (the `4`
/// is a version, not params — not followed by `b`).
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

/// Param counts (billions) for fleet models whose id doesn't encode the size,
/// taken from the box specs above. Keeps the heuristic from under-rating a known
/// big model just because its name is a codename.
pub(crate) fn known_model_billions(model_l: &str) -> Option<f64> {
    if model_l.contains("gemma4") {
        return Some(26.0); // the 26B/4B-active NVFP4 MoE the swarm fans over
    }
    if model_l.contains("longcat-2.0")
        || model_l.contains("longcat2.0")
        || model_l.contains("longcat")
    {
        return Some(520.0); // score the configured LongCat account above local fleet models
    }
    None
}

/// Heuristic "smartness" for the driver auto-pick: bigger models score higher,
/// with a nudge for the swarm (a multi-agent amplifier over its inner model) and
/// reasoning/coder heads. Param count is the spine — the backend's reported
/// `n_params`, else a known-model size, else the largest `<N>B` token in the id.
/// Unknown size falls to a low floor so an unsized model still outranks the
/// practice swing but loses to any peer we *can* size.
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

/// Put the smartest *available* fleet model in hand: among auto-eligible slots
/// that are up, pick the highest [`model_smartness`], set that box's active mode
/// to it, and return the box index. `None` when nothing auto-eligible is up.
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

/// Spawn the background fleet-discovery thread: sweep for live model surfaces
/// ([`scan_fleet`]: tailnet ports + Hydra-registered chat surfaces), stream
/// anything new into the bag, register what was found with the LOCAL Hydra
/// head (`POST /surfaces/register`) so the rest of the fleet/tools see what
/// this rig found — then sleep and sweep again ([`scan_interval`]), so a model
/// the operator pulls up mid-session appears without a relaunch. Runs ENTIRELY
/// off the startup path — a slow `tailscale` subprocess or a black-holed port
/// can never delay the first frame (the cockpit guarantees a <50ms bootstrap).
/// Best-effort: a Hydra head that's down just means nothing is published or
/// read this round. Endpoints carry the tailnet IP (not `0.0.0.0`), so Hydra's
/// unauthenticated-public guard never rejects them. `ANGEL_SCAN=1` enables
/// discovery; `ANGEL_HYDRA_PUBLISH=1` enables registration;
/// `ANGEL_HYDRA_DISCOVER=1` enables registry reads; `ANGEL_HYDRA_URL` overrides
/// the loopback head address.
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
                // Stream every NEW machine/model into the bag as a selectable box. A
                // port the static specs (or an earlier round) already cover is skipped
                // — same endpoint, just confirmed live; its follow-backend club picks
                // up a swapped checkpoint on its own. The UI thread folds the rest in
                // via drain_discovered.
                for s in &surfaces {
                    if !known.insert((s.ip.clone(), s.port)) {
                        continue;
                    }
                    let label = short_model_label(&s.model_id);
                    // follow_backend: the operator swaps checkpoints on fleet rigs
                    // freely — the club renames itself instead of going stale.
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
                        return; // the bag (receiver) is gone — nothing left to fold into
                    }
                }
                publish_surfaces_to_hydra(&surfaces, &brain);
                let Some(delay) = scan_interval() else {
                    return; // ANGEL_SCAN_INTERVAL_SECS=0 → the old one-shot scan
                };
                std::thread::sleep(delay);
            }
        });
}

/// Register every surface the sweep confirmed live with the LOCAL Hydra head.
/// Re-registering each round is deliberate: it refreshes `updated_at`/`status`,
/// so the registry doubles as a liveness record for other agents on this rig.
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
