//! `llm_probe` / `llm_bench` — OpenAI-compatible inference endpoint recon and
//! measured single-stream benchmarks.
//!
//! The fleet's serving stack (llama.cpp, vLLM, SGLang, …) all speaks the same
//! `/v1` dialect; these tools give the agent a first-class way to answer the
//! two questions every serving session starts with: *is it up, and what does
//! it serve?* (`llm_probe`) and *how fast is it, measured?* (`llm_bench` —
//! streaming TTFT + decode tok/s, multiple runs, honest about approximation).
//! Meant for local/tailnet endpoints; nothing here spends paid tokens unless
//! the operator points it at a paid URL.

use crate::club::ToolDef;
use crate::harness::{Tool, ToolRegistry, env_flag};
use serde_json::Value;
use std::io::BufRead;
#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

/// Normalize a user-supplied endpoint to a base URL: trailing slashes and a
/// trailing `/v1` are stripped, so `http://h:8000`, `http://h:8000/`, and
/// `http://h:8000/v1` all mean the same server. Pure → testable.
pub(crate) fn normalize_base(url: &str) -> String {
    let mut u = url.trim().trim_end_matches('/').to_string();
    if u.to_ascii_lowercase().ends_with("/v1") {
        u.truncate(u.len() - 3);
    }
    u.trim_end_matches('/').to_string()
}

fn bearer(
    req: super::http_transport::Request,
    key: Option<&str>,
) -> super::http_transport::Request {
    match key {
        Some(k) if !k.trim().is_empty() => req.set("Authorization", &format!("Bearer {k}")),
        _ => req,
    }
}

