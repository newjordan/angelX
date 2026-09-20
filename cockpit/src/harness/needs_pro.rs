//! Model self-report escalation (`ANGEL_NEEDS_PRO`).
//!
//! Ported from DeepSeek-Reasonix: the model itself decides a task exceeds the
//! tier it is being served on. Nothing here counts failures or ranks seats —
//! the only trigger is the model emitting `<<<NEEDS_PRO>>>` (bare, or with a
//! one-sentence reason) as the first line of a reply, and the only destination
//! is the seat the operator named. Silent escalation is forbidden: every
//! escalated call is announced, and so is an armed-but-unusable gate.
//!
//! Default off. With `ANGEL_NEEDS_PRO` unset the resolver returns before it
//! reads anything else, so no contract is installed and a reply that happens to
//! contain the marker is ordinary text.

use super::{ToolRegistry, TurnEvent, env_flag};
use crate::club::{ChatMsg, ChatRole, Club};
use std::sync::Arc;
use std::sync::mpsc;

/// Which rung of the tier ladder a turn is being served on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NeedsProTier {
    /// Feature disarmed (or no usable seat): no contract, marker is prose.
    Off,
    /// The fast seat with the escalation contract live; a leading marker aborts
    /// the attempt and re-runs the turn on the escalation seat.
    Fast,
    /// The escalation seat: the contract is a stable no-op and a leading marker
    /// is stripped, never escalated again.
    Pro,
}

/// Opening token of the self-report marker. The closing `>>>` may be preceded
/// by `: <reason>`.
pub(crate) const NEEDS_PRO_OPEN: &str = "<<<NEEDS_PRO";
const NEEDS_PRO_CLOSE: &str = ">>>";

/// Leading tag of the injected contract message. Used to find the fragment
/// again so a second turn replaces it instead of stacking another copy, and so
/// the tier swap is a replacement rather than an append.
const CONTRACT_TAG: &str = "[tier contract]";

/// The fast-tier contract. Deliberately a `const` with no seat name, count, or
/// timestamp in it: it rides the front of every request, so any dynamic content
/// here would invalidate a byte-exact provider prefix cache on every turn.
pub(crate) const FAST_TIER_CONTRACT: &str = "[tier contract] You are serving this turn on a \
fast tier. If THIS task clearly needs stronger reasoning than you can bring to it, make \
`<<<NEEDS_PRO>>>` — or `<<<NEEDS_PRO: one-sentence reason>>>` — the very first line of your \
reply and stop there; the turn will be re-run on a stronger seat. Otherwise do the work and \
never mention the marker.";

/// The escalation seat's replacement contract — same stable-string rule. The
/// marker is defused rather than left unmentioned so a model that learned the
/// convention upstream cannot bounce a turn that has nowhere further to go.
pub(crate) const PRO_TIER_CONTRACT: &str = "[tier contract] You are serving this turn on the \
top tier available here. There is no stronger seat to hand it to, so the `<<<NEEDS_PRO>>>` \
marker does nothing — never emit it. Answer the task yourself.";

/// What a completed reply's first line means for the tier ladder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelfReport {
    /// No marker, or the ladder is not armed: ordinary answer text.
    None,
    /// The model asked for a stronger seat, with the reason it gave (if any).
    Escalate(Option<String>),
    /// A marker on the top tier: nothing to escalate to. Carries the reply with
    /// the marker line removed.
    Ignored(String),
}

/// Split a leading `<<<NEEDS_PRO>>>` / `<<<NEEDS_PRO: reason>>>` line off
/// `answer`. The marker must be the whole first non-whitespace line — a reply
/// that merely mentions it further in stays prose, which is what keeps a model
/// explaining the protocol from triggering it.
fn split_marker(answer: &str) -> Option<(Option<String>, &str)> {
    let rest = answer.trim_start().strip_prefix(NEEDS_PRO_OPEN)?;
    let (inside, tail) = rest.split_once(NEEDS_PRO_CLOSE)?;
    if inside.contains('\n') {
        return None;
    }
    let reason = match inside.trim() {
        "" => None,
        text => {
            let reason = text.strip_prefix(':')?.trim();
            (!reason.is_empty()).then(|| reason.to_string())
        }
    };
    // Whatever else shares the marker's line was not part of the marker, so
    // this reply is not the bare control token the contract asks for.
    let (marker_line_tail, body) = tail.split_once('\n').unwrap_or((tail, ""));
    if !marker_line_tail.trim().is_empty() {
        return None;
    }
    Some((reason, body))
}

