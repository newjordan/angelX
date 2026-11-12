// MT2 proof. Ignored unless ANGEL_MT2_PROOF=1. Writes only under .lane-out/mt2.
use crate::rules::caves::world::{
    Cell, Cave, FL_ASH, FL_FUNGUS, FL_GRASS, FL_ICE, FL_WALL, FL_WOOD, FLUID_ACID, FLUID_LAVA,
    FLUID_NONE, FLUID_WATER, REFREEZE_TICKS,
};
use std::time::Instant;

fn entered(seed: u32) -> Field {
    let mut run = crate::rules::Run::new(u64::from(seed), 1, Some("Ash"));
    run.enter_one(1, false);
    run.caves.seed = seed;
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
}

fn stamp(field: &mut Field, x: i32, y: i32, flags: u16, fluid: u8, amount: u8, melt: u8) {
    let mut c = field.cell(x, y).unwrap_or(Cell {
        flags: 0, fire: 0, fluid: 0, amount: 0, integrity: 0, steam: 0,
        height: y as u8, stone: 0, melt: 0, swings: 0,
    });
    c.flags = flags;
    c.fluid = fluid;
    c.amount = amount;
    c.fire = 0;
    c.steam = 0;
    field.record(x, y, &c);
    let k = key(x, y);
    if melt == 0 { field.melts.remove(&k); } else { let end = field.clock + u32::from(melt); field.melts.insert(k, end); }
}

fn clear_room(field: &mut Field, x: i32, y: i32) {
    for dy in -3..=3 {
        for dx in -3..=3 {
            stamp(field, x + dx, y + dy, 0, FLUID_NONE, 0, 0);
        }
    }
}

fn open_pair(field: &Field) -> (i32, i32) {
    let cave = field.view();
    for y in 8..MAP_H - 8 {
        for x in 8..MAP_W - 8 {
            let here = cave.at(x, y).is_some_and(|c| c.open() && !c.wall());
            let north = cave.at(x, y - 1).is_some_and(|c| c.open() && !c.wall());
            if here && north { return (x, y); }
        }
    }
    (40, 40)
}

fn quiet() -> Field {
    let mut field = entered(3);
    still_fluids(&mut field);
    field
}

fn wake_all(cave: &mut Cave) {
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let hot = cave.at(x, y).is_some_and(|c| {
                c.fire > 0 || c.steam > 0 || c.melt > 0 || (c.fluid != 0 && c.amount > 0)
            });
            if hot { cave.wake(x, y); }
        }
    }
}

fn stepped(field: &Field, ticks: u32, fires: &[(i32, i32)]) -> Vec<Cell> {
    let mut cave = field.view_live();
    for &(x, y) in fires {
        if let Some(c) = cave.at_mut(x, y) { c.fire = 40; }
        cave.wake(x, y);
    }
    for _ in 0..ticks {
        wake_all(&mut cave);
        cave.step_sim();
    }
    cave.world.cells.clone()
}

fn paint_of(field: &Field, cells: &[Cell], x: i32, y: i32, tick: u32, flat: bool) -> Img {
    let mut run = crate::rules::Run::new(u64::from(field.seed), 1, Some("Ash"));
    run.enter_one(1, false);
    run.caves = field.clone();
    run.tick = u64::from(tick);
    if let Some(seat) = run.caves.seats.get_mut(&1) {
        seat.x = x as i16;
        seat.y = (y + 4) as i16;
    }
    cave_paint::flat_pools(flat);
    cave_paint::set_paint_over(Some(cells.to_vec()));
    let im = paint_seat(&run, 1);
    cave_paint::set_paint_over(None);
    cave_paint::flat_pools(false);
    im
}

fn aim(x: i32, y: i32) -> (i32, i32) {
    let feet = cave_paint::cell_px(x, y + 4);
    let cam = camera(feet);
    let centre = cave_paint::cell_px(x, y);
    (centre.0 - cam.0, centre.1 - cam.1)
}

fn crop3(frame: &Img, cx: i32, cy: i32, hw: i32, hh: i32) -> Img {
    let mut out = Img::black(hw * 6, hh * 6);
    for y in 0..hh * 2 {
        for x in 0..hw * 2 {
            let c = frame.get(cx - hw + x, cy - hh + y).unwrap_or([0, 0, 0]);
            for dy in 0..3 {
                for dx in 0..3 {
                    out.set(x * 3 + dx, y * 3 + dy, c);
                }
            }
        }
    }
    out
}

