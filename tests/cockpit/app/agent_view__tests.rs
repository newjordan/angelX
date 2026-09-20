use super::*;
use crate::agent_profile::profile_for;

fn flatten(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn profile_lines_include_identity_and_role() {
    let profile = profile_for("spark", false);
    let text = flatten(&profile_lines(profile, "spark", false));
    assert!(text.contains("Sparky"));
    assert!(text.contains("powerhouse"));
    assert!(text.contains("idle"));
    let compact = flatten(&compact_profile_lines(profile, "", false));
    assert!(compact.contains("Sparky"));
    assert!(
        !compact.contains("spark"),
        "caption must not repeat the machine slug: {compact}"
    );
}

/// Precedence is a strict ladder: Blocked beats Tool beats Thinking beats
/// any end-of-turn marker; nothing lit at all reads Idle.
#[test]
fn portrait_state_precedence_blocked_tool_thinking() {
    let fresh = Some((PortraitMarker::Victory, Duration::ZERO));
    assert_eq!(
        portrait_state_from(true, true, true, fresh),
        PortraitState::Blocked
    );
    assert_eq!(
        portrait_state_from(false, true, true, fresh),
        PortraitState::Tool
    );
    assert_eq!(
        portrait_state_from(false, false, true, fresh),
        PortraitState::Thinking
    );
    assert_eq!(
        portrait_state_from(false, false, false, None),
        PortraitState::Idle
    );
}

/// Markers show only while younger than the lifetime, then expire to Idle
/// with no clearing step; settlement picks Recovery only after a fail run
/// of >= 2, and a failed settlement never marks at all.
#[test]
fn portrait_marker_expires_and_recovery_needs_a_fail_run() {
    let fresh = PORTRAIT_MARKER_LIFETIME.saturating_sub(Duration::from_millis(1));
    assert_eq!(
        portrait_state_from(false, false, false, Some((PortraitMarker::Victory, fresh))),
        PortraitState::Victory
    );
    assert_eq!(
        portrait_state_from(false, false, false, Some((PortraitMarker::Recovery, fresh))),
        PortraitState::Recovery
    );
    assert_eq!(
        portrait_state_from(
            false,
            false,
            false,
            Some((PortraitMarker::Victory, PORTRAIT_MARKER_LIFETIME))
        ),
        PortraitState::Idle
    );
    assert_eq!(turn_end_marker(true, 0), Some(PortraitMarker::Victory));
    assert_eq!(turn_end_marker(true, 1), Some(PortraitMarker::Victory));
    assert_eq!(turn_end_marker(true, 2), Some(PortraitMarker::Recovery));
    assert_eq!(turn_end_marker(true, 7), Some(PortraitMarker::Recovery));
    assert_eq!(turn_end_marker(false, 5), None);
}

/// The stateful caption renders the dot + state word in the state color,
/// and the legacy wrapper still compiles by mapping active onto Thinking.
#[test]
fn stateful_caption_carries_dot_and_word() {
    let profile = profile_for("spark", false);
    let lines = compact_profile_lines_stateful(profile, "spark", PortraitState::Blocked);
    let text = flatten(&lines);
    assert!(text.contains("blocked · awaiting approval"), "got: {text}");
    assert!(text.contains("●"), "dot missing: {text}");
    let dot = &lines[1].spans[2];
    assert_eq!(dot.style.fg, Some(HUD_GOLD), "blocked dot must be amber");
    let text = flatten(&compact_profile_lines(profile, "spark", true));
    assert!(text.contains("thinking"), "got: {text}");
    let text = flatten(&compact_profile_lines(profile, "spark", false));
    assert!(text.contains("idle"), "got: {text}");
}

#[test]
fn compact_profile_lines_borrow_the_route_label() {
    let src = include_str!("../../../cockpit/src/agent_view.rs");
    let start = src
        .find("pub(crate) fn compact_profile_lines_stateful")
        .expect("stateful caption");
    let body = src[start..].split("#[cfg(test)]").next().expect("body");
    assert!(
        !body.contains("to_string()"),
        "caption must borrow the route label: {body}"
    );
    assert!(
        body.contains("Span::raw(label)"),
        "caption must span the borrowed label: {body}"
    );
}
