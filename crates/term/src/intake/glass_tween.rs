//! Tweens: a property moved from where it is to where the program wants
//! it, interpolated by glass every frame, so the motion is smooth while
//! the program says nothing. The dialect's `tween` op builds them
//! (docs/GLASS_DIALECT.md); this file holds what does not care what is
//! moved: the easings, the timing (delay, repeat, yoyo, `after=`
//! chains), the values and how they mix, and the engine that runs them
//! against a clock. What a target is, and how its property is read and
//! written, is the caller's ([`Targets`]).
//!
//! The clock is the store's, in milliseconds. A tween arrives unarmed and
//! takes the clock of the first tick after it as its start, so a reader
//! thread's late view of the clock never makes it jump. It starts from
//! the property's value at the moment its run begins, so nothing jumps
//! when one tween replaces another or a chained one follows.

/// The most tweens kept at once, across every target.
pub const TWEEN_CAP: usize = 4_096;
/// The longest `dur=` or `delay=`, in seconds.
pub const TIME_MAX: f64 = 3_600.0;
/// The most repeats a counted `repeat=` may ask for.
pub const REPEAT_MAX: u32 = 1_000_000;
/// The longest a spring may take to settle, in seconds; past it the
/// tween ends on its target.
pub const SPRING_SETTLE_MAX: f64 = 60.0;
/// How close to its target a spring counts as settled.
pub const SPRING_EPSILON: f64 = 1e-3;

/// How a tween's time is shaped into its progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    /// One of the named curves, in, out or in-out.
    Curve(Curve, Side),
    /// CSS's `cubic-bezier(x1, y1, x2, y2)`.
    Bezier([f64; 4]),
    /// A damped spring's step response: stiffness, damping, mass. It sets
    /// its own duration, the time it takes to settle.
    Spring { stiffness: f64, damping: f64, mass: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    Quad,
    Cubic,
    Quart,
    Quint,
    Sine,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    In,
    Out,
    InOut,
}

impl Ease {
    /// `linear`, `<curve>-in|out|in-out`, `cubic-bezier(x1,y1,x2,y2)` or
    /// `spring(stiffness,damping[,mass])`.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text == "linear" {
            return Ok(Ease::Linear);
        }
        if let Some(args) = call(text, "cubic-bezier") {
            let n = numbers(args, 4, 4).ok_or("cubic-bezier takes four numbers")?;
            if !(0.0..=1.0).contains(&n[0]) || !(0.0..=1.0).contains(&n[2]) {
                return Err("cubic-bezier's x1 and x2 must be in 0..1".into());
            }
            if n[1].abs() > 100.0 || n[3].abs() > 100.0 {
                return Err("cubic-bezier's y1 and y2 must be in -100..100".into());
            }
            return Ok(Ease::Bezier([n[0], n[1], n[2], n[3]]));
        }
        if let Some(args) = call(text, "spring") {
            let n = numbers(args, 2, 3).ok_or("spring takes stiffness, damping and an optional mass")?;
            let mass = n.get(2).copied().unwrap_or(1.0);
            if !(n[0] > 0.0 && n[0] <= 10_000.0) {
                return Err("spring stiffness must be in (0, 10000]".into());
            }
            if !(n[1] > 0.0 && n[1] <= 1_000.0) {
                return Err("spring damping must be in (0, 1000]".into());
            }
            if !(mass > 0.0 && mass <= 1_000.0) {
                return Err("spring mass must be in (0, 1000]".into());
            }
            return Ok(Ease::Spring { stiffness: n[0], damping: n[1], mass });
        }
        let (name, side) = if let Some(name) = text.strip_suffix("-in-out") {
            (name, Side::InOut)
        } else if let Some(name) = text.strip_suffix("-out") {
            (name, Side::Out)
        } else if let Some(name) = text.strip_suffix("-in") {
            (name, Side::In)
        } else {
            return Err(format!("ease={text} is not an easing"));
        };
        let curve = match name {
            "quad" => Curve::Quad,
            "cubic" => Curve::Cubic,
            "quart" => Curve::Quart,
            "quint" => Curve::Quint,
            "sine" => Curve::Sine,
            "expo" => Curve::Expo,
            "circ" => Curve::Circ,
            "back" => Curve::Back,
            "elastic" => Curve::Elastic,
            "bounce" => Curve::Bounce,
            _ => return Err(format!("ease={text} is not an easing")),
        };
        Ok(Ease::Curve(curve, side))
    }

    /// The progress at `u`, the run's share of its time (0..=1). Back,
    /// elastic, a bezier with a y outside 0..1 and a spring overshoot.
    pub fn at(&self, u: f64) -> f64 {
        let u = if u.is_finite() { u.clamp(0.0, 1.0) } else { 1.0 };
        match *self {
            Ease::Linear => u,
            Ease::Curve(curve, Side::In) => curve_in(curve, u),
            Ease::Curve(curve, Side::Out) => 1.0 - curve_in(curve, 1.0 - u),
            Ease::Curve(Curve::Back, Side::InOut) => back_in_out(u),
            Ease::Curve(Curve::Elastic, Side::InOut) => elastic_in_out(u),
            Ease::Curve(curve, Side::InOut) => {
                if u < 0.5 {
                    curve_in(curve, 2.0 * u) / 2.0
                } else {
                    1.0 - curve_in(curve, 2.0 - 2.0 * u) / 2.0
                }
            }
            Ease::Bezier([x1, y1, x2, y2]) => bezier(x1, y1, x2, y2, u),
            Ease::Spring { stiffness, damping, mass } => {
                if u >= 1.0 {
                    return 1.0;
                }
                let settle = spring_settle(stiffness, damping, mass);
                spring(stiffness, damping, mass, u * settle)
            }
        }
    }

    /// A spring's own duration in milliseconds; `None` for the others,
    /// which take `dur=`.
    pub fn own_duration_ms(&self) -> Option<f64> {
        match *self {
            Ease::Spring { stiffness, damping, mass } => Some(spring_settle(stiffness, damping, mass) * 1000.0),
            _ => None,
        }
    }
}

