//! The knights a player may take into the Delve: who they are, what they
//! carry in, and the colours they wear. Chosen on the Delve's intro.
//!
//! Each entry is a kit — weapon, guard, hand, ultimate, colours — and the
//! knight who wears it comes from a model house's castle
//! (`crate::stage::houses`): the house serving the party's seat names him,
//! in its own words. With no house serving (the stub route), the Keep's own
//! household takes the field under the kits' old names.

use super::{Card, Hero, Pack, Run};
use crate::stage::houses::{self, HouseId};

/// One knight of the company.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Knight {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) title: &'static str,
    pub(crate) about: &'static str,
    /// Cards they start with: weapon, guard, and anything in hand.
    pub(crate) arm: &'static str,
    pub(crate) guard: &'static str,
    pub(crate) hand: &'static [&'static str],
    pub(crate) bombs: u32,
    pub(crate) max_hp: u32,
    /// Ink swaps on the realm's knight: plume and tabard, shield charge.
    pub(crate) colours: &'static [(char, char)],
}

pub(crate) const COMPANY: &[Knight] = &[
    Knight {
        id: "percival",
        name: "Sir Percival",
        title: "the Seeker",
        about: "Asks every question. A good bow, a quick roll, and a potion for the road.",
        arm: "bow",
        guard: "dodge-roll",
        hand: &["potion"],
        bombs: 2,
        max_hp: 100,
        colours: &[],
    },
    Knight {
        id: "lynette",
        name: "Dame Lynette",
        title: "the Shieldmaiden",
        about: "Says what she thinks and holds the line. Crossbow and kite shield.",
        arm: "crossbow",
        guard: "kite-shield",
        hand: &[],
        bombs: 2,
        max_hp: 100,
        colours: &[('7', '1'), ('8', '0'), ('5', '3'), ('4', '2')],
    },
    Knight {
        id: "gareth",
        name: "Sir Gareth",
        title: "the Kitchen Knight",
        about: "Started in the kitchens; loud and proud of it. Handgonne and extra bombs.",
        arm: "handgonne",
        guard: "dodge-roll",
        hand: &[],
        bombs: 4,
        max_hp: 100,
        colours: &[('7', 'A'), ('8', 'l'), ('5', '6'), ('4', '@')],
    },
    Knight {
        id: "galahad",
        name: "Sir Galahad",
        title: "the Pure",
        about: "The one who found it. Close work with a bright blade, and a stout heart.",
        arm: "knights-blade",
        guard: "dodge-roll",
        hand: &[],
        bombs: 1,
        max_hp: 130,
        colours: &[('7', 'c'), ('8', '$'), ('5', '9'), ('4', '6')],
    },
    // The posse: each lane of the harness, knighted.
    Knight {
        id: "composer",
        name: "the Composer",
        title: "First on the Scene",
        about: "Parses what you type and submits it clean. Big blade, kite shield, a gold chain.",
        arm: "knights-blade",
        guard: "kite-shield",
        hand: &[],
        bombs: 1,
        max_hp: 140,
        colours: &[('H', 'z'), ('i', 'Q'), ('h', 'Q'), ('J', 'q'), ('G', 'S'), ('g', 'x'), ('5', '4')],
    },
    Knight {
        id: "dispatcher",
        name: "the Dispatcher",
        title: "Tool In, Receipt Out",
        about: "Every tool on the belt and a capsule for each. Crossbow, quick roll, extra bombs.",
        arm: "crossbow",
        guard: "dodge-roll",
        hand: &["potion"],
        bombs: 3,
        max_hp: 100,
        colours: &[('H', 't'), ('i', 'o'), ('h', 'o'), ('J', 'R'), ('G', 'r'), ('g', 'B'), ('7', '3')],
    },
    Knight {
        id: "loop",
        name: "the Loop Knight",
        title: "of the Ouroboros",
        about: "Sets the goal, keeps the loop, never skips a beat. Bow and a steady roll.",
        arm: "bow",
        guard: "dodge-roll",
        hand: &[],
        bombs: 2,
        max_hp: 110,
        colours: &[('H', 'T'), ('i', '6'), ('h', '5'), ('J', '4'), ('G', '@'), ('g', 'a')],
    },
    Knight {
        id: "money",
        name: "the Money Knight",
        title: "Cha-Ching",
        about: "Fewer tokens, faster runs, coins everywhere. Handgonne and a sack of bombs.",
        arm: "handgonne",
        guard: "kite-shield",
        hand: &[],
        bombs: 4,
        max_hp: 100,
        colours: &[('H', '6'), ('i', '5'), ('h', '4'), ('J', 'C'), ('G', 'e'), ('g', 'F'), ('7', 'y')],
    },
    Knight {
        id: "competition",
        name: "the Competition Knight",
        title: "Leaderboard in Sight",
        about: "Kernels shaved, hashes verified, submitted fast. Chrome plate, crossbow.",
        arm: "crossbow",
        guard: "dodge-roll",
        hand: &[],
        bombs: 2,
        max_hp: 100,
        colours: &[('7', 'C'), ('8', 'E'), ('5', 'y'), ('4', 'C')],
    },
    Knight {
        id: "scryglass",
        name: "the Scryglass Knight",
        title: "Pictures in the Shell",
        about: "Casts the pictures into your terminal. Bow, kite shield, a potion for the road.",
        arm: "bow",
        guard: "kite-shield",
        hand: &["potion"],
        bombs: 2,
        max_hp: 100,
        colours: &[('H', '3'), ('i', '2'), ('h', '2'), ('J', '0'), ('G', '0'), ('g', 's'), ('7', 'w')],
    },
];

