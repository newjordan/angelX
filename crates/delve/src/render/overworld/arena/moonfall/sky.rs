//! The sky over the town: a band of it above the roofs, the Gloam Moon and
//! its face, the light of the hour on everything below, and the dread of
//! the last six hours.
use super::super::super::ink::{BLACK, Img, Rgb, bayer, hash, is_signal, nearest_plain};
use super::labels::Rect;
use super::{Cam, VIEW_H, VIEW_Y};
use crate::rules::moonfall::{
    Session, clock, control,
    state::{Knight, Phase},
};
use std::cell::RefCell;
use std::collections::HashMap;

/// A time of day: the hour, the sky overhead and at the horizon, and the
/// light on the ground (a factor for red, green and blue).
struct Key {
    hour: f32,
    top: [f32; 3],
    horizon: [f32; 3],
    ambient: [f32; 3],
}

const KEYS: [Key; 10] = [
    Key { hour: 0.0, top: [8.0, 10.0, 26.0], horizon: [24.0, 34.0, 64.0], ambient: [0.30, 0.36, 0.60] },
    Key { hour: 5.0, top: [16.0, 24.0, 52.0], horizon: [60.0, 72.0, 100.0], ambient: [0.36, 0.42, 0.64] },
    Key { hour: 6.0, top: [70.0, 96.0, 140.0], horizon: [240.0, 176.0, 112.0], ambient: [0.98, 0.84, 0.74] },
    Key { hour: 8.0, top: [96.0, 146.0, 196.0], horizon: [204.0, 216.0, 224.0], ambient: [1.0, 1.0, 1.0] },
    Key { hour: 12.0, top: [84.0, 152.0, 214.0], horizon: [216.0, 232.0, 236.0], ambient: [1.0, 1.0, 1.0] },
    Key { hour: 16.0, top: [96.0, 144.0, 194.0], horizon: [214.0, 216.0, 208.0], ambient: [1.0, 0.98, 0.92] },
    Key { hour: 18.0, top: [70.0, 66.0, 112.0], horizon: [236.0, 132.0, 72.0], ambient: [1.0, 0.72, 0.62] },
    Key { hour: 20.0, top: [22.0, 28.0, 58.0], horizon: [86.0, 62.0, 84.0], ambient: [0.40, 0.42, 0.66] },
    Key { hour: 22.0, top: [10.0, 14.0, 34.0], horizon: [30.0, 40.0, 72.0], ambient: [0.32, 0.38, 0.62] },
    Key { hour: 24.0, top: [8.0, 10.0, 26.0], horizon: [24.0, 34.0, 64.0], ambient: [0.30, 0.36, 0.60] },
];

/// The hour of the calendar day (0 to 24) a clock reads.
fn hour_of(clock_now: u32, day: u32) -> f32 {
    let m = (u64::from(clock_now) * 1440 / u64::from(day.max(1))) as u32 + 360;
    (m % 1440) as f32 / 60.0
}

fn blend(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn at_hour(h: f32) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let i = KEYS.iter().rposition(|k| k.hour <= h).unwrap_or(0).min(KEYS.len() - 2);
    let (a, b) = (&KEYS[i], &KEYS[i + 1]);
    let t = ((h - a.hour) / (b.hour - a.hour)).clamp(0.0, 1.0);
    (blend(a.top, b.top, t), blend(a.horizon, b.horizon, t), blend(a.ambient, b.ambient, t))
}

/// How far into the Final Hours, 0 to 1.
pub(super) fn dread(s: &Session, shown: u32) -> f32 {
    let fin = clock::final_start(s.nums.day);
    let end = clock::end(s.nums.day);
    if shown < fin || s.completed.is_some() {
        0.0
    } else {
        ((shown - fin) as f32 / (end - fin).max(1) as f32).min(1.0)
    }
}

/// The ambient light factors at the shown clock.
pub(super) fn ambient(s: &Session, shown: u32) -> [f32; 3] {
    let (_, _, mut amb) = at_hour(hour_of(shown, s.nums.day));
    let d = dread(s, shown);
    if d > 0.0 {
        amb = [amb[0] * (1.0 - 0.18 * d), amb[1] * (1.0 - 0.34 * d), amb[2] * (1.0 - 0.30 * d)];
    }
    amb
}

