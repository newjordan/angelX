//! # GANDER JOUST (round two)
//!
//! ## The feeling
//! Flapping a heavy war-goose up in a desperate climb — and the clack when
//! your lance comes down on top of anyone at all, friend or rival. This is
//! the point of Joust: one hall, everyone in it.
//!
//! ## The loop
//! Every knight in the heat rides the SAME hall at the same time, against
//! each other and against the rival waves, on stone ledges over a tar pit.
//! Tap F to flap, steer with left and right; when two riders meet, the
//! higher lance unhorses the lower. An unhorsed rider drops an egg that
//! rolls along the ledge: touch it in time for a bonus clack; leave it and
//! it hatches into a rival rider. The tar's hand rises for anyone who
//! sinks too low. Most unhorsings in 75 seconds wins; fewest takes the
//! booby.
//!
//! ## Words on the board
//! FLAP · LAND ON TOP
//!
//! ## Who plays
//! Every seated knight, at once, in ONE shared hall (a lone knight still
//! has the three rival waves).
//!
//! ## Controls
//! Move (left/right) to lean the goose, F to flap. Nothing else.
//!
//! ## The five numbers (1/256 tile a tick; HZ = 30)
//! 1. FLAP_KICK = 52 — one flap's kick up. Heavy: the goose sags a moment
//!      before it climbs, so climbs must be planned.
//! 2. GRAVITY = 14 — the constant pull down; with FALL_MAX = 96 the goose
//!      gathers speed quickly and coasts when level.
//! 3. HATCH = 150 ticks (5 s) — how long an egg waits on its ledge before
//!      it hatches into a rival rider. Five seconds is a dash and a flap:
//!      reachable, but only if you see it at once.
//! 4. RESPAWN = 20 — ticks a felled rival is down before its wave returns.
//! 5. NEED = 8 — unhorsings to take the heat early, inside 75 s.
//!
//! ## Round two's changes (from the operator's review of round one)
//! - ONE shared hall: the round's `gander` is the hall for every seat.
//!   Friends clack friends, and it scores.
//! - EGGS: every unhorsed rider drops one; touching it is a bonus clack,
//!   letting it hatch brings a rival rider.
//! - The hall is drawn SIDE-ON (a hall, not slabs on the map): stone
//!   ledges with moss and torches over a tar pit, realm inks.
//! - Riders twice their old size, a wing frame on every flap, a lance
//!   that tips down when diving, each knight in their own colour.
//! - A knight playing sees ONLY the game's board; quest boards wait.
//! - The tar's hand: four frames of a real hand rising out of the tar.
//!
//! ## Fail and retry
//! Unhorsed or tarred: a short tumble, then the goose is back under you on
//! a ledge. No menu, no walk back — under half a second either way.
//!
//! ## Pays
//! Score and the game's best (Records::gander), a first-win mark, the heat
//! laurel for the winner and the booby prize for the rest. No treasury
//! faucet: rounds pay the shared bounded purse.
//!
//! ---
//! 1. In the first ten seconds: the goose wallows, gravity wins, the
//!    knight learns to tap F in a rhythm — then a friend drifts underneath
//!    and the first CLACK lands on them.
//! 2. The five numbers are the constants above; each is a proposal until
//!    the operator plays it.
//! 3. Still left out: pterodactyl-style raiders, a second hall, an OST.
//!
//! Determinism: every branch below runs on integers; the whole heat is a
//! pure function of the starting seeds (the wave schedule) and the inputs.

use super::*;

