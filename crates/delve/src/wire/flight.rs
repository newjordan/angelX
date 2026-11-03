//! Shots on the wire as flights. A shot is sent when it is loosed: where it
//! is, how fast it goes, and how its course turns a tick (`Course`), with
//! the tick that was (`kf`). A client flies it on from there itself
//! (`Flown::at`). The host flies the same course by the same arithmetic
//! every tick and sends the shot's course again only when the shot has
//! left it by more than `SLIP` (or its speed by more than `SLIP_SPEED`): a
//! bounce, a homing turn that changed, a hit that turned it. A wall is not
//! one of those: the client stops a shot on the same stone the host does
//! (`Run::shot_stone`), so a shot sitting on a wall is not sent again.
//! So a shot is sent as it changes, not every tick, and where a client
//! draws it is never further than `SLIP` from where the host has it.
//!
//! What counts down or up a tick at a time is sent as the tick it ends or
//! began, so it is not sent again while it runs: its life (`ttl`), a
//! pattern shot's wait and age (`patterns::Path`), a thrown blade's way out
//! (`ShotTraits::out`). A pattern shot with no pattern is sent without one.
//!
//! A straight or turning course is flown with the arithmetic IEEE 754 gives
//! the same answer for on every machine, so a browser's client and a native
//! one agree with the host to the bit. A weaving or homing one takes sines
//! and angles, which a browser's maths library may round a last bit
//! differently: a millionth of a unit, far inside `SLIP`.

use crate::rules::{HZ, Projectile, Run, Shot};
use angel_net::doc::value::{self, Value};
use std::borrow::Cow;

/// How far a client's shot may stand from the host's before its course is
/// sent again (run units): an eighth of a pixel at the art's scale
/// (`arena::UNIT`, 8 pixels).
pub const SLIP: f32 = 1.0 / 64.0;
/// How far its speed may be off (units a second) before its course is sent
/// again: what a client draws a shot back along its speed between two ticks
/// (`Run::blend`, two ticks at most) is then off by no more than `SLIP`.
pub const SLIP_SPEED: f32 = 1.0 / 4.0;
/// A turn of more than this a tick (radians) is no course: a bounce, an
/// aim. The shot is sent flying straight from there.
const MOST_TURN: f32 = 0.25;
/// The most a turning course's speed may change a tick (a share of it).
const MOST_SPEED_CHANGE: f32 = 0.2;
/// The longest a turning course is flown before it is sent again (ticks):
/// a client flies one a tick at a time.
pub const LONGEST_TURN: u64 = 2 * HZ as u64;
/// The longest a stone-meeting line is flown a tick at a time. Past this
/// the closed form stands, so a damaged tick cannot walk the line forever.
const STONE_STEPS: u64 = 1 << 16;
/// How far from where its course has it a shot may be found and still be
/// itself: a curving shot turns, a bouncing one comes off a wall.
pub const SAME_SHOT: f32 = 0.75;
/// One tick, in seconds.
const DT: f32 = 1.0 / HZ as f32;
const TICK_S: f64 = 1.0 / HZ as f64;

/// A shot's course as sent: where it was and how fast it went on the tick
/// it was sent (`kf`), and the turn its velocity takes each tick after, as
/// the cosine and sine of the turn times the change of speed (a complex
/// number), or none: straight on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Course {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub turn: Option<(f32, f32)>,
}

/// A course as a client flies it: what the tree says of the shot beside
/// its course (`flown`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Flown {
    course: Course,
    /// Ticks it stands still waiting from the course's tick
    /// (`patterns::Path::wait`).
    still: u64,
    /// A pattern shot's weave: how far either side, how fast, and its age
    /// on the course's tick (`patterns::steer`).
    weave: Option<(f32, f32, u64)>,
    /// A knight's homing shot: the most it turns a tick toward the nearest
    /// live monster (`Run::play`), the monsters standing where the tick's
    /// tree has them.
    homing: Option<f32>,
    /// How wide a friendly shot is where stone stops it (`patterns::body`).
    /// 0: it flies through stone, as the host lets a hostile shot and a
    /// blade on its way home.
    body: f32,
}

impl Course {
    /// Flying on from where `shot` is now, turning by `turn`.
    pub fn of(shot: &Projectile, turn: Option<(f32, f32)>) -> Course {
        Course { x: shot.x, y: shot.y, vx: shot.vx, vy: shot.vy, turn }
    }
}