fn mt2_mt2_scale3(frame: &Img) -> Img {
    let mut out = Img::black(frame.w * 3, frame.h * 3);
    for y in 0..frame.h {
        for x in 0..frame.w {
            let c = frame.get(x, y).unwrap_or([0, 0, 0]);
            for dy in 0..3 {
                for dx in 0..3 {
                    out.set(x * 3 + dx, y * 3 + dy, c);
                }
            }
        }
    }
    out
}

fn beside(a: &Img, b: &Img, gap: i32) -> Img {
    let h = a.h.max(b.h);
    let mut out = Img::black(a.w + gap + b.w, h);
    for y in 0..a.h {
        for x in 0..a.w {
            out.set(x, y, a.get(x, y).unwrap_or([0, 0, 0]));
        }
    }
    for y in 0..b.h {
        for x in 0..b.w {
            out.set(a.w + gap + x, y, b.get(x, y).unwrap_or([0, 0, 0]));
        }
    }
    out
}

fn row(panels: &[Img]) -> Img {
    let gap = 4;
    let w = panels.iter().map(|p| p.w).sum::<i32>() + gap * (panels.len() as i32 - 1).max(0);
    let h = panels.iter().map(|p| p.h).max().unwrap_or(1);
    let mut sheet = Img::black(w, h);
    let mut ox = 0;
    for panel in panels {
        for y in 0..panel.h {
            for x in 0..panel.w {
                sheet.set(ox + x, y, panel.get(x, y).unwrap_or([0, 0, 0]));
            }
        }
        ox += panel.w + gap;
    }
    sheet
}

fn save(path: &std::path::Path, im: &Img) {
    let mut rgb = Vec::with_capacity((im.w * im.h * 3) as usize);
    for [r, g, b] in im.pixels() {
        rgb.extend_from_slice(&[r, g, b]);
    }
    write_png(path, im.w as u32, im.h as u32, &rgb);
}

fn strip(name: &str, field: &Field, x: i32, y: i32, fires: &[(i32, i32)], out: &std::path::Path) {
    let marks = [0u32, 4, 16, 64];
    let mut before = Vec::new();
    let mut after = Vec::new();
    for mark in marks {
        let cells = stepped(field, mark, fires);
        let b = paint_of(field, &cells, x, y, mark, true);
        let a = paint_of(field, &cells, x, y, mark, false);
        let (px, py) = aim(x, y);
        before.push(crop3(&b, px, py, 22, 18));
        after.push(crop3(&a, px, py, 22, 18));
    }
    let sheet = beside(&row(&before), &row(&after), 8);
    save(&out.join(format!("react-{name}.png")), &sheet);
}

fn gif_frames(name: &str, field: &Field, x: i32, y: i32, fires: &[(i32, i32)], out: &std::path::Path) {
    let dir = out.join(format!("{name}-frames"));
    std::fs::create_dir_all(&dir).unwrap();
    for tick in 0..36u32 {
        let cells = stepped(field, tick, fires);
        let im = paint_of(field, &cells, x, y, tick, false);
        let (px, py) = aim(x, y);
        save(&dir.join(format!("{tick:02}.png")), &crop3(&im, px, py, 28, 22));
    }
}

fn mean(xs: &[u128]) -> u128 {
    if xs.is_empty() { 0 } else { xs.iter().sum::<u128>() / xs.len() as u128 }
}

fn time_walk(flat: bool) -> (u128, u128) {
    let field = quiet();
    let cells = stepped(&field, 0, &[]);
    let mut samples = Vec::new();
    cave_paint::flat_pools(flat);
    cave_paint::set_paint_over(Some(cells));
    let _ = {
        let mut run = crate::rules::Run::new(3, 1, Some("Ash"));
        run.enter_one(1, false);
        run.caves = field.clone();
        paint_seat(&run, 1)
    };
    for i in 0..8u32 {
        let mut run = crate::rules::Run::new(3, 1, Some("Ash"));
        run.enter_one(1, false);
        run.caves = field.clone();
        run.tick = u64::from(i);
        let _ = paint_seat(&run, 1);
    }
    for i in 0..60u32 {
        let mut run = crate::rules::Run::new(3, 1, Some("Ash"));
        run.enter_one(1, false);
        run.caves = field.clone();
        run.tick = u64::from(i);
        if let Some(seat) = run.caves.seats.get_mut(&1) {
            seat.x = 20 + (i % 12) as i16;
            seat.y = 20;
        }
        let t0 = Instant::now();
        let _ = paint_seat(&run, 1);
        samples.push(t0.elapsed().as_micros());
    }
    cave_paint::set_paint_over(None);
    cave_paint::flat_pools(false);
    (mean(&samples), samples.iter().copied().max().unwrap_or(0))
}

