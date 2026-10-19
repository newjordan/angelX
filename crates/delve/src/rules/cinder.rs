//! Cinder Stack
//!
//! The feeling: dropping the last blue pot and watching a whole kiln crack open.
//! First ten seconds: slide a clay pair, drop it, and hear three matching pots pop.
//! Loop: steer a falling pair, turn with F, drop with E, match three to crack
//! the kiln spirit while its marked column fills with slag; first crack wins.
//! Words: MATCH THREE POTS. Every seated knight plays on their own screen.
//! Five numbers (30 Hz), proposed then tuned through the live server knobs:
//! cinder_fall = 20 ticks/cell; cinder_slide = 5 ticks/repeat;
//! cinder_lock = 10 ticks grounded; cinder_tell = 27 ticks wind-up;
//! cinder_attack = 165 ticks between attacks (late phases shorten the rest).
//! Miss: the kiln coughs and resets after 15 ticks; score damage is retained.
//! Version one: one Alchemy Lab room, colour triples, one spirit, no inventory,
//! no between-player garbage, no long campaign, no ante menu.
//! Stake: every successful round pays its winner 10 gold per seated knight.
//! Crown brings the existing extra bomb into the next dungeon. Equal cracks
//! rank by the tick they were reached; exact ties share the purse and crown.
//!
//! Feel ledger, in priority order: drop -> landing -> lock ticks; match ->
//! pop/cascade -> fall ticks; rival -> precise column tell -> tell ticks;
//! slide -> tap/repeat -> slide ticks; turn -> pair pivots -> lock ticks;
//! chain -> extra cracks -> fall ticks; miss -> cough/retry -> 15 ticks;
//! lead -> live standings -> attack ticks; win -> crown/purse -> 25 gold;
//! replay -> E -> one tick. The first three define the room.

use super::{BTreeMap, Deserialize, HZ, Input, MAX_PLAYERS, RoomKind, Run, Serialize, Spoil};

