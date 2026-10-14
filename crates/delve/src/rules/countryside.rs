//! The surface ramble: authored terrain, one-time discoveries and paid works.
//! Visits and orders use the host's existing mark/settle/save transaction. No
//! practice round mints spoils, and an older cellar/settlement is never redrawn.

use super::*;
use crate::realm::{Spoil, Spoils};

pub const PLACES: [RoomKind; 8] = [
    RoomKind::Village,
    RoomKind::Fairground,
    RoomKind::Riverbank,
    RoomKind::Windmill,
    RoomKind::Ruins,
    RoomKind::Meadow,
    RoomKind::Orchard,
    RoomKind::Marsh,
];

const VISIT_MARKS: [&str; 8] = [
    "country:visit:0",
    "country:visit:1",
    "country:visit:2",
    "country:visit:3",
    "country:visit:4",
    "country:visit:5",
    "country:visit:6",
    "country:visit:7",
];
const RELIC_MARKS: [&str; 3] = ["country:relic:0", "country:relic:1", "country:relic:2"];
const LAMP_MARKS: [&str; 3] = ["country:lamp:0", "country:lamp:1", "country:lamp:2"];
pub const WEST_PLACES: [RoomKind; 2] = [RoomKind::Downs, RoomKind::Cliffs];
const WEST_MARKS: [&str; 2] = ["country:west:0", "country:west:1"];
pub const EAST_PLACES: [RoomKind; 2] = [RoomKind::Causeway, RoomKind::Priory];
const EAST_MARKS: [&str; 2] = ["country:east:0", "country:east:1"];
const PICNIC_MARKS: [&str; 3] = ["country:picnic:0", "country:picnic:1", "country:picnic:2"];
pub const NORTH_PLACES: [RoomKind; 3] =
    [RoomKind::Crag, RoomKind::Rookery, RoomKind::Observatory];
const NORTH_MARKS: [&str; 3] = ["country:north:0", "country:north:1", "country:north:2"];
const FORECAST_MARKS: [&str; 6] = [
    "country:forecast:0",
    "country:forecast:1",
    "country:forecast:2",
    "country:forecast:3",
    "country:forecast:4",
    "country:forecast:5",
];
/// A clerk, a brass rooster, three constellation lenses, then the clerk again.
/// Lenses are separated so holding E at one cannot solve the whole puzzle.
pub const FORECAST: [(RoomKind, (f32, f32)); 6] = [
    (RoomKind::Rookery, (6.0, 9.0)),
    (RoomKind::Crag, (19.0, 4.5)),
    (RoomKind::Observatory, (5.0, 5.5)),
    (RoomKind::Observatory, (12.0, 9.5)),
    (RoomKind::Observatory, (18.5, 6.5)),
    (RoomKind::Rookery, (6.0, 9.0)),
];
pub const INTENT_LIMIT: usize = super::college::INTENT_LIMIT + VISIT_MARKS.len()
    + WEST_MARKS.len()
    + EAST_MARKS.len()
    + NORTH_MARKS.len()
    + super::coastal::VISIT_MARKS.len()
    + super::coastal::QUEST_MARKS.len()
    + super::woodland::VISIT_MARKS.len()
    + super::woodland::QUEST_MARKS.len()
    + super::wetland::VISIT_MARKS.len()
    + super::wetland::QUEST_MARKS.len()
    + super::aerie::VISIT_MARKS.len()
    + super::aerie::QUEST_MARKS.len()
    + super::mummers::VISIT_MARKS.len()
    + super::hippodrome::VISIT_MARKS.len()
    + super::hippodrome::QUEST_MARKS.len()
    + super::hippodrome::WAGER_MARKS.len()
    + super::windcountry::VISIT_MARKS.len()
    + super::windcountry::QUEST_MARKS.len()
    + super::bowcountry::VISIT_MARKS.len()
    + super::bowcountry::QUEST_MARKS.len()
    + PICNIC_MARKS.len()
    + FORECAST_MARKS.len()
    + RELIC_MARKS.len()
    + LAMP_MARKS.len()
    + 1
    + SURVEY_MARKS.len()
    + PLOTS.len();

pub fn forecast_stage(country: &Country, marks: &BTreeMap<String, u32>) -> u8 {
    let mut stage = country.forecast.min(6);
    while stage < 6 && marks.contains_key(FORECAST_MARKS[usize::from(stage)]) {
        stage += 1;
    }
    stage
}
/// An ordered errand, not another set of interchangeable visit flags.
pub const PICNIC: [(RoomKind, (f32, f32)); 3] = [
    (RoomKind::Causeway, (20.5, 4.0)),
    (RoomKind::Priory, (5.0, 9.0)),
    (RoomKind::Riverbank, (18.0, 6.5)),
];

pub fn picnic_stage(country: &Country, marks: &BTreeMap<String, u32>) -> u8 {
    let mut stage = country.picnic.min(3);
    while stage < 3 && marks.contains_key(PICNIC_MARKS[usize::from(stage)]) {
        stage += 1;
    }
    stage
}

/// Finite whitelist for the private checkpoint intent journal. These marks
/// settle idempotently; ordinary kill counters remain transient.
/// The Firefly Grove's feats, settled into the realm at save.
pub const GROVE_MARKS: [&str; 2] = ["country:fireflies", "country:song"];

/// A surface game's first win, by its index in `pastimes::Game::ALL`.
pub fn game_mark(index: usize) -> String {
    format!("country:game:{index}")
}
/// What a surface game's first win pays the realm, once.
pub const FIRST_WIN_GOLD: u32 = 25;

/// Each surface game's first win, paid into the realm once (beside the
/// errands' `settle`, so their purses stay their own).
pub fn settle_games(
    country: &mut Country,
    treasury: &mut Spoils,
    marks: &BTreeMap<String, u32>,
) -> (bool, Vec<String>) {
    let mut news = Vec::new();
    if marks.contains_key(super::race3d::RACE_MARK) && !country.race_won {
        country.race_won = true;
        news.push("First race won.".to_string());
    }
    for (i, game) in super::pastimes::Game::ALL.iter().enumerate() {
        if marks.contains_key(&game_mark(i)) && country.games_paid & (1 << i) == 0 {
            country.games_paid |= 1 << i;
            treasury.add(Spoil::Gold, FIRST_WIN_GOLD);
            news.push(format!("First {} won: {FIRST_WIN_GOLD} gold.", game.name()));
        }
    }
    (!news.is_empty(), news)
}

