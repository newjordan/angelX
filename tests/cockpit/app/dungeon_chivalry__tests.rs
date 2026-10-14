use super::*;
use crate::drive::{chivalry::Mount, together_realm::Realm, together_shooter::RoomKind};

fn app() -> App {
    App::preview(crate::ui::viewer::Viewer::static_preview())
}

/// An app whose realm lives in a scratch directory.
fn app_in(dir: &std::path::Path) -> App {
    let mut a = app();
    a.dungeon.realm = Some(Realm::beside(Some(&dir.join("island.json"))));
    a
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("chivalry-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_stable_and_the_lists_are_walked_to_in_the_world() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("walk");
    let mut a = app_in(&dir);
    let said = a.dungeon_command(Some("stable"));
    assert!(said.contains("stables"), "{said}");
    let run = a.dungeon.shooter.as_ref().expect("a delve opened");
    assert_eq!(run.room().kind, RoomKind::Stables);
    assert!(a.dungeon.expanded);
    let said = a.dungeon_command(Some("tournament"));
    assert!(said.contains("next rival: Sir Kay"), "{said}");
    assert_eq!(
        a.dungeon.shooter.as_ref().unwrap().room().kind,
        RoomKind::Lists
    );
    a.input = "/world visit stables".into();
    a.submit();
    assert_eq!(
        a.dungeon.shooter.as_ref().unwrap().room().kind,
        RoomKind::Stables
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn text_commands_saddle_and_tend_the_same_way_the_plates_do() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("text");
    let mut a = app_in(&dir);
    a.dungeon_command(Some("stable"));
    let said = a.dungeon_command(Some("stable select cinder"));
    assert!(said.contains("Cinder is saddled"), "{said}");
    let said = a.dungeon_command(Some("stable tend"));
    assert!(said.contains("brushed"), "{said}");
    // The realm keeps it, and the open delve sees it.
    let saved = Realm::beside(Some(&dir.join("island.json")));
    assert_eq!(saved.home.stable.selected, Mount::Cinder);
    assert!(saved.home.stable.is_tended(Mount::Cinder));
    let run = a.dungeon.shooter.as_ref().unwrap();
    assert_eq!(run.home.stable.selected, Mount::Cinder);
    for bad in [
        "stable select ghost",
        "stable gallop",
        "tournament round 1 guard",
    ] {
        let before = a.realm().home.stable.clone();
        let said = a.dungeon_command(Some(bad));
        assert!(
            said.contains("No mount") || said.contains("Unknown"),
            "{bad}: {said}"
        );
        assert_eq!(a.realm().home.stable, before);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_failed_save_leaves_the_stable_as_it_was() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("fail");
    let bad = dir.join("not-a-directory");
    std::fs::write(&bad, b"x").unwrap();
    let mut a = app();
    a.dungeon.realm = Some(Realm::beside(Some(&bad.join("island.json"))));
    let said = a.dungeon_command(Some("stable select mist"));
    assert!(said.contains("Not saved"), "{said}");
    assert_eq!(a.realm().home.stable.selected, Mount::Bramble);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_bout_at_the_lists_is_kept_by_the_realm() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("bout");
    let mut a = app_in(&dir);
    a.dungeon_command(Some("stable tend"));
    a.dungeon_command(Some("tournament"));
    {
        let run = a.dungeon.shooter.edit().unwrap();
        run.mount_up(1);
        run.marks.insert("joust:kay:won".into(), 1);
        run.marks.insert("joust:unhorsed".into(), 1);
    }
    assert!(a.settle_home());
    let stable = &a.realm().home.stable;
    assert_eq!((stable.bouts, stable.unhorsed), (1, 1));
    assert_eq!(stable.wins.get("kay"), Some(&1));
    assert!(
        !stable.is_tended(Mount::Bramble),
        "the bout spent the tending"
    );
    let saved = Realm::beside(Some(&dir.join("island.json")));
    assert_eq!(saved.home.stable.bouts, 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_bout_at_the_lists_paints_in_the_expanded_delve() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("paint");
    let mut a = app_in(&dir);
    a.dungeon_command(Some("tournament"));
    {
        let run = a.dungeon.shooter.edit().unwrap();
        run.mount_up(1);
        let spur = std::collections::BTreeMap::from([(
            1,
            crate::drive::together_shooter::Input {
                fire: true,
                ..Default::default()
            },
        )]);
        run.step(&Default::default());
        run.step(&spur);
        for _ in 0..30 {
            run.step(&Default::default());
        }
    }
    for (w, h) in [(160, 48), (80, 24), (48, 16)] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw::ui(frame, &mut a))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        if w == 160 {
            assert!(text.contains("The Lists"), "the header names the lists");
            assert!(text.contains("THE STABLE"), "the sidebar keeps the stable");
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_horse_market_buys_breeds_and_sells_for_the_realm() {
    let _guard = crate::tests::env_lock();
    let dir = scratch("market");
    let mut a = app_in(&dir);
    a.realm().treasury.add(crate::drive::together_realm::Spoil::Gold, 5_000);
    let said = a.dungeon_command(Some("stable market"));
    assert!(said.contains("gold"), "{said}");
    let said = a.dungeon_command(Some("stable buy 1"));
    assert!(said.contains("joins your yard"), "{said}");
    let said = a.dungeon_command(Some("stable buy 2"));
    assert!(said.contains("joins your yard"), "{said}");
    assert_eq!(a.realm().home.stable.herd.len(), 2);
    let said = a.dungeon_command(Some("stable breed 1 2"));
    assert!(said.contains("A foal in the yard"), "{said}");
    assert_eq!(a.realm().home.stable.herd.len(), 3);
    let gold_before = a.realm().treasury.get(crate::drive::together_realm::Spoil::Gold);
    let said = a.dungeon_command(Some("stable status"));
    assert!(said.contains("foaled in the") && said.contains("out of"), "{said}");
    let said = a.dungeon_command(Some("stable sell 3"));
    assert!(said.contains("Sold"), "{said}");
    assert!(a.realm().treasury.get(crate::drive::together_realm::Spoil::Gold) > gold_before);
    // And it all survives the save.
    let saved = Realm::beside(Some(&dir.join("island.json")));
    assert_eq!(saved.home.stable.herd.len(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn chariot_keyboard_controls_board_with_e_and_drop_iron_with_x_only() {
    use ratatui::crossterm::event::KeyCode;
    use crate::drive::together_shooter::{chariot, home::BUY_HOLD, world};
    let _guard = crate::tests::env_lock();
    let dir = scratch("chariot-keys");
    let mut a = app_in(&dir);
    a.dungeon_command(Some("stable"));
    let run = a.dungeon.shooter.edit().unwrap();
    run.arrive(world::room_of(&run.dungeon, RoomKind::Hippodrome).unwrap(), (20.0, 10.5));
    run.step(&Default::default());
    a.dungeon.key_releases = true;
    for key in ['f', 'e'] {
        a.dungeon.held.clear();
        a.dungeon.hold(KeyCode::Char(key), false);
        let input = a.dungeon.local_inputs(std::time::Instant::now());
        for _ in 0..BUY_HOLD + 2 { a.dungeon.shooter.edit().unwrap().step(&input); }
        assert_eq!(a.dungeon.shooter.as_ref().unwrap().chariot.is_some(), key == 'e');
    }
    for _ in 0..chariot::GATE_TICKS + 2 { a.dungeon.shooter.edit().unwrap().step(&Default::default()); }
    for key in ['e', 'f', 'x'] {
        a.dungeon.held.clear();
        a.dungeon.hold(KeyCode::Char(key), false);
        let input = a.dungeon.local_inputs(std::time::Instant::now());
        let run = a.dungeon.shooter.edit().unwrap();
        for _ in 0..150 {
            if run.chariot.as_ref().unwrap().player().wrecked == 0 { break; }
            run.step(&Default::default());
        }
        assert_eq!(run.chariot.as_ref().unwrap().player().wrecked, 0);
        let before = run.chariot.as_ref().unwrap().player().caltrops;
        let bombs = run.players[&1].bombs;
        run.step(&input);
        assert_eq!(run.chariot.as_ref().unwrap().player().caltrops, before - u8::from(key == 'x'));
        assert_eq!(run.players[&1].bombs, bombs);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_herd_selection_and_helios_birth_save_and_grow_in_the_live_home() {
    use crate::drive::chivalry::{Horse, bloodline};
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::ranch;
    let _guard = crate::tests::env_lock();
    let dir = scratch("helios-herd");
    let mut a = app_in(&dir);
    a.realm().home.stable.herd = vec![Horse {
        name: "Cloud".into(), house: "qwen".into(), blood: bloodline("qwen"), mare: true,
        ..Horse::default()
    }];
    a.realm().treasury.add(Spoil::Gold, 1_000_000);
    a.dungeon_command(Some("stable"));
    let said = a.dungeon_command(Some("stable select 1"));
    assert!(said.contains("Cloud selected"), "{said}");
    assert_eq!(a.realm().home.stable.selected, Mount::Bramble);
    assert_eq!(a.dungeon.shooter.as_ref().unwrap().home.stable.selected_herd, Some(0));
    for _ in 0..400 {
        a.dungeon.shooter.edit().unwrap().marks.insert("ranch:breed".into(), 1);
        assert!(a.settle_home());
        if ranch::herd_foal(&a.realm().home.stable).is_some() { break; }
    }
    let foal = ranch::herd_foal(&a.realm().home.stable).unwrap().clone();
    assert_eq!((&foal.sire[..], &foal.dam[..]), ("Helios", "Cloud"));
    assert_eq!(foal.blood, ranch::HELIOS_BLOOD);
    a.realm().raids_won = ranch::YEARLING;
    assert!(a.settle_home());
    let run = a.dungeon.shooter.as_ref().unwrap();
    assert!(run.home.ranch.foal.is_none());
    assert_eq!(ranch::age(ranch::herd_foal(&run.home.stable).unwrap(), run.home.ranch.wins), ranch::Age::Yearling);
    let saved = Realm::beside(Some(&dir.join("island.json")));
    assert_eq!(ranch::herd_foal(&saved.home.stable).unwrap(), &foal);
    let status = a.dungeon_command(Some("stable status"));
    assert!(status.contains("Helios") && status.contains("Yearling"), "{status}");
    let gold = saved.treasury.get(Spoil::Gold);
    assert!(a.settle_home());
    assert_eq!(a.realm().treasury.get(Spoil::Gold), gold, "the covering mark is consumed once");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_last_place_chariot_finish_still_saves_the_record() {
    use crate::drive::chivalry::{Horse, bloodline};
    let _guard = crate::tests::env_lock();
    let dir = scratch("chariot-last");
    let mut a = app_in(&dir);
    a.realm().home.stable.herd.push(Horse { name: "Cloud".into(), house: "qwen".into(), blood: bloodline("qwen"), ..Horse::default() });
    a.dungeon_command(Some("stable"));
    a.settle_home();
    a.dungeon.shooter.edit().unwrap().marks.insert("chariot:4:Cloud".into(), 1);
    assert!(a.settle_home());
    let saved = Realm::beside(Some(&dir.join("island.json")));
    assert_eq!(saved.home.stable.herd[0].starts, 1);
    assert_eq!(saved.home.stable.herd[0].earned, 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn loading_and_saving_an_old_realm_migrates_its_ranch_foal_once() {
    use crate::drive::together_shooter::ranch;
    let _guard = crate::tests::env_lock();
    let dir = scratch("helios-migrate");
    let path = dir.join("island.json");
    let mut old = Realm::beside(Some(&path));
    old.raids_won = 7;
    old.home.ranch.foal = Some(ranch::Foal { name: "Phaethon".into(), filly: false, born: 2 });
    old.home.ranch.coverings = 19;
    old.save().unwrap();
    let loaded = Realm::beside(Some(&path));
    assert!(loaded.home.ranch.foal.is_none());
    assert_eq!(loaded.home.ranch.coverings, 19);
    assert_eq!(loaded.home.stable.herd.len(), 1);
    let horse = ranch::herd_foal(&loaded.home.stable).unwrap();
    assert_eq!(horse.born, Some(2));
    assert_eq!(ranch::age(horse, loaded.home.ranch.wins), ranch::Age::Grown);
    loaded.save().unwrap();
    let loaded = Realm::beside(Some(&path));
    assert_eq!(loaded.home.stable.herd.len(), 1);
    assert!(loaded.home.ranch.helios_bred);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_failed_ranch_save_restores_the_herd_fee_and_attempt_for_retry() {
    use crate::drive::chivalry::Horse;
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::ranch;
    let _guard = crate::tests::env_lock();
    let dir = scratch("helios-retry");
    let bad = dir.join("not-a-directory");
    std::fs::write(&bad, b"x").unwrap();
    let mut a = app_in(&bad);
    a.realm().home.stable.herd.push(Horse { name: "Cloud".into(), house: "keep".into(), mare: true, ..Horse::default() });
    a.realm().treasury.add(Spoil::Gold, 10_000);
    a.dungeon_command(Some("stable"));
    let stable = a.realm().home.stable.clone();
    a.dungeon.shooter.edit().unwrap().marks.insert("ranch:breed".into(), 1);
    assert!(!a.settle_home());
    assert_eq!(a.realm().home.stable, stable);
    assert_eq!(a.realm().home.ranch.coverings, 0);
    assert_eq!(a.realm().treasury.get(Spoil::Gold), 10_000);
    assert!(a.dungeon.shooter.as_ref().unwrap().marks.contains_key("ranch:breed"));
    std::fs::remove_file(&bad).unwrap();
    assert!(a.settle_home());
    assert_eq!(a.realm().home.ranch.coverings, 1);
    assert_eq!(a.realm().treasury.get(Spoil::Gold), 10_000 - ranch::FEE);
    assert!(a.settle_home());
    assert_eq!(a.realm().treasury.get(Spoil::Gold), 10_000 - ranch::FEE);
    std::fs::remove_dir_all(dir).unwrap();
}
