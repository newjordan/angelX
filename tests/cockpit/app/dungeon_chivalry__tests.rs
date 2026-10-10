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
        let run = a.dungeon.shooter.as_mut().unwrap();
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
        let run = a.dungeon.shooter.as_mut().unwrap();
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
