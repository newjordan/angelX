//! Achievements, the way the Delve's Herald hands them out: a moment the
//! game notices, a name, a line in the Herald's voice, and a box.
//!
//! The run only notices (`feats`, by id). The realm keeps which ones the
//! party has earned — each once — and the boxes they came with; the
//! cockpit tells the run to announce a new one, and the party opens the
//! boxes before Fortune's audience, at the Herald's coffer in her hall.

use super::*;
use crate::drive::together_realm::{Spoil, Spoils};

/// A loot box's grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Tier {
    Bronze,
    Silver,
    Gold,
    Legendary,
}

impl Tier {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Tier::Bronze => "Bronze Box",
            Tier::Silver => "Silver Box",
            Tier::Gold => "Gold Box",
            Tier::Legendary => "Legendary Box",
        }
    }

    /// What a box of this grade holds, the `n`th the realm has opened.
    pub(crate) fn contents(self, n: u32) -> Spoils {
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
pub(crate) struct Feat {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) says: &'static str,
    pub(crate) tier: Tier,
}

pub(crate) const FEATS: &[Feat] = &[
    Feat {
        id: "first_delve",
        name: "Welcome to the Delve",
        says: "You walked down a staircase. The bar was on the floor, and you cleared it.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "flawless",
        name: "Untouchable",
        says: "A whole room, and not a scratch. The monsters have filed a complaint.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "bonded",
        name: "Better Together",
        says: "Two knights, one room, zero monsters. Friendship is a weapon. It is now a currency too.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "my_ult",
        name: "That's My Ultimate",
        says: "You pressed R at exactly the right moment. Or any moment. It worked.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "spin",
        name: "Spin to Win",
        says: "Dame Fortune's wheel has been spun. The audience is thrilled. The audience is always thrilled.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "home_improvement",
        name: "Home Improvement",
        says: "You spent your hard-won spoils on furniture. Very responsible. Very boring. Here, have a box.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "it_was_a_chest",
        name: "It Was a Chest",
        says: "Correction: it was a mouth. In the Herald's defence, it was also a chest.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "sticky_fingers",
        name: "Sticky Fingers",
        says: "A goblin ran off with a sack of gold while you watched. Consolation prize enclosed. It is smaller.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "not_yet",
        name: "Not Yet",
        says: "You died and declined to stay dead. The chapel's bond holds. The Herald is impressed and slightly unnerved.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "fresh_meat",
        name: "Fresh Meat",
        says: "The Flesher hooked you and reeled you in. You are still here. The Flesher is as surprised as you are.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "first_lesson",
        name: "Ector's Pupil",
        says: "You learned something from Sir Ector. He'd like it noted that he taught Arthur, too, and Arthur was worse.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "daredevil",
        name: "Daredevil",
        says: "You took one of Dame Fortune's dares and kept it. She has started calling you 'darling'. Run.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "cat_person",
        name: "Cat Person",
        says: "Lady Tallow fetched ten piles of spoils in one delve. She would like a raise. She would like a throne.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "siege_perilous",
        name: "The Siege Perilous",
        says: "You sat in the Siege Perilous and lived. Galahad is going to be insufferable about this.",
        tier: Tier::Gold,
    },
    Feat {
        id: "secret_room",
        name: "Through the Wall",
        says: "You blew a hole in a wall and found a room nobody told the Herald about. He has checked the plans. There are no plans.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "ribbit",
        name: "Kiss Me, I'm a Knight",
        says: "Hexed into a frog three times in one delve. The Herald has stopped trying to tell you apart from the real ones.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "event_horizon",
        name: "Event Horizon",
        says: "You hit the Hollow One so hard its Black Hole fell shut. The Herald would like to know what was on the other side. Don't tell him.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "rune_runner",
        name: "Rune Runner",
        says: "Four runes in one delve. You run at glowing things on the floor now. The Herald has seen how that ends, and it ends with a mimic.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "rescuer",
        name: "Out of the Cage",
        says: "You opened a cage and somebody walked out. Now they live in your basement. That's how it works down here.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "must_see_tv",
        name: "Must-See TV",
        says: "A million viewers watched you crawl. The Herald has been asked for autographs. The Herald does not sign.",
        tier: Tier::Bronze,
    },
    Feat {
        id: "roadkill",
        name: "Roadkill",
        says: "A rock fell on a monster. You were standing nearby, so the Herald is giving you the credit.",
        tier: Tier::Silver,
    },
    Feat {
        id: "splitting_headache",
        name: "Splitting Headache",
        says: "You killed a slime, both its halves, and all four of its quarters. An entire family tree, pruned.",
        tier: Tier::Silver,
    },
    Feat {
        id: "demolitions",
        name: "Demolitions Expert",
        says: "One keg, three monsters. The sapper who planted it is having a very bad day. Was having.",
        tier: Tier::Silver,
    },
    Feat {
        id: "not_today_goblin",
        name: "Not Today, Goblin",
        says: "You caught a loot goblin. He wants you to know he was going to share.",
        tier: Tier::Silver,
    },
    Feat {
        id: "bigger_they_are",
        name: "The Bigger They Are",
        says: "A floor's guardian is down. It had a name and a backstory. Now it has a drop table.",
        tier: Tier::Silver,
    },
    Feat {
        id: "overkill",
        name: "Overkill",
        says: "Three hundred damage in a single hit. The monster had a fraction of that. The Herald respects excess.",
        tier: Tier::Silver,
    },
    Feat {
        id: "fragile",
        name: "Fragile, Handle With Care",
        says: "A whole floor of Glass Jaw, and you walked off it. Everything hit twice as hard. You too.",
        tier: Tier::Silver,
    },
    Feat {
        id: "out_of_time",
        name: "Out of Time",
        says: "The floor fell in and you took the stairs anyway. Fashionably late to your own escape.",
        tier: Tier::Silver,
    },
    Feat {
        id: "party_of_four",
        name: "Party of Four",
        says: "Four knights in one delve. The Herald has had to buy more chairs.",
        tier: Tier::Silver,
    },
    Feat {
        id: "prime_time",
        name: "Prime Time",
        says: "Ten million viewers. Dame Fortune has fainted. The Herald is handling it, and would like a raise.",
        tier: Tier::Gold,
    },
    Feat {
        id: "master_of_arms",
        name: "Master of Arms",
        says: "All four of Sir Ector's lessons, learned. He has nothing left to teach you, and finds that suspicious.",
        tier: Tier::Gold,
    },
    Feat {
        id: "fortunes_favourite",
        name: "Fortune's Favourite",
        says: "Five of Fortune's dares kept in one delve. Her purse is empty. Her heart is full. Her lawyers are calling.",
        tier: Tier::Gold,
    },
    Feat {
        id: "pit_tyrant",
        name: "Into the Pit",
        says: "You went into the Pit on purpose and killed what lives there. The Talisman is yours. So are the Herald's nightmares.",
        tier: Tier::Gold,
    },
    Feat {
        id: "full_house",
        name: "A Full House",
        says: "Mabel, Anselm, Pip and Maud, all home. The Undercroft is crowded. The Herald has started a rota.",
        tier: Tier::Gold,
    },
    Feat {
        id: "dragonslayer",
        name: "Dragonslayer",
        says: "You killed the dragon. The old stories end here. Yours, apparently, does not.",
        tier: Tier::Gold,
    },
    Feat {
        id: "below_the_bottom",
        name: "Below the Bottom",
        says: "There was a floor under Dragon Keep. Nobody told the Herald. You found it anyway.",
        tier: Tier::Gold,
    },
    Feat {
        id: "full_hall",
        name: "A Full Hall",
        says: "Every plinth in the Trophy Hall has something on it. Sir Kay has run out of things to complain about. He is furious.",
        tier: Tier::Gold,
    },
    Feat {
        id: "the_grail",
        name: "The Search Is the Grail",
        says: "You went to the bottom of everything and met what was there. It was you, mostly. Well done.",
        tier: Tier::Legendary,
    },
];

