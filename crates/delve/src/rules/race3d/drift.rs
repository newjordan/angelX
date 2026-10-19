//! Torchlight Touge: tip a chariot sideways into a torchlit hairpin, catch
//! the slide, release Space and surge onto the straight. Every seat drives.
//! The first ten seconds teach throttle and the approach to the S-bends.
//! Five starting knobs: top speed 18 tiles/s, road 10 tiles wide, 3 laps,
//! charge at 18/42/72 ticks, spin at 60 degrees. All tuning lives here.
//! W/S throttle/brake; A/D steer; hold Space while steering to initiate a
//! low-grip slide. Countersteer to catch it. No weapons or new economy:
//! the race hall's tenth-gold pot and laurel pay once at the shared podium.
//! Version one uses plain mesh placeholders with slots in assets/drift.
use super::*;

pub const DRIFT_LAPS: u32 = 3;
pub const DRIFT_RACE_TIME: u32 = 300 * HZ;
pub const DRIFT_TOP: f32 = 18.0;
pub const DRIFT_WIDTH: f32 = 10.0;
pub const DRIFT_ACCEL: f32 = 5.5;
pub const DRIFT_BRAKE: f32 = 12.0;
pub const DRIFT_STEER: f32 = 1.25;
pub const DRIFT_GRIP: f32 = 7.0;
pub const SLIDE_GRIP: f32 = 1.0;
pub const SPIN_ANGLE: f32 = std::f32::consts::PI / 3.0;
pub const CHARGE_TICKS: [u32; 3] = [18, 42, 72];
pub const BOOST_SPEED: [f32; 3] = [3.0, 5.0, 8.0];
pub const BOOST_TICKS: [u32; 3] = [18, 30, 45];
pub const SPIN_TICKS: u32 = 24;
const MIN_ANGLE: f32 = 0.12;
const MIN_SPEED: f32 = 7.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Drift {
    /// Chassis and velocity headings relative to the road, radians.
    pub heading: f32,
    pub travel: f32,
    pub sliding: bool,
    pub charge: u32,
    pub boost: u32,
    pub boost_stage: u8,
    /// Current corner, last banked corner and whole-race score, degree-ticks.
    pub corner: u32,
    pub last_corner: u32,
    pub score: u32,
    pub corners: u32,
}
impl Drift {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn angle(&self) -> f32 {
        self.heading - self.travel
    }
    pub fn stage(&self) -> usize {
        CHARGE_TICKS
            .iter()
            .take_while(|&&t| self.charge >= t)
            .count()
    }
    pub(super) fn valid(&self) -> bool {
        [self.heading, self.travel]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 4.0)
            && self.charge <= CHARGE_TICKS[2]
            && self.boost <= BOOST_TICKS[2]
            && self.boost_stage <= 3
    }
    fn bank(&mut self) {
        if self.corner > 0 {
            self.last_corner = self.corner;
            self.score = self.score.saturating_add(self.corner);
            self.corners = self.corners.saturating_add(1);
            self.corner = 0;
        }
    }
}

/// Two half-turns with matching tangents close the circuit exactly. The
/// paired S-bends precede the hairpins and the long descending sweeper;
/// the tunnel climbs back to the line. No teleport between laps.
pub(super) fn touge() -> Track {
    let half = [
        piece(120, 0.0, 0.0, DRIFT_WIDTH, true, Ground::Floor),
        piece(55, 35.0, 1.0, DRIFT_WIDTH, true, Ground::Floor),
        piece(80, -70.0, 1.0, DRIFT_WIDTH, true, Ground::Floor),
        piece(55, 35.0, 0.0, DRIFT_WIDTH, true, Ground::Floor),
        piece(80, 160.0, 2.0, DRIFT_WIDTH, true, Ground::Floor),
        piece(130, 20.0, 2.0, DRIFT_WIDTH, true, Ground::Floor),
        piece(80, 0.0, -6.0, DRIFT_WIDTH, true, Ground::Rails),
    ];
    let pieces: Vec<_> = half.into_iter().chain(half).collect();
    let (mut samples, posts) = lay(&pieces);
    let first = samples[0];
    let last = samples.last_mut().unwrap();
    last.p = first.p;
    last.yaw = std::f32::consts::TAU;
    let finish = (samples.len() - 1) as f32;
    Track {
        looped: true,
        samples,
        finish,
        posts,
        items: vec![],
        blocks: vec![],
        gaps: vec![],
    }
}

impl Run {
    /// Space + E at the Hippodrome coach: the same seats and hall stakes.
    pub fn board_drift(&mut self) {
        if self.room().kind != RoomKind::Hippodrome
            || self.race3d_showing()
            || self.chariot.is_some()
        {
            return;
        }
        self.start_race3d_course(self.at, Course::Touge);
    }
}

