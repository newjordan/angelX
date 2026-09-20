//! List-price cost binding for the trace ledger (`cost` block).
//!
//! The record's own `usage` totals are priced against the list-price table
//! (`[[prices]]` rows). Unknowns are explicit: a null is always explained by
//! `basis`. Observed balance deltas are joined separately and are never a
//! list price, so they play no part here.
//!
//! Price-table resolution order:
//!   1. `ANGEL_PRICES_FILE` (if set)
//!   2. `~/.angel0/prices.toml`
//!   3. the repo table embedded at build time (`include_str!`)
//!
//! The source actually used is reported in `cost.source`. Resolution is
//! re-read per record (cheap, and the overrides must take effect at record
//! time, not process start).

use serde_json::{Value, json};
use std::path::PathBuf;

/// Metered per-million prices or a nominal monthly subscription fee.
struct PriceRow {
    #[allow(dead_code)]
    provider: String,
    model: String,
    input: Option<f64>,
    cached_input: Option<f64>,
    output: Option<f64>,
    reasoning: Option<f64>,
    plan_fee_usd_month: Option<f64>,
    confirmed: Option<bool>,
    currency: String,
    price_date: String,
}

fn decimal_field(table: &toml::Value, key: &str) -> Option<f64> {
    match table.get(key)? {
        toml::Value::String(s) => s.parse::<f64>().ok(),
        toml::Value::Integer(i) => Some(*i as f64),
        toml::Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn field_str(table: &toml::Value, key: &str) -> String {
    table
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

fn parse_rows(text: &str) -> Vec<PriceRow> {
    let Ok(doc) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(prices) = doc.get("prices").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    prices
        .iter()
        .filter_map(|row| {
            let provider = row.get("model").and_then(|v| v.as_str())?; // presence gate
            let model = provider.to_string();
            let provider = field_str(row, "provider");
            // A row prices a field only when it is present and numeric; the
            // operator's "unknown" strings deliberately stay unpriced.
            let input = decimal_field(row, "input");
            let output = decimal_field(row, "output");
            let reasoning = decimal_field(row, "reasoning");
            let cached_input = decimal_field(row, "cached_input");
            let subscription = input.is_none()
                && cached_input.is_none()
                && output.is_none()
                && reasoning.is_none();
            let plan_fee_usd_month =
                decimal_field(row, "monthly_fee_usd").filter(|fee| fee.is_finite() && *fee >= 0.0);
            if subscription && plan_fee_usd_month.is_none() {
                return None;
            }
            let currency = if subscription {
                "USD".to_string()
            } else {
                field_str(row, "currency")
            };
            if currency == "unknown" {
                return None;
            }
            Some(PriceRow {
                provider,
                model,
                input,
                cached_input,
                output,
                reasoning,
                plan_fee_usd_month: plan_fee_usd_month.filter(|_| subscription),
                confirmed: row.get("confirmed").and_then(toml::Value::as_bool),
                currency,
                price_date: field_str(row, "price_date"),
            })
        })
        .collect()
}

/// Resolution order per the trace contract: env override, user file, embedded.
fn load_rows() -> (Vec<PriceRow>, &'static str) {
    if let Some(path) = std::env::var_os("ANGEL_PRICES_FILE")
        && let Ok(text) = std::fs::read_to_string(&path)
    {
        return (parse_rows(&text), "ANGEL_PRICES_FILE");
    }
    let user: PathBuf = [
        std::env::var_os("HOME").unwrap_or_default(),
        ".angel0/prices.toml".into(),
    ]
    .iter()
    .collect();
    if let Ok(text) = std::fs::read_to_string(&user) {
        return (parse_rows(&text), "~/.angel0/prices.toml");
    }
    (
        parse_rows(include_str!("../../../../docs/telemetry/prices.toml")),
        "embedded:docs/telemetry/prices.toml",
    )
}

/// Documented family fallback: the row id must be a prefix of the record id
/// ending at a `-`, `/`, or `:` boundary (`deepseek-v4-flash-exp` matches row
/// `deepseek-v4-flash`). A bounded prefix shares the family stem by
/// construction; never a guess across families.
fn family_prefix(row_model: &str, id: &str) -> bool {
    let Some(rest) = id.strip_prefix(row_model) else {
        return false;
    };
    rest.is_empty() || rest.starts_with(['-', '/', ':'])
}

fn u64_field(usage: &Value, key: &str) -> Option<u64> {
    usage.get(key).and_then(|v| v.as_u64())
}

/// Compute `cost` for one record from its own `usage` block. Called after the
/// usage ledger is attached. Subscription marginal cost does not require tokens.
pub(super) fn cost_for(model_id: Option<&str>, usage: Option<&Value>) -> Value {
    let Some(model_id) = model_id.filter(|id| !id.is_empty()) else {
        return json!({"paid":null,"cached":null,"currency":null,"price_date":null,
            "basis":"unreported-usage","source":"identity-model-absent","model":"unreported"});
    };
    let (rows, source) = load_rows();
    let row = rows
        .iter()
        .find(|row| row.model == model_id)
        .or_else(|| rows.iter().find(|row| family_prefix(&row.model, model_id)));
    if let Some(row) = row
        && let Some(fee) = row.plan_fee_usd_month
    {
        return json!({"paid":0.0,"cached":null,"currency":"USD",
            "price_date":row.price_date,"basis":"subscription-marginal",
            "plan_fee_usd_month":fee,"confirmed":row.confirmed,
            "source":source,"model":model_id});
    }
    let Some(usage) = usage.filter(|u| u.is_object()) else {
        return json!({"paid":null,"cached":null,"currency":null,"price_date":null,
            "basis":"unreported-usage","source":"no-price-table","model":model_id});
    };
    // Exactly the fields the accounting `usage` block carries, and only when
    // the provider reported them.
    let input = u64_field(usage, "input");
    let uncached_input = u64_field(usage, "uncached_input");
    let cache_read = u64_field(usage, "cache_read");
    let output = u64_field(usage, "output");
    let reasoning = u64_field(usage, "reasoning");
    if input.is_none()
        && uncached_input.is_none()
        && cache_read.is_none()
        && output.is_none()
        && reasoning.is_none()
    {
        return json!({"paid":null,"cached":null,"currency":null,"price_date":null,
            "basis":"unreported-usage","source":"no-price-table","model":model_id});
    }
    let Some(row) = row else {
        return json!({"paid":null,"cached":null,"currency":null,"price_date":null,
            "basis":"no-price-row","source":source,"model":model_id});
    };
    let per_million = |tokens: Option<u64>, rate: Option<f64>| -> Option<f64> {
        match (tokens, rate) {
            (Some(tokens), Some(rate)) => Some(tokens as f64 * rate / 1_000_000.0),
            _ => None,
        }
    };
    // Cached share: tokens billed at the cached rate. Unambiguous when a
    // separate uncached_input total exists, or under the OpenAI convention
    // where the input total includes cached tokens (cache_read <= input).
    let cached_tokens = match (cache_read, uncached_input) {
        (Some(cached), Some(_)) => Some(cached),
        (Some(cached), None) if input.is_some_and(|total| cached <= total) => Some(cached),
        _ => None,
    };
    let mut terms = Vec::new();
    let cached_cost = per_million(cached_tokens, row.cached_input);
    if let Some(cost) = cached_cost {
        terms.push(cost);
    }
    // Uncached input: explicit field, else input minus cached tokens when both
    // were reported and the cached rate applied.
    let uncached = uncached_input.or_else(|| match (input, cached_tokens) {
        (Some(input), Some(cached)) => input.checked_sub(cached),
        (Some(input), None) => Some(input),
        _ => None,
    });
    if let Some(cost) = per_million(uncached, row.input) {
        terms.push(cost);
    }
    if let Some(cost) = per_million(output, row.output) {
        terms.push(cost);
    }
    // Reasoning is priced only when it is a separate reported total; when the
    // provider includes reasoning in output, its price is already counted.
    let reasoning_separate = reasoning.filter(|_| output.is_some());
    if let Some(cost) = per_million(reasoning_separate, row.reasoning) {
        terms.push(cost);
    }
    if terms.is_empty() {
        return json!({"paid":null,"cached":null,"currency":null,"price_date":null,
            "basis":"unreported-usage","source":source,"model":model_id});
    }
    json!({"paid": round_cents(terms.iter().sum()),
        "cached": cached_cost.map(round_cents),
        "currency":row.currency,
        "price_date":row.price_date,
        "basis":"list-price",
        "source":source,
        "model":model_id})
}

/// Money is reported to 8 decimal places — sub-cent precision without float
/// display noise.
fn round_cents(value: f64) -> f64 {
    (value * 1e8).round() / 1e8
}

#[cfg(test)]
mod tests {
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
            crate::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
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
        let _env =
            crate::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
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
        let _env =
            crate::harness::tests::EnvGuard::set("ANGEL_PRICES_FILE", path.to_str().unwrap());
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
}
