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
    // What the Keep's site says on its own account (the Works): the progress
    // readout, the tally board, the trade plate in the corner, a teacher's offer,
    // and the board on the Great Shaft's winch, which is the shaft's tier.
    let works = |line: &str| {
        line.starts_with("KEEP ")
            || line.starts_with("REALM ")
            || line.starts_with("NO TRADE")
            || line.contains("HOLD E TO TAKE UP")
            || ["TALLY", "HOLD N TO STAND BACK", "1"].contains(&line)
    };
    let says = |line: &str| {
        houses.iter().any(|h| h == line)
            || line.starts_with("DEEPEST ")
            || ["THE DELVE", "DELVE", "DOWN", "E ENTER", "[4] KINGDOM WORKS"].contains(&line)
            || works(line)
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

/// The surface's HUD, at every spot and in every state a knight meets it in: the boxes it lays
/// down (the corner block, the trade plate, the builder's strip and the Clerk's lines over it,
/// the goal's arrow and its name, the doors' marks) share no pixel, and none runs off the view;
/// with the ledger open nothing else is laid down at all.
/// `ANGEL_REALM_PLAN=tabula cargo test -p angel-delve --lib the_hud_never_overlaps -- --include-ignored`
#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula"]
fn the_hud_never_overlaps_at_any_frame() {
    use crate::world::plan::tabula as t;
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let mut spots: Vec<(i32, i32)> = vec![t::KEEP_STAND, t::delve_stand(), (100, 76), (133, 100), (150, 96), (126, 80), (133, 66)];
    spots.extend(t::CAMPS.iter().map(|c| t::camp_stand(*c)));
    let (w, h) = (384, 224);
    let mut frames = 0;
    let mut bad: Vec<String> = Vec::new();
    for &spot in &spots {
        for state in 0..10 {
            let mut run = settled(spot, state % 2 == 1);
            run.home.industry.found_hillside(crate::kingdom::catalog(), 7);
            // How long since the party came up: the hint's first seconds, and after.
            let mut wait = 0;
            for later in [0, 300, 300] {
                for _ in 0..later {
                    run.step(&BTreeMap::new());
                }
                wait += later;
                let mut run = run.clone();
                match state / 2 {
                    // Walking about, nothing out.
                    0 => {}
                    // A block on the back: the goal is the stockyard.
                    1 => run.players.get_mut(&1).unwrap().block = Some("limestone".into()),
                    // The builder out, the flow overlay on (the Clerk's lines).
                    2 => {
                        let b = run.kingdom.industry.entry(1).or_default();
                        (b.open, b.page, b.overlay) = (true, 0, true);
                    }
                    // The ledger over the world.
                    3 => {
                        let b = run.kingdom.industry.entry(1).or_default();
                        (b.open, b.page) = (true, 1);
                    }
                    // The builder out with the card of the camp up, plans laid, a copy half marked.
                    _ => {
                        let c = crate::kingdom::catalog();
                        let camp = run.home.industry.camp();
                        let _ = run.home.industry.apply(c, crate::kingdom::Command::Plan { plan: "well".into(), tile: (camp.0 + 6, camp.1 + 4), priority: 2, by: 1 });
                        let card = run.home.industry.card(c, camp);
                        let b = run.kingdom.industry.entry(1).or_default();
                        (b.open, b.page, b.overlay, b.tile) = (true, 0, true, camp);
                        b.rig.card = true;
                        b.rig.planning = true;
                        b.rig.mark = Some((camp.0 - 2, camp.1 - 2));
                        b.rig.numbers = Some(crate::kingdom::Numbers { at: 0, card, ..Default::default() });
                    }
                }
                run.step(&BTreeMap::new());
                let (_, boxes) = super::kingdom::hud_log::read(|| frame_for(&run, w, h, Some(1)));
                frames += 1;
                let what = format!("at {spot:?}, state {state}, wait {wait}");
                if state / 2 == 3 {
                    if !boxes.iter().all(|(n, _)| *n == "ledger") {
                        bad.push(format!("{what}: the ledger is up and the HUD lays down {boxes:?}"));
                    }
                    continue;
                }
                for (i, (na, a)) in boxes.iter().enumerate() {
                    if !(a.0 >= 0 && a.1 >= 0 && a.0 + a.2 <= w && a.1 + a.3 <= h) {
                        bad.push(format!("{what}: {na} {a:?} runs off the view"));
                    }
                    for (nb, b) in &boxes[i + 1..] {
                        let hit = a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3;
                        if hit {
                            bad.push(format!("{what}: {na} {a:?} lies on {nb} {b:?}"));
                        }
                    }
                }
            }
        }
    }
    assert!(frames > 100, "{frames} frames swept");
    assert!(bad.is_empty(), "{} of {frames} frames overlap:\n{}", bad.len(), bad.iter().take(25).cloned().collect::<Vec<_>>().join("\n"));
}

/// The Clerk of Works' sentence, however long the sim makes it, is broken at words into at most
/// three lines, each on a plate of its own width inside the view, and ends in two dots when it
/// does not all fit.
#[test]
#[ignore = "needs the blank-slate plan standing: ANGEL_REALM_PLAN=tabula"]
fn the_clerks_sentence_never_runs_off_its_plate() {
    use super::kingdom::{hillside_art, hud_log};
    assert!(crate::world::plan::tabula(), "run with ANGEL_REALM_PLAN=tabula");
    let mut run = settled(crate::world::plan::tabula::KEEP_STAND, false);
    run.home.industry.found_hillside(crate::kingdom::catalog(), 7);
    let b = run.kingdom.industry.entry(1).or_default();
    (b.open, b.page, b.overlay) = (true, 0, true);
    let long = "waiting on scaffold poles; 3 stand of 24 wanted; the carter's yard has no free carts; all 8 are out, and more carts or another yard would clear it, so lay a second road or buy carts for the yard".to_uppercase();
    for sentence in ["", "short", long.as_str(), &long[..60], &long.repeat(3)] {
        let view = crate::world::hillside::HillView { clerk: sentence.to_string(), ..Default::default() };
        let (_, boxes) = hud_log::read(|| {
            let mut ui = Img::new(384, 224);
            hillside_art::clerk(&mut ui, &run, &view, Some(1));
        });
        assert!(!boxes.is_empty() && boxes.len() <= 3, "{} plates for {sentence:?}", boxes.len());
        for (_, (x, y, w, h)) in &boxes {
            assert!(*x >= 0 && x + w <= 384 - 4 && *y >= 0 && y + h <= 224, "{sentence:?}: a plate {:?} off the view", (x, y, w, h));
        }
    }
}
