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

/// One record's price features as the leg's plain numbers: token counts and
/// presence flags exactly as the record reported them, and each rate as whole
/// cents per million tokens (a rate of `r` USD/M crosses as `r * 100`). A term
/// is then `tokens * rate_cents` in 1e-8 dollars, which is the native
/// `round(tokens * rate / 1e6 * 1e8)` without any division.
#[derive(serde::Serialize)]
struct CostLegInput {
    subscription: bool,
    has_usage: bool,
    any_usage_field: bool,
    row_exists: bool,
    in_total: u64,
    in_cached: u64,
    in_uncached: u64,
    has_in_total: bool,
    has_cache_read: bool,
    has_uncached: bool,
    has_output: bool,
    out_tokens: u64,
    has_reasoning: bool,
    reason_tokens: u64,
    cached_le_input: bool,
    reasoning_separate: bool,
    rate_cached_p: bool,
    rate_uncached_p: bool,
    rate_output_p: bool,
    rate_reasoning_p: bool,
    rate_cached_cents: u64,
    rate_uncached_cents: u64,
    rate_output_cents: u64,
    rate_reasoning_cents: u64,
    fee_e8: u64,
}

/// The leg's answer: which basis applies and, when the record is priced, the
/// money as e8 numerators. The record's own identity fields (source, model,
/// confirmed, the row's currency and date) are not decisions; they are copied
/// from the row when the record is assembled.
#[derive(serde::Deserialize)]
struct CostLegDecision {
    basis: String,
    #[serde(default)]
    paid_e8: u64,
    #[serde(default)]
    cached_e8: Option<u64>,
    #[serde(default)]
    fee_e8: u64,
}

/// A rate as whole cents per million tokens, or `None` when the leg cannot
/// carry it exactly (negative, non-finite, or finer than a cent). Those records
/// keep the native decision; a card with two decimal places never needs more.
fn rate_cents(rate: Option<f64>) -> Option<(bool, u64)> {
    match rate {
        None => Some((false, 0)),
        Some(rate) if rate.is_finite() && rate >= 0.0 => {
            let cents = (rate * 100.0).round();
            ((cents / 100.0) == rate).then_some((true, cents as u64))
        }
        Some(_) => None,
    }
}

/// A Bend Nat is an immediate below 2^48; the four summed terms keep headroom.
const LEG_NAT_LIMIT: u64 = 1 << 46;

fn fits_leg_nat(tokens: u64, cents: u64) -> bool {
    tokens
        .checked_mul(cents)
        .is_some_and(|product| product < LEG_NAT_LIMIT)
}

fn fee_e8(fee: Option<f64>) -> u64 {
    fee.map(|fee| (fee * 1e8).round() as u64).unwrap_or(0)
}

/// e8 units back to dollars: the same value `round_cents` reports, because both
/// are the nearest f64 to that exact amount.
fn e8_dollars(units: u64) -> f64 {
    units as f64 / 1e8
}

/// Native's separate-reasoning probe: a reasoning total is its own line only
/// when every attempt declared it separate (and output was reported at all).
fn reasoning_separate(usage: Option<&Value>, output: Option<u64>, reasoning: Option<u64>) -> bool {
    reasoning.is_some()
        && output.is_some()
        && usage
            .and_then(|usage| usage.get("reasoning_convention_attempts"))
            .is_none_or(|conventions| {
                conventions["separate"].as_u64() == usage.unwrap()["attempts"].as_u64()
                    && usage.unwrap()["attempts"]
                        .as_u64()
                        .is_some_and(|n| n > 0)
            })
}

