use crate::ui::agent_panel::profile::AgentProfile;
use crate::ui::hud::{HUD_BLUE, HUD_DIM, HUD_GOLD, HUD_PHOSPHOR, HUD_TEXT, HUD_VERIFIED};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::time::Duration;

/// How long a Victory/Recovery marker keeps the portrait lit after a turn
/// settles. Expiry is age-based and pure: the render simply stops matching
/// once the marker is this old — nothing has to tick or clear it.
pub(crate) const PORTRAIT_MARKER_LIFETIME: Duration = Duration::from_secs(4);

/// Short-lived end-of-turn marker recorded by the App at the two settlement
/// sites where `world.turn_ended` fires. Derived from turn facts held on the
/// App (its own consecutive-failure count), never read back from world state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PortraitMarker {
    Victory,
    Recovery,
}

/// The six portrait states on the agent bay, in precedence order:
/// Blocked > Tool > Thinking > Victory/Recovery > Idle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PortraitState {
    /// A pending approval modal is waiting on the operator — the highest-value
    /// slice: a missed modal turns the portrait amber instead of idling mute.
    Blocked,
    /// A tool call is still running (unfinished strip entry).
    Tool,
    /// The model holds the flight slot and is producing.
    Thinking,
    /// The last turn settled clean.
    Victory,
    /// The last turn settled clean after >= 2 consecutive failures.
    Recovery,
    Idle,
}

impl PortraitState {
    /// The state word rendered beside the colored dot.
    pub(crate) fn word(self) -> &'static str {
        match self {
            PortraitState::Blocked => "blocked · awaiting approval",
            PortraitState::Tool => "tooling",
            PortraitState::Thinking => "thinking",
            PortraitState::Victory => "victory",
            PortraitState::Recovery => "recovered",
            PortraitState::Idle => "idle",
        }
    }

    /// Dot + border accent. Victory and Recovery share the alchemical green —
    /// the identity palette reserves HUD_VERIFIED for exactly "verified
    /// success and recovered state"; the word carries the distinction.
    pub(crate) fn color(self) -> Color {
        match self {
            PortraitState::Blocked => HUD_GOLD,
            PortraitState::Tool => HUD_BLUE,
            PortraitState::Thinking => HUD_PHOSPHOR,
            PortraitState::Victory | PortraitState::Recovery => HUD_VERIFIED,
            PortraitState::Idle => HUD_DIM,
        }
    }

    /// Border tint for the agent-bay frame; Idle keeps the default chrome.
    pub(crate) fn border_tint(self) -> Option<Color> {
        match self {
            PortraitState::Idle => None,
            state => Some(state.color()),
        }
    }
}

/// Pure precedence fold from already-sampled signals. The marker travels with
/// its age so expiry is decided here rather than at the sampling site: an
/// expired Victory/Recovery falls straight through to Idle.
pub(crate) fn portrait_state_from(
    blocked: bool,
    tool_running: bool,
    thinking: bool,
    marker: Option<(PortraitMarker, Duration)>,
) -> PortraitState {
    if blocked {
        return PortraitState::Blocked;
    }
    if tool_running {
        return PortraitState::Tool;
    }
    if thinking {
        return PortraitState::Thinking;
    }
    match marker {
        Some((PortraitMarker::Victory, age)) if age < PORTRAIT_MARKER_LIFETIME => {
            PortraitState::Victory
        }
        Some((PortraitMarker::Recovery, age)) if age < PORTRAIT_MARKER_LIFETIME => {
            PortraitState::Recovery
        }
        _ => PortraitState::Idle,
    }
}

/// The marker a settling turn leaves behind: ok after a failure run of >= 2 is
/// a Recovery, a plain ok is a Victory, a failed settlement leaves none.
/// `preceding_fail_run` is the App-side consecutive failed-settlement count
/// BEFORE this turn (turn facts, never world state).
pub(crate) fn turn_end_marker(ok: bool, preceding_fail_run: u32) -> Option<PortraitMarker> {
    if !ok {
        return None;
    }
    Some(if preceding_fail_run >= 2 {
        PortraitMarker::Recovery
    } else {
        PortraitMarker::Victory
    })
}

#[cfg(test)]
pub fn profile_lines(profile: AgentProfile, label: &str, active: bool) -> Vec<Line<'static>> {
    let status = if active { "active" } else { "idle" };
    vec![
        Line::from(vec![
            Span::styled("agent  ", Style::new().fg(HUD_BLUE)),
            Span::styled(
                profile.name,
                Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  label ", Style::new().fg(HUD_BLUE)),
            Span::raw(label.to_string()),
        ]),
        Line::from(vec![
            Span::styled("role   ", Style::new().fg(HUD_BLUE)),
            Span::raw(profile.role),
            Span::styled("  state ", Style::new().fg(HUD_BLUE)),
            Span::raw(status),
        ]),
        Line::from(vec![
            Span::styled("load   ", Style::new().fg(HUD_BLUE)),
            Span::raw(profile.detail),
        ]),
    ]
}

/// Legacy two-signal entry, kept as a wrapper over the stateful caption so
/// existing callers keep compiling: `active` maps onto the Thinking state.
#[allow(dead_code)]
pub fn compact_profile_lines<'a>(
    profile: AgentProfile,
    label: &'a str,
    active: bool,
) -> Vec<Line<'a>> {
    let state = if active {
        PortraitState::Thinking
    } else {
        PortraitState::Idle
    };
    compact_profile_lines_stateful(profile, label, state)
}

/// The compact bay caption with the six-state row: identity line unchanged,
/// the state row carries a colored dot + state word, detail stays dimmed.
/// Static styling per frame — nothing here animates. The route label is
/// borrowed; callers own any formatted preview string.
pub(crate) fn compact_profile_lines_stateful<'a>(
    profile: AgentProfile,
    label: &'a str,
    state: PortraitState,
) -> Vec<Line<'a>> {
    let mut identity = vec![Span::styled(
        profile.name,
        Style::new().fg(HUD_TEXT).add_modifier(Modifier::BOLD),
    )];
    // Skip machine slugs that only repeat the portrait name (`spark` next to
    // Sparky, empty when the box has no extra mode).
    if !label.is_empty() && !label.eq_ignore_ascii_case(profile.name) {
        identity.push(Span::styled(" · ", Style::new().fg(HUD_DIM)));
        identity.push(Span::raw(label));
    }
    // Aligned `key : value` rows (concept chrome: AGENT CORE identity table).
    let key = |name: &'static str| {
        vec![
            Span::styled(name, Style::new().fg(HUD_BLUE)),
            Span::styled(" : ", Style::new().fg(HUD_DIM)),
        ]
    };
    let mut agent_row = key("agent");
    agent_row.extend(identity);
    let mut state_row = key("state");
    state_row.extend([
        Span::styled("● ", Style::new().fg(state.color())),
        Span::styled(
            state.word(),
            Style::new().fg(state.color()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::new().fg(HUD_DIM)),
        Span::styled(profile.role, Style::new().fg(HUD_BLUE)),
    ]);
    let mut info_row = key("route");
    info_row.push(Span::styled(profile.detail, Style::new().fg(HUD_DIM)));
    vec![
        Line::from(agent_row),
        Line::from(state_row),
        Line::from(info_row),
    ]
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/agent_view__tests.rs"]
mod tests;
