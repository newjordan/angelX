//! The Great Bellows: a cold forge between two great leather bellows. A
//! pump (E at a bellows' handle) heats it, but only in turn: west, east,
//! west. Two knights take a side each; a knight alone runs between them.
//! Half hot, the heat wakes two imps; full hot, the forge roars and every
//! knight in the room has their arm tempered a step.
use super::*;

/// The forge and its two bellows (tiles), and where each handle is worked.
pub const FORGE: (i32, i32, i32, i32) = (9, 1, 6, 3);
pub const BLOCKS: [(i32, i32, i32, i32); 3] = [FORGE, (4, 2, 3, 2), (17, 2, 3, 2)];
pub const HANDLES: [(f32, f32); 2] = [(5.5, 5.0), (18.5, 5.0)];
/// Heat a pump adds, the full forge, the heat that wakes the imps, and
/// how often the forge cools a step while nobody pumps.
pub const PUMP: u32 = 8;
pub const HEAT_FULL: u32 = 100;
pub const HEAT_IMPS: u32 = 50;
pub const COOL_EVERY: u32 = 4;

impl Run {
    pub(super) fn tick_bellows(&mut self, nook: &mut Nook, pressed: u8) {
        if nook.t.is_multiple_of(COOL_EVERY) {
            nook.heat = nook.heat.saturating_sub(1);
        }
        let before = nook.heat;
        for id in self.nook_players() {
            if !has(pressed, id) {
                continue;
            }
            let Some(side) = (0..2).find(|&s| self.nook_near(id, HANDLES[s], REACH)) else {
                continue;
            };
            if nook.last == side as u8 + 1 {
                self.found = Some((self.tick, id, "OTHER SIDE!".into()));
                continue;
            }
            nook.last = side as u8 + 1;
            nook.heat = (nook.heat + PUMP).min(HEAT_FULL);
            *nook.score.entry(id).or_default() += 1;
            self.sounds.push("vent_fire");
        }
        if before < HEAT_IMPS && nook.heat >= HEAT_IMPS && nook.raised == 0 {
            self.found = Some((self.tick, 0, "THE HEAT WAKES IMPS!".into()));
            self.nook_raise(nook, EnemyKind::Imp, (8.0, 7.0));
            self.nook_raise(nook, EnemyKind::Imp, (16.0, 7.0));
        }
        if nook.heat >= HEAT_FULL {
            nook.done = true;
            let mut tempered = 0;
            for id in self.nook_players() {
                let Some(hero) = self.players.get_mut(&id) else { continue };
                if hero.weapon != Weapon::Fists && hero.forged.is_none() && hero.temper < feel::TEMPER_MAX {
                    hero.temper += 1;
                    tempered += 1;
                    self.cues.push(format!("temper:{}", hero.temper));
                }
            }
            self.shake = self.shake.max(4);
            self.sounds.push("hell_gate_rise");
            self.found = Some((
                self.tick,
                0,
                if tempered > 0 { "THE FORGE ROARS! ARMS TEMPERED." } else { "THE FORGE ROARS!" }.into(),
            ));
        }
    }
}
