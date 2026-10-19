//! Follies: a room of the delve that plays like another game. The first is
//! the Wax Maze, a nod to Pac-Man in our own crypt: the room's pillars make
//! a maze strewn with candle wax, four wraiths roam it, and every knight in
//! the room munches at once. The wax is shared, so friends race each other
//! for the corridors; each knight counts their own.
//!
//! The maze grows with the party, as a fight room does (`party_room`): one
//! screen for a lone knight, four for a full table, the camera following
//! each knight through it. Its lanes are one tile wide, every one of them
//! reachable, with no dead ends; it is the same left and right, and the
//! wraiths' crypt is the pillar in its middle. Knights glide its lanes as
//! Pac-Man does (`WaxMaze::glide`): kept to the middle of a lane, a turn
//! pressed early taken at the next opening, one pressed a moment late
//! still taken, a wall ahead a stop in the middle of the last tile.
//!
//! Each wraith has a temperament: the Hound chases the nearest knight, the
//! Cutter heads for where that knight is going, the Flanker comes from the
//! far side of the Hound, and the Drifter chases from afar but drifts off
//! to its corner when it comes near. Through the candle they keep to their
//! corners a while, then hunt, by turns (`MOODS`); once its first stretch
//! has burnt they quicken (`LATE`). Holy water in the corners turns them
//! pale and fleeing for a while, flashing as it wears off; a pale wraith
//! caught is worth wax (more for each caught on the same water) and a
//! bone, and its eyes race home to the crypt, where it rests and comes out
//! calm. A calm wraith that touches a knight spills some of their wax and
//! sends them back to where they came in, left alone a moment so no wraith
//! waits for them there; nobody is hurt. Fresh wax slows its eater a
//! little (`EAT_PACE`), so an eaten lane is the quick way, as in the
//! arcade.
//!
//! The even floors' folly is the Belfry Shoot instead (below).
//!
//! Stakes: each knight antes a tenth of the gold they carry. When the wax is
//! gone or the candle burns down, the most wax takes the pot and the crown;
//! everyone keeps a gold for every five drops they ate.
use super::*;
use std::collections::{BTreeSet, VecDeque};

/// How long the maze lasts, and the title card before it starts (ticks).
pub const MAZE_TIME: u32 = 75 * HZ;
pub const TITLE: u32 = 2 * HZ;
/// How long holy water keeps the wraiths pale, and the end of it when they
/// flash its warning.
pub const PALE: u32 = 6 * HZ;
pub const PALE_WARN: u32 = 2 * HZ;
/// Ticks a wraith takes to cross a tile: calm, calm in the candle's last
/// stretch, pale, its eyes racing home, and climbing out of the crypt.
/// Slower is easier; a knight crosses one in about four and a half.
pub const WRAITH_STEP: u32 = 6;
pub const LATE_STEP: u32 = 5;
pub const PALE_STEP: u32 = 10;
pub const EYES_STEP: u32 = 2;
pub const LEAVE_STEP: u32 = 8;
/// The candle's last stretch, when the calm wraiths quicken (ticks of play
/// left): all but its first twenty-five seconds.
pub const LATE: u32 = 50 * HZ;
/// Wax a wraith's touch spills, and wax the pale wraiths caught on one holy
/// water are worth, the first to the fourth.
pub const SPILL: u16 = 5;
pub const CATCH: [u16; 4] = [5, 10, 15, 20];
/// A wraith caught rests in the crypt this long before it roams again.
pub const REST: u32 = 3 * HZ;
/// When each wraith first climbs out of the crypt, after the title card:
/// the Hound at once, then the Cutter, the Flanker and the Drifter.
pub const RELEASE: [u32; 4] = [0, HZ, 3 * HZ, 5 * HZ];
/// The wraiths' moods through the candle, after the title card: so long in
/// their corners, so long hunting, by turns; the last hunt lasts to the end.
pub const MOODS: [u32; 5] = [5 * HZ, 20 * HZ, 5 * HZ, 20 * HZ, 4 * HZ];
/// The Cutter heads this many tiles ahead of its knight; the Flanker
/// reckons from this many ahead, doubled from the Hound; the Drifter keeps
/// to its corner within this many tiles of a knight.
pub const CUT_AHEAD: i32 = 4;
pub const FLANK_AHEAD: i32 = 2;
pub const SHY: i32 = 8;
/// A knight's ante, in tenths of the gold they carry.
pub const ANTE_TENTHS: u32 = 1;
/// Drops of wax that make a gold to keep.
pub const WAX_PER_GOLD: u16 = 5;
/// After a touch, how long the wraiths leave a knight alone: they neither
/// chase nor touch them.
pub const SPOOKED: u32 = 2 * HZ;
/// How near a wraith comes to touch a knight, in tiles between their
/// middles: about where their shapes meet.
pub const TOUCH: f32 = 0.6;
/// How far from a crossing's middle a knight's turn is still taken (units;
/// a tile is two).
pub const TURN_WINDOW: f32 = 0.9;
/// A knight's pace munching through wax, as a share of their own: a lane
/// already eaten is the quick way, as in the arcade.
pub const EAT_PACE: f32 = 0.85;
/// Of the inner lanes, the share (in percent) offered for closing when the
/// maze is laid: more makes longer walls and fewer crossings.
pub const CLOSE: u32 = 45;

/// A wraith's temperament.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Temper {
    Hound,
    Cutter,
    Flanker,
    Drifter,
}

