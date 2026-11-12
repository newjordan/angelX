//! Room encounters: the minigames come back to the delve as rooms the party
//! stumbles on. The upper world was cut to one tiny Keep and camps
//! (`world::plan`), so the games that stood at its doors lost them; every
//! floor of a delve now deals one hall (from the second floor a second, one
//! time in `SECOND_PERCENT`) a game picked from a wide pool by the run's seed
//! (`Kind::ALL`). Walking in opens it for every living knight in the room,
//! as the old folly always has; holding Esc leaves.
//!
//! The four follies (the Wax Maze, the Belfry Shoot, the Powder Room and
//! Book Hop) keep their own rules, stake and podium; this module only deals
//! them. Every other game is hosted here: its own state lives on the `Run`
//! as it does at its kingdom door, and `Live` wraps it in the one lifecycle
//! a random room needs:
//!
//! - **Stakes.** Each knight antes a tenth of the gold they carry
//!   (`folly::ANTE_TENTHS`), as the follies ask. A game for the table is won
//!   together (everyone is paid and gets their ante back; a loss costs the
//!   ante and no more); a game against each other pays in finishing order,
//!   the first taking the pot. Both pay the crossroads' purse
//!   (`crossroads::gold`) and a wish for the winner.
//! - **Length.** A long game is cut: a campaign (Lanternward, Moonfall,
//!   Bannerpeak, Griffinwing, Lightgallop) plays to its first objective
//!   (`Kind::objective`) or the cap, whichever is first; anything else that
//!   could run on stops at its cap with whoever is ahead paid (`cap`).
//! - **Safety.** A knight's health and gear are as they were at the door
//!   when it is over, and nobody falls to zero inside it (`Run::keep_hearts`).
//! - **After.** The room is cleared and its doors open; the floor goes on as
//!   it was. Leaving by the door ends the game (the stake is lost); coming
//!   back deals it afresh, as the folly does.
use super::pastimes::Game;
use super::*;

/// Every encounter's title card: how long the name and its one line of how to
/// play stand over the game.
pub const TITLE_TICKS: u32 = 4 * HZ;
/// How long the payout card holds the room once a game is settled.
pub const PAYOUT_TICKS: u32 = 5 * HZ;
/// The most a game's ticks may count: past this a saved game is not one.
const LONGEST: u32 = 10 * 60 * HZ;
/// From this floor a second hall may deal a game, and the chance of it in
/// percent. Fixed numbers, one each.
pub const SECOND_FROM: u32 = 2;
pub const SECOND_PERCENT: u64 = 40;

/// How long a hosted game runs before it is cut, in ticks. A campaign or a
/// match that could run past about three minutes ends here, the knights who
/// are ahead paid (`Kind::cap`).
pub const CAP: u32 = 3 * 60 * HZ;
/// The race's grace after the last knight is home, as the crossroads keeps.
pub const GRACE: u32 = 2 * HZ;

/// One game a random room can deal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    WaxMaze,
    BelfryShoot,
    PowderRoom,
    BookHop,
    Lanternward,
    Moonfall,
    Bannerpeak,
    Griffinwing,
    Lightgallop,
    KnightRacers,
    KnightsWish,
    ShaftRun,
    GriffinRun,
    TorchlightTouge,
    /// One of the kingdom's arcade games, played on its own stage in the hall
    /// (`ARCADE`).
    Arcade(Game),
}

/// The arcade games a hall deals, each on the stage of its own kingdom room.
/// The Powder Keg is the Powder Room folly, so it is not here; `LEFT_OUT` says
/// why the others of the old map are not.
pub const ARCADE: [Game; 18] = [
    Game::Broadsheet,
    Game::Barrel,
    Game::Turnip,
    Game::Washtub,
    Game::Cheese,
    Game::Hobbyhorse,
    Game::Sled,
    Game::Pot,
    Game::Cart,
    Game::Surf,
    Game::Volley,
    Game::Halfpipe,
    Game::Chariot,
    Game::Trencher,
    Game::DuckShoot,
    Game::Longbow,
    Game::Peal,
    Game::Gander,
];

/// The games that lost their door on the blank-slate map and are not dealt to
/// a hall, and the reason for each (a test holds every arcade game to being
/// dealt, the Powder Room, or here).
pub const LEFT_OUT: &[(&str, &str)] = &[
    (
        "SIEGE TRAIL",
        "its trail is five legs and 300 s, which a 3 min cap never lets a table win, and its own save check caps a round at 120 volleys (a table firing steadily passes it, and the save is refused)",
    ),
    ("PENNON RUSH", "its stage, the muster tent, is a room of the Undercroft's own plan with no builder of its own"),
    ("RAFTERWAKE", "its stage, the workshop, is a room of the Undercroft's own plan with no builder of its own"),
    ("BRASSWAKE", "its stage, the tavern, is a room of the Undercroft's own plan with no builder of its own"),
    ("MOSSBELL", "stage builder exists (the bathhouse) but not staged here: no result adapter for its round yet"),
    ("LATCH DUEL", "stage builder exists (the smithy) but not staged here: no result adapter for its round yet"),
    ("CINDER STACK", "stage builder exists (the college lab) but not staged here: no result adapter for its round yet"),
    ("THE LISTS", "a bout needs a saddled herd horse and the stable's state, which a delve does not carry"),
    ("HIPPODROME WAGERS", "wagers on a herd horse's team need the stable's state; the arcade's Cabbage and Roll Racing is dealt instead"),
    ("FOREST ROAD", "its campaign mark is the realm's, and its stage is the Dark Forest's own side-view road"),
    ("ORE CART", "off in play by the operator's order (2026-10-17), as the race hall is"),
    ("KEG GANTRY, LEDGE, WANTED", "side-on halls, off in play"),
];

