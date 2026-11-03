//! King Cinder's tick, unchanged, as the oracle the leg is held to.
//!
//! This is the code that used to run in `crates/delve/src/rules/bannerpeak/boss.rs`:
//! it moved here verbatim when the tick became Bend (`cockpit/live/bannerpeak-rules`),
//! and it now runs only in tests — `crates/delve`'s own, through an engine that
//! answers from here, and the cockpit's parity test, which ticks the shipped leg
//! and this side by side and compares every state.
//!
//! It is included by `#[path]` from both trees, so it may only name what a
//! `super::` can supply: the boss's types, the summit's geometry, the tile size
//! and serde's JSON.
use super::boss::{Attack, Beat, Boss, Hazard, HEAD_DOWN, HEAD_UP, PHASES, REACH, floor, home};
use super::layout::{SUMMIT, info};
use super::world::TILE;
use serde::{Deserialize, Serialize};

/// The attacks of a crown, in order, before he kneels.
pub fn pattern(phase: u8) -> &'static [Attack] {
    match phase {
        0 => &[Attack::Stomp, Attack::Embers],
        1 => &[Attack::Sweep, Attack::Stomp, Attack::Embers],
        _ => &[Attack::Embers, Attack::Sweep, Attack::Stomp, Attack::Embers],
    }
}

/// One tick. `at` are the feet of the knights standing; `seats` how many sit.
pub fn step(boss: &mut Boss, at: &[(i32, i32)], seats: u8, elapsed: u32) {
    boss.head_before = boss.head;
    boss.x_before = boss.x;
    boss.flinch = boss.flinch.saturating_sub(1);
    for h in &mut boss.hazards {
        if h.tell > 0 {
            h.tell -= 1;
        } else {
            h.x += h.vx;
            h.live = h.live.saturating_sub(1);
        }
    }
    boss.hazards.retain(|h| h.live > 0 || h.tell > 0);
    let arena = |x: i32, y: i32| {
        x > (SUMMIT.ox + 3) * TILE && x < (SUMMIT.ox + 38) * TILE && y > (SUMMIT.oy) * TILE
    };
    let near = at.iter().copied().filter(|&(x, y)| arena(x, y)).collect::<Vec<_>>();
    let crowd = i32::from(seats.max(1)) - 1;
    match boss.beat {
        Beat::Dormant => {
            if !near.is_empty() {
                boss.beat = Beat::Rest;
                boss.t = 60;
            }
        }
        Beat::Down => {
            boss.head = (boss.head - 1).max(HEAD_DOWN - 1);
        }
        Beat::Rage => {
            boss.t = boss.t.saturating_sub(1);
            boss.head = (boss.head + 1).min(HEAD_UP);
            if boss.t == 0 {
                boss.plates = seats.max(1);
                boss.count = 0;
                boss.beat = Beat::Rest;
                boss.t = 40;
            }
        }
        Beat::Rest => {
            boss.t = boss.t.saturating_sub(1);
            boss.head = (boss.head + 1).min(HEAD_UP);
            // Step toward the nearest knight, slowly.
            if let Some(&(tx, _)) = near.iter().min_by_key(|(x, _)| (x - boss.x).abs()) {
                if elapsed % 2 == 0 {
                    boss.x += (tx - boss.x).clamp(-1, 1);
                }
                boss.x = boss.x.clamp(home() - 110, home() + 110);
            }
            if boss.t == 0 {
                let seq = pattern(boss.phase.min(2));
                if usize::from(boss.count) >= seq.len() {
                    boss.beat = Beat::Wince;
                    boss.t = (150 - 20 * i32::from(boss.phase) - 3 * crowd).max(70) as u32;
                } else {
                    boss.kind = seq[usize::from(boss.count)];
                    boss.beat = Beat::Tell;
                    boss.t = match boss.kind {
                        Attack::Stomp => 40,
                        Attack::Sweep => 32,
                        Attack::Embers => 30,
                    } - 3 * u32::from(boss.phase);
                    warn(boss, &near, seats, elapsed);
                }
            }
        }
        Beat::Tell => {
            boss.t = boss.t.saturating_sub(1);
            if boss.t == 0 {
                strike(boss);
                boss.beat = Beat::Strike;
                boss.t = match boss.kind {
                    Attack::Stomp => 22,
                    Attack::Sweep => 14,
                    Attack::Embers => 12,
                };
            }
        }
        Beat::Strike => {
            boss.t = boss.t.saturating_sub(1);
            if boss.t == 0 {
                boss.count += 1;
                boss.beat = Beat::Rest;
                boss.t = (44 - 6 * u32::from(boss.phase) - 3 * crowd as u32).max(16);
            }
        }
        Beat::Wince => {
            boss.t = boss.t.saturating_sub(1);
            boss.head = (boss.head - 2).max(HEAD_DOWN);
            if boss.t == 0 {
                boss.count = 0;
                boss.beat = Beat::Rest;
                boss.t = 30;
            }
        }
    }
}