/// `name(a,b,...)` to its arguments.
fn call<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')')
}

fn numbers(args: &str, min: usize, max: usize) -> Option<Vec<f64>> {
    let mut out = Vec::with_capacity(max);
    for part in args.split(',') {
        if out.len() == max {
            return None;
        }
        out.push(part.trim().parse::<f64>().ok().filter(|n| n.is_finite())?);
    }
    (out.len() >= min).then_some(out)
}

/// Each named curve's in form; out and in-out are made from it. These
/// are the usual (Penner) curves: back overshoots by its 1.70158, and the
/// bounce and elastic are the common ones.
fn curve_in(curve: Curve, u: f64) -> f64 {
    use std::f64::consts::PI;
    match curve {
        Curve::Quad => u * u,
        Curve::Cubic => u * u * u,
        Curve::Quart => u * u * u * u,
        Curve::Quint => u * u * u * u * u,
        Curve::Sine => 1.0 - (u * PI / 2.0).cos(),
        Curve::Expo => {
            if u <= 0.0 {
                0.0
            } else {
                2f64.powf(10.0 * u - 10.0)
            }
        }
        Curve::Circ => 1.0 - (1.0 - u * u).max(0.0).sqrt(),
        Curve::Back => {
            const C1: f64 = 1.70158;
            (C1 + 1.0) * u * u * u - C1 * u * u
        }
        Curve::Elastic => {
            if u <= 0.0 {
                0.0
            } else if u >= 1.0 {
                1.0
            } else {
                let c4 = 2.0 * PI / 3.0;
                -(2f64.powf(10.0 * u - 10.0)) * ((u * 10.0 - 10.75) * c4).sin()
            }
        }
        Curve::Bounce => 1.0 - bounce_out(1.0 - u),
    }
}

/// Back's in-out overshoots further each way (its 1.70158 times 1.525),
/// as the usual curve does.
fn back_in_out(u: f64) -> f64 {
    const C2: f64 = 1.70158 * 1.525;
    if u < 0.5 {
        (2.0 * u).powi(2) * ((C2 + 1.0) * 2.0 * u - C2) / 2.0
    } else {
        ((2.0 * u - 2.0).powi(2) * ((C2 + 1.0) * (2.0 * u - 2.0) + C2) + 2.0) / 2.0
    }
}

/// Elastic's in-out rings on a period of its own (4.5), as the usual
/// curve does.
fn elastic_in_out(u: f64) -> f64 {
    use std::f64::consts::PI;
    if u <= 0.0 {
        return 0.0;
    }
    if u >= 1.0 {
        return 1.0;
    }
    let c5 = 2.0 * PI / 4.5;
    if u < 0.5 {
        -(2f64.powf(20.0 * u - 10.0) * ((20.0 * u - 11.125) * c5).sin()) / 2.0
    } else {
        2f64.powf(-20.0 * u + 10.0) * ((20.0 * u - 11.125) * c5).sin() / 2.0 + 1.0
    }
}

