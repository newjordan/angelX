//! The March: the model houses' castles in the realm's south.
//!
//! One great castle per model family (`crate::world::houses`), each a
//! curtain wall round its whole footprint with a tower at every corner and
//! its gate in the south face on the March's roads, its own keep standing
//! in the ward behind. Each is built the way its own model chose (walls,
//! tops, towers, roof), and its name is carved below the road. The Keep in
//! Castle Town and the Dragon Keep get the same wall round their own
//! buildings (`crate::world::castles`).
//!
//! Every pixel is a fact:
//! - a castle's banner flies only while its house is serving — the route in
//!   hand, or a seat a formation gave it — and its windows and gate are lit
//!   then; an idle castle stands dark with a bare pole;
//! - the serving knight wears his house's colours and rides out of that
//!   castle to the work, and home to it when the work is done;
//! - when the club changes, the old knight rides home while a knight of the
//!   new house rides out;
//! - houses a formation seats ride out of their own castles to join the
//!   lead knight, and ride home when the formation stands down.

use super::ink::{Img, Rgb, ink};
use super::kit::{self, Roof, Top, Wall};
use super::light::{Light, darker, dusk_ink};
use super::map::TILE;
use super::scene::Prop;
use super::View;
use crate::world::castles::{self, Castle, CastleKind};
use crate::world::houses::{self, House, HouseId, RoofKind, Tops, Walls};

pub use crate::world::houses::{footprint, footprints, stand};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiderPose {
    pub house: HouseId,
    pub x: f32,
    pub y: f32,
    pub walking: bool,
    pub facing_left: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct March {
    pub raised: Vec<HouseId>,
    pub lead: Option<HouseId>,
    pub riders: Vec<RiderPose>,
    pub watch: Vec<RiderPose>,
}

impl std::hash::Hash for March {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        (&self.raised, self.lead).hash(h);
        for r in self.riders.iter().chain(&self.watch) {
            (
                r.house,
                r.x.to_bits(),
                r.y.to_bits(),
                r.walking,
                r.facing_left,
            )
                .hash(h);
        }
    }
}

pub(super) fn colours(h: &House) -> (Rgb, Rgb, Rgb) {
    let field = ink(dusk_ink(h.field)).unwrap_or_default();
    let charge = ink(dusk_ink(h.charge)).unwrap_or_default();
    (field, darker(field), charge)
}

fn swap(img: &Img, pairs: &[(Rgb, Rgb)]) -> Img {
    let mut out = img.clone();
    for y in 0..img.h {
        for x in 0..img.w {
            if let Some(c) = img.get(x, y)
                && let Some(&(_, to)) = pairs.iter().find(|(from, _)| *from == c)
            {
                out.set(x, y, to);
            }
        }
    }
    out
}

pub fn dress(img: &Img, house: Option<HouseId>) -> Img {
    let Some(id) = house else {
        return img.clone();
    };
    let h = houses::get(id);
    if h.placeholder {
        return img.clone();
    }
    let (field, dark, charge) = colours(h);
    let pairs: Vec<(Rgb, Rgb)> = [('7', field), ('8', dark), ('5', charge)]
        .into_iter()
        .filter_map(|(from, to)| Some((ink(from)?, to)))
        .collect();
    swap(img, &pairs)
}

fn wall_of(h: &House) -> Wall {
    match h.walls {
        Walls::PaleStone => Wall::Stone,
        Walls::DarkStone => Wall::Dark,
        Walls::Plaster => Wall::Plaster,
        Walls::Timber => Wall::Timber,
    }
}

fn top_of(h: &House) -> Top {
    match h.tops {
        Tops::Battlements => Top::Crenel,
        Tops::Domes => Top::Dome(false),
        Tops::Pointed => Top::Cone(match h.roof {
            RoofKind::Slate => Roof::Slate,
            RoofKind::Tile => Roof::Tile,
            RoofKind::Thatch => Roof::Thatch,
        }),
    }
}

pub const CASTLE_W: i32 = 48;
pub const CASTLE_H: i32 = 60;

fn banner(im: &mut Img, x: i32, y: i32, h: &House, raised: bool, wave: u32) {
    im.line(x, y, x, y + 11, 'J');
    im.put(x, y - 1, 'h');
    if !raised {
        return;
    }
    let (field, dark, charge) = colours(h);
    for dx in 1..=8 {
        let ripple = i32::from((dx + wave as i32) % 4 == 0);
        for dy in 0..5 {
            if dx == 8 && dy == 2 {
                continue;
            }
            let c = if dy == 4 { dark } else { field };
            im.set(x + dx, y + dy + ripple, c);
        }
    }
    for (cx, cy) in [(4, 1), (3, 2), (5, 2), (4, 3), (4, 2)] {
        let ripple = i32::from((cx + wave as i32) % 4 == 0);
        im.set(x + cx, y + cy + ripple, charge);
    }
}

