//! A seat's run, read again only where the snapshot changed.
//!
//! `read` builds the whole run from the tree every tick. A fight's tree
//! changes in a few fields (who moved, which shots were resent, what the
//! tick is); the floor, the book and the rest of the run are the same value
//! they were. This keeps the run it read and puts back only the fields the
//! patch named, then flies shots, foes and knights and counts ages and feel
//! for the new tick without rebuilding those lists. A knight on a course or
//! a clock is landed the way `read` lands it, including a tick that did not
//! resend the list. A field it does not know how to put back, or a patch that
//! is not a struct's, is read whole, so a guest run is the one `read` would
//! have built.

use super::flight::{self, ShotHold};
use super::{Run, read};
use angel_net::doc::delta::Patch;
use angel_net::doc::value::{self, Value};
use std::borrow::Cow;

/// What a patch did to the shot list. Ids only: the tree, after the patch
/// is applied, has the courses.
pub enum Shots {
    /// The list was not in the patch.
    Same,
    /// Ids gone, and ids whose value was resent. Order is the list's old
    /// order with those ids removed, then any new id.
    Edit { gone: Vec<u64>, touched: Vec<u64> },
    /// Replaced or reordered: read the list again.
    Rebuild,
}

/// Wire fields that are not the run's own (`finish`, `aim_sent`). A patch
/// that only names these still leaves the run to be flown for the new tick.
const BESIDE: &[&str] = &[flight::AIM, super::WORLD_ITEM, super::WORLD_CHUNKS, super::WORLD_NEAR];

/// The run a seat is keeping, and the flights of the shots it last read, so
/// a tick that did not resend them flies them again.
#[derive(Default)]
pub struct Guest {
    run: Option<Run>,
    holds: Vec<ShotHold>,
    /// Foes as the tree last carried them (courses and timer starts), so a
    /// tick that did not resend the list can fly them as `read` would.
    foes: Vec<Value>,
    fast: u32,
    full: u32,
    miss: String,
}

