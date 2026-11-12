//! The Works' flat kit: every plan drawn in every state, checked and
//! shown. `ANGEL_WORKS_SHOTS=<dir> cargo test -p angel-delve --lib
//! write_works_sheets -- --ignored` writes the contact sheets and the
//! frames the GIFs are made of.

use super::*;
use crate::render::overworld::ink::Img;
use std::path::Path;

fn states() -> Vec<State> {
    let mut v: Vec<State> = (0..STEPS).map(State::Building).collect();
    v.extend([State::Idle, State::Working, State::Starved(Load::Timber), State::BackedUp(Load::Stone)]);
    v
}

fn lit(im: &Img) -> usize {
    (0..im.h).flat_map(|y| (0..im.w).map(move |x| (x, y))).filter(|&(x, y)| im.get(x, y).is_some()).count()
}

#[test]
fn every_plan_draws_in_every_state_at_its_size() {
    for id in plans() {
        let (w, h) = sprite_size(id).unwrap();
        for state in states() {
            for frame in 0..state.frames() {
                let im = flat(id, state, frame).unwrap();
                assert_eq!((im.w, im.h), (w, h), "{id} {state:?}");
                assert!(lit(&im) > (w * h) as usize / 12, "{id} {state:?} frame {frame} is nearly empty");
            }
        }
    }
}

#[test]
fn a_working_building_moves_and_loops() {
    for id in plans() {
        let frames: Vec<_> = (0..FRAMES).map(|f| flat(id, State::Working, f).unwrap()).collect();
        let moving = frames.iter().any(|f| **f != *frames[0]);
        assert!(moving, "{id} does not move when it works");
        assert_eq!(*flat(id, State::Working, FRAMES).unwrap(), *frames[0], "{id} does not loop");
        let idle = flat(id, State::Idle, 0).unwrap();
        assert!(*idle != *frames[0], "{id} looks the same working and idle");
    }
}

#[test]
fn states_read_differently() {
    for id in plans() {
        let idle = flat(id, State::Idle, 0).unwrap();
        let starved = flat(id, State::Starved(Load::Lime), 0).unwrap();
        let backed = flat(id, State::BackedUp(Load::Lime), 0).unwrap();
        assert!(*idle != *starved && *idle != *backed && *starved != *backed, "{id}");
        // Construction climbs: each step shows more of the building.
        let counts: Vec<usize> = (0..STEPS).map(|s| lit(&flat(id, State::Building(s), 0).unwrap())).collect();
        assert!(counts.windows(2).all(|w| w[0] != w[1]), "{id} steps {counts:?}");
    }
}

#[test]
fn no_two_plans_draw_alike() {
    let all: Vec<_> = plans().map(|id| (id, flat(id, State::Idle, 0).unwrap())).collect();
    for (i, (a, ia)) in all.iter().enumerate() {
        for (b, ib) in &all[i + 1..] {
            assert!(ia != ib, "{a} and {b} are the same picture");
        }
    }
}

#[test]
fn a_hundred_buildings_are_cheap() {
    use std::time::Instant;
    // The first sight of a (plan, state, frame) draws it; every later one is a lookup of a shared picture.
    let ids: Vec<&str> = plans().collect();
    let started = Instant::now();
    for k in 0..100 {
        let _ = flat(ids[k % ids.len()], State::Working, (k % 8) as u32).unwrap();
    }
    let first = started.elapsed();
    let started = Instant::now();
    let mut pixels = 0;
    for _ in 0..10 {
        for k in 0..100 {
            pixels += flat(ids[k % ids.len()], State::Working, (k % 8) as u32).unwrap().w;
        }
    }
    let again = started.elapsed() / 10;
    assert!(pixels > 0);
    println!("works_kit flat: 100 buildings first sight {first:?}; a frame of a hundred again {again:?}");
    assert!(again.as_micros() < 5_000, "{again:?}");
}

// ----------------------------------------------------------- the proof ----

const GRASS: char = 'E';

fn backdrop(w: i32, h: i32) -> Img {
    let mut im = Img::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let n = crate::render::overworld::ink::hash(x, y, 5) % 23;
            im.put(x, y, if n == 0 { 'l' } else if n < 3 { 'F' } else { GRASS });
        }
    }
    im
}

