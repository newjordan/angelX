use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn record() -> Value {
    json!({"id":"gpt-6.1-sol", "name":"Sol", "provider":"openai-codex", "api":"openai-codex-responses",
        "reasoning":true, "input":["text","image"], "contextWindow":272000, "maxTokens":128000,
        "thinkingLevelMap":{"minimal":"low","xhigh":"xhigh","max":"max"}})
}
fn store(records: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({"openai-codex":{"models":records}})).unwrap()
}
fn load_record(r: Value) -> CatalogLoad {
    CatalogLoad {
        source: Source::PiStore,
        checked: parse(&store(vec![r])),
    }
}
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "angel-catalog-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, bytes: &[u8]) -> std::path::PathBuf {
        let path = self.0.join("models-store.json");
        std::fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn catalog_r1_explicit_failures_never_consult_legacy() {
    let fixture = Fixture::new();
    let missing = format!("pi-store:{}", fixture.0.join("missing").display());
    let mut choices = vec!["", "unknown:/file", "pi-store:relative", missing.as_str()];
    let bad = fixture.file(b"not JSON");
    let selector = format!("pi-store:{}", bad.display());
    choices.push(&selector);
    for selector in choices {
        let load = CatalogLoad::load(Some(selector), || {
            panic!("explicit source called legacy reader")
        });
        assert!(load.checked.is_err());
        assert!(load.resolve("gpt-5.6-luna", Some("max"), true).is_err());
    }
    for records in [
        vec![],
        vec![{
            let mut r = record();
            r["id"] = json!("gpt-5.3-codex-spark");
            r
        }],
        vec![{
            let mut r = record();
            r["reasoning"] = json!(false);
            r
        }],
    ] {
        let path = fixture.file(&store(records));
        let load = CatalogLoad::load(Some(&format!("pi-store:{}", path.display())), || {
            panic!("fallback")
        });
        assert_eq!(load.checked.unwrap_err(), Diagnostic::NoSelectableModels);
    }
}
#[test]
fn catalog_r1_negative_shape_identity_dimensions_and_thinking() {
    for (key, value, error) in [
        ("provider", json!("openai"), Diagnostic::Identity),
        ("api", json!("openai-responses"), Diagnostic::Identity),
        ("contextWindow", json!(2559), Diagnostic::Capacity),
        ("contextWindow", json!(0), Diagnostic::Capacity),
        ("contextWindow", json!(272000.5), Diagnostic::Capacity),
        ("contextWindow", json!(-1), Diagnostic::Capacity),
        ("contextWindow", json!("272000"), Diagnostic::Capacity),
        ("maxTokens", Value::Null, Diagnostic::Capacity),
        ("input", json!(["audio"]), Diagnostic::Shape),
        ("input", json!(["text", "text"]), Diagnostic::Duplicate),
        ("reasoning", json!("true"), Diagnostic::Thinking),
        ("thinkingLevelMap", json!([]), Diagnostic::Thinking),
        ("thinkingLevelMap", json!({"high":12}), Diagnostic::Thinking),
        (
            "thinkingLevelMap",
            json!({"high":"invented"}),
            Diagnostic::Thinking,
        ),
        (
            "thinkingLevelMap",
            json!({"off":"invented"}),
            Diagnostic::Thinking,
        ),
        ("name", json!("x".repeat(321)), Diagnostic::Shape),
        ("id", json!("bad\nlabel"), Diagnostic::Shape),
    ] {
        let mut r = record();
        r[key] = value;
        assert_eq!(load_record(r).checked.unwrap_err(), error, "{key}");
    }
    assert_eq!(
        parse(&store(vec![record(), record()])).unwrap_err(),
        Diagnostic::Duplicate
    );
    for root in [
        json!({}),
        json!({"openai-codex":[]}),
        json!({"openai-codex":{"models":{}}}),
    ] {
        assert_eq!(
            parse(&serde_json::to_vec(&root).unwrap()).unwrap_err(),
            Diagnostic::Shape
        );
    }
}
#[test]
fn given_pi_off_metadata_when_loading_then_high_works_without_enabling_off() {
    // Given the two actual Pi off-map forms in the same store,
    // When that store is checked, Then selectable reasoning models survive,
    // while neither pi:off nor a native none effort is authorized.
    let mut none = record();
    none["id"] = json!("gpt-6-luna");
    none["thinkingLevelMap"]["off"] = json!("none");
    let mut null = record();
    null["thinkingLevelMap"]["off"] = Value::Null;
    let checked = parse(&store(vec![none, null])).expect("actual Pi off metadata is compatible");
    let load = CatalogLoad {
        source: Source::PiStore,
        checked: Ok(checked),
    };
    for model in ["gpt-6-luna", "gpt-6.1-sol"] {
        assert_eq!(
            load.resolve(model, Some("pi:high"), false)
                .unwrap()
                .wire
                .as_deref(),
            Some("high")
        );
        assert!(load.resolve(model, Some("pi:off"), false).is_err());
        assert!(load.resolve(model, Some("none"), false).is_err());
    }
}