pub fn intent_key(key: &str) -> bool {
    super::college::intent_key(key)
        || GROVE_MARKS.contains(&key)
        || key == super::race3d::RACE_MARK
        || key == super::woodland::ACORN_MARK
        || (0..super::pastimes::Game::ALL.len()).any(|i| key == game_mark(i))
        || VISIT_MARKS.contains(&key)
        || WEST_MARKS.contains(&key)
        || EAST_MARKS.contains(&key)
        || NORTH_MARKS.contains(&key)
        || PICNIC_MARKS.contains(&key)
        || FORECAST_MARKS.contains(&key)
        || super::coastal::VISIT_MARKS.contains(&key)
        || super::coastal::QUEST_MARKS.contains(&key)
        || super::woodland::VISIT_MARKS.contains(&key)
        || super::woodland::QUEST_MARKS.contains(&key)
        || super::wetland::VISIT_MARKS.contains(&key)
        || super::wetland::QUEST_MARKS.contains(&key)
        || super::aerie::VISIT_MARKS.contains(&key)
        || super::aerie::QUEST_MARKS.contains(&key)
        || super::mummers::VISIT_MARKS.contains(&key)
        || super::hippodrome::VISIT_MARKS.contains(&key)
        || super::hippodrome::QUEST_MARKS.contains(&key)
        || super::hippodrome::WAGER_MARKS.contains(&key)
        || super::windcountry::VISIT_MARKS.contains(&key)
        || super::windcountry::QUEST_MARKS.contains(&key)
        || super::bowcountry::VISIT_MARKS.contains(&key)
        || super::bowcountry::QUEST_MARKS.contains(&key)
        || (0..RELICS.len()).any(|i| key == format!("country:relic:{i}"))
        || (0..LAMPS.len()).any(|i| key == format!("country:lamp:{i}"))
        || (0..SURVEY.len()).any(|i| key == format!("country:survey:{i}"))
        || PLOTS
            .iter()
            .any(|p| key == format!("country:build:{}", p.id))
}
/// Intent values are bounded per action, including a private wager selection.
pub fn intent_valid(key:&str, value:u32)->bool {
    if !intent_key(key) || value==0 {return false;}
    if super::hippodrome::WAGER_MARKS[..3].contains(&key) {(1..=3).contains(&value)}
    // A finish names its winner, 2 to 4; an older save wrote 1.
    else if super::hippodrome::WAGER_MARKS[3..].contains(&key) {(1..=4).contains(&value)}
    else if key.starts_with("country:build:") {value <= super::MAX_PLAYERS}
    else {value==1}
}

const SURVEY_MARKS: [&str; 3] = ["country:survey:0", "country:survey:1", "country:survey:2"];
pub const SURVEY: [(RoomKind, (f32, f32)); 3] = [
    (RoomKind::Windmill, (18.0, 9.0)),
    (RoomKind::Downs, (12.0, 11.0)),
    (RoomKind::Cliffs, (17.0, 9.5)),
];