impl Flown {
    /// Where it is and how fast it goes `n` ticks on, as the sim moves a
    /// shot (`Run::play`, `patterns::steer`): still while it waits; then
    /// straight, as a start and a speed, or turning a tick at a time (the
    /// velocity turns, then the shot moves by it, at most `LONGEST_TURN`
    /// ticks); weaving either side of that line as its age runs on.
    fn at(
        &self,
        n: u64,
        marks: &[(f32, f32)],
        stone: &impl Fn((f32, f32), (f32, f32), f32) -> Option<(f32, f32)>,
        aim: &[(u64, Vec<(f32, f32)>)],
        kf: u64,
    ) -> (f32, f32, f32, f32) {
        let c = self.course;
        let fly = n.saturating_sub(self.still);
        if let Some(most) = self.homing {
            let (mut x, mut y, mut vx, mut vy) = (c.x, c.y, c.vx, c.vy);
            for step in 0..fly.min(LONGEST_TURN) {
                let marks = aimed(aim, kf.saturating_add(self.still).saturating_add(step).saturating_add(1), marks);
                if let Some(&(tx, ty)) =
                    marks.iter().min_by(|a, b| far(a, (x, y)).total_cmp(&far(b, (x, y))))
                {
                    let speed = vx.hypot(vy);
                    let now = vy.atan2(vx);
                    let want = (ty - y).atan2(tx - x);
                    let turn = (want - now).sin().atan2((want - now).cos());
                    let next = now + turn.clamp(-most, most);
                    (vx, vy) = (next.cos() * speed, next.sin() * speed);
                }
                let (ox, oy) = (x, y);
                x += vx * DT;
                y += vy * DT;
                if self.body != 0.0
                    && let Some((sx, sy)) = stone((ox, oy), (x, y), self.body)
                {
                    (x, y) = (sx, sy);
                }
            }
            return (x, y, vx, vy);
        }
        let side = |age: u64| match self.weave {
            Some((wave, rate, _)) => wave * (rate * age as f32).sin(),
            None => 0.0,
        };
        let born = self.weave.map_or(0, |w| w.2);
        match c.turn {
            None => {
                let since = fly as f64 * TICK_S;
                let (mut x, mut y) = (
                    (f64::from(c.x) + f64::from(c.vx) * since) as f32,
                    (f64::from(c.y) + f64::from(c.vy) * since) as f32,
                );
                if self.weave.is_some() && fly > 0 {
                    let heading = c.vy.atan2(c.vx);
                    let off = side(born + fly) - side(born);
                    x += -heading.sin() * off;
                    y += heading.cos() * off;
                }
                // A clear line is the closed form, to the bit. A line that meets
                // stone is flown a tick at a time, as the host moves and
                // stops it, velocity unchanged. A damaged tick past
                // `STONE_STEPS` keeps the closed form: walking it would not
                // finish.
                if self.body == 0.0
                    || self.weave.is_some()
                    || fly == 0
                    || fly > STONE_STEPS
                    || stone((c.x, c.y), (x, y), self.body).is_none()
                {
                    return (x, y, c.vx, c.vy);
                }
                let (mut x, mut y) = (c.x, c.y);
                for _ in 0..fly {
                    let (ox, oy) = (x, y);
                    x += c.vx * DT;
                    y += c.vy * DT;
                    if let Some((sx, sy)) = stone((ox, oy), (x, y), self.body) {
                        (x, y) = (sx, sy);
                    }
                }
                (x, y, c.vx, c.vy)
            }
            Some((tc, ts)) => {
                let (mut x, mut y, mut vx, mut vy) = (c.x, c.y, c.vx, c.vy);
                for k in 1..=fly.min(LONGEST_TURN) {
                    (vx, vy) = (vx * tc - vy * ts, vx * ts + vy * tc);
                    if self.weave.is_some() {
                        let heading = vy.atan2(vx);
                        let off = side(born + k) - side(born + k - 1);
                        x += -heading.sin() * off;
                        y += heading.cos() * off;
                    }
                    let (ox, oy) = (x, y);
                    x += vx * DT;
                    y += vy * DT;
                    if self.body != 0.0
                        && let Some((sx, sy)) = stone((ox, oy), (x, y), self.body)
                    {
                        (x, y) = (sx, sy);
                    }
                }
                (x, y, vx, vy)
            }
        }
    }
}

/// The turn a velocity took in one tick from `was` to `now`, when it is a
/// course's (`MOST_TURN`, `MOST_SPEED_CHANGE`) and is one.
fn turn_of(was: (f32, f32), now: (f32, f32)) -> Option<(f32, f32)> {
    let (ax, ay) = was;
    let (bx, by) = now;
    let d = ax * ax + ay * ay;
    if !(d > 1e-9) {
        return None;
    }
    let (c, s) = ((bx * ax + by * ay) / d, (by * ax - bx * ay) / d);
    let scale = (c * c + s * s).sqrt();
    if !(s.atan2(c).abs() <= MOST_TURN && (scale - 1.0).abs() <= MOST_SPEED_CHANGE) {
        return None;
    }
    (c != 1.0 || s != 0.0).then_some((c, s))
}

