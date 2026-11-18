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

pub const TITLE_TICKS: u32 = 4 * HZ;
pub const PAYOUT_TICKS: u32 = 5 * HZ;
const LONGEST: u32 = 10 * 60 * HZ;
pub const SECOND_FROM: u32 = 2;
pub const SECOND_PERCENT: u64 = 40;

pub const CAP: u32 = 3 * 60 * HZ;
pub const GRACE: u32 = 2 * HZ;

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
    Arcade(Game),
}

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

    pub fn folly(self) -> bool {
        matches!(self, Kind::WaxMaze | Kind::BelfryShoot | Kind::PowderRoom | Kind::BookHop)
    }

    pub fn co_op(self) -> bool {
        matches!(
            self,
            Kind::Lanternward | Kind::Moonfall | Kind::Bannerpeak | Kind::Griffinwing | Kind::Lightgallop
        )
    }

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

    pub fn cut_by_cap(self) -> bool {
        self.co_op() || matches!(self, Kind::KnightRacers | Kind::KnightsWish)
    }

    pub fn least(self) -> usize {
        if self == Kind::KnightsWish { 2 } else { 1 }
    }

    pub fn course(self) -> Option<race3d::Course> {
        match self {
            Kind::ShaftRun => Some(race3d::Course::Shaft),
            Kind::GriffinRun => Some(race3d::Course::Griffin),
            Kind::TorchlightTouge => Some(race3d::Course::Touge),
            _ => None,
        }
    }

    pub fn cap(self) -> u32 {
        CAP
    }

    fn stand_in(self) -> Kind {
        match self {
            Kind::KnightsWish => Kind::KnightRacers,
            other => other,
        }
    }
}