pub fn name(kind: RoomKind) -> &'static str {
    match kind {
        RoomKind::Gate => "GATE COURTYARD",
        RoomKind::Stables => "STABLES",
        RoomKind::Lists => "JOUSTING LISTS",
        RoomKind::MineHead => "MINE-HEAD",
        RoomKind::KingsHall => "KING'S HALL",
        RoomKind::LandOffice => "THE COURT OF DEEDS",
        RoomKind::Forest => "FIREFLY GROVE",
        RoomKind::Village => "BROADSHEET ROW",
        RoomKind::Fairground => "FOOL'S GREEN",
        RoomKind::Riverbank => "WASHBUCKET QUAY",
        RoomKind::Windmill => "WONKY WINDMILL",
        RoomKind::Ruins => "THE OLD WATCH",
        RoomKind::Meadow => "WAYFARER MEADOW",
        RoomKind::Orchard => "APPLE APOLOGY ORCHARD",
        RoomKind::Marsh => "SIR DAMP'S MARSH",
        RoomKind::Downs => "WHEYWARD DOWNS",
        RoomKind::Cliffs => "GULLIBLE CLIFFS",
        RoomKind::Causeway => "TOLL TROLL CAUSEWAY",
        RoomKind::Priory => "PUDDINGSTONE PRIORY",
        RoomKind::Crag => "CANDLEWICK CRAG",
        RoomKind::Rookery => "ROOKERY PASS",
        RoomKind::Observatory => "CUCKOO OBSERVATORY",
        RoomKind::Strand => "SOGGY BOTTOM STRAND",
        RoomKind::Saltworks => "SALTSPITE WORKS",
        RoomKind::Lighthouse => "THE BICKERING BEACON",
        RoomKind::Timberyard => "SPLINTER'S TIMBERYARD",
        RoomKind::HedgeMaze => "THE HEDGE OF GRIEVANCES",
        RoomKind::Bothy => "BADGER'S BOTHY",
        RoomKind::Millpond => "THE ROYAL DOG-PADDLE",
        RoomKind::Sluice => "THE SLUICE OF EXCUSES",
        RoomKind::SunkenAbbey => "THE SUNKEN ABBEY",
        RoomKind::Belfry => "THE BASHFUL BELFRY",
        RoomKind::Aqueduct => "UNNECESSARY AQUEDUCT",
        RoomKind::Aviary => "PIGEON PARLIAMENT",
        RoomKind::Mummers => "MUMMERS' HOLLOW",
        RoomKind::Laundry => "THE ROYAL SPIN CYCLE",
        RoomKind::Caravan => "THE CARAVAN OF QUESTIONABLE COMFORT",
        RoomKind::Hippodrome => "THE RUSTY HIPPODROME",
        RoomKind::Wheelwright => "SQUARE-WHEEL WORKS",
        RoomKind::TollBridge => "THE TOLL BRIDGE TO NOWHERE",
        RoomKind::GooseGreen => "SIR HONKS-A-LOT PICNIC GREEN",
        RoomKind::Kitewright => "THE KNOTTY KITEWRIGHT",
        RoomKind::SignalHill => "THE HILL OF EXTREMELY LOUD FLAGS",
        RoomKind::LongbowGreen => "LONGBOW GREEN",
        RoomKind::Fletchery => "THE FLETCHERY",
        RoomKind::Scarecrow => "THE SCARECROW'S FIELD",
        RoomKind::Market => "HAGGLER'S SQUARE",
        RoomKind::Bathhouse => "THE ROYAL SOAK",
        RoomKind::Smithy => "THE DENTED ANVIL",
        RoomKind::Eyrie => "THE GRIFFIN EYRIE",
        kind if super::college::is_room(kind) => super::college::label(kind),
        kind if super::ranch::is_room(kind) => super::ranch::label(kind),
        _ => "UNDERCROFT",
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Country {
    pub visited: u8,
    pub relics: u8,
    pub built: u16,
    pub ramble_paid: bool,
    pub relics_paid: bool,
    pub lamps: u8,
    pub lamps_paid: bool,
    /// Expansion masks never change the original eight visit bits.
    pub west_visited: u8,
    pub survey: u8,
    pub survey_paid: bool,
    pub east_visited: u8,
    /// 0: seek ticket, 1: seek hamper, 2: return to abbot, 3: delivered.
    pub picnic: u8,
    pub picnic_paid: bool,
    pub north_visited: u8,
    /// 0: clerk, 1: rooster, 2..4: lenses, 5: return forecast, 6: delivered.
    pub forecast: u8,
    pub forecast_paid: bool,
    /// The Firefly Grove's two once-only purses: all eight lanterns, and the
    /// woodland's song.
    pub fireflies_paid: bool,
    pub song_paid: bool,
    /// Each surface game's first win pays the realm once (bit per game).
    pub games_paid: u16,
    /// Somebody has won a race (a race hall's or the Eyrie's practice
    /// flight); until then Maud and the gold arrow send knights to the
    /// Griffin Eyrie.
    pub race_won: bool,
    /// The hedge maze's golden acorn has paid its gold.
    pub acorn_paid: bool,
    pub coast_visited: u8,
    /// Keeper, low-tide parcel, keeper again.
    pub harbour: u8,
    pub harbour_paid: bool,
    pub wood_visited: u8,
    /// Soup commission, three absurd mushrooms in order, then deliver.
    pub soup: u8,
    pub soup_paid: bool,
    pub wet_visited: u8,
    /// Brother Bucket, FISH/BOOT/DUCK valves, bell, Brother Bucket again.
    pub sluice: u8,
    pub sluice_paid: bool,
    pub aerie_visited: u8,
    /// 0: uncommissioned, 1: recover minutes in any order, 2: delivered.
    pub post: u8,
    pub minutes: u8,
    pub post_paid: bool,
    pub mummers_visited: u8,
    pub halfpipe_paid: bool,
    pub race_visited: u8,
    pub axle: u8,
    pub axle_parts: u8,
    pub axle_paid: bool,
    pub wind_visited: u8,
    pub signal: u8,
    pub signal_paid: bool,
    /// Longbow country's places seen, and the scarecrow's wardrobe: 0 not
    /// asked, 1 clothes being found, 2 dressed; the pieces found; paid.
    pub bow_visited: u8,
    pub wardrobe: u8,
    pub clothes: u8,
    pub wardrobe_paid: bool,
    /// Three finite spectator heats; never an infinite treasury faucet.
    pub wager_mask: u8,
    pub wager_tickets: [u8; 3],
    pub games: super::pastimes::Records,
    /// Times Humphrey the Risen has been felled; his grudge (and bulk)
    /// grow with it.
    #[serde(default)]
    pub humphrey_felled: u8,
    /// Ticks until he claws into the party's room again; 0 and a live world
    /// means he is due right now.
    #[serde(default)]
    pub humphrey_dig: u32,
}

impl Country {
    pub fn valid(&self) -> bool {
        self.wind_visited & !7 == 0
            && self.bow_visited & !7 == 0
            && self.wardrobe <= 2
            && self.clothes & !7 == 0
            && (self.wardrobe != 0 || self.clothes == 0)
            && (self.wardrobe != 2 || self.clothes == 7)
            && (!self.wardrobe_paid || self.wardrobe == 2)
            && (!self.has(super::bowcountry::PLOT) || self.wardrobe == 2)
            && self.signal <= 6
            && (!self.signal_paid || self.signal == 6)
            && (!self.has(super::windcountry::PLOT) || self.signal == 6)
            && self.race_visited & !7 == 0
            && self.axle <= 2 && self.axle_parts & !7 == 0
            && (self.axle != 0 || self.axle_parts == 0)
            && (self.axle != 2 || self.axle_parts == 7)
            && (!self.axle_paid || self.axle == 2)
            && self.wager_mask & !7 == 0
            && self.wager_tickets.iter().all(|&p| p <= 3)
            && (0..3).all(|i| self.wager_mask & (1<<i) == 0 || self.wager_tickets[i] != 0)
            && (!self.has(super::hippodrome::PLOT) || self.axle == 2)
            && self.mummers_visited & !7 == 0
            && (!self.halfpipe_paid || self.games.halfpipe.wins > 0)
            && (!self.has(super::mummers::PLOT)
                || (self.mummers_visited == 7 && self.halfpipe_paid))
            && self.aerie_visited & !7 == 0
            && self.post <= 2
            && self.minutes & !7 == 0
            && (self.post != 0 || self.minutes == 0)
            && (self.post != 2 || self.minutes == 7)
            && (!self.post_paid || self.post == 2)
            && (!self.has(10) || self.post == 2)
            && self.relics & !7 == 0
            && self.built & !((1u16 << PLOTS.len()) - 1) == 0
            && self.wet_visited & !7 == 0
            && self.sluice <= 6
            && (!self.sluice_paid || self.sluice == 6)
            && (!self.has(9) || self.sluice == 6)
            && self.wood_visited & !7 == 0
            && self.soup <= 5
            && (!self.soup_paid || self.soup == 5)
            && (!self.has(8) || self.soup == 5)
            && self.coast_visited & !7 == 0
            && self.harbour <= 3
            && (!self.harbour_paid || self.harbour == 3)
            && (!self.has(7) || self.harbour == 3)
            && self.lamps & !7 == 0
            && self.west_visited & !3 == 0
            && self.survey & !7 == 0
            && self.east_visited & !3 == 0
            && self.picnic <= 3
            && (!self.picnic_paid || self.picnic == 3)
            && (!self.has(5) || self.picnic == 3)
            && self.north_visited & !7 == 0
            && self.forecast <= 6
            && (!self.forecast_paid || self.forecast == 6)
            && (!self.has(6) || self.forecast == 6)
            && (!self.survey_paid || self.survey == 7)
            && (!self.has(4) || self.survey == 7)
            && (!self.ramble_paid || self.core_visits() == 6)
            && (!self.relics_paid || self.relics == 7)
            && (self.lamps == 0 || self.can_light())
            && (!self.lamps_paid || self.lamps == 7)
            && (!self.has(3) || self.lamps == 7)
            && self.games.valid()
            && self.humphrey_felled <= 9
            && self.humphrey_dig <= super::risen::RISE_AGAIN
    }

    /// The original six-place reward and inn requirement survive expansion.
    pub fn core_visits(&self) -> u32 {
        (self.visited & 63).count_ones()
    }

    pub fn can_light(&self) -> bool {
        self.has(1) && self.relics & 2 != 0
    }

    pub fn has(&self, index: usize) -> bool {
        index < PLOTS.len() && self.built & (1 << index) != 0
    }
}

impl Country {
    /// Mend what a bad save broke, one field at a time: a broken field is
    /// clamped or set from the progress that depends on it, and the rest is
    /// kept. The names of the fields mended.
    pub fn repair(&mut self) -> Vec<&'static str> {
        let mut mended = Vec::new();
        macro_rules! mend {
            ($bad:expr, $name:expr, $fix:expr) => {
                if $bad {
                    $fix;
                    mended.push($name);
                }
            };
        }
        // Stray bits and stages past their end.
        macro_rules! mask {
            ($field:ident, $bits:expr) => {
                mend!(self.$field & !$bits != 0, stringify!($field), self.$field &= $bits)
            };
        }
        mask!(wind_visited, 7);
        mask!(bow_visited, 7);
        mask!(clothes, 7);
        mask!(race_visited, 7);
        mask!(mummers_visited, 7);
        mask!(aerie_visited, 7);
        mask!(minutes, 7);
        mask!(relics, 7);
        mask!(wet_visited, 7);
        mask!(wood_visited, 7);
        mask!(coast_visited, 7);
        mask!(lamps, 7);
        mask!(west_visited, 3);
        mask!(survey, 7);
        mask!(east_visited, 3);
        mask!(north_visited, 7);
        mask!(axle_parts, 7);
        mask!(wager_mask, 7);
        mask!(built, (1u16 << PLOTS.len()) - 1);
        macro_rules! stage {
            ($field:ident, $last:expr) => {
                mend!(self.$field > $last, stringify!($field), self.$field = 0)
            };
        }
        stage!(signal, 6);
        stage!(wardrobe, 2);
        stage!(axle, 2);
        stage!(post, 2);
        stage!(sluice, 6);
        stage!(soup, 5);
        stage!(harbour, 3);
        stage!(picnic, 3);
        stage!(forecast, 6);
        for ticket in &mut self.wager_tickets {
            mend!(*ticket > 3, "wager_tickets", *ticket = 0);
        }
        mend!(self.humphrey_felled > 9, "humphrey_felled", self.humphrey_felled = 9);
        mend!(
            self.humphrey_dig > super::risen::RISE_AGAIN,
            "humphrey_dig",
            self.humphrey_dig = super::risen::RISE_AGAIN
        );
        if !self.games.valid() {
            self.games.repair();
            mended.push("games");
        }
        // Later progress vouches for what it needed: a paid reward or a
        // built plot means its job was done.
        let lamps_lit = self.lamps != 0 || self.lamps_paid || self.has(3);
        mend!(lamps_lit && !self.has(1), "built", self.built |= 1 << 1);
        mend!(lamps_lit && self.relics & 2 == 0, "relics", self.relics |= 2);
        macro_rules! done {
            ($vouched:expr, $field:ident, $last:expr) => {
                mend!($vouched && self.$field != $last, stringify!($field), self.$field = $last)
            };
        }
        done!(self.signal_paid || self.has(super::windcountry::PLOT), signal, 6);
        done!(self.wardrobe_paid || self.has(super::bowcountry::PLOT), wardrobe, 2);
        done!(self.wardrobe == 2, clothes, 7);
        mend!(
            self.wardrobe == 0 && self.clothes != 0,
            "wardrobe",
            self.wardrobe = 1
        );
        done!(self.axle_paid || self.has(super::hippodrome::PLOT), axle, 2);
        done!(self.axle == 2, axle_parts, 7);
        mend!(
            self.axle == 0 && self.axle_parts != 0,
            "axle",
            self.axle = 1
        );
        done!(self.has(super::mummers::PLOT), mummers_visited, 7);
        mend!(
            self.has(super::mummers::PLOT) && !self.halfpipe_paid,
            "halfpipe_paid",
            self.halfpipe_paid = true
        );
        if self.halfpipe_paid && self.games.halfpipe.wins == 0 {
            self.games.halfpipe.wins = 1;
            self.games.halfpipe.plays = self.games.halfpipe.plays.max(1);
            mended.push("games");
        }
        done!(self.post_paid || self.has(10), post, 2);
        done!(self.post == 2, minutes, 7);
        mend!(self.post == 0 && self.minutes != 0, "post", self.post = 1);
        done!(self.sluice_paid || self.has(9), sluice, 6);
        done!(self.soup_paid || self.has(8), soup, 5);
        done!(self.harbour_paid || self.has(7), harbour, 3);
        done!(self.picnic_paid || self.has(5), picnic, 3);
        done!(self.forecast_paid || self.has(6), forecast, 6);
        done!(self.survey_paid || self.has(4), survey, 7);
        mend!(
            self.ramble_paid && self.core_visits() != 6,
            "visited",
            self.visited |= 63
        );
        done!(self.relics_paid, relics, 7);
        done!(self.lamps_paid || self.has(3), lamps, 7);
        for i in 0..3 {
            mend!(
                self.wager_mask & (1 << i) != 0 && self.wager_tickets[i] == 0,
                "wager_mask",
                self.wager_mask &= !(1 << i)
            );
        }
        mended.dedup();
        mended
    }
}

