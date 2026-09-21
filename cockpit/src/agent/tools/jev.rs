//! Bounded TypeSafe/Jev decisions. These are model estimates, never verifier receipts.
use crate::agent::club::ToolDef;
use crate::agent::harness::{Tool, ToolRegistry, env_flag};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Read;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const MAX_REQUEST: usize = 32_768;
const MAX_RESPONSE: usize = 65_536;
const MAX_QUESTIONS: usize = 8;
const MAX_OPTIONS: usize = 16;
const CACHE_ENTRIES: usize = 32;
const CACHE_TTL: Duration = Duration::from_secs(300);

struct Session {
    attempts: usize,
    cache: VecDeque<(String, Instant, Value)>,
}

pub(crate) struct JevTool {
    key: String,
    model: String,
    endpoint: String,
    timeout: Duration,
    max_calls: usize,
    session: Mutex<Session>,
}

pub(crate) fn available() -> bool {
    env_flag("ANGEL_JEV", true)
        && std::env::var("TYPESAFE_API_KEY").is_ok_and(|key| !key.trim().is_empty())
}

fn setting(name: &str, default: u64, min: u64, max: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

impl JevTool {
    fn from_env() -> Result<Self, String> {
        if !available() {
            return Err(
                "jev_decide unavailable: set TYPESAFE_API_KEY; ANGEL_JEV=0 disables it".into(),
            );
        }
        let key = std::env::var("TYPESAFE_API_KEY")
            .unwrap_or_default()
            .trim()
            .to_owned();
        if key.len() > 4096 || key.chars().any(char::is_control) {
            return Err("invalid TYPESAFE_API_KEY".into());
        }
        let model = std::env::var("ANGEL_JEV_MODEL").unwrap_or_else(|_| "jev-latest".into());
        if model.is_empty()
            || model.len() > 80
            || !model
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
        {
            return Err("invalid ANGEL_JEV_MODEL".into());
        }
        Ok(Self {
            key,
            model,
            endpoint: ENDPOINT.into(),
            timeout: Duration::from_millis(setting("ANGEL_JEV_TIMEOUT_MS", 5000, 250, 30_000)),
            max_calls: setting("ANGEL_JEV_MAX_CALLS", 64, 1, 256) as usize,
            session: Mutex::new(Session {
                attempts: 0,
                cache: VecDeque::new(),
            }),
        })
    }

    fn decide(&self, args: &Value) -> Result<Value, String> {
        let request = request_body(args, &self.model)?;
        let body = serde_json::to_string(&request).map_err(|_| "invalid Jev request")?;
        if body.len() > MAX_REQUEST {
            return Err("Jev request exceeds 32768 bytes; narrow the evidence".into());
        }
        // A tool argument must never accidentally become an authentication channel.
        if body.contains(&self.key) || crate::platform::secrets::contains_secret(&body) {
            return Err("Jev evidence contains a credential; remove it before sending".into());
        }
        let fingerprint = crate::knowledge::cut::sha256_hex(body.as_bytes());
        let mut session = self.session.lock().map_err(|_| "Jev session unavailable")?;
        if let Some((_, _, output)) = session
            .cache
            .iter()
            .find(|(hash, at, _)| hash == &fingerprint && at.elapsed() < CACHE_TTL)
        {
            let mut output = output.clone();
            output["cached"] = json!(true);
            output["latency_ms"] = json!(0);
            output["cached_provider_usage"] = output["usage"].clone();
            output["usage"] = json!({"input_tokens":0,"output_tokens":0});
            output["requests_remaining"] = json!(self.max_calls - session.attempts);
            return Ok(output);
        }
        if session.attempts >= self.max_calls {
            return Err(
                "Jev session request budget exhausted; continue with local evidence".into(),
            );
        }
        session.attempts += 1;
        let start = Instant::now();
        let payload = super::http_transport::with_timeout(self.timeout, || {
            let response = super::http_transport::request("POST", &self.endpoint, false, 0)
                .set("authorization", &format!("Bearer {}", self.key))
                .set("content-type", "application/json")
                .send_string(&body)
                .map_err(|error| match error {
                    super::http_transport::Error::Status(status, _) => {
                        format!("Jev HTTP {status}; no automatic retry")
                    }
                    // Provider errors may echo headers or submitted state. Never forward them.
                    super::http_transport::Error::Transport(_) => {
                        "Jev transport failed, timed out, or was cancelled".into()
                    }
                })?;
            if response.status() != 200 {
                return Err(format!(
                    "Jev unexpected HTTP {}; redirects are disabled",
                    response.status()
                ));
            }
            let mut bytes = Vec::new();
            response
                .into_reader()
                .take((MAX_RESPONSE + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| "Jev response read failed or was cancelled")?;
            if bytes.len() > MAX_RESPONSE {
                return Err("Jev response exceeds 65536 bytes".into());
            }
            serde_json::from_slice::<Value>(&bytes).map_err(|_| "Jev returned invalid JSON".into())
        })?;
        let mut output = normalize_response(&request, &payload)?;
        output["request_sha256"] = json!(fingerprint);
        output["latency_ms"] = json!(start.elapsed().as_millis());
        output["cached"] = json!(false);
        output["requests_remaining"] = json!(self.max_calls - session.attempts);
        if session.cache.len() == CACHE_ENTRIES {
            session.cache.pop_front();
        }
        session
            .cache
            .push_back((fingerprint, Instant::now(), output.clone()));
        Ok(output)
    }
}

fn text<'a>(value: &'a Value, name: &str, max: usize) -> Result<&'a str, String> {
    value
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= max)
        .ok_or_else(|| format!("{name} must be nonempty text of at most {max} bytes"))
}

