//! Host-path reactions must survive the view rebuild. Fire, steam, acid and
//! melt live as tick entries on the field, not as a countdown rewritten each tick.
use super::*;
use crate::rules::caves::world::{
    Cell, FL_FUNGUS, FL_GRASS, FL_ICE, FL_WALL, FL_WOOD, FIRE_LIFE, FLUID_ACID, FLUID_LAVA,
    FLUID_NONE, FLUID_WATER, LAVA_STEAM, MAP_H, MAP_W, QUENCH_STEAM, REFREEZE_TICKS,
};

fn entered(seed: u32) -> Field {
    let mut run = crate::rules::Run::new(u64::from(seed), 1, Some("Ash"));
    run.enter_one(1, false);
    run.caves.seed = seed;
    still_fluids(&mut run.caves);
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

fn stamp(field: &mut Field, x: i32, y: i32, flags: u16, fluid: u8, amount: u8) {
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
}

fn light(field: &mut Field, x: i32, y: i32) {
    field.fires.insert(key(x, y), field.clock);
    field.stir(x, y);
    field.diet.ready = false;
}

fn step(field: &mut Field, n: u32) {
    for _ in 0..n {
        field.step_fluids();
    }
}

#[test]
fn a_wood_line_burns_to_ash_on_the_host_path() {
    let mut field = entered(7);
    let (x, y) = (40, 40);
    for i in 0..6 {
        stamp(&mut field, x + i, y, FL_WOOD, FLUID_NONE, 0);
    }
    light(&mut field, x, y);
    step(&mut field, 50);
    for i in 0..6 {
        let cell = field.view_live().at(x + i, y).copied().unwrap();
        assert!(cell.ash(), "cell {i} should be ash, flags {:#x}", cell.flags);
        assert_eq!(cell.fire, 0, "cell {i} still burning");
    }
}

#[test]
fn water_on_a_flame_leaves_steam_for_its_full_count() {
    let mut field = entered(7);
    let (x, y) = (40, 40);
    stamp(&mut field, x, y, FL_WOOD, FLUID_WATER, 2);
    light(&mut field, x, y);
    field.step_fluids();
    let quenched = field.view_live().at(x, y).copied().unwrap();
    assert_eq!(quenched.fire, 0);
    assert_eq!(quenched.steam, QUENCH_STEAM);
    let end = field.steams[&key(x, y)];
    step(&mut field, u32::from(QUENCH_STEAM) - 1);
    let hanging = field.view_live().at(x, y).copied().unwrap();
    assert_eq!(hanging.steam, 1);
    assert_eq!(field.steams.get(&key(x, y)), Some(&end));
    field.step_fluids();
    assert_eq!(field.view_live().at(x, y).unwrap().steam, 0);
    assert!(!field.steams.contains_key(&key(x, y)));
}

#[test]
fn acid_opens_a_12_integrity_wall_in_about_12_ticks() {
    let mut field = entered(7);
    let cave = field.view();
    let mut wall = None;
    for y in 61..=62 {
        for x in 110..120 {
            if cave.at(x, y).is_some_and(|c| c.wall() && c.integrity == 12) {
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = x + dx;
                    let ny = y + dy;
                    if cave.at(nx, ny).is_some_and(|c| c.open() && !c.wall()) {
                        wall = Some((x, y, nx, ny));
                    }
                }
            }
        }
    }
    let (wx, wy, ax, ay) = wall.expect("a 12-integrity wall with an open neighbour");
    stamp(&mut field, ax, ay, 0, FLUID_ACID, 4);
    step(&mut field, 12);
    let opened = field.view_live().at(wx, wy).copied().unwrap();
    assert!(!opened.wall(), "wall still up after 12 acid ticks, integrity {}", opened.integrity);
}

#[test]
fn a_spell_melted_ice_cell_refreezes_after_refreeze_ticks() {
    let mut field = entered(7);
    let (x, y) = (40, 30);
    stamp(&mut field, x, y - 1, FL_WALL, FLUID_NONE, 0);
    stamp(&mut field, x, y, FL_ICE, FLUID_NONE, 0);
    field.edit(x, y, |cave| cave.melt_ice(x, y));
    assert!(field.view_live().at(x, y).unwrap().fluid == FLUID_WATER);
    assert!(field.melts.contains_key(&key(x, y)));
    step(&mut field, u32::from(REFREEZE_TICKS) - 1);
    assert!(
        field.view_live().at(x, y).unwrap().fluid == FLUID_WATER,
        "refroze early"
    );
    field.step_fluids();
    let back = field.view_live().at(x, y).copied().unwrap();
    assert!(back.ice(), "flags {:#x}", back.flags);
    assert_eq!(back.fluid, FLUID_NONE);
    assert!(!field.melts.contains_key(&key(x, y)));
}

