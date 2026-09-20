//! `web_search` — a thin, read-only GET over the local SearXNG JSON API
//! (`ANGEL_SEARXNG_URL` overrides; `ANGEL_WEB_SEARCH=0` disables registration).

use super::http_transport;
use crate::agent::club::ToolDef;
use crate::agent::harness::{Tool, ToolRegistry, env_flag};
use serde_json::Value;
use std::io;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

const HTTP_PRIVATE_NETWORK_ENV: &str = "ANGEL_HTTP_ALLOW_PRIVATE_NETWORK";
const HTTP_MUTATIONS_ENV: &str = "ANGEL_HTTP_ALLOW_MUTATIONS";

fn is_shared_ipv4(ip: std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

fn is_reserved_ipv4(ip: std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 0
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        || octets[0] >= 240
}

fn is_deprecated_site_local_ipv6(ip: std::net::Ipv6Addr) -> bool {
    ip.segments()[0] & 0xffc0 == 0xfec0
}

/// Allow public addresses and explicit loopback. RFC1918, ULA, and CGNAT
/// require an operator opt-in; link-local/metadata, multicast, unspecified,
/// documentation, benchmark, and reserved ranges always fail closed.
pub(crate) fn http_ip_allowed(ip: IpAddr, allow_private_network: bool) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            if ip.is_loopback() {
                return true;
            }
            if ip.is_unspecified()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_documentation()
                || is_reserved_ipv4(ip)
            {
                return false;
            }
            if ip.is_private() || is_shared_ipv4(ip) {
                return allow_private_network;
            }
            true
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return http_ip_allowed(IpAddr::V4(mapped), allow_private_network);
            }
            if ip.is_loopback() {
                return true;
            }
            let segments = ip.segments();
            if ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unicast_link_local()
                || is_deprecated_site_local_ipv6(ip)
                || (segments[0] == 0x2001 && segments[1] == 0x0db8)
            {
                return false;
            }
            if ip.is_unique_local() {
                return allow_private_network;
            }
            true
        }
    }
}

pub(crate) fn resolve_http_target(
    netloc: &str,
    allow_private_network: bool,
) -> io::Result<Vec<SocketAddr>> {
    let addresses: Vec<SocketAddr> = netloc.to_socket_addrs()?.collect();
    if addresses.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("HTTP target {netloc:?} resolved to no addresses"),
        ));
    }
    if let Some(blocked) = addresses
        .iter()
        .find(|addr| !http_ip_allowed(addr.ip(), allow_private_network))
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "HTTP target {netloc:?} resolved to blocked address {}; set {HTTP_PRIVATE_NETWORK_ENV}=1 only for an intended RFC1918/ULA/CGNAT service",
                blocked.ip()
            ),
        ));
    }
    Ok(addresses)
}

fn http_method_allowed(method: &str, allow_mutations: bool) -> bool {
    matches!(method, "GET" | "HEAD") || allow_mutations
}

fn credential_bearing_header(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    matches!(
        name.as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "cookie2"
            | "x-api-key"
            | "api-key"
            | "apikey"
    ) || name.contains("token")
        || name.contains("secret")
}

fn http_request_redirect_limit(
    method: &str,
    headers: Option<&serde_json::Map<String, Value>>,
) -> u32 {
    if !matches!(method, "GET" | "HEAD")
        || headers.is_some_and(|headers| headers.keys().any(|key| credential_bearing_header(key)))
    {
        0
    } else {
        5
    }
}

// ---------------------------------------------------------------------------
// web_search — native search over the local SearXNG (angel's web stack, :8888).
// Codex ships a built-in web_search; angel already has the *capability* at the
// angel/Hermes layer, but the cockpit's own agent loop lacked a first-class tool.
// This is a thin, read-only GET over SearXNG's JSON API, mirroring the moa's
// research query. `ANGEL_SEARXNG_URL` overrides; `ANGEL_WEB_SEARCH=0` disables.
// ---------------------------------------------------------------------------

