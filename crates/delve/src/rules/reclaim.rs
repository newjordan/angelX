//! King Brannoc's reclaiming of the underground: the lost halls of Caer
//! Dwfn, and the knights who take them back for him.
//!
//! Below the mine-head the goblin clans took every hall of the dwarf-kingdom.
//! Once the King has been met he sends the party after them, one hall at a
//! time, down the delve he names:
//!
//! - **The Delvers' Gallery** (the Mines): Chief Nubbin Tallowtooth sits in
//!   the King's grandsire's chair. Pay his toll, or fight him for it.
//! - **The Hall of Lamps** (the Crypt): Thane Madoc Greyhelm cannot rest
//!   while his four sons' lamps are cold. Kindle them eldest son first.
//! - **The Anvil Gallery** (Dragon Keep): Olwen Anvilhand, the King's smith,
//!   lies under a hundred years of fallen roof. Dig her out.
//!
//! A hall asked of the party is a room off every floor of its delve
//! (`add_lost_hall`), held against the King until the party plays its
//! set-piece out (`play`). A hall won is the King's: he thanks the party
//! once, his tribute grows by the hall's rent for every floor cleared, and
//! the dwarves who come home with it join his crew. The run only marks
//! (`reclaim:*`); `settle` pays and counts, a pure function of the realm's
//! state, like the barony's. Every number is a fixed one.

use super::activity::{Realm, Settled};
use super::barony::{Barony, Court};
use super::*;
use crate::realm::{Spoil, Spoils};

pub mod play;
pub mod rooms;
pub use play::{HallPlay, Play, Talk, TalkView, Who};
pub use rooms::{add_lost_hall, lost_hall_room};

pub struct Hall {
    pub id: &'static str,
    pub name: &'static str,
    pub pack: Pack,
    pub keeper: &'static str,
    pub asks: &'static str,
    pub lore: &'static str,
    pub thanks: &'static [(Spoil, u32)],
    pub rent: &'static [(Spoil, u32)],
    pub rent_bargain: &'static [(Spoil, u32)],
    pub crew: u32,
    pub ways: &'static [&'static str],
}

pub const HALLS: [Hall; 3] = [
    Hall {
        id: "gallery",
        name: "The Delvers' Gallery",
        pack: Pack::Cavern,
        keeper: "Chief Nubbin Tallowtooth",
        asks: "Me delvers' gallery's the first hall below the workings, and a goblin called Nubbin sits in me grandsire's chair, eating from me grandsire's barrels. Take it back: pay him off or knock him off. Either way I'll have me chair.",
        lore: "A long gallery in the Mines, off one of its halls, pillared in the old way. Nubbin Tallowtooth leads the Tallow-Hands, who burn dwarf-candles in their teeth. He'll take a toll to leave, or he'll fight, and he rides a boar.",
        thanks: &[(Spoil::Gold, 250)],
        rent: &[(Spoil::Gold, 25)],
        rent_bargain: &[(Spoil::Ore, 2)],
        crew: 1,
        ways: &["fight", "bargain"],
    },
    Hall {
        id: "lamps",
        name: "The Hall of Lamps",
        pack: Pack::Crypt,
        keeper: "Thane Madoc Greyhelm",
        asks: "Me father's lamp-hall lies under the Crypt, and Madoc Greyhelm, his lamp-warden, never left it. Four lamps for four sons, and all four cold. He'll not rest till they burn. Light them for him and give an old thane his sleep.",
        lore: "Four name-stones, four lamps, and a ghost who has stood between them for a hundred years. The sons' names are cut in the stones with their winters. Madoc wants them lit eldest first. Light one out of turn and they all gutter, and the dead come to see why.",
        thanks: &[(Spoil::Gem, 3)],
        rent: &[(Spoil::Gold, 30), (Spoil::Bone, 2)],
        rent_bargain: &[],
        crew: 1,
        ways: &["rite"],
    },
    Hall {
        id: "anvils",
        name: "The Anvil Gallery",
        pack: Pack::Hellforge,
        keeper: "Olwen Anvilhand",
        asks: "Olwen Anvilhand was at her last bar in the Anvil Gallery when the roof came down on her. A dwarf can sleep in the rock a hundred years if it falls kindly. Dig her out. The Keep's imps nest in that rubble, mind.",
        lore: "A smithy-gallery under Dragon Keep, three heaps of fallen roof, and the King's smith somewhere beneath them with her apprentice. Every heap dug wakes the Keep's imps, and the dust does not settle.",
        thanks: &[(Spoil::Gold, 150), (Spoil::Ore, 10)],
        rent: &[(Spoil::Gold, 20), (Spoil::Ore, 4)],
        rent_bargain: &[],
        crew: 2,
        ways: &["freed"],
    },
];

pub fn hall(id: &str) -> Option<&'static Hall> {
    HALLS.iter().find(|h| h.id == id)
}

pub fn hall_of(pack: Pack) -> Option<(usize, &'static Hall)> {
    HALLS.iter().enumerate().find(|(_, h)| h.pack == pack)
}

