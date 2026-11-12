//! The waystations (`crate::world::waystations`): a keeper at every halt on
//! the kingdom's roads. E beside one and they greet the knight, then ask
//! where to: the coach on to the next halt round the ring, the coach back to
//! the one before, or a rally. The coach carries every knight within
//! [`COACH_REACH`] tiles of its post, together, and sets them down on the
//! next halt's apron; a knight left behind has a rally called for them. A
//! rally points every other knight's arrow at the halt for [`RALLY_TICKS`],
//! or until each of them has come within [`RALLY_NEAR`] tiles of it.
use super::*;
use crate::world::waystations::{self as halts, STOPS};

/// How near (tiles) a knight stands to speak with a keeper.
pub const REACH: f32 = 2.6;
/// The coach carries every living knight within this many tiles of its post.
pub const COACH_REACH: f32 = 10.0;
/// A knight this near (tiles) a rally's halt has answered it.
pub const RALLY_NEAR: f32 = 6.0;
/// How long a rally stands, three minutes.
pub const RALLY_TICKS: u64 = 180 * HZ as u64;
/// How long a halt's lantern burns after a coach ran or a rally was called.
pub const BURN: u64 = 20 * HZ as u64;

/// A knight's talk with a keeper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Talk {
    /// Which keeper, by place in the ring.
    pub stop: u8,
    /// 0: the greeting; 1: where to.
    pub page: u8,
    pub opened: u64,
}

/// A rally called at a halt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rally {
    pub stop: u8,
    pub since: u64,
}

/// What the halts are doing: who is talking to whom, the rally standing, the
/// last coach to run, and the knights holding E from a press already taken.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stops {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub talks: BTreeMap<u32, Talk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rally: Option<Rally>,
    /// The last coach to run: the halt it left, the halt it set down at, and
    /// when.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rode: Option<(u8, u8, u64)>,
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub held: std::collections::BTreeSet<u32>,
}

impl Stops {
    pub fn is_idle(&self) -> bool {
        *self == Self::default()
    }

    pub fn valid(&self) -> bool {
        let stop = |s: u8| usize::from(s) < STOPS.len();
        self.talks.len() <= MAX_PLAYERS as usize
            && self.talks.iter().all(|(id, t)| (1..=MAX_PLAYERS).contains(id) && stop(t.stop) && t.page <= 1)
            && self.held.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.rally.is_none_or(|r| stop(r.stop))
            && self.rode.is_none_or(|(from, to, _)| stop(from) && stop(to))
    }

    /// Whether halt `i`'s lantern burns at tick `now`: a coach ran from or to
    /// it, or a rally stands there.
    pub fn burning(&self, i: usize, now: u64) -> bool {
        let recent = |since: u64| now.saturating_sub(since) < BURN;
        self.rode.is_some_and(|(from, to, at)| (usize::from(from) == i || usize::from(to) == i) && recent(at))
            || self.rally.is_some_and(|r| usize::from(r.stop) == i && now.saturating_sub(r.since) < RALLY_TICKS)
    }
}

/// What a knight's talk shows: the keeper's name and halt, their words, and
/// the answers on 1, 2 and 3.
pub struct View {
    pub keeper: &'static str,
    pub halt: &'static str,
    pub words: String,
    pub answers: Vec<String>,
}

impl Run {
    /// The keeper beside knight `id`, if any: its place in the ring.
    pub fn keeper_beside(&self, id: u32) -> Option<usize> {
        let hero = self.players.get(&id)?;
        halts::keeper_near(hero.x, hero.y, REACH * TILE_UNITS)
    }

    /// How many living knights besides `id` are on the road.
    fn others_on_the_road(&self, id: u32) -> usize {
        self.players.iter().filter(|&(&other, h)| other != id && h.hp > 0).count()
    }

