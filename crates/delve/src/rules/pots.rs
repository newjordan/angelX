//! Clay pots along the walls of the rooms below: shoot one, or catch it
//! with a sword's swing, and it shatters; every other pot had a coin in it,
//! which flies to the nearest knight. A pot broken stays broken for the
//! floor, so nobody farms them. The places are the seed's, so every knight
//! and every friend's copy sees the same pots.
use super::*;

/// Pots a room keeps, at most, and how long shards lie about (ticks).
pub const POTS: usize = 4;
pub const SHARDS: u32 = HZ / 2;
/// How near a shot or a swing must come (units).
const SHOT_REACH: f32 = 0.8;
const SWING_REACH: f32 = 2.2;

/// Whether room `kind` keeps pots: the ordinary rooms of a delve.
fn potted(kind: RoomKind) -> bool {
    matches!(
        kind,
        RoomKind::Fight | RoomKind::Hall | RoomKind::Treasure | RoomKind::Start | RoomKind::Stairs
    )
}

impl Run {
    /// The pots of room `index`, where they stand (units), by their number.
    pub fn pots_of(&self, index: usize) -> Vec<(u8, (f32, f32))> {
        let Some(room) = self.dungeon.rooms.get(index) else {
            return Vec::new();
        };
        // The merchant's camp keeps its crockery on the cart.
        if self.dungeon.depth == 0
            || !potted(room.kind)
            || room.great()
            || self.merchant_camp().is_some_and(|c| c.room == index)
        {
            return Vec::new();
        }
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        // Hard by the north and south walls, clear of the doorways.
        let spots: Vec<(i32, i32)> = (2..cols - 2)
            .flat_map(|x| [(x, 1), (x, rows - 2)])
            .filter(|&(x, y)| {
                room.tile(x, y) == Tile::Floor
                    && (x - cols / 2).abs() > 2
                    && room.tile(x, if y == 1 { 0 } else { rows - 1 }) != Tile::Door
            })
            .collect();
        let broken = self.pots_broken.get(&index).copied().unwrap_or(0);
        (0..POTS as u8)
            .filter_map(|i| {
                if spots.is_empty() || broken & (1 << i) != 0 {
                    return None;
                }
                let h = mix(self.seed ^ (index as u64) << 12 ^ u64::from(i) << 3 ^ u64::from(self.dungeon.depth));
                let (x, y) = spots[(h % spots.len() as u64) as usize];
                Some((i, ((x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS)))
            })
            .collect()
    }

    /// Shots and swings break the pots they meet.
    pub(super) fn tick_pots(&mut self) {
        self.pot_shards.retain_mut(|s| {
            s.2 = s.2.saturating_sub(1);
            s.2 > 0
        });
        let pots = self.pots_of(self.at);
        if pots.is_empty() {
            return;
        }
        let mut broken = Vec::new();
        for &(i, (px, py)) in &pots {
            let shot = self.projectiles.iter_mut().find(|p| {
                !p.hostile && p.ttl > 0 && (p.x - px).hypot(p.y - py) < SHOT_REACH
            });
            if let Some(p) = shot {
                p.ttl = 0;
                broken.push((i, px, py));
                continue;
            }
            if self
                .players
                .values()
                .any(|h| h.hp > 0 && h.swing > 0 && (h.x - px).hypot(h.y - py) < SWING_REACH)
            {
                broken.push((i, px, py));
            }
        }
        for (i, x, y) in broken {
            *self.pots_broken.entry(self.at).or_default() |= 1 << i;
            self.pot_shards.push((x, y, SHARDS));
            self.sounds.push("bones_clatter");
            // Every other pot kept a coin.
            if mix(self.seed ^ (self.at as u64) << 5 ^ u64::from(i)) % 2 == 0
                && self.feel.coins.len() < feel::COINS
            {
                self.feel.coins.push(feel::Coin { x, y, age: 0 });
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_pots__tests.rs"]
mod tests;
