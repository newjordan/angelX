//! Once-per-sprite auto-rig for wished summons, boss stills and whole-body looks.
//!
//! `ANGEL_DELVE_AUTORIG=0` turns the path off. The variable is read once per
//! draw (not once per pixel). A thread-local override exists so a proof test
//! can render both arms in one process.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use super::quick_rig::{self, Clip, Playback, Rig};
use super::*;
use crate::rules::imps::{Behaviour, Imp};
use crate::rules::looks::Sprite;
use crate::rules::{signatures, Enemy, Hero};

thread_local! {
    static FORCE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

static CACHE: OnceLock<Mutex<HashMap<u64, Option<std::sync::Arc<Rig>>>>> = OnceLock::new();
static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static MOTION: OnceLock<Mutex<HashMap<u64, (f32, f32, f32)>>> = OnceLock::new();

/// Test seam. `None` follows the environment.
pub fn force_enabled(v: Option<bool>) {
    FORCE.with(|c| c.set(v));
}

pub fn enabled() -> bool {
    if let Some(v) = FORCE.with(|c| c.get()) {
        return v;
    }
    std::env::var_os("ANGEL_DELVE_AUTORIG").is_none_or(|v| v != "0")
}

pub fn rig_attempts() -> u64 {
    ATTEMPTS.load(Ordering::Relaxed)
}

/// Clip choice from an actor's state. Dying (when the rules give a window)
/// beats a hit, which beats a boss wind-up, which beats a strike, which
/// beats walking.
#[derive(Clone, Debug, PartialEq)]
pub struct ActorState {
    pub moving: bool,
    pub stride: f32,
    pub striking: Option<(u32, u32)>,
    pub hurt: Option<(u32, u32)>,
    pub dying: Option<u64>,
    pub windup: Option<(u32, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tell {
    Idle,
    Walk { stride: f32 },
    Slash { left: u32, total: u32 },
    Hurt { left: u32, total: u32 },
    Die { age: u64 },
    Overhead { left: u32, total: u32 },
}

pub fn choose(state: &ActorState) -> Tell {
    if let Some(age) = state.dying {
        return Tell::Die { age };
    }
    if let Some((left, total)) = state.hurt {
        return Tell::Hurt { left, total: total.max(1) };
    }
    if let Some((left, total)) = state.windup {
        return Tell::Overhead { left, total: total.max(1) };
    }
    if let Some((left, total)) = state.striking {
        return Tell::Slash { left, total: total.max(1) };
    }
    if state.moving {
        return Tell::Walk { stride: state.stride };
    }
    Tell::Idle
}

pub fn clip_name(tell: &Tell) -> &'static str {
    match tell {
        Tell::Idle => "idle",
        Tell::Walk { .. } => "walk",
        Tell::Slash { .. } => "slash",
        Tell::Hurt { .. } => "hurt",
        Tell::Die { .. } => "die",
        Tell::Overhead { .. } => "overhead",
    }
}

/// A turret that just loosed a bolt slashes for six ticks (its rearm is 24
/// the tick it fires, matching `TURRET_EVERY`). A swarm or guard with a
/// stride walks. There is no death window on a summon: expiry draws as today.
pub fn minion_state(imp: &Imp, stride: f32) -> ActorState {
    const EVERY: u32 = 24;
    const STRIKE: u32 = 6;
    let striking = if imp.behaviour == Behaviour::Turret
        && imp.rearm > EVERY.saturating_sub(STRIKE)
        && imp.rearm <= EVERY
    {
        Some((imp.rearm - (EVERY - STRIKE), STRIKE))
    } else {
        None
    };
    let moving = striking.is_none() && imp.behaviour != Behaviour::Turret && stride > 0.04;
    ActorState {
        moving,
        stride,
        striking,
        hurt: None,
        dying: None,
        windup: None,
    }
}

/// Walk / attack tell / hurt / boss wind-up from the foe's real fields.
/// A death window is not stored on the enemy (the burst is a separate
/// effect), so `dying` stays empty unless a caller sets it.
pub fn foe_state(enemy: &Enemy, wind: Option<u32>, stride: f32) -> ActorState {
    let windup = if enemy.stage == signatures::WIND {
        let total = wind.unwrap_or(1).max(1);
        Some((total.saturating_sub(enemy.timer).max(1), total))
    } else {
        None
    };
    let striking = if windup.is_none() && enemy.stage == signatures::TELL {
        let total = 33u32;
        Some((total.saturating_sub(enemy.timer).max(1), total))
    } else {
        None
    };
    let hurt = if enemy.hurt > 0 {
        Some((
            u32::from(enemy.hurt),
            u32::from(crate::rules::desperate::HIT_FLASH).max(1),
        ))
    } else {
        None
    };
    let moving = hurt.is_none() && windup.is_none() && striking.is_none() && stride > 0.04;
    ActorState {
        moving,
        stride,
        striking,
        hurt,
        dying: None,
        windup,
    }
}

fn ink_span(rows: &[&str]) -> Option<(i32, i32)> {
    let mut x0 = i32::MAX;
    let mut y0 = i32::MAX;
    let mut x1 = -1i32;
    let mut y1 = -1i32;
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            if c == ' ' || c == '.' {
                continue;
            }
            let (x, y) = (x as i32, y as i32);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x1 < 0 {
        return None;
    }
    Some((x1 - x0 + 1, y1 - y0 + 1))
}

/// Frame 0 is a standing figure: at least 12 px of ink tall, 6 wide, and
/// taller than it is wide times 0.6.
pub fn stands(rows: &[&str]) -> bool {
    let Some((w, h)) = ink_span(rows) else {
        return false;
    };
    h >= 12 && w >= 6 && (h as f32) > (w as f32) * 0.6
}

fn hash_rows(rows: &[&str]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for row in rows {
        for b in row.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^ rows.len() as u64
}

fn standing_parts(rows: &[&str]) -> bool {
    let Ok(parts) = quick_rig::auto::figure_parts(rows) else {
        return false;
    };
    parts.head && parts.torso && (parts.leg || parts.plan == "robe")
}

/// Cached rig for rows that qualify. `None` is cached too, so a miss is
/// computed once.
pub fn rigged(rows: &[&str]) -> Option<std::sync::Arc<Rig>> {
    if !enabled() || !stands(rows) {
        return None;
    }
    let key = hash_rows(rows);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().ok().and_then(|g| g.get(&key).cloned()) {
        return hit;
    }
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let built = if standing_parts(rows) {
        quick_rig::auto::auto_rig(rows).ok().map(std::sync::Arc::new)
    } else {
        None
    };
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, built.clone());
    }
    built
}

fn note_motion(key: u64, x: f32, y: f32) -> f32 {
    let map = MOTION.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut guard) = map.lock() else {
        return 0.0;
    };
    let entry = guard.entry(key).or_insert((x, y, 0.0));
    let d = (x - entry.0).hypot(y - entry.1);
    if d > 0.001 && d < 8.0 {
        entry.2 += d;
    }
    entry.0 = x;
    entry.1 = y;
    entry.2
}