fn request_body(args: &Value, model: &str) -> Result<Value, String> {
    let state = text(&args["state"], "state", 24_576)?;
    let questions = args["questions"]
        .as_array()
        .filter(|q| !q.is_empty() && q.len() <= MAX_QUESTIONS)
        .ok_or("questions must contain 1..8 questions")?;
    let mut mapped = serde_json::Map::new();
    for question in questions {
        let id = text(&question["id"], "question id", 64)?;
        if !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') || mapped.contains_key(id) {
            return Err("question ids must be unique ASCII letters, digits, or underscores".into());
        }
        let instructions = text(&question["instructions"], "instructions", 2048)?;
        let kind = question["type"].as_str().ok_or("question type required")?;
        let mut item = json!({"type": kind, "instructions": instructions});
        match kind {
            "noul" => {
                if question
                    .get("options")
                    .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
                {
                    return Err("noul has no options; ask a yes/no question".into());
                }
            }
            "choice" | "score" => {
                let options = question["options"]
                    .as_array()
                    .filter(|o| (2..=MAX_OPTIONS).contains(&o.len()))
                    .ok_or("choice and score require 2..16 ordered options")?;
                let mut criteria = serde_json::Map::new();
                for option in options {
                    let label = text(option, "option", 256)?;
                    if criteria.insert(label.into(), Value::Null).is_some() {
                        return Err("options must be distinct".into());
                    }
                }
                item["criteria"] = if kind == "choice" {
                    Value::Object(criteria)
                } else {
                    json!(options)
                };
            }
            _ => return Err("type must be noul, choice, or score".into()),
        }
        mapped.insert(id.into(), item);
    }
    Ok(json!({"model": model, "state": state, "questions": mapped}))
}

fn number(value: &Value, min: f64, max: f64) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|v| v.is_finite() && *v >= min && *v <= max)
        .ok_or_else(|| "Jev returned an invalid numeric answer".into())
}

