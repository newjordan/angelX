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
use crate::agent::club::{ChatMsg, ChatRole, Club};
use std::sync::Arc;
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NeedsProTier {
    Off,
    Fast,
    Pro,
}

pub(crate) const NEEDS_PRO_OPEN: &str = "<<<NEEDS_PRO";
const NEEDS_PRO_CLOSE: &str = ">>>";

pub(crate) const FAST_TIER_CONTRACT: &str = "⠞⠛";

pub(crate) const PRO_TIER_CONTRACT: &str = "⠞⠓";

fn is_contract(content: &str) -> bool {
    content == FAST_TIER_CONTRACT || content == PRO_TIER_CONTRACT
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelfReport {
    None,
    Escalate(Option<String>),
    Ignored(String),
}

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
    let (marker_line_tail, body) = tail.split_once('\n').unwrap_or((tail, ""));
    if !marker_line_tail.trim().is_empty() {
        return None;
    }
    Some((reason, body))
}

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

pub(crate) fn install_contract(history: &mut Vec<ChatMsg>, contract: &str) {
    let leading = history
        .iter()
        .take_while(|message| message.role == ChatRole::System)
        .count();
    if let Some(existing) = history[..leading]
        .iter_mut()
        .find(|message| is_contract(&message.content))
    {
        if existing.content.as_ref() != contract {
            existing.content = Arc::from(contract);
        }
        return;
    }
    history.insert(leading, ChatMsg::system(contract));
}

pub(crate) fn remove_contract(history: &mut Vec<ChatMsg>) {
    history.retain(|message| !(message.role == ChatRole::System && is_contract(&message.content)));
}

fn speak_disarmed(events: &mpsc::Sender<TurnEvent>, why: &str) {
    let _ = events.send(TurnEvent::Notice(format!(
        "needs-pro: armed but {why} — self-escalation disabled"
    )));
}

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
        match crate::agent::tools::consult::find_in_roster(registry.roster(), &wanted) {
            Some(seat) if seat.label().eq_ignore_ascii_case(in_hand.label()) => {
                speak_disarmed(
                    events,
                    &format!("ANGEL_NEEDS_PRO_SEAT={wanted} is the seat already in hand"),
                );
                None
            }
            Some(seat)
                if crate::agent::tools::consult::is_optional_local_label(seat.label())
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
