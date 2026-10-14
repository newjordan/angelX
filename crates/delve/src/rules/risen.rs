//! Humphrey the Risen: the countryside's very persistent former neighbor.
//!
//! Once the festival is properly underway he claws up through the compost,
//! shambling after the party with a hedge grudge and a two-handed shove.
//! Felling him pays a bounty and buys quiet — never peace: he reassembles,
//! each time a little larger. He never spawns in the Delve; below the gate
//! is somebody else's problem.

use super::*;

/// Festival rounds played before the compost starts to stir.
pub const AWAKE_PLAYS: u32 = 3;
/// How much round-free lingering in one room before he claws in.
pub const FOLLOW: u32 = 8 * HZ;
/// Compost time after he is felled.
pub const RISE_AGAIN: u32 = 45 * HZ;
/// Each fall grows him; the ninth leaves him at rest.
const GRUDGE_HP: u32 = 40;

/// Candidate doors of entry, in tiles: the four quarters of the room.
const CORNERS: [(f32, f32); 4] = [(3.5, 3.5), (20.5, 3.5), (3.5, 10.5), (20.5, 10.5)];

impl Run {
    /// Every living knight is giving no steering and no fire: the party is
    /// standing about, not traveling.
    fn all_still(&self, inputs: &BTreeMap<u32, Input>) -> bool {
        self.players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
            .all(|(id, _)| {
                inputs
                    .get(id)
                    .map(|i| {
                        !i.valid()
                            || (i.move_x == 0
                                && i.move_y == 0
                                && i.aim_x == 0
                                && i.aim_y == 0
                                && !i.fire
                                && !i.interact)
                    })
                    .unwrap_or(true)
            })
    }

    /// Retired: no longer called (the surface keeps no zombies). Kept for
    /// the bestiary's art and reels.
    #[allow(dead_code)]
    pub(super) fn tick_risen(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self
            .enemies
            .iter()
            .any(|e| e.kind == EnemyKind::Risen && e.hp > 0)
        {
            return;
        }
        // He counts only the still: a live round freezes the compost clock,
        // and so does a party on the move. Stand about browsing the boards
        // and he finds you; walk with purpose and he never catches up.
        if self.pastimes.round.is_some() || !self.all_still(inputs) {
            return;
        }
        let felled = self.home.countryside.humphrey_felled;
        let dig = &mut self.home.countryside.humphrey_dig;
        if *dig > 0 {
            *dig -= 1;
            return;
        }
        let games = &self.home.countryside.games;
        // He wakes for a festival worth interrupting: three rounds and a
        // victory. A party that has not won anything is beneath his grudge.
        if games.plays_total() < AWAKE_PLAYS || games.wins_total() == 0 {
            return;
        }
        let heroes: Vec<(f32, f32)> = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone && h.privy == 0)
            .map(|h| (h.x, h.y))
            .collect();
        if heroes.is_empty() {
            return;
        }
        let (cx, cy) = (
            heroes.iter().map(|&(x, _)| x).sum::<f32>() / heroes.len() as f32,
            heroes.iter().map(|&(_, y)| y).sum::<f32>() / heroes.len() as f32,
        );
        // The quarter farthest from the party, and walkable.
        let Some((tx, ty)) = CORNERS
            .iter()
            .copied()
            .filter(|&(x, y)| {
                !matches!(
                    self.room().tile(x as i32, y as i32),
                    Tile::Wall | Tile::Block
                )
            })
            .max_by(|a, b| {
                (a.0 * 2.0 - cx)
                    .hypot(a.1 * 2.0 - cy)
                    .total_cmp(&(b.0 * 2.0 - cx).hypot(b.1 * 2.0 - cy))
            })
        else {
            return;
        };
        let hp = EnemyKind::Risen.hp() + GRUDGE_HP * u32::from(felled);
        let Some(id) = self.spawn_at(EnemyKind::Risen, tx * TILE_UNITS, ty * TILE_UNITS) else {
            return;
        };
        if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == id) {
            enemy.base_hp = Some(hp);
            enemy.max_hp = hp;
            enemy.hp = hp;
            let (dx, dy) = (cx - enemy.x, cy - enemy.y);
            let length = dx.hypot(dy).max(1e-6);
            enemy.dir = (dx / length, dy / length);
        }
        self.home.countryside.humphrey_dig = FOLLOW;
        self.sounds.push("boss_rise");
        self.fx.push(Fx::Shake(6));
        self.found = Some((
            self.tick,
            0,
            if felled == 0 {
                "HUMPHREY THE RISEN claws up through the cabbages. He has questions about his hedge."
                    .into()
            } else {
                format!(
                    "HUMPHREY THE RISEN is back ({} felled). He has been practicing.",
                    felled
                )
            },
        ));
    }
}
