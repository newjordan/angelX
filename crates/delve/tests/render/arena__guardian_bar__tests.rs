use super::super::*;
use super::State;
use crate::rules::bosses::{Stage, Windup};
use crate::rules::{desperate, signatures};

/// Guardian `id` alone in its room at `share` of its health.
fn scene(id: &str, share: f32) -> Run {
    let mut run = super::super::testing::boss(id);
    let e = &mut run.enemies[0];
    e.hp = ((e.max_hp as f32) * share) as u32;
    e.seen_hp = e.hp;
    run
}

fn boss(run: &Run) -> &crate::rules::bosses::Boss {
    &run.bosses[usize::from(run.enemies[0].boss.unwrap())]
}

/// Cinderjaw spent after its breath: its furnace stands open.
fn vent(run: &mut Run) {
    let inhale = boss(run).windups.iter().position(|u| u.kind == Windup::Inhale).unwrap() as u8;
    let e = &mut run.enemies[0];
    (e.stage, e.timer, e.signature.windup) = (signatures::DAZED, 5, inhale);
}

/// The plate and its bar alone, on a blank picture.
fn plate(run: &Run) -> Img {
    let mut cv = Img::new(NATIVE_W, NATIVE_H);
    super::super::wanted::nameplates(&mut cv, run, (0, 0));
    cv
}

fn count(im: &Img, ch: char) -> usize {
    let c = super::super::super::ink::ink(ch);
    im.pixels().filter(|&p| Some(p) == c).count()
}

#[test]
fn the_fill_reads_each_guardians_own_rules() {
    let state = |run: &Run| super::state(&run.enemies[0], boss(run));
    // Cinderjaw: iron until it breathes, open after, molten in its rage.
    let mut cj = scene(crate::rules::furnace::ID, 0.9);
    assert_eq!(state(&cj), State::Armored);
    vent(&mut cj);
    assert_eq!(state(&cj), State::Venting);
    assert_eq!(state(&scene(crate::rules::furnace::ID, 0.2)), State::Molten);
    // The Relic Liege: its ward whole until its rage cracks it.
    assert_eq!(state(&scene(crate::rules::court::ID, 0.9)), State::Warded);
    assert_eq!(state(&scene(crate::rules::court::ID, 0.5)), State::Warded);
    assert_eq!(state(&scene(crate::rules::court::ID, 0.2)), State::Cracked);
    // A guardian of data alone: red, then orange in its rage.
    let plain = scene("crypt-tax-keeper", 0.5);
    assert_eq!(boss(&plain).stage(plain.enemies[0].hp, plain.enemies[0].max_hp), Stage::Middle);
    assert_eq!(state(&plain), State::Plain);
    assert_eq!(state(&scene("crypt-tax-keeper", 0.2)), State::Raging);
}

#[test]
fn the_bar_fills_with_health_and_shows_a_blows_bite() {
    let full = plate(&scene("crypt-tax-keeper", 1.0));
    let half = plate(&scene("crypt-tax-keeper", 0.5));
    // The banner is whole however hurt it is; the bar's red is its health.
    assert_eq!(count(&full, '8'), count(&half, '8'));
    assert!(count(&full, '7') > count(&half, '7') + 10, "the fill follows its health");
    // A blow's bite: pale where it was, flashed while the hit is fresh.
    let mut run = scene("crypt-tax-keeper", 0.5);
    run.enemies[0].drain = run.enemies[0].max_hp * 7 / 10;
    // (The name is lettered in the same parchment.)
    assert!(count(&plate(&run), '$') >= count(&half, '$') + 6, "the bite drains pale");
    run.enemies[0].hurt = desperate::HIT_FLASH;
    let fresh = plate(&run);
    assert!(count(&fresh, 'T') >= 6 && count(&fresh, 'c') >= 2, "a fresh hit is lit");
    assert_eq!((count(&half, 'T'), count(&half, 'c')), (0, 0));
}

#[test]
fn the_marks_stand_where_its_fight_turns_and_dim_once_passed() {
    // The ticks under the bar: grey for the middle, brass for the rage.
    let ahead = plate(&scene("grinwick", 0.95));
    assert!(count(&ahead, 'h') >= 2 && count(&ahead, 'O') >= 2);
    let past = plate(&scene("grinwick", 0.3));
    assert_eq!((count(&past, 'h'), count(&past, 'O')), (0, 0));
}

#[test]
fn a_mirror_draws_the_bar_the_host_does() {
    // The bar is read off the run alone: the same run, the same picture,
    // and the run as a guest decodes it the same again.
    let mut run = scene(crate::rules::furnace::ID, 0.4);
    vent(&mut run);
    run.enemies[0].drain = run.enemies[0].max_hp / 2;
    run.enemies[0].hurt = 3;
    let guest: Run = serde_json::from_str(&serde_json::to_string(&run).unwrap()).unwrap();
    assert_eq!(plate(&run).rgb_bytes(), plate(&guest).rgb_bytes());
}