impl Temper {
    pub const ALL: [Temper; 4] = [Temper::Hound, Temper::Cutter, Temper::Flanker, Temper::Drifter];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wraith {
    pub temper: Temper,
    /// The tile it is on and the tile it is going to.
    pub at: (i32, i32),
    pub to: (i32, i32),
    /// Ticks into the step from `at` to `to`.
    pub step: u32,
    /// Ticks left resting in the crypt.
    pub rest: u32,
    /// Ticks the step from `at` to `to` takes: its pace when it set out.
    #[serde(default)]
    pub span: u32,
    /// Turned pale by the holy water now running.
    #[serde(default)]
    pub pale: bool,
    /// Caught: only its eyes, racing home to the crypt.
    #[serde(default)]
    pub eyes: bool,
    /// Its place in the crypt.
    #[serde(default)]
    pub home: (i32, i32),
    /// The tile it came from, which it does not go back to.
    #[serde(default)]
    pub came: (i32, i32),
}

impl Wraith {
    /// Where it is now, in tiles (fractional, between `at` and `to`; a
    /// tile's middle is its corner and a half).
    pub fn pos(&self) -> (f32, f32) {
        let t = (self.step as f32 / self.span.max(1) as f32).min(1.0);
        (
            self.at.0 as f32 + (self.to.0 - self.at.0) as f32 * t + 0.5,
            self.at.1 as f32 + (self.to.1 - self.at.1) as f32 * t + 0.5,
        )
    }
    /// Out of its rest, eyes and all: going somewhere.
    pub fn about(&self) -> bool {
        self.rest == 0
    }
    /// Abroad in the maze and able to touch: out of its rest and more than
    /// a pair of eyes.
    pub fn roaming(&self) -> bool {
        self.rest == 0 && !self.eyes
    }
    /// The way it is going, a tile a step.
    pub fn way(&self) -> (i32, i32) {
        (self.to.0 - self.at.0, self.to.1 - self.at.1)
    }
    /// Turn about, mid-step: back the way it came, from where it is.
    fn reverse(&mut self) {
        if self.at != self.to {
            (self.at, self.to) = (self.to, self.at);
            self.step = self.span.saturating_sub(self.step);
        } else {
            // Standing: free to go any way, back included.
            self.came = self.at;
        }
    }
    /// Stop on the nearer of its two tiles.
    fn settle(&mut self) {
        if self.step * 2 >= self.span.max(1) {
            self.at = self.to;
        }
        (self.to, self.step) = (self.at, 0);
    }
}

/// Wax lying on a maze's tiles, a bit a tile (`y * cols + x`), kept as
/// words: a drop eaten changes one word of what a friend's mirror is sent,
/// where a list of tiles would shift every drop after it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Drops(Vec<u64>);

impl Drops {
    pub fn contains(&self, i: &u16) -> bool {
        self.0.get(usize::from(*i) / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    pub fn insert(&mut self, i: u16) {
        let word = usize::from(i) / 64;
        if self.0.len() <= word {
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1 << (i % 64);
    }
    /// Whether there was a drop to take.
    pub fn remove(&mut self, i: &u16) -> bool {
        let had = self.contains(i);
        if had {
            self.0[usize::from(*i) / 64] &= !(1 << (i % 64));
        }
        had
    }
    pub fn len(&self) -> usize {
        self.0.iter().map(|w| w.count_ones() as usize).sum()
    }
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }
    /// The tiles with wax on them, in order.
    pub fn iter(&self) -> impl Iterator<Item = u16> + '_ {
        self.0
            .iter()
            .enumerate()
            .flat_map(|(n, &w)| (0..64).filter(move |b| w & (1 << b) != 0).map(move |b| (n * 64 + b) as u16))
    }
    /// Words enough for the largest hall.
    fn fits(&self) -> bool {
        self.0.len() * 64 <= layout::MAX_COLS * layout::MAX_ROWS + 63
    }
}

impl FromIterator<u16> for Drops {
    fn from_iter<T: IntoIterator<Item = u16>>(tiles: T) -> Self {
        let mut out = Drops::default();
        for i in tiles {
            out.insert(i);
        }
        out
    }
}

/// One Wax Maze, as it stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaxMaze {
    /// The room it is played in.
    pub room: usize,
    /// Ticks since it began (the title card first).
    pub t: u32,
    /// Wax still lying on each tile, by tile index (`y * cols + x`).
    pub wax: Drops,
    pub holy: BTreeSet<u16>,
    pub wraiths: Vec<Wraith>,
    /// Ticks the wraiths stay pale.
    pub pale: u32,
    /// Each knight's drops eaten, and their ante in gold.
    pub eaten: BTreeMap<u32, u16>,
    pub ante: BTreeMap<u32, u32>,
    /// Knights left alone after a wraith's touch, until this tick.
    pub spooked: BTreeMap<u32, u32>,
    /// Where a knight with no place of their own goes back to (units * 16).
    pub door: (i32, i32),
    pub done: bool,
    /// The wraiths' crypt, the pillar in the middle: its first tile, and
    /// how wide and tall (tiles).
    #[serde(default)]
    pub crypt: (i32, i32, i32, i32),
    /// Where each knight came in, and goes back to after a touch (units *
    /// 16).
    #[serde(default)]
    pub starts: BTreeMap<u32, (i32, i32)>,
    /// Each knight's way along the lanes: the way they last went, held on
    /// while a turn they pressed early waits for its opening.
    #[serde(default)]
    pub ways: BTreeMap<u32, (i8, i8)>,
    /// Pale wraiths caught on the holy water now running: the next is worth
    /// `CATCH[chain]`.
    #[serde(default)]
    pub chain: u8,
}

/// The lines a maze's lanes run along, across a room `n` tiles wide (or
/// tall): the walls' own ring at 1 and `n - 2`, and between them gaps of
/// three (pillars two tiles thick) with as few of four as make the room,
/// laid the same from both walls about a middle gap whose pillar is the
/// crypt's.
pub fn lanes(n: i32) -> Vec<i32> {
    let span = n - 3;
    // Threes, and the fewest fours that make `side`, a four first.
    let gaps = |side: i32| {
        (0..=side.max(0) / 4).find(|f| (side - 4 * f) % 3 == 0).map(|fours| {
            let (mut f, mut t) = (fours, (side - 4 * fours) / 3);
            let mut out = Vec::new();
            while f + t > 0 {
                if f > 0 && (t == 0 || out.last() != Some(&4)) {
                    out.push(4);
                    f -= 1;
                } else {
                    out.push(3);
                    t -= 1;
                }
            }
            out
        })
    };
    for mid in [3, 4, 5, 2] {
        if span < mid || (span - mid) % 2 != 0 {
            continue;
        }
        if let Some(side) = gaps((span - mid) / 2) {
            let mut out = vec![1];
            for g in side.iter().chain([mid].iter()).chain(side.iter().rev()) {
                out.push(out.last().unwrap() + g);
            }
            return out;
        }
    }
    (1..n - 1).collect()
}

/// The maze for a room `cols` by `rows`: which tiles are lanes (`y * cols
/// + x`), and the crypt (`WaxMaze::crypt`). A lattice of lanes (`lanes`),
/// then some of its inner lanes closed by the seed, west and east alike,
/// each only while every crossing keeps two ways out and the whole maze
/// stays one; the ring inside the walls and the lanes round the crypt
/// always stay open.
pub fn lay_maze(cols: i32, rows: i32, seed: u64) -> (Vec<bool>, (i32, i32, i32, i32)) {
    let (xs, ys) = (lanes(cols), lanes(rows));
    let (nx, ny) = (xs.len(), ys.len());
    let (ci, cj) = ((nx - 2) / 2, (ny - 2) / 2);
    let crypt = (xs[ci] + 1, ys[cj] + 1, xs[ci + 1] - xs[ci] - 1, ys[cj + 1] - ys[cj] - 1);
    // Lanes between crossings: `across[i][j]` from (xs[i], ys[j]) east,
    // `down[i][j]` from (xs[i], ys[j]) south.
    let mut across = vec![vec![true; ny]; nx - 1];
    let mut down = vec![vec![true; ny - 1]; nx];
    let kept_across = |i: usize, j: usize| j == 0 || j == ny - 1 || (i == ci && (j == cj || j == cj + 1));
    let kept_down = |i: usize, j: usize| i == 0 || i == nx - 1 || (j == cj && (i == ci || i == ci + 1));
    let mut offered: Vec<(bool, usize, usize)> = Vec::new();
    for i in 0..nx - 1 {
        for j in 0..ny {
            if !kept_across(i, j) && i <= nx - 2 - i {
                offered.push((true, i, j));
            }
        }
    }
    for i in 0..nx {
        for j in 0..ny - 1 {
            if !kept_down(i, j) && i <= nx - 1 - i {
                offered.push((false, i, j));
            }
        }
    }
    let mut rng = Rng::new(seed);
    for k in (1..offered.len()).rev() {
        offered.swap(k, rng.below(k + 1));
    }
    let sound = |across: &[Vec<bool>], down: &[Vec<bool>]| {
        let ways = |i: usize, j: usize| {
            let mut out = Vec::with_capacity(4);
            if i > 0 && across[i - 1][j] {
                out.push((i - 1, j));
            }
            if i + 1 < nx && across[i][j] {
                out.push((i + 1, j));
            }
            if j > 0 && down[i][j - 1] {
                out.push((i, j - 1));
            }
            if j + 1 < ny && down[i][j] {
                out.push((i, j + 1));
            }
            out
        };
        if (0..nx).any(|i| (0..ny).any(|j| ways(i, j).len() < 2)) {
            return false;
        }
        let mut seen = vec![vec![false; ny]; nx];
        let mut queue = VecDeque::from([(0, 0)]);
        seen[0][0] = true;
        let mut count = 1;
        while let Some((i, j)) = queue.pop_front() {
            for (a, b) in ways(i, j) {
                if !seen[a][b] {
                    seen[a][b] = true;
                    count += 1;
                    queue.push_back((a, b));
                }
            }
        }
        count == nx * ny
    };
    for (horizontal, i, j) in offered {
        if !rng.chance(CLOSE) {
            continue;
        }
        let twin = if horizontal { nx - 2 - i } else { nx - 1 - i };
        let lane = |across: &mut Vec<Vec<bool>>, down: &mut Vec<Vec<bool>>, open: bool| {
            for at in [i, twin] {
                if horizontal {
                    across[at][j] = open;
                } else {
                    down[at][j] = open;
                }
            }
        };
        lane(&mut across, &mut down, false);
        if !sound(&across, &down) {
            lane(&mut across, &mut down, true);
        }
    }
    let mut open = vec![false; (cols * rows) as usize];
    let mut dig = |x: i32, y: i32| open[(y * cols + x) as usize] = true;
    for i in 0..nx - 1 {
        for j in 0..ny {
            if across[i][j] {
                (xs[i]..=xs[i + 1]).for_each(|x| dig(x, ys[j]));
            }
        }
    }
    for i in 0..nx {
        for j in 0..ny - 1 {
            if down[i][j] {
                (ys[j]..=ys[j + 1]).for_each(|y| dig(xs[i], y));
            }
        }
    }
    (open, crypt)
}

/// Whether the wraiths keep to their corners at tick `t` of a maze (`MOODS`).
fn scatter_at(t: u32) -> bool {
    let mut t = t.saturating_sub(TITLE);
    for (i, &span) in MOODS.iter().enumerate() {
        if t < span {
            return i % 2 == 0;
        }
        t -= span;
    }
    false
}

/// A lane of the maze: open floor, the doors held shut.
fn lane(room: &Room, (x, y): (i32, i32)) -> bool {
    room.tile(x, y) == Tile::Floor
}

/// The tile under a point (units).
fn tile_of(x: f32, y: f32) -> (i32, i32) {
    ((x / TILE_UNITS).floor() as i32, (y / TILE_UNITS).floor() as i32)
}

/// A tile's middle (units).
fn mid(i: i32) -> f32 {
    (i as f32 + 0.5) * TILE_UNITS
}

impl WaxMaze {
    pub fn valid(&self) -> bool {
        self.t <= TITLE + MAZE_TIME + HZ
            && self.wax.fits()
            && self.holy.len() <= 8
            && self.wraiths.len() <= 4
            && self.pale <= PALE
            && usize::from(self.chain) <= CATCH.len()
            && self.eaten.len() <= MAX_PLAYERS as usize
            && self.ante.len() <= MAX_PLAYERS as usize
            && self.spooked.len() <= MAX_PLAYERS as usize
            && self.starts.len() <= MAX_PLAYERS as usize
            && self.ways.len() <= MAX_PLAYERS as usize
            && self.eaten.keys().chain(self.ante.keys()).all(|id| (1..=MAX_PLAYERS).contains(id))
            && self.ways.values().all(|&(x, y)| x.abs() + y.abs() == 1)
            && self.wraiths.iter().all(|w| {
                w.rest <= REST.max(RELEASE[3]) && w.span <= PALE_STEP.max(LEAVE_STEP) && w.step <= PALE_STEP.max(LEAVE_STEP)
            })
    }
    /// Seconds left on the candle.
    pub fn left(&self) -> u32 {
        (TITLE + MAZE_TIME).saturating_sub(self.t).min(MAZE_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < TITLE
    }
    /// Being played: past its title card and not yet done.
    pub fn running(&self) -> bool {
        !self.titled() && !self.done
    }
    /// Whether wraith `w` is pale now.
    pub fn pale_now(&self, w: &Wraith) -> bool {
        w.pale && self.pale > 0
    }
    /// The pale wraiths flash: the holy water is wearing off.
    pub fn warning(&self) -> bool {
        self.pale > 0 && self.pale <= PALE_WARN
    }
    /// The wraiths keep to their corners now, rather than hunt.
    pub fn scatter(&self) -> bool {
        scatter_at(self.t)
    }
    /// Whether a tile is the crypt's.
    pub fn in_crypt(&self, (x, y): (i32, i32)) -> bool {
        let (cx, cy, cw, ch) = self.crypt;
        (cx..cx + cw).contains(&x) && (cy..cy + ch).contains(&y)
    }
    /// The lane tile a wraith climbs out of the crypt onto, from its place
    /// in it: straight up from it, above the crypt.
    pub fn gate(&self, home: (i32, i32)) -> (i32, i32) {
        (home.0, self.crypt.1 - 1)
    }

    /// Knight `id`'s going along the lanes this tick, pressing `(mx, my)`
    /// (any length; only its signs count) for `dist` units; `None` when the
    /// knight stands on no lane (the room's own sliding takes them then).
    /// A knight keeps to the middle of a lane. A way pressed is taken from
    /// the crossing the knight is within `TURN_WINDOW` of, cutting the
    /// corner as Pac-Man does; pressed too early, the knight goes on the way
    /// they were going until the opening comes. A wall ahead stops them in
    /// the middle of the last tile. During the title card nobody moves.
    pub(super) fn glide(&mut self, id: u32, room: &Room, (x, y): (f32, f32), (mx, my): (f32, f32), dist: f32) -> Option<(f32, f32)> {
        let (tx, ty) = tile_of(x, y);
        if self.done || !lane(room, (tx, ty)) {
            return None;
        }
        if self.titled() {
            return Some((x, y));
        }
        let sign = |v: f32| if v > 0.01 { 1i8 } else if v < -0.01 { -1 } else { 0 };
        let (sx, sy) = (sign(mx), sign(my));
        if (sx, sy) == (0, 0) || dist <= 0.0 {
            return Some((x, y));
        }
        let way = self.ways.get(&id).copied().unwrap_or((0, 0));
        let mut tries: Vec<(i8, i8)> = Vec::with_capacity(3);
        if sx != 0 && sy != 0 {
            // Both pressed: the turn off the way first, then on along it.
            if way.0 != 0 {
                tries.extend([(0, sy), (sx, 0)]);
            } else {
                tries.extend([(sx, 0), (0, sy)]);
            }
        } else {
            tries.push((sx, sy));
        }
        // A turn waiting for its opening: on the way they were going.
        if way != (0, 0) && !tries.contains(&way) && !tries.contains(&(-way.0, -way.1)) {
            tries.push(way);
        }
        let open = |c: i32, r: i32| lane(room, (c, r));
        let cols = room.cols as i32;
        let waxed = |c: i32, r: i32| c >= 0 && r >= 0 && self.wax.contains(&((r * cols + c) as u16));
        let go = |(dx, dy): (i8, i8)| -> Option<(f32, f32)> {
            let (dx, dy) = (i32::from(dx), i32::from(dy));
            let dist = if waxed(tx, ty) || waxed(tx + dx, ty + dy) { dist * EAT_PACE } else { dist };
            // Along x or along y: `a` the coordinate along, `b` across.
            let (a, b, ta, tb) = if dx != 0 { (x, y, tx, ty) } else { (y, x, ty, tx) };
            let d = dx + dy;
            let ahead = |n: i32| if dx != 0 { open(tx + n * dx, ty) } else { open(tx, ty + n * dy) };
            let off = b - mid(tb);
            if off.abs() > TURN_WINDOW || !ahead(1) && (mid(ta) - a) * d as f32 <= 0.0 {
                return None;
            }
            let mut na = a + d as f32 * dist;
            let stop = if !ahead(1) {
                Some(mid(ta))
            } else if !ahead(2) {
                Some(mid(ta + d))
            } else {
                None
            };
            if let Some(stop) = stop
                && (na - stop) * d as f32 > 0.0
            {
                na = stop;
            }
            let nb = mid(tb) + off.signum() * (off.abs() - dist).max(0.0);
            ((na - a).abs() > 1e-4 || (nb - b).abs() > 1e-4).then(|| if dx != 0 { (na, nb) } else { (nb, na) })
        };
        for d in tries {
            if let Some(p) = go(d) {
                self.ways.insert(id, d);
                return Some(p);
            }
        }
        Some((x, y))
    }
}

impl Run {
    /// One hall on each floor of the delve is the floor's folly: the seed's
    /// pick, never the start, a boss's camp or the way down.
    pub(super) fn pick_folly(&mut self) {
        self.folly = None;
        self.bats = None;
        self.powder = None;
        self.hop = None;
        self.folly_room = None;
        if self.dungeon.depth == 0 {
            return;
        }
        let camps = |i: usize| {
            self.boss_gates
                .as_ref()
                .is_some_and(|g| g.economy.camp(i).is_some() || g.guardian == i)
        };
        let fights: Vec<usize> = (0..self.dungeon.rooms.len())
            .filter(|&i| {
                let r = &self.dungeon.rooms[i];
                r.kind == RoomKind::Hall && !r.great() && !r.cleared && i != self.at && !camps(i)
            })
            .collect();
        if fights.is_empty() {
            return;
        }
        let pick = (mix(self.seed ^ self.raid_id.rotate_left(9) ^ u64::from(self.dungeon.depth)) % fights.len() as u64) as usize;
        self.folly_room = Some(fights[pick]);
    }

