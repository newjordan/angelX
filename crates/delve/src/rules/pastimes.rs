//! The kingdom's tiny arcade: model-free, host-authoritative and co-operative.
//! Medieval homages, not imported characters or assets. Feet, F and a clock
//! are enough. Rounds pay score only; never an infinite treasury gold faucet.

use super::*;
use std::collections::BTreeSet;
mod cart;
mod duckshoot;
mod put;
mod surf;
mod volley;
mod halfpipe;
mod chariot;
mod trencher;
mod longbow;
pub use longbow::{Longbow, ARROWS as LONGBOW_ARROWS, FULL_DRAW as LONGBOW_FULL_DRAW, FLIGHT as LONGBOW_FLIGHT};
pub use trencher::{Trencher, FLIGHT as TRENCHER_FLIGHT};
pub use chariot::{Chariot, LAP, RIVALS, BARRELS};
pub use halfpipe::{Halfpipe, LIP_OPEN, LIP_CLOSE, FLIGHT_TICKS};
pub use cart::{CART_AIR, CART_LOGS, CART_RUTS, Cart};
pub use duckshoot::{DuckShoot, EYE_Z, FOCAL, RAIL_Y, RAIL_Z};
pub use put::{Put, TARGETS};
pub use surf::{CREST_WINDOW, Surf, WAVE_LEANS, WAVE_TICKS};
pub use volley::{BELL_X, PADDLE_REACH, Volley};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Game {
    Broadsheet,
    Barrel,
    Turnip,
    Washtub,
    Cheese,
    Hobbyhorse,
    Sled,
    Pot,
    Cart,
    Surf,
    Volley,
    Halfpipe,
    Chariot,
    Trencher,
    DuckShoot,
    Longbow,
}

impl Game {
    pub const ALL: [Self; 16] = [
        Self::Broadsheet,
        Self::Barrel,
        Self::Turnip,
        Self::Washtub,
        Self::Cheese,
        Self::Hobbyhorse,
        Self::Sled,
        Self::Pot,
        Self::Cart,
        Self::Surf,
        Self::Volley,
        Self::Halfpipe,
        Self::Chariot,
        Self::Trencher,
        Self::DuckShoot,
        Self::Longbow,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|g| *g == self).unwrap()
    }
    pub fn room(self) -> RoomKind {
        match self {
            Self::Broadsheet => RoomKind::Village,
            Self::Barrel | Self::Turnip => RoomKind::Fairground,
            Self::Washtub => RoomKind::Riverbank,
            Self::Cheese => RoomKind::Downs,
            Self::Hobbyhorse => RoomKind::Causeway,
            Self::Sled => RoomKind::Crag,
            Self::Pot => RoomKind::Strand,
            Self::Cart => RoomKind::Timberyard,
            Self::Surf => RoomKind::Millpond,
            Self::Volley => RoomKind::Belfry,
            Self::Halfpipe => RoomKind::Mummers,
            Self::Chariot => RoomKind::Hippodrome,
            Self::Trencher => RoomKind::GooseGreen,
            Self::DuckShoot => RoomKind::Fairground,
            Self::Longbow => RoomKind::LongbowGreen,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Broadsheet => "BROADSHEET BOY",
            Self::Barrel => "BARREL-BOARD DERBY",
            Self::Turnip => "TURNIP KEEPY-UPPY",
            Self::Washtub => "WASHTUB REGATTA",
            Self::Cheese => "CHEESE CHUCK",
            Self::Hobbyhorse => "HOBBYHORSE HURDLES",
            Self::Sled => "SQUIRE SLED SLALOM",
            Self::Pot => "CHAMBER-POT PUT",
            Self::Cart => "WHEELBARROW CROSS",
            Self::Surf => "SHIELD SURF",
            Self::Volley => "BELFRY BAT-AND-BALL",
            Self::Halfpipe => "JESTER'S HALF-PIPE",
            Self::Chariot => "CABBAGE AND ROLL RACING",
            Self::Trencher => "TRENCHER TOSS",
            Self::DuckShoot => "CARNIVAL DUCK RANGE",
            Self::Longbow => "POPINJAY SHOOT",
        }
    }
    pub fn start(self) -> (f32, f32) {
        match self {
            Self::Broadsheet => (3.0, 7.0),
            Self::Barrel => (4.0, 9.0),
            Self::Turnip => (19.0, 10.0),
            Self::Washtub => (4.0, 10.0),
            Self::Cheese => (4.0, 6.0),
            Self::Hobbyhorse => (3.0, 7.0),
            Self::Sled => (3.0, 7.0),
            Self::Pot => (4.0, 7.0),
            Self::Cart => (3.0, 7.0),
            Self::Surf => (3.0, 9.0),
            Self::Volley => (3.0, 9.0),
            Self::Halfpipe => (3.0, 9.0),
            Self::Chariot => (3.0, 9.0),
            Self::Trencher => (3.0, 9.0),
            Self::DuckShoot => (11.0, 4.0),
            Self::Longbow => (3.0, 9.0),
        }
    }
    pub fn time(self) -> u32 {
        match self {
            Self::Broadsheet => 35 * HZ,
            Self::Barrel => 25 * HZ,
            Self::Turnip => 20 * HZ,
            Self::Washtub => 30 * HZ,
            Self::Cheese => 40 * HZ,
            Self::Hobbyhorse => 35 * HZ,
            Self::Sled => 45 * HZ,
            Self::Pot => 45 * HZ,
            Self::Cart => 40 * HZ,
            Self::Surf => 20 * HZ,
            Self::Volley => 40 * HZ,
            Self::Halfpipe => 30 * HZ,
            Self::Chariot => 35 * HZ,
            Self::Trencher => 30 * HZ,
            Self::DuckShoot => 40 * HZ,
            Self::Longbow => 40 * HZ,
        }
    }
    pub fn need(self) -> u8 {
        match self {
            Self::Broadsheet => 3,
            Self::Barrel | Self::Washtub => 6,
            Self::Turnip => 8,
            Self::Cheese => 5,
            Self::Hobbyhorse => 6,
            Self::Sled => 6,
            Self::Pot => 3,
            Self::Cart => 4,
            Self::Surf => 6,
            Self::Volley => 4,
            Self::Halfpipe => 4,
            Self::Chariot => 3,
            Self::Trencher => 3,
            Self::DuckShoot => 10,
            Self::Longbow => 3,
        }
    }
}

