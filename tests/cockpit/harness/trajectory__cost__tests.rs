use super::*;

const TABLE: &str = r#"
[[prices]]
provider = "test"
model = "glm-5.3-air"
currency = "usd"
price_date = "2026-09-08"
input = "0.50"
cached_input = "0.05"
output = "2.00"
reasoning = "1.00"

[[prices]]
provider = "test"
model = "deepseek-v4-flash"
currency = "usd"
price_date = "2026-09-08"
input = "0.30"
cached_input = "0.03"
output = "1.20"
reasoning = "0.60"

[[prices]]
provider = "test"
model = "subscription-model"
monthly_fee_usd = 20.0
price_date = "2026-09-09"
confirmed = false
input = "unknown"

[[prices]]
provider = "test"
model = "unknown-price-model"
price_date = "2026-09-08"
input = "unknown"
currency = "unknown"
"#;

// Caller holds the shared env lock before creating or deleting this shared fixture.
fn with_table(_guard: &std::sync::MutexGuard<'_, ()>, f: impl FnOnce()) {
    let dir = std::env::temp_dir().join(format!("angel-cost-table-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prices.toml");
    std::fs::write(&path, TABLE).unwrap();
    let _env = crate::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
    let _no_home = crate::harness::tests::EnvGuard::unset("HOME");
    f();
    std::fs::remove_file(&path).ok();
}

fn usage(json: Value) -> Option<Value> {
    Some(json)
}

#[test]
fn cost_exact_id_pricing() {
    let _guard = crate::tests::env_lock();
    with_table(&_guard, || {
        let cost = cost_for(
            Some("glm-5.3-air"),
            usage(json!({"input":1_000_000,"cache_read":200_000,
                    "uncached_input":800_000,"output":500_000,
                    "reasoning":100_000,"attempts":1}))
            .as_ref(),
        );
        // 800k*0.50 + 200k*0.05 + 500k*2.00 + 100k*1.00 = 1.51
        assert_eq!(cost["paid"], json!(1.51));
        assert!(cost.get("plan_fee_usd_month").is_none());
        assert_eq!(cost["cached"], json!(0.01));
        assert_eq!(cost["basis"], json!("list-price"));
        assert_eq!(cost["currency"], json!("usd"));
        assert_eq!(cost["price_date"], json!("2026-09-08"));
        assert!(
            cost["source"]
                .as_str()
                .unwrap()
                .contains("ANGEL_PRICES_FILE")
        );
    });
}

#[test]
fn cost_subscription_has_zero_marginal_cost_even_without_token_usage() {
    let _guard = crate::tests::env_lock();
    with_table(&_guard, || {
        for usage in [
            None,
            Some(json!({})),
            Some(json!({"input":1000,"output":500})),
        ] {
            let cost = cost_for(Some("subscription-model"), usage.as_ref());
            assert_eq!(
                cost,
                json!({"paid":0.0,"cached":null,"currency":"USD",
                    "price_date":"2026-09-09","basis":"subscription-marginal",
                    "plan_fee_usd_month":20.0,"confirmed":false,
                    "source":"ANGEL_PRICES_FILE","model":"subscription-model"})
            );
        }
    });
}

#[test]
fn cost_family_fallback_never_crosses_families() {
    let _guard = crate::tests::env_lock();
    with_table(&_guard, || {
        let cost = cost_for(
            Some("deepseek-v4-flash-exp"),
            usage(json!({"input":1_000_000,"output":1_000_000})).as_ref(),
        );
        // 1M*0.30 + 1M*1.20 = 1.50 via the deepseek-v4-flash stem.
        assert_eq!(cost["paid"], json!(1.5));
        assert_eq!(cost["model"], json!("deepseek-v4-flash-exp"));
        // Any delimiter-bounded extension of a priced stem resolves to it
        // (`glm-5.3-air-quantized` → glm-5.3-air); only a different stem
        // (a different family) must stay unpriced.
        let stem = cost_for(
            Some("glm-5.3-air-quantized"),
            usage(json!({"input":1_000_000,"output":0})).as_ref(),
        );
        assert_eq!(stem["basis"], json!("list-price"));
        assert_eq!(stem["paid"], json!(0.5));
        let none = cost_for(Some("glm-5.3-airx"), usage(json!({"input":10})).as_ref());
        assert_eq!(none["basis"], json!("no-price-row"));
    });
}

#[test]
fn cost_missing_usage_is_unreported() {
    let _guard = crate::tests::env_lock();
    with_table(&_guard, || {
        let cost = cost_for(Some("glm-5.3-air"), None);
        assert_eq!(cost["basis"], json!("unreported-usage"));
        assert!(cost["paid"].is_null());
        let empty = cost_for(Some("glm-5.3-air"), Some(&json!({"attempts":2})));
        assert_eq!(empty["basis"], json!("unreported-usage"));
    });
}

#[test]
fn cost_unknown_model_has_no_price_row() {
    let _guard = crate::tests::env_lock();
    with_table(&_guard, || {
        let cost = cost_for(
            Some("totally-other-model"),
            usage(json!({"input":100,"output":100})).as_ref(),
        );
        assert_eq!(cost["basis"], json!("no-price-row"));
        assert!(cost["paid"].is_null());
        assert!(cost["currency"].is_null());
    });
}

#[test]
fn cost_partial_prices_price_only_reported_fields() {
    let _guard = crate::tests::env_lock();
    // Row without a cached_input price: cached tokens must not be priced
    // at the full input rate. Use a separate file for the modified table.
    let table = TABLE.replace("cached_input = \"0.03\"", "cached_input = \"unknown\"");
    let dir = std::env::temp_dir().join(format!("angel-cost-part-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prices.toml");
    std::fs::write(&path, &table).unwrap();
    let _env = crate::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
    let _home = crate::harness::tests::EnvGuard::unset("HOME");
    let cost = cost_for(
        Some("deepseek-v4-flash"),
        usage(json!({"input":1_000_000,"cache_read":400_000,
                "uncached_input":600_000,"output":0}))
        .as_ref(),
    );
    // Only 600k uncached at 0.30 is priced; cached rate unknown.
    assert_eq!(cost["paid"], json!(0.18));
    assert!(cost["cached"].is_null());
    std::fs::remove_file(&path).ok();
}

#[test]
fn cost_embedded_table_resolves_and_is_reported() {
    let _guard = crate::tests::env_lock();
    let _env = crate::harness::tests::EnvGuard::unset("ANGEL_PRICES_FILE");
    let _home = crate::harness::tests::EnvGuard::set(
        "HOME",
        std::env::temp_dir()
            .join(format!("angel-cost-nohome-{}", std::process::id()))
            .to_str()
            .unwrap(),
    );
    let cost = cost_for(
        Some("glm-5.3-flash"),
        Some(&json!({"input":1_000,"output":1_000})),
    );
    // The nominal subscription resolves marginal cost, not token pricing.
    assert_eq!(cost["basis"], json!("subscription-marginal"));
    assert_eq!(cost["paid"], json!(0.0));
    assert_eq!(cost["plan_fee_usd_month"], json!(20.0));
    assert_eq!(cost["confirmed"], json!(false));
    assert_eq!(cost["currency"], json!("USD"));
    assert_eq!(cost["price_date"], json!("2026-09-09"));
    assert_eq!(cost["source"], json!("embedded:docs/telemetry/prices.toml"));
}

#[test]
fn cost_env_file_override_wins() {
    let _guard = crate::tests::env_lock();
    let dir = std::env::temp_dir().join(format!("angel-cost-env-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("prices.toml");
    std::fs::write(
        &path,
        r#"
[[prices]]
provider = "env"
model = "override-model"
currency = "eur"
price_date = "2026-09-01"
input = "1.0"
output = "1.0"
"#,
    )
    .unwrap();
    let _env = crate::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
    let cost = cost_for(
        Some("override-model"),
        Some(&json!({"input":2_000_000,"output":1_000_000})),
    );
    assert_eq!(cost["paid"], json!(3.0));
    assert_eq!(cost["currency"], json!("eur"));
    assert_eq!(cost["price_date"], json!("2026-09-01"));
    assert!(
        cost["source"]
            .as_str()
            .unwrap()
            .contains("ANGEL_PRICES_FILE")
    );
    std::fs::remove_file(&path).ok();
}