fn save(dir: &Path, name: &str, im: &Img, scale: u32) {
    std::fs::create_dir_all(dir).unwrap();
    image::RgbaImage::from_raw(im.w as u32 * scale, im.h as u32 * scale, im.rgba_scaled(scale))
        .unwrap()
        .save(dir.join(format!("{name}.png")))
        .unwrap();
}

/// A sheet of `ids`, a row each: the four steps of raising, idle, working
/// at four steps of its loop, starved, backed up.
fn sheet(ids: &[&str]) -> Img {
    let cols: Vec<State> = (0..STEPS)
        .map(State::Building)
        .chain([State::Idle])
        .chain([State::Working; 4])
        .chain([State::Starved(Load::Lime), State::BackedUp(Load::Stone)])
        .collect();
    let cell_w = ids.iter().filter_map(|id| sprite_size(id)).map(|s| s.0).max().unwrap_or(32) + 6;
    let rows: Vec<i32> = ids.iter().filter_map(|id| sprite_size(id)).map(|s| s.1 + 12).collect();
    let label = 40;
    let mut sheet = backdrop(label + cols.len() as i32 * cell_w, rows.iter().sum::<i32>() + 4);
    let mut y = 2;
    for (id, rh) in ids.iter().zip(&rows) {
        sheet.text(2, y + rh / 2 - 3, &id.chars().take(7).collect::<String>(), 'H');
        let mut working = 0;
        for (c, state) in cols.iter().enumerate() {
            let frame = if *state == State::Working {
                working += 1;
                (working - 1) * 2
            } else {
                0
            };
            let im = flat(id, *state, frame as u32).unwrap();
            sheet.stamp(&im, label + c as i32 * cell_w + 3, y + rh - 4 - im.h);
        }
        y += rh;
    }
    sheet
}

#[test]
#[ignore = "writes the contact sheets when ANGEL_WORKS_SHOTS is set"]
fn write_works_sheets() {
    let Some(dir) = std::env::var_os("ANGEL_WORKS_SHOTS") else { return };
    let dir = Path::new(&dir).join("flat");
    let ids: Vec<&str> = plans().collect();
    for (n, group) in ids.chunks(5).enumerate() {
        let s = sheet(group);
        save(&dir, &format!("sheet-{n:02}-x1"), &s, 1);
        save(&dir, &format!("sheet-{n:02}-x2"), &s, 2);
        save(&dir, &format!("sheet-{n:02}-x4"), &s, 4);
    }
}

/// Detail sheets: idle, two steps of work, starved and backed up, large, a
/// few plans to a sheet, for looking at.
#[test]
#[ignore = "writes the detail sheets when ANGEL_WORKS_SHOTS is set"]
fn write_works_details() {
    let Some(dir) = std::env::var_os("ANGEL_WORKS_SHOTS") else { return };
    let dir = Path::new(&dir).join("flat-detail");
    let ids: Vec<&str> = plans().collect();
    let cols = [State::Idle, State::Working, State::Working, State::Starved(Load::Lime), State::BackedUp(Load::Timber)];
    for (n, group) in ids.chunks(4).enumerate() {
        let cell_w = group.iter().filter_map(|id| sprite_size(id)).map(|s| s.0).max().unwrap_or(32) + 6;
        let rows: Vec<i32> = group.iter().filter_map(|id| sprite_size(id)).map(|s| s.1 + 8).collect();
        let mut sheet = backdrop(cols.len() as i32 * cell_w, rows.iter().sum::<i32>() + 2);
        let mut y = 1;
        for (id, rh) in group.iter().zip(&rows) {
            for (c, state) in cols.iter().enumerate() {
                let frame = if c == 2 { 4 } else { 0 };
                let im = flat(id, *state, frame).unwrap();
                sheet.stamp(&im, c as i32 * cell_w + 3, y + rh - 3 - im.h);
            }
            y += rh;
        }
        save(&dir, &format!("detail-{n:02}"), &sheet, 4);
    }
}

