//! The blank-slate realm's two places, drawn (`crate::world::plan`): the tiny
//! castle just begun, and a camp where each model house's castle will be.
//!
//! - **The Keep begun.** A squat tower half raised, its stump of a twin, low
//!   wall stubs with a gap for the gate, scaffolding up the tower, a hoist on
//!   top, a stone pile and a banner. Small enough to see whole: eight tiles
//!   by five on the ground, the tower rising over it.
//! - **A camp.** Two tents in the house's own colours (a plain grey while the
//!   house has not chosen any), a fire between them, the house's banner on
//!   its pole and its knight standing by the fire at home (`houses::stand`).
//!   No wall, no gate, nothing inside. The house's name is carved in the
//!   ground below.
//!
//! Every pixel is a fact, as the March's castles are: nothing here is a
//! signal ink, and a camp's fire burns low always and high while its house
//! serves.

use super::castles::{March, carving, colours, dress};
use super::ink::{Img, Rgb};
use super::kit::{self, Top, Wall};
use super::light::{Light, darker};
use super::map::TILE;
use super::scene::Prop;
use super::View;
use crate::world::houses::{self, House, HouseId};
use std::sync::OnceLock;

pub const KEEP_W: i32 = 8 * 16;
pub const KEEP_H: i32 = 96;

pub const CAMP_W: i32 = 7 * 16;
pub const CAMP_H: i32 = 4 * 16 + 12;

fn raw_wall(w: i32, h: i32, steps: &[(i32, i32, i32)]) -> Img {
    let mut im = kit::tower(w, h, 2, Top::Crenel, Wall::Stone);
    for &(x0, x1, cut) in steps {
        for x in x0..x1.min(w) {
            for y in 0..cut {
                im.clear(x, y);
            }
            im.put(x, cut, 'V');
            im.put(x, cut + 1, 'v');
        }
    }
    im
}

fn stones(w: i32, h: i32, seed: u32) -> Img {
    let mut im = Img::new(w, h);
    let (bw, bh) = (7, 4);
    let rows = (h / bh).max(1);
    for r in 0..rows {
        let n = ((w / bw) - r).max(1);
        let y = h - (r + 1) * bh;
        let x0 = (w - n * bw) / 2;
        for k in 0..n {
            let x = x0 + k * bw + (super::ink::hash(k, r, seed) % 3) as i32 - 1;
            let top = ['v', 'U', 'v'][(super::ink::hash(k, r, seed + 1) % 3) as usize];
            im.rect(x, y, bw - 1, bh, 'u');
            im.rect(x, y, bw - 1, 1, top);
            im.rect(x + bw - 2, y + 1, 1, bh - 1, 'S');
            im.rect(x, y + bh - 1, bw - 1, 1, 'x');
        }
    }
    kit::shaded_rim(&im)
}

fn scaffold(w: i32, h: i32) -> Img {
    let mut im = Img::new(w, h);
    im.line(0, 0, 0, h - 1, 'r');
    im.line(w - 1, 0, w - 1, h - 1, 'r');
    let mut y = h - 12;
    let mut flip = false;
    while y > 2 {
        im.rect(0, y, w, 2, 'o');
        im.line(
            if flip { 0 } else { w - 1 },
            y,
            if flip { w - 1 } else { 0 },
            (y - 12).max(0),
            'P',
        );
        flip = !flip;
        y -= 12;
    }
    im.put(0, 0, 'O');
    im.put(w - 1, 0, 'O');
    im
}

fn gate_frame(w: i32, h: i32) -> Img {
    let mut im = Img::new(w, h);
    for x in [0, 1, w - 2, w - 1] {
        im.line(x, 0, x, h - 1, if x % 2 == 0 { 'r' } else { 'B' });
    }
    im.rect(0, 0, w, 3, 'o');
    im.rect(0, 3, w, 1, 'B');
    im.line(2, 4, 6, 8, 'P');
    im.line(w - 3, 4, w - 7, 8, 'P');
    im
}

fn hoist() -> Img {
    let mut im = Img::new(22, 30);
    im.line(5, 29, 5, 3, 'r');
    im.line(6, 29, 6, 3, 'B');
    im.line(5, 4, 20, 8, 'o');
    im.line(5, 5, 20, 9, 'B');
    im.line(7, 16, 6, 7, 'P');
    im.line(19, 8, 19, 22, 'J');
    im.rect(17, 22, 5, 4, 'U');
    im.rect(17, 25, 5, 1, 'S');
    im.put(5, 2, 'O');
    im
}

pub fn keep_begun() -> &'static Img {
    static DRAWN: OnceLock<Img> = OnceLock::new();
    DRAWN.get_or_init(keep_begun_drawn)
}

