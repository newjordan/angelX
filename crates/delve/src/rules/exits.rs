//! A retreat is a seat's decision. The last departure tears down the room;
//! earlier departures wait safely at the entrance while friends play on.
use super::*;

/// Half a second of host steps, independent of rendering and client sampling.
pub const LEAVE_TICKS: u32 = HZ.div_ceil(2);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub entry: Option<Entry>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub holding: BTreeMap<u32, u32>,
    pub outside: BTreeMap<u32, Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub at: usize,
    pub party: BTreeMap<u32, (f32, f32)>,
}

pub fn room(kind: RoomKind) -> bool {
    (kind.in_world() && kind != RoomKind::Kingdom)
        || matches!(
            kind,
            RoomKind::Fortune
                | RoomKind::Yard
                | RoomKind::Trophies
                | RoomKind::Tavern
                | RoomKind::Pit
                | RoomKind::Wanted
                | RoomKind::Ledge
                | RoomKind::Forge
                | RoomKind::Holding
                | RoomKind::Kitchen
                | RoomKind::Reading
                | RoomKind::Bellows
                | RoomKind::Ossuary
                | RoomKind::Still
                | RoomKind::Loft
                | RoomKind::Secret
                | RoomKind::Lair
                | RoomKind::Muster | RoomKind::Workshop
                | RoomKind::Stockpile
                | RoomKind::Quarters
        )
}

impl State {
    pub fn valid(&self, rooms: usize) -> bool {
        let valid = |e: &Entry| {
            e.at < rooms
                && e.party.len() <= MAX_PLAYERS as usize
                && e.party.iter().all(|(&id, &(x, y))| {
                    (1..=MAX_PLAYERS).contains(&id)
                        && x.is_finite()
                        && y.is_finite()
                        && x >= 0.0
                        && y >= 0.0
                })
        };
        self.holding.len() <= MAX_PLAYERS as usize
            && self.holding.iter().all(|(&id, &t)| (1..=MAX_PLAYERS).contains(&id) && t < LEAVE_TICKS)
            && self.entry.as_ref().is_none_or(valid)
            && self.outside.len() <= MAX_PLAYERS as usize
            && self
                .outside
                .iter()
                .all(|(&id, e)| (1..=MAX_PLAYERS).contains(&id) && valid(e))
    }
}