/// Explicit configuration only: never advertise a guessed host or port.
pub(crate) fn configured_research_origin() -> Option<String> {
    std::env::var("ANGEL_SEARXNG_URL")
        .ok()
        .and_then(|raw| research_origin(&raw))
}

pub(crate) fn research_origin(raw: &str) -> Option<String> {
    let parsed = url::Url::parse(raw.trim()).ok()?;
    (matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str().is_some()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none())
    .then(|| raw.trim().trim_end_matches('/').to_string())
}

fn research_description(description: &str) -> String {
    match configured_research_origin() {
        Some(origin) => format!(
            "{description} Research search origin: {origin}; fetch document URLs returned by web_search with web_fetch. Do not probe the host with shell."
        ),
        None => description.to_string(),
    }
}

pub(crate) fn research_search_endpoint(origin: &str) -> String {
    match url::Url::parse(origin) {
        Ok(mut url) if url.path() == "/" => {
            url.set_path("/search");
            url.into()
        }
        _ => origin.to_string(),
    }
}

pub(crate) fn searxng_url() -> String {
    std::env::var("ANGEL_SEARXNG_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|origin| research_search_endpoint(&origin))
        .unwrap_or_else(|| "http://127.0.0.1:8888/search".to_string())
}

/// Format SearXNG's JSON response into a compact, model-friendly result list.
/// Pure (no network) so it's unit-testable with a canned response.
pub(crate) fn format_searxng(v: &Value, limit: usize) -> String {
    let Some(results) = v.get("results").and_then(|r| r.as_array()) else {
        return "no results".to_string();
    };
    let mut lines = Vec::new();
    for r in results {
        if lines.len() >= limit {
            break;
        }
        let title = r.get("title").and_then(|x| x.as_str()).unwrap_or("").trim();
        let url = r.get("url").and_then(|x| x.as_str()).unwrap_or("").trim();
        let content = r
            .get("content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim();
        if title.is_empty() && content.is_empty() {
            continue;
        }
        lines.push(format!(
            "{}. {title}\n   {url}\n   {content}",
            lines.len() + 1
        ));
    }
    if lines.is_empty() {
        "no results".to_string()
    } else {
        lines.join("\n")
    }
}

pub(crate) struct WebSearchTool;
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "web_search".to_string(),
            description: research_description(
                "Search the web via the local SearXNG. Returns ranked title / url / \
                          snippet. Use for current information, documentation, error messages, \
                          or anything outside the workspace.",
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "search query" },
                    "limit": { "type": "integer", "description": "max results (default 5)" },
                },
                "required": ["query"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("missing 'query'")?;
        if query.trim().is_empty() {
            return Err("'query' must not be empty".to_string());
        }
        let limit = args["limit"].as_u64().unwrap_or(5).clamp(1, 20) as usize;
        // The harness supplies an explicit per-task declaration without mutating
        // process environment shared by other turns or dispatch workers.
        let url = args
            .get("_research_origin")
            .and_then(Value::as_str)
            .and_then(research_origin)
            .map(|origin| research_search_endpoint(&origin))
            .unwrap_or_else(searxng_url);
        let req = http_transport::request("GET", &url, false, 5)
            .query("q", query)
            .query("format", "json");
        let resp = match req.call() {
            Ok(r) => r,
            // The service answered but rejected the request — that is a
            // configuration/quota problem, not an unreachable host. Say which,
            // so the model doesn't chase a "start SearXNG" red herring. The most
            // common causes: JSON output not enabled (403) and bot/rate limiting
            // (429).
            Err(http_transport::Error::Status(code, resp)) => {
                let snippet = crate::drive::science::sanitize_metadata(
                    &resp.into_string().map_err(|e| e.to_string())?,
                    200,
                );
                let hint = match code {
                    403 => " — enable JSON output (`formats: [json]` in SearXNG's settings.yml)",
                    429 => " — rate/bot limited; back off and retry",
                    _ => "",
                };
                return Err(format!(
                    "web_search: SearXNG at {url} responded HTTP {code} (service is UP){hint}. {snippet}"
                ));
            }
            Err(e) => {
                return Err(format!(
                    "web_search: SearXNG unreachable at {url} ({e}). Is it running? \
                     Override with ANGEL_SEARXNG_URL."
                ));
            }
        };
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("web_search: bad JSON from SearXNG: {e}"))?;
        Ok(format_searxng(&v, limit))
    }
}

