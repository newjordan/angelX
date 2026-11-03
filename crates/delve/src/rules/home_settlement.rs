//! Pure home/treasury settlement policy owned by the game implementation.
//! The host persists an identified result, then the game applies its deliveries.
use crate::rules as game;
use super::{Run, home, merchant, cards};
use crate::realm::Spoils;
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotedPurchase {
    pub purchase: merchant::Purchase,
    pub gold: u32,
    pub guard: String,
    pub goods: Option<merchant::Goods>,
    pub card: Option<cards::Card>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squire: Option<game::squire_company::company::PreparedPurchase>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeTransfer {
    pub orders: Vec<home::Order>,
    #[serde(default)]
    pub lane_squire_orders: Vec<game::lane_squires::Order>,
    #[serde(default)]
    pub raise_orders: Vec<game::raisings::Order>,
    /// Exact source prefix, including requests that could not be quoted.
    pub raw_purchases: Vec<merchant::Purchase>,
    pub purchases: Vec<QuotedPurchase>,
    pub noticed: Vec<String>,
    pub marks: BTreeMap<String, u32>,
    pub stall: Vec<String>,
    pub stall_prices: Vec<u32>,
    pub stall_names: Vec<String>,
    pub stall_cards: Vec<Option<cards::Card>>,
    pub reserved: Vec<usize>,
    pub depth: u32,
    pub reach: u32,
    pub view: home::Home,
    pub fighting: bool,
    pub names: BTreeMap<u32, String>,
    pub keys: BTreeMap<u32, String>,
    /// Each seat's own record (`talents::profile_key`), which `keys` keeps by kit.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<u32, String>,
    pub location: [u64; 3],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub company_reserved: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeClaim {
    pub session: String,
    pub raid: u64,
    pub request: u64,
    pub transfer: HomeTransfer,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeCompletion {
    pub result: HomeResult,
    pub delivered: HomeDelivered,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeResult {
    pub home: home::Home,
    pub treasury: Spoils,
    pub reclaimed: BTreeMap<String, u32>,
    pub raids_won: u32,
    pub sales: Vec<(usize, u32)>,
    pub purchases: Vec<usize>,
    pub announced: Vec<String>,
    pub unboxed: Option<(game::feats::Tier, Spoils)>,
    pub said: Vec<(String, String)>,
    pub changed: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeDelivered {
    /// Stable indexes into the committed result's sale/purchase vectors.
    pub refunded_sales: Vec<usize>,
    pub refunded_purchases: Vec<usize>,
    pub refund: Spoils,
}
/// The last consumed game-local observations. Global home projection changes
/// alone must not create an endless cycle of empty settlement claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeObserved {
    view: home::Home,
    depth: u32,
    reach: u32,
    names: BTreeMap<u32, String>,
    keys: BTreeMap<u32, String>,
    context: HomeContext,
}
/// Economy inputs that can change without another game event (for example a
/// newly banked raid growing a foal). Comparing the last committed context
/// avoids both skipped derived work and endless empty settlement claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeContext {
    pub home: home::Home,
    pub treasury: Spoils,
    pub reclaimed: BTreeMap<String, u32>,
    pub raids_won: u32,
}

pub const POLICY_CONTRACT: &str = "delve.home";
pub const POLICY_API: u32 = 1;
pub const MAX_POLICY_RESULT_BYTES: usize = 512 * 1024;

/// Version 1 of delve.home. All inputs are copies; the worker cannot access the
/// realm writer or the mutable game. Only the host's identified transaction may
/// publish a validated result. Retried prepared claims never run policy again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomePolicyRequest {
    pub context: HomeContext,
    pub transfer: HomeTransfer,
    pub host: String,
    pub unsaved: bool,
}
impl HomePolicyRequest {
    pub fn validate(&self) -> Result<(), String> {
        if !self.transfer.valid()
            || self.host.len() > 1024
            || self.context.reclaimed.len() > 64
            || self.context.reclaimed.keys().any(|key| key.len() > 1024)
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 1024 * 1024
        {
            return Err("invalid home policy inputs".into());
        }
        Ok(())
    }
    pub fn validate_result(&self, result: &HomeResult) -> Result<(), String> {
        if !result.valid_for(&self.transfer)
            || result.reclaimed != self.context.reclaimed
            || result.raids_won != self.context.raids_won
            || serde_json::to_vec(result).map_err(|e| e.to_string())?.len()
                > MAX_POLICY_RESULT_BYTES
        {
            return Err("home policy result violates its game contract".into());
        }
        Ok(())
    }
    pub fn evaluate(&self) -> Result<HomeResult, String> {
        self.validate()?;
        let mut context = self.context.clone();
        let result = settle(
            &mut game::activity::Realm {
                home: &mut context.home,
                treasury: &mut context.treasury,
                reclaimed: &context.reclaimed,
                raids_won: context.raids_won,
            },
            &self.transfer,
            &self.host,
            self.unsaved,
        )?;
        self.validate_result(&result)?;
        Ok(result)
    }
}

/// Private checkpoint of facts still visible to gameplay. Public Run saves
/// intentionally omit these queues; a live session must retain all of them,
/// including facts added while an older claim awaits persistence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeOutbox {
    pub orders: Vec<home::Order>,
    #[serde(default)]
    pub lane_squire_orders: Vec<game::lane_squires::Order>,
    #[serde(default)]
    pub raise_orders: Vec<game::raisings::Order>,
    pub peddled: Vec<merchant::Purchase>,
    pub noticed: Vec<String>,
    pub marks: BTreeMap<String, u32>,
    #[serde(default)]
    pub squire_company_reserved: bool,
    #[serde(default)]
    pub squire_company_claimed: bool,
    #[serde(default)]
    pub squire_pending_delves: u32,
}
impl HomeOutbox {
    pub fn capture(run: &Run) -> Result<Self, String> {
        let outbox = Self {
            orders: run.orders.clone(),
            lane_squire_orders: run.lane_squire_orders.clone(),
            raise_orders: run.raise_orders.clone(),
            peddled: run.peddled.clone(),
            noticed: run.feats.iter().map(|id| (*id).to_owned()).collect(),
            marks: run.marks.clone(),
            squire_company_reserved: run.squire_company_reserved,
            squire_company_claimed: run.squire_company_claimed,
            squire_pending_delves: run.squire_pending_delves,
        };
        outbox.validate()?;
        Ok(outbox)
    }
    pub fn validate(&self) -> Result<(), String> {
        if (!self.squire_company_reserved && !self.squire_company_claimed && self.squire_pending_delves != 0)
            || self.orders.len() > 4096
            || self.lane_squire_orders.len() > 4096
            || self.raise_orders.len() > 4096
            || self.peddled.len() > 4096
            || self.noticed.len() > 4096
            || self.marks.len() > 2048
            || !self
                .marks
                .iter()
                .all(|(key, value)| valid_mark(key, *value))
            || !self
                .noticed
                .iter()
                .all(|id| game::feats::feat(id).is_some())
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 1024 * 1024
        {
            return Err("invalid private home outbox".into());
        }
        Ok(())
    }
    pub fn install(self, run: &mut Run) -> Result<(), String> {
        self.validate()?;
        let feats = self
            .noticed
            .iter()
            .map(|id| game::feats::feat(id).unwrap().id)
            .collect();
        run.orders = self.orders;
        run.lane_squire_orders = self.lane_squire_orders;
        run.raise_orders = self.raise_orders;
        run.peddled = self.peddled;
        run.feats = feats;
        run.marks = self.marks;
        run.squire_company_reserved = self.squire_company_reserved;
        run.squire_company_claimed = self.squire_company_claimed;
        run.squire_pending_delves = self.squire_pending_delves;
        Ok(())
    }
}
fn valid_mark(key: &str, value: u32) -> bool {
    if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
        return false;
    }
    if game::countryside::intent_key(key) {
        return game::countryside::intent_valid(key, value);
    }
    if key.starts_with("country:sell:") {
        return game::town::STALLS.iter().enumerate().any(|(slot, stall)| {
            (stall.base..=stall.top).contains(&value)
                && (1..=game::MAX_PLAYERS).any(|id| key == game::town::sale_mark(slot, id))
        });
    }
    !key.starts_with("country:") && !key.starts_with("college:")
}
fn state_mark(key: &str) -> bool {
    game::countryside::intent_key(key) || key.starts_with("country:sell:")
}
impl HomeObserved {
    pub(super) fn matches_company(&self, company: &super::squire_company::Company) -> bool {
        &self.context.home.squire_company == company
    }
    pub fn matches_context(&self, context: &HomeContext) -> bool {
        &self.context == context
    }
    pub fn matches(&self, run: &Run) -> bool {
        self.view.squire_company == run.home.squire_company
            && self.view.squires_lodge == run.home.squires_lodge
            && self.view.wardrobe == run.home.wardrobe
            && self.view.squire_academy == run.home.squire_academy
            && self.view.privy_wall == run.home.privy_wall
            && self.view.salvage == run.home.salvage
            && self.view.wished == run.home.wished
            && self.view.sky == run.home.sky
            && self.view.deep == run.home.deep
            && self.view.countryside.games == run.home.countryside.games
            && self.view.landing == run.home.landing
            && self.depth == run.dungeon.depth
            && self.reach == run.record_floor()
            && self.names.len() == run.players.len()
            && run.players.iter().all(|(id, hero)| {
                self.names.get(id) == Some(&hero.name)
                    && self.keys.get(id) == Some(&game::talents::knight_key(hero))
            })
    }
    pub fn view(&self) -> &home::Home {
        &self.view
    }
}
impl HomeTransfer {
    pub fn observed(&self, result: &HomeResult) -> HomeObserved {
        let mut view = self.view.clone();
        if self.company_reserved {
            view.squire_company = result.home.squire_company.clone();
        }
        HomeObserved {
            view,
            depth: self.depth,
            reach: self.reach,
            names: self.names.clone(),
            keys: self.keys.clone(),
            context: result.context(),
        }
    }
    pub fn capture_and_reserve(run: &mut Run) -> Result<Self, String> {
        if run.squire_company_reserved || run.squire_company_claimed {
            return Err("squire company already reserved".into());
        }
        if run.marks.len() > 2048 || run.stall.len() > 64 {
            return Err("home facts exceed the transaction contract".into());
        }
        let raw_purchases: Vec<_> = run.peddled.iter().take(64).copied().collect();
        let purchases = raw_purchases
            .iter()
            .copied()
            .filter_map(|purchase| quote(run, purchase))
            .collect();
        let orders: Vec<_> = run.orders.iter().take(64).copied().collect();
        let reserved: Vec<_> = orders
            .iter()
            .filter_map(|order| stall_slot(order.station))
            .filter(|slot| run.stall.get(*slot).is_some_and(|card| !card.is_empty()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let transfer = Self {
            orders,
            lane_squire_orders: run.lane_squire_orders.iter().take(64).cloned().collect(),
            raise_orders: run.raise_orders.iter().take(64).cloned().collect(),
            raw_purchases,
            purchases,
            noticed: run
                .feats
                .iter()
                .take(64)
                .map(|id| (*id).to_owned())
                .collect(),
            marks: run.marks.clone(),
            stall: run.stall.clone(),
            stall_prices: run
                .stall
                .iter()
                .map(|id| run.book.get(id).map_or(0, game::yard::stall_price))
                .collect(),
            stall_names: run
                .stall
                .iter()
                .map(|id| {
                    run.book
                        .get(id)
                        .map_or(id.clone(), |card| card.name.clone())
                })
                .collect(),
            stall_cards: run
                .stall
                .iter()
                .map(|id| run.book.get(id).cloned())
                .collect(),
            reserved,
            depth: run.dungeon.depth,
            reach: run.record_floor(),
            view: run.home.clone(),
            fighting: run.phase == game::Phase::Fighting,
            names: run
                .players
                .iter()
                .map(|(id, hero)| (*id, hero.name.clone()))
                .collect(),
            keys: run
                .players
                .iter()
                .map(|(id, hero)| (*id, game::talents::knight_key(hero)))
                .collect(),
            profiles: run
                .players
                .iter()
                .map(|(id, hero)| (*id, game::talents::profile_key(hero)))
                .collect(),
            location: [run.raid_id, run.dungeon.depth.into(), run.at as u64],
            company_reserved: true,
        };
        run.squire_company_claimed = true;
        // A reserved stall slot is empty but the stock vector stays present,
        // so ordinary home rebuilds cannot reroll or sell it a second time.
        for slot in &transfer.reserved {
            run.stall[*slot].clear();
        }
        Ok(transfer)
    }
    /// Unpaid facts remain part of gameplay. A replacement must retain their
    /// FIFO prefixes and monotonic counters; a newer state intent may replace
    /// an older value, and will be consumed by the following transaction.
    pub fn validate_world(&self, run: &Run) -> Result<(), String> {
        if (self.company_reserved && (!run.squire_company_claimed || run.home.squire_company != self.view.squire_company))
            || !run.orders.starts_with(&self.orders)
            || !run.lane_squire_orders.starts_with(&self.lane_squire_orders)
            || !run.raise_orders.starts_with(&self.raise_orders)
            || !run.peddled.starts_with(&self.raw_purchases)
            || run.feats.len() < self.noticed.len()
            || !run
                .feats
                .iter()
                .zip(&self.noticed)
                .all(|(live, claimed)| *live == claimed)
            || !self.marks.iter().all(|(key, captured)| {
                // The claim itself owns this absolute clock. A restored outbox
                // may omit it or contain an older clock; settling the claim
                // advances the guild to the captured time regardless.
                key == "squires:clock" || run.marks.get(key).is_some_and(|live| {
                    valid_mark(key, *live) && (state_mark(key) || live >= captured)
                })
            })
        {
            return Err("pending home facts changed or disappeared from the game".into());
        }
        Ok(())
    }
    pub(super) fn consume(&self, run: &mut Run) -> Result<(), String> {
        self.validate_world(run)?;
        run.orders.drain(..self.orders.len());
        run.lane_squire_orders.drain(..self.lane_squire_orders.len());
        run.raise_orders.drain(..self.raise_orders.len());
        run.peddled.drain(..self.raw_purchases.len());
        run.feats.drain(..self.noticed.len());
        for (key, captured) in &self.marks {
            if key == "squires:clock" {
                if run.marks.get(key).is_some_and(|live| live <= captured) {
                    run.marks.remove(key);
                }
                continue;
            }
            let live = run.marks.get_mut(key).unwrap();
            if state_mark(key) {
                if live == captured {
                    run.marks.remove(key);
                }
            } else {
                *live -= captured;
                if *live == 0 {
                    run.marks.remove(key);
                }
            }
        }
        Ok(())
    }
    pub fn valid(&self) -> bool {
        self.orders.len() <= 64
            && self.lane_squire_orders.len() <= 64
            && self.raise_orders.len() <= 64
            && self.raw_purchases.len() <= 64
            && self.purchases.len() <= 64
            && {
                let mut raw = self.raw_purchases.iter();
                self.purchases
                    .iter()
                    .all(|quoted| raw.any(|p| *p == quoted.purchase))
            }
            && self.noticed.len() <= 64
            && self.marks.len() <= 2048
            && self
                .marks
                .iter()
                .all(|(key, value)| valid_mark(key, *value))
            && self
                .noticed
                .iter()
                .all(|id| game::feats::feat(id).is_some())
            && self.stall.len() <= 64
            && self.stall.len() == self.stall_prices.len()
            && self.stall.len() == self.stall_names.len()
            && self.stall.len() == self.stall_cards.len()
            && self.reserved.len() <= 3
            && self
                .reserved
                .iter()
                .all(|slot| *slot < 3 && *slot < self.stall.len())
            && self
                .reserved
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.reserved.len()
            && self.names.len() <= game::MAX_PLAYERS as usize
            && self.keys.len() <= game::MAX_PLAYERS as usize
            && self.profiles.len() <= game::MAX_PLAYERS as usize
            && self.purchases.iter().all(|p| {
                p.guard.len() <= 4096 && match (&p.squire, p.purchase.shop) {
                    (Some(prepared), merchant::Shop::Squires) => self.company_reserved
                        && self.names.get(&p.purchase.knight).is_some_and(|name| prepared.matches(p.purchase, name))
                        && prepared.price(&self.view.squire_company) == Some(p.gold),
                    (None, merchant::Shop::Squires) => false,
                    (None, _) => true,
                    (Some(_), _) => false,
                }
            })
    }
}
fn quote(run: &Run, purchase: merchant::Purchase) -> Option<QuotedPurchase> {
    let gold = run.peddle_price(purchase)?;
    let hero = run.players.get(&purchase.knight)?;
    let guard = if purchase.shop == merchant::Shop::Peddler {
        format!("{}|{}", hero.name, game::talents::knight_key(hero))
    } else {
        format!(
            "{:?}|{}|{}|{}|{}|{:?}",
            [run.raid_id, run.dungeon.depth.into(), run.at as u64],
            hero.name,
            game::talents::knight_key(hero),
            hero.weapon.name(),
            hero.temper,
            hero.owned
        )
    };
    let goods = if purchase.shop == merchant::Shop::Peddler {
        Some(run.merchant_camp()?.offers.get(purchase.offer)?.goods)
    } else {
        None
    };
    let card = match goods {
        Some(merchant::Goods::Potion) => Some(run.book.get("potion")?.clone()),
        Some(merchant::Goods::Scroll) => Some(run.book.get("thunder-scroll")?.clone()),
        _ => None,
    };
    Some(QuotedPurchase {
        purchase,
        gold,
        guard,
        goods,
        card,
        squire: if purchase.shop == merchant::Shop::Squires {
            Some(run.prepare_squire_purchase(purchase)?)
        } else {
            None
        },
    })
}
/// Evaluate captured facts; no file access, UI callbacks or mutable host Run.
/// The caller commits this result with the economy before delivering it.
pub fn settle(
    realm: &mut game::activity::Realm<'_>,
    transfer: &HomeTransfer,
    host: &str,
    unsaved: bool,
) -> Result<HomeResult, String> {
    if !transfer.valid() {
        return Err("invalid home transaction facts".into());
    }
    let host = host.to_owned();
    let orders = &transfer.orders;
    let noticed = &transfer.noticed;
    let marks = &transfer.marks;
    let stall = &transfer.stall;
    let stall_prices = &transfer.stall_prices;
    let stall_names = &transfer.stall_names;
    let depth = transfer.depth;
    let reach = transfer.reach;
    let landing = transfer.view.landing;
    let country_games = transfer.view.countryside.games.clone();
    let wardrobe = transfer.view.wardrobe.clone();
    let privy_wall = transfer.view.privy_wall.clone();
    let wished = transfer.view.wished.clone();
    let sky = transfer.view.sky.clone();
    let fighting = transfer.fighting;
    let names = &transfer.names;
    let keys = &transfer.keys;
    let profiles = &transfer.profiles;
    let mut changed = wardrobe != realm.home.wardrobe;
    for (id, profile) in profiles {
        if let Some(kit) = keys.get(id) {
            changed |= realm.home.adopt(kit, profile);
        }
    }
    changed |= game::squires_town::settle(
        &mut realm.home.squires_lodge, realm.treasury, &transfer.view.squires_lodge, marks,
    ).map_err(|e| format!("Squires settlement: {e:?}"))?;
    changed |= realm.home.lane_squires.settle(&mut realm.treasury, &transfer.lane_squire_orders);
    realm.home.wardrobe = wardrobe;
    if transfer.company_reserved && realm.home.squire_company != transfer.view.squire_company {
        realm.home.squire_company = transfer.view.squire_company.clone();
        changed = true;
    }
    changed |= game::squire_academy::settle(&mut realm.home.squire_academy, realm.treasury, &transfer.view.squire_academy);
    if realm.home.privy_wall != privy_wall {
        realm.home.privy_wall = privy_wall;
        changed = true;
    }
    // The salvage yard changes in the run (a find, a wish, a build): the
    // realm keeps it when the run's is newer (`salvage`).
    if transfer.view.salvage.rev > realm.home.salvage.rev {
        realm.home.salvage = transfer.view.salvage.clone();
        changed = true;
    }
    // The freedom wishes' places, and the wishes held unspent (`freedom`):
    // only the game writes them, as the privy wall.
    if realm.home.wished != wished {
        realm.home.wished = wished;
        changed = true;
    }
    // The realm's sky (`rewards`): only the game writes it.
    if realm.home.sky != sky && sky.valid() {
        realm.home.sky = sky;
        changed = true;
    }
    // The Deep's record is the run's own to write, as the privy wall is:
    // villages met and lit, errands asked and done, wishes made (`deep`).
    if realm.home.deep != transfer.view.deep && transfer.view.deep.valid() {
        realm.home.deep = transfer.view.deep.clone();
        changed = true;
    }
    changed |= realm.home.countryside.games.merge(&country_games);
    let (country_changed, mut country_news) =
        game::countryside::settle(&mut realm.home.countryside, &mut realm.treasury, &marks);
    changed |= country_changed;
    let (games_changed, games_news) =
        game::countryside::settle_games(&mut realm.home.countryside, &mut realm.treasury, &marks);
    changed |= games_changed;
    country_news.extend(games_news);
    let (college_changed, college_news) =
        game::college::settle(&mut realm.home.college, &mut realm.treasury, &marks);
    changed |= college_changed;
    if reach > realm.home.deepest {
        realm.home.deepest = reach;
        changed = true;
    }
    if depth == 0 && landing != realm.home.landing {
        realm.home.landing = landing;
        changed = true;
    }
    let (raised_changed, raised_news) =
        game::raisings::settle(&mut realm.home.raised, realm.treasury, &transfer.raise_orders);
    changed |= raised_changed;
    country_news.extend(raised_news);
    let mut said: Vec<(String, String)> = country_news
        .into_iter()
        .map(|line| ("country".into(), line))
        .collect();
    let mut earned: Vec<&'static str> = Vec::new();
    // Wren's bounties: what the delve just did. A finished one is paid
    // on the spot, and Wren pins the next. Consumed progress commits with
    // its source claim before acknowledgement.
    let board = realm.home.bounties.clone();
    let finished = realm.home.work_bounties(&marks, reach);
    let mut paid = Vec::new();
    for bounty in &finished {
        let reward = bounty.reward();
        realm.treasury.merge(&reward);
        changed = true;
        paid.push(format!("{}, for {}", bounty.title, reward.label()));
    }
    let pinned = realm.home.pin_bounties();
    if !pinned.is_empty() {
        changed = true;
    }
    // The holdings' raids, pinned beside the bounties: Wren's alarms,
    // pulling the party back down. They pay nothing — the raiders'
    // purse is won in the hall.
    let raid_pinned = game::bounties::note_raids(&mut realm.home);
    if !raid_pinned.is_empty() {
        changed = true;
        said.extend(
            raid_pinned
                .into_iter()
                .map(|line| ("raid_pinned".to_string(), line)),
        );
    }
    if !paid.is_empty() {
        let next = pinned.last().map_or(String::new(), |b| {
            format!(" Next on her board: {}.", b.title)
        });
        said.push((
            "bounty_paid".into(),
            format!("Wren's bounty done: {}.{next}", paid.join("; ")),
        ));
    } else if let Some(bounty) = pinned.last() {
        said.push((
            "bounty_pinned".into(),
            format!("Wren pins a bounty: {}. \"{}\"", bounty.title, bounty.says),
        ));
    }
    // The Trophy Hall: what fell, kept by the realm. Sir Kay says so the
    // first time a piece comes in.
    for (mark, &n) in marks {
        let Some(id) = mark.strip_prefix("trophy:") else {
            continue;
        };
        let count = realm.home.trophies.entry(id.to_string()).or_default();
        let first = *count == 0;
        *count += n;
        changed = true;
        if first && let Some(plinth) = game::trophies::PLINTHS.iter().find(|p| p.id == id) {
            said.push((
                "trophy_new".into(),
                format!(
                    "Sir Kay has a new piece for the Trophy Hall: {}",
                    plinth.name
                ),
            ));
        }
    }
    if game::trophies::PLINTHS
        .iter()
        .all(|p| realm.home.trophies.contains_key(p.id))
    {
        earned.push("full_hall");
    }
    // Rescues: whoever was let out of a cage goes home up the stair, and
    // stays.
    for mark in marks.keys() {
        let Some(who) = mark
            .strip_prefix("rescue:")
            .and_then(game::rescues::resident)
        else {
            continue;
        };
        if realm.home.residents.insert(who.id.to_string()) {
            changed = true;
            said.push((
                "rescue_home".into(),
                format!(
                    "{} is going home to the Undercroft, with {}",
                    who.name, who.gives
                ),
            ));
        }
    }
    if game::rescues::RESIDENTS
        .iter()
        .all(|r| realm.home.residents.contains(r.id))
    {
        earned.push("full_house");
    }
    // Maud's round was drunk at the top of the stair: the realm's tab
    // is clear for the next.
    if marks.contains_key("round_drunk") && realm.home.round.take().is_some() {
        changed = true;
    }
    // And Sir Dinadan's song went down with them.
    if marks.contains_key("song_sung") && realm.home.song.take().is_some() {
        changed = true;
    }
    // And Beaumains, with his wage in his pocket.
    if marks.contains_key("hired") && realm.home.hire.take().is_some() {
        changed = true;
    }
    // King Brannoc's barony: his coming, the works, the forges relit, his
    // missions.
    let mines = realm
        .reclaimed
        .get(game::Pack::Cavern.name())
        .copied()
        .unwrap_or(0);
    let who = |id: u32| match names.get(&id).map(String::as_str) {
        Some("You") | None if id == 1 => host.clone(),
        Some(name) => name.to_string(),
        None => format!("Knight {id}"),
    };
    let mut treasury = realm.treasury.clone();
    let barony = game::barony::settle(&mut realm.home, &mut treasury, mines, &marks, &who);
    *realm.treasury = treasury;
    if barony.changed {
        changed = true;
    }
    said.extend(barony.said);
    said.extend(
        college_news
            .into_iter()
            .map(|line| ("college".into(), line)),
    );
    earned.extend(barony.earned);
    // The games and places: the lists and their stables and market, the
    // Hippodrome's purses, the Court of Deeds, the ranch
    // (`activity::settle_all`).
    let games = game::activity::settle_all(
        &mut game::activity::Realm {
            home: &mut realm.home,
            treasury: &mut realm.treasury,
            reclaimed: &realm.reclaimed,
            raids_won: realm.raids_won,
        },
        &marks,
    );
    changed |= games.changed;
    said.extend(games.said);
    // Experience: every knight in the party learns from what fell. A
    // new level is a lesson waiting with Sir Ector.
    let xp = marks.get("xp").copied().unwrap_or(0);
    if xp > 0 {
        for (id, key) in keys {
            let key = profiles.get(id).unwrap_or(key);
            let prowess = realm.home.knights.entry(key.clone()).or_default();
            let before = prowess.level();
            prowess.xp += xp;
            if prowess.level() > before {
                let who = names.get(id).cloned().unwrap_or_else(|| key.clone());
                said.push((
                        "level_up".into(),
                        format!(
                            "{who} reached level {}: Sir Ector has a lesson waiting in the Training Yard",
                            prowess.level()
                        ),
                    ));
            }
        }
    }
    // The Herald's Bestiary counts every kill.
    let mut counted = false;
    for (mark, &n) in marks {
        if let Some(kind) = mark.strip_prefix("slay:") {
            *realm.home.bestiary.entry(kind.to_string()).or_default() += n;
            counted = true;
        }
    }
    let unsaved = unsaved || realm.home.bounties != board || xp > 0 || counted;
    if unsaved && !fighting {
        changed = true;
    }
    // Every goblin that got away; the first brings Grubbins home.
    let escapes = noticed
        .iter()
        .filter(|f| f.as_str() == "sticky_fingers")
        .count() as u32;
    if escapes > 0 {
        realm.home.goblins += escapes;
        changed = true;
    }
    // The surface peddler is paid from the treasury, as Grubbins is.
    let mut peddles = Vec::new();
    for (index, quote) in transfer.purchases.iter().enumerate() {
        let gold = quote.gold;
        if quote.purchase.shop == merchant::Shop::Squires
            && quote.squire.as_ref().and_then(|p| p.price(&realm.home.squire_company)) != Some(gold) {
            continue;
        }
        let mut cost = crate::realm::Spoils::default();
        cost.add(crate::realm::Spoil::Gold, gold);
        if !realm.treasury.covers(&cost) {
            said.push((
                "cant_afford".into(),
                format!(
                    "That's {gold} gold, and the treasury holds {}.",
                    realm.treasury.get(crate::realm::Spoil::Gold)
                ),
            ));
            continue;
        }
        if let Some(prepared) = &quote.squire {
            if prepared.apply(&mut realm.home.squire_company) != Some(gold) {
                continue;
            }
        } else {
            peddles.push(index);
        }
        realm.treasury.take(&cost);
        changed = true;
    }
    let mut sales: Vec<(usize, u32)> = Vec::new();
    let mut unboxed: Option<(game::feats::Tier, crate::realm::Spoils)> = None;
    for order in orders {
        use game::home::Station;
        if let Some(item) = match order.station {
            Station::StallA => Some(0),
            Station::StallB => Some(1),
            Station::StallC => Some(2),
            _ => None,
        } {
            // Grubbins sells for gold, and only gold.
            if stall.get(item).is_none_or(|c| c.is_empty()) || transfer.stall_cards[item].is_none()
            {
                continue;
            }
            // One stock slot cannot be charged twice by simultaneous plates.
            if sales.iter().any(|(slot, _)| *slot == item) {
                continue;
            }
            let price = stall_prices[item];
            let mut cost = crate::realm::Spoils::default();
            cost.add(crate::realm::Spoil::Gold, price);
            if !realm.treasury.covers(&cost) {
                said.push((
                    "cant_afford".into(),
                    format!(
                        "Grubbins: that's {price} gold, friend. You've got {}.",
                        realm.treasury.get(crate::realm::Spoil::Gold)
                    ),
                ));
                continue;
            }
            realm.treasury.take(&cost);
            changed = true;
            sales.push((item, order.knight));
            said.push((
                "grubbins_sold".into(),
                format!("Grubbins sold you {} for {price} gold", stall_names[item]),
            ));
            continue;
        }
        if let Some(side) = match order.station {
            game::home::Station::LessonA => Some(0u8),
            game::home::Station::LessonB => Some(1),
            _ => None,
        } {
            // Sir Ector teaches the knight standing at his lectern.
            let Some(key) = profiles.get(&order.knight).or_else(|| keys.get(&order.knight)) else {
                continue;
            };
            match realm.home.learn(key, side) {
                Ok(talent) => {
                    changed = true;
                    earned.push("first_lesson");
                    if realm.home.prowess(key).learned.len() == game::talents::LESSONS.len() {
                        earned.push("master_of_arms");
                    }
                    let who = names
                        .get(&order.knight)
                        .cloned()
                        .unwrap_or_else(|| key.clone());
                    said.push((
                        "lesson_learned".into(),
                        format!(
                            "{who} learned {} from Sir Ector ({}): \"{}\"",
                            talent.name, talent.does, talent.says
                        ),
                    ));
                }
                Err(why) => said.push(("cant_afford".into(), why)),
            }
            continue;
        }
        if let Some(tap) = match order.station {
            game::home::Station::TapA => Some(0),
            game::home::Station::TapB => Some(1),
            game::home::Station::TapC => Some(2),
            _ => None,
        } {
            // Maud pours a round for the next delve: gold from the
            // treasury, one round at a time.
            let drink = &game::tavern::DRINKS[tap];
            if let Some(waiting) = realm.home.round.as_deref().and_then(game::tavern::drink) {
                said.push((
                    "cant_afford".into(),
                    format!(
                        "Maud: you've a {} waiting already, love. Drink that first.",
                        waiting.name
                    ),
                ));
                continue;
            }
            let mut cost = crate::realm::Spoils::default();
            cost.add(crate::realm::Spoil::Gold, drink.price);
            if !realm.treasury.covers(&cost) {
                said.push((
                    "cant_afford".into(),
                    format!("Maud: that's {} gold, love. The tab's closed.", drink.price),
                ));
                continue;
            }
            realm.treasury.take(&cost);
            realm.home.round = Some(drink.id.to_string());
            changed = true;
            said.push((
                format!("round_poured:{}", drink.id),
                format!(
                    "Maud poured {} for the next delve ({} gold): {}",
                    drink.name, drink.price, drink.does
                ),
            ));
            continue;
        }
        if let Some(plate) = match order.station {
            game::home::Station::SongA => Some(0),
            game::home::Station::SongB => Some(1),
            game::home::Station::SongC => Some(2),
            _ => None,
        } {
            // Sir Dinadan sings for the next delve: gold from the
            // treasury, one song at a time.
            let song = &game::tavern::SONGS[plate];
            if let Some(waiting) = realm.home.song.as_deref().and_then(game::tavern::song) {
                said.push((
                    "cant_afford".into(),
                    format!(
                        "Sir Dinadan: I've {} rehearsed already. One song at a time, friend.",
                        waiting.name
                    ),
                ));
                continue;
            }
            let mut cost = crate::realm::Spoils::default();
            cost.add(crate::realm::Spoil::Gold, song.price);
            if !realm.treasury.covers(&cost) {
                said.push((
                    "cant_afford".into(),
                    format!(
                        "Sir Dinadan: {} gold, friend. Art isn't free. Mostly.",
                        song.price
                    ),
                ));
                continue;
            }
            realm.treasury.take(&cost);
            realm.home.song = Some(song.id.to_string());
            changed = true;
            said.push((
                format!("song_asked:{}", song.id),
                format!(
                    "Sir Dinadan will sing {} for the next delve ({} gold): {}",
                    song.name, song.price, song.does
                ),
            ));
            continue;
        }
        if order.station == game::home::Station::Hire {
            // Beaumains takes a wage for the next delve: gold from the
            // treasury, once.
            if realm.home.hire.is_some() {
                said.push((
                    "cant_afford".into(),
                    "Beaumains: I'm yours for the next one already, my lord. I'll be at the stair."
                        .into(),
                ));
                continue;
            }
            let wage = game::hireling::WAGE;
            let mut cost = crate::realm::Spoils::default();
            cost.add(crate::realm::Spoil::Gold, wage);
            if !realm.treasury.covers(&cost) {
                said.push((
                    "cant_afford".into(),
                    format!(
                        "Beaumains: {wage} gold, my lord, and a hot meal. I'll throw in the meal."
                    ),
                ));
                continue;
            }
            realm.treasury.take(&cost);
            realm.home.hire = Some("beaumains".into());
            changed = true;
            said.push((
                    "hire_asked:beaumains".into(),
                    format!(
                        "Beaumains will go down with you for the next delve ({wage} gold): he throws knives and keeps his distance"
                    ),
                ));
            continue;
        }
        if order.station == game::home::Station::Coffer {
            // A box opened at the Herald's coffer.
            if realm.home.boxes.is_empty() {
                said.push((
                    "cant_afford".into(),
                    "The coffer is empty: achievements fill it.".into(),
                ));
                continue;
            }
            let tier = realm.home.boxes.remove(0);
            let spoils = tier.contents(realm.home.opened);
            realm.home.opened += 1;
            realm.treasury.merge(&spoils);
            changed = true;
            said.push((
                format!("box_opened:{}", game::feats::tier_word(tier)),
                format!("The {} held {}", tier.name(), spoils.label()),
            ));
            unboxed = Some((tier, spoils));
            continue;
        }
        let ladder = order.station.ladder();
        let who = match names.get(&order.knight).map(String::as_str) {
            Some("You") | None if order.knight == 1 => host.clone(),
            Some(name) => name.to_string(),
            None => format!("Knight {}", order.knight),
        };
        let mut treasury = realm.treasury.clone();
        match realm.home.buy(order.station, &mut treasury) {
            Ok(level) => {
                *realm.treasury = treasury;
                changed = true;
                earned.push("home_improvement");
                said.push((
                    format!("built:{}", order.station.word()),
                    format!(
                        "{who} built {} {}: {}",
                        ladder.name,
                        game::home::numeral(level),
                        ladder.rungs[usize::from(level) - 1].says
                    ),
                ));
            }
            Err(why) => said.push(("cant_afford".into(), why)),
        }
    }
    // Achievements: each kept once, each with its box.
    let mut announced = Vec::new();
    for id in noticed
        .iter()
        .map(String::as_str)
        .chain(earned.iter().copied())
    {
        if let Some(feat) = game::feats::feat(id)
            && realm.home.feats.insert(id.to_string())
        {
            realm.home.boxes.push(feat.tier);
            changed = true;
            announced.push(feat.id.to_owned());
        }
    }

    // Every consumed fact must be durable before source acknowledgement, even
    // when the legacy host deferred combat XP/bestiary writes until a safe room.
    changed |= unsaved;
    Ok(HomeResult {
        home: realm.home.clone(),
        treasury: realm.treasury.clone(),
        reclaimed: (*realm.reclaimed).clone(),
        raids_won: realm.raids_won,
        sales,
        purchases: peddles,
        announced,
        unboxed,
        said,
        changed,
    })
}

fn stall_slot(station: home::Station) -> Option<usize> {
    match station {
        home::Station::StallA => Some(0),
        home::Station::StallB => Some(1),
        home::Station::StallC => Some(2),
        _ => None,
    }
}
impl HomeResult {
    pub fn context(&self) -> HomeContext {
        HomeContext {
            home: self.home.clone(),
            treasury: self.treasury.clone(),
            reclaimed: self.reclaimed.clone(),
            raids_won: self.raids_won,
        }
    }
    pub fn valid_for(&self, transfer: &HomeTransfer) -> bool {
        transfer.valid()
            && self.reclaimed.len() <= 64
            && self.reclaimed.keys().all(|key| key.len() <= 1024)
            && self.sales.len() <= 3
            && self.purchases.len() <= transfer.purchases.len()
            && self
                .sales
                .iter()
                .map(|(slot, _)| slot)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.sales.len()
            && self.sales.iter().all(|(slot, knight)| {
                transfer.reserved.contains(slot)
                    && transfer.orders.iter().any(|order| {
                        order.knight == *knight && stall_slot(order.station) == Some(*slot)
                    })
            })
            && self
                .purchases
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.purchases.len()
            && self
                .purchases
                .iter()
                .all(|index| *index < transfer.purchases.len())
            && self.announced.len() <= 256
            && self
                .announced
                .iter()
                .all(|id| game::feats::feat(id).is_some())
            && self.said.len() <= 1024
            && self
                .said
                .iter()
                .all(|(cue, line)| cue.len() <= 1024 && line.len() <= 8192)
    }
    /// Apply an already committed result to a prepared candidate Run. The
    /// journal commits that candidate and this delivery receipt together.
    /// Undeliverable goods become an identified refund, never a second charge.
    pub(super) fn deliver(
        &self,
        run: &mut Run,
        transfer: &HomeTransfer,
        key: &str,
    ) -> Result<HomeDelivered, String> {
        if !self.valid_for(transfer) || key.len() > 160 {
            return Err("invalid committed home result".into());
        }
        let mut home = self.home.clone();
        // Changes made while persistence was outstanding stay in the live
        // game; their queued facts are settled by the following claim.
        if run.home.squires != transfer.view.squires {
            home.squires = run.home.squires.clone();
        }
        if run.home.squire_academy != transfer.view.squire_academy {
            home.squire_academy = run.home.squire_academy.clone();
        }
        if run.home.wardrobe != transfer.view.wardrobe {
            home.wardrobe = run.home.wardrobe.clone();
        }
        if run.home.privy_wall != transfer.view.privy_wall {
            home.privy_wall = run.home.privy_wall.clone();
        }
        if run.home.salvage.rev > home.salvage.rev {
            home.salvage = run.home.salvage.clone();
        }
        if run.home.wished != transfer.view.wished {
            home.wished = run.home.wished.clone();
        }
        if run.home.sky != transfer.view.sky {
            home.sky = run.home.sky.clone();
        }
        if run.home.deep != transfer.view.deep {
            home.deep = run.home.deep.clone();
        }
        home.countryside.games.merge(&run.home.countryside.games);
        if transfer.company_reserved {
            run.complete_squire_company(Some(home.squire_company.clone()));
            home.squire_company = run.home.squire_company.clone();
        }
        // Rebase actions made after this claim was captured on the committed
        // company and treasury. Their orders remain queued for the next claim.
        let mut projected_treasury = self.treasury.clone();
        super::squire_guild::settle(&mut home.squire_guild, &mut projected_treasury, &run.marks);
        home.lane_squires.settle(&mut projected_treasury, &run.lane_squire_orders);
        if run.home != home || run.treasury != projected_treasury {
            run.rebuild_home(home, projected_treasury);
        }
        let mut delivered = HomeDelivered::default();
        let mut refund = 0u32;
        let mut sold = std::collections::BTreeSet::new();
        for (index, &(slot, knight)) in self.sales.iter().enumerate() {
            let available = run.stall.get(slot).is_some_and(String::is_empty)
                && same_knight(run, transfer, knight)
                && run.room().items.len()
                    < if run.open_map() {
                        game::depths::OPEN_ITEMS
                    } else {
                        game::MAX_ITEMS
                    };
            let card = if available {
                transfer.stall_cards[slot]
                    .as_ref()
                    .and_then(|card| bind_card(run, card, &format!("paid-{key}-stock-{slot}")))
            } else {
                None
            };
            if let Some(card) = card {
                let hero = &run.players[&knight];
                run.drop_item(card, hero.x, hero.y + 1.5, None);
                run.sounds.push("card_pickup");
                run.cues.push("grubbins_sold".into());
                sold.insert(slot);
            } else {
                delivered.refunded_sales.push(index);
                refund = refund
                    .checked_add(transfer.stall_prices[slot])
                    .ok_or("home refund overflow")?;
            }
        }
        for &slot in &transfer.reserved {
            if !sold.contains(&slot) && run.stall.get(slot).is_some_and(String::is_empty) {
                run.stall[slot] = transfer.stall[slot].clone();
            }
        }
        for (index, &quoted) in self.purchases.iter().enumerate() {
            let original = &transfer.purchases[quoted];
            let purchase = original.purchase;
            let mut received = false;
            if same_knight(run, transfer, purchase.knight) {
                if let Some(goods) = original.goods {
                    let fits = run
                        .players
                        .get(&purchase.knight)
                        .is_some_and(|hero| match goods {
                            merchant::Goods::Bomb => hero.bombs < hero.bomb_limit(),
                            _ => hero.hand.len() < cards::HAND,
                        });
                    if fits {
                        let card = match goods {
                            merchant::Goods::Bomb => Some(None),
                            _ => original
                                .card
                                .as_ref()
                                .and_then(|card| {
                                    bind_card(run, card, &format!("paid-{key}-cart-{quoted}"))
                                })
                                .map(Some),
                        };
                        if let Some(card) = card {
                            let same_place = [run.raid_id, run.dungeon.depth.into(), run.at as u64]
                                == transfer.location;
                            received = run.deliver_peddled(purchase, goods, card, same_place);
                        }
                    }
                } else if quote(run, purchase).as_ref() == Some(original) {
                    run.town_paid(purchase);
                    received = true;
                }
            }
            if !received {
                delivered.refunded_purchases.push(index);
                refund = refund
                    .checked_add(original.gold)
                    .ok_or("home refund overflow")?;
            }
        }
        for id in &self.announced {
            run.announce(id);
        }
        if let Some((tier, spoils)) = &self.unboxed {
            run.unbox(*tier, spoils);
        }
        delivered.refund.add(crate::realm::Spoil::Gold, refund);
        Ok(delivered)
    }
}
fn same_knight(run: &Run, transfer: &HomeTransfer, id: u32) -> bool {
    run.players.get(&id).is_some_and(|hero| {
        transfer.names.get(&id) == Some(&hero.name)
            && transfer.keys.get(&id) == Some(&game::talents::knight_key(hero))
    })
}
fn bind_card(run: &mut Run, original: &cards::Card, key: &str) -> Option<String> {
    if run.book.get(&original.id) == Some(original) {
        return Some(original.id.clone());
    }
    let mut captured = original.clone();
    captured.id = key.to_owned();
    if run.book.get(key) == Some(&captured) {
        return Some(key.to_owned());
    }
    if run.book.get(key).is_some() || !run.book.insert(captured) {
        return None;
    }
    Some(key.to_owned())
}

#[cfg(test)]
#[path = "../../tests/rules/home_settlement__tests.rs"]
mod tests;

impl HomeDelivered {
    pub fn valid_for(&self, transfer: &HomeTransfer, result: &HomeResult) -> bool {
        if !result.valid_for(transfer)
            || self
                .refunded_sales
                .iter()
                .any(|index| *index >= result.sales.len())
            || self
                .refunded_purchases
                .iter()
                .any(|index| *index >= result.purchases.len())
            || self
                .refunded_sales
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.refunded_sales.len()
            || self
                .refunded_purchases
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.refunded_purchases.len()
        {
            return false;
        }
        let mut prices = self
            .refunded_sales
            .iter()
            .map(|index| transfer.stall_prices[result.sales[*index].0])
            .chain(
                self.refunded_purchases
                    .iter()
                    .map(|index| transfer.purchases[result.purchases[*index]].gold),
            );
        let Some(total) = prices.try_fold(0u32, |sum, price| sum.checked_add(price)) else {
            return false;
        };
        self.refund
            .0
            .iter()
            .all(|(kind, amount)| *kind == crate::realm::Spoil::Gold && *amount == total)
            && self.refund.get(crate::realm::Spoil::Gold) == total
    }
}

#[cfg(test)]
mod squire_clock_transfer_tests {
    use super::*;

    #[test]
    fn newer_absolute_clock_survives_consumption_and_delivery() {
        let mut run = Run::at_home(3, 9, Some("Guest"), home::Home::default(), Default::default());
        run.home.squire_guild.clock = 100;
        run.marks.insert("squires:clock".into(), 100);
        let transfer = HomeTransfer::capture_and_reserve(&mut run).unwrap();
        for current in [None, Some(50), Some(100), Some(300)] {
            let mut copy = run.clone();
            copy.marks.remove("squires:clock");
            if let Some(clock) = current { copy.marks.insert("squires:clock".into(), clock); }
            transfer.validate_world(&copy).unwrap();
            transfer.consume(&mut copy).unwrap();
            assert_eq!(copy.marks.get("squires:clock").copied(), current.filter(|clock| *clock > 100));
        }
        let mut home = home::Home::default();
        let mut treasury = Default::default();
        let result = settle(&mut game::activity::Realm { home: &mut home, treasury: &mut treasury, reclaimed: &Default::default(), raids_won: 0 }, &transfer, "Host", false).unwrap();
        run.home.squire_guild.clock = 300;
        run.marks.insert("squires:clock".into(), 300);
        transfer.consume(&mut run).unwrap();
        assert_eq!(run.marks["squires:clock"], 300);
        result.deliver(&mut run, &transfer, "clock-test").unwrap();
        assert_eq!(run.home.squire_guild.clock, 300);
        let next = HomeTransfer::capture_and_reserve(&mut run).unwrap();
        let outbox = HomeOutbox::capture(&run).unwrap();
        let mut restored: Run = serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
        outbox.install(&mut restored).unwrap();
        next.validate_world(&restored).unwrap();
    }
}