fn bounce_out(u: f64) -> f64 {
    const N1: f64 = 7.5625;
    const D1: f64 = 2.75;
    if u < 1.0 / D1 {
        N1 * u * u
    } else if u < 2.0 / D1 {
        let u = u - 1.5 / D1;
        N1 * u * u + 0.75
    } else if u < 2.5 / D1 {
        let u = u - 2.25 / D1;
        N1 * u * u + 0.9375
    } else {
        let u = u - 2.625 / D1;
        N1 * u * u + 0.984375
    }
}

/// CSS's cubic bezier from (0, 0) to (1, 1) through two control points:
/// `y` where the curve's `x` is `x`. Newton's method from `x` itself,
/// then bisection if it strays; x1 and x2 in 0..1 make `x(t)` monotone.
fn bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;
    let sample_x = |t: f64| ((ax * t + bx) * t + cx) * t;
    let slope_x = |t: f64| (3.0 * ax * t + 2.0 * bx) * t + cx;
    let mut t = x;
    let mut solved = false;
    for _ in 0..8 {
        let err = sample_x(t) - x;
        if err.abs() < 1e-12 {
            solved = true;
            break;
        }
        let d = slope_x(t);
        if d.abs() < 1e-9 {
            break;
        }
        t -= err / d;
    }
    if !solved || !(0.0..=1.0).contains(&t) {
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        t = x;
        for _ in 0..64 {
            let v = sample_x(t);
            if (v - x).abs() < 1e-12 {
                break;
            }
            if v < x {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) / 2.0;
        }
    }
    ((ay * t + by) * t + cy) * t
}

/// The damped spring `m x'' = -k (x - 1) - c x'`, from rest at 0, at `t`
/// seconds: under-, critically or over-damped, in closed form.
pub fn spring(stiffness: f64, damping: f64, mass: f64, t: f64) -> f64 {
    let w0 = (stiffness / mass).sqrt();
    let zeta = damping / (2.0 * (stiffness * mass).sqrt());
    if (zeta - 1.0).abs() < 1e-9 {
        1.0 - (-w0 * t).exp() * (1.0 + w0 * t)
    } else if zeta < 1.0 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        1.0 - (-zeta * w0 * t).exp() * ((wd * t).cos() + zeta * w0 / wd * (wd * t).sin())
    } else {
        let s = (zeta * zeta - 1.0).sqrt();
        let r1 = -w0 * (zeta - s);
        let r2 = -w0 * (zeta + s);
        1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
    }
}

/// How far from its target the spring can be at `t` at most, from then on:
/// the decaying envelope of [`spring`], falling with `t` in every case.
fn spring_envelope(stiffness: f64, damping: f64, mass: f64, t: f64) -> f64 {
    let w0 = (stiffness / mass).sqrt();
    let zeta = damping / (2.0 * (stiffness * mass).sqrt());
    if (zeta - 1.0).abs() < 1e-9 {
        (-w0 * t).exp() * (1.0 + w0 * t)
    } else if zeta < 1.0 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        (-zeta * w0 * t).exp() * (1.0 + (zeta * w0 / wd).powi(2)).sqrt()
    } else {
        let s = (zeta * zeta - 1.0).sqrt();
        let r1 = -w0 * (zeta - s);
        let r2 = -w0 * (zeta + s);
        (r2.abs() * (r1 * t).exp() + r1.abs() * (r2 * t).exp()) / (r1 - r2).abs()
    }
}

/// When the spring has settled within [`SPRING_EPSILON`] for good, in
/// seconds, at most [`SPRING_SETTLE_MAX`].
pub fn spring_settle(stiffness: f64, damping: f64, mass: f64) -> f64 {
    if spring_envelope(stiffness, damping, mass, SPRING_SETTLE_MAX) >= SPRING_EPSILON {
        return SPRING_SETTLE_MAX;
    }
    let (mut lo, mut hi) = (0.0f64, SPRING_SETTLE_MAX);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if spring_envelope(stiffness, damping, mass, mid) < SPRING_EPSILON {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// How many times a tween runs past its first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Count(u32),
    Forever,
}

/// When a tween runs and how often.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub delay_ms: u64,
    /// One run, in milliseconds.
    pub run_ms: f64,
    pub repeat: Repeat,
    /// Every second run goes back the way it came.
    pub yoyo: bool,
}