/// The decision's features for one record, or `None` when a rate cannot be
/// carried exactly.
fn cost_leg_input(usage: Option<&Value>, row: Option<&PriceRow>) -> Option<CostLegInput> {
    let usage = usage.filter(|value| value.is_object());
    let input = usage.and_then(|usage| u64_field(usage, "input"));
    let uncached_input = usage.and_then(|usage| u64_field(usage, "uncached_input"));
    let cache_read = usage.and_then(|usage| u64_field(usage, "cache_read"));
    let output = usage.and_then(|usage| u64_field(usage, "output"));
    let reasoning = usage.and_then(|usage| u64_field(usage, "reasoning"));
    let (rate_cached_p, rate_cached_cents) = rate_cents(row.and_then(|row| row.cached_input))?;
    let (rate_uncached_p, rate_uncached_cents) = rate_cents(row.and_then(|row| row.input))?;
    let (rate_output_p, rate_output_cents) = rate_cents(row.and_then(|row| row.output))?;
    let (rate_reasoning_p, rate_reasoning_cents) = rate_cents(row.and_then(|row| row.reasoning))?;
    // Bounds only, never a decision: a term the leg cannot hold in a Nat keeps
    // the native record. Uncached tokens are the explicit total or `input`
    // itself, so `max` of the two reported counts bounds them.
    let cached_term = cache_read.is_some()
        && (uncached_input.is_some()
            || match (cache_read, input) {
                (Some(cached), Some(total)) => cached <= total,
                _ => false,
            })
        && rate_cached_p;
    if cached_term && !fits_leg_nat(cache_read.unwrap_or(0), rate_cached_cents) {
        return None;
    }
    if (uncached_input.is_some() || input.is_some())
        && rate_uncached_p
        && !fits_leg_nat(
            uncached_input.unwrap_or(0).max(input.unwrap_or(0)),
            rate_uncached_cents,
        )
    {
        return None;
    }
    if output.is_some() && rate_output_p && !fits_leg_nat(output.unwrap_or(0), rate_output_cents)
    {
        return None;
    }
    if reasoning.is_some()
        && rate_reasoning_p
        && !fits_leg_nat(reasoning.unwrap_or(0), rate_reasoning_cents)
    {
        return None;
    }
    let fee_units = fee_e8(row.and_then(|row| row.plan_fee_usd_month));
    if fee_units >= (1 << 48) {
        return None;
    }
    Some(CostLegInput {
        subscription: row.is_some_and(|row| row.plan_fee_usd_month.is_some()),
        has_usage: usage.is_some(),
        any_usage_field: input.is_some()
            || uncached_input.is_some()
            || cache_read.is_some()
            || output.is_some()
            || reasoning.is_some(),
        row_exists: row.is_some(),
        in_total: input.unwrap_or(0),
        in_cached: cache_read.unwrap_or(0),
        in_uncached: uncached_input.unwrap_or(0),
        has_in_total: input.is_some(),
        has_cache_read: cache_read.is_some(),
        has_uncached: uncached_input.is_some(),
        has_output: output.is_some(),
        out_tokens: output.unwrap_or(0),
        has_reasoning: reasoning.is_some(),
        reason_tokens: reasoning.unwrap_or(0),
        cached_le_input: match (cache_read, input) {
            (Some(cached), Some(total)) => cached <= total,
            _ => false,
        },
        reasoning_separate: reasoning_separate(usage, output, reasoning),
        rate_cached_p,
        rate_uncached_p,
        rate_output_p,
        rate_reasoning_p,
        rate_cached_cents,
        rate_uncached_cents,
        rate_output_cents,
        rate_reasoning_cents,
        fee_e8: fee_units,
    })
}

/// The decision's contract: a basis the native ladder actually produces for
/// this record. Anything else keeps the native leg.
fn cost_leg_check(row: Option<&PriceRow>, decision: &CostLegDecision) -> Result<(), String> {
    let expected_fee = row.and_then(|row| row.plan_fee_usd_month);
    match decision.basis.as_str() {
        "subscription-marginal" if expected_fee.is_some() && decision.fee_e8 == fee_e8(expected_fee) => {
            Ok(())
        }
        "list-price" if row.is_some() => Ok(()),
        "no-price-row" if row.is_none() => Ok(()),
        "unreported-usage" => Ok(()),
        other => Err(format!("out-of-contract cost basis: {other}")),
    }
}