pub(super) fn sky(im: &mut Img, s: &Session, shown: u32, tick: u32) {
    let h = hour_of(shown, s.nums.day);
    let (top, horizon, amb) = at_hour(h);
    for y in 0..VIEW_Y {
        let t = y as f32 / (VIEW_Y - 1) as f32;
        let c = blend(top, horizon, t);
        // A touch of dither between two palette steps keeps the band soft.
        let c = [c[0] + (bayer(0, y) - 0.5) * 8.0, c[1] + (bayer(1, y) - 0.5) * 8.0, c[2] + (bayer(2, y) - 0.5) * 8.0];
        let rgb = nearest_plain(c);
        for x in 0..im.w {
            let jitter = (bayer(x, y) - 0.5) * 6.0;
            let c2 = nearest_plain([c[0] + jitter, c[1] + jitter, c[2] + jitter]);
            im.set(x, y, if (x + y) % 2 == 0 { rgb } else { c2 });
        }
    }
    // Stars when the light is low.
    let dark = 1.0 - amb[0].max(amb[1]).max(amb[2]).clamp(0.3, 1.0);
    if dark > 0.1 {
        for i in 0..40 {
            let (x, y) = ((hash(i, 1, 70) % 384) as i32, (hash(i, 2, 70) % 28) as i32);
            if (tick / 20 + i as u32) % 7 != 0 {
                im.put(x, y, if i % 5 == 0 { 'w' } else { 'V' });
            }
        }
    }
    // The roofs of the town along the horizon.
    let lit = h >= 17.5 || h < 5.5;
    let mut x = 0;
    while x < im.w {
        let hh = hash(x / 12, 5, 71);
        let w = 10 + (hh % 14) as i32;
        let ht = 3 + ((hh >> 8) % 7) as i32;
        im.rect(x, VIEW_Y - ht, w, ht, 'K');
        for k in 0..ht / 2 {
            im.rect(x + k, VIEW_Y - ht - 1 - k, w - 2 * k, 1, 'K');
        }
        if lit && hh % 3 == 0 {
            im.put(x + 3, VIEW_Y - 2, '4');
        }
        x += w + (hh % 5) as i32;
    }
    im.rect(0, VIEW_Y - 1, im.w, 1, 'k');
}

thread_local! {
    static TINTED: RefCell<HashMap<(Rgb, [u8; 3]), Rgb>> = RefCell::new(HashMap::new());
}

fn tinted(c: Rgb, f: [u8; 3]) -> Rgb {
    TINTED.with(|cache| {
        *cache.borrow_mut().entry((c, f)).or_insert_with(|| {
            nearest_plain([
                f32::from(c[0]) * f32::from(f[0]) / 16.0,
                f32::from(c[1]) * f32::from(f[1]) / 16.0,
                f32::from(c[2]) * f32::from(f[2]) / 16.0,
            ])
        })
    })
}

/// A light on the ground: where, and how far it reaches, in screen pixels.
struct Pool {
    x: f32,
    y: f32,
    r: f32,
}