/// Register `web_search` over the local SearXNG, gated on `ANGEL_WEB_SEARCH`
/// (default on). Read-only; errors gracefully if SearXNG is down.
pub(crate) fn maybe_register_web_search(r: &mut ToolRegistry) {
    if env_flag("ANGEL_WEB_SEARCH", true) {
        r.register(Box::new(WebSearchTool));
    }
}

// ---------------------------------------------------------------------------
// grok_research — live web + X research via Grok (xAI). SearXNG finds pages;
// this taps Grok's own agentic web/X search for current, dated, source-cited
// findings — the same scout the MoA panel grounds on, now a first-class tool
// any driver can call on demand (not just passive proposer grounding). Reuses
// the OAuth-CLI GrokResearchClub, so `~/.grok/auth.json` and silent refresh are
// shared. Registered only when Grok OAuth is actually configured, so a box
// without a Grok login simply doesn't advertise it. `ANGEL_GROK_TOOL=0` disables.
// ---------------------------------------------------------------------------

/// Frame a bare query as a research-scout brief so the tool returns comparable,
/// source-cited findings to the MoA grounding path. Pure — unit-testable.
pub(crate) fn grok_tool_prompt(query: &str) -> String {
    format!(
        "You are Grok with live web and X (twitter) search. Research the request below and \
         report back concise, well-organized findings the caller can act on. Prefer current, \
         latest, trending, or online facts; include dates when available, cite source URLs, and \
         flag anything uncertain or contested. Do not refuse for recency — search.\n\n\
         Request:\n{query}"
    )
}

pub(crate) struct GrokResearchTool {
    club: std::sync::Arc<crate::agent::club::GrokResearchClub>,
}

impl GrokResearchTool {
    pub(crate) fn from_env() -> Option<Self> {
        crate::agent::club::GrokResearchClub::from_env().map(|club| Self {
            club: std::sync::Arc::new(club),
        })
    }
}

impl Tool for GrokResearchTool {
    fn name(&self) -> &str {
        "grok_research"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "grok_research".to_string(),
            description: "Live web + X (twitter) research via Grok (xAI). Returns fresh, dated, \
                          source-cited findings. Prefer over web_search when recency, breaking \
                          news, or social/X signal matters — it runs Grok's own agentic web + X \
                          search and brings back cited context."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "what to research (a question or topic)"
                    },
                },
                "required": ["query"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }
    fn call_with_cancel(
        &self,
        args: &Value,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<String, String> {
        let query = args["query"].as_str().ok_or("missing 'query'")?;
        if query.trim().is_empty() {
            return Err("'query' must not be empty".to_string());
        }
        let uncancelled = std::sync::atomic::AtomicBool::new(false);
        crate::agent::club::Club::respond_cancellable(
            &*self.club,
            &grok_tool_prompt(query),
            cancel.unwrap_or(&uncancelled),
        )
        .map_err(|e| format!("grok_research: {e}"))
    }
}

/// Register `grok_research` when Grok OAuth CLI is configured and
/// `ANGEL_GROK_TOOL` isn't disabled. Absent config → not advertised, no error.
pub(crate) fn maybe_register_grok_research(r: &mut ToolRegistry) {
    if !env_flag("ANGEL_GROK_TOOL", true) {
        return;
    }
    if let Some(tool) = GrokResearchTool::from_env() {
        r.register(Box::new(tool));
    }
}

// ---------------------------------------------------------------------------
// web_fetch — direct HTTP GET of an arbitrary URL. web_search finds pages;
// this reads them. Without it the agent's only route to a page's *content*
// was `shell` + curl — functional, but invisible to models that reach for a
// first-class fetch tool. HTML is stripped to readable text; other content
// types are returned as-is (lossy UTF-8).
// ---------------------------------------------------------------------------