/// A shot in flight as last published.
pub(super) struct Flight {
    id: u64,
    kind: Shot,
    hostile: bool,
    /// The tick its course was sent on, the course, and how a client flies
    /// it from what was sent.
    kf: u64,
    course: Course,
    flown: Flown,
    /// Its velocity when last published (for the turn it takes next).
    seen: (f32, f32),
    /// What it was last published as, and the stamp of what that was made
    /// from (`stamp`).
    value: Value,
    stamp: Vec<u8>,
}

/// The shots in flight as last published, each known again by where its
/// course has it now.
#[derive(Default)]
pub(super) struct Flights {
    flying: Vec<Flight>,
    tick: Option<u64>,
    next: u64,
    /// The list last published.
    list: Option<Value>,
    /// Where the monsters stood on recent ticks, so a homing shot is flown
    /// toward where they were, not only where they stand now.
    aim: Vec<(u64, Vec<(f32, f32)>)>,
}

/// How far down the shots in flight the next shot of the run is looked for:
/// the run keeps its shots in the order they were loosed, and those that
/// land drop out.
const LOOK_AHEAD: usize = 48;

/// What of a shot `stamp` reads whole: all but where it is and how fast it
/// goes (its course's), and its counters (the ticks they end on).
static MASK: value::Mask = value::Mask {
    skip: &["x", "y", "vx", "vy", "ttl"],
    inside: &[("traits", value::Mask { skip: &["out"], inside: &[("path", value::Mask { skip: &["wait", "age"], inside: &[] })] })],
};

/// What a shot as sent on `tick` is made of beside its course (`shot_value`):
/// the same stamp, the same shot sent.
fn stamp(shot: &Projectile, tick: u64, out: &mut Vec<u8>) -> Result<(), value::Error> {
    out.clear();
    value::fingerprint_except(shot, out, &MASK)?;
    let path = &shot.traits.path;
    let (wait, age) = (u64::from(path.wait), u64::from(path.age));
    for n in [
        tick + u64::from(shot.ttl),
        if shot.traits.out == 0 { 0 } else { tick + u64::from(shot.traits.out) },
        if wait == 0 || age == 0 { (tick + wait).saturating_sub(age) } else { u64::MAX },
        if wait == 0 || age == 0 { 0 } else { wait << 16 | age },
    ] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    Ok(())
}

impl Flights {
    /// The run's shots as the tick's list (`shot_value`), each with an id
    /// that stays with it (so a list of shots is sent by id,
    /// `angel_net::doc::delta`'s keyed lists), and its course as last sent
    /// unless it has left it (`Flown::holds`, on what a client reads of the
    /// shot as sent now: the host checks just what a client will draw).
    /// Recent monster positions, as the tree carries them (`AIM`).
    pub(super) fn aim_sent(&self) -> Value {
        aim_value(&self.aim)
    }