/// Bad optional surface data cannot discard a treasury or the cellar, nor
/// the rest of the countryside: a field that will not read is left at its
/// default, and broken progress is mended field by field.
pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Country, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(load(value))
}

/// A saved countryside, read and mended field by field.
pub fn load(value: serde_json::Value) -> Country {
    let serde_json::Value::Object(mut fields) = value else {
        if !value.is_null() {
            tracing::warn!("countryside save is not an object; starting it afresh");
        }
        return Country::default();
    };
    let unread: Vec<String> = fields
        .iter()
        .filter(|(key, field)| {
            let alone = serde_json::Map::from_iter([((*key).clone(), (*field).clone())]);
            serde_json::from_value::<Country>(serde_json::Value::Object(alone)).is_err()
        })
        .map(|(key, _)| key.clone())
        .collect();
    for key in &unread {
        fields.remove(key);
    }
    let mut country =
        serde_json::from_value::<Country>(serde_json::Value::Object(fields)).unwrap_or_default();
    let mended = country.repair();
    if !country.valid() {
        tracing::warn!(?unread, ?mended, "countryside save beyond mending; starting it afresh");
        return Country::default();
    }
    if !unread.is_empty() || !mended.is_empty() {
        tracing::warn!(?unread, ?mended, "countryside save mended field by field");
    }
    country
}

pub struct Relic {
    pub room: RoomKind,
    pub at: (f32, f32),
    pub name: &'static str,
    pub clue: &'static str,
}

pub const RELICS: [Relic; 3] = [
    Relic {
        room: RoomKind::Windmill,
        at: (19.0, 4.5),
        name: "The Windmill's Wobbly Cog",
        clue: "THE MILL'S EAST FENCE HIDES A COG",
    },
    Relic {
        room: RoomKind::Ruins,
        at: (5.0, 4.5),
        name: "The Watchman's Lost Bell",
        clue: "A BELL LIES WEST OF THE OLD WATCH",
    },
    Relic {
        room: RoomKind::Riverbank,
        at: (19.0, 10.5),
        name: "The Royal Left Sock",
        clue: "THE QUAY'S SOUTH BANK HIDES A SOCK",
    },
];

pub struct Lamp {
    pub room: RoomKind,
    pub at: (f32, f32),
    pub name: &'static str,
}

pub const LAMPS: [Lamp; 3] = [
    Lamp {
        room: RoomKind::Ruins,
        at: (17.0, 6.0),
        name: "The Watchman's Wick",
    },
    Lamp {
        room: RoomKind::Orchard,
        at: (6.0, 9.0),
        name: "The Apple Apology Light",
    },
    Lamp {
        room: RoomKind::Marsh,
        at: (17.0, 5.5),
        name: "The Bog Bottom Lantern",
    },
];