/// Review sheets (`ANGEL_ARENA_SHOTS=<dir> cargo test -p angel-delve
/// write_guardian_bar_shots -- --ignored`): each guardian at its three
/// stages, at 1x and 3x, and a sheet of the bars close up.
#[test]
#[ignore = "writes the guardian bar review PNGs"]
fn write_guardian_bar_shots() {
    let dir = std::path::PathBuf::from(std::env::var_os("ANGEL_ARENA_SHOTS").expect("shot directory"));
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, img: &Img, k: u32| {
        image::save_buffer(dir.join(format!("{name}.png")), &img.rgba_scaled(k),
            img.w as u32 * k, img.h as u32 * k, image::ColorType::Rgba8).unwrap();
    };
    let shot = |run: &Run| frame_for(run, NATIVE_W, NATIVE_H, Some(1));
    // Close up: the plate and the bar over the guardian's head.
    let close = |run: &Run, img: &Img| {
        let e = &run.enemies[0];
        let (cx, cy) = camera(run, Some(1));
        let (x, y) = at(e.x, e.y);
        Img::from_view(img, x - cx - 90, (y - cy - 110).max(0), 180, 64)
    };
    let mut rows: Vec<(String, Img)> = Vec::new();
    for id in ["cinderjaw", "crypt-relic-liege", "grinwick", "crypt-tax-keeper"] {
        for (stage, share) in [(Stage::Open, 0.92), (Stage::Middle, 0.55), (Stage::Rage, 0.2)] {
            let run = scene(id, share);
            let img = shot(&run);
            save(&format!("{id}-{stage:?}-1x"), &img, 1);
            save(&format!("{id}-{stage:?}-3x"), &img, 3);
            rows.push((format!("{id} {stage:?}"), close(&run, &img)));
        }
        // A fresh blow, and its bite draining.
        let mut run = scene(id, 0.55);
        run.enemies[0].drain = run.enemies[0].max_hp * 66 / 100;
        run.enemies[0].hurt = desperate::HIT_FLASH;
        let img = shot(&run);
        save(&format!("{id}-hit-3x"), &img, 3);
        rows.push((format!("{id} hit"), close(&run, &img)));
        run.enemies[0].hurt = 0;
        run.enemies[0].drain = run.enemies[0].max_hp * 60 / 100;
        rows.push((format!("{id} drain"), close(&run, &shot(&run))));
    }
    // Cinderjaw's open furnace, both beats.
    for tick in [0, 4] {
        let mut run = scene("cinderjaw", 0.6);
        vent(&mut run);
        run.tick = run.tick / 8 * 8 + tick;
        let img = shot(&run);
        save(&format!("cinderjaw-vent{tick}-3x"), &img, 3);
        rows.push((format!("cinderjaw vent {tick}"), close(&run, &img)));
    }
    // Its rage's word over it, the moment it turns.
    let mut run = scene("crypt-relic-liege", 0.28);
    let e = &run.enemies[0];
    run.spectacle.phase = Some(crate::rules::spectacle::PhaseMark { x: e.x, y: e.y, born: run.tick - 6, room: run.at, rage: true });
    let img = shot(&run);
    save("liege-rages-3x", &img, 3);
    rows.push(("liege rages".into(), close(&run, &img)));
    // A busy room: rings of shots about its head.
    let mut run = scene("crypt-tax-keeper", 0.45);
    let (bx, by) = (run.enemies[0].x, run.enemies[0].y);
    for k in 0..3 {
        run.volley_for_test(bx, by - 3.0 - k as f32 * 1.5, k as f32 * 0.4);
    }
    let img = shot(&run);
    save("busy-3x", &img, 3);
    rows.push(("busy".into(), close(&run, &img)));
    // Runs for the dev host to start a live fight from (`--snapshot`).
    for (id, share) in [("cinderjaw", 0.62), ("cinderjaw", 0.32), ("crypt-relic-liege", 0.33), ("crypt-tax-keeper", 0.38)] {
        let run = scene(id, share);
        std::fs::write(dir.join(format!("{id}-{}.json", (share * 100.0) as u32)), serde_json::to_vec(&run).unwrap()).unwrap();
    }
    // The close-ups on one sheet, 4x.
    let (cw, ch) = (180, 64 + 10);
    let cols = 3;
    let mut sheet = Img::black(cols * cw, (rows.len() as i32 + cols - 1) / cols * ch);
    for (i, (label, im)) in rows.iter().enumerate() {
        let (x, y) = (i as i32 % cols * cw, i as i32 / cols * ch);
        sheet.text(x + 2, y + 1, &label.to_uppercase(), 'h');
        sheet.stamp(im, x, y + 10);
    }
    save("bars-sheet-4x", &sheet, 4);
}
