//! The kingdom's tiny arcade: model-free, host-authoritative and co-operative.
//! Medieval homages, not imported characters or assets. Feet, F and a clock
//! are enough. Rounds pay score only; never an infinite treasury gold faucet.

use super::*;
use std::collections::BTreeSet;
mod cart;
mod chariot;
mod duckshoot;
mod gander;
mod halfpipe;
mod longbow;
mod powder;
mod put;
mod peal;
mod siege;
mod surf;
mod trencher;
mod volley;
pub use cart::{CART_AIR, CART_LOGS, CART_RUTS, Cart};
pub use chariot::{BARRELS, Chariot, LAP, RIVALS};
pub use duckshoot::{DOWN_RESET, DuckShoot, EYE_Z, FOCAL, RAIL_Y, RAIL_Z};
pub use gander::{
    FLAP_KICK as GANDER_FLAP_KICK, GRAVITY as GANDER_GRAVITY, Gander, GanderEgg, GanderRider,
    HATCH as GANDER_HATCH, LEDGES as GANDER_LEDGES, NEED as GANDER_NEED, TAR_ROW as GANDER_TAR_ROW,
};
pub use halfpipe::{FLIGHT_TICKS, Halfpipe, LIP_CLOSE, LIP_OPEN};
pub use longbow::{
    ARROWS as LONGBOW_ARROWS, FLIGHT as LONGBOW_FLIGHT, FULL_DRAW as LONGBOW_FULL_DRAW,
    HIT_BURST as LONGBOW_HIT_BURST, Longbow,
};
pub use powder::{
    BALES as POWDER_BALES, BLAST, BLAST_CHARM, Charm, FIRE, FUSE, Flame, H as POWDER_H, KEGS,
    KEGS_CHARM, Keg, Knight, POSTS as POWDER_POSTS, Powder, SPEED, SPEED_BOOTS, STOCK, STOCK_CHARM,
    Scorch, W as POWDER_W, X0 as POWDER_X0, Y0 as POWDER_Y0,
};
pub use put::{Put, TARGETS};
pub use siege::{
    Ammo, BLOCK_H, BLOCK_W, CASTLE_X, ENGINE_X, EPITAPHS, EVENT_LOSS, EVENT_RASH_LOSS, EVENT_WAIT,
    FIELD_W, GRAVE_NONE, GROUND_LINE, KEEP_COLS, KEEP_ROWS, Keep, LEGS, MISSING_CREW_TICKS, MOON,
    MOURN, NOTICE_FORD_PAID, NOTICE_MARCHES, NOTICE_NONE, NOTICES, POT, RELOAD_TICKS_BASE, RETREAT,
    SOLDIERS, SPLAT, Shot, Siege, VOLLEY_HZ, WIND_MAX, WINDS,
};
pub use peal::{FIRST as PEAL_FIRST, Peal, STEP as PEAL_STEP, STRIKES as PEAL_STRIKES, WINDOW as PEAL_WINDOW, interval as peal_interval};
pub use surf::{CREST_WINDOW, Surf, WAVE_LEANS, WAVE_TICKS};
pub use trencher::{
    CATCH as TRENCHER_CATCH, FLIGHT as TRENCHER_FLIGHT, Trencher, WINDS as TRENCHER_WINDS,
};
pub use volley::{
    BELL_X, COURT_LEFT, COURT_RIGHT, COURT_TOP, MISS_FLASH_TICKS, PADDLE_REACH, PADDLE_Y,
    PERFECT_FLASH_TICKS, PERFECT_TICKS, RING_TICKS, SERVE_SPEED, SHIMMER_TICKS, SWING_TICKS,
    TRAIL_LEN, Volley,
};

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
    Powder,
    Gander,
    Siege,
    Peal,
}

