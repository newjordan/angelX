//! The Delve's secrets, settled into the cockpit's realm.

use crate::drive::together_shooter::*;
use crate::drive::together_shooter::secrets::testing::*;
use std::collections::BTreeMap;

#[test]
fn a_hint_is_said_once_though_the_host_takes_the_marks_every_frame() {
    let _guard = crate::tests::env_lock();
    let dir = std::env::temp_dir().join(format!("delve-hints-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut a = crate::app::App::preview(crate::ui::viewer::Viewer::static_preview());
    a.dungeon.realm = Some(crate::drive::together_realm::Realm::beside(Some(
        &dir.join("island.json"),
    )));
    let mut run = on_a_generated_floor(Pack::Crypt, 2, first_of(RoomKind::Sanctuary));
    let (ax, ay) = run.room().altar();
    stand(&mut run, (ax + 1.0, ay));
    a.dungeon.shooter.set(Some(run));
    let mut hints = 0;
    for _ in 0..20 {
        let run = a.dungeon.shooter.edit().unwrap();
        let before = run.cues.len();
        run.step(&BTreeMap::new());
        hints += run.cues[before..]
            .iter()
            .filter(|c| c.starts_with("secret_hint:"))
            .count();
        assert!(a.settle_home());
        assert!(
            a.dungeon.shooter.as_ref().unwrap().marks.is_empty(),
            "taken"
        );
    }
    assert_eq!(hints, 1, "said once across twenty frames");
    std::fs::remove_dir_all(dir).unwrap();
}
