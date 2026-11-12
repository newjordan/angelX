//! The weapon preview: `parts.png` cut into the library's parts, placed by the rules' own
//! `Assembly` (so the picture and the stats can never disagree), repainted by the weapon's paint,
//! stood upright on the stage like the arms library's held pose.

use super::super::overworld::ink::Img;
use crate::rules::creation::{kit, HaftCut, Material, Placed, School, WeaponCreation, WeaponKind};
use std::collections::HashMap;
use std::sync::OnceLock;

fn parts() -> &'static Vec<Img> {
    static PARTS: OnceLock<Vec<Img>> = OnceLock::new();
    PARTS.get_or_init(|| {
        let atlas = image::load_from_memory(include_bytes!("../../../assets/creator/weapons/parts.png"))
            .expect("the embedded weapon atlas")
            .to_rgba8();
        kit()
            .parts
            .iter()
            .map(|p| {
                let [x0, y0, w, h] = p.rect;
                let mut im = Img::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        let c = atlas.get_pixel((x0 + x) as u32, (y0 + y) as u32);
                        if c[3] > 127 {
                            im.set(x, y, [c[0], c[1], c[2]]);
                        }
                    }
                }
                im
            })
            .collect()
    })
}

/// Every library ramp colour, to the ramp and step it belongs to.
fn lut() -> &'static HashMap<[u8; 3], (Material, usize)> {
    static LUT: OnceLock<HashMap<[u8; 3], (Material, usize)>> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut map = HashMap::new();
        for (name, steps) in &kit().ramps {
            let Some(m) = Material::from_ramp(name) else { continue };
            for (i, hex) in steps.iter().enumerate() {
                map.entry(rgb(hex)).or_insert((m, i));
            }
        }
        map
    })
}

fn rgb(hex: &str) -> [u8; 3] {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

fn ramp(m: Material, step: usize) -> [u8; 3] {
    kit().ramps.get(m.ramp()).and_then(|r| r.get(step)).map_or([255, 0, 255], |h| rgb(h))
}

/// The ramps a school's glow repaints: everything on a magic part that is not metal.
const MAGIC: [Material; 8] = [
    Material::Cloth,
    Material::Shell,
    Material::FleshInner,
    Material::Glow,
    Material::Eye,
    Material::Chitin,
    Material::GagPink,
    Material::Skin,
];

fn repaint(c: [u8; 3], slot: &str, magic: bool, w: &WeaponCreation) -> [u8; 3] {
    let Some(&(from, step)) = lut().get(&c) else { return c };
    let p = &w.paint;
    let mut to = from;
    if from == Material::Plate {
        to = p.slot_metal.get(slot).copied().unwrap_or(p.metal);
    } else if matches!(from, Material::Leather | Material::Cloth) && p.binding != Material::Leather {
        to = p.binding;
    }
    if magic && let Some(school) = p.glow && MAGIC.contains(&from) {
        to = Material::of_school(school);
    }
    if to == from { c } else { ramp(to, step) }
}

/// A part as placed: haft or blade stretched, scaled, flipped.
fn shaped(q: &Placed) -> Img {
    let src = &parts()[q.kit];
    let mut im = src.clone();
    if let Some(len) = q.len {
        let cut = HaftCut::new(src.w, i32::from(len));
        let mut out = Img::new(cut.len(), src.h);
        for (x, &sx) in cut.cols.iter().enumerate() {
            for y in 0..src.h {
                if let Some(c) = src.get(sx, y) {
                    out.set(x as i32, y, c);
                }
            }
        }
        im = out;
    }
    if q.scale != 100 {
        let s = i32::from(q.scale);
        let (w, h) = (((im.w * s + 50) / 100).max(1), ((im.h * s + 50) / 100).max(1));
        let mut out = Img::new(w, h);
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = im.get(x * 100 / s, y * 100 / s) {
                    out.set(x, y, c);
                }
            }
        }
        im = out;
    }
    if q.flipped { im.flip_h() } else { im }
}

fn painted(w: &WeaponCreation, q: &Placed) -> Img {
    let mut im = shaped(q);
    let magic = q.gem || (w.kind.is_focus() && q.slot == "head");
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                im.set(x, y, repaint(c, &q.slot, magic, w));
            }
        }
    }
    im
}

/// The weapon on its own axis (tip to the right), every part repainted.
pub fn assembled(w: &WeaponCreation, _mark: Option<&str>) -> Option<Img> {
    let a = w.assemble()?;
    let mut canvas = Img::new(a.size.0, a.size.1);
    for q in &a.placed {
        canvas.stamp(&painted(w, q), q.off.0, q.off.1);
    }
    Some(canvas)
}

/// Only one slot's pixels, on a canvas the weapon's size: the part the picker is on.
pub fn part_mask(w: &WeaponCreation, slot: &str) -> Option<Img> {
    let a = w.assemble()?;
    let mut canvas = Img::new(a.size.0, a.size.1);
    for q in a.placed.iter().filter(|q| q.slot == slot && !q.gem) {
        canvas.stamp(&shaped(q), q.off.0, q.off.1);
    }
    Some(canvas)
}