pub(crate) fn knight(id: &str) -> Option<&'static Knight> {
    COMPANY.iter().find(|k| k.id == id)
}

impl Knight {
    /// Which of a house's knights wears this kit.
    pub(crate) fn slot(&self) -> usize {
        COMPANY.iter().position(|k| k.id == self.id).unwrap_or(0)
    }

    /// The kit as a calling, for a house knight who wears it.
    pub(crate) fn class(&self) -> &'static str {
        if self.name.starts_with("the ") {
            self.name
        } else {
            self.title
        }
    }

    /// The house knight who wears this kit, named by the house's model.
    pub(crate) fn of_house(&self, house: Option<HouseId>) -> Option<&'static houses::Knight> {
        houses::get(house?).knight(self.slot())
    }

    /// The wearer's name: the house's knight, or the Keep's own.
    pub(crate) fn name_for(&self, house: Option<HouseId>) -> String {
        self.of_house(house)
            .map_or_else(|| self.name.to_string(), |k| k.name.clone())
    }

    /// The wearer's name with his castle: "Ardent of Lanternmere".
    pub(crate) fn full_name(&self, house: Option<HouseId>) -> String {
        match (self.of_house(house), house) {
            (Some(k), Some(id)) => {
                let castle = &houses::get(id).castle;
                let castle = castle
                    .strip_prefix("The ")
                    .map_or_else(|| castle.clone(), |rest| format!("the {rest}"));
                format!("{} of {castle}", k.name)
            }
            _ => self.name.to_string(),
        }
    }

    /// Name and calling for the intro: "Ardent of Lanternmere, the Seeker".
    pub(crate) fn styled(&self, house: Option<HouseId>) -> String {
        if self.of_house(house).is_some() {
            format!("{}, {}", self.full_name(house), self.class())
        } else {
            format!("{}, {}", self.name, self.title)
        }
    }
}

/// The house a seat's knight would ride for, if dressed now.
pub(crate) fn house_for_seat(seat: u32) -> Option<HouseId> {
    houses::for_seat(&houses::serving(), seat)
}

impl Run {
    /// Dress knight `id` as `who`: their weapon, guard, hand, bombs and colours.
    pub(crate) fn outfit(&mut self, id: u32, who: &Knight) {
        let card = |id: &str| -> Option<Card> { self.book.get(id).cloned() };
        let (arm, guard) = (card(who.arm), card(who.guard));
        let Some(hero) = self.players.get_mut(&id) else {
            return;
        };
        if let Some(arm) = arm {
            hero.equip(&arm);
        }
        if let Some(guard) = guard {
            hero.guard = guard.guard.unwrap_or_default();
            hero.guard_card = Some(guard.id.clone());
        }
        hero.hand = who.hand.iter().map(|c| c.to_string()).collect();
        hero.bombs = who.bombs;
        // The hearth's warmth comes on top of the knight's own health.
        hero.max_hp = who.max_hp + hero.home_hp;
        hero.hp = hero.max_hp;
        hero.knight = Some(who.id.to_string());
        // The seat's house names the knight: the serving house for the host,
        // the formation's other houses and then the March for friends.
        hero.house = house_for_seat(id).map(|h| houses::get(h).key.to_string());
    }

