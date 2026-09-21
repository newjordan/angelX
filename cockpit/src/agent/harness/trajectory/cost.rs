//! List-price cost binding for the trace ledger (`cost` block).
//!
//! The record's own `usage` totals are priced against the list-price table
//! (`[[prices]]` rows). Unknowns are explicit: a null is always explained by
//! `basis`. Observed balance deltas are joined separately and are never a
//! list price, so they play no part here.
//!
//! Price-table resolution order:
//!   1. `ANGEL_PRICES_FILE` (if set)
//!   2. `~/.angelX/prices.toml`
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
        ".angelX/prices.toml".into(),
    ]
    .iter()
    .collect();
    if let Ok(text) = std::fs::read_to_string(&user) {
        return (parse_rows(&text), "~/.angelX/prices.toml");
    }
    (
        parse_rows(include_str!("../../../../../docs/telemetry/prices.toml")),
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
#[path = "../../../../../tests/cockpit/harness/trajectory__cost__tests.rs"]
mod tests;