/// Separate records keep old four-element JSON arrays readable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Score {
    pub best: u16,
    pub wins: u32,
    pub plays: u32,
}
impl Score {
    fn valid(&self) -> bool {
        self.best <= 1000 && self.wins <= self.plays
    }
    fn repair(&mut self) {
        self.best = self.best.min(1000);
        self.plays = self.plays.max(self.wins);
    }
    fn merge(&mut self, other: &Self) {
        self.best = self.best.max(other.best);
        self.wins = self.wins.max(other.wins);
        self.plays = self.plays.max(other.plays);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Records {
    pub best: [u16; 4],
    pub wins: [u32; 4],
    pub plays: [u32; 4],
    pub cheese: Score,
    pub hobbyhorse: Score,
    pub sled: Score,
    pub pot: Score,
    pub cart: Score,
    pub surf: Score,
    pub volley: Score,
    pub halfpipe: Score,
    pub chariot: Score,
    pub trencher: Score,
    pub duckshoot: Score,
    pub longbow: Score,
}
impl Records {
    pub fn valid(&self) -> bool {
        self.best.iter().all(|&n| n <= 1000)
            && (0..4).all(|i| self.wins[i] <= self.plays[i])
            && self.cheese.valid()
            && self.hobbyhorse.valid()
            && self.sled.valid()
            && self.pot.valid()
            && self.cart.valid()
            && self.surf.valid()
            && self.volley.valid()
            && self.halfpipe.valid()
            && self.chariot.valid()
            && self.trencher.valid()
            && self.duckshoot.valid()
            && self.longbow.valid()
    }
    /// Mend records a bad save broke: bests capped, plays at least wins.
    pub fn repair(&mut self) {
        for i in 0..4 {
            self.best[i] = self.best[i].min(1000);
            self.plays[i] = self.plays[i].max(self.wins[i]);
        }
        for score in [
            &mut self.cheese,
            &mut self.hobbyhorse,
            &mut self.sled,
            &mut self.pot,
            &mut self.cart,
            &mut self.surf,
            &mut self.volley,
            &mut self.halfpipe,
            &mut self.chariot,
            &mut self.trencher,
            &mut self.duckshoot,
            &mut self.longbow,
        ] {
            score.repair();
        }
    }
    pub fn best_for(&self, game: Game) -> u16 {
        if game == Game::Trencher {
            self.trencher.best
        } else if game == Game::Chariot {
            self.chariot.best
        } else if game == Game::Halfpipe {
            self.halfpipe.best
        } else if game == Game::Volley {
            self.volley.best
        } else if game == Game::Surf {
            self.surf.best
        } else if game == Game::Cheese {
            self.cheese.best
        } else if game == Game::Hobbyhorse {
            self.hobbyhorse.best
        } else if game == Game::Sled {
            self.sled.best
        } else if game == Game::Cart {
            self.cart.best
        } else if game == Game::Pot {
            self.pot.best
        } else if game == Game::DuckShoot {
            self.duckshoot.best
        } else if game == Game::Longbow {
            self.longbow.best
        } else {
            self.best[game.index()]
        }
    }
    pub fn plays_for(&self, game: Game) -> u32 {
        if game == Game::Trencher {
            self.trencher.plays
        } else if game == Game::Chariot {
            self.chariot.plays
        } else if game == Game::Halfpipe {
            self.halfpipe.plays
        } else if game == Game::Volley {
            self.volley.plays
        } else if game == Game::Surf {
            self.surf.plays
        } else if game == Game::Cheese {
            self.cheese.plays
        } else if game == Game::Hobbyhorse {
            self.hobbyhorse.plays
        } else if game == Game::Sled {
            self.sled.plays
        } else if game == Game::Cart {
            self.cart.plays
        } else if game == Game::Pot {
            self.pot.plays
        } else if game == Game::DuckShoot {
            self.duckshoot.plays
        } else if game == Game::Longbow {
            self.longbow.plays
        } else {
            self.plays[game.index()]
        }
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn wins_for(&self, game: Game) -> u32 {
        if game == Game::Trencher {
            self.trencher.wins
        } else if game == Game::Chariot {
            self.chariot.wins
        } else if game == Game::Halfpipe {
            self.halfpipe.wins
        } else if game == Game::Volley {
            self.volley.wins
        } else if game == Game::Surf {
            self.surf.wins
        } else if game == Game::Cheese {
            self.cheese.wins
        } else if game == Game::Hobbyhorse {
            self.hobbyhorse.wins
        } else if game == Game::Sled {
            self.sled.wins
        } else if game == Game::Cart {
            self.cart.wins
        } else if game == Game::Pot {
            self.pot.wins
        } else if game == Game::DuckShoot {
            self.duckshoot.wins
        } else if game == Game::Longbow {
            self.longbow.wins
        } else {
            self.wins[game.index()]
        }
    }
    /// Every round the realm has played, named games and the first four
    /// alike — Humphrey wakes once the festival is properly underway.
    pub fn plays_total(&self) -> u32 {
        let named: u32 = [
            &self.cheese,
            &self.hobbyhorse,
            &self.sled,
            &self.pot,
            &self.cart,
            &self.surf,
            &self.volley,
            &self.halfpipe,
            &self.chariot,
            &self.trencher,
            &self.longbow,
        ]
        .iter()
        .map(|s| s.plays)
        .sum();
        named + self.plays.iter().copied().sum::<u32>()
    }

    /// Every round the realm has finished in victory; a festival with no
    /// wins is not yet worth a grudge.
    pub fn wins_total(&self) -> u32 {
        let named: u32 = [
            &self.cheese,
            &self.hobbyhorse,
            &self.sled,
            &self.pot,
            &self.cart,
            &self.surf,
            &self.volley,
            &self.halfpipe,
            &self.chariot,
            &self.trencher,
            &self.longbow,
        ]
        .iter()
        .map(|s| s.wins)
        .sum();
        named + self.wins.iter().copied().sum::<u32>()
    }

    pub fn merge(&mut self, other: &Self) -> bool {
        let before = self.clone();
        if other.valid() {
            self.cheese.merge(&other.cheese);
            self.hobbyhorse.merge(&other.hobbyhorse);
            self.sled.merge(&other.sled);
            self.pot.merge(&other.pot);
            self.cart.merge(&other.cart);
            self.surf.merge(&other.surf);
            self.volley.merge(&other.volley);
            self.halfpipe.merge(&other.halfpipe);
            self.chariot.merge(&other.chariot);
            self.trencher.merge(&other.trencher);
            self.duckshoot.merge(&other.duckshoot);
            self.longbow.merge(&other.longbow);
            for i in 0..4 {
                self.best[i] = self.best[i].max(other.best[i]);
                self.plays[i] = self.plays[i].max(other.plays[i]);
                self.wins[i] = self.wins[i].max(other.wins[i]);
            }
        }
        *self != before
    }
}

/// A single host flight, in 1/256 tile units: mirrors stay Eq and bounded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cheese {
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    pub left: u32,
}
impl Cheese {
    fn valid(&self) -> bool {
        (0..layout::COLS as i32 * 256).contains(&self.x)
            && (0..layout::ROWS as i32 * 256).contains(&self.y)
            && self.dx.unsigned_abs() <= 108
            && self.dy.unsigned_abs() <= 108
            && self.dx.unsigned_abs() + self.dy.unsigned_abs() > 0
            && self.left > 0
            && self.left <= 2 * HZ
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    pub game: Game,
    pub left: u32,
    pub progress: u8,
    /// Mailboxes already attempted. Wrong deliveries cannot be spammed either.
    pub tried: u8,
    pub recipients: u8,
    pub mistakes: u8,
    pub last_beat: Option<u32>,
    pub hazard_left: u32,
    #[serde(default)]
    pub cheese: Option<Cheese>,
    #[serde(default)]
    pub shots: u8,
    #[serde(default)]
    pub put: Option<Put>,
    #[serde(default)]
    pub cart: Option<Cart>,
    #[serde(default)]
    pub surf: Option<Surf>,
    #[serde(default)]
    pub volley: Option<Volley>,
    #[serde(default)]
    pub halfpipe: Option<Halfpipe>,
    #[serde(default)]
    pub chariot: Option<Chariot>,
    #[serde(default)]
    pub trencher: Option<Trencher>,
    #[serde(default)]
    pub duckshoot: Option<DuckShoot>,
    #[serde(default)]
    pub longbow: Option<Longbow>,
    /// Each rider jumps only for their own key edge, never the party's OR.
    #[serde(default)]
    pub riders: BTreeMap<u32, Rider>,
    #[serde(default)]
    pub sled: Option<Sled>,
}

/// A volunteer rides a shield, while the controlling knight stays on foot.
/// Fixed-point coordinates/velocity keep saves and mirrors bounded and Eq.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sled {
    pub driver: u32,
    pub x: i32,
    pub y: i32,
    pub dy: i32,
}
impl Sled {
    fn valid(&self) -> bool {
        (1..=super::MAX_PLAYERS).contains(&self.driver)
            && (3 * 256..=21 * 256).contains(&self.x)
            && (4 * 256..=10 * 256).contains(&self.y)
            && self.dy.unsigned_abs() <= 32
    }
}
pub const SLED_GATES: [(i32, i32); 6] = [(6, 5), (9, 9), (12, 5), (15, 9), (18, 5), (21, 9)];

// At normal walking pace the next fence is ~14 ticks away. A short jump
// and 12-tick recovery permit a continuous rhythm, without held-key hopping.
pub const JUMP_TICKS: u32 = 10;
pub const HURDLES: [f32; 6] = [6.0, 9.0, 12.0, 15.0, 18.0, 21.0];
pub const RIDING_LANE: (f32, f32) = (5.5, 8.5);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rider {
    pub x: i32,
    pub y: i32,
    pub air_left: u32,
    pub rest_left: u32,
}
impl Rider {
    fn valid(&self) -> bool {
        (0..layout::COLS as i32 * 256).contains(&self.x)
            && (0..layout::ROWS as i32 * 256).contains(&self.y)
            && self.air_left <= JUMP_TICKS
            && self.rest_left <= JUMP_TICKS + 2
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub round: Option<Round>,
    pub wager: Option<super::race_wagers::Heat>,
    pub wager_pick: u8,
    pub wager_key: bool,
    pub wager_owner: Option<u32>,
    pads: u32,
    fire: BTreeSet<u32>,
    /// Everyone plays at once: each knight in the room when a game starts
    /// gets a round of their own. The knight who started it plays `round`;
    /// the others play here, by seat, and each sees their own on screen
    /// (`Run::view_for_seat`).
    pub heats: BTreeMap<u32, Round>,
    /// Who is in the running and how each finished, until all have.
    pub heat: Option<Heat>,
}

/// One game played by everyone in the room at once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heat {
    pub game: Game,
    /// The seats in the running, the starter first.
    pub seats: Vec<u32>,
    /// How each seat finished: progress made, and points if it was won.
    pub done: BTreeMap<u32, Finish>,
    /// How far each seat has got, live, for the standings board.
    #[serde(default)]
    pub standing: BTreeMap<u32, u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finish {
    pub progress: u8,
    pub points: Option<u16>,
}

impl Heat {
    fn valid(&self) -> bool {
        !self.seats.is_empty()
            && self.seats.len() <= MAX_PLAYERS as usize
            && self.seats.iter().all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.done.keys().all(|id| self.seats.contains(id))
            && self.standing.keys().all(|id| self.seats.contains(id))
            && self.standing.values().all(|&p| p <= self.game.need())
            && self.done.values().all(|f| f.progress <= self.game.need() && f.points.is_none_or(|p| p <= 1000))
    }
    /// The seat that did best: a win beats none, then points, then
    /// progress; a dead heat goes to the earlier seat.
    pub fn winner(&self) -> Option<u32> {
        self.seats
            .iter()
            .filter_map(|id| self.done.get(id).map(|f| (id, f)))
            .max_by(|(a, fa), (b, fb)| {
                (fa.points.is_some(), fa.points.unwrap_or(0), fa.progress)
                    .cmp(&(fb.points.is_some(), fb.points.unwrap_or(0), fb.progress))
                    .then_with(|| {
                        let at = |id: &u32| self.seats.iter().position(|s| s == id).unwrap_or(0);
                        at(b).cmp(&at(a))
                    })
            })
            .map(|(id, _)| *id)
    }
}

/// What one tick of a round came to.
enum Played {
    Going(Round),
    Over(Finish),
}
impl State {
    pub fn valid(&self) -> bool {
        self.wager_pick < 3 && self.wager.as_ref().is_none_or(|h| h.valid())
            && self.wager_owner.is_none_or(|id| (1..=MAX_PLAYERS).contains(&id))
            && (self.wager.is_none() || self.round.is_none())
            && self.pads & !((1u32 << Game::ALL.len()) - 1) == 0
            && self.fire.len() <= MAX_PLAYERS as usize
            && self.round.as_ref().is_none_or(round_valid)
            && self.heats.len() < MAX_PLAYERS as usize
            && self.heats.iter().all(|(&id, r)| (1..=MAX_PLAYERS).contains(&id) && round_valid(r))
            && self.heat.as_ref().is_none_or(Heat::valid)
            && (self.heats.is_empty() || self.heat.is_some())
    }
    pub fn depart(&mut self, id: u32, remaining: Vec<u32>) {
        self.fire.remove(&id);
        let Some(next) = remaining.first().copied() else {
            self.leave();
            return;
        };
        if self.wager_owner == Some(id) {
            self.wager_owner = Some(next);
        }
        // In a heat a knight who leaves forfeits their own round; the rest
        // play on.
        if let Some(heat) = &mut self.heat {
            let lead = heat.seats.first() == Some(&id);
            let gone = if lead {
                self.round.take()
            } else {
                self.heats.remove(&id)
            };
            if let Some(r) = gone {
                heat.done.insert(
                    id,
                    Finish {
                        progress: r.progress,
                        points: None,
                    },
                );
            }
            return;
        }
        let Some(r) = &mut self.round else {
            return;
        };
        r.riders.remove(&id);
        if let Some(s) = &mut r.sled {
            if s.driver == id {
                s.driver = next;
            }
        }
        if let Some(s) = &mut r.cart {
            if s.driver == id {
                s.driver = next;
            }
        }
        if let Some(s) = &mut r.put {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.surf {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.volley {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.halfpipe {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.chariot {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.trencher {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.duckshoot {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.longbow {
            if s.owner == id {
                s.owner = next;
            }
        }
    }

    pub fn leave(&mut self) {
        self.wager = None;
        self.wager_key = false;
        *self = Self::default();
    }
}

/// A fresh round of `game` for `owner`, standing at `at` (units).
fn new_round(game: Game, owner: u32, at: (f32, f32), rotation: usize) -> Round {
    let recipients = [0, 2, 3]
        .into_iter()
        .fold(0, |m, i| m | (1 << ((i + rotation) % 5)));
    Round {
        game,
        left: game.time(),
        progress: 0,
        tried: 0,
        recipients,
        mistakes: 0,
        last_beat: None,
        hazard_left: 0,
        cheese: None,
        shots: 0,
        put: (game == Game::Pot).then(|| Put::new(owner)),
        cart: (game == Game::Cart).then(|| Cart::new(owner)),
        surf: (game == Game::Surf).then(|| Surf::new(owner)),
        volley: (game == Game::Volley).then(|| Volley::new(owner)),
        halfpipe: (game == Game::Halfpipe).then(|| Halfpipe::new(owner)),
        chariot: (game == Game::Chariot).then(|| Chariot::new(owner)),
        trencher: (game == Game::Trencher).then(|| Trencher::new(owner)),
        duckshoot: (game == Game::DuckShoot).then(|| DuckShoot::new(owner)),
        longbow: (game == Game::Longbow).then(|| Longbow::new(owner)),
        riders: if game == Game::Hobbyhorse {
            BTreeMap::from([(
                owner,
                Rider {
                    x: (at.0 / TILE_UNITS * 256.0) as i32,
                    y: (at.1 / TILE_UNITS * 256.0) as i32,
                    air_left: 0,
                    rest_left: 0,
                },
            )])
        } else {
            BTreeMap::new()
        },
        sled: (game == Game::Sled).then_some(Sled {
            driver: owner,
            x: 3 * 256,
            y: 7 * 256,
            dy: 0,
        }),
    }
}

/// A round that ends without the win.
fn lost(round: &Round) -> Played {
    Played::Over(Finish {
        progress: round.progress,
        points: None,
    })
}

/// A round's own bounds, for the lead's round and every heat alike.
fn round_valid(r: &Round) -> bool {

                r.left > 0
                    && r.left <= r.game.time()
                    && r.progress < r.game.need()
                    && r.tried & !31 == 0
                    && r.recipients & !31 == 0
                    && (r.game != Game::Broadsheet
                        || (r.recipients.count_ones() == 3
                            && u32::from(r.progress) == (r.tried & r.recipients).count_ones()))
                    && (if r.game == Game::Pot {
                        r.put.as_ref().is_some_and(|p| p.valid(r))
                    } else {
                        r.put.is_none()
                    })
                    && (if r.game == Game::Trencher {
                        r.trencher.as_ref().is_some_and(|t| t.valid(r))
                    } else { r.trencher.is_none() })
                    && (if r.game == Game::DuckShoot {
                        r.duckshoot.as_ref().is_some_and(|d| d.valid(r))
                    } else {
                        r.duckshoot.is_none()
                    })
                    && (if r.game == Game::Longbow {
                        r.longbow.as_ref().is_some_and(|b| b.valid(r))
                    } else {
                        r.longbow.is_none()
                    })
                    && (if r.game == Game::Chariot {
                        r.chariot.as_ref().is_some_and(|c| c.valid(r))
                    } else { r.chariot.is_none() })
                    && (if r.game == Game::Halfpipe {
                        r.halfpipe.as_ref().is_some_and(|h| h.valid(r))
                    } else {
                        r.halfpipe.is_none()
                    })
                    && (if r.game == Game::Volley {
                        r.volley.as_ref().is_some_and(|v| v.valid(r))
                    } else {
                        r.volley.is_none()
                    })
                    && (if r.game == Game::Surf {
                        r.surf.as_ref().is_some_and(|s| s.valid(r))
                    } else {
                        r.surf.is_none()
                    })
                    && (if r.game == Game::Cart {
                        r.cart.as_ref().is_some_and(|c| c.valid(r))
                    } else {
                        r.cart.is_none()
                    })
                    && r.shots <= 12
                    && r.cheese.as_ref().is_none_or(|c| c.valid())
                    && (r.game == Game::Cheese || (r.shots == 0 && r.cheese.is_none()))
                    && r.riders.len() <= MAX_PLAYERS as usize
                    && r.riders.iter().all(|(&id, rider)| id > 0 && rider.valid())
                    && (r.game == Game::Hobbyhorse || r.riders.is_empty())
                    && (if r.game == Game::Sled {
                        r.sled.as_ref().is_some_and(|s| {
                            s.valid()
                                && s.x < SLED_GATES[usize::from(r.progress)].0 * 256
                                && (r.progress == 0
                                    || s.x >= SLED_GATES[usize::from(r.progress) - 1].0 * 256)
                        })
                    } else {
                        r.sled.is_none()
                    })
                    && (r.game != Game::Cheese
                        || (r.progress <= r.shots
                            && r.mistakes <= r.shots
                            && u16::from(r.progress)
                                + u16::from(r.mistakes)
                                + u16::from(r.cheese.is_some())
                                == u16::from(r.shots)))
                    && r.mistakes <= 20
                    && r.hazard_left <= HZ
                    && r.last_beat.is_none_or(|b| b <= r.game.time() / HZ)
}

/// Tile coordinates; each is in front of a house, never on its collision.
pub const MAILBOXES: [(f32, f32); 5] =
    [(5.0, 4.5), (8.0, 4.5), (18.0, 4.5), (5.0, 9.0), (17.0, 9.0)];
pub const FLAGS: [(f32, f32); 6] = [
    (6.0, 5.0),
    (9.0, 9.0),
    (12.0, 5.0),
    (15.0, 9.0),
    (18.0, 5.0),
    (21.0, 8.0),
];
pub const WAKES: [(f32, f32); 6] = [
    (7.0, 9.0),
    (10.0, 5.5),
    (13.0, 9.0),
    (16.0, 5.5),
    (19.0, 9.0),
    (21.0, 6.5),
];
pub fn wake(i: usize, tick: u64) -> (f32, f32) {
    let (x, y) = WAKES[i];
    let phase =
        (tick % (8 * HZ as u64)) as f32 / (8 * HZ) as f32 * std::f32::consts::TAU + i as f32;
    (x, y + phase.sin() * 0.35)
}
pub fn nuisance(game: Game, tick: u64) -> (f32, f32) {
    let phase = (tick % (10 * HZ as u64)) as f32 / (10 * HZ) as f32 * std::f32::consts::TAU;
    match game {
        Game::Broadsheet => (12.0 + phase.sin() * 7.0, 7.0 + phase.cos()),
        Game::Barrel => (12.0 + phase.sin() * 7.0, 7.0 + phase.cos()),
        _ => (16.0 + phase.sin() * 2.0, 7.0 + phase.cos()),
    }
}

/// Basket-bearing goats pace. Lead the target: walking into it never scores.
pub fn cheese_target(round: &Round) -> (f32, f32) {
    let age = round.game.time().saturating_sub(round.left);
    let period = 4 * HZ;
    let phase = (age + u32::from(round.progress) * HZ / 2) % period;
    let wave = phase.min(period - phase) as f32 / (2 * HZ) as f32;
    (15.0 + f32::from(round.progress % 3) * 2.0, 4.5 + wave * 4.0)
}

impl Run {
    pub(super) fn tick_pastimes(&mut self, inputs: &BTreeMap<u32, Input>) {
        let room = self.room().kind;
        if !Game::ALL.iter().any(|g| g.room() == room) {
            self.pastimes.leave();
            return;
        }
        // There are at most four knights. Keep this per-tick working set on
        // the stack rather than allocating two vectors and a new held-key tree.
        let mut eligible = [(0u32, 0.0, 0.0); MAX_PLAYERS as usize];
        let mut count = 0;
        for (&id, h) in self
            .players
            .iter()
            .filter(|(_, h)| h.hp > 0 && !h.stone && h.privy == 0)
            .take(MAX_PLAYERS as usize)
        {
            eligible[count] = (id, h.x, h.y);
            count += 1;
        }
        let eligible = &eligible[..count];
        let near = |(tx, ty): (f32, f32), reach: f32| {
            eligible
                .iter()
                .any(|&(_, x, y)| (x - tx * TILE_UNITS).hypot(y - ty * TILE_UNITS) < reach)
        };
        self.pastimes.fire.retain(|id| {
            eligible.iter().any(|&(who, _, _)| who == *id)
                && inputs.get(id).is_some_and(|i| i.valid() && i.fire)
        });
        let mut tapped = [0u32; MAX_PLAYERS as usize];
        let mut taps = 0;
        for &(id, _, _) in eligible {
            if inputs.get(&id).is_some_and(|i| i.valid() && i.fire) && self.pastimes.fire.insert(id)
            {
                tapped[taps] = id;
                taps += 1;
            }
        }
        let tapped = &tapped[..taps];
        let occupied = Game::ALL
            .iter()
            .filter(|g| g.room() == room && near(g.start(), 1.25))
            .fold(0, |m, g| m | (1 << g.index()));
        let pressed = occupied & !self.pastimes.pads;
        self.pastimes.pads = occupied;
        if eligible.is_empty() {
            self.pastimes.round = None;
            self.pastimes.heats.clear();
            self.pastimes.heat = None;
            return;
        }
        if self.pastimes.heat.is_none() {
            self.pastimes.round = None;
            if pressed.count_ones() == 1 {
                let game = Game::ALL[pressed.trailing_zeros() as usize];
                let Some(&(starter, sx, sy)) = eligible.iter().find(|&&(_, x, y)| {
                    (x - game.start().0 * TILE_UNITS).hypot(y - game.start().1 * TILE_UNITS) < 1.25
                }) else {
                    return;
                };
                let record = &mut self.home.countryside.games;
                let rotation = (record.plays_for(game) % 5) as usize;
                if game == Game::Trencher {
                    record.trencher.plays = record.trencher.plays.saturating_add(1);
                } else if game == Game::Chariot {
                    record.chariot.plays = record.chariot.plays.saturating_add(1);
                    self.pastimes.wager = None;
                } else if game == Game::Halfpipe {
                    record.halfpipe.plays = record.halfpipe.plays.saturating_add(1);
                } else if game == Game::Volley {
                    record.volley.plays = record.volley.plays.saturating_add(1);
                } else if game == Game::Surf {
                    record.surf.plays = record.surf.plays.saturating_add(1);
                } else if game == Game::Cart {
                    record.cart.plays = record.cart.plays.saturating_add(1);
                } else if game == Game::Pot {
                    record.pot.plays = record.pot.plays.saturating_add(1);
                } else if game == Game::Sled {
                    record.sled.plays = record.sled.plays.saturating_add(1);
                } else if game == Game::Hobbyhorse {
                    record.hobbyhorse.plays = record.hobbyhorse.plays.saturating_add(1);
                } else if game == Game::Cheese {
                    record.cheese.plays = record.cheese.plays.saturating_add(1);
                } else if game == Game::DuckShoot {
                    record.duckshoot.plays = record.duckshoot.plays.saturating_add(1);
                } else if game == Game::Longbow {
                    record.longbow.plays = record.longbow.plays.saturating_add(1);
                } else {
                    record.plays[game.index()] = record.plays[game.index()].saturating_add(1);
                }
                // Everyone in the room is in: the starter plays the lead
                // round, each friend a round of their own.
                self.pastimes.round = Some(new_round(game, starter, (sx, sy), rotation));
                let mut seats = vec![starter];
                for &(id, x, y) in eligible {
                    if id != starter {
                        self.pastimes.heats.insert(id, new_round(game, id, (x, y), rotation));
                        seats.push(id);
                    }
                }
                let crowd = seats.len();
                self.pastimes.heat = Some(Heat {
                    game,
                    seats,
                    done: BTreeMap::new(),
                    standing: BTreeMap::new(),
                });
                self.sounds.push("play_card");
                self.found = Some((
                    self.tick,
                    0,
                    if crowd > 1 {
                        format!("{}! Everyone plays. Best wins.", game.name())
                    } else {
                        format!("{} begins!", game.name())
                    },
                ));
            }
            return;
        }
        // Each seat plays its own round: only its own feet and keys count.
        let seats = self.pastimes.heat.as_ref().map(|h| h.seats.clone()).unwrap_or_default();
        for (n, &id) in seats.iter().enumerate() {
            let round = if n == 0 {
                self.pastimes.round.take()
            } else {
                self.pastimes.heats.remove(&id)
            };
            let Some(round) = round else {
                continue;
            };
            let mine: Vec<(u32, f32, f32)> =
                eligible.iter().copied().filter(|&(who, _, _)| who == id).collect();
            let taps: Vec<u32> = tapped.iter().copied().filter(|&who| who == id).collect();
            let played = if round.game.room() == room {
                self.play_round(round, inputs, &mine, &taps)
            } else {
                Played::Over(Finish {
                    progress: round.progress,
                    points: None,
                })
            };
            if let Some(heat) = &mut self.pastimes.heat {
                let progress = match &played {
                    Played::Going(round) => round.progress,
                    Played::Over(finish) => finish.progress,
                };
                heat.standing.insert(id, progress.min(heat.game.need()));
            }
            match played {
                Played::Going(round) if n == 0 => self.pastimes.round = Some(round),
                Played::Going(round) => {
                    self.pastimes.heats.insert(id, round);
                }
                Played::Over(finish) => {
                    if let Some(heat) = &mut self.pastimes.heat {
                        heat.done.insert(id, finish);
                    }
                    // A race: the first knight home wins, and the others'
                    // rounds stop where they stand.
                    if finish.points.is_some() {
                        self.end_heat_rounds();
                        break;
                    }
                }
            }
        }
        if self.pastimes.round.is_none()
            && self.pastimes.heats.is_empty()
            && let Some(heat) = self.pastimes.heat.take()
        {
            self.judge_heat(&heat);
        }
    }

    /// Stops every round still going, keeping how far each got.
    fn end_heat_rounds(&mut self) {
        let Some(heat) = &mut self.pastimes.heat else {
            return;
        };
        let lead = heat.seats.first().copied();
        let going = self
            .pastimes
            .round
            .take()
            .into_iter()
            .map(|r| (lead.unwrap_or(0), r))
            .chain(std::mem::take(&mut self.pastimes.heats));
        for (id, r) in going {
            heat.done.entry(id).or_insert(Finish {
                progress: r.progress,
                points: None,
            });
        }
    }

    /// Everyone has finished: the best is crowned before the party.
    fn judge_heat(&mut self, heat: &Heat) {
        if heat.seats.len() < 2 {
            return;
        }
        let Some(winner) = heat.winner() else {
            return;
        };
        let name = self
            .players
            .get(&winner)
            .map_or_else(|| "A knight".to_string(), |h| h.name.clone());
        // The stakes are worn: a laurel for the winner, the game's booby
        // prize for everyone else, until they win one back.
        for &id in &heat.seats {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.laurel = id == winner;
                hero.booby = (id != winner).then_some(heat.game);
            }
        }
        self.found = Some((
            self.tick,
            0,
            format!("{} WINS THE {}!", name.to_uppercase(), heat.game.name()),
        ));
        self.sounds.push("play_card");
        self.cues.push("pastime_won".into());
    }

    /// One tick of one knight's round. `eligible` and `tapped` hold that
    /// knight alone.
    fn play_round(
        &mut self,
        mut round: Round,
        inputs: &BTreeMap<u32, Input>,
        eligible: &[(u32, f32, f32)],
        tapped: &[u32],
    ) -> Played {
        let near = |(tx, ty): (f32, f32), reach: f32| {
            eligible
                .iter()
                .any(|&(_, x, y)| (x - tx * TILE_UNITS).hypot(y - ty * TILE_UNITS) < reach)
        };
        // Expiry wins the race against a final delivery/flag on the same tick.
        // Input answers the round's published cue, before this tick advances
        // the clock. Expiry still wins below, including a tap on the last tick.
        let shown_left = round.left;
        round.left = round.left.saturating_sub(1);
        if round.left == 0 {
            self.found = Some((
                self.tick,
                0,
                format!(
                    "{}: time! {}/{}.",
                    round.game.name(),
                    round.progress,
                    round.game.need()
                ),
            ));
            self.cues.push("pastime_lost".into());
            return lost(&round);
        }
        round.hazard_left = round.hazard_left.saturating_sub(1);
        if matches!(round.game, Game::Broadsheet | Game::Barrel | Game::Washtub)
            && round.hazard_left == 0
            && !(round.game == Game::Broadsheet && self.home.countryside.has(0))
            && near(nuisance(round.game, self.tick), 1.2)
        {
            round.hazard_left = HZ;
            round.mistakes = round.mistakes.saturating_add(1).min(20);
            self.found = Some((
                self.tick,
                0,
                match round.game {
                    Game::Broadsheet => "HONK! The goose has eaten your editorial. -25",
                    Game::Barrel => "A jester demands a toll in dignity. -25",
                    _ => "The royal duck disapproves. -25",
                }
                .into(),
            ));
        }
        match round.game {
            Game::Trencher => {
                if !self.tick_trencher(&mut round, inputs, &eligible, &tapped) {
                    self.found = Some((self.tick, 0, "Picnic over.".into()));
                    return lost(&round);
                }
            }
            Game::Chariot => {
                let c = round.chariot.as_mut().unwrap();
                if !eligible.iter().any(|&(id,_,_)|id==c.owner) {
                    self.found=Some((self.tick,0,"Driver retired.".into()));
                    return lost(&round);
                }
                let input=inputs.get(&c.owner).copied().filter(|i|i.valid()).unwrap_or_default();
                let before=c.bog;
                let result=c.step(input,tapped.contains(&c.owner));
                if c.bog>before {round.mistakes=round.mistakes.saturating_add(1).min(20);self.sounds.push("hero_hurt");}
                round.progress=(c.distance/LAP) as u8;
                if result<0 {self.found=Some((self.tick,c.owner,"Outraced by a goat!".into()));return lost(&round);}
            }
            Game::Halfpipe => {
                let h = round.halfpipe.as_mut().unwrap();
                if !eligible.iter().any(|&(id, _, _)| id == h.owner) {
                    self.found = Some((self.tick, 0, "Jester retired.".into()));
                    return lost(&round);
                }
                let input = inputs.get(&h.owner).copied().filter(|i| i.valid()).unwrap_or_default();
                let direction = Halfpipe::direction(round.progress, round.mistakes);
                if let Some(landed) = h.step(input, tapped.contains(&h.owner), direction) {
                    if landed {
                        round.progress += 1;
                        self.sounds.push("spoil_pickup");
                    } else {
                        round.mistakes += 1;
                        self.sounds.push("hero_hurt");
                        if round.mistakes == 3 {
                            self.found = Some((self.tick, h.owner, "Three helmets dented! Step off J, then retry.".into()));
                            return lost(&round);
                        }
                    }
                }
            }
            Game::Volley => {
                let volley = round.volley.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == volley.owner) {
                    self.found = Some((
                        self.tick,
                        0,
                        "Bat retired.".into(),
                    ));
                    return lost(&round);
                }
                let input = inputs
                    .get(&volley.owner)
                    .filter(|i| i.valid())
                    .copied()
                    .unwrap_or_default();
                let lives = volley.lives;
                let bells = volley.bells;
                volley.step(input, tapped.contains(&volley.owner));
                round.progress = volley.bells.count_ones() as u8;
                if bells != volley.bells {
                    self.sounds.push("spoil_pickup");
                }
                if lives != volley.lives {
                    round.mistakes = round.mistakes.saturating_add(1).min(20);
                    self.sounds.push("clang");
                }
                if volley.lives == 0 {
                    self.found = Some((self.tick, volley.owner, "Three turnips lost!".into()));
                    return lost(&round);
                }
            }
            Game::Surf => {
                let surf = round.surf.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == surf.owner) {
                    self.found = Some((
                        self.tick,
                        0,
                        "Surfer retired.".into(),
                    ));
                    return lost(&round);
                }
                let input = inputs
                    .get(&surf.owner)
                    .filter(|i| i.valid())
                    .copied()
                    .unwrap_or_default();
                match surf.step(input, tapped.contains(&surf.owner), round.progress) {
                    Some(true) => {
                        round.progress += 1;
                        self.sounds.push("spoil_pickup");
                    }
                    Some(false) => {
                        self.found = Some((
                            self.tick,
                            surf.owner,
                            format!(
                                "Wipeout on wave {}!",
                                round.progress + 1
                            ),
                        ));
                        self.sounds.push("clang");
                        return lost(&round);
                    }
                    None => {}
                }
            }
            Game::DuckShoot => {
                let range = round.duckshoot.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == range.owner) {
                    self.found = Some((
                        self.tick,
                        0,
                        "Gun retired.".into(),
                    ));
                    return lost(&round);
                }
                let input = inputs
                    .get(&range.owner)
                    .filter(|i| i.valid())
                    .copied()
                    .unwrap_or_default();
                if let Some(rail) = range.step(input, tapped.contains(&range.owner)) {
                    // The far rail's ducks count double: worth the squint.
                    let worth = if rail == 2 { 2 } else { 1 };
                    round.progress = (round.progress + worth).min(round.game.need());
                    self.sounds.push("spoil_pickup");
                } else if tapped.contains(&range.owner) {
                    self.sounds.push("shot_handgonne");
                }
                if range.shots >= 8 * 5 && round.progress < round.game.need() && range.shots == 40 {
                    // Out of BBs: the barker keeps the rest as rent.
                    self.found = Some((
                        self.tick,
                        range.owner,
                        "Out of BBs!".into(),
                    ));
                    return lost(&round);
                }
            }
            Game::Longbow => {
                if !self.tick_longbow(&mut round, inputs, eligible) {
                    return lost(&round);
                }
            }
            Game::Cart => {
                if !self.tick_cart(&mut round, inputs, eligible, tapped) {
                    return lost(&round);
                }
            }
            Game::Pot => {
                if !self.tick_put(&mut round, inputs, eligible) {
                    return lost(&round);
                }
            }
            Game::Sled => {
                let sled = round.sled.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == sled.driver) {
                    self.found = Some((
                        self.tick,
                        0,
                        "Squire parked.".into(),
                    ));
                    return lost(&round);
                }
                // Never OR a friend's steering or brake into the driver's.
                let input = inputs
                    .get(&sled.driver)
                    .filter(|i| i.valid())
                    .copied()
                    .unwrap_or_default();
                let steer = if input.aim_y != 0 {
                    input.aim_y
                } else {
                    input.move_y
                };
                if steer == 0 {
                    sled.dy = sled.dy * 7 / 8;
                } else {
                    sled.dy = (sled.dy + i32::from(steer) * 4).clamp(-32, 32);
                }
                if input.fire {
                    sled.dy = sled.dy * 3 / 4;
                }
                sled.x += if input.fire { 6 } else { 12 };
                sled.y += sled.dy;
                if sled.y < 4 * 256 || sled.y > 10 * 256 {
                    sled.y = sled.y.clamp(4 * 256, 10 * 256);
                    sled.dy = 0;
                    if round.hazard_left == 0 {
                        round.hazard_left = HZ;
                        round.mistakes = round.mistakes.saturating_add(1).min(20);
                        self.sounds.push("clang");
                    }
                }
                let (gx, gy) = SLED_GATES[usize::from(round.progress)];
                if sled.x >= gx * 256 {
                    if sled.y.abs_diff(gy * 256) > 256 {
                        self.found = Some((
                            self.tick,
                            0,
                            format!(
                                "Missed gate {}!",
                                round.progress + 1
                            ),
                        ));
                        self.sounds.push("clang");
                        return lost(&round);
                    }
                    round.progress += 1;
                    self.sounds.push("spoil_pickup");
                }
            }
            Game::Broadsheet => {
                for &id in tapped {
                    let Some(&(_, x, y)) = eligible.iter().find(|&&(who, _, _)| who == id) else {
                        continue;
                    };
                    let mailbox = MAILBOXES
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| (x - p.0 * TILE_UNITS).hypot(y - p.1 * TILE_UNITS) < 3.0)
                        .min_by(|(_, a), (_, b)| {
                            (x - a.0 * TILE_UNITS)
                                .hypot(y - a.1 * TILE_UNITS)
                                .total_cmp(&(x - b.0 * TILE_UNITS).hypot(y - b.1 * TILE_UNITS))
                        })
                        .map(|(i, _)| i);
                    if let Some(i) = mailbox.filter(|&i| round.tried & (1 << i) == 0) {
                        round.tried |= 1 << i;
                        if round.recipients & (1 << i) != 0 {
                            round.progress += 1;
                            self.sounds.push("spoil_pickup");
                            self.found = Some((
                                self.tick,
                                id,
                                "Delivered: 'Local knight mistakes goose for dragon'.".into(),
                            ));
                        } else {
                            round.mistakes = round.mistakes.saturating_add(1).min(20);
                            self.found = Some((
                                self.tick,
                                id,
                                "Not a subscriber! -25".into(),
                            ));
                        }
                    }
                }
            }
            Game::Barrel | Game::Washtub => {
                let i = usize::from(round.progress);
                let target = if round.game == Game::Barrel {
                    FLAGS[i]
                } else {
                    wake(i, self.tick)
                };
                if near(target, 1.5) {
                    round.progress += 1;
                    self.sounds.push("spoil_pickup");
                }
            }
            Game::Cheese => {
                if let Some(mut cheese) = round.cheese.take() {
                    cheese.x += cheese.dx;
                    cheese.y += cheese.dy;
                    cheese.left = cheese.left.saturating_sub(1);
                    let (x, y) = (cheese.x as f32 / 256.0, cheese.y as f32 / 256.0);
                    let basket = cheese_target(&round);
                    if (x - basket.0).hypot(y - basket.1) <= 0.75 {
                        round.progress += 1;
                        round.hazard_left = HZ / 3;
                        self.sounds.push("spoil_pickup");
                    } else if !cheese.valid()
                        || matches!(
                            self.room().tile(x.floor() as i32, y.floor() as i32),
                            Tile::Wall | Tile::Block
                        )
                    {
                        round.mistakes += 1;
                    } else {
                        round.cheese = Some(cheese);
                    }
                }
                if round.cheese.is_none()
                    && round.shots < 12
                    && round.progress < round.game.need()
                    && let Some(&(id, x, y)) = eligible.iter().find(|&&(id, x, y)| {
                        tapped.contains(&id)
                            && (x - 4.0 * TILE_UNITS).hypot(y - 6.0 * TILE_UNITS) < 2.4
                    })
                {
                    let input = &inputs[&id];
                    let (ax, ay) = (f32::from(input.aim_x), f32::from(input.aim_y));
                    let length = ax.hypot(ay);
                    let (ax, ay) = if length > 0.1 {
                        (ax / length, ay / length)
                    } else {
                        (1.0, 0.0)
                    };
                    round.cheese = Some(Cheese {
                        x: (x / TILE_UNITS * 256.0).round() as i32,
                        y: (y / TILE_UNITS * 256.0).round() as i32,
                        dx: (ax * 108.0).round() as i32,
                        dy: (ay * 108.0).round() as i32,
                        left: 2 * HZ,
                    });
                    round.shots += 1;
                    self.sounds.push("play_card");
                }
                if round.shots == 12 && round.cheese.is_none() && round.progress < round.game.need()
                {
                    self.found = Some((self.tick, 0,
                        "Out of cheese!".into()));
                    return lost(&round);
                }
            }
            Game::Hobbyhorse => {
                let fence = (HURDLES[usize::from(round.progress)] * 256.0) as i32;
                round
                    .riders
                    .retain(|id, _| eligible.iter().any(|(who, _, _)| who == id));
                let mut cleared = false;
                for &(id, x, y) in eligible {
                    let (x, y) = (
                        (x / TILE_UNITS * 256.0) as i32,
                        (y / TILE_UNITS * 256.0) as i32,
                    );
                    let rider = round.riders.entry(id).or_insert(Rider {
                        x,
                        y,
                        air_left: 0,
                        rest_left: 0,
                    });
                    rider.air_left = rider.air_left.saturating_sub(1);
                    rider.rest_left = rider.rest_left.saturating_sub(1);
                    if tapped.contains(&id) && rider.rest_left == 0 {
                        rider.air_left = JUMP_TICKS;
                        rider.rest_left = JUMP_TICKS + 2;
                    }
                    let in_lane = |y: i32| {
                        y >= (RIDING_LANE.0 * 256.0) as i32 && y <= (RIDING_LANE.1 * 256.0) as i32
                    };
                    // Crossing, not standing on a ring. Both ends must be in
                    // the lane; a new rider or discontinuous move cannot score.
                    if !cleared
                        && rider.x < fence
                        && x >= fence
                        && x.abs_diff(rider.x) <= 256
                        && y.abs_diff(rider.y) <= 256
                        && in_lane(y)
                        && in_lane(rider.y)
                    {
                        if rider.air_left > 0 {
                            cleared = true;
                        } else if round.hazard_left == 0 {
                            round.mistakes = round.mistakes.saturating_add(1).min(20);
                            round.hazard_left = HZ / 2;
                            self.sounds.push("clang");
                        }
                    }
                    (rider.x, rider.y) = (x, y);
                }
                if cleared {
                    round.progress += 1;
                    self.sounds.push("spoil_pickup");
                }
            }
            Game::Turnip => {
                let elapsed = round.game.time().saturating_sub(shown_left);
                let beat = elapsed / HZ;
                let phase = elapsed % HZ;
                let hit = tapped.iter().any(|id| {
                    eligible.iter().any(|&(who, x, y)| {
                        who == *id
                            && (x - Game::Turnip.start().0 * TILE_UNITS)
                                .hypot(y - Game::Turnip.start().1 * TILE_UNITS)
                                < 2.2
                    })
                });
                if hit && round.last_beat != Some(beat) {
                    round.last_beat = Some(beat);
                    if phase <= HZ / 5 {
                        round.progress += 1;
                        self.sounds.push("spoil_pickup");
                    } else {
                        round.mistakes = round.mistakes.saturating_add(1).min(20);
                        self.found = Some((
                            self.tick,
                            0,
                            "Too soon! Kick as it lands.".into(),
                        ));
                    }
                }
            }
        }
        if round.progress >= round.game.need() {
            let result = (u32::from(round.progress) * 75 + round.left / HZ * 5)
                .saturating_sub(u32::from(round.mistakes) * 25)
                .clamp(25, 1000) as u16;
            let record = &mut self.home.countryside.games;
            if round.game == Game::Trencher {
                record.trencher.best = record.trencher.best.max(result);
                record.trencher.wins = record.trencher.wins.saturating_add(1).min(record.trencher.plays);
            } else if round.game == Game::Chariot {
                record.chariot.best=record.chariot.best.max(result);
                record.chariot.wins=record.chariot.wins.saturating_add(1).min(record.chariot.plays);
            } else if round.game == Game::Halfpipe {
                record.halfpipe.best = record.halfpipe.best.max(result);
                record.halfpipe.wins = record.halfpipe.wins.saturating_add(1).min(record.halfpipe.plays);
            } else if round.game == Game::Volley {
                record.volley.best = record.volley.best.max(result);
                record.volley.wins = record
                    .volley
                    .wins
                    .saturating_add(1)
                    .min(record.volley.plays);
            } else if round.game == Game::Surf {
                record.surf.best = record.surf.best.max(result);
                record.surf.wins = record.surf.wins.saturating_add(1).min(record.surf.plays);
            } else if round.game == Game::Cart {
                record.cart.best = record.cart.best.max(result);
                record.cart.wins = record.cart.wins.saturating_add(1).min(record.cart.plays);
            } else if round.game == Game::Pot {
                record.pot.best = record.pot.best.max(result);
                record.pot.wins = record.pot.wins.saturating_add(1).min(record.pot.plays);
            } else if round.game == Game::Sled {
                record.sled.best = record.sled.best.max(result);
                record.sled.wins = record.sled.wins.saturating_add(1).min(record.sled.plays);
            } else if round.game == Game::Hobbyhorse {
                record.hobbyhorse.best = record.hobbyhorse.best.max(result);
                record.hobbyhorse.wins = record
                    .hobbyhorse
                    .wins
                    .saturating_add(1)
                    .min(record.hobbyhorse.plays);
            } else if round.game == Game::Cheese {
                record.cheese.best = record.cheese.best.max(result);
                record.cheese.wins = record
                    .cheese
                    .wins
                    .saturating_add(1)
                    .min(record.cheese.plays);
            } else if round.game == Game::DuckShoot {
                record.duckshoot.best = record.duckshoot.best.max(result);
                record.duckshoot.wins = record
                    .duckshoot
                    .wins
                    .saturating_add(1)
                    .min(record.duckshoot.plays);
            } else if round.game == Game::Longbow {
                record.longbow.best = record.longbow.best.max(result);
                record.longbow.wins = record.longbow.wins.saturating_add(1).min(record.longbow.plays);
            } else {
                let i = round.game.index();
                record.best[i] = record.best[i].max(result);
                record.wins[i] = record.wins[i].saturating_add(1).min(record.plays[i]);
            }
            self.score = self.score.saturating_add(u32::from(result));
            // Every win is marked; the realm pays a game's first one once.
            self.marks.insert(super::countryside::game_mark(round.game.index()), 1);
            self.found = Some((
                self.tick,
                0,
                format!(
                    "{} complete! +{result}. Best {}.",
                    round.game.name(),
                    record.best_for(round.game)
                ),
            ));
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
            return Played::Over(Finish {
                progress: round.progress,
                points: Some(result),
            });
        }
        Played::Going(round)
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_heats__tests.rs"]
mod heat_tests;
