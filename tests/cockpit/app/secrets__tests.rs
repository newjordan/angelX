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
    let path = std::env::temp_dir().join(format!("angel-j-writer-ws-{}.json", std::process::id()));
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