/// Strip an HTML document to readable text: drop `<script>`/`<style>` blocks,
/// remove tags, decode the common entities, and collapse whitespace runs.
/// Pure (no network) so it's unit-testable with canned markup.
pub(crate) fn html_to_text(html: &str) -> String {
    // Case-insensitive block removal by scanning a lowercased shadow copy —
    // ASCII lowercasing preserves byte offsets into the original.
    fn strip_blocks(s: &str, tag: &str) -> String {
        let lower = s.to_ascii_lowercase();
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        let mut out = String::with_capacity(s.len());
        let mut pos = 0;
        while let Some(start) = lower[pos..].find(&open) {
            let start = pos + start;
            out.push_str(&s[pos..start]);
            match lower[start..].find(&close) {
                Some(end) => pos = start + end + close.len(),
                None => return out, // unterminated block: drop the rest
            }
        }
        out.push_str(&s[pos..]);
        out
    }

    let cleaned = strip_blocks(&strip_blocks(html, "script"), "style");
    let mut text = String::with_capacity(cleaned.len());
    let mut in_tag = false;
    for c in cleaned.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                // Tags often separate words/blocks; keep them separated.
                text.push(' ');
            }
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    // Collapse whitespace: runs of spaces/tabs → one space, 3+ newlines → 2.
    let mut out = String::with_capacity(text.len());
    let mut spaces = 0usize;
    let mut newlines = 0usize;
    for c in text.chars() {
        if c == '\n' {
            newlines += 1;
            spaces = 0;
            if newlines <= 2 {
                out.push('\n');
            }
        } else if c.is_whitespace() {
            spaces += 1;
            if spaces == 1 && newlines == 0 {
                out.push(' ');
            }
        } else {
            spaces = 0;
            newlines = 0;
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// The declared charset from a `Content-Type` header (`text/html; charset=…`),
/// lowercased. `None` when the header is absent or carries no `charset`
/// parameter — distinct from an explicit `utf-8`, so an HTML `<meta charset>`
/// sniff can still take over.
fn header_charset(resp: &http_transport::Response) -> Option<String> {
    let ct = resp.header("Content-Type")?.to_ascii_lowercase();
    ct.split(';')
        .skip(1)
        .map(str::trim)
        .find_map(|p| p.strip_prefix("charset="))
        .map(|c| c.trim().trim_matches('"').to_string())
        .filter(|c| !c.is_empty())
}

/// Sniff a leading `<meta charset=…>` / `<meta http-equiv=… charset=…>` from the
/// head of an HTML document when the HTTP header declared no charset.
fn sniff_meta_charset(head: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    let idx = text.find("charset")?;
    let rest = text[idx + "charset".len()..]
        .trim_start_matches(|c: char| c == '=' || c == '"' || c == '\'' || c.is_whitespace());
    let end = rest
        .find(['"', '\'', ' ', ';', '>', '/'])
        .unwrap_or(rest.len());
    let label = &rest[..end];
    (!label.is_empty()).then(|| label.to_string())
}

/// Decode a response body to UTF-8. When a non-UTF-8 charset is declared (HTTP
/// header, or an HTML `<meta charset>` when the header is silent), transcode via
/// `encoding_rs`; otherwise fall back to lossy UTF-8. Returns the text plus an
/// optional note when a non-UTF-8 charset was applied or replacement characters
/// appeared, so a page's mojibake is flagged rather than trusted silently.
fn decode_body(raw: &[u8], charset: Option<&str>, html: bool) -> (String, Option<String>) {
    let label = charset.map(str::to_string).or_else(|| {
        if html {
            sniff_meta_charset(&raw[..raw.len().min(2048)])
        } else {
            None
        }
    });
    if let Some(label) = label
        && let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes())
        && enc != encoding_rs::UTF_8
    {
        let (text, _, had_errors) = enc.decode(raw);
        let note = if had_errors {
            Some(format!(
                "decoded as {}; replacement characters present",
                enc.name()
            ))
        } else {
            Some(format!("decoded as {}", enc.name()))
        };
        return (text.into_owned(), note);
    }
    match std::str::from_utf8(raw) {
        Ok(s) => (s.to_string(), None),
        Err(_) => (
            String::from_utf8_lossy(raw).into_owned(),
            Some("invalid UTF-8; replacement characters inserted".to_string()),
        ),
    }
}

/// A `Content-Encoding` the HTTP client did not transparently decode. `ureq`
/// strips the header for encodings it handled (gzip/br), so a value still
/// present and not `identity` means the body is raw compressed bytes.
fn undecoded_encoding(resp: &http_transport::Response) -> Option<String> {
    resp.header("Content-Encoding")
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty() && !e.eq_ignore_ascii_case("identity"))
}

