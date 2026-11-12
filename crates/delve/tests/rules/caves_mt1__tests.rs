//! Awake-set fluid step must match the whole-map path, cell for cell.
use super::*;
use crate::render::overworld::cave_paint::{camera, cell_px};
use crate::render::overworld::cave_seat::paint_seat;
use crate::render::overworld::ink::{Img, Rgb};
use crate::rules::caves::world::{
    Cell, FL_ASH, FL_FUNGUS, FL_GRASS, FL_ICE, FL_WALL, FL_WOOD, FLUID_ACID, FLUID_LAVA, FLUID_NONE,
    FLUID_WATER, MAP_H, MAP_W, REFREEZE_TICKS,
};
use std::time::Instant;

fn entered(seed: u32) -> Field {
    let mut run = crate::rules::Run::new(u64::from(seed), 1, Some("Ash"));
    run.enter_one(1, false);
    run.caves.seed = seed;
    run.caves.diet.ready = false;
    run.caves
}

fn still_fluids(field: &mut Field) {
    let cave = field.view();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let Some(c) = cave.at(x, y) else { continue };
            if c.fluid == FLUID_NONE && c.amount == 0 {
                continue;
            }
            let mut d = *c;
            d.fluid = FLUID_NONE;
            d.amount = 0;
            field.record(x, y, &d);
        }
    }
    field.diet.ready = false;
    field.diet.stirred.clear();
}

fn clear_room(field: &mut Field, x: i32, y: i32) {
    for dy in -2..=2 {
        for dx in -2..=2 {
            stamp(field, x + dx, y + dy, 0, FLUID_NONE, 0, 0);
        }
    }
}

fn stamp(field: &mut Field, x: i32, y: i32, flags: u16, fluid: u8, amount: u8, melt: u8) {
    let mut c = field.cell(x, y).unwrap_or(Cell {
        flags: 0,
        fire: 0,
        fluid: 0,
        amount: 0,
        integrity: 0,
        steam: 0,
        height: y as u8,
        stone: 0,
        melt: 0,
        swings: 0,
    });
    c.flags = flags;
    c.fluid = fluid;
    c.amount = amount;
    c.fire = 0;
    c.steam = 0;
    field.record(x, y, &c);
    let k = key(x, y);
    if melt == 0 {
        field.melts.remove(&k);
    } else {
        field.melts.insert(k, field.clock.saturating_add(u32::from(melt)));
    }
    field.stir(x, y);
}

fn open_pair(field: &Field) -> (i32, i32) {
    let cave = field.view();
    for y in 8..MAP_H - 8 {
        for x in 8..MAP_W - 8 {
            let here = cave.at(x, y).is_some_and(|c| c.open() && !c.wall());
            let north = cave.at(x, y - 1).is_some_and(|c| c.open() && !c.wall());
            if here && north {
                return (x, y);
            }
        }
    }
    (40, 40)
}

fn quiet_cave() -> Field {
    let mut field = entered(3);
    still_fluids(&mut field);
    field
}

fn contact_cave() -> Field {
    let mut field = quiet_cave();
    let (x, y) = open_pair(&field);
    stamp(&mut field, x, y, 0, FLUID_WATER, 4, 0);
    stamp(&mut field, x, y - 1, 0, FLUID_LAVA, 4, 0);
    field.diet.ready = false;
    field
}

fn fire_cave() -> Field {
    let mut field = quiet_cave();
    let (x, y) = open_pair(&field);
    for i in 0..6 {
        stamp(&mut field, x + i, y, FL_WOOD, FLUID_NONE, 0, 0);
    }
    field.diet.ready = false;
    field
}

fn cells_match(a: &Field, b: &Field) -> bool {
    let left = a.view_live();
    let right = b.view_live();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if left.at(x, y) != right.at(x, y) {
                return false;
            }
        }
    }
    a.changes == b.changes && a.melts == b.melts && a.fires == b.fires && a.steams == b.steams && a.acids == b.acids && a.clock == b.clock
}

#[test]
fn awake_path_matches_the_scan_for_300_ticks() {
    for (name, field) in [("quiet", quiet_cave()), ("contact", contact_cave()), ("fire", fire_cave())] {
        let mut old = field.clone();
        let mut young = field.clone();
        old.diet.ready = false;
        young.diet.ready = false;
        for tick in 0..300 {
            old.step_fluids_scan();
            young.step_fluids();
            assert!(cells_match(&old, &young), "{name} diverged at tick {tick}");
        }
    }
}