#[test]
fn lava_lights_wood_grass_and_fungus() {
    let mut field = entered(11);
    let (x, y) = (50, 40);
    stamp(&mut field, x, y, 0, FLUID_LAVA, 4);
    stamp(&mut field, x + 1, y, FL_WOOD, FLUID_NONE, 0);
    stamp(&mut field, x, y + 1, FL_GRASS, FLUID_NONE, 0);
    stamp(&mut field, x - 1, y, FL_FUNGUS, FLUID_NONE, 0);
    field.step_fluids();
    for (dx, dy) in [(1, 0), (0, 1), (-1, 0)] {
        let cell = field.view_live().at(x + dx, y + dy).copied().unwrap();
        assert_eq!(cell.fire, FIRE_LIFE, "at {dx},{dy}");
    }
}

#[test]
fn lava_beside_ice_ends_as_a_steam_cloud_and_a_cooled_plug() {
    let mut field = entered(11);
    let (x, y) = (50, 40);
    stamp(&mut field, x, y, 0, FLUID_LAVA, 4);
    stamp(&mut field, x + 1, y, FL_ICE, FLUID_NONE, 0);
    field.step_fluids();
    let lava = field.view_live().at(x, y).copied().unwrap();
    let ice = field.view_live().at(x + 1, y).copied().unwrap();
    assert_eq!(lava.fluid, FLUID_NONE);
    assert_eq!(ice.fluid, FLUID_NONE);
    assert!(!ice.ice());
    assert!(lava.hard() && ice.hard());
    assert_eq!(lava.steam, LAVA_STEAM);
    assert_eq!(ice.steam, LAVA_STEAM);
}

#[test]
fn fungus_burns_to_ash() {
    let mut field = entered(11);
    let (x, y) = (50, 40);
    stamp(&mut field, x, y, FL_FUNGUS, FLUID_NONE, 0);
    light(&mut field, x, y);
    step(&mut field, u32::from(FIRE_LIFE) + 2);
    let cell = field.view_live().at(x, y).copied().unwrap();
    assert!(cell.ash(), "flags {:#x}", cell.flags);
    assert!(!cell.fungus());
}

#[test]
fn a_fire_costs_two_wire_writes() {
    let mut field = entered(7);
    let (x, y) = (40, 40);
    stamp(&mut field, x, y, FL_WOOD, FLUID_NONE, 0);
    light(&mut field, x, y);
    let caught = field.fires[&key(x, y)];
    let mut rewrites = 0;
    let mut prev = field.fires.clone();
    for _ in 0..u32::from(FIRE_LIFE) - 1 {
        field.step_fluids();
        if field.fires != prev {
            rewrites += 1;
            prev = field.fires.clone();
        }
    }
    assert_eq!(rewrites, 0, "countdown rewrote the fire entry");
    assert_eq!(field.fires.get(&key(x, y)), Some(&caught));
    field.step_fluids();
    assert!(!field.fires.contains_key(&key(x, y)));
    assert_eq!(rewrites, 0);
}

#[test]
fn a_guest_decodes_hot_state_with_the_cave_field() {
    let mut field = entered(7);
    let (x, y) = (40, 40);
    stamp(&mut field, x, y, FL_WOOD, FLUID_NONE, 0);
    light(&mut field, x, y);
    let mut seen = Vec::new();
    for _ in 0..5 {
        field.step_fluids();
        let raw = serde_json::to_string(&field).unwrap();
        let mut back: Field = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.fires, field.fires);
        assert_eq!(back.steams, field.steams);
        assert_eq!(back.acids, field.acids);
        assert_eq!(back.melts, field.melts);
        back.clock = field.clock;
        assert_eq!(
            back.view_live().at(x, y).unwrap().fire,
            field.view_live().at(x, y).unwrap().fire
        );
        seen.push(field.fires.clone());
    }
    assert!(seen.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn lava_and_fungus_match_on_both_paths() {
    let mut awake = entered(11);
    let mut scan = awake.clone();
    let (x, y) = (50, 40);
    for field in [&mut awake, &mut scan] {
        stamp(field, x, y, 0, FLUID_LAVA, 4);
        stamp(field, x + 1, y, FL_WOOD, FLUID_NONE, 0);
        stamp(field, x, y + 1, FL_ICE, FLUID_NONE, 0);
        stamp(field, x - 1, y, FL_FUNGUS, FLUID_NONE, 0);
        light(field, x - 1, y);
    }
    for _ in 0..16 {
        awake.diet.ready = false;
        scan.diet.ready = false;
        awake.step_fluids();
        scan.step_fluids_scan();
        assert!(paths_match(&awake, &scan));
    }
}

fn paths_match(a: &Field, b: &Field) -> bool {
    let left = a.view_live();
    let right = b.view_live();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            if left.at(x, y) != right.at(x, y) {
                return false;
            }
        }
    }
    a.changes == b.changes
        && a.melts == b.melts
        && a.fires == b.fires
        && a.steams == b.steams
        && a.acids == b.acids
        && a.clock == b.clock
}