#[test]
#[ignore]
fn write_mt2_proof() {
    if std::env::var("ANGEL_MT2_PROOF").ok().as_deref() != Some("1") { return; }
    let out = std::env::temp_dir().join("delve-proof/mt2");
    std::fs::create_dir_all(&out).unwrap();
    let (x, y) = { let f = quiet(); open_pair(&f) };
    let mut lines = vec![format!("open pair {x},{y}")];

    let (bm, bx) = time_walk(true);
    let (am, ax) = time_walk(false);
    lines.push(format!("quiet mid-walk 60 frames us before mean {bm} max {bx} after mean {am} max {ax}"));

    let mut busy = quiet();
    clear_room(&mut busy, x, y);
    for dx in 0..4 {
        for dy in 0..3 {
            stamp(&mut busy, x + dx, y + dy, 0, FLUID_LAVA, 6, 0);
        }
    }
    for dx in 0..4 {
        for dy in 0..3 {
            stamp(&mut busy, x + 6 + dx, y + dy, 0, FLUID_WATER, 5, 0);
        }
    }
    stamp(&mut busy, x, y, FL_WOOD, FLUID_NONE, 0, 0);
    let cells = stepped(&busy, 8, &[(x, y)]);
    let mut run = crate::rules::Run::new(3, 1, Some("Ash"));
    run.enter_one(1, false);
    run.caves = busy.clone();
    run.tick = 8;
    if let Some(seat) = run.caves.seats.get_mut(&1) {
        seat.x = x as i16;
        seat.y = (y + 4) as i16;
    }
    cave_paint::flat_pools(true);
    cave_paint::set_paint_over(Some(cells.clone()));
    let t0 = Instant::now();
    let _ = paint_seat(&run, 1);
    let before_busy = t0.elapsed().as_micros();
    cave_paint::flat_pools(false);
    let t1 = Instant::now();
    let _ = paint_seat(&run, 1);
    let after_busy = t1.elapsed().as_micros();
    cave_paint::set_paint_over(None);
    lines.push(format!("busy frame us before {before_busy} after {after_busy}"));

    let scenes: Vec<(&str, Field, (i32, i32), Vec<(i32, i32)>)> = {
        let mut water_lava = quiet();
        clear_room(&mut water_lava, x, y);
        stamp(&mut water_lava, x, y, 0, FLUID_WATER, 4, 0);
        stamp(&mut water_lava, x, y - 1, 0, FLUID_LAVA, 4, 0);
        let mut lava_flow = quiet();
        clear_room(&mut lava_flow, x, y);
        stamp(&mut lava_flow, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
        stamp(&mut lava_flow, x, y, 0, FLUID_LAVA, 4, 0);
        let mut water_flow = quiet();
        clear_room(&mut water_flow, x, y);
        stamp(&mut water_flow, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
        stamp(&mut water_flow, x, y, 0, FLUID_WATER, 4, 0);
        let mut quench = quiet();
        clear_room(&mut quench, x, y);
        stamp(&mut quench, x, y, FL_WOOD, FLUID_WATER, 4, 0);
        let mut acid = quiet();
        clear_room(&mut acid, x, y);
        stamp(&mut acid, x, y, 0, FLUID_ACID, 4, 0);
        stamp(&mut acid, x, y - 1, FL_WALL, FLUID_NONE, 0, 0);
        let mut acid_wood = quiet();
        clear_room(&mut acid_wood, x, y);
        stamp(&mut acid_wood, x, y, 0, FLUID_ACID, 4, 0);
        stamp(&mut acid_wood, x, y - 1, FL_WOOD, FLUID_NONE, 0, 0);
        let mut fire_wood = quiet();
        clear_room(&mut fire_wood, x, y);
        stamp(&mut fire_wood, x, y, FL_WOOD, FLUID_NONE, 0, 0);
        stamp(&mut fire_wood, x + 1, y, FL_WOOD, FLUID_NONE, 0, 0);
        let mut fire_grass = quiet();
        clear_room(&mut fire_grass, x, y);
        stamp(&mut fire_grass, x, y, FL_GRASS, FLUID_NONE, 0, 0);
        let mut fire_fungus = quiet();
        clear_room(&mut fire_fungus, x, y);
        stamp(&mut fire_fungus, x, y, FL_FUNGUS, FLUID_NONE, 0, 0);
        let mut ash = quiet();
        clear_room(&mut ash, x, y);
        stamp(&mut ash, x, y, FL_ASH, FLUID_NONE, 0, 0);
        let mut melt = quiet();
        clear_room(&mut melt, x, y);
        stamp(&mut melt, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
        stamp(&mut melt, x, y - 1, 0, FLUID_WATER, 4, REFREEZE_TICKS);
        let mut ice = quiet();
        clear_room(&mut ice, x, y);
        stamp(&mut ice, x, y - 2, FL_WALL, FLUID_NONE, 0, 0);
        stamp(&mut ice, x, y, FL_ICE, FLUID_NONE, 0, 0);
        stamp(&mut ice, x, y - 1, 0, FLUID_LAVA, 4, 0);
        vec![
            ("water-lava", water_lava, (x, y - 1), vec![]),
            ("lava-flow", lava_flow, (x, y), vec![]),
            ("water-flow", water_flow, (x, y), vec![]),
            ("water-quench", quench, (x, y), vec![(x, y)]),
            ("acid-rock", acid, (x, y), vec![]),
            ("acid-wood", acid_wood, (x, y), vec![]),
            ("fire-wood", fire_wood, (x, y), vec![(x, y)]),
            ("fire-grass", fire_grass, (x, y), vec![(x, y)]),
            ("fire-fungus", fire_fungus, (x, y), vec![(x, y)]),
            ("ash", ash, (x, y), vec![]),
            ("melt-refreeze", melt, (x, y - 1), vec![]),
            ("lava-ice", ice, (x, y), vec![]),
        ]
    };
    for (name, field, at, fires) in &scenes {
        strip(name, field, at.0, at.1, fires, &out);
        lines.push(format!("react-{name}.png"));
    }

    let mut pools = quiet();
    for dy in -3..=6 {
        for dx in -12..=12 {
            stamp(&mut pools, x + dx, y + dy, 0, FLUID_NONE, 0, 0);
        }
    }
    for dy in -1..=1 {
        for dx in -9..=-5 {
            stamp(&mut pools, x + dx, y + dy, 0, FLUID_LAVA, 6, 0);
        }
        stamp(&mut pools, x - 4, y + dy, 0, FLUID_WATER, 6, 0);
        for dx in -3..=-1 {
            stamp(&mut pools, x + dx, y + dy, 0, FLUID_WATER, 6, 0);
        }
        for dx in 2..=6 {
            stamp(&mut pools, x + dx, y + dy, 0, FLUID_ACID, 6, 0);
        }
    }
    for dx in -3..=-1 {
        stamp(&mut pools, x + dx, y - 2, FL_WALL, FLUID_NONE, 0, 0);
    }
    let cells = stepped(&pools, 0, &[]);
    let shot = paint_of(&pools, &cells, x, y + 5, 8, false);
    let (cx, cy) = {
        let feet = cave_paint::cell_px(x, y + 5);
        let cam = camera(feet);
        let centre = cave_paint::cell_px(x, y);
        (centre.0 - cam.0, centre.1 - cam.1)
    };
    let mut raw = Img::black(176, 56);
    for yy in 0..56 {
        for xx in 0..176 {
            raw.set(xx, yy, shot.get(cx - 88 + xx, cy - 28 + yy).unwrap_or([0, 0, 0]));
        }
    }
    let big = crop3(&shot, cx, cy, 88, 28);
    save(&out.join("pools.png"), &beside(&raw, &big, 6));
    lines.push("pools.png".into());

    let mut fire = quiet();
    clear_room(&mut fire, x, y);
    for i in 0..4 { stamp(&mut fire, x + i, y, FL_WOOD, FLUID_NONE, 0, 0); }
    gif_frames("fire", &fire, x, y, &[(x, y)], &out);
    let mut quench = quiet();
    clear_room(&mut quench, x, y);
    stamp(&mut quench, x, y, FL_WOOD, FLUID_NONE, 0, 0);
    stamp(&mut quench, x, y + 1, 0, FLUID_WATER, 6, 0);
    gif_frames("quench", &quench, x, y, &[(x, y)], &out);
    let mut meet = quiet();
    clear_room(&mut meet, x, y);
    for i in 0..3 { stamp(&mut meet, x + i, y, 0, FLUID_LAVA, 6, 0); }
    stamp(&mut meet, x + 1, y + 1, 0, FLUID_WATER, 6, 0);
    gif_frames("lava-water", &meet, x, y, &[], &out);
    lines.push("gif frames 36".into());
    std::fs::write(out.join("timings.txt"), lines.join("\n")).unwrap();
}
