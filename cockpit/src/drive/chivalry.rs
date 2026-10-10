//! Local horsemanship and a three-pass practice tournament. No economy, model,
//! benchmark, research or guest-protocol authority. The realm owns this state.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Mount {
    #[default]
    Bramble,
    Cinder,
    Mist,
}
impl Mount {
    pub(crate) const ALL: [Self; 3] = [Self::Bramble, Self::Cinder, Self::Mist];
    pub(crate) fn index(self) -> usize {
        self as usize
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Bramble => "Bramble",
            Self::Cinder => "Cinder",
            Self::Mist => "Mist",
        }
    }
    pub(crate) fn specialty(self) -> Choice {
        match self {
            Self::Bramble => Choice::Guard,
            Self::Cinder => Choice::Charge,
            Self::Mist => Choice::Aim,
        }
    }
    pub(crate) fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Choice {
    Guard,
    Aim,
    Charge,
}
impl Choice {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Guard => "guard",
            Self::Aim => "aim",
            Self::Charge => "charge",
        }
    }
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s {
            "guard" => Some(Self::Guard),
            "aim" => Some(Self::Aim),
            "charge" => Some(Self::Charge),
            _ => None,
        }
    }
    fn beats(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Guard, Self::Charge) | (Self::Charge, Self::Aim) | (Self::Aim, Self::Guard)
        )
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    #[default]
    Ready,
    Running,
    Finished,
    Left,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Tournament {
    pub(crate) phase: Phase,
    pub(crate) mount: Mount,
    pub(crate) prepared: bool,
    pub(crate) passes: [Option<Choice>; 3],
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Chivalry {
    pub(crate) selected: Mount,
    pub(crate) tended: [bool; 3],
    pub(crate) tournament: Tournament,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Place {
    Stables,
    Tournament,
}
impl Place {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "stable" | "stables" => Some(Self::Stables),
            "tournament" | "knights" => Some(Self::Tournament),
            _ => None,
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Stables => "STABLES",
            Self::Tournament => "KNIGHTS TOURNAMENT",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Visit {
    pub(crate) place: Place,
    pub(crate) inside: bool,
    pub(crate) station: u8,
}

impl Tournament {
    pub(crate) fn played(&self) -> usize {
        self.passes.iter().take_while(|p| p.is_some()).count()
    }
    pub(crate) fn rival(round: usize) -> Choice {
        [Choice::Charge, Choice::Guard, Choice::Aim][round.min(2)]
    }
    pub(crate) fn score(&self) -> u8 {
        self.passes
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                c.map(|c| {
                    let rival = Self::rival(i);
                    let base = if c.beats(rival) {
                        3
                    } else if c == rival {
                        1
                    } else {
                        0
                    };
                    base + u8::from(c == self.mount.specialty()) + u8::from(i == 0 && self.prepared)
                })
            })
            .sum()
    }
    pub(crate) fn result(&self) -> &'static str {
        match self.score().cmp(&6) {
            std::cmp::Ordering::Greater => "WIN",
            std::cmp::Ordering::Equal => "DRAW",
            _ => "LOSS",
        }
    }
    pub(crate) fn status(&self) -> String {
        match self.phase {
            Phase::Ready => "Practice lists ready · three passes · no prizes or rewards".into(),
            Phase::Running => format!(
                "{} · pass {}/3 · rival {} · score {}:{}. Choose guard|aim|charge with the pass number.",
                self.mount.name(),
                self.played() + 1,
                Self::rival(self.played()).word(),
                self.score(),
                self.played() * 2
            ),
            Phase::Finished => format!(
                "RESULT · {} on {} · {}:6 · three passes · no rewards",
                self.result(),
                self.mount.name(),
                self.score()
            ),
            Phase::Left => format!(
                "LEFT · {} · {}/3 passes · score {}:{} · no rewards",
                self.mount.name(),
                self.played(),
                self.score(),
                self.played() * 2
            ),
        }
    }
}
/// Decode the optional practice subtree independently from the realm. A valid
/// JSON realm may contain a malformed/newer practice schema; resetting that
/// subtree must not trigger Realm::beside's whole-save fallback.
pub(crate) fn deserialize_saved<'de, D>(deserializer: D) -> Result<Chivalry, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
    let mut state = serde_json::from_value::<Chivalry>(value).unwrap_or_default();
    state.normalize();
    Ok(state)
}

