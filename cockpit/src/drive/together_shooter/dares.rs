//! Fortune's dares. On every floor Dame Fortune dares the party: the dare
//! is called as the party arrives, shown in the header, and kept or broken before the
//! stairs. A kept dare pays from Fortune's purse into what each knight
//! carries (gold, and a gem), and the audience loves it.

use super::*;
use crate::drive::together_realm::Spoil;

/// What Fortune dares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DareKind {
    /// Two rooms on this floor without a scratch.
    Untouched,
    /// No rolling, all floor.
    StandFirm,
    /// The stairs within four minutes of arriving.
    AgainstTheClock,
    /// 300K more viewers on this floor.
    ShowOff,
    /// The guardian felled within forty-five seconds of rising.
    MakeItQuick,
    /// Four in a breath, twice.
    Unstoppable,
}

impl DareKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            DareKind::Untouched => "Not a Scratch",
            DareKind::StandFirm => "Stand Firm",
            DareKind::AgainstTheClock => "Against the Clock",
            DareKind::ShowOff => "Show-Off",
            DareKind::MakeItQuick => "Make It Quick",
            DareKind::Unstoppable => "Unstoppable",
        }
    }

    /// The dare as Fortune puts it.
    pub(crate) fn says(self) -> &'static str {
        match self {
            DareKind::Untouched => "two rooms on this floor without a scratch",
            DareKind::StandFirm => "no rolling, the whole floor",
            DareKind::AgainstTheClock => "the stairs within four minutes",
            DareKind::ShowOff => "300K more viewers on this floor",
            DareKind::MakeItQuick => "fell the guardian within 45 seconds of it rising",
            DareKind::Unstoppable => "four in a breath, twice",
        }
    }

    pub(crate) fn word(self) -> &'static str {
        match self {
            DareKind::Untouched => "untouched",
            DareKind::StandFirm => "stand_firm",
            DareKind::AgainstTheClock => "clock",
            DareKind::ShowOff => "show_off",
            DareKind::MakeItQuick => "quick",
            DareKind::Unstoppable => "unstoppable",
        }
    }

    /// How many of its moment the dare needs (1 for the ones judged once).
    fn need(self) -> u32 {
        match self {
            DareKind::Untouched | DareKind::Unstoppable => 2,
            _ => 1,
        }
    }
}

/// The dare on this floor: what, since when, how far along, and how it
/// stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Dare {
    pub(crate) kind: DareKind,
    pub(crate) since: u64,
    #[serde(default)]
    pub(crate) count: u32,
    #[serde(default)]
    pub(crate) kept: bool,
    #[serde(default)]
    pub(crate) broken: bool,
    /// The show when the dare was called, for Show-Off.
    #[serde(default)]
    pub(crate) viewers: u32,
    /// When the guardian rose, for Make It Quick.
    #[serde(default)]
    pub(crate) rose: Option<u64>,
}

/// Against the Clock's clock, and Make It Quick's.
pub(crate) const CLOCK: u64 = 4 * 60 * HZ as u64;
pub(crate) const QUICK: u64 = 45 * HZ as u64;
/// How long the dare's board shows as the party arrives.
pub(crate) const CALLED: u64 = 5 * HZ as u64;

impl Dare {
    /// How the dare stands, short enough for the sidebar: "1 of 2",
    /// "3:42 left", "kept!", "broken". `audience`: the show now.
    pub(crate) fn standing(&self, tick: u64, audience: u32) -> String {
        if self.kept {
            return "kept!".into();
        }
        if self.broken {
            return "broken".into();
        }
        match self.kind {
            DareKind::AgainstTheClock => {
                let left = CLOCK.saturating_sub(tick.saturating_sub(self.since)) / HZ as u64;
                format!("{}:{:02} left", left / 60, left % 60)
            }
            DareKind::MakeItQuick => match self.rose {
                Some(at) => format!(
                    "{}s left",
                    QUICK.saturating_sub(tick.saturating_sub(at)) / HZ as u64
                ),
                None => "guardian not met".into(),
            },
            DareKind::StandFirm => "no rolls yet".into(),
            DareKind::ShowOff => format!(
                "{} of 300K",
                super::audience::viewers(audience.saturating_sub(self.viewers))
            ),
            kind => format!("{} of {}", self.count, kind.need()),
        }
    }
}

