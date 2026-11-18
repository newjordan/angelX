//! Cutting stone below and carrying it up, before any machinery. A knight
//! swings at a claimed rock face and parts a rough block from it; a block is
//! heavy, so the knight who carries it walks slower and can pass it to a
//! friend beside them, and a party can chain one up the stair by hand. The
//! stair ends at the Delve's gate, where the stockyard stands; a knight who
//! reaches it sets the block down there (`stockyard`). The first stones in
//! the castle come up on someone's back.
use super::stir::{self, CHIPS, FACE_BLOCKS};
use super::*;
use crate::kingdom::catalog;

pub const CUT_REACH: f32 = 3.4;
pub const PASS_REACH: f32 = 3.0;
pub const YARD_REACH: f32 = 3.2;
const WORD: &str = "BLOCK";

pub fn yard_spot() -> (f32, f32) {
    let (sx, sy) = crate::world::wishes::delve_stand();
    let (tx, ty) = if crate::world::plan::tabula() { (sx - 3, sy - 1) } else { (sx, sy + 2) };
    (tx as f32 * TILE_UNITS + TILE_UNITS / 2.0, ty as f32 * TILE_UNITS + TILE_UNITS / 2.0)
}

#[cfg(any(test, feature = "testing"))]
pub fn won_face_run() -> Run {
    use super::layout::{self, Pack};
    let mut run = Run::new(511, 7, None);
    run.dungeon = layout::floor(1, Pack::Cavern, &mut Rng::new(10));
    run.boss_gates = None;
    let room = (1..run.dungeon.rooms.len())
        .find(|&i| i % 3 == 1 && run.dungeon.rooms[i].kind == RoomKind::Fight)
        .expect("a fight room");
    run.enter(room, None);
    run.enemies.clear();
    run.waves = Default::default();
    run.dungeon.rooms[room].cleared = true;
    run.phase = Phase::Exploring;
    for y in 1..run.room().rows - 1 {
        for x in 1..run.room().cols - 1 {
            if matches!(run.room().tile(x as i32, y as i32), Tile::Block | Tile::Hazard | Tile::Bog) {
                run.dungeon.rooms[room].set(x, y, Tile::Floor);
            }
        }
    }
    let deposit = run.industry_deposits().into_iter().next().expect("a deposit");
    let hero = run.players.get_mut(&1).expect("a knight");
    (hero.x, hero.y) = (deposit.x, deposit.y);
    Run::found_in(&mut run.home.industry, run.seed);
    run.home.industry.discover(
        catalog(),
        deposit.id.clone(),
        crate::kingdom::Claim { resource: deposit.resource.into(), name: deposit.name.clone(), source: "the dev host".into(), secured: false, rate: deposit.seam.rate(), yields: deposit.seam.yields(), surface: false },
    );
    run.home.industry.secure(&deposit.id);
    run.kingdom.ore_sentries.remove(&deposit.id);
    run
}

impl Run {
    pub fn carry_block(&mut self, id: u32, stone: &str) -> Result<(), String> {
        if !geology::is_rough(stone) {
            return Err(format!("{stone} is not rough stone"));
        }
        let hero = self.players.get_mut(&id).ok_or("knight no longer exists")?;
        hero.block.get_or_insert_with(|| stone.to_string());
        Ok(())
    }

    pub fn block_of(&self, id: u32) -> Option<&str> {
        self.players.get(&id)?.block.as_deref()
    }

    fn stone_name(stone: &str) -> String {
        super::geology::rock_name(stone).to_uppercase()
    }