/// One flap's kick up (1/256 tile a tick).
pub const FLAP_KICK: i32 = 52;
/// The pull down each tick, and the fastest the goose may fall or climb.
pub const GRAVITY: i32 = 14;
pub const FALL_MAX: i32 = 96;
pub const RISE_MAX: i32 = 48;
/// Steering: the lean a tick of left/right adds, and the cruise cap.
pub const WALK: i32 = 28;
pub const CRUISE_X: i32 = 72;
/// Ticks off the saddle after a clack, and in the tar's grip.
pub const STUN: u8 = 9;
pub const TAR_STUN: u8 = 6;
/// Ticks a felled rival is down before its wave returns.
pub const RESPAWN: u32 = 20;
/// Unhorsings to take the heat early.
pub const NEED: u8 = 8;
/// The whole heat: 75 s of hall, then the podium.
pub const TIME: u32 = 75 * HZ;
/// The hall: ledges (x0, x1, row) and the tar's surface row, all tiles.
pub const LEDGES: [(i32, i32, i32); 3] = [(4, 9, 4), (12, 18, 6), (7, 13, 8)];
pub const TAR_ROW: i32 = 11;
/// The highest a rider climbs (1/256 tile): under the gallery's rail, and
/// the top of a rider's valid range.
pub const CEILING: i32 = 2 * 256 + 128;
/// The hall's walls, in tiles.
pub const WALL_X: (i32, i32) = (1, 22);
/// When each rival wave first arrives (ticks into the round), the lane it
/// holds and the side it enters from.
pub const WAVES: [(u32, i32, i32); 3] = [(30, 5, 21), (240, 7, 2), (450, 6, 12)];
/// How long an unhorsed rider's egg waits before it hatches a rival.
pub const HATCH: u32 = 150;

/// A rider: a seated knight (who 0, `seat` their id) or a rival
/// (who 1..=3, seat 0). Fixed-point 1/256 tile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GanderRider {
    /// 0 for a seated knight, 1..=3 for a rival wave.
    pub who: u8,
    /// The rider's seat (their colour on screen); 0 for a rival.
    pub seat: u8,
    pub x: i32,
    pub y: i32,
    pub dx: i32,
    pub dy: i32,
    pub face: i8,
    pub stun: u8,
    /// Which rival this rider last clashed with (0: none), so a pass
    /// resolves once and shoves apart instead of re-triggering.
    pub clash: u8,
    /// Ticks before a felled rival's wave returns.
    pub down: u32,
    pub lane: i32,
}
impl GanderRider {
    fn knight(seat: u32, x: i32, y: i32) -> Self {
        Self {
            who: 0,
            seat: seat as u8,
            x,
            y,
            dx: 0,
            dy: 0,
            face: 1,
            stun: 0,
            clash: 0,
            down: 0,
            lane: 8 * 256,
        }
    }
    fn rival(i: usize) -> Self {
        let (_, lane, from) = WAVES[i];
        Self {
            who: i as u8 + 1,
            seat: 0,
            x: from * 256,
            y: lane * 256,
            dx: if from > 11 { -40 } else { 40 },
            dy: 0,
            face: if from > 11 { -1 } else { 1 },
            stun: 0,
            clash: 0,
            down: WAVES[i].0,
            lane: lane * 256,
        }
    }
    fn seat_of(&self) -> Option<u32> {
        (self.who == 0).then_some(u32::from(self.seat)).filter(|&s| s > 0)
    }
    pub fn resting_on(&self) -> Option<i32> {
        LEDGES.iter().find_map(|&(x0, x1, row)| {
            (self.y == row * 256 && (x0 * 256..=x1 * 256).contains(&self.x)).then_some(row * 256)
        })
    }
    fn valid(&self) -> bool {
        (WALL_X.0 * 256..=WALL_X.1 * 256).contains(&self.x)
            && (2 * 256..=TAR_ROW * 256).contains(&self.y)
            && self.dx.abs() <= CRUISE_X
            && self.dy.abs() <= FALL_MAX.max(RISE_MAX)
            && self.stun <= STUN
            && self.clash <= 3 + MAX_PLAYERS as u8
            && self.down <= 30 * HZ
            && (-1..=1).contains(&self.face)
            && ((self.who == 0 && (1..=MAX_PLAYERS as u8).contains(&self.seat))
                || (1..=3).contains(&self.who))
    }
}