fn keep_begun_drawn() -> Img {
    let (w, h) = (KEEP_W, KEEP_H);
    let mut im = Img::new(w, h);
    im.stamp(&raw_wall(112, 20, &[(0, 30, 5), (30, 62, 9), (62, 88, 3), (88, 112, 7)]), 8, 24);
    im.stamp(&raw_wall(32, 26, &[(0, 12, 4), (12, 32, 9)]), 84, 36);
    im.stamp(&raw_wall(38, 50, &[(0, 14, 6), (14, 26, 12), (26, 38, 4)]), 12, 20);
    for (x, y) in [(22, 38), (36, 44)] {
        im.rect(x, y, 2, 6, 'K');
    }
    im.rect(26, 60, 10, 10, 'K');
    for y in [62, 66] {
        im.rect(25, y, 12, 2, 'o');
    }
    im.stamp(&scaffold(10, 40), 52, 28);
    im.stamp(&scaffold(10, 26), 2, 40);
    im.stamp(&hoist(), 19, 0);
    im.stamp(&stones(30, 13, 71), 90, 62);
    im.rect(78, 74, 7, 3, 'r');
    im.line(76, 75, 70, 71, 'B');
    im.rect(80, 77, 2, 2, 'k');
    im.stamp(&raw_wall(40, 18, &[(0, 14, 4), (14, 40, 8)]), 0, h - 18);
    im.stamp(&raw_wall(34, 14, &[(0, 20, 5), (20, 34, 3)]), 94, h - 14);
    im.stamp(&gate_frame(30, 32), 49, h - 32);
    for (x, y) in [(42, h - 5), (46, h - 4), (88, h - 3), (36, h - 24)] {
        im.rect(x, y, 5, 3, 'U');
        im.rect(x, y + 2, 5, 1, 'x');
        im.put(x, y, 'v');
    }
    im.line(46, 1, 46, 26, 'h');
    im.line(47, 1, 47, 26, 'J');
    im.put(46, 0, 'H');
    im.rect(48, 1, 14, 8, '7');
    im.rect(48, 7, 14, 2, '8');
    im.rect(53, 3, 4, 3, '5');
    im.put(61, 4, '8');
    im.clear(61, 5);
    kit::shaded_rim(&im)
}

fn cloth(h: &House) -> (Rgb, Rgb, Rgb) {
    colours(h)
}

pub fn house_cloth(h: &House) -> Rgb {
    cloth(h).0
}

pub fn keep_cloth() -> Rgb {
    super::ink::ink(super::light::dusk_ink('7')).unwrap_or_default()
}

fn lighter(c: Rgb) -> Rgb {
    use super::ink::{bank_colours, bank_of, is_signal};
    let Some(bank) = bank_of(c).filter(|_| !is_signal(c)) else {
        return c;
    };
    let lum = |p: Rgb| 0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2]);
    let target = [f32::from(c[0]) * 1.5, f32::from(c[1]) * 1.5, f32::from(c[2]) * 1.5];
    bank_colours(bank)
        .iter()
        .map(|&v| [(v >> 16) as u8, (v >> 8) as u8, v as u8])
        .filter(|&p| lum(p) > lum(c) + 1.0)
        .min_by(|a, b| {
            let d = |p: &Rgb| (0..3).map(|i| (f32::from(p[i]) - target[i]).powi(2)).sum::<f32>();
            d(a).total_cmp(&d(b))
        })
        .unwrap_or(c)
}

fn tent(w: i32, h: i32, h_house: &House, seed: u32) -> Img {
    let (field, _, charge) = cloth(h_house);
    let mut im = Img::new(w, h);
    let half = w as f32 / 2.0;
    for y in 0..h {
        let reach = (y as f32 + 1.0) / h as f32 * half;
        for x in 0..w {
            let d = x as f32 + 0.5 - half;
            if d.abs() <= reach {
                let seam = d.abs() < 0.6;
                let c = if d > reach * 0.15 { field } else { lighter(field) };
                let c = if seam { darker(c) } else { c };
                im.set(x, y, c);
            }
        }
    }
    for y in h - 3..h - 1 {
        for x in 0..w {
            if im.get(x, y).is_some() {
                im.set(x, y, if (x / 3 + seed as i32) % 2 == 0 { charge } else { darker(charge) });
            }
        }
    }
    let dw = (w / 6).max(2);
    for y in h / 2..h {
        for x in (w / 2 - dw / 2)..(w / 2 + dw / 2 + 1) {
            if im.get(x, y).is_some() {
                im.put(x, y, 'K');
            }
        }
    }
    im.line(w / 2, 0, w / 2, 3, 'h');
    im.put(w / 2, 0, 'H');
    im = kit::shaded_rim(&im);
    im
}