/// The hour's light on the town below the sky, the lantern pools, the
/// moon's shadow and the red edge of the Final Hours. `band` is the sky as
/// it was drawn: whatever stands over the sky band (a tall painted house, the
/// edge of the ground) takes the same light, the sky itself is left as it is.
pub(super) fn tint(im: &mut Img, s: &Session, cam: Cam, shown: u32, me: Option<&Knight>, band: &Img) {
    let amb = ambient(s, shown);
    let d = dread(s, shown);
    let bright = amb[0] >= 0.99 && amb[1] >= 0.99 && amb[2] >= 0.99;
    if bright && d == 0.0 {
        return;
    }
    let mut pools: Vec<Pool> = Vec::new();
    let to = |p: (i32, i32)| ((p.0 - cam.x) as f32, (p.1 - cam.y + VIEW_Y) as f32);
    if amb[0] < 0.99 || amb[2] < 0.99 {
        for (id, at) in s.lights() {
            let (x, y) = to(at);
            pools.push(Pool { x, y, r: if id < 6 { 46.0 } else { 84.0 } });
        }
        for seat in s.seats.values().filter(|st| st.knight.up()) {
            let (x, y) = to(seat.knight.px());
            let mine = me.is_some_and(|m| m.px() == seat.knight.px());
            pools.push(Pool { x, y: y - 6.0, r: if mine { 42.0 } else { 34.0 } });
        }
    }
    let (cx, cy) = (192.0, (VIEW_Y + VIEW_H / 2) as f32);
    let warm = [1.0f32, 0.88, 0.66];
    // The lamps show against the dark, not against the day: they fade in
    // as the ambient light falls.
    let dusk = ((0.92 - (amb[0] + amb[1] + amb[2]) / 3.0) / 0.4).clamp(0.0, 1.0);
    // The moon's shadow, sweeping the town over the Final Hours.
    let shadow = (d > 0.0).then(|| {
        let wx = -120.0 + 860.0 * d;
        to((wx as i32, 250))
    });
    for y in 0..VIEW_Y + VIEW_H {
        for x in 0..im.w {
            let Some(c) = im.get(x, y) else { continue };
            if c == BLACK || is_signal(c) || (y < VIEW_Y && band.get(x, y) == Some(c)) {
                continue;
            }
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut lit = 0.0f32;
            for p in &pools {
                let k = 1.0 - ((fx - p.x).powi(2) + (fy - p.y).powi(2)).sqrt() / p.r;
                if k > lit {
                    lit = k;
                }
            }
            let lit = (lit.max(0.0).powi(2) * dusk * 6.0 + bayer(x, y)).floor() / 6.0;
            let mut f = [
                amb[0] + (warm[0] - amb[0]) * lit,
                amb[1] + (warm[1] - amb[1]) * lit,
                amb[2] + (warm[2] - amb[2]) * lit,
            ];
            if d > 0.0 {
                let vig = ((fx - cx).powi(2) / (cx * cx) + (fy - cy).powi(2) / (90.0 * 90.0)).min(2.0);
                let k = 1.0 - d * 0.28 * vig;
                f = [f[0] * k, f[1] * k, f[2] * k];
                if let Some((sx, sy)) = shadow {
                    let e = ((fx - sx) / 200.0).powi(2) + ((fy - sy) / 110.0).powi(2);
                    if e < 1.0 && e + (bayer(x, y) - 0.5) * 0.3 < 0.85 {
                        f = [f[0] * 0.5, f[1] * 0.5, f[2] * 0.55];
                    }
                }
            }
            let q = [
                (f[0] * 16.0).round().clamp(0.0, 20.0) as u8,
                (f[1] * 16.0).round().clamp(0.0, 20.0) as u8,
                (f[2] * 16.0).round().clamp(0.0, 20.0) as u8,
            ];
            if q != [16, 16, 16] {
                im.set(x, y, tinted(c, q));
            }
        }
    }
}

/// How big and how low the moon hangs: 0 at the first dawn, 1 when it falls.
fn nearness(s: &Session, shown: u32) -> f32 {
    if s.completed.is_some() || matches!(s.phase, Phase::Won { .. }) {
        return 0.12;
    }
    (shown as f32 / clock::end(s.nums.day).max(1) as f32).min(1.0)
}

/// The painted face the moon shows, when all its faces have arrived: the
/// sulk through the first day of the loop, the grimace through the second,
/// the snarl through the third (the thirds of the clock the moon grows by);
/// the two roaring faces while it comes down for the wipe; the calm one once
/// the lights hold and it turns back, and after.
pub(super) fn painted_face(s: &Session, shown: u32, tick: u32) -> Option<&'static Img> {
    use super::bank::frame;
    if ["far", "near", "close", "falling", "turned"].iter().any(|state| frame("moon", state, 0).is_none()) {
        return None;
    }
    let state = match s.phase {
        _ if s.completed.is_some() => "turned",
        Phase::Won { .. } => "turned",
        Phase::Rewind { t, fell: true, .. } if (t as f32 / control(&s.nums).max(1) as f32) < 0.45 => "falling",
        _ => ["far", "near", "close"][((shown / s.nums.day.max(1)) as usize).min(2)],
    };
    frame("moon", state, tick)
}

