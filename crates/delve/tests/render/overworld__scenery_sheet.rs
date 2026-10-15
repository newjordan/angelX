//! The world's scenery on one sheet: the material system's trees, bushes,
//! rocks, buildings, walls and water; the kit's buildings, terrain tiles
//! and landmarks; the dungeon's walls, props and lights.
//! `ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve write_scenery_sheet -- --ignored`
//! writes `scenery-sheet.png` (four times, nearest neighbour) and
//! `scenery-sheet-1x.png`.

use super::super::ink::Img;
use super::super::kit::{self, House as KHouse, Roof, Tiles, Trade, Wall};
use super::super::landmarks;
use super::super::material::{self, House, Leaf, ramp};
use super::sprites;
use crate::rules::RoomKind;

/// A row of the sheet: things stood on a common floor line, left to right.
struct Row {
    im: Img,
    x: i32,
    floor: i32,
}

impl Row {
    fn new(w: i32, h: i32) -> Row {
        Row { im: Img::black(w, h), x: 6, floor: h - 6 }
    }

    /// `im` with its foot on the floor line.
    fn put(&mut self, im: &Img) {
        self.im.stamp(im, self.x, self.floor - im.h + 1);
        self.x += im.w + 6;
    }

    /// A drawing made straight on a scratch canvas `w` x `h` with its foot
    /// on the canvas's bottom, then stood on the row.
    fn paint(&mut self, w: i32, h: i32, draw: impl FnOnce(&mut Img)) {
        let mut cv = Img::black(w, h);
        draw(&mut cv);
        // Black paper is no part of the drawing.
        let mut out = Img::new(w, h);
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = cv.get(x, y).filter(|&c| c != [0, 0, 0]) {
                    out.set(x, y, c);
                }
            }
        }
        self.put(&out);
    }
}

