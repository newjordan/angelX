//! Wren's bounties: three jobs pinned to the board by the stair, for
//! whoever will take them. Each asks for something a delve can do (so many of a monster, a
//! moment, a floor, a show) and pays into the realm's treasury the moment
//! it is done, wherever the party is. Wren pins the next in its place.
//!
//! The run only counts (`marks`: kills by kind, the moments the Herald
//! calls); the realm keeps the board and its progress.

use super::home::Home;
use super::*;
use crate::drive::together_realm::{Spoil, Spoils};

/// What a bounty asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Goal {
    /// So many of a monster slain.
    Slay(EnemyKind),
    /// So many of a moment the Herald calls (a cue's head).
    Moment(&'static str),
    /// A floor reached.
    Reach(u32),
    /// A show of so many thousand viewers.
    Show(u32),
}

/// One of Wren's bounties.
pub(crate) struct Bounty {
    pub(crate) id: &'static str,
    pub(crate) title: &'static str,
    /// Wren's note pinned to it.
    pub(crate) says: &'static str,
    pub(crate) goal: Goal,
    pub(crate) need: u32,
    /// The floor the party must have reached for Wren to pin it.
    pub(crate) from: u32,
    pub(crate) pays: &'static [(Spoil, u32)],
}

impl Bounty {
    pub(crate) fn reward(&self) -> Spoils {
        let mut spoils = Spoils::default();
        for &(spoil, n) in self.pays {
            spoils.add(spoil, n);
        }
        spoils
    }
}

/// How many bounties the board holds.
pub(crate) const PINNED: usize = 3;

