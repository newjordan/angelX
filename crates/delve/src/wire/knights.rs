//! Knights on the wire as courses and clocks.
//!
//! A knight has about a hundred fields. Most of them stand still. What moves
//! every tick on a fighting floor is what a guest can already keep: a walk in
//! a straight line (the same f32 step the sim adds), a counter that ticks by
//! one, a cooldown counting down, and a field sitting at the value a missing
//! field deserializes as. Those are not sent again while they hold. A guest
//! flies the course and winds the clocks forward to the tick, and fills a
//! missing field from its serde default, so the knight it reads is the host's.

use crate::rules::{Hero, Run};
use angel_net::doc::value::{self, Value};
use std::borrow::Cow;
use std::sync::OnceLock;

/// The course bytes on a knight, and the clocks beside them.
const COURSE: &str = "course";
const CLOCKS: &str = "clocks";

/// A straight step, as the sim adds it. `x` and `y` are where the knight
/// stood on tick `kf`; each later tick adds `vx` and `vy` once.
#[derive(Clone, Copy)]
struct Course {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    kf: u64,
}

/// A field that changes by the same step every tick, sent as the tick that
/// step was measured from. `base` is the value on `from`.
#[derive(Clone)]
struct Clock {
    name: String,
    from: u64,
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Kind {
    /// An integer field. `unsigned` is how it is written back.
    Int { base: i64, step: i64, unsigned: bool },
    /// A float field, wound by the same f32 add the course uses.
    Float { base: f32, step: f32 },
}

/// One knight as last published, and the step we last saw, so a course or a
/// clock is adopted only once the step has repeated.
struct Knight {
    id: u32,
    value: Value,
    last_x: f32,
    last_y: f32,
    last_vx: f32,
    last_vy: f32,
    course: Option<Course>,
    /// Numeric fields last seen, and the step into that sight, so the next
    /// tick can tell a steady count from a jump.
    seen: Vec<Seen>,
    clocks: Vec<Clock>,
}

struct Seen {
    name: String,
    value: Num,
    step: Option<Num>,
}

#[derive(Clone, Copy, PartialEq)]
enum Num {
    Int(i64, bool),
    Float(f32),
}

#[derive(Default)]
pub(super) struct Knights {
    flying: Vec<Knight>,
    list: Option<Value>,
    tick: Option<u64>,
}

impl Knights {
    /// The run's knights as the tick's map. A knight already sent, still on
    /// its course and its clocks, is the same shared value: the patch carries
    /// nothing for it.
    pub(super) fn publish(&mut self, run: &Run, absent: &[u8]) -> Result<Value, value::Error> {
        let tick = run.tick;
        let step = match self.tick {
            Some(last) if last < tick => tick - last,
            Some(_) => 0,
            None => {
                self.flying.clear();
                0
            }
        };
        self.tick = Some(tick);
        let bare = bare_hero();
        let mut was = std::mem::take(&mut self.flying);
        let mut taken = vec![false; was.len()];
        let mut items = Vec::with_capacity(run.players.len());
        for (id, hero) in &run.players {
            if absent.chunks_exact(4).any(|a| u32::from_le_bytes([a[0], a[1], a[2], a[3]]) == *id) {
                continue;
            }
            let near = was.iter().enumerate().find(|(j, k)| !taken[*j] && k.id == *id).map(|(j, _)| j);
            let shaped = match near {
                Some(j) => {
                    taken[j] = true;
                    let mut knight = std::mem::replace(&mut was[j], Knight::gone());
                    let raw = value::to_value(hero)?;
                    if knight.hold(hero, &raw, tick, step, bare)? {
                        // Sent as before.
                    } else {
                        knight.value = Value::shared(knight.shape(hero, &raw, tick, step, bare)?);
                    }
                    knight
                }
                None => {
                    let raw = value::to_value(hero)?;
                    let mut knight = Knight::fresh(*id, hero);
                    knight.value = Value::shared(knight.shape(hero, &raw, tick, step, bare)?);
                    knight
                }
            };
            items.push((Value::U(u64::from(*id)), shaped.value.clone()));
            self.flying.push(shaped);
        }
        if let Some(list) = &self.list
            && same_knights(list, &items)
        {
            return Ok(list.clone());
        }
        let list = Value::shared(Value::Map(items));
        self.list = Some(list.clone());
        Ok(list)
    }
}

fn same_knights(list: &Value, items: &[(Value, Value)]) -> bool {
    let Value::Map(old) = list.get() else {
        return false;
    };
    old.len() == items.len()
        && old.iter().zip(items).all(|((ak, av), (bk, bv))| {
            ak == bk
                && match (av, bv) {
                    (Value::Shared(a), Value::Shared(b)) => std::sync::Arc::ptr_eq(a, b),
                    _ => false,
                }
        })
}

impl Knight {
    fn gone() -> Knight {
        Knight {
            id: 0,
            value: Value::Unit,
            last_x: 0.0,
            last_y: 0.0,
            last_vx: 0.0,
            last_vy: 0.0,
            course: None,
            seen: Vec::new(),
            clocks: Vec::new(),
        }
    }

