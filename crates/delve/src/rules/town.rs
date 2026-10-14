//! The town's row, south of the quay: three rooms that tie the realm's
//! purse to its people.
//!
//! - **Haggler's Square** (the Artisan Quarter's door): three stalls buy the
//!   realm's spare ore, wax and bone for gold. Hold E and the offer climbs;
//!   let go to sell. Hold too long and the stallholder is offended and
//!   shuts up shop for a while. The sale settles with the realm's save.
//! - **The Royal Soak**: four tubs. A soak costs the treasury a few gold;
//!   the knight climbs in, Brother Suds scrubs, and they come out scrubbed:
//!   the next floor left without the privy costs them nothing.
//! - **The Dented Anvil** (the Smithy's door): Old Dint tempers the arm in
//!   hand a step, and mends missing helm, mail and shield, for the
//!   treasury's gold. He tests his mending by hitting it.
//!
//! The soak and the smith are paid like the surface peddler: the run asks
//! (`Run::peddled`), the cockpit charges the treasury, then `peddle` hands
//! the work over. Every number is fixed.
use super::*;
use super::merchant::{Purchase, Shop};

pub const PLACES: [RoomKind; 3] = [RoomKind::Market, RoomKind::Bathhouse, RoomKind::Smithy];

/// A stall: who keeps it, what they buy, the opening offer and the most
/// they will ever pay, where its counter stands and where a seller stands.
pub struct Stall {
    pub keeper: &'static str,
    pub buys: Spoil,
    pub word: &'static str,
    pub base: u32,
    pub top: u32,
    pub counter: (i32, i32, i32, i32),
    pub at: (f32, f32),
}
pub const STALLS: [Stall; 3] = [
    Stall { keeper: "MAGDA", buys: Spoil::Ore, word: "ORE", base: 6, top: 10, counter: (3, 2, 4, 2), at: (5.0, 4.6) },
    Stall { keeper: "THE CHANDLER", buys: Spoil::Wax, word: "WAX", base: 5, top: 9, counter: (17, 2, 4, 2), at: (19.0, 4.6) },
    Stall { keeper: "SOUP NED", buys: Spoil::Bone, word: "BONE", base: 3, top: 6, counter: (16, 10, 4, 2), at: (18.0, 9.2) },
];
/// The market cross in the south-west corner.
pub const CROSS: (i32, i32, i32, i32) = (4, 10, 2, 2);
/// Ticks of holding E for each extra gold; how long past the top offer a
/// stallholder's patience lasts; how long an offended one stays shut.
pub const HAGGLE_STEP: u32 = 6;
pub const PATIENCE: u32 = HZ;
pub const SHUT: u32 = 3 * HZ;

/// The four tubs (their middles, tiles), the copper that heats them, the
/// towel bench; what a soak costs and how long it lasts.
pub const TUBS: [(f32, f32); 4] = [(5.0, 3.6), (9.5, 3.6), (14.5, 3.6), (19.0, 3.6)];
pub const COPPER: (i32, i32, i32, i32) = (21, 1, 2, 3);
pub const BENCH: (i32, i32, i32, i32) = (2, 11, 5, 1);
pub const BATH_GOLD: u32 = 5;
pub const SOAK: u32 = 3 * HZ;

/// The smithy: the hearth, the anvil, the armour rack and the quench
/// barrel; where a knight stands at the anvil and at the rack.
pub const HEARTH: (i32, i32, i32, i32) = (9, 1, 6, 3);
pub const ANVIL: (i32, i32, i32, i32) = (11, 5, 2, 1);
pub const RACK: (i32, i32, i32, i32) = (18, 1, 4, 2);
pub const BARREL: (i32, i32, i32, i32) = (3, 2, 2, 2);
pub const AT_ANVIL: (f32, f32) = (9.9, 5.6);
pub const AT_RACK: (f32, f32) = (20.0, 3.7);
pub const TEMPER_GOLD: u32 = 40;
pub const MEND_GOLD: u32 = 25;
/// How long the hammer rings on a tempering, and the helmet on a mending.
pub const HAMMER: u32 = HZ;
pub const DING: u32 = HZ;

/// How near (arena units) a knight stands to use a counter, tub or anvil.
const REACH: f32 = 1.6;

/// The town's moment-to-moment state: who is haggling at which stall and
/// for how long, which stalls are shut and for how long, and the smith's
/// hammer and helmet test.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Town {
    pub haggles: BTreeMap<u32, (u8, u32)>,
    pub shut: [u32; 3],
    pub hammer: Option<(u32, u32)>,
    pub ding: Option<(u32, u32)>,
}