pub(crate) const BOUNTIES: &[Bounty] = &[
    Bounty {
        id: "bones",
        title: "Bone Collector",
        says: "The Ossuary is short on stock. Twenty-five skeletons, please. Not the bones. Well. The bones.",
        goal: Goal::Slay(EnemyKind::Skeleton),
        need: 25,
        from: 0,
        pays: &[(Spoil::Gold, 120), (Spoil::Bone, 3)],
    },
    Bounty {
        id: "bats",
        title: "Bat Problem",
        says: "Something has been at my maps. Something with wings. Twenty of them.",
        goal: Goal::Slay(EnemyKind::Bat),
        need: 20,
        from: 0,
        pays: &[(Spoil::Gold, 100), (Spoil::Wax, 2)],
    },
    Bounty {
        id: "untouched",
        title: "Not a Scratch",
        says: "Three rooms without a scratch. Tobbin says it can't be done. Prove Tobbin wrong; he loves that.",
        goal: Goal::Moment("flawless"),
        need: 3,
        from: 0,
        pays: &[(Spoil::Gold, 150), (Spoil::Gem, 1)],
    },
    Bounty {
        id: "goblin",
        title: "Catch That Goblin",
        says: "A goblin owes me money. Any goblin. As far as my ledger is concerned they're all the same goblin.",
        goal: Goal::Moment("goblin_caught"),
        need: 1,
        from: 0,
        pays: &[(Spoil::Gold, 200)],
    },
    Bounty {
        id: "slimes",
        title: "Mop Duty",
        says: "Two whole slime families, every last quarter of them. Bring a mop.",
        goal: Goal::Slay(EnemyKind::Slime),
        need: 14,
        from: 0,
        pays: &[(Spoil::Gold, 120), (Spoil::Wax, 3)],
    },
    Bounty {
        id: "streak",
        title: "Unstoppable",
        says: "Four in a breath, twice. The audience asked for it by name.",
        goal: Goal::Moment("slay4"),
        need: 2,
        from: 0,
        pays: &[(Spoil::Gold, 150), (Spoil::Ember, 1)],
    },
    Bounty {
        id: "ratings",
        title: "Ratings",
        says: "Fortune says the audience is getting bored. Give them a show: a million and a half.",
        goal: Goal::Show(1500),
        need: 1500,
        from: 0,
        pays: &[(Spoil::Gold, 180), (Spoil::Gem, 1)],
    },
    Bounty {
        id: "keep",
        title: "Down to the Keep",
        says: "Go and look at Dragon Keep. Just look. Then come back and tell me what's changed.",
        goal: Goal::Reach(3),
        need: 1,
        from: 1,
        pays: &[(Spoil::Gold, 150), (Spoil::Ore, 3)],
    },
    Bounty {
        id: "sappers",
        title: "Defuse the Situation",
        says: "Six sappers, before one of them finds my cellar.",
        goal: Goal::Slay(EnemyKind::Sapper),
        need: 6,
        from: 1,
        pays: &[(Spoil::Gold, 150), (Spoil::Ore, 3)],
    },
    Bounty {
        id: "necromancers",
        title: "Stay Dead",
        says: "Three necromancers. The skeletons will thank you. Briefly.",
        goal: Goal::Slay(EnemyKind::Necromancer),
        need: 3,
        from: 2,
        pays: &[(Spoil::Gold, 200), (Spoil::Bond, 1)],
    },
    Bounty {
        id: "boars",
        title: "Bacon",
        says: "Four warboars. Blaise has promised a feast, and I have promised Blaise.",
        goal: Goal::Slay(EnemyKind::Warboar),
        need: 4,
        from: 2,
        pays: &[(Spoil::Gold, 200), (Spoil::Ore, 2)],
    },
    Bounty {
        id: "guardians",
        title: "Bigger Game",
        says: "Two guardians. Their names have been on my map long enough.",
        goal: Goal::Moment("boss_fall"),
        need: 2,
        from: 1,
        pays: &[(Spoil::Gold, 250), (Spoil::Gem, 2)],
    },
    Bounty {
        id: "mimic",
        title: "Trust Issues",
        says: "Kill a mimic. The chests have been looking at me funny.",
        goal: Goal::Moment("mimic_fall"),
        need: 1,
        from: 1,
        pays: &[(Spoil::Gold, 150), (Spoil::Gem, 1)],
    },
    Bounty {
        id: "demons",
        title: "Hellforge Overtime",
        says: "Six demons out of the Keep. Tobbin's fire is getting jealous.",
        goal: Goal::Slay(EnemyKind::Demon),
        need: 6,
        from: 3,
        pays: &[(Spoil::Gold, 250), (Spoil::Ember, 2)],
    },
    Bounty {
        id: "dragon",
        title: "Here Be Dragons",
        says: "There is a dragon on my map. I would like there not to be.",
        goal: Goal::Moment("victory"),
        need: 1,
        from: 2,
        pays: &[(Spoil::Gold, 400), (Spoil::Scale, 1)],
    },
    Bounty {
        id: "shamans",
        title: "Ward Off",
        says: "Three shamans. I'm tired of being boxed in by furniture I didn't build.",
        goal: Goal::Slay(EnemyKind::Shaman),
        need: 3,
        from: 3,
        pays: &[(Spoil::Gold, 220), (Spoil::Bond, 1)],
    },
    Bounty {
        id: "deep",
        title: "Below the Bottom",
        says: "Under Dragon Keep my map just says 'no'. Go and find out what 'no' looks like.",
        goal: Goal::Reach(4),
        need: 1,
        from: 3,
        pays: &[(Spoil::Gold, 300), (Spoil::Gem, 2)],
    },
    Bounty {
        id: "fleshers",
        title: "The Flesher's Bill",
        says: "Two fleshers. One of them has my good knife. I'd like it back, with the hand still attached or not.",
        goal: Goal::Slay(EnemyKind::Flesher),
        need: 2,
        from: 4,
        pays: &[(Spoil::Gold, 300), (Spoil::Ember, 2)],
    },
    Bounty {
        id: "spiders",
        title: "Pest Control",
        says: "Thirty spiderlings. The Archive's stacks are full of them and so, now, is my hair.",
        goal: Goal::Slay(EnemyKind::Spiderling),
        need: 30,
        from: 3,
        pays: &[(Spoil::Gold, 220), (Spoil::Wax, 4)],
    },
    Bounty {
        id: "spores",
        title: "Something Smells Like Bread",
        says: "Floor five smells like bread. Find out why. Don't eat anything.",
        goal: Goal::Reach(5),
        need: 1,
        from: 4,
        pays: &[(Spoil::Gold, 350), (Spoil::Gem, 3)],
    },
    Bounty {
        id: "prime",
        title: "Prime Time",
        says: "Five million viewers. Fortune has promised me a seat at the finale.",
        goal: Goal::Show(5000),
        need: 5000,
        from: 3,
        pays: &[(Spoil::Gold, 350), (Spoil::Ember, 2)],
    },
    Bounty {
        id: "grail",
        title: "The Search",
        says: "Everyone is looking for it. You'd be the first to find it. No pressure.",
        goal: Goal::Moment("grail"),
        need: 1,
        from: 5,
        pays: &[(Spoil::Gold, 800), (Spoil::Gem, 5), (Spoil::Scale, 2)],
    },
    Bounty {
        id: "vault",
        title: "Through the Wall",
        says: "Pip swears there are rooms on no map. Find me one. Bring a bomb.",
        goal: Goal::Moment("secret_found"),
        need: 1,
        from: 1,
        pays: &[(Spoil::Gold, 200), (Spoil::Gem, 2)],
    },
    Bounty {
        id: "hush",
        title: "Hush Money",
        says: "There's a goblin in the walls paying people to forget him. I'd like to forget him too. For a fee.",
        goal: Goal::Moment("snibbet"),
        need: 1,
        from: 1,
        pays: &[(Spoil::Gold, 150), (Spoil::Wax, 3)],
    },
    Bounty {
        id: "runes",
        title: "Glow-Chaser",
        says: "Five runes, any colour. I'm making a chart. The chart is mostly arrows.",
        goal: Goal::Moment("rune"),
        need: 5,
        from: 1,
        pays: &[(Spoil::Gold, 150), (Spoil::Ember, 2)],
    },
    Bounty {
        id: "frogs",
        title: "Frog Prince",
        says: "Get yourself hexed into a frog twice. For science. Tobbin is running a book on it.",
        goal: Goal::Moment("hexed"),
        need: 2,
        from: 2,
        pays: &[(Spoil::Gold, 120), (Spoil::Bond, 2)],
    },
    Bounty {
        id: "witches",
        title: "Witch Hunt",
        says: "Five hexers. They've been sending me frogs. Through the post.",
        goal: Goal::Slay(EnemyKind::Hexer),
        need: 5,
        from: 2,
        pays: &[(Spoil::Gold, 220), (Spoil::Gem, 2)],
    },
];

