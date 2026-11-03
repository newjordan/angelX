use super::*;

#[test]
fn shake_is_smooth_deterministic_and_ends() {
    for tick in 0..120 {
        assert_eq!(camera::smooth_shake(tick, 0, 12, 6.0), (0, 0));
        let at = camera::smooth_shake(tick, 12, 12, 6.0);
        assert_eq!(at, camera::smooth_shake(tick, 12, 12, 6.0));
        let next = camera::smooth_shake(tick + 1, 12, 12, 6.0);
        assert!((at.0 - next.0).abs() <= 3 && (at.1 - next.1).abs() <= 3);
        let small = camera::smooth_shake(tick, 3, 12, 6.0);
        assert_eq!(small, (0, 0), "small trauma must settle to a subpixel");
        assert_eq!(camera::smooth_shake(tick, 12, 12, 6.0), camera::smooth_shake(tick, 24, 12, 6.0));
    }
    let mut run = crate::rules::feel::testing::quiet();
    run.shake = 0;
    assert_eq!(view_origin(&run, Some(1)), camera(&run, Some(1)));
}

#[test]
fn calm_screen_scales_shake_flash_and_restores_the_viewer() {
    use crate::render::viewer::{self, Preferences};
    let loud = camera::smooth_shake(3, 12, 12, 6.0);
    assert_ne!(loud, (0, 0));
    let mut flash = Img::new(2, 1);
    flash.set(0, 0, [244, 225, 203]);
    flash.set(1, 0, [61, 58, 54]);
    let original = flash.words().to_vec();
    viewer::soften(&mut flash);
    assert_eq!(flash.words(), original);
    viewer::with(Preferences { calm_screen: true }, || {
        assert_eq!(camera::smooth_shake(3, 12, 12, 6.0), (0, 0));
        viewer::soften(&mut flash);
        assert_eq!(flash.get(0, 0), Some([150, 138, 124]));
        assert_eq!(flash.get(1, 0), Some([61, 58, 54]));
        viewer::with(Preferences::default(), || assert!(!viewer::calm()));
        assert!(viewer::calm());
    });
    assert!(!viewer::calm());
    assert_eq!(camera::smooth_shake(3, 12, 12, 6.0), loud);
}

#[test]
fn pickup_pop_overshoots_then_rests_and_guardian_marks_end() {
    assert!(feel::pickup_lift(0) < 0);
    assert!((1..16).any(|age| feel::pickup_lift(age) > 0));
    assert_eq!(feel::pickup_lift(16), 0);
    assert_eq!(feel::pickup_lift(100), 0);
    let mut run = crate::rules::feel::testing::quiet();
    let mut blank = Img::new(NATIVE_W, NATIVE_H);
    run.spectacle.phase = Some(crate::rules::spectacle::PhaseMark {
        x: 24.0, y: 14.0, born: run.tick, room: run.at, rage: false,
    });
    feel::guardian_beats(&mut blank, &run);
    assert!(blank.words().iter().any(|&p| p != 0));
    run.tick += crate::rules::spectacle::PHASE_MARK;
    let mut end = Img::new(NATIVE_W, NATIVE_H);
    feel::guardian_beats(&mut end, &run);
    assert!(end.words().iter().all(|&p| p == 0));
    run.spectacle.corpses.push(crate::rules::spectacle::Corpse {
        x: 24.0, y: 14.0, born: run.tick, room: run.at, radius: 2.0,
        kind: crate::rules::EnemyKind::Boss, boss: Some(0),
    });
    feel::guardian_beats(&mut end, &run);
    assert!(end.words().iter().any(|&p| p != 0));
    run.tick += 36;
    let mut rest = Img::new(NATIVE_W, NATIVE_H);
    feel::guardian_beats(&mut rest, &run);
    assert!(rest.words().iter().all(|&p| p == 0));
}