impl Town {
    pub fn valid(&self) -> bool {
        self.haggles.len() <= MAX_PLAYERS as usize
            && self.haggles.iter().all(|(&id, &(stall, ticks))| {
                (1..=MAX_PLAYERS).contains(&id) && usize::from(stall) < STALLS.len() && ticks <= haggle_limit(usize::from(stall))
            })
            && self.shut.iter().all(|&t| t <= SHUT)
            && self.hammer.is_none_or(|(id, t)| (1..=MAX_PLAYERS).contains(&id) && t <= HAMMER)
            && self.ding.is_none_or(|(id, t)| (1..=MAX_PLAYERS).contains(&id) && t <= DING)
    }
}

/// The ticks of holding before stall `i`'s keeper takes offence.
pub fn haggle_limit(i: usize) -> u32 {
    (STALLS[i].top - STALLS[i].base) * HAGGLE_STEP + PATIENCE
}

/// What stall `i` offers after `ticks` of haggling.
pub fn offer(i: usize, ticks: u32) -> u32 {
    (STALLS[i].base + ticks / HAGGLE_STEP).min(STALLS[i].top)
}

/// A sale waiting for the realm's save: which stall's spoil, by which seat.
pub fn sale_mark(i: usize, id: u32) -> String {
    format!("country:sell:{}:{id}", STALLS[i].word.to_lowercase())
}

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, tile);
        }
    }
}

/// Solid furniture; the tubs stay floor (a knight climbs in).
pub fn shape(room: &mut Room) {
    match room.kind {
        RoomKind::Market => {
            for s in &STALLS {
                fill(room, s.counter, Tile::Block);
            }
            fill(room, CROSS, Tile::Block);
        }
        RoomKind::Bathhouse => {
            fill(room, COPPER, Tile::Block);
            fill(room, BENCH, Tile::Block);
        }
        RoomKind::Smithy => {
            for rect in [HEARTH, ANVIL, RACK, BARREL] {
                fill(room, rect, Tile::Block);
            }
        }
        _ => {}
    }
}

/// The realm settles the square's sales: a spoil out, its gold in.
pub fn settle(treasury: &mut Spoils, marks: &BTreeMap<String, u32>, news: &mut Vec<String>) -> bool {
    let mut changed = false;
    for (i, stall) in STALLS.iter().enumerate() {
        for id in 1..=MAX_PLAYERS {
            let Some(&gold) = marks.get(&sale_mark(i, id)) else {
                continue;
            };
            let gold = gold.clamp(stall.base, stall.top);
            let mut one = Spoils::default();
            one.add(stall.buys, 1);
            if !treasury.covers(&one) {
                news.push(format!("{}: no {} in the treasury to sell.", stall.keeper, stall.word.to_lowercase()));
                continue;
            }
            treasury.take(&one);
            treasury.add(Spoil::Gold, gold);
            news.push(format!("Sold one {} to {} for {gold} gold.", stall.word.to_lowercase(), stall.keeper));
            changed = true;
        }
    }
    changed
}

impl Run {
    fn near_spot(&self, id: u32, at: (f32, f32)) -> bool {
        self.players
            .get(&id)
            .is_some_and(|h| (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < REACH)
    }

    /// One tick in the town; `pressed` holds the knights whose E went down.
    pub(super) fn tick_town(&mut self, inputs: &BTreeMap<u32, Input>, pressed: u8) {
        let kind = self.room().kind;
        // Clocks run down anywhere; the soakers stay in their tubs.
        for t in &mut self.town.shut {
            *t = t.saturating_sub(1);
        }
        for slot in [&mut self.town.hammer, &mut self.town.ding] {
            if let Some((_, t)) = slot {
                *t = t.saturating_sub(1);
            }
            if slot.is_some_and(|(_, t)| t == 0) {
                *slot = None;
            }
        }
        self.tick_soaks(kind);
        if kind != RoomKind::Market {
            self.town.haggles.clear();
        }
        if !PLACES.contains(&kind) {
            return;
        }
        let able: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0 && h.soak == 0)
            .map(|(&id, _)| id)
            .collect();
        match kind {
            RoomKind::Market => self.tick_market(inputs, pressed, &able),
            RoomKind::Bathhouse => {
                for &id in &able {
                    if pressed & 1 << (id & 7) != 0 {
                        self.ask_for_a_soak(id);
                    }
                }
            }
            _ => {
                for &id in &able {
                    if pressed & 1 << (id & 7) == 0 {
                        continue;
                    }
                    if self.near_spot(id, AT_ANVIL) {
                        self.ask_the_smith(id, 0);
                    } else if self.near_spot(id, AT_RACK) {
                        self.ask_the_smith(id, 1);
                    }
                }
            }
        }
    }

