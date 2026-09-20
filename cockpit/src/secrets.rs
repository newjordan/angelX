//! Shared secret redaction for session history, experience, trajectory and Cut
//! persistence. Exact-body rollout stores use the same detection to reject
//! sensitive payloads before hashing/writing; they never rewrite sealed data.
//!
//! Known values and provider shapes are checked in decoded strings. Structured
//! records are walked before serialization, preserving JSON types and escaping.
//!
//! 1. **Known values.** Every environment variable whose name looks secret
//!    (`KEY`, `TOKEN`, `SECRET`, `AUTH`, `PASSWORD` — the same rule
//!    [`crate::experience::is_secret_name`] uses for child-env stripping) and
//!    whose value is at least [`MIN_SECRET_LEN`] bytes is replaced by
//!    `«redacted:NAME»`. The name survives so a trainer can still see *which*
//!    credential a tool call used; the value never does.
//! 2. **Secret shapes.** Provider key prefixes (`sk-…`, z.ai `hex32.suffix`,
//!    GitHub `ghp_…`, Slack `xox…`, AWS `AKIA…`), bearer tokens, JWTs and PEM
//!    private-key blocks are replaced by `«redacted»` even when the value is
//!    not in this process's environment (pasted by the user, read from a
//!    file, printed by a tool).
//!
//! Over-redaction is the accepted trade, exactly as in [`crate::barrel`]:
//! these records feed training and audit; a shredded token is always safer
//! than a leaked one. The environment is read on every call (a few dozen
//! variables) so tests and late-loaded credentials are seen without a cache
//! to invalidate.

use regex::Regex;
use std::borrow::Cow;
use std::sync::OnceLock;

/// Shortest environment value treated as a credential. Shorter values are
/// far more likely to be words (`ANGEL_META_MAX_TOKENS=4096`) than keys, and
/// replacing them would shred ordinary prose.
pub(crate) const MIN_SECRET_LEN: usize = 12;

/// Marker written in place of a value of unknown provenance.
pub(crate) const REDACTED: &str = "«redacted»";

fn shape_patterns() -> &'static [Regex] {
    static SHAPES: OnceLock<Vec<Regex>> = OnceLock::new();
    SHAPES.get_or_init(|| {
        [
            // OpenAI / DeepSeek / Anthropic-style prefixed keys.
            r"\bsk-[A-Za-z0-9_-]{16,}",
            // z.ai / BigModel: 32 hex, a dot, an alphanumeric suffix.
            r"\b[0-9a-f]{32}\.[A-Za-z0-9]{12,}\b",
            // GitHub personal / OAuth / app tokens.
            r"\bgh[pousr]_[A-Za-z0-9]{30,}\b",
            r"\bgithub_pat_[A-Za-z0-9_]{20,}\b",
            // Hugging Face and Google API credentials.
            r"\bhf_[A-Za-z0-9]{20,}\b",
            r"\bAIza[A-Za-z0-9_-]{30,}\b",
            // Slack.
            r"\bxox[abpr]-[A-Za-z0-9-]{10,}",
            // AWS access key ids.
            r"\bAKIA[0-9A-Z]{16}\b",
            // JWTs (three base64url segments, first starts with `{"`).
            r"\beyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}(?:\.[A-Za-z0-9_-]{10,})?",
            // PEM private-key blocks, serialized (`\n`) or raw.
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

/// Credential fields, not telemetry such as `max_tokens`, `cache_key` or
/// `auth_enabled`. Non-string values retain their original types.
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

/// `(NAME, value)` for every secret-named environment variable whose value is
/// long enough to be a credential, longest value first so a key that embeds
/// another key's value is replaced whole.
pub(crate) fn secret_values() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(name, value)| {
            value.len() >= MIN_SECRET_LEN
                && crate::experience::is_secret_name(name)
                && !value.starts_with("«redacted")
                // `ANGEL_*_KEY_FILE` names a path, not a key.
                && !value.starts_with(['/', '~', '.'])
        })
        .collect();
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    out
}

/// Replace known credential values and secret-shaped strings in `text`.
/// Returns the input untouched (no allocation) when nothing matches.
pub(crate) fn redact_str(text: &str) -> Cow<'_, str> {
    Redactor::new().redact_str(text)
}

