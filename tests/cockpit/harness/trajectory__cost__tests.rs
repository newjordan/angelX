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
    let _env =
        crate::agent::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
    let _no_home = crate::agent::harness::tests::EnvGuard::unset("HOME");
    f();
    std::fs::remove_file(&path).ok();
}

fn usage(json: Value) -> Option<Value> {
    Some(json)
}

fn attempt(id: usize, model: &str, usage: Value) -> Value {
    json!({"id":id,"end":1,"identity":{"model":{"id":model}},"usage_accounting":usage})
}

#[test]
fn cost_attempts_price_route_switch_and_retry_at_each_executed_model() {
    let guard = crate::tests::env_lock();
    with_table(&guard, || {
        let usage = json!({"input":1_000_000,"cache_read":0,"uncached_input":1_000_000,
            "output":1_000_000,"reasoning":100_000,"attempts":1,
            "accounting_status":"reported","reasoning_convention_attempts":{"included":1}});
        let samples = vec![
            attempt(0, "glm-5.3-air", usage.clone()),
            attempt(1, "deepseek-v4-flash", usage.clone()),
            attempt(2, "deepseek-v4-flash", usage),
        ];
        let cost = cost_for_attempts(
            Some("deepseek-v4-flash"),
            Some(&json!({"attempts":3})),
            &samples,
        );
        assert_eq!(cost["paid"], 5.5); // 2.5 + 1.5 + 1.5; reasoning is already included.
        assert!(
            cost["model"].is_null(),
            "mixed usage has no single model price"
        );
        assert_eq!(cost["source"], "provider-attempts");
        assert_eq!(cost["attempts"][0]["model"], "glm-5.3-air");
        assert_eq!(cost["attempts"][1]["model"], "deepseek-v4-flash");
        let switched = cost_for_attempts(
            Some("grok-4.7"),
            Some(&json!({"attempts":1})),
            &samples[1..2],
        );
        assert_eq!(switched["model"], "deepseek-v4-flash");
        assert_eq!(switched["paid"], 1.5);
    });
}

#[test]
fn cost_attempts_missing_parallel_seat_or_retry_usage_keeps_total_unknown() {
    let guard = crate::tests::env_lock();
    with_table(&guard, || {
        let sample = attempt(
            0,
            "glm-5.3-air",
            json!({"input":1_000_000,"output":0,
            "accounting_status":"reported"}),
        );
        let cost = cost_for_attempts(
            Some("glm-5.3-air"),
            Some(&json!({"attempts":2})),
            std::slice::from_ref(&sample),
        );
        assert!(cost["paid"].is_null());
        assert_eq!(cost["source"], "incomplete-model-attribution");
        assert_eq!(cost["attempts"][0]["paid"], 0.5);
        let missing = attempt(
            1,
            "deepseek-v4-flash",
            json!({"accounting_status":"unreported"}),
        );
        let cost = cost_for_attempts(
            Some("deepseek-v4-flash"),
            Some(&json!({"attempts":2})),
            &[sample, missing],
        );
        assert!(cost["paid"].is_null());
        assert_eq!(cost["basis"], "unreported-usage");
    });
}

#[test]
fn cost_attempts_subscription_retains_zero_without_provider_counters() {
    let guard = crate::tests::env_lock();
    with_table(&guard, || {
        let samples = [attempt(0, "subscription-model", Value::Null)];
        let cost = cost_for_attempts(
            Some("subscription-model"),
            Some(&json!({"attempts":1})),
            &samples,
        );
        assert_eq!(cost["paid"], 0.0);
        assert_eq!(cost["basis"], "subscription-marginal");
        assert_eq!(cost["attempts"][0]["plan_fee_usd_month"], 20.0);
    });
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
    let _env =
        crate::agent::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
    let _home = crate::agent::harness::tests::EnvGuard::unset("HOME");
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
    let _env = crate::agent::harness::tests::EnvGuard::unset("ANGEL_PRICES_FILE");
    let _home = crate::agent::harness::tests::EnvGuard::set(
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
    let _env =
        crate::agent::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
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

// --- the live `cost` leg -----------------------------------------------------
//
// `cost_with_rows` stays the oracle: the leg is only allowed to be a different
// implementation of the same decision, never a different decision.

fn leg_row<'a>(rows: &'a [PriceRow], model: &str) -> Option<&'a PriceRow> {
    rows.iter()
        .find(|row| row.model == model)
        .or_else(|| rows.iter().find(|row| family_prefix(&row.model, model)))
}