#[test]
fn react_touch_matches_step_while_fire_spreads() {
    let mut world = world::Cave::generate(9).world;
    let i = (40 * MAP_W + 40) as usize;
    for n in 0..8 {
        let j = i + n;
        world.cells[j].flags |= FL_WOOD;
        world.cells[j].flags &= !FL_WALL;
        world.awake[j] = true;
    }
    world.cells[i].fire = 8;
    world.awake[i] = true;
    for tick in 0..64 {
        let mut stepped = world.clone();
        stepped.step();
        let touch: Vec<usize> = world
            .awake
            .iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .map(|(n, _)| n)
            .collect();
        let sealed: Vec<usize> = world
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.sealed())
            .map(|(n, _)| n)
            .collect();
        let steps = world.steps;
        let got = world::react_touch(MAP_W, MAP_H, steps, |n| world.cells[n], &touch, &sealed);
        let mut applied = world.cells.clone();
        for (n, cell) in got {
            applied[n] = cell;
        }
        assert_eq!(applied, stepped.cells, "fire world diverged at tick {tick}");
        world = stepped;
    }
}

fn mean_max(samples: &[u128]) -> (u128, u128) {
    let n = samples.len() as u128;
    let sum: u128 = samples.iter().copied().sum();
    let mean = if n == 0 { 0 } else { sum / n };
    let max = samples.iter().copied().max().unwrap_or(0);
    (mean, max)
}

fn time_path(mut field: Field, scan: bool) -> (u128, u128, usize) {
    field.diet.ready = false;
    let mut samples = Vec::with_capacity(300);
    for _ in 0..300 {
        let started = Instant::now();
        if scan {
            field.step_fluids_scan();
        } else {
            field.step_fluids();
        }
        samples.push(started.elapsed().as_micros());
    }
    let (mean, max) = mean_max(&samples);
    (mean, max, field.changes.len())
}

fn save_png(path: &std::path::Path, im: &Img) {
    let mut buf = image::RgbaImage::new(im.w.max(1) as u32, im.h.max(1) as u32);
    for y in 0..im.h {
        for x in 0..im.w {
            let p = im.get(x, y).unwrap_or([0, 0, 0]);
            buf.put_pixel(x as u32, y as u32, image::Rgba([p[0], p[1], p[2], 255]));
        }
    }
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    buf.save(path).expect("png");
}

fn crop_3x(frame: &Img, cx: i32, cy: i32) -> Img {
    let cells = 5;
    let side = cells * world::CELL;
    let x0 = cx - side / 2;
    let y0 = cy - side / 2;
    let mut out = Img::black(side * 3, side * 3);
    for y in 0..side {
        for x in 0..side {
            let c = frame.get(x0 + x, y0 + y).unwrap_or([0, 0, 0]);
            for dy in 0..3 {
                for dx in 0..3 {
                    out.set(x * 3 + dx, y * 3 + dy, c);
                }
            }
        }
    }
    out
}

fn paint_at(field: &Field, x: i32, y: i32) -> Img {
    let mut run = crate::rules::Run::new(u64::from(field.seed), 1, Some("Ash"));
    run.caves = field.clone();
    if let Some(seat) = run.caves.seats.get_mut(&1) {
        seat.x = x as i16;
        seat.y = (y + 5) as i16;
    }
    paint_seat(&run, 1)
}

fn frame_px(x: i32, y: i32) -> (i32, i32) {
    let centre = cell_px(x, y);
    let cam = camera(centre);
    (centre.0 - cam.0 + world::CELL / 2, centre.1 - cam.1 + world::CELL / 2)
}

fn strip(name: &str, mut field: Field, x: i32, y: i32, out: &std::path::Path) {
    let marks = [0u32, 4, 16, 64];
    let mut panels = Vec::new();
    let mut tick = 0u32;
    for mark in marks {
        while tick < mark {
            field.step_fluids();
            tick += 1;
        }
        let painted = paint_at(&field, x, y);
        let (px, py) = frame_px(x, y);
        panels.push(crop_3x(&painted, px, py));
    }
    let gap = 4;
    let w = panels.iter().map(|p| p.w).sum::<i32>() + gap * (panels.len() as i32 - 1);
    let h = panels.iter().map(|p| p.h).max().unwrap_or(1);
    let mut sheet = Img::black(w, h);
    let mut ox = 0;
    for panel in &panels {
        for y in 0..panel.h {
            for x in 0..panel.w {
                let c: Rgb = panel.get(x, y).unwrap_or([0, 0, 0]);
                sheet.set(ox + x, y, c);
            }
        }
        ox += panel.w + gap;
    }
    save_png(&out.join(format!("react-{name}.png")), &sheet);
}

