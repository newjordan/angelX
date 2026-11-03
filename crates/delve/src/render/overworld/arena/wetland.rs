//! The abbey's drowned cloister, absurd sluice valves, and a shield on a wave.
use super::super::ink::{Img, hash};
use super::super::material as m;
use super::{TILE, at, home};
use crate::rules::{
    RoomKind, Run,
    pastimes::{self, Game},
    wetland as wet,
};

fn ring(cv: &mut Img, x: i32, y: i32, ink: char) {
    for n in 0..40 {
        let a = n as f32 * std::f32::consts::TAU / 40.0;
        cv.put(
            x + (a.cos() * 11.0) as i32,
            y + (a.sin() * 11.0) as i32,
            ink,
        );
    }
}
pub(super) fn scenery(cv: &mut Img, run: &Run) {
    match run.room().kind {
        RoomKind::Millpond => {
            // Two pond wings joined under a dry stone bridge.
            m::ashlar(cv, (11 * TILE, 4 * TILE, 2 * TILE, 5 * TILE), &['K', 'X', 'g', 'j', 'G', 'J'], 180);
            for x in [11 * TILE, 13 * TILE - 2] {
                m::block(cv, (x, 4 * TILE - 4, 2, 5 * TILE + 4), m::ramp::STONE, false, 181);
            }
            m::sign(cv, 6 * TILE, 4 * TILE - 13, "ROYAL DOG-PADDLE", 'T');
            cv.text(6 * TILE, 10 * TILE, "NO KING-SIZED SPLASHES", 'o');
        }
        RoomKind::Sluice => {
            for (_, (x, y)) in &wet::ERRAND[1..4] {
                // The valve's stone footing; the valve itself is live ink.
                let (x, y) = at(x * 2.0, y * 2.0);
                m::block(cv, (x - 7, y - 3, 14, 7), m::ramp::STONE, false, 182);
            }
            m::sign(cv, 7 * TILE, 11 * TILE, "ROYAL BATH: CANCELLED", 'T');
        }
        RoomKind::SunkenAbbey => {
            // The choir's columns and arches stand up out of the water.
            for (k, x) in [3, 5, 7, 8].into_iter().enumerate() {
                let x = x * TILE + 3;
                m::block(cv, (x, 2 * TILE - 10, 7, 3 * TILE + 2), m::ramp::STONE, false, 183 + k as u32);
                m::block(cv, (x - 2, 2 * TILE - 13, 11, 4), m::ramp::STONE, false, 187);
                for y in (2 * TILE..5 * TILE - 6).step_by(5) {
                    if hash(k as i32, y, 188) % 3 == 0 {
                        cv.put(x + 1, y, 'l');
                        cv.put(x + 1, y + 1, 'E');
                    }
                }
            }
            m::sign(cv, 3 * TILE + 7, 5 * TILE + 2, "SUNKEN CHOIR", 'T');
            m::sign(cv, 3 * TILE, 10 * TILE + 6, "BELL NOW RINGS FOR LUNCH", 'T');
        }
        _ => {}
    }
}
pub(super) fn figures(cv: &mut Img, run: &Run) {
    let s = wet::stage(&run.home.countryside, &run.marks);
    for (i, &(kind, (x, y))) in wet::ERRAND[..5].iter().enumerate() {
        if kind != run.room().kind {
            continue;
        }
        let (x, y) = at(x * 2.0, y * 2.0);
        let ink = if usize::from(s) == i || (i == 0 && s == 5) {
            'o'
        } else if usize::from(s) > i {
            'g'
        } else {
            'h'
        };
        if !(1..4).contains(&i) {
            ring(cv, x, y, ink);
        }
        if i == 0 {
            cv.rect(x - 6, y - 18, 12, 16, 'b');
            cv.rect(x - 4, y - 24, 8, 7, 'u');
            cv.text(x - 22, y - 33, "B. BUCKET", 'O');
        } else if i == 4 {
            cv.rect(x - 7, y - 20, 14, 10, 'o');
            cv.line(x - 10, y - 9, x + 10, y - 9, 'O');
            cv.rect(x - 2, y - 8, 4, 4, 'O');
            cv.text(x - 8, y - 32, "BELL", 'O');
        } else {
            valve(cv, run, x, y, i - 1, s);
        }
    }
    let Some(r) = run.pastimes.round.as_ref().filter(|r| r.game == Game::Surf) else {
        return;
    };
    let Some(surf) = r.surf.as_ref() else {
        return;
    };
    let wave = usize::from(r.progress);
    let x = 8 * TILE;
    let y = 6 * TILE;
    let crest = surf.phase >= pastimes::WAVE_TICKS - pastimes::CREST_WINDOW;
    // Large wave sweeps toward the shield; F window changes cyan to gold.
    let wx = 10 * TILE - (surf.phase * 3 * TILE as u32 / pastimes::WAVE_TICKS) as i32;
    for n in 0..4 {
        cv.line(
            wx - n * 3,
            y - 18 + n * 7,
            wx + 12 - n * 3,
            y - 23 + n * 7,
            if crest { 'O' } else { 'C' },
        );
    }
    // Wave crest glow: gold ink overlay + warm halo during brace window.
    if surf.crest_glow > 0 {
        for n in 0..4 {
            cv.line(
                wx - n * 3,
                y - 18 + n * 7,
                wx + 12 - n * 3,
                y - 23 + n * 7,
                'O',
            );
        }
        super::fx::glow(cv, wx + 3, y - 14, super::fx::Hue::Warm);
    }
    cv.line(x - 14, y + 10, x + 15, y + 10, 'C');
    cv.line(x - 9, y + 5, x + 9, y + 5, 'o');
    cv.line(x - 7, y + 6, x + 7, y + 9, 'O');
    let lean = surf.lean / 8;
    // Surfboard (elongated ellipse)
    cv.ellipse(x as f32, (y + 8) as f32, 6.0, 2.0, 'n');
    // Body / torso
    cv.rect(x + lean - 2, y - 6, 4, 6, 'B');
    // Round head
    cv.ellipse((x + lean) as f32, (y - 8) as f32, 2.0, 2.0, 'h');
    // Hair / helmet
    cv.ellipse((x + lean) as f32, (y - 9) as f32, 2.0, 1.0, 'k');
    // Eye (face direction)
    cv.put(x + lean + 1, y - 8, 'k');
    // Legs
    cv.line(x + lean - 1, y, x + lean - 2, y + 6, 'h');
    cv.line(x + lean + 1, y, x + lean + 2, y + 6, 'h');
    // Arms with hands
    cv.line(x + lean - 2, y - 4, x + lean - 5, y - 2, 'h');
    cv.line(x + lean + 2, y - 4, x + lean + 5, y - 2, 'h');
    cv.put(x + lean - 5, y - 2, 'h');
    cv.put(x + lean + 5, y - 2, 'h');
    // Water spray particles behind the surfboard while balanced.
    if surf.spray > 0 {
        let spray_ink = ['C', 'V', 'z'];
        for i in 0..6 {
            let seed = hash(x + i, y, surf.phase + i as u32);
            let dx = -8 - (seed % 12) as i32;
            let dy = 8 + (seed % 5) as i32 - 2;
            let ink = spray_ink[(seed % 3) as usize];
            cv.put(x + lean + dx, y + dy, ink);
        }
        // Wake trail behind the surfboard.
        for i in 0..4 {
            let tx = x - 10 - i * 4;
            let ty = y + 10 + (i % 2);
            cv.put(tx, ty, if i < 2 { 'C' } else { 'V' });
        }
    }
    // Bright flash burst on successful brace.
    if surf.brace_flash > 0 {
        let flash_ink = if surf.brace_flash > 2 { '9' } else { 'c' };
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::TAU / 8.0;
            let r = (4 - surf.brace_flash as i32).max(1) * 3;
            let fx = x + lean + (a.cos() * r as f32) as i32;
            let fy = y - 10 + (a.sin() * r as f32) as i32;
            cv.put(fx, fy, flash_ink);
        }
        super::fx::glow(cv, x + lean, y - 10, super::fx::Hue::Warm);
        // Spray burst radiating outward from brace point.
        for i in 0..6 {
            let seed = hash(x + i, y, surf.brace_flash as u32 + i as u32);
            let dx = (seed % 16) as i32 - 8;
            let dy = (seed % 10) as i32 - 5;
            cv.put(x + lean + dx, y - 10 + dy, 'V');
        }
    }
    // Expanding splash ring on wipeout.
    if surf.splash > 0 {
        let splash_radius = 14 - surf.splash as i32;
        let splash_ink = if surf.splash > 8 { 'C' } else if surf.splash > 4 { 'V' } else { 'z' };
        for i in 0..16 {
            let a = i as f32 * std::f32::consts::TAU / 16.0;
            let sx = x + (a.cos() * splash_radius as f32) as i32;
            let sy = y + 5 + (a.sin() * splash_radius as f32 * 0.5) as i32;
            cv.put(sx, sy, splash_ink);
        }
        // Splash droplets flying upward.
        for i in 0..4 {
            let seed = hash(x + i, y, surf.splash as u32 + i as u32);
            let dx = (seed % 20) as i32 - 10;
            let dy = -(surf.splash as i32) - (seed % 8) as i32;
            cv.put(x + dx, y + 5 + dy, 'V');
        }
    }
    // A balance gauge and the upcoming direction remain below the top board.
    super::super::chrome::inset_panel(cv, (6 * TILE, 10 * TILE, 12 * TILE, 12), super::super::chrome::Tone::WELL.rimmed(super::super::chrome::COLD_RIM));
    cv.line(7 * TILE, 10 * TILE + 6, 17 * TILE, 10 * TILE + 6, 'h');
    let target = 12 * TILE + pastimes::WAVE_LEANS[wave] * TILE / 20;
    super::super::chrome::shaped(cv, (target - 13, 10 * TILE + 2, 27, 9), &[1], super::super::chrome::Tone::KEYCAP, false);
    let current = 12 * TILE + surf.lean * TILE / 20;
    // Lean indicator pulse: brightens when in target zone.
    let in_zone = (surf.lean - pastimes::WAVE_LEANS[wave]).abs() <= 16;
    let indicator_ink = if in_zone && (surf.phase / 2) % 2 == 0 { '9' } else { 'O' };
    cv.rect(current - 1, 10 * TILE, 3, 12, indicator_ink);
    cv.text(
        7 * TILE,
        11 * TILE,
        if crest {
            "TAP F NOW!"
        } else {
            "ALIGN GOLD WITH GREEN"
        },
        'O',
    );
}
/// A sluice valve: a wheel on a post, its creature on top, and while it
/// stands open, a spring-bar running down to SPROING.
fn valve(cv: &mut Img, run: &Run, x: i32, y: i32, which: usize, s: u8) {
    let already = usize::from(s.saturating_sub(1)).min(3);
    let left = run.valves.map_or(0, |v| v[which]);
    let open = which < already || s >= 4 || left > 0;
    let ink = if open { 'O' } else { 'h' };
    cv.rect(x - 1, y - 8, 3, 9, 'b');
    // The wheel: spokes turned a notch while open.
    let (a, b) = if open { ((5, 5), (5, -5)) } else { ((7, 0), (0, 7)) };
    cv.line(x - a.0, y - 10 - a.1, x + a.0, y - 10 + a.1, ink);
    cv.line(x - b.0, y - 10 - b.1, x + b.0, y - 10 + b.1, ink);
    for n in 0..24 {
        let t = n as f32 * std::f32::consts::TAU / 24.0;
        cv.put(x + (t.cos() * 7.0) as i32, y - 10 + (t.sin() * 7.0) as i32, ink);
    }
    // Its creature, sat on top.
    let top = y - 20;
    match which {
        0 => {
            cv.ellipse(x as f32, top as f32, 4.0, 2.0, 'U');
            cv.line(x + 4, top, x + 7, top - 2, 'U');
            cv.line(x + 4, top, x + 7, top + 2, 'U');
            cv.put(x - 2, top - 1, 'k');
        }
        1 => {
            cv.rect(x - 3, top - 4, 4, 5, 'B');
            cv.rect(x - 3, top, 8, 2, 'B');
        }
        _ => {
            cv.ellipse(x as f32, top as f32, 4.0, 2.5, 'T');
            cv.rect(x - 4, top - 5, 3, 3, 'T');
            cv.put(x - 5, top - 4, '@');
        }
    }
    cv.text(x - super::super::ink::text_width(wet::VALVES[which]) / 2, y + 4, wet::VALVES[which], ink);
    if left > 0 {
        let w = (left * 20 / wet::VALVE_OPEN) as i32;
        cv.rect(x - 10, y + 13, 20, 3, 'k');
        cv.rect(x - 10, y + 13, w, 3, if left < 2 * crate::rules::HZ { '7' } else { 'O' });
    }
}