pub fn sprite(h: &House, raised: bool, tick: u32) -> Img {
    let mut im = Img::new(CASTLE_W, CASTLE_H);
    let (wall, top) = (wall_of(h), top_of(h));
    let window = if raised { '5' } else { 'K' };
    let wave = tick / 3;
    let base = CASTLE_H;
    match h.towers {
        3 => {
            let keep = kit::tower(16, 34, 12, top, wall);
            im.stamp(&keep, 16, base - 46);
            im.rect(23, base - 34, 2, 3, window);
            banner(&mut im, 24, base - 58, h, raised, wave);
            im.stamp(&kit::tower(30, 18, 6, Top::Crenel, wall), 9, base - 18);
            let side = kit::tower(11, 30, 10, top, wall);
            im.stamp(&side, 1, base - 30);
            im.stamp(&side, 36, base - 30);
            im.rect(6, base - 17, 1, 3, window);
            im.rect(41, base - 17, 1, 3, window);
            kit::arch(&mut im, 19, base - 11, 10, 11, !raised);
        }
        2 => {
            let hall = kit::tower(26, 28, 10, top, wall);
            im.stamp(&hall, 11, base - 40);
            im.rect(17, base - 26, 2, 3, window);
            im.rect(29, base - 26, 2, 3, window);
            banner(&mut im, 24, base - 54, h, raised, wave);
            im.stamp(&kit::tower(24, 16, 6, Top::Crenel, wall), 12, base - 16);
            let side = kit::tower(12, 34, 10, top, wall);
            im.stamp(&side, 1, base - 34);
            im.stamp(&side, 35, base - 34);
            im.rect(6, base - 21, 1, 3, window);
            im.rect(40, base - 21, 1, 3, window);
            kit::arch(&mut im, 19, base - 11, 10, 11, !raised);
        }
        _ => {
            let keep = kit::tower(18, 40, 12, top, wall);
            im.stamp(&keep, 15, base - 50);
            im.rect(23, base - 36, 2, 3, window);
            banner(&mut im, 24, base - 60, h, raised, wave);
            im.stamp(&kit::tower(34, 14, 6, Top::Crenel, wall), 7, base - 14);
            kit::arch(&mut im, 19, base - 11, 10, 11, !raised);
        }
    }
    im
}

fn idle(id: HouseId) -> &'static Img {
    static IDLE: std::sync::OnceLock<Vec<Img>> = std::sync::OnceLock::new();
    &IDLE.get_or_init(|| {
        houses::ids()
            .map(|id| sprite(houses::get(id), false, 0))
            .collect()
    })[usize::from(id.0)]
}

pub(super) fn carving(h: &House) -> Img {
    let word = h.label();
    let mut im = Img::new(super::ink::text_width(&word) + 1, 8);
    im.text(0, 0, &word, 'u');
    im
}

type Piece = (i32, i32, Img);

fn walls(c: &Castle, wall: Wall, top: Top, open: &[i32], broken: bool, gate_lit: bool) -> Vec<Piece> {
    let (tx, ty, tw, th) = c.foot;
    let (x0, y0, w, h) = (tx * TILE, ty * TILE, tw * TILE, th * TILE);
    let ruin = |mut im: Img, seed: u32| {
        if broken {
            for x in 0..im.w {
                let cut = (super::ink::vnoise(x as f32 / 5.0, seed as f32, 93) * 16.0) as i32;
                for y in 0..cut.min(im.h / 2) {
                    im.clear(x, y);
                }
            }
            for y in 0..im.h {
                for x in 0..im.w {
                    if super::ink::hash(x, y, 94 + seed).is_multiple_of(29) {
                        im.clear(x, y);
                    }
                }
            }
        }
        im
    };
    let mut out: Vec<Piece> = Vec::new();
    out.push((x0, y0 + TILE, ruin(kit::tower(w, 34, 10, Top::Crenel, wall), 1)));
    let side = |seed| ruin(kit::tower(12, h - 8, h - 18, Top::Crenel, wall), seed);
    out.push((x0, y0 + h - 1, side(2)));
    out.push((x0 + w - 12, y0 + h - 1, side(3)));
    let mut front = kit::tower(w, 40, 10, Top::Crenel, wall);
    let (gx, _, gw, _) = c.gate;
    if !open.contains(&gx) {
        kit::arch(&mut front, (gx - tx) * TILE + 4, 40 - 26, gw * TILE - 8, 26, !gate_lit);
    }
    for &col in open {
        for y in 0..front.h {
            for x in (col - tx) * TILE..(col - tx + 1) * TILE {
                front.clear(x, y);
            }
        }
    }
    out.push((x0, y0 + h, ruin(front, 4)));
    let corner = kit::tower(22, 58, 12, top, wall);
    out.push((x0 - 3, y0 + TILE + 2, ruin(corner.clone(), 5)));
    out.push((x0 + w - 19, y0 + TILE + 2, ruin(corner.clone(), 6)));
    if !open.contains(&tx) {
        out.push((x0 - 3, y0 + h + 2, ruin(corner.clone(), 7)));
    }
    if !open.contains(&(tx + tw - 1)) {
        out.push((x0 + w - 19, y0 + h + 2, ruin(corner, 8)));
    }
    if matches!(c.kind, CastleKind::House(_)) {
        let flank = kit::tower(16, 50, 10, top, wall);
        out.push(((gx - tx) * TILE + x0 - 12, y0 + h + 2, flank.clone()));
        out.push(((gx + gw - tx) * TILE + x0 - 4, y0 + h + 2, flank));
    }
    out
}