    fn fresh(id: u32, hero: &Hero) -> Knight {
        Knight {
            id,
            value: Value::Unit,
            last_x: hero.x,
            last_y: hero.y,
            last_vx: 0.0,
            last_vy: 0.0,
            course: None,
            seen: Vec::new(),
            clocks: Vec::new(),
        }
    }

    /// The value already sent still lands on this knight: the course flies to
    /// where it stands, and every clock winds to the value it has.
    fn hold(&mut self, hero: &Hero, raw: &Value, tick: u64, step: u64, bare: &Value) -> Result<bool, value::Error> {
        let on_course = self.course.is_some_and(|c| {
            let (px, py) = at(&c, tick);
            px == hero.x && py == hero.y
        });
        if !on_course || step == 0 {
            return Ok(false);
        }
        let nums = nums_of(raw);
        if !clocks_hold(&self.clocks, &nums, tick) {
            return Ok(false);
        }
        // The rest of the knight, with the course and the clocks put back as
        // they were sent, must be the value the guest already holds.
        let shaped = self.shape_with(hero, raw, self.course, &self.clocks, bare)?;
        Ok(shaped == *self.value.get())
    }

    fn shape(&mut self, hero: &Hero, raw: &Value, tick: u64, step: u64, bare: &Value) -> Result<Value, value::Error> {
        let observed = (step == 1).then_some((hero.x - self.last_x, hero.y - self.last_y));
        let steady = observed.is_some_and(|(dx, dy)| dx == self.last_vx && dy == self.last_vy);
        let course = if self.course.is_some_and(|c| {
            let (px, py) = at(&c, tick);
            px == hero.x && py == hero.y
        }) {
            self.course
        } else if steady {
            let (vx, vy) = observed.unwrap_or((0.0, 0.0));
            Some(Course { x: hero.x, y: hero.y, vx, vy, kf: tick })
        } else {
            None
        };
        let nums = nums_of(raw);
        self.clocks = next_clocks(&self.clocks, &self.seen, &nums, tick, step);
        let shaped = self.shape_with(hero, raw, course, &self.clocks, bare)?;
        if let Some((vx, vy)) = observed {
            self.last_vx = vx;
            self.last_vy = vy;
        }
        self.last_x = hero.x;
        self.last_y = hero.y;
        self.course = course;
        self.seen = nums
            .iter()
            .map(|(name, value)| Seen {
                name: name.clone(),
                value: *value,
                step: step_of(&self.seen, name, *value, step),
            })
            .collect();
        Ok(shaped)
    }