    pub(super) fn tick_quarry_face(&mut self, d: &industry::Deposit) {
        if !self.active() || self.dungeon.depth == 0 {
            return;
        }
        let secured = self.home.industry.claims.get(&d.id).is_some_and(|c| c.secured);
        if !secured {
            return;
        }
        let tick = self.tick;
        let held_by_raid = self.kingdom.stir.raiding(&d.id);
        let stone = d.resource;
        let ids: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| {
                h.hp > 0
                    && !h.absent
                    && !h.stone
                    && h.sword
                    && h.swing == 8
                    && (h.x - d.x).hypot(h.y - d.y) < CUT_REACH
            })
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            if held_by_raid {
                self.found = Some((tick, id, "THE RAIDERS HOLD THE FACE".into()));
                continue;
            }
            let worked_out = self.kingdom.stir.cut.get(&d.id).copied().unwrap_or(0) >= FACE_BLOCKS;
            if worked_out {
                self.found = Some((tick, id, "WORKED OUT. DIG DEEPER".into()));
                continue;
            }
            if self.players[&id].block.is_some() {
                self.found = Some((tick, id, "HANDS FULL. PASS IT ON [E]".into()));
                continue;
            }
            let chips = self.kingdom.stir.chips.entry(d.id.clone()).or_default();
            *chips += 1;
            self.kingdom.stir.struck = Some((d.id.clone(), tick));
            self.sounds.push("rock_land");
            self.fx.push(Fx::Shake(1));
            if *chips < CHIPS {
                continue;
            }
            *chips = 0;
            *self.kingdom.stir.cut.entry(d.id.clone()).or_default() += 1;
            if let Some(h) = self.players.get_mut(&id) {
                h.block = Some(stone.to_string());
            }
            self.stir_up(stir::CUT);
            self.sounds.push("door_open");
            self.found = Some((tick, id, format!("{} {WORD}. CARRY IT UP", Self::stone_name(stone))));
        }
    }

    pub(super) fn tick_carry(&mut self, inputs: &BTreeMap<u32, Input>) {
        let down: std::collections::BTreeSet<u32> = inputs
            .iter()
            .filter(|(_, i)| i.valid() && (i.interact || i.take))
            .map(|(&id, _)| id)
            .collect();
        let fresh: Vec<u32> = down.difference(&self.kingdom.carry_held).copied().collect();
        self.kingdom.carry_held = down;
        for h in self.players.values_mut() {
            if h.block.is_some() && (h.hp == 0 || self.phase == Phase::Wiped) {
                h.block = None;
            }
        }
        if self.players.values().all(|h| h.block.is_none()) {
            return;
        }
        let takers: Vec<u32> = self
            .players
            .iter()
            .filter(|(id, h)| fresh.contains(id) && h.hp > 0 && !h.absent && !h.stone && h.block.is_none())
            .map(|(&id, _)| id)
            .collect();
        for taker in takers {
            let (tx, ty) = {
                let t = &self.players[&taker];
                (t.x, t.y)
            };
            let from = self
                .players
                .iter()
                .filter(|(id, h)| **id != taker && h.block.is_some() && h.hp > 0 && !h.absent)
                .map(|(&id, h)| (id, (h.x - tx).hypot(h.y - ty)))
                .filter(|&(_, d)| d < PASS_REACH)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id);
            if let Some(from) = from {
                let block = self.players.get_mut(&from).and_then(|h| h.block.take());
                if let Some(h) = self.players.get_mut(&taker) {
                    h.block = block;
                }
                self.sounds.push("clang");
                self.found = Some((self.tick, taker, "TAKES THE BLOCK".into()));
            }
        }
        if self.dungeon.depth == 0 && self.room().kind == RoomKind::Kingdom {
            let (yx, yy) = yard_spot();
            let at: Vec<u32> = self
                .players
                .iter()
                .filter(|(_, h)| {
                    h.block.is_some() && h.hp > 0 && !h.absent && (h.x - yx).hypot(h.y - yy) < YARD_REACH
                })
                .map(|(&id, _)| id)
                .collect();
            for id in at {
                self.stock_block(id);
            }
        }
    }

    fn stock_block(&mut self, id: u32) {
        let Some(stone) = self.players.get(&id).and_then(|h| h.block.clone()) else {
            return;
        };
        if self.stock_rough(&stone, 1) == 0 {
            let up = self.home.stockyard.tier.next().map(|t| t.name().to_lowercase());
            self.found = Some((
                self.tick,
                id,
                match up {
                    Some(_) => "STOCKYARD FULL. RAISE THE SHAFT".into(),
                    None => "STOCKYARD FULL".into(),
                },
            ));
            return;
        }
        if let Some(h) = self.players.get_mut(&id) {
            h.block = None;
        }
        self.stir_up(stir::LIFT);
        self.sounds.push("rock_land");
        let held = self.yard_view().count(&stone);
        let name = self.players.get(&id).map_or_else(String::new, |h| h.name.clone());
        self.notices.push(format!(
            "{name} sets a block of {} in the stockyard. {held} waiting.",
            Self::stone_name(&stone).to_lowercase()
        ));
    }

    pub(super) fn stock_blocks_at_the_stair(&mut self) {
        let ids: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.block.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.stock_block(id);
            if let Some(h) = self.players.get_mut(&id) {
                h.block = None;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/quarrying__tests.rs"]
mod tests;
