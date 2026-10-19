//! Achievements, the way the Delve's Herald hands them out: a moment the
//! game notices, a name, a line in the Herald's voice, and a box.
//!
//! The run only notices (`feats`, by id). The realm keeps which ones the
//! party has earned — each once — and the boxes they came with; the
//! cockpit tells the run to announce a new one, and the party opens the
//! boxes at the Herald's coffer in Fortune's hall.

use super::*;
use crate::realm::{Spoil, Spoils};

/// A loot box's grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Bronze,
    Silver,
    Gold,
    Legendary,
}

impl Tier {
    pub fn name(self) -> &'static str {
        match self {
            Tier::Bronze => "Bronze Box",
            Tier::Silver => "Silver Box",
            Tier::Gold => "Gold Box",
            Tier::Legendary => "Legendary Box",
        }
    }

    /// What a box of this grade holds, the `n`th the realm has opened.
    pub fn contents(self, n: u32) -> Spoils {
        let roll =
            |salt: u32, span: u32| mix(u64::from(n) * 7919 + u64::from(salt)) as u32 % span.max(1);
        let common = [Spoil::Bone, Spoil::Wax, Spoil::Ore][roll(1, 3) as usize];
        let mut spoils = Spoils::default();
        match self {
            Tier::Bronze => {
                spoils.add(Spoil::Gold, 40 + roll(2, 50));
                spoils.add(common, 1 + roll(3, 2));
            }
            Tier::Silver => {
                spoils.add(Spoil::Gold, 100 + roll(2, 80));
                spoils.add(common, 2 + roll(3, 2));
                spoils.add(Spoil::Gem, 1);
            }
            Tier::Gold => {
                spoils.add(Spoil::Gold, 220 + roll(2, 130));
                spoils.add(common, 3);
                spoils.add(Spoil::Gem, 2);
                spoils.add(Spoil::Ember, 1);
                spoils.add(Spoil::Bond, 1);
            }
            Tier::Legendary => {
                spoils.add(Spoil::Gold, 500);
                spoils.add(Spoil::Gem, 5);
                spoils.add(Spoil::Ember, 3);
                spoils.add(Spoil::Scale, 1);
                spoils.add(Spoil::Bond, 2);
            }
        }
        spoils
    }
}

/// One achievement: its id, its name, what the Herald says, its box.
pub struct Feat {
    pub id: &'static str,
    pub name: &'static str,
    pub says: &'static str,
    pub tier: Tier,
}