impl Chivalry {
    /// Reject malformed saved progress without changing the treasury or the floor.
    pub(crate) fn normalize(&mut self) {
        let t = &mut self.tournament;
        let n = t.played();
        if t.passes[n..].iter().any(Option::is_some)
            || (t.phase == Phase::Running && n == 3)
            || (t.phase == Phase::Finished && n != 3)
            || (t.phase == Phase::Ready && n != 0)
        {
            *t = Tournament::default();
        }
    }
    pub(crate) fn stable_status(&self) -> String {
        let stalls = Mount::ALL
            .map(|m| {
                format!(
                    "{} · {}{}",
                    m.name(),
                    m.specialty().word(),
                    if self.tended[m.index()] {
                        " · tended"
                    } else {
                        ""
                    }
                )
            })
            .join("; ");
        format!(
            "Stables · selected {} · {stalls}. Select <name>; tend brushes, waters and checks tack, preparing one first-pass point. Mount specialty +1; guard beats charge, charge beats aim, aim beats guard.",
            self.selected.name()
        )
    }
    pub(crate) fn select(&mut self, mount: Mount) -> Result<String, String> {
        if self.tournament.phase == Phase::Running {
            return Err("Mount locked during a tournament; finish or leave first.".into());
        }
        if self.selected == mount {
            return Err(format!("{} is already selected.", mount.name()));
        }
        self.selected = mount;
        Ok(format!(
            "{} selected · specialty {}.",
            mount.name(),
            mount.specialty().word()
        ))
    }
    pub(crate) fn tend(&mut self) -> Result<String, String> {
        if self.tournament.phase == Phase::Running {
            return Err("Tending waits until the tournament ends or you leave.".into());
        }
        let ready = &mut self.tended[self.selected.index()];
        if *ready {
            return Err(format!(
                "{} is already tended; preparation cannot stack.",
                self.selected.name()
            ));
        }
        *ready = true;
        Ok(format!(
            "{} brushed, watered and tack checked · prepared for the next start.",
            self.selected.name()
        ))
    }
    pub(crate) fn start(&mut self) -> Result<String, String> {
        if self.tournament.phase == Phase::Running {
            return Err(
                "Tournament already underway; choose the current numbered pass or leave.".into(),
            );
        }
        let prepared = std::mem::take(&mut self.tended[self.selected.index()]);
        self.tournament = Tournament {
            phase: Phase::Running,
            mount: self.selected,
            prepared,
            passes: [None; 3],
        };
        Ok(self.tournament.status())
    }
    pub(crate) fn choose(&mut self, round: usize, choice: Choice) -> Result<String, String> {
        let t = &mut self.tournament;
        if t.phase != Phase::Running {
            return Err("No tournament underway; start one first.".into());
        }
        if round != t.played() + 1 {
            return Err(format!(
                "Expected pass {}; repeated or out-of-order passes do nothing.",
                t.played() + 1
            ));
        }
        t.passes[round - 1] = Some(choice);
        if t.played() == 3 {
            t.phase = Phase::Finished;
        }
        Ok(t.status())
    }
    pub(crate) fn leave(&mut self) -> Result<String, String> {
        if self.tournament.phase != Phase::Running {
            return Err("No underway tournament to leave.".into());
        }
        self.tournament.phase = Phase::Left;
        Ok(self.tournament.status())
    }
}
#[cfg(test)]
#[path = "../../../tests/cockpit/drive/chivalry__tests.rs"]
mod tests;