#[test]
#[ignore]
fn write_mt3_proofs() {
    if std::env::var("ANGEL_MT3_PROOF").ok().as_deref() != Some("1") {
        return;
    }
    let out = std::env::temp_dir().join("delve-proof/mt3");
    std::fs::create_dir_all(&out).unwrap();
    proof_scenes(&out);
    proof_wire(&out);
    proof_timings(&out);
}

fn wall12(field: &Field) -> (i32, i32) {
    let cave = field.view();
    for y in 61..=62 {
        for x in 110..120 {
            if cave.at(x, y).is_some_and(|c| c.wall() && c.integrity == 12) {
                return (x, y);
            }
        }
    }
    panic!("no 12-integrity wall");
}

fn proof_scenes(out: &std::path::Path) {
    let (x, y) = (40, 40);
    let wood = |flags| {
        let mut f = entered(9);
        stamp(&mut f, x, y, flags, FLUID_NONE, 0);
        light(&mut f, x, y);
        f
    };
    strip("fire-wood", wood(FL_WOOD), x, y, out);
    strip("fire-grass", wood(FL_GRASS), x, y, out);
    strip("fire-fungus", wood(FL_FUNGUS), x, y, out);
    let mut quench = entered(9);
    stamp(&mut quench, x, y, FL_WOOD, FLUID_NONE, 0);
    light(&mut quench, x, y);
    stamp(&mut quench, x, y, FL_WOOD, FLUID_WATER, 2);
    light(&mut quench, x, y);
    strip("water-quench", quench, x, y, out);
    let mut acid = entered(11);
    let (ax, ay) = wall12(&acid);
    stamp(&mut acid, ax, ay + 1, 0, FLUID_ACID, 2);
    strip("acid-rock", acid, ax, ay, out);
    let mut acid_wood = entered(11);
    stamp(&mut acid_wood, x, y, FL_WOOD, FLUID_NONE, 0);
    stamp(&mut acid_wood, x, y + 1, 0, FLUID_ACID, 2);
    strip("acid-wood", acid_wood, x, y, out);
    let mut ice = entered(7);
    stamp(&mut ice, x, y - 1, FL_WALL, FLUID_NONE, 0);
    stamp(&mut ice, x, y, FL_ICE, FLUID_NONE, 0);
    stamp(&mut ice, x + 1, y, 0, FLUID_LAVA, 3);
    strip("lava-ice", ice, x, y, out);
    let mut melt = entered(8);
    stamp(&mut melt, x, y - 1, FL_WALL, FLUID_NONE, 0);
    stamp(&mut melt, x, y, FL_ICE, FLUID_NONE, 0);
    melt.edit(x, y, |cave| cave.melt_ice(x, y));
    strip("melt-refreeze", melt, x, y, out);
    let mut ash = entered(9);
    for i in 0..6 {
        stamp(&mut ash, x + i, y, FL_WOOD, FLUID_NONE, 0);
    }
    light(&mut ash, x, y);
    strip("ash", ash, x + 2, y, out);
    let mut water = entered(3);
    stamp(&mut water, x, y - 1, FL_WALL, FLUID_NONE, 0);
    stamp(&mut water, x, y, 0, FLUID_WATER, 4);
    strip("water-flow", water, x, y, out);
    let mut lava = entered(4);
    stamp(&mut lava, x, y + 1, FL_WALL, FLUID_NONE, 0);
    stamp(&mut lava, x, y, 0, FLUID_LAVA, 4);
    strip("lava-flow", lava, x, y, out);
    let mut contact = entered(5);
    stamp(&mut contact, x, y, 0, FLUID_WATER, 3);
    stamp(&mut contact, x + 1, y, 0, FLUID_LAVA, 3);
    strip("water-lava", contact, x, y, out);
}

fn proof_wire(out: &std::path::Path) {
    let mut field = entered(9);
    let (x, y) = (40, 40);
    for i in 0..6 {
        stamp(&mut field, x + i, y, FL_WOOD, FLUID_NONE, 0);
    }
    light(&mut field, x, y);
    let mut lines = Vec::new();
    for tick in 0..5 {
        let raw = serde_json::to_string(&field).unwrap();
        let mut back: Field = serde_json::from_str(&raw).unwrap();
        back.clock = field.clock;
        let host = field.view_live().at(x, y).unwrap().fire;
        let guest = back.view_live().at(x, y).unwrap().fire;
        lines.push(format!(
            "tick {tick} clock {} fires {:?} guest_fires {:?} host_fire {host} guest_fire {guest} equal {}",
            field.clock, field.fires, back.fires, field.fires == back.fires && host == guest
        ));
        field.step_fluids();
    }
    std::fs::write(out.join("wire.txt"), lines.join("\n") + "\n").unwrap();
}