pub const FEATS: &[Feat] = &[
    Feat {
        id: "first_delve",
        name: "First Descent",
        says: "Went down the first staircase.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "flawless",
        name: "Untouchable",
        says: "Cleared a room without taking a hit.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "bonded",
        name: "Better Together",
        says: "Two knights cleared a room together.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "my_ult",
        name: "Hour of Glory",
        says: "Unleashed an ultimate ability.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "spin",
        name: "Wheel of Fortune",
        says: "Spun Dame Fortune's wheel.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "home_improvement",
        name: "Home Improvement",
        says: "Paid for an upgrade to the home.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "it_was_a_chest",
        name: "Not a Chest",
        says: "Opened a chest that was a mimic.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "sticky_fingers",
        name: "Sticky Fingers",
        says: "Let a loot goblin escape with its haul.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "not_yet",
        name: "Not Yet",
        says: "Revived after falling in battle.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "fresh_meat",
        name: "Fresh Meat",
        says: "Was caught on the Flesher's hook.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "first_lesson",
        name: "Ector's Pupil",
        says: "Learned a lesson from Sir Ector.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "daredevil",
        name: "Daredevil",
        says: "Completed one of Fortune's dares.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "cat_person",
        name: "Cat Person",
        says: "Lady Tallow fetched ten items in one delve.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "siege_perilous",
        name: "The Siege Perilous",
        says: "Sat the Siege Perilous and proved worthy.",
        tier: Tier::Gold,
    },
    Feat {
        id: "secret_room",
        name: "Through the Wall",
        says: "Blasted open a hidden room.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "ribbit",
        name: "Frog Knight",
        says: "Was hexed into a frog three times.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "event_horizon",
        name: "Event Horizon",
        says: "Broke the Hollow One's Black Hole.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "rune_runner",
        name: "Rune Runner",
        says: "Took four runes in one delve.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "rescuer",
        name: "Cage Breaker",
        says: "Freed a captive from a cage.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "roadkill",
        name: "Roadkill",
        says: "Killed a monster with a falling rock.",
        tier: Tier::Silver,
    },
    Feat {
        id: "mind_the_gap",
        name: "Mind the Gap",
        says: "A gauntlet threw a monster off its feet and into the dark. The Herald is still listening for it to land.",
        tier: Tier::Silver,
    },
    Feat {
        id: "splitting_headache",
        name: "Splitting Headache",
        says: "Killed seven slimes in one delve.",
        tier: Tier::Silver,
    },
    Feat {
        id: "demolitions",
        name: "Demolitions Expert",
        says: "Felled three foes with one blast.",
        tier: Tier::Silver,
    },
    Feat {
        id: "not_today_goblin",
        name: "Not Today, Goblin",
        says: "Caught a loot goblin before it escaped.",
        tier: Tier::Silver,
    },
    Feat {
        id: "bigger_they_are",
        name: "Giant Killer",
        says: "Defeated a floor's guardian.",
        tier: Tier::Silver,
    },
    Feat {
        id: "overkill",
        name: "Overkill",
        says: "Landed a single hit worth 300 damage.",
        tier: Tier::Silver,
    },
    Feat {
        id: "fragile",
        name: "Glass Knight",
        says: "Took the stairs down under Glass Jaw.",
        tier: Tier::Silver,
    },
    Feat {
        id: "out_of_time",
        name: "Out of Time",
        says: "Escaped a floor as it collapsed.",
        tier: Tier::Silver,
    },
    Feat {
        id: "party_of_four",
        name: "Party of Four",
        says: "Four knights joined one delve.",
        tier: Tier::Silver,
    },
    Feat {
        id: "master_of_arms",
        name: "Master of Arms",
        says: "Learned all four of Sir Ector's lessons.",
        tier: Tier::Gold,
    },
    Feat {
        id: "fortunes_favourite",
        name: "Fortune's Favourite",
        says: "Kept five of Fortune's dares in one delve.",
        tier: Tier::Gold,
    },
    Feat {
        id: "pit_tyrant",
        name: "Into the Pit",
        says: "Slew the Pit Tyrant and took the Talisman.",
        tier: Tier::Gold,
    },
    Feat {
        id: "white_horse",
        name: "The White Horse",
        says: "Freed the white horse from the swamp.",
        tier: Tier::Silver,
    },
    Feat {
        id: "full_house",
        name: "Full House",
        says: "Brought every rescued resident home.",
        tier: Tier::Gold,
    },
    Feat {
        id: "dragonslayer",
        name: "Dragonslayer",
        says: "Slew the dragon of Dragon Keep.",
        tier: Tier::Gold,
    },
    Feat {
        id: "below_the_bottom",
        name: "Below the Bottom",
        says: "Reached the floor below Dragon Keep.",
        tier: Tier::Gold,
    },
    Feat {
        id: "full_hall",
        name: "Full Hall",
        says: "Filled every plinth in the Trophy Hall.",
        tier: Tier::Gold,
    },
    Feat {
        id: "fealty",
        name: "Baron Sworn",
        says: "Won the dwarf king's fealty.",
        tier: Tier::Silver,
    },
    Feat {
        id: "hammers_ring",
        name: "The Hammers Ring",
        says: "Relit a forge of Caer Dwfn.",
        tier: Tier::Gold,
    },
    Feat {
        id: "unhorsed",
        name: "Unhorsed",
        says: "Knocked a champion from his horse.",
        tier: Tier::Silver,
    },
    Feat {
        id: "laurel_of_the_lists",
        name: "Victor's Laurel",
        says: "Won a bout against a friend at the lists.",
        tier: Tier::Silver,
    },
    Feat {
        id: "champion",
        name: "Lists Champion",
        says: "Defeated Sir Lancelot at the lists.",
        tier: Tier::Gold,
    },
    Feat {
        id: "popinjay",
        name: "Popinjay",
        says: "Hit every popinjay target without a miss.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "unwashed",
        name: "The Great Unwashed",
        says: "Slew the dragon without using a privy.",
        tier: Tier::Gold,
    },
    Feat {
        id: "the_grail",
        name: "The Grail",
        says: "Reached the Threshold and found the Grail.",
        tier: Tier::Legendary,
    },
];

pub fn feat(id: &str) -> Option<&'static Feat> {
    FEATS.iter().find(|f| f.id == id)
}

/// How long a new achievement's banner stays up.
pub const BANNER_TICKS: u64 = 5 * HZ as u64;

impl Run {
    /// The game noticed something worth an achievement (each is kept once
    /// by the realm; noticing it again does nothing there).
    pub(super) fn notice(&mut self, id: &'static str) {
        if !self.feats.contains(&id) && feat(id).is_some() {
            self.feats.push(id);
        }
    }

    /// The cockpit's word that an achievement is new. Down in a delve it
    /// is kept for the match's end (`deeds`), never thrown up over the
    /// fighting (the operator, 2026-10-18: "they are unecessary until the
    /// end of the match"); up in the realm its banner shows.
    pub fn announce(&mut self, id: &str) {
        let Some(feat) = feat(id) else { return };
        if !self.at_home_now() {
            if !self.deeds.iter().any(|d| d == feat.id) {
                self.deeds.push(feat.id.to_string());
            }
            return;
        }
        self.banner = Some((self.tick, feat.id.to_string()));
        self.cues.push("achievement".into());
        self.sounds.push("wheel_land");
    }

    /// The banner up now, if any: the achievement it shows.
    pub fn banner_now(&self) -> Option<&'static Feat> {
        let (at, id) = self.banner.as_ref()?;
        (self.tick.saturating_sub(*at) < BANNER_TICKS)
            .then(|| feat(id))
            .flatten()
    }

    /// The cockpit opened a box at the coffer: show what came out.
    pub fn unbox(&mut self, tier: Tier, spoils: &Spoils) {
        self.unboxed = Some((self.tick, tier, spoils.label()));
        self.cues.push(format!("box_opened:{}", tier_word(tier)));
        self.sounds.push("chest_open");
        self.fx.push(Fx::Shake(6));
    }
}

pub fn tier_word(tier: Tier) -> &'static str {
    match tier {
        Tier::Bronze => "bronze",
        Tier::Silver => "silver",
        Tier::Gold => "gold",
        Tier::Legendary => "legendary",
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_feats__tests.rs"]
mod tests;