    pub(super) fn publish(&mut self, run: &Run) -> Result<Value, value::Error> {
        let tick = run.tick;
        let step = match self.tick {
            Some(last) if last <= tick => tick - last,
            // Ticks went back: another run, or this one again from a save.
            _ => {
                self.flying.clear();
                self.aim.clear();
                0
            }
        };
        self.tick = Some(tick);
        let marks = marks_of_run(run);
        self.aim.retain(|(t, _)| tick.saturating_sub(*t) <= LONGEST_TURN);
        self.aim.push((tick, marks.clone()));
        let stone = |from, to, body| run.shot_stone(from, to, body);
        let aim = self.aim.clone();
        let mut was = std::mem::take(&mut self.flying);
        // Where each was last tick's course has it now, stone included,
        // homing toward the monsters as they stood on each of those ticks.
        let now: Vec<(f32, f32, f32, f32)> =
            was.iter().map(|f| f.flown.at(tick.saturating_sub(f.kf), &marks, &stone, &aim, f.kf)).collect();
        let mut taken = vec![false; was.len()];
        let mut at = 0;
        let mut items = Vec::with_capacity(run.projectiles.len());
        let last = match self.list.as_ref().map(Value::get) {
            Some(Value::Seq(list)) if list.len() == run.projectiles.len() => Some(list),
            _ => None,
        };
        let mut same = last.is_some();
        let mut print = Vec::new();
        for (i, shot) in run.projectiles.iter().enumerate() {
            // The next of the same kind and side near where its course has
            // it, in the order they were loosed.
            let near = (at..was.len().min(at + LOOK_AHEAD))
                .filter(|&j| !taken[j] && was[j].kind == shot.kind && was[j].hostile == shot.hostile)
                .find(|&j| (now[j].0 - shot.x).hypot(now[j].1 - shot.y) <= SAME_SHOT);
            stamp(shot, tick, &mut print)?;
            let flight = match near {
                Some(j) => {
                    taken[j] = true;
                    at = j + 1;
                    let mut f = std::mem::replace(&mut was[j], Flight::gone());
                    let n = tick - f.kf;
                    let held = if f.stamp == print {
                        // Sent as before, unless it left its course.
                        holds(&f.flown, n, now[j], shot)
                    } else {
                        let built = shot_value(shot, tick, f.kf, &f.course, Some(f.id))?;
                        let flown = flown_of(&built, f.kf);
                        let held = holds(&flown, n, flown.at(n, &marks, &stone, &aim, f.kf), shot);
                        if held && built != f.value {
                            f.value = Value::shared(built);
                            f.flown = flown;
                        }
                        held
                    };
                    if !held {
                        let steady = shot.traits.path.wait == 0 && !homes(shot);
                        let turn = (step == 1 && steady).then(|| turn_of(f.seen, (shot.vx, shot.vy))).flatten();
                        f.course = Course::of(shot, turn);
                        f.kf = tick;
                        let built = shot_value(shot, tick, tick, &f.course, Some(f.id))?;
                        f.flown = flown_of(&built, tick);
                        f.value = Value::shared(built);
                    }
                    f.seen = (shot.vx, shot.vy);
                    std::mem::swap(&mut f.stamp, &mut print);
                    // A long stamp's spare room does not stay with the shot.
                    super::kept::fit(&mut f.stamp);
                    super::kept::fit(&mut print);
                    f
                }
                None => {
                    self.next += 1;
                    let course = Course::of(shot, None);
                    let built = shot_value(shot, tick, tick, &course, Some(self.next))?;
                    super::kept::fit(&mut print);
                    Flight {
                        id: self.next,
                        kind: shot.kind,
                        hostile: shot.hostile,
                        kf: tick,
                        course,
                        flown: flown_of(&built, tick),
                        seen: (shot.vx, shot.vy),
                        value: Value::shared(built),
                        stamp: std::mem::take(&mut print),
                    }
                }
            };
            same &= last.and_then(|l| l.get(i)).is_some_and(|was| match (was, &flight.value) {
                (Value::Shared(a), Value::Shared(b)) => std::sync::Arc::ptr_eq(a, b),
                _ => false,
            });
            items.push(flight.value.clone());
            self.flying.push(flight);
        }
        if same && let Some(list) = &self.list {
            return Ok(list.clone());
        }
        let list = Value::shared(Value::Seq(items));
        self.list = Some(list.clone());
        Ok(list)
    }
}

impl Flight {
    /// A flight taken by a shot of this tick, left in its place.
    fn gone() -> Flight {
        Flight {
            id: 0,
            kind: Shot::Arrow,
            hostile: false,
            kf: 0,
            course: Course { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, turn: None },
            flown: Flown { course: Course { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, turn: None }, still: 0, weave: None, homing: None, body: 0.0 },
            seen: (0.0, 0.0),
            value: Value::Unit,
            stamp: Vec::new(),
        }
    }
}

/// Whether `shot` stands where `flown` has it `n` ticks on (`at`, as worked
/// out), and goes as fast, near enough; and its course is not one flown
/// too long.
fn holds(flown: &Flown, n: u64, at: (f32, f32, f32, f32), shot: &Projectile) -> bool {
    if (flown.course.turn.is_some() || flown.homing.is_some()) && n >= LONGEST_TURN {
        return false;
    }
    let (x, y, vx, vy) = at;
    // Written so that a NaN anywhere sends the course again.
    (x - shot.x).abs() <= SLIP && (y - shot.y).abs() <= SLIP && (vx - shot.vx).abs() <= SLIP_SPEED && (vy - shot.vy).abs() <= SLIP_SPEED
}

/// The run's shots as a tree carries them with no flights behind them
/// (`wire::publish`): each on a straight course from where it is now.
pub(super) fn shots_now(run: &Run) -> Result<Value, value::Error> {
    let shots = run
        .projectiles
        .iter()
        .map(|shot| shot_value(shot, run.tick, run.tick, &Course::of(shot, None), None))
        .collect::<Result<_, _>>()?;
    Ok(Value::Seq(shots))
}

fn field(name: &'static str, value: Value) -> (Cow<'static, str>, Value) {
    (Cow::Borrowed(name), value)
}

fn set(item: &mut Value, name: &str, to: Value) {
    if let Some(field) = item.field_mut(name) {
        *field = to;
    }
}

/// `set`, or the field added when the wire left it out (`course` carries
/// where a shot is, and a read puts the floats back for the run).
fn ensure(item: &mut Value, name: &'static str, to: Value) {
    if let Some(field) = item.field_mut(name) {
        *field = to;
    } else if let Value::Struct(fields) = item {
        fields.push(field(name, to));
    }
}