    /// The party walks into the folly through door `from`: the hall grows
    /// for the party (`party_room`), pillars rise into a maze, wax is
    /// strewn, the wraiths wait in their crypt, and each knight antes.
    pub(super) fn start_wax_maze(&mut self, index: usize, from: Option<usize>) {
        let playing = self.players.values().filter(|h| h.hp > 0 && !h.stone && !h.absent).count();
        let (want_cols, want_rows) = layout::party_room(playing);
        let room = &mut self.dungeon.rooms[index];
        let (was_w, was_h) = (room.width(), room.height());
        room.regrow(want_cols.max(room.cols), want_rows.max(room.rows));
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        // The party keeps its place by the door it came in at.
        let (grow_w, grow_h) = (room.width() - was_w, room.height() - was_h);
        let shift = match from {
            Some(0) => (grow_w / 2.0, 0.0),
            Some(1) => (grow_w, grow_h / 2.0),
            Some(2) | None => (grow_w / 2.0, grow_h),
            Some(_) => (0.0, grow_h / 2.0),
        };
        for hero in self.players.values_mut() {
            hero.x += shift.0;
            hero.y += shift.1;
        }
        let seed = mix(self.seed ^ (index as u64) << 20 ^ u64::from(self.dungeon.depth));
        let (open, crypt) = lay_maze(cols, rows, seed);
        let mut wax = Drops::default();
        for y in 1..rows - 1 {
            for x in 1..cols - 1 {
                if open[(y * cols + x) as usize] {
                    room.set(x as usize, y as usize, Tile::Floor);
                    wax.insert((y * cols + x) as u16);
                } else {
                    room.set(x as usize, y as usize, Tile::Block);
                }
            }
        }
        // What lay on the floor is set on the nearest lane.
        let snap = |x: &mut f32, y: &mut f32| {
            let (tx, ty) = tile_of(*x, *y);
            let near = (0..rows)
                .flat_map(|r| (0..cols).map(move |c| (c, r)))
                .filter(|&(c, r)| open[(r * cols + c) as usize])
                .min_by_key(|&(c, r)| (c - tx).pow(2) + (r - ty).pow(2));
            if let Some((c, r)) = near.filter(|&n| n != (tx, ty)) {
                (*x, *y) = (mid(c), mid(r));
            }
        };
        for item in &mut room.items {
            snap(&mut item.x, &mut item.y);
        }
        if let Some(chest) = &mut room.chest {
            snap(&mut chest.x, &mut chest.y);
        }
        for hero in self.players.values_mut() {
            snap(&mut hero.x, &mut hero.y);
        }
        let corners = [(1, 1), (cols - 2, 1), (1, rows - 2), (cols - 2, rows - 2)];
        let holy: BTreeSet<u16> = corners.iter().map(|&(x, y)| (y * cols + x) as u16).collect();
        let (cx, cy, cw, ch) = crypt;
        let homes = [(cx, cy), (cx + cw - 1, cy), (cx, cy + ch - 1), (cx + cw - 1, cy + ch - 1)];
        // No wax on the holy water, nor on the crypt's doorstep.
        for (gx, _) in homes {
            wax.remove(&(((cy - 1) * cols + gx) as u16));
        }
        for h in &holy {
            wax.remove(h);
        }
        let wraiths = Temper::ALL
            .iter()
            .zip(homes)
            .zip(RELEASE)
            .map(|((&temper, home), rest)| Wraith {
                temper,
                at: home,
                to: home,
                step: 0,
                rest,
                span: LEAVE_STEP,
                pale: false,
                eyes: false,
                home,
                came: home,
            })
            .collect();
        let (ante, scores) = take_antes(&mut self.players);
        let eaten = scores.keys().map(|&id| (id, 0u16)).collect();
        let at16 = |h: &Hero| ((h.x * 16.0) as i32, (h.y * 16.0) as i32);
        let starts: BTreeMap<u32, (i32, i32)> = self
            .players
            .iter()
            .filter(|(_, h)| !h.absent)
            .map(|(&id, h)| (id, at16(h)))
            .collect();
        let door = starts.values().next().copied().unwrap_or(((mid(cols / 2) * 16.0) as i32, (mid(rows - 2) * 16.0) as i32));
        // The room's stones changed: friends' mirrors take the whole floor.
        self.dungeon.world_revision = self.dungeon.world_revision.wrapping_add(1);
        self.folly = Some(WaxMaze {
            room: index,
            t: 0,
            wax,
            holy,
            wraiths,
            pale: 0,
            eaten,
            ante,
            spooked: BTreeMap::new(),
            door,
            done: false,
            crypt,
            starts,
            ways: BTreeMap::new(),
            chain: 0,
        });
        // Those who follow the party find their feet in the maze.
        self.hireling_follows();
        self.tallow_follows();
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// The maze's tick: the knights eat, the wraiths roam, and the end. It
    /// waits, as it stands, while the party is elsewhere.
    pub(super) fn tick_folly(&mut self) {
        let Some(mut maze) = self.folly.take() else {
            return;
        };
        if maze.room != self.at || maze.done {
            self.folly = Some(maze);
            return;
        }
        if maze.crypt.2 == 0 {
            // A maze from before the crypt: called as it stands.
            self.finish_wax_maze(&mut maze);
            self.folly = Some(maze);
            return;
        }
        // While the room holds (a friend's screen still coming in, or
        // nobody here at all) the candle and the wraiths wait, as foes do.
        if self.holding() {
            self.folly = Some(maze);
            return;
        }
        maze.t += 1;
        if maze.titled() {
            self.folly = Some(maze);
            return;
        }
        let (cols, rows) = (self.room().cols as i32, self.room().rows as i32);
        let index = |(x, y): (i32, i32)| (x >= 0 && y >= 0 && x < cols && y < rows).then(|| (y * cols + x) as u16);
        // Eating, and the holy water.
        let mut chomps = 0;
        let mut holy = false;
        for (&id, hero) in self.players.iter().filter(|(_, h)| h.hp > 0 && !h.stone) {
            let Some(at) = index(tile_of(hero.x, hero.y)) else {
                continue;
            };
            if maze.wax.remove(&at) {
                let eaten = maze.eaten.entry(id).or_default();
                *eaten = eaten.saturating_add(1);
                chomps += 1;
            }
            if maze.holy.remove(&at) {
                holy = true;
            }
        }
        if chomps > 0 && self.tick % 4 == 0 {
            self.sounds.push("spoil_pickup");
        }
        if holy {
            // Every wraith but a pair of eyes turns pale, and those abroad
            // turn tail.
            maze.pale = PALE;
            maze.chain = 0;
            for w in maze.wraiths.iter_mut().filter(|w| !w.eyes) {
                if !w.pale && w.roaming() && !maze_crypt(&maze.crypt, w.at) {
                    w.reverse();
                }
                w.pale = true;
            }
            self.sounds.push("holy_water");
        } else if maze.pale > 0 {
            maze.pale -= 1;
            if maze.pale == 0 {
                maze.chain = 0;
                for w in &mut maze.wraiths {
                    w.pale = false;
                }
            }
        }
        // Who the wraiths hunt: knights standing, here, and not just
        // touched.
        let hunted: Vec<(u32, (i32, i32), (i32, i32))> = self
            .players
            .iter()
            .filter(|&(&id, h)| {
                h.hp > 0 && !h.stone && !h.absent && !self.away(id) && maze.spooked.get(&id).is_none_or(|&until| maze.t >= until)
            })
            .map(|(&id, h)| {
                let way = maze.ways.get(&id).map_or((0, 0), |&(x, y)| (i32::from(x), i32::from(y)));
                (id, tile_of(h.x, h.y), way)
            })
            .collect();
        // A pale wraith flees every knight playing, hunted or not.
        let threats: Vec<(i32, i32)> = self
            .players
            .values()
            .filter(|h| h.hp > 0 && !h.stone && !h.absent)
            .map(|h| tile_of(h.x, h.y))
            .collect();
        self.roam(&mut maze, &hunted, &threats);
        // Touches.
        let at16 = |p: (i32, i32)| (p.0 as f32 / 16.0, p.1 as f32 / 16.0);
        let hunted: BTreeSet<u32> = hunted.iter().map(|&(id, _, _)| id).collect();
        for k in 0..maze.wraiths.len() {
            let w = &maze.wraiths[k];
            if !w.roaming() {
                continue;
            }
            let pale = maze.pale_now(w);
            let (wx, wy) = w.pos();
            for (&id, hero) in self.players.iter_mut().filter(|(_, h)| h.hp > 0 && !h.stone && !h.absent) {
                if (hero.x / TILE_UNITS - wx).hypot(hero.y / TILE_UNITS - wy) > TOUCH {
                    continue;
                }
                if pale {
                    let worth = CATCH[usize::from(maze.chain).min(CATCH.len() - 1)];
                    maze.chain = maze.chain.saturating_add(1).min(CATCH.len() as u8);
                    let eaten = maze.eaten.entry(id).or_default();
                    *eaten = eaten.saturating_add(worth);
                    hero.carried.add(Spoil::Bone, 1);
                    let w = &mut maze.wraiths[k];
                    w.settle();
                    (w.eyes, w.pale) = (true, false);
                    self.sounds.push("hit_magic");
                    break;
                }
                if !hunted.contains(&id) {
                    continue;
                }
                let eaten = maze.eaten.entry(id).or_default();
                *eaten = eaten.saturating_sub(SPILL);
                maze.spooked.insert(id, maze.t + SPOOKED);
                maze.ways.remove(&id);
                (hero.x, hero.y) = at16(maze.starts.get(&id).copied().unwrap_or(maze.door));
                hero.invulnerable = hero.invulnerable.max(SPOOKED);
                hero.dash_ticks = 0;
                self.sounds.push("hero_hurt");
            }
        }
        if maze.wax.is_empty() || maze.t >= TITLE + MAZE_TIME {
            self.finish_wax_maze(&mut maze);
        }
        self.folly = Some(maze);
    }

    /// The wraiths' going this tick, each a tile at a time: resting, out
    /// of the crypt, hunting or in their corners by the candle's mood,
    /// fleeing while pale, and caught ones' eyes home.
    fn roam(&self, maze: &mut WaxMaze, hunted: &[(u32, (i32, i32), (i32, i32))], threats: &[(i32, i32)]) {
        let room = self.room();
        let (cols, rows) = (room.cols as i32, room.rows as i32);
        let scatter = maze.scatter();
        // A change of mood turns the calm ones about, as in the arcade.
        let turned = scatter != scatter_at(maze.t.saturating_sub(1));
        let late = maze.t >= TITLE + MAZE_TIME - LATE;
        let near = |p: (i32, i32)| hunted.iter().min_by_key(|(_, k, _)| (k.0 - p.0).pow(2) + (k.1 - p.1).pow(2)).copied();
        let hound = maze.wraiths.iter().find(|w| w.temper == Temper::Hound && w.roaming()).map(|w| w.at);
        let crypt = maze.crypt;
        for k in 0..maze.wraiths.len() {
            let mut w = maze.wraiths[k].clone();
            if w.rest > 0 {
                w.rest -= 1;
                maze.wraiths[k] = w;
                continue;
            }
            let pale = maze.pale_now(&w);
            if turned && w.roaming() && !pale && !maze_crypt(&crypt, w.at) {
                w.reverse();
            }
            w.step += 1;
            if w.at != w.to && w.step < w.span.max(1) {
                maze.wraiths[k] = w;
                continue;
            }
            if w.at != w.to {
                w.came = w.at;
                w.at = w.to;
            }
            w.step = 0;
            let behind = (w.came != w.at).then_some(w.came);
            let gate = maze.gate(w.home);
            // Another wraith abroad on a tile, or going into it, keeps this
            // one out of it: it waits its turn rather than walk into it.
            let others: Vec<&Wraith> = maze.wraiths.iter().enumerate().filter(|&(j, o)| j != k && o.roaming()).map(|(_, o)| o).collect();
            let blocked = |p: (i32, i32)| others.iter().any(|o| o.at == p || o.to == p);
            if w.eyes {
                // Home: down into the crypt to its place, and rest.
                if w.at == w.home {
                    (w.eyes, w.pale, w.rest, w.span) = (false, false, REST, LEAVE_STEP);
                    maze.wraiths[k] = w;
                    continue;
                }
                w.to = if maze_crypt(&crypt, w.at) || w.at == gate {
                    (w.at.0, w.at.1 + 1)
                } else {
                    toward(room, w.at, gate).unwrap_or(gate)
                };
                w.span = if maze_crypt(&crypt, w.to) { LEAVE_STEP } else { EYES_STEP };
                maze.wraiths[k] = w;
                continue;
            }
            if maze_crypt(&crypt, w.at) {
                // Climbing out: up to the lane above the crypt, once it is
                // clear.
                let up = (w.at.0, w.at.1 - 1);
                w.to = if blocked(up) { w.at } else { up };
                w.span = LEAVE_STEP;
                maze.wraiths[k] = w;
                continue;
            }
            // At a crossing (or along a lane): the next tile. Never back
            // the way it came unless there is no other way; never into a
            // tile another wraith holds. Met head on, or by one standing,
            // it turns about; behind one going on, it waits.
            let options: Vec<(i32, i32)> = [(0, -1), (-1, 0), (0, 1), (1, 0)]
                .into_iter()
                .map(|(dx, dy)| (w.at.0 + dx, w.at.1 + dy))
                .filter(|&p| lane(room, p) && !maze_crypt(&crypt, p))
                .collect();
            let fresh: Vec<(i32, i32)> = options.iter().copied().filter(|&p| Some(p) != behind).collect();
            let free: Vec<(i32, i32)> = fresh.iter().copied().filter(|&p| !blocked(p)).collect();
            // Only one going on out of the way is worth waiting for; one
            // standing in it, or coming into it, is not.
            let going_on = |o: &&Wraith, p: (i32, i32)| o.at == p && o.to != p && o.to != w.at;
            let head_on = fresh
                .iter()
                .any(|&p| others.iter().any(|o| (o.at == p || o.to == p) && !going_on(o, p)));
            let back: Vec<(i32, i32)> = behind.into_iter().filter(|&p| options.contains(&p) && !blocked(p)).collect();
            let choices = if !free.is_empty() {
                free
            } else if fresh.is_empty() || head_on {
                back
            } else {
                Vec::new()
            };
            let dist2 = |a: (i32, i32), b: (i32, i32)| (a.0 - b.0).pow(2) + (a.1 - b.1).pow(2);
            let pick = if pale {
                // Fleeing: the way that leaves the nearest knight farthest.
                let threat = threats.iter().min_by_key(|&&t| dist2(t, w.at)).copied();
                choices.iter().copied().max_by_key(|&p| threat.map_or(0, |t| dist2(p, t)))
            } else {
                let quarry = near(w.at).map(|(_, p, way)| (p, way));
                let flanked = hound.and_then(|h| near(h).map(|(_, p, way)| (h, (p, way))));
                let target = aim(w.temper, w.at, (cols, rows), if scatter { None } else { quarry }, flanked);
                // Ties go up, left, down, right, as in the arcade.
                choices.iter().copied().min_by_key(|&p| dist2(p, target))
            };
            w.to = pick.unwrap_or(w.at);
            w.span = if pale {
                PALE_STEP
            } else if late {
                LATE_STEP
            } else {
                WRAITH_STEP
            };
            maze.wraiths[k] = w;
        }
    }

    /// The podium: everyone keeps a gold a five drops, the most wax takes
    /// the pot and the crown, and the room is done.
    fn finish_wax_maze(&mut self, maze: &mut WaxMaze) {
        maze.done = true;
        maze.pale = 0;
        let pot: u32 = maze.ante.values().sum();
        let best = maze
            .eaten
            .iter()
            .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
            .map(|(&id, _)| id);
        let rivals = maze.eaten.len() > 1;
        for (&id, &n) in &maze.eaten {
            if let Some(hero) = self.players.get_mut(&id) {
                hero.carried.add(Spoil::Gold, u32::from(n / WAX_PER_GOLD));
                if rivals {
                    hero.laurel = Some(id) == best;
                }
                if Some(id) == best {
                    hero.carried.add(Spoil::Gold, pot);
                }
            }
        }
        if let Some(room) = self.dungeon.rooms.get_mut(maze.room) {
            room.cleared = true;
        }
        let name = best
            .and_then(|id| self.players.get(&id))
            .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
        let most = best.and_then(|id| maze.eaten.get(&id)).copied().unwrap_or(0);
        self.found = Some((self.tick, 0, format!("{name} ATE THE MOST WAX: {most}!")));
        self.sounds.push("play_card");
        self.cues.push("pastime_won".into());
    }
}

/// The tile a calm wraith of `temper` standing on `at` heads for, in a
/// maze `cols` by `rows`, hunting `quarry` (the nearest knight's tile and
/// the way they go; `None` while the wraiths keep to their corners, or with
/// nobody to hunt), with the Hound on `flanked.0` hunting `flanked.1`:
///
/// - the Hound, the knight's own tile;
/// - the Cutter, `CUT_AHEAD` tiles ahead of the knight;
/// - the Flanker, the Hound's line to `FLANK_AHEAD` tiles ahead of the
///   Hound's knight, doubled: from the far side of the knight;
/// - the Drifter, the knight from afar and its own corner within `SHY`.
///
/// Their corners lie past the maze's own: the Hound's top right, the
/// Cutter's top left, the Flanker's foot right, the Drifter's foot left.
pub fn aim(
    temper: Temper,
    at: (i32, i32),
    (cols, rows): (i32, i32),
    quarry: Option<((i32, i32), (i32, i32))>,
    flanked: Option<((i32, i32), ((i32, i32), (i32, i32)))>,
) -> (i32, i32) {
    let corner = match temper {
        Temper::Hound => (cols + 1, -2),
        Temper::Cutter => (-2, -2),
        Temper::Flanker => (cols + 1, rows + 1),
        Temper::Drifter => (-2, rows + 1),
    };
    let Some((p, way)) = quarry else {
        return corner;
    };
    match temper {
        Temper::Hound => p,
        Temper::Cutter => (p.0 + way.0 * CUT_AHEAD, p.1 + way.1 * CUT_AHEAD),
        Temper::Flanker => {
            let (from, (q, qw)) = flanked.unwrap_or((at, (p, way)));
            let ahead = (q.0 + qw.0 * FLANK_AHEAD, q.1 + qw.1 * FLANK_AHEAD);
            (2 * ahead.0 - from.0, 2 * ahead.1 - from.1)
        }
        Temper::Drifter => {
            if (p.0 - at.0).pow(2) + (p.1 - at.1).pow(2) > SHY * SHY {
                p
            } else {
                corner
            }
        }
    }
}

/// Whether tile `p` is in crypt `(x, y, w, h)`.
fn maze_crypt(&(cx, cy, cw, ch): &(i32, i32, i32, i32), (x, y): (i32, i32)) -> bool {
    (cx..cx + cw).contains(&x) && (cy..cy + ch).contains(&y)
}

/// The first tile on the shortest way along the lanes from `from` to `to`.
fn toward(room: &Room, from: (i32, i32), to: (i32, i32)) -> Option<(i32, i32)> {
    let mut came: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::from([(to, to)]);
    let mut queue = VecDeque::from([to]);
    // Searched from the goal, so the step is read straight off.
    while let Some(at) = queue.pop_front() {
        for (dx, dy) in [(0, -1), (-1, 0), (0, 1), (1, 0)] {
            let next = (at.0 + dx, at.1 + dy);
            if next == from {
                return Some(at);
            }
            if lane(room, next) && !came.contains_key(&next) {
                came.insert(next, at);
                queue.push_back(next);
            }
        }
    }
    None
}

/// The Belfry Shoot, a nod to Duck Hunt: on the even floors the folly hall
/// fills with bats, a flock every two seconds, for a minute. Everyone shoots
/// with their own arms; whoever deals the bats the most harm takes the pot
/// and the crown. The bats bite back, as bats do.
pub const BAT_TIME: u32 = 60 * HZ;
pub const BAT_EVERY: u32 = 2 * HZ;
pub const BAT_FLOCK: u32 = 3;
pub const BAT_ALIVE: usize = 12;
/// Harm dealt that makes a gold to keep.
pub const HARM_PER_GOLD: u32 = 100;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatShoot {
    pub room: usize,
    pub t: u32,
    /// Harm each knight has dealt the bats, and their ante.
    pub scores: BTreeMap<u32, u32>,
    pub ante: BTreeMap<u32, u32>,
    pub done: bool,
}

impl BatShoot {
    pub fn valid(&self) -> bool {
        self.t <= TITLE + BAT_TIME + HZ
            && self.scores.len() <= MAX_PLAYERS as usize
            && self.ante.len() <= MAX_PLAYERS as usize
            && self.scores.keys().chain(self.ante.keys()).all(|id| (1..=MAX_PLAYERS).contains(id))
    }
    pub fn left(&self) -> u32 {
        (TITLE + BAT_TIME).saturating_sub(self.t).min(BAT_TIME).div_ceil(HZ)
    }
    pub fn titled(&self) -> bool {
        self.t < TITLE
    }
}

/// Each knight antes a tenth of the gold they carry; the ante and a fresh
/// score line for everyone standing.
pub(super) fn take_antes(players: &mut BTreeMap<u32, Hero>) -> (BTreeMap<u32, u32>, BTreeMap<u32, u32>) {
    let mut ante = BTreeMap::new();
    let mut scores = BTreeMap::new();
    for (&id, hero) in players.iter_mut().filter(|(_, h)| h.hp > 0 && !h.stone) {
        let gold = hero.carried.get(Spoil::Gold) * ANTE_TENTHS / 10;
        if gold > 0 {
            let mut price = Spoils::default();
            price.add(Spoil::Gold, gold);
            hero.carried.take(&price);
        }
        ante.insert(id, gold);
        scores.insert(id, 0);
    }
    (ante, scores)
}

impl Run {
    /// A folly is being played in the room the party stands in: it is not
    /// cleared, and its doors hold, until the podium.
    pub fn folly_running(&self) -> bool {
        self.folly.as_ref().is_some_and(|f| f.room == self.at && !f.done)
            || self.bats.as_ref().is_some_and(|b| b.room == self.at && !b.done)
            || self.gantry.as_ref().is_some_and(|g| g.room == self.at && !g.done)
            || self.powder.as_ref().is_some_and(|p| p.room == self.at && !p.done)
            || self.hop.as_ref().is_some_and(|h| h.room == self.at && !h.done)
            || self.race3d.as_ref().is_some_and(|r| r.room == self.at && !r.done)
    }