/// An unhorsed rider's egg, rolling on a ledge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GanderEgg {
    /// Its x, fixed point; y is the ledge row it sits on (`row`).
    pub x: i32,
    pub row: i32,
    /// Direction it rolls (1 or -1).
    pub dir: i8,
    /// Ticks until it hatches (0: it hatched — see `hatched`).
    pub left: u32,
}
impl GanderEgg {
    fn valid(&self) -> bool {
        (WALL_X.0 * 256..=WALL_X.1 * 256).contains(&self.x)
            && LEDGES.iter().any(|&(_, _, row)| row == self.row)
            && self.dir.abs() == 1
            && self.left <= HATCH
    }
}

/// The tar's hand, mid-grab: where it rose and how far.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GanderHand {
    pub x: i32,
    /// 1..=4: the frame of the rise (4 is closing on the rider).
    pub frame: u8,
}
impl GanderHand {
    fn valid(&self) -> bool {
        (WALL_X.0 * 256..=WALL_X.1 * 256).contains(&self.x) && (1..=4).contains(&self.frame)
    }
}

/// The one hall every seat in the heat rides together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gander {
    /// The seats riding, first the starter; matches `Heat::seats`.
    pub seats: Vec<u32>,
    /// Riders 0..seats.len() are the knights, in seat order; then the
    /// three rival waves.
    pub riders: Vec<GanderRider>,
    /// Each seat's unhorsing count (its rider's `who` is 0; a rival's is
    /// its index past the knights).
    pub tally: Vec<u8>,
    /// Eggs on the ledges (at most three: one per ledge).
    pub eggs: Vec<GanderEgg>,
    /// The tar's hand, mid-grab (at most one at a time).
    pub hand: Option<GanderHand>,
}
impl Gander {
    pub fn new(seats: &[u32]) -> Self {
        let n = seats.len().min(MAX_PLAYERS as usize);
        let mut riders: Vec<GanderRider> = Vec::with_capacity(n + 3);
        for (i, &seat) in seats.iter().take(n).enumerate() {
            riders.push(GanderRider::knight(seat, (5 + i as i32 * 2) * 256, 8 * 256));
        }
        for i in 0..3 {
            riders.push(GanderRider::rival(i));
        }
        let mut g = Self {
            seats: seats.to_vec(),
            riders,
            tally: vec![0; n],
            eggs: Vec::new(),
            hand: None,
        };
        let _ = 0;
        g
    }
    /// The first knight's rider (the starter's).
    pub fn rider(&self) -> &GanderRider {
        &self.riders[0]
    }
    /// A seat's rider in the hall.
    pub fn rider_of(&self, seat: u32) -> Option<&GanderRider> {
        let i = self.seats.iter().position(|&s| s == seat)?;
        self.riders.get(i)
    }
    pub fn valid(&self, r: &Round) -> bool {
        let n = self.seats.len();
        (1..=MAX_PLAYERS as usize).contains(&n)
            && self.seats.iter().all(|&s| (1..=MAX_PLAYERS).contains(&s))
            && self.tally.len() == n
            && self.tally.iter().all(|&t| t <= r.game.need())
            && self.riders.iter().take(n).enumerate().all(|(i, g)| {
                g.who == 0 && g.seat as u32 == self.seats[i] && g.down == 0 && g.lane == 8 * 256
            })
            && self.riders.iter().skip(n).enumerate().all(|(i, g)| {
                g.who == i as u8 + 1
                    && g.seat == 0
                    && (2 * 256..=9 * 256).contains(&g.lane)
                    && (i < 3 || g.down > 0)
            })
            && self.riders.iter().all(|g| g.valid())
            && self.eggs.len() <= 3
            && self.eggs.iter().all(|e| e.valid())
            && self.hand.is_none_or(|h| h.valid())
            && r.mistakes <= 20
    }
}