/// What the stage shows: the weapon standing on its pommel or lying along the slab, whichever
/// shrinks it less, with a one-pixel margin all round and, when `mark` names a slot, that part's
/// outline in teal at display size (so a reduced weapon still shows where the picker is).
/// Returns the picture and the whole ratio it was shrunk by.
pub fn display(w: &WeaponCreation, mark: Option<&str>, up_box: (i32, i32), lie_box: (i32, i32)) -> Option<(Img, i32)> {
    let canvas = assembled(w, None)?;
    let (up, k_up) = fitted(&upright(&canvas), up_box.0, up_box.1);
    let (lie, k_lie) = fitted(&canvas, lie_box.0, lie_box.1);
    let lying = k_lie < k_up;
    let (base, k) = if lying { (lie, k_lie) } else { (up, k_up) };
    let mut out = Img::new(base.w + 2, base.h + 2);
    out.stamp(&base, 1, 1);
    if let Some(mask) = mark.and_then(|slot| part_mask(w, slot)) {
        let m = if lying { mask } else { upright(&mask) };
        let m = if k == 1 { m } else { reduced(&m, k) };
        let mut halo = Vec::new();
        for y in 0..m.h {
            for x in 0..m.w {
                if m.get(x, y).is_none() {
                    continue;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (px, py) = (x + dx + 1, y + dy + 1);
                    if px >= 0 && py >= 0 && px < out.w && py < out.h && out.get(px, py).is_none() {
                        halo.push((px, py));
                    }
                }
            }
        }
        for (px, py) in halo {
            out.put(px, py, '2');
        }
    }
    Some((out, k))
}

/// The held pose: the axis turned to point up, as `weapon-up.png` is cut.
pub fn upright(canvas: &Img) -> Img {
    let mut up = Img::new(canvas.h, canvas.w);
    for y in 0..canvas.h {
        for x in 0..canvas.w {
            if let Some(c) = canvas.get(x, y) {
                up.set(y, canvas.w - 1 - x, c);
            }
        }
    }
    up
}

/// Shrunk by a whole ratio, a block keeping its most common colour, so outlines survive.
fn reduced(im: &Img, k: i32) -> Img {
    let (w, h) = ((im.w + k - 1) / k, (im.h + k - 1) / k);
    let mut out = Img::new(w, h);
    for by in 0..h {
        for bx in 0..w {
            let mut seen: Vec<([u8; 3], u32)> = Vec::new();
            let mut solid = 0;
            for y in by * k..(by * k + k).min(im.h) {
                for x in bx * k..(bx * k + k).min(im.w) {
                    if let Some(c) = im.get(x, y) {
                        solid += 1;
                        match seen.iter_mut().find(|(s, _)| *s == c) {
                            Some(e) => e.1 += 1,
                            None => seen.push((c, 1)),
                        }
                    }
                }
            }
            if solid * 2 >= k * k && let Some((c, _)) = seen.iter().max_by_key(|(c, n)| (*n, u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2]))) {
                out.set(bx, by, *c);
            }
        }
    }
    out
}

/// The held weapon fitted to the stage by the largest whole ratio that fits (1, 1/2, 1/3, 1/4).
/// Returns the picture and the denominator, so the stage can say it is shrunk.
pub fn fitted(upright: &Img, max_w: i32, max_h: i32) -> (Img, i32) {
    for k in 1..=4 {
        let (w, h) = ((upright.w + k - 1) / k, (upright.h + k - 1) / k);
        if w <= max_w && h <= max_h {
            return (if k == 1 { upright.clone() } else { reduced(upright, k) }, k);
        }
    }
    (reduced(upright, 4), 4)
}

/// A glint travelling along the metal every few seconds, and a slow pulse in a gem: the idle
/// shimmer of the arms library's weapons. `age` is in ticks.
pub fn shimmer(im: &mut Img, tick: u64, gem: Option<School>) {
    const PERIOD: u64 = 96;
    let span = im.w + im.h + 24;
    let at = ((tick % PERIOD) as i32 * span / PERIOD as i32) - 12;
    let pulse = ((tick % 48) as f32 / 48.0 * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    for y in 0..im.h {
        for x in 0..im.w {
            let Some(c) = im.get(x, y) else { continue };
            let d = ((x + (im.h - 1 - y)) - at).abs();
            let lum = (u32::from(c[0]) + u32::from(c[1]) + u32::from(c[2])) / 3;
            if d <= 2 && lum > 70 {
                // A glint with a soft shoulder: bright in the middle, fading over two pixels.
                let k = [0.9, 0.55, 0.25][d as usize];
                im.set(x, y, c.map(|v| (f32::from(v) + (255.0 - f32::from(v)) * k) as u8));
            } else if gem.is_some() && lum > 150 && c[0].abs_diff(c[2]) > 40 {
                let k = pulse * 0.25;
                im.set(x, y, c.map(|v| (f32::from(v) + (255.0 - f32::from(v)) * k) as u8));
            }
        }
    }
}

/// What the stage says under the weapon.
pub fn kind_label(kind: WeaponKind) -> &'static str {
    match kind {
        WeaponKind::Blade => "BLADE",
        WeaponKind::Haft => "HAFT",
        WeaponKind::Bow => "BOW",
        WeaponKind::Shield => "SHIELD",
        WeaponKind::Staff => "STAFF",
        WeaponKind::Wand => "WAND",
    }
}