pub struct Plot {
    pub id: &'static str,
    pub room: RoomKind,
    pub at: (f32, f32),
    pub footprint: (i32, i32, i32, i32),
    pub name: &'static str,
    pub says: &'static str,
    pub price: &'static [(Spoil, u32)],
    pub need: u8,
}

impl Plot {
    pub fn blocked(&self, country: &Country) -> Option<String> {
        if self.id == "kitepost" && country.signal != 6 {
            Some("First, the goose's sail and flags".into())
        } else if self.id == "quivercoach" && country.wardrobe != 2 {
            Some("First, dress the scarecrow".into())
        } else if self.id == "racecoach" && country.axle != 2 {
            Some("First, the coach's three parts".into())
        } else if self.id == "caravan" && (country.mummers_visited != 7 || !country.halfpipe_paid) {
            Some("First, win the half-pipe".into())
        } else if self.id == "aerie" && country.post != 2 {
            Some("First, the pigeons' minutes".into())
        } else if self.id == "punthouse" && country.sluice != 6 {
            Some("First, Brother Bucket's bell".into())
        } else if self.id == "bothy" && country.soup != 5 {
            Some("First, the badger's soup".into())
        } else if self.id == "harbour" && country.harbour != 3 {
            Some("First, the beacon's oil".into())
        } else if self.id == "ropeway" && country.forecast != 6 {
            Some("First, the king's forecast".into())
        } else if self.id == "post" && country.picnic != 3 {
            Some("First, the abbot's picnic".into())
        } else if self.id == "lift" && country.survey != 7 {
            Some("First, sign three stakes".into())
        } else if self.id == "ferry" && country.lamps != 7 {
            Some("First, light three lanterns".into())
        } else if country.core_visits() < u32::from(self.need) {
            Some(format!("Visit {} original surface places first", self.need))
        } else {
            None
        }
    }
}

pub const PLOTS: [Plot; 15] = [
    Plot {
        id: "bakery",
        room: RoomKind::Windmill,
        at: (6.0, 9.0),
        footprint: (3, 10, 5, 2),
        name: "Goose-Proof Bakery",
        says: "FEEDS THE GOOSE: COURIER SAFE",
        price: &[(Spoil::Gold, 80), (Spoil::Wax, 2)],
        need: 0,
    },
    Plot {
        id: "beacon",
        room: RoomKind::Ruins,
        at: (18.5, 9.0),
        footprint: (17, 10, 4, 2),
        name: "Wayfarer's Beacon",
        says: "BELL + BEACON: LIGHT THE ROAD LANTERNS",
        price: &[(Spoil::Gold, 140), (Spoil::Ore, 4)],
        need: 3,
    },
    Plot {
        id: "inn",
        room: RoomKind::Meadow,
        at: (7.0, 9.5),
        footprint: (3, 2, 6, 4),
        name: "The Bent Spoon Inn",
        says: "OPENS THE TWO-WAY GATE COACH",
        price: &[(Spoil::Gold, 180), (Spoil::Bone, 4), (Spoil::Wax, 3)],
        need: 6,
    },
    Plot {
        id: "ferry",
        room: RoomKind::Marsh,
        at: (18.5, 9.0),
        footprint: (17, 10, 4, 2),
        name: "Sir Damp's Ferry Office",
        says: "TWO-WAY FERRY: MARSH TO QUAY",
        price: &[(Spoil::Gold, 90), (Spoil::Bone, 6), (Spoil::Ore, 2)],
        need: 0,
    },
    Plot {
        id: "lift",
        room: RoomKind::Cliffs,
        at: (8.0, 4.7),
        footprint: (6, 1, 4, 3),
        name: "The Counterweight Goat Lift",
        says: "TWO-WAY LIFT: CLIFFS TO WINDMILL",
        price: &[(Spoil::Gold, 70), (Spoil::Bone, 3), (Spoil::Ore, 2)],
        need: 0,
    },
    Plot {
        id: "post",
        room: RoomKind::Priory,
        at: (18.5, 6.0),
        footprint: (16, 2, 5, 3),
        name: "The Overqualified Pigeon Post",
        says: "TWO-WAY FLIGHT: PRIORY TO LISTS",
        price: &[(Spoil::Gold, 100), (Spoil::Bone, 4), (Spoil::Wax, 2)],
        need: 0,
    },
    Plot {
        id: "ropeway",
        room: RoomKind::Observatory,
        at: (18.5, 8.5),
        footprint: (16, 2, 5, 3),
        name: "The Fair-Weather Ropeway",
        says: "TWO-WAY ROPEWAY: OBSERVATORY TO MINE",
        price: &[(Spoil::Gold, 110), (Spoil::Ore, 3), (Spoil::Wax, 2)],
        need: 0,
    },
    Plot {
        id: "harbour",
        room: RoomKind::Saltworks,
        at: (6.0, 9.0),
        footprint: (3, 10, 5, 2),
        name: "The Crab-Approved Harbour",
        says: "TWO-WAY BARGE: SALTWORKS TO QUAY",
        price: &[(Spoil::Gold, 90), (Spoil::Bone, 3), (Spoil::Ore, 3)],
        need: 0,
    },
    Plot {
        id: "bothy",
        room: RoomKind::Bothy,
        at: (18.5, 8.5),
        footprint: (16, 2, 5, 3),
        name: "The Badger's Board-and-Breakfast",
        says: "TWO-WAY WAGON: BOTHY TO ORCHARD",
        price: &[(Spoil::Gold, 95), (Spoil::Bone, 3), (Spoil::Wax, 2)],
        need: 0,
    },
    Plot {
        id: "punthouse",
        room: RoomKind::SunkenAbbey,
        at: (18.5, 8.5),
        footprint: (16, 2, 5, 3),
        name: "The Abbey Punt House",
        says: "TWO-WAY PUNT: ABBEY TO ORCHARD",
        price: &[(Spoil::Gold, 105), (Spoil::Bone, 4), (Spoil::Ore, 2)],
        need: 0,
    },
    Plot {
        id: "aerie",
        room: RoomKind::Aviary,
        at: (18.5, 8.5),
        footprint: (16, 2, 5, 3),
        name: "The Air-Mail Monstrosity",
        says: "TWO-WAY MAIL CART: PARLIAMENT TO ROOKERY",
        price: &[(Spoil::Gold, 100), (Spoil::Bone, 4), (Spoil::Wax, 2)],
        need: 0,
    },
    Plot {
        id: "caravan", room: RoomKind::Caravan, at: (18.5, 8.5),
        footprint: (16, 2, 5, 3), name: "The Wheely Inn",
        says: "TWO-WAY CARAVAN: CAMP TO FOOL'S GREEN",
        price: &[(Spoil::Gold, 110), (Spoil::Bone, 3), (Spoil::Wax, 2)], need: 0,
    },
    Plot {
        id: "racecoach", room: RoomKind::Hippodrome, at: (18.5, 10.0),
        footprint: (16, 11, 5, 2), name: "The Suspiciously Fast Coach",
        says: "TWO-WAY COACH: HIPPODROME TO STABLES",
        price: &[(Spoil::Gold, 120), (Spoil::Bone, 4), (Spoil::Ore, 2)], need: 0,
    },
    Plot {
        id: "kitepost", room: RoomKind::Kitewright, at: (19.5, 9.0),
        footprint: super::windcountry::KITE, name: "The Extremely Unwise Kite-Post",
        says: "A picnic sail, a basket and absolutely no safety officer.",
        price: &[(Spoil::Gold, 90), (Spoil::Bone, 3), (Spoil::Wax, 3)], need: 0,
    },
    Plot {
        id: "quivercoach", room: RoomKind::Fletchery, at: (19.5, 9.0),
        footprint: super::bowcountry::COACH, name: "The Quiver Coach",
        says: "TWO-WAY COACH: FLETCHERY TO BROADSHEET ROW",
        price: &[(Spoil::Gold, 90), (Spoil::Wax, 2), (Spoil::Ore, 2)], need: 0,
    },
];

