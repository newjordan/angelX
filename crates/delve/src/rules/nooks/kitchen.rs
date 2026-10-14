//! Grubb's Kitchen: a goblin cook stirs a pot with his back to the room.
//! Scoop while he stirs (E at the pot); when he spins round, freeze. Any
//! knight who moves, swings, shoots or scoops while he looks gets the
//! ladle and he bellows for a cousin. Every scoop has a coin in it; the
//! best scooper is crowned when the pot is empty.
use super::*;

/// The pot (tiles), and where Grubb stands behind it.
pub const POT: (i32, i32, i32, i32) = (11, 3, 2, 2);
pub const STOVE: (f32, f32) = (12.0, 2.5);
/// The pot's middle, for reach.
const POT_MIDDLE: (f32, f32) = (12.0, 4.0);
const POT_REACH: f32 = 4.4;
pub const BLOCKS: [(i32, i32, i32, i32); 5] = [POT, (2, 1, 5, 2), (18, 1, 4, 2), (4, 9, 5, 1), (15, 9, 5, 1)];
/// Scoops in the pot, and the coin in each.
pub const SCOOPS: u8 = 12;
pub const SCOOP_GOLD: u32 = 3;
/// The cook's beats: how long he stirs each time round (ticks), how long
/// he warns before he turns, how long he looks.
pub const BEATS: [u32; 6] = [75, 50, 95, 40, 60, 85];
pub const WARN: u32 = 15;
pub const LOOK: u32 = 40;
/// The ladle's blow, and how far it sends a knight.
pub const LADLE: u32 = 10;
const SHOVE: f32 = 3.0;

/// What the cook is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cook {
    Stirring,
    /// About to turn: his ears twitch.
    Turning,
    Looking,
}

pub fn beat_len(beat: usize) -> u32 {
    BEATS[beat % BEATS.len()] + WARN + LOOK
}

pub fn cook(nook: &Nook) -> Cook {
    let stir = BEATS[usize::from(nook.beat) % BEATS.len()];
    if nook.cook < stir {
        Cook::Stirring
    } else if nook.cook < stir + WARN {
        Cook::Turning
    } else {
        Cook::Looking
    }
}

impl Run {
    pub(super) fn tick_kitchen(&mut self, nook: &mut Nook, inputs: &BTreeMap<u32, Input>, pressed: u8) {
        if nook.pot == 0 {
            return;
        }
        nook.cook += 1;
        if nook.cook >= beat_len(usize::from(nook.beat)) {
            nook.cook = 0;
            nook.beat = (nook.beat + 1) % BEATS.len() as u8;
            nook.caught = 0;
        }
        let state = cook(nook);
        if state == Cook::Looking && nook.cook == BEATS[usize::from(nook.beat)] + WARN {
            self.sounds.push("boss_roar");
        }
        let take = (self.tick % 2) as usize;
        for id in self.nook_players() {
            let input = inputs.get(&id).copied().filter(|i| i.valid()).unwrap_or_default();
            let fidget = input.move_x != 0 || input.move_y != 0 || input.fire || input.dash || has(pressed, id);
            if state == Cook::Looking {
                if fidget && !has(nook.caught, id) {
                    nook.caught |= 1 << (id & 7);
                    if let Some(hero) = self.players.get_mut(&id) {
                        hero.invulnerable = 0;
                        hero.hurt(LADLE);
                        let (dx, dy) = (hero.x - POT_MIDDLE.0 * TILE_UNITS, hero.y - POT_MIDDLE.1 * TILE_UNITS);
                        let len = dx.hypot(dy).max(0.01);
                        let to = (hero.x + dx / len * SHOVE, hero.y + dy / len * SHOVE);
                        if self.dungeon.rooms[nook.room].tile((to.0 / TILE_UNITS) as i32, (to.1 / TILE_UNITS) as i32) == Tile::Floor {
                            (hero.x, hero.y) = to;
                        }
                    }
                    self.sounds.push("clang");
                    self.sounds.push(super::super::chatter::clip(id, "oof", take));
                    self.found = Some((self.tick, id, "CAUGHT! COUSINS!".into()));
                    self.nook_raise(nook, EnemyKind::Goblin, (12.0, 6.5));
                }
                continue;
            }
            if has(pressed, id) && self.nook_near(id, POT_MIDDLE, POT_REACH) && nook.pot > 0 {
                nook.pot -= 1;
                *nook.score.entry(id).or_default() += 1;
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, SCOOP_GOLD);
                }
                self.sounds.push("gold");
                self.found = Some((self.tick, id, "SLURP! A COIN!".into()));
            }
        }
        if nook.pot == 0 {
            nook.done = true;
            let best = self.nook_crown(nook);
            let name = best.and_then(|id| self.players.get(&id)).map_or_else(String::new, |h| h.name.to_uppercase());
            self.sounds.push("chest_open");
            self.found = Some((self.tick, 0, format!("THE POT'S EMPTY! {name} ATE MOST.")));
        }
    }
}