impl Kind {
    /// Every game of the pool, in the order the seed picks from.
    pub const ALL: [Kind; 14] = [
        Kind::WaxMaze,
        Kind::BelfryShoot,
        Kind::PowderRoom,
        Kind::BookHop,
        Kind::Lanternward,
        Kind::Moonfall,
        Kind::Bannerpeak,
        Kind::Griffinwing,
        Kind::Lightgallop,
        Kind::KnightRacers,
        Kind::KnightsWish,
        Kind::ShaftRun,
        Kind::GriffinRun,
        Kind::TorchlightTouge,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::WaxMaze => "WAX MAZE",
            Kind::BelfryShoot => "BELFRY SHOOT",
            Kind::PowderRoom => "POWDER ROOM",
            Kind::BookHop => "BOOK HOP",
            Kind::Lanternward => lanternward::NAME,
            Kind::Moonfall => moonfall::NAME,
            Kind::Bannerpeak => bannerpeak::NAME,
            Kind::Griffinwing => griffinwing::NAME,
            Kind::Lightgallop => lightgallop::NAME,
            Kind::KnightRacers => knight_racers::NAME,
            Kind::KnightsWish => deathmatch::NAME,
            Kind::ShaftRun => "SHAFT RUN",
            Kind::GriffinRun => "GRIFFIN RUN",
            Kind::TorchlightTouge => "TORCHLIGHT TOUGE",
            Kind::Arcade(game) => game.name(),
        }
    }

    /// One short line of how to play, in capitals for the pixel font.
    pub fn words(self) -> &'static str {
        match self {
            Kind::WaxMaze => "EAT THE WAX, DODGE THE WRAITHS",
            Kind::BelfryShoot => "SHOOT THE BATS, MOST WINS",
            Kind::PowderRoom => "DROP KEGS, BREAK THE CRATES",
            Kind::BookHop => "HOP THE BOOKS ACROSS THE RIVER",
            Kind::Lanternward => "LOCK ON, SWORD AND SHIELD. CLEAR THE FIRST ROOM",
            Kind::Moonfall => "TALK AND FETCH. DO THE FIRST FAVOUR",
            Kind::Bannerpeak => "RUN AND JUMP. RAISE A BANNER",
            Kind::Griffinwing => "FLY AND SHOOT. FINISH THE FIRST BEAT",
            Kind::Lightgallop => "GALLOP AND LEAP. LIGHT A LANTERN POST",
            Kind::KnightRacers => "ONE LAP. W A S D STEER, SPACE BOOSTS",
            Kind::KnightsWish => "WASD MOVES, F FIRES, TEN KILLS WINS",
            Kind::ShaftRun => "ROLL, DON'T FALL. FIRST HOME WINS",
            Kind::GriffinRun => "FLY THE GORGE. FIRST HOME WINS",
            Kind::TorchlightTouge => "DRIFT, THEN BOOST. FIRST HOME WINS",
            Kind::Arcade(game) => arcade_words(game),
        }
    }

    /// One of the four follies: they run, stake and pay themselves.
    pub fn folly(self) -> bool {
        matches!(self, Kind::WaxMaze | Kind::BelfryShoot | Kind::PowderRoom | Kind::BookHop)
    }

    /// Won together: everyone is paid on a win. The rest play each other.
    pub fn co_op(self) -> bool {
        matches!(
            self,
            Kind::Lanternward | Kind::Moonfall | Kind::Bannerpeak | Kind::Griffinwing | Kind::Lightgallop
        )
    }

    /// An adventure of the Round Table, hosted in the hall as it is in the
    /// Dark Forest.
    pub fn adventure(self) -> Option<&'static roundtable::Adventure> {
        roundtable::adventure(match self {
            Kind::Lanternward => "lanternward",
            Kind::Moonfall => "moonfall",
            Kind::Bannerpeak => "bannerpeak",
            Kind::Griffinwing => "griffinwing",
            Kind::Lightgallop => "lightgallop",
            Kind::KnightRacers => "knight_racers",
            Kind::KnightsWish => "deathmatch",
            _ => return None,
        })
    }

    /// Whether the room's own cap is what limits it (a campaign, a match, a
    /// race): the card shows the cap's clock. The arcade rounds and the race
    /// hall keep clocks of their own on the screen.
    pub fn cut_by_cap(self) -> bool {
        self.co_op() || matches!(self, Kind::KnightRacers | Kind::KnightsWish)
    }

    /// The fewest knights it can be dealt to: a free-for-all needs a rival.
    pub fn least(self) -> usize {
        if self == Kind::KnightsWish { 2 } else { 1 }
    }

    /// The race hall's course, for the three that ride the race hall.
    pub fn course(self) -> Option<race3d::Course> {
        match self {
            Kind::ShaftRun => Some(race3d::Course::Shaft),
            Kind::GriffinRun => Some(race3d::Course::Griffin),
            Kind::TorchlightTouge => Some(race3d::Course::Touge),
            _ => None,
        }
    }

    /// The most ticks it runs before it is cut. The follies and the race
    /// hall keep their own clocks, which are all under it.
    pub fn cap(self) -> u32 {
        CAP
    }

    /// What a solo-safe game stands in for one that needs a rival.
    fn stand_in(self) -> Kind {
        match self {
            Kind::KnightsWish => Kind::KnightRacers,
            other => other,
        }
    }
}

