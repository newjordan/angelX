//! Lady Tallow, Blaise's cat. Once Blaise's hearth is built to its second
//! rung she comes down the stair with the party: a fluffy grey cat with a
//! very small crown and a very large opinion of herself.
//!
//! She keeps near her knight (the first of the party), hisses at any
//! monster that comes too close (it freezes, startled, for a second), and
//! fetches spoils lying on the floor, trotting them back to her knight. No
//! monster can touch her; she would like that noted. The audience adores
//! her. The Herald does not.

use super::home::Station;
use super::*;

/// Lady Tallow, in a delve: where she is, where she's off to fetch, what
/// she's carrying, her hiss's cooldown, and which way she faces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Cat {
    pub(crate) x: f32,
    pub(crate) y: f32,
    #[serde(default)]
    pub(crate) fetching: Option<(f32, f32)>,
    #[serde(default)]
    pub(crate) carrying: Option<String>,
    #[serde(default)]
    pub(crate) hiss: u32,
    /// The tick of her last hiss, for the drawing.
    #[serde(default)]
    pub(crate) hissed: u64,
    #[serde(default)]
    pub(crate) left: bool,
    #[serde(default)]
    pub(crate) moving: bool,
    /// Spoils fetched this delve.
    #[serde(default)]
    pub(crate) fetched: u32,
    /// Ticks she has pushed at something without getting anywhere.
    #[serde(default)]
    pub(crate) stuck: u32,
}

/// The hearth's rung that brings her along.
pub(crate) const HEARTH_FOR_TALLOW: u8 = 2;
/// How close a monster comes before she hisses, how long it stays frozen,
/// and how long before she hisses again.
pub(crate) const HISS_REACH: f32 = 3.0;
const HISS_FREEZE: u32 = HZ;
const HISS_EVERY: u32 = 6 * HZ;
/// How far she'll go for something shiny, and her pace.
const FETCH_REACH: f32 = 12.0;
const TROT: f32 = 10.0;
const RADIUS: f32 = 0.35;

/// What she'll fetch: anything lying about that's worth carrying home,
/// gold most of all.
fn shiny(card: &Card) -> bool {
    card.kind == cards::Kind::Take && card.effects.iter().any(|e| e.spoil().is_some())
}

impl Run {
    /// Down the stair: Lady Tallow comes too, if the hearth is warm enough.
    pub(super) fn bring_tallow(&mut self) {
        self.cat = (self.home.level(Station::Hearth) >= HEARTH_FOR_TALLOW).then(|| {
            let (x, y) = self.tallow_home();
            Cat {
                x: x + 1.2,
                y: y + 0.8,
                fetching: None,
                carrying: None,
                hiss: HZ,
                hissed: 0,
                left: false,
                moving: false,
                fetched: 0,
                stuck: 0,
            }
        });
        if self.cat.is_some() {
            self.cues.push("tallow".into());
        }
    }

    /// Her knight: the first one standing (where she settles by).
    fn tallow_home(&self) -> (f32, f32) {
        self.players
            .values()
            .find(|h| h.hp > 0 && !h.stone)
            .or_else(|| self.players.values().next())
            .map_or((24.0, 14.0), |h| (h.x, h.y))
    }

    /// A new room: she's already there, beside her knight.
    pub(super) fn tallow_follows(&mut self) {
        let (x, y) = self.tallow_home();
        if let Some(cat) = self.cat.as_mut() {
            (cat.x, cat.y) = (x + 1.2, y + 0.8);
            cat.fetching = None;
        }
    }

