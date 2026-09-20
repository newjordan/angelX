//! Route/context capability formatting (module-breakup: the route-caps
//! cluster, extracted from `draw.rs`).
//!
//! Pure text formatting over [`RouteMetadata`]: evidence stats, context
//! window and pressure, capability facts and badges for the brain-route and
//! header rails. Consumed by the parent and `brain_route_view.rs` through
//! the parent's re-exports.

use super::truncate_control_value;

pub(crate) fn format_evidence_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        format!("{}m{:02}s", ms / 60_000, (ms / 1_000) % 60)
    }
}

pub(crate) fn format_evidence_tokens(tokens: u64) -> String {
    if tokens < 1_000 {
        tokens.to_string()
    } else {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    }
}

pub(crate) fn format_context_window(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        if tokens.is_multiple_of(1_000_000) {
            format!("{}m", tokens / 1_000_000)
        } else {
            format!("{:.1}m", tokens as f64 / 1_000_000.0)
        }
    } else if tokens >= 1_000 {
        if tokens.is_multiple_of(1_000) {
            format!("{}k", tokens / 1_000)
        } else {
            format!("{:.1}k", tokens as f64 / 1_000.0)
        }
    } else {
        tokens.to_string()
    }
}

pub(crate) fn model_capability_detail(metadata: &crate::agent::club::RouteMetadata) -> String {
    let mut facts = route_capability_facts(metadata);
    if let Some(description) = &metadata.description {
        facts.push(description.clone());
    }
    if facts.is_empty() {
        "model: backend-native metadata unavailable".to_string()
    } else {
        format!("model: {}", facts.join(" · "))
    }
}

pub(crate) fn route_capability_facts(metadata: &crate::agent::club::RouteMetadata) -> Vec<String> {
    let mut facts = vec![
        metadata
            .output_budget
            .label(metadata.output_budget_provenance.as_deref()),
    ];
    if let Some(context_window) = metadata.context_window.filter(|tokens| *tokens > 0) {
        facts.push(format!("{} ctx", format_context_window(context_window)));
    }
    if !metadata.input_modalities.is_empty() {
        facts.push(metadata.input_modalities.join("+"));
    }
    if !metadata.speed_tiers.is_empty() {
        facts.push(format!(
            "speed {} available",
            metadata.speed_tiers.join("+")
        ));
    }
    facts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContextPressure {
    Normal,
    Warning(u64),
    Critical(u64),
}

pub(crate) fn context_pressure(
    metadata: &crate::agent::club::RouteMetadata,
    context_used: usize,
) -> ContextPressure {
    let Some(percent) = metadata.context_usage_percent(context_used) else {
        return ContextPressure::Normal;
    };
    if percent >= 95 {
        ContextPressure::Critical(percent)
    } else if percent >= 80 {
        ContextPressure::Warning(percent)
    } else {
        ContextPressure::Normal
    }
}

pub(crate) fn context_pressure_label(percent: u64) -> String {
    if percent > 100 {
        "~100%+".to_string()
    } else {
        format!("~{percent}%")
    }
}

pub(crate) fn context_usage_status(percent: u64) -> String {
    let occupancy = context_pressure_label(percent);
    if percent >= 95 {
        format!("ctx {occupancy} full")
    } else if percent >= 80 {
        format!("ctx {occupancy} tight")
    } else {
        format!("ctx {occupancy}")
    }
}

pub(crate) fn route_context_badge(
    metadata: &crate::agent::club::RouteMetadata,
    context_used: usize,
) -> String {
    if context_used == 0 {
        return String::new();
    }
    metadata
        .context_usage_percent(context_used)
        .map(|percent| format!(" · {}", context_usage_status(percent)))
        .unwrap_or_default()
}

pub(crate) fn compact_header_route_capability_line(
    metadata: &crate::agent::club::RouteMetadata,
    context_used: usize,
    width: usize,
) -> Option<String> {
    let mut facts = Vec::new();
    if let Some(window) = metadata.context_window.filter(|window| *window > 0) {
        if context_used > 0 {
            facts.push(format!(
                "ctx ~{}/{}",
                format_context_window(context_used as u64),
                format_context_window(window)
            ));
        } else {
            facts.push(format!("{} ctx", format_context_window(window)));
        }
    }
    match context_pressure(metadata, context_used) {
        ContextPressure::Warning(percent) => facts.push(format!(
            "{} · compact soon",
            context_pressure_label(percent)
        )),
        ContextPressure::Critical(percent) => facts.push(format!(
            "{} · /compact now",
            context_pressure_label(percent)
        )),
        ContextPressure::Normal => {
            if !metadata.speed_tiers.is_empty() {
                facts.push(format!(
                    "speed {} available",
                    metadata.speed_tiers.join("+")
                ));
            } else if !metadata.input_modalities.is_empty() {
                facts.push(metadata.input_modalities.join("+"));
            }
        }
    }
    if facts.is_empty() {
        return None;
    }
    Some(truncate_control_value(
        &format!("brain · {}", facts.join(" · ")),
        width,
    ))
}