/// The folly an old floor's hall was dealt before the pool: the rotation the
/// seed's pick replaced, kept for a save that has none.
pub fn legacy(depth: u32) -> Kind {
    match depth % 3 {
        1 => Kind::WaxMaze,
        2 => Kind::BelfryShoot,
        _ if depth % 6 == 0 => Kind::BookHop,
        _ => Kind::PowderRoom,
    }
}

/// What a knight was at the door, put back when the game is over: health and
/// armour, and the gear they were dressed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub hp: u32,
    pub armor: u32,
    pub gear: u8,
    pub gear_was: u8,
    /// Where they stood in the hall, in units.
    pub x: i32,
    pub y: i32,
}

/// How a settled game ended and what it paid, for the payout card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct End {
    /// Whether the table won (a game for the table) or anybody was paid.
    pub won: bool,
    /// Whether the cap ended it.
    pub capped: bool,
    /// The knights in the order they are paid.
    pub order: Vec<u32>,
    /// Gold each knight is paid, the ante returned and the pot included.
    pub paid: BTreeMap<u32, u32>,
    /// Ticks the card has left.
    pub left: u32,
}

/// The game a hall is playing now.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Live {
    pub room: usize,
    pub kind: Kind,
    /// Ticks since it opened.
    pub t: u32,
    /// The knights it seats: every living knight in the room as it opened,
    /// less those who have walked out.
    pub seats: Vec<u32>,
    /// What each antes (a folly keeps its own).
    pub ante: BTreeMap<u32, u32>,
    pub kept: BTreeMap<u32, Kept>,
    /// The hall as it was, while a stage of another kind stands in its place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hall: Option<Box<Room>>,
    /// Ticks the table has been all home, for the race's grace.
    #[serde(default)]
    pub home: u32,
    /// An arcade game's heat is judged: the knights who won their round, best
    /// first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fin: Option<Vec<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<End>,
}

impl Live {
    pub fn valid(&self, rooms: usize) -> bool {
        let id = |i: &u32| (1..=MAX_PLAYERS).contains(i);
        self.room < rooms
            && self.t <= LONGEST
            && self.seats.len() <= MAX_PLAYERS as usize
            && self.seats.iter().all(id)
            && self.seats.iter().enumerate().all(|(i, s)| !self.seats[..i].contains(s))
            && self.ante.len() <= MAX_PLAYERS as usize
            && self.ante.keys().all(id)
            && self.kept.len() <= MAX_PLAYERS as usize
            && self.kept.keys().all(id)
            && self.kept.values().all(|k| k.x >= 0 && k.y >= 0)
            && self.home <= LONGEST
            && self.fin.as_ref().is_none_or(|f| f.len() <= MAX_PLAYERS as usize && f.iter().all(id))
            && self.end.as_ref().is_none_or(|e| {
                e.left <= PAYOUT_TICKS
                    && e.order.len() <= MAX_PLAYERS as usize
                    && e.order.iter().all(id)
                    && e.paid.len() <= MAX_PLAYERS as usize
                    && e.paid.keys().all(id)
            })
    }

    /// Whether the title card is up.
    pub fn titled(&self) -> bool {
        self.t < TITLE_TICKS
    }

    /// Whole seconds of the clock left, counting to the cap.
    pub fn left(&self) -> u32 {
        self.kind.cap().saturating_sub(self.t).div_ceil(HZ)
    }
}

/// A floor's encounter hall and the game it deals.
pub type Door = (usize, Kind);

#[cfg(any(test, feature = "testing"))]
impl Run {
    /// Deal the folly hall `kind`: the fixtures that draw or play one particular
    /// game of the pool. `None` when the floor has no such hall.
    pub fn deal_for_test(&mut self, kind: Kind) -> Option<usize> {
        let index = self.folly_room?;
        self.encounter_rooms = vec![(index, kind)];
        Some(index)
    }

    /// Deal the folly hall the rotation's game for this floor (the Wax Maze on
    /// the first, the Belfry Shoot on the second ...): what the floors dealt
    /// before the pool, for the fixtures that draw or play those follies.
    pub fn deal_legacy_for_test(&mut self) -> Option<usize> {
        self.deal_for_test(legacy(self.dungeon.depth))
    }
}

/// The pick for hall number `slot` of a floor: the seed's choice from the
/// pool, never a game the floor before dealt nor one this floor has. Pure:
/// it draws from no dice, so dealing never changes a floor or the next.
pub fn pick(seed: u64, raid: u64, depth: u32, slot: usize, party: usize, avoid: &[Kind]) -> Kind {
    let salt = mix(seed ^ raid.rotate_left(11) ^ u64::from(depth).rotate_left(5) ^ (slot as u64 + 1).rotate_left(23));
    let choose = |pool: &[Kind], salt: u64| {
        let free: Vec<Kind> = pool.iter().copied().filter(|k| !avoid.contains(k)).collect();
        let from: &[Kind] = if free.is_empty() { pool } else { &free };
        from[(salt % from.len() as u64) as usize]
    };
    let pool = pool();
    let kind = choose(&pool, salt);
    if kind.least() > party {
        // A free-for-all needs a rival: a lone knight is dealt a game for one.
        let solo: Vec<Kind> = pool.iter().copied().filter(|k| k.least() <= party).collect();
        return choose(&solo, mix(salt ^ 0x5010));
    }
    kind
}