#[test]
#[ignore]
fn write_mt1_proofs() {
    if std::env::var("ANGEL_MT1_PROOF").ok().as_deref() != Some("1") {
        return;
    }
    let out = std::env::temp_dir().join("delve-proof/mt1");
    std::fs::create_dir_all(&out).unwrap();
    let mut lines = Vec::new();
    lines.push("timings microseconds mean/max over 300 ticks, change-log entries after".to_string());
    for (name, field) in [("quiet", quiet_cave()), ("contact", contact_cave()), ("fire", fire_cave())] {
        let (bm, bx, bn) = time_path(field.clone(), true);
        let (am, ax, an) = time_path(field, false);
        lines.push(format!(
            "{name} before mean {bm} max {bx} changes {bn} after mean {am} max {ax} changes {an}"
        ));
    }
    std::fs::write(out.join("timings.txt"), lines.join("\n")).unwrap();

    let (x, y) = {
        let field = quiet_cave();
        open_pair(&field)
    };
    let mut water_lava = quiet_cave();
    clear_room(&mut water_lava, x, y);
    stamp(&mut water_lava, x, y, 0, FLUID_WATER, 4, 0);
    stamp(&mut water_lava, x, y - 1, 0, FLUID_LAVA, 4, 0);
    strip("water-lava", water_lava, x, y - 1, &out);

    let mut lava_flow = quiet_cave();
    clear_room(&mut lava_flow, x, y);
    stamp(&mut lava_flow, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
    stamp(&mut lava_flow, x, y, 0, FLUID_LAVA, 4, 0);
    strip("lava-flow", lava_flow, x, y, &out);

    let mut water_flow = quiet_cave();
    clear_room(&mut water_flow, x, y);
    stamp(&mut water_flow, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
    stamp(&mut water_flow, x, y, 0, FLUID_WATER, 4, 0);
    strip("water-flow", water_flow, x, y, &out);

    let mut quench = quiet_cave();
    clear_room(&mut quench, x, y);
    stamp(&mut quench, x, y, FL_WOOD, FLUID_WATER, 4, 0);
    strip("water-quench", quench, x, y, &out);

    let mut acid = quiet_cave();
    clear_room(&mut acid, x, y);
    stamp(&mut acid, x, y, 0, FLUID_ACID, 4, 0);
    stamp(&mut acid, x, y - 1, FL_WALL, FLUID_NONE, 0, 0);
    strip("acid-rock", acid, x, y, &out);

    let mut acid_wood = quiet_cave();
    clear_room(&mut acid_wood, x, y);
    stamp(&mut acid_wood, x, y, 0, FLUID_ACID, 4, 0);
    stamp(&mut acid_wood, x, y - 1, FL_WOOD, FLUID_NONE, 0, 0);
    strip("acid-wood", acid_wood, x, y, &out);

    let mut fire_wood = quiet_cave();
    clear_room(&mut fire_wood, x, y);
    stamp(&mut fire_wood, x, y, FL_WOOD, FLUID_NONE, 0, 0);
    stamp(&mut fire_wood, x + 1, y, FL_WOOD, FLUID_NONE, 0, 0);
    strip("fire-wood", fire_wood, x, y, &out);

    let mut fire_grass = quiet_cave();
    clear_room(&mut fire_grass, x, y);
    stamp(&mut fire_grass, x, y, FL_GRASS, FLUID_NONE, 0, 0);
    strip("fire-grass", fire_grass, x, y, &out);

    let mut fire_fungus = quiet_cave();
    clear_room(&mut fire_fungus, x, y);
    stamp(&mut fire_fungus, x, y, FL_FUNGUS, FLUID_NONE, 0, 0);
    strip("fire-fungus", fire_fungus, x, y, &out);

    let mut ash = quiet_cave();
    clear_room(&mut ash, x, y);
    stamp(&mut ash, x, y, FL_ASH, FLUID_NONE, 0, 0);
    strip("ash", ash, x, y, &out);

    let mut melt = quiet_cave();
    clear_room(&mut melt, x, y);
    stamp(&mut melt, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
    stamp(&mut melt, x, y - 1, 0, FLUID_WATER, 4, REFREEZE_TICKS);
    strip("melt-refreeze", melt, x, y - 1, &out);

    let mut ice = quiet_cave();
    clear_room(&mut ice, x, y);
    stamp(&mut ice, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
    stamp(&mut ice, x, y, FL_ICE, FLUID_NONE, 0, 0);
    stamp(&mut ice, x, y - 1, 0, FLUID_LAVA, 4, 0);
    strip("lava-ice", ice, x, y, &out);
}
