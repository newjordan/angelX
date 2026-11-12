use super::*;

#[test]
fn finished_paint_retries_admission_without_restarting_jobs_or_rewriting_spells() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-paint-app-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("captured.card");
    let source = "name Captured\nkind spell\ncooldown 5\nnova 10\ndraw shot\n00\n00\n";
    let mut current = cards::check("captured", source).unwrap().card;
    current.by = "Original maker".into();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(root.clone());
    std::fs::write(&path, source.replace("00", "11")).unwrap();
    let target = painter::Target::Card(path.clone());
    app.realm.dungeon.painters.retry(target.clone());
    // The completion is retained while there is no world to admit it into.
    app.watch_painters();
    assert_eq!(app.realm.dungeon.painters.running(), 0);
    let mut run = crate::delve::together_shooter::Run::new(0, 9, None);
    run.book.insert(current.clone());
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    app.realm.dungeon.shooter.set(Some(run));
    app.watch_painters();
    let run = app.realm.dungeon.shooter.as_ref().unwrap();
    let painted = run.book.get("captured").unwrap();
    assert_eq!(painted.looks.shot.as_ref().unwrap().frames[0][0], "11");
    assert_eq!(painted.by, "Original maker");
    assert_eq!(painted.cooldown, 5);
    assert_eq!(run.players[&1].spell_cooldowns[0], 71);
    // A later file edit is an executable definition change, not painter policy.
    std::fs::write(
        &path,
        source.replace("nova 10", "nova 99").replace("00", "22"),
    )
    .unwrap();
    app.realm.dungeon.painters.retry(target);
    app.watch_painters();
    let run = app.realm.dungeon.shooter.as_ref().unwrap();
    assert_eq!(
        run.book
            .get("captured")
            .unwrap()
            .looks
            .shot
            .as_ref()
            .unwrap()
            .frames[0][0],
        "11"
    );
    assert_eq!(run.book.get("captured").unwrap().effects, current.effects);
    assert!(app.realm.dungeon.painters.poll().is_empty());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
