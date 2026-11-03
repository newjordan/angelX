//! Choices belonging to one drawing, never to the run or its wire tree.
use super::overworld::arena::NATIVE_W;
use super::overworld::ink::Img;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub calm_screen: bool,
}

std::thread_local! {
    static CURRENT: std::cell::Cell<Preferences> = const { std::cell::Cell::new(Preferences { calm_screen: false }) };
}

/// Scoped so a mirror's picture cannot change the next viewer's picture.
pub struct Scope(Preferences);
impl Drop for Scope {
    fn drop(&mut self) { CURRENT.with(|c| c.set(self.0)); }
}

pub fn enter(preferences: Preferences) -> Scope {
    Scope(CURRENT.with(|c| c.replace(preferences)))
}

pub fn with<T>(preferences: Preferences, draw: impl FnOnce() -> T) -> T {
    let _scope = enter(preferences);
    draw()
}

pub(crate) fn calm() -> bool { CURRENT.with(|c| c.get().calm_screen) }

/// Keep dark planes unchanged; fold bright transient highlights into slate.
/// Applied after composition, never while filling shared sprite caches.
pub(crate) fn soften(img: &mut Img) {
    if !calm() { return; }
    for y in 0..img.h {
        for x in 0..img.w {
            if let Some(rgb) = img.get(x, y) {
                img.set(x, y, soft_tint(rgb));
            }
        }
    }
}

pub(crate) fn soft_tint(rgb: [u8; 3]) -> [u8; 3] {
    let top = *rgb.iter().max().unwrap();
    if top <= 150 { return rgb; }
    let mut out = rgb;
    for c in &mut out { *c = (u16::from(*c) * 150 / u16::from(top)) as u8; }
    out
}

/// The parts compositor must not reintroduce a bright post pass or replace
/// a softened silhouette with its original painting.
pub(crate) fn finish_scene(scene: &mut super::overworld::arena::RoomScene) {
    if !calm() { return; }
    scene.impact = 0.0;
    for light in &mut scene.lights {
        if light.flicker { light.strength *= 0.2; light.flicker = false; }
    }
    for layer in [&mut scene.under, &mut scene.over].into_iter().flatten() {
        soften(layer);
    }
    for figure in &mut scene.figures {
        soften(&mut figure.img);
        figure.additive = false;
        figure.bodies.clear();
        figure.paints.clear();
    }
    scene.paints.clear();
    for shot in &mut scene.shots { soften(&mut shot.mark); }
    soften(&mut scene.screen);
}

/// The gnome's garden seen strangely (`nooks::gnome`): the room sways on
/// a slow swell, its colours swing round the wheel in soft bands that
/// drift across it, stronger toward its edges than its middle, and
/// sparkles open and close round its rim in mirrored fours, as through a
/// kaleidoscope, over shards of colour that come and go round it. Black
/// stays black away from the rim, so the dark planes keep the room's
/// shape. A calm screen sees only a slow, slight turn of the colours.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn trip(img: &mut Img, tick: u64) {
    let h = img.h;
    trip_rows(img, tick, 0, h);
}

/// `trip` over rows `top` to `bottom` of `img` alone: a seat's view of
/// the room, its bands above and below left to be read.
pub(crate) fn trip_rows(img: &mut Img, tick: u64, top: i32, bottom: i32) {
    let (top, bottom) = (top.clamp(0, img.h), bottom.clamp(0, img.h));
    if img.w <= 0 || bottom <= top {
        return;
    }
    let calm = calm();
    let t = tick as f32;
    let k = NATIVE_W as f32 / img.w as f32;
    let sway = if calm { 0.0 } else { 1.1 / k };
    Field::new(t, calm).roll(img, (0.0, -top as f32 * k), k, (top, bottom), sway);
    if !calm {
        kaleidoscope(img, t, 1.0 / k, (top, bottom));
    }
}