/// Every game the seed may deal.
pub fn pool() -> Vec<Kind> {
    Kind::ALL.into_iter().chain(ARCADE.map(Kind::Arcade)).collect()
}

/// An arcade game's goal in one line (its own intro board has the keys).
fn arcade_words(game: Game) -> &'static str {
    match game {
        Game::Broadsheet => "POST THE PAPERS IN THE GOLD BOXES",
        Game::Barrel => "RIDE THE FLAGS IN ORDER, F JUMPS",
        Game::Turnip => "KEEP THE TURNIP UP, F KICKS",
        Game::Washtub => "PADDLE THROUGH THE RINGS, F STROKES",
        Game::Cheese => "LOB CHEESE AT THE GOATS, HOLD F FOR POWER",
        Game::Hobbyhorse => "JUMP EVERY HURDLE WITH F",
        Game::Sled => "STEER THROUGH ALL SIX GATES",
        Game::Pot => "HOLD F, LET GO TO PUT THE POT",
        Game::Cart => "WHEEL THE CART ACROSS, F HOPS THE LOGS",
        Game::Surf => "LEAN AND CATCH THE GOLD CREST",
        Game::Volley => "RING THE FOUR BELLS, F SWINGS",
        Game::Halfpipe => "PUMP, THEN F AT THE LIP FOR A TRICK",
        Game::Chariot => "RACE THE LAPS HOME, F THROWS CABBAGE",
        Game::Trencher => "TOSS PLATES TO THE GOOSE WITH F",
        Game::DuckShoot => "AIM AND SHOOT THE DUCKS WITH F",
        Game::Longbow => "HOLD F TO DRAW, LET GO TO LOOSE",
        Game::Powder => "DROP KEGS, THE LAST ONE STANDS",
        Game::Gander => "FLAP WITH F, LAND ON TOP",
        Game::Siege => "AIM AND LOB TO BREACH THE CASTLE",
        Game::Peal => "PULL ON THE GOLD, NOT EARLY",
    }
}

impl Run {
    /// The game hall `index` deals, if it is an encounter room: the pick
    /// the floor was dealt, or (an old save's folly hall) the rotation it
    /// would have had.
    pub fn encounter_at(&self, index: usize) -> Option<Kind> {
        if let Some(&(_, kind)) = self.encounter_rooms.iter().find(|&&(room, _)| room == index) {
            return Some(kind);
        }
        (self.folly_room == Some(index)).then(|| legacy(self.dungeon.depth))
    }

    /// Whether hall `index` hosts a game of any kind (a folly, a gantry, a
    /// race hall, or an encounter): the rooms nothing else is raised in.
    pub fn hosts_game(&self, index: usize) -> bool {
        [self.folly_room, self.gantry_room, self.race3d_room].contains(&Some(index))
            || self.encounter_rooms.iter().any(|&(room, _)| room == index)
    }

    /// A new floor starts: what this one dealt is what the next must not.
    pub(super) fn roll_encounters(&mut self) {
        self.encounter_prev = self.encounter_rooms.iter().map(|&(_, kind)| kind).collect();
        self.encounter_rooms.clear();
    }

