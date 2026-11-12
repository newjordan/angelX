//! The race camera: a low, close chase that sits just behind and above the engines, the
//! grid fly-in of the countdown, and the slow pull-away after the flag.
//!
//! The chase is built to be felt. The lens drops to the road and closes on the pod as the
//! engines run up; its field widens hard with speed and is kicked wider by a boost, easing
//! back; it lags a steer and a bend, so the pod swings in the frame and the lens catches
//! up; it rolls into a bend; a landing dips it and it bounces; a high speed rumbles it; a
//! wall, a hazard and a near miss shake it. Everything is a function of the simulation
//! (the race and its ticks), never of the wall clock: `fx` keeps the springs between ticks
//! and starts them over from the car whenever the clock goes back or the car jumps.
use super::*;
use super::fx::{self, Mood};

/// The frame the camera shoots into.
pub const W: f32 = 384.;
pub const H: f32 = 224.;

/// Ticks of countdown before the lights go green (the sim's own number).
pub const GRID_TICKS: u32 = super::state::GRID_TICKS;

/// A camera number. In tests `ANGEL_CAM_<NAME>` overrides it, so a shot can be
/// framed without a rebuild; the game always runs the default.
#[cfg(test)]
thread_local! { static TUNED: std::cell::RefCell<std::collections::HashMap<String, Option<f32>>> = Default::default(); }
/// Read once and kept: a frame asks for dozens of numbers, and the environment is slow to search.
#[cfg(test)]
pub fn tune(name: &str, default: f32) -> f32 {
    TUNED.with(|m| *m.borrow_mut().entry(name.to_string()).or_insert_with(|| std::env::var(format!("ANGEL_CAM_{name}")).ok().and_then(|v| v.parse().ok())))
        .unwrap_or(default)
}
/// Forget the numbers read so far, so the next read looks at the environment again.
#[cfg(test)]
pub fn tune_reset() { TUNED.with(|m| m.borrow_mut().clear()); }
#[cfg(not(test))]
#[inline]
pub fn tune(_name: &str, default: f32) -> f32 { default }

fn smooth(t: f32) -> f32 { let t = t.clamp(0., 1.); t * t * (3. - 2. * t) }

/// How fast the car is, 0 at rest to 1 at a boost's top speed.
pub fn speed01(car: &rules::Car) -> f32 { (car.v / 58.).clamp(0., 1.) }

/// Distance behind the engines and height over the road at rest: just behind the chariot's
/// tail and a hand over the engines' roof (a half metre higher than it was: the pod sits lower in the
/// picture, and the road and the pod ahead show over it).
pub const BACK: f32 = 15.;
pub const UP: f32 = 4.4;
/// How much of the pod's lane the lens takes: it rides nearer the road's middle than the pod does.
pub const FOLLOW: f32 = 0.65;

/// The lens' view and its roll about its own axis (radians, positive clockwise).
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub view: View3,
    pub roll: f32,
}

/// The pure chase rig behind a car, and what the springs add to it (`Mood::default()` is
/// nothing). At rest it sits `BACK` behind the engines and `UP` over the road on the road's
/// own line; running, it drops and closes on the pod, widens its field and leans.
pub fn rig(car: &rules::Car, m: &Mood) -> Shot {
    let speed = speed01(car);
    let run = speed.powf(1.3);
    // The engines at full cry tuck the lens in behind them and a hand lower.
    let back = tune("BACK", BACK) - speed * tune("BACKV", 1.2) + m.pull;
    let up = tune("UP", UP) - speed * tune("UPV", 0.55) - m.dip;
    // Over a jump the lens rises with the pod and keeps it in frame.
    let lift = car.lift.clamp(-6., 40.);
    // The lens rides the road a little behind the pod's own lane, and lags it.
    let lane = (car.y + m.lag_y) * tune("FOLLOW", FOLLOW);
    let base = point(car.d - back, lane, up + lift * 0.6);
    // Aim along the road ahead, but level with the pod: a climb or a drop in the road ahead
    // tilts the lens by only a part, so the pod keeps its place in the frame on a hill.
    let reach = tune("LEAD", 12.) + speed * 16.;
    let here = point(car.d, car.y * 0.2, 0.);
    let ahead = point(car.d + reach, car.y * 0.2, 0.);
    let aim = v3(ahead.x, ahead.y, here.z + (ahead.z - here.z) * 0.4 + tune("AIMZ", 2.2) + lift * 0.85);
    let d = aim - base;
    let heading = d.y.atan2(d.x) + m.heading;
    let (sh, ch) = heading.sin_cos();
    // The shake in the lens' own right, up and forward, so it reads on the picture, not the compass.
    let eye = base + v3(-sh, ch, 0.) * m.shake.x + v3(0., 0., 1.) * m.shake.y + v3(ch, sh, 0.) * m.shake.z;
    let fov = tune("FOV0", 1.02) + (tune("FOV1", 1.52) - tune("FOV0", 1.02)) * run + m.fov;
    Shot {
        view: View3 { pos: eye, heading_rad: heading, pitch: d.z.atan2(d.x.hypot(d.y)) + m.pitch, fov_rad: fov },
        roll: m.roll,
    }
}

/// The lens behind a car with no memory of the ticks before it.
pub fn chase(car: &rules::Car, _ticks: u32) -> View3 { rig(car, &Mood::default()).view }