pub(crate) struct WebFetchTool;
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "web_fetch".to_string(),
            description: research_description(
                "Fetch a URL over HTTP GET and return the body as text (HTML is \
                          stripped to readable text). Use to read documentation, APIs, or \
                          pages found via web_search.",
            ),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "http(s) URL to fetch" },
                    "max_bytes": {
                        "type": "integer",
                        "description": "cap on returned text bytes (default 20000, max 200000)"
                    },
                },
                "required": ["url"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let url = args["url"].as_str().ok_or("missing 'url'")?.trim();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("'url' must start with http:// or https://".to_string());
        }
        let max = args["max_bytes"]
            .as_u64()
            .unwrap_or(20_000)
            .clamp(1_000, 200_000) as usize;
        let resp = match http_transport::request("GET", url, true, 5).call() {
            Ok(r) => r,
            // A non-2xx response still has a useful status + body (error
            // pages, JSON error payloads) — return it rather than erroring.
            Err(http_transport::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("web_fetch: {url} unreachable ({e})")),
        };
        let status = resp.status();
        let ctype = resp.content_type().to_string();
        let charset = header_charset(&resp);
        let undecoded = undecoded_encoding(&resp);
        let mut raw = Vec::new();
        use std::io::Read;
        // Read a generous multiple of the cap: HTML shrinks a lot when
        // stripped, so a tight pre-cap would under-fill the text budget.
        resp.into_reader()
            .take(4 * max as u64)
            .read_to_end(&mut raw)
            .map_err(|e| format!("web_fetch: read failed for {url}: {e}"))?;
        let is_html = ctype.contains("html");
        let (text, decode_note) = decode_body(&raw, charset.as_deref(), is_html);
        let body = if is_html { html_to_text(&text) } else { text };
        // Surface anything that makes the text possibly-unusable so the model can
        // distinguish real content from a garbled body under a 200 status.
        let mut flags = String::new();
        if let Some(enc) = &undecoded {
            flags.push_str(&format!(" [undecoded Content-Encoding: {enc}]"));
        }
        if let Some(note) = &decode_note {
            flags.push_str(&format!(" [{note}]"));
        }
        let mut out = format!("[{status} {ctype}]{flags} {url}\n\n{body}");
        if out.len() > max {
            crate::agent::harness::truncate_to_char_boundary(&mut out, max);
            if max >= 200_000 {
                out.push_str(
                    "…[truncated — 200000-byte hard limit reached; fetch a more specific URL for the rest]",
                );
            } else {
                out.push_str("…[truncated — raise max_bytes for more]");
            }
        }
        Ok(out)
    }
}

/// Register `web_fetch`, gated on `ANGEL_WEB_FETCH` (default on).
pub(crate) fn maybe_register_web_fetch(r: &mut ToolRegistry) {
    if env_flag("ANGEL_WEB_FETCH", true) {
        r.register(Box::new(WebFetchTool));
    }
}

// ---------------------------------------------------------------------------
// http_request — full-verb HTTP for APIs. web_fetch reads pages (GET, HTML →
// text); this one *talks to services*: method + headers + body, raw response
// back. The REST surface his stack actually uses — local inference servers,
// Hydra mailboxes, HF hub, Vast.ai — is unreachable through a GET-only tool.
// ---------------------------------------------------------------------------

pub(crate) const HTTP_METHODS: [&str; 6] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"];