impl Run {
    /// A new floor: Fortune calls her dare, one the floor allows (the
    /// Unknown has no stairs and no guardian).
    pub(super) fn call_dare(&mut self) {
        let depth = self.dungeon.depth;
        if depth == 0 {
            self.dare = None;
            return;
        }
        let bottom = depth >= DEEPEST;
        let kinds: Vec<DareKind> = [
            DareKind::Untouched,
            DareKind::StandFirm,
            DareKind::AgainstTheClock,
            DareKind::ShowOff,
            DareKind::MakeItQuick,
            DareKind::Unstoppable,
        ]
        .into_iter()
        .filter(|k| {
            !bottom
                || !matches!(
                    k,
                    DareKind::StandFirm | DareKind::AgainstTheClock | DareKind::MakeItQuick
                )
        })
        .collect();
        let kind = kinds[self.rng.below(kinds.len())];
        self.dare = Some(Dare {
            kind,
            since: self.tick,
            count: 0,
            kept: false,
            broken: false,
            viewers: self.audience,
            rose: None,
        });
        self.cues.push(format!("dare:{}", kind.word()));
    }

    /// The dare each tick: the moments called since `heard` and the sounds
    /// since `sounded` keep it or break it.
    pub(super) fn watch_dare(&mut self, heard: usize, sounded: usize) {
        let Some(mut dare) = self.dare.clone().filter(|d| !d.kept && !d.broken) else {
            return;
        };
        let cues: Vec<String> = self.cues.get(heard..).unwrap_or_default().to_vec();
        let head = |cue: &str| cue.split(':').next().unwrap_or(cue).to_string();
        match dare.kind {
            DareKind::Untouched => {
                dare.count += cues.iter().filter(|c| head(c) == "flawless").count() as u32;
            }
            DareKind::Unstoppable => {
                dare.count += cues.iter().filter(|c| head(c) == "slay4").count() as u32;
            }
            DareKind::ShowOff => {
                if self.audience >= dare.viewers + 300 {
                    dare.count = 1;
                }
            }
            DareKind::StandFirm => {
                if self
                    .sounds
                    .get(sounded..)
                    .unwrap_or_default()
                    .contains(&"roll")
                {
                    dare.broken = true;
                }
            }
            DareKind::AgainstTheClock => {
                if self.tick.saturating_sub(dare.since) > CLOCK {
                    dare.broken = true;
                }
            }
            DareKind::MakeItQuick => {
                if dare.rose.is_none() && cues.iter().any(|c| head(c) == "boss_rise" || c == "lair")
                {
                    dare.rose = Some(self.tick);
                }
                let fell = cues
                    .iter()
                    .any(|c| head(c) == "boss_fall" || c == "victory");
                match dare.rose {
                    Some(at) if fell && self.tick.saturating_sub(at) <= QUICK => dare.count = 1,
                    Some(at) if self.tick.saturating_sub(at) > QUICK => dare.broken = true,
                    _ => {}
                }
            }
        }
        let judged_at_stairs = matches!(dare.kind, DareKind::StandFirm | DareKind::AgainstTheClock);
        if !judged_at_stairs && dare.count >= dare.kind.need() {
            dare.kept = true;
        }
        if dare.broken {
            self.cues.push("dare_broken".into());
        }
        let kept = dare.kept;
        self.dare = Some(dare);
        if kept {
            self.pay_dare();
        }
    }

    /// The party takes the stairs: a dare judged there is kept if it
    /// wasn't broken on the way.
    pub(super) fn settle_dare(&mut self) {
        let Some(dare) = self.dare.as_mut().filter(|d| !d.kept && !d.broken) else {
            return;
        };
        if matches!(dare.kind, DareKind::StandFirm | DareKind::AgainstTheClock) {
            dare.kept = true;
            self.pay_dare();
        }
    }

    /// Fortune's purse: gold and a gem for every knight standing, and the
    /// audience's delight.
    fn pay_dare(&mut self) {
        let depth = self.dungeon.depth;
        for hero in self.players.values_mut().filter(|h| h.hp > 0) {
            hero.carried.add(Spoil::Gold, 60 + 20 * depth);
            hero.carried.add(Spoil::Gem, 1);
        }
        self.dares_kept += 1;
        self.notice("daredevil");
        if self.dares_kept >= 5 {
            self.notice("fortunes_favourite");
        }
        self.cues.push("dare_kept".into());
        self.sounds.push("wheel_land");
        self.thrill(100);
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_dares__tests.rs"]
mod tests;
