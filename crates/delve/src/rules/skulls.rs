//! A fallen knight's skull: a friend picks it up by walking over it, and
//! the interact key puts it on a weak monster (a tenth of its health or
//! less, no guardian or boss). The monster is gone and the fallen knight
//! rises in its body, playing on with their own keys and its arms and
//! spell (`bodies`), until the next Sanctuary gives them back their own
//! shape. A carrier who falls
//! drops the skull at their feet; a raised knight who falls lies down as
//! bones again, skull and all.

use super::*;

pub const SKULL_REACH: f32 = 1.6;
pub const RAISE_REACH: f32 = 2.4;
pub const SKULL_SETTLE: u64 = HZ as u64;
pub const RAISED_SHARE: u32 = 50;
pub const RAISED_WARD: u32 = 2 * HZ;

impl Run {
    pub fn skull_lying(&self, id: u32) -> Option<(f32, f32)> {
        let hero = self.players.get(&id)?;
        if hero.hp > 0 || hero.absent || self.skull_carrier(id).is_some() {
            return None;
        }
        Some(hero.skull_at.unwrap_or((hero.x, hero.y)))
    }

    pub fn skull_carrier(&self, id: u32) -> Option<u32> {
        self.players
            .iter()
            .find(|(_, h)| h.hp > 0 && h.carrying_skull == Some(id))
            .map(|(&carrier, _)| carrier)
    }

    pub fn raisable(enemy: &Enemy) -> bool {
        enemy.hp > 0
            && enemy.hp * 10 <= enemy.max_hp
            && enemy.age >= TELEGRAPH
            && !enemy.is_boss()
            && !matches!(
                enemy.kind,
                EnemyKind::Boss
                    | EnemyKind::Dummy
                    | EnemyKind::Goblin
                    | EnemyKind::Risen
                    | EnemyKind::Oathbound
                    | EnemyKind::Ward
                    | EnemyKind::Whirlgrave
            )
    }

    pub fn skull_carried(&self) -> bool {
        self.players.values().any(|h| h.hp > 0 && h.carrying_skull.is_some())
    }

    pub(super) fn tick_skulls(&mut self) {
        if self.at_home_now() {
            for hero in self.players.values_mut() {
                hero.possessed = None;
                hero.carrying_skull = None;
                hero.skull_at = None;
            }
            return;
        }
        for hero in self.players.values_mut().filter(|h| h.hp == 0) {
            hero.possessed = None;
        }
        let fallen: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp == 0 && !h.absent)
            .map(|(&id, _)| id)
            .collect();
        let mut dropped: Vec<(u32, (f32, f32))> = Vec::new();
        for hero in self.players.values_mut() {
            let Some(skull) = hero.carrying_skull else {
                continue;
            };
            if hero.hp == 0 {
                dropped.push((skull, (hero.x, hero.y)));
                hero.carrying_skull = None;
            } else if !fallen.contains(&skull) {
                hero.carrying_skull = None;
            }
        }
        for (skull, at) in dropped {
            if let Some(owner) = self.players.get_mut(&skull) {
                owner.skull_at = Some(at);
                self.sounds.push("bones_clatter");
            }
        }
        if self.phase == Phase::Wiped {
            return;
        }
        let tick = self.tick;
        let lying: Vec<(u32, (f32, f32))> = fallen
            .iter()
            .filter(|&&id| {
                self.players
                    .get(&id)
                    .is_some_and(|h| h.fell != 0 && tick >= h.fell + SKULL_SETTLE)
            })
            .filter_map(|&id| self.skull_lying(id).map(|at| (id, at)))
            .collect();
        let mut taken: Vec<u32> = Vec::new();
        for hero in self
            .players
            .values_mut()
            .filter(|h| h.hp > 0 && !h.stone && !h.absent && h.carrying_skull.is_none())
        {
            let near = lying
                .iter()
                .filter(|(id, _)| !taken.contains(id))
                .map(|&(id, (x, y))| (id, (hero.x - x).hypot(hero.y - y)))
                .filter(|&(_, d)| d < SKULL_REACH)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((id, _)) = near {
                hero.carrying_skull = Some(id);
                taken.push(id);
            }
        }
        for id in &taken {
            if let Some(owner) = self.players.get_mut(id) {
                owner.skull_at = None;
            }
        }
        if !taken.is_empty() {
            self.cues.push("skull_pickup".into());
            self.sounds.push("bones_clatter");
        }
        let carriers: Vec<(u32, u32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.taking)
            .filter_map(|(&id, h)| h.carrying_skull.map(|skull| (id, skull)))
            .collect();
        for (carrier, skull) in carriers {
            let (cx, cy) = {
                let h = &self.players[&carrier];
                (h.x, h.y)
            };
            let Some(index) = self
                .enemies
                .iter()
                .enumerate()
                .filter(|(_, e)| Self::raisable(e))
                .map(|(i, e)| (i, (e.x - cx).hypot(e.y - cy) - e.radius()))
                .filter(|&(_, d)| d < RAISE_REACH)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
            else {
                continue;
            };
            if !self.players.get(&skull).is_some_and(|h| h.hp == 0) {
                continue;
            }
            let barred = self.barred();
            let body = self.enemies.remove(index);
            let mut key_lost = false;
            let was_open = self.boss_gates.as_ref().is_some_and(boss_gates::Gates::unlocked);
            if let Some(g) = &mut self.boss_gates {
                if let Some(slot) = body.camp_slot {
                    if g.economy.alive_slot(self.at, slot, body.kind) {
                        g.economy.casualty(self.at, slot);
                    }
                    key_lost = g.carrier_fell(self.at, slot);
                } else if g.economy.emission(body.id).is_some() {
                    g.economy.retire(body.id);
                }
            }
            if key_lost {
                self.carrier_lost(body.x, body.y, body.kind, was_open);
            }
            self.floor_tally.line(carrier).raised += 1;
            if let Some(hero) = self.players.get_mut(&carrier) {
                hero.carrying_skull = None;
                hero.taking = false;
            }
            let at = self.at;
            if let Some(hero) = self.players.get_mut(&skull) {
                hero.hp = (hero.max_hp * RAISED_SHARE / 100).max(1);
                let room = &self.dungeon.rooms[at];
                let grid = Grid { room, barred };
                let was = (hero.x, hero.y);
                (hero.x, hero.y) = grid.across_bar(was, (body.x, body.y));
                hero.possessed = Some(body.kind);
                (hero.fire_cooldown, hero.body_rearm) = (0, 0);
                hero.skull_at = None;
                hero.invulnerable = hero.invulnerable.max(RAISED_WARD);
                hero.struck = 0;
                hero.dress();
            }
            self.cues.push("possess".into());
            self.sounds.push("hand_rise");
            if self.blasts.len() < 16 {
                self.blasts.push((body.x, body.y, BLAST_TICKS));
            }
        }
    }

    pub(super) fn unpossess_all(&mut self) {
        for hero in self.players.values_mut() {
            if hero.possessed.take().is_some() && hero.hp > 0 {
                hero.hp = hero.hp.max((hero.max_hp * REVIVE_SHARE / 100).max(1));
                self.cues.push("unpossess".into());
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_skulls__tests.rs"]
mod tests;