    /// Knight `id`'s talk, as shown.
    pub fn stop_talk_view(&self, id: u32) -> Option<View> {
        let talk = self.kingdom.stops.talks.get(&id)?;
        let i = usize::from(talk.stop);
        let stop = STOPS.get(i)?;
        let (words, answers) = if talk.page == 0 {
            (stop.hello.to_string(), vec!["E: GO ON".to_string()])
        } else {
            let onward = STOPS[halts::onward(i)].name.to_uppercase();
            let back = STOPS[halts::back(i)].name.to_uppercase();
            let called = match self.kingdom.stops.rally {
                Some(r) if usize::from(r.stop) == i => "3: THE RALLY STANDS HERE".to_string(),
                _ if self.others_on_the_road(id) == 0 => "3: NO ONE ELSE TO CALL".to_string(),
                _ => "3: CALL THE PARTY HERE".to_string(),
            };
            (
                format!(
                    "Where to? My coach takes every knight within {} tiles of me. A rally points every other knight's arrow here until they come near, or for three minutes.",
                    COACH_REACH as u32
                ),
                vec![format!("1: COACH TO {onward}"), format!("2: COACH TO {back}"), called],
            )
        };
        Some(View { keeper: stop.keeper, halt: stop.name, words, answers })
    }

    /// The keys a halt takes before the rest of the tick sees them: a knight
    /// in talk answers with 1, 2 and 3 rather than casting.
    pub(super) fn stops_inputs(&mut self, inputs: &BTreeMap<u32, Input>) -> Option<BTreeMap<u32, Input>> {
        if self.kingdom.stops.talks.is_empty() || self.room().kind != RoomKind::Kingdom {
            return None;
        }
        let mut out = inputs.clone();
        for (&id, input) in out.iter_mut() {
            if input.valid() && input.cast != 0 && self.kingdom.stops.talks.contains_key(&id) {
                let pick = input.cast;
                input.cast = 0;
                self.answer_keeper(id, pick);
            }
        }
        Some(out)
    }