fn rows() -> Vec<Img> {
    let keep = |_: i32, _: i32| true;
    let mut out = Vec::new();

    // The material system's living things.
    let mut r = Row::new(640, 64);
    for (k, leaf) in [Leaf::Oak, Leaf::Focal, Leaf::Copper, Leaf::Hedge, Leaf::Pine, Leaf::Edge, Leaf::EdgePine]
        .into_iter()
        .enumerate()
    {
        r.paint(34, 46, |cv| material::tree(cv, 16, 45, 10.0, leaf, 11 + k as u32, &keep));
    }
    r.paint(24, 18, |cv| material::bush(cv, 11, 17, 7.0, Leaf::Oak, 3));
    r.paint(24, 18, |cv| material::bush(cv, 11, 17, 7.0, Leaf::Copper, 4));
    r.paint(22, 16, |cv| material::boulder(cv, 10, 15, 7.0, ramp::STONE, 5));
    r.paint(22, 16, |cv| material::boulder(cv, 10, 15, 7.0, ramp::SLATE, 6));
    r.paint(20, 16, |cv| material::hay(cv, 9, 15, 6.0, 7));
    r.paint(16, 16, |cv| material::reeds(cv, 8, 15, 8));
    out.push(r.im);

    // The kit's terrain tiles: every tree, and rocks of each kind.
    let tiles = Tiles::get();
    let mut r = Row::new(640, 40);
    for v in 0..8 {
        r.put(tiles.tree(v));
    }
    for kind in [kit::RockKind::Slate, kit::RockKind::Ore, kit::RockKind::Char] {
        for v in 0..3 {
            r.put(tiles.rock(kind, v));
        }
    }
    r.put(&tiles.dead);
    out.push(r.im);

    // The material system's buildings.
    let mut r = Row::new(640, 64);
    for (k, (roof, stone)) in [(ramp::THATCH, true), (ramp::TILE, false), (ramp::SLATE, true), (ramp::TILE, true)]
        .into_iter()
        .enumerate()
    {
        r.paint(52, 46, |cv| material::house(cv, (4, 8, 42, 34), &House { roof, stone, windows: 2 }, 20 + k as u32));
    }
    r.paint(40, 30, |cv| material::block(cv, (2, 4, 30, 22), ramp::STONE, false, 21));
    r.paint(40, 30, |cv| material::block(cv, (2, 4, 30, 22), ramp::TIMBER, true, 22));
    r.paint(40, 22, |cv| material::hedge(cv, (2, 4, 34, 14), 23));
    r.paint(30, 60, |cv| material::tower(cv, 14, 58, 7, 34, 24));
    r.paint(32, 18, |cv| {
        material::barrel(cv, 6, 17);
        material::crate_box(cv, 20, 17, 25);
    });
    out.push(r.im);

    // The kit's buildings, as the kingdom and the mini realm stand them.
    let mut r = Row::new(640, 72);
    for (wall, roof) in [(Wall::Stone, Roof::Tile), (Wall::Timber, Roof::Slate), (Wall::Plaster, Roof::Thatch), (Wall::Dark, Roof::Slate)] {
        r.put(&kit::house(&KHouse {
            w: 48,
            h: 40,
            roof_h: 17,
            wall,
            roof,
            door_glow: false,
            windows: 2,
            lit: false,
            chimney: true,
        }));
    }
    for v in 0..4 {
        r.put(&kit::cottage(v, v == 1));
    }
    r.put(&kit::workshop(Trade::Smith, true));
    r.put(&kit::chapel(false));
    r.put(&kit::well());
    r.put(&kit::market_stall(1));
    out.push(r.im);

    let mut r = Row::new(640, 96);
    r.put(&kit::keep(1));
    r.put(&kit::gatehouse());
    r.put(&kit::mine_mouth());
    r.put(&kit::windmill(0));
    r.put(&kit::docks());
    r.put(&kit::ruins());
    out.push(r.im);

    // Landmarks of the countryside.
    let mut r = Row::new(640, 80);
    for kind in [RoomKind::Orchard, RoomKind::Bothy, RoomKind::Timberyard, RoomKind::Belfry, RoomKind::Crag, RoomKind::Aqueduct] {
        if let Some(im) = landmarks::landmark(kind) {
            r.put(&im);
        }
    }
    out.push(r.im);

    // The dungeon: wall tiles of each pack, open to the floor at the south
    // and east, and its props and lights.
    let mut r = Row::new(640, 52);
    let packs: [(&[char], bool); 5] = [
        (ramp::SLATE, false),
        (ramp::STONE, false),
        (&['n', 'b', 'I', 'B', 'P', 'r', 'o'], true),
        (&['n', 'D', 'F', 'E', 'e', 'l', 'm'], true),
        (&['k', 'K', 'X', 'g', 'j', 'G', 'J'], false),
    ];
    for (k, (ramp, rough)) in packs.into_iter().enumerate() {
        r.paint(52, 40, |cv| {
            for ty in 0..2 {
                for tx in 0..3 {
                    let open = [false, tx == 2, ty == 1, false];
                    material::wall_tile(cv, (tx * 16, ty * 16), 16, open, ramp, rough, 300 + k as u32);
                }
            }
        });
    }
    out.push(r.im);

    let mut r = Row::new(640, 40);
    for v in 0..3 {
        r.put(&sprites::tomb(v));
    }
    for v in 0..2 {
        r.put(&sprites::shelf(v));
    }
    for v in 0..3 {
        r.put(&sprites::mushroom(v));
    }
    r.put(&sprites::eye_pillar(0, 1));
    r.put(&sprites::stairs(true));
    r.put(&sprites::portcullis(true));
    r.put(&sprites::brazier(true, 3, 1));
    r.put(&kit::torch(3, 1));
    r.put(&kit::chest(true));
    r.put(&kit::lantern(true));
    out.push(r.im);

    let mut r = Row::new(640, 44);
    r.paint(90, 36, |cv| material::water(cv, (2, 2, 86, 32), 3, 12));
    r.paint(70, 36, |cv| material::ashlar(cv, (2, 2, 66, 32), ramp::STONE, 31));
    r.paint(70, 36, |cv| material::boards(cv, (2, 2, 66, 32), true, 32));
    r.paint(70, 36, |cv| material::rubble(cv, (2, 2, 66, 32), ramp::SLATE, 6, 33));
    out.push(r.im);
    out
}

#[test]
#[ignore]
fn write_scenery_sheet() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rows = rows();
    let h: i32 = rows.iter().map(|r| r.h).sum();
    let mut sheet = Img::black(640, h);
    let mut y = 0;
    for r in &rows {
        sheet.stamp(r, 0, y);
        y += r.h;
    }
    let save = |name: &str, k: u32| {
        image::RgbaImage::from_raw(sheet.w as u32 * k, sheet.h as u32 * k, sheet.rgba_scaled(k))
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    save("scenery-sheet.png", 4);
    save("scenery-sheet-1x.png", 1);
}