fn remove(item: &mut Value, name: &str) {
    if let Value::Struct(fields) = item.get_mut() {
        fields.retain(|(n, _)| n != name);
    }
}

/// The tree's field for a pattern shot's flight: the tick its wait ends,
/// which is the tick before its first in flight (its wait and its age, one
/// counting down to it and the other up from it).
const START: &str = "start";

/// A shot's fields left out when they hold what most shots do, and what
/// they then hold: read back as that (`shot_now`).
const PLAIN: [(&str, Value); 4] =
    [("look", Value::None), ("last_hit", Value::None), ("empowered", Value::Bool(false)), ("pierce", Value::U(0))];

/// A shot's gifts as most shots have them (`ShotTraits::default`): a gift a
/// shot has as those is left out, and read back as it.
fn plain_traits() -> &'static [(value::Name, Value)] {
    static PLAIN: std::sync::OnceLock<Vec<(value::Name, Value)>> = std::sync::OnceLock::new();
    PLAIN.get_or_init(|| match value::to_value(&crate::rules::ShotTraits::default()) {
        Ok(Value::Struct(fields)) => fields.into_iter().filter(|(name, _)| name != "path").collect(),
        _ => Vec::new(),
    })
}

/// A pattern shot's path as most have it (`Path::default`): a part of it a
/// shot has as that is left out, and read back as it.
fn plain_path() -> &'static [(value::Name, Value)] {
    static PLAIN: std::sync::OnceLock<Vec<(value::Name, Value)>> = std::sync::OnceLock::new();
    PLAIN.get_or_init(|| match value::to_value(&crate::rules::patterns::Path::default()) {
        Ok(Value::Struct(fields)) => fields.into_iter().filter(|(name, _)| name != "wait" && name != "age").collect(),
        _ => Vec::new(),
    })
}

/// `fields` without those that hold what `plain` says they hold.
fn leave_plain(fields: &mut Vec<(value::Name, Value)>, plain: &[(value::Name, Value)]) {
    fields.retain(|(name, v)| !plain.iter().any(|(n, p)| n == name && p == v));
}

/// `fields` with those `leave_plain` left out back.
fn put_plain(fields: &mut Vec<(value::Name, Value)>, plain: &[(value::Name, Value)]) {
    for (name, p) in plain {
        if !fields.iter().any(|(n, _)| n == name) {
            fields.push((name.clone(), p.clone()));
        }
    }
}

/// A shot's course and the tick it was taken, as one field (`course_bytes`).
const COURSE: &str = "course";

fn course_bytes(course: &Course, kf: u64) -> Value {
    let mut raw = Vec::with_capacity(24);
    for n in [course.x, course.y, course.vx, course.vy] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    raw.extend_from_slice(&kf.to_le_bytes());
    Value::Bytes(raw.into())
}

/// `(course, kf)` as `course_bytes` wrote them. A tree from before that
/// packing still has the four floats and `kf` as fields.
fn unpacked_course(shot: &Value) -> (Course, u64) {
    let turn = match shot.field("turn").map(Value::get) {
        Some(Value::Seq(cs)) => match cs.as_slice() {
            [Value::F32(c), Value::F32(s)] => Some((*c, *s)),
            _ => None,
        },
        _ => None,
    };
    if let Some(Value::Bytes(raw)) = shot.field(COURSE).map(Value::get) {
        if raw.len() >= 24 {
            let f = |i: usize| f32::from_le_bytes(raw[i..i + 4].try_into().unwrap());
            let kf = u64::from_le_bytes(raw[16..24].try_into().unwrap());
            return (Course { x: f(0), y: f(4), vx: f(8), vy: f(12), turn }, kf);
        }
    }
    let kf = shot.field("kf").and_then(Value::as_id).unwrap_or(0);
    (Course { x: float(shot, "x"), y: float(shot, "y"), vx: float(shot, "vx"), vy: float(shot, "vy"), turn }, kf)
}