    /// One tick at the halts: talks end where a knight walks off, E opens a
    /// keeper's greeting and goes on through it, a rally lapses or is
    /// answered.
    pub(super) fn tick_stops(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::Kingdom {
            self.kingdom.stops.talks.clear();
            self.kingdom.stops.held.clear();
            return;
        }
        let gone: Vec<u32> = self
            .kingdom
            .stops
            .talks
            .iter()
            .filter(|(id, t)| {
                self.players.get(id).is_none_or(|h| h.hp == 0)
                    || self.keeper_beside(**id) != Some(usize::from(t.stop))
            })
            .map(|(&id, _)| id)
            .collect();
        for id in gone {
            self.kingdom.stops.talks.remove(&id);
        }
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let down = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if !down {
                self.kingdom.stops.held.remove(&id);
                continue;
            }
            let free = self.players.get(&id).is_some_and(|h| h.hp > 0 && !h.stone)
                && !self.kingdom.door_latched.contains(&id)
                && !self.kingdom.survey.contains_key(&id);
            if !free || !self.kingdom.stops.held.insert(id) {
                continue;
            }
            if let Some(talk) = self.kingdom.stops.talks.get(&id).cloned() {
                // E on an open talk: on to where to, then away.
                if talk.page == 0 {
                    self.kingdom.stops.talks.insert(id, Talk { page: 1, ..talk });
                } else {
                    self.kingdom.stops.talks.remove(&id);
                }
            } else if let Some(i) = self.keeper_beside(id) {
                self.kingdom.stops.talks.insert(id, Talk { stop: i as u8, page: 0, opened: self.tick });
                self.sounds.push("play_card");
            }
        }
        self.tick_rally();
    }

    /// A rally lapses after [`RALLY_TICKS`], or stands down when every living
    /// knight has come near its halt.
    fn tick_rally(&mut self) {
        let Some(rally) = self.kingdom.stops.rally else {
            return;
        };
        let lapsed = self.tick.saturating_sub(rally.since) >= RALLY_TICKS;
        let gathered = self.players.values().filter(|h| h.hp > 0).all(|h| self.near_halt(h, usize::from(rally.stop)));
        if lapsed || gathered {
            self.kingdom.stops.rally = None;
            if gathered && !lapsed && self.players.values().filter(|h| h.hp > 0).count() > 1 {
                self.sounds.push("wish_grant");
                self.found = Some((self.tick, 0, format!("THE PARTY IS GATHERED AT {}", STOPS[usize::from(rally.stop)].name.to_uppercase())));
            }
        }
    }

    /// Whether `hero` stands within [`RALLY_NEAR`] tiles of halt `i`.
    pub(super) fn near_halt(&self, hero: &Hero, i: usize) -> bool {
        halts::placed().iter().find(|p| p.stop == i).is_some_and(|p| {
            let (kx, ky) = ((p.keeper.0 as f32 + 0.5) * TILE_UNITS, (p.keeper.1 as f32 + 0.5) * TILE_UNITS);
            (hero.x - kx).hypot(hero.y - ky) < RALLY_NEAR * TILE_UNITS
        })
    }

    /// Knight `id` answers keeper's where-to with key `pick`.
    fn answer_keeper(&mut self, id: u32, pick: u8) {
        let Some(talk) = self.kingdom.stops.talks.get(&id).cloned() else {
            return;
        };
        if talk.page != 1 {
            return;
        }
        let from = usize::from(talk.stop);
        match pick {
            1 => self.coach(id, from, halts::onward(from)),
            2 => self.coach(id, from, halts::back(from)),
            3 => self.call_rally(id, from),
            _ => {}
        }
    }

    /// The coach from halt `from` to halt `to`: every living knight within
    /// [`COACH_REACH`] tiles of its post rides, together, and is set down on
    /// `to`'s apron side by side. A knight left behind has a rally called
    /// for them at `to`.
    fn coach(&mut self, id: u32, from: usize, to: usize) {
        let placed = halts::placed();
        let (Some(here), Some(there)) = (placed.iter().find(|p| p.stop == from), placed.iter().find(|p| p.stop == to)) else {
            return;
        };
        let post = ((here.coach.0 as f32 + 0.5) * TILE_UNITS, (here.coach.1 as f32 + 0.5) * TILE_UNITS);
        let riders: Vec<u32> = self
            .players
            .iter()
            .filter(|&(&other, h)| {
                h.hp > 0 && (other == id || (h.x - post.0).hypot(h.y - post.1) < COACH_REACH * TILE_UNITS)
            })
            .map(|(&other, _)| other)
            .collect();
        let count = riders.len() as f32;
        let (ax, ay) = there.apron();
        for (slot, &rider) in riders.iter().enumerate() {
            let offset = (slot as f32 - (count - 1.0) / 2.0) * stand_apart(count);
            if let Some(hero) = self.players.get_mut(&rider) {
                (hero.x, hero.y) = (ax * TILE_UNITS + offset, ay * TILE_UNITS);
                hero.camera = Some((hero.x, hero.y - 1.0));
                hero.sitting = false;
                hero.stocked = false;
            }
            self.kingdom.stops.talks.remove(&rider);
        }
        self.kingdom.stops.talks.remove(&id);
        let left = self.players.values().filter(|h| h.hp > 0).count() - riders.len();
        let name = STOPS[to].name.to_uppercase();
        self.kingdom.stops.rode = Some((from as u8, to as u8, self.tick));
        self.sounds.push("roll");
        let take = (self.tick % 2) as usize;
        self.sounds.push(super::chatter::clip(id, "go", take));
        if left > 0 {
            self.kingdom.stops.rally = Some(Rally { stop: to as u8, since: self.tick });
            self.found = Some((self.tick, id, format!("COACH TO {name}: {left} LEFT BEHIND, RALLY CALLED")));
        } else {
            if self.kingdom.stops.rally.is_some() {
                self.kingdom.stops.rally = None;
            }
            self.found = Some((self.tick, id, format!("COACH TO {name}")));
        }
    }

    /// A rally at halt `at`: every other knight's arrow points here.
    fn call_rally(&mut self, id: u32, at: usize) {
        self.kingdom.stops.talks.remove(&id);
        if self.others_on_the_road(id) == 0 {
            self.found = Some((self.tick, id, "NO ONE ELSE IS ON THE ROAD".into()));
            return;
        }
        self.kingdom.stops.rally = Some(Rally { stop: at as u8, since: self.tick });
        self.sounds.push("circle_toll");
        self.found = Some((self.tick, id, format!("RALLY AT {}", STOPS[at].name.to_uppercase())));
    }

    /// Where knight `id` is called to, if a rally stands and they have not
    /// yet come near it: the halt, and the keeper's tile.
    pub fn rally_call(&self, id: u32) -> Option<(&'static str, (i32, i32))> {
        let rally = self.kingdom.stops.rally?;
        let hero = self.players.get(&id)?;
        let i = usize::from(rally.stop);
        if self.near_halt(hero, i) {
            return None;
        }
        let p = halts::placed().iter().find(|p| p.stop == i)?;
        Some((STOPS[i].name, p.keeper))
    }
}

#[cfg(test)]
#[path = "../../tests/rules/waystation__tests.rs"]
mod tests;
