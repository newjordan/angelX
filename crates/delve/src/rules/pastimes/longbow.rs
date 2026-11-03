//! Longbow Green: a painted wooden popinjay slides along a rope before the
//! butts. Walk the line to lead it, hold F to draw, let go to loose; the
//! wind shifts after every hit. Three hits in six arrows wins. Host-owned
//! fixed-point arrows; score only, no treasury minting.
use super::*;
use super::super::bowcountry::{LINE, ROPE};

/// An arrow's time in the air, and the draw that carries it to the rope.
pub const FLIGHT: u32 = HZ;
pub const FULL_DRAW: u32 = HZ / 2;
/// How near the popinjay a hit must land (1/256 tile).
pub const HIT: u32 = 160;
/// Ticks the particle burst plays after a hit.
pub const HIT_BURST: u8 = 12;
/// Arrows in the quiver, and the wind (1/256 tile a tick) for each hit so far.
pub const ARROWS: u8 = 6;
pub const WINDS: [i32; 4] = [-3, 3, 0, -2];
/// The popinjay's run along the rope (tile columns) and its round trip.
pub const RUN: (i32, i32) = (7, 17);
pub const PERIOD: u32 = 4 * HZ;
/// Where an archer may stand on the line (1/256 tile).
pub const AIM: (i32, i32) = (4 * 256, 20 * 256);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrow {
    pub x: i32,
    pub launch: i32,
    pub power: u32,
    pub left: u32,
}
impl Arrow {
    /// Its height (1/256 tile): from the line towards the rope, as far as
    /// the draw carries it.
    pub fn y(&self) -> i32 {
        let rise = ((LINE - ROPE) * 256.0) as u32;
        (LINE * 256.0) as i32
            - (rise * self.power.min(FULL_DRAW) * (FLIGHT - self.left) / (FULL_DRAW * FLIGHT)) as i32
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Longbow {
    pub owner: u32,
    /// Where the archer stands on the line (1/256 tile).
    pub aim: i32,
    /// Ticks F has been held.
    pub draw: u32,
    pub arrows: u8,
    pub flight: Option<Arrow>,
    pub wait: u32,
    /// Ticks left of the hit-burst particle effect (feathers/wood chips).
    #[serde(default)]
    pub hit_burst: u8,
    /// Last 6 arrow positions in flight (for trail rendering).
    #[serde(default)]
    pub arrow_trail: Vec<(i32, i32)>,
    /// Ticks remaining for arrow stuck in ground after a miss.
    #[serde(default)]
    pub ground_arrow: u8,
    /// Position of the grounded arrow (miss landing spot).
    #[serde(default)]
    pub ground_x: i32,
    #[serde(default)]
    pub ground_y: i32,
}
impl Longbow {
    pub fn new(owner: u32) -> Self {
        Self { owner, aim: 12 * 256, draw: 0, arrows: 0, flight: None, wait: 0, hit_burst: 0, arrow_trail: Vec::new(), ground_arrow: 0, ground_x: 0, ground_y: 0 }
    }
    pub fn wind(progress: u8) -> i32 {
        WINDS[usize::from(progress.min(3))]
    }
    /// The popinjay at `age` ticks into a round: a steady run along the
    /// rope and back, so a knight can learn to lead it.
    pub fn target_at(age: u32, progress: u8) -> i32 {
        let phase = (age + u32::from(progress) * HZ) % PERIOD;
        let span = (RUN.1 - RUN.0) * 256;
        RUN.0 * 256 + (phase.min(PERIOD - phase) * span as u32 / (PERIOD / 2)) as i32
    }
    pub fn target(round: &Round) -> i32 {
        Self::target_at(round.game.time() - round.left, round.progress)
    }
    pub fn valid(&self, r: &Round) -> bool {
        (1..=MAX_PLAYERS).contains(&self.owner)
            && (AIM.0..=AIM.1).contains(&self.aim)
            && self.draw <= HZ
            && self.arrows <= ARROWS
            && self.wait <= HZ / 2
            && self.hit_burst <= HIT_BURST
            && self.arrow_trail.len() <= 6
            && self.ground_arrow <= 6
            && r.mistakes <= ARROWS
            && u16::from(self.arrows) == u16::from(r.progress) + u16::from(r.mistakes) + u16::from(self.flight.is_some())
            && (self.flight.is_none() || (self.draw == 0 && self.wait == 0))
            && (self.wait == 0 || self.draw == 0)
            && self.flight.as_ref().is_none_or(|a| {
                a.left > 0
                    && a.left <= FLIGHT
                    && a.power > 0
                    && a.power <= HZ
                    && (AIM.0..=AIM.1).contains(&a.launch)
                    && a.x == a.launch + Self::wind(r.progress) * (FLIGHT - a.left) as i32
            })
    }
}
impl Run {
    /// One tick of one archer's round. False once it is lost.
    pub(super) fn tick_longbow(&mut self, r: &mut Round, inputs: &BTreeMap<u32, Input>, eligible: &[(u32, f32, f32)]) -> bool {
        let b = r.longbow.as_mut().unwrap();
        if !eligible.iter().any(|&(id, _, _)| id == b.owner) {
            self.found = Some((self.tick, 0, "ARCHER RETIRED.".into()));
            return false;
        }
        let input = inputs.get(&b.owner).filter(|i| i.valid()).copied().unwrap_or_default();
        // The archer toes the line: walking moves the aim, drawing holds still.
        if let Some(hero) = self.players.get_mut(&b.owner) {
            if b.draw == 0 && b.flight.is_none() {
                b.aim = ((hero.x / TILE_UNITS * 256.0) as i32).clamp(AIM.0, AIM.1);
            }
            hero.x = b.aim as f32 / 256.0 * TILE_UNITS;
            hero.y = LINE * TILE_UNITS;
        }
        if let Some(mut a) = b.flight.take() {
            // Track arrow position for trail rendering (keep last 6).
            b.arrow_trail.push((a.x, a.y()));
            if b.arrow_trail.len() > 6 {
                b.arrow_trail.remove(0);
            }
            a.x += Longbow::wind(r.progress);
            a.left -= 1;
            if a.left == 0 {
                let target = Longbow::target_at(r.game.time() - r.left, r.progress);
                if a.power >= FULL_DRAW && a.x.abs_diff(target) <= HIT {
                    r.progress += 1;
                    r.score_popup = 8;
                    r.score_popup_text = "POPINJAY!".to_string();
                    r.score_popup_x = (target / 256) as i32;
                    r.score_popup_y = 4;
                    self.sounds.push("hit");
                    self.shake = self.shake.max(2);
                    self.found = Some((self.tick, b.owner, "POPINJAY HIT!".into()));
                    b.hit_burst = HIT_BURST;
                    b.arrow_trail.clear();
                    if r.progress >= r.game.need() && r.mistakes == 0 {
                        self.notice("popinjay");
                    }
                } else {
                    r.mistakes += 1;
                    self.sounds.push("clang");
                    self.found = Some((
                        self.tick,
                        b.owner,
                        if a.power < FULL_DRAW { "TOO SHORT! DRAW LONGER." } else { "MISS! LEAD IT." }.into(),
                    ));
                    // Arrow stuck in ground with dust puff.
                    b.ground_arrow = 6;
                    b.ground_x = a.x;
                    b.ground_y = a.y();
                    b.arrow_trail.clear();
                }
                b.wait = HZ / 2;
            } else {
                b.flight = Some(a);
            }
        } else if b.wait > 0 {
            b.wait -= 1;
            b.hit_burst = b.hit_burst.saturating_sub(1);
            b.ground_arrow = b.ground_arrow.saturating_sub(1);
        } else if input.fire && b.arrows < ARROWS {
            // Draw sound: play "swing" when the draw starts (first frame)
            if b.draw == 0 { self.sounds.push("swing"); }
            b.draw = (b.draw + 1).min(HZ);
        } else if b.draw > 0 {
            b.flight = Some(Arrow { x: b.aim, launch: b.aim, power: b.draw, left: FLIGHT });
            b.draw = 0;
            b.arrows += 1;
            b.arrow_trail.clear();
            self.sounds.push("shot_bow");
        }
        let lost = r.progress < r.game.need() && b.arrows >= ARROWS && b.flight.is_none();
        if lost {
            self.found = Some((self.tick, b.owner, "QUIVER EMPTY!".into()));
        }
        !lost
    }
}
