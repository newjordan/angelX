//! Deterministic forge and party-turn dungeon prototype. No model or tools run
//! during combat. Blueprints are data, compiled into a fixed combat vocabulary.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

pub(crate) const ITEM_BUDGET: u32 = 24;
pub(crate) const WIDTH: i16 = 21;
pub(crate) const HEIGHT: i16 = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Slot {
    Weapon,
    Spell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Pattern {
    Bolt,
    Spread,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Effect {
    None,
    Pierce,
    Heal,
    Shield,
}

impl Effect {
    fn cost(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Pierce => 4,
            Self::Heal | Self::Shield => 6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Blueprint {
    pub(crate) version: u32,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) slot: Slot,
    pub(crate) power: u32,
    pub(crate) speed: u32,
    pub(crate) range: u32,
    pub(crate) pattern: Pattern,
    pub(crate) effect: Effect,
}

pub(crate) fn safe_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub(crate) fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}

impl Blueprint {
    pub(crate) fn load(workspace: &Path, slug: &str) -> Result<Self, String> {
        if matches!(slug, "spark-wand" | "ember-spell") {
            return serde_json::from_str(if slug == "spark-wand" {
                include_str!("../../assets/together/spark-wand.json")
            } else {
                include_str!("../../assets/together/ember-spell.json")
            })
            .map_err(|e| e.to_string());
        }
        if !safe_slug(slug) {
            return Err("blueprint id must be 1-48 lowercase letters, digits, or hyphens".into());
        }
        let root = workspace.canonicalize().map_err(|e| e.to_string())?;
        let path = root
            .join("together-items")
            .join(format!("{slug}.json"))
            .canonicalize()
            .map_err(|e| format!("cannot open together-items/{slug}.json: {e}"))?;
        if !path.starts_with(&root) {
            return Err("blueprint must remain inside the active workspace".into());
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("blueprint must be a regular file".into());
        }
        let mut bytes = Vec::new();
        file.take(16_385)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 16_384 {
            return Err("blueprint exceeds 16 KiB".into());
        }
        let blueprint: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if blueprint.id != slug {
            return Err("blueprint id must match its filename".into());
        }
        Ok(blueprint)
    }

    pub(crate) fn compile(&self) -> Result<Item, String> {
        if self.version != 1 || !safe_slug(&self.id) || !valid_text(&self.title, 64) {
            return Err("blueprint requires version 1, a safe id, and a title of 1-64 characters without controls".into());
        }
        if matches!(self.effect, Effect::Heal | Effect::Shield)
            && (self.slot != Slot::Spell || self.pattern != Pattern::Bolt)
        {
            return Err("heal and shield require a spell with the bolt pattern".into());
        }
        let mut item = Item {
            id: self.id.clone(),
            title: self.title.clone(),
            slot: self.slot,
            power: self.power.clamp(1, 8),
            speed: self.speed.clamp(1, 6),
            range: self.range.clamp(1, 8),
            pattern: self.pattern,
            effect: self.effect,
        };
        while item.points() > ITEM_BUDGET {
            // Reduce the largest weighted axis; ties resolve deterministically.
            if item.power > 1 && 2 * item.power >= item.speed.max(item.range) {
                item.power -= 1;
            } else if item.range > 1 && item.range >= item.speed {
                item.range -= 1;
            } else if item.speed > 1 {
                item.speed -= 1;
            } else {
                item.power -= 1;
            }
        }
        Ok(item)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Item {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) slot: Slot,
    pub(crate) power: u32,
    pub(crate) speed: u32,
    pub(crate) range: u32,
    pub(crate) pattern: Pattern,
    pub(crate) effect: Effect,
}

impl Item {
    pub(crate) fn points(&self) -> u32 {
        2 * self.power
            + self.speed
            + self.range
            + if self.pattern == Pattern::Spread {
                4
            } else {
                0
            }
            + self.effect.cost()
    }
    pub(crate) fn stats(&self, boost: Option<Pickup>) -> AttackStats {
        let cooldown = (7 - self.speed).max(2);
        let cooldown = if boost == Some(Pickup::Haste) {
            (cooldown / 2).max(2)
        } else {
            cooldown
        };
        let damage = self.power * 4;
        let damage = if boost == Some(Pickup::Power) {
            damage * 5 / 4
        } else {
            damage
        };
        // The aggregate damage ceiling is checked AFTER all modifiers. Spread
        // splits this total; it never multiplies it by the projectile count.
        let damage = damage.min(36).min(cooldown * 8);
        AttackStats {
            damage,
            cooldown,
            energy: damage.div_ceil(4).max(2) + self.effect.cost(),
            targets: if self.pattern == Pattern::Spread || boost == Some(Pickup::Spread) {
                3
            } else if self.effect == Effect::Pierce {
                2
            } else {
                1
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AttackStats {
    pub(crate) damage: u32,
    pub(crate) cooldown: u32,
    pub(crate) energy: u32,
    pub(crate) targets: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Loadout {
    pub(crate) weapon: Option<Item>,
    pub(crate) spell: Option<Item>,
}
impl Loadout {
    pub(crate) fn complete(&self) -> bool {
        self.weapon.is_some() && self.spell.is_some()
    }
    pub(crate) fn equip(&mut self, item: Item) {
        match item.slot {
            Slot::Weapon => self.weapon = Some(item),
            Slot::Spell => self.spell = Some(item),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Direction {
    North,
    South,
    East,
    West,
}
impl Direction {
    fn delta(self) -> (i16, i16) {
        match self {
            Self::North => (0, -1),
            Self::South => (0, 1),
            Self::East => (1, 0),
            Self::West => (-1, 0),
        }
    }
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "north" | "n" => Ok(Self::North),
            "south" | "s" => Ok(Self::South),
            "east" | "e" => Ok(Self::East),
            "west" | "w" => Ok(Self::West),
            _ => Err("direction: north|south|east|west".into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum CombatAction {
    Move(Direction),
    Dash(Direction),
    Fire,
    Cast,
    Wait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Pickup {
    Energy,
    Power,
    Haste,
    Spread,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Hero {
    pub(crate) name: String,
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) hp: u32,
    pub(crate) energy: u32,
    pub(crate) shield: u32,
    pub(crate) loadout: Loadout,
    pub(crate) weapon_cooldown: u32,
    pub(crate) spell_cooldown: u32,
    pub(crate) boost: Option<Pickup>,
    pub(crate) boost_turns: u32,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Enemy {
    pub(crate) id: u32,
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) hp: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    Fighting,
    Stairs,
    Won,
    Wiped,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Run {
    pub(crate) floor: u32,
    pub(crate) tick: u32,
    pub(crate) phase: Phase,
    pub(crate) heroes: BTreeMap<u32, Hero>,
    pub(crate) enemies: Vec<Enemy>,
    pub(crate) danger: BTreeSet<(i16, i16)>,
    pub(crate) pickups: Vec<(i16, i16, Pickup)>,
    pub(crate) pending: BTreeMap<u32, CombatAction>,
    seed: u64,
}

fn distance(a: (i16, i16), b: (i16, i16)) -> u32 {
    (a.0.abs_diff(b.0) + a.1.abs_diff(b.1)).into()
}
fn walkable(x: i16, y: i16) -> bool {
    x > 0
        && x < WIDTH - 1
        && y > 0
        && y < HEIGHT - 1
        && !matches!((x, y), (10, 3) | (10, 4) | (10, 6) | (10, 7))
}

fn clear_shot(origin: (i16, i16), target: (i16, i16)) -> bool {
    let (mut x, mut y) = origin;
    let dx = (target.0 - x).abs();
    let dy = -(target.1 - y).abs();
    let sx = (target.0 - x).signum();
    let sy = (target.1 - y).signum();
    let mut error = dx + dy;
    while (x, y) != target {
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
        if !walkable(x, y) {
            return false;
        }
    }
    true
}

impl Run {
    pub(crate) fn new(seed: u64, heroes: impl IntoIterator<Item = (u32, String, Loadout)>) -> Self {
        let heroes = heroes
            .into_iter()
            .enumerate()
            .map(|(index, (id, name, loadout))| {
                (
                    id,
                    Hero {
                        name,
                        x: 2,
                        y: 1 + index as i16,
                        hp: 100,
                        energy: 100,
                        shield: 0,
                        loadout,
                        weapon_cooldown: 0,
                        spell_cooldown: 0,
                        boost: None,
                        boost_turns: 0,
                    },
                )
            })
            .collect();
        let mut run = Self {
            floor: 1,
            tick: 0,
            phase: Phase::Fighting,
            heroes,
            enemies: Vec::new(),
            danger: BTreeSet::new(),
            pickups: Vec::new(),
            pending: BTreeMap::new(),
            seed,
        };
        run.spawn_floor();
        run
    }

    fn spawn_floor(&mut self) {
        self.enemies = [2, 5, 8]
            .into_iter()
            .enumerate()
            .map(|(index, y)| Enemy {
                id: index as u32 + 1,
                x: 17,
                y,
                hp: 12 + self.floor * 4,
            })
            .collect();
        if self.floor == 3 {
            self.enemies[1].hp = 48;
        }
        self.danger.clear();
        self.pickups.clear();
        self.pending.clear();
        self.phase = Phase::Fighting;
    }

    pub(crate) fn active(&self) -> bool {
        matches!(self.phase, Phase::Fighting | Phase::Stairs)
    }

    pub(crate) fn descend(&mut self) -> Result<String, String> {
        if self.phase != Phase::Stairs {
            return Err("clear the current floor before descending".into());
        }
        if !self.pending.is_empty() {
            return Err("finish the queued party turn before descending".into());
        }
        self.floor += 1;
        self.spawn_floor();
        for (index, hero) in self
            .heroes
            .values_mut()
            .filter(|hero| hero.hp > 0)
            .enumerate()
        {
            hero.x = 2;
            hero.y = 1 + index as i16;
            hero.energy = (hero.energy + 20).min(100);
        }
        Ok(format!(
            "party descends to floor {}{}",
            self.floor,
            if self.floor == 3 {
                " · guardian room"
            } else {
                ""
            }
        ))
    }

    pub(crate) fn queue(&mut self, actor: u32, action: CombatAction) -> Result<String, String> {
        if !matches!(self.phase, Phase::Fighting | Phase::Stairs) {
            return Err("raid ended; host: /together return to forge".into());
        }
        if self.phase == Phase::Stairs && matches!(action, CombatAction::Fire | CombatAction::Cast)
        {
            return Err("floor clear; collect pickups, then host: /together descend".into());
        }
        let hero = self
            .heroes
            .get(&actor)
            .ok_or("you are watching; join the next raid")?;
        if hero.hp == 0 {
            return Err("you are down; the party can finish or return to the forge".into());
        }
        if self.pending.contains_key(&actor) {
            return Err("your action is queued; wait for the other living players".into());
        }
        match &action {
            CombatAction::Move(direction) | CombatAction::Dash(direction) => {
                let (dx, dy) = direction.delta();
                let steps = if matches!(action, CombatAction::Dash(_)) {
                    3
                } else {
                    1
                };
                if (1..=steps).any(|step| {
                    let position = (hero.x + dx * step, hero.y + dy * step);
                    !walkable(position.0, position.1)
                        || self
                            .enemies
                            .iter()
                            .any(|enemy| (enemy.x, enemy.y) == position)
                }) {
                    return Err("a wall or enemy blocks that move".into());
                }
                if steps == 3 && hero.energy < 6 {
                    return Err("dash needs 6 energy".into());
                }
            }
            CombatAction::Fire | CombatAction::Cast => {
                let (item, cooldown) = if action == CombatAction::Fire {
                    (hero.loadout.weapon.as_ref(), hero.weapon_cooldown)
                } else {
                    (hero.loadout.spell.as_ref(), hero.spell_cooldown)
                };
                let item = item.ok_or("forge both a weapon and spell before combat")?;
                if cooldown > 0 {
                    return Err(format!(
                        "item cooling down ({cooldown} party turns); move or wait"
                    ));
                }
                if hero.energy < item.stats(hero.boost).energy {
                    return Err("not enough energy; move or wait to regenerate".into());
                }
            }
            CombatAction::Wait => {}
        }
        self.pending.insert(actor, action);
        let living = self.heroes.values().filter(|hero| hero.hp > 0).count();
        if self.pending.len() < living {
            return Ok(format!(
                "action queued · {}/{living} living players ready",
                self.pending.len()
            ));
        }
        self.advance();
        Ok(format!(
            "party turn {} · floor {} · {} enemies · {:?}",
            self.tick,
            self.floor,
            self.enemies.iter().filter(|enemy| enemy.hp > 0).count(),
            self.phase
        ))
    }

    fn advance(&mut self) {
        self.tick += 1;
        let inputs = std::mem::take(&mut self.pending);
        for hero in self.heroes.values_mut().filter(|hero| hero.hp > 0) {
            hero.energy = (hero.energy + 2).min(100);
            hero.weapon_cooldown = hero.weapon_cooldown.saturating_sub(1);
            hero.spell_cooldown = hero.spell_cooldown.saturating_sub(1);
        }
        let mut moves = BTreeMap::new();
        for (id, input) in &inputs {
            if let CombatAction::Move(direction) | CombatAction::Dash(direction) = input {
                let (dx, dy) = direction.delta();
                let steps = if matches!(input, CombatAction::Dash(_)) {
                    3
                } else {
                    1
                };
                let hero = &self.heroes[id];
                moves.insert(*id, (hero.x + dx * steps, hero.y + dy * steps, steps));
            }
        }
        // Party movement is simultaneous: swaps and following a vacated tile
        // work regardless of actor or arrival order. Equal destinations cancel
        // both moves; cancellations propagate through blocked convoys.
        let conflicts: Vec<_> = moves
            .iter()
            .filter(|(id, (x, y, _))| {
                moves.iter().any(|(other, (other_x, other_y, _))| {
                    other != *id && (x, y) == (other_x, other_y)
                })
            })
            .map(|(id, _)| *id)
            .collect();
        for id in conflicts {
            moves.remove(&id);
        }
        loop {
            let blocked: Vec<_> = moves
                .iter()
                .filter(|(_, (x, y, _))| {
                    self.heroes.iter().any(|(other, hero)| {
                        hero.hp > 0 && (hero.x, hero.y) == (*x, *y) && !moves.contains_key(other)
                    })
                })
                .map(|(id, _)| *id)
                .collect();
            if blocked.is_empty() {
                break;
            }
            for id in blocked {
                moves.remove(&id);
            }
        }
        for (id, (x, y, steps)) in moves {
            let hero = self.heroes.get_mut(&id).unwrap();
            hero.x = x;
            hero.y = y;
            if steps == 3 {
                hero.energy -= 6;
            }
        }
        for (id, input) in &inputs {
            if !matches!(input, CombatAction::Fire | CombatAction::Cast) {
                continue;
            }
            let hero = &self.heroes[id];
            let item = if *input == CombatAction::Fire {
                hero.loadout.weapon.as_ref().unwrap()
            } else {
                hero.loadout.spell.as_ref().unwrap()
            }
            .clone();
            let stats = item.stats(hero.boost);
            let origin = (hero.x, hero.y);
            let hero = self.heroes.get_mut(id).unwrap();
            hero.energy -= stats.energy;
            if *input == CombatAction::Fire {
                hero.weapon_cooldown = stats.cooldown - 1;
            } else {
                hero.spell_cooldown = stats.cooldown - 1;
            }
            if matches!(item.effect, Effect::Heal | Effect::Shield) {
                let target = self
                    .heroes
                    .iter()
                    .filter(|(_, hero)| {
                        hero.hp > 0
                            && distance(origin, (hero.x, hero.y)) <= item.range
                            && clear_shot(origin, (hero.x, hero.y))
                    })
                    .min_by_key(|(id, hero)| (hero.hp, **id))
                    .map(|(id, _)| *id);
                if let Some(target) = target {
                    let hero = self.heroes.get_mut(&target).unwrap();
                    if item.effect == Effect::Heal {
                        hero.hp = (hero.hp + stats.damage).min(100);
                    } else {
                        hero.shield = (hero.shield + stats.damage).min(24);
                    }
                }
            } else {
                let mut targets: Vec<_> = self
                    .enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, enemy)| {
                        enemy.hp > 0
                            && distance(origin, (enemy.x, enemy.y)) <= item.range
                            && clear_shot(origin, (enemy.x, enemy.y))
                    })
                    .map(|(index, enemy)| (distance(origin, (enemy.x, enemy.y)), index))
                    .collect();
                targets.sort_unstable();
                targets.truncate(stats.targets);
                let count = targets.len() as u32;
                for (offset, (_, index)) in targets.into_iter().enumerate() {
                    let damage =
                        stats.damage / count + u32::from((offset as u32) < stats.damage % count);
                    self.enemies[index].hp = self.enemies[index].hp.saturating_sub(damage);
                }
            }
        }
        for enemy in self.enemies.iter().filter(|enemy| enemy.hp == 0) {
            let pickup = match (self
                .seed
                .wrapping_add(enemy.id as u64)
                .wrapping_add(self.floor as u64))
                % 4
            {
                0 => Pickup::Energy,
                1 => Pickup::Power,
                2 => Pickup::Haste,
                _ => Pickup::Spread,
            };
            self.pickups.push((enemy.x, enemy.y, pickup));
        }
        self.enemies.retain(|enemy| enemy.hp > 0);
        for hero in self.heroes.values_mut().filter(|hero| hero.hp > 0) {
            if !self.enemies.is_empty() && self.danger.contains(&(hero.x, hero.y)) {
                let damage = 8 + self.floor * 2;
                let absorbed = hero.shield.min(damage);
                hero.shield -= absorbed;
                hero.hp = hero.hp.saturating_sub(damage - absorbed);
            }
            if hero.hp == 0 {
                continue;
            }
            hero.boost_turns = hero.boost_turns.saturating_sub(1);
            if hero.boost_turns == 0 {
                hero.boost = None;
            }
            if let Some(index) = self
                .pickups
                .iter()
                .position(|(x, y, _)| (*x, *y) == (hero.x, hero.y))
            {
                let (_, _, pickup) = self.pickups.remove(index);
                if pickup == Pickup::Energy {
                    hero.energy = (hero.energy + 30).min(100);
                } else {
                    hero.boost = Some(pickup);
                    hero.boost_turns = 8;
                }
            }
        }
        self.danger.clear();
        let positions: Vec<_> = self
            .heroes
            .values()
            .filter(|hero| hero.hp > 0)
            .map(|hero| (hero.x, hero.y))
            .collect();
        if positions.is_empty() {
            self.phase = Phase::Wiped;
        } else if self.enemies.is_empty() {
            self.phase = if self.floor == 3 {
                Phase::Won
            } else {
                Phase::Stairs
            };
        } else if self.tick.is_multiple_of(3) {
            for index in 0..self.enemies.len() {
                let (x, y) = positions[(index + self.tick as usize) % positions.len()];
                self.danger.insert((x, y));
                if self.floor == 3 {
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if walkable(x + dx, y + dy) {
                            self.danger.insert((x + dx, y + dy));
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn board(&self) -> Vec<String> {
        (0..HEIGHT)
            .map(|y| {
                (0..WIDTH)
                    .map(|x| {
                        if !walkable(x, y) {
                            '#'
                        } else if let Some((id, _)) = self
                            .heroes
                            .iter()
                            .find(|(_, hero)| hero.hp > 0 && (hero.x, hero.y) == (x, y))
                        {
                            self.hero_symbol(*id)
                        } else if let Some(enemy) = self
                            .enemies
                            .iter()
                            .find(|enemy| (enemy.x, enemy.y) == (x, y))
                        {
                            if self.floor == 3 && enemy.id == 2 {
                                'D'
                            } else {
                                'm'
                            }
                        } else if let Some((_, _, pickup)) =
                            self.pickups.iter().find(|(px, py, _)| (*px, *py) == (x, y))
                        {
                            match pickup {
                                Pickup::Energy => 'E',
                                Pickup::Power => 'P',
                                Pickup::Haste => 'H',
                                Pickup::Spread => 'S',
                            }
                        } else if self.danger.contains(&(x, y)) {
                            '!'
                        } else {
                            '.'
                        }
                    })
                    .collect()
            })
            .collect()
    }

    pub(crate) fn hero_symbol(&self, id: u32) -> char {
        self.heroes
            .keys()
            .position(|key| *key == id)
            .and_then(|index| char::from_digit(index as u32 + 1, 10))
            .unwrap_or('@')
    }
}

#[cfg(test)]
#[path = "../../../tests/cockpit/app/together_dungeon__tests.rs"]
mod tests;