    fn tick_market(&mut self, inputs: &BTreeMap<u32, Input>, pressed: u8, able: &[u32]) {
        for &id in able {
            let holding = inputs.get(&id).is_some_and(|i| i.valid() && i.interact);
            if let Some((stall, ticks)) = self.town.haggles.get(&id).copied() {
                let i = usize::from(stall);
                if !holding || !self.near_spot(id, STALLS[i].at) {
                    // Let go: sold at the offer on the table.
                    self.town.haggles.remove(&id);
                    let gold = offer(i, ticks);
                    self.marks.insert(sale_mark(i, id), gold);
                    self.sounds.push("gold");
                    self.found = Some((self.tick, id, format!("SOLD! {gold} GOLD.")));
                } else if ticks + 1 > haggle_limit(i) {
                    // Pushed too far: the shutters come down.
                    self.town.haggles.remove(&id);
                    self.town.shut[i] = SHUT;
                    self.sounds.push("door_seal");
                    self.sounds.push(super::chatter::clip(id, "uhoh", (self.tick % 2) as usize));
                    self.found = Some((self.tick, id, format!("{}: OUT!", STALLS[i].keeper)));
                } else {
                    self.town.haggles.insert(id, (stall, ticks + 1));
                    if offer(i, ticks + 1) > offer(i, ticks) {
                        self.sounds.push("card_pickup");
                    }
                }
                continue;
            }
            if pressed & 1 << (id & 7) == 0 {
                continue;
            }
            let Some(i) = STALLS.iter().position(|s| self.near_spot(id, s.at)) else {
                continue;
            };
            let stall = &STALLS[i];
            let message = if self.town.shut[i] > 0 {
                "SHUT. SULKING.".to_string()
            } else if self.treasury.get(stall.buys) == 0 {
                format!("NO {} TO SELL.", stall.word)
            } else if self.town.haggles.values().any(|&(s, _)| usize::from(s) == i) {
                "ONE AT A TIME!".to_string()
            } else {
                self.town.haggles.insert(id, (i as u8, 0));
                format!("{} {}G?", stall.word, stall.base)
            };
            self.found = Some((self.tick, id, message));
        }
    }

    /// The tub before knight `id`, if they stand at one.
    fn tub_before(&self, id: u32) -> Option<usize> {
        TUBS.iter().position(|&(x, y)| self.near_spot(id, (x, y + 1.6)))
    }

    fn ask_for_a_soak(&mut self, id: u32) {
        let Some(tub) = self.tub_before(id) else {
            return;
        };
        let message = if self.players.values().any(|h| h.soak > 0 && tub_of(h) == Some(tub)) {
            "TUB TAKEN.".to_string()
        } else if self.players[&id].scrubbed {
            "ALREADY SQUEAKY.".to_string()
        } else if self.treasury.get(Spoil::Gold) < BATH_GOLD {
            format!("A SOAK IS {BATH_GOLD}G.")
        } else {
            self.peddled.push(Purchase { knight: id, offer: tub, shop: Shop::Bath });
            format!("SOAK: {BATH_GOLD}G.")
        };
        self.found = Some((self.tick, id, message));
    }

    /// Old Dint's two jobs: 0 tempers the arm in hand, 1 mends the kit.
    fn ask_the_smith(&mut self, id: u32, job: usize) {
        let hero = &self.players[&id];
        let (refusal, gold) = if job == 0 {
            let refusal = if hero.weapon == Weapon::Fists && hero.forged.is_none() {
                Some("NO ARM TO TEMPER.")
            } else if hero.forged.is_some() {
                Some("TOO FANCY. SORRY.")
            } else if hero.temper >= feel::TEMPER_MAX {
                Some("CAN'T BEAT IT BETTER.")
            } else {
                None
            };
            (refusal, TEMPER_GOLD)
        } else {
            let whole = hero.owned.helmet && hero.owned.body && hero.owned.shield;
            (whole.then_some("NOTHING TO MEND."), MEND_GOLD)
        };
        let message = if let Some(no) = refusal {
            no.to_string()
        } else if self.treasury.get(Spoil::Gold) < gold {
            format!("THAT'S {gold}G.")
        } else {
            self.peddled.push(Purchase { knight: id, offer: job, shop: Shop::Smith });
            format!("{}: {gold}G.", if job == 0 { "TEMPER" } else { "MEND" })
        };
        self.found = Some((self.tick, id, message));
    }