pub const COLS: usize = 6;
pub const ROWS: usize = 10;
/// Lower: pairs descend sooner; hard drop remains immediate.
pub const FALL: u32 = 20;
/// Lower: a held direction slides to the next column sooner.
pub const SLIDE: u32 = 5;
/// Higher: more time to turn after touching the stack.
pub const LOCK: u32 = 10;
/// Higher: more time to read the exact marked slag landing.
pub const TELL: u32 = 27;
/// Lower: less rest between single slag drops; rage removes 60 ticks.
pub const ATTACK: u32 = 165;
pub const CRACKS: u16 = 36;
pub const LENGTH: u32 = 60 * HZ;
pub const RETRY: u32 = 15;
/// Per-seat winner purse; each finished round settles exactly once.
pub const GOLD_PER_SEAT: u32 = 10;
pub const KILN: (f32, f32) = (40.0, 20.0);
const PAIRS: [[u8; 2]; 9] = [
    [1, 2],
    [2, 3],
    [3, 1],
    [1, 1],
    [2, 2],
    [3, 3],
    [2, 1],
    [3, 2],
    [1, 3],
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub cells: [[u8; COLS]; ROWS],
    #[serde(default)]
    pub seed: u64,
    /// Tick when the current crack total was reached (the visible tiebreak).
    #[serde(default)]
    pub crack_tick: u32,
    pub x: i8,
    pub y: i8,
    pub turn: u8,
    pub piece: u16,
    pub fall: u32,
    pub lock: u32,
    pub slide: u32,
    pub direction: i8,
    pub fire_key: bool,
    pub drop_key: bool,
    pub cracks: u16,
    pub misses: u16,
    pub retry: u32,
    pub attack: u32,
    pub tell: u32,
    pub windup: u32,
    pub column: u8,
    pub attacks: u16,
    /// Cleared cells stay visible during the pop, before gravity settles.
    pub pop: [[bool; COLS]; ROWS],
    pub flash: u32,
    pub chain: u8,
    pub impact: u32,
    pub impact_row: u8,
}
impl Default for Board {
    fn default() -> Self {
        let mut b = Self {
            cells: [[0; COLS]; ROWS],
            seed: 0,
            crack_tick: 0,
            x: 2,
            y: 1,
            turn: 0,
            piece: 0,
            fall: 0,
            lock: 0,
            slide: 0,
            direction: 0,
            fire_key: false,
            drop_key: true,
            cracks: 0,
            misses: 0,
            retry: 0,
            attack: 0,
            tell: 0,
            windup: 0,
            column: 0,
            attacks: 0,
            pop: [[false; COLS]; ROWS],
            flash: 0,
            chain: 0,
            impact: 0,
            impact_row: 9,
        };
        b.seed();
        b
    }
}
impl Board {
    pub fn seeded(seed: u64) -> Self {
        let mut b = Self {
            seed,
            ..Self::default()
        };
        b.seed();
        b
    }
    fn seed(&mut self) {
        self.cells = [[0; COLS]; ROWS];
        let mut rng = super::loot::Rng::new(self.seed);
        // Two of each colour in each row prevents horizontal triples;
        // two rows cannot contain a vertical triple before the first pair lands.
        for row in ROWS - 2..ROWS {
            let mut pots = [1, 1, 2, 2, 3, 3];
            for col in (1..COLS).rev() {
                pots.swap(col, rng.below(col + 1));
            }
            self.cells[row] = pots;
        }
    }
    pub fn height(&self) -> usize {
        self.cells
            .iter()
            .position(|r| r.iter().any(|&c| c != 0))
            .map_or(0, |y| ROWS - y)
    }
    pub fn crack_stage(&self) -> usize {
        usize::from((self.cracks / 12).min(3))
    }
    pub fn finish_tick(&self) -> Option<u32> {
        (self.cracks == CRACKS).then_some(self.crack_tick)
    }
    pub fn colours(&self) -> [u8; 2] {
        PAIRS[self.piece as usize % PAIRS.len()]
    }
    pub fn next(&self) -> [u8; 2] {
        PAIRS[(self.piece as usize + 1) % PAIRS.len()]
    }
    pub fn pair_at(x: i8, y: i8, turn: u8) -> [(i8, i8); 2] {
        let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)][turn as usize % 4];
        [(x, y), (x + dx, y + dy)]
    }
    fn fits(&self, x: i8, y: i8, turn: u8) -> bool {
        Self::pair_at(x, y, turn).iter().all(|&(x, y)| {
            (0..COLS as i8).contains(&x)
                && (0..ROWS as i8).contains(&y)
                && self.cells[y as usize][x as usize] == 0
        })
    }
    pub fn ghost(&self) -> i8 {
        let mut y = self.y;
        while self.fits(self.x, y + 1, self.turn) {
            y += 1;
        }
        y
    }
    pub fn valid(&self) -> bool {
        self.cells.iter().flatten().all(|&c| c <= 4)
            && (0..COLS as i8).contains(&self.x)
            && (0..ROWS as i8).contains(&self.y)
            && self.turn < 4
            && (-1..=1).contains(&self.direction)
            && self.fall <= 60
            && self.lock <= 30
            && self.slide <= 15
            && self.retry <= RETRY
            && self.tell <= 60
            && self.windup <= 60
            && self.attack <= 300
            && self.column < COLS as u8
            && self.flash <= 8
            && self.chain <= 20
            && self.impact <= 12
            && self.impact_row < ROWS as u8
            && self.crack_tick <= LENGTH
            && self.cracks <= CRACKS
            && (self.retry > 0 || self.flash > 0 || self.fits(self.x, self.y, self.turn))
    }
    fn spawn(&mut self) {
        self.piece = self.piece.wrapping_add(1);
        self.x = 2;
        self.y = 1;
        self.turn = 0;
        self.fall = 0;
        self.lock = 0;
        if !self.fits(self.x, self.y, self.turn) {
            self.miss();
        }
    }
    fn miss(&mut self) {
        self.misses = self.misses.saturating_add(1);
        self.retry = RETRY;
        self.flash = 0;
        self.pop = [[false; COLS]; ROWS];
        self.chain = 0;
        self.tell = 0;
        self.attack = 0;
    }
    fn gravity(&mut self) {
        for x in 0..COLS {
            let mut bottom = ROWS;
            for y in (0..ROWS).rev() {
                if self.cells[y][x] != 0 {
                    bottom -= 1;
                    self.cells[bottom][x] = self.cells[y][x];
                    if bottom != y {
                        self.cells[y][x] = 0;
                    }
                }
            }
        }
    }
    fn matches(&mut self) -> bool {
        self.pop = [[false; COLS]; ROWS];
        for y in 0..ROWS {
            for x in 0..COLS {
                let c = self.cells[y][x];
                if !(1..=3).contains(&c) {
                    continue;
                }
                if x + 2 < COLS && self.cells[y][x + 1] == c && self.cells[y][x + 2] == c {
                    for dx in 0..3 {
                        self.pop[y][x + dx] = true;
                    }
                }
                if y + 2 < ROWS && self.cells[y + 1][x] == c && self.cells[y + 2][x] == c {
                    for dy in 0..3 {
                        self.pop[y + dy][x] = true;
                    }
                }
            }
        }
        let n = self.pop.iter().flatten().filter(|&&p| p).count();
        if n == 0 {
            return false;
        }
        // A match cracks neighbouring slag; slag never matches by itself.
        let pots = self.pop;
        for y in 0..ROWS {
            for x in 0..COLS {
                if self.cells[y][x] == 4
                    && [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)]
                        .iter()
                        .any(|&(dx, dy)| {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            (0..COLS as i32).contains(&nx)
                                && (0..ROWS as i32).contains(&ny)
                                && pots[ny as usize][nx as usize]
                        })
                {
                    self.pop[y][x] = true;
                }
            }
        }
        self.chain = (self.chain + 1).min(20);
        self.cracks = (self.cracks + n as u16 * u16::from(self.chain)).min(CRACKS);
        self.flash = 8;
        true
    }
    fn land(&mut self) {
        let colours = self.colours();
        for (i, (x, y)) in Self::pair_at(self.x, self.y, self.turn)
            .into_iter()
            .enumerate()
        {
            self.cells[y as usize][x as usize] = colours[i];
        }
        self.gravity();
        self.chain = 0;
        if !self.matches() {
            self.spawn();
        }
    }
    /// One integer tick; the return value names an existing sound cue.
    pub fn step(&mut self, input: Input, tune: &super::tune::Tune) -> Option<&'static str> {
        let input = if input.valid() {
            input
        } else {
            Input::default()
        };
        let rotate = input.fire && !self.fire_key;
        let drop = input.interact && !self.drop_key;
        self.fire_key = input.fire;
        self.drop_key = input.interact;
        self.impact = self.impact.saturating_sub(1);
        if self.retry > 0 {
            self.retry -= 1;
            if self.retry == 0 {
                self.seed();
                self.spawn();
            }
            return None;
        }
        // Resolve a cascade before releasing the next pair or a pending attack.
        if self.flash > 0 {
            self.flash -= 1;
            if self.flash == 0 {
                for y in 0..ROWS {
                    for x in 0..COLS {
                        if self.pop[y][x] {
                            self.cells[y][x] = 0;
                        }
                    }
                }
                self.pop = [[false; COLS]; ROWS];
                self.gravity();
                if !self.matches() {
                    self.spawn();
                } else {
                    return Some("spoil_pickup");
                }
            }
            return None;
        }
        let mut sound = None;
        if self.tell > 0 {
            self.tell -= 1;
            if self.tell == 0 {
                let x = self.column as usize;
                if let Some(y) = (0..ROWS).rev().find(|&y| self.cells[y][x] == 0) {
                    self.cells[y][x] = 4;
                    self.impact_row = y as u8;
                    self.impact = 12;
                    self.attacks = self.attacks.saturating_add(1);
                    // A block placed into the active pair is a visible kiln cough.
                    if !self.fits(self.x, self.y, self.turn) {
                        self.miss();
                    }
                } else {
                    self.miss();
                }
                sound = Some("clang");
            }
        } else {
            self.attack += 1;
            let phase = (self.cracks / (CRACKS / 3)).min(2) as u32;
            if self.attack >= tune.cinder_attack.saturating_sub(phase * 30) {
                self.attack = 0;
                self.column = ((self.attacks as usize * 5 + 1) % COLS) as u8;
                self.tell = tune.cinder_tell;
                self.windup = tune.cinder_tell;
                sound = Some("play_card");
            }
        }
        if self.retry > 0 {
            return sound;
        }
        let direction = input.move_x;
        self.slide = self.slide.saturating_sub(1);
        if direction != 0 && (direction != self.direction || self.slide == 0) {
            if self.fits(self.x + direction, self.y, self.turn) {
                self.x += direction;
            }
            self.slide = tune.cinder_slide;
        }
        self.direction = direction;
        if rotate {
            let turn = (self.turn + 1) % 4;
            for kick in [0, -1, 1] {
                if self.fits(self.x + kick, self.y, turn) {
                    self.x += kick;
                    self.turn = turn;
                    break;
                }
            }
        }
        if drop {
            self.y = self.ghost();
            self.land();
            return Some(if self.flash > 0 {
                "spoil_pickup"
            } else {
                "clang"
            });
        }
        self.fall += 1;
        if self.fits(self.x, self.y + 1, self.turn) {
            self.lock = 0;
            if self.fall >= tune.cinder_fall {
                self.y += 1;
                self.fall = 0;
            }
        } else {
            self.fall = 0;
            self.lock += 1;
            if self.lock >= tune.cinder_lock {
                self.land();
                sound = Some(if self.flash > 0 {
                    "spoil_pickup"
                } else {
                    "clang"
                });
            }
        }
        sound
    }
    /// A small deterministic playtester: evaluate every legal landing by clears,
    /// height and cavities. It uses only this seat's published board.
    pub fn bot_input(&self) -> Input {
        if self.flash > 0 || self.retry > 0 {
            return Input::default();
        }
        let mut best = (i32::MIN, self.x, self.turn);
        for turn in 0..4 {
            for x in 0..COLS as i8 {
                let mut b = self.clone();
                b.x = x;
                b.turn = turn;
                if !b.fits(x, b.y, turn) {
                    continue;
                }
                b.y = b.ghost();
                b.land();
                let gain = i32::from(b.cracks) - i32::from(self.cracks);
                let mut height = 0;
                let mut holes = 0;
                for x in 0..COLS {
                    let mut occupied = false;
                    for y in 0..ROWS {
                        if b.cells[y][x] > 0 {
                            occupied = true;
                            height += (ROWS - y) as i32;
                        } else if occupied {
                            holes += 1;
                        }
                    }
                }
                let score =
                    gain * 50 - height * 2 - holes * 20 - (i32::from(x) - i32::from(self.x)).abs();
                if score > best.0 {
                    best = (score, x, turn);
                }
            }
        }
        if self.x != best.1 {
            Input {
                move_x: (best.1 - self.x).signum(),
                ..Default::default()
            }
        } else if self.turn != best.2 {
            Input {
                fire: !self.fire_key,
                ..Default::default()
            }
        } else {
            Input {
                interact: !self.drop_key,
                ..Default::default()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub boards: BTreeMap<u32, Board>,
    pub left: u32,
    pub order: Vec<u32>,
    pub key: bool,
    pub won: bool,
    #[serde(default)]
    pub seed: u64,
}
impl Session {
    pub fn new(seats: impl Iterator<Item = u32>, seed: u64) -> Self {
        Self {
            boards: seats.map(|id| (id, Board::seeded(seed))).collect(),
            seed,
            left: LENGTH,
            order: Vec::new(),
            key: true,
            won: false,
        }
    }
    pub fn place(&self, id: u32) -> usize {
        let b = &self.boards[&id];
        1 + self
            .order
            .iter()
            .filter(|who| {
                let rival = &self.boards[who];
                (std::cmp::Reverse(rival.cracks), rival.crack_tick)
                    < (std::cmp::Reverse(b.cracks), b.crack_tick)
            })
            .count()
    }
    /// Exact ties split the whole purse, with spare gold in seat order.
    pub fn payout(&self, id: u32) -> u32 {
        let Some(&first) = self.order.first().filter(|_| self.won) else {
            return 0;
        };
        let score = |who: &u32| (self.boards[who].cracks, self.boards[who].crack_tick);
        if score(&id) != score(&first) {
            return 0;
        }
        let tied = self
            .boards
            .keys()
            .filter(|who| score(who) == score(&first))
            .count() as u32;
        let earlier = self
            .boards
            .range(..id)
            .filter(|(who, _)| score(who) == score(&first))
            .count() as u32;
        let total = GOLD_PER_SEAT * self.boards.len() as u32;
        total / tied + u32::from(earlier < total % tied)
    }
    pub fn valid(&self) -> bool {
        !self.boards.is_empty()
            && self.boards.len() <= MAX_PLAYERS as usize
            && self
                .boards
                .iter()
                .all(|(&id, b)| (1..=MAX_PLAYERS).contains(&id) && b.valid())
            && self.left <= LENGTH
            && self.order.len() <= self.boards.len()
            && self.order.iter().all(|id| self.boards.contains_key(id))
            && self
                .order
                .iter()
                .enumerate()
                .all(|(i, id)| !self.order[..i].contains(id))
            && (self.left > 0 || self.order.len() == self.boards.len())
    }
    pub fn repair(&mut self) {
        self.boards.retain(|&id, _| (1..=MAX_PLAYERS).contains(&id));
        for b in self.boards.values_mut() {
            if !b.valid() {
                *b = Board::seeded(self.seed);
            }
        }
        if self.boards.is_empty() {
            self.boards.insert(1, Board::seeded(self.seed));
        }
        self.left = self.left.clamp(1, LENGTH);
        self.order.clear();
        self.won = false;
    }
}
impl Run {
    pub fn cinder_showing(&self) -> bool {
        self.room().kind == RoomKind::CollegeLab && self.cinder.is_some()
    }
    pub(super) fn tick_cinder(&mut self, inputs: &BTreeMap<u32, Input>) {
        if self.room().kind != RoomKind::CollegeLab {
            self.cinder = None;
            return;
        }
        let pressed = inputs.iter().any(|(id, i)| {
            i.valid()
                && i.interact
                && self.players.get(id).is_some_and(|h| {
                    h.hp > 0
                        && !h.stone
                        && !self.exits.outside.contains_key(id)
                        && (h.x - KILN.0).hypot(h.y - KILN.1) < 3.0
                })
        });
        if self.cinder.is_none() {
            if pressed {
                self.start_cinder();
            }
            return;
        }
        let mut s = self.cinder.take().unwrap();
        s.boards
            .retain(|id, _| self.players.contains_key(id) && !self.exits.outside.contains_key(id));
        if s.boards.is_empty() {
            return;
        }
        s.order.retain(|id| s.boards.contains_key(id));
        if s.left == 0 {
            let take = inputs
                .iter()
                .any(|(id, i)| s.boards.contains_key(id) && i.valid() && i.interact);
            if take && !s.key {
                self.start_cinder();
            } else {
                s.key = take;
                self.cinder = Some(s);
            }
            return;
        }
        s.left -= 1;
        for (&id, b) in &mut s.boards {
            if self.players.get(&id).is_none_or(|h| h.away || h.stone)
                || self.exits.outside.contains_key(&id)
            {
                continue;
            }
            let i = inputs.get(&id).copied().unwrap_or_default();
            let cracks = b.cracks;
            if let Some(sound) = b.step(i, &self.tune) {
                self.sounds.push(sound);
            }
            if b.cracks != cracks {
                b.crack_tick = LENGTH - s.left;
            }
        }
        let cracked = s.boards.values().any(|b| b.cracks >= CRACKS);
        if cracked || s.left == 0 {
            s.order = s.boards.keys().copied().collect();
            s.order.sort_by_key(|id| {
                (
                    std::cmp::Reverse(s.boards[id].cracks),
                    s.boards[id].crack_tick,
                    *id,
                )
            });
            s.left = 0;
            s.won = cracked;
            s.key = true;
            let winner = s.order[0];
            let best = s.boards[&winner].cracks;
            let record = &mut self.home.countryside.games.cinder;
            record.best = record.best.max(best);
            if cracked {
                record.wins = record.wins.saturating_add(1).min(record.plays);
                let first = &s.boards[&winner];
                let winners: Vec<_> = s
                    .order
                    .iter()
                    .copied()
                    .filter(|id| {
                        let b = &s.boards[id];
                        (b.cracks, b.crack_tick) == (first.cracks, first.crack_tick)
                    })
                    .collect();
                for (&id, h) in &mut self.players {
                    if s.boards.contains_key(&id) {
                        h.laurel = winners.contains(&id);
                        if h.laurel {
                            h.booby = None;
                            h.carried.add(Spoil::Gold, s.payout(id));
                        }
                    }
                }
            }
            self.sounds.push("play_card");
        }
        self.cinder = Some(s);
    }
    fn start_cinder(&mut self) {
        let seed = self.rng.next();
        let seats = self
            .players
            .iter()
            .filter(|(id, h)| {
                h.hp > 0 && !h.stone && !h.away && !self.exits.outside.contains_key(id)
            })
            .map(|(&id, _)| id);
        let session = Session::new(seats, seed);
        if session.boards.is_empty() {
            return;
        }
        self.cinder = Some(session);
        let record = &mut self.home.countryside.games.cinder;
        record.plays = record.plays.saturating_add(1);
        self.sounds.push("play_card");
    }
    #[cfg(any(test, feature = "testing"))]
    pub fn cinder_for_test() -> Run {
        let mut run = Run::at_home(
            41,
            1,
            Some("Gawain"),
            Default::default(),
            Default::default(),
        );
        let at = run
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == RoomKind::CollegeLab)
            .unwrap();
        run.arrive(at, (KILN.0 / 2.0, KILN.1 / 2.0));
        run
    }
}