/// The warning an attack gives: where it will be.
fn warn(boss: &mut Boss, near: &[(i32, i32)], seats: u8, elapsed: u32) {
    let f = floor();
    let tell = boss.t;
    match boss.kind {
        Attack::Stomp => {}
        Attack::Sweep => {
            let toward = near
                .iter()
                .min_by_key(|(x, _)| (x - boss.x).abs())
                .map_or(1, |&(x, _)| if x >= boss.x { 1 } else { -1 });
            let (l, w) = if toward > 0 {
                (boss.x + 6, REACH)
            } else {
                (boss.x - 6 - REACH, REACH)
            };
            boss.hazards.push(Hazard {
                x: l,
                y: f - 22,
                w,
                h: 22,
                vx: 0,
                tell,
                live: 14,
            });
        }
        Attack::Embers => {
            // A column on a knight, more of them for more seats and later crowns.
            let columns = 1 + i32::from(boss.phase) + i32::from(seats.saturating_sub(1)) / 2;
            let mut aims: Vec<i32> = near.iter().map(|&(x, _)| x).collect();
            aims.sort_unstable();
            let top = (SUMMIT.oy + 1) * TILE;
            for c in 0..columns {
                let pick = if aims.is_empty() {
                    home()
                } else {
                    aims[((elapsed as i32 + c * 3) as usize) % aims.len()]
                };
                let x = (pick + (c % 3 - 1) * 26)
                    .clamp((SUMMIT.ox + 4) * TILE, (SUMMIT.ox + 36) * TILE);
                boss.hazards.push(Hazard {
                    x: x - 10,
                    y: top,
                    w: 20,
                    h: f - top,
                    vx: 0,
                    tell,
                    live: 12,
                });
            }
        }
    }
}

/// The blow lands.
fn strike(boss: &mut Boss) {
    if boss.kind == Attack::Stomp {
        let f = floor();
        let n = 1 + i32::from(boss.phase >= 1);
        for k in 0..n {
            for dir in [-1, 1] {
                // The second wave follows the first.
                boss.hazards.push(Hazard {
                    x: boss.x - 8 + dir * k * 40,
                    y: f - 14,
                    w: 16,
                    h: 14,
                    vx: dir * 3,
                    tell: (k * 14) as u32,
                    live: 70,
                });
            }
        }
    }
}

/// A slam landed with its feet at `(x, y)` pixels: whether a plate fell.
pub fn slam(boss: &mut Boss, x: i32, y: i32) -> bool {
    let Some(c) = boss.crown() else {
        return false;
    };
    if boss.plates > 0 && (x - boss.x).abs() <= 24 && (y - c.3).abs() <= 3 {
        boss.plates -= 1;
        boss.flinch = 14;
        if boss.plates == 0 {
            fall(boss);
        }
        return true;
    }
    false
}

/// A crown falls: the next, or the last of them.
fn fall(boss: &mut Boss) {
    boss.phase += 1;
    boss.hazards.clear();
    if boss.phase >= PHASES {
        boss.beat = Beat::Down;
        boss.t = 0;
    } else {
        boss.beat = Beat::Rage;
        boss.t = 54;
    }
}

/// The leg's own wire, as the adapter sends it: a tick of the fight.
#[derive(Serialize, Deserialize)]
pub struct Ask {
    pub elapsed: u32,
    pub seats: u8,
    pub feet: Vec<(i32, i32)>,
    pub boss: Boss,
}

/// The leg's own wire back: the fight after the tick.
#[derive(Serialize)]
pub struct Next {
    pub op: &'static str,
    pub boss: Boss,
}

/// A slam, with the feet it landed at.
#[derive(Serialize, Deserialize)]
pub struct AskSlam {
    pub x: i32,
    pub y: i32,
    pub boss: Boss,
}

/// The slam's answer.
#[derive(Serialize)]
pub struct Slammed {
    pub op: &'static str,
    pub hit: bool,
    pub boss: Boss,
}

/// The oracle's engine: the same two ops the leg answers, answered natively.
pub fn answer(id: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
    if id != "bannerpeak-rules" {
        return Err(format!("no oracle for {id}"));
    }
    match input.get("op").and_then(|op| op.as_str()) {
        Some("boss") => {
            let ask: Ask = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
            let mut boss = ask.boss;
            step(&mut boss, &ask.feet, ask.seats, ask.elapsed);
            serde_json::to_value(Next { op: "boss", boss }).map_err(|e| e.to_string())
        }
        Some("slam") => {
            let ask: AskSlam = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
            let mut boss = ask.boss;
            let hit = slam(&mut boss, ask.x, ask.y);
            serde_json::to_value(Slammed {
                op: "slam",
                hit,
                boss,
            })
            .map_err(|e| e.to_string())
        }
        _ => Err("bannerpeak-rules: the oracle answers boss and slam only".into()),
    }
}
