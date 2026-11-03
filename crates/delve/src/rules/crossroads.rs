//! Three cards between every third floor and the next. The ballot's serials
//! carry each seat's choice; only the host deals, judges and pays the round.
use super::*;
use super::votes::{Action, Command};

pub const VOTE_TIME: u32 = 20 * HZ;
pub const RESULTS_TIME: u32 = 5 * HZ;
pub const DEATHMATCH_TIME: u32 = 3 * 60 * HZ;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Game { Powder, Gander, Shaft, Griffin, Touge, Deathmatch }

impl Game {
    pub const ALL: [Self; 6] = [Self::Powder, Self::Gander, Self::Shaft, Self::Griffin, Self::Touge, Self::Deathmatch];
    pub fn name(self) -> &'static str {
        match self {
            Self::Powder => "POWDER KEG",
            Self::Gander => "GANDER JOUST",
            Self::Shaft => "SHAFT RUN",
            Self::Griffin => "GRIFFIN RUN",
            Self::Touge => "TORCHLIGHT TOUGE",
            Self::Deathmatch => "KNIGHTS WISH",
        }
    }
    pub fn words(self) -> &'static str {
        match self {
            Self::Powder => "DROP DODGE OUTLAST",
            Self::Gander => "FLAP CLIMB STOMP",
            Self::Shaft => "ROLL DON'T FALL",
            Self::Griffin => "FLY AND SHOOT",
            Self::Touge => "DRIFT THEN BOOST",
            Self::Deathmatch => "TEN KILLS WINS",
        }
    }
    pub fn course(self) -> Option<race3d::Course> {
        match self {
            Self::Shaft => Some(race3d::Course::Shaft),
            Self::Griffin => Some(race3d::Course::Griffin),
            Self::Touge => Some(race3d::Course::Touge),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage { Voting, Playing, Results }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub depth: u32,
    pub pack: Pack,
    pub choices: [Game; 3],
    pub seats: Vec<u32>,
    pub votes: BTreeMap<u32, u8>,
    pub stage: Stage,
    pub left: u32,
    pub chosen: Option<u8>,
    pub order: Vec<u32>,
}

impl State {
    pub fn voting(&self) -> bool { self.stage == Stage::Voting }
    pub fn counts(&self) -> [usize; 3] {
        std::array::from_fn(|i| self.votes.values().filter(|&&v| usize::from(v) == i).count())
    }
    /// Most votes wins; equal counts keep the earlier card, even at zero.
    pub fn winner(&self) -> usize {
        let counts = self.counts();
        (0..3).max_by_key(|&i| (counts[i], 3 - i)).unwrap_or(0)
    }
    pub fn valid(&self) -> bool {
        (1..=DEEPEST).contains(&self.depth)
            && self.choices.iter().enumerate().all(|(i, g)| !self.choices[..i].contains(g))
            && self.seats.len() <= MAX_PLAYERS as usize
            && self.seats.iter().enumerate().all(|(i, id)| (1..=MAX_PLAYERS).contains(id) && !self.seats[..i].contains(id))
            && self.votes.iter().all(|(id, &v)| self.seats.contains(id) && v < 3)
            && self.chosen.is_none_or(|v| v < 3)
            && (self.voting() == self.chosen.is_none())
            && self.order.iter().enumerate().all(|(i, id)| self.seats.contains(id) && !self.order[..i].contains(id))
            && self.left <= DEATHMATCH_TIME.max(VOTE_TIME).max(RESULTS_TIME)
    }
}

pub fn gold(place: usize) -> u32 {
    match place { 0 => 100, 1 => 60, _ => 30 }
}

impl Run {
    pub fn crossroads_voting(&self) -> bool {
        self.crossroads.as_ref().is_some_and(State::voting)
    }

    pub(super) fn begin_crossroads(&mut self, depth: u32, pack: Pack) {
        let seats: Vec<u32> = self.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
        let mut pool: Vec<Game> = Game::ALL.into_iter()
            .filter(|g| *g != Game::Deathmatch || seats.len() >= 2).collect();
        // A separate seed keeps dealing cards from changing the next floor.
        let mut rng = Rng::new(mix(self.seed ^ self.raid_id.rotate_left(17) ^ u64::from(self.dungeon.depth)));
        for i in (1..pool.len()).rev() {
            let j = (rng.next() % (i + 1) as u64) as usize;
            pool.swap(i, j);
        }
        for (&id, hero) in &self.players {
            self.playtest.arrive(id, &hero.name);
            let seat = self.playtest.seats.get_mut(&id).unwrap();
            seat.open = false;
            seat.selected = 0;
            seat.hint = 0;
        }
        self.floor_plates.clear();
        self.hitstop = Default::default();
        self.fellowship.clear();
        self.crossroads = Some(State {
            depth, pack, choices: [pool[0], pool[1], pool[2]], seats,
            votes: BTreeMap::new(), stage: Stage::Voting, left: VOTE_TIME,
            chosen: None, order: Vec::new(),
        });
        self.sounds.push("card_pickup");
    }

    pub(super) fn crossroads_command(&mut self, id: u32, command: Command) {
        if !command.valid() || !self.crossroads.as_ref().is_some_and(|c| c.voting() && c.seats.contains(&id)) {
            return;
        }
        let Some(hero) = self.players.get(&id) else { return; };
        if hero.absent || self.away(id) { return; }
        self.playtest.arrive(id, &hero.name);
        let seat = self.playtest.seats.get_mut(&id).unwrap();
        if command.serial <= seat.serial { return; }
        seat.serial = command.serial;
        seat.selected = seat.selected.min(2);
        let c = self.crossroads.as_mut().unwrap();
        match command.action {
            Action::Rate(n @ 1..=3) => {
                seat.selected = usize::from(n - 1);
                c.votes.insert(id, n - 1);
            }
            Action::Left | Action::Up => seat.selected = (seat.selected + 2) % 3,
            Action::Right | Action::Down => seat.selected = (seat.selected + 1) % 3,
            Action::Clear => { c.votes.remove(&id); }
            _ => {}
        }
    }

    pub(super) fn tick_crossroads(&mut self, inputs: &BTreeMap<u32, Input>) {
        self.tick += 1;
        self.hitstop = Default::default();
        match self.crossroads.as_ref().unwrap().stage {
            Stage::Voting => {
                let seats: Vec<u32> = self.players.iter().filter(|(_, h)| !h.absent).map(|(&id, _)| id).collect();
                let c = self.crossroads.as_mut().unwrap();
                c.seats = seats;
                c.votes.retain(|id, _| c.seats.contains(id));
                for (&id, input) in inputs.iter().filter(|(_, i)| i.valid()) {
                    if let Some(command) = input.vote { self.vote_command(id, command); }
                }
                let pending = self.crossroads.as_ref().unwrap().seats.iter().any(|id| {
                    self.players.get(id).is_some_and(|h| !h.absent) && !self.away(*id)
                        && !self.crossroads.as_ref().unwrap().votes.contains_key(id)
                });
                let c = self.crossroads.as_mut().unwrap();
                c.left = c.left.saturating_sub(1);
                if !pending || c.left == 0 { self.launch_crossroads(); }
            }
            Stage::Playing => {
                let game = self.crossroads.as_ref().unwrap().choices[usize::from(self.crossroads.as_ref().unwrap().chosen.unwrap())];
                if game == Game::Deathmatch {
                    self.tick_deathmatch(inputs);
                    let c = self.crossroads.as_mut().unwrap();
                    c.left = c.left.saturating_sub(1);
                    if let Some(s) = &self.deathmatch {
                        if s.winner.is_some() || c.left == 0 {
                            let order = s.order();
                            self.finish_crossroads(order);
                        }
                    } else {
                        // A restored interlude missing its arena still ends cleanly.
                        self.finish_crossroads(Vec::new());
                    }
                } else if game.course().is_some() {
                    self.tick_race3d(inputs);
                    if let Some(r) = self.race3d.as_ref().filter(|r| r.done) {
                        self.finish_crossroads(r.order());
                    }
                } else {
                    self.tick_pastimes(inputs);
                    if self.crossroads.as_ref().is_some_and(|c| c.stage == Stage::Playing) && self.pastimes.heat.is_none() {
                        self.finish_crossroads(Vec::new());
                    }
                }
            }
            Stage::Results => {
                let c = self.crossroads.as_mut().unwrap();
                c.left = c.left.saturating_sub(1);
                if c.left == 0 {
                    let c = self.crossroads.take().unwrap();
                    self.pastimes.leave();
                    self.race3d = None;
                    self.finish_descent(c.depth, c.pack, false);
                }
            }
        }
    }

    fn launch_crossroads(&mut self) {
        let c = self.crossroads.as_mut().unwrap();
        c.seats.retain(|id| self.players.get(id).is_some_and(|h| !h.absent));
        let mut chosen = c.winner();
        if c.choices[chosen] == Game::Deathmatch && c.seats.len() < 2 {
            // A seat may leave after the deal. Use the leftmost playable card.
            chosen = c.choices.iter().position(|g| *g != Game::Deathmatch).unwrap();
        }
        c.chosen = Some(chosen as u8);
        c.stage = Stage::Playing;
        let game = c.choices[chosen];
        c.left = if game == Game::Deathmatch { DEATHMATCH_TIME } else { 0 };
        self.pastimes.leave();
        self.race3d = None;
        self.deathmatch = None;
        self.folly = None;
        self.bats = None;
        self.powder = None;
        self.hop = None;
        self.gantry = None;
        self.joust = None;
        self.chariot = None;
        self.window = None;
        self.light = None;
        self.rune = None;
        self.enemies.clear();
        self.projectiles.clear();
        self.traps.clear();
        self.rocks.clear();
        self.gore.clear();
        self.spectacle.corpses.clear();
        if game == Game::Deathmatch {
            let players = self.players.iter().filter(|(id, _)| self.crossroads.as_ref().unwrap().seats.contains(id))
                .map(|(&id, h)| (id, h.clone())).collect();
            self.deathmatch = Some(deathmatch::Session::new(&players, &self.book));
        } else if let Some(course) = game.course() {
            self.start_race3d_course(self.at, course);
        } else {
            let game = if game == Game::Powder { pastimes::Game::Powder } else { pastimes::Game::Gander };
            let cell = self.room().cell;
            self.dungeon.rooms[self.at] = colosseum::room(game.room(), cell);
            self.dungeon.rooms[self.at].cleared = true;
            self.start_crossroads_pastime(game);
        }
        if self.crossroads.as_ref().unwrap().seats.is_empty() { self.finish_crossroads(Vec::new()); }
    }

    pub(super) fn finish_crossroads(&mut self, mut order: Vec<u32>) {
        let Some(c) = self.crossroads.as_mut().filter(|c| c.stage == Stage::Playing) else { return; };
        order.retain(|id| c.seats.contains(id) && self.players.contains_key(id));
        c.stage = Stage::Results;
        c.left = RESULTS_TIME;
        c.order = order;
        self.deathmatch = None;
        for (place, &id) in c.order.iter().enumerate() {
            let mut spoils = Spoils::default();
            spoils.add(Spoil::Gold, gold(place));
            self.bank.push(Haul { hero: id, spoils, why: "crossroads prize".into() });
            if let Some(h) = self.players.get_mut(&id) {
                h.laurel = place == 0;
                if place == 0 { h.wishes = h.wishes.saturating_add(1).min(overclass::MOST_WISHES); }
            }
        }
        self.sounds.push("gold");
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_crossroads__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/together_crossroads__fixtures.rs"]
pub(super) mod testing;