pub const BARGE: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const QUAY_BARGE: (i32, i32, i32, i32) = (3, 11, 2, 1);

pub const COACH: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const MARSH_FERRY: (i32, i32, i32, i32) = (5, 9, 2, 1);
pub const QUAY_FERRY: (i32, i32, i32, i32) = (7, 11, 2, 1);
pub const LIFT: (i32, i32, i32, i32) = (3, 9, 2, 1);
pub const LIFT_FRONT: (f32, f32) = (4.0, 7.2);
pub const POST_FLIGHT: (i32, i32, i32, i32) = (19, 10, 2, 1);
pub const ROPEWAY: (i32, i32, i32, i32) = (19, 10, 2, 1);

pub fn cost(plot: &Plot) -> Spoils {
    let mut result = Spoils::default();
    for &(kind, amount) in plot.price {
        result.add(kind, amount);
    }
    result
}

fn fill(room: &mut Room, (c, r, w, h): (i32, i32, i32, i32), tile: Tile) {
    for y in r..r + h {
        for x in c..c + w {
            room.set(x as usize, y as usize, tile);
        }
    }
}

/// Obstacle silhouettes agree with scenery and leave all four door approaches
/// connected. Building sites reserve their footprint before anything is paid.
pub fn room(mut room: Room) -> Room {
    match room.kind {
        RoomKind::Village => {
            for rect in [
                (3, 1, 4, 2),
                (7, 1, 2, 2),
                (16, 1, 5, 2),
                (3, 10, 4, 3),
                (15, 10, 4, 3),
            ] {
                fill(&mut room, rect, Tile::Block);
            }
        }
        RoomKind::Fairground => {
            for rect in [(3, 1, 5, 2), (17, 1, 4, 2)] {
                fill(&mut room, rect, Tile::Block);
            }
        }
        RoomKind::Riverbank => {
            fill(&mut room, (1, 1, 22, 3), Tile::Hazard);
            // Leave room for a four-knight arrival fan, not just one rider.
            fill(&mut room, (9, 1, 6, 3), Tile::Floor);
            fill(&mut room, QUAY_FERRY, Tile::Stairs);
            fill(&mut room, QUAY_BARGE, Tile::Stairs);
        }
        RoomKind::Windmill => {
            fill(&mut room, (6, 2, 4, 3), Tile::Block);
            fill(&mut room, (17, 2, 4, 1), Tile::Block);
            fill(&mut room, LIFT, Tile::Stairs);
        }
        RoomKind::Ruins => {
            for rect in [(4, 2, 2, 1), (6, 2, 4, 3), (18, 3, 2, 2), (6, 10, 2, 2)] {
                fill(&mut room, rect, Tile::Block);
            }
        }
        RoomKind::Meadow => fill(&mut room, COACH, Tile::Stairs),
        RoomKind::Orchard => {
            for rect in [(4, 2, 3, 3), (17, 2, 3, 3), (4, 10, 3, 2), (17, 10, 3, 2)] {
                fill(&mut room, rect, Tile::Block);
            }
        }
        RoomKind::Marsh => {
            // The east doorway's four-knight fan grazes row four.
            fill(&mut room, (1, 1, 22, 3), Tile::Hazard);
            fill(&mut room, (1, 10, 11, 3), Tile::Hazard);
            fill(&mut room, MARSH_FERRY, Tile::Stairs);
        }
        RoomKind::Downs => {
            fill(&mut room, (4, 1, 4, 2), Tile::Block);
            fill(&mut room, (16, 1, 4, 2), Tile::Block);
        }
        RoomKind::Cliffs => {
            // Rail the sea; leave a broad dry road to the southern door.
            for c in [1, 15] {
                fill(&mut room, (c, 11, 8, 2), Tile::Hazard);
                fill(&mut room, (c, 10, 8, 1), Tile::Block);
            }
            fill(&mut room, LIFT, Tile::Stairs);
        }
        RoomKind::Causeway => {
            // Shallow water north and south; a broad, dry sporting causeway.
            fill(&mut room, (2, 1, 7, 3), Tile::Hazard);
            fill(&mut room, (15, 1, 7, 2), Tile::Hazard);
            fill(&mut room, (2, 11, 7, 2), Tile::Hazard);
            fill(&mut room, (15, 11, 7, 2), Tile::Hazard);
        }
        RoomKind::Priory => {
            fill(&mut room, (3, 2, 6, 3), Tile::Block);
            fill(&mut room, (4, 11, 5, 2), Tile::Block);
            fill(&mut room, POST_FLIGHT, Tile::Stairs);
        }
        RoomKind::Crag => {
            for rect in [(3, 1, 5, 2), (16, 1, 5, 2), (3, 11, 6, 2), (16, 11, 5, 2)] {
                fill(&mut room, rect, Tile::Block);
            }
        }
        RoomKind::Rookery => {
            fill(&mut room, (3, 2, 6, 3), Tile::Block);
            fill(&mut room, (16, 2, 4, 3), Tile::Block);
        }
        RoomKind::Observatory => {
            fill(&mut room, (6, 1, 4, 3), Tile::Block);
            fill(&mut room, (3, 11, 5, 2), Tile::Block);
            fill(&mut room, ROPEWAY, Tile::Stairs);
        }
        RoomKind::Eyrie => {
            // Crags either side of the griffins' perch, and the perch.
            for rect in super::race3d::EYRIE_ROCKS {
                fill(&mut room, rect, Tile::Block);
            }
            // The yard gate, back down to the kingdom's road.
            room.open_door(2);
        }
        _ => {}
    }
    super::coastal::shape(&mut room);
    super::woodland::shape(&mut room);
    super::wetland::shape(&mut room);
    super::aerie::shape(&mut room);
    super::mummers::shape(&mut room);
    super::hippodrome::shape(&mut room);
    super::windcountry::shape(&mut room);
    super::bowcountry::shape(&mut room);
    super::town::shape(&mut room);
    let kind = room.kind;
    for p in PLOTS.iter().filter(|p| p.room == kind) {
        fill(&mut room, p.footprint, Tile::Block);
    }
    room
}

