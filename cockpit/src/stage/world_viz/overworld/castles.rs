//! The March: the model houses' castles in the realm's south.
//!
//! One small castle per model family (`crate::stage::houses`), two to a
//! March screen, each gate on a path down to the March road. Each castle is
//! built the way its own model chose (walls, tops, towers, roof), and its
//! name is carved below the road.
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

use super::ink::{Img, ink};
use super::kit::{self, Roof, Top, Wall};
use super::light::Light;
use super::map::{SCREEN_H, SCREEN_W, TILE, place_tile};
use super::scene::Prop;
use crate::stage::houses::{self, House, HouseId, RoofKind, Tops, Walls};

/// The castle's footprint, in authored tiles, for house index `i`: the first
/// eight houses on the near March row, the rest on the far one; two to a
/// screen, west slot then east slot.
fn authored_slot(i: usize) -> (i32, i32) {
    let (row, within) = ((i / 8) as i32, i % 8);
    let ax = (within / 2) as i32;
    let lx = if within % 2 == 0 { 2 } else { 10 };
    (
        ax * SCREEN_W + lx,
        (super::map::HEARTLAND_Y + row) * SCREEN_H + 2,
    )
}

/// Footprint width and depth in tiles.
pub(crate) const FOOT: (i32, i32) = (3, 2);

/// A castle's footprint in realm tiles `(tx, ty, tw, th)`.
pub(crate) fn footprint(id: HouseId) -> (i32, i32, i32, i32) {
    let (ax, ay) = authored_slot(usize::from(id.0));
    let (tx, ty) = place_tile(ax, ay);
    (tx, ty, FOOT.0, FOOT.1)
}

/// The road tile before a castle's gate, where its knight stands at home.
pub(crate) fn stand(id: HouseId) -> (i32, i32) {
    let (tx, ty, tw, th) = footprint(id);
    (tx + tw / 2, ty + th)
}

/// Every castle's footprint, for the walker's closed ground.
pub(crate) fn footprints() -> impl Iterator<Item = (i32, i32, i32, i32)> {
    houses::ids().map(footprint)
}

/// A knight of another house out on the roads, as the map draws him.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RiderPose {
    pub(crate) house: HouseId,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) walking: bool,
    pub(crate) facing_left: bool,
}

/// What the March shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct March {
    /// Houses with a banner up: the lead and every seated house.
    pub(crate) raised: Vec<HouseId>,
    /// The serving knight's house; he wears its colours.
    pub(crate) lead: Option<HouseId>,
    /// Knights of other houses on the roads: a formation's seats riding
    /// with the lead, or a relieved knight riding home.
    pub(crate) riders: Vec<RiderPose>,
}