/// Where the moon hangs and how big, at the shown clock: how near it is (0 to
/// 1), its centre and radius in pixels, and whether the wipe has made it a
/// solid disc over the town.
pub(super) fn disc(s: &Session, shown: u32) -> (f32, f32, f32, f32, bool) {
    let mut p = nearness(s, shown);
    if let Phase::Won { t } = s.phase {
        // The moon draws back as the lights hold.
        p = 1.0 - (1.0 - 0.12) * (t as f32 / 60.0).min(1.0);
    }
    let mut r = 12.0 + 36.0 * p * p;
    let mut cx = 312.0 - 120.0 * p;
    let mut cy = 16.0 + 34.0 * p.powf(1.3);
    let mut solid = false;
    if let Phase::Rewind { t, fell: true, .. } = s.phase {
        // The wipe: the moon comes down the screen and fills it, then the
        // flash takes the town back (`ritual`).
        let k = (t as f32 / control(&s.nums).max(1) as f32 / 0.45).min(1.0);
        if t as f32 / (control(&s.nums).max(1) as f32) < 0.45 {
            r += (240.0 - r) * k * k;
            cx += (192.0 - cx) * k;
            cy += (130.0 - cy) * k;
            solid = k > 0.15;
        }
    }
    (p, cx, cy, r, solid)
}

/// The Gloam Moon, face and all, on the effects layer.
pub(super) fn moon(fx: &mut Img, s: &Session, shown: u32, tick: u32) {
    let (p, cx, cy, r, solid) = disc(s, shown);
    let day_ghost = hour_of(shown, s.nums.day);
    let ghost = (8.0..16.0).contains(&day_ghost);
    if let Some(pic) = painted_face(s, shown, tick) {
        // The same disc, place and half-dropped pixels over the roofs as the
        // code's: the painted face is drawn to the disc's width, a plain pixel
        // repeat or drop (it grows over the day, and fills the screen in the wipe).
        let d = ((2.0 * r).round() as i32).max(1);
        let (ox, oy) = ((cx - d as f32 / 2.0).round() as i32, (cy - d as f32 / 2.0).round() as i32);
        for y in oy.max(0)..(oy + d).min(224) {
            for x in ox.max(0)..(ox + d).min(384) {
                if (!solid && y >= VIEW_Y - 1 && (x + y) % 2 == 0) || (ghost && (x + y) % 2 == 1) {
                    continue;
                }
                let (u, v) = (((2 * (x - ox) + 1) * pic.w / (2 * d)).min(pic.w - 1), ((2 * (y - oy) + 1) * pic.h / (2 * d)).min(pic.h - 1));
                if let Some(c) = pic.get(u, v) {
                    fx.set(x, y, c);
                }
            }
        }
        return;
    }
    let sleepy = s.completed.is_some();
    let (x0, x1, y0, y1) = ((cx - r) as i32 - 1, (cx + r) as i32 + 1, (cy - r) as i32 - 1, (cy + r) as i32 + 1);
    for y in y0.max(0)..y1.min(224) {
        for x in x0.max(0)..x1.min(384) {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let q = (dx * dx + dy * dy).sqrt() / r;
            if q > 1.0 {
                continue;
            }
            // Over the town, only half the pixels: the roofs still show.
            if (!solid && y >= VIEW_Y - 1 && (x + y) % 2 == 0) || (ghost && (x + y) % 2 == 1) {
                continue;
            }
            let lit = -dx * 0.5 - dy * 0.6;
            let base = if q > 0.9 {
                'U'
            } else if lit / r > 0.35 {
                'W'
            } else if lit / r < -0.3 {
                'v'
            } else {
                'V'
            };
            fx.put(x, y, base);
        }
    }
    let rr = r;
    // Craters.
    for (kx, ky, kr) in [(-0.5, 0.35, 0.16), (0.45, 0.55, 0.12), (0.1, -0.7, 0.1)] {
        fx.ellipse(cx + kx * rr, cy + ky * rr, kr * rr, kr * rr * 0.8, 'U');
    }
    let rage = 0.4 + 0.6 * p;
    // Brows, slanted down toward the middle: it frowns.
    for side in [-1.0f32, 1.0] {
        let (bx0, by0) = (cx + side * rr * 0.62, cy - rr * 0.42 - rr * 0.1 * rage);
        let (bx1, by1) = (cx + side * rr * 0.1, cy - rr * 0.2);
        for k in 0..=(1 + (rr * 0.1) as i32) {
            fx.line(bx0 as i32, by0 as i32 + k, bx1 as i32, by1 as i32 + k, 's');
        }
    }
    // Eyes: dark sockets with red fire in them.
    for side in [-1.0f32, 1.0] {
        let (ex, ey) = (cx + side * rr * 0.38, cy - rr * 0.08);
        let (w, h) = (rr * 0.22, if sleepy { rr * 0.03 } else { rr * (0.13 + 0.05 * (1.0 - rage)) });
        fx.ellipse(ex, ey, w, h.max(0.6), 'k');
        if !sleepy && rr >= 12.0 {
            let pulse = if (tick / 8) % 6 == 0 { 'w' } else { '7' };
            fx.ellipse(ex + side * -w * 0.2, ey, w * 0.45, h * 0.6, pulse);
        } else if !sleepy {
            fx.put(ex as i32, ey as i32, '7');
        }
    }
    // A grin of teeth.
    if !sleepy {
        let (mx, my) = (cx, cy + rr * 0.45);
        let (mw, mh) = (rr * 0.55, rr * (0.12 + 0.12 * rage));
        fx.ellipse(mx, my, mw, mh, 'k');
        if rr >= 14.0 {
            let n = 7;
            for k in 0..n {
                let tx = mx - mw + (2.0 * mw) * (k as f32 + 0.5) / n as f32;
                let tw = mw / n as f32;
                let up = k % 2 == 0;
                fx.rect((tx - tw * 0.5) as i32, (my - mh * 0.95) as i32, tw.max(1.0) as i32, (mh * 0.8).max(1.0) as i32, 'W');
                if up {
                    fx.rect((tx - tw * 0.5) as i32, (my + mh * 0.15) as i32, tw.max(1.0) as i32, (mh * 0.7).max(1.0) as i32, 'W');
                }
            }
        }
    } else {
        fx.rect((cx - rr * 0.2) as i32, (cy + rr * 0.45) as i32, (rr * 0.4) as i32, 1, 's');
    }
}