pub(crate) fn bounty(id: &str) -> Option<&'static Bounty> {
    BOUNTIES.iter().find(|b| b.id == id)
}

/// How far along a bounty is, as the board writes it: 7/25, 1.20M/1.50M,
/// or the floor to reach.
pub(crate) fn progress(bounty: &Bounty, have: u32) -> String {
    match bounty.goal {
        Goal::Show(need) => format!(
            "{}/{}",
            super::audience::viewers(have),
            super::audience::viewers(need)
        ),
        Goal::Reach(floor) => format!("floor {floor}"),
        _ => format!("{have}/{}", bounty.need),
    }
}

/// A bounty pinned to the board, and how far along it is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Pinned {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) have: u32,
}

impl Home {
    /// Fill the board to three. Wren pins what the floors the party has
    /// reached make fair, the ones she has paid least often first, in her
    /// order: every job comes round before any comes round twice, and a
    /// new floor's jobs go up first. Returns those just pinned.
    pub(crate) fn pin_bounties(&mut self) -> Vec<&'static Bounty> {
        let mut pinned = Vec::new();
        while self.bounties.len() < PINNED {
            let next = BOUNTIES
                .iter()
                .enumerate()
                .filter(|(_, b)| b.from <= self.deepest)
                .filter(|(_, b)| !self.bounties.iter().any(|p| p.id == b.id))
                .min_by_key(|&(order, b)| {
                    (self.bounties_paid.get(b.id).copied().unwrap_or(0), order)
                });
            let Some((_, next)) = next else {
                break;
            };
            self.bounties.push(Pinned {
                id: next.id.to_string(),
                have: 0,
            });
            pinned.push(next);
        }
        pinned
    }

    /// What a delve just did: `marks` (kills and moments), the floor it is
    /// on and its show. Returns the bounties it finished, taken off the
    /// board (the cockpit pays them and Wren pins the next).
    pub(crate) fn work_bounties(
        &mut self,
        marks: &BTreeMap<String, u32>,
        depth: u32,
        audience: u32,
    ) -> Vec<&'static Bounty> {
        let mut done = Vec::new();
        for pinned in &mut self.bounties {
            let Some(bounty) = bounty(&pinned.id) else {
                continue;
            };
            pinned.have = match bounty.goal {
                Goal::Slay(kind) => pinned.have + marks.get(&slay_mark(kind)).copied().unwrap_or(0),
                Goal::Moment(cue) => {
                    pinned.have + marks.get(&moment_mark(cue)).copied().unwrap_or(0)
                }
                Goal::Reach(floor) => u32::from(depth >= floor),
                Goal::Show(_) => pinned.have.max(audience),
            }
            .min(bounty.need);
            if pinned.have >= bounty.need {
                done.push(bounty);
            }
        }
        // Done ones come down, and any this build doesn't know.
        self.bounties
            .retain(|p| bounty(&p.id).is_some_and(|b| p.have < b.need));
        for bounty in &done {
            *self.bounties_paid.entry(bounty.id.to_string()).or_default() += 1;
        }
        done
    }
}

/// The mark a kill of `kind` leaves.
pub(crate) fn slay_mark(kind: EnemyKind) -> String {
    format!("slay:{kind:?}")
}

/// The mark a moment leaves (a cue's head).
pub(crate) fn moment_mark(cue: &str) -> String {
    format!("cue:{}", cue.split(':').next().unwrap_or(cue))
}

impl Run {
    /// Count what this tick did for Wren's bounties: the moments since
    /// `heard`, and the fallen.
    pub(super) fn mark_moments(&mut self, heard: usize) {
        let marks: Vec<String> = self
            .cues
            .get(heard..)
            .unwrap_or_default()
            .iter()
            .map(|cue| moment_mark(cue))
            .collect();
        for mark in marks {
            *self.marks.entry(mark).or_default() += 1;
        }
    }

    pub(super) fn mark_kill(&mut self, kind: EnemyKind) {
        *self.marks.entry(slay_mark(kind)).or_default() += 1;
    }
}

#[cfg(test)]
#[path = "../../../../tests/cockpit/app/together_bounties__tests.rs"]
mod tests;
