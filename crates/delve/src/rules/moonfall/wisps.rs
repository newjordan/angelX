//! The Gloam's wisps: after dusk a few hooded lights drift to the nearest
//! flame to put it out. A knight's lantern keeps its neighbours lit; a touch
//! costs a candle. There is nothing to fight: they are weather with
//! intent, and every extra knight at the table brings more of it.
use super::clock;
use super::state::*;
use super::town::{self, Q, TILE};
use super::Session;

/// Pixels (times `Q`) a wisp drifts a tick: one, and a tenth more for each
/// knight beyond the first.
const DRIFT: i32 = 256;
/// Ticks a wisp spends putting a flame out.
pub const SNUFF: u32 = 60;
/// How near a knight must stand to a flame to keep a wisp off it, pixels.
pub const WARD: i32 = 40;
/// How near a wisp must come to a knight to touch.
const TOUCH: i32 = 9;
/// Ticks a knight is warded after a touch.
pub const WARDED: u32 = 75;
/// Ticks a snuffed knight lies before the well carries them back.
pub const DOWN: u32 = 90;

/// Where the wisps come in from, around the edge of the town, in pixels.
const COMING: [(i32, i32); 8] = [
    (-20, 400),
    (150, 440),
    (300, 450),
    (470, 440),
    (630, 300),
    (630, 120),
    (330, -20),
    (-20, 180),
];

/// The length of `(dx, dy)`, in integers: the same answer on every machine.
fn isqrt(dx: i32, dy: i32) -> i32 {
    let n = (i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy)) as u64;
    let (mut lo, mut hi) = (0u64, 1u64 << 32);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if mid * mid <= n {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo as i32
}

impl Session {
    /// How many wisps the night holds for the table: the number, and two
    /// more for every knight beyond the first, and one more in the Final Hours.
    pub fn wisp_want(&self) -> usize {
        if !clock::night(self.clock, self.nums.day) || self.completed.is_some() {
            return 0;
        }
        let seats = self.seats_peak() as usize;
        let more = usize::from(self.final_hours());
        (self.nums.wisps as usize + 2 * (seats - 1) + more).min(MAX_WISPS)
    }

    /// The lit lights, by id and place, in pixels.
    pub fn lights(&self) -> Vec<(u8, (i32, i32))> {
        let mut out = Vec::new();
        for n in 0..6u8 {
            if self.lantern_lit(n) {
                let (tx, ty) = town::LAMPS[usize::from(n)];
                out.push((n, (tx * TILE + 8, ty * TILE)));
            }
        }
        for (i, at) in [town::NORTH_DOOR, town::SOUTH_DOOR, town::WELL].into_iter().enumerate() {
            if self.flame(i) {
                out.push((6 + i as u8, at));
            }
        }
        out
    }

    /// Whether a knight stands close enough to keep wisps off a light.
    pub fn guarded(&self, at: (i32, i32)) -> bool {
        self.seats.values().any(|s| {
            let (x, y) = s.knight.px();
            s.knight.up() && (x - at.0).pow(2) + (y - at.1).pow(2) <= WARD * WARD
        })
    }

    fn put_out(&mut self, aim: u8) {
        if aim < 6 {
            self.world.snuffed |= 1 << aim;
        } else {
            self.world.lit_until[usize::from(aim) - 6] = 0;
        }
        self.pending.push("mana_empty");
        self.announce(Note::Out);
    }