    /// Deal the floor's games: its folly hall (`pick_folly`) takes the seed's
    /// pick, and from the second floor a second hall may take another.
    /// Never the start, a boss's camp or a guardian's room, nor the way down
    /// (`pick_folly` keeps to halls); never a game the floor before dealt.
    pub(super) fn pick_encounters(&mut self) {
        self.encounter_rooms.clear();
        if self.dungeon.depth == 0 {
            return;
        }
        let Some(first) = self.folly_room else {
            return;
        };
        let (seed, raid, depth, party) = (self.seed, self.raid_id, self.dungeon.depth, self.party_size());
        let prev = self.encounter_prev.clone();
        let one = pick(seed, raid, depth, 0, party, &prev);
        self.encounter_rooms.push((first, one));
        let coin = mix(seed ^ raid.rotate_left(29) ^ u64::from(depth).rotate_left(3) ^ 0xc01f) % 100;
        if depth < SECOND_FROM || coin >= SECOND_PERCENT {
            return;
        }
        let camps = |i: usize| {
            self.boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(i).is_some() || g.guardian == i)
        };
        let of = |kind: RoomKind| -> Vec<usize> {
            (0..self.dungeon.rooms.len())
                .filter(|&i| {
                    let r = &self.dungeon.rooms[i];
                    r.kind == kind && !r.great() && !r.cleared && i != self.at && !camps(i) && !self.hosts_game(i)
                })
                .collect()
        };
        // A spare hall, else a spare fight room (as the folly hall is picked).
        let mut halls = of(RoomKind::Hall);
        if halls.is_empty() {
            halls = of(RoomKind::Fight);
        }
        if halls.is_empty() {
            return;
        }
        let at = (mix(seed ^ raid.rotate_left(13) ^ u64::from(depth).rotate_left(41) ^ 0x5ec0d) % halls.len() as u64) as usize;
        let mut avoid = prev;
        avoid.push(one);
        let two = pick(seed, raid, depth, 1, party, &avoid);
        self.encounter_rooms.push((halls[at], two));
    }

    /// An encounter is under way in the hall the party stands in.
    pub fn encounter_running(&self) -> bool {
        self.encounter.as_ref().is_some_and(|e| e.room == self.at)
    }

    /// An encounter that this module hosts (not a folly) is under way in this
    /// room: the games that ask whether the stage is theirs.
    pub fn encounter_hosts(&self) -> bool {
        self.encounter.as_ref().is_some_and(|e| e.room == self.at && !e.kind.folly())
    }

    /// The stage an adventure of the Round Table plays on: the Dark Forest's
    /// room at its kingdom door, or the hall an encounter dealt it.
    pub fn on_stage(&self) -> bool {
        self.room().kind == RoomKind::Forest || self.encounter_hosts()
    }

    /// Whether the room's game is paid here and not by the game itself: the
    /// crossroads' vote or an encounter.
    pub fn host_pays(&self) -> bool {
        self.crossroads.is_some() || self.encounter_hosts()
    }

    /// Walk the party into hall `index` and open its game for every living
    /// knight in it.
    pub(super) fn start_encounter(&mut self, index: usize, from: Option<usize>, dealt: Kind) {
        let seats: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && !h.absent && !h.away)
            .map(|(&id, _)| id)
            .collect();
        // A free-for-all is never dealt to a lone knight, whoever has left.
        let kind = if dealt.least() > seats.len() { dealt.stand_in() } else { dealt };
        if kind != dealt {
            for door in &mut self.encounter_rooms {
                if door.0 == index {
                    door.1 = kind;
                }
            }
        }
        let kept = seats
            .iter()
            .filter_map(|id| self.players.get(id).map(|h| (*id, h)))
            .map(|(id, h)| {
                let at = |v: f32| v.max(0.0) as i32;
                (id, Kept { hp: h.hp, armor: h.armor, gear: h.gear, gear_was: h.gear_was, x: at(h.x * 16.0), y: at(h.y * 16.0) })
            })
            .collect();
        // The games say how to play in a line of their own at a kingdom door;
        // here the title card does, and whatever was on the notice stays.
        let notice = self.found.clone();
        self.encounter = Some(Live {
            room: index,
            kind,
            t: 0,
            seats: seats.clone(),
            ante: BTreeMap::new(),
            kept,
            hall: None,
            home: 0,
            fin: None,
            end: None,
        });
        match kind {
            Kind::WaxMaze => self.start_wax_maze(index, from),
            Kind::BelfryShoot => self.start_bat_shoot(index),
            Kind::PowderRoom => self.start_powder(index),
            Kind::BookHop => self.start_book_hop(index),
            _ => {
                // Everyone antes a tenth of what they carry, as at a folly.
                let mut stake = self.players.clone();
                stake.retain(|id, _| seats.contains(id));
                let (ante, _) = folly::take_antes(&mut stake);
                for (id, h) in stake {
                    if let Some(mine) = self.players.get_mut(&id) {
                        mine.carried = h.carried;
                    }
                }
                if let Some(live) = &mut self.encounter {
                    live.ante = ante;
                }
                self.open_hosted(index, kind, &seats);
                // A game that would not open (its engine is missing) is never
                // left standing as an empty room: the stake goes back and the
                // hall deals the Wax Maze, which needs nothing.
                if !self.hosted_open(kind, self.encounter.as_ref()) {
                    self.refund_ante();
                    self.encounter = None;
                    for door in &mut self.encounter_rooms {
                        if door.0 == index {
                            door.1 = Kind::WaxMaze;
                        }
                    }
                    return self.start_encounter(index, from, Kind::WaxMaze);
                }
            }
        }
        self.found = notice;
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// Raise the hosted game itself.
    fn open_hosted(&mut self, index: usize, kind: Kind, seats: &[u32]) {
        match kind {
            Kind::Lanternward => self.start_lanternward(),
            Kind::Moonfall => self.start_moonfall(),
            Kind::Bannerpeak => self.start_bannerpeak(),
            Kind::Griffinwing => self.start_griffinwing(),
            Kind::Lightgallop => self.start_lightgallop(),
            Kind::KnightsWish => self.start_deathmatch(),
            Kind::KnightRacers => self.start_encounter_race(seats),
            Kind::ShaftRun | Kind::GriffinRun | Kind::TorchlightTouge => {
                if let Some(course) = kind.course() {
                    self.start_race3d_hosted(index, course, seats);
                }
            }
            // Its stage is raised on the first tick, after the hall's own
            // entry has run (`tick_encounter`).
            Kind::Arcade(_) => {}
            _ => {}
        }
    }

    /// An arcade game on its own stage: the hall is put by and the game's room
    /// stands in its place, every knight on its pad with a round of their own.
    fn start_arcade(&mut self, index: usize, game: Game, seats: &[u32]) {
        let cell = self.dungeon.rooms[index].cell;
        let mut stage = world::world_room(game.room(), cell);
        // A stage's own stairs and doors are no way out of the hall.
        for row in 0..stage.rows as i32 {
            for col in 0..stage.cols as i32 {
                match stage.tile(col, row) {
                    Tile::Stairs => stage.set(col as usize, row as usize, Tile::Floor),
                    Tile::Door => stage.set(col as usize, row as usize, Tile::Wall),
                    _ => {}
                }
            }
        }
        stage.doors = self.dungeon.rooms[index].doors;
        stage.visited = true;
        stage.cleared = true;
        let hall = std::mem::replace(&mut self.dungeon.rooms[index], stage);
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        if let Some(live) = &mut self.encounter {
            live.hall = Some(Box::new(hall));
        }
        // Side by side on the pad, as clear of stone as the stage allows.
        let (px, py) = game.start();
        let grid = Grid { room: &self.dungeon.rooms[index], barred: false };
        let mut spots = Vec::new();
        for k in 0..seats.len() {
            let want = ((px) * TILE_UNITS, (py + (k as f32 - (seats.len() as f32 - 1.0) / 2.0) * 0.9) * TILE_UNITS);
            let spot = if grid.clear(want.0, want.1, HERO_RADIUS, Mover::Hero) { want } else { (px * TILE_UNITS, py * TILE_UNITS) };
            spots.push(spot);
        }
        for (&id, &(x, y)) in seats.iter().zip(&spots) {
            if let Some(h) = self.players.get_mut(&id) {
                (h.x, h.y) = (x, y);
                h.dash_ticks = 0;
                h.invulnerable = h.invulnerable.max(HZ);
            }
        }
        self.start_encounter_pastime(game, seats, &spots);
    }

    /// The heat is judged (`pastimes::judge_heat`): who won their round, best
    /// first. A round nobody won is the room's loss.
    pub(super) fn heat_to_encounter(&mut self, heat: &pastimes::Heat) {
        let mut winners: Vec<u32> = heat
            .seats
            .iter()
            .copied()
            .filter(|id| heat.done.get(id).is_some_and(|f| f.points.is_some()))
            .collect();
        winners.sort_by(|a, b| {
            let score = |id: &u32| heat.done.get(id).map(|f| (f.points.unwrap_or(0), f.progress));
            score(b).cmp(&score(a)).then(a.cmp(b))
        });
        if let Some(live) = &mut self.encounter {
            live.fin = Some(winners);
        }
    }

    /// One lap of a cup course the seed and the floor pick, every knight on the
    /// grid; a lone knight races five rivals, two or more race each other.
    fn start_encounter_race(&mut self, seats: &[u32]) {
        let humans: Vec<(u32, String)> = seats
            .iter()
            .filter_map(|id| self.players.get(id).map(|h| (*id, h.name.clone())))
            .collect();
        let cup = knight_racers::cup();
        let course = cup[(mix(self.seed ^ u64::from(self.dungeon.depth).rotate_left(29) ^ self.at as u64) % cup.len() as u64) as usize];
        let mut s = knight_racers::Session::blank_on(course);
        knight_racers::flow::quick(&mut s, &humans, knight_racers::Mode::Single, knight_racers::Class::Knight, Some(course), 0);
        for (id, car) in s.cars.iter_mut() {
            if !car.cpu {
                car.pod = knight_racers::pods::seat_pod(*id);
            }
        }
        s.laps = 1;
        self.knight_racers = Some(s);
    }

    /// The race hall's course, staked by this module and not by the race.
    fn start_race3d_hosted(&mut self, index: usize, course: race3d::Course, seats: &[u32]) {
        let scores = seats.iter().map(|&id| (id, 0u32)).collect();
        self.open_race3d(index, course, BTreeMap::new(), scores, true);
    }

    /// Whether the hosted game's state is up (`live` is the game being asked
    /// about, which the caller may hold off the run for the moment).
    fn hosted_open(&self, kind: Kind, live: Option<&Live>) -> bool {
        match kind {
            Kind::Lanternward => self.lanternward.is_some(),
            Kind::Moonfall => self.moonfall.is_some(),
            Kind::Bannerpeak => self.bannerpeak.is_some(),
            Kind::Griffinwing => self.griffinwing.is_some(),
            Kind::Lightgallop => self.lightgallop.is_some(),
            Kind::KnightRacers => self.knight_racers.is_some(),
            Kind::KnightsWish => self.deathmatch.is_some(),
            Kind::ShaftRun | Kind::GriffinRun | Kind::TorchlightTouge => self.race3d.is_some(),
            Kind::Arcade(_) => {
                self.pastimes.heat.is_some() || live.is_some_and(|e| e.fin.is_some() || (e.hall.is_none() && e.t == 0))
            }
            _ => true,
        }
    }

    /// Put every knight's ante back in their pockets.
    fn refund_ante(&mut self) {
        let Some(live) = &mut self.encounter else {
            return;
        };
        for (id, gold) in std::mem::take(&mut live.ante) {
            if let Some(h) = self.players.get_mut(&id) {
                h.carried.add(Spoil::Gold, gold);
            }
        }
    }

    /// Hold every seated knight above zero: a knight never falls in a random
    /// room, and nothing here can end the run.
    pub(super) fn keep_hearts(&mut self) {
        let Some(live) = &self.encounter else {
            return;
        };
        if live.room != self.at {
            return;
        }
        for id in live.seats.clone() {
            if let Some(h) = self.players.get_mut(&id) {
                h.hp = h.hp.max(1);
            }
        }
    }

    /// The tick of the room's game, after the game's own: who has left, the
    /// clock, the end and the payout.
    pub(super) fn tick_encounter(&mut self, inputs: &BTreeMap<u32, Input>) {
        // An arcade game's stage is raised on the first tick, then its round
        // plays on it.
        if let Some(live) = &self.encounter
            && let Kind::Arcade(game) = live.kind
            && live.room == self.at
            && live.t == 0
            && live.hall.is_none()
            && live.end.is_none()
        {
            let (room, seats) = (live.room, live.seats.clone());
            self.start_arcade(room, game, &seats);
        }
        if self
            .encounter
            .as_ref()
            .is_some_and(|e| matches!(e.kind, Kind::Arcade(_)) && e.room == self.at && e.end.is_none() && e.fin.is_none())
        {
            self.tick_pastimes(inputs);
        }
        let Some(mut live) = self.encounter.take() else {
            return;
        };
        if live.room != self.at || live.room >= self.dungeon.rooms.len() {
            // The party has left the hall: its game is over.
            self.unstage(live);
            return;
        }
        live.t = live.t.saturating_add(1);
        live.seats.retain(|id| self.players.contains_key(id) && !self.exits.outside.contains_key(id));
        for id in live.seats.clone() {
            if let Some(h) = self.players.get_mut(&id) {
                h.hp = h.hp.max(1);
            }
        }
        if let Some(end) = &mut live.end {
            end.left = end.left.saturating_sub(1);
            if end.left == 0 {
                self.close_encounter(live);
            } else {
                self.encounter = Some(live);
            }
            return;
        }
        if live.kind.folly() {
            // A folly runs, stakes and pays itself: it is over when its state is.
            if self.folly_over(&live) {
                self.close_encounter(live);
            } else {
                self.encounter = Some(live);
            }
            return;
        }
        if live.seats.is_empty() {
            self.unstage(live);
            return;
        }
        if let Some((won, capped, order)) = self.encounter_result(&mut live) {
            self.settle_encounter(&mut live, won, capped, order);
        }
        self.encounter = Some(live);
    }

    /// Whether the folly the hall played is done.
    fn folly_over(&self, live: &Live) -> bool {
        let here = |room: usize| room == live.room;
        match live.kind {
            Kind::WaxMaze => self.folly.as_ref().is_none_or(|f| !here(f.room) || f.done),
            Kind::BelfryShoot => self.bats.as_ref().is_none_or(|b| !here(b.room) || b.done),
            Kind::PowderRoom => self.powder.as_ref().is_none_or(|p| !here(p.room) || p.done),
            Kind::BookHop => self.hop.as_ref().is_none_or(|h| !here(h.room) || h.done),
            _ => true,
        }
    }

    /// Whether a hosted game is over, how, and the knights in the order they
    /// are paid. `None` while it goes on.
    fn encounter_result(&self, live: &mut Live) -> Option<(bool, bool, Vec<u32>)> {
        let capped = live.t >= live.kind.cap();
        let seats = live.seats.clone();
        // A game whose state has gone (a restored save missing it) is over,
        // the stake lost.
        if !self.hosted_open(live.kind, Some(live)) {
            return Some((false, false, Vec::new()));
        }
        let all = |won: bool| Some((won, false, seats.clone()));
        match live.kind {
            Kind::Lanternward | Kind::Moonfall | Kind::Bannerpeak | Kind::Griffinwing | Kind::Lightgallop => {
                if self.objective_met(live.kind) {
                    return all(true);
                }
                capped.then(|| (false, true, Vec::new()))
            }
            Kind::KnightsWish => {
                let s = self.deathmatch.as_ref()?;
                if s.winner.is_some() || capped {
                    return Some((true, capped && s.winner.is_none(), standing(&s.order(), &seats)));
                }
                None
            }
            Kind::KnightRacers => {
                let s = self.knight_racers.as_ref()?;
                let home = s.finished || s.cars.values().filter(|car| !car.cpu).all(|car| car.finish.is_some());
                if home {
                    live.home += 1;
                }
                if live.home >= GRACE || capped {
                    return Some(ranked(&s.order(), &seats, capped && live.home < GRACE));
                }
                None
            }
            Kind::ShaftRun | Kind::GriffinRun | Kind::TorchlightTouge => {
                let r = self.race3d.as_ref()?;
                if r.done || capped {
                    return Some(ranked(&r.order(), &seats, capped && !r.done));
                }
                None
            }
            Kind::Arcade(_) => {
                if let Some(winners) = live.fin.clone() {
                    let winners = standing(&winners, &seats);
                    return Some((!winners.is_empty(), false, winners));
                }
                if capped {
                    // A long round (the siege) is cut: those who had won theirs are paid.
                    let heat = self.pastimes.heat.as_ref()?;
                    let mut won: Vec<(u32, u16, u8)> = heat
                        .done
                        .iter()
                        .filter_map(|(&id, f)| f.points.map(|p| (id, p, f.progress)))
                        .collect();
                    won.sort_by(|a, b| (b.1, b.2).cmp(&(a.1, a.2)).then(a.0.cmp(&b.0)));
                    let winners = standing(&won.iter().map(|w| w.0).collect::<Vec<_>>(), &seats);
                    return Some((!winners.is_empty(), true, winners));
                }
                None
            }
            _ => None,
        }
    }

    /// The first objective of a campaign: what a few minutes' play can win.
    pub fn objective_met(&self, kind: Kind) -> bool {
        match kind {
            // The first room's door taken to the second.
            Kind::Lanternward => self.lanternward.as_ref().is_some_and(|s| s.room >= 1 || s.done.is_some()),
            // The first favour done: a gate opened, the ink taken, the ditch filled.
            Kind::Moonfall => self.moonfall.as_ref().is_some_and(|s| {
                s.completed.is_some()
                    || s.world.has(moonfall::state::NORTH_OPEN)
                    || s.world.has(moonfall::state::SOUTH_OPEN)
                    || s.world.has(moonfall::state::INK_TAKEN)
                    || s.world.has(moonfall::state::DITCH_FILLED)
            }),
            // The first banner raised.
            Kind::Bannerpeak => self.bannerpeak.as_ref().is_some_and(|s| s.banners != 0 || s.completed.is_some()),
            // The first beat flown.
            Kind::Griffinwing => self.griffinwing.as_ref().is_some_and(|s| s.stage >= 1 || s.done.is_some()),
            // The first lantern post lit, or the first bell rung.
            Kind::Lightgallop => self.lightgallop.as_ref().is_some_and(|s| s.posts != 0 || s.bell_t.is_some()),
            _ => false,
        }
    }

    /// The game is settled: stakes, purse and wish paid, and the payout card
    /// up for `PAYOUT_TICKS`.
    fn settle_encounter(&mut self, live: &mut Live, won: bool, capped: bool, order: Vec<u32>) {
        let mut paid: BTreeMap<u32, u32> = BTreeMap::new();
        let co_op = live.kind.co_op();
        let pot: u32 = live.ante.values().sum();
        for (place, &id) in order.iter().enumerate() {
            if !live.seats.contains(&id) || !self.players.contains_key(&id) {
                continue;
            }
            // A game for the table pays everyone the first place's purse.
            let place = if co_op { 0 } else { place };
            let mut gold = crossroads::gold(place);
            let mut spoils = Spoils::default();
            spoils.add(Spoil::Gold, gold);
            self.tally_coin(&spoils);
            self.bank.push(Haul { hero: id, spoils, why: "encounter prize".into() });
            // The ante comes back to the winner of a game for the table, and
            // the pot goes to the first of a game between them.
            let back = if co_op {
                live.ante.get(&id).copied().unwrap_or(0)
            } else if place == 0 {
                pot
            } else {
                0
            };
            if back > 0 {
                if let Some(h) = self.players.get_mut(&id) {
                    h.carried.add(Spoil::Gold, back);
                }
                gold += back;
            }
            if let Some(h) = self.players.get_mut(&id) {
                h.laurel = place == 0;
                if place == 0 {
                    h.wishes = h.wishes.saturating_add(1).min(overclass::MOST_WISHES);
                }
            }
            paid.insert(id, gold);
        }
        let any = !paid.is_empty();
        // The card lists them in the order they were paid in.
        let order: Vec<u32> = order.into_iter().filter(|id| paid.contains_key(id)).collect();
        live.end = Some(End { won: won && any, capped, order, paid, left: PAYOUT_TICKS });
        self.sounds.push(if any { "gold" } else { "door_seal" });
        if any {
            self.cues.push("pastime_won".into());
        }
    }

    /// The payout card is down: the game ends, the room is cleared and its
    /// doors open, the knights as they came in.
    fn close_encounter(&mut self, live: Live) {
        let room = live.room;
        self.unstage(live);
        if let Some(r) = self.dungeon.rooms.get_mut(room) {
            r.cleared = true;
            r.visited = true;
        }
        if self.phase == Phase::Fighting && !self.foes_left() {
            self.phase = Phase::Exploring;
        }
        self.sounds.push("door_open");
    }

    /// End a game: its state cleared, the hall put back, and every knight's
    /// health and gear as they were at the door.
    pub(super) fn unstage(&mut self, mut live: Live) {
        match live.kind {
            Kind::WaxMaze | Kind::BelfryShoot | Kind::PowderRoom | Kind::BookHop => {}
            _ => {
                roundtable::clear(self);
                self.race3d = None;
                if matches!(live.kind, Kind::Arcade(_)) {
                    self.pastimes.leave();
                }
            }
        }
        let swapped = live.hall.is_some();
        if let Some(hall) = live.hall.take() {
            if let Some(slot) = self.dungeon.rooms.get_mut(live.room) {
                *slot = *hall;
            }
            self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        }
        for (id, kept) in &live.kept {
            let Some(h) = self.players.get_mut(id) else {
                continue;
            };
            h.hp = kept.hp.max(1);
            h.armor = kept.armor;
            h.gear = kept.gear;
            h.gear_was = kept.gear_was;
            if swapped {
                (h.x, h.y) = (kept.x as f32 / 16.0, kept.y as f32 / 16.0);
            }
        }
        self.encounter = None;
    }

    /// The party has left the hall (or the floor changed): end the game.
    pub(super) fn abandon_encounter(&mut self) {
        if let Some(live) = self.encounter.take() {
            self.unstage(live);
        }
    }
}

/// The knights of `seats` in `order`: the humans' running order, a rival's
/// seat dropped.
fn standing(order: &[u32], seats: &[u32]) -> Vec<u32> {
    order.iter().copied().filter(|id| seats.contains(id)).collect()
}

/// A race's result: the knights in order. A lone knight against rivals is
/// paid only for first place overall, as a win is.
fn ranked(order: &[u32], seats: &[u32], capped: bool) -> (bool, bool, Vec<u32>) {
    let humans = standing(order, seats);
    if seats.len() == 1 && order.first() != seats.first() {
        return (false, capped, Vec::new());
    }
    (true, capped, humans)
}

#[cfg(test)]
#[path = "../../tests/rules/encounter__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/encounter_reel__tests.rs"]
mod reel;

#[cfg(test)]
#[path = "../../tests/rules/encounter_arcade__tests.rs"]
mod arcade_tests;