/// PNG review: no host, clocks or renderer-specific randomness.
#[test]
#[ignore = "writes the polish review PNGs"]
fn write_polish_feel_shots() {
    use crate::rules::feel::testing::{bare, quiet};
    use crate::rules::{EnemyKind, Input};
    use std::collections::BTreeMap;
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("shot directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, frames: Vec<Img>| {
        let cols = frames.len().min(4) as i32;
        let rows = (frames.len() as i32 + cols - 1) / cols;
        let mut sheet = Img::black(cols * NATIVE_W, rows * (NATIVE_H + 14));
        for (i, img) in frames.iter().enumerate() {
            image::save_buffer(dir.join(format!("{name}-{}.png", i + 1)), &img.rgba_scaled(3),
                (img.w * 3) as u32, (img.h * 3) as u32, image::ColorType::Rgba8).unwrap();
            let (x, y) = (i as i32 % cols * NATIVE_W, i as i32 / cols * (NATIVE_H + 14));
            sheet.text(x + 4, y + 3, &format!("FRAME {}", i + 1), 'h');
            sheet.stamp(img, x, y + 14);
        }
        crate::rules::feel::testing::save(&sheet, &dir, name);
    };
    let draw = |run: &Run, calm: bool| crate::render::viewer::with(
        crate::render::viewer::Preferences { calm_screen: calm }, || {
            let mut img = frame_for(run, NATIVE_W, NATIVE_H, Some(1));
            crate::render::viewer::soften(&mut img);
            img
        });
    let mut run = quiet();
    bare(&mut run);
    run.spawn_boss_at_for_test(0, 24.0, 9.0);
    run.enemies[0].age = crate::rules::TELEGRAPH + 1;
    let start = run.tick;
    let mut before = Vec::new();
    let mut after = Vec::new();
    for dt in [0, 1, 2, 3, 5, 7, 9, 12] {
        run.tick = start + dt;
        run.shake = 12u32.saturating_sub(dt as u32);
        // Baseline formula, over exactly the same unshaken room picture.
        let mut base = run.clone();
        base.shake = 0;
        let mut old = scene_for(&base, NATIVE_W, NATIVE_H, Some(1));
        let k = run.shake.min(12) as i32 / 2;
        let hash = super::super::ink::hash(run.tick as i32, run.shake as i32, 91);
        let (x, y) = if k == 0 { (0, 0) } else {
            ((hash % (2 * k as u32 + 1)) as i32 - k, ((hash >> 8) % (2 * k as u32 + 1)) as i32 - k)
        };
        old.camera.0 += x as f32;
        old.camera.1 += y as f32;
        before.push(old.compose());
        after.push(draw(&run, false));
    }
    save("shake-before", before);
    save("shake-after", after);
    run.shake = 0;
    run.tick = start;
    let mut changing = run.clone();
    let (definition, _) = crate::rules::bosses::check("phase-shot",
        "name Test\nwhere crypt\nphases flower sweep every=40 volleys=2\nart\nkkkk\nkkkk\n").unwrap();
    changing.bosses[0].phases = definition.phases;
    for _ in 0..100 {
        changing.step(&BTreeMap::new());
        if changing.spectacle.phase.is_some() { break; }
    }
    assert!(!changing.spectacle.phase.expect("real volley transition").rage);
    let born = changing.tick;
    let mut phase = Vec::new();
    for age in [0, 4, 12, 30] {
        let mut at = changing.clone();
        while at.tick < born + age { at.step(&BTreeMap::new()); }
        phase.push(draw(&at, false));
    }
    save("boss-phase", phase);
    // Rage is a real phase transition through the watcher, not a fabricated mark.
    run.enemies[0].hp = run.enemies[0].max_hp / 10;
    run.bosses[0].rage = 0.5;
    run.step(&BTreeMap::new());
    assert!(run.spectacle.phase.is_some());
    let born = run.tick;
    let mut phase = Vec::new();
    for age in [0, 4, 12, 30] {
        let mut at = run.clone();
        while at.tick < born + age { at.step(&BTreeMap::new()); }
        phase.push(draw(&at, false));
    }
    save("boss-rage", phase);
    run.hitstop = Default::default();
    for e in &mut run.enemies { e.hp = 0; }
    run.step(&BTreeMap::new());
    assert!(!run.spectacle.corpses.is_empty());
    let born = run.tick;
    let mut fall = Vec::new();
    for age in [0, 5, 18, 36] {
        let mut at = run.clone();
        while at.tick < born + age { at.step(&BTreeMap::new()); }
        fall.push(draw(&at, false));
    }
    save("boss-fall", fall);
    run.shake = 12;
    run.feel.deaths.push(crate::rules::feel::Death { x: 25.0, y: 13.0, r: 2.5,
        ttl: crate::rules::feel::DEATH_TICKS, kind: Some(EnemyKind::Demon) });
    save("calm-off-on", vec![draw(&run, false), draw(&run, true)]);
    let mut pickup = quiet();
    bare(&mut pickup);
    let at = pickup.at;
    pickup.dungeon.rooms[at].items.clear();
    pickup.dungeon.rooms[at].items.push(crate::rules::Item::lying("bow".into(), 24.0, 14.0, 0));
    pickup.step(&BTreeMap::from([(1, Input { take: true, ..Input::default() })]));
    assert!(pickup.found.as_ref().is_some_and(|(_, _, line)| crate::rules::take_line(line)));
    let born = pickup.tick;
    let mut pop = Vec::new();
    for age in [0, 5, 10, 16] {
        let mut at = pickup.clone();
        while at.tick < born + age { at.step(&BTreeMap::new()); }
        pop.push(draw(&at, false));
    }
    save("pickup-pop", pop);
}

#[test]
fn feedback_reaches_the_actual_flat_and_parts_pictures() {
    let mut run = crate::rules::feel::testing::quiet();
    let base = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    run.spectacle.phase = Some(crate::rules::spectacle::PhaseMark {
        x: 24.0, y: 14.0, born: run.tick, room: run.at, rage: false,
    });
    assert_ne!(frame_for(&run, NATIVE_W, NATIVE_H, Some(1)).words(), base.words());
    run.spectacle.phase = None;
    run.found = Some((run.tick, 1, "tempered Bow".into()));
    let mut settled = run.clone();
    settled.found = Some((run.tick.saturating_sub(16), 1, "tempered Bow".into()));
    // Keep room animation fixed while changing only the label's age.
    if run.tick < 16 {
        run.tick = 16;
        settled.tick = 16;
        run.found.as_mut().unwrap().0 = 16;
        settled.found.as_mut().unwrap().0 = 0;
    }
    let launch = frame_for(&run, NATIVE_W, NATIVE_H, Some(1));
    let rest = frame_for(&settled, NATIVE_W, NATIVE_H, Some(1));
    assert_ne!(launch.words(), rest.words());
    let mut changes = Vec::new();
    for y in 0..NATIVE_H { for x in 0..NATIVE_W {
        if launch.get(x, y) != rest.get(x, y) { changes.push((x, y)); }
    }}
    assert!(changes.iter().all(|&(x, y)| (110..275).contains(&x) && (60..100).contains(&y)), "only pickup label moves: {:?}", changes.first());
    let parts = scene_for(&run, NATIVE_W, NATIVE_H, Some(1)).compose();
    assert_eq!(launch.words(), parts.words());
}