impl Guest {
    /// The top-level fields a patch puts back, or `None` when the patch is
    /// not a struct's (the run is read whole).
    pub fn fields(patch: &Patch) -> Option<Vec<Cow<'static, str>>> {
        match patch {
            Patch::Struct { set, gone } => {
                let mut names = Vec::with_capacity(set.len() + gone.len());
                names.extend(set.iter().map(|(name, _)| name.clone()));
                names.extend(gone.iter().cloned());
                Some(names)
            }
            _ => None,
        }
    }

    /// How the patch treats the shot list, read before the patch is applied.
    pub fn shots_of(patch: &Patch) -> Shots {
        let Patch::Struct { set, .. } = patch else {
            return Shots::Rebuild;
        };
        let Some((_, nested)) = set.iter().find(|(name, _)| name == super::SHOTS) else {
            return Shots::Same;
        };
        match nested {
            Patch::KeyedEdit { gone, items } => Shots::Edit {
                gone: gone.clone(),
                touched: items.iter().map(|(id, _)| *id).collect(),
            },
            _ => Shots::Rebuild,
        }
    }

    /// The run `tree` holds. `changed` is the fields the patch named; `None`
    /// reads the tree whole (`read`). `shots` says whether the shot list
    /// can be flown on, or has to be read again.
    pub fn read(
        &mut self,
        tree: &Value,
        changed: Option<&[Cow<'static, str>]>,
        shots: &Shots,
    ) -> Result<Run, value::Error> {
        let Some(changed) = changed else {
            return self.whole(tree);
        };
        let Some(prev) = self.run.take() else {
            return self.whole(tree);
        };
        let old_tick = prev.tick;
        if tree.field("tick").and_then(Value::as_id).is_some_and(|tick| tick < old_tick) {
            self.run = Some(prev);
            return self.whole(tree);
        }
        let mut run = prev;
        // Ages and feel are reckoned against the tick the tree names, which
        // may be listed after the field that needs it.
        if let Some(tick) = tree.field("tick").and_then(Value::as_id) {
            run.tick = tick;
        }
        let mut unknown = Vec::new();
        let mut saw_tick = false;
        let mut saw_enemies = false;
        let mut saw_feel = false;
        let mut saw_players = false;
        let mut saw_shots = false;
        let mut saw_aim = false;
        let mut saw_room = false;
        for name in changed {
            match name.as_ref() {
                "tick" => {
                    if let Some(tick) = tree.field("tick").and_then(Value::as_id) {
                        run.tick = tick;
                    }
                    saw_tick = true;
                }
                "enemies" => {
                    if self.put_enemies(&mut run, tree).is_ok() {
                        saw_enemies = true;
                    } else {
                        unknown.push("enemies");
                    }
                }
                "players" => {
                    if self.put_players(&mut run, tree) {
                        saw_players = true;
                    } else {
                        unknown.push("players");
                    }
                }
                "feel" => {
                    self.put_feel(&mut run, tree)?;
                    saw_feel = true;
                }
                "projectiles" => saw_shots = true,
                flight::AIM => saw_aim = true,
                "dungeon" | "at" => {
                    if !put_field(&mut run, tree, name) {
                        unknown.push(name.as_ref());
                    }
                    saw_room = true;
                }
                other if BESIDE.contains(&other) => {}
                other => {
                    if !put_field(&mut run, tree, other) {
                        unknown.push(other);
                    }
                }
            }
        }
        if !unknown.is_empty() {
            for name in &unknown {
                if !self.miss.split(',').any(|have| have == *name) {
                    if !self.miss.is_empty() {
                        self.miss.push(',');
                    }
                    self.miss.push_str(name);
                }
            }
            return self.whole(tree);
        }
        let tick = run.tick;
        let delta = tick.wrapping_sub(old_tick);
        if delta != 0 && !saw_enemies && !self.fly_foes(&mut run) {
            // No course kept for this list: read the field as it stands.
            if self.put_enemies(&mut run, tree).is_err() {
                return self.whole(tree);
            }
        }
        // A course or a clock is the same value while it holds, so the patch
        // may not name `players`. Land it to this tick anyway, as `read` does.
        if delta != 0 && !saw_players && !self.fly_knights(&mut run, tree) {
            return self.whole(tree);
        }
        if delta != 0 && !saw_feel {
            fade_feel(&mut run.feel, delta);
        }
        if saw_shots || saw_tick || saw_enemies || saw_aim || saw_room {
            self.fly(&mut run, tree, shots)?;
        }
        self.fast += 1;
        let out = run.clone();
        self.run = Some(run);
        Ok(out)
    }

    /// Ticks read by putting fields back, ticks read whole, and the fields
    /// the last whole read was for.
    pub fn stats(&self) -> (u32, u32, &str) {
        (self.fast, self.full, &self.miss)
    }

    fn whole(&mut self, tree: &Value) -> Result<Run, value::Error> {
        let run = read(tree)?;
        self.holds = tree
            .field(super::SHOTS)
            .map(|shots| flight::hold_shots(shots, &run.projectiles))
            .unwrap_or_default();
        self.remember_foes(tree);
        self.full += 1;
        self.run = Some(run.clone());
        Ok(run)
    }

    /// Foes as `read` lands them: courses flown to the tick, timers counted
    /// from their start, ages reckoned from the birth tick the wire stores.
    /// The raw list is kept so a later tick can fly it without reading it
    /// again. A field this cannot land is an error, and the caller reads
    /// the tree whole.
    fn put_enemies(&mut self, run: &mut Run, tree: &Value) -> Result<(), value::Error> {
        let Some(enemies) = tree.field("enemies") else {
            run.enemies.clear();
            self.foes.clear();
            return Ok(());
        };
        let tick = run.tick;
        let landed = if super::foes::needs_land(enemies) {
            super::foes::land(enemies, tick)
        } else {
            enemies.get().clone()
        };
        run.enemies = value::from_value(&landed)?;
        let tick32 = tick as u32;
        for monster in &mut run.enemies {
            monster.age = tick32.wrapping_sub(monster.age);
        }
        self.remember_foes(tree);
        Ok(())
    }

    fn remember_foes(&mut self, tree: &Value) {
        self.foes = match tree.field("enemies").map(Value::get) {
            Some(Value::Seq(items)) => items.clone(),
            _ => Vec::new(),
        };
    }

    /// Fly the kept foe list to this tick, as `land` would. False when this
    /// seat has no list to fly, so the caller reads the field.
    fn fly_foes(&self, run: &mut Run) -> bool {
        if run.enemies.len() != self.foes.len() {
            return false;
        }
        let tick = run.tick;
        let tick32 = tick as u32;
        for (monster, foe) in run.enemies.iter_mut().zip(&self.foes) {
            let (x, y) = super::foes::pos_at(foe, tick);
            monster.x = x;
            monster.y = y;
            if let Some(timer) = super::foes::timer_at(foe, tick) {
                monster.timer = timer;
            }
            let birth = foe.field("age").and_then(Value::as_id).unwrap_or(0) as u32;
            monster.age = tick32.wrapping_sub(birth);
        }
        true
    }

    /// Knights as `read` lands them: a course flown to the tick, a clock wound
    /// from its start, a field the wire left out filled from its serde default.
    /// False when the field is gone or will not deserialize, so the caller
    /// reads the tree whole.
    fn put_players(&self, run: &mut Run, tree: &Value) -> bool {
        let Some(players) = tree.field("players") else {
            return false;
        };
        let tick = run.tick;
        let landed = if super::knights::needs_land(players) {
            super::knights::land(players, tick)
        } else {
            players.get().clone()
        };
        match value::from_value(&landed) {
            Ok(players) => {
                run.players = players;
                true
            }
            Err(_) => false,
        }
    }

    /// The tree's knights, brought to this tick when the patch did not name
    /// them. False when the field is gone or will not land, so the caller
    /// reads the tree whole. A list with no course and no clock is already
    /// the run's.
    fn fly_knights(&self, run: &mut Run, tree: &Value) -> bool {
        match tree.field("players") {
            Some(players) if super::knights::needs_land(players) => self.put_players(run, tree),
            Some(_) => true,
            None => false,
        }
    }

    fn put_feel(&mut self, run: &mut Run, tree: &Value) -> Result<(), value::Error> {
        let Some(feel) = tree.field("feel") else {
            run.feel = Default::default();
            return Ok(());
        };
        let plain = super::unreckon_feel(feel, run.tick);
        run.feel = value::from_value(&plain)?;
        Ok(())
    }

    fn fly(&mut self, run: &mut Run, tree: &Value, shots: &Shots) -> Result<(), value::Error> {
        let Some(list) = tree.field(super::SHOTS) else {
            run.projectiles.clear();
            self.holds.clear();
            return Ok(());
        };
        // The stop indexes the room. A patch that leaves it unreadable is
        // refused, not panicked.
        if !super::room_can_stop_shot(run) {
            return Err(serde::de::Error::custom("the room cannot stop a shot"));
        }
        if tree.field("tick").and_then(Value::as_id).is_some_and(|tick| tick > super::LONGEST_TICK) {
            return Err(serde::de::Error::custom("tick is not a session"));
        }
        let tick = run.tick;
        let marks = flight::marks_of_tree(tree);
        let aim = flight::aim_of(tree);
        let stone = |from, to, body| run.shot_stone(from, to, body);
        let edited = match shots {
            Shots::Edit { gone, touched } if !self.holds.is_empty() && touched.len() <= 24 => self.edit_holds(list, gone, touched, tick, &marks, &stone, &aim),
            Shots::Same if !self.holds.is_empty() => true,
            _ => false,
        };
        if edited {
            let flown = flight::fly_holds(&self.holds, tick, &marks, &stone, &aim);
            let count = match list.get() {
                Value::Seq(items) => items.len(),
                _ => 0,
            };
            if flown.len() == count {
                run.projectiles = flown;
                return Ok(());
            }
        }
        let (projectiles, holds) = flight::place_shots(list, tick, &marks, &stone, &aim)?;
        run.projectiles = projectiles;
        self.holds = holds;
        Ok(())
    }

    /// Drop gone shots and replace the ones resent, keeping the list's order.
    /// False when the holds no longer match the tree, so the caller reads
    /// the list again.
    fn edit_holds(
        &mut self,
        list: &Value,
        gone: &[u64],
        touched: &[u64],
        tick: u64,
        marks: &[(f32, f32)],
        stone: &impl Fn((f32, f32), (f32, f32), f32) -> Option<(f32, f32)>,
        aim: &[(u64, Vec<(f32, f32)>)],
    ) -> bool {
        self.holds.retain(|held| !gone.contains(&held.id));
        let Value::Seq(items) = list.get() else {
            return false;
        };
        for id in touched {
            let Some(shot) = items.iter().find(|shot| shot.field("id").and_then(Value::as_id) == Some(*id)) else {
                return false;
            };
            let Some(hold) = flight::place_one(shot, tick, marks, stone, aim) else {
                return false;
            };
            if let Some(slot) = self.holds.iter_mut().find(|held| held.id == *id) {
                *slot = hold;
            } else {
                self.holds.push(hold);
            }
        }
        items.len() == self.holds.len()
            && items.iter().zip(self.holds.iter()).all(|(shot, held)| shot.field("id").and_then(Value::as_id) == Some(held.id))
    }
}