/// `shot` as the tick's tree carries it: its course from `kf`, its life as
/// the tick it ends, a blade's way out as the tick it ends (0: done), a
/// pattern shot's wait and age as the tick between them (`START`), a shot
/// with no pattern without one, and its id.
pub(super) fn shot_value(shot: &Projectile, tick: u64, kf: u64, course: &Course, id: Option<u64>) -> Result<Value, value::Error> {
    let mut value = value::to_value(shot)?;
    set(&mut value, "ttl", Value::U(tick + u64::from(shot.ttl)));
    // The course is one field, not five: a resend is the four floats and
    // the tick they were taken, and a shot that has not left it is the
    // same bytes the seat already holds.
    remove(&mut value, "x");
    remove(&mut value, "y");
    remove(&mut value, "vx");
    remove(&mut value, "vy");
    if let Value::Struct(fields) = &mut value {
        fields.retain(|(name, v)| !PLAIN.iter().any(|(n, plain)| n == name && plain == v));
    }
    if let Some(traits) = value.field_mut("traits") {
        let out = u64::from(shot.traits.out);
        set(traits, "out", Value::U(if out == 0 { 0 } else { tick + out }));
        if let Value::Struct(fields) = traits {
            leave_plain(fields, plain_traits());
        }
        let path = &shot.traits.path;
        if *path == crate::rules::patterns::Path::default() {
            remove(traits, "path");
        } else if (path.wait == 0 || path.age == 0)
            && let Some(sent) = traits.field_mut("path")
        {
            let start = (tick + u64::from(path.wait)).saturating_sub(u64::from(path.age));
            remove(sent, "wait");
            remove(sent, "age");
            if let Value::Struct(fields) = sent {
                leave_plain(fields, plain_path());
                fields.push(field(START, Value::U(start)));
            }
        } else if let Some(Value::Struct(fields)) = traits.field_mut("path") {
            leave_plain(fields, plain_path());
        }
    }
    // Defaults already restore absent traits, including in older clients.
    // An empty struct still costs a name and a tag on every shot birth.
    if matches!(value.field("traits").map(Value::get), Some(Value::Struct(fields)) if fields.is_empty()) {
        remove(&mut value, "traits");
    }
    if let Value::Struct(fields) = &mut value {
        fields.push(field(COURSE, course_bytes(course, kf)));
        if let Some((c, s)) = course.turn {
            fields.push(field("turn", Value::Seq(vec![Value::F32(c), Value::F32(s)])));
        }
        if let Some(id) = id {
            fields.push(field("id", Value::U(id)));
        }
    }
    Ok(value)
}

/// How wide a friendly shot is where stone stops it, or 0 when the host
/// lets it through (`play`: a hostile shot, or a blade on its way home).
fn body_of(shot: &Value) -> f32 {
    let traits = shot.field("traits");
    let back = traits.and_then(|t| t.field("back")).and_then(Value::as_id).unwrap_or(0);
    let hostile = matches!(shot.field("hostile"), Some(Value::Bool(true)));
    if hostile || back == 2 {
        return 0.0;
    }
    let size = traits.and_then(|t| t.field("size")).and_then(Value::as_id).unwrap_or(0).min(3);
    crate::rules::patterns::KNIGHT_BODY + crate::rules::wishcraft::SIZE_REACH * size as f32
}

fn float(shot: &Value, name: &str) -> f32 {
    match shot.field(name) {
        Some(Value::F32(v)) => *v,
        _ => 0.0,
    }
}

/// A pattern shot's `START` as sent, if it has one.
fn start_of(shot: &Value) -> Option<u64> {
    shot.field("traits")?.field("path")?.field(START)?.as_id()
}

/// The course a shot as sent (`shot_value`) is flown on from its tick `kf`.
fn flown_of(shot: &Value, kf: u64) -> Flown {
    let (course, _) = unpacked_course(shot);
    let start = start_of(shot);
    let path = shot.field("traits").and_then(|t| t.field("path"));
    let weave = path.and_then(|path| {
        let wave = float(path, "wave");
        (wave != 0.0).then(|| (wave, float(path, "wave_rate"), start.map_or(0, |s| kf.saturating_sub(s))))
    });
    let traits = shot.field("traits");
    let homing = traits.and_then(|t| t.field("homing")).and_then(Value::as_id).unwrap_or(0);
    let hostile = matches!(shot.field("hostile"), Some(Value::Bool(true)));
    let homing = (!hostile && homing > 0).then(|| crate::rules::HOMING_TURN * homing as f32 * DT);
    Flown { course, still: start.map_or(0, |s| s.saturating_sub(kf)), weave, homing, body: body_of(shot) }
}


/// The monsters as they stood on `tick`, or `fallback` (where they stand now)
/// when that tick was not kept.
fn aimed<'a>(aim: &'a [(u64, Vec<(f32, f32)>)], tick: u64, fallback: &'a [(f32, f32)]) -> &'a [(f32, f32)] {
    aim.iter().rev().find(|(t, _)| *t == tick).map(|(_, m)| m.as_slice()).unwrap_or(fallback)
}

/// How far `mark` stands from `at`, squared: the nearest is the nearest.
fn far(mark: &(f32, f32), at: (f32, f32)) -> f32 {
    let (dx, dy) = (mark.0 - at.0, mark.1 - at.1);
    dx * dx + dy * dy
}