pub(crate) struct HttpRequestTool;
impl Tool for HttpRequestTool {
    fn name(&self) -> &str {
        "http_request"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "http_request".to_string(),
            description: "Make an HTTP request with full control: method, headers, body. \
                          Returns status + raw body text (no HTML stripping). Use for JSON \
                          APIs — local inference servers, webhooks, REST services; use \
                          web_fetch for reading pages. A body with no explicit Content-Type \
                          is sent as application/json."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "method": {
                        "type": "string",
                        "enum": HTTP_METHODS,
                        "description": "HTTP method (default GET)"
                    },
                    "url": { "type": "string", "description": "http(s) URL" },
                    "headers": {
                        "type": "object",
                        "description": "header name → value",
                        "additionalProperties": { "type": "string" }
                    },
                    "body": { "type": "string", "description": "request body (verbatim)" },
                    "max_bytes": {
                        "type": "integer",
                        "description": "cap on returned body bytes (default 20000, max 200000)"
                    },
                },
                "required": ["url"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let url = args["url"].as_str().ok_or("missing 'url'")?.trim();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("'url' must start with http:// or https://".to_string());
        }
        let method = args["method"].as_str().unwrap_or("GET").to_uppercase();
        if !HTTP_METHODS.contains(&method.as_str()) {
            return Err(format!(
                "unsupported method '{method}' (one of {})",
                HTTP_METHODS.join(", ")
            ));
        }
        if !http_method_allowed(
            &method,
            crate::platform::yolo::enabled() || env_flag(HTTP_MUTATIONS_ENV, false),
        ) {
            return Err(format!(
                "HTTP method {method} can mutate remote state and is disabled by default; set {HTTP_MUTATIONS_ENV}=1 for an intentional run"
            ));
        }
        let max = args["max_bytes"]
            .as_u64()
            .unwrap_or(20_000)
            .clamp(1_000, 200_000) as usize;
        // Mutating and credential-bearing calls never follow redirects.
        // Public reads may redirect, but every hop is still resolved through
        // the same outbound-address policy.
        let headers = args["headers"].as_object();
        let redirects = http_request_redirect_limit(&method, headers);
        let mut req = http_transport::request(&method, url, true, redirects);
        let mut has_ctype = false;
        if let Some(headers) = headers {
            for (k, v) in headers {
                let v = v
                    .as_str()
                    .ok_or_else(|| format!("header '{k}' not a string"))?;
                if k.eq_ignore_ascii_case("content-type") {
                    has_ctype = true;
                }
                req = req.set(k, v);
            }
        }
        let body = args["body"].as_str();
        if body.is_some() && !has_ctype {
            req = req.set("Content-Type", "application/json");
        }
        let result = match body {
            Some(b) => req.send_string(b),
            None => req.call(),
        };
        let resp = match result {
            Ok(r) => r,
            // Non-2xx bodies carry the API's actual error — return them.
            Err(http_transport::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("http_request: {url} unreachable ({e})")),
        };
        let status = resp.status();
        let ctype = resp.content_type().to_string();
        // gzip/br are decoded transparently by the client; a Content-Encoding
        // still present means the body is raw compressed bytes we can't read.
        let undecoded = undecoded_encoding(&resp);
        let mut raw = Vec::new();
        use std::io::Read;
        resp.into_reader()
            .take(max as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|e| format!("http_request: read failed for {url}: {e}"))?;
        let truncated = raw.len() > max;
        raw.truncate(max);
        let flags = undecoded
            .map(|e| format!(" [undecoded Content-Encoding: {e}]"))
            .unwrap_or_default();
        let mut out = format!(
            "[{status} {ctype}]{flags} {method} {url}\n\n{}",
            String::from_utf8_lossy(&raw)
        );
        if truncated {
            out.push_str("…[truncated — raise max_bytes for more]");
        }
        Ok(out)
    }
}

/// Register `http_request`, gated on `ANGEL_HTTP_REQUEST` (default on).
pub(crate) fn maybe_register_http_request(r: &mut ToolRegistry) {
    if env_flag("ANGEL_HTTP_REQUEST", true) {
        r.register(Box::new(HttpRequestTool));
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/web__decode_tests.rs"]
mod decode_tests;
