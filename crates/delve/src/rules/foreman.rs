//! The Foreman and his petitions: the only way a holding's rooms are
//! bought. The realm does not pick rooms off a board — the Foreman of the
//! Court of Deeds draws up petitions, three at a time, and any knight
//! answers them at the Court by holding E at the AYE or NAY post.
//!
//! His petitions are honest most of the time: the next room of some
//! family, at the list price (the King's masons' discount included). But
//! the Foreman has a wagon to feed, and a deal counter rather than dice:
//!
//! - every fifth deal is **padded**, the quote ten to forty gold in the
//!   hundred over the list price, with a reason that almost holds ("the
//!   wagon axle broke on the oak");
//! - every seventh deal is a **dud**: a real room at its real price, but
//!   the one the holding's neighbours like least — bought honest, regretted
//!   at leisure.
//!
//! The list prices stay on the steward's hall's readouts, so a padded
//! invoice is caught by knowledge and refused at the NAY post (a refusal
//! costs nothing but his temper: three in a row stir a point of unrest).
//! Petitions denied, lapsed or answered fall off at the next rent day,
//! when he drafts again. An approved petition is paid from the treasury at
//! the save, exactly as quoted, and the room starts going up in its bay.
//!
//! Every choice below is a fixed arithmetic of the deal counter, so the
//! Court's dealings are the same every time the realm rides the same road.

use super::holdings::{self, Built, Deed, Family, Holdings, SLOTS, UNREST_CAP};
use super::*;
use crate::realm::{Spoil, Spoils};

/// Petitions the Foreman will leave on his ledge at once.
pub const QUEUE: usize = 3;
/// Every fifth deal is padded, by this much in the hundred at most.
pub const PAD_EVERY: u32 = 5;
pub const PAD_STEP: u32 = 10;
/// Every seventh deal is the dud the neighbours like least.
pub const DUD_EVERY: u32 = 7;
/// Refusals in a row that stir a point of unrest.
pub const NAY_STIR: u32 = 3;

/// One of the Foreman's petitions: a room for a holding, at his price.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Petition {
    /// The holding it is for, by its deed key.
    pub key: String,
    /// The room, by its `RoomDef` id.
    pub room: String,
    /// The gold he asks, as quoted (padding and all).
    pub gold: u32,
    /// A relic the room wants from the deep, by its spoil word, if any.
    #[serde(default)]
    pub relic: Option<Spoil>,
    /// His line for it.
    #[serde(default)]
    pub says: String,
}

impl Petition {
    pub fn valid(&self) -> bool {
        self.key.len() <= 64
            && self.room.len() <= 32
            && self.says.len() <= 128
            && self.gold <= 100_000
    }
}

/// The Foreman the realm keeps at the Court, inside the holdings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Foreman {
    /// His ledge: the petitions waiting, front first.
    #[serde(default)]
    pub queue: Vec<Petition>,
    /// Deals he has drawn, all told: his padding and duds ride on it.
    #[serde(default)]
    pub dealt: u32,
    /// Refusals in a row.
    #[serde(default)]
    pub nays: u32,
}

impl Foreman {
    pub fn valid(&self) -> bool {
        self.queue.len() <= QUEUE && self.queue.iter().all(Petition::valid) && self.nays <= u32::MAX / 2
    }
}

/// The Foreman's word for a deal, however it went.
fn says(deal: u32, padded: bool, dud: bool, name: &str) -> String {
    if dud {
        [
            format!("trust me: {name} wants it"),
            format!("the neighbours will thank you for the {name}"),
            format!("I've a feeling about the {name}"),
        ][(deal / DUD_EVERY) as usize % 3]
            .clone()
    } else if padded {
        [
            "the wagon axle broke on the oak".to_string(),
            "oak, not pine — you'll thank me".to_string(),
            "the nail-seller has grown proud of late".to_string(),
            "good timber is shy this season".to_string(),
        ][(deal / PAD_EVERY) as usize % 4]
            .clone()
    } else {
        [
            "sound oak and honest nails".to_string(),
            "the crew is idle; the price is fair".to_string(),
            "measured twice, m'lord".to_string(),
            "I'd build it myself at this price".to_string(),
        ][deal as usize % 4]
            .clone()
    }
}

/// The holdings the Foreman will petition for: ours, unraided, with a bay
/// to spare, in the order they were bought.
fn eligible(h: &Holdings) -> Vec<String> {
    h.owned
        .iter()
        .filter(|w| w.raid == 0 && w.rooms.len() < SLOTS)
        .map(|w| w.key.clone())
        .collect()
}