impl std::hash::Hash for March {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        (&self.raised, self.lead).hash(h);
        for r in &self.riders {
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

/// The darker partner of a signal ink, for the shadowed side of a tabard.
fn shadow(c: char) -> char {
    match c {
        '1' | '2' | '0' => '0',
        '3' => '2',
        'w' => '3',
        'a' | '8' | '7' => '8',
        '4' | '@' | '$' => 'a',
        '5' | '6' => '4',
        'c' | '9' => '$',
        other => other,
    }
}

/// Swap inks all at once (a chain like `7→5, 5→4` must not run on).
fn swap(img: &Img, pairs: &[(char, char)]) -> Img {
    let pairs: Vec<_> = pairs
        .iter()
        .filter_map(|&(f, t)| Some((ink(f)?, ink(t)?)))
        .collect();
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

/// Dress a figure in house colours: its red becomes the field, the gold
/// star on its shield the charge. With no house he keeps the Keep's red.
pub(crate) fn dress(img: &Img, house: Option<HouseId>) -> Img {
    let Some(id) = house else {
        return img.clone();
    };
    let h = houses::get(id);
    if h.placeholder {
        return img.clone();
    }
    swap(
        img,
        &[('7', h.field), ('8', shadow(h.field)), ('5', h.charge)],
    )
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

/// Sprite size: three tiles wide, tall enough for the pole above the
/// highest tower.
pub(crate) const CASTLE_W: i32 = 48;
pub(crate) const CASTLE_H: i32 = 60;

/// The house banner on its pole, flying while the house serves; `wave`
/// ripples the fly end.
fn banner(im: &mut Img, x: i32, y: i32, h: &House, raised: bool, wave: u32) {
    // The pole is part of the castle: it stands whether or not a banner flies.
    im.line(x, y, x, y + 11, 'J');
    im.put(x, y - 1, 'h');
    if !raised {
        return;
    }
    let (field, charge) = (h.field, h.charge);
    let dark = shadow(field);
    for dx in 1..=8 {
        // A swallowtail: the fly end is cut back in its middle row.
        let ripple = i32::from((dx + wave as i32) % 4 == 0);
        for dy in 0..5 {
            if dx == 8 && dy == 2 {
                continue;
            }
            let c = if dy == 4 { dark } else { field };
            im.put(x + dx, y + dy + ripple, c);
        }
    }
    // The charge: a small lozenge in the banner's middle.
    for (cx, cy) in [(4, 1), (3, 2), (5, 2), (4, 3), (4, 2)] {
        let ripple = i32::from((cx + wave as i32) % 4 == 0);
        im.put(x + cx, y + cy + ripple, charge);
    }
}

/// A castle of the March, built as its house chose. `raised`: the house is
/// serving — banner up, windows and gate lit.
pub(crate) fn sprite(h: &House, raised: bool, tick: u32) -> Img {
    let mut im = Img::new(CASTLE_W, CASTLE_H);
    let (wall, top) = (wall_of(h), top_of(h));
    let window = if raised { '5' } else { 'K' };
    let wave = tick / 3;
    let base = CASTLE_H;
    match h.towers {
        3 => {
            // A tall keep behind, a curtain wall, a tower at each front corner.
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
            // A hall between two gate towers.
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
            // One tower behind a low wall.
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

/// An idle castle never moves: built once.
fn idle(id: HouseId) -> &'static Img {
    static IDLE: std::sync::OnceLock<Vec<Img>> = std::sync::OnceLock::new();
    &IDLE.get_or_init(|| {
        houses::ids()
            .map(|id| sprite(houses::get(id), false, 0))
            .collect()
    })[usize::from(id.0)]
}

/// A castle's name carved by the road: quiet stone ink on black paper.
fn carving(h: &House) -> Img {
    let word = h.label();
    let mut im = Img::new(super::ink::text_width(&word) + 1, 8);
    im.text(0, 0, &word, 'u');
    im
}

/// Stage the March in realm coordinates: castles, banners, carvings,
/// lamplight, and knights of other houses out on the roads.
pub(crate) fn stage(march: &March, props: &mut Vec<Prop>, lights: &mut Vec<Light>, tick: u32) {
    let breath = |k: u32| 0.92 + 0.08 * (tick as f32 * 0.5 + k as f32 * 2.3).sin();
    for id in houses::ids() {
        let h = houses::get(id);
        let raised = march.raised.contains(&id);
        let (tx, ty, tw, th) = footprint(id);
        let img = if raised {
            sprite(h, true, tick + u32::from(id.0) * 5)
        } else {
            idle(id).clone()
        };
        let (x, base) = (tx * TILE + (tw * TILE - img.w) / 2, (ty + th) * TILE);
        props.push(Prop { x, base, img });
        let label = carving(h);
        let (sx, sy) = stand(id);
        props.push(Prop {
            x: sx * TILE + TILE / 2 - label.w / 2,
            base: (sy + 2) * TILE + 2 + label.h,
            img: label,
        });
        if raised {
            // The gate and the hall burn while the house serves.
            lights.push(Light {
                x: (sx * TILE + TILE / 2) as f32,
                y: (base - 8) as f32,
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
    for r in &march.riders {
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
#[path = "../../../../../tests/cockpit/world_viz/overworld__castles_tests.rs"]
mod tests;
