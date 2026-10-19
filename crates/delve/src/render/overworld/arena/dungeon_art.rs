//! Delivered dungeon scenery, shared by both native grains. All conversion,
//! grading and resampling is offline; this path only stamps registered pixels.
use super::super::ink::Rgb;
use super::*;
use std::sync::{Arc, Mutex, OnceLock};
mod lighting;

// The Swamp of Horrors has no delivered room kit: its slots are empty and
// it keeps its own painted mire (`swamp_art`).
const PACKS: [&str; 7] = ["crypt", "mines", "keep", "archive", "fungal", "unknown", "swamp"];
const FLOOR: [&str; 12] = [
    "plain", "worn", "cracked", "patched", "stained", "rich", "trim-0", "trim-1", "crack",
    "puddle", "debris", "detail",
];
const WALL: [&str; 12] = [
    "face-0",
    "face-1",
    "face-2",
    "face-3",
    "outer-nw",
    "outer-ne",
    "side-left",
    "side-right",
    "inner-nw",
    "inner-ne",
    "cap",
    "collapsed",
];
const BLOCK: [&str; 12] = [
    "pillar-0",
    "pillar-1",
    "pillar-2",
    "tomb-0",
    "tomb-1",
    "tomb-2",
    "boulder-0",
    "boulder-1",
    "boulder-2",
    "broken-0",
    "broken-1",
    "broken-2",
];
const HAZARD: [&str; 12] = [
    "fill-0", "fill-1", "fill-2", "fill-3", "edge-n", "edge-s", "edge-w", "edge-e", "outer-nw",
    "outer-se", "inner-nw", "inner-se",
];
const GATE: [&str; 12] = [
    "barred",
    "open",
    "opening",
    "side-left-barred",
    "side-left-open",
    "side-right-barred",
    "side-right-open",
    "stairs-down",
    "stairs-up",
    "boss",
    "alcove",
    "seal",
];
const DECOR: [&str; 12] = [
    "flame-0", "flame-1", "flame-2", "lamp", "barrel", "bones", "web", "prop-0", "prop-1",
    "decal-0", "decal-1", "glow",
];
const OWALL: [&str; 12] = [
    "face-0",
    "face-1",
    "face-2",
    "corner-nw",
    "corner-ne",
    "west-0",
    "west-1",
    "east-0",
    "east-1",
    "south",
    "corner-sw",
    "corner-se",
];
const ODOOR: [&str; 12] = [
    "barred",
    "open",
    "opening",
    "west-barred",
    "west-open",
    "east-barred",
    "east-open",
    "south-barred",
    "south-open",
    "stairs-down",
    "boss",
    "seal",
];
struct Bank {
    floor: Vec<Option<Img>>,
    slabs: Vec<[Option<Img>;4]>,
    wall: Vec<Option<Img>>,
    block: Vec<Option<Img>>,
    hazard: Vec<Option<Img>>,
    gate: Vec<Option<Img>>,
    decor: Vec<Option<Img>>,
    owall: Vec<Option<Img>>,
    odoor: Vec<Option<Img>>,
    sidedoor: Vec<Option<Img>>,
}
fn bank(pack: Pack, scale: i32) -> &'static Bank {
    static BANKS: [OnceLock<Bank>; 14] = [const { OnceLock::new() }; 14];
    BANKS[pack as usize * 2 + (scale - 1) as usize].get_or_init(|| {
        let load = |kind: &str, names: &[&str]| {
            names
                .iter()
                .map(|name| {
                    let slot = format!("dgn.{kind}.{}", PACKS[pack as usize]);
                    let name = if scale == 2 {
                        format!("v2-{name}")
                    } else {
                        name.to_string()
                    };
                    slot_art::bytes(&slot, &name).map(|_| {
                        let mut im=slot_art::load(&slot,&name,&[]);
                        if kind=="decor" || kind=="block" {
                            for y in 0..im.h {for x in 0..im.w {
                                if im.get(x,y).is_some_and(|c| matches!(c,[80,198,179]|[143,232,220]|[228,255,248])) {
                                    im.set(x,y,[47,74,82]);
                                }
                            }}
                        }im
                    })
                })
                .collect()
        };
        let floor:Vec<Option<Img>>=load("floor",&FLOOR);
        // Cache the four material quadrants once. Room admission then uses
        // packed row stamps rather than sampling every floor pixel again.
        let slabs=floor.iter().map(|im|std::array::from_fn(|q|im.as_ref().map(|source| {
            let t=16*scale;let mut out=Img::new(t,t);
            for y in 0..t {for x in 0..t {
                if let Some(c)=source.get((q as i32%2*t+x)/2,(q as i32/2*t+y)/2) {out.set(x,y,c);}
            }}out
        }))).collect();
        Bank {
            floor,
            slabs,
            wall: load("wall", &WALL),
            block: load("block", &BLOCK),
            hazard: load("hazard", &HAZARD),
            gate: load("gate", &GATE),
            decor: load("decor", &DECOR),
            owall: load("owall", &OWALL),
            odoor: load("odoor", &ODOOR),
            sidedoor: ["west-closed", "west-open", "west-locked", "east-closed", "east-open", "east-locked"].iter().map(|name| {
                let slot = format!("dgn.sidedoor-{}", PACKS[pack as usize]);
                let name = if scale == 2 {format!("v2-{name}")} else {name.to_string()};
                slot_art::bytes(&slot, &name).map(|_|slot_art::load(&slot, &name, &[]))
            }).collect(),
        }
    })
}
/// Decode and prepare each delivered bank before the first displayed room.
pub(super) fn warm() {
    let scale = if art_v2::enabled() { 2 } else { 1 };
    for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown] {
        let _ = bank(pack, scale);
    }
}