    /// What a town purchase costs, while it can still be done.
    pub(super) fn town_price(&self, p: Purchase) -> Option<u32> {
        let hero = self.players.get(&p.knight)?;
        match p.shop {
            Shop::Bath => (self.room().kind == RoomKind::Bathhouse
                && p.offer < TUBS.len()
                && hero.soak == 0
                && !hero.scrubbed
                && !self.players.values().any(|h| h.soak > 0 && tub_of(h) == Some(p.offer)))
            .then_some(BATH_GOLD),
            Shop::Smith if self.room().kind == RoomKind::Smithy => match p.offer {
                0 => (hero.weapon != Weapon::Fists && hero.forged.is_none() && hero.temper < feel::TEMPER_MAX)
                    .then_some(TEMPER_GOLD),
                1 => (!(hero.owned.helmet && hero.owned.body && hero.owned.shield)).then_some(MEND_GOLD),
                _ => None,
            },
            _ => None,
        }
    }

    /// The treasury paid for town purchase `p`: the work is done.
    pub(super) fn town_paid(&mut self, p: Purchase) {
        let Some(hero) = self.players.get_mut(&p.knight) else {
            return;
        };
        let take = (self.tick % 2) as usize;
        match (p.shop, p.offer) {
            (Shop::Bath, tub) => {
                let (x, y) = TUBS[tub.min(TUBS.len() - 1)];
                (hero.x, hero.y) = (x * TILE_UNITS, y * TILE_UNITS);
                hero.soak = SOAK;
                hero.sitting = false;
                self.sounds.push("holy_water");
                self.sounds.push(super::chatter::clip(p.knight, "ooh", take));
                self.found = Some((self.tick, p.knight, "SCRUB-A-DUB!".into()));
            }
            (Shop::Smith, 0) => {
                hero.temper = (hero.temper + 1).min(feel::TEMPER_MAX);
                let arm = format!("{}{}", hero.weapon.name(), hud::tier(hero.temper)).to_uppercase();
                self.town.hammer = Some((p.knight, HAMMER));
                self.cues.push(format!("temper:{}", hero.temper));
                self.sounds.push("clang");
                self.fx.push(Fx::Shake(2));
                self.found = Some((self.tick, p.knight, format!("CLANG! {arm}.")));
            }
            (Shop::Smith, _) => {
                for (missing, piece) in [
                    (!hero.owned.helmet, "iron-helm"),
                    (!hero.owned.body, "body-armour"),
                    (!hero.owned.shield, "kite-shield"),
                ] {
                    if missing {
                        hero.wear_piece(piece);
                    }
                }
                self.keep_gear(p.knight);
                self.town.ding = Some((p.knight, DING));
                self.sounds.push("clang");
                self.sounds.push(super::chatter::clip(p.knight, "oof", take));
                self.fx.push(Fx::Shake(3));
                self.found = Some((self.tick, p.knight, "DING! GOOD AS NEW. MOSTLY.".into()));
            }
            _ => {}
        }
    }

    /// Soakers stay in their tubs until the soak is done; out, they are
    /// scrubbed. A knight who leaves the bathhouse gets out at once.
    fn tick_soaks(&mut self, kind: RoomKind) {
        let tick = self.tick;
        let mut done = Vec::new();
        for (&id, hero) in &mut self.players {
            if hero.soak == 0 {
                continue;
            }
            let Some(tub) = tub_of(hero) else {
                hero.soak = 0;
                continue;
            };
            if kind != RoomKind::Bathhouse || hero.hp == 0 {
                hero.soak = 0;
                continue;
            }
            let (x, y) = TUBS[tub];
            (hero.x, hero.y) = (x * TILE_UNITS, y * TILE_UNITS);
            hero.soak -= 1;
            if hero.soak == 0 {
                hero.scrubbed = true;
                hero.y = (y + 1.6) * TILE_UNITS;
                done.push(id);
            }
        }
        for id in done {
            self.sounds.push("wish_grant");
            self.found = Some((tick, id, "SQUEAKY CLEAN!".into()));
        }
    }
}

/// The tub a knight stands in, if any.
pub fn tub_of(hero: &Hero) -> Option<usize> {
    TUBS.iter()
        .position(|&(x, y)| (hero.x - x * TILE_UNITS).hypot(hero.y - y * TILE_UNITS) < 0.5)
}

#[cfg(test)]
#[path = "../../tests/rules/together_town__tests.rs"]
mod tests;