fn sample_tell<'a>(rig: &'a Rig, tell: &Tell) -> quick_rig::Sample<'a> {
    match *tell {
        Tell::Idle => rig.sample(Clip::Idle, 0, Playback::Loop, false),
        Tell::Walk { stride } => {
            let dur = rig.duration(Clip::Walk).max(1);
            let phase = ((stride / 3.4).fract() * dur as f32) as u64 % dur;
            rig.sample(Clip::Walk, phase, Playback::Loop, false)
        }
        Tell::Slash { left, total } => rig.action(Clip::Slash, left, total.max(1), false),
        Tell::Hurt { left, total } => rig.action(Clip::Hurt, left, total.max(1), false),
        Tell::Die { age } => {
            let dur = rig.duration(Clip::Die).max(1);
            rig.sample(Clip::Die, age.min(dur - 1), Playback::Once, false)
        }
        Tell::Overhead { left, total } => rig.action(Clip::Overhead, left, total.max(1), false),
    }
}

fn plant(sample: &quick_rig::Sample) -> Img {
    let src = sample.image();
    if src.w <= 0 || src.h <= 0 {
        return Img::new(1, 1);
    }
    let [px, py] = sample.pivot();
    let px = px.clamp(0, src.w - 1);
    let py = py.clamp(0, src.h - 1);
    let mut x0 = src.w;
    let mut y0 = src.h;
    let mut x1 = -1;
    let mut y1 = -1;
    for y in 0..src.h {
        for x in 0..src.w {
            if src.get(x, y).is_some() {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < 0 {
        return Img::new(1, 1);
    }
    x0 = x0.min(px);
    x1 = x1.max(px);
    y0 = y0.min(py);
    y1 = py;
    let half = (px - x0).max(x1 - px);
    let w = half * 2 + 1;
    let h = (py - y0) + 1;
    let mut out = Img::new(w.max(1), h.max(1));
    out.stamp(&src, half - px, -y0);
    out
}

fn fit_box(src: &Img, w: i32, h: i32) -> Img {
    let mut out = Img::new(w.max(1), h.max(1));
    if src.w <= 0 || src.h <= 0 || w <= 0 || h <= 0 {
        return out;
    }
    let sw = (src.w * h / src.h).max(1);
    let ox = (w - sw) / 2;
    for y in 0..h {
        for x in 0..sw {
            let dx = ox + x;
            if dx < 0 || dx >= w {
                continue;
            }
            let sx = x * src.w / sw;
            let sy = y * src.h / h;
            if let Some(c) = src.get(sx, sy) {
                out.set(dx, y, c);
            }
        }
    }
    out
}

/// A wished ink sprite on the Creator's stage: a strike, then a walk, from the tick.
pub(super) fn stage_pose(rows: &[&str], tick: u64) -> Option<Img> {
    if !enabled() {
        return None;
    }
    let phase = tick % 72;
    let tell = if phase < 16 {
        Tell::Slash { left: (16 - phase) as u32, total: 16 }
    } else {
        Tell::Walk { stride: (tick as f32) * 0.4 }
    };
    posed(rows, &tell, false)
}

fn posed(rows: &[&str], tell: &Tell, mirror: bool) -> Option<Img> {
    let rig = rigged(rows)?;
    let sample = sample_tell(&rig, tell);
    let mut frame = plant(&sample);
    if mirror {
        frame = frame.flip_h();
    }
    let like = Img::from_rows(rows);
    Some(fit_box(&frame, like.w.max(1), like.h.max(1)))
}

fn frame0(sprite: &Sprite) -> Option<Vec<&str>> {
    let frame = sprite.frames.first()?;
    if frame.is_empty() {
        return None;
    }
    Some(frame.iter().map(String::as_str).collect())
}

fn same_figure(frames: &[Vec<String>]) -> bool {
    let Some(first) = frames.first() else {
        return false;
    };
    !first.is_empty() && frames.iter().all(|f| f == first)
}

/// Rigged summon, same box as the still, feet on the bottom row. `None`
/// means draw exactly as today (switch off, or the sprite does not qualify).
pub(super) fn summon_img(imp: &Imp, sprite: &Sprite, _tick: u64) -> Option<Img> {
    if !enabled() {
        return None;
    }
    let rows = frame0(sprite)?;
    if !stands(&rows) {
        return None;
    }
    let stride = note_motion(imp_key(imp), imp.x, imp.y);
    let tell = choose(&minion_state(imp, stride));
    posed(&rows, &tell, false)
}

fn imp_key(imp: &Imp) -> u64 {
    let mut h = 0x9e3779b97f4a7c15u64;
    h ^= u64::from(imp.owner);
    h = h.wrapping_mul(0x100000001b3);
    h ^= u64::from(imp.seat);
    h = h.wrapping_mul(0x100000001b3);
    for b in imp.card.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h | (1 << 63)
}

/// Boss `.art` still, posed from the foe that wears this boss index.
pub(super) fn boss_still(run: &Run, index: u8, rows: &[&str]) -> Option<Img> {
    if !enabled() || rows.is_empty() {
        return None;
    }
    let enemy = run.enemies.iter().find(|e| e.boss == Some(index))?;
    let stride = note_motion(u64::from(enemy.id), enemy.x, enemy.y);
    let wind = run
        .bosses
        .get(usize::from(index))
        .and_then(|boss| signatures::winding(enemy, boss))
        .map(|up| up.wind);
    let tell = choose(&foe_state(enemy, wind, stride));
    posed(rows, &tell, enemy.dir.0 < -0.05)
}

/// A single-frame (or repeated-frame) whole-body wish, ready for
/// `with_sprite_rig`. Multi-frame sheets are left to their authoring.
pub(super) fn whole_install<'a>(
    hero: &Hero,
    book: &'a crate::rules::cards::Book,
) -> Option<(&'a Sprite, std::sync::Arc<Rig>)> {
    if !enabled() {
        return None;
    }
    if hero.avatar.is_some()
        || hero.shape.is_some()
        || hero.pit.stop.is_some()
        || hero.possessed.is_some()
        || hero.frog()
    {
        return None;
    }
    let sprite = hero.worn(book).into_iter().rev().find(|s| {
        s.at.as_deref().is_none_or(|at| at == "whole") && same_figure(&s.frames)
    })?;
    let rows = frame0(sprite)?;
    let rig = rigged(&rows)?;
    Some((sprite, rig))
}
