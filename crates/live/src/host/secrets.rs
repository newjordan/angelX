//! Shared secret redaction for session history, experience, trajectory and Cut
//! persistence. Exact-body rollout stores use the same detection to reject
//! sensitive payloads before hashing/writing; they never rewrite sealed data.
//!
//! Known values and provider shapes are checked in decoded strings. Structured
//! records are walked before serialization, preserving JSON types and escaping.
//!
//! 1. **Known values.** Every environment variable whose name looks secret
//!    (`KEY`, `TOKEN`, `SECRET`, `AUTH`, `PASSWORD` — the same rule
//!    [`is_secret_name`] uses for child-env stripping) and
//!    whose value is at least [`MIN_SECRET_LEN`] bytes is replaced by
//!    `«redacted:NAME»`. The name survives so a trainer can still see *which*
//!    credential a tool call used; the value never does.
//! 2. **Secret shapes.** Provider key prefixes (`sk-…`, z.ai `hex32.suffix`,
//!    GitHub `ghp_…`, Slack `xox…`, AWS `AKIA…`), bearer tokens, JWTs and PEM
//!    private-key blocks are replaced by `«redacted»` even when the value is
//!    not in this process's environment (pasted by the user, read from a
//!    file, printed by a tool).
//!
//! Over-redaction is the accepted trade, exactly as in the cockpit's `knowledge::barrel`:
//! these records feed training and audit; a shredded token is always safer
//! than a leaked one. The environment is read on every call (a few dozen
//! variables) so tests and late-loaded credentials are seen without a cache
//! to invalidate.

use regex::Regex;
use std::borrow::Cow;
use std::sync::OnceLock;

pub const MIN_SECRET_LEN: usize = 12;

pub const REDACTED: &str = "«redacted»";

fn shape_patterns() -> &'static [Regex] {
    static SHAPES: OnceLock<Vec<Regex>> = OnceLock::new();
    SHAPES.get_or_init(|| {
        [
            r"\bsk-[A-Za-z0-9_-]{16,}",
            r"\b[0-9a-f]{32}\.[A-Za-z0-9]{12,}\b",
            r"\bgh[pousr]_[A-Za-z0-9]{30,}\b",
            r"\bgithub_pat_[A-Za-z0-9_]{20,}\b",
            r"\bhf_[A-Za-z0-9]{20,}\b",
            r"\bAIza[A-Za-z0-9_-]{30,}\b",
            r"\bxox[abpr]-[A-Za-z0-9-]{10,}",
            r"\bAKIA[0-9A-Z]{16}\b",
            r"\beyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}(?:\.[A-Za-z0-9_-]{10,})?",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("secret shape pattern"))
        .collect()
    })
}

fn bearer_pattern() -> &'static Regex {
    static BEARER: OnceLock<Regex> = OnceLock::new();
    BEARER.get_or_init(|| Regex::new(r"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]{20,}").expect("bearer"))
}

fn assignment_pattern() -> &'static Regex {
    static ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    ASSIGNMENT.get_or_init(|| {
        Regex::new(r#"(?i)(\b(?:[a-z0-9]+_)*(?:api[_-]?key|access_token|refresh_token|client_secret|token|password|passwd|secret)\s*[=:]\s*)(?:"[^"\r\n]+"|'[^'\r\n]+'|[^\s,;\}\]]+)"#)
            .expect("credential assignment")
    })
}

fn credential_field(name: &str) -> bool {
    let name = name.to_ascii_lowercase().replace('-', "_");
    matches!(
        name.as_str(),
        "token"
            | "secret"
            | "password"
            | "passwd"
            | "authorization"
            | "api_key"
            | "apikey"
            | "auth"
    ) || [
        "_api_key",
        "_access_token",
        "_refresh_token",
        "_client_secret",
        "_password",
    ]
    .iter()
    .any(|suffix| name.ends_with(suffix))
        || matches!(
            name.as_str(),
            "access_token" | "refresh_token" | "client_secret"
        )
}

pub fn is_secret_name(name: &str) -> bool {
    let up = name.to_ascii_uppercase();
    ["KEY", "TOKEN", "SECRET", "AUTH", "PASSWORD"]
        .iter()
        .any(|p| up.contains(p))
}

pub fn secret_values() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(name, value)| {
            value.len() >= MIN_SECRET_LEN
                && is_secret_name(name)
                && !value.starts_with("«redacted")
                && !value.starts_with(['/', '~', '.'])
        })
        .collect();
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    out
}

