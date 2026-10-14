//! The King's favour, and his cards: one at a time, after a rent day, his
//! herald stands outside the Land Office with a choice (Crusader Kings'
//! events, cut to two short answers with a price on each). Any knight
//! answers by holding E at the answer's post; the realm keeps the result.
//!
//! The favour is one number (`Holdings::favour`). Buying in fief raises it,
//! a claim lowers it, and the cards move it. High, the King's masons build
//! rooms for less; low, his men take the tithe from every holding.
//!
//! Every number is fixed. Gold is only ever spent here, never minted.

use super::holdings::{self, FAVOUR_CAP, Holdings, RAIDERS, STORES_CAP, SCROLLS_CAP, RETAINERS_CAP, UNREST_CAP, WALLS};
use super::*;
use crate::realm::{Spoil, Spoils};

/// One answer to a card: its words, and what it costs and does.
#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub label: &'static str,
    pub gold: u32,
    pub favour: i32,
    pub stores: i32,
    pub unrest: i32,
    /// Walls at every holding.
    pub walls: i32,
    pub scrolls: i32,
    pub retainers: i32,
    /// Raiders fall on the holding with the weakest walls.
    pub raid: bool,
}

const NIL: Choice = Choice {
    label: "",
    gold: 0,
    favour: 0,
    stores: 0,
    unrest: 0,
    walls: 0,
    scrolls: 0,
    retainers: 0,
    raid: false,
};

/// A card from the King: who comes, in a few words, and two answers.
#[derive(Clone, Copy, Debug)]
pub struct Card {
    pub title: &'static str,
    pub line: &'static str,
    pub picks: [Choice; 2],
}

/// The King's cards, sent in this order, round and round.
pub const CARDS: [Card; 8] = [
    Card {
        title: "ROYAL VISIT",
        line: "THE KING RIDES OUT",
        picks: [
            Choice { label: "FEAST HIM", gold: 300, favour: 2, ..NIL },
            Choice { label: "BAR THE GATE", favour: -2, unrest: -1, ..NIL },
        ],
    },
    Card {
        title: "TITHE DEMAND",
        line: "HIS CLERKS COUNT COIN",
        picks: [
            Choice { label: "PAY IT", gold: 200, favour: 1, ..NIL },
            Choice { label: "REFUSE", favour: -2, unrest: 1, ..NIL },
        ],
    },
    Card {
        title: "DWARF MASONS",
        line: "BRANNOC'S MASONS WANT WORK",
        picks: [
            Choice { label: "HIRE THEM", gold: 250, walls: 3, ..NIL },
            Choice { label: "SEND AWAY", ..NIL },
        ],
    },
    Card {
        title: "BREWERS' GUILD",
        line: "THE BREWERS WANT A CELLAR",
        picks: [
            Choice { label: "GRANT IT", stores: 6, favour: -1, ..NIL },
            Choice { label: "REFUSE", unrest: 1, ..NIL },
        ],
    },
    Card {
        title: "PILGRIMS",
        line: "PILGRIMS BEG A ROOF",
        picks: [
            Choice { label: "TAKE THEM IN", stores: -3, unrest: -2, ..NIL },
            Choice { label: "TURN AWAY", unrest: 1, ..NIL },
        ],
    },
    Card {
        title: "MONKS' OFFER",
        line: "MONKS WILL COPY CHARTERS",
        picks: [
            Choice { label: "BUY SCROLLS", gold: 300, scrolls: 2, ..NIL },
            Choice { label: "NO THANKS", ..NIL },
        ],
    },
    Card {
        title: "SELLSWORDS",
        line: "FREE LANCES SEEK A LORD",
        picks: [
            Choice { label: "HIRE THEM", gold: 200, retainers: 3, ..NIL },
            Choice { label: "MOVE ON", ..NIL },
        ],
    },
    Card {
        title: "BORDER BARON",
        line: "A BARON EYES YOUR WALLS",
        picks: [
            Choice { label: "STAND FIRM", favour: 1, raid: true, ..NIL },
            Choice { label: "GIVE GROUND", walls: -2, favour: -1, ..NIL },
        ],
    },
];

fn shift(n: u32, by: i32, cap: u32) -> u32 {
    (i64::from(n) + i64::from(by)).clamp(0, i64::from(cap)) as u32
}

/// Whether an answer can be paid.
pub fn open(choice: &Choice, gold: u32) -> Result<(), &'static str> {
    if gold < choice.gold { Err("PURSE SHORT") } else { Ok(()) }
}

/// The realm answers card `card` with `pick`: paid from the treasury, its
/// effects on the holdings, the card cleared. The line the ledger keeps.
pub fn answer(h: &mut Holdings, treasury: &mut Spoils, card: u8, pick: usize) -> Result<String, &'static str> {
    let c = CARDS.get(usize::from(card)).ok_or("NO CARD")?;
    let choice = c.picks.get(pick).ok_or("NO CARD")?;
    open(choice, treasury.get(Spoil::Gold))?;
    let mut cost = Spoils::default();
    cost.add(Spoil::Gold, choice.gold);
    treasury.take(&cost);
    h.favour = (h.favour + choice.favour).clamp(-FAVOUR_CAP, FAVOUR_CAP);
    h.stores = shift(h.stores, choice.stores, STORES_CAP);
    h.unrest = shift(h.unrest, choice.unrest, UNREST_CAP);
    h.scrolls = shift(h.scrolls, choice.scrolls, SCROLLS_CAP);
    h.retainers = shift(h.retainers, choice.retainers, RETAINERS_CAP);
    for w in &mut h.owned {
        w.walls = shift(w.walls, choice.walls, WALLS);
    }
    if choice.raid
        && let Some(w) = h.owned.iter_mut().filter(|w| w.raid == 0).min_by_key(|w| w.walls)
    {
        w.raid = 1;
    }
    h.card = None;
    let title = c.title.to_lowercase();
    Ok(format!("{}{}: {}", title[..1].to_uppercase(), &title[1..], choice.label.to_lowercase()))
}