/// The square the ritual's dark glass covers (`ritual` draws the glass at
/// this radius): words keep out of it while it turns.
pub(super) fn glass(s: &Session) -> Option<Rect> {
    let Phase::Rewind { t, fell, .. } = s.phase else { return None };
    let k = t as f32 / control(&s.nums).max(1) as f32;
    if fell && k < 0.45 {
        return None;
    }
    let r = (40.0 - 8.0 * k + 4.0).ceil() as i32;
    Some(Rect::new(192 - r, 112 - r, 2 * r, 2 * r))
}

/// The rewind's own picture: a clock face whose hands spin back, rings
/// that gather to the instant the town returns, and the flash after it.
pub(super) fn ritual(fx: &mut Img, s: &Session, shown: u32, tick: u32) {
    let Phase::Rewind { t, fell, .. } = s.phase else { return };
    let ctl = control(&s.nums).max(1);
    let (cx, cy) = (192.0f32, 112.0f32);
    let k = t as f32 / ctl as f32;
    if fell && k < 0.45 {
        return;
    }
    let face = if fell { 'w' } else { '3' };
    let rad = 40.0 - 8.0 * k;
    // A dark glass behind the face so the hands read over any town.
    for y in (cy - rad - 4.0) as i32..(cy + rad + 4.0) as i32 {
        for x in (cx - rad - 4.0) as i32..(cx + rad + 4.0) as i32 {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            if d < rad + 3.0 && (x + y) % 2 == 0 {
                fx.put(x, y, 'k');
            }
        }
    }
    for a in 0..48 {
        let ang = a as f32 / 48.0 * std::f32::consts::TAU;
        let (x, y) = (cx + ang.sin() * rad, cy - ang.cos() * rad);
        fx.rect(x as i32 - 1, y as i32 - 1, if a % 4 == 0 { 3 } else { 2 }, if a % 4 == 0 { 3 } else { 2 }, face);
    }
    let st = clock::stamp(shown, s.nums.day);
    let hour = (st.hour % 12) as f32 + st.minute as f32 / 60.0;
    let hand = |fx: &mut Img, turns: f32, len: f32, ink: char| {
        let ang = turns * std::f32::consts::TAU;
        for step in 0..(len as i32) {
            let (x, y) = (cx + ang.sin() * step as f32, cy - ang.cos() * step as f32);
            fx.rect(x as i32, y as i32, 2, 2, ink);
        }
    };
    hand(fx, hour / 12.0, rad * 0.55, '6');
    hand(fx, st.minute as f32 / 60.0, rad * 0.85, 'w');
    fx.ellipse(cx, cy, 3.0, 3.0, '6');
    // Rings drawn in as the town winds back.
    for ring in 0..3 {
        let rr = 70.0 + ring as f32 * 22.0 - 80.0 * k * (1.0 + ring as f32 * 0.2);
        if rr > 6.0 {
            for a in 0..90 {
                let ang = a as f32 / 90.0 * std::f32::consts::TAU;
                if (a + (tick as i32 / 2)) % 3 == 0 {
                    fx.put((cx + ang.sin() * rr) as i32, (cy - ang.cos() * rr) as i32, '2');
                }
            }
        }
    }
}