/// The world where players walk it, at the browser's 1280x720, in both art
/// kits: the kingdom at the ranch, a crypt fight, the college hall, the
/// fields, the ranch, the stables, the village and home, and a fight in
/// each of the other packs. `scene-<name>-<kit>.png`.
#[test]
#[ignore]
fn write_scenery_scenes() {
    use super::art_v2::testing;
    use super::frame;
    use crate::rules::Pack;
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img| {
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes())
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    };
    let packed = |pack: Pack| {
        let mut run = testing::fight();
        run.dungeon.pack = pack;
        run
    };
    // A fight room with a pit or a pool cut in its open floor.
    let flooded = move |pack: Pack| {
        let mut run = packed(pack);
        let at = run.at;
        let room = &mut run.dungeon.rooms[at];
        for (c0, r0, w, h) in [(9usize, 2usize, 4usize, 2usize), (16, 9, 3, 2), (17, 11, 2, 1)] {
            for c in c0..c0 + w {
                for r in r0..r0 + h {
                    if room.tile(c as i32, r as i32) == crate::rules::Tile::Floor {
                        room.set(c, r, crate::rules::Tile::Hazard);
                    }
                }
            }
        }
        run
    };
    let scenes: Vec<(&str, Box<dyn Fn() -> crate::rules::Run>)> = vec![
        ("kingdom", Box::new(testing::kingdom)),
        ("crypt", Box::new(testing::fight)),
        ("archive", Box::new(move || packed(Pack::Archive))),
        ("mines", Box::new(move || packed(Pack::Cavern))),
        ("keep", Box::new(move || packed(Pack::Hellforge))),
        ("fungal", Box::new(move || packed(Pack::Fungal))),
        ("college", Box::new(|| testing::home(RoomKind::CollegeHall, 83))),
        ("fields", Box::new(|| testing::home(RoomKind::Pasture, 20))),
        ("ranch", Box::new(|| testing::home(RoomKind::Ranch, 40))),
        ("stables", Box::new(|| testing::home(RoomKind::Stables, 40))),
        ("village", Box::new(|| testing::home(RoomKind::Village, 40))),
        ("home", Box::new(|| testing::home(RoomKind::Home, 40))),
        ("crypt-pit", Box::new(move || flooded(Pack::Crypt))),
        ("mines-pool", Box::new(move || flooded(Pack::Cavern))),
    ];
    // The whole kingdom map as the Delve walks it, at one pixel a pixel.
    let k = testing::kingdom();
    save("kingdom-atlas.png", &super::super::kingdom::atlas(&k.kingdom.wishes));
    let before = crate::settings::get();
    for (kit, v2) in [("v1", false), ("v2", true)] {
        crate::settings::set(crate::settings::Settings { art_v2: v2, ..before });
        for (name, run) in &scenes {
            save(&format!("scene-{name}-{kit}.png"), &frame(&run(), 1280, 720));
        }
    }
    crate::settings::set(before);
}

/// Frame time where the scenery is baked again as the tick runs (home rooms
/// redraw their scenery on every torch step), room by room:
/// `cargo test --release -p angel-delve time_scenery_rooms -- --ignored --nocapture`.
#[test]
#[ignore]
fn time_scenery_rooms() {
    use super::art_v2::testing;
    use super::frame;
    let time = |label: &str, mut run: crate::rules::Run| {
        // The first frame draws what is kept (the kingdom's whole map, a
        // room's still layers); the rest are the frames a player sees.
        let start = std::time::Instant::now();
        std::hint::black_box(frame(&run, super::NATIVE_W, super::NATIVE_H));
        let first = start.elapsed().as_secs_f64() * 1000.0;
        let n = 600;
        let start = std::time::Instant::now();
        for _ in 0..n {
            run.tick += 1;
            std::hint::black_box(frame(&run, super::NATIVE_W, super::NATIVE_H));
        }
        let us = start.elapsed().as_micros() as f64 / n as f64;
        eprintln!("SCENERY {label:<12} {us:>8.1} us a frame, first {first:>7.1} ms");
    };
    time("crypt", testing::fight());
    time("kingdom", testing::kingdom());
    for (label, kind) in [
        ("college", RoomKind::CollegeHall),
        ("fields", RoomKind::Pasture),
        ("ranch", RoomKind::Ranch),
        ("stables", RoomKind::Stables),
        ("village", RoomKind::Village),
        ("home", RoomKind::Home),
    ] {
        time(label, testing::home(kind, 40));
    }
}