impl Run {
    /// The knights (a bit each) whose E went down this tick, for the new
    /// rooms: a press, never a hold.
    fn rooms_pressed(&mut self, inputs: &BTreeMap<u32, Input>) -> u8 {
        let held = self
            .players
            .keys()
            .filter(|id| inputs.get(id).is_some_and(|i| i.valid() && i.interact))
            .fold(0u8, |bits, &id| bits | 1 << (id & 7));
        let pressed = held & !self.rooms_held;
        self.rooms_held = held;
        pressed
    }

    pub(super) fn tick_countryside(&mut self, inputs: &BTreeMap<u32, Input>) {
        // Arrived where someone sent the party: the arrow's work is done.
        if self.kingdom.moments.errand == Some(self.room().kind) {
            self.kingdom.moments.errand = None;
        }
        self.tick_coastal(inputs);
        self.tick_woodland(inputs);
        self.tick_wetland(inputs);
        self.tick_aerie(inputs);
        self.tick_mummers();
        self.tick_hippodrome(inputs);
        self.tick_windcountry(inputs);
        let pressed = self.rooms_pressed(inputs);
        self.tick_bowcountry(pressed);
        self.tick_town(inputs, pressed);
        self.tick_race_wagers(inputs);
        // Humphrey the Risen is retired: the surface stays free of zombies,
        // and any still standing from an older save are cleared away.
        self.enemies.retain(|e| e.kind != EnemyKind::Risen);
        let kind = self.room().kind;
        let living = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0);
        if living.clone().next().is_none() {
            self.build_hold = None;
            return;
        }
        if let Some(i) = PLACES.iter().position(|&k| k == kind) {
            if self.home.countryside.visited & (1 << i) == 0 {
                self.marks.insert(VISIT_MARKS[i].into(), 1);
            }
        }
        if let Some(i) = WEST_PLACES.iter().position(|&k| k == kind) {
            if self.home.countryside.west_visited & (1 << i) == 0 {
                self.marks.insert(WEST_MARKS[i].into(), 1);
            }
        }
        if let Some(i) = EAST_PLACES.iter().position(|&k| k == kind) {
            if self.home.countryside.east_visited & (1 << i) == 0 {
                self.marks.insert(EAST_MARKS[i].into(), 1);
            }
        }
        let stage = picnic_stage(&self.home.countryside, &self.marks);
        if let Some(i) = NORTH_PLACES.iter().position(|&k| k == kind) {
            if self.home.countryside.north_visited & (1 << i) == 0 {
                self.marks.insert(NORTH_MARKS[i].into(), 1);
            }
        }
        let forecast = forecast_stage(&self.home.countryside, &self.marks);
        if forecast < 6 && self.pastimes.round.is_none() {
            let (room, at) = FORECAST[usize::from(forecast)];
            if room == kind
                && living.clone().any(|(id, h)| {
                    inputs.get(id).is_some_and(|i| i.valid() && i.interact)
                        && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.8
                })
            {
                self.marks
                    .insert(FORECAST_MARKS[usize::from(forecast)].into(), 1);
                self.sounds.push("spoil_pickup");
                self.found = Some((self.tick, 0, match forecast {
                    0 => "Lost forecast! Fetch the brass rooster.",
                    1 => "Rooster found! Lenses: BOOT, GOOSE, CROWN.",
                    2 => "BOOT aligned! Seek the GOOSE lens south of the telescope.",
                    3 => "GOOSE aligned! Seek the CROWN lens beside the ropeway plot.",
                    4 => "Forecast: mostly knights, scattered embarrassment.",
                    _ => "Delivered! The king cancels Tuesday's rain.",
                }.into()));
            }
        }
        if stage < 3 && self.pastimes.round.is_none() {
            let (room, at) = PICNIC[usize::from(stage)];
            if room == kind
                && living.clone().any(|(id, h)| {
                    inputs.get(id).is_some_and(|i| i.valid() && i.interact)
                        && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 1.8
                })
            {
                self.marks
                    .insert(PICNIC_MARKS[usize::from(stage)].into(), 1);
                self.sounds.push("spoil_pickup");
                self.found = Some((self.tick, 0, match stage {
                    0 => "Ticket found! Now the hamper at the Priory.",
                    1 => "Hamper found! Back to the hungry abbot.",
                    _ => "Picnic delivered! The mustard is forgiven.",
                }.into()));
            }
        }
        for (i, &(_, at)) in SURVEY
            .iter()
            .enumerate()
            .filter(|(_, (room, _))| *room == kind)
        {
            let mark = SURVEY_MARKS[i];
            if self.pastimes.round.is_none()
                && self.home.countryside.survey & (1 << i) == 0
                && !self.marks.contains_key(mark)
                && living.clone().any(|(id, h)| {
                    inputs
                        .get(id)
                        .is_some_and(|input| input.valid() && input.interact)
                        && (h.x - at.0 * TILE_UNITS).hypot(h.y - at.1 * TILE_UNITS) < 2.2
                })
            {
                self.marks.insert(mark.into(), 1);
                self.sounds.push("spoil_pickup");
                self.found = Some((
                    self.tick,
                    0,
                    "Survey signed! The goat questions your qualifications.".into(),
                ));
            }
        }
        for (i, relic) in RELICS.iter().enumerate().filter(|(_, r)| r.room == kind) {
            if self.home.countryside.relics & (1 << i) == 0
                && living.clone().any(|(_, h)| {
                    (h.x - relic.at.0 * TILE_UNITS).hypot(h.y - relic.at.1 * TILE_UNITS) < 1.35
                })
            {
                // Pending marks latch pickups until the host saves them.
                let mark = RELIC_MARKS[i];
                if !self.marks.contains_key(mark) {
                    self.marks.insert(mark.into(), 1);
                    self.found = Some((
                        self.tick,
                        0,
                        format!("Found {}! Wren wants all three.", relic.name),
                    ));
                    self.sounds.push("spoil_pickup");
                }
            }
        }
        if self.home.countryside.can_light() && self.pastimes.round.is_none() {
            for (i, lamp) in LAMPS.iter().enumerate().filter(|(_, l)| l.room == kind) {
                let mark = LAMP_MARKS[i];
                if self.home.countryside.lamps & (1 << i) == 0
                    && !self.marks.contains_key(mark)
                    && living.clone().any(|(id, h)| {
                        inputs.get(id).is_some_and(|i| i.valid() && i.interact)
                            && (h.x - lamp.at.0 * TILE_UNITS).hypot(h.y - lamp.at.1 * TILE_UNITS)
                                < 1.4
                    })
                {
                    self.marks.insert(mark.into(), 1);
                    self.found = Some((
                        self.tick,
                        0,
                        format!("DING! {} is lit. The frogs applaud politely.", lamp.name),
                    ));
                    self.sounds.push("play_card");
                }
            }
        }
        // A plot already built takes no more orders, and none while an
        // arcade round is playing.
        let arcade = self.pastimes.round.is_some();
        let buyer = PLOTS
            .iter()
            .enumerate()
            .filter(|&(index, p)| {
                !arcade && p.room == kind && !self.home.countryside.has(index)
            })
            .find_map(|(index, p)| {
                living
                    .clone()
                    .find(|(id, h)| {
                        !h.buy_spent
                            && inputs.get(id).is_some_and(|i| i.valid() && i.interact)
                            && (h.x - p.at.0 * TILE_UNITS).hypot(h.y - p.at.1 * TILE_UNITS) < 1.4
                    })
                    .map(|(&id, _)| (index, id))
            });
        for (&id, h) in &mut self.players {
            if !inputs.get(&id).is_some_and(|i| i.valid() && i.interact) {
                h.buy_spent = false;
            }
        }
        let Some((index, id)) = buyer else {
            self.build_hold = None;
            return;
        };
        if !inputs.get(&id).is_some_and(|i| i.valid() && i.interact) {
            self.build_hold = None;
            return;
        }
        // Marking is a request, never a local spend. A continued hold cannot
        // place another order, including after a refused payment.
        if self.players[&id].buy_spent {
            return;
        }
        let ticks = self
            .build_hold
            .filter(|&(plot, who, _)| who == id && plot == index)
            .map_or(0, |(_, _, ticks)| ticks);
        let ticks = ticks.saturating_add(1).min(home::BUY_HOLD + 1);
        self.build_hold = Some((index, id, ticks));
        if ticks == home::BUY_HOLD {
            self.players.get_mut(&id).unwrap().buy_spent = true;
            self.marks
                .insert(format!("country:build:{}", PLOTS[index].id), id);
        }
    }
}