pub(super) fn supported(run: &Run) -> bool {
    !run.race3d_showing()
        && slot_art::enabled()
        && !run.at_home_now()
        && !run.side_on()
        && run.circle() == 0
        && run.mode != crate::rules::fortune::Mode::LightsOut
        && run.room().kind != RoomKind::Wanted
        && bank(run.dungeon.pack, 1).floor[..6]
            .iter()
            .all(Option::is_some)
}
#[derive(Clone, Copy)]
pub(super) struct Object {
    pub x: i32,
    pub feet: i32,
    pub index: usize,
}
pub(super) struct Scene {
    pub base: Img,
    pub objects: Vec<Object>,
    pub fires: Vec<(i32, i32)>,
    shimmer: Vec<(i32, i32, [Rgb; 4])>,
}
#[derive(PartialEq, Eq, Clone)]
struct Key {
    layout: u64,
    room: usize,
    seed: u64,
    pack: Pack,
    barred: bool,
    side_state: usize,
}
pub(super) fn scene(run: &Run, scale: i32) -> Arc<Scene> {
    type Cache = Mutex<Option<(Key, Arc<Scene>)>>;
    static CACHE: [OnceLock<Cache>; 2] = [const { OnceLock::new() }; 2];
    let cache = CACHE[(scale - 1) as usize].get_or_init(Default::default);
    let key = Key {
        layout: run.room().fingerprint(),
        room: run.at,
        seed: run.raid_id,
        pack: run.dungeon.pack,
        barred: run.barred(),
        side_state: side_state(run),
    };
    if let Some((k, scene)) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *k == key {
            return scene.clone();
        }
    }
    let scene = Arc::new(bake(run, scale));
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, scene.clone()));
    scene
}
// Existing combat bars and an empty-room wave lock have the same collision.
// Their delivered drawings distinguish the live foe closure from the wave lock.
pub(super) fn side_state(run: &Run) -> usize {
    if !run.barred() {1} else if run.foes_left() {0} else {2}
}
fn stamp(cv: &mut Img, bank: &[Option<Img>], index: usize, x: i32, y: i32) {
    if let Some(im) = &bank[index] {
        cv.stamp(im, x, y);
    }
}
fn wall(cv: &mut Img, b: &Bank, room: &Room, c: i32, r: i32, s: i32, v: usize) {
    let t = 16 * s;
    let (x, y) = (c * t, r * t);
    if b.owall[0].is_some() {
        let i = if r == 0 {
            if c == 0 {
                3
            } else if c == room.cols as i32 - 1 {
                4
            } else {
                if c % 5 == 3 { 2 } else { v % 2 }
            }
        } else if r == room.rows as i32 - 1 {
            if c == 0 {
                10
            } else if c == room.cols as i32 - 1 {
                11
            } else {
                9
            }
        } else if c == 0 {
            5 + v % 2
        } else if c == room.cols as i32 - 1 {
            7 + v % 2
        } else {
            v % 3
        };
        if let Some(im) = &b.owall[i] {
            cv.stamp(im, x, if r == 0 || (c > 0 && c < room.cols as i32 - 1 && r < room.rows as i32 - 1) { y } else { y + t - im.h });
            return;
        }
    }
    // The old angled corners/side runs are deliberately not transformed.
    if r == 0 || (c > 0 && c < room.cols as i32 - 1 && r < room.rows as i32 - 1) {
        if let Some(im) = &b.wall[v % 4] {
            cv.stamp(im, x, y + t - im.h);
        }
    } else if r == room.rows as i32 - 1 && b.wall[6].is_some() {
        if let Some(im) = &b.wall[6 + v % 2] {
            cv.stamp(im, x, y + t - im.h);
        }
    } else if let Some(im) = &b.wall[10] {
        // End-pillars give a truthful upright boundary until flat caps arrive.
        cv.stamp(im, x, y + t - im.h);
    }
}
fn hazard(cv: &mut Img, b: &Bank, room: &Room, c: i32, r: i32, s: i32, v: usize) {
    let t = 16 * s;
    let (x, y) = (c * t, r * t);
    if b.hazard[0].is_none() {
        return;
    }
    stamp(cv, &b.hazard, v % 4, x, y);
    let dry = |dc, dr| room.tile(c + dc, r + dr) != Tile::Hazard;
    // Rim samples are clipped to the danger tile: they never claim a dry tile
    // is blocked. Quarter tiles compose every combination without rotating art.
    for (edge, (dx, dy)) in [(0, -1), (0, 1), (-1, 0), (1, 0)].into_iter().enumerate() {
        if !dry(dx, dy) {
            continue;
        }
        if let Some(im) = &b.hazard[4 + edge] {
            for yy in 0..t {
                for xx in 0..t {
                    let rim = match edge {
                        0 => yy < t / 4,
                        1 => yy >= t * 3 / 4,
                        2 => xx < t / 4,
                        _ => xx >= t * 3 / 4,
                    };
                    if rim {
                        if let Some(c) = im.get(xx, yy) {
                            cv.set(x + xx, y + yy, c);
                        }
                    }
                }
            }
        }
    }
    for (west, north) in [(true, true), (false, true), (true, false), (false, false)] {
        let dx = if west { -1 } else { 1 };
        let dy = if north { -1 } else { 1 };
        let outer = dry(dx, 0) && dry(0, dy);
        let inner = !dry(dx, 0) && !dry(0, dy) && dry(dx, dy);
        if !outer && !inner {
            continue;
        }
        // The delivered top-left outer and top-right inner quadrants cover
        // their counterparts by reflection, as requested by the art judge.
        let index = if outer { 8 } else { 10 };
        if let Some(im) = &b.hazard[index] {
            let q = t / 3;
            for yy in 0..q {
                for xx in 0..q {
                    let sx = if outer { xx } else { t - 1 - xx };
                    if let Some(c) = im.get(sx, yy) {
                        cv.set(
                            x + if west { xx } else { t - 1 - xx },
                            y + if north { yy } else { t - 1 - yy },
                            c,
                        );
                    }
                }
            }
        }
    }
}
fn objects_for_room(run:&Run,s:i32)->Vec<Object> {
    let room=run.room();let t=16*s;
    let seed=(run.raid_id as u32).wrapping_add(run.at as u32*31);
    let mut objects=vec![];
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            if room.tile(c, r) == Tile::Block
                && room.kind != RoomKind::Forge
                && !room.kind.nook()
                && !(room.kind == RoomKind::Threshold && r <= 3)
            {
                let v = hash(c, r, seed);
                let (x, y) = (c * t, r * t);
                // Templates own collision; type selection never adds a prop to
                // an unblocked floor cell. Three families, all one tile wide.
                let family = match run.dungeon.pack {
                    Pack::Crypt | Pack::Archive => 3,
                    Pack::Cavern => 6,
                    _ => 0,
                };
                let index = family + v as usize % 3;
                let object = Object {
                    x,
                    feet: y + t,
                    index,
                };
                objects.push(object);
            }
        }
    }
    objects
}
fn bake(run: &Run, s: i32) -> Scene {
    let room = run.room();
    let b = bank(run.dungeon.pack, s);
    let t = 16 * s;
    let mut cv = Img::black(room.cols as i32 * t, room.rows as i32 * t);
    let objects;
    // An open floor's edge is rock, left dark: its fires are its own
    // (`open::fixtures`, drawn over this), not the kit's sconces round it.
    let mut fires = if run.open_here().is_some() { Vec::new() } else { lighting::fixtures(room, t) };
    let seed = (run.raid_id as u32).wrapping_add(run.at as u32 * 31);
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            let v = hash(c / 2, r / 2, seed);
            let (x, y) = (c * t, r * t);
            match room.tile(c, r) {
                Tile::Floor | Tile::Bog | Tile::Door | Tile::Stairs | Tile::Ledge | Tile::Block => {
                    let variant = if run.dungeon.pack == Pack::Fungal {
                        // Quiet broad earth in the fighting lanes; root-rich
                        // deliveries stay rare and against the perimeter.
                        if v % 29 == 0 { 1 } else { 0 }
                    } else if run.dungeon.pack == Pack::Hellforge {
                        if v % 23 == 0 {
                            2
                        } else {
                            [0, 1, 3][v as usize % 3]
                        }
                    } else if v % 16 == 0 {
                        5
                    } else if v % 11 == 0 {
                        3
                    } else {
                        [0, 1, 2, 4][v as usize % 4]
                    };
                    let variant = if run.dungeon.pack != Pack::Fungal && room.tile(c, r - 1) == Tile::Wall {
                        6 + (v as usize % 2)
                    } else {
                        variant
                    };
                    // A source slab spans two collision cells. Registration and
                    // collision stay on the original grid; only material scale changes.
                    if let Some(im) = &b.slabs[variant][(r%2*2+c%2) as usize] {
                        cv.stamp(im,x,y);
                    }

                }
                Tile::Hazard => hazard(&mut cv, b, room, c, r, s, v as usize),
                Tile::Wall => {},
            }
        }
    }
    let (mut shimmer, reveal) = lighting::floor(&mut cv, run, s, &fires);
    // What stands on the floor is drawn alone, put in the room's light,
    // then laid over it.
    let floor = std::mem::replace(&mut cv, Img::new(room.cols as i32 * t, room.rows as i32 * t));
    // Faces overlap the first floor row; drawing them after the floor is
    // essential, otherwise the following tile erases their lower half.
    for r in 0..room.rows as i32 { for c in 0..room.cols as i32 {
        // Rock with no floor round it is the dark the chambers stand in.
        if room.tile(c, r) == Tile::Wall && !super::open::deep_rock(room, c, r) {
            wall(&mut cv, b, room, c, r, s, hash(c,r,seed) as usize);
        }
    }}
    objects=objects_for_room(run,s);
    for &object in &objects {stamp_object(&mut cv,run,s,object);}
    reveal.grade(&mut cv);
    let standing = std::mem::replace(&mut cv, floor);
    cv.stamp(&standing, 0, 0);
    for (side, &(c, r)) in super::doors(room).iter().enumerate() {
        if !room.doors[side] {
            continue;
        }
        let index = match side {
            0 => usize::from(!run.barred()),
            1 => 5 + usize::from(!run.barred()),
            2 => 7 + usize::from(!run.barred()),
            _ => 3 + usize::from(!run.barred()),
        };
        let (x, y) = (c * t, r * t);
        let side_art = match side {1 => b.sidedoor[3 + side_state(run)].as_ref(), 3 => b.sidedoor[side_state(run)].as_ref(), _ => None};
        if let Some(im) = side_art {
            cv.stamp(im, x, y);
        } else if let Some(im) = &b.odoor[index] {
            cv.stamp(im, x, y);
        } else if side % 2 == 0 {
            stamp(&mut cv, &b.gate, usize::from(!run.barred()), x, y - 8 * s);
        } else if !run.barred() {
            cv.rect(x+2*s,y,t-4*s,2*t,'s');
        } else if run.barred() {
            let mut im = super::sprites::portcullis(false);
            let tint=lighting::colour(run.dungeon.pack);
            for yy in 0..im.h {for xx in 0..im.w {
                if let Some(c)=im.get(xx,yy) {
                    let value=(54*c[0] as u32+183*c[1] as u32+19*c[2] as u32)/256;
                    im.set(xx,yy,tint.map(|v|(v as u32*value/360).min(90) as u8));
                }
            }}
            im.rect(im.w/2-1,im.h/2-1,3,3,'7');
            im.put(im.w/2,im.h/2,'6');
            if s == 2 {
                cv.stamp_twice(&im, x, y);
            } else {
                cv.stamp(&im, x, y);
            }
        }
    }
    for r in 0..room.rows as i32 {
        for c in 0..room.cols as i32 {
            if room.tile(c, r) == Tile::Stairs
                && room.tile(c - 1, r) != Tile::Stairs
                && room.tile(c, r - 1) != Tile::Stairs
            {
                if b.odoor[9].is_some() {
                    stamp(&mut cv, &b.odoor, 9, c * t, r * t);
                } else if b.gate[8].is_some() {
                    stamp(&mut cv, &b.gate, 8, c * t, r * t);
                } else {
                    let im = super::sprites::stairs(!run.barred());
                    if s == 2 {
                        cv.stamp_twice(&im, c * t, r * t);
                    } else {
                        cv.stamp(&im, c * t, r * t);
                    }
                }
            }
        }
    }
    // Cluster paired delivered props at wall bases, clear of door mouths.
    // Walkable lanes never gain a solid-looking decoration.
    for (i, &(x,y)) in fires.iter().enumerate() {
        let c = x / t; let r = y / t;
        let neighbour = if c == 0 { (c, r+1) } else if c == room.cols as i32-1 { (c,r-1) } else { (c+1,r) };
        let spacing = if run.dungeon.pack == Pack::Fungal { 6 } else { 2 };
        if i % spacing == 0 && room.tile(neighbour.0,neighbour.1) == Tile::Wall {
            let index = [4,6,7,8][hash(c,r,seed+19) as usize % 4];
            if let Some(im) = &b.decor[index] {
                cv.stamp(im,neighbour.0*t,neighbour.1*t+t-im.h);
            }
        }
    }

    shimmer.retain(|&(x,y,c)| cv.get(x,y)==Some(c[0]));
    Scene {
        base: cv,
        objects,
        fires: std::mem::take(&mut fires),
        shimmer,
    }
}
pub(super) fn stamp_object(cv: &mut Img, run: &Run, s: i32, o: Object) {
    if let Some(im) = &bank(run.dungeon.pack, s).block[o.index] {
        cv.stamp(im, o.x, o.feet - im.h);
    }
}
/// Special knight states still use the legacy body painter. Preserve the
/// earlier overlay under opaque foreground props, leaving the native v2
/// obstacle visible there. Cues and the final HUD are painted afterward.
pub(super) fn legacy_body(cv: &mut Img, run: &Run, feet: f32, paint: impl FnOnce(&mut Img)) {
    let objects=objects_for_room(run,1);
    let b = bank(run.dungeon.pack, 1);
    let feet = (feet * UNIT).round() as i32;
    let mut covered = Vec::new();
    for object in &objects {
        if object.feet <= feet {
            continue;
        }
        if let Some(im) = &b.block[object.index] {
            for y in 0..im.h {
                for x in 0..im.w {
                    if im.get(x, y).is_some() {
                        let (x, y) = (object.x + x, object.feet - im.h + y);
                        covered.push((x, y, cv.get(x, y)));
                    }
                }
            }
        }
    }
    paint(cv);
    for (x, y, previous) in covered {
        if let Some(c) = previous {
            cv.set(x, y, c);
        } else {
            cv.clear(x, y);
        }
    }
}
pub(super) fn flames(cv: &mut Img, run: &Run, s: i32, scene: &Scene) {
    let b = bank(run.dungeon.pack, s);
    for &(x,y,colours) in &scene.shimmer { cv.set(x,y,colours[(run.tick / 4 % 4) as usize]); }
    for (i, &(x, y)) in scene.fires.iter().enumerate() {
        if let Some(im) = &b.decor[((run.tick / 6 + i as u64) % 3) as usize] {
            cv.stamp(im, x - im.w / 2, y + 8 * s - im.h);
        }
    }
}
pub(super) fn scenery(run: &Run) -> Img {
    let scene = scene(run, 1);
    let mut im = scene.base.clone();
    flames(&mut im, run, 1, &scene);
    let tint=lighting::colour(run.dungeon.pack);
    for i in 0..14 {
        let v=hash(i,run.at as i32,71);
        let x=((v as u64+run.tick/3)%im.w as u64) as i32;
        let y=(((v>>10) as u64+run.tick/5)%im.h as u64) as i32;
        if let Some(c)=im.get(x,y) {im.set(x,y,std::array::from_fn(|k|((c[k] as u16*89+tint[k] as u16*11)/100) as u8));}
    }
    // Special furniture, traps, chests and sealed walls keep their existing
    // exact geometry and state. Dungeon doors/stairs above own only their art.
    super::room_details(run, &mut im, &mut vec![]);
    im
}
pub(super) fn v2_ground(cv: &mut super::art_v2::paint::Canvas, run: &Run) {
    let scene=scene(run,2);
    let (ox,oy) = (cv.origin.0 as i32*2,cv.origin.1 as i32*2);
    for &(x,y,colours) in &scene.shimmer {
        let (x,y)=(x-ox,y-oy);
        if x>=0 && y>=0 && (x as usize)<cv.w && (y as usize)<cv.h {cv.px[y as usize*cv.w+x as usize]=colours[(run.tick/4%4) as usize];}
    }
}
pub(super) fn v2_flames(cv: &mut super::art_v2::paint::Canvas, run: &Run) {
    let scene = scene(run, 2);
    let b = bank(run.dungeon.pack, 2);
    for (i, &(x, y)) in scene.fires.iter().enumerate() {
        if let Some(im) = &b.decor[((run.tick / 6 + i as u64) % 3) as usize] {
            stamp_canvas(cv, im, x - im.w / 2, y + 16 - im.h);
        }
    }
    let pixels = details(run);
    let (ox, oy) = (cv.origin.0 as i32 * 2, cv.origin.1 as i32 * 2);
    for &(x, y, c) in pixels.iter() {
        let (x, y) = (x - ox, y - oy);
        if x >= 0 && y >= 0 && (x as usize) < cv.w && (y as usize) < cv.h {
            cv.px[y as usize * cv.w + x as usize] = c;
        }
    }
}
// Keep sparse fixture pixels, not an empty full-room overlay on every seat.
fn details(run: &Run) -> Arc<Vec<(i32, i32, Rgb)>> {
    type DetailKey = (u64, Pack, u64, bool, Option<bool>, usize, bool);
    type Cache = Mutex<Option<(DetailKey, Arc<Vec<(i32, i32, Rgb)>>)>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = (
        run.room().fingerprint(),
        run.dungeon.pack,
        run.tick / 6,
        run.barred(),
        run.room().chest.map(|c| c.open),
        run.traps.len(),
        run.wishing,
    );
    let cache = CACHE.get_or_init(Default::default);
    if let Some((k, pixels)) = cache.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *k == key {
            return pixels.clone();
        }
    }
    let mut im = Img::new(run.room().cols as i32 * 16, run.room().rows as i32 * 16);
    super::room_details(run, &mut im, &mut vec![]);
    let mut pixels = Vec::new();
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                for dy in 0..2 {
                    for dx in 0..2 {
                        pixels.push((x * 2 + dx, y * 2 + dy, c));
                    }
                }
            }
        }
    }
    let pixels = Arc::new(pixels);
    *cache.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, pixels.clone()));
    pixels
}
fn stamp_canvas(cv: &mut super::art_v2::paint::Canvas, im: &Img, x: i32, y: i32) {
    let ox = (cv.origin.0 as i32) * 2;
    let oy = (cv.origin.1 as i32) * 2;
    for yy in 0..im.h {
        for xx in 0..im.w {
            let (px, py) = (x + xx - ox, y + yy - oy);
            if px >= 0 && py >= 0 && (px as usize) < cv.w && (py as usize) < cv.h {
                if let Some(c) = im.get(xx, yy) {
                    cv.px[py as usize * cv.w + px as usize] = c;
                }
            }
        }
    }
}
pub(super) fn v2_foreground(cv: &mut super::art_v2::paint::Canvas, run: &Run, depth: &[i32]) {
    let scene = scene(run, 2);
    let b = bank(run.dungeon.pack, 2);
    let (ox, oy) = (cv.origin.0 as i32 * 2, cv.origin.1 as i32 * 2);
    for o in &scene.objects {
        if let Some(im) = &b.block[o.index] {
            for yy in 0..im.h {
                for xx in 0..im.w {
                    let (x, y) = (o.x + xx - ox, o.feet - im.h + yy - oy);
                    if x >= 0 && y >= 0 && (x as usize) < cv.w && (y as usize) < cv.h {
                        let i = y as usize * cv.w + x as usize;
                        if depth[i] < o.feet {
                            if let Some(c) = im.get(xx, yy) {
                                cv.px[i] = c;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A tight 60% contact ellipse on the ink overlay; no opaque navy stamp.
pub(super) fn contact(cv:&mut Img,run:&Run,x:i32,y:i32,width:i32) {
    let rx=(width/4).clamp(3,14);
    let ground=scene(run,1);
    for dy in -2..=2 {for dx in -rx..=rx {
        if dx*dx*4+dy*dy*rx*rx<=rx*rx*4 {
            let c=cv.get(x+dx,y+dy).or_else(||ground.base.get(x+dx,y+dy)).unwrap_or([0;3]);
            cv.set(x+dx,y+dy,c.map(|v|(v as u16*2/5) as u8));
        }
    }}
}
pub(super) fn foe_rim(im:&Img,pack:Pack)->Img {
    // The source silhouette is retained. A one-pixel upper/left edge
    // distinguishes moving threats from the desaturated static props.
    let mut out=im.clone();
    let tint=match pack {
        Pack::Crypt=>[99,157,194],Pack::Unknown=>[143,163,168],
        Pack::Hellforge=>[110,138,142],Pack::Cavern=>[143,163,168],
        Pack::Fungal=>[179,194,196],Pack::Archive=>[176,178,168],
        Pack::Swamp=>[150,176,140],
    };
    for y in 0..im.h {for x in 0..im.w {
        if let Some(c)=im.get(x,y) {
            if im.get(x-1,y).is_none() || im.get(x,y-1).is_none() {
                // Shared-palette rim replaces only the outer source pixel.
                if u16::from(c[0])+u16::from(c[1])+u16::from(c[2])<250 {out.set(x,y,std::array::from_fn(|k|((c[k] as u16*2+tint[k] as u16)/3) as u8));}
            }
        }
    }}out
}
pub(super) fn knight_shot(cv:&mut Img,p:&crate::rules::Projectile,x:i32,y:i32)->bool {
    if p.hostile || !matches!(p.kind,Shot::Arrow|Shot::Bolt) || p.sprite_card().is_some()
        || p.look.is_some() || p.empowered || p.traits.path.beam.is_some() || p.traits.path.waiting() {return false;}
    let speed=p.vx.abs().max(p.vy.abs()).max(0.01);
    let (dx,dy)=(p.vx/speed,p.vy/speed);
    let at=|n:f32|(x+(dx*n).round() as i32,y+(dy*n).round() as i32);
    let (tx,ty)=at(-5.0);let (hx,hy)=at(2.0);
    // Short direction-bearing stroke; the one-pixel contour follows that
    // stroke instead of making a black disc behind the whole projectile.
    for (ox,oy) in [(-1,0),(1,0),(0,-1),(0,1)] {cv.line(tx+ox,ty+oy,hx+ox,hy+oy,'s');}
    cv.line(tx,ty,hx,hy,'2');
    let (mx,my)=at(-2.0);cv.line(mx,my,hx,hy,'V');
    cv.put(hx,hy,'W');true
}
/// The terminal keeps the native raster. Grade ground and foreground
/// separately before the shared half-block/braille coverage reduction.
pub(super) fn terminal_layers(cv:&super::art_v2::paint::Canvas,run:&Run)->(Img,Img) {
    let mut base=Img::black(cv.w as i32,cv.h as i32);
    for y in 0..cv.h {for x in 0..cv.w {
        let i=y*cv.w+x;let c=cv.px[i];
        let foreground=cv.ground.get(i).is_some_and(|&g|g!=c);
        let value=(54*c[0] as i32+183*c[1] as i32+19*c[2] as i32)/256;
        let hazard=run.room().tile((x as i32+cv.origin.0 as i32*2).div_euclid(32),(y as i32+cv.origin.1 as i32*2).div_euclid(32))==Tile::Hazard;
        let grade=if foreground && value>55 {
            // Preserve steel glints and the original cloth/attack chroma at
            // fractional cell coverage; no scene-wide desaturation.
            std::array::from_fn(|k|((value+(c[k] as i32-value)*145/100)*115/100).clamp(0,226) as u8)
        } else if hazard {
            std::array::from_fn(|k|(value+(c[k] as i32-value)*125/100).clamp(0,226) as u8)
        } else {c.map(|v|(v as u16*3/4) as u8)};
        base.set(x as i32,y as i32,grade);
    }}
    (base,Img::from_rgb(cv.w as i32,cv.h as i32,&cv.fx))
}
pub(super) fn v2_motes(cv:&mut super::art_v2::paint::Canvas,run:&Run) {
    let tint=lighting::colour(run.dungeon.pack);
    for i in 0..14 {
        let v=hash(i,run.at as i32,71);
        let x=((v%768) as u64+run.tick/3)%768;
        let y=(((v>>10)%448) as u64+448-run.tick/5%448)%448;
        let index=y as usize*cv.w+x as usize;
        if index<cv.px.len() {for k in 0..3 {cv.px[index][k]=(cv.px[index][k] as u16*89/100+tint[k] as u16*11/100) as u8;}}
        // Fifteen-percent premultiplied light, covering one real braille
        // sample. Keep it on the atmosphere plane so quiet rooms retain dots.
        let emission=match run.dungeon.pack {
            Pack::Crypt|Pack::Cavern|Pack::Hellforge=>[33,27,20],
            Pack::Archive=>[28,33,20],Pack::Fungal=>[22,33,30],Pack::Unknown=>[31,27,33],
            Pack::Swamp=>[24,32,22],
        };
        let (x,y)=(x as usize/4*4,y as usize/4*4);
        for dy in 0..4 {for dx in 0..4 {
            let at=(y+dy)*cv.w+x+dx;
            if at<cv.fx.len() {for k in 0..3 {cv.fx[at][k]=cv.fx[at][k].max(emission[k]);}}
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn four(pack: Pack) -> Run {
        let mut r = super::super::art_v2::testing::fight();
        for id in 2..=4 {
            r.join_seat(id, &format!("Knight {id}"));
        }
        r.dungeon.pack = pack;
        r.floor_plates.clear();
        r
    }
    #[test]
    fn side_door_cache_distinguishes_foes_from_an_empty_wave_lock() {
        for pack in [Pack::Crypt,Pack::Cavern,Pack::Hellforge,Pack::Fungal,Pack::Unknown] {
            let mut run=Run::new(17,1,None);
            run.dungeon=crate::rules::crawl_floor(2,pack,17);
            let at=run.dungeon.rooms.iter().position(|r|r.kind==RoomKind::Fight).unwrap();
            run.enter_for_test(at);run.calm_for_test();run.enemies.clear();run.waves=Default::default();
            run.phase=Phase::Fighting;
            for side in [1,3] {run.dungeon.rooms[at].open_door(side);}
            run.spawn_at_for_test(EnemyKind::Imp,24.0,6.0);
            assert!(run.barred());
            let closed=[scene(&run,1),scene(&run,2)];
            run.enemies.clear();run.waves.left=1;
            assert!(run.barred());
            for scale in 1..=2 {
                let locked=scene(&run,scale);
                for side in [1,3] {
                    let (x,y)=super::super::doors(run.room())[side];
                    let rect=(x*16*scale,y*16*scale,16*scale,32*scale);
                    let crop=|im:&Img|Img::from_view(im,rect.0,rect.1,rect.2,rect.3).content_hash();
                    assert_ne!(crop(&closed[(scale-1) as usize].base),crop(&locked.base),"{pack:?} side {side} grain {scale}");
                }
            }
        }
    }

    #[test]
    fn both_grains_cover_each_pack_and_only_collision_tiles_get_obstacles() {
        for pack in [
            Pack::Crypt,
            Pack::Cavern,
            Pack::Hellforge,
            Pack::Archive,
            Pack::Fungal,
            Pack::Unknown,
        ] {
            let r = four(pack);
            assert!(
                super::super::drawn_elsewhere(&r),
                "live wasm must retain the slot scenery path"
            );
            let before = serde_json::to_vec(&r).unwrap();
            for s in [1, 2] {
                let a = scene(&r, s);
                let b = bake(&r, s);
                assert_eq!(a.base, b.base, "cached scenery is exact {pack:?}");
                assert_eq!(
                    (a.base.w, a.base.h),
                    (r.room().cols as i32 * 16 * s, r.room().rows as i32 * 16 * s)
                );
                // Native source banks still obey the shared ink gate. RGB
                // illumination is a post-process, like v2's existing lighting.
                for im in bank(pack,s).floor.iter().flatten() {
                    assert!(im.pixels().all(|c| super::super::super::ink::palette_index(c).is_some()));
                }
                assert!(a.shimmer.iter().all(|&(x,y,_)| r.room().tile(x/(16*s),y/(16*s))!=Tile::Wall));
                let blocks = (0..r.room().rows as i32)
                    .flat_map(|row| (0..r.room().cols as i32).map(move |col| (col, row)))
                    .filter(|&(c, row)| r.room().tile(c, row) == Tile::Block)
                    .count();
                assert_eq!(a.objects.len(), blocks);
                for o in &a.objects {
                    assert_eq!(
                        r.room().tile(o.x / (16 * s), o.feet / (16 * s) - 1),
                        Tile::Block
                    );
                    assert!(bank(pack, s).block[o.index].is_some());
                }
                for f in &bank(pack, s).floor[..6] {
                    let f = f.as_ref().unwrap();
                    let brightness = f
                        .pixels()
                        .map(|c| {
                            (u32::from(c[0]) * 54 + u32::from(c[1]) * 183 + u32::from(c[2]) * 19)
                                / 256
                        })
                        .sum::<u32>()
                        / (f.w * f.h) as u32;
                    assert!(brightness < 65, "quiet floor {pack:?}: {brightness}");
                    assert!(
                        (0..16 * s).all(|y| (0..16 * s).all(|x| f.get(x, y).is_some())),
                        "no transparent seams {pack:?}"
                    );
                }
            }
            assert_eq!(before, serde_json::to_vec(&r).unwrap());
        }
    }
    fn luma_of(c: Rgb) -> u32 {
        (u32::from(c[0]) * 54 + u32::from(c[1]) * 183 + u32::from(c[2]) * 19) / 256
    }

    #[test]
    fn a_rooms_floor_away_from_every_light_is_near_black_and_beside_a_sconce_it_is_lit() {
        for pack in [Pack::Crypt, Pack::Cavern, Pack::Hellforge, Pack::Archive, Pack::Fungal, Pack::Unknown] {
            let run = four(pack);
            let room = run.room();
            let t = 16;
            let scene = scene(&run, 1);
            assert!(!scene.fires.is_empty(), "{pack:?}: the room has sconces");
            // Every light the floor knows: sconces, open doors, hazards.
            let mut lights: Vec<(i32, i32)> = lighting::fixtures(room, t);
            for (side, &(c, r)) in super::super::doors(room).iter().enumerate() {
                if room.doors[side] {
                    lights.push((c * t + t / 2, r * t + t / 2));
                }
            }
            for r in 0..room.rows as i32 {
                for c in 0..room.cols as i32 {
                    if room.tile(c, r) == Tile::Hazard {
                        lights.push((c * t + t / 2, r * t + t / 2));
                    }
                }
            }
            let near = |x: i32, y: i32, tiles: i32| lights.iter().any(|&(lx, ly)| (lx - x).pow(2) + (ly - y).pow(2) < (tiles * t).pow(2));
            let (mut far, mut lit) = (Vec::new(), Vec::new());
            for r in 0..room.rows as i32 {
                for c in 0..room.cols as i32 {
                    if room.tile(c, r) != Tile::Floor {
                        continue;
                    }
                    // A tile's middle, away from the walls' faces.
                    let (x, y) = (c * t + t / 2, r * t + t / 2);
                    let Some(px) = scene.base.get(x, y) else { continue };
                    if !near(x, y, 4) {
                        far.push(luma_of(px));
                    } else if scene.fires.iter().any(|&(fx, fy)| (fx - x).pow(2) + (fy - y).pow(2) <= (2 * t).pow(2)) {
                        lit.push(luma_of(px));
                    }
                }
            }
            assert!(far.len() > 10, "{pack:?}: a room has floor away from its lights ({})", far.len());
            let mean = far.iter().sum::<u32>() / far.len() as u32;
            assert!(mean < 20, "{pack:?}: the unlit floor is near black, mean luma {mean}");
            assert!(!lit.is_empty(), "{pack:?}: floor beside a sconce");
            let beside = *lit.iter().max().unwrap();
            assert!(beside >= mean * 2 + 12, "{pack:?}: beside a sconce {beside} against {mean} in the dark");
        }
    }

    #[test]
    fn an_open_floors_deep_rock_is_left_black_and_its_edge_has_no_kit_sconces() {
        let mut run = Run::new(7, 7, None);
        run.join_seat(2, "Wren");
        run.join_seat(3, "Bors");
        run.open_floor_for_test(3);
        assert!(run.open_here().is_some());
        assert!(supported(&run), "the open map takes the slot scenery");
        let room = run.room();
        let scene = scene(&run, 1);
        let t = 16;
        // The rock a wall's art cannot spill onto: rock all round, two deep.
        let core = |c: i32, r: i32| (-1..=1).all(|dr| (-1..=1).all(|dc| super::super::open::deep_rock(room, c + dc, r + dr)));
        let (mut rock, mut walls) = (0, 0);
        for r in 0..room.rows as i32 {
            for c in 0..room.cols as i32 {
                walls += usize::from(room.tile(c, r) == Tile::Wall && !super::super::open::deep_rock(room, c, r));
                if room.tile(c, r) == Tile::Wall && core(c, r) {
                    rock += 1;
                    for y in 0..t {
                        for x in 0..t {
                            let px = scene.base.get(c * t + x, r * t + y).unwrap_or([0; 3]);
                            assert!(px == [0; 3], "deep rock at {c},{r} is black, was {px:?} at {x},{y}");
                        }
                    }
                }
            }
        }
        assert!(walls > 20, "the walls round the floor are still drawn ({walls})");
        assert!(rock > 100, "an open map is mostly rock ({rock})");
        // The kit's sconces round the edge are not used; the floor's own fires light it.
        assert!(scene.fires.is_empty());
        assert!(!lights(&run).is_empty());
        assert_eq!(lights(&run).len(), lighting::open_fires(&run).len());
    }

    #[test]
    fn light_fields_are_tick_only_and_do_not_rebake_for_flicker() {
        let r=four(Pack::Crypt);
        let mut later=r.clone();later.tick+=4;
        for s in [1,2] {
            let a=scene(&r,s);let b=scene(&later,s);
            assert!(Arc::ptr_eq(&a,&b));
            assert!(!a.fires.is_empty());
            assert!(a.shimmer.iter().any(|&(_,_,c)| c[0]!=c[1]));
            let mut im=a.base.clone();flames(&mut im,&r,s,&a);
            let mut other=a.base.clone();flames(&mut other,&later,s,&b);
            assert_ne!(im,other);
        }
    }
    #[test]
    fn friendly_arrow_keeps_a_thin_directional_core_without_a_disc() {
        let p:crate::rules::Projectile=serde_json::from_value(serde_json::json!({
            "x":7.0,"y":7.0,"vx":1.0,"vy":0.0,"hostile":false,
            "kind":"arrow","damage":1,"pierce":0,"ttl":100
        })).unwrap();
        let mut im=Img::new(32,32);
        assert!(knight_shot(&mut im,&p,16,16));
        assert!((0..32).all(|y| (0..32).all(|x| (15..=17).contains(&y) || im.get(x,y).is_none())));
        assert!(im.get(18,16).is_some());
        let mut hostile=p;hostile.hostile=true;
        assert!(!knight_shot(&mut im,&hostile,16,16));
    }
    #[test]
    fn cell_grade_preserves_raster_and_separates_actor_chroma_from_ground() {
        let r=four(Pack::Crypt);
        let mut cv=super::super::art_v2::paint::Canvas::new(16,16,[50;3]);
        cv.mark_ground();cv.px[17]=[180,45,35];
        let (base,_) = terminal_layers(&cv,&r);
        assert_eq!((base.w,base.h),(16,16));
        let body=base.get(1,1).unwrap();
        assert!(body[0]-body[1]>135,"cloth saturation survives coverage");
        assert_eq!(base.get(0,0),Some([37;3]));
        for im in bank(Pack::Fungal,2).decor.iter().flatten() {
            assert!(im.pixels().all(|c|!matches!(c,[80,198,179]|[143,232,220]|[228,255,248])));
        }
    }
    #[test]
    fn every_hazard_neighbourhood_stays_inside_its_collision_tile() {
        for pack in [
            Pack::Crypt,
            Pack::Cavern,
            Pack::Hellforge,
            Pack::Archive,
            Pack::Fungal,
            Pack::Unknown,
        ] {
            let r = four(pack);
            for s in [1, 2] {
                for mask in 0..256 {
                    let mut room = r.room().clone();
                    room.set(1, 1, Tile::Hazard);
                    for (i, (dx, dy)) in [
                        (-1, -1),
                        (0, -1),
                        (1, -1),
                        (-1, 0),
                        (1, 0),
                        (-1, 1),
                        (0, 1),
                        (1, 1),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        room.set(
                            (1 + dx) as usize,
                            (1 + dy) as usize,
                            if mask & (1 << i) != 0 {
                                Tile::Hazard
                            } else {
                                Tile::Floor
                            },
                        );
                    }
                    let t = 16 * s;
                    let mut cv = Img::new(t * 3, t * 3);
                    hazard(&mut cv, bank(pack, s), &room, 1, 1, s, 0);
                    for y in 0..cv.h {
                        for x in 0..cv.w {
                            if x < t || x >= 2 * t || y < t || y >= 2 * t {
                                assert_eq!(
                                    cv.get(x, y),
                                    None,
                                    "hazard must not paint dry collision tiles"
                                );
                            }
                        }
                    }
                    assert!((t..2 * t).any(|y| (t..2 * t).any(|x| cv.get(x, y).is_some())));
                }
            }
        }
    }
    #[test]
    fn door_state_and_depth_order_change_art_without_changing_geometry() {
        let mut r = four(Pack::Crypt);
        let at = r.at;
        // A prop to stand behind, whatever the floor laid in this room.
        if !(0..r.room().rows as i32).any(|y| (0..r.room().cols as i32).any(|x| r.room().tile(x, y) == Tile::Block)) {
            r.dungeon.rooms[at].set_for_test(5, 5, Tile::Block);
        }
        for side in 0..4 {
            r.dungeon.rooms[at].open_door(side);
        }
        let mut open = r.clone();
        open.enemies.clear();
        open.phase = Phase::Exploring;
        assert_eq!(r.room().fingerprint(), open.room().fingerprint());
        assert_ne!(scene(&r, 1).base, scene(&open, 1).base);
        for scale in [1, 2] {
            let barred = scene(&r, scale);
            let clear = scene(&open, scale);
            let t = 16 * scale;
            for (side, &(c, row)) in super::super::doors(r.room()).iter().enumerate() {
                let (x0, y0) = (c * t, row * t);
                let (w, h) = if side % 2 == 0 {
                    (2 * t, t)
                } else {
                    (t, 2 * t)
                };
                assert!(
                    (y0..y0 + h).any(
                        |y| (x0..x0 + w).any(|x| barred.base.get(x, y) != clear.base.get(x, y))
                    ),
                    "door {side} changes at grain {scale}"
                );
            }
        }
        let scene = scene(&r, 2);
        let o = scene.objects[0];
        let b = bank(Pack::Crypt, 2);
        let im = b.block[o.index].as_ref().unwrap();
        let mut cv = super::super::art_v2::paint::Canvas::new(
            scene.base.w as usize,
            scene.base.h as usize,
            [17, 19, 23],
        );
        let behind = vec![i32::MIN; cv.px.len()];
        v2_foreground(&mut cv, &r, &behind);
        assert!(
            (0..im.h).any(|y| (0..im.w).any(|x| im.get(x, y).is_some_and(|c| cv.px
                [(o.feet - im.h + y) as usize * cv.w + (o.x + x) as usize]
                == c)))
        );
        let mut legacy = Img::new(scene.base.w / 2, scene.base.h / 2);
        let (w, h) = (legacy.w, legacy.h);
        legacy_body(&mut legacy, &r, 0.0, |cv| cv.rect(0, 0, w, h, '6'));
        let first = super::scene(&r, 1).objects[0];
        let low = bank(Pack::Crypt, 1).block[first.index].as_ref().unwrap();
        assert!(
            (0..low.h).any(|y| (0..low.w).any(|x| low.get(x, y).is_some()
                && legacy.get(first.x + x, first.feet - low.h + y).is_none()))
        );
        legacy_body(&mut legacy, &r, 1000.0, |cv| cv.rect(0, 0, w, h, '6'));
        assert!(
            legacy
                .pixels()
                .all(|c| c == super::super::super::ink::ink('6').unwrap()),
            "dressed/rolling front knights keep their samples"
        );
        cv.px.fill([17, 19, 23]);
        let front = vec![i32::MAX; cv.px.len()];
        v2_foreground(&mut cv, &r, &front);
        assert!(
            cv.px.iter().all(|&c| c == [17, 19, 23]),
            "a knight in front keeps every body sample"
        );
    }
    #[test]
    fn four_seat_rendering_is_read_only_and_lights_out_retains_its_old_path() {
        let mut r = four(Pack::Crypt);
        let before = serde_json::to_vec(&r).unwrap();
        for id in 1..=4 {
            let a = super::super::frame_v1(&r, 384, 224, Some(id));
            assert_eq!(a, super::super::frame_v1(&r, 384, 224, Some(id)));
            let a = super::super::art_v2::frame(&r, 768, 448, Some(id));
            assert_eq!(a, super::super::art_v2::frame(&r, 768, 448, Some(id)));
        }
        assert_eq!(before, serde_json::to_vec(&r).unwrap());
        r.mode = crate::rules::fortune::Mode::LightsOut;
        assert!(!supported(&r));
    }
}

/// v1 transport retains the realm's indexed-ink contract after RGB lighting.
/// Exact RGB neighbours are memoised; native v2 keeps the continuous grade.
pub(super) fn palette_frame(im: Img) -> Img {
    let mut snap = snapper();
    let pixels=im.words().iter().map(|&word| {
        let v=snap([(word>>16) as u8,(word>>8) as u8,word as u8]);
        0xff00_0000 | (v[0] as u32)<<16 | (v[1] as u32)<<8 | v[2] as u32
    }).collect();
    Img::from_words(im.w,im.h,pixels)
}

/// The colour `palette_frame` snaps each pixel to, its memo held while the
/// snapper lives: a frame in parts snaps what it composes the same way
/// (`room_scene`).
pub(super) fn snapper() -> impl FnMut(Rgb) -> Rgb {
    use super::super::ink::{PALETTE,RgbMap,rgb};
    static COLOURS:OnceLock<Mutex<RgbMap<Rgb>>>=OnceLock::new();
    let mut colours=COLOURS.get_or_init(|| Mutex::new(PALETTE.iter().map(|&c|(rgb(c),rgb(c))).collect()))
        .lock().unwrap_or_else(|e|e.into_inner());
    move |c: Rgb| *colours.entry(c).or_insert_with(|| PALETTE.iter().filter(|&&p|p!=0xe4fff8).map(|&p|rgb(p))
        .min_by_key(|p|(0..3).map(|k|(p[k] as i32-c[k] as i32).pow(2)).sum::<i32>()).unwrap())
}

/// `im`'s inked pixels snapped as `palette_frame` snaps them; its paper
/// stays clear.
pub(super) fn palette_layer(im: &mut Img) {
    let mut snap = snapper();
    for y in 0..im.h {
        for x in 0..im.w {
            if let Some(c) = im.get(x, y) {
                im.set(x, y, snap(c));
            }
        }
    }
}

/// The delivered kit's fixtures as the room's own fires, for a window that
/// lights the room itself (`room_scene`): their pools are already in the
/// still art, as a brazier's are in the first kit's.
pub(super) fn lights(run: &Run) -> Vec<super::super::light::Light> {
    if run.open_here().is_some() {
        return lighting::open_fires(run);
    }
    lighting::fixtures(run.room(), 16)
        .into_iter()
        .map(|(x, y)| super::super::light::Light { x: x as f32, y: y as f32, r: 52.0, s: 0.28, fire: true })
        .collect()
}