/// Model ids and usage blocks that reach every gate the native ladder has:
/// reported fields, cache conventions both blessed and unblessed, reasoning
/// conventions, zero counts, a subscription row, an unparseable row and no usage.
fn leg_cases() -> Vec<(&'static str, Option<Value>)> {
    vec![
        ("glm-5.3-air", usage(json!({"input":1_000_000,"cache_read":200_000,"uncached_input":800_000,"output":500_000,"reasoning":100_000,"attempts":1}))),
        ("glm-5.3-air", usage(json!({"input":1_000_000}))),
        ("glm-5.3-air", usage(json!({"output":500_000}))),
        ("glm-5.3-air", usage(json!({"cache_read":200_000}))),
        ("glm-5.3-air", usage(json!({"input":1_000,"cache_read":100}))),
        ("glm-5.3-air", usage(json!({"input":100,"cache_read":1_000}))),
        ("glm-5.3-air", usage(json!({"input":1_000,"cache_read":1_000}))),
        ("glm-5.3-air", usage(json!({"uncached_input":7_000}))),
        ("glm-5.3-air", usage(json!({"reasoning":9_000}))),
        ("glm-5.3-air", usage(json!({"output":1_000,"reasoning":9_000,"attempts":2,
            "reasoning_convention_attempts":{"separate":2}}))),
        ("glm-5.3-air", usage(json!({"output":1_000,"reasoning":9_000,"attempts":2,
            "reasoning_convention_attempts":{"separate":1}}))),
        ("glm-5.3-air", usage(json!({"output":1_000,"reasoning":9_000,"attempts":0,
            "reasoning_convention_attempts":{"separate":0}}))),
        ("glm-5.3-air", usage(json!({"input":0,"cache_read":0,"uncached_input":0,"output":0,"reasoning":0,"attempts":1}))),
        ("glm-5.3-air", usage(json!({"output":0}))),
        ("glm-5.3-air", usage(json!({"input":1_000_000_000,"output":1_000_000_000}))),
        ("glm-5.3-air", usage(json!({"input":123_457,"output":9_876,"cache_read":57}))),
        ("glm-5.3-air", usage(json!({"input":5,"cache_read":5,"uncached_input":0}))),
        ("glm-5.3-air", usage(json!({"input":3,"output":3,"attempts":1,
            "reasoning_convention_attempts":{"separate":1}}))),
        ("deepseek-v4-flash", usage(json!({"input":2_000_000,"cache_read":500_000,"output":1_500_000,"reasoning":250_000,"attempts":1}))),
        ("deepseek-v4-flash", usage(json!({"input":10_000,"output":20_000}))),
        ("deepseek-v4-flash", usage(json!({"cache_read":4_000}))),
        ("deepseek-v4-flash-exp", usage(json!({"input":1_000_000,"output":1_000_000}))),
        ("deepseek-v4-flash-exp", usage(json!({"output":0}))),
        ("subscription-model", usage(json!({"input":1_000,"output":500}))),
        ("subscription-model", usage(json!({}))),
        ("subscription-model", None),
        ("subscription-model", usage(json!({"input":0,"output":0}))),
        ("unknown-price-model", usage(json!({"input":1_000,"output":500}))),
        ("unknown-price-model", None),
        ("unknown-price-model", usage(json!({}))),
        ("glm-5.3-air", usage(json!({}))),
        ("glm-5.3-air", None),
        ("glm-5.3-air", usage(json!({"attempts":3}))),
        ("glm-5.3-air", usage(json!({"input":9_000_000,"cache_read":3_000_000,"output":4_000_000,
            "reasoning":2_000_000,"attempts":2,"reasoning_convention_attempts":{"separate":2}}))),
    ]
}

#[test]
fn cost_leg_decides_every_record_exactly_like_the_native_ladder() {
    let rows = parse_rows(TABLE);
    let leg = crate::agent::harness::live_policy::shipped_leg("cost");
    let cases = leg_cases();
    assert!(cases.len() >= 30, "the parity table stays broad");
    for (model, usage) in &cases {
        // Every case must be inside the leg's contract: a rate the e8 units
        // cannot carry keeps the native decision and is reported here.
        let row = leg_row(&rows, model);
        let input = serde_json::to_value(
            cost_leg_input(usage.as_ref(), row).expect("fixture rates are e8-exact"),
        )
        .unwrap();
        let native = cost_with_rows(Some(model), usage.as_ref(), &rows, "fixture");
        let (decision, source) = crate::agent::harness::live_policy::decide_with(
            Some(leg.clone()),
            "cost",
            &input,
            |value: &CostLegDecision| cost_leg_check(row, value),
            || native_decision(model, usage.as_ref(), row, "fixture"),
        );
        assert!(
            matches!(source, crate::agent::harness::live_policy::LegSource::Live { .. }),
            "{model}/{usage:?} did not decide in the leg: {source:?}"
        );
        assert_eq!(
            cost_record(model, usage.as_ref(), row, "fixture", &decision),
            native,
            "{model}/{usage:?} diverged from the native ladder"
        );
    }
}