pub(crate) fn feat(id: &str) -> Option<&'static Feat> {
    FEATS.iter().find(|f| f.id == id)
}

/// How long a new achievement's banner stays up.
pub(crate) const BANNER_TICKS: u64 = 5 * HZ as u64;

impl Run {
    /// The game noticed something worth an achievement (each is kept once
    /// by the realm; noticing it again does nothing there).
    pub(super) fn notice(&mut self, id: &'static str) {
        if !self.feats.contains(&id) && feat(id).is_some() {
            self.feats.push(id);
        }
    }

    /// The cockpit's word that an achievement is new: show its banner.
    pub(crate) fn announce(&mut self, id: &str) {
        if let Some(feat) = feat(id) {
            self.banner = Some((self.tick, feat.id.to_string()));
            self.cues.push("achievement".into());
            self.sounds.push("wheel_land");
        }
    }

    /// The banner up now, if any: the achievement it shows.
    pub(crate) fn banner_now(&self) -> Option<&'static Feat> {
        let (at, id) = self.banner.as_ref()?;
        (self.tick.saturating_sub(*at) < BANNER_TICKS)
            .then(|| feat(id))
            .flatten()
    }

    /// The cockpit opened a box at the coffer: show what came out.
    pub(crate) fn unbox(&mut self, tier: Tier, spoils: &Spoils) {
        self.unboxed = Some((self.tick, tier, spoils.label()));
        self.cues.push(format!("box_opened:{}", tier_word(tier)));
        self.sounds.push("chest_open");
        self.shake = self.shake.max(6);
    }
}

pub(crate) fn tier_word(tier: Tier) -> &'static str {
    match tier {
        Tier::Bronze => "bronze",
        Tier::Silver => "silver",
        Tier::Gold => "gold",
        Tier::Legendary => "legendary",
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_feats__tests.rs"]
mod tests;