    /// Her tick: hiss, fetch, carry, follow.
    pub(super) fn tick_tallow(&mut self) {
        let Some(mut cat) = self.cat.take() else {
            return;
        };
        let owner = self
            .players
            .iter()
            .find(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, h)| (id, h.x, h.y));
        cat.hiss = cat.hiss.saturating_sub(1);
        // A monster too close: HSSSS.
        if cat.hiss == 0 {
            let mut startled = false;
            for enemy in self.enemies.iter_mut().filter(|e| {
                e.hp > 0 && e.boss.is_none() && e.kind != EnemyKind::Dummy && e.age >= TELEGRAPH
            }) {
                if (enemy.x - cat.x).hypot(enemy.y - cat.y) < HISS_REACH + enemy.radius() {
                    enemy.frozen = enemy.frozen.max(HISS_FREEZE);
                    startled = true;
                }
            }
            if startled {
                cat.hiss = HISS_EVERY;
                cat.hissed = self.tick;
                self.cues.push("tallow_hiss".into());
                self.thrill(5);
            }
        }
        let room = &self.dungeon.rooms[self.at];
        let grid = Grid {
            room,
            barred: self.barred(),
        };
        let walk = |cat: &mut Cat, (tx, ty): (f32, f32), pace: f32| -> f32 {
            let d = (tx - cat.x).hypot(ty - cat.y);
            if d > 0.05 {
                let step = (pace * DT).min(d);
                let (ux, uy) = unit(tx - cat.x, ty - cat.y);
                let was = (cat.x, cat.y);
                (cat.x, cat.y) =
                    grid.slide((cat.x, cat.y), (ux * step, uy * step), RADIUS, Mover::Hero);
                cat.left = ux < 0.0;
                // Up against a pillar's corner: a cat goes over, or round,
                // or simply is there, as cats are.
                let moved = (cat.x - was.0).hypot(cat.y - was.1);
                cat.stuck = if moved < step * 0.2 { cat.stuck + 1 } else { 0 };
                if cat.stuck > HZ / 2 {
                    (cat.x, cat.y) = (tx, ty);
                    cat.stuck = 0;
                    return 0.0;
                }
            }
            cat.moving = d > 0.3;
            d
        };
        if let Some(card) = cat.carrying.clone() {
            // Bringing it home.
            if let Some((id, ox, oy)) = owner
                && walk(&mut cat, (ox, oy), TROT) < 1.2
            {
                if let Some(found) = self.book.get(&card).cloned()
                    && let Some(hero) = self.players.get_mut(&id)
                {
                    let mut nova = 0;
                    hero.spend(&found, &mut self.score, &mut nova);
                }
                cat.carrying = None;
                cat.fetched += 1;
                self.sounds.push("card_pickup");
                self.cues.push("tallow_fetch".into());
                self.thrill(5);
                if cat.fetched >= 10 {
                    self.notice("cat_person");
                }
            }
        } else {
            // Anything shiny lying about?
            let shiny = self.dungeon.rooms[self.at]
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| self.book.get(&i.card).is_some_and(shiny))
                .map(|(n, i)| (n, (i.x - cat.x).hypot(i.y - cat.y), i.x, i.y))
                .filter(|&(_, d, ..)| d < FETCH_REACH)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match shiny {
                Some((n, _, ix, iy)) => {
                    cat.fetching = Some((ix, iy));
                    if walk(&mut cat, (ix, iy), TROT) < 0.8 {
                        let item = self.dungeon.rooms[self.at].items.swap_remove(n);
                        cat.carrying = Some(item.card);
                        cat.fetching = None;
                    }
                }
                None => {
                    cat.fetching = None;
                    if let Some((_, ox, oy)) = owner {
                        // At her knight's heel, unhurried.
                        let spot = (ox + 1.4, oy + 0.9);
                        let far = (spot.0 - cat.x).hypot(spot.1 - cat.y);
                        if far > 14.0 {
                            // Somehow she was there all along.
                            (cat.x, cat.y) = spot;
                        } else if far > 1.0 {
                            walk(&mut cat, spot, if far > 4.0 { TROT } else { TROT * 0.5 });
                        } else {
                            cat.moving = false;
                        }
                    }
                }
            }
        }
        self.cat = Some(cat);
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_cat__tests.rs"]
mod tests;
