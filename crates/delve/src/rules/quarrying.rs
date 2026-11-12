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

/// How near a knight stands to a face to cut it, in units.
pub const CUT_REACH: f32 = 3.4;
/// How near a friend stands to take a block from them, in units.
pub const PASS_REACH: f32 = 3.0;
/// How near the stockyard a knight sets a block down, in units.
pub const YARD_REACH: f32 = 3.2;
/// A knight's block as a short line for the HUD.
const WORD: &str = "BLOCK";

/// Where the stockyard's stacks stand at the Delve's gate, in units, so a
/// party coming up the stair walks the last yards to it. On the old map: two
/// tiles south of the stand before the gate's mouth. On the blank-slate map
/// the gate stands inside the Keep's bailey with the teachers in rows south of
/// its mouth and the curtain wall shutting its east side: the yard is the
/// pocket of open ground by the gate rock's west face, three tiles west of the
/// stand and one up, open from the first stone to the last (`world::works`
/// tests).
pub fn yard_spot() -> (f32, f32) {
    let (sx, sy) = crate::world::wishes::delve_stand();
    let (tx, ty) = if crate::world::plan::tabula() { (sx - 3, sy - 1) } else { (sx, sy + 2) };
    (tx as f32 * TILE_UNITS + TILE_UNITS / 2.0, ty as f32 * TILE_UNITS + TILE_UNITS / 2.0)
}

/// A closed, cleared room on the Mines' first floor with a limestone seam the party has won, and
/// the knight standing at its face: the rules' own fixture, for a browser seat to swing at
/// (`delve-dev-host --scene works-face`; Q swings, three chips part a block).
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
    // (A knight beside a seam founds the realm's industry; founded first, the won seam is not lost to it.)
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
    /// Knight `id` takes up a rough block of `stone` that came up with them
    /// from the last delve (`host_commands::Command::CarryBlock`): a hand
    /// that is free takes it on their shoulders; one that is not has no more.
    pub fn carry_block(&mut self, id: u32, stone: &str) -> Result<(), String> {
        if !geology::is_rough(stone) {
            return Err(format!("{stone} is not rough stone"));
        }
        let hero = self.players.get_mut(&id).ok_or("knight no longer exists")?;
        hero.block.get_or_insert_with(|| stone.to_string());
        Ok(())
    }

    /// The rough block knight `id` carries, as its stone's catalogue id.
    pub fn block_of(&self, id: u32) -> Option<&str> {
        self.players.get(&id)?.block.as_deref()
    }

    /// The stone's name for the words on screen.
    fn stone_name(stone: &str) -> String {
        super::geology::rock_name(stone).to_uppercase()
    }

    /// A knight's sword swing that begins this tick on a face the party has
    /// won: a chip, and every third one parts a block, which goes on the
    /// knight's shoulders if their hands are free.
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
                    // The sword swing that began this tick (`arms`).
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

    /// Blocks in hand: a fresh press of E beside a carrier takes the block
    /// from them, so a party can chain one along; a knight on the surface at
    /// the stockyard sets theirs down; a fallen knight loses theirs.
    pub(super) fn tick_carry(&mut self, inputs: &BTreeMap<u32, Input>) {
        // A fresh press of E (or take), as the spoils' own pickup sees it:
        // the knights whose key was already down last tick are not fresh.
        let down: std::collections::BTreeSet<u32> = inputs
            .iter()
            .filter(|(_, i)| i.valid() && (i.interact || i.take))
            .map(|(&id, _)| id)
            .collect();
        let fresh: Vec<u32> = down.difference(&self.kingdom.carry_held).copied().collect();
        self.kingdom.carry_held = down;
        // The dead carry nothing, and a wiped party's stone stays below.
        for h in self.players.values_mut() {
            if h.block.is_some() && (h.hp == 0 || self.phase == Phase::Wiped) {
                h.block = None;
            }
        }
        if self.players.values().all(|h| h.block.is_none()) {
            return;
        }
        // Pass it on: one swap a tick, the nearest taker.
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
        // Set it down at the stockyard.
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

    /// Knight `id` sets their block down in the stockyard, if it has room.
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

    /// Going down from home: the porters at the stair head take any block a
    /// knight still carries, so none goes down a second time.
    pub(super) fn stock_blocks_at_the_stair(&mut self) {
        let ids: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.block.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.stock_block(id);
            // Nowhere to put it: it is set down at the stair head, lost.
            if let Some(h) = self.players.get_mut(&id) {
                h.block = None;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/quarrying__tests.rs"]
mod tests;
