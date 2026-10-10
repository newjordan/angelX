//! The west wing: Maud's tavern, the Siege Perilous. Behind the Trophy Hall's west wall there is a
//! doorway full of rubble; Tobbin's crew digs it out (the West Wing's rung,
//! ordered at the plate before it) and the realm has a pub.
//!
//! Maud keeps the bar once she's been rescued (until then the tap is dry).
//! The rumour board says what the next delve will meet. And in the corner
//! stands the Siege Perilous itself, the chair at the Round Table that only
//! the worthiest knight may sit in: a knight who has done enough is found
//! worthy; everyone else is found wanting, and zapped off it.

use super::home::{BUY_HOLD, Order, Station};
use super::*;

/// The plate before the rubble in the Trophy Hall's west wall (tiles).
pub(crate) const WING_PLATE: (i32, i32, i32, i32) = (1, 6, 2, 2);
/// The bar along the tavern's north wall, and Maud behind it (her feet).
pub(crate) const BAR: (i32, i32, i32, i32) = (3, 3, 12, 1);
pub(crate) const MAUD_AT: (f32, f32) = (8.5, 2.8);
/// The rumour board on its posts, east of the bar.
pub(crate) const BOARD: (i32, i32, i32, i32) = (17, 2, 3, 1);
/// Two tables in the middle of the room.
pub(crate) const TABLES: [(i32, i32, i32, i32); 2] = [(5, 7, 3, 2), (12, 7, 3, 2)];
/// The Siege Perilous, in the south-west corner: stand on it to sit.
pub(crate) const SIEGE: (i32, i32, i32, i32) = (2, 11, 1, 1);
/// How long a knight sits before the chair decides, and how many of the
/// realm's achievements make a knight worthy of it.
const SIEGE_HOLD: u32 = HZ;
pub(crate) const SIEGE_WORTHY: usize = 24;
/// How near the bar a knight comes before Maud speaks.
const BAR_REACH: f32 = 4.5;

/// Maud's three taps, before the bar.
pub(crate) const TAPS: [(i32, i32, i32, i32); 3] = [(4, 4, 2, 1), (8, 4, 2, 1), (12, 4, 2, 1)];

/// A round from one of Maud's taps.
pub(crate) struct Drink {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) does: &'static str,
    /// Its price in gold, from the treasury.
    pub(crate) price: u32,
}

pub(crate) const DRINKS: [Drink; 3] = [
    Drink {
        id: "ale",
        name: "Maud's Mushroom Ale",
        does: "a second wind for every knight",
        price: 40,
    },
    Drink {
        id: "stout",
        name: "Dragon's Breath Stout",
        does: "+15% damage for every knight",
        price: 80,
    },
    Drink {
        id: "reserve",
        name: "The Herald's Reserve",
        does: "the audience starts warm",
        price: 60,
    },
];

/// Sir Dinadan's stage, east of the tables; Dinadan on it; the plates
/// before it where a song is asked for.
pub(crate) const STAGE: (i32, i32, i32, i32) = (17, 8, 5, 1);
pub(crate) const DINADAN_AT: (f32, f32) = (19.5, 8.3);
pub(crate) const SONG_PLATES: [(i32, i32, i32, i32); 3] =
    [(15, 10, 2, 1), (18, 10, 2, 1), (21, 10, 2, 1)];

/// One of Sir Dinadan's songs: an aura for the next delve.
pub(crate) struct Song {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) does: &'static str,
    pub(crate) price: u32,
}

pub(crate) const SONGS: [Song; 3] = [
    Song {
        id: "haste",
        name: "The Lay of Haste",
        does: "knights who hear it run faster",
        price: 50,
    },
    Song {
        id: "iron",
        name: "The Iron Hymn",
        does: "knights who hear it take less",
        price: 70,
    },
    Song {
        id: "red",
        name: "The Red Ballad",
        does: "knights who hear it mend on kills",
        price: 90,
    },
];

/// Who hears a song: a knight alone, always; in company, a knight this
/// near another. And what each song gives.
pub(crate) const SONG_REACH: f32 = 9.0;
pub(crate) const SONG_HASTE: f32 = 1.15;
pub(crate) const SONG_ARMOR: u32 = 2;
pub(crate) const SONG_VAMP: u32 = 3;

/// The song plate a point stands on, if any.
pub(crate) fn song_at(x: f32, y: f32) -> Option<usize> {
    SONG_PLATES.iter().position(|&p| inside(p, x, y))
}

pub(crate) fn song(id: &str) -> Option<&'static Song> {
    SONGS.iter().find(|s| s.id == id)
}

/// The stout's edge, and the Reserve's warm start (thousands of viewers).
pub(crate) const STOUT: u32 = 15;
const RESERVE: u32 = 300;

/// The tap a point stands at, if any: 0, 1 or 2.
pub(crate) fn tap_at(x: f32, y: f32) -> Option<usize> {
    TAPS.iter().position(|&t| inside(t, x, y))
}

pub(crate) fn drink(id: &str) -> Option<&'static Drink> {
    DRINKS.iter().find(|d| d.id == id)
}

