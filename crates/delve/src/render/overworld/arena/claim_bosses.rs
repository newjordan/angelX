//! Resource guardians: retained paintings with combat-state poses. Their
//! floor warnings are the existing signature renderer's actual hit geometry.
use super::*;
use crate::rules::{Boss, Enemy, claim_bosses::Guardian, signatures as fight};

fn index(g: Guardian) -> usize {
    match g {
        Guardian::Ironroot => 0,
        Guardian::Cindermaw => 1,
        Guardian::Amethyst => 2,
    }
}
fn bodies() -> &'static [Img; 6] {
    static BODIES: std::sync::OnceLock<[Img; 6]> = std::sync::OnceLock::new();
    BODIES.get_or_init(|| {
        [
            include_bytes!(concat!(env!("OUT_DIR"), "/claim-ironroot-colossus.rgba")) as &[u8],
            include_bytes!(concat!(env!("OUT_DIR"), "/claim-cindermaw.rgba")),
            include_bytes!(concat!(
                env!("OUT_DIR"),
                "/claim-amethyst-crowned-wyrm.rgba"
            )),
            include_bytes!(concat!(
                env!("OUT_DIR"),
                "/claim-ironroot-colossus-wind.rgba"
            )),
            include_bytes!(concat!(env!("OUT_DIR"), "/claim-cindermaw-wind.rgba")),
            include_bytes!(concat!(
                env!("OUT_DIR"),
                "/claim-amethyst-crowned-wyrm-wind.rgba"
            )),
        ]
        .map(|raw| {
            let mut im = Img::new(64, 64);
            for (i, p) in raw.chunks_exact(4).enumerate() {
                if p[3] >= 100 {
                    im.set(i as i32 % 64, i as i32 / 64, [p[0], p[1], p[2]]);
                }
            }
            im
        })
    })
}
pub(super) fn body(id: &str) -> Option<Img> {
    Some(bodies()[index(Guardian::for_id(id)?)].clone())
}

pub(super) fn figure(run: &Run, enemy: &Enemy, boss: &Boss, tick: u32) -> Option<Img> {
    let guardian = Guardian::for_id(&boss.id)?;
    let wind = fight::winding(enemy, boss);
    let charging = enemy.stage == fight::WIND;
    let source = &bodies()[index(guardian) + if charging { 3 } else { 0 }];
    let progress = if charging {
        wind.map_or(0.0, |w| enemy.timer as f32 / w.wind.max(1) as f32)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let resting = enemy.stage == fight::DAZED;
    let phase = boss.stage(enemy.hp, enemy.max_hp);
    let pulse = (tick as f32 * 0.13).sin();
    let (sx, sy) = if charging {
        (1.0 + progress * 0.10, 1.0 - progress * 0.13)
    } else if enemy.stage == fight::DASH {
        (1.12, 0.89)
    } else if resting {
        (1.04, 0.93)
    } else {
        (1.0, 1.0 + 0.015 * pulse)
    };
    let rise = if enemy.stage == fight::JUMP {
        ((enemy.timer as f32 / fight::FLIGHT as f32 * std::f32::consts::PI).sin() * 18.0).max(0.0)
            as i32
    } else {
        0
    };
    let (w, h) = ((64.0 * sx).ceil() as i32, (64.0 * sy).ceil() as i32);
    let mut im = Img::new(w + 6, h + rise + 3);
    let facing = run
        .players
        .values()
        .filter(|h| h.hp > 0 && !h.absent)
        .min_by(|a, b| {
            (a.x - enemy.x)
                .hypot(a.y - enemy.y)
                .total_cmp(&(b.x - enemy.x).hypot(b.y - enemy.y))
        })
        .is_some_and(|h| h.x > enemy.x);
    for y in 0..h {
        for x in 0..w {
            // Tail and shoulder motion is tied to the actual attack stage;
            // feet stay planted while the chest breathes and the crown hums.
            let row = (y as f32 / sy).min(63.0) as i32;
            let sway = if guardian == Guardian::Amethyst && !charging && !resting {
                ((row as f32 / 14.0 + tick as f32 * 0.075).sin() * 1.4) as i32
            } else if charging && progress > 0.65 {
                (tick % 2) as i32
            } else {
                0
            };
            let col = (x as f32 / sx).min(63.0) as i32;
            let col = if facing { 63 - col } else { col };
            if let Some(mut c) = source.get(col, row) {
                let luminous = match guardian {
                    Guardian::Ironroot | Guardian::Cindermaw => {
                        c[0] > 165 && c[0] > c[2].saturating_add(55)
                    }
                    Guardian::Amethyst => c[2] > 145 && c[2] > c[1].saturating_add(35),
                };
                if luminous && (charging || phase == crate::rules::bosses::Stage::Rage) {
                    let lift = (15.0 + 20.0 * progress + 9.0 * pulse) as u8;
                    c = c.map(|v| v.saturating_add(lift));
                }
                im.set(x + 3 + sway, y, c);
            }
        }
    }
    if resting {
        // The window to retaliate is visible even without reading a tooltip.
        let orbit = (tick / 5 % 4) as i32;
        for (dx, dy) in [(-12, 1), (0, 0), (12, 2)] {
            im.put(w / 2 + dx + orbit - 2, 3 + dy, '9');
            im.put(w / 2 + dx + orbit - 2, 2 + dy, 'y');
        }
    }
    Some(im)
}

#[cfg(test)]
#[path = "../../../../tests/render/claim_bosses__shots.rs"]
mod tests;