// ------------------------------------------------- lines, loads, hands ----

#[test]
fn lines_and_hands_draw() {
    use lines::{Dir, Kind};
    for kind in Kind::ALL {
        for links in 0..16u8 {
            let im = lines::line_flat(kind, links, links % 2 == 0, 0);
            assert_eq!((im.w, im.h), (lines::TILE_W, lines::tile_h(kind)), "{kind:?} {links}");
        }
        for dir in [Dir::East, Dir::West, Dir::North, Dir::South] {
            for load in std::iter::once(None).chain(Load::ALL.iter().map(|l| Some(*l))) {
                for f in 0..FRAMES {
                    assert!(lit(&lines::vehicle_flat(kind, dir, load, f)) > 20, "{kind:?} {dir:?} {load:?}");
                }
            }
        }
    }
    for trade in Trade::ALL {
        for act in [Act::Idle, Act::Walk, Act::Work, Act::Carry(Load::Stone)] {
            for t in 0..FRAMES {
                let h = hands::hand(trade, act, t, false);
                assert!(lit(&h.img) > 40, "{trade:?} {act:?}");
            }
        }
    }
}

#[test]
fn loads_read_differently() {
    let mut seen: Vec<(Load, Img)> = Vec::new();
    for load in Load::ALL {
        let mut im = Img::new(24, 14);
        loads::heap(&mut im, 12, 13, 20, 10, load, 3);
        for (other, o) in &seen {
            assert!(*o != im, "{load:?} and {other:?} are drawn alike");
        }
        seen.push((load, im));
    }
}

#[test]
fn every_good_has_a_look() {
    for (good, load) in [("stone", Load::Stone), ("limestone", Load::Stone), ("quicklime", Load::Lime), ("mortar", Load::Mortar), ("charcoal", Load::Charcoal), ("coal", Load::Coal), ("iron-ore", Load::Ore), ("plank", Load::Planks), ("timber", Load::Timber), ("grain", Load::Grain), ("flour", Load::Flour), ("rope", Load::Rope), ("ashlar-limestone", Load::Ashlar)] {
        assert_eq!(Load::of(good), load, "{good}");
    }
}

#[test]
#[ignore = "writes the lines, loads and hands sheets when ANGEL_WORKS_SHOTS is set"]
fn write_works_lines_and_hands() {
    use lines::{Dir, Kind};
    let Some(dir) = std::env::var_os("ANGEL_WORKS_SHOTS") else { return };
    let dir = Path::new(&dir).join("flat-lines");

    // Loads: every one, heaped.
    let mut sheet = backdrop(Load::ALL.len() as i32 * 26 + 4, 24);
    for (i, load) in Load::ALL.iter().enumerate() {
        loads::heap(&mut sheet, i as i32 * 26 + 14, 20, 22, 12, *load, 3);
    }
    save(&dir, "loads", &sheet, 4);

    // The laid line per kind: every join, with and without a post.
    for kind in Kind::ALL {
        let th = lines::tile_h(kind);
        let mut sheet = backdrop(16 * 18 + 4, th * 2 + 6);
        for links in 0..16u8 {
            for (row, post) in [false, true].into_iter().enumerate() {
                let im = lines::line_flat(kind, links, post, 0);
                sheet.stamp(&im, links as i32 * 18 + 2, row as i32 * (th + 2) + 2);
            }
        }
        save(&dir, &format!("line-{}", kind.id()), &sheet, 4);
    }

    // Vehicles: each kind, four ways, empty and with a few loads, two steps.
    let loads = [None, Some(Load::Stone), Some(Load::Lime), Some(Load::Timber), Some(Load::Ore), Some(Load::Grain)];
    for kind in Kind::ALL {
        let cell = lines::vehicle_flat(kind, Dir::East, None, 0).w + 4;
        let mut sheet = backdrop(loads.len() as i32 * cell * 2 + 4, 4 * 36 + 4);
        for (r, d) in [Dir::East, Dir::West, Dir::North, Dir::South].into_iter().enumerate() {
            for (c, load) in loads.iter().enumerate() {
                let im = lines::vehicle_flat(kind, d, *load, 0);
                sheet.stamp(&im, (c as i32 * 2) * cell + 4, r as i32 * 36 + 34 - im.h);
                let im = lines::vehicle_flat(kind, d, *load, 3);
                sheet.stamp(&im, (c as i32 * 2 + 1) * cell + 4, r as i32 * 36 + 34 - im.h);
            }
        }
        save(&dir, &format!("vehicle-{}", kind.id()), &sheet, 4);
    }

    // Hands: every trade idle, walking, working (four steps), carrying.
    let mut sheet = backdrop(9 * 30 + 4, Trade::ALL.len() as i32 * 28 + 4);
    for (r, trade) in Trade::ALL.iter().enumerate() {
        let acts = [(Act::Idle, 0), (Act::Walk, 0), (Act::Walk, 2), (Act::Work, 0), (Act::Work, 2), (Act::Work, 4), (Act::Work, 6), (Act::Carry(Load::Stone), 1), (Act::Carry(Load::Planks), 3)];
        for (c, (act, t)) in acts.into_iter().enumerate() {
            hands::stamp(&mut sheet, c as i32 * 30 + 16, r as i32 * 28 + 26, *trade, act, t, false);
        }
    }
    save(&dir, "hands", &sheet, 4);
}