/// The family a deal picks for `key`: the next of the family whose turn it
/// is on `deal`, or — for a dud — the one whose next room the neighbours
/// like least there.
fn family_for(h: &Holdings, deal: u32, key: &str, dud: bool) -> Option<(Family, &'static holdings::RoomDef)> {
    let w = h.holding(key)?;
    let mut turns: Vec<Family> = Family::ALL.into_iter().filter(|&f| w.next_of(f).is_some()).collect();
    if turns.is_empty() {
        return None;
    }
    if dud {
        // The dud: the family whose next room feels worst beside what
        // stands there. A tie stays in the families' own order.
        turns.sort_by_key(|&f| {
            let def = w.next_of(f).expect("kept");
            let felt: i32 = w.preview(def).iter().map(|&(n, _)| n).sum();
            (felt, f as usize)
        });
    } else {
        let at = (deal as usize) % turns.len();
        turns.rotate_left(at);
    }
    let family = turns[0];
    w.next_of(family).map(|def| (family, def))
}

/// Draw petitions until the ledge is full. Called on every rent day; the
/// deal counter, not dice, decides what he brings.
pub fn draft(h: &mut Holdings) {
    while h.foreman.queue.len() < QUEUE {
        let deal = h.foreman.dealt;
        let ours = eligible(h);
        if ours.is_empty() {
            break;
        }
        // The deal turns to the next holding that can take one.
        let start = (deal / QUEUE as u32) as usize % ours.len();
        let dud = deal > 0 && deal % DUD_EVERY == DUD_EVERY - 1;
        let picked = ours
            .iter()
            .cycle()
            .skip(start)
            .take(ours.len())
            .find_map(|key| family_for(h, deal, key, dud).map(|(_, def)| (key.clone(), def)));
        let Some((key, def)) = picked else {
            break;
        };
        let list = holdings::room_price(h, def);
        let padded = deal > 0 && deal % PAD_EVERY == PAD_EVERY - 1 && !dud;
        let gold = if padded { list + list / 100 * (PAD_STEP * (1 + deal % 4)) } else { list };
        let name = def.name;
        h.foreman.queue.push(Petition {
            key,
            room: def.id.to_string(),
            gold,
            relic: def.relic,
            says: says(deal, padded, dud, name),
        });
        h.foreman.dealt += 1;
    }
}

/// Whether the head petition can be answered now, and what an aye costs:
/// its quote, and a relic from the treasury if it wants one.
pub fn open(h: &Holdings, treasury: &Spoils) -> Result<(u32, Option<Spoil>), &'static str> {
    let p = h.foreman.queue.first().ok_or("NO PETITIONS")?;
    let Some(w) = h.holding(&p.key) else {
        return Err("GONE");
    };
    if w.raid > 0 {
        return Err("RAIDED");
    }
    if w.rooms.len() >= SLOTS {
        return Err("BAYS FULL");
    }
    if w.has(&p.room) {
        return Err("ALREADY BUILT");
    }
    if treasury.get(Spoil::Gold) < p.gold {
        return Err("PURSE SHORT");
    }
    if let Some(relic) = p.relic
        && treasury.get(relic) == 0
    {
        return Err("NEEDS A RELIC");
    }
    Ok((p.gold, p.relic))
}

/// The realm answers the head petition: `aye` pays his quote (and a relic,
/// if the room wants one) and the room starts going up; `nay` refuses it.
/// Either way it leaves the ledge. The line the ledger keeps.
pub fn answer(h: &mut Holdings, treasury: &mut Spoils, aye: bool) -> Result<String, &'static str> {
    let Some(p) = h.foreman.queue.first().cloned() else {
        return Err("NO PETITIONS");
    };
    let name = h
        .holding(&p.key)
        .and_then(|w| w.deed())
        .map_or_else(|| p.key.clone(), Deed::name);
    if !aye {
        h.foreman.queue.remove(0);
        h.foreman.nays += 1;
        if h.foreman.nays.is_multiple_of(NAY_STIR) {
            h.unrest = (h.unrest + 1).min(UNREST_CAP);
            return Ok(format!("The Foreman takes the refusal sorely ({name}): the works grumble"));
        }
        return Ok(format!("Refused: the Foreman folds his drawing ({name})"));
    }
    let (gold, relic) = open(h, treasury)?;
    let mut cost = Spoils::default();
    cost.add(Spoil::Gold, gold);
    if let Some(relic) = relic {
        cost.add(relic, 1);
    }
    treasury.take(&cost);
    let Some(w) = h.holding_mut(&p.key) else {
        return Err("GONE");
    };
    w.rooms.push(Built { id: p.room.clone(), labour: 0 });
    h.foreman.queue.remove(0);
    h.foreman.nays = 0;
    let room = holdings::room_def(&p.room).map_or("A room", |d| d.name);
    let relic_line = relic.map_or_else(String::new, |r| format!(" and a {}", r.word()));
    Ok(format!("A {room} is going up at {name} ({gold} gold{relic_line})"))
}

pub fn aye_mark() -> String {
    "holdings:petition:aye".into()
}

pub fn nay_mark() -> String {
    "holdings:petition:nay".into()
}