fn inside((col, row, w, h): (i32, i32, i32, i32), x: f32, y: f32) -> bool {
    let (c, r) = (x / TILE_UNITS, y / TILE_UNITS);
    c >= col as f32 && c < (col + w) as f32 && r >= row as f32 && r < (row + h) as f32
}

impl Run {
    /// The Trophy Hall's west wall: holding F at its plate orders the West
    /// Wing's next rung.
    pub(super) fn tick_wing_plate(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.home.next(Station::Wing).is_none() {
            return;
        }
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            if !fire {
                hero.buy_spent = false;
            }
            let on = hero.hp > 0 && !hero.stone && inside(WING_PLATE, hero.x, hero.y);
            if on && fire && !hero.buy_spent {
                hero.buying = hero.buying.saturating_add(1);
                if hero.buying >= BUY_HOLD {
                    hero.buying = 0;
                    hero.buy_spent = true;
                    asked.push(Order {
                        knight: id,
                        station: Station::Wing,
                    });
                }
            } else if on {
                hero.buying = 0;
            }
        }
        self.orders.extend(asked);
    }

    /// The tavern: Maud at the bar (or a dry tap), her taps, and the Siege
    /// Perilous.
    pub(super) fn tick_tavern(&mut self, inputs: &BTreeMap<u32, Input>) {
        let (bx, by) = (
            MAUD_AT.0 * TILE_UNITS,
            (BAR.1 + 1) as f32 * TILE_UNITS + 1.0,
        );
        let near_bar = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - bx).hypot(h.y - by) < BAR_REACH);
        if near_bar && self.greeted & 1 == 0 {
            self.cues.push(
                if self.home.residents.contains("maud") {
                    "bar:maud"
                } else {
                    "tavern_dry"
                }
                .into(),
            );
        }
        let (dx, dy) = (DINADAN_AT.0 * TILE_UNITS, DINADAN_AT.1 * TILE_UNITS + 3.0);
        let near_stage = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - dx).hypot(h.y - dy) < BAR_REACH);
        if near_stage && self.greeted & 2 == 0 {
            self.cues.push("npc:dinadan".into());
        }
        let (gx, gy) = (
            hireling::BEAUMAINS_AT.0 * TILE_UNITS,
            hireling::BEAUMAINS_AT.1 * TILE_UNITS,
        );
        let near_table = self
            .players
            .values()
            .any(|h| h.hp > 0 && !h.stone && (h.x - gx).hypot(h.y - gy) < BAR_REACH);
        if near_table && self.greeted & 4 == 0 {
            self.cues.push("npc:beaumains".into());
        }
        let worthy = self.home.feats.len() >= SIEGE_WORTHY;
        let kept = self.home.residents.contains("maud");
        let mut judged = Vec::new();
        let mut asked = Vec::new();
        for (&id, hero) in self.players.iter_mut() {
            let standing = hero.hp > 0 && !hero.stone;
            let sitting = standing && inside(SIEGE, hero.x, hero.y);
            let tap = tap_at(hero.x, hero.y).filter(|_| standing && kept);
            let fire = inputs
                .get(&id)
                .copied()
                .filter(|i| i.valid())
                .is_some_and(|i| i.fire);
            let hiring = standing && hireling::hire_at(hero.x, hero.y);
            let asking = song_at(hero.x, hero.y)
                .filter(|_| standing)
                .map(|plate| [Station::SongA, Station::SongB, Station::SongC][plate])
                .or(hiring.then_some(Station::Hire));
            if let Some(station) = asking {
                // At Dinadan's stage, or Beaumains' table: hold F.
                if !fire {
                    hero.buy_spent = false;
                    hero.buying = 0;
                } else if !hero.buy_spent {
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        asked.push(Order {
                            knight: id,
                            station,
                        });
                    }
                }
                continue;
            }
            if let Some(tap) = tap {
                // At a tap: hold F for a round.
                if !fire {
                    hero.buy_spent = false;
                    hero.buying = 0;
                } else if !hero.buy_spent {
                    hero.buying = hero.buying.saturating_add(1);
                    if hero.buying >= BUY_HOLD {
                        hero.buying = 0;
                        hero.buy_spent = true;
                        asked.push(Order {
                            knight: id,
                            station: [Station::TapA, Station::TapB, Station::TapC][tap],
                        });
                    }
                }
                continue;
            }
            if !sitting {
                hero.buying = 0;
                hero.buy_spent = false;
                continue;
            }
            if hero.buy_spent {
                continue;
            }
            hero.buying = hero.buying.saturating_add(1);
            if hero.buying >= SIEGE_HOLD {
                hero.buying = 0;
                hero.buy_spent = true;
                judged.push(id);
            }
        }
        self.orders.extend(asked);
        for id in judged {
            let (x, y) = self.players.get(&id).map_or((0.0, 0.0), |h| (h.x, h.y));
            if worthy {
                self.cues.push("siege_worthy".into());
                self.found = Some((self.tick, id, "Found worthy.".into()));
                self.notice("siege_perilous");
                self.thrill(50);
                self.sounds.push("boss_rise");
            } else if let Some(hero) = self.players.get_mut(&id) {
                // Found wanting: a sting, and off the chair.
                hero.hp = hero.hp.saturating_sub(5).max(1);
                hero.x += 2.5;
                self.cues.push("siege_wanting".into());
                self.found = Some((self.tick, id, "Found wanting.".into()));
                self.sounds.push("spike");
                self.shake = self.shake.max(8);
                self.thrill(5);
            }
            if self.blasts.len() < 16 {
                self.blasts.push((x, y, BLAST_TICKS));
            }
        }
        self.greeted =
            u8::from(near_bar) | (u8::from(near_stage) << 1) | (u8::from(near_table) << 2);
    }

    /// Down the stair: Maud's round, if one was poured, is drunk, and does
    /// what it does for the delve. The realm forgets it (the cockpit hears
    /// the mark).
    pub(super) fn drink_round(&mut self) {
        self.round = self.home.round.take();
        let Some(id) = self.round.clone() else {
            return;
        };
        match id.as_str() {
            "ale" => {
                for hero in self.players.values_mut() {
                    hero.winds = hero.winds.saturating_add(1);
                }
            }
            "reserve" => self.thrill(RESERVE),
            _ => {}
        }
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for knight in ids {
            self.deal_home(knight);
        }
        *self.marks.entry("round_drunk".into()).or_default() += 1;
        self.cues.push(format!("round_drunk:{id}"));
    }

    /// Down the stair: Sir Dinadan's song, if one was asked for, goes with
    /// the party. The realm forgets it (the cockpit hears the mark).
    pub(super) fn sing_song(&mut self) {
        self.song = self.home.song.take();
        let Some(id) = self.song.clone() else {
            return;
        };
        *self.marks.entry("song_sung".into()).or_default() += 1;
        self.cues.push(format!("song:{id}"));
    }

    /// Each tick of a delve: who hears the song (a knight alone, always; in
    /// company, a knight near another), and what it gives them.
    pub(super) fn tick_song(&mut self) {
        let iron = self.song.as_deref() == Some("iron");
        let song = self.song.is_some() && self.dungeon.depth > 0;
        let standing: Vec<(u32, f32, f32)> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone)
            .map(|(&id, h)| (id, h.x, h.y))
            .collect();
        let alone = standing.len() <= 1;
        for (&id, hero) in self.players.iter_mut() {
            hero.singing = song
                && hero.hp > 0
                && !hero.stone
                && (alone
                    || standing
                        .iter()
                        .any(|&(o, x, y)| o != id && (x - hero.x).hypot(y - hero.y) < SONG_REACH));
            hero.armor = hero.bonus.armor.min(super::MAX_ARMOR)
                + if iron && hero.singing { SONG_ARMOR } else { 0 };
        }
    }

    /// What the rumour board says: who waits at the bottom of the next
    /// delve's first floor, what Fortune's wheel said, and talk.
    pub(crate) fn rumours(&self) -> [String; 3] {
        let landing = self.home.landing().clamp(1, DEEPEST - 1);
        let pack = Pack::at(landing, self.dungeon.pack);
        let guardian = if let Some(g) = &self.boss_gates {
            let names = g
                .leaders
                .iter()
                .filter(|l| !l.defeated)
                .map(|l| self.bosses[usize::from(l.boss)].name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}: {}", g.economy.profile.name().to_uppercase(), names)
        } else {
            match pack {
                Pack::Cavern | Pack::Archive => format!(
                    "RIVAL CLAIMS IN {}; ANY CHIEF OPENS A ROUTE",
                    pack.name().to_uppercase()
                ),
                Pack::Fungal | Pack::Unknown => format!(
                    "BROOD SUPPORT IN {}; CORE LEADER HOLDS THE ROUTE",
                    pack.name().to_uppercase()
                ),
                Pack::Crypt | Pack::Hellforge => format!(
                    "TRIBUTE IN {}; ALL GATE HOUSES HOLD THE ROUTE",
                    pack.name().to_uppercase()
                ),
            }
        };
        let wheel = if self.mode == fortune::Mode::LongWayDown {
            "THE WHEEL HAS NOT SPOKEN".to_string()
        } else {
            format!("FORTUNE SAYS {}", self.mode.name().to_uppercase())
        };
        const TALK: [&str; 6] = [
            "THEY SAY THE STAIR HAS NO BOTTOM",
            "THEY SAY SNIBBET OWES MAUD MONEY",
            "THEY SAY THE HERALD SLEEPS IN THE COFFER",
            "THEY SAY GALAHAD SAT IN THAT CHAIR",
            "THEY SAY MERLIN WAS A TOAD. HE WAS",
            "THEY SAY THE PIT TYRANT WAS A PUPPY ONCE",
        ];
        let talk = TALK[(mix(self.raid_id ^ 0x7a1e) % TALK.len() as u64) as usize];
        [guardian, wheel, talk.to_string()]
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_tavern__tests.rs"]
mod tests;