/// The frames of every plan's working loop, each on the hillside, for the
/// GIFs (`art/make-gifs.sh` joins them): `frames/<id>/NN.png`; and the
/// loops of the hands and of the carriers.
#[test]
#[ignore = "writes the working loops when ANGEL_WORKS_SHOTS is set"]
fn write_works_frames() {
    use lines::{Dir, Kind};
    let Some(dir) = std::env::var_os("ANGEL_WORKS_SHOTS") else { return };
    let dir = Path::new(&dir).join("frames");
    for id in plans() {
        let (w, h) = sprite_size(id).unwrap();
        for f in 0..FRAMES {
            let mut sheet = backdrop(w + 16, h + 14);
            sheet.stamp(&flat(id, State::Working, f).unwrap(), 8, 8);
            save(&dir.join(id), &format!("{f:02}"), &sheet, 6);
        }
        // The same building idle, to set beside it.
        let mut sheet = backdrop(w + 16, h + 14);
        sheet.stamp(&flat(id, State::Idle, 0).unwrap(), 8, 8);
        save(&dir.join(id), "idle", &sheet, 6);
    }
    // Hands: every trade at work, four to a frame.
    for (n, group) in Trade::ALL.chunks(8).enumerate() {
        for f in 0..FRAMES {
            let mut sheet = backdrop(group.len() as i32 * 24 + 8, 40);
            for (i, trade) in group.iter().enumerate() {
                hands::stamp(&mut sheet, i as i32 * 24 + 16, 32, *trade, Act::Work, f, false);
            }
            save(&dir.join(format!("hands-{n}")), &format!("{f:02}"), &sheet, 6);
        }
    }
    // Carriers on their lines: a cart on a road, a wagon on rails, a bucket on a cable, a sled, a barge, laden.
    for f in 0..FRAMES {
        let mut sheet = backdrop(5 * 64 + 8, 120);
        for (i, (kind, load)) in [(Kind::Cart, Load::Stone), (Kind::Tram, Load::Lime), (Kind::Rope, Load::Ore), (Kind::Chute, Load::Timber), (Kind::Barge, Load::Grain)].into_iter().enumerate() {
            let x = i as i32 * 64 + 6;
            let tile = lines::line_flat(kind, lines::E | lines::W, false, f);
            for t in 0..4 {
                sheet.stamp(&tile, x + t * 16 - 4, 100 - tile.h + 8);
            }
            let v = lines::vehicle_flat(kind, Dir::East, Some(load), f);
            let lift = if kind == Kind::Rope { 28 } else { 0 };
            sheet.stamp(&v, x + 6 + (f as i32 * 2) % 16, 100 - v.h - lift + if kind == Kind::Barge { 6 } else { 0 });
        }
        save(&dir.join("carriers"), &format!("{f:02}"), &sheet, 6);
    }
}