/// What the realm's favour does now, in two words, if anything.
pub fn favour_word(h: &Holdings) -> Option<&'static str> {
    if h.favour >= holdings::ROYAL {
        Some("ROYAL MASONS")
    } else if h.favour <= -holdings::ROYAL {
        Some("KING'S MEN")
    } else {
        None
    }
}

/// Where the King's herald stands with his card, by the Land Office's door,
/// and the two answer posts either side of him (kingdom tiles). Clear of
/// every door, banner and little thing of the kingdom's.
pub fn herald(geo: &super::map::Geography) -> Option<((i32, i32), [(i32, i32); 2])> {
    let site = geo.sites.iter().find(|s| s.room == RoomKind::LandOffice)?;
    let (x, y) = site.stand;
    let others: Vec<(i32, i32)> = super::classes::stones(geo)
        .into_iter()
        .map(|(_, p)| p)
        .chain(super::trinkets::things(geo).into_iter().map(|(_, p)| p))
        .collect();
    let open = |(px, py): (i32, i32)| {
        geo.room.tile(px, py) == Tile::Floor
            && geo.sites.iter().all(|s| (s.stand.0 - px).abs() + (s.stand.1 - py).abs() > 2)
            && others.iter().all(|o| (o.0 - px).abs() + (o.1 - py).abs() > 3)
    };
    [(4, 2), (-4, 2), (0, 3), (5, 1), (-5, 1), (4, 4), (-4, 4), (0, 5)]
        .into_iter()
        .chain((-9..=9).flat_map(|dx| (-3..=7).map(move |dy| (dx, dy))))
        .map(|(dx, dy)| (x + dx, y + dy))
        .map(|h| (h, [(h.0 - 2, h.1), (h.0 + 2, h.1)]))
        .find(|&(h, [a, b])| open(h) && open(a) && open(b))
}

/// The answer post a knight stands at, if any (within a step of it).
pub fn post_under(run: &Run, id: u32) -> Option<usize> {
    if run.room().kind != RoomKind::Kingdom || run.home.holdings.card.is_none() {
        return None;
    }
    let hero = run.players.get(&id).filter(|h| h.hp > 0 && !h.stone)?;
    let geo = super::map::geography(&run.kingdom.wishes);
    let (_, posts) = herald(&geo)?;
    posts.iter().position(|&(px, py)| {
        (hero.x - (px as f32 + 0.5) * TILE_UNITS).hypot(hero.y - (py as f32 + 0.7) * TILE_UNITS) < 1.4 * TILE_UNITS
    })
}

pub fn card_mark(card: u8, pick: usize) -> String {
    format!("holdings:card:{card}:{pick}")
}

impl Run {
    /// The King's herald in the kingdom: while a card waits the way-on
    /// arrow leads to him, and holding E at a post answers it.
    pub(super) fn tick_crown(&mut self, inputs: &BTreeMap<u32, Input>) {
        let Some(card) = self.home.holdings.card else {
            return;
        };
        if self.room().kind != RoomKind::Kingdom {
            return;
        }
        if self.kingdom.moments.errand.is_none() {
            self.kingdom.moments.errand = Some(RoomKind::LandOffice);
        }
        let answered = (0..2).any(|pick| self.marks.contains_key(&card_mark(card, pick)));
        let gold = self.treasury.get(Spoil::Gold);
        let on: Vec<(u32, usize)> =
            self.players.keys().filter_map(|&id| post_under(self, id).map(|p| (id, p))).collect();
        for (id, pick) in on {
            let pressing = inputs.get(&id).copied().filter(|i| i.valid()).is_some_and(|i| i.interact);
            let hero = self.players.get_mut(&id).expect("listed");
            if !pressing {
                hero.buying = 0;
                continue;
            }
            if answered || hero.buy_spent || open(&CARDS[usize::from(card)].picks[pick], gold).is_err() {
                continue;
            }
            hero.buying = hero.buying.saturating_add(1);
            if hero.buying >= super::home::BUY_HOLD {
                hero.buying = 0;
                hero.buy_spent = true;
                *self.marks.entry(card_mark(card, pick)).or_default() += 1;
                self.sounds.push("play_card");
                self.cues.push("king_card".into());
                return;
            }
        }
    }
}

/// How many raiders a raid of `level` sends, less one for each standing
/// trade room (bought off), and never fewer than two.
pub fn raiders(level: u8, bought: u32) -> u32 {
    RAIDERS[usize::from(level.min(holdings::RAID_MAX))].saturating_sub(bought).max(2)
}