    /// Begin the delve in `pack` instead of the one the dice chose.
    pub(crate) fn begin_in(&mut self, pack: Pack) {
        if self.dungeon.depth == 0 {
            // In the Undercroft, the stair will go down into it.
            self.dungeon.pack = pack;
            return;
        }
        if self.dungeon.pack == pack || self.dungeon.depth != 1 {
            return;
        }
        self.settlement_site = None;
        self.settlement_exhibits.clear();
        self.dungeon = super::layout::floor(1, pack, &mut self.rng);
        self.populate_boss_gates();
        self.cues.retain(|c| c != "run_start");
        self.enter(0, None);
        self.cues.insert(0, "run_start".into());
    }
}

impl Run {
    /// The music this moment wants: a guardian's, the Sanctuary's calm, or
    /// the delve's own.
    pub(crate) fn music(&self) -> Option<&'static str> {
        if !self.active() {
            return None;
        }
        Some(if self.room().kind == super::RoomKind::Sanctuary || self.at_home_now() {
            "sanctuary"
        } else if self
            .enemies
            .iter()
            .any(|e| e.boss.is_some() || e.kind == super::EnemyKind::Dragon)
        {
            "boss"
        } else {
            match self.dungeon.pack.kin() {
                Pack::Crypt => "crypt",
                Pack::Cavern => "mines",
                _ => "keep",
            }
        })
    }
}

/// Which part of a kit a Sanctuary reforge changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// The weapon: an arm card.
    Offense,
    /// The guard key: a guard card.
    Defense,
}

impl Part {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Part::Offense => "offence",
            Part::Defense => "defence",
        }
    }

    pub(crate) fn from_word(word: &str) -> Option<Part> {
        match word.trim().to_ascii_lowercase().as_str() {
            "offense" | "offence" | "weapon" | "attack" => Some(Part::Offense),
            "defense" | "defence" | "guard" | "defend" => Some(Part::Defense),
            _ => None,
        }
    }
}

impl Run {
    /// Knight `id` stands in a Sanctuary and has not reforged on this floor.
    pub(crate) fn can_reforge(&self, id: u32) -> bool {
        self.room().kind == super::RoomKind::Sanctuary
            && self
                .players
                .get(&id)
                .is_some_and(|h| h.hp > 0 && h.reforged_on != Some(self.dungeon.depth))
    }

    /// Equip a reforged card on knight `id`: a weapon for offence, a guard
    /// for defence. The card joins the run's book.
    pub(crate) fn reforge(&mut self, id: u32, part: Part, card: Card) -> Result<String, String> {
        let wanted = match part {
            Part::Offense => super::cards::Kind::Arm,
            Part::Defense => super::cards::Kind::Guard,
        };
        if card.kind != wanted {
            return Err(format!(
                "a {} reforge needs `kind {}`, not `kind {}`",
                part.word(),
                wanted.word(),
                card.kind.word()
            ));
        }
        if !self.can_reforge(id) {
            return Err("only once per Sanctuary, while standing in it".into());
        }
        let name = card.name.clone();
        self.book.insert(card.clone());
        let depth = self.dungeon.depth;
        let hero = self.players.get_mut(&id).ok_or("no such knight")?;
        match part {
            Part::Offense => hero.equip(&card),
            Part::Defense => {
                hero.guard = card.guard.unwrap_or_default();
                hero.guard_card = Some(card.id.clone());
            }
        }
        hero.reforged_on = Some(depth);
        self.cues.push("reforged".into());
        self.sounds.push("card_pickup");
        Ok(name)
    }
}

impl Hero {
    /// The house this knight rides for.
    pub(crate) fn house_id(&self) -> Option<HouseId> {
        self.house.as_deref().and_then(houses::by_key)
    }

    /// The knight's own name, from their house (or the Keep's household).
    pub(crate) fn knight_name(&self) -> Option<String> {
        let kit = self.knight.as_deref().and_then(knight)?;
        Some(kit.name_for(self.house_id()))
    }

    /// The colours this knight wears over the realm's red.
    pub(crate) fn colours(&self) -> &'static [(char, char)] {
        self.knight
            .as_deref()
            .and_then(knight)
            .map_or(&[], |k| k.colours)
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_knights__tests.rs"]
mod tests;