/// Shared with the host save transaction. All rewards are first-time world
/// objectives; revisits and festival replays can never farm the treasury.
pub fn settle(
    country: &mut Country,
    treasury: &mut Spoils,
    marks: &BTreeMap<String, u32>,
) -> (bool, Vec<String>) {
    if !marks.keys().any(|m| m.starts_with("country:"))
        && !(country.games.halfpipe.wins > 0 && !country.halfpipe_paid)
    {
        return (false, Vec::new());
    }
    let before = country.clone();
    let mut news = Vec::new();
    for i in 0..PLACES.len() {
        if marks.contains_key(VISIT_MARKS[i]) {
            country.visited |= 1 << i;
        }
    }
    for (i, mark) in WEST_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) {
            country.west_visited |= 1 << i;
        }
    }
    for (i, mark) in EAST_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) {
            country.east_visited |= 1 << i;
        }
    }
    country.picnic = picnic_stage(country, marks);
    for (i, mark) in NORTH_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) {
            country.north_visited |= 1 << i;
        }
    }
    super::coastal::settle(country, treasury, marks, &mut news);
    super::woodland::settle(country, treasury, marks, &mut news);
    super::wetland::settle(country, treasury, marks, &mut news);
    super::aerie::settle(country, treasury, marks, &mut news);
    super::mummers::settle(country, treasury, marks, &mut news);
    super::hippodrome::settle(country, treasury, marks, &mut news);
    super::windcountry::settle(country, treasury, marks, &mut news);
    super::bowcountry::settle(country, treasury, marks, &mut news);
    let sold = super::town::settle(treasury, marks, &mut news);
    super::race_wagers::settle(country, treasury, marks, &mut news);
    if marks.contains_key(GROVE_MARKS[0]) && !country.fireflies_paid {
        country.fireflies_paid = true;
        treasury.add(Spoil::Gold, 40);
        treasury.add(Spoil::Wax, 2);
        news.push("Eight fireflies in eight lanterns! +40 gold, 2 wax.".into());
    }
    if marks.contains_key(GROVE_MARKS[1]) && !country.song_paid {
        country.song_paid = true;
        treasury.add(Spoil::Gold, 40);
        news.push("The woodland sang it back. +40 gold.".into());
    }
    country.forecast = forecast_stage(country, marks);
    if country.forecast == 6 && !country.forecast_paid {
        country.forecast_paid = true;
        treasury.add(Spoil::Gold, 55);
        treasury.add(Spoil::Ore, 3);
        news.push("The King's Missing Forecast: 55 gold, 3 ore.".into());
    }
    if country.picnic == 3 && !country.picnic_paid {
        country.picnic_paid = true;
        treasury.add(Spoil::Gold, 60);
        treasury.add(Spoil::Wax, 3);
        news.push("The Abbot's Lost Picnic: 60 gold, 3 wax.".into());
    }
    for (i, mark) in SURVEY_MARKS.iter().enumerate() {
        if marks.contains_key(*mark) {
            country.survey |= 1 << i;
        }
    }
    if country.survey == 7 && !country.survey_paid {
        country.survey_paid = true;
        treasury.add(Spoil::Gold, 45);
        treasury.add(Spoil::Ore, 2);
        news.push("Western survey complete: 45 gold, 2 ore.".into());
    }
    for i in 0..RELICS.len() {
        if marks.contains_key(RELIC_MARKS[i]) {
            country.relics |= 1 << i;
        }
    }
    if country.core_visits() == 6 && !country.ramble_paid {
        country.ramble_paid = true;
        treasury.add(Spoil::Gold, 150);
        news.push("Wren's Grand Ramble: all six places visited! Treasury +150 gold.".into());
    }
    if country.relics == 7 && !country.relics_paid {
        country.relics_paid = true;
        treasury.add(Spoil::Gold, 100);
        treasury.add(Spoil::Ore, 4);
        news.push("A cog, a bell and one heroic sock: 100 gold, 4 ore.".into());
    }
    if country.can_light() {
        for (i, mark) in LAMP_MARKS.iter().enumerate() {
            if marks.contains_key(*mark) {
                country.lamps |= 1 << i;
            }
        }
    }
    if country.lamps == 7 && !country.lamps_paid {
        country.lamps_paid = true;
        treasury.add(Spoil::Gold, 75);
        treasury.add(Spoil::Wax, 4);
        news.push("Three lanterns lit: 75 gold, 4 wax.".into());
    }
    for (i, plot) in PLOTS.iter().enumerate() {
        if !marks.contains_key(&format!("country:build:{}", plot.id)) {
            continue;
        }
        if country.has(i) {
            news.push(format!("{} already stands.", plot.name));
            continue;
        }
        if let Some(reason) = plot.blocked(country) {
            news.push(format!("{}: {reason}.", plot.name));
            continue;
        }
        let price = cost(plot);
        if !treasury.covers(&price) {
            news.push(format!("{}: {}.", plot.name, treasury.shortfall(&price)));
            continue;
        }
        treasury.take(&price);
        country.built |= 1 << i;
        news.push(format!(
            "The realm raised {} for {}! {}.",
            plot.name,
            price.label(),
            plot.says
        ));
    }
    (*country != before || sold, news)
}