#[test]
fn catalog_r1_reader_rejects_fifo_directory_and_byte_record_limits() {
    let fixture = Fixture::new();
    assert_eq!(
        read_checked(&fixture.0).unwrap_err(),
        Diagnostic::NotRegular
    );
    let fifo = fixture.0.join("fifo");
    let path = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    assert_eq!(read_checked(&fifo).unwrap_err(), Diagnostic::NotRegular);
    let file = fixture.file(b"");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_len(MAX_BYTES + 1)
        .unwrap();
    assert_eq!(read_checked(&file).unwrap_err(), Diagnostic::TooLarge);
    assert_eq!(
        parse(&store(vec![record(); MAX_MODELS + 1])).unwrap_err(),
        Diagnostic::TooLarge
    );
}
#[test]
fn catalog_r1_checked_projection_accepts_sol_ignores_endpoint_auth_instructions() {
    let mut r = record();
    let clean = load_record(r.clone());
    r["baseUrl"] = json!("https://do-not-use.invalid");
    r["headers"] = json!({"Authorization":"do-not-use"});
    r["authCommand"] = json!("do-not-execute");
    r["instructions"] = json!("do-not-adopt");
    r["compat"] = json!({"supportsTools":false});
    let fixture = Fixture::new();
    let path = fixture.file(&store(vec![r]));
    let before = std::fs::read(&path).unwrap();
    let load = CatalogLoad::load(Some(&format!("pi-store:{}", path.display())), || {
        panic!("legacy")
    });
    assert_eq!(load.models()[0].slug, "gpt-6.1-sol");
    assert_eq!(load.models()[0].context_window, Some(272000));
    assert_eq!(
        load.models()[0]
            .supported_reasoning_levels
            .iter()
            .map(|l| l.effort.as_str())
            .collect::<Vec<_>>(),
        ["low", "medium", "high", "xhigh", "max"]
    );
    assert_eq!(
        load.checked.as_ref().unwrap().hash,
        clean.checked.unwrap().hash
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    assert_eq!(load.status()["source"], "pi-store");
}
#[test]
fn catalog_r2_published_snapshot_survives_file_replacement() {
    let fixture = Fixture::new();
    let path = fixture.file(&store(vec![record()]));
    let selector = format!("pi-store:{}", path.display());
    let first = CatalogLoad::load(Some(&selector), || panic!("legacy"));
    let mut r = record();
    r["contextWindow"] = json!(8192);
    let replacement = fixture.0.join("replacement");
    std::fs::write(&replacement, store(vec![r])).unwrap();
    std::fs::rename(replacement, &path).unwrap();
    for _ in 0..3 {
        assert_eq!(first.models()[0].context_window, Some(272000));
        assert!(first.resolve("gpt-6.1-sol", Some("pi:high"), false).is_ok());
    }
    let second = CatalogLoad::load(Some(&selector), || panic!("legacy"));
    assert_eq!(second.models()[0].context_window, Some(8192));
    assert_ne!(first.checked.unwrap().hash, second.checked.unwrap().hash);
}
#[test]
fn catalog_r1_revision_change_retries_once_then_fails() {
    let fixture = Fixture::new();
    let path = fixture.file(&store(vec![record()]));
    let mut reads = 0;
    let stable = read_checked_with(&path, |attempt| {
        reads += 1;
        if attempt == 0 {
            let mut r = record();
            r["contextWindow"] = json!(8192);
            std::fs::write(&path, store(vec![r])).unwrap();
        }
    })
    .unwrap();
    assert_eq!(reads, 2);
    assert_eq!(stable.models[0].context_window, Some(8192));
    reads = 0;
    assert_eq!(
        read_checked_with(&path, |attempt| {
            reads += 1;
            let mut r = record();
            r["name"] = json!("new".repeat(attempt + 2));
            std::fs::write(&path, store(vec![r])).unwrap();
        })
        .unwrap_err(),
        Diagnostic::Changed
    );
    assert_eq!(reads, 2);
}
#[test]
fn catalog_r3_effort_namespaces_null_omission_extended_and_no_clamping() {
    let load = load_record(record());
    assert_eq!(
        load.resolve("gpt-6.1-sol", Some("pi:minimal"), false)
            .unwrap()
            .wire
            .as_deref(),
        Some("low")
    );
    assert!(load.resolve("gpt-6.1-sol", Some("minimal"), false).is_err()); // explicit mapping declares low, not native minimal
    assert_eq!(
        load.resolve("gpt-6.1-sol", Some("ultra"), false)
            .unwrap()
            .wire
            .as_deref(),
        Some("xhigh")
    );
    let mut r = record();
    r["thinkingLevelMap"] = json!({"high":null});
    let load = load_record(r);
    assert_eq!(
        load.resolve("gpt-6.1-sol", Some("pi:minimal"), false)
            .unwrap()
            .wire
            .as_deref(),
        Some("minimal")
    );
    for effort in [
        "pi:high", "high", "pi:xhigh", "pi:max", "pi:ultra", "pi:off", "invented",
    ] {
        assert!(
            load.resolve("gpt-6.1-sol", Some(effort), false).is_err(),
            "{effort}"
        );
    }
    let legacy = CatalogLoad::from_models(Source::Legacy, load.models().to_vec());
    assert!(
        legacy
            .resolve("gpt-6.1-sol", Some("pi:low"), false)
            .is_err()
    );
}
#[test]
fn catalog_r4_legacy_unknown_retains_fallback_and_known_tiny_rejects() {
    let empty = CatalogLoad::from_models(Source::Legacy, vec![]);
    assert!(empty.resolve("gpt-5.6-luna", Some("max"), true).is_ok());
    assert!(empty.resolve("gpt-5.6-luna", Some("max"), false).is_err());
    let mut spec = CodexModelInfo {
        slug: "legacy".into(),
        supported_reasoning_levels: vec![CodexReasoningLevel {
            effort: "high".into(),
            description: String::new(),
        }],
        ..Default::default()
    };
    assert!(
        CatalogLoad::from_models(Source::Legacy, vec![spec.clone()])
            .resolve("legacy", Some("high"), false)
            .is_ok()
    );
    spec.context_window = Some(512);
    assert!(
        CatalogLoad::from_models(Source::Legacy, vec![spec])
            .resolve("legacy", Some("high"), false)
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(checked_context(2560).unwrap(), 2560);
    assert!(checked_context(0).is_err());
    if usize::BITS < 64 {
        assert!(checked_context(u64::MAX).is_err());
    }
}