impl Run {
    /// One tick of THE hall, for every seat at once. False once the round
    /// is over (need reached); the caller handles the clock separately.
    pub(super) fn tick_gander(
        &mut self,
        r: &mut Round,
        inputs: &BTreeMap<u32, Input>,
        eligible: &[(u32, f32, f32)],
        tapped: &[u32],
    ) -> bool {
        let seats = r.gander.as_ref().unwrap().seats.clone();
        if !seats
            .iter()
            .any(|&s| eligible.iter().any(|&(id, _, _)| id == s))
        {
            self.found = Some((self.tick, 0, "GOOSE FLOWN OFF.".into()));
            return false;
        }
        let count = r.gander.as_ref().unwrap().tally.len();
        if r.gander.as_ref().unwrap().tally.iter().any(|&t| t >= r.game.need()) {
            return true;
        }
        // The hall is taken out of the round while it ticks, so the tick
        // may read the round (mistakes, sounds) and write the hall freely.
        let mut gander = r.gander.take().unwrap();
        // Physics for every rider: flaps, lean, gravity, ledges.
        for i in 0..count + 3 {
            // A tumble is short: the stun runs down a tick at a time, and
            // the goose is back under the rider when it is spent.
            gander.riders[i].stun = gander.riders[i].stun.saturating_sub(1);
            let mut clash_at = None;
            if gander.riders[i].clash != 0 {
                let c = gander.riders[i].clash as usize;
                if c < gander.riders.len() {
                    clash_at = Some((c, gander.riders[c].down > 0));
                }
            }
            let knight = gander.riders[i].who == 0;
            // A clash against a felled rider is spent: clear it at once.
            if clash_at.is_some_and(|c| c.1) {
                gander.riders[i].clash = 0;
            }
            if !knight && gander.riders[i].down > 0 {
                gander.riders[i].down -= 1;
                if gander.riders[i].down == 0 {
                    let (_, lane, from) = WAVES[i - count];
                    gander.riders[i].x = from * 256;
                    gander.riders[i].y = lane * 256;
                    gander.riders[i].dx = if from > 11 { -40 } else { 40 };
                    gander.riders[i].dy = 0;
                    gander.riders[i].face = if from > 11 { -1 } else { 1 };
                }
                continue;
            }
            // The flaps: a knight's own key edge, a rival's hunger for its
            // lane (a rival hunts the nearest knight now, not only the
            // first).
            let target_x = if knight {
                gander.riders[i].x
            } else {
                gander.riders[..count]
                    .iter()
                    .filter(|k| k.stun == 0 && k.y < TAR_ROW * 256)
                    .min_by_key(|k| k.x.abs_diff(gander.riders[i].x))
                    .map_or(gander.riders[i].x, |k| k.x)
            };
            let flap = if knight {
                tapped.contains(&u32::from(gander.riders[i].seat)) && gander.riders[i].stun == 0
            } else {
                (gander.riders[i].dy > 42 || gander.riders[i].y > gander.riders[i].lane + 96)
                    && gander.riders[i].stun == 0
            };
            gander.riders[i].dy = (gander.riders[i].dy + GRAVITY).clamp(-RISE_MAX, FALL_MAX);
            if flap {
                // A press wins its whole tick: the goose leaves the stone
                // at the full kick, gravity collecting from the next.
                gander.riders[i].dy = gander.riders[i].dy.min(-FLAP_KICK);
            }
            let lean = if knight {
                inputs
                    .get(&u32::from(gander.riders[i].seat))
                    .filter(|j| j.valid())
                    .map_or(0, |j| i32::from(j.aim_x))
            } else {
                (target_x - gander.riders[i].x).signum() * (if i % 2 == 0 { 1 } else { 0 })
            };
            if gander.riders[i].stun == 0 {
                gander.riders[i].dx =
                    (gander.riders[i].dx + lean * WALK).clamp(-CRUISE_X, CRUISE_X);
            }
            let before_y = gander.riders[i].y;
            gander.riders[i].x =
                (gander.riders[i].x + gander.riders[i].dx).clamp(WALL_X.0 * 256, WALL_X.1 * 256);
            gander.riders[i].y += gander.riders[i].dy;
            // The hall has a roof: under the gallery rail a climb stops.
            if gander.riders[i].y < CEILING {
                gander.riders[i].y = CEILING;
                gander.riders[i].dy = gander.riders[i].dy.max(0);
            }
            // And a floor: the tar's skin holds a sunk rider for its hand.
            if gander.riders[i].y > TAR_ROW * 256 {
                gander.riders[i].y = TAR_ROW * 256;
                gander.riders[i].dy = 0;
            }
            if gander.riders[i].dx != 0 {
                gander.riders[i].face = gander.riders[i].dx.signum() as i8;
            }
            // Ledges: land when falling across one, and rest while on it.
            if gander.riders[i].dy > 0 {
                for &(x0, x1, row) in LEDGES.iter() {
                    let top = row * 256;
                    if before_y <= top
                        && gander.riders[i].y >= top
                        && (x0 * 256..=x1 * 256).contains(&gander.riders[i].x)
                    {
                        gander.riders[i].y = top;
                        gander.riders[i].dy = 0;
                        gander.riders[i].dx = gander.riders[i].dx * 3 / 4;
                    }
                }
            } else if gander.riders[i].dy == GRAVITY && gander.riders[i].resting_on().is_some() {
                // Standing on the ledge: gravity is held at bay while the
                // goose is walked along it.
                gander.riders[i].y = gander.riders[i].resting_on().unwrap();
                gander.riders[i].dy = 0;
            }
        }
        // The tar's hand: it rises for any rider who sinks too low, in
        // four frames, and pulls the rider down. Only one hand at a time.
        {
            let grabbed = gander
                .riders
                .iter()
                .enumerate()
                .find(|(i, k)| k.stun == 0 && k.down == 0 && k.y >= TAR_ROW * 256 - 26)
                .map(|(i, k)| (i, k.who == 0, k.x));
            match (grabbed, gander.hand.as_mut()) {
                (Some((i, is_knight, x)), Some(h)) => {
                    // The rise is under way: keep after the same rider.
                    if h.frame < 4 {
                        h.frame += 1;
                    }
                    if h.frame == 4 {
                        // Closed: the rider goes down into the tar.
                        gander.hand = None;
                        self.tar_takes(&mut gander, r, i, is_knight);
                    }
                }
                (Some((i, is_knight, x)), None) => {
                    gander.hand = Some(GanderHand { x, frame: 1 });
                    self.sounds.push("hero_hurt");
                }
                (None, Some(h)) => {
                    // The rider escaped the reach: the hand sinks.
                    h.frame = h.frame.saturating_sub(1);
                    if h.frame == 0 {
                        gander.hand = None;
                    }
                }
                (None, None) => {}
            }
        }
        // The clacks: every pair of riders, knights and rivals alike. The
        // higher lance unhorses the lower; a friend's lance counts the
        // same as a rival's. A pair resolves once (clash), then separates.
        let mut clacks: Vec<(usize, usize)> = Vec::new();
        for a in 0..count + 3 {
            if gander.riders[a].stun > 0 || gander.riders[a].down > 0 || gander.riders[a].clash != 0
            {
                continue;
            }
            for b in (a + 1)..count + 3 {
                if gander.riders[b].stun > 0
                    || gander.riders[b].down > 0
                    || gander.riders[b].clash != 0
                {
                    continue;
                }
                let (ax, ay) = (gander.riders[a].x, gander.riders[a].y);
                let (bx, by) = (gander.riders[b].x, gander.riders[b].y);
                if ax.abs_diff(bx) < 200 && ay.abs_diff(by) < 176 {
                    clacks.push((a, b));
                }
            }
            // (a's clash flag is only set below, so no re-scan is needed.)
        }
        clacks.sort_unstable();
        for &(a, b) in &clacks {
            // Resolve every pair on the same positions they were found at.
            if gander.riders[a].clash != 0 || gander.riders[b].clash != 0 {
                continue;
            }
            if gander.riders[a].stun > 0 || gander.riders[b].stun > 0 {
                continue;
            }
            let above = gander.riders[a].y + 40 < gander.riders[b].y;
            let below = gander.riders[b].y + 40 < gander.riders[a].y;
            let winner = if above { Some(a) } else if below { Some(b) } else { None };
            match winner {
                Some(w) if gander.riders[w].who == 0 => {
                    // A knight's lance came down on a friend or a rival.
                    self.clack(&mut gander, r, w, if w == a { b } else { a });
                }
                Some(w) => {
                    // A rival's lance came down on the other rider.
                    let v = if w == a { b } else { a };
                    if gander.riders[v].who == 0 {
                        self.unhorse_knight(&mut gander, r, v);
                    } else {
                        // Rival on rival: the lower one goes down, no score.
                        self.felled(&mut gander, v);
                        gander.riders[v].down = RESPAWN;
                    }
                }
                None => {
                    // Level pass: both geese shove apart, once.
                    gander.riders[a].clash = b as u8;
                    gander.riders[b].clash = a as u8;
                    gander.riders[a].dx = -(gander.riders[a].dx.signum() * 48).clamp(-CRUISE_X, CRUISE_X);
                    gander.riders[b].dx = -(gander.riders[b].dx.signum() * 48).clamp(-CRUISE_X, CRUISE_X);
                }
            }
        }
        // A spent clash clears once the pair separates.
        for i in 0..count + 3 {
            let c = gander.riders[i].clash;
            if c != 0
                && (usize::from(c) >= gander.riders.len()
                    || gander.riders[usize::from(c)].down > 0
                    || gander.riders[i].x.abs_diff(gander.riders[usize::from(c)].x) >= 200
                    || gander.riders[i].y.abs_diff(gander.riders[usize::from(c)].y) >= 176)
            {
                gander.riders[i].clash = 0;
            }
        }
        // The eggs: roll along their ledge; touch one for a bonus clack,
        // let it hatch for a rival. At most three eggs (one per ledge).
        let mut hatch: Option<i32> = None;
        for k in (0..gander.eggs.len()).rev() {
            let e = &mut gander.eggs[k];
            let (x0, x1, _) = LEDGES.iter().find(|&&(_, _, row)| row == e.row).unwrap();
            e.x = (e.x + e.dir as i32 * 40).clamp(x0 * 256 + 16, x1 * 256 - 16);
            if e.x == x0 * 256 + 16 || e.x == x1 * 256 - 16 {
                e.dir = -(e.dir);
            }
            e.left = e.left.saturating_sub(1);
            if e.left == 0 {
                hatch = Some(k as i32);
                continue;
            }
            // A rider touches the egg (any knight or rival touching counts:
            // the egg was dropped for the knights to fight over).
            let touched = gander
                .riders
                .iter()
                .enumerate()
                .find(|(_, q)| q.stun == 0 && q.down == 0 && q.x.abs_diff(e.x) < 128 && q.y.abs_diff(e.row * 256) < 128)
                .map(|(i, _)| i);
            if let Some(t) = touched {
                gander.eggs.remove(k);
                if gander.riders[t].who == 0 && t < count {
                    let seat = u32::from(gander.riders[t].seat);
                    gander.tally[t] = gander.tally[t].saturating_add(1).min(r.game.need());
                    self.sounds.push("spoil_pickup");
                    self.found = Some((self.tick, seat, "EGG! A BONUS CLACK.".into()));
                }
            }
        }
        if let Some(k) = hatch {
            let e = gander.eggs.remove(k as usize);
            // Hatched: a rival rider enters the hall where the egg lay.
            // Reuse whichever rival slot is down (or the first, if none).
            let slot = (count..count + 3).find(|&i| gander.riders[i].down > 0).unwrap_or(count);
            gander.riders[slot].x = e.x.clamp(WALL_X.0 * 256, WALL_X.1 * 256);
            gander.riders[slot].y = (e.row - 2).max(2) * 256;
            gander.riders[slot].dx = if e.dir > 0 { 40 } else { -40 };
            gander.riders[slot].dy = 0;
            gander.riders[slot].face = e.dir;
            gander.riders[slot].stun = 0;
            gander.riders[slot].clash = 0;
            gander.riders[slot].down = 0;
            gander.riders[slot].lane = e.row * 256;
            self.sounds.push("clang");
            self.found = Some((self.tick, 0, "AN EGG HATCHED! A RIVAL RIDES.".into()));
        }
        // Every knight rides in the sim too: their own screen shows them
        // aboard, not standing on the marsh floor.
        let riders: [(u32, i32, i32); MAX_PLAYERS as usize] = std::array::from_fn(|i| {
            gander
                .seats
                .get(i)
                .map(|&s| {
                    let k = &gander.riders[i];
                    (s, k.x, k.y)
                })
                .unwrap_or((0, 0, 0))
        });
        for (seat, x, y) in riders {
            if seat == 0 {
                continue;
            }
            if let Some(hero) = self.players.get_mut(&seat) {
                hero.x = x as f32 / 256.0 * TILE_UNITS;
                hero.y = y as f32 / 256.0 * TILE_UNITS;
            }
        }
        r.gander = Some(gander);
        true
    }