pub(super) fn board_visible(run: &Run) -> bool {
    let s = wet::stage(&run.home.countryside, &run.marks);
    if s == 6
        || run
            .pastimes
            .round
            .as_ref()
            .is_some_and(|r| r.game == Game::Surf)
    {
        return false;
    }
    if (1..4).contains(&s) {
        return run.room().kind == RoomKind::Sluice;
    }
    let (kind, (x, y)) = wet::ERRAND[usize::from(s)];
    kind == run.room().kind
        && run.players.values().any(|h| {
            h.hp > 0 && !h.stone && h.privy == 0 && (h.x - x * 2.0).hypot(h.y - y * 2.0) < 5.0
        })
}
pub(super) fn boards(cv: &mut Img, run: &Run) {
    if !board_visible(run) {
        return;
    }
    let s = wet::stage(&run.home.countryside, &run.marks);
    let lines = match s {
        0 => [("BUCKET'S BELL", '9'), ("E: TAKE JOB", 'O')],
        1..=3 => [("ALL THREE OPEN", '9'), ("E: TURN", 'O')],
        4 => [("THE BELL", '9'), ("E: FISH OUT", 'O')],
        _ => [("BUCKET'S BELL", '9'), ("E: GIVE BELL", 'O')],
    };
    let board = home::board(&lines.map(|(t, ink)| (t.to_string(), ink)));
    cv.stamp(&board, 4, 13);
}