/// The trip's colour field over the room's view: how far each point's
/// colours turn round the wheel, how much richer they come, and how
/// strongly the rim's mirrored shards of colour show.
struct Field {
    t: f32,
    base: f32,
    amp: f32,
    sat: f32,
    glow: f32,
    rim: f32,
}

/// How far in from the view's edge its kaleidoscope rim reaches (native
/// pixels).
const RIM: f32 = 34.0;

/// How finely a turn is told apart: the steps between its two ends.
const TURNS: usize = 96;

impl Field {
    fn new(t: f32, calm: bool) -> Field {
        if calm {
            Field { t: t * 0.4, base: (t * 0.012).sin() * 0.22, amp: 0.12, sat: 1.0, glow: 0.0, rim: 0.0 }
        } else {
            Field { t, base: (t * 0.021).sin() * 1.1 + (t * 0.0067).sin() * 0.6, amp: 1.05, sat: 1.28, glow: 16.0, rim: 0.42 }
        }
    }

    /// Turn the pixels of `img` in rows `rows`, pixel `(x, y)` standing at
    /// view point `origin + (x, y) * k` (native pixels of the room's view);
    /// each row first slid along a slow swell up to `sway` pixels.
    fn roll(&self, img: &mut Img, origin: (f32, f32), k: f32, (top, bottom): (i32, i32), sway: f32) {
        use super::overworld::arena::{NATIVE_H, NATIVE_W};
        let (w, vw, vh) = (img.w.max(0) as usize, NATIVE_W as f32, NATIVE_H as f32);
        if w == 0 {
            return;
        }
        let edge = |v: f32, span: f32| {
            let e = 2.0 * v / span - 1.0;
            (e * e).min(1.0)
        };
        // Per column and per row: the drifting bands' halves, how near the
        // edge, and the rim's two mirrored waves (folded at the middle).
        let t = self.t;
        let mut cols = Vec::with_capacity(w);
        for x in 0..w {
            let vx = origin.0 + x as f32 * k;
            let (su, cu) = (vx * 0.024).sin_cos();
            let mx = vx.min(vw - vx);
            let (a1, b1) = (mx * 0.23 + t * 0.07).sin_cos();
            let (a2, b2) = (mx * 0.11 - t * 0.05).sin_cos();
            cols.push((su, cu, edge(vx, vw), mx, [a1, b1, a2, b2]));
        }
        let (lo, hi) = (self.base - self.amp, self.base + self.amp);
        let turns: [(f32, f32); TURNS] = std::array::from_fn(|i| {
            let a = lo + (hi - lo) * i as f32 / (TURNS - 1) as f32;
            let (s, c) = a.sin_cos();
            (c, s)
        });
        let mut row: Vec<Option<[u8; 3]>> = vec![None; w];
        for y in top..bottom {
            let vy = origin.1 + y as f32 * k;
            let (sv, cv) = (vy * 0.031 - self.t * 0.05).sin_cos();
            let ey = edge(vy, vh);
            let my = vy.min(vh - vy);
            let (c1, d1) = (my * 0.17).sin_cos();
            let (c2, d2) = (-my * 0.19 + t * 0.03).sin_cos();
            let shift = (sway * (vy * 0.08 + self.t * 0.09).sin() * (0.65 + 0.35 * (self.t * 0.031).sin())).round() as i32;
            for (x, p) in row.iter_mut().enumerate() {
                *p = img.get(x as i32, y);
            }
            for (x, &(su, cu, ex, mx, [a1, b1, a2, b2])) in cols.iter().enumerate() {
                let from = (x as i32 - shift).clamp(0, w as i32 - 1) as usize;
                let Some(rgb) = row[from] else {
                    img.clear(x as i32, y);
                    continue;
                };
                if rgb.iter().all(|&c| c < 6) {
                    img.set(x as i32, y, rgb);
                    continue;
                }
                let band = su * cv + cu * sv;
                let e = ex.max(ey);
                let a = (band * (0.35 + 0.65 * e) + 1.0) * 0.5;
                let (c, s) = turns[((a * (TURNS - 1) as f32).round() as usize).min(TURNS - 1)];
                let lift = self.glow * band.max(0.0).powi(3) * e;
                let mut out = turned(rgb, c, s, self.sat, lift);
                // The rim: shards of colour where two folded waves cross,
                // mirrored at the view's middle both ways.
                let d = mx.min(my);
                if self.rim > 0.0 && d < RIM {
                    let p = (a1 * d1 + b1 * c1) * (a2 * d2 + b2 * c2);
                    if p > 0.18 {
                        let k = self.rim * (1.0 - d / RIM).powi(2) * ((p - 0.18) / 0.4).min(1.0);
                        let tint = hue(p * 0.6 + t * 0.006 + (mx + my) * 0.004);
                        for (o, h) in out.iter_mut().zip(tint) {
                            *o = (f32::from(*o) + (h - f32::from(*o)) * k).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
                img.set(x as i32, y, out);
            }
        }
    }
}

/// `rgb` turned round the colour wheel by the angle whose cosine and sine
/// are `c`, `s`, its colour made `sat` times as rich and its light lifted
/// by `lift` (YIQ).
fn turned([r, g, b]: [u8; 3], c: f32, s: f32, sat: f32, lift: f32) -> [u8; 3] {
    let (r, g, b) = (f32::from(r), f32::from(g), f32::from(b));
    let y = 0.299 * r + 0.587 * g + 0.114 * b + lift;
    let i = 0.596 * r - 0.274 * g - 0.322 * b;
    let q = 0.211 * r - 0.523 * g + 0.312 * b;
    let (i, q) = ((i * c - q * s) * sat, (i * s + q * c) * sat);
    let k = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    [k(y + 0.956 * i + 0.621 * q), k(y - 0.272 * i - 0.647 * q), k(y - 1.106 * i + 1.703 * q)]
}

/// A hue at full light, softly coloured.
fn hue(h: f32) -> [f32; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let f = h.fract();
    let (lo, hi) = (0.45, 1.0);
    let up = lo + (hi - lo) * f;
    let down = hi - (hi - lo) * f;
    let rgb = match h as u32 {
        0 => [hi, up, lo],
        1 => [down, hi, lo],
        2 => [lo, hi, up],
        3 => [lo, down, hi],
        4 => [up, lo, hi],
        _ => [hi, lo, down],
    };
    rgb.map(|v| v * 255.0)
}

/// Sparkles round the rim of the room's view in rows `rows` of `img`
/// (`scale` its pixels to a native one): each opens and closes as a
/// four-pointed star, and every one is drawn four times over, mirrored
/// across the view's middle both ways, as a kaleidoscope has it.
fn kaleidoscope(img: &mut Img, t: f32, scale: f32, (top, bottom): (i32, i32)) {
    use super::overworld::arena::{NATIVE_H, NATIVE_W};
    use super::overworld::ink::hash;
    let (vw, vh) = (NATIVE_W as f32, ((bottom - top) as f32 / scale).min(NATIVE_H as f32));
    let margin = 30.0;
    for s in 0..14u32 {
        let period = 34.0 + (s * 7 % 17) as f32;
        let run = t + s as f32 * 23.0;
        let cycle = (run / period).floor() as i32;
        let phase = (run - cycle as f32 * period) / period;
        let h = hash(s as i32, cycle, 0x7a1);
        let u = |b: u32| ((h >> b) & 0xff) as f32 / 255.0;
        let near = u(8) * u(8) * margin;
        let along = u(16);
        let (px, py) = if h & 1 == 0 { (along * vw * 0.5, near) } else { (near, along * vh * 0.5) };
        let arm = (phase * std::f32::consts::PI).sin() * (3.0 + u(24) * 4.0);
        if arm < 0.6 {
            continue;
        }
        let colour = hue(s as f32 * 0.13 + t * 0.004 + cycle as f32 * 0.37);
        for (mx, my) in [(px, py), (vw - 1.0 - px, py), (px, vh - 1.0 - py), (vw - 1.0 - px, vh - 1.0 - py)] {
            let (cx, cy) = ((mx * scale) as i32, top + (my * scale) as i32);
            let reach = (arm * scale).round() as i32;
            let mut dab = |x: i32, y: i32, k: f32| {
                if y < top || y >= bottom {
                    return;
                }
                if let Some(was) = img.get(x, y) {
                    let mix = |a: u8, b: f32| (f32::from(a) + (b - f32::from(a)).max(0.0) * k).round().clamp(0.0, 255.0) as u8;
                    img.set(x, y, [mix(was[0], colour[0]), mix(was[1], colour[1]), mix(was[2], colour[2])]);
                }
            };
            for d in 0..=reach {
                let k = 1.0 - d as f32 / (reach + 1) as f32;
                for (dx, dy) in [(d, 0), (-d, 0), (0, d), (0, -d)] {
                    dab(cx + dx, cy + dy, k * 0.9);
                }
            }
            let diag = (reach / 3).max(1);
            for d in 1..=diag {
                for (dx, dy) in [(d, d), (-d, d), (d, -d), (-d, -d)] {
                    dab(cx + dx, cy + dy, 0.45);
                }
            }
        }
    }
}

/// The parts compositor's trip: the room's ground, every layer, figure
/// and shot turned in one field over the room's view; the camera swaying
/// a little; the rim's sparkles on the screen's own marks over the view;
/// and the colour split breathing. The bands keep their colours.
pub(crate) fn trip_scene(scene: &mut super::overworld::arena::RoomScene, tick: u64) {
    use super::overworld::arena::{BAND_H, HUD_H, NATIVE_W};
    let calm = calm();
    let t = tick as f32;
    let field = Field::new(t, calm);
    let k = (scene.view.0 / NATIVE_W).max(1);
    let (band, hud) = (BAND_H * k, HUD_H * k);
    let cam = (scene.camera.0, scene.camera.1 + band as f32);
    let room = (-cam.0, -cam.1);
    if let Some(ground) = &mut scene.ground {
        let ground = std::sync::Arc::make_mut(ground);
        let h = ground.h;
        field.roll(ground, room, 1.0, (0, h), 0.0);
    }
    for layer in [&mut scene.under, &mut scene.over].into_iter().flatten() {
        let h = layer.h;
        field.roll(layer, room, 1.0, (0, h), 0.0);
    }
    for figure in &mut scene.figures {
        let h = figure.img.h;
        field.roll(&mut figure.img, (figure.x as f32 - cam.0, figure.y as f32 - cam.1), 1.0, (0, h), 0.0);
    }
    for shot in &mut scene.shots {
        let (w, h) = (shot.mark.w, shot.mark.h);
        field.roll(&mut shot.mark, (shot.x - w as f32 / 2.0 - cam.0, shot.y - h as f32 / 2.0 - cam.1), 1.0, (0, h), 0.0);
    }
    let bottom = scene.screen.h - hud;
    field.roll(&mut scene.screen, (0.0, -(band as f32) / k as f32), 1.0 / k as f32, (band, bottom), 0.0);
    if !calm {
        kaleidoscope(&mut scene.screen, t, k as f32, (band, bottom));
        scene.camera.0 += (t * 0.05).sin() * 1.3;
        scene.camera.1 += (t * 0.037 + 1.0).sin() * 0.7;
        scene.impact = scene.impact.max(0.16 + 0.08 * (t * 0.09).sin());
    }
}

/// The map and down menu already contain local camera/display choices.
/// N+E works with native keys, a browser keyboard and the touch map/use keys.
pub(crate) fn options(out: &mut Img, map: bool, preferences: Preferences) {
    if !map { return; }
    let line = if preferences.calm_screen { "E  CALM SCREEN: ON" } else { "E  CALM SCREEN: OFF" };
    let w = super::overworld::ink::text_width(line) + 8;
    let x = (out.w - w) / 2;
    let y = (out.h - 13).max(0);
    out.rect(x, y, w, 12, 's');
    out.text(x + 4, y + 2, line, 'h');
}