fn normalize_response(request: &Value, payload: &Value) -> Result<Value, String> {
    let model = text(&payload["model"], "Jev response model", 80)?;
    if !model
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
    {
        return Err("Jev returned an invalid model identifier".into());
    }
    let questions = request["questions"]
        .as_object()
        .ok_or("invalid questions")?;
    let answers = payload["answers"]
        .as_object()
        .filter(|a| a.len() == questions.len())
        .ok_or("Jev response question count mismatch")?;
    let mut normalized = serde_json::Map::new();
    for (id, question) in questions {
        let answer = answers
            .get(id)
            .ok_or("Jev response is missing a question")?;
        if answer["type"] != question["type"] {
            return Err("Jev response type mismatch".into());
        }
        let kind = question["type"].as_str().ok_or("invalid question type")?;
        let mut row = json!({"type": kind});
        if kind == "noul" {
            let probability = number(&answer["noul"], 0.0, 1.0)?;
            row["probability"] = json!(probability);
            row["probability_percent"] = json!(100.0 * probability);
        } else {
            let labels: Vec<String> = if kind == "choice" {
                question["criteria"]
                    .as_object()
                    .ok_or("invalid choices")?
                    .keys()
                    .cloned()
                    .collect()
            } else {
                (0..question["criteria"]
                    .as_array()
                    .ok_or("invalid levels")?
                    .len())
                    .map(|i| i.to_string())
                    .collect()
            };
            let probabilities = answer["probabilities"]
                .as_object()
                .filter(|p| p.len() == labels.len())
                .ok_or("Jev probability labels mismatch")?;
            let mut sum = 0.0;
            let mut expected_score = 0.0;
            let mut percentages = serde_json::Map::new();
            for (index, label) in labels.iter().enumerate() {
                let p = number(
                    probabilities
                        .get(label)
                        .ok_or("Jev probability label missing")?,
                    0.0,
                    1.0,
                )?;
                sum += p;
                expected_score += index as f64 * p;
                percentages.insert(label.clone(), json!(100.0 * p));
            }
            if (sum - 1.0).abs() > 0.01 {
                return Err("Jev probabilities do not sum to one".into());
            }
            row["confidence_percent"] = json!(100.0 * number(&answer["confidence"], 0.0, 1.0)?);
            row["probabilities_percent"] = json!(percentages);
            if kind == "choice" {
                let chosen = answer["choice"]
                    .as_str()
                    .filter(|c| labels.iter().any(|l| l == c))
                    .ok_or("Jev returned an unknown choice")?;
                let chosen_probability = number(&probabilities[chosen], 0.0, 1.0)?;
                if probabilities
                    .values()
                    .any(|p| p.as_f64().unwrap_or(0.0) > chosen_probability + 0.001)
                {
                    return Err("Jev choice disagrees with its probabilities".into());
                }
                row["choice"] = json!(chosen);
            } else {
                let max = (labels.len() - 1) as f64;
                let score = number(&answer["score"], 0.0, max)?;
                if (score - expected_score).abs() > 0.03 {
                    return Err("Jev score disagrees with its probabilities".into());
                }
                row["score"] = json!(score);
                row["levels"] = question["criteria"].clone();
                row["rubric_position_percent"] = json!(score / max * 100.0);
            }
        }
        normalized.insert(id.clone(), row);
    }
    let input = payload["usage"]["input_tokens"]
        .as_u64()
        .ok_or("Jev usage missing input_tokens")?;
    let output = payload["usage"]["output_tokens"]
        .as_u64()
        .ok_or("Jev usage missing output_tokens")?;
    Ok(json!({
        "schema": "angel.jev-decision/v1", "source": "model_estimate", "model": model,
        "answers": normalized, "usage": {"input_tokens": input, "output_tokens": output},
        "interpretation": "Advisory estimates from supplied evidence. Probability and confidence are not measured benchmark gains or correctness. Rubric position is a score, not a probability. Independent tests and evaluator receipts remain authoritative."
    }))
}

impl Tool for JevTool {
    fn name(&self) -> &str {
        "jev_decide"
    }
    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.name().into(),
            description: "Ask Jev fast typed technical questions about explicit evidence: defect triage, hypothesis ranking, regression risk, or next diagnostic. Returns advisory probability percentages, choice confidence, rubric scores, latency and token usage. Batch independent questions about the same state. Never use as a verifier or as measured benchmark improvement; use benchmark_compare for measured percentages. Sends only supplied state/questions to TypeSafe; omit secrets. No automatic retries.".into(),
            params: json!({"type":"object", "properties": {
                "state": {"type":"string", "maxLength":24576, "description":"Concise evidence, code excerpts or measurement records. Include dataset, source and uncertainty. No credentials."},
                "questions": {"type":"array", "minItems":1, "maxItems":8, "items":{
                    "type":"object", "properties":{
                        "id":{"type":"string", "maxLength":64},
                        "type":{"type":"string", "enum":["noul","choice","score"]},
                        "instructions":{"type":"string", "maxLength":2048, "description":"Explicit question; the model does not see the question id."},
                        "options":{"type":"array", "maxItems":16, "items":{"type":"string", "maxLength":256}, "description":"For choice: distinct candidate descriptions. For score: ordered rubric levels from 0 upwards. Omit for noul."}
                    }, "required":["id","type","instructions"], "additionalProperties":false
                }}
            }, "required":["state","questions"], "additionalProperties":false}),
        }
    }
    fn call(&self, args: &Value) -> Result<String, String> {
        self.decide(args).and_then(|result| {
            serde_json::to_string(&result).map_err(|_| "invalid Jev result".into())
        })
    }
}

pub(crate) fn maybe_register_jev(r: &mut ToolRegistry) {
    if available() {
        match JevTool::from_env() {
            Ok(tool) => r.register_deferred(Box::new(tool)),
            Err(error) => eprintln!("angelX: {error}"),
        }
    }
    r.register_deferred(Box::new(super::benchmark::BenchmarkCompareTool));
}

/// Direct runner access without starting another generative model turn.
pub(crate) fn run_cli() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .take((MAX_REQUEST + 1) as u64)
        .read_to_string(&mut input)
        .map_err(|_| "could not read Jev input")?;
    if input.len() > MAX_REQUEST {
        return Err("Jev CLI input exceeds 32768 bytes".into());
    }
    let args: Value = serde_json::from_str(&input).map_err(|_| "invalid Jev input JSON")?;
    println!("{}", JevTool::from_env()?.call(&args)?);
    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/tools/jev__tests.rs"]
mod tests;