/// Model ids from a `/v1/models` response body. Pure → testable.
pub(crate) fn model_ids(v: &Value) -> Vec<String> {
    v.get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|i| i.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn fetch_models(base: &str, key: Option<&str>) -> Result<Vec<String>, String> {
    let url = format!("{base}/v1/models");
    let resp = bearer(super::http_transport::request("GET", &url, false, 5), key)
        .call()
        .map_err(|e| format!("{url} unreachable ({e})"))?;
    let v: Value = resp
        .into_json()
        .map_err(|e| format!("{url}: bad JSON ({e})"))?;
    Ok(model_ids(&v))
}

// ---------------------------------------------------------------------------
// llm_probe
// ---------------------------------------------------------------------------

pub(crate) struct LlmProbeTool;

impl Tool for LlmProbeTool {
    fn name(&self) -> &str {
        "llm_probe"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "llm_probe".to_string(),
            description: "Check an OpenAI-compatible inference endpoint (llama.cpp, vLLM, \
                          SGLang, …): GET /v1/models and report the served model ids. Use \
                          after starting a server with proc_run to confirm it's ready, or to \
                          discover what a fleet box serves."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "endpoint base, e.g. http://host:8000 (with or without /v1)"
                    },
                    "api_key": { "type": "string", "description": "authentication credential if required" },
                },
                "required": ["url"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let url = args["url"].as_str().ok_or("missing 'url'")?;
        let base = normalize_base(url);
        if !base.starts_with("http://") && !base.starts_with("https://") {
            return Err("'url' must start with http:// or https://".to_string());
        }
        let ids =
            fetch_models(&base, args["api_key"].as_str()).map_err(|e| format!("llm_probe: {e}"))?;
        if ids.is_empty() {
            Ok(format!("{base} is up, but /v1/models lists no models"))
        } else {
            Ok(format!(
                "{base} is up — {} model(s):\n{}",
                ids.len(),
                ids.join("\n")
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// llm_bench
// ---------------------------------------------------------------------------

/// One streamed generation's measurements.
pub(crate) struct RunStats {
    pub ttft_ms: f64,
    pub decode_tps: f64,
    pub tokens: usize,
    pub total_s: f64,
    /// Whether `tokens` came from the server's reported usage (exact) or from
    /// counting SSE content chunks (≈ tokens on most servers).
    pub exact: bool,
}

/// Does this SSE chunk carry a generated token (content or reasoning delta)?
/// Pure → testable.
pub(crate) fn chunk_has_token(v: &Value) -> bool {
    let delta = &v["choices"][0]["delta"];
    for key in ["content", "reasoning_content"] {
        if delta[key].as_str().is_some_and(|s| !s.is_empty()) {
            return true;
        }
    }
    false
}

fn bench_one(
    base: &str,
    model: &str,
    prompt: &str,
    max_tokens: u64,
    key: Option<&str>,
) -> Result<RunStats, String> {
    let url = format!("{base}/v1/chat/completions");
    let mut body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": max_tokens,
        "temperature": 0,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    let t0 = Instant::now();
    let resp = match bearer(super::http_transport::request("POST", &url, false, 5), key)
        .send_json(body.clone())
    {
        Ok(r) => r,
        // A strict server may reject stream_options — retry once without it
        // (we then fall back to chunk-counting for the token count).
        Err(super::http_transport::Error::Status(400, _)) => {
            body.as_object_mut().unwrap().remove("stream_options");
            bearer(super::http_transport::request("POST", &url, false, 5), key)
                .send_json(body)
                .map_err(|e| format!("{url}: {e}"))?
        }
        Err(super::http_transport::Error::Status(code, r)) => {
            let detail = r.into_string().map_err(|e| e.to_string())?;
            return Err(format!("{url}: HTTP {code}: {}", detail.trim()));
        }
        Err(e) => return Err(format!("{url} unreachable ({e})")),
    };
    let reader = std::io::BufReader::new(resp.into_reader());
    let mut first: Option<Instant> = None;
    let mut last = t0;
    let mut chunks = 0usize;
    let mut usage_tokens: Option<usize> = None;
    for line in reader.lines() {
        let line = line.map_err(|e| format!("stream read: {e}"))?;
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if let Some(u) = v
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|n| n.as_u64())
        {
            usage_tokens = Some(u as usize);
        }
        if chunk_has_token(&v) {
            chunks += 1;
            let now = Instant::now();
            if first.is_none() {
                first = Some(now);
            }
            last = now;
        }
    }
    let first = first.ok_or("stream produced no content tokens")?;
    let tokens = usage_tokens.unwrap_or(chunks);
    let decode_window = last.duration_since(first).as_secs_f64();
    let decode_tps = if decode_window > 0.0 && tokens > 1 {
        (tokens - 1) as f64 / decode_window
    } else {
        0.0
    };
    Ok(RunStats {
        ttft_ms: first.duration_since(t0).as_secs_f64() * 1000.0,
        decode_tps,
        tokens,
        total_s: last.duration_since(t0).as_secs_f64(),
        exact: usage_tokens.is_some(),
    })
}

pub(crate) struct LlmBenchTool;

impl Tool for LlmBenchTool {
    fn name(&self) -> &str {
        "llm_bench"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: "llm_bench".to_string(),
            description: "Measure an OpenAI-compatible endpoint's single-stream speed: streamed \
                          runs reporting TTFT (prefill) and decode tok/s per run plus the mean. \
                          The first run is warmup and excluded from the mean. Meant for \
                          local/self-hosted endpoints — pointing it at a paid API spends real \
                          tokens."
                .to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "endpoint base, e.g. http://host:8000 (with or without /v1)"
                    },
                    "model": {
                        "type": "string",
                        "description": "model id (default: first id from /v1/models)"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "fixed prompt (default: a ~40-token instruction)"
                    },
                    "max_tokens": { "type": "integer", "description": "per run (default 128)" },
                    "runs": {
                        "type": "integer",
                        "description": "measured runs after warmup (default 3, max 10)"
                    },
                    "api_key": { "type": "string", "description": "authentication credential if required" },
                },
                "required": ["url"],
            }),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        let url = args["url"].as_str().ok_or("missing 'url'")?;
        let base = normalize_base(url);
        if !base.starts_with("http://") && !base.starts_with("https://") {
            return Err("'url' must start with http:// or https://".to_string());
        }
        let key = args["api_key"].as_str();
        let model = match args["model"].as_str() {
            Some(m) => m.to_string(),
            None => fetch_models(&base, key)
                .map_err(|e| format!("llm_bench: {e}"))?
                .into_iter()
                .next()
                .ok_or("llm_bench: /v1/models lists no models — pass 'model' explicitly")?,
        };
        let prompt = args["prompt"].as_str().unwrap_or(
            "Explain, in about two hundred words, how speculative decoding accelerates \
             autoregressive inference. Plain prose, no lists.",
        );
        let max_tokens = args["max_tokens"].as_u64().unwrap_or(128).clamp(16, 4096);
        let runs = args["runs"].as_u64().unwrap_or(3).clamp(1, 10) as usize;

        let mut lines = vec![format!("llm_bench @ {base} — model {model}")];
        let warm = bench_one(&base, &model, prompt, max_tokens, key)
            .map_err(|e| format!("llm_bench (warmup): {e}"))?;
        lines.push(format!(
            "warmup: TTFT {:.0} ms · decode {:.1} tok/s · {} tok in {:.2} s (excluded)",
            warm.ttft_ms, warm.decode_tps, warm.tokens, warm.total_s
        ));
        let mut measured: Vec<RunStats> = Vec::new();
        for i in 0..runs {
            let s = bench_one(&base, &model, prompt, max_tokens, key)
                .map_err(|e| format!("llm_bench (run {}): {e}", i + 1))?;
            lines.push(format!(
                "run {}: TTFT {:.0} ms · decode {:.1} tok/s · {} tok in {:.2} s",
                i + 1,
                s.ttft_ms,
                s.decode_tps,
                s.tokens,
                s.total_s
            ));
            measured.push(s);
        }
        let n = measured.len() as f64;
        let mean_ttft = measured.iter().map(|s| s.ttft_ms).sum::<f64>() / n;
        let mean_tps = measured.iter().map(|s| s.decode_tps).sum::<f64>() / n;
        let exact = measured.iter().all(|s| s.exact);
        lines.push(format!(
            "mean of {runs}: TTFT {:.0} ms · decode {:.1} tok/s (temperature 0, max_tokens \
             {max_tokens}; tokens {})",
            mean_ttft,
            mean_tps,
            if exact {
                "from server usage"
            } else {
                "≈ SSE chunk count"
            }
        ));
        Ok(lines.join("\n"))
    }
}

/// Register the LLM endpoint tools, gated on `ANGEL_LLM_TOOLS` (default on).
pub(crate) fn maybe_register_llm_tools(r: &mut ToolRegistry) {
    if env_flag("ANGEL_LLM_TOOLS", true) {
        r.register(Box::new(LlmProbeTool));
        r.register(Box::new(LlmBenchTool));
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/tools/llm__tests.rs"]
mod tests;