/// Read a completed reply as a tier self-report.
pub(crate) fn read_self_report(answer: &str, tier: NeedsProTier) -> SelfReport {
    if tier == NeedsProTier::Off {
        return SelfReport::None;
    }
    let Some((reason, body)) = split_marker(answer) else {
        return SelfReport::None;
    };
    if tier == NeedsProTier::Fast {
        SelfReport::Escalate(reason)
    } else {
        SelfReport::Ignored(body.trim_start().to_string())
    }
}

/// Install (or swap) the tier contract inside the leading system run, so the
/// fragment sits on the stable prefix rather than drifting through the tail as
/// the conversation grows. Idempotent: re-running a turn at the same tier
/// leaves history byte-identical.
pub(crate) fn install_contract(history: &mut Vec<ChatMsg>, contract: &str) {
    let leading = history
        .iter()
        .take_while(|message| message.role == ChatRole::System)
        .count();
    if let Some(existing) = history[..leading]
        .iter_mut()
        .find(|message| message.content.starts_with(CONTRACT_TAG))
    {
        if existing.content.as_ref() != contract {
            existing.content = Arc::from(contract);
        }
        return;
    }
    history.insert(leading, ChatMsg::system(contract));
}

/// Drop a previously installed contract — the armed-but-unusable path, where
/// promising the model an escalation that cannot happen would be a lie.
pub(crate) fn remove_contract(history: &mut Vec<ChatMsg>) {
    history.retain(|message| {
        !(message.role == ChatRole::System && message.content.starts_with(CONTRACT_TAG))
    });
}

fn speak_disarmed(events: &mpsc::Sender<TurnEvent>, why: &str) {
    let _ = events.send(TurnEvent::Notice(format!(
        "needs-pro: armed but {why} — self-escalation disabled"
    )));
}

/// Resolve the escalation seat for this turn, or `None` to serve it at
/// [`NeedsProTier::Off`].
///
/// `ANGEL_NEEDS_PRO_SEAT` is matched against the session roster with the same
/// rule every other operator-typed seat name uses
/// ([`crate::tools::consult::find_in_roster`]): exact label first, then a
/// label/model-id substring. No ranking and no implicit fallback — an
/// unresolvable name disarms the feature out loud rather than picking a seat
/// nobody asked for.
pub(crate) fn resolve_seat(
    in_hand: &dyn Club,
    registry: &ToolRegistry,
    history: &mut Vec<ChatMsg>,
    events: &mpsc::Sender<TurnEvent>,
) -> Option<Arc<dyn Club>> {
    if !env_flag("ANGEL_NEEDS_PRO", false) {
        return None;
    }
    let wanted = std::env::var("ANGEL_NEEDS_PRO_SEAT")
        .unwrap_or_default()
        .trim()
        .to_string();
    let seat = if wanted.is_empty() {
        speak_disarmed(events, "no ANGEL_NEEDS_PRO_SEAT");
        None
    } else {
        match crate::tools::consult::find_in_roster(registry.roster(), &wanted) {
            Some(seat) if seat.label().eq_ignore_ascii_case(in_hand.label()) => {
                speak_disarmed(
                    events,
                    &format!("ANGEL_NEEDS_PRO_SEAT={wanted} is the seat already in hand"),
                );
                None
            }
            Some(seat)
                if crate::tools::consult::is_optional_local_label(seat.label())
                    && !seat.is_available() =>
            {
                speak_disarmed(
                    events,
                    &format!("ANGEL_NEEDS_PRO_SEAT={wanted} is not reachable"),
                );
                None
            }
            Some(seat) => Some(seat),
            None => {
                speak_disarmed(
                    events,
                    &format!("ANGEL_NEEDS_PRO_SEAT={wanted} matches no club in the roster"),
                );
                None
            }
        }
    };
    if seat.is_none() {
        remove_contract(history);
    }
    seat
}
