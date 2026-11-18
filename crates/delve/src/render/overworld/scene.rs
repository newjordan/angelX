//! The scene: everything the map shows, as plain data, and how it is staged
//! onto the default realm.
//!
//! A [`Scene`] is the whole contract between the live world and the pixel
//! map. It holds facts (which place is live, who sits at the council, what
//! tier the town has earned) and never pixels; [`stage`] turns it into
//! sprites, lights and beacons at fixed places on the realm.

use super::deeds::{self, Errand, Record, Wayfarer};
use super::glass::Glass;
use super::ink::{Img, hash};
use super::kit::{self, Heraldry, House, Roof, Tool, Wall};
use super::light::{DUSK, Light};
use super::map::{Place, TILE, place_px};
use super::outcomes::{self, Echo, Seal};
use super::View;

#[derive(Clone, Debug, PartialEq)]
pub struct Joust {
    pub red: String,
    pub blue: String,
    pub red_score: u32,
    pub blue_score: u32,
    pub charge: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ward {
    pub name: String,
    pub banner: char,
    pub lit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoldierKind {
    Judge,
    Verify,
    Quorum,
    Layer,
    Foot,
}

impl SoldierKind {
    pub fn of_stage(name: &str) -> SoldierKind {
        let lower = name.to_ascii_lowercase();
        if lower.contains("judge") {
            SoldierKind::Judge
        } else if lower.contains("verify") {
            SoldierKind::Verify
        } else if lower.contains("quorum") {
            SoldierKind::Quorum
        } else if lower.contains("layer") || lower.contains("moa") {
            SoldierKind::Layer
        } else {
            SoldierKind::Foot
        }
    }

    pub fn ink(self) -> char {
        match self {
            SoldierKind::Judge => '3',
            SoldierKind::Verify => '1',
            SoldierKind::Quorum => '2',
            SoldierKind::Layer => '@',
            SoldierKind::Foot => '5',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoldierState {
    Running,
    Returned,
    Failed,
    Cut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Soldier {
    pub kind: SoldierKind,
    pub state: SoldierState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weather {
    Fair,
    Clouds,
    Drizzle,
    Rain,
    Storm,
    Rainbow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Full,
    Reduced,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisitorPose {
    pub x: f32,
    pub y: f32,
    pub walking: bool,
    pub facing_left: bool,
}

pub trait Overlay: std::fmt::Debug + Send + Sync {
    fn paint_ground(&self, cv: &mut Img, view: View);
    fn stage(&self, props: &mut Vec<Prop>, cues: &mut Vec<Prop>);
    fn paint_top(&self, cv: &mut Img, scene: &Scene, view: View);
    fn key(&self, h: &mut dyn std::hash::Hasher);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knight {
    pub x: f32,
    pub y: f32,
    pub walking: bool,
}

impl Knight {
    pub fn at_place(place: Place) -> Knight {
        let (tx, ty) = place.stand_world();
        Knight {
            x: (tx * TILE + TILE / 2) as f32,
            y: ((ty + 1) * TILE - 2) as f32,
            walking: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub tier: u32,
    pub active: Option<Place>,
    pub tool: Option<Tool>,
    pub knight: Knight,
    pub visitor: Option<VisitorPose>,
    pub camera: (f32, f32),
    pub council: Vec<(usize, char)>,
    pub quest: Option<(u32, u32)>,
    pub joust: Option<Joust>,
    pub chapel_lit: bool,
    pub cottages: [bool; 3],
    pub forge_hot: bool,
    pub wards: Vec<Ward>,
    pub muster: Vec<Soldier>,
    pub region: Option<Place>,
    pub weather: Weather,
    pub fireworks: bool,
    pub glass: Option<Glass>,
    pub party: Vec<(f32, f32)>,
    pub dragon: bool,
    pub chests: (u32, u32),
    pub wisps: bool,
    pub wayfarers: Vec<Wayfarer>,
    pub record: Record,
    pub sparks: Option<u32>,
    pub outcomes: Vec<Echo>,
    pub outcome_motion: Motion,
    pub stargazing: bool,
    pub overlay: Option<std::sync::Arc<dyn Overlay>>,
    pub school: super::school::SchoolSnapshot,
    pub school_room: Option<super::school::Room>,
    pub wishes: std::sync::Arc<Vec<crate::realm::Wish>>,
    pub delve_lit: bool,
    pub delve_boons: u8,
    pub march: super::castles::March,
    pub ambient: f32,
    pub tick: u32,
}

impl Scene {
    pub fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (
            self.tier,
            self.active,
            self.tool,
            self.delve_lit,
            self.delve_boons,
        )
            .hash(&mut h);
        (
            self.knight.x.to_bits(),
            self.knight.y.to_bits(),
            self.knight.walking,
            self.camera.0.to_bits(),
            self.camera.1.to_bits(),
        )
            .hash(&mut h);
        self.visitor
            .map(|v| (v.x.to_bits(), v.y.to_bits(), v.walking, v.facing_left))
            .hash(&mut h);
        (&self.council, self.quest).hash(&mut h);
        if let Some(j) = &self.joust {
            (
                &j.red,
                &j.blue,
                j.red_score,
                j.blue_score,
                j.charge.to_bits(),
            )
                .hash(&mut h);
        }
        (self.chapel_lit, self.cottages, self.forge_hot).hash(&mut h);
        for w in &self.wards {
            (&w.name, w.banner, w.lit).hash(&mut h);
        }
        for s in &self.muster {
            (s.kind as u8, s.state as u8).hash(&mut h);
        }
        (self.region, self.weather as u8, self.fireworks).hash(&mut h);
        for &(x, y) in &self.party {
            (x.to_bits(), y.to_bits()).hash(&mut h);
        }
        (self.dragon, self.chests, self.wisps).hash(&mut h);
        for w in &self.wayfarers {
            (
                w.errand,
                w.x.to_bits(),
                w.y.to_bits(),
                w.facing_left,
                w.moving,
                w.verdict,
                w.carrying,
            )
                .hash(&mut h);
        }
        (&self.record, self.sparks, self.stargazing).hash(&mut h);
        (&self.outcomes, self.outcome_motion as u8).hash(&mut h);
        (self.school, self.school_room).hash(&mut h);
        for w in self.wishes.iter() {
            (&w.id, w.raised, &w.name, &w.near, &w.art).hash(&mut h);
        }
        if let Some(overlay) = &self.overlay {
            overlay.key(&mut h);
        }
        if let Some(g) = &self.glass {
            (g.anchor, &g.title, g.live, g.sequence).hash(&mut h);
        }
        self.march.hash(&mut h);
        (self.ambient.to_bits(), self.tick).hash(&mut h);
        h.finish()
    }

    pub fn resting() -> Scene {
        Scene {
            tier: 0,
            active: None,
            tool: None,
            knight: Knight::at_place(Place::Keep),
            visitor: None,
            camera: {
                let home = Knight::at_place(Place::Keep);
                (home.x, home.y - 8.0)
            },
            council: Vec::new(),
            quest: None,
            joust: None,
            chapel_lit: false,
            cottages: [false; 3],
            forge_hot: false,
            wards: Vec::new(),
            muster: Vec::new(),
            region: None,
            weather: Weather::Fair,
            fireworks: false,
            glass: None,
            party: Vec::new(),
            dragon: false,
            chests: (0, 0),
            wisps: false,
            wayfarers: Vec::new(),
            record: Record::default(),
            sparks: None,
            outcomes: Vec::new(),
            outcome_motion: Motion::Full,
            stargazing: false,
            overlay: None,
            school: super::school::SchoolSnapshot::default(),
            school_room: None,
            wishes: std::sync::Arc::default(),
            delve_lit: false,
            delve_boons: 0,
            march: super::castles::March::default(),
            ambient: DUSK,
            tick: 0,
        }
    }
}

const BOOKS: u32 = 10;
const BLADES: u32 = 6;
const RAVENS: u32 = 5;
const STOOKS: u32 = deeds::STOOKS.len() as u32;
const SACKS: u32 = 12;
const CANDLES: u32 = 8;

const STARS: [(i32, i32); 16] = [
    (186, 10),
    (204, 20),
    (122, 12),
    (216, 6),
    (138, 4),
    (230, 22),
    (108, 24),
    (174, 4),
    (196, 34),
    (240, 10),
    (116, 36),
    (224, 40),
    (150, 12),
    (210, 48),
    (100, 8),
    (236, 30),
];

pub struct Prop {
    pub x: i32,
    pub base: i32,
    pub img: Img,
}

pub struct Stage {
    pub props: Vec<Prop>,
    pub cues: Vec<Prop>,
    pub lights: Vec<Light>,
    pub beacons: Vec<(i32, i32, i32, i32)>,
}

fn on(tx: i32, ty: i32, tw: i32, th: i32, img: Img) -> Prop {
    Prop {
        x: tx * TILE + (tw * TILE - img.w) / 2,
        base: (ty + th) * TILE,
        img,
    }
}

fn at_place(place: Place, img: Img) -> Prop {
    let (tx, ty, tw, th) = place.footprint();
    on(tx, ty, tw, th, img)
}

pub fn stage(scene: &Scene) -> Stage {
    stage_in(scene, None)
}

pub fn stage_in(scene: &Scene, view: Option<super::View>) -> Stage {
    let mut props = Vec::new();
    let mut lights = Vec::new();
    let mut beacons: Vec<(i32, i32, i32, i32)> = Vec::new();
    let mut cues = Vec::new();
    let tick = scene.tick;
    let flicker = |k: u32| 0.92 + 0.08 * (tick as f32 * 0.5 + k as f32 * 2.3).sin();
    let fire = |x: i32, y: i32, r: f32, s: f32| Light {
        x: x as f32,
        y: y as f32,
        r,
        s,
        fire: true,
    };
    let live = |p: Place| scene.active == Some(p);
    let mut cues_authored: Vec<Prop> = Vec::new();

    'authored: {
    if crate::world::plan::tabula() {
        break 'authored;
    }

    let seated = &scene.council;
    let mut pavilion = at_place(Place::RoundTable, kit::round_table(seated));
    pavilion.base += 7;
    props.push(pavilion);
    if !seated.is_empty() {
        lights.push(fire(18 * TILE, 13 * TILE + 2, 30.0, 0.45 * flicker(5)));
    }
    props.push(at_place(Place::Keep, kit::keep(scene.tier)));
    for (i, x) in [22 * TILE + 14, 22 * TILE + 30].into_iter().enumerate() {
        props.push(Prop {
            x,
            base: 15 * TILE + 1,
            img: kit::torch(tick, i as u32),
        });
        lights.push(fire(x + 2, 15 * TILE - 12, 34.0, 0.55 * flicker(i as u32)));
    }
    props.push(at_place(Place::Rookery, kit::rookery()));
    props.push(at_place(Place::Chapel, kit::chapel(scene.chapel_lit)));
    if scene.chapel_lit {
        lights.push(fire(29 * TILE, 14 * TILE - 10, 26.0, 0.35 * flicker(7)));
    }
    let smithy_live = live(Place::Smithy);
    props.push(at_place(
        Place::Smithy,
        kit::house(&House {
            w: 32,
            h: 32,
            roof_h: 15,
            wall: Wall::Timber,
            roof: Roof::Tile,
            door_glow: smithy_live,
            windows: 1,
            lit: false,
            chimney: true,
        }),
    ));
    if smithy_live {
        props.push(Prop {
            x: 17 * TILE + 22,
            base: 17 * TILE + 2,
            img: kit::smoke(tick),
        });
        lights.push(fire(18 * TILE, 19 * TILE - 4, 48.0, 0.75 * flicker(3)));
    }
    let reading = live(Place::Scriptorium);
    props.push(at_place(
        Place::Scriptorium,
        kit::house(&House {
            w: 32,
            h: 32,
            roof_h: 15,
            wall: Wall::Stone,
            roof: Roof::Slate,
            door_glow: false,
            windows: 2,
            lit: reading,
            chimney: false,
        }),
    ));
    if reading {
        lights.push(fire(29 * TILE, 19 * TILE - 12, 30.0, 0.45 * flicker(4)));
    }
    if scene.tier >= 1 {
        props.push(on(26, 20, 2, 1, kit::garden()));
    }
    if scene.tier >= 2 {
        props.push(on(25, 17, 1, 1, kit::well()));
    }
    if scene.tier >= 3 {
        for (tx, ty) in [(19, 14), (27, 14), (21, 20), (25, 20)] {
            props.push(on(tx, ty, 1, 1, kit::lantern(true)));
            lights.push(fire(
                tx * TILE + 8,
                ty * TILE + 5,
                38.0,
                0.5 * flicker(tx as u32),
            ));
        }
    }
    if scene.tier >= 5 {
        props.push(Prop {
            x: 0,
            base: 16 * TILE,
            img: kit::docks(),
        });
    }
    if scene.tier >= 6 {
        props.push(on(42, 26, 2, 2, kit::windmill(tick)));
    }
    if scene.tier >= 7 {
        props.push(on(20, 17, 1, 1, kit::market_stall(0)));
        props.push(on(20, 18, 1, 1, kit::market_stall(1)));
    }
    if scene.tier >= 8 {
        props.push(on(21, 12, 1, 3, kit::keep_turret()));
        props.push(on(25, 12, 1, 3, kit::keep_turret()));
    }

    props.push(at_place(Place::Gatehouse, kit::gatehouse()));
    props.push(at_place(
        Place::Observatory,
        kit::observatory(live(Place::Observatory)),
    ));
    if live(Place::Observatory) {
        lights.push(Light {
            x: (10 * TILE) as f32,
            y: (3 * TILE) as f32,
            r: 36.0,
            s: 0.5,
            fire: false,
        });
    }
    props.push(at_place(Place::Mines, kit::mine_mouth()));
    props.push(at_place(Place::DragonKeep, kit::ruins()));

    let (lx, ly) = (32 * TILE + 48, 11 * TILE + 16);
    props.push(Prop {
        x: lx,
        base: ly + 80,
        img: kit::lists_ground(),
    });
    props.push(Prop {
        x: lx - 6,
        base: ly + 66,
        img: kit::tent(Heraldry::Red),
    });
    props.push(Prop {
        x: lx + 174,
        base: ly + 66,
        img: kit::tent(Heraldry::Blue),
    });
    if let Some(joust) = &scene.joust {
        for (i, (bx, by)) in [
            (lx + 12, ly + 18),
            (lx + 172, ly + 18),
            (lx + 12, ly + 80),
            (lx + 172, ly + 80),
        ]
        .into_iter()
        .enumerate()
        {
            props.push(Prop {
                x: bx,
                base: by,
                img: kit::brazier(tick, i as u32),
            });
            lights.push(fire(bx + 4, by - 10, 46.0, 0.6 * flicker(10 + i as u32)));
        }
        let run = (joust.charge.clamp(0.0, 1.0) * 105.0) as i32;
        let gallop = (tick % 2) as i32;
        props.push(Prop {
            x: lx + 18 + run,
            base: ly + 42 - gallop,
            img: kit::rider(Heraldry::Red),
        });
        props.push(Prop {
            x: lx + 140 - run,
            base: ly + 68 - (1 - gallop),
            img: kit::rider(Heraldry::Blue).flip_h(),
        });
        for k in 1..4 {
            props.push(Prop {
                x: lx + 18 + run - k * 5,
                base: ly + 41 + k % 2,
                img: kit::dust(k),
            });
            props.push(Prop {
                x: lx + 174 - run + k * 5,
                base: ly + 67 + k % 2,
                img: kit::dust(k),
            });
        }
        props.push(on(
            41,
            19,
            3,
            1,
            kit::board(&[
                (joust.red.clone(), joust.red_score, Heraldry::Red),
                (joust.blue.clone(), joust.blue_score, Heraldry::Blue),
            ]),
        ));
    }
    props.push(on(36, 19, 1, 1, kit::quintain(scene.quest.is_some(), tick)));

    if scene.dragon {
        props.push(Prop {
            x: 39 * TILE - 12,
            base: 5 * TILE + 1,
            img: kit::dragon(true, tick),
        });
        lights.push(fire(42 * TILE + 6, 3 * TILE + 12, 60.0, 0.8 * flicker(40)));
    }
    let (full, empty) = scene.chests;
    for i in 0..full.min(5) as i32 {
        props.push(on(23 - i, 5, 1, 1, kit::chest(true)));
    }
    for i in 0..empty.min(5) as i32 {
        props.push(on(25 + i, 5, 1, 1, kit::chest(false)));
    }
    if full > 0 {
        lights.push(fire(22 * TILE, 5 * TILE + 10, 30.0, 0.3));
    }
    if scene.wisps {
        let (sx, sy) = (11 * TILE + 8, 27 * TILE);
        for k in 0..3 {
            let a = tick as f32 * 0.4 + k as f32 * 2.1;
            let (wx, wy) = (
                sx + (a.cos() * 18.0) as i32,
                sy - 10 + (a.sin() * 8.0) as i32,
            );
            props.push(Prop {
                x: wx - 3,
                base: wy + 3,
                img: kit::wisp(),
            });
            lights.push(Light {
                x: wx as f32,
                y: wy as f32,
                r: 26.0,
                s: 0.5,
                fire: false,
            });
        }
    }

    for (i, (ward, (tx, ty))) in scene
        .wards
        .iter()
        .zip([(51, 13), (56, 13), (58, 19)])
        .enumerate()
    {
        let wall = if i == 1 { Wall::Plaster } else { Wall::Stone };
        props.push(on(
            tx,
            ty,
            2,
            2,
            kit::house(&House {
                w: 30,
                h: 30,
                roof_h: 14,
                wall,
                roof: Roof::Tile,
                door_glow: false,
                windows: 1,
                lit: ward.lit,
                chimney: false,
            }),
        ));
        props.push(Prop {
            x: (tx + 2) * TILE + 2,
            base: (ty + 2) * TILE,
            img: kit::flag(ward.banner, ward.banner),
        });
        let plaque = kit::realm_sign(&ward.name, true);
        props.push(Prop {
            x: tx * TILE + TILE - plaque.w / 2,
            base: (ty + 2) * TILE + 13,
            img: plaque,
        });
        if ward.lit {
            lights.push(fire(tx * TILE + 16, (ty + 2) * TILE - 14, 24.0, 0.3));
        }
    }

    for (tx, ty, wall, roof) in [
        (21, 24, Wall::Plaster, Roof::Tile),
        (21, 29, Wall::Timber, Roof::Thatch),
        (24, 29, Wall::Plaster, Roof::Thatch),
    ] {
        props.push(on(
            tx,
            ty,
            2,
            2,
            kit::house(&House {
                w: 28,
                h: 28,
                roof_h: 13,
                wall,
                roof,
                door_glow: false,
                windows: 1,
                lit: false,
                chimney: ty == 24,
            }),
        ));
    }
    props.push(on(30, 29, 1, 1, kit::well()));
    for (i, (tx, ty)) in [(18, 24), (25, 24), (28, 24)].into_iter().enumerate() {
        let lit = scene.cottages[i];
        props.push(on(
            tx,
            ty,
            2,
            2,
            kit::house(&House {
                w: 30,
                h: 30,
                roof_h: 14,
                wall: Wall::Plaster,
                roof: Roof::Thatch,
                door_glow: false,
                windows: 2,
                lit,
                chimney: true,
            }),
        ));
        if lit {
            lights.push(fire(
                tx * TILE + 16,
                (ty + 2) * TILE - 14,
                26.0,
                0.32 * flicker(20 + i as u32),
            ));
        }
    }
    props.push(on(
        18,
        29,
        2,
        2,
        kit::house(&House {
            w: 32,
            h: 30,
            roof_h: 14,
            wall: Wall::Timber,
            roof: Roof::Tile,
            door_glow: scene.forge_hot,
            windows: 0,
            lit: false,
            chimney: true,
        }),
    ));
    if scene.forge_hot {
        props.push(Prop {
            x: 18 * TILE + 22,
            base: 29 * TILE + 2,
            img: kit::smoke(tick + 3),
        });
        lights.push(fire(19 * TILE, 31 * TILE - 4, 44.0, 0.7 * flicker(30)));
    }
    props.push(on(
        27,
        29,
        2,
        2,
        kit::house(&House {
            w: 32,
            h: 30,
            roof_h: 16,
            wall: Wall::Timber,
            roof: Roof::Thatch,
            door_glow: false,
            windows: 0,
            lit: false,
            chimney: false,
        }),
    ));

    for (i, (tx, ty)) in [(17, 34), (20, 34), (25, 34), (28, 34)]
        .into_iter()
        .enumerate()
    {
        props.push(on(
            tx,
            ty,
            2,
            2,
            kit::cottage(i as u32, scene.cottages[i % 3]),
        ));
    }
    for (tx, trade, width) in [
        (17, kit::Trade::Smith, 3),
        (21, kit::Trade::Weaver, 2),
        (24, kit::Trade::Cooper, 2),
        (27, kit::Trade::Granary, 3),
    ] {
        props.push(on(tx, 40, width, 2, kit::workshop(trade, scene.forge_hot)));
    }
    for (i, tx) in [18, 20, 26, 28].into_iter().enumerate() {
        props.push(on(tx, 37, 1, 1, kit::market_stall(i as u32)));
    }
    props.push(on(30, 36, 1, 1, kit::well()));
    props.push(on(18, 39, 1, 1, kit::barrels()));
    props.push(on(29, 39, 1, 1, kit::handcart()));
    for tx in [17, 25] {
        props.push(on(tx, 43, 5, 0, kit::garden_fence(65)));
    }
    for (tx, ty, carrying) in [
        (19, 36, false),
        (27, 36, true),
        (22, 39, false),
        (26, 39, true),
    ] {
        props.push(on(tx, ty, 1, 1, kit::villager(0, carrying)));
    }
    if scene.forge_hot {
        props.push(Prop {
            x: 19 * TILE + 4,
            base: 40 * TILE - 1,
            img: kit::smoke(tick + 3),
        });
        lights.push(fire(18 * TILE + 10, 42 * TILE - 8, 38.0, 0.5 * flicker(60)));
    }
    for (i, tx) in [17, 20, 25, 28].into_iter().enumerate() {
        if scene.cottages[i % 3] {
            lights.push(fire(tx * TILE + 16, 36 * TILE - 12, 23.0, 0.3));
        }
    }

    let mut arena = kit::colosseum();
    for (i, soldier) in scene.muster.iter().take(8).enumerate() {
        arena.stamp(
            &kit::soldier(*soldier),
            49 + (i % 4) as i32 * 17,
            30 + (i / 4) as i32 * 13,
        );
    }
    props.push(at_place(Place::Colosseum, arena));
    let (tx, ty) = (50 * TILE, 34 * TILE);
    let mut tilt = kit::lists_ground();
    if let Some(joust) = &scene.joust {
        let advance = (joust.charge.clamp(0.0, 1.0) * 95.0) as i32;
        tilt.stamp(&kit::rider(Heraldry::Red), 23 + advance, 18);
        tilt.stamp(&kit::rider(Heraldry::Blue).flip_h(), 148 - advance, 43);
    }
    props.push(Prop {
        x: tx,
        base: ty + 85,
        img: tilt,
    });
    props.push(on(50, 34, 2, 2, kit::tent(Heraldry::Red)));
    props.push(on(60, 34, 2, 2, kit::tent(Heraldry::Blue)));
    props.push(on(60, 40, 2, 1, kit::garden_fence(31)));
    props.push(on(58, 40, 1, 1, kit::quintain(scene.quest.is_some(), tick)));
    for (place, label) in [
        (Place::ArtisanQuarter, "ARTISANS"),
        (Place::Colosseum, "COLOSSEUM"),
        (Place::Tournament, "TOURNAMENT"),
    ] {
        let (tx, ty) = place.stand();
        let plaque = kit::realm_sign(label, false);
        props.push(Prop {
            x: tx * TILE + 8 - plaque.w / 2,
            base: if place == Place::ArtisanQuarter {
                ty * TILE + 14
            } else {
                (ty + 1) * TILE + 12
            },
            img: plaque,
        });
    }

    let ranks = scene.muster.len().min(18);
    for (i, soldier) in scene.muster.iter().take(ranks).enumerate() {
        let (row, col) = ((i / 6) as i32, (i % 6) as i32);
        let in_row = (ranks as i32 - row * 6).min(6);
        let x = 23 * TILE + 8 - in_row * 4 + col * 8;
        let base = 17 * TILE + 2 + row * 12;
        props.push(Prop {
            x: x - 3,
            base,
            img: kit::soldier(*soldier),
        });
    }

    if scene.fireworks {
        lights.push(Light {
            x: (23 * TILE + 8) as f32,
            y: (12 * TILE) as f32,
            r: 90.0,
            s: 0.45,
            fire: false,
        });
    }

    let record = &scene.record;
    let (lx, ly) = (32 * TILE + 48, 11 * TILE + 16);
    for (i, &passed) in record.pennants.iter().enumerate() {
        props.push(Prop {
            x: lx + 26 + i as i32 * 8,
            base: ly + 81,
            img: kit::pennant(passed),
        });
    }
    for (i, &(tx, ty)) in deeds::STOOKS
        .iter()
        .take(record.stooks.min(STOOKS) as usize)
        .enumerate()
    {
        props.push(Prop {
            x: tx * TILE + 5 + (i as i32 % 2) * 2,
            base: (ty + 1) * TILE - 3,
            img: kit::stook(),
        });
    }
    if record.sacks > 0 {
        props.push(Prop {
            x: 29 * TILE + 2,
            base: 31 * TILE - 1,
            img: kit::sacks(record.sacks.min(SACKS)),
        });
    }
    if record.candles > 0 {
        props.push(Prop {
            x: 28 * TILE + 3,
            base: 14 * TILE + 3,
            img: kit::candles(record.candles.min(CANDLES), tick),
        });
    }
    for (i, &(x, y)) in STARS
        .iter()
        .take(record.stars.min(STARS.len() as u32) as usize)
        .enumerate()
    {
        let twinkle = !hash(i as i32, (tick / 3) as i32, 83).is_multiple_of(3);
        cues_authored.push(Prop {
            x: x - 1,
            base: y + 2,
            img: kit::star(twinkle),
        });
    }
    if scene.stargazing {
        let (dx, dy) = (10 * TILE, 2 * TILE + 4);
        let sweep = (tick as f32 * 0.05).sin();
        let img = kit::beam(std::f32::consts::FRAC_PI_2 + sweep * 1.1);
        cues_authored.push(Prop {
            x: dx - img.w / 2,
            base: dy + 1,
            img,
        });
        lights.push(Light {
            x: dx as f32,
            y: dy as f32,
            r: 40.0,
            s: 0.55,
            fire: false,
        });
    }
    if record.ravens > 0 {
        let (x, base) = deeds::ROOST;
        props.push(Prop {
            x,
            base,
            img: kit::roost(record.ravens.min(RAVENS)),
        });
    }
    if record.books > 0 {
        props.push(Prop {
            x: 30 * TILE + 2,
            base: 19 * TILE - 1,
            img: kit::books(record.books.min(BOOKS)),
        });
    }
    if record.blades > 0 {
        let img = kit::blade_rack(record.blades.min(BLADES));
        props.push(Prop {
            x: 17 * TILE - 2 - img.w,
            base: 19 * TILE - 1,
            img,
        });
    }
    if let Some(age) = scene.sparks {
        props.push(Prop {
            x: 18 * TILE - 7,
            base: 19 * TILE + 3,
            img: kit::sparks(age),
        });
        let fade = 1.0 - age as f32 / deeds::SPARKS as f32;
        lights.push(fire(18 * TILE, 19 * TILE - 6, 30.0, 0.55 * fade));
    }

    props.extend(super::clerks::stage(tick, scene.outcome_motion));
    super::school::stage(
        scene.school,
        tick,
        scene.outcome_motion,
        &mut props,
        &mut lights,
    );
    }
    for p in props.iter_mut().chain(cues_authored.iter_mut()) {
        let (ax, ay) = (p.x + p.img.w / 2, p.base - 1);
        let (rx, ry) = place_px(ax, ay);
        p.x += rx - ax;
        p.base += ry - ay;
    }
    cues.extend(cues_authored);
    for l in &mut lights {
        let (rx, ry) = place_px(l.x as i32, l.y as i32);
        l.x += (rx - l.x as i32) as f32;
        l.y += (ry - l.y as i32) as f32;
    }
    for b in &mut beacons {
        let (ax, ay) = (b.0 + b.2 / 2, b.1 + b.3 - 1);
        let (rx, ry) = place_px(ax, ay);
        b.0 += rx - ax;
        b.1 += ry - ay;
    }

    if crate::world::plan::full() {
        super::college::stage(tick, &mut props, &mut lights);
    }
    super::wishes::stage_gate(
        scene.delve_lit,
        scene.delve_boons,
        &mut props,
        &mut lights,
        tick,
    );
    super::wishes::stage(&scene.wishes, &mut props, &mut lights, tick);

    super::castles::stage(&scene.march, &mut props, &mut lights, tick, view);

    super::signs::stage(&mut props);

    for echo in &scene.outcomes {
        let (tx, ty) = echo.place.stand_world();
        let (x, base) = (tx * TILE + TILE / 2 + 14, (ty + 1) * TILE - 2);
        let img = outcomes::lantern(*echo, scene.outcome_motion);
        cues.push(Prop {
            x: x - img.w / 2,
            base,
            img,
        });
        if matches!(echo.seal, Seal::Completed | Seal::Verified) {
            lights.push(Light {
                x: x as f32,
                y: (base - 15) as f32,
                r: 18.0,
                s: 0.22,
                fire: echo.seal == Seal::Completed,
            });
        }
    }

    if let Some(overlay) = &scene.overlay {
        overlay.stage(&mut props, &mut cues);
    }

    const ROBES: [char; 4] = ['1', '2', '@', '3'];
    for (i, &(x, y)) in scene.party.iter().take(4).enumerate() {
        let img = kit::squire(ROBES[i]);
        props.push(Prop {
            x: x as i32 - img.w / 2,
            base: y as i32 + 2,
            img,
        });
    }

    let frame = tick / 2;
    for w in &scene.wayfarers {
        let img = match w.errand {
            Errand::Wagon => kit::wagon(w.moving, frame, w.verdict),
            Errand::Courier => kit::courier(w.moving, frame, w.verdict),
            Errand::Raven => kit::raven(frame, w.carrying),
            Errand::Hand => kit::field_hand(if w.moving { frame } else { tick / 5 }),
            Errand::Messenger => kit::messenger(frame),
            Errand::Villager => kit::villager(frame, w.carrying),
            Errand::Monk => kit::monk(frame, tick),
            Errand::Owl => kit::owl(frame),
        };
        let img = if w.facing_left { img.flip_h() } else { img };
        let prop = Prop {
            x: w.x as i32 - img.w / 2,
            base: w.y as i32 + 2,
            img,
        };
        let flying = matches!(w.errand, Errand::Raven | Errand::Owl);
        if flying {
            cues.push(prop);
        } else {
            props.push(prop);
        }
        if !flying {
            lights.push(Light {
                x: w.x,
                y: w.y - 6.0,
                r: 30.0,
                s: 0.36,
                fire: false,
            });
        }
        if w.errand == Errand::Monk {
            lights.push(Light {
                x: w.x + if w.facing_left { -3.0 } else { 3.0 },
                y: w.y - 7.0,
                r: 20.0,
                s: 0.5 * flicker(70),
                fire: true,
            });
        }
        if (w.errand, w.verdict) == (Errand::Wagon, None) {
            let ahead = if w.facing_left { -3.0 } else { 3.0 };
            lights.push(Light {
                x: w.x + ahead,
                y: w.y - 6.0,
                r: 26.0,
                s: 0.55 * flicker(60),
                fire: true,
            });
        }
    }

    let k = scene.knight;
    let bob = if k.walking && ((k.x + k.y) as i32 / 4) % 2 == 1 {
        1
    } else {
        0
    };
    let reading = scene.tool == Some(Tool::Book) && !k.walking;
    let kimg = super::castles::dress(
        &if reading {
            kit::reader()
        } else {
            kit::knight()
        },
        scene.march.lead,
    );
    let (kx, kbase) = (k.x as i32 - 8, k.y as i32 + 2 - bob);
    props.push(Prop {
        x: kx,
        base: kbase,
        img: kimg,
    });
    lights.push(Light {
        x: k.x,
        y: k.y - 8.0,
        r: 40.0,
        s: 0.42,
        fire: false,
    });
    if reading {
        lights.push(Light {
            x: k.x + 11.0,
            y: k.y - 4.0,
            r: 38.0,
            s: 0.62 * flicker(50),
            fire: true,
        });
    }
    if let (Some(tool), false, false) = (scene.tool, k.walking, reading) {
        cues.push(Prop {
            x: kx - 1,
            base: kbase - 15 - ((tick / 2) % 2) as i32,
            img: kit::bubble(tool),
        });
    }
    if let Some(v) = scene.visitor {
        let bob = i32::from(v.walking && ((v.x + v.y) as i32 / 4) % 2 == 1);
        let figure = if v.facing_left {
            kit::visitor().flip_h()
        } else {
            kit::visitor()
        };
        let mut img = Img::new(figure.w, figure.h + bob);
        img.stamp(&figure, 0, 0);
        props.push(Prop {
            x: v.x as i32 - img.w / 2,
            base: v.y as i32 + 2,
            img,
        });
        lights.push(Light {
            x: v.x,
            y: v.y - 8.0,
            r: 40.0,
            s: 0.42,
            fire: false,
        });
    }
    if let Some(place) = scene.active {
        let (tx, ty, tw, th) = place.footprint_world();
        let top = match place {
            Place::Keep => 16,
            Place::Chapel => 12,
            Place::Smithy => 6,
            Place::Observatory | Place::Rookery | Place::Gatehouse => 8,
            _ => 0,
        };
        beacons.push((tx * TILE, ty * TILE - top, tw * TILE, th * TILE + top));
    }

    Stage {
        props,
        cues,
        lights,
        beacons,
    }
}