pub const TOLL: u32 = 300;

fn spoils(list: &[(Spoil, u32)]) -> Spoils {
    let mut s = Spoils::default();
    for &(spoil, n) in list {
        s.add(spoil, n);
    }
    s
}

impl Hall {
    pub fn thanks(&self) -> Spoils {
        spoils(self.thanks)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Won {
    pub id: String,
    pub how: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Halls {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub won: Vec<Won>,
}

impl Halls {
    pub fn is_empty(&self) -> bool {
        self.asked.is_none() && self.won.is_empty()
    }

    pub fn is_held(&self, id: &str) -> bool {
        self.won.iter().any(|w| w.id == id)
    }

    pub fn how(&self, id: &str) -> Option<&str> {
        self.won.iter().find(|w| w.id == id).map(|w| w.how.as_str())
    }

    pub fn count(&self) -> usize {
        self.won.len()
    }

    pub fn offer(&self, b: &Barony) -> Option<(usize, &'static Hall)> {
        if b.court < Court::Met || self.asked.is_some() {
            return None;
        }
        HALLS.iter().enumerate().find(|(_, h)| !self.is_held(h.id))
    }

    pub fn asked_hall(&self) -> Option<(usize, &'static Hall)> {
        let i = usize::from(self.asked?);
        HALLS.get(i).map(|h| (i, h))
    }

    pub fn crew(&self) -> u32 {
        self.won.iter().filter_map(|w| hall(&w.id)).map(|h| h.crew).sum()
    }

    pub fn rent(&self) -> Spoils {
        let mut t = Spoils::default();
        for w in &self.won {
            let Some(h) = hall(&w.id) else { continue };
            t.merge(&spoils(h.rent));
            if w.how == "bargain" {
                t.merge(&spoils(h.rent_bargain));
            }
        }
        t
    }
}

pub fn ask_mark(i: usize) -> String {
    format!("reclaim:ask:{i}")
}

pub fn toll_mark(i: usize) -> String {
    format!("reclaim:toll:{i}")
}

pub fn won_mark(i: usize, how: &str) -> String {
    format!("reclaim:won:{i}:{how}")
}

pub fn settle(realm: &mut Realm<'_>, marks: &BTreeMap<String, u32>) -> Settled {
    let mut out = Settled::default();
    let b = &mut realm.home.barony;
    for mark in marks.keys().filter_map(|m| m.strip_prefix("reclaim:")) {
        let mut parts = mark.split(':');
        let (Some(kind), Some(i)) = (parts.next(), parts.next().and_then(|i| i.parse::<usize>().ok())) else {
            continue;
        };
        let Some(hall) = HALLS.get(i) else { continue };
        match kind {
            "ask" => {
                if b.halls.offer(b).is_some_and(|(n, _)| n == i) {
                    b.halls.asked = Some(i as u8);
                    out.changed = true;
                    out.said.push((
                        format!("hall_asked:{}", hall.id),
                        format!(
                            "King Brannoc sends the party to {}: it lies off the floors of {}",
                            hall.name,
                            hall.pack.name()
                        ),
                    ));
                }
            }
            "toll" => {
                if b.halls.asked != Some(i as u8) || b.halls.is_held(hall.id) || !hall.ways.contains(&"bargain") {
                    continue;
                }
                let toll = spoils(&[(Spoil::Gold, TOLL)]);
                if !realm.treasury.covers(&toll) {
                    out.said.push((
                        "cant_afford".into(),
                        format!("{}: {}", hall.keeper, realm.treasury.shortfall(&toll)),
                    ));
                    continue;
                }
                realm.treasury.take(&toll);
                b.ledger.write(format!("The party paid {} his toll", hall.keeper), &toll, true);
                win(b, realm.treasury, i, "bargain", &mut out);
            }
            "won" => {
                let how = parts.next().unwrap_or_default();
                if b.halls.asked != Some(i as u8) || b.halls.is_held(hall.id) || !hall.ways.contains(&how) || how == "bargain" {
                    continue;
                }
                win(b, realm.treasury, i, how, &mut out);
            }
            _ => {}
        }
    }
    out
}

fn win(b: &mut Barony, treasury: &mut Spoils, i: usize, how: &str, out: &mut Settled) {
    let hall = &HALLS[i];
    b.halls.asked = None;
    b.halls.won.push(Won {
        id: hall.id.into(),
        how: how.into(),
    });
    let thanks = if how == "bargain" { Spoils::default() } else { hall.thanks() };
    if !thanks.is_empty() {
        treasury.merge(&thanks);
        b.ledger.write(format!("{} won: his thanks", hall.name), &thanks, false);
    }
    out.changed = true;
    let paid = if thanks.is_empty() {
        String::new()
    } else {
        format!(", and {} in thanks", thanks.label())
    };
    out.said.push((
        format!("hall_won:{}", hall.id),
        format!("{} is King Brannoc's again{paid}. His tribute grows with it", hall.name),
    ));
}

#[cfg(test)]
#[path = "../../tests/rules/reclaim__tests.rs"]
mod tests;