/// One credential snapshot for a complete record or multiline capture.
pub(crate) struct Redactor {
    values: Vec<(String, String)>,
}

impl Redactor {
    pub(crate) fn new() -> Self {
        Self {
            values: secret_values(),
        }
    }

    pub(crate) fn redact_str<'a>(&self, text: &'a str) -> Cow<'a, str> {
        redact_with_values(text, &self.values)
    }
}

fn redact_with_values<'a>(text: &'a str, values: &[(String, String)]) -> Cow<'a, str> {
    // Tool output and chat content can themselves be encoded JSON. Decode that
    // layer before matching, preserving the original bytes when nothing changes.
    // Parsing a string unwraps one strictly smaller JSON encoding each time.
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
                    // Keep existing markers stable so repeated staging is idempotent.
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

/// Redact decoded strings without changing numbers, booleans, arrays, nulls or
/// record field names. Environment values are captured once per record.
pub(crate) fn redact_value(value: &mut serde_json::Value) -> usize {
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

pub(crate) fn to_redacted_vec(
    value: &(impl serde::Serialize + ?Sized),
) -> serde_json::Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    redact_value(&mut value);
    serde_json::to_vec(&value)
}

/// Pretty sibling of [`to_redacted_vec`] for human-readable state files.
pub(crate) fn to_redacted_vec_pretty(
    value: &(impl serde::Serialize + ?Sized),
) -> serde_json::Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    redact_value(&mut value);
    serde_json::to_vec_pretty(&value)
}

