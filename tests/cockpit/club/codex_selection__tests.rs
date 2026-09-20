use super::*;
fn catalog() -> Vec<CodexModelInfo> {
    serde_json::from_value(serde_json::json!([
        {"slug":"gpt-5.6-luna","display_name":"Luna","supported_reasoning_levels":[{"effort":"max"}]},
        {"slug":"gpt-6-astra","display_name":"Astra","supported_reasoning_levels":[{"effort":"medium"}]}
    ])).unwrap()
}
#[test]
fn openai_codex_resolution_order_and_sources() {
    let c = catalog();
    let env = resolve(
        Some("gpt-5.6-luna".into()),
        Some("max".into()),
        Some("gpt-6-astra".into()),
        Some("medium".into()),
        &c,
    );
    assert_eq!(
        (
            &*env.model,
            &*env.effort,
            env.model_source,
            env.effort_source
        ),
        ("gpt-5.6-luna", "max", "env", "env")
    );
    assert!(env.error.is_none());
    let config = resolve(
        None,
        None,
        Some("gpt-6-astra".into()),
        Some("medium".into()),
        &c,
    );
    assert_eq!(
        (
            &*config.model,
            &*config.effort,
            config.model_source,
            config.effort_source
        ),
        ("gpt-6-astra", "medium", "codex-config", "codex-config")
    );
    assert!(config.error.is_none());
    let fallback = resolve(None, None, None, None, &c);
    assert_eq!(
        (
            &*fallback.model,
            &*fallback.effort,
            fallback.model_source,
            fallback.effort_source
        ),
        (
            OPENAI_LUNA_MODEL,
            OPENAI_LUNA_EFFORT,
            "fallback",
            "fallback"
        )
    );
    assert!(fallback.error.is_none());
}
#[test]
fn openai_codex_refuses_unsupported_pins_without_substitution() {
    let c = catalog();
    let model = resolve(Some("invented".into()), None, None, None, &c);
    assert_eq!(model.model, "invented");
    assert!(
        model
            .error
            .unwrap()
            .contains("supported models: [gpt-5.6-luna, gpt-6-astra]")
    );
    let effort = resolve(
        Some("gpt-5.6-luna".into()),
        Some("medium".into()),
        None,
        None,
        &c,
    );
    assert_eq!(effort.effort, "medium");
    assert!(effort.error.unwrap().contains("supported efforts: [max]"));
    assert!(
        resolve(Some("gpt-5.6-luna".into()), None, None, None, &[])
            .error
            .is_some()
    );
}