/// Whether a shot turns toward the monsters by itself (`Flown::homing`).
fn homes(shot: &Projectile) -> bool {
    !shot.hostile && shot.traits.homing > 0
}

/// Where the live monsters stand, as a homing shot sees them.
fn marks_of_run(run: &Run) -> Vec<(f32, f32)> {
    run.enemies.iter().filter(|e| e.hp > 0).map(|e| (e.x, e.y)).collect()
}

/// `marks_of_run` of the run a tree carries: its `enemies` as sent, flown
/// to the tick when a foe is on a course. A homing shot's aim list, when
/// the tree carries it, is where the host flew toward, which is the truth.
pub(super) fn marks_of_tree(tree: &Value) -> Vec<(f32, f32)> {
    let tick = tree.field("tick").and_then(Value::as_id).unwrap_or(0);
    let aim = aim_of(tree);
    if let Some((_, marks)) = aim.iter().rev().find(|(t, _)| *t == tick) {
        return marks.clone();
    }
    match tree.field("enemies").map(Value::get) {
        Some(Value::Seq(enemies)) => enemies
            .iter()
            .filter(|e| e.field("hp").and_then(Value::as_id).is_some_and(|hp| hp > 0))
            .map(|e| super::foes::pos_at(e, tick))
            .collect(),
        _ => Vec::new(),
    }
}

/// A shot as a tree carries it (`shot_value`), as it stands on `tick`: as
/// the run holds it, for reading back.
pub(super) fn shot_now(
    shot: &Value,
    tick: u64,
    marks: &[(f32, f32)],
    stone: &impl Fn((f32, f32), (f32, f32), f32) -> Option<(f32, f32)>,
    aim: &[(u64, Vec<(f32, f32)>)],
) -> Value {
    let (_, kf) = unpacked_course(shot);
    let (x, y, vx, vy) = flown_of(shot, kf).at(tick.saturating_sub(kf), marks, stone, aim, kf);
    let start = start_of(shot);
    let mut shot = shot.get().clone();
    ensure(&mut shot, "x", Value::F32(x));
    ensure(&mut shot, "y", Value::F32(y));
    ensure(&mut shot, "vx", Value::F32(vx));
    ensure(&mut shot, "vy", Value::F32(vy));
    remove(&mut shot, COURSE);
    if let Some(end) = shot.field("ttl").and_then(Value::as_id) {
        set(&mut shot, "ttl", Value::U(end.saturating_sub(tick)));
    }
    if let Value::Struct(fields) = &mut shot {
        for (name, plain) in &PLAIN {
            if !fields.iter().any(|(n, _)| n == name) {
                fields.push((Cow::Borrowed(*name), plain.clone()));
            }
        }
    }
    if let Some(Value::Struct(fields)) = shot.field_mut("traits") {
        put_plain(fields, plain_traits());
    }
    if let Some(Value::Struct(fields)) = shot.field_mut("traits").and_then(|t| t.field_mut("path")) {
        put_plain(fields, plain_path());
    }
    if let Some(traits) = shot.field_mut("traits") {
        if let Some(end) = traits.field("out").and_then(Value::as_id) {
            set(traits, "out", Value::U(if end == 0 { 0 } else { end.saturating_sub(tick) }));
        }
        if let (Some(start), Some(Value::Struct(path))) = (start, traits.field_mut("path")) {
            path.retain(|(n, _)| n != START);
            path.push(field("wait", Value::U(start.saturating_sub(tick))));
            path.push(field("age", Value::U(tick.saturating_sub(start).min(u64::from(u16::MAX)))));
        }
    }
    shot
}

/// A tree's list of shots as they stand on `tick` (`shot_now`).
pub(super) fn shots_at(
    shots: &Value,
    tick: u64,
    marks: &[(f32, f32)],
    stone: &impl Fn((f32, f32), (f32, f32), f32) -> Option<(f32, f32)>,
    aim: &[(u64, Vec<(f32, f32)>)],
) -> Value {
    match shots.get() {
        Value::Seq(items) => Value::Seq(items.iter().map(|shot| shot_now(shot, tick, marks, stone, aim)).collect()),
        other => other.clone(),
    }
}

/// The field a tree carries recent monster positions under (`aim_value`).
pub(super) const AIM: &str = "shot_aim";

/// `aim` as the tree carries it, one entry a tick.
pub(super) fn aim_value(aim: &[(u64, Vec<(f32, f32)>)]) -> Value {
    Value::Seq(
        aim.iter()
            .map(|(t, marks)| {
                let marks = marks
                    .iter()
                    .map(|(x, y)| Value::Seq(vec![Value::F32(*x), Value::F32(*y)]))
                    .collect();
                Value::Seq(vec![Value::U(*t), Value::Seq(marks)])
            })
            .collect(),
    )
}

