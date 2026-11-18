//! Where a knight's missions can be done: every mission a knight holds,
//! and the place it names, as plain data the maps mark for that knight's
//! own view.
//!
//! "if a player has a mission, they need a location on the map to see
//! where it can be done" (the operator, 2026-10-19). Each kind of mission
//! names its place its own way; here they all come to a place of the realm:
//!
//! - **A teacher's questline**, its gate open and not yet walked: the
//!   teacher, in the room their file names, at their spot (`at`). A file
//!   with no `at` names the room only. A kingdom questline is held only
//!   while its teacher has news for the knight, and is marked at the door
//!   of the hall they keep.
//! - **Fortune's dare** on this floor: the guardian for Make It Quick, the
//!   stairs for the two judged there. The others are kept in any room.
//! - **King Brannoc's mission**: the Delve, whose halls and guardians he
//!   names.
//! - **A holding's raid**, as Wren pins it: the Delve, whose held floor's
//!   hall the raiders must be thrown out of.
//!
//! The kingdom's errands have the gold arrow already (`kingdom_goal`), and
//! Wren's bounties are pinned for whoever will take them: no knight holds
//! one.

use super::home::Home;
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Spot {
    Guardian,
    Stairs,
    Hall,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Room(RoomKind, Option<(f32, f32)>),
    Ground((i32, i32)),
    Delve(Option<Spot>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub name: String,
    pub place: Place,
}

impl Run {
    pub fn missions_held(&self, id: u32) -> Vec<Held> {
        let Some(hero) = self.players.get(&id) else {
            return Vec::new();
        };
        let key = Home::profile_key(hero);
        let mut held = Vec::new();
        for q in &questline::book().lines {
            if q.is_kingdom() {
                if self.quest_news(&key, q) {
                    let place = match self.hall_door(&q.hall) {
                        Some(door) => Place::Ground(door),
                        None if q.hall.is_empty() => {
                            Place::Room(q.kind(), (q.at != (0, 0)).then(|| (q.at.0 as f32, q.at.1 as f32)))
                        }
                        None => continue,
                    };
                    held.push(Held {
                        name: q.teacher.clone(),
                        place,
                    });
                }
                continue;
            }
            if self.home.questlines.walk(&key, &q.power).finished() || !self.quest_gate(id, q) {
                continue;
            }
            let spot = (q.at != (0, 0)).then(|| (q.at.0 as f32, q.at.1 as f32));
            held.push(Held {
                name: q.teacher.clone(),
                place: Place::Room(q.kind(), spot),
            });
        }
        if let Some(dare) = self.dare.as_ref().filter(|d| !d.kept && !d.broken) {
            let spot = match dare.kind {
                dares::DareKind::MakeItQuick => Some(Spot::Guardian),
                dares::DareKind::StandFirm | dares::DareKind::AgainstTheClock => Some(Spot::Stairs),
                _ => None,
            };
            held.push(Held {
                name: dare.kind.name().into(),
                place: Place::Delve(spot),
            });
        }
        if self.home.barony.mission.is_some() {
            held.push(Held {
                name: "King Brannoc".into(),
                place: Place::Delve(None),
            });
        }
        if let Some((_, hall)) = self.home.barony.halls.asked_hall() {
            held.push(Held {
                name: format!("King Brannoc: {}", hall.name),
                place: Place::Delve(Some(Spot::Hall)),
            });
        }
        for key in &self.home.raids {
            let name = holdings::Deed::of_key(key).map_or_else(|| key.clone(), |d| d.name());
            held.push(Held {
                name: format!("Raiders hold {name}"),
                place: Place::Delve(None),
            });
        }
        held
    }

    pub fn mission_doors(&self, id: u32) -> Vec<(String, (i32, i32))> {
        if self.room().kind != RoomKind::Kingdom || crate::world::plan::tabula() {
            return Vec::new();
        }
        let geo = map::geography(&self.kingdom.wishes);
        let door = |room: RoomKind| {
            geo.sites
                .iter()
                .find(|s| s.room == room)
                .or_else(|| self.gateway(room, &geo))
                .or_else(|| self.stair_to(room, &geo))
                .map(|s| s.stand)
        };
        let mut doors: Vec<(String, (i32, i32), usize)> = Vec::new();
        for held in self.missions_held(id) {
            let stand = match held.place {
                Place::Room(room, _) => door(room),
                Place::Ground(stone) => Some(stone),
                Place::Delve(_) => door(RoomKind::Gate),
            };
            let Some(stand) = stand else {
                continue;
            };
            match doors.iter_mut().find(|(_, s, _)| *s == stand) {
                Some((_, _, more)) => *more += 1,
                None => doors.push((held.name, stand, 0)),
            }
        }
        doors
            .into_iter()
            .map(|(name, stand, more)| match more {
                0 => (name, stand),
                n => (format!("{name} +{n}"), stand),
            })
            .collect()
    }

    pub(super) fn stair_to<'g>(&self, room: RoomKind, geo: &'g map::Geography) -> Option<&'g map::Site> {
        let rooms = &self.dungeon.rooms;
        let start = world::room_of(&self.dungeon, room)?;
        let mut seen = vec![false; rooms.len()];
        let mut queue = std::collections::VecDeque::from([start]);
        seen[start] = true;
        while let Some(i) = queue.pop_front() {
            let kind = rooms[i].kind;
            if let Some(site) = geo.sites.iter().find(|s| s.room == kind) {
                return Some(site);
            }
            let doors = (0..4).filter_map(|dir| self.dungeon.neighbour(i, dir));
            let stairs = world::ENTRANCES.iter().filter_map(|e| match e.to {
                world::Dest::Room(to, _) if e.room == kind => world::room_of(&self.dungeon, to),
                _ => None,
            });
            for j in doors.chain(stairs).collect::<Vec<_>>() {
                if !seen[j] {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        None
    }

    pub fn mission_spots(&self, id: u32) -> Vec<(String, (f32, f32))> {
        if !self.at_home_now() || self.room().kind == RoomKind::Kingdom {
            return Vec::new();
        }
        let here = self.room().kind;
        self.missions_held(id)
            .into_iter()
            .filter_map(|held| match held.place {
                Place::Room(room, Some(spot)) if room == here => Some((held.name, spot)),
                _ => None,
            })
            .collect()
    }

    pub fn mission_floor_spots(&self, id: u32) -> BTreeSet<Spot> {
        if self.at_home_now() {
            return BTreeSet::new();
        }
        self.missions_held(id)
            .into_iter()
            .filter_map(|held| match held.place {
                Place::Delve(spot) => spot,
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "../../tests/rules/missions__tests.rs"]
mod tests;