/// How hard the town is whited out, and in what tint, this tick: the moon's
/// landing, the snap as the town lands on its dawn, and the fade after it.
pub(super) fn flare_level(s: &Session) -> Option<(f32, [f32; 3])> {
    match s.phase {
        Phase::Rewind { t, fell, .. } => {
            let k = t as f32 / control(&s.nums).max(1) as f32;
            if fell && k >= 0.45 {
                Some((1.0 - (k - 0.45) / 0.55, [1.0, 1.0, 1.0]))
            } else if k >= 0.8 {
                Some(((k - 0.8) / 0.2 * 0.7, [1.0, 1.4, 1.4]))
            } else {
                None
            }
        }
        Phase::Play if s.memory.loops > 0 => {
            let total = s.nums.rewind.saturating_sub(control(&s.nums)) + 24;
            let age = total.saturating_sub(s.card.min(total));
            (age < 8 && s.card > 0).then(|| (0.7 * (1.0 - age as f32 / 8.0), [1.0, 1.4, 1.4]))
        }
        _ => None,
    }
}

/// Whiten the town (above the clock bar): every colour stepped toward light.
pub(super) fn flare(im: &mut Img, level: f32, tint: [f32; 3]) {
    let level = level.clamp(0.0, 1.0);
    if level <= 0.01 {
        return;
    }
    let gain = 1.0 + 3.0 * level;
    let f = [
        ((gain * tint[0] * 16.0).round() as u32).min(64) as u8,
        ((gain * tint[1] * 16.0).round() as u32).min(64) as u8,
        ((gain * tint[2] * 16.0).round() as u32).min(64) as u8,
    ];
    for y in 0..VIEW_Y + VIEW_H {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                im.set(x, y, tinted(c, f));
            }
        }
    }
}

/// Beams of light from the three lit places while the moon turns back.
pub(super) fn beams(fx: &mut Img, s: &Session, cam: Cam, tick: u32) {
    if !(matches!(s.phase, Phase::Won { .. }) || s.completed.is_some()) {
        return;
    }
    let grow = match s.phase {
        Phase::Won { t } => (t as f32 / 40.0).min(1.0),
        _ => 1.0,
    };
    use crate::rules::moonfall::town;
    for at in [town::NORTH_DOOR, town::SOUTH_DOOR, town::WELL] {
        let (x, y) = (at.0 - cam.x, at.1 - cam.y + VIEW_Y);
        let top = (y as f32 - 160.0 * grow) as i32;
        for yy in top.max(0)..y {
            for xx in x - 3..=x + 3 {
                if ((xx + yy + (tick as i32 / 2)) & 3) < 2 {
                    fx.put(xx, yy, '9');
                }
            }
        }
    }
}
