//! Sir Ector's lessons. Arthur's foster father, who taught him arms, keeps
//! the Training Yard now, and teaches any knight who has earned it: each
//! level a knight reaches is a lesson, a choice between two talents. What a
//! knight learns, the
//! realm keeps, and it goes down the stair with them as a held card.
//!
//! Knights learn by fighting: everything the party fells is experience for
//! each knight in it. The run marks it; the realm keeps it.

use super::home::{BUY_HOLD, Home, Order, Station};
use super::*;

/// One talent: its name, Sir Ector's word on it, and what it does (a held
/// card's effect).
pub(crate) struct Talent {
    pub(crate) name: &'static str,
    pub(crate) says: &'static str,
    pub(crate) effect: &'static str,
    /// What it does, for the lectern.
    pub(crate) does: &'static str,
}

/// Four lessons, a talent on the left lectern and one on the right.
pub(crate) const LESSONS: [[Talent; 2]; 4] = [
    [
        Talent {
            name: "Iron Hide",
            says: "Take the blow on the mail, not the face.",
            effect: "armor 1",
            does: "one more piece of mail",
        },
        Talent {
            name: "Keen Edge",
            says: "A sharp blade is a short fight.",
            effect: "damage 12",
            does: "12% more damage",
        },
    ],
    [
        Talent {
            name: "Quickdraw",
            says: "Faster. No, faster than that.",
            effect: "rate 15",
            does: "strike 15% faster",
        },
        Talent {
            name: "Fleet Foot",
            says: "The knight who isn't there doesn't get hit.",
            effect: "speed 12",
            does: "walk 12% faster",
        },
    ],
    [
        Talent {
            name: "Morningstar",
            says: "Swing it round your head. Mind the horse.",
            effect: "orbit 1",
            does: "a morningstar circles you",
        },
        Talent {
            name: "Arc Lightning",
            says: "Hit one. Then, somehow, the one next to it.",
            effect: "chain 1",
            does: "each hit sparks to another",
        },
    ],
    [
        Talent {
            name: "Bloodthirst",
            says: "Every fallen thing gives a little back. Don't ask what.",
            effect: "vamp 3",
            does: "3 health back for each kill",
        },
        Talent {
            name: "Lady's Veil",
            says: "Your blows mend your friends. Arthur never learned this one.",
            effect: "mend 2",
            does: "each hit mends a friend by 2",
        },
    ],
];

/// The experience that reaches levels 2, 3, 4 and 5.
pub(crate) const LEVELS: [u32; 4] = [600, 1800, 3600, 6000];

/// A knight's prowess, kept by the realm: experience, and the talent
/// learned at each lesson so far (0 the left, 1 the right).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Prowess {
    #[serde(default)]
    pub(crate) xp: u32,
    #[serde(default)]
    pub(crate) learned: Vec<u8>,
}

impl Prowess {
    pub(crate) fn level(&self) -> u32 {
        1 + LEVELS.iter().filter(|&&n| self.xp >= n).count() as u32
    }

    /// Lessons earned and not yet taken.
    pub(crate) fn waiting(&self) -> usize {
        (self.level() as usize - 1).saturating_sub(self.learned.len())
    }

    /// The lesson after those learned, if there is one.
    pub(crate) fn next(&self) -> Option<&'static [Talent; 2]> {
        LESSONS.get(self.learned.len())
    }

    /// The talents learned, in order.
    pub(crate) fn talents(&self) -> impl Iterator<Item = (usize, &'static Talent)> + '_ {
        self.learned
            .iter()
            .enumerate()
            .filter_map(|(lesson, &side)| {
                Some((lesson, LESSONS.get(lesson)?.get(usize::from(side))?))
            })
    }
}

/// Sir Ector's two lecterns in the Yard's south-west corner (column, row,
/// width, height in tiles): the left talent, and the right.
pub(crate) const LECTERNS: [(i32, i32, i32, i32); 2] = [(2, 11, 2, 1), (6, 11, 2, 1)];
/// Where Sir Ector stands, between and behind them (tiles).
pub(crate) const ECTOR_AT: (f32, f32) = (5.0, 10.2);

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

/// The lectern a point stands on: 0 the left, 1 the right.
pub(crate) fn lectern_at(x: f32, y: f32) -> Option<usize> {
    LECTERNS.iter().position(|&p| inside(p, x, y))
}

/// The name the realm keeps a knight's prowess under.
pub(crate) fn knight_key(hero: &Hero) -> String {
    hero.knight
        .clone()
        .unwrap_or_else(|| hero.name.to_lowercase())
}

impl Home {
    pub(crate) fn prowess(&self, who: &str) -> Prowess {
        self.knights.get(who).cloned().unwrap_or_default()
    }

    /// `who` learns the left (0) or right (1) talent of their next lesson,
    /// if one is waiting.
    pub(crate) fn learn(&mut self, who: &str, side: u8) -> Result<&'static Talent, String> {
        let prowess = self.knights.entry(who.to_string()).or_default();
        let Some(lesson) = prowess.next() else {
            return Err("Sir Ector: I've taught you all I know. Go and teach someone else.".into());
        };
        if prowess.waiting() == 0 {
            let at = LEVELS[prowess.learned.len()];
            return Err(format!(
                "Sir Ector: not yet. The next lesson is at level {} ({at} experience; you have {}).",
                prowess.learned.len() + 2,
                prowess.xp
            ));
        }
        prowess.learned.push(side.min(1));
        Ok(&lesson[usize::from(side.min(1))])
    }
}

impl Run {
    /// Experience for what fell this tick, for every knight in the party.
    pub(super) fn mark_experience(&mut self, kinds: &[EnemyKind]) {
        let xp: u32 = kinds.iter().map(|k| (k.bounty() / 25).max(1)).sum();
        if xp > 0 {
            *self.marks.entry("xp".into()).or_default() += xp;
        }
    }

    /// The lecterns: holding F on one asks Sir Ector for its talent; he
    /// greets whoever walks up.
    pub(super) fn tick_lessons(&mut self, inputs: &BTreeMap<u32, Input>) {
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let Some(side) = lectern_at(hero.x, hero.y).filter(|_| hero.hp > 0 && !hero.stone)
            else {
                continue;
            };
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            if !fire {
                hero.buy_spent = false;
                hero.buying = 0;
                continue;
            }
            if hero.buy_spent {
                continue;
            }
            hero.buying = hero.buying.saturating_add(1);
            if hero.buying >= BUY_HOLD {
                hero.buying = 0;
                hero.buy_spent = true;
                asked.push(Order {
                    knight: id,
                    station: [Station::LessonA, Station::LessonB][side],
                });
            }
        }
        self.orders.extend(asked);
        const ECTOR_BIT: u8 = 1 << 4;
        let (ex, ey) = (ECTOR_AT.0 * TILE_UNITS, ECTOR_AT.1 * TILE_UNITS);
        let near = self
            .players
            .values()
            .any(|h| h.hp > 0 && (h.x - ex).hypot(h.y - ey) < 5.0);
        if near && self.greeted & ECTOR_BIT == 0 {
            let waiting = self
                .players
                .values()
                .any(|h| self.home.prowess(&knight_key(h)).waiting() > 0);
            self.cues.push(
                if waiting {
                    "npc:ector_lesson"
                } else {
                    "npc:ector"
                }
                .into(),
            );
        }
        self.greeted = if near {
            self.greeted | ECTOR_BIT
        } else {
            self.greeted & !ECTOR_BIT
        };
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_talents__tests.rs"]
mod tests;