fn spin_out(r: &mut Rider, noise: &mut Noise) {
    wreck(r, Wreck::SpinOut, noise);
    r.crash = SPIN_TICKS;
    r.drift.bank();
    r.drift.sliding = false;
    r.drift.charge = 0;
    r.drift.boost = 0;
    r.vs *= 0.25;
}

pub(super) fn drive(race: &Race3d, r: &mut Rider, input: Input, noise: &mut Noise) {
    if r.finish.is_some() {
        return;
    }
    if r.crash > 0 {
        r.crash -= 1;
        r.spin += 0.3;
        if r.crash == 0 {
            r.wreck = Wreck::None;
            r.drift.heading = 0.0;
            r.drift.travel = 0.0;
            r.vx = 0.0;
        }
        return;
    }
    let at = track(Course::Touge).at(r.s);
    let d = &mut r.drift;
    if d.sliding && !input.dash {
        let stage = d.stage();
        if stage > 0 {
            d.boost = BOOST_TICKS[stage - 1];
            d.boost_stage = stage as u8;
            r.vs += BOOST_SPEED[stage - 1];
            noise.sounds.push("roll");
        }
        d.sliding = false;
        d.charge = 0;
        d.bank();
    }
    if input.dash && input.move_x != 0 && r.vs >= MIN_SPEED && !d.sliding {
        d.sliding = true;
    }
    let grip = if d.sliding { SLIDE_GRIP } else { DRIFT_GRIP };
    // Steering changes the chassis first; the velocity follows with grip.
    // Road heading changes beneath both, so inertia carries a slide wide.
    let turn = f32::from(input.move_x) * DRIFT_STEER * (r.vs / MIN_SPEED).min(1.0);
    d.heading = (d.heading + turn * DT).clamp(-1.5, 1.5);
    d.travel += (d.heading - d.travel) * (grip * DT).min(1.0);
    let forward = d.travel.cos().max(0.2);
    let road_turn = r.vs * forward * at.bend * DT;
    d.heading -= road_turn;
    d.travel -= road_turn;
    let angle = d.angle().abs();
    if d.sliding && angle > SPIN_ANGLE {
        spin_out(r, noise);
        return;
    }
    if d.sliding && angle >= MIN_ANGLE && r.vs >= MIN_SPEED {
        d.charge = (d.charge + 1).min(CHARGE_TICKS[2]);
        d.corner = d.corner.saturating_add(angle.to_degrees().round() as u32);
    }
    if at.bend.abs() < 0.001 && angle < MIN_ANGLE {
        d.bank();
    }
    let bonus = if d.boost > 0 {
        BOOST_SPEED[d.boost_stage.saturating_sub(1) as usize]
    } else {
        0.0
    };
    d.boost = d.boost.saturating_sub(1);
    let a = if input.move_y > 0 || input.interact {
        -DRIFT_BRAKE
    } else if input.move_y < 0 {
        DRIFT_ACCEL
    } else {
        -1.4
    };
    r.vs = (r.vs + a * DT).clamp(0.0, DRIFT_TOP + bonus);
    r.vx = r.vs * d.travel.sin();
    r.x += r.vx * DT;
    r.s += r.vs * forward * DT;
    r.spin += r.vs * DT;
    let edge = at.width * 0.5 - 0.65;
    if r.x.abs() > edge {
        r.x = r.x.clamp(-edge, edge);
        if d.sliding {
            spin_out(r, noise);
        } else {
            r.vs *= 0.8;
            r.drift.heading = 0.0;
            r.drift.travel = 0.0;
            r.vx = 0.0;
        }
    }
    if r.s >= race.finish_distance() {
        r.drift.bank();
    }
}

/// The same inputs a seat uses: look ahead for curvature, feed steering
/// into the bend and correct lateral error, countersteer before a wall.
/// The controller releases a charged slide before its angle runs away.
pub fn drift_bot(race: &Race3d, id: u32) -> Input {
    let Some(r) = race.riders.get(&id) else {
        return Input::default();
    };
    let t = track(Course::Touge);
    let bend = t.at(r.s + r.vs * 0.25).bend;
    let lane = (id as f32 - 2.5) * 1.6;
    let target = (-(r.x - lane) * 0.12 - r.drift.travel * 0.7).clamp(-0.65, 0.65);
    let error = target - r.drift.heading;
    let feed = r.vs * bend;
    let steering = feed + error * 3.5;
    let move_x = if steering > 0.08 {
        1
    } else if steering < -0.08 {
        -1
    } else {
        0
    };
    let safe = r.x.abs() < 3.3 && r.drift.angle().abs() < 0.8;
    let dash = safe
        && if r.drift.sliding {
            r.drift.charge < CHARGE_TICKS[2] && bend.abs() > 0.003
        } else {
            bend.abs() > 0.006 && r.vs > 10.0 && r.drift.boost == 0
        };
    Input {
        move_x,
        move_y: -1,
        dash,
        ..Input::default()
    }
}

#[cfg(test)]
#[path = "../../../tests/rules/race3d_drift.rs"]
mod tests;