pub fn redact_str(text: &str) -> Cow<'_, str> {
    Redactor::new().redact_str(text)
}

pub struct Redactor {
    values: Vec<(String, String)>,
}

impl Redactor {
    pub fn new() -> Self {
        Self {
            values: secret_values(),
        }
    }

    pub fn redact_str<'a>(&self, text: &'a str) -> Cow<'a, str> {
        redact_with_values(text, &self.values)
    }
}

fn redact_with_values<'a>(text: &'a str, values: &[(String, String)]) -> Cow<'a, str> {
    if matches!(
        text.trim_start().as_bytes().first(),
        Some(b'{' | b'[' | b'"')
    ) && let Ok(mut nested) = serde_json::from_str(text)
        && redact_value_with(&mut nested, values, false) > 0
    {
        return Cow::Owned(serde_json::to_string(&nested).expect("JSON value serialization"));
    }
    let mut out: Cow<'_, str> = Cow::Borrowed(text);
    for (name, value) in values {
        if out.contains(value.as_str()) {
            let marker = format!("«redacted:{name}»");
            out = Cow::Owned(out.replace(value.as_str(), &marker));
        }
    }
    for shape in shape_patterns() {
        if shape.is_match(&out) {
            out = Cow::Owned(shape.replace_all(&out, REDACTED).into_owned());
        }
    }
    if bearer_pattern().is_match(&out) {
        out = Cow::Owned(
            bearer_pattern()
                .replace_all(&out, format!("${{1}}{REDACTED}").as_str())
                .into_owned(),
        );
    }
    if assignment_pattern().is_match(&out) {
        out = Cow::Owned(
            assignment_pattern()
                .replace_all(&out, |caps: &regex::Captures<'_>| {
                    if caps[0].contains("«redacted") || caps[0].ends_with('…') {
                        caps[0].to_string()
                    } else {
                        format!("{}{REDACTED}", &caps[1])
                    }
                })
                .into_owned(),
        );
    }
    out
}

pub fn redact_value(value: &mut serde_json::Value) -> usize {
    redact_value_with(value, &secret_values(), false)
}

fn redact_value_with(
    value: &mut serde_json::Value,
    values: &[(String, String)],
    credential: bool,
) -> usize {
    match value {
        serde_json::Value::String(text) => {
            if credential && !text.is_empty() && !text.starts_with("«redacted") {
                *text = REDACTED.to_string();
                return 1;
            }
            match redact_with_values(text, values) {
                Cow::Borrowed(_) => 0,
                Cow::Owned(clean) if clean == *text => 0,
                Cow::Owned(clean) => {
                    let hits = clean.matches("«redacted").count();
                    *text = clean;
                    hits
                }
            }
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| redact_value_with(item, values, credential))
            .sum(),
        serde_json::Value::Object(object) => object
            .iter_mut()
            .map(|(name, value)| redact_value_with(value, values, credential_field(name)))
            .sum(),
        _ => 0,
    }
}

pub fn to_redacted_vec(value: &(impl serde::Serialize + ?Sized)) -> serde_json::Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    redact_value(&mut value);
    serde_json::to_vec(&value)
}

pub fn to_redacted_vec_pretty(
    value: &(impl serde::Serialize + ?Sized),
) -> serde_json::Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    redact_value(&mut value);
    serde_json::to_vec_pretty(&value)
}

pub fn redact_bytes(bytes: &[u8]) -> Vec<u8> {
    if let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(bytes) {
        if redact_value(&mut value) == 0 {
            return bytes.to_vec();
        }
        let encoded = if bytes.contains(&b'\n') {
            serde_json::to_vec_pretty(&value)
        } else {
            serde_json::to_vec(&value)
        };
        return encoded.unwrap_or_else(|_| bytes.to_vec());
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => match redact_str(text) {
            Cow::Borrowed(_) => bytes.to_vec(),
            Cow::Owned(clean) => clean.into_bytes(),
        },
        Err(_) => bytes.to_vec(),
    }
}

pub fn redact_error(text: &str) -> String {
    redact_str(text).into_owned()
}

pub fn contains_secret(text: &str) -> bool {
    if let Ok(mut value) = serde_json::from_str(text) {
        redact_value(&mut value) > 0
    } else {
        redact_str(text) != text
    }
}