/// Scrub credentials in a persistence payload. JSON is walked with
/// [`redact_value`]; pretty layout is kept when the input already contained a
/// newline. Non-JSON UTF-8 goes through [`redact_str`]. Binary is unchanged.
pub(crate) fn redact_bytes(bytes: &[u8]) -> Vec<u8> {
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

/// Owned sibling of [`redact_str`] for error strings and line-oriented sinks.
pub(crate) fn redact_error(text: &str) -> String {
    redact_str(text).into_owned()
}

/// Exact-body stores reject sensitive records rather than altering the bytes
/// their content hashes certify. Parse JSON before inspecting escaped strings.
pub(crate) fn contains_secret(text: &str) -> bool {
    if let Ok(mut value) = serde_json::from_str(text) {
        redact_value(&mut value) > 0
    } else {
        redact_str(text) != text
    }
}

/// Redact one valid JSONL record after decoding escaped strings. Malformed
/// input is left alone; persistence writers use `to_redacted_vec` directly.
#[cfg(test)]
pub(crate) fn redact_line(line: &mut Vec<u8>) -> usize {
    let mut value = match serde_json::from_slice(line) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let hits = redact_value(&mut value);
    if hits > 0 {
        *line = serde_json::to_vec(&value).expect("JSON value serialization");
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set one variable for the test body and restore the prior value after.
    struct EnvGuard(&'static str, Option<std::ffi::OsString>);
    impl EnvGuard {
        fn set(name: &'static str, value: &str) -> Self {
            let prior = std::env::var_os(name);
            // SAFETY: every test here holds `env_lock`, so no other thread
            // reads or writes the environment concurrently.
            unsafe { std::env::set_var(name, value) };
            Self(name, prior)
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: see `set`.
            unsafe {
                match self.1.take() {
                    Some(prior) => std::env::set_var(self.0, prior),
                    None => std::env::remove_var(self.0),
                }
            }
        }
    }

    #[test]
    fn known_env_value_is_replaced_by_its_name() {
        let _lock = crate::tests::env_lock();
        let _key = EnvGuard::set("ANGEL_OWNED_TEST_KEY", "owned-secret-value-0123456789");
        let text = r#"{"cmd":"curl -H 'x: owned-secret-value-0123456789' https://h"}"#;
        let out = redact_str(text);
        assert_eq!(
            out,
            r#"{"cmd":"curl -H 'x: «redacted:ANGEL_OWNED_TEST_KEY»' https://h"}"#
        );
    }

    #[test]
    fn short_env_values_are_left_alone() {
        let _lock = crate::tests::env_lock();
        let _key = EnvGuard::set("ANGEL_OWNED_TEST_TOKEN", "4096");
        assert_eq!(redact_str("max tokens 4096 ok"), "max tokens 4096 ok");
    }

    #[test]
    fn provider_shapes_are_redacted_without_env_knowledge() {
        let _lock = crate::tests::env_lock();
        let cases = [
            "sk-abcdefghijklmnopqrstuvwxyz0123",
            "0123456789abcdef0123456789abcdef.AbCdEfGhIjKlMnOp",
            "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
            "hf_abcdefghijklmnopqrstuvwxyz01234567",
            "AIzaSyabcdefghijklmnopqrstuvwxyz0123456",
            "xoxb-1234567890-abcdefghij",
            "AKIAABCDEFGHIJKLMNOP",
        ];
        for case in cases {
            let text = format!("token {case} end");
            let out = redact_str(&text);
            assert!(!out.contains(case), "{case} survived: {out}");
            assert!(out.contains(REDACTED), "{case}: {out}");
        }
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijk";
        assert!(!redact_str(jwt).contains("eyJhbGci"));
        let pem = "-----BEGIN RSA PRIVATE KEY-----\\nMIIB\\n-----END RSA PRIVATE KEY-----";
        assert_eq!(redact_str(pem), REDACTED);
        let bearer = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz0123456789";
        assert_eq!(
            redact_str(bearer),
            format!("Authorization: Bearer {REDACTED}")
        );
    }

    #[test]
    fn ordinary_text_is_borrowed_unchanged() {
        let _lock = crate::tests::env_lock();
        let text = "cargo test --release -- harness::tests::cargo_tool 3603 passed";
        assert!(matches!(redact_str(text), Cow::Borrowed(_)));
        let hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(matches!(redact_str(hash), Cow::Borrowed(_)));
    }

    #[test]
    fn serialized_line_stays_valid_json() {
        let _lock = crate::tests::env_lock();
        let _key = EnvGuard::set("ANGEL_OWNED_TEST_SECRET", "abcdefghijklmnopqrstuvwxyz");
        let record = serde_json::json!({
            "text": "export K=abcdefghijklmnopqrstuvwxyz; curl -H 'Authorization: Bearer sk-abcdefghijklmnopqrstuvwxyz0123'",
            "n": 3
        });
        let mut line = serde_json::to_vec(&record).unwrap();
        let hits = redact_line(&mut line);
        assert_eq!(hits, 2);
        let back: serde_json::Value = serde_json::from_slice(&line).unwrap();
        assert_eq!(back["n"], 3);
        let text = back["text"].as_str().unwrap();
        assert!(text.contains("«redacted:ANGEL_OWNED_TEST_SECRET»"));
        assert!(!text.contains("sk-abc"));
    }

    #[test]
    fn decoded_json_redaction_preserves_types_and_escaped_values() {
        let _lock = crate::tests::env_lock();
        let credential = "fixture-雪-\"value\"\\with\n\r\t\u{1}newline-012345";
        let _key = EnvGuard::set("ANGEL_T_PERSIST_SECRET", credential);
        let record = serde_json::json!({
            "text": format!("before {credential} after"),
            "args": {"access_token": "opaque-short", "auth_enabled": true},
            "max_tokens": 4096,
            "token": 17,
            "cache_key": "ordinary-cache-label",
            "ok": [false, null, 2.5]
        });
        let encoded = to_redacted_vec(&record).unwrap();
        let back: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            back["text"],
            "before «redacted:ANGEL_T_PERSIST_SECRET» after"
        );
        assert_eq!(back["args"]["access_token"], REDACTED);
        assert_eq!(back["args"]["auth_enabled"], true);
        assert_eq!(back["max_tokens"], 4096);
        assert_eq!(back["token"], 17);
        assert_eq!(back["cache_key"], record["cache_key"]);
        assert_eq!(back["ok"], record["ok"]);
        assert_eq!(to_redacted_vec(&back).unwrap(), encoded);
        assert_eq!(
            record["args"]["access_token"], "opaque-short",
            "live call remains intact"
        );
    }

    #[test]
    fn explicit_assignments_scrub_without_masking_token_counts() {
        let _lock = crate::tests::env_lock();
        for text in [
            "token=opaque_value",
            "API_KEY='opaque quoted value'",
            "access_token: other-value",
        ] {
            assert!(redact_str(text).contains(REDACTED));
        }
        let text = "max_tokens=4096 cache_key=stable-key auth_enabled=true";
        assert_eq!(redact_str(text), text);
    }

    #[test]
    fn nested_encoded_json_content_is_scrubbed_without_changing_clean_content() {
        let _lock = crate::tests::env_lock();
        let secret = "fixture-雪-\"nested\"\\value\n\u{1}-012345";
        let _key = EnvGuard::set("ANGEL_T_NESTED_SECRET", secret);
        let payload =
            serde_json::json!({"access_token": "opaque", "diagnostic": secret, "n": 4, "ok": true});
        let mut record = serde_json::json!({"content": serde_json::to_string(&payload).unwrap()});
        assert!(redact_value(&mut record) >= 2);
        let clean: serde_json::Value =
            serde_json::from_str(record["content"].as_str().unwrap()).unwrap();
        assert_eq!(clean["access_token"], REDACTED);
        assert_eq!(clean["diagnostic"], "«redacted:ANGEL_T_NESTED_SECRET»");
        assert_eq!(clean["n"], 4);
        assert_eq!(clean["ok"], true);
        let clean_content =
            "{\n  \"max_tokens\": 4096, \"auth_enabled\": true, \"cache_key\": \"stable\"\n}";
        assert_eq!(redact_str(clean_content), clean_content);
    }

    fn plant_secret() -> (EnvGuard, &'static str, &'static str) {
        const SECRET: &str = "fixture-auth-matrix-12";
        const CONTROL: &str = "control-string-untouched";
        (
            EnvGuard::set("ANGEL_J_WRITER_SECRET", SECRET),
            SECRET,
            CONTROL,
        )
    }

    fn assert_sink_redacted(path: &std::path::Path, secret: &str, control: &str) {
        let body = std::fs::read_to_string(path).unwrap();
        assert!(
            !body.contains(secret),
            "secret leaked into {}",
            path.display()
        );
        assert!(body.contains(control), "control string missing");
        assert!(
            body.contains("«redacted:ANGEL_J_WRITER_SECRET»") || body.contains(REDACTED),
            "no redaction marker in {}",
            path.display()
        );
    }

    #[test]
    fn writer_workspace_store_redacts_at_write() {
        let _lock = crate::tests::env_lock();
        let (_g, secret, control) = plant_secret();
        let path =
            std::env::temp_dir().join(format!("angel-j-writer-ws-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        crate::workspace_store::write_private_atomic(
            &path,
            format!(r#"{{"secret":"{secret}","ok":"{control}"}}"#).as_bytes(),
        )
        .unwrap();
        assert_sink_redacted(&path, secret, control);
    }

    #[test]
    fn writer_village_save_state_redacts() {
        let _lock = crate::tests::env_lock();
        let (_g, secret, control) = plant_secret();
        let path = std::env::temp_dir().join(format!(
            "angel-j-writer-village-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let state = crate::village::VillageState {
            apprentice_name: format!("{secret} {control}"),
            ..Default::default()
        };
        crate::village::save_state(&path, &state);
        assert_sink_redacted(&path, secret, control);
    }

    #[test]
    fn writer_redact_bytes_plain_and_json() {
        let _lock = crate::tests::env_lock();
        let (_g, secret, control) = plant_secret();
        let plain = redact_bytes(format!("{secret} {control}").as_bytes());
        let text = String::from_utf8(plain).unwrap();
        assert!(!text.contains(secret));
        assert!(text.contains(control));
        let json = redact_bytes(format!(r#"{{"k":"{secret}","ok":"{control}"}}"#).as_bytes());
        let text = String::from_utf8(json).unwrap();
        assert!(!text.contains(secret));
        assert!(text.contains(control));
    }

    #[test]
    fn redact_error_scrubs_bearer_and_env() {
        let _lock = crate::tests::env_lock();
        let (_g, secret, control) = plant_secret();
        let out = redact_error(&format!("HTTP 401: Bearer {secret} {control}"));
        assert!(!out.contains(secret));
        assert!(out.contains(control));
    }
}
