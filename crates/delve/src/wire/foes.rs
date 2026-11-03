//! Monsters on the wire, the way shots in flight are (`flight`): a foe
//! standing still or walking a straight line is sent when it starts, and
//! again when it leaves that line, not every tick. Its `timer`, which counts
//! up one a tick while it acts, is the tick it counts from.
//!
//! A client adds the same step the sim did (`at`), so a held course is the
//! host's place to the bit, inside the tolerance a shot is allowed. A foe
//! whose step is not that line is sent where it stands: a wobble that will
//! not hold is not a heavier course.

use crate::rules::{Enemy, Run};
use angel_net::doc::value::{self, Value};
use std::borrow::Cow;

/// A foe's course and the tick it was taken, as one field: where it stood,
/// and the step it was taking, so a client can fly it (`at`).
const COURSE: &str = "course";

/// The tick a foe's `timer` counts up from, when it does. Absent, `timer` is
/// the count itself (a foe standing dormant, or one whose count just jumped).
const FROM: &str = "from";

/// Where a foe was sent from, and the step it was taking.
#[derive(Clone, Copy)]
struct Course {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

/// A foe as last published.
struct Foe {
    id: u32,
    /// The tick its course was sent on. A raw position has none.
    kf: u64,
    course: Option<Course>,
    value: Value,
    stamp: Vec<u8>,
    /// Where it stood last tick, and the step it took, so a steady line can
    /// be told from a wobble.
    last_x: f32,
    last_y: f32,
    last_vx: f32,
    last_vy: f32,
    last_timer: u32,
}

/// The monsters of one run, as last published.
#[derive(Default)]
pub(super) struct Foes {
    flying: Vec<Foe>,
    list: Option<Value>,
    tick: Option<u64>,
}

/// What of a foe `stamp` reads whole: all but where it stands (its course's)
/// and the counters a client keeps (`timer` counting up, `age` the tick it
/// was born).
static MASK: value::Mask = value::Mask { skip: &["x", "y", "timer", "age"], inside: &[] };

/// Steps still added one by one, so the bits match the host. Past this a
/// damaged tick is the closed form: it must not walk a foe forever.
const EXACT_STEPS: u64 = 1 << 16;

/// Where `course` has a foe `n` ticks on. The step is the sim's own f32
/// add, so a straight walk is the same bits the host has, not a nearby float.
fn at(course: &Course, n: u64) -> (f32, f32) {
    if n > EXACT_STEPS {
        return (
            course.x + course.vx * n as f32,
            course.y + course.vy * n as f32,
        );
    }
    let mut x = course.x;
    let mut y = course.y;
    for _ in 0..n {
        x += course.vx;
        y += course.vy;
    }
    (x, y)
}

/// The tick `timer` counts from, when it has counted up by `step` since it
/// was last sent, or none when the count jumped or stood.
fn counting_from(enemy: &Enemy, tick: u64, step: u64, last_timer: u32) -> Option<u64> {
    let up = step > 0 && last_timer.checked_add(step as u32) == Some(enemy.timer);
    up.then(|| tick.wrapping_sub(u64::from(enemy.timer)))
}

/// What a foe as sent on `tick` is made of beside its course: the same stamp,
/// the same foe sent. `from` is the tick its timer counts from, or none.
fn stamp(enemy: &Enemy, tick: u64, from: Option<u64>, out: &mut Vec<u8>) -> Result<(), value::Error> {
    out.clear();
    value::fingerprint_except(enemy, out, &MASK)?;
    let birth = tick.wrapping_sub(u64::from(enemy.age)) & u64::from(u32::MAX);
    out.extend_from_slice(&birth.to_le_bytes());
    out.push(u8::from(from.is_some()));
    let mark = from.unwrap_or(u64::from(enemy.timer));
    out.extend_from_slice(&mark.to_le_bytes());
    Ok(())
}

impl Foes {
    /// The run's foes as the tick's list, each sent where a client can keep
    /// it: on its course while that step still lands on the host, and its timer as
    /// the tick it counts from while the count climbs by one a tick.
    pub(super) fn publish(&mut self, run: &Run) -> Result<Value, value::Error> {
        let tick = run.tick;
        let step = match self.tick {
            Some(last) if last <= tick => tick - last,
            _ => {
                self.flying.clear();
                0
            }
        };
        self.tick = Some(tick);
        let mut was = std::mem::take(&mut self.flying);
        let mut taken = vec![false; was.len()];
        let last = match self.list.as_ref().map(Value::get) {
            Some(Value::Seq(list)) if list.len() == run.enemies.len() => Some(list),
            _ => None,
        };
        let mut same = last.is_some();
        let mut items = Vec::with_capacity(run.enemies.len());
        let mut print = Vec::new();
        for (i, enemy) in run.enemies.iter().enumerate() {
            let near = was.iter().enumerate().find(|(j, f)| !taken[*j] && f.id == enemy.id).map(|(j, _)| j);
            let from = near.and_then(|j| counting_from(enemy, tick, step, was[j].last_timer));
            stamp(enemy, tick, from, &mut print)?;
            let foe = match near {
                Some(j) => {
                    taken[j] = true;
                    let mut f = std::mem::replace(&mut was[j], Foe::gone());
                    let observed = (step > 0).then(|| ((enemy.x - f.last_x), (enemy.y - f.last_y)));
                    let steady = observed.is_some_and(|(dx, dy)| dx == f.last_vx && dy == f.last_vy);
                    let n = tick.saturating_sub(f.kf);
                    let on_course = f.course.is_some_and(|c| {
                        let (px, py) = at(&c, n);
                        px == enemy.x && py == enemy.y
                    });
                    if f.stamp == print && on_course {
                        // Sent as before: the client already has it.
                    } else if on_course {
                        f.value = Value::shared(foe_value(enemy, tick, f.course.as_ref(), f.kf, from)?);
                    } else if steady {
                        let (vx, vy) = observed.unwrap_or((0.0, 0.0));
                        let course = Course { x: enemy.x, y: enemy.y, vx, vy };
                        f.kf = tick;
                        f.course = Some(course);
                        f.value = Value::shared(foe_value(enemy, tick, Some(&course), tick, from)?);
                    } else {
                        // A wobble, or the first step of a new line: where it
                        // stands, not a course that will not hold.
                        f.course = None;
                        f.kf = tick;
                        f.value = Value::shared(foe_value(enemy, tick, None, tick, from)?);
                    }
                    if let Some((vx, vy)) = observed {
                        f.last_vx = vx;
                        f.last_vy = vy;
                    }
                    f.last_x = enemy.x;
                    f.last_y = enemy.y;
                    f.last_timer = enemy.timer;
                    std::mem::swap(&mut f.stamp, &mut print);
                    // A long stamp's spare room does not stay with the foe.
                    super::kept::fit(&mut f.stamp);
                    super::kept::fit(&mut print);
                    f
                }
                None => {
                    let built = foe_value(enemy, tick, None, tick, None)?;
                    super::kept::fit(&mut print);
                    Foe {
                        id: enemy.id,
                        kf: tick,
                        course: None,
                        value: Value::shared(built),
                        stamp: std::mem::take(&mut print),
                        last_x: enemy.x,
                        last_y: enemy.y,
                        last_vx: 0.0,
                        last_vy: 0.0,
                        last_timer: enemy.timer,
                    }
                }
            };
            same &= last.and_then(|l| l.get(i)).is_some_and(|old| match (old, &foe.value) {
                (Value::Shared(a), Value::Shared(b)) => std::sync::Arc::ptr_eq(a, b),
                _ => false,
            });
            items.push(foe.value.clone());
            self.flying.push(foe);
        }
        if same && let Some(list) = &self.list {
            return Ok(list.clone());
        }
        let list = Value::shared(Value::Seq(items));
        self.list = Some(list.clone());
        Ok(list)
    }
}

impl Foe {
    fn gone() -> Foe {
        Foe {
            id: 0,
            kf: 0,
            course: None,
            value: Value::Unit,
            stamp: Vec::new(),
            last_x: 0.0,
            last_y: 0.0,
            last_vx: 0.0,
            last_vy: 0.0,
            last_timer: 0,
        }
    }
}

/// `enemy` as the tick's tree carries it: its age as the tick it was born,
/// its timer as the tick it counts from when `from` is that tick, and its
/// place as `course` when it is on one (a client flies that, `at`).
fn foe_value(enemy: &Enemy, tick: u64, course: Option<&Course>, kf: u64, from: Option<u64>) -> Result<Value, value::Error> {
    let mut value = value::to_value(enemy)?;
    let birth = tick.wrapping_sub(u64::from(enemy.age)) & u64::from(u32::MAX);
    set(&mut value, "age", Value::U(birth));
    if from.is_some() {
        remove(&mut value, "timer");
    }
    if course.is_some() {
        remove(&mut value, "x");
        remove(&mut value, "y");
    }
    if let Value::Struct(fields) = &mut value {
        if let Some(course) = course {
            fields.push(field(COURSE, course_bytes(course, kf)));
        }
        if let Some(from) = from {
            fields.push(field(FROM, Value::U(from)));
        }
    }
    Ok(value)
}

fn course_bytes(course: &Course, kf: u64) -> Value {
    let mut raw = Vec::with_capacity(24);
    for n in [course.x, course.y, course.vx, course.vy] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    raw.extend_from_slice(&kf.to_le_bytes());
    Value::Bytes(raw.into())
}

fn unpacked(foe: &Value) -> Option<(Course, u64)> {
    let Value::Bytes(raw) = foe.field(COURSE)? else {
        return None;
    };
    if raw.len() < 24 {
        return None;
    }
    let f = |i: usize| f32::from_le_bytes(raw[i..i + 4].try_into().unwrap());
    let kf = u64::from_le_bytes(raw[16..24].try_into().unwrap());
    Some((Course { x: f(0), y: f(4), vx: f(8), vy: f(12) }, kf))
}

/// Where a foe as sent stands on `tick`: flown on its course, or where the
/// tree puts it when it has none.
pub(super) fn pos_at(foe: &Value, tick: u64) -> (f32, f32) {
    if let Some((course, kf)) = unpacked(foe) {
        return at(&course, tick.saturating_sub(kf));
    }
    (float(foe, "x"), float(foe, "y"))
}

/// The count on `tick` when the foe counts from a start, else none (the
/// count already on the foe stands, as `read` leaves it).
pub(super) fn timer_at(foe: &Value, tick: u64) -> Option<u32> {
    let from = foe.field(FROM).and_then(Value::as_id)?;
    Some((tick.wrapping_sub(from) & u64::from(u32::MAX)) as u32)
}

/// Whether `enemies` carries a course or a timer start a read must bring to
/// the tick. A tree from `publish` (one shot, no publisher) does not.
pub(super) fn needs_land(enemies: &Value) -> bool {
    match enemies.get() {
        Value::Seq(items) => items.iter().any(|e| e.field(COURSE).is_some() || e.field(FROM).is_some()),
        _ => false,
    }
}

/// `enemies` as the tick's tree carries them, as they stand on `tick`: as the
/// run holds them, for reading back.
pub(super) fn land(enemies: &Value, tick: u64) -> Value {
    match enemies.get() {
        Value::Seq(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let (x, y) = pos_at(item, tick);
                let mut foe = item.get().clone();
                ensure(&mut foe, "x", Value::F32(x));
                ensure(&mut foe, "y", Value::F32(y));
                remove(&mut foe, COURSE);
                if let Some(from) = foe.field(FROM).and_then(Value::as_id) {
                    let timer = tick.wrapping_sub(from) & u64::from(u32::MAX);
                    ensure(&mut foe, "timer", Value::U(timer));
                    remove(&mut foe, FROM);
                }
                out.push(foe);
            }
            Value::Seq(out)
        }
        _ => enemies.get().clone(),
    }
}

fn field(name: &'static str, value: Value) -> (Cow<'static, str>, Value) {
    (Cow::Borrowed(name), value)
}

fn set(item: &mut Value, name: &str, to: Value) {
    if let Some(field) = item.field_mut(name) {
        *field = to;
    }
}

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

fn float(foe: &Value, name: &str) -> f32 {
    match foe.field(name) {
        Some(Value::F32(v)) => *v,
        _ => 0.0,
    }
}
