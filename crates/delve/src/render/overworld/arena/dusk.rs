//! The dusk over a room: firelight pools on black paper, and the light
//! laid row by row so a room can be lit in stages (`DungeonBake`).

use super::*;

pub(super) fn fire(x: i32, y: i32, r: f32, s: f32) -> Light {
    Light {
        x: x as f32,
        y: y as f32,
        r,
        s,
        fire: true,
    }
}

pub(super) fn dusk(cv: &mut Img, lights: &[Light], pool: (char, char)) {
    dusk_with(cv, lights, pool, 0.0);
}

pub(super) fn dusk_with(cv: &mut Img, lights: &[Light], pool: (char, char), lift: f32) {
    super::super::material::relief(cv);
    let solid: Vec<bool> = if lift > 0.0 {
        (0..cv.h * cv.w)
            .map(|i| {
                let (x, y) = (i % cv.w, i / cv.w);
                let inked = |dx: i32, dy: i32| cv.get(x + dx, y + dy).is_some_and(|c| c != BLACK);
                inked(0, 0)
                    && [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (1, -1), (-1, 1)]
                        .iter()
                        .filter(|&&(dx, dy)| inked(dx, dy))
                        .count()
                        >= 6
            })
            .collect()
    } else {
        Vec::new()
    };
    let h = cv.h;
    light_rows(cv, lights, pool, lift, &solid, 0..h);
}

pub(super) fn light_rows(cv: &mut Img, lights: &[Light], (cool, lit): (char, char), lift: f32, solid: &[bool], rows: std::ops::Range<i32>) {
    static DAPPLE: OnceLock<Vec<f32>> = OnceLock::new();
    static WARM: OnceLock<super::super::ink::RgbMap<Rgb>> = OnceLock::new();
    let dapple = DAPPLE.get_or_init(|| {
        (0..NATIVE_W * NATIVE_H)
            .map(|i| {
                let (x, y) = ((i % NATIVE_W) as f32 + 0.5, (i / NATIVE_W) as f32 + 0.5);
                (vnoise(x / 48.0, y / 48.0, 77) - 0.5) * super::super::light::DAPPLE
            })
            .collect()
    });
    let warmed = WARM.get_or_init(|| {
        let lum = |c: Rgb| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
        PALETTE
            .iter()
            .map(|&v| {
                let (c, w) = (rgb(v), warm(rgb(v)));
                (c, if lum(w) + 6.0 < lum(c) { c } else { w })
            })
            .collect()
    });
    let mut near: Vec<Light> = Vec::with_capacity(lights.len());
    for y in rows {
        let py = y as f32 + 0.5;
        near.clear();
        near.extend(lights.iter().filter(|li| ((py - li.y) * 1.2).abs() < li.r).copied());
        for x in 0..cv.w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (local, fire) = light_at(&near, px, py);
            let level = (DUSK
                + dapple[(y.rem_euclid(NATIVE_H) * NATIVE_W + x.rem_euclid(NATIVE_W)) as usize]
                + local)
                .min(1.0);
            match cv.get(x, y) {
                Some(c) if c != BLACK => {
                    let th = seam(x, y);
                    let c = if fire > 0.16 + th * 0.24 {
                        let w = warmed.get(&c).copied().unwrap_or(c);
                        if fire > 0.18 + th * 0.08 { super::super::material::lighter(w) } else { w }
                    } else {
                        c
                    };
                    let mass = lift > 0.0 && solid[(y * cv.w + x) as usize];
                    let v = ((1.0 - level) * STEPS - if mass { lift } else { 0.0 }).max(0.0);
                    let t = ((v.fract() - 0.5) * super::super::light::SEAM_EDGE + 0.5).clamp(0.0, 1.0);
                    let steps = (v.floor() + if t > th { 1.0 } else { 0.0 }).max(0.0) as usize;
                    cv.set(x, y, step_down(c, steps));
                }
                _ => {
                    let p = (local - 0.22) * 0.55;
                    if p > 0.0 && bayer(x, y) < p {
                        cv.put(x, y, if fire > local * 0.5 { lit } else { cool });
                    }
                }
            }
        }
    }
}

pub(super) fn lights_out(scenery: &mut Img, over: &mut Img, run: &Run, (ox, oy): (i32, i32)) {
    const LANTERN: f32 = 58.0;
    let mut lamps: Vec<(f32, f32, f32)> = run
        .players
        .values()
        .filter(|h| h.hp > 0)
        .map(|h| {
            let (x, y) = at(h.x, h.y);
            ((x - ox) as f32, (y - oy - 6) as f32, LANTERN)
        })
        .collect();
    let room = run.room();
    for (side, &(col, row)) in doors(room).iter().enumerate() {
        if room.doors[side] {
            lamps.push((
                (col * TILE + TILE - ox) as f32,
                (row * TILE + TILE - oy) as f32,
                26.0,
            ));
        }
    }
    let (pw, ph) = room_px(room);
    for (cx, cy) in [(8, 8), (pw - 8, 8), (8, ph - 8), (pw - 8, ph - 8)] {
        lamps.push(((cx - ox) as f32, (cy - oy) as f32, 30.0));
    }
    for y in 0..scenery.h {
        for x in 0..scenery.w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let lit = lamps
                .iter()
                .map(|&(lx, ly, r)| 1.0 - (px - lx).hypot(py - ly) / r)
                .fold(0.0f32, f32::max);
            if lit > 0.35 {
                continue;
            }
            let dark = ((0.35 - lit) * 16.0 + bayer(x, y) * 2.0) as usize;
            if let Some(c) = scenery.get(x, y)
                && c != BLACK
                && !is_signal(c)
            {
                scenery.set(x, y, step_down(c, dark.min(7)));
            }
            if let Some(c) = over.get(x, y)
                && c != BLACK
                && !is_signal(c)
            {
                over.set(x, y, step_down(c, dark.min(4)));
            }
        }
    }
}