/// The native record as the leg's decision shape, so the parity tests and an
/// out-of-units record answer the same type. Test-only: the shipped leg is the
/// fallback now.
#[cfg(test)]
fn native_decision(
    model_id: &str,
    usage: Option<&Value>,
    row: Option<&PriceRow>,
    source: &str,
) -> CostLegDecision {
    let rows: &[PriceRow] = row.map(std::slice::from_ref).unwrap_or(&[]);
    let value = cost_with_rows(Some(model_id), usage, rows, source);
    let e8 = |key: &str| {
        value[key]
            .as_f64()
            .map(|amount| (amount * 1e8).round() as u64)
    };
    CostLegDecision {
        basis: value["basis"].as_str().unwrap_or("unreported-usage").to_string(),
        paid_e8: e8("paid").unwrap_or(0),
        cached_e8: e8("cached"),
        fee_e8: e8("plan_fee_usd_month").unwrap_or(0),
    }
}

/// The record the native ladder would report, from the leg's decision. Every
/// field is either the decided basis and money, or a copy of the row/record
/// identity that no leg owns.
fn cost_record(
    model_id: &str,
    usage: Option<&Value>,
    row: Option<&PriceRow>,
    source: &str,
    decision: &CostLegDecision,
) -> Value {
    let unpriced = |basis: &str, source: &str| {
        json!({"paid":null,"cached":null,"currency":null,"price_date":null,
            "basis":basis,"source":source,"model":model_id})
    };
    match decision.basis.as_str() {
        "subscription-marginal" => match row {
            Some(row) => json!({
                "paid": 0.0,
                "cached": null,
                "currency": "USD",
                "price_date": row.price_date,
                "basis": "subscription-marginal",
                "plan_fee_usd_month": row.plan_fee_usd_month,
                "confirmed": row.confirmed,
                "source": source,
                "model": model_id,
            }),
            None => unpriced("unreported-usage", "no-price-table"),
        },
        "list-price" => match row {
            Some(row) => json!({
                "paid": e8_dollars(decision.paid_e8),
                "cached": decision.cached_e8.map(e8_dollars),
                "currency": row.currency,
                "price_date": row.price_date,
                "basis": "list-price",
                "source": source,
                "model": model_id,
            }),
            None => unpriced("unreported-usage", "no-price-table"),
        },
        "no-price-row" => unpriced("no-price-row", source),
        // The native ladder names "no-price-table" when the record reported no
        // usable usage block at all, and the table source when it reported
        // fields that no rate priced.
        _ => {
            let priced = usage.filter(|usage| usage.is_object()).is_some_and(|usage| {
                ["input", "uncached_input", "cache_read", "output", "reasoning"]
                    .iter()
                    .any(|key| u64_field(usage, key).is_some())
            });
            unpriced("unreported-usage", if priced { source } else { "no-price-table" })
        }
    }
}

/// One record through the `cost` leg. `None` when the leg cannot be consulted
/// at all.
fn cost_leg(
    model_id: &str,
    usage: Option<&Value>,
    row: Option<&PriceRow>,
    source: &str,
) -> Option<Value> {
    let input = serde_json::to_value(cost_leg_input(usage, row)?).ok()?;
    let (decision, _leg) = crate::agent::harness::live_policy::decide_shipped(
        "cost",
        &input,
        |value: &CostLegDecision| cost_leg_check(row, value),
    );
    Some(cost_record(model_id, usage, row, source, &decision))
}

/// The record's cost through the live leg when it can decide, else natively.
fn cost_decided(
    model_id: Option<&str>,
    usage: Option<&Value>,
    rows: &[PriceRow],
    source: &str,
) -> Value {
    let decided = model_id.filter(|id| !id.is_empty()).and_then(|model_id| {
        let row = rows
            .iter()
            .find(|row| row.model == model_id)
            .or_else(|| rows.iter().find(|row| family_prefix(&row.model, model_id)));
        cost_leg(model_id, usage, row, source)
    });
    decided.unwrap_or_else(|| cost_with_rows(model_id, usage, rows, source))
}

/// Compute `cost` for one record from its own `usage` block. Called after the
/// usage ledger is attached. Subscription marginal cost does not require tokens.
pub(super) fn cost_for(model_id: Option<&str>, usage: Option<&Value>) -> Value {
    let (rows, source) = load_rows();
    cost_decided(model_id, usage, &rows, source)
}