    /// Into the even floors' folly: the belfry's bats wake.
    pub(super) fn start_bat_shoot(&mut self, index: usize) {
        let (ante, scores) = take_antes(&mut self.players);
        self.bats = Some(BatShoot {
            room: index,
            t: 0,
            scores,
            ante,
            done: false,
        });
        self.sounds.push("door_seal");
        self.cues.push("folly".into());
    }

    /// Harm dealt this tick, credited to whoever dealt it.
    pub(super) fn credit_bats(&mut self, credits: &[(u32, u32)]) {
        let Some(shoot) = self.bats.as_mut().filter(|b| b.room == self.at && !b.done && !b.titled()) else {
            return;
        };
        for &(id, harm) in credits {
            if let Some(score) = shoot.scores.get_mut(&id) {
                *score = score.saturating_add(harm);
            }
        }
    }

    /// The shoot's tick: a flock every two seconds, then the podium.
    pub(super) fn tick_bats(&mut self) {
        let Some(mut shoot) = self.bats.take() else {
            return;
        };
        if shoot.room != self.at || shoot.done {
            self.bats = Some(shoot);
            return;
        }
        shoot.t += 1;
        if !shoot.titled() && (shoot.t - TITLE).is_multiple_of(BAT_EVERY) {
            // Down from the rafters: open floor in the room's upper half,
            // spread by the seed.
            let room = self.room();
            let open: Vec<(i32, i32)> = (1..room.rows as i32 / 2)
                .flat_map(|y| (1..room.cols as i32 - 1).map(move |x| (x, y)))
                .filter(|&(x, y)| room.tile(x, y) == Tile::Floor)
                .collect();
            for k in 0..BAT_FLOCK {
                if self.enemies.len() >= BAT_ALIVE || open.is_empty() {
                    break;
                }
                let spot = mix(self.seed ^ u64::from(shoot.t) << 4 ^ u64::from(k));
                let (x, y) = open[(spot % open.len() as u64) as usize];
                self.spawn_body(EnemyKind::Bat, (x as f32 + 0.5) * TILE_UNITS, (y as f32 + 0.5) * TILE_UNITS);
            }
        }
        if shoot.t >= TITLE + BAT_TIME {
            shoot.done = true;
            self.enemies.retain(|e| e.kind != EnemyKind::Bat);
            let pot: u32 = shoot.ante.values().sum();
            let best = shoot
                .scores
                .iter()
                .max_by_key(|(id, n)| (**n, std::cmp::Reverse(**id)))
                .map(|(&id, _)| id);
            let rivals = shoot.scores.len() > 1;
            for (&id, &n) in &shoot.scores {
                if let Some(hero) = self.players.get_mut(&id) {
                    hero.carried.add(Spoil::Gold, n / HARM_PER_GOLD);
                    if rivals {
                        hero.laurel = Some(id) == best;
                    }
                    if Some(id) == best {
                        hero.carried.add(Spoil::Gold, pot);
                    }
                }
            }
            if let Some(room) = self.dungeon.rooms.get_mut(shoot.room) {
                room.cleared = true;
            }
            let name = best
                .and_then(|id| self.players.get(&id))
                .map_or_else(|| "NOBODY".to_string(), |h| h.name.to_uppercase());
            self.found = Some((self.tick, 0, format!("{name} SHOT THE MOST BATS!")));
            self.sounds.push("play_card");
            self.cues.push("pastime_won".into());
        }
        self.bats = Some(shoot);
    }
}

#[cfg(test)]
#[path = "../../tests/rules/together_folly__tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/rules/together_folly_sim__tests.rs"]
mod sim;