impl Game {
    pub const ALL: [Self; 20] = [
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
        Self::Powder,
        Self::Gander,
        Self::Siege,
        Self::Peal,
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
            // The arena games are played inside the Colosseum, each in its
            // own room behind one of its gates (`colosseum`).
            Self::Powder => RoomKind::KegYard,
            Self::Gander => RoomKind::GanderHall,
            // The war-camp pitches on the Flower Fields: engines among
            // the flowers, the toll of the trail already paid.
            Self::Siege => RoomKind::Pasture,
            // The second game in the Bashful Belfry: its own ring under
            // the east tower, east of the volley's pad.
            Self::Peal => RoomKind::Belfry,
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
            Self::Volley => "BELFRY BELL RALLY",
            Self::Halfpipe => "JESTER'S HALF-PIPE",
            Self::Chariot => "CABBAGE AND ROLL RACING",
            Self::Trencher => "TRENCHER TOSS",
            Self::DuckShoot => "CARNIVAL DUCK RANGE",
            Self::Longbow => "POPINJAY SHOOT",
            Self::Powder => "POWDER KEG",
            Self::Gander => "GANDER JOUST",
            Self::Siege => "SIEGE TRAIL",
            Self::Peal => "CHANGE RINGING",
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
            Self::Powder => (3.0, 5.0),
            Self::Gander => (9.0, 8.0),
            Self::Siege => (9.0, 10.5),
            Self::Peal => (20.0, 9.0),
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
            Self::Powder => 60 * HZ,
            Self::Gander => 75 * HZ,
            // Five legs over three castles; the trail's own clocks, not
            // the round clock, pace the march.
            Self::Siege => 300 * HZ,
            // The thirteen swings sum to 390 ticks; the clock is the
            // backstop, the peal's own strikes end it first.
            Self::Peal => 15 * HZ,
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
            Self::Powder => 8,
            Self::Gander => 8,
            Self::Siege => 3,
            Self::Peal => 10,
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
    fn is_empty(&self) -> bool { *self == Self::default() }
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
    #[serde(skip_serializing_if = "Score::is_empty")]
    pub cinder: Score,
    #[serde(skip_serializing_if = "Score::is_empty")]
    pub latch: Score,
    #[serde(default, skip_serializing_if = "Score::is_empty")]
    pub brasswake: Score,
    #[serde(default, skip_serializing_if = "Score::is_empty")]
    pub rafterwake: Score,
    /// Book Hop: the most crossings a knight made in a minute.
    #[serde(default, skip_serializing_if = "Score::is_empty")]
    pub hop: Score,
    #[serde(default, skip_serializing_if = "Score::is_empty")]
    pub pennon: Score,
    #[serde(skip_serializing_if = "Score::is_empty")]
    pub mossbell: Score,
    /// The Round Table adventures' records, in `roundtable::ADVENTURES`
    /// order (`Records::roundtable[lanternward::SLOT]`).
    #[serde(skip_serializing_if = "super::roundtable::unscored")]
    pub roundtable: [Score; 5],
    pub powder: Score,
    pub gander: Score,
    pub siege: Score,
    #[serde(default, skip_serializing_if = "Score::is_empty")]
    pub peal: Score,
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
            && self.cinder.valid()
            && self.latch.valid()
            && self.brasswake.valid()
            && self.rafterwake.valid()
            && self.hop.valid()
            && self.pennon.valid()
            && self.mossbell.valid()
            && self.roundtable.iter().all(Score::valid)
            && self.powder.valid()
            && self.gander.valid()
            && self.siege.valid()
            && self.peal.valid()
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
            &mut self.cinder,
            &mut self.latch,
            &mut self.brasswake,
            &mut self.rafterwake,
            &mut self.hop,
            &mut self.pennon,
            &mut self.mossbell,
            &mut self.gander,
            &mut self.siege,
            &mut self.peal,
        ] {
            score.repair();
        }
        self.roundtable.iter_mut().for_each(Score::repair);
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
        } else if game == Game::Powder {
            self.powder.best
        } else if game == Game::Gander {
            self.gander.best
        } else if game == Game::Siege {
            self.siege.best
        } else if game == Game::Peal {
            self.peal.best
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
        } else if game == Game::Powder {
            self.powder.plays
        } else if game == Game::Gander {
            self.gander.plays
        } else if game == Game::Siege {
            self.siege.plays
        } else if game == Game::Peal {
            self.peal.plays
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
        } else if game == Game::Gander {
            self.gander.wins
        } else if game == Game::Siege {
            self.siege.wins
        } else if game == Game::Peal {
            self.peal.wins
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
            &self.cinder,
            &self.latch,
            &self.brasswake,
            &self.rafterwake,
            &self.pennon,
            &self.mossbell,
            &self.gander,
        ]
        .iter()
        .map(|s| s.plays)
        .sum();
        named + self.roundtable.iter().map(|s| s.plays).sum::<u32>() + self.plays.iter().copied().sum::<u32>()
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
            &self.cinder,
            &self.latch,
            &self.brasswake,
            &self.rafterwake,
            &self.pennon,
            &self.mossbell,
            &self.gander,
        ]
        .iter()
        .map(|s| s.wins)
        .sum();
        named + self.roundtable.iter().map(|s| s.wins).sum::<u32>() + self.wins.iter().copied().sum::<u32>()
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
            self.cinder.merge(&other.cinder);
            self.latch.merge(&other.latch);
            self.brasswake.merge(&other.brasswake);
            self.rafterwake.merge(&other.rafterwake);
            self.hop.merge(&other.hop);
            self.pennon.merge(&other.pennon);
            self.mossbell.merge(&other.mossbell);
            self.peal.merge(&other.peal);
            for (mine, theirs) in self.roundtable.iter_mut().zip(&other.roundtable) {
                mine.merge(theirs);
            }
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
    #[serde(default)]
    pub powder: Option<Powder>,
    pub gander: Option<Gander>,
    pub siege: Option<Siege>,
    #[serde(default)]
    pub peal: Option<Peal>,
    /// Each rider jumps only for their own key edge, never the party's OR.
    #[serde(default)]
    pub riders: BTreeMap<u32, Rider>,
    #[serde(default)]
    pub sled: Option<Sled>,
    /// Ticks remaining for the floating score popup (rises and fades over 8 ticks).
    #[serde(default)]
    pub score_popup: u8,
    /// Text to display in the score popup (e.g., "+75", "PERFECT!", "COMBO!").
    #[serde(default)]
    pub score_popup_text: String,
    /// Tile-space X position of the score popup.
    #[serde(default)]
    pub score_popup_x: i32,
    /// Tile-space Y position of the score popup.
    #[serde(default)]
    pub score_popup_y: i32,
    /// Consecutive successful actions (for combo sounds).
    #[serde(default)]
    pub streak: u8,
    /// Ticks remaining for the intro screen: controls are shown and the
    /// round does not advance while it is up. Press fire to skip.
    #[serde(default)]
    pub intro_ticks: u8,
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
///
/// `Going` is the common tick result and holds a full `Round`. Boxing it
/// would allocate on every pastime tick just to shrink the rare `Over`.
#[allow(clippy::large_enum_variant)]
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
            if lead {
                // The shared courtyard needs a seat to carry it: when the
                // starter departs, the next knight's round takes the yard
                // over rather than forfeiting it.
                if self.round.as_ref().is_some_and(|r| r.game == Game::Powder) {
                    let yard = self.round.take();
                    let next_owner = heat
                        .seats
                        .iter()
                        .find(|s| **s != id && !heat.done.contains_key(s))
                        .copied();
                    if let (Some(next), Some(mut r)) = (next_owner, yard)
                        && next != id
                    {
                        if let Some(friend) = self.heats.remove(&next) {
                            r.powder = friend.powder.or(r.powder);
                        }
                        r.game = Game::Powder;
                        heat.seats.retain(|s| *s != id);
                        heat.seats.insert(0, next);
                        self.round = Some(r);
                        heat.done.remove(&id);
                        return;
                    }
                } else {
                    self.round = None;
                }
            } else {
                self.heats.remove(&id);
            }
            // Keep the standing for the board, but a withdrawn score must
            // never win when every remaining round times out.
            heat.done.remove(&id);
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
        // The shared gander hall keeps every seat; a departing starter's
        // seat rides on (their tally stays), the seat list never empties.
        if let Some(s) = &mut r.siege {
            if s.owner == id {
                s.owner = next;
            }
        }
        if let Some(s) = &mut r.peal {
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
fn new_round(
    game: Game,
    owner: u32,
    at: (f32, f32),
    rotation: usize,
    seats: Option<&[u32]>,
) -> Round {
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
        powder: (game == Game::Powder).then(|| {
            let seed = ((rotation as u32 + owner * 3) % 7) as u8;
            Powder::new(std::slice::from_ref(&owner), seed)
        }),
        gander: (game == Game::Gander).then(|| {
            // The shared hall: every seat in the room rides together.
            let mut hall = vec![owner];
            if let Some(seats) = seats {
                for &id in seats {
                    if id != owner {
                        hall.push(id);
                    }
                }
            }
            Gander::new(&hall)
        }),
        siege: (game == Game::Siege).then(|| Siege::new(owner)),
        peal: (game == Game::Peal).then(|| Peal::new(owner)),
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
        score_popup: 0,
        score_popup_text: String::new(),
        score_popup_x: 0,
        score_popup_y: 0,
        streak: 0,
        intro_ticks: 0,
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
                    && (if r.game == Game::Powder {
                        r.powder.as_ref().is_some_and(|p| p.valid())
                    } else {
                        r.powder.is_none()
                    })
                    && (if r.game == Game::Gander {
                        r.gander.as_ref().is_some_and(|g| g.valid(r))
                    } else {
                        r.gander.is_none()
                    })
                    && (if r.game == Game::Siege {
                        r.siege.as_ref().is_some_and(|s| s.valid(r))
                    } else {
                        r.siege.is_none()
                    })
                    && (if r.game == Game::Peal {
                        r.peal.as_ref().is_some_and(|p| p.valid(r))
                    } else {
                        r.peal.is_none()
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
                    && r.score_popup <= 8
                    && r.streak <= 20
                    && r.intro_ticks <= 30
}

/// Ticks between two of a pastime room's ambient sounds. The longest (a
/// fair's wheel) runs 3.35 s, so this keeps it to a third of the time and
/// every other room's tone under a tenth; they came every one to three
/// seconds before, a laser burst for a river and a guardian's sting for
/// the pasture among them.
const AMBIENT_EVERY: u64 = 10 * HZ as u64;

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
    /// The ambient sound for the current room, if any: the place's own
    /// tone (feet on its ground, a fair's wheel, a belfry's bell), never a
    /// fight's cue.
    fn ambient_for_room(room: RoomKind) -> Option<&'static str> {
        match room {
            RoomKind::Village => Some("step_gravel"),
            RoomKind::Fairground => Some("wheel_spin"),
            RoomKind::Riverbank => Some("step_wet"),
            RoomKind::Downs => Some("step_gravel"),
            RoomKind::Causeway => Some("step_stone"),
            RoomKind::Crag => Some("step_gravel"),
            RoomKind::Strand => Some("step_wet"),
            RoomKind::Timberyard => Some("step_wood"),
            RoomKind::Millpond => Some("step_wet"),
            RoomKind::Belfry => Some("circle_toll"),
            RoomKind::Mummers => Some("wheel_spin"),
            RoomKind::Hippodrome => Some("wheel_spin"),
            RoomKind::GooseGreen => Some("step_gravel"),
            RoomKind::LongbowGreen => Some("swing"),
            RoomKind::KegYard => Some("vent_fire"),
            RoomKind::GanderHall => Some("swing"),
            RoomKind::Pasture => Some("step_moss"),
            _ => None,
        }
    }

    /// Pushes an ambient sound for the current room every `AMBIENT_EVERY`.
    /// Ambient sounds are sparse background texture, not foreground events.
    fn push_ambient(&mut self) {
        let room = self.room().kind;
        if let Some(sound) = Self::ambient_for_room(room)
            && self.tick.is_multiple_of(AMBIENT_EVERY)
        {
            self.sounds.push(sound);
        }
    }

    /// A round of `game` is begun: the realm's count of its plays.
    fn note_play(&mut self, game: Game) {
        let record = &mut self.home.countryside.games;
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
        } else if game == Game::Powder {
            record.powder.plays = record.powder.plays.saturating_add(1);
        } else if game == Game::Gander {
            record.gander.plays = record.gander.plays.saturating_add(1);
        } else if game == Game::Siege {
            record.siege.plays = record.siege.plays.saturating_add(1);
        } else if game == Game::Peal {
            record.peal.plays = record.peal.plays.saturating_add(1);
        } else {
            record.plays[game.index()] = record.plays[game.index()].saturating_add(1);
        }
    }

    /// A random room's arcade game (`encounter`): one round of `game` for
    /// every seat, begun as the pad would begin it, each knight standing at
    /// their spot on the stage already raised in the hall.
    pub(super) fn start_encounter_pastime(&mut self, game: Game, seats: &[u32], spots: &[(f32, f32)]) {
        let Some(&starter) = seats.first() else {
            return;
        };
        self.pastimes.leave();
        let rotation = (self.home.countryside.games.plays_for(game) % 5) as usize;
        self.note_play(game);
        let (sx, sy) = spots.first().copied().unwrap_or((0.0, 0.0));
        self.pastimes.round = Some(new_round(game, starter, (sx, sy), rotation, Some(seats)));
        for (&id, &(x, y)) in seats.iter().zip(spots).skip(1) {
            self.pastimes.heats.insert(id, new_round(game, id, (x, y), rotation, Some(seats)));
        }
        // The shared courtyard and the joust hall have one yard for the heat.
        if game == Game::Powder {
            let yard = Powder::new(seats, (self.seed % 7) as u8);
            if let Some(r) = self.pastimes.round.as_mut() {
                r.powder = Some(yard.clone());
            }
            for r in self.pastimes.heats.values_mut() {
                r.powder = Some(yard.clone());
            }
        }
        self.pastimes.heat = Some(Heat { game, seats: seats.to_vec(), done: BTreeMap::new(), standing: BTreeMap::new() });
    }

    pub(super) fn start_crossroads_pastime(&mut self, game: Game) {
        let seats = self.crossroads.as_ref().unwrap().seats.clone();
        let Some(&owner) = seats.first() else { return; };
        let at = game.start();
        let mut round = new_round(game, owner, at, 0, Some(&seats));
        if game == Game::Powder {
            round.powder = Some(Powder::new(&seats, (self.seed % 7) as u8));
            for k in &round.powder.as_ref().unwrap().knights {
                if let Some(h) = self.players.get_mut(&k.id) {
                    h.x = (f32::from(POWDER_X0) + f32::from(k.x) / 256.0) * TILE_UNITS;
                    h.y = (f32::from(POWDER_Y0) + f32::from(k.y) / 256.0 + 0.3) * TILE_UNITS;
                }
            }
            for &id in seats.iter().skip(1) {
                self.pastimes.heats.insert(id, round.clone());
            }
            self.home.countryside.games.powder.plays =
                self.home.countryside.games.powder.plays.saturating_add(1);
        } else {
            self.home.countryside.games.gander.plays =
                self.home.countryside.games.gander.plays.saturating_add(1);
        }
        self.pastimes.round = Some(round);
        self.pastimes.heat = Some(Heat {
            game,
            seats,
            done: BTreeMap::new(),
            standing: BTreeMap::new(),
        });
    }

    /// The crossroads keeps one yard, including after its starter is sooted.
    /// Survival decides places, then bales burned, then seat order.
    fn tick_crossroads_kegs(
        &mut self,
        inputs: &BTreeMap<u32, Input>,
        eligible: &[(u32, f32, f32)],
        tapped: &[u32],
    ) {
        let mut round = self.pastimes.round.take().unwrap();
        round.left = round.left.saturating_sub(1);
        let going = round.left > 0 && self.tick_kegs(&mut round, inputs, eligible, tapped);
        let yard = round.powder.as_ref().unwrap();
        let alive = yard.alive();
        let heat = self.pastimes.heat.as_mut().unwrap();
        let elapsed = round.game.time().saturating_sub(round.left);
        for &id in &heat.seats {
            let progress = yard.burned.get(&id).copied().unwrap_or(0).min(round.game.need());
            heat.standing.insert(id, progress);
            if !alive.contains(&id) {
                heat.done.entry(id).or_insert(Finish {
                    progress,
                    points: Some((elapsed * 999 / round.game.time()) as u16),
                });
            }
        }
        let over = !going || (heat.seats.len() > 1 && alive.len() <= 1)
            || (heat.seats.len() == 1 && round.progress >= round.game.need());
        if over {
            for &id in &heat.seats {
                heat.done.entry(id).or_insert(Finish {
                    progress: heat.standing.get(&id).copied().unwrap_or(0),
                    points: Some(1000),
                });
            }
            let heat = self.pastimes.heat.take().unwrap();
            self.pastimes.heats.clear();
            self.judge_heat(&heat);
        } else {
            for r in self.pastimes.heats.values_mut() { *r = round.clone(); }
            self.pastimes.round = Some(round);
        }
    }

    pub(crate) fn tick_pastimes(&mut self, inputs: &BTreeMap<u32, Input>) {
        let room = self.room().kind;
        if !Game::ALL.iter().any(|g| g.room() == room) {
            self.pastimes.leave();
            return;
        }
        // There are at most `MAX_PLAYERS` knights. Keep this per-tick working set on
        // the stack rather than allocating two vectors and a new held-key tree.
        let mut eligible = [(0u32, 0.0, 0.0); MAX_PLAYERS as usize];
        let mut count = 0;
        for (&id, h) in self
            .players
            .iter()
            .filter(|(id, h)| {
                self.crossroads
                    .as_ref()
                    .map_or(h.hp > 0 && !h.stone && h.privy == 0, |c| {
                        c.seats.contains(id)
                    })
            })
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
        if self.crossroads.is_some() && self.pastimes.round.as_ref().is_some_and(|r| r.game == Game::Powder) {
            self.tick_crossroads_kegs(inputs, eligible, tapped);
            return;
        }
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
                let rotation = (self.home.countryside.games.plays_for(game) % 5) as usize;
                self.note_play(game);
                // Everyone in the room is in: the starter plays the lead
                // round, each friend a round of their own.
                let seat_ids: Vec<u32> = eligible.iter().map(|&(id, _, _)| id).collect();
                self.pastimes.round = Some(new_round(
                    game,
                    starter,
                    (sx, sy),
                    rotation,
                    Some(&seat_ids),
                ));
                // The intro screen shows controls while players are invulnerable.
                #[cfg(not(test))]
                if let Some(r) = &mut self.pastimes.round {
                    r.intro_ticks = 30;
                }
                let mut seats = vec![starter];
                for &(id, x, y) in eligible {
                    if id != starter {
                        self.pastimes.heats.insert(id, new_round(game, id, (x, y), rotation, Some(&seat_ids)));
                        #[cfg(not(test))]
                        if let Some(r) = self.pastimes.heats.get_mut(&id) {
                            r.intro_ticks = 30;
                        }
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
                // Powder Keg: every knight plays in ONE shared courtyard, so
                // a keg can trap and soot a friend. Each seat's round
                // mirrors the same yard.
                if game == Game::Powder {
                    let ids = self.pastimes.heat.as_ref().unwrap().seats.clone();
                    let seed = self
                        .pastimes
                        .round
                        .as_ref()
                        .and_then(|r| r.powder.as_ref())
                        .map(|p| p.seed)
                        .unwrap_or(0);
                    let yard = Powder::new(&ids, seed);
                    if let Some(r) = self.pastimes.round.as_mut() {
                        r.powder = Some(yard.clone());
                    }
                    for r in self.pastimes.heats.values_mut() {
                        r.powder = Some(yard.clone());
                    }
                }
                // Door opens to signal the game beginning
                self.sounds.push("door_open");
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
        // The Gander Joust hall is one hall for the whole heat: it ticks
        // once, every seat riding together, before any seat's own round
        // is judged.
        if self.pastimes.round.as_ref().is_some_and(|r| r.game == Game::Gander) {
            let mut round = self.pastimes.round.take().unwrap();
            let seats = round.gander.as_ref().map(|g| g.seats.clone()).unwrap_or_default();
            let going = self.tick_gander(&mut round, inputs, eligible, tapped);
            // The hall's clock runs for the whole heat.
            round.left = round.left.saturating_sub(1);
            if let Some(heat) = self.pastimes.heat.as_mut() {
                let tally = round.gander.as_ref().map(|g| g.tally.clone());
                if let Some(tally) = tally {
                    for (k, &seat) in seats.iter().enumerate() {
                        let t = tally.get(k).copied().unwrap_or(0);
                        heat.standing.insert(seat, t.min(heat.game.need()));
                    }
                }
            }
            // The heat ends at the clock (or the need, for a single knight):
            // every seat finishes with its unhorsings in the shared hall.
            let over = !going
                || round.left == 0
                || round
                    .gander
                    .as_ref()
                    .is_some_and(|g| g.tally.iter().any(|&t| t >= round.game.need()));
            self.pastimes.round = Some(round);
            if over {
                self.finish_gander_heat();
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
            let Some(mut round) = round else {
                continue;
            };
            let mine: Vec<(u32, f32, f32)> =
                eligible.iter().copied().filter(|&(who, _, _)| who == id).collect();
            let taps: Vec<u32> = tapped.iter().copied().filter(|&who| who == id).collect();
            let played = if round.game == Game::Powder && n > 0 {
                // The shared courtyard advances once, on the starter's
                // round; every other seat's round mirrors its own knight's
                // standing in that one yard. Once the starter's round is
                // gone (they stepped out, or their knight was sooted), the
                // friend's round carries the yard itself.
                let lead = self.pastimes.round.as_ref();
                if let Some(p) = lead.and_then(|r| r.powder.as_ref()) {
                    let out = !p.alive().contains(&id);
                    if out || round.left == 0 {
                        Played::Over(Finish {
                            progress: round.progress,
                            points: None,
                        })
                    } else {
                        round.progress = p
                            .burned
                            .get(&id)
                            .copied()
                            .unwrap_or(0)
                            .min(round.game.need());
                        round.left = round.left.saturating_sub(1);
                        Played::Going(round)
                    }
                } else {
                    let all: Vec<(u32, f32, f32)> = eligible.to_vec();
                    self.play_round(round, inputs, &all, tapped)
                }
            } else if round.game == Game::Powder {
                // The shared courtyard's lead seat ticks the whole yard:
                // every knight's feet and taps count in the one yard.
                let all: Vec<(u32, f32, f32)> = eligible.to_vec();
                self.play_round(round, inputs, &all, tapped)
            } else if round.game.room() == room {
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
        // Ambient room tone: sparse background sounds that set the mood.
        self.push_ambient();
    }

    /// The shared Gander hall is over (clock, or the need): each seat
    /// finishes with its unhorsings, the most takes the crown and the
    /// winner's ride pays the purse.
    fn finish_gander_heat(&mut self) {
        let Some(round) = self.pastimes.round.take() else {
            return;
        };
        let Some(heat) = self.pastimes.heat.take() else {
            self.pastimes.round = Some(round);
            return;
        };
        self.pastimes.heats.clear();
        let Some(g) = round.gander.as_ref() else {
            return;
        };
        let mut finishes = BTreeMap::new();
        for (k, &seat) in g.seats.iter().enumerate() {
            let p = u8::try_from(g.tally.get(k).copied().unwrap_or(0))
                .unwrap_or(u8::MAX)
                .min(heat.game.need());
            finishes.insert(
                seat,
                Finish {
                    progress: p,
                    points: None,
                },
            );
        }
        let crowned = Heat {
            game: heat.game,
            seats: heat.seats.clone(),
            standing: heat.standing.clone(),
            done: finishes,
        };
        let winner = crowned.winner();
        if let Some(w) = winner {
            // The winner's ride pays, as a round-one heat did.
            let progress = u32::from(crowned.done.get(&w).map(|f| f.progress).unwrap_or(0));
            let result = (progress * 75 + round.left / HZ * 5)
                .saturating_sub(u32::from(round.mistakes) * 25)
                .clamp(25, 1000) as u16;
            let record = &mut self.home.countryside.games;
            record.gander.best = record.gander.best.max(result);
            record.gander.wins = record.gander.wins.saturating_add(1).min(record.gander.plays);
            self.score = self.score.saturating_add(u32::from(result));
            self.marks
                .insert(super::countryside::game_mark(round.game.index()), 1);
            self.found = Some((
                self.tick,
                0,
                format!("GANDER JOUST complete! +{result}. Best {}.", record.gander.best),
            ));
        } else {
            self.found = Some((self.tick, 0, "GANDER JOUST: time!".into()));
        }
        if winner.is_some() {
            self.sounds.push("spoil_pickup");
            self.sounds.push("gold");
        } else {
            self.sounds.push("clang");
            self.sounds.push("hero_hurt");
        }
        self.cues.push("pastime_won".into());
        self.judge_heat(&crowned);
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
        // A random room's game pays the room's purse (`encounter`).
        if self.encounter_hosts() {
            self.heat_to_encounter(heat);
            return;
        }
        if self.crossroads.is_some() {
            let mut seats = heat.seats.clone();
            seats.sort_by(|a, b| {
                let score = |id: &u32| {
                    heat.done
                        .get(id)
                        .map(|f| (f.points.is_some(), f.points.unwrap_or(0), f.progress))
                };
                score(b).cmp(&score(a)).then(a.cmp(b))
            });
            self.finish_crossroads(seats);
            return;
        }
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
        // While the intro is up the round is frozen: the clock does not run
        // and the players are invulnerable (no game logic advances). Fire skips.
        if round.intro_ticks > 0 {
            if inputs.values().any(|i| i.valid() && i.fire) {
                round.intro_ticks = 0;
            } else {
                round.intro_ticks -= 1;
                return Played::Going(round);
            }
        }
        // Track progress for milestone sounds
        let prev_progress = round.progress;
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
            // Failure sounds: clang + hero_hurt
            self.sounds.push("clang");
            self.sounds.push("hero_hurt");
            self.cues.push("pastime_lost".into());
            return lost(&round);
        }
        round.hazard_left = round.hazard_left.saturating_sub(1);
        round.score_popup = round.score_popup.saturating_sub(1);
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
            if round.game == Game::Barrel { self.sounds.push("hit"); }
            else { self.sounds.push("wave_gate"); }
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
                let before_lap=c.distance/LAP;
                let before_reload=c.reload;
                let result=c.step(input,tapped.contains(&c.owner));
                if c.reload > 0 && before_reload == 0 {self.sounds.push("vent_fire");}
                if c.bog>before {round.mistakes=round.mistakes.saturating_add(1).min(20);self.sounds.push("hero_hurt");}
                round.progress=(c.distance/LAP) as u8;
                if c.distance/LAP>before_lap {
                    round.score_popup = 8;
                    round.score_popup_text = "LAP!".to_string();
                    round.score_popup_x = 12;
                    round.score_popup_y = 6;
                    self.sounds.push("spoil_pickup");
                }
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
                let was_ground = h.flight == 0;
                let pump_axis = (if input.aim_x != 0 { input.aim_x } else { input.move_x }).signum();
                if let Some(landed) = h.step(input, tapped.contains(&h.owner), direction) {
                    if landed {
                        round.progress += 1;
                        round.score_popup = 8;
                        round.score_popup_text = "NICE!".to_string();
                        round.score_popup_x = 12;
                        round.score_popup_y = 6;
                        self.sounds.push("spoil_pickup");
                    } else {
                        round.mistakes += 1;
                        self.sounds.push("hero_hurt");
                        if round.mistakes == 3 {
                            self.found = Some((
                                self.tick,
                                h.owner,
                                "Three helmets dented! Step off J, then retry.".into(),
                            ));
                            return lost(&round);
                        }
                    }
                }
                // Pump sound: while on the ground actively pumping in the right direction
                if was_ground && !tapped.contains(&h.owner) && pump_axis == direction {
                    self.sounds.push("roll");
                }
                h.tick_vfx();
            }
            Game::Volley => {
                let volley = round.volley.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == volley.owner) {
                    self.found = Some((self.tick, 0, "Rally retired.".into()));
                    return lost(&round);
                }
                let input = inputs
                    .get(&volley.owner)
                    .filter(|i| i.valid())
                    .copied()
                    .unwrap_or_default();
                let beat = volley.step(input, tapped.contains(&volley.owner));
                round.progress = volley.bells.count_ones() as u8;
                round.streak = volley.combo;
                if beat.returned {
                    round.score_popup = 8;
                    round.score_popup_text = if beat.perfect {
                        format!("PERFECT x{}", volley.combo)
                    } else {
                        format!("KEEP IT UP x{}", volley.combo)
                    };
                    round.score_popup_x = 12;
                    round.score_popup_y = 6;
                    self.sounds.push("swing");
                    if beat.perfect {
                        self.sounds.push("spoil_pickup");
                    }
                }
                if beat.scored {
                    round.score_popup = 8;
                    round.score_popup_text = if round.progress == 4 {
                        "FINAL CHIME!".to_string()
                    } else {
                        format!("BELL {}!", round.progress)
                    };
                    round.score_popup_x = 12;
                    round.score_popup_y = 6;
                    self.sounds.push("circle_toll");
                }
                if beat.missed {
                    round.mistakes = round.mistakes.saturating_add(1).min(20);
                    self.sounds.push("clang");
                    self.found = Some((
                        self.tick,
                        volley.owner,
                        "Turnip down! Catch the next serve.".into(),
                    ));
                }
                if volley.lives == 0 {
                    self.found = Some((self.tick, volley.owner, "Three turnips lost!".into()));
                    return lost(&round);
                }
            }
            Game::Surf => {
                let surf = round.surf.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == surf.owner) {
                    self.found = Some((self.tick, 0, "Surfer retired.".into()));
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
                        round.score_popup = 8;
                        round.score_popup_text = "WAVE!".to_string();
                        round.score_popup_x = 12;
                        round.score_popup_y = 6;
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
                        self.sounds.push("hit");
                        return lost(&round);
                    }
                    None => {}
                }
            }
            Game::Peal => {
                let peal = round.peal.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == peal.owner) {
                    self.found = Some((
                        self.tick,
                        0,
                        "Ringer retired.".into(),
                    ));
                    return lost(&round);
                }
                // The pull only counts at the rope: the owner standing
                // at the start ring, as a turnip kick only counts at home.
                let at_rope = eligible.iter().any(|&(who, x, y)| {
                    who == peal.owner
                        && (x - Game::Peal.start().0 * TILE_UNITS)
                            .hypot(y - Game::Peal.start().1 * TILE_UNITS)
                            < 2.2
                });
                let prev_ring = peal.ring_flash;
                let prev_clang = peal.clang_flash;
                let closed = peal.step(tapped.contains(&peal.owner) && at_rope);
                if prev_ring == 0 && peal.ring_flash > 0 {
                    self.sounds.push("circle_toll");
                }
                if prev_clang == 0 && peal.clang_flash > 0 {
                    round.score_popup = 8;
                    round.score_popup_text = "TOO SOON!".to_string();
                    round.score_popup_x = 20;
                    round.score_popup_y = 6;
                    self.sounds.push("clang");
                }
                match closed {
                    Some(true) => {
                        round.progress += 1;
                        round.score_popup = 8;
                        round.score_popup_text = "RUNG!".to_string();
                        round.score_popup_x = 20;
                        round.score_popup_y = 6;
                    }
                    Some(false) => {
                        round.mistakes = round.mistakes.saturating_add(1).min(20);
                    }
                    None => {}
                }
                if round.progress < round.game.need()
                    && (peal.struck >= peal::STRIKES
                        || peal.struck - round.progress > peal::STRIKES - round.game.need())
                {
                    self.found = Some((
                        self.tick,
                        peal.owner,
                        "The peal fell apart.".into(),
                    ));
                    self.sounds.push("clang");
                    return lost(&round);
                }
            }
            Game::DuckShoot => {
                let range = round.duckshoot.as_mut().unwrap();
                if !eligible.iter().any(|(id, _, _)| *id == range.owner) {
                    self.found = Some((self.tick, 0, "Gun retired.".into()));
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
                    round.score_popup = 8;
                    round.score_popup_text = if rail == 2 { "LONG!" .to_string() } else { "HIT!".to_string() };
                    round.score_popup_x = 12;
                    round.score_popup_y = 4;
                    self.sounds.push("hit");
                } else if tapped.contains(&range.owner) {
                    self.sounds.push("shot_handgonne");
                    self.sounds.push("clang");
                }
                if range.shots >= 8 * 5 && round.progress < round.game.need() && range.shots == 40 {
                    // Out of BBs: the barker keeps the rest as rent.
                    self.found = Some((self.tick, range.owner, "Out of BBs!".into()));
                    return lost(&round);
                }
            }
            Game::Longbow => {
                if !self.tick_longbow(&mut round, inputs, eligible) {
                    return lost(&round);
                }
            }
            Game::Powder => {
                // The shared courtyard: every knight's tap counts, not only
                // this seat's.
                if !self.tick_kegs(&mut round, inputs, eligible, tapped) {
                    return lost(&round);
                }
            }
            Game::Gander => {
                // The shared hall ticks once for the whole heat, before the
                // per-seat loop (see `tick_pastimes`); nothing to do here.
            }
            Game::Siege => {
                if !self.tick_siege(&mut round, inputs, eligible) {
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
                    self.found = Some((self.tick, 0, "Squire parked.".into()));
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
                        self.found =
                            Some((self.tick, 0, format!("Missed gate {}!", round.progress + 1)));
                        self.sounds.push("clang");
                        return lost(&round);
                    }
                    round.progress += 1;
                    round.score_popup = 8;
                    round.score_popup_text = "GATE!".to_string();
                    round.score_popup_x = (gx as i32).clamp(2, 22);
                    round.score_popup_y = (gy as i32).clamp(2, 12);
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
                            round.score_popup = 8;
                            round.score_popup_text = "+75".to_string();
                            round.score_popup_x = MAILBOXES[i].0 as i32;
                            round.score_popup_y = MAILBOXES[i].1 as i32;
                            self.sounds.push("card_pickup");
                            self.found = Some((
                                self.tick,
                                id,
                                "Delivered: 'Local knight mistakes goose for dragon'.".into(),
                            ));
                        } else {
                            round.mistakes = round.mistakes.saturating_add(1).min(20);
                            self.sounds.push("clang");
                            self.found = Some((self.tick, id, "Not a subscriber! -25".into()));
                        }
                    }
                }
            }
            Game::Barrel | Game::Washtub => {
                // Paddle stroke sound for Washtub: play "wave_gate" when moving, once every half-second
                if round.game == Game::Washtub && self.tick % u64::from(HZ / 2) == 0 {
                    let moving = eligible.iter().any(|&(id, _, _)| {
                        inputs.get(&id).filter(|i| i.valid()).is_some_and(|i| {
                            i.move_x != 0 || i.move_y != 0 || i.aim_x != 0 || i.aim_y != 0
                        })
                    });
                    if moving { self.sounds.push("wave_gate"); }
                }
                let i = usize::from(round.progress);
                let target = if round.game == Game::Barrel {
                    FLAGS[i]
                } else {
                    wake(i, self.tick)
                };
                if near(target, 1.5) {
                    round.progress += 1;
                    round.score_popup = 8;
                    round.score_popup_text = if round.game == Game::Barrel { "FLAG!".to_string() } else { "RING!".to_string() };
                    round.score_popup_x = target.0 as i32;
                    round.score_popup_y = target.1 as i32;
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
                        round.score_popup = 8;
                        round.score_popup_text = "HIT!".to_string();
                        round.score_popup_x = basket.0 as i32;
                        round.score_popup_y = basket.1 as i32;
                        self.sounds.push("hit");
                    } else if !cheese.valid()
                        || matches!(
                            self.room().tile(x.floor() as i32, y.floor() as i32),
                            Tile::Wall | Tile::Block
                        )
                    {
                        round.mistakes += 1;
                        self.sounds.push("rock_land");
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
                        self.sounds.push("roll");
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
                    round.score_popup = 8;
                    round.score_popup_text = "CLEAR!".to_string();
                    round.score_popup_x = HURDLES[usize::from(round.progress - 1)] as i32;
                    round.score_popup_y = 7;
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
                    self.sounds.push("swing");
                    if phase <= HZ / 5 {
                        round.progress += 1;
                        round.score_popup = 8;
                        round.score_popup_text = "PERFECT!".to_string();
                        round.score_popup_x = Game::Turnip.start().0 as i32;
                        round.score_popup_y = Game::Turnip.start().1 as i32;
                        self.sounds.push("spoil_pickup");
                    } else {
                        round.mistakes = round.mistakes.saturating_add(1).min(20);
                        self.found = Some((self.tick, 0, "Too soon! Kick as it lands.".into()));
                    }
                }
            }
        }
        // Score milestone sounds and combo/streak tracking
        let need = round.game.need();
        if need > 0 && round.progress > prev_progress && round.game != Game::Volley {
            // Progress increased: increment streak
            round.streak = round.streak.saturating_add(1).min(20);
            // 50% milestone
            let halfway = need / 2;
            if prev_progress < halfway && round.progress >= halfway {
                self.sounds.push("card_pickup");
            }
            // 100% milestone (anticipation before win)
            if prev_progress < need && round.progress >= need {
                self.sounds.push("wish_grant");
            }
            // Combo streak sound at 3+ consecutive successes
            if round.streak >= 3 {
                self.sounds.push("spell_cast");
            }
        } else if round.progress == prev_progress && round.game != Game::Volley {
            // No progress this tick: reset streak
            round.streak = 0;
        }
        if round.game != Game::Gander && round.progress >= round.game.need() {
            let result = if round.game == Game::Siege {
                // The trail's purse: damage on the keeps, then a bounty for
                // every soldier still standing at the end.
                let damage = round
                    .siege
                    .as_ref()
                    .map(|s| u32::from(s.damage.min(u16::MAX)))
                    .unwrap_or(0);
                let men = round
                    .siege
                    .as_ref()
                    .map(|s| u32::from(s.soldiers.min(u8::MAX)))
                    .unwrap_or(0);
                (damage * 5 + men * 25).min(1000) as u16
            } else {
                (u32::from(round.progress) * 75
                    + round.left / HZ * 5
                    + if round.game == Game::Volley {
                        u32::from(round.volley.as_ref().map_or(0, |v| v.best_combo)) * 5
                    } else {
                        0
                    })
                .saturating_sub(u32::from(round.mistakes) * 25)
                .clamp(25, 1000) as u16
            };
            let record = &mut self.home.countryside.games;
            let prev_best = record.best_for(round.game);
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
            } else if round.game == Game::Powder {
                record.powder.best = record.powder.best.max(result);
                record.powder.wins = record.powder.wins.saturating_add(1).min(record.powder.plays);
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
            } else if round.game == Game::Gander {
                record.gander.best = record.gander.best.max(result);
                record.gander.wins = record.gander.wins.saturating_add(1).min(record.gander.plays);
            } else if round.game == Game::Siege {
                record.siege.best = record.siege.best.max(result);
                record.siege.wins = record.siege.wins.saturating_add(1).min(record.siege.plays);
            } else if round.game == Game::Peal {
                record.peal.best = record.peal.best.max(result);
                record.peal.wins = record.peal.wins.saturating_add(1).min(record.peal.plays);
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
            // New high score fanfare, otherwise celebration
            let is_new_best = result > prev_best;
            if is_new_best {
                self.sounds.push("wish_grant");
                self.sounds.push("gold");
            } else {
                self.sounds.push("spoil_pickup");
                self.sounds.push("gold");
            }
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
#[path = "../../tests/rules/together_kegs__tests.rs"]
mod kegs_tests;

#[cfg(test)]
#[path = "../../tests/rules/together_heats__tests.rs"]
mod heat_tests;

#[cfg(test)]
#[path = "../../tests/rules/together_games_order__tests.rs"]
mod games_order_tests;

#[cfg(test)]
#[path = "../../tests/rules/together_siege__tests.rs"]
mod siege_tests;

#[cfg(test)]
#[path = "../../tests/rules/together_belfry__tests.rs"]
mod belfry_tests;
#[cfg(all(test, feature = "bend-cases-games-b"))]
#[path = "../../tests/rules/pastimes_bend_cases__tests.rs"]
mod pastimes_bend_cases_tests;