pub fn legacy(depth: u32) -> Kind {
    match depth % 3 {
        1 => Kind::WaxMaze,
        2 => Kind::BelfryShoot,
        _ if depth % 6 == 0 => Kind::BookHop,
        _ => Kind::PowderRoom,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub hp: u32,
    pub armor: u32,
    pub gear: u8,
    pub gear_was: u8,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct End {
    pub won: bool,
    pub capped: bool,
    pub order: Vec<u32>,
    pub paid: BTreeMap<u32, u32>,
    pub left: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Live {
    pub room: usize,
    pub kind: Kind,
    pub t: u32,
    pub seats: Vec<u32>,
    pub ante: BTreeMap<u32, u32>,
    pub kept: BTreeMap<u32, Kept>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hall: Option<Box<Room>>,
    #[serde(default)]
    pub home: u32,
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

    pub fn titled(&self) -> bool {
        self.t < TITLE_TICKS
    }

    pub fn left(&self) -> u32 {
        self.kind.cap().saturating_sub(self.t).div_ceil(HZ)
    }
}

pub type Door = (usize, Kind);

#[cfg(any(test, feature = "testing"))]
impl Run {
    pub fn deal_for_test(&mut self, kind: Kind) -> Option<usize> {
        let index = self.folly_room?;
        self.encounter_rooms = vec![(index, kind)];
        Some(index)
    }

    pub fn deal_legacy_for_test(&mut self) -> Option<usize> {
        self.deal_for_test(legacy(self.dungeon.depth))
    }
}

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
        let solo: Vec<Kind> = pool.iter().copied().filter(|k| k.least() <= party).collect();
        return choose(&solo, mix(salt ^ 0x5010));
    }
    kind
}

pub fn pool() -> Vec<Kind> {
    Kind::ALL.into_iter().chain(ARCADE.map(Kind::Arcade)).collect()
}

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
    pub fn encounter_at(&self, index: usize) -> Option<Kind> {
        if let Some(&(_, kind)) = self.encounter_rooms.iter().find(|&&(room, _)| room == index) {
            return Some(kind);
        }
        (self.folly_room == Some(index)).then(|| legacy(self.dungeon.depth))
    }

    pub fn hosts_game(&self, index: usize) -> bool {
        [self.folly_room, self.gantry_room, self.race3d_room].contains(&Some(index))
            || self.encounter_rooms.iter().any(|&(room, _)| room == index)
    }

    pub(super) fn roll_encounters(&mut self) {
        self.encounter_prev = self.encounter_rooms.iter().map(|&(_, kind)| kind).collect();
        self.encounter_rooms.clear();
    }

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

    pub fn encounter_running(&self) -> bool {
        self.encounter.as_ref().is_some_and(|e| e.room == self.at)
    }

    pub fn encounter_hosts(&self) -> bool {
        self.encounter.as_ref().is_some_and(|e| e.room == self.at && !e.kind.folly())
    }

    pub fn on_stage(&self) -> bool {
        self.room().kind == RoomKind::Forest || self.encounter_hosts()
    }

    pub fn host_pays(&self) -> bool {
        self.crossroads.is_some() || self.encounter_hosts()
    }

    pub(super) fn start_encounter(&mut self, index: usize, from: Option<usize>, dealt: Kind) {
        let seats: Vec<u32> = self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && !h.absent && !h.away)
            .map(|(&id, _)| id)
            .collect();
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
            Kind::Arcade(_) => {}
            _ => {}
        }
    }

    fn start_arcade(&mut self, index: usize, game: Game, seats: &[u32]) {
        let cell = self.dungeon.rooms[index].cell;
        let mut stage = world::world_room(game.room(), cell);
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

    fn start_race3d_hosted(&mut self, index: usize, course: race3d::Course, seats: &[u32]) {
        let scores = seats.iter().map(|&id| (id, 0u32)).collect();
        self.open_race3d(index, course, BTreeMap::new(), scores, true);
    }

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

    pub(super) fn tick_encounter(&mut self, inputs: &BTreeMap<u32, Input>) {
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

    fn encounter_result(&self, live: &mut Live) -> Option<(bool, bool, Vec<u32>)> {
        let capped = live.t >= live.kind.cap();
        let seats = live.seats.clone();
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

    pub fn objective_met(&self, kind: Kind) -> bool {
        match kind {
            Kind::Lanternward => self.lanternward.as_ref().is_some_and(|s| s.room >= 1 || s.done.is_some()),
            Kind::Moonfall => self.moonfall.as_ref().is_some_and(|s| {
                s.completed.is_some()
                    || s.world.has(moonfall::state::NORTH_OPEN)
                    || s.world.has(moonfall::state::SOUTH_OPEN)
                    || s.world.has(moonfall::state::INK_TAKEN)
                    || s.world.has(moonfall::state::DITCH_FILLED)
            }),
            Kind::Bannerpeak => self.bannerpeak.as_ref().is_some_and(|s| s.banners != 0 || s.completed.is_some()),
            Kind::Griffinwing => self.griffinwing.as_ref().is_some_and(|s| s.stage >= 1 || s.done.is_some()),
            Kind::Lightgallop => self.lightgallop.as_ref().is_some_and(|s| s.posts != 0 || s.bell_t.is_some()),
            _ => false,
        }
    }

    fn settle_encounter(&mut self, live: &mut Live, won: bool, capped: bool, order: Vec<u32>) {
        let mut paid: BTreeMap<u32, u32> = BTreeMap::new();
        let co_op = live.kind.co_op();
        let pot: u32 = live.ante.values().sum();
        for (place, &id) in order.iter().enumerate() {
            if !live.seats.contains(&id) || !self.players.contains_key(&id) {
                continue;
            }
            let place = if co_op { 0 } else { place };
            let mut gold = crossroads::gold(place);
            let mut spoils = Spoils::default();
            spoils.add(Spoil::Gold, gold);
            self.tally_coin(&spoils);
            self.bank.push(Haul { hero: id, spoils, why: "encounter prize".into() });
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
        let order: Vec<u32> = order.into_iter().filter(|id| paid.contains_key(id)).collect();
        live.end = Some(End { won: won && any, capped, order, paid, left: PAYOUT_TICKS });
        self.sounds.push(if any { "gold" } else { "door_seal" });
        if any {
            self.cues.push("pastime_won".into());
        }
    }

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

    pub(super) fn abandon_encounter(&mut self) {
        if let Some(live) = self.encounter.take() {
            self.unstage(live);
        }
    }
}

fn standing(order: &[u32], seats: &[u32]) -> Vec<u32> {
    order.iter().copied().filter(|id| seats.contains(id)).collect()
}

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