fn proof_timings(out: &std::path::Path) {
    let quiet = || entered(1);
    let contact = || {
        let mut f = entered(2);
        stamp(&mut f, 40, 40, 0, FLUID_WATER, 3);
        stamp(&mut f, 41, 40, 0, FLUID_LAVA, 3);
        f
    };
    let cold = || {
        let mut f = entered(3);
        for i in 0..6 {
            stamp(&mut f, 40 + i, 40, FL_WOOD, FLUID_NONE, 0);
        }
        f
    };
    let hot = || {
        let mut f = entered(3);
        for i in 0..6 {
            stamp(&mut f, 40 + i, 40, FL_WOOD, FLUID_NONE, 0);
        }
        light(&mut f, 40, 40);
        f
    };
    let mut lines = vec!["step_fluids microseconds mean/max over 300 ticks. before is the scan path, after is the awake path.".to_string()];
    for (name, make) in [
        ("quiet", quiet as fn() -> Field),
        ("contact", contact),
        ("cold-line", cold),
        ("fire-line", hot),
    ] {
        let (bmean, bmax) = time_path(make(), true);
        let (amean, amax) = time_path(make(), false);
        lines.push(format!("{name} before mean {bmean} max {bmax} after mean {amean} max {amax}"));
    }
    let mut burning = hot();
    let mut pubs = 0u32;
    let mut prev = burning.fingerprint();
    let mut bytes = 0u64;
    let mut ticks = 0u32;
    for _ in 0..40 {
        burning.step_fluids();
        ticks += 1;
        let now = burning.fingerprint();
        if now != prev {
            pubs += 1;
            bytes += serde_json::to_string(&burning).unwrap().len() as u64;
            prev = now;
        }
    }
    lines.push(format!(
        "fire-line fingerprint republishes {pubs} in {ticks} ticks; caves-field json bytes summed on those republishes {bytes}"
    ));
    std::fs::write(out.join("timings.txt"), lines.join("\n") + "\n").unwrap();
}

fn time_path(mut field: Field, scan: bool) -> (u128, u128) {
    let mut samples = Vec::new();
    for _ in 0..300 {
        let t0 = std::time::Instant::now();
        if scan {
            field.step_fluids_scan();
        } else {
            field.step_fluids();
        }
        samples.push(t0.elapsed().as_micros());
    }
    let sum: u128 = samples.iter().sum();
    (sum / samples.len() as u128, samples.into_iter().max().unwrap_or(0))
}

fn strip(name: &str, mut field: Field, x: i32, y: i32, out: &std::path::Path) {
    use crate::render::overworld::ink::Img;
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
        for py in 0..panel.h {
            for px in 0..panel.w {
                let c = panel.get(px, py).unwrap_or([0, 0, 0]);
                sheet.set(ox + px, py, c);
            }
        }
        ox += panel.w + gap;
    }
    save_png(&out.join(format!("react-{name}.png")), &sheet);
}

fn save_png(path: &std::path::Path, im: &crate::render::overworld::ink::Img) {
    let mut buf = image::RgbaImage::new(im.w.max(1) as u32, im.h.max(1) as u32);
    for y in 0..im.h {
        for x in 0..im.w {
            let p = im.get(x, y).unwrap_or([0, 0, 0]);
            buf.put_pixel(x as u32, y as u32, image::Rgba([p[0], p[1], p[2], 255]));
        }
    }
    buf.save(path).expect("png");
}

fn paint_at(field: &Field, x: i32, y: i32) -> crate::render::overworld::ink::Img {
    use crate::render::overworld::cave_seat::paint_seat;
    let mut run = crate::rules::Run::new(u64::from(field.seed), 1, Some("Ash"));
    run.caves = field.clone();
    if let Some(seat) = run.caves.seats.get_mut(&1) {
        seat.x = x as i16;
        seat.y = (y + 1) as i16;
    }
    paint_seat(&run, 1)
}

fn frame_px(x: i32, y: i32) -> (i32, i32) {
    use crate::render::overworld::cave_paint::{camera, cell_px};
    let centre = cell_px(x, y);
    let cam = camera(cell_px(x, y + 1));
    (centre.0 - cam.0 + world::CELL / 2, centre.1 - cam.1 + world::CELL / 2)
}

fn crop_3x(frame: &crate::render::overworld::ink::Img, cx: i32, cy: i32) -> crate::render::overworld::ink::Img {
    use crate::render::overworld::ink::Img;
    let side = 28;
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
