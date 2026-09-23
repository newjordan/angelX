//! The HUD band over the map, Zelda-style: where and what on top, the
//! counters, the B/A item slots, the quest bar and -LIFE- (context left).
//!
//! There is no minimap: the world below is the map.

use super::ink::Img;
use super::kit::{self, Tool};
use super::scene::Scene;

/// Band height in pixels.
pub(crate) const HUD_H: i32 = 64;

pub(crate) fn draw(f: &mut Img, scene: &Scene) {
    // Line 1: the town, the live place, the work.
    let mut x = 5;
    x = f.text(x, 3, &scene.town.to_uppercase(), '5');
    if let Some(place) = scene.active {
        x = f.text(x, 3, " · ", 'G');
        x = f.text(x, 3, place.label(), '2');
    }
    x = f.text(x, 3, " · ", 'G');
    let room = ((f.w - x - 4) / 6).max(0) as usize;
    let activity: String = scene.activity.to_uppercase().chars().take(room).collect();
    f.text(x, 3, &activity, '9');

    // Counters: renown, verified wins, quest round.
    let cx = 8;
    f.stamp(&kit::gem(), cx, 17);
    f.text(cx + 8, 17, &format!("×{}", scene.renown), '9');
    f.stamp(&kit::key(), cx, 28);
    f.text(cx + 8, 28, &format!("×{}", scene.verified), '9');
    f.stamp(&kit::icon(Tool::Scroll), cx, 40);
    let round = scene.quest.map_or(0, |(i, _)| i);
    f.text(
        cx + 8,
        40,
        &format!("×{round}"),
        if scene.quest.is_some() { '9' } else { 'G' },
    );

    // B: what the knight holds. A: the sword — the model.
    for (bx, label, item) in [(58, "B", scene.tool), (82, "A", Some(Tool::Sword))] {
        f.frame(bx, 18, 20, 30, '1');
        f.rect(bx + 7, 15, 7, 7, 'k');
        f.text(bx + 8, 15, label, '9');
        if let Some(tool) = item {
            let icon = kit::icon(tool);
            f.stamp(&icon, bx + (20 - icon.w) / 2, 22 + (24 - icon.h) / 2);
        }
    }

    // The quest: a loop's rounds as a segmented bar.
    let qx = 112;
    match scene.quest {
        Some((round, max)) => {
            f.text(qx, 17, "-QUEST-", '2');
            let segs = max.clamp(1, 12) as i32;
            let lit = ((round.min(max) as f32 / max.max(1) as f32) * segs as f32).round() as i32;
            let seg_w = (58 - (segs - 1)) / segs;
            for s in 0..segs {
                let ink = if s < lit { '2' } else { 'K' };
                f.rect(qx + s * (seg_w + 1), 29, seg_w, 5, ink);
            }
            f.text(qx, 40, &format!("{round}/{max}"), '2');
        }
        None => {
            f.text(qx, 17, "-QUEST-", 'G');
            f.text(qx, 29, "AT REST", 'G');
        }
    }

    // -LIFE-: the context window still free.
    f.text(186, 16, "-LIFE-", '7');
    let halves = (scene.hud.ctx_free.min(100) * 16 + 50) / 100;
    for i in 0..8u32 {
        let fill = if halves >= (i + 1) * 2 {
            2
        } else if halves == i * 2 + 1 {
            1
        } else {
            0
        };
        f.stamp(&kit::heart(fill), 180 + i as i32 * 9, 29);
    }
    f.text(
        180,
        40,
        &format!("CTX {}%", 100 - scene.hud.ctx_free.min(100)),
        'G',
    );

    // Line 2: the route.
    let mut x = 5;
    if !scene.hud.model.is_empty() {
        x = f.text(x, 55, &scene.hud.model.to_uppercase(), '9');
    }
    if !scene.hud.think.is_empty() {
        x = f.text(x, 55, " · THINK ", 'G');
        f.text(x, 55, &scene.hud.think.to_uppercase(), 'h');
    }
}