fn cost_with_rows(
    model_id: Option<&str>,
    usage: Option<&Value>,
    rows: &[PriceRow],
    source: &str,
) -> Value {
    let Some(model_id) = model_id.filter(|id| !id.is_empty()) else {
        return json!({"paid":null,"cached":null,"currency":null,"price_date":null,
            "basis":"unreported-usage","source":"identity-model-absent","model":"unreported"});
    };
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
    let reasoning_separate = reasoning.filter(|_| {
        output.is_some()
            && usage
                .get("reasoning_convention_attempts")
                .is_none_or(|conventions| {
                    conventions["separate"].as_u64() == usage["attempts"].as_u64()
                        && usage["attempts"].as_u64().is_some_and(|n| n > 0)
                })
    });
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

/// Price the attempts that actually ran. Aggregate club counters can include
/// retries, failover seats and parallel workers; never price all of them as the
/// final answering model. Missing worker receipts leave the total unknown.
pub(super) fn cost_for_attempts(
    answer_model: Option<&str>,
    usage: Option<&Value>,
    samples: &[Value],
) -> Value {
    if samples.is_empty() && usage.is_none_or(|usage| usage["attempts"].as_u64() == Some(0)) {
        return cost_for(answer_model, None);
    }
    let (rows, source) = load_rows();
    let attempts: Vec<_> = samples
        .iter()
        .map(|sample| {
            let model = sample["identity"]["model"]["id"]
                .as_str()
                .filter(|model| !model.is_empty() && *model != "unbound");
            let mut cost = cost_decided(model, Some(&sample["usage_accounting"]), &rows, source);
            cost["attempt_id"] = sample["id"].clone();
            cost
        })
        .collect();
    let covered = usage.is_some_and(|usage| {
        usage["attempts"].as_u64() == Some(samples.len() as u64)
            && usage["untracked_sources"] != true
            && usage["overflowed"] != true
    }) && !samples.is_empty()
        && samples.iter().all(|sample| {
            !sample["end"].is_null()
                && sample["identity"]["model"]["id"]
                    .as_str()
                    .is_some_and(|id| !id.is_empty() && id != "unbound")
        });
    let models: std::collections::BTreeSet<_> = samples
        .iter()
        .filter_map(|sample| sample["identity"]["model"]["id"].as_str())
        .collect();
    let model = if models.len() == 1 {
        models.first().copied()
    } else {
        None
    };
    let currency = attempts.first().and_then(|cost| cost["currency"].as_str());
    let one_currency = currency.is_some_and(|currency| {
        attempts.iter().all(|cost| {
            cost["currency"]
                .as_str()
                .is_some_and(|other| other.eq_ignore_ascii_case(currency))
        })
    });
    let complete = covered
        && one_currency
        && attempts.iter().zip(samples).all(|(cost, sample)| {
            cost["paid"].is_number()
                && (cost["basis"] == "subscription-marginal"
                    || sample["usage_accounting"]["accounting_status"] == "reported")
        });
    let sum = |key: &str| {
        attempts
            .iter()
            .try_fold(0.0, |total, cost| Some(total + cost[key].as_f64()?))
            .map(round_cents)
    };
    let price_date = attempts
        .first()
        .map(|cost| cost["price_date"].clone())
        .filter(|date| attempts.iter().all(|cost| cost["price_date"] == *date));
    let subscription = !attempts.is_empty()
        && attempts
            .iter()
            .all(|cost| cost["basis"] == "subscription-marginal");
    json!({
        "paid": if complete { sum("paid") } else { None },
        "cached": if complete { sum("cached") } else { None },
        "currency": if one_currency { currency } else { None },
        "price_date": price_date,
        "basis": if !complete { "unreported-usage" } else if subscription { "subscription-marginal" } else { "list-price" },
        "source": if covered { "provider-attempts" } else { "incomplete-model-attribution" },
        "model": model,
        "attempts": attempts,
    })
}

/// Money is reported to 8 decimal places — sub-cent precision without float
/// display noise.
fn round_cents(value: f64) -> f64 {
    (value * 1e8).round() / 1e8
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/trajectory__cost__tests.rs"]
mod tests;
