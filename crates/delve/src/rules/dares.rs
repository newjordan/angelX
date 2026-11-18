//! Fortune's dares. On every floor Dame Fortune dares the party: the dare
//! is called as the party arrives, shown in the header, and kept or broken before the
//! stairs. A kept dare pays from Fortune's purse into what each knight
//! carries (gold and a gem). The party leaves a safer road behind them.

use super::*;
use crate::realm::Spoil;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DareKind {
    Untouched,
    StandFirm,
    AgainstTheClock,
    #[serde(alias = "show_off")]
    Pathfinder,
    MakeItQuick,
    Unstoppable,
}

impl DareKind {
    pub fn name(self) -> &'static str {
        match self {
            DareKind::Untouched => "Not a Scratch",
            DareKind::StandFirm => "Stand Firm",
            DareKind::AgainstTheClock => "Against the Clock",
            DareKind::Pathfinder => "Pathfinder",
            DareKind::MakeItQuick => "Make It Quick",
            DareKind::Unstoppable => "Unstoppable",
        }
    }

    pub fn says(self) -> &'static str {
        match self {
            DareKind::Untouched => "two rooms on this floor without a scratch",
            DareKind::StandFirm => "no rolling, the whole floor",
            DareKind::AgainstTheClock => "the stairs within four minutes",
            DareKind::Pathfinder => "clear three rooms on this floor",
            DareKind::MakeItQuick => "fell the guardian within 45 seconds of it rising",
            DareKind::Unstoppable => "four in a breath, twice",
        }
    }

    pub fn card_name(self) -> &'static str {
        match self {
            DareKind::Untouched => "NOT A SCRATCH",
            DareKind::StandFirm => "STAND FIRM",
            DareKind::AgainstTheClock => "BEAT THE CLOCK",
            DareKind::Pathfinder => "PATHFINDER",
            DareKind::MakeItQuick => "MAKE IT QUICK",
            DareKind::Unstoppable => "UNSTOPPABLE",
        }
    }

    pub fn card_rule(self) -> &'static str {
        match self {
            DareKind::Untouched => "TWO CLEAN ROOMS",
            DareKind::StandFirm => "NO ROLLING",
            DareKind::AgainstTheClock => "STAIRS BY 4:00",
            DareKind::Pathfinder => "CLEAR 3 ROOMS",
            DareKind::MakeItQuick => "BOSS IN 45S",
            DareKind::Unstoppable => "4 KILLS, TWICE",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            DareKind::Untouched => "untouched",
            DareKind::StandFirm => "stand_firm",
            DareKind::AgainstTheClock => "clock",
            DareKind::Pathfinder => "pathfinder",
            DareKind::MakeItQuick => "quick",
            DareKind::Unstoppable => "unstoppable",
        }
    }

    fn need(self) -> u32 {
        match self {
            DareKind::Untouched | DareKind::Unstoppable => 2,
            DareKind::Pathfinder => 3,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dare {
    pub kind: DareKind,
    pub since: u64,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub kept: bool,
    #[serde(default)]
    pub broken: bool,
    #[serde(default)]
    pub rose: Option<u64>,
}

pub const CLOCK: u64 = 4 * 60 * HZ as u64;
pub const QUICK: u64 = 45 * HZ as u64;
pub const CALLED: u64 = 5 * HZ as u64;

impl Dare {
    pub fn standing(&self, tick: u64) -> String {
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
            DareKind::Pathfinder => format!("{}/3 rooms cleared", self.count),
            kind => format!("{} of {}", self.count, kind.need()),
        }
    }
}

impl Run {
    pub fn floor_has_guardian(&self) -> bool {
        if self.dungeon.open.is_some() {
            return self
                .enemies
                .iter()
                .any(|e| e.hp > 0 && (e.boss.is_some() || e.kind == EnemyKind::Dragon));
        }
        if let Some(g) = &self.boss_gates {
            let kind = self.dungeon.rooms.get(g.guardian).map(|r| r.kind);
            return !g.guardian_defeated
                && (g.guardian_boss.is_some()
                    || matches!(kind, Some(RoomKind::Lair | RoomKind::Threshold)));
        }
        let pack = self.dungeon.pack;
        let has_boss = self.bosses.iter().any(|b| b.only_in == pack);
        let swamp_above = pack == Pack::Swamp && self.dungeon.depth < swamp::FLOORS;
        self.dungeon.rooms.iter().any(|r| {
            !r.cleared
                && match r.kind {
                    RoomKind::Lair | RoomKind::Threshold => true,
                    RoomKind::Stairs => has_boss && !swamp_above,
                    _ => false,
                }
        })
    }

    pub(super) fn call_dare(&mut self) {
        let depth = self.dungeon.depth;
        if depth == 0 {
            self.dare = None;
            return;
        }
        let bottom = depth >= DEEPEST;
        let guardian = self.floor_has_guardian();
        let kinds: Vec<DareKind> = [
            DareKind::Untouched,
            DareKind::StandFirm,
            DareKind::AgainstTheClock,
            DareKind::Pathfinder,
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
        .filter(|k| guardian || *k != DareKind::MakeItQuick)
        .collect();
        let kind = kinds[self.rng.below(kinds.len())];
        self.dare = Some(Dare {
            kind,
            since: self.tick,
            count: 0,
            kept: false,
            broken: false,
            rose: None,
        });
        self.cues.push(format!("dare:{}", kind.word()));
    }

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
            DareKind::Pathfinder => {
                dare.count += cues.iter().filter(|c| matches!(head(c).as_str(), "room_clear" | "flawless")).count() as u32;
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

    pub(super) fn settle_dare(&mut self) {
        let Some(dare) = self.dare.as_mut().filter(|d| !d.kept && !d.broken) else {
            return;
        };
        if matches!(dare.kind, DareKind::StandFirm | DareKind::AgainstTheClock) {
            dare.kept = true;
            self.pay_dare();
        }
    }

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
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_dares__tests.rs"]
mod tests;
#[cfg(all(test, feature = "bend-cases-games-b"))]
#[path = "../../tests/rules/dares_bend_cases__tests.rs"]
mod dares_bend_cases_tests;