    /// One tick of the Gloam. Returns the cues to sound.
    pub fn tick_wisps(&mut self) -> Vec<&'static str> {
        let mut cues = Vec::new();
        let want = self.wisp_want();
        if want == 0 {
            self.world.wisps.clear();
            return cues;
        }
        if self.world.wisps.len() < want && self.clock % 45 == 0 {
            let born = self.elapsed;
            let (x, y) = COMING[(born / 45) as usize % COMING.len()];
            self.world.wisps.push(Wisp {
                x: x * Q,
                y: y * Q,
                mode: WispMode::Hunt,
                aim: 0,
                born,
            });
        }
        let drift = DRIFT + DRIFT * (self.seats_peak() as i32 - 1) / 10;
        let lights = self.lights();
        let knights: Vec<(i32, i32)> = self
            .seats
            .values()
            .filter(|s| s.knight.up())
            .map(|s| s.knight.px())
            .collect();
        let mut wisps = std::mem::take(&mut self.world.wisps);
        let mut out_now: Vec<u8> = Vec::new();
        for w in &mut wisps {
            let here = (w.x / Q, w.y / Q);
            match w.mode {
                WispMode::Hunt => {
                    if lights.is_empty() {
                        continue;
                    }
                    // The nearest few, one each by the wisp's birth.
                    let mut order: Vec<_> = lights
                        .iter()
                        .map(|&(id, p)| ((p.0 - here.0).pow(2) + (p.1 - here.1).pow(2), id, p))
                        .collect();
                    order.sort();
                    let (_, id, target) = order[(w.born as usize) % order.len().min(3)];
                    w.aim = id;
                    let (dx, dy) = (target.0 * Q - w.x, target.1 * Q - w.y);
                    let len = isqrt(dx, dy);
                    if len <= 5 * Q {
                        let guarded = knights
                            .iter()
                            .any(|&(x, y)| (x - target.0).pow(2) + (y - target.1).pow(2) <= WARD * WARD);
                        w.mode = if guarded { WispMode::Shy(90) } else { WispMode::Snuff(SNUFF) };
                    } else {
                        w.x += (i64::from(dx) * i64::from(drift) / i64::from(len)) as i32;
                        w.y += (i64::from(dy) * i64::from(drift) / i64::from(len)) as i32;
                    }
                }
                WispMode::Snuff(t) => {
                    match lights.iter().find(|&&(id, _)| id == w.aim) {
                        None => w.mode = WispMode::Hunt,
                        Some(&(_, at)) => {
                            let guarded = knights
                                .iter()
                                .any(|&(x, y)| (x - at.0).pow(2) + (y - at.1).pow(2) <= WARD * WARD);
                            if guarded {
                                w.mode = WispMode::Shy(90);
                            } else if t <= 1 {
                                out_now.push(w.aim);
                                w.mode = WispMode::Hunt;
                            } else {
                                w.mode = WispMode::Snuff(t - 1);
                            }
                        }
                    }
                }
                WispMode::Shy(t) => {
                    // Away from the nearest knight, or from the light.
                    let from = knights
                        .iter()
                        .copied()
                        .min_by_key(|&(x, y)| (x - here.0).pow(2) + (y - here.1).pow(2))
                        .or_else(|| lights.iter().find(|&&(id, _)| id == w.aim).map(|&(_, p)| p))
                        .unwrap_or(here);
                    let (dx, dy) = (here.0 - from.0, here.1 - from.1);
                    let len = isqrt(dx, dy).max(1);
                    w.x += (i64::from(dx) * i64::from(drift * 3 / 2) / i64::from(len)) as i32;
                    w.y += (i64::from(dy) * i64::from(drift * 3 / 2) / i64::from(len)) as i32;
                    w.mode = if t <= 1 { WispMode::Hunt } else { WispMode::Shy(t - 1) };
                }
            }
        }
        // A touch costs a candle.
        let mut hurts: Vec<u32> = Vec::new();
        for w in &wisps {
            let here = (w.x / Q, w.y / Q);
            for (&id, s) in &self.seats {
                let k = &s.knight;
                let (x, y) = k.px();
                if k.up()
                    && k.hurt == 0
                    && k.roll == 0
                    && k.panel == Panel::Shut
                    && !hurts.contains(&id)
                    && (x - here.0).pow(2) + (y - here.1).pow(2) <= TOUCH * TOUCH
                {
                    hurts.push(id);
                }
            }
        }
        self.world.wisps = wisps;
        for aim in out_now {
            self.put_out(aim);
        }
        for id in hurts {
            let ground = self.world.ground(self.clock, self.nums.day, false);
            let crates = self.world.crates.clone();
            let me = self.seats[&id].knight.px();
            // Pushed away from the nearest wisp.
            let from = self
                .world
                .wisps
                .iter()
                .map(|w| (w.x / Q, w.y / Q))
                .min_by_key(|p| (p.0 - me.0).pow(2) + (p.1 - me.1).pow(2))
                .unwrap_or(me);
            let Some(s) = self.seats.get_mut(&id) else { continue };
            let k = &mut s.knight;
            k.candles = k.candles.saturating_sub(1);
            k.hurt = WARDED;
            k.say = None;
            let (dx, dy) = ((me.0 - from.0).signum(), (me.1 - from.1).signum());
            for step in (1..=3).rev() {
                let (nx, ny) = (k.x + dx * 4 * Q * step, k.y + dy * 4 * Q * step);
                if town::clear(nx, ny, ground, &crates) {
                    k.x = nx;
                    k.y = ny;
                    break;
                }
            }
            if k.candles == 0 {
                k.down = DOWN;
                if k.held == Item::Ink {
                    self.world.flags &= !INK_TAKEN;
                }
                k.held = Item::Empty;
                k.panel = Panel::Shut;
                k.note = Some((Note::Snuffed, 90));
                cues.push("knight_fall");
            } else {
                k.note = Some((Note::Candle(k.candles), 90));
                cues.push("hero_hurt");
            }
        }
        cues
    }
}