fn open_front(c: &Castle) -> Vec<i32> {
    let (tx, ty, tw, th) = c.foot;
    let mut cols: Vec<i32> = super::map::STRUCTURES
        .iter()
        .map(|&(ax, ay, w, h)| {
            let (x, y) = super::map::place_tile(ax, ay);
            (x, y, w, h)
        })
        .filter(|&(x, y, w, h)| x >= tx && y >= ty && x + w <= tx + tw && y + h <= ty + th)
        .flat_map(|(x, _, w, _)| x..x + w)
        .collect();
    cols.sort_unstable();
    cols.dedup();
    cols
}

fn built() -> &'static [[Vec<Piece>; 2]] {
    static WALLS: std::sync::OnceLock<Vec<[Vec<Piece>; 2]>> = std::sync::OnceLock::new();
    WALLS.get_or_init(|| {
        castles::castles()
            .iter()
            .map(|c| {
                let (wall, top, broken) = match c.kind {
                    CastleKind::House(id) => {
                        let h = houses::get(id);
                        (wall_of(h), top_of(h), false)
                    }
                    CastleKind::Keep => (Wall::Stone, Top::Crenel, false),
                    CastleKind::DragonKeep => (Wall::Dark, Top::Crenel, true),
                };
                let open = open_front(c);
                [false, true].map(|lit| walls(c, wall, top, &open, broken, lit))
            })
            .collect()
    })
}

fn carving_row((sx, sy): (i32, i32)) -> i32 {
    let realm = super::map::Realm::get();
    (sy + 1..sy + 4)
        .find(|&y| !matches!(realm.at(sx, y), b'=' | b'H'))
        .unwrap_or(sy + 2)
}

pub fn stage(march: &March, props: &mut Vec<Prop>, lights: &mut Vec<Light>, tick: u32, view: Option<View>) {
    if crate::world::plan::tabula() {
        return super::camps::stage(march, props, lights, tick, view);
    }
    let breath = |k: u32| 0.92 + 0.08 * (tick as f32 * 0.5 + k as f32 * 2.3).sin();
    let shows = |x: i32, base: i32, img: &Img| view.is_none_or(|v| v.touches(x, base - img.h, img.w, img.h));
    for (c, pieces) in castles::castles().iter().zip(built()) {
        let raised = match c.kind {
            CastleKind::House(id) => march.raised.contains(&id),
            _ => false,
        };
        for (x, base, img) in &pieces[usize::from(raised)] {
            if !shows(*x, *base, img) {
                continue;
            }
            props.push(Prop {
                x: *x,
                base: *base,
                img: img.clone(),
            });
        }
    }
    for id in houses::ids() {
        let h = houses::get(id);
        let raised = march.raised.contains(&id);
        let (tx, ty, tw, th) = footprint(id);
        let base = (ty + th / 2 + 2) * TILE;
        let place = |img: &Img| (tx * TILE + (tw * TILE - img.w) / 2, base);
        if raised {
            let img = sprite(h, true, tick + u32::from(id.0) * 5);
            let (x, base) = place(&img);
            props.push(Prop { x, base, img });
        } else {
            let img = idle(id);
            let (x, base) = place(img);
            if shows(x, base, img) {
                props.push(Prop { x, base, img: img.clone() });
            }
        }
        let label = carving(h);
        let (sx, sy) = stand(id);
        props.push(Prop {
            x: sx * TILE + TILE / 2 - label.w / 2,
            base: carving_row((sx, sy)) * TILE + 2 + label.h,
            img: label,
        });
        if raised {
            lights.push(Light {
                x: (sx * TILE + TILE / 2) as f32,
                y: (sy * TILE - 8) as f32,
                r: 30.0,
                s: 0.5 * breath(u32::from(id.0)),
                fire: true,
            });
            lights.push(Light {
                x: (sx * TILE + TILE / 2) as f32,
                y: (base - 34) as f32,
                r: 22.0,
                s: 0.3,
                fire: true,
            });
        }
    }
    for r in march.riders.iter().chain(&march.watch) {
        let img = dress(&kit::knight(), Some(r.house));
        let img = if r.facing_left { img.flip_h() } else { img };
        let bob = i32::from(r.walking && ((r.x + r.y) as i32 / 4) % 2 == 1);
        let prop = Prop {
            x: r.x as i32 - 8,
            base: r.y as i32 + 2 - bob,
            img,
        };
        props.push(prop);
        lights.push(Light {
            x: r.x,
            y: r.y - 8.0,
            r: 30.0,
            s: 0.36,
            fire: false,
        });
    }
}

#[cfg(test)]
#[path = "../../../tests/render/overworld__castles_tests.rs"]
mod tests;