/// A shipped directory with one deliberately out-of-contract `cost` module.
fn out_of_contract_leg() -> angel_live::Program {
    let root = std::env::temp_dir().join(format!("angel-cost-broken-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("cost");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("module.json"),
        r#"{"api_version":1,"id":"cost","kind":"policy","entry":"index.js","probe":{"input":1}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("index.js"),
        "module.exports={apiVersion:1,render: () => ({basis: 'list-price', paid_e8: 1})}",
    )
    .unwrap();
    let mut registry = angel_live::Registry::validated(
        root.clone(),
        crate::agent::hotload::evaluate,
        |_, _| Ok(()),
    );
    registry.refresh(true);
    let program = registry
        .snapshot()
        .policies
        .get("cost")
        .cloned()
        .expect("the out-of-contract leg loads");
    let _ = std::fs::remove_dir_all(&root);
    program
}

#[test]
fn cost_leg_out_of_contract_answer_keeps_the_native_decision() {
    let rows = parse_rows(TABLE);
    let model = "unknown-price-model";
    let usage = usage(json!({"input":1_000,"output":500}));
    let row = leg_row(&rows, model);
    assert!(row.is_none(), "this case has no price row to claim");
    let input =
        serde_json::to_value(cost_leg_input(usage.as_ref(), row).unwrap()).unwrap();
    let native = cost_with_rows(Some(model), usage.as_ref(), &rows, "fixture");
    let (decision, source) = crate::agent::harness::live_policy::decide_with(
        Some(out_of_contract_leg()),
        "cost",
        &input,
        |value: &CostLegDecision| cost_leg_check(row, value),
        || native_decision(model, usage.as_ref(), row, "fixture"),
    );
    assert!(
        matches!(
            source,
            crate::agent::harness::live_policy::LegSource::LiveFallback { .. }
        ),
        "a basis the record cannot have must fall back: {source:?}"
    );
    assert_eq!(
        cost_record(model, usage.as_ref(), row, "fixture", &decision),
        native
    );
    // The same contract also rejects a subscription row that invents a fee.
    let rows = parse_rows(TABLE);
    let model = "subscription-model";
    let row = leg_row(&rows, model);
    let denied = CostLegDecision {
        basis: "subscription-marginal".into(),
        paid_e8: 0,
        cached_e8: None,
        fee_e8: 1,
    };
    assert!(cost_leg_check(row, &denied).is_err());
}

#[test]
fn cost_leg_decision_timing() {
    let rows = parse_rows(TABLE);
    let leg = crate::agent::harness::live_policy::shipped_leg("cost");
    let model = "glm-5.3-air";
    let usage = usage(json!({"input":1_000_000,"cache_read":200_000,"uncached_input":800_000,
        "output":500_000,"reasoning":100_000,"attempts":1}));
    let row = leg_row(&rows, model);
    let input = serde_json::to_value(cost_leg_input(usage.as_ref(), row).unwrap()).unwrap();
    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let started = std::time::Instant::now();
        let (_decision, source) = crate::agent::harness::live_policy::decide_with(
            Some(leg.clone()),
            "cost",
            &input,
            |value: &CostLegDecision| cost_leg_check(row, value),
            || native_decision(model, usage.as_ref(), row, "fixture"),
        );
        assert!(matches!(
            source,
            crate::agent::harness::live_policy::LegSource::Live { .. }
        ));
        samples.push(started.elapsed().as_micros() as u64);
    }
    samples.sort_unstable();
    eprintln!(
        "cost leg: median {} µs per decision over 200 calls (p95 {} µs, max {} µs)",
        samples[100],
        samples[190],
        samples[199]
    );
}

/// A card whose rates are finer than a cent, or a record whose product outgrows
/// a Bend Nat immediate, stays on the native decision rather than being rounded.
#[test]
fn cost_leg_keeps_records_outside_its_units_on_the_native_decision() {
    const FINE_TABLE: &str = r#"
[[prices]]
provider = "test"
model = "fine-rate-model"
currency = "usd"
price_date = "2026-09-10"
input = "0.035"
output = "1.005"
"#;
    let rows = parse_rows(FINE_TABLE);
    let model = "fine-rate-model";
    let spend = usage(json!({"input":1_000_000,"output":1_000_000}));
    let row = leg_row(&rows, model);
    assert!(row.is_some(), "the row itself parses");
    assert!(
        cost_leg_input(spend.as_ref(), row).is_none(),
        "a rate finer than a cent has no exact integer"
    );
    assert!(cost_leg(model, spend.as_ref(), row, "fixture").is_none());
    assert_eq!(
        cost_decided(Some(model), spend.as_ref(), &rows, "fixture"),
        cost_with_rows(Some(model), spend.as_ref(), &rows, "fixture")
    );

    // 2*10^12 input tokens at 0.50/M is past the leg's Nat budget (2^46).
    let rows = parse_rows(TABLE);
    let model = "glm-5.3-air";
    let huge = usage(json!({"input":2_000_000_000_000u64}));
    let row = leg_row(&rows, model);
    assert!(
        cost_leg_input(huge.as_ref(), row).is_none(),
        "a product past 2^48-1 is out of contract"
    );
    assert_eq!(
        cost_decided(Some(model), huge.as_ref(), &rows, "fixture"),
        cost_with_rows(Some(model), huge.as_ref(), &rows, "fixture")
    );
}