/// Put one named field of the tree back onto the run. False when the name
/// is not a field this knows, or the patch took it off the tree (read whole).
fn put_field(run: &mut Run, tree: &Value, name: &str) -> bool {
    if name == "module_visuals" {
        let scene = tree.field(name).map(value::from_value).transpose();
        return match scene {
            Ok(scene) => {
                let scene: crate::realm::effects::Scene = scene.unwrap_or_default();
                if scene.validate().is_err() { return false; }
                run.module_visuals = scene;
                true
            }
            Err(_) => false,
        };
    }
    let Some(value) = tree.field(name) else {
        return false;
    };
    macro_rules! put {
        ($($field:ident),* $(,)?) => {
            match name {
                $(stringify!($field) => {
                    match value::from_value(value) {
                        Ok(field) => {
                            run.$field = field;
                            true
                        }
                        Err(_) => false,
                    }
                })*
                _ => false,
            }
        };
    }
    put!(
        playtest, raid_id, naked_start, identity, rite, hitstop, phase, floor_plates, exits, parked, score, dungeon,
        boss_gates, settlement_site, settlement_exhibits, at, kingdom, sites, site_replay, hands, tune, bank, reclaimed, home, treasury, merchant,
        mode, open_floors, spin, collapse_at, deep_beat, strikes, zones, spheres, phantoms, bard_songs, scarecrows, imps,
        cauldron, brewing, first, light, banner, deeds, unboxed, stall, descending, captive, folly, bats, encounter,
        encounter_rooms, gantry_room,
        gantry, powder, hop, race3d_room, race3d, pots_broken, holding_used, holding_raid, crab, forage, valves,
        pigeons, heist, town, nook, reclaim, pot_shards, cat, dare, dares_kept, rune, runes_taken, hexes, round, song, marks,
        lifted, hinted, kegs, lobs, flings, wells, sacs, hooks, webs, slams, ravages, hireling, squires, squire_academy, countryside_squires, squire_hall, squire_field, squire_mark, joust, chariot, forest,
        forest_road, lanternward, moonfall, bannerpeak, griffinwing, lightgallop, deathmatch, college, questline, campaign, alehouse, champions, heroes, gore, minions,
        pastimes, cinder, latch, brasswake, rafterwake, pennon, wayfarers, mossbell, critters, pits, frogs, shoulders, bosses, book,
        found, shake, traps, rocks, waves, works, window, makers, ladders, spectacle, sparks, blasts, floor_tally, floor_results, boons,
        wishing, lamp, genie, asked, held, fellowship, again, rewards,
        // The ballot between floors counts its clock down a tick at a time, and a Knight Racers
        // card puts its whole race there: left out of this list each tick read the run whole.
        crossroads, knight_racers, caves,
    )
}