/// A high rear three-quarter shot over the whole grid: all six pods and the
/// gate beyond them.
fn grid_view(race: &rules::Session) -> View3 {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for c in race.cars.values() { lo = lo.min(c.d); hi = hi.max(c.d); }
    if lo > hi { lo = -42.; hi = -6.; }
    let mid = (lo + hi) * 0.5;
    let eye = point(lo - tune("GX", 30.), tune("GY", -2.), tune("GZ", 36.));
    let aim = point(mid + tune("GAIMX", -8.), 0., tune("GAIMZ", 0.0));
    let d = aim - eye;
    View3 { pos: eye, heading_rad: d.y.atan2(d.x), pitch: d.z.atan2(d.x.hypot(d.y)), fov_rad: tune("GFOV", 1.05) }
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let d = (b - a + PI).rem_euclid(TAU) - PI;
    a + d * t
}

fn mix(a: &View3, b: &View3, t: f32) -> View3 {
    View3 {
        pos: a.pos.lerp(b.pos, t),
        heading_rad: lerp_angle(a.heading_rad, b.heading_rad, t),
        pitch: a.pitch + (b.pitch - a.pitch) * t,
        fov_rad: a.fov_rad + (b.fov_rad - a.fov_rad) * t,
    }
}

/// What a seat's lens does at this tick: the grid shot swooping into the chase through the
/// countdown, the chase through the race, a slow orbit after the flag. `seat` is the seat
/// the camera follows; its springs live in `fx`.
pub fn shot(race: &rules::Session, car: &rules::Car, seat: u32) -> Shot {
    let mood = fx::mood(race, seat, car);
    let chased = rig(car, &mood);
    if race.ticks < GRID_TICKS + 24 && !race.finished {
        // Hold the overview for the first count, then swoop in so the lens
        // is behind the pod by the time the lights go out.
        let t = smooth((race.ticks as f32 - 26.) / 76.);
        return Shot { view: mix(&grid_view(race), &chased.view, t), roll: chased.roll * t };
    }
    if let Some(time) = car.finish {
        let since = (race.ticks as f32 - GRID_TICKS as f32 - time * 30.).max(0.);
        let swing = smooth(since / 90.);
        // Out and round to a three-quarter shot of the winner's pod.
        let side = point(car.d - 14., car.y * 0.45 - 17., 8.5);
        let aim = point(car.d - 4., car.y, 1.2);
        let d = aim - side;
        let orbit = View3 { pos: side, heading_rad: d.y.atan2(d.x), pitch: d.z.atan2(d.x.hypot(d.y)), fov_rad: 1.0 };
        return Shot { view: mix(&chased.view, &orbit, swing), roll: chased.roll * (1. - swing) };
    }
    chased
}

/// The view a seat sees at this tick; the seat is the one whose car this is. A car that is
/// no seat of the race gets the lens without springs.
pub fn view(race: &rules::Session, car: &rules::Car) -> View3 {
    match race.cars.iter().find(|(_, c)| std::ptr::eq(*c, car)) {
        Some((&seat, _)) => shot(race, car, seat).view,
        None => chase(car, race.ticks),
    }
}

/// A lens ready to project points: the camera's basis and field, built once.
#[derive(Clone, Copy)]
pub struct Lens {
    pos: V3,
    right: V3,
    up: V3,
    fwd: V3,
    tan_h: f32,
    tan_v: f32,
}

impl Lens {
    /// The raster's own lens for `view`, rolled as the raster is on this thread.
    pub fn new(view: &View3) -> Lens {
        let (sh, ch) = view.heading_rad.sin_cos();
        let pitch = view.pitch.clamp(-1.35, 1.35);
        let (sp, cp) = pitch.sin_cos();
        let right = v3(-sh, ch, 0.0);
        let fwd = v3(ch * cp, sh * cp, sp);
        let up = fwd.cross(right).normalize();
        let (right, up) = raster::rolled(right, up, raster::current_roll());
        let tan_h = (view.fov_rad.clamp(0.20, 2.40) * 0.5).tan();
        Lens { pos: view.pos, right, up, fwd, tan_h, tan_v: tan_h * (H / W) }
    }

    /// Where a world point lands in the frame: (x, y, depth), or none if it is behind the lens.
    pub fn project(&self, p: V3) -> Option<(f32, f32, f32)> {
        let d = p - self.pos;
        let (x, y, z) = (d.dot(self.right), d.dot(self.up), d.dot(self.fwd));
        if z < 0.5 { return None; }
        Some((W * 0.5 * (1.0 + x / z / self.tan_h), H * 0.5 * (1.0 - y / z / self.tan_v), z))
    }

    /// Dots across one metre at depth `z`.
    pub fn scale(&self, z: f32) -> f32 { W * 0.5 / self.tan_h / z.max(0.5) }

    pub fn right(&self) -> V3 { self.right }
    pub fn up(&self) -> V3 { self.up }
    pub fn fwd(&self) -> V3 { self.fwd }
    pub fn pos(&self) -> V3 { self.pos }
}

/// Where a world point lands in the frame: (x, y, depth), or none if it is
/// behind the lens. Same basis and field as the raster's own camera.
pub fn project(view: &View3, p: V3) -> Option<(f32, f32, f32)> { Lens::new(view).project(p) }