    /// A knight's lance came down on another rider: it scores, and the
    /// felled rider (friend or rival) drops an egg on the ledge below.
    fn clack(&mut self, gander: &mut Gander, r: &Round, w: usize, v: usize) {
        let seat = u32::from(gander.riders[w].seat);
        gander.tally[w] = gander.tally[w].saturating_add(1).min(r.game.need());
        // The pair resolves once, then separates.
        gander.riders[w].clash = v as u8;
        gander.riders[v].clash = w as u8;
        if gander.riders[v].who == 0 {
            // A friend: unhorsed, a tumble, back on a ledge.
            gander.riders[v].stun = STUN;
            gander.riders[v].dy = 0;
        } else {
            gander.riders[v].down = RESPAWN;
            gander.riders[v].stun = 0;
        }
        self.sounds.push("spoil_pickup");
        self.found = Some((self.tick, seat, "CLACK! OFF IT COMES.".into()));
        self.felled(gander, v);
    }

    /// A rival's lance came down on a knight: a mistake, and an egg.
    fn unhorse_knight(&mut self, gander: &mut Gander, r: &mut Round, v: usize) {
        let seat = u32::from(gander.riders[v].seat);
        gander.riders[v].stun = STUN;
        gander.riders[v].clash = 0;
        gander.riders[v].dy = 0;
        r.mistakes = r.mistakes.saturating_add(1).min(20);
        self.sounds.push("clang");
        self.found = Some((self.tick, seat, "UNHORSED! CLIMB AGAIN.".into()));
        self.felled(gander, v);
    }