fn fire(frame: u32) -> Img {
    let mut im = Img::new(14, 14);
    for k in 0..10 {
        let a = k as f32 / 10.0 * std::f32::consts::TAU;
        im.put(7 + (a.cos() * 6.0) as i32, 10 + (a.sin() * 2.6) as i32, if k % 2 == 0 { 'u' } else { 'U' });
    }
    im.line(3, 11, 10, 8, 'B');
    im.line(3, 8, 10, 11, 'r');
    im.line(4, 10, 9, 10, 'p');
    if frame > 0 {
        let f = (frame % 3) as i32;
        im.rect(5, 5, 4, 4, '@');
        im.rect(6, 3 - f.min(1), 2, 3, '5');
        im.put(6 + f % 2, 1 - f / 2, '6');
        im.put(7, 4, '6');
    } else {
        for x in 5..9 {
            im.put(x, 9, 'X');
        }
    }
    im
}

fn camp_banner(h: &House, wave: u32) -> Img {
    let (field, shadow, charge) = cloth(h);
    let mut im = Img::new(16, 36);
    im.line(1, 2, 1, 35, 'J');
    im.line(2, 2, 2, 35, 'G');
    im.put(1, 0, 'H');
    im.put(1, 1, 'h');
    for dx in 3..15 {
        let ripple = i32::from((dx + wave as i32) % 5 == 0);
        for dy in 0..12 {
            if dx >= 12 && (4..8).contains(&dy) && dx - 12 >= 3 - (dy - 4).min(7 - dy) {
                continue;
            }
            im.set(dx, 3 + dy + ripple, if dy >= 10 { shadow } else { field });
        }
    }
    for (cx, cy) in [(7, 5), (6, 6), (8, 6), (7, 7), (7, 6)] {
        let ripple = i32::from((cx + wave as i32) % 5 == 0);
        im.set(cx + 1, 3 + cy + ripple, charge);
    }
    im
}

fn camp_ground(h: &House, wave: u32) -> Img {
    let mut im = Img::new(CAMP_W, CAMP_H);
    let base = CAMP_H - 14;
    im.stamp(&tent(40, 30, h, 0), 14, base - 40);
    im.stamp(&tent(32, 24, h, 1), 62, base - 28);
    im.rect(8, base - 5, 10, 4, 'P');
    im.rect(8, base - 5, 10, 1, 'o');
    im.rect(56, base - 9, 6, 8, 'B');
    im.rect(56, base - 9, 6, 1, 'o');
    im.rect(56, base - 5, 6, 1, 'g');
    im.stamp(&camp_banner(h, wave), CAMP_W - 24, base - 44);
    kit::shaded_rim(&im)
}

fn idle(id: HouseId) -> &'static Img {
    static IDLE: OnceLock<Vec<Img>> = OnceLock::new();
    &IDLE.get_or_init(|| houses::ids().map(|id| camp_ground(houses::get(id), 0)).collect())[usize::from(id.0)]
}

pub fn stage(march: &March, props: &mut Vec<Prop>, lights: &mut Vec<Light>, tick: u32, view: Option<View>) {
    let shows = |x: i32, base: i32, img: &Img| view.is_none_or(|v| v.touches(x, base - img.h, img.w, img.h));
    let breath = |k: u32| 0.92 + 0.08 * (tick as f32 * 0.5 + k as f32 * 2.3).sin();
    for id in houses::ids() {
        let h = houses::get(id);
        let (tx, ty, tw, th) = houses::footprint(id);
        let raised = march.raised.contains(&id);
        let camp = if raised { camp_ground(h, tick / 3) } else { idle(id).clone() };
        let (x, base) = (tx * TILE + (tw * TILE - camp.w) / 2, (ty + th) * TILE + 2);
        if shows(x, base, &camp) {
            props.push(Prop { x, base, img: camp });
            let flame = fire(1 + (tick / 4 + u32::from(id.0)) % 3);
            let (fx, fy) = (tx * TILE + tw * TILE / 2 - 7 + 6, base - 2);
            props.push(Prop { x: fx, base: fy, img: flame });
            lights.push(Light {
                x: (fx + 7) as f32,
                y: (fy - 6) as f32,
                r: if raised { 40.0 } else { 24.0 },
                s: if raised { 0.55 } else { 0.32 } * breath(u32::from(id.0)),
                fire: true,
            });
            if !raised {
                let (sx, sy) = houses::stand(id);
                let knight = dress(&kit::knight(), Some(id));
                props.push(Prop { x: sx * TILE + TILE / 2 - knight.w / 2 + 26, base: sy * TILE + TILE - 6, img: knight });
            }
        }
        let label = carving(h);
        let (sx, sy) = houses::stand(id);
        props.push(Prop { x: sx * TILE + TILE / 2 - label.w / 2, base: (sy + 1) * TILE + 2 + label.h, img: label });
    }
}