fn pair_at(item: &Value, i: usize) -> f32 {
    match item.get() {
        Value::Seq(xy) => match xy.get(i) {
            Some(Value::F32(v)) => *v,
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// `aim_value` read back.
pub(super) fn aim_of(tree: &Value) -> Vec<(u64, Vec<(f32, f32)>)> {
    let Some(Value::Seq(items)) = tree.field(AIM).map(Value::get) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item.get() {
            Value::Seq(pair) => {
                let t = pair.first().and_then(Value::as_id)?;
                let marks = match pair.get(1).map(Value::get) {
                    Some(Value::Seq(marks)) => marks.iter().map(|m| (pair_at(m, 0), pair_at(m, 1))).collect(),
                    _ => Vec::new(),
                };
                Some((t, marks))
            }
            _ => None,
        })
        .collect()
}

/// A shot's course, kept so a tick that did not resend it can fly it again
/// (`Flown::at`) without building the list or reading it back.
pub(in crate::wire) struct ShotHold {
    pub(in crate::wire) id: u64,
    flown: Flown,
    kf: u64,
    end: Option<u64>,
    out_end: Option<u64>,
    start: Option<u64>,
    pub(in crate::wire) shot: Projectile,
}

/// Courses for `placed`, which `shots_at` just put where `tick` has them.
pub(in crate::wire) fn hold_shots(shots: &Value, placed: &[Projectile]) -> Vec<ShotHold> {
    let Value::Seq(items) = shots.get() else {
        return Vec::new();
    };
    if items.len() != placed.len() {
        return Vec::new();
    }
    items
        .iter()
        .zip(placed)
        .map(|(shot, placed)| {
            let (_, kf) = unpacked_course(shot);
            ShotHold {
                id: shot.field("id").and_then(Value::as_id).unwrap_or(0),
                flown: flown_of(shot, kf),
                kf,
                end: shot.field("ttl").and_then(Value::as_id),
                out_end: shot.field("traits").and_then(|traits| traits.field("out")).and_then(Value::as_id),
                start: start_of(shot),
                shot: placed.clone(),
            }
        })
        .collect()
}

/// `placed` flown on to `tick` (`shot_now`), from courses already read.
pub(in crate::wire) fn fly_holds(
    holds: &[ShotHold],
    tick: u64,
    marks: &[(f32, f32)],
    stone: &impl Fn((f32, f32), (f32, f32), f32) -> Option<(f32, f32)>,
    aim: &[(u64, Vec<(f32, f32)>)],
) -> Vec<Projectile> {
    holds
        .iter()
        .map(|held| {
            let (x, y, vx, vy) = held.flown.at(tick.saturating_sub(held.kf), marks, stone, aim, held.kf);
            let mut shot = held.shot.clone();
            shot.x = x;
            shot.y = y;
            shot.vx = vx;
            shot.vy = vy;
            if let Some(end) = held.end {
                shot.ttl = end.saturating_sub(tick) as u32;
            }
            if let Some(end) = held.out_end {
                shot.traits.out = if end == 0 { 0 } else { end.saturating_sub(tick) as u16 };
            }
            if let Some(start) = held.start {
                shot.traits.path.wait = start.saturating_sub(tick) as u8;
                shot.traits.path.age = tick.saturating_sub(start).min(u64::from(u16::MAX)) as u16;
            }
            shot
        })
        .collect()
}

/// Shots as they stand on `tick`, and the courses to fly them on from here.
/// One pass (`shots_at`), not the open-air pass `read` throws away.
pub(in crate::wire) fn place_shots(
    shots: &Value,
    tick: u64,
    marks: &[(f32, f32)],
    stone: &impl Fn((f32, f32), (f32, f32), f32) -> Option<(f32, f32)>,
    aim: &[(u64, Vec<(f32, f32)>)],
) -> Result<(Vec<Projectile>, Vec<ShotHold>), value::Error> {
    let placed = shots_at(shots, tick, marks, stone, aim);
    let projectiles: Vec<Projectile> = value::from_value(&placed)?;
    let holds = hold_shots(shots, &projectiles);
    Ok((projectiles, holds))
}

/// One shot placed on `tick`, for a list that lost or gained a few.
pub(in crate::wire) fn place_one(
    shot: &Value,
    tick: u64,
    marks: &[(f32, f32)],
    stone: &impl Fn((f32, f32), (f32, f32), f32) -> Option<(f32, f32)>,
    aim: &[(u64, Vec<(f32, f32)>)],
) -> Option<ShotHold> {
    let wrapped = Value::Seq(vec![shot.clone()]);
    let (_, mut holds) = place_shots(&wrapped, tick, marks, stone, aim).ok()?;
    holds.pop()
}

