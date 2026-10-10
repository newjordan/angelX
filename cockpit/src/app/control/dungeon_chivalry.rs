//! The stables and the lists from the composer: a way into those rooms of
//! the world, and text forms of what their plates do. Every change goes
//! through the same marks the plates leave, settled by `settle_home`.
use super::super::App;
use crate::drive::chivalry::{Mount, Place};
use crate::drive::together_shooter::{self as shooter, RoomKind};

const HELP: &str = "/dungeon stable [select Bramble|Cinder|Mist|tend|status] · /dungeon tournament [status] · /world visit stables|lists · walk in from the Delve's gate";

impl App {
    /// Open the Delve at the stables or the lists: start a delve if none is
    /// open (it begins at the gate, next door) and walk the party in.
    pub(crate) fn visit_horsemanship(&mut self, place: Place) -> String {
        if self.dungeon.joined.is_some() {
            return format!(
                "You are in a friend's delve: {} is a walk from its gate.",
                place.label().to_lowercase()
            );
        }
        if self.dungeon.shooter.is_none() {
            self.start_shooter(None);
        }
        let kind = match place {
            Place::Stables => RoomKind::Stables,
            Place::Tournament => RoomKind::Lists,
        };
        let Some(run) = self.dungeon.shooter.as_mut() else {
            return self.dungeon.notice.clone();
        };
        if !run.at_home_now() {
            return format!(
                "The party is down in the Delve; {} waits at the gate for the next delve.",
                place.label().to_lowercase()
            );
        }
        if run.joust.is_some() {
            self.expand_dungeon();
            return "A bout is being ridden at the lists.".into();
        }
        if let Some(index) = shooter::world::room_of(&run.dungeon, kind) {
            let at = match place {
                Place::Stables => (18.0, 6.6),
                // On the mount plate itself: its board says who and how.
                Place::Tournament => (2.0, 8.5),
            };
            run.arrive(index, at);
        }
        self.expand_dungeon();
        match place {
            Place::Stables => self.realm_stable_status(),
            Place::Tournament => self.realm_lists_status(),
        }
    }

    fn realm_stable_status(&mut self) -> String {
        let stable = &self.realm().home.stable;
        let stalls = Mount::ALL
            .map(|m| {
                format!(
                    "{}{} ({})",
                    m.name(),
                    if stable.is_tended(m) { ", tended" } else { "" },
                    m.says()
                )
            })
            .join(" · ");
        format!(
            "The stables · saddled: {} · {stalls}. Hold F at a stall's plate to saddle that mount; at the trough to tend it (a knock more of balance for one bout).",
            stable.selected.name()
        )
    }

    fn realm_lists_status(&mut self) -> String {
        let stable = self.realm().home.stable.clone();
        let next = shooter::joust::RIVALS[shooter::joust::next_rival(&stable)].name;
        format!(
            "The lists · next rival: {next} · on {}{} · {} bouts, {} rivals unhorsed. Hold F at the plate by the red pavilion to mount; F spurs, W/S aims high or low, F strikes as the lances meet, Space braces.",
            stable.selected.name(),
            if stable.is_tended(stable.selected) {
                ", tended"
            } else {
                ""
            },
            stable.bouts,
            stable.unhorsed,
        )
    }

    /// `/dungeon stable …` and `/dungeon tournament …`.
    pub(crate) fn chivalry_command(&mut self, place: Place, tail: &str) -> String {
        let args: Vec<_> = tail.split_whitespace().collect();
        let mark = match (place, args.as_slice()) {
            (_, [] | ["enter"]) => return self.visit_horsemanship(place),
            (Place::Stables, ["status"]) => return self.realm_stable_status(),
            (Place::Tournament, ["status"]) => return self.realm_lists_status(),
            (Place::Stables, ["select", name]) => match Mount::parse(name) {
                Some(m) => format!("stable:select:{}", m.name().to_lowercase()),
                None => return format!("No mount called {name}. {HELP}"),
            },
            (Place::Stables, ["tend"]) => "stable:tend".to_string(),
            _ => return format!("Unknown. {HELP}"),
        };
        if self.dungeon.joined.is_some() {
            return "The host's stable is theirs to keep: walk to it in their delve.".into();
        }
        // The same mark a plate leaves, settled the same way.
        let marks = std::collections::BTreeMap::from([(mark, 1u32)]);
        let mut stable = self.realm().home.stable.clone();
        let said = shooter::joust::settle_stable(&mut stable, &marks);
        let realm = self.realm();
        let before = std::mem::replace(&mut realm.home.stable, stable);
        if let Err(error) = realm.save() {
            realm.home.stable = before;
            return format!("Not saved: {error}");
        }
        let (home, treasury) = (realm.home.clone(), realm.treasury.clone());
        if let Some(run) = self.dungeon.shooter.as_mut() {
            run.rebuild_home(home, treasury);
        }
        said.into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/dungeon_chivalry__tests.rs"]
mod tests;