    fn shape_with(
        &self,
        hero: &Hero,
        raw: &Value,
        course: Option<Course>,
        clocks: &[Clock],
        bare: &Value,
    ) -> Result<Value, value::Error> {
        let _ = hero;
        let mut value = raw.clone();
        if course.is_some() {
            remove(&mut value, "x");
            remove(&mut value, "y");
        }
        for clock in clocks {
            remove(&mut value, &clock.name);
        }
        strip_defaults(&mut value, bare);
        if let Value::Struct(fields) = &mut value {
            if let Some(course) = course {
                fields.push(field(COURSE, course_bytes(&course)));
            }
            if !clocks.is_empty() {
                fields.push(field(CLOCKS, clocks_value(clocks)));
            }
        }
        Ok(value)
    }
}

/// The step into `value` from what we last saw, when this publish is one
/// tick on. A gap, or a first sight, is no step to adopt a clock from.
fn step_of(seen: &[Seen], name: &str, value: Num, step: u64) -> Option<Num> {
    if step != 1 {
        return None;
    }
    let last = seen.iter().find(|s| s.name == name)?;
    match (last.value, value) {
        (Num::Int(a, ua), Num::Int(b, ub)) if ua == ub => Some(Num::Int(b.wrapping_sub(a), ua)),
        (Num::Float(a), Num::Float(b)) => Some(Num::Float(b - a)),
        _ => None,
    }
}

fn clocks_hold(clocks: &[Clock], nums: &[(String, Num)], tick: u64) -> bool {
    clocks.iter().all(|clock| {
        let Some((_, value)) = nums.iter().find(|(n, _)| n == &clock.name) else {
            return false;
        };
        predict(clock, tick) == Some(*value)
    })
}

/// Clocks for this tick. One is kept while it still lands on the field. A
/// new one starts only once the same non-zero step has been seen twice, so a
/// hit that changes a field once is sent as itself, not as a clock that
/// breaks on the next tick.
fn next_clocks(had: &[Clock], seen: &[Seen], nums: &[(String, Num)], tick: u64, step: u64) -> Vec<Clock> {
    let mut out = Vec::new();
    for (name, value) in nums {
        if let Some(clock) = had.iter().find(|c| c.name == *name)
            && predict(clock, tick) == Some(*value)
        {
            out.push(clock.clone());
            continue;
        }
        let Some(seen) = seen.iter().find(|s| s.name == *name) else {
            continue;
        };
        let Some(prev) = seen.step else {
            continue;
        };
        if step != 1 || prev == num_zero(*value) {
            continue;
        }
        let this = match (seen.value, *value) {
            (Num::Int(a, ua), Num::Int(b, ub)) if ua == ub => Num::Int(b.wrapping_sub(a), ua),
            (Num::Float(a), Num::Float(b)) => Num::Float(b - a),
            _ => continue,
        };
        if this != prev {
            continue;
        }
        if let Some(clock) = clock_from(name, *value, this, tick) {
            out.push(clock);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn num_zero(value: Num) -> Num {
    match value {
        Num::Int(_, unsigned) => Num::Int(0, unsigned),
        Num::Float(_) => Num::Float(0.0),
    }
}

fn clock_from(name: &str, value: Num, step: Num, tick: u64) -> Option<Clock> {
    let kind = match (value, step) {
        (Num::Int(base, unsigned), Num::Int(step, _)) => Kind::Int { base, step, unsigned },
        (Num::Float(base), Num::Float(step)) => Kind::Float { base, step },
        _ => return None,
    };
    // The clock must land on the value we just saw, or it is not this field.
    let clock = Clock { name: name.to_string(), from: tick, kind };
    (predict(&clock, tick) == Some(value)).then_some(clock)
}

fn predict(clock: &Clock, tick: u64) -> Option<Num> {
    let n = tick.checked_sub(clock.from)?;
    match clock.kind {
        Kind::Int { base, step, unsigned } => {
            let n = i64::try_from(n).ok()?;
            let value = step.checked_mul(n)?.checked_add(base)?;
            if unsigned && value < 0 {
                return None;
            }
            Some(Num::Int(value, unsigned))
        }
        Kind::Float { base, step } => {
            let mut value = base;
            for _ in 0..n {
                value += step;
            }
            Some(Num::Float(value))
        }
    }
}

fn nums_of(raw: &Value) -> Vec<(String, Num)> {
    let Value::Struct(fields) = raw.get() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, value) in fields {
        if name == "x" || name == "y" {
            continue;
        }
        let Some(num) = as_num(value) else {
            continue;
        };
        out.push((name.to_string(), num));
    }
    out
}

fn as_num(value: &Value) -> Option<Num> {
    match value.get() {
        Value::U(n) => i64::try_from(*n).ok().map(|n| Num::Int(n, true)),
        Value::I(n) => Some(Num::Int(*n, false)),
        Value::F32(n) => Some(Num::Float(*n)),
        _ => None,
    }
}

/// A knight deserialized from no fields: what a missing field becomes. A
/// field sitting at that value is left out of the tree.
fn bare_hero() -> &'static Value {
    static BARE: OnceLock<Value> = OnceLock::new();
    BARE.get_or_init(|| {
        value::from_value::<Hero>(&Value::Struct(Vec::new()))
            .and_then(|hero| value::to_value(&hero))
            .unwrap_or(Value::Struct(Vec::new()))
    })
}

fn strip_defaults(value: &mut Value, bare: &Value) {
    let Value::Struct(fields) = value.get_mut() else {
        return;
    };
    fields.retain(|(name, field)| bare.field(name).is_none_or(|b| b != field.get()));
}

fn course_bytes(course: &Course) -> Value {
    let mut raw = Vec::with_capacity(24);
    for n in [course.x, course.y, course.vx, course.vy] {
        raw.extend_from_slice(&n.to_le_bytes());
    }
    raw.extend_from_slice(&course.kf.to_le_bytes());
    Value::Bytes(raw.into())
}

fn unpacked(knight: &Value) -> Option<Course> {
    let Value::Bytes(raw) = knight.field(COURSE)? else {
        return None;
    };
    if raw.len() < 24 {
        return None;
    }
    let f = |i: usize| -> Option<f32> { Some(f32::from_le_bytes(raw[i..i + 4].try_into().ok()?)) };
    let kf = u64::from_le_bytes(raw[16..24].try_into().ok()?);
    Some(Course { x: f(0)?, y: f(4)?, vx: f(8)?, vy: f(12)?, kf })
}

fn at(course: &Course, tick: u64) -> (f32, f32) {
    let mut x = course.x;
    let mut y = course.y;
    for _ in 0..tick.saturating_sub(course.kf) {
        x += course.vx;
        y += course.vy;
    }
    (x, y)
}

fn clocks_value(clocks: &[Clock]) -> Value {
    let mut fields = Vec::with_capacity(clocks.len());
    for clock in clocks {
        fields.push(field_owned(clock.name.clone(), clock_value(clock)));
    }
    Value::Struct(fields)
}

fn clock_value(clock: &Clock) -> Value {
    let mut fields = vec![field("from", Value::U(clock.from))];
    match clock.kind {
        Kind::Int { base, step, unsigned } => {
            fields.push(field("base", if unsigned { Value::U(base as u64) } else { Value::I(base) }));
            fields.push(field("step", Value::I(step)));
        }
        Kind::Float { base, step } => {
            fields.push(field("base", Value::F32(base)));
            fields.push(field("step", Value::F32(step)));
        }
    }
    Value::Struct(fields)
}

/// Whether `players` carries a course or a clock a read must bring to the tick.
pub(super) fn needs_land(players: &Value) -> bool {
    let Value::Map(items) = players.get() else {
        return false;
    };
    items.iter().any(|(_, hero)| hero.field(COURSE).is_some() || hero.field(CLOCKS).is_some())
}

/// `players` as the tick's tree carries them, as they stand on `tick`.
pub(super) fn land(players: &Value, tick: u64) -> Value {
    let Value::Map(items) = players.get() else {
        return players.get().clone();
    };
    let mut out = Vec::with_capacity(items.len());
    for (key, hero) in items {
        out.push((key.clone(), land_one(hero, tick)));
    }
    Value::Map(out)
}

fn land_one(hero: &Value, tick: u64) -> Value {
    let mut knight = hero.get().clone();
    if let Some(course) = unpacked(&knight) {
        let (x, y) = at(&course, tick);
        ensure(&mut knight, "x", Value::F32(x));
        ensure(&mut knight, "y", Value::F32(y));
        remove(&mut knight, COURSE);
    }
    if let Some(Value::Struct(clocks)) = knight.field(CLOCKS).cloned() {
        for (name, clock) in clocks {
            if let Some(value) = land_clock(&clock, tick) {
                ensure_owned(&mut knight, name, value);
            }
        }
        remove(&mut knight, CLOCKS);
    }
    knight
}

fn land_clock(clock: &Value, tick: u64) -> Option<Value> {
    let from = clock.field("from").and_then(Value::as_id)?;
    let n = tick.checked_sub(from)?;
    match (clock.field("base"), clock.field("step")) {
        (Some(Value::U(base)), Some(Value::I(step))) => {
            let n = i64::try_from(n).ok()?;
            let value = step.checked_mul(n)?.checked_add(i64::try_from(*base).ok()?)?;
            u64::try_from(value).ok().map(Value::U)
        }
        (Some(Value::I(base)), Some(Value::I(step))) => {
            let n = i64::try_from(n).ok()?;
            step.checked_mul(n)?.checked_add(*base).map(Value::I)
        }
        (Some(Value::F32(base)), Some(Value::F32(step))) => {
            let mut value = *base;
            for _ in 0..n {
                value += *step;
            }
            Some(Value::F32(value))
        }
        _ => None,
    }
}

fn field(name: &'static str, value: Value) -> (Cow<'static, str>, Value) {
    (Cow::Borrowed(name), value)
}

fn field_owned(name: String, value: Value) -> (Cow<'static, str>, Value) {
    (Cow::Owned(name), value)
}

fn ensure(item: &mut Value, name: &'static str, to: Value) {
    if let Some(field) = item.field_mut(name) {
        *field = to;
    } else if let Value::Struct(fields) = item {
        fields.push(field(name, to));
    }
}

fn ensure_owned(item: &mut Value, name: Cow<'static, str>, to: Value) {
    if let Some(field) = item.field_mut(&name) {
        *field = to;
    } else if let Value::Struct(fields) = item {
        fields.push((name, to));
    }
}

fn remove(item: &mut Value, name: &str) {
    if let Value::Struct(fields) = item.get_mut() {
        fields.retain(|(n, _)| n != name);
    }
}
