//! Brasswake's room alive: lanterns that flicker, steam off the vats, dust in the lamplight and a
//! crowd of patrons in the arches who sway and, when a kick lands, roar and throw their mugs up.
//! All of it goes on a layer of its own that is stamped over the painted room and under the bodies.
use super::super::super::ink::{Img, bayer, hash};
use super::super::pfx::{Pen, ramps, unit};
use super::fx::{Ctx, STEAM};
use super::hud::{TOP, W};
use crate::rules::brasswake::show::Kind;
use crate::rules::juice::Weight;

/// Where the lanterns hang in one 240-wide section of the room (the painting's own pixels).
const LAMPS: [(i32, i32); 3] = [(13, 28), (56, 61), (203, 61)];
/// The vats' vents.
const VENTS: [(i32, i32); 2] = [(98, 8), (170, 6)];
/// The patrons' spots, the middle of their feet, in the arches above the barrels.
const SPOTS: [(i32, i32, u8); 6] = [(37, 88, 0), (96, 79, 1), (112, 80, 2), (165, 77, 3), (181, 88, 4), (28, 84, 5)];

const PATRON: [&str; 11] = [
    "...kkkk...",
    "..kHHHHk..",
    "..kHHHHk..",
    "..kSSSSk..",
    "..kSkSSk..",
    "..kSSSSk..",
    ".kkTTTTkk.",
    ".kTTTTTTk.",
    ".kTTMTTTk.",
    ".kTTTTTTk.",
    "..kkkkkk..",
];
const CHEER: [&str; 11] = [
    ".........M",
    "...kkkk.TM",
    "..kHHHHkTk",
    "..kHHHHkTk",
    "..kSSSSkk.",
    "..kSSSSk..",
    ".kkTTTTkk.",
    ".kTTTTTTk.",
    ".kTTTTTTk.",
    ".kTTTTTTk.",
    "..kkkkkk..",
];

fn patron(kind: u8, cheer: bool) -> Img {
    let hat = ['7', '1', 'A', '4', 'P', 'Q'][usize::from(kind) % 6];
    let skin = ['o', 't', 'O', 'o', 'P', 't'][usize::from(kind) % 6];
    let tunic = ['p', 'b', 'B', 'a', 'I', 'x'][usize::from(kind) % 6];
    Img::from_rows(if cheer { &CHEER } else { &PATRON }).recolor(&[('H', hat), ('S', skin), ('T', tunic), ('M', 'T')])
}

/// How loud the room is at `now`: 0 at rest, up to 1 when a heavy blow just landed or the keeper fell.
fn roar(c: &Ctx) -> f32 {
    let mut r = 0.0f32;
    for m in &c.d.show.marks {
        let age = m.age(c.now) as f32;
        let w = match m.kind {
            Kind::Finish => Weight::Finale,
            Kind::Blow { .. } | Kind::Clear { .. } => m.kind.weight(),
            _ => continue,
        };
        let peak = match w {
            Weight::Graze => 0.0,
            Weight::Solid => 0.25,
            Weight::Crushing => 0.6,
            Weight::Kill => 0.85,
            Weight::Finale => 1.0,
        };
        r = r.max(peak * (1.0 - age / 36.0).max(0.0));
    }
    if c.d.finish.is_some() {
        r = r.max(0.8);
    }
    r
}

/// The living backdrop, painted over the room: lanterns, dust and the crowd straight on it, the
/// steam on a small layer of its own (it goes only on bare paper) stamped over the vats.
pub(super) fn paint(scene: &mut Img, c: Ctx, tick: u64) {
    let mut im = Img::window(W, super::hud::H, (0, TOP, W, 100));
    let first = c.cam.div_euclid(240);
    let last = (c.cam + W).div_euclid(240);
    let t = tick as f32;
    for s in first..=last {
        let ox = s * 240 - c.cam;
        // Lanterns breathe: a halo that pulses, nothing that dims the paint.
        for (n, &(lx, ly)) in LAMPS.iter().enumerate() {
            let flick = 0.55 + 0.45 * unit(n as i32 + s * 3, (tick / 3) as i32, 0x1f);
            let (cx, cy) = (ox + lx, TOP + ly);
            if !(-14..W + 14).contains(&cx) {
                continue;
            }
            let r = 7.0 + 4.0 * flick;
            for dy in -(r as i32)..=(r as i32) {
                for dx in -(r as i32)..=(r as i32) {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
                    if d < 1.0 && bayer(cx + dx, cy + dy) < (1.0 - d) * 0.32 * flick {
                        im.put(cx + dx, cy + dy, if d < 0.5 { 'O' } else { 'o' });
                    }
                }
            }
        }
        // The vats breathe steam.
        for (n, &(vx, vy)) in VENTS.iter().enumerate() {
            let x = ox + vx;
            if (-30..W + 30).contains(&x) {
                let mut pen = Pen::new(&mut im, 700);
                pen.wisp((x as f32, (TOP + vy) as f32), t + (n * 17) as f32, (0.0, 1.0e6), 46.0, 34.0, 7.0, STEAM, 5 + n as u32 + s as u32 * 3);
            }
        }
    }
    // Dust drifting in the lamplight: slower than the room, so it sits behind it.
    for i in 0..16i32 {
        let u = |k: u32| unit(i, 0x77, k);
        let x = ((u(1) * (W as f32 + 80.0) - t * (0.08 + u(2) * 0.12) - c.cam as f32 * 0.35).rem_euclid(W as f32 + 80.0) - 40.0) as i32;
        let y = TOP + 8 + (u(3) * 110.0 + (t * 0.03 + u(4) * 6.28).sin() * 5.0) as i32;
        if (0..W).contains(&x) && (tick as i32 / 4 + i) % 5 != 0 {
            im.put(x, y, if u(5) < 0.3 { 'T' } else { 't' });
        }
    }
    // The crowd.
    let loud = roar(&c);
    for s in first..=last {
        let ox = s * 240 - c.cam;
        for &(sx, sy, kind) in &SPOTS {
            let x = ox + sx;
            if !(-12..W + 2).contains(&x) {
                continue;
            }
            let who = hash(i32::from(kind), s, 0x3c) % 8;
            let phase = (tick as i32 + who as i32 * 7) as f32;
            let cheering = loud > 0.2 && (who as f32 / 8.0) < loud + 0.2;
            let sway = ((phase * 0.09).sin() * 1.0).round() as i32;
            let hop = if cheering { -(((phase * 0.55).sin().abs() * (1.0 + loud * 3.0)) as i32) } else { 0 };
            let frame = cheering && (tick / 4 + who as u64) % 2 == 0;
            let img = patron(kind, frame);
            // Only the head and shoulders show over the barrels.
            let shown = Img::from_view(&img, 0, 0, img.w, 8.min(img.h));
            im.stamp(&shown, x - 5 + sway, TOP + sy - 11 + hop);
        }
    }
    let _ = ramps::EMBER;
    scene.stamp(&im, 0, 0);
}