/// Feel the tree did not resend: each fading line's life is the tick it
/// ends, so a later tick is that many less (`unreckon_feel`). Lines are not
/// dropped here; the host sends the shorter list when one ends.
fn fade_feel(feel: &mut crate::rules::feel::Feel, delta: u64) {
    let sub = |ttl: &mut u8| {
        *ttl = u64::from(*ttl).saturating_sub(delta) as u8;
    };
    for item in &mut feel.melee.swings { sub(&mut item.ttl); }
    for item in &mut feel.melee.hits { sub(&mut item.ttl); }
    for (_, ttl) in feel.melee.chains.values_mut() { sub(ttl); }
    feel.melee.swings.retain(|s| s.ttl > 0);
    feel.melee.hits.retain(|h| h.ttl > 0);
    feel.melee.chains.retain(|_, (_, ttl)| *ttl > 0);
    for item in &mut feel.impacts {
        sub(&mut item.ttl);
    }
    for item in &mut feel.tracers {
        sub(&mut item.ttl);
    }
    for item in &mut feel.puffs {
        sub(&mut item.ttl);
    }
    for item in &mut feel.deaths {
        sub(&mut item.ttl);
    }
    for item in &mut feel.blocks {
        sub(&mut item.ttl);
    }
    for item in &mut feel.glints {
        sub(&mut item.ttl);
    }
    for item in &mut feel.takes {
        sub(&mut item.ttl);
    }
    for item in &mut feel.bolts {
        sub(&mut item.ttl);
    }
}