impl Timing {
    /// Every run together; `None` for ever.
    pub fn total_ms(&self) -> Option<f64> {
        match self.repeat {
            Repeat::Forever => None,
            Repeat::Count(n) => Some(self.run_ms * (f64::from(n) + 1.0)),
        }
    }

    /// Where a run that began `elapsed` milliseconds ago stands: the share
    /// of the run to ease (time runs back on a yoyo's return), and whether
    /// every run is over.
    pub fn at(&self, elapsed: f64) -> (f64, bool) {
        let elapsed = elapsed.max(0.0);
        if let Some(total) = self.total_ms()
            && elapsed >= total
        {
            return (self.end_share(), true);
        }
        if self.run_ms <= 0.0 {
            return (self.end_share(), true);
        }
        let run = (elapsed / self.run_ms).floor();
        let u = (elapsed - run * self.run_ms) / self.run_ms;
        let back = self.yoyo && (run as u64) % 2 == 1;
        (if back { 1.0 - u } else { u }, false)
    }

    /// The share the last run ends at: 1, or 0 when a yoyo's runs are even
    /// in number and the last went back.
    fn end_share(&self) -> f64 {
        match self.repeat {
            Repeat::Count(n) if self.yoyo && n % 2 == 1 => 0.0,
            _ => 1.0,
        }
    }
}

/// A property's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    F(f32),
    V3([f32; 3]),
    V4([f32; 4]),
    /// A rotation, `[x, y, z, w]`, mixed along the shorter arc.
    Q([f32; 4]),
    /// Taken at the end of the run.
    B(bool),
}

impl Value {
    /// `self` toward `to` by `p` (past 0..1 for an easing that overshoots).
    pub fn mix(self, to: Value, p: f64) -> Value {
        let p = p as f32;
        let lerp = |a: f32, b: f32| a + (b - a) * p;
        match (self, to) {
            (Value::F(a), Value::F(b)) => Value::F(lerp(a, b)),
            (Value::V3(a), Value::V3(b)) => Value::V3([lerp(a[0], b[0]), lerp(a[1], b[1]), lerp(a[2], b[2])]),
            (Value::V4(a), Value::V4(b)) => {
                Value::V4([lerp(a[0], b[0]), lerp(a[1], b[1]), lerp(a[2], b[2]), lerp(a[3], b[3])])
            }
            (Value::Q(a), Value::Q(b)) => Value::Q(super::glass_scene::slerp(a, b, p)),
            (Value::B(a), Value::B(b)) => Value::B(if p >= 1.0 { b } else { a }),
            (_, to) => to,
        }
    }
}

/// What a tween moves. Layers and scenes share the program's ids, so a
/// `Layer` is a layer, or a scene's placement when no layer has the id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Layer(u32),
    Node { scene: u32, node: u64 },
    Camera(u32),
}

impl Target {
    /// The scene this target lives in, for a scene's own targets.
    pub fn scene(&self) -> Option<u32> {
        match *self {
            Target::Node { scene, .. } | Target::Camera(scene) => Some(scene),
            Target::Layer(_) => None,
        }
    }
}

/// One property of a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    /// A layer's `at=` (cells) or `px=` (pixels) rectangle.
    Rect,
    Opacity,
    Angle,
    Color,
    /// A particle emitter's mote size.
    Size,
    Breathe,
    Lift,
    NodeAt,
    NodeRot,
    NodeScale,
    NodeColor,
    NodeEmissive,
    NodeVisible,
    Eye,
    LookAt,
    Fov,
}

impl Prop {
    /// A property written only once its run is over.
    fn at_end(self) -> bool {
        matches!(self, Prop::NodeVisible)
    }
}

/// Where the caller keeps what tweens move.
pub trait Targets {
    /// The property's value now; `None` once the target has gone.
    fn read(&mut self, target: Target, prop: Prop) -> Option<Value>;
    /// Write the property; `false` once the target has gone.
    fn write(&mut self, target: Target, prop: Prop, value: Value) -> bool;
}

/// One property a tween moves: to `to`, from where it stood when the run
/// began.
#[derive(Clone, Copy, Debug)]
pub struct Track {
    pub prop: Prop,
    pub to: Value,
    from: Option<Value>,
}