    /// The tar's hand closed on a rider: pull it down (a knight lifts back
    /// onto the middle ledge; a rival goes down until its wave returns).
    fn tar_takes(&mut self, gander: &mut Gander, r: &mut Round, i: usize, is_knight: bool) {
        if is_knight {
            gander.riders[i].stun = TAR_STUN;
            gander.riders[i].y = LEDGES[2].2 * 256;
            gander.riders[i].dy = 0;
            let (x0, _, _) = LEDGES[2];
            gander.riders[i].x = gander.riders[i].x.clamp(x0 * 256, LEDGES[2].1 * 256);
            r.mistakes = r.mistakes.saturating_add(1).min(20);
            self.sounds.push("hero_hurt");
            self.found = Some((self.tick, u32::from(gander.riders[i].seat), "THE TAR TOOK YOU.".into()));
        } else {
            gander.riders[i].down = RESPAWN;
            gander.riders[i].stun = 0;
            self.felled(gander, i);
        }
    }

    /// A rider leaves the saddle: if the fall is over a ledge, an egg
    /// drops and rolls there; one egg per ledge at a time.
    fn felled(&mut self, gander: &mut Gander, v: usize) {
        let (x, y) = (gander.riders[v].x, gander.riders[v].y);
        let ledge = LEDGES.iter().copied().find(|&(x0, x1, row)| {
            y >= (row - 1) * 256 && y <= (row + 2) * 256 && (x0 * 256..=x1 * 256).contains(&x)
        });
        let Some((_, _, row)) = ledge else { return };
        if gander.eggs.iter().any(|e| e.row == row) {
            return;
        }
        gander.eggs.push(GanderEgg {
            x,
            row,
            dir: gander.riders[v].face,
            left: HATCH,
        });
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/together_gander__tests.rs"]
mod tests;
