//! The blank-slate surface on screen (`crate::world::plan`): every line it
//! says to a fresh knight and a long-played one (`probe_surface_lines`), and
//! the whole loop's frames (`write_tabula_loop`): the spawn, the Keep, each
//! camp, the Delve's gate, down into its courtyard and back up. Both are
//! ignored and need the plan standing (`ANGEL_REALM_PLAN=tabula`).
use super::*;
use crate::rules::{Input, Run};
use std::collections::BTreeMap;

fn lines_of(run: &Run, focus: u32) -> Vec<String> {
    let (_, said) = crate::ink::said::read(|| frame_for(run, 384, 224, Some(focus)));
    let mut v: Vec<String> = said.lines.into_iter().map(|l| l.text).collect();
    v.sort();
    v.dedup();
    v
}

fn settled(tile: (i32, i32), veteran: bool) -> Run {
    let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
    if veteran {
        run.home.deepest = 4;
        run.home.residents.insert("maud".into());
    }
    run.walk_kingdom(Some(tile));
    for _ in 0..90 {
        run.step(&BTreeMap::new());
    }
    run
}

#[test]
#[ignore = "prints what the surface says; needs ANGEL_REALM_PLAN=tabula"]
fn probe_surface_lines() {
    use crate::world::plan::tabula as t;
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let spots = [
        ("keep stand", t::KEEP_STAND),
        ("delve stand", t::delve_stand()),
        ("camp0 stand", t::camp_stand(t::CAMPS[0])),
        ("hillside", (100, 76)),
    ];
    for veteran in [false, true] {
        for (name, tile) in spots {
            let run = settled(tile, veteran);
            println!("--- {name} veteran={veteran}: found={:?} goal={:?}", run.found, run.kingdom_goal(1));
            for l in lines_of(&run, 1) {
                println!("   | {l}");
            }
        }
    }
}

#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib tabula_probe -- --include-ignored"]
fn the_surface_says_only_what_stands_on_it() {
    use crate::world::plan::tabula as t;
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let houses: Vec<String> = crate::world::houses::ids().map(|id| crate::world::houses::get(id).label()).collect();
    let says = |line: &str| {
        houses.iter().any(|h| h == line)
            || line.starts_with("DEEPEST ")
            || ["THE DELVE", "DELVE", "DOWN", "E ENTER", "[4] KINGDOM WORKS"].contains(&line)
    };
    let mut spots: Vec<(i32, i32)> = vec![t::KEEP_STAND, t::delve_stand(), (100, 76)];
    spots.extend(t::CAMPS.iter().map(|c| t::camp_stand(*c)));
    for veteran in [false, true] {
        for &spot in &spots {
            for line in lines_of(&settled(spot, veteran), 1) {
                assert!(says(&line), "at {spot:?} (veteran {veteran}) the surface says {line:?}");
            }
        }
    }
}

/// The blank-slate realm's whole loop, flat, a frame at every stop: the spawn,
/// the Keep, each camp, the Delve's gate, down into its courtyard and back up.
/// `ANGEL_TABULA_LOOP=<dir> ANGEL_REALM_PLAN=tabula cargo test -p angel-delve
/// --lib write_tabula_loop -- --ignored`
#[test]
#[ignore = "writes the loop's frames to ANGEL_TABULA_LOOP; needs ANGEL_REALM_PLAN=tabula"]
fn write_tabula_loop() {
    use crate::world::plan::tabula as t;
    let Some(dir) = std::env::var_os("ANGEL_TABULA_LOOP").map(std::path::PathBuf::from) else {
        return;
    };
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    std::fs::create_dir_all(&dir).unwrap();
    let shoot = |run: &Run, name: &str| {
        let img = frame_for(run, 384, 224, Some(1));
        image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.rgba_bytes()).unwrap().save(dir.join(format!("{name}.png"))).unwrap();
    };
    let (kx, ky) = (t::KEEP_STAND.0 as f32, t::KEEP_STAND.1 as f32);
    // A frame of knight 1 at realm tile `tile`, the camera `up` half tiles above it.
    let placed = |tile: (f32, f32), up: f32| {
        let mut run = Run::at_home(41, 1, None, Default::default(), Default::default());
        run.walk_kingdom(Some((tile.0 as i32, tile.1 as i32)));
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = ((tile.0 + 0.5) * 2.0, (tile.1 + 0.7) * 2.0);
        hero.camera = Some((hero.x, hero.y - 1.0 - up));
        for _ in 0..90 {
            run.step(&BTreeMap::new());
        }
        run
    };
    shoot(&placed((kx, ky + 1.0), 3.0), "t00-spawn");
    shoot(&placed((kx - 7.0, ky - 2.0), 2.0), "t01-keep-west");
    for (i, camp) in t::CAMPS.iter().enumerate() {
        let stand = t::camp_stand(*camp);
        shoot(&placed((stand.0 as f32 + 5.0, stand.1 as f32 + 2.0), 3.0), &format!("t{:02}-camp-{i}", i + 2));
    }
    let (dx, dy) = t::delve_stand();
    let mut run = placed((dx as f32, dy as f32 + 0.5), 0.0);
    shoot(&run, "t18-delve-gate");
    // E at the gate, and a moment in the courtyard.
    let press = Input { interact: true, ..Default::default() };
    for _ in 0..6 {
        run.step(&BTreeMap::from([(1, press)]));
    }
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.room().kind, crate::rules::RoomKind::Gate, "E at the Delve's door goes down");
    shoot(&run, "t19-gate-courtyard");
    run.leave_kingdom_place();
    for _ in 0..40 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.room().kind, crate::rules::RoomKind::Kingdom, "and comes back up");
    shoot(&run, "t20-back-up");
    // Every line any of them said, once.
    let mut said = Vec::new();
    for (name, tile) in [("keep", t::KEEP_STAND), ("delve", t::delve_stand()), ("camp0", t::camp_stand(t::CAMPS[0]))] {
        for veteran in [false, true] {
            said.push(format!("{name} veteran={veteran}: {:?}", lines_of(&settled(tile, veteran), 1)));
        }
    }
    std::fs::write(dir.join("said.txt"), said.join("\n")).unwrap();
}