impl Track {
    pub fn new(prop: Prop, to: Value) -> Self {
        Self { prop, to, from: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Not yet seen by a tick: the next tick is its start.
    New,
    /// Waiting for the tween `after=` names to end.
    Waiting,
    /// Its run begins at this clock.
    Delay(u64),
    /// Its run began at this clock.
    Running(u64),
}

/// One tween: some properties of one target, eased together.
#[derive(Clone, Debug)]
pub struct Tween {
    pub tid: Option<u32>,
    pub target: Target,
    pub tracks: Vec<Track>,
    pub ease: Ease,
    pub timing: Timing,
    pub after: Option<u32>,
    phase: Phase,
}

impl Tween {
    pub fn new(target: Target, tracks: Vec<Track>, ease: Ease, timing: Timing) -> Self {
        Self { tid: None, target, tracks, ease, timing, after: None, phase: Phase::New }
    }

    /// When its every run is over, once it has begun.
    fn end(&self) -> Option<f64> {
        match self.phase {
            Phase::Running(start) => self.timing.total_ms().map(|t| start as f64 + t),
            _ => None,
        }
    }
}

/// Every tween glass is running, in arrival order.
#[derive(Clone, Debug, Default)]
pub struct Tweens {
    list: Vec<Tween>,
}

impl Tweens {
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Add a tween. One with a `tid` already running replaces that one.
    /// One that waits on no other replaces, on every property it moves,
    /// whatever already moves that property of that target, at once; one
    /// with `after=` does so when its run begins. Past [`TWEEN_CAP`] the
    /// tween is refused.
    pub fn add(&mut self, tween: Tween) -> Result<(), String> {
        if let Some(tid) = tween.tid {
            self.list.retain(|t| t.tid != Some(tid));
        }
        if tween.after.is_none() {
            for track in &tween.tracks {
                self.cancel(tween.target, track.prop, None);
            }
            self.list.retain(|t| !t.tracks.is_empty());
        }
        if self.list.len() >= TWEEN_CAP {
            return Err(format!("tweens are capped at {TWEEN_CAP}"));
        }
        self.list.push(tween);
        Ok(())
    }

    /// Stop every tween on `target`; what they moved stays where it is.
    pub fn stop_target(&mut self, target: Target) {
        self.list.retain(|t| t.target != target);
    }

    /// Stop every tween on a scene's nodes and camera.
    pub fn stop_scene(&mut self, scene: u32) {
        self.list.retain(|t| t.target.scene() != Some(scene));
    }

    /// Stop the tween named `tid`.
    pub fn stop_tid(&mut self, tid: u32) {
        self.list.retain(|t| t.tid != Some(tid));
    }

    /// Take `prop` of `target` away from every tween (but `keep`), only
    /// from running ones when `running_only`; a tween left moving nothing
    /// goes.
    fn cancel(&mut self, target: Target, prop: Prop, keep: Option<usize>) {
        self.cancel_where(target, prop, keep, false);
    }

    fn cancel_where(&mut self, target: Target, prop: Prop, keep: Option<usize>, running_only: bool) {
        for (i, t) in self.list.iter_mut().enumerate() {
            if Some(i) == keep || t.target != target {
                continue;
            }
            if running_only && !matches!(t.phase, Phase::Running(_)) {
                continue;
            }
            t.tracks.retain(|track| track.prop != prop);
        }
    }

    /// Advance every tween to `now` and write what moved. Returns whether
    /// anything was written. Chains are followed within the tick: a tween
    /// that ended at `e` starts the one after it at `e` plus its delay,
    /// however far the clock jumped.
    pub fn tick(&mut self, now: u64, targets: &mut dyn Targets) -> bool {
        let mut changed = false;
        let mut ended: Vec<(u32, u64)> = Vec::new();
        let mut rounds = 0;
        loop {
            rounds += 1;
            let mut progressed = false;
            let mut i = 0;
            while i < self.list.len() {
                let step = self.step(i, now, &ended, targets);
                match step {
                    Step::Same => i += 1,
                    Step::Moved => {
                        progressed = true;
                        i += 1;
                    }
                    Step::Wrote => {
                        changed = true;
                        i += 1;
                    }
                    Step::Done(at) => {
                        changed = true;
                        progressed = true;
                        if let (Some(tid), Some(at)) = (self.list[i].tid, at) {
                            ended.push((tid, at));
                        }
                        self.list.remove(i);
                    }
                    Step::Gone => {
                        progressed = true;
                        self.list.remove(i);
                    }
                }
            }
            self.list.retain(|t| !t.tracks.is_empty());
            if !progressed || rounds > self.list.len() + 1 {
                break;
            }
        }
        changed
    }

    fn step(&mut self, i: usize, now: u64, ended: &[(u32, u64)], targets: &mut dyn Targets) -> Step {
        if self.list[i].tracks.is_empty() {
            return Step::Gone;
        }
        let delay = self.list[i].timing.delay_ms;
        match self.list[i].phase {
            Phase::New => {
                let after = self.list[i].after;
                let waiting = after.is_some_and(|a| self.list.iter().enumerate().any(|(j, t)| j != i && t.tid == Some(a)));
                self.list[i].phase = if waiting {
                    Phase::Waiting
                } else {
                    let base = after.and_then(|a| ended.iter().find(|(tid, _)| *tid == a).map(|(_, at)| *at));
                    Phase::Delay(base.unwrap_or(now).saturating_add(delay))
                };
                Step::Moved
            }
            Phase::Waiting => {
                let after = self.list[i].after;
                let still = after.is_some_and(|a| self.list.iter().enumerate().any(|(j, t)| j != i && t.tid == Some(a)));
                if still {
                    return Step::Same;
                }
                let base = after.and_then(|a| ended.iter().find(|(tid, _)| *tid == a).map(|(_, at)| *at));
                self.list[i].phase = Phase::Delay(base.unwrap_or(now).saturating_add(delay));
                Step::Moved
            }
            Phase::Delay(start) => {
                if now < start {
                    return Step::Same;
                }
                // The run begins: it takes over its properties from any
                // other running tween, and starts from where they stand.
                let target = self.list[i].target;
                let props: Vec<Prop> = self.list[i].tracks.iter().map(|t| t.prop).collect();
                for prop in props {
                    self.cancel_where(target, prop, Some(i), true);
                }
                let tween = &mut self.list[i];
                for track in &mut tween.tracks {
                    match targets.read(target, track.prop) {
                        Some(value) => track.from = Some(value),
                        None => return Step::Gone,
                    }
                }
                tween.phase = Phase::Running(start);
                match self.run(i, now, targets) {
                    Step::Same => Step::Moved,
                    other => other,
                }
            }
            Phase::Running(_) => self.run(i, now, targets),
        }
    }

    fn run(&mut self, i: usize, now: u64, targets: &mut dyn Targets) -> Step {
        let tween = &self.list[i];
        let Phase::Running(start) = tween.phase else { return Step::Same };
        let elapsed = now.saturating_sub(start) as f64;
        let (share, done) = tween.timing.at(elapsed);
        let p = if done && share >= 1.0 {
            1.0
        } else if done && share <= 0.0 {
            0.0
        } else {
            tween.ease.at(share)
        };
        for track in &tween.tracks {
            if track.prop.at_end() && !done {
                continue;
            }
            let Some(from) = track.from else { continue };
            let value = if track.prop.at_end() {
                if share >= 1.0 { track.to } else { from }
            } else {
                from.mix(track.to, p)
            };
            if !targets.write(tween.target, track.prop, value) {
                return Step::Gone;
            }
        }
        if done {
            let at = tween.end().map(|e| e.round().max(0.0) as u64).or(Some(now));
            Step::Done(at)
        } else {
            Step::Wrote
        }
    }

    /// When a tick next has something to do: at once for a new tween, at
    /// the start of a delayed one, a display frame on for a running one
    /// (or its end, when that comes sooner). A tween that only waits, or
    /// whose properties change only at the end, wakes nothing in between.
    pub fn deadline(&self, clock: u64, frame_ms: u64) -> Option<u64> {
        let mut out: Option<u64> = None;
        let mut take = |at: u64| out = Some(out.map_or(at, |o| o.min(at)));
        for t in &self.list {
            match t.phase {
                Phase::New => take(clock),
                Phase::Waiting => {}
                Phase::Delay(start) => take(start.max(clock)),
                Phase::Running(start) => {
                    let end = t.timing.total_ms().map(|total| (start as f64 + total).ceil().max(0.0) as u64);
                    let only_end = t.tracks.iter().all(|track| track.prop.at_end());
                    match (end, only_end) {
                        (Some(end), true) => take(end.max(clock)),
                        (Some(end), false) => take(end.max(clock).min(clock.saturating_add(frame_ms))),
                        (None, true) => {}
                        (None, false) => take(clock.saturating_add(frame_ms)),
                    }
                }
            }
        }
        out
    }
}

enum Step {
    /// Nothing changed.
    Same,
    /// Its phase moved on, nothing was written.
    Moved,
    /// Values were written; it runs on.
    Wrote,
    /// Its last values were written; it ended at this clock.
    Done(Option<u64>),
    /// Its target went: it goes, unwritten.
    Gone,
}