impl Run {
    pub fn can_leave_activity(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|_| {
            !self.exits.outside.contains_key(&id) && (room(self.room().kind) || self.minigame_showing() || self.at_privy(id))
        })
    }

    /// A game with its own stage or rules, including its result board.
    pub fn minigame_showing(&self) -> bool {
        self.forest_road_showing() || self.roundtable_showing() || self.cinder_showing() || self.latch_showing() || self.mossbell_showing()
            || self.brasswake_showing() || self.rafterwake_showing() || self.pennon_showing()
            || self.joust.is_some() || self.chariot.is_some() || self.race3d_showing()
            || self.pastimes.round.is_some() || !self.pastimes.heats.is_empty()
            || self.folly.as_ref().is_some_and(|s| s.room == self.at)
            || self.bats.as_ref().is_some_and(|s| s.room == self.at)
            || self.gantry.as_ref().is_some_and(|s| s.room == self.at)
            || self.powder.as_ref().is_some_and(|s| s.room == self.at)
            || self.hop.as_ref().is_some_and(|s| s.room == self.at)
            || self.nook.as_ref().is_some_and(|s| s.room == self.at)
    }

    pub fn can_hold_leave(&self, id: u32) -> bool {
        self.can_leave_activity(id) && (self.minigame_showing() || self.at_privy(id))
    }

    pub fn leave_progress(&self, id: u32) -> u32 {
        self.exits.holding.get(&id).copied().unwrap_or(0)
    }

    pub(super) fn tick_leave_hold(&mut self, inputs: &mut BTreeMap<u32, Input>) {
        let ids: Vec<_> = self.players.keys().copied().collect();
        self.exits.holding.retain(|id, _| ids.contains(id));
        for id in ids {
            if !self.can_hold_leave(id) || !inputs.get(&id).is_some_and(|i| i.valid() && i.leave_held) {
                self.exits.holding.remove(&id);
                continue;
            }
            let ticks = self.leave_progress(id) + 1;
            if ticks >= LEAVE_TICKS {
                self.exits.holding.remove(&id);
                if self.leave_activity(id) { inputs.insert(id, Input::default()); }
            } else {
                self.exits.holding.insert(id, ticks);
            }
        }
    }

    fn at_privy(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|h| {
            h.pit.stop.is_some()
                || h.privy > 0
                || self
                    .room()
                    .privy()
                    .is_some_and(|(x, y)| (h.x - x).hypot(h.y - y) < PRIVY_REACH + 2.0)
        })
    }

    pub(super) fn remember_activity_entry(&mut self, index: usize) {
        if index == self.at {
            return;
        }
        self.exits.holding.clear();
        if !room(self.dungeon.rooms[index].kind)
            && ![self.folly_room, self.gantry_room, self.race3d_room].contains(&Some(index)) {
            self.exits.entry = None;
        } else {
            // Where each knight stands in the room left, kept inside it: a
            // race's own track may put a knight past the room's edge, and an
            // entry outside the room is no snapshot at all (`Exits::valid`),
            // so every page would refuse the run and the save would not come
            // back (the race on the play box, 2026-10-17).
            let r = &self.dungeon.rooms[self.at];
            let (w, h) = (r.width(), r.height());
            let inside = |x: f32, top: f32| if x.is_finite() { x.clamp(0.0, top) } else { top / 2.0 };
            self.exits.entry = Some(Entry {
                at: self.at,
                party: self
                    .players
                    .iter()
                    .map(|(&id, k)| (id, (inside(k.x, w), inside(k.y, h))))
                    .collect(),
            });
        }
    }

    pub(super) fn activity_return(&self) -> Option<Entry> {
        // Out of a Colosseum arena is back into its entrance hall.
        if let Some(hall) = self.arena_return() {
            return Some(hall);
        }
        if let Some(back) = &self.kingdom.return_to
            && (!back.party.is_empty() || self.exits.entry.is_none())
            && let Some(at) = world::room_of(&self.dungeon, RoomKind::Kingdom)
        {
            return Some(Entry {
                at,
                party: self
                    .players
                    .keys()
                    .map(|&id| (id, back.party.get(&id).copied().unwrap_or((back.x, back.y))))
                    .collect(),
            });
        }
        self.exits.entry.clone().or_else(|| {
            // Old saves predate the entry journal. Optional delve halls have
            // one return door; surface rooms return to the Undercroft.
            let at = (0..4)
                .find_map(|d| self.dungeon.neighbour(self.at, d))
                .or_else(|| world::room_of(&self.dungeon, RoomKind::Home))?;
            let r = &self.dungeon.rooms[at];
            Some(Entry {
                at,
                party: self
                    .players
                    .keys()
                    .map(|&id| (id, (r.width() / 2.0, r.height() - 4.5)))
                    .collect(),
            })
        })
    }

    fn release_activity_controls(hero: &mut Hero) {
        hero.stone = false;
        hero.vigil = false;
        hero.vigil_key = false;
        hero.vigil_rearm = 0;
        hero.privy = 0;
        hero.pit.stop = None;
        hero.pit.result = None;
        hero.dress();
        hero.gear_was = hero.gear;
        hero.dash_ticks = 0;
        hero.vy = 0.0;
        hero.jump_held = false;
        hero.dropping = 0;
        hero.footing = None;
        hero.shielding = false;
        hero.walling = false;
        hero.buying = 0;
        hero.buy_spent = false;
        hero.invulnerable = hero.invulnerable.max(HZ);
        hero.hp = hero.hp.max(1);
        hero.aiming = None;
        hero.spin = 0;
        hero.whirl = 0;
        hero.ult = None;
        hero.hexed = 0;
        hero.chilled = 0;
        hero.mired = 0;
    }

    pub fn leave_activity(&mut self, id: u32) -> bool {
        if !self.can_leave_activity(id) {
            return false;
        }
        self.exits.holding.remove(&id);
        self.hitstop.buffered.remove(&id);
        // A privy is a personal pop-up, not a party room transition.
        if self.at_privy(id) {
            let at = self.players[&id]
                .pit
                .stop
                .as_ref()
                .map(|s| s.from)
                .or_else(|| self.room().pit_box());
            let hero = self.players.get_mut(&id).unwrap();
            Self::release_activity_controls(hero);
            if let Some((x, y)) = at {
                (hero.x, hero.y) = (x, y);
            }
            return true;
        }
        let Some(back) = self.activity_return() else {
            return false;
        };
        let hero = self.players.get_mut(&id).unwrap();
        Self::release_activity_controls(hero);
        if let Some(&(x, y)) = back.party.get(&id) {
            (hero.x, hero.y) = (x, y);
        }
        self.exits.outside.insert(id, back.clone());
        self.safe_activity_return(id, back.at);
        let remaining: Vec<_> = self
            .players
            .keys()
            .filter(|i| !self.exits.outside.contains_key(i))
            .copied()
            .collect();
        self.depart_lists(id);
        if let Some(c) = &mut self.chariot {
            c.depart(id);
        }
        self.forfeit_stage(id);
        if self.college.owner == Some(id) {
            self.college = Default::default();
        }
        self.pastimes.depart(id, remaining);
        // The last one out closes the place behind them; a knight who is
        // away (`Run::away`) is no one to wait for, and comes along.
        if self
            .players
            .keys()
            .all(|&i| self.exits.outside.contains_key(&i) || self.away(i))
        {
            self.finish_activity_return(&back);
        }
        self.sounds.push("door_open");
        true
    }

    fn forfeit_stage(&mut self, id: u32) {
        if let Some(s) = &mut self.folly { s.eaten.remove(&id); }
        if let Some(s) = &mut self.bats { s.scores.remove(&id); }
        if let Some(s) = &mut self.gantry { s.jumped.remove(&id); s.height.remove(&id); }
        if let Some(s) = &mut self.powder { s.broken.remove(&id); }
        if let Some(s) = &mut self.hop { s.crossings.remove(&id); }
        if let Some(s) = &mut self.nook { s.score.remove(&id); s.jars.remove(&id); s.brews.remove(&id); }
        if let Some(s) = &mut self.race3d { s.riders.remove(&id); }
    }

    /// A knight waits outside while friends play on inside. Once everyone
    /// still inside is away, the place closes and the party is whole again
    /// where the waiting knight stands, so their doors work. An away knight
    /// left waiting outside goes back in to the party: no stair waits on
    /// them.
    pub(super) fn close_behind_the_away(&mut self) {
        if self.exits.outside.is_empty() {
            return;
        }
        let waiting = self.exits.outside.keys().any(|&id| !self.away(id));
        let inside_away = self
            .players
            .keys()
            .filter(|id| !self.exits.outside.contains_key(id))
            .all(|&id| self.away(id));
        if waiting && inside_away {
            if let Some(back) = self.exits.outside.values().next().cloned() {
                self.finish_activity_return(&back);
            }
            return;
        }
        let gone: Vec<u32> = self.exits.outside.keys().copied().filter(|&id| self.away(id)).collect();
        for id in gone {
            self.rejoin(id);
        }
    }

    /// Knight `id`, waiting outside, back in with the party.
    pub(super) fn rejoin(&mut self, id: u32) {
        self.exits.outside.remove(&id);
        let (w, h) = (self.dungeon.rooms[self.at].width(), self.dungeon.rooms[self.at].height());
        // On an open map, beside a friend standing (else at the arrival):
        // the map's foot is rock.
        let spot = self.dungeon.open.as_ref().filter(|_| self.open_map()).map(|open| {
            let near = self
                .players
                .iter()
                .find(|(other, h)| **other != id && h.hp > 0 && !h.absent && !self.exits.outside.contains_key(other))
                .map_or(open.arrival, |(_, h)| (h.x, h.y));
            self.open_spots(open.map, near, 2)[1]
        });
        if let Some(hero) = self.players.get_mut(&id) {
            (hero.x, hero.y) = spot.unwrap_or((w / 2.0, h - 4.5));
            hero.camera = None;
            hero.invulnerable = HZ;
        }
    }

    pub(super) fn finish_activity_return(&mut self, back: &Entry) {
        // Those out already stand where they walked to; anyone still inside
        // (away) comes out where they went in.
        let positions: BTreeMap<_, _> = self
            .players
            .iter()
            .map(|(&i, h)| {
                let out = self.exits.outside.contains_key(&i);
                (i, if out { (h.x, h.y) } else { back.party.get(&i).copied().unwrap_or((h.x, h.y)) })
            })
            .collect();
        self.exits = State::default();
        // Back into the Colosseum's hall from an arena, the visit goes on:
        // its great door still leads out to the kingdom.
        if self.dungeon.rooms[back.at].kind != RoomKind::ColosseumHall {
            self.kingdom.return_to = None;
        }
        self.clear_game_stages(Some(self.at));
        self.enter(back.at, None);
        for (&i, h) in &mut self.players {
            if let Some(&(x, y)) = positions.get(&i) {
                (h.x, h.y) = (x, y);
            }
            Self::release_activity_controls(h);
        }
        self.kingdom.door_held = true;
        self.stairs_held = true;
        self.exits.entry = None;
    }

    fn safe_activity_return(&mut self, id: u32, at: usize) {
        let room = &self.dungeon.rooms[at];
        let grid = Grid {
            room,
            barred: false,
        };
        let Some(h) = self.players.get_mut(&id) else {
            return;
        };
        // A real doorway records the crossed threshold. Stand inside it,
        // otherwise travel immediately carries the knight back into the game.
        h.x = h.x.clamp(1.6, room.width() - 1.6);
        h.y = h.y.clamp(1.6, room.height() - 1.6);
        if !grid.clear(h.x, h.y, HERO_RADIUS, Mover::Hero) {
            if let Some((x, y)) = (1..room.rows - 1)
                .flat_map(|y| (1..room.cols - 1).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    grid.clear(
                        (x as f32 + 0.5) * TILE_UNITS,
                        (y as f32 + 0.5) * TILE_UNITS,
                        HERO_RADIUS,
                        Mover::Hero,
                    )
                })
                .min_by(|&(ax, ay), &(bx, by)| {
                    let dist = |x: usize, y: usize| {
                        ((x as f32 + 0.5) * TILE_UNITS - h.x)
                            .hypot((y as f32 + 0.5) * TILE_UNITS - h.y)
                    };
                    dist(ax, ay).total_cmp(&dist(bx, by))
                })
            {
                (h.x, h.y) = ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
            }
        }
        h.camera = (room.kind == RoomKind::Kingdom).then_some((h.x, h.y - 1.0));
    }

    /// Seats outside the room walk freely and can rejoin at their entry point.
    /// Their bodies are removed from the activity's working set, so every
    /// targeting, damage, reward and pad test uses only those still inside.
    pub(super) fn detached_seats(&mut self, inputs: &BTreeMap<u32, Input>) -> BTreeMap<u32, Hero> {
        let outside: Vec<_> = self.exits.outside.keys().copied().collect();
        for id in outside {
            let back = &self.exits.outside[&id];
            let input = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .unwrap_or_default();
            let Some(h) = self.players.get_mut(&id) else {
                continue;
            };
            let entry = back.party.get(&id).copied().unwrap_or((h.x, h.y));
            if input.interact && (h.x - entry.0).hypot(h.y - entry.1) < 2.5 {
                self.rejoin(id);
                continue;
            }
            let grid = Grid {
                room: &self.dungeon.rooms[back.at],
                barred: false,
            };
            let (dx, dy) = unit(f32::from(input.move_x), f32::from(input.move_y));
            let (ax, ay) = unit(f32::from(input.aim_x), f32::from(input.aim_y));
            if ax != 0.0 || ay != 0.0 {
                (h.aim_x, h.aim_y) = (ax, ay);
            }
            h.pit.tick();
            // The party's webs and song stay with the party; a road is a road.
            let roads = (grid.room.kind == RoomKind::Kingdom)
                .then(|| map::geography(&self.kingdom.wishes));
            let pace = fx::ground_pace(h, &[], None, roads.as_deref(), &self.tune);
            (h.x, h.y) = grid.slide(
                (h.x, h.y),
                (dx * pace * DT, dy * pace * DT),
                HERO_RADIUS,
                Mover::Hero,
            );
            h.camera = (grid.room.kind == RoomKind::Kingdom).then_some((h.x, h.y - 1.0));
        }
        let ids: Vec<_> = self
            .players
            .keys()
            .copied()
            .filter(|id| {
                self.exits.outside.contains_key(id)
                    || inputs.get(id).is_some_and(|i| {
                        i.valid() && i.activity_menu && self.can_leave_activity(*id)
                    })
            })
            .collect();
        ids.into_iter()
            .filter_map(|id| self.players.remove(&id).map(|h| (id, h)))
            .collect()
    }

    pub fn view_for_seat(&self, id: u32) -> Option<Run> {
        // In a heat each knight sees their own round of the game.
        // In a horse race a friend sees the race from their own team.
        if let Some(index) = self.chariot.as_ref().and_then(|c| c.racer_of(id)).filter(|&i| i > 0) {
            let mut view = self.outside_view(id).unwrap_or_else(|| self.clone());
            if let Some(race) = view.chariot.as_mut() {
                race.racers.swap(0, index);
                // The one who boarded drives the team the friend left, so
                // every chariot still carries its own knight.
                for friend in race.friends.iter_mut().filter(|(_, s)| *s == id) {
                    friend.1 = race.driver;
                }
                race.driver = id;
            }
            return Some(view);
        }
        if let Some(round) = self.pastimes.round.as_ref().filter(|r| r.game == crate::rules::pastimes::Game::Gander) {
            // A knight who has left the hall sees where they went.
            if let Some(view) = self.outside_view(id) {
                return Some(view);
            }
            // The joust hall is one shared hall: every seat's view carries
            // it whole, so friends joust each other on their own screens.
            let mut view = self.clone();
            view.pastimes.round = Some(round.clone());
            view.pastimes.heats.clear();
            return Some(view);
        }
        if let Some(round) = self.pastimes.heats.get(&id) {
            let mut view = self.outside_view(id).unwrap_or_else(|| self.clone());
            view.pastimes.round = Some(round.clone());
            view.pastimes.heats.clear();
            return Some(view);
        }
        self.outside_view(id)
    }

    fn outside_view(&self, id: u32) -> Option<Run> {
        if self.exits.outside.is_empty() {
            return None;
        }
        let mut view = self.clone();
        let Some(back) = self.exits.outside.get(&id) else {
            view.players
                .retain(|id, _| !self.exits.outside.contains_key(id));
            return Some(view);
        };
        let place = self
            .kingdom
            .return_to
            .as_ref()
            .map_or_else(|| spaced(&format!("{:?}", self.room().kind)), |r| r.name.clone());
        let entry = back.party.get(&id).copied().or_else(|| self.players.get(&id).map(|h| (h.x, h.y)));
        view.at = back.at;
        view.players
            .retain(|id, _| self.exits.outside.get(id).is_some_and(|e| e.at == back.at));
        view.clear_activity();
        view.exits = State::default();
        view.kingdom.return_to = None;
        view.kingdom.waiting = entry.map(|entry| super::kingdom::Waiting { place, entry });
        view.phase = Phase::Exploring;
        Some(view)
    }

    fn clear_game_stages(&mut self, room: Option<usize>) {
        // The floor can remember a puzzle in another room. A retreat ends
        // only this stage; a detached view discards all of their pictures.
        if self.folly.as_ref().is_some_and(|s| room.is_none_or(|at| s.room == at)) { self.folly = None; }
        if self.bats.as_ref().is_some_and(|s| room.is_none_or(|at| s.room == at)) { self.bats = None; }
        if self.gantry.as_ref().is_some_and(|s| room.is_none_or(|at| s.room == at)) { self.gantry = None; }
        if self.powder.as_ref().is_some_and(|s| room.is_none_or(|at| s.room == at)) { self.powder = None; }
        if self.hop.as_ref().is_some_and(|s| room.is_none_or(|at| s.room == at)) { self.hop = None; }
        if self.nook.as_ref().is_some_and(|s| room.is_none_or(|at| s.room == at)) { self.nook = None; }
        if self.race3d.as_ref().is_some_and(|s| room.is_none_or(|at| s.room == at)) { self.race3d = None; }
    }

    pub(super) fn clear_activity(&mut self) {
        // The rides end with the room (`activity::RIDES`).
        self.end_outside_activities();
        self.clear_game_stages(None);
        self.forest.leave();
        self.college = Default::default();
        self.pastimes.leave();
        self.waves = Waves::default();
        self.traps.clear();
        self.rocks.clear();
        self.enemies.clear();
        self.projectiles.clear();
        self.imps.clear();
        self.strikes.clear();
        self.spheres.clear();
        self.phantoms.clear();
        self.kegs.clear();
        self.lobs.clear();
        self.flings.clear();
        self.wells.clear();
        self.hooks.clear();
        self.webs.clear();
        self.slams.clear();
        self.ravages.clear();
        self.holes.clear();
        self.critters.clear();
        self.works.leave_room();
        self.sparks.clear();
        self.blasts.clear();
        self.boons.clear();
        self.light = None;
        self.window = None;
        self.rune = None;
        self.build_hold = None;
        self.descending = 0;
        self.descend_for = None;
    }
}

/// `RealmHall` as `Realm Hall`.
fn spaced(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[path = "../../tests/rules/together_exits__tests.rs"]
mod tests;

