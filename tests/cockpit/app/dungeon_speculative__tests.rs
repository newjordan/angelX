use super::*;
fn restore_point(
    app: &App,
    root: &std::path::Path,
) -> (
    crate::drive::together_shooter::Run,
    angel_delve::rules::host_commands::Journal,
) {
    let path = root.join("session.json");
    app.dungeon.shooter.save_checkpoint(&path, 9).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let commands = saved["commands"].clone();
    let session = commands["session"].as_str().unwrap().to_owned();
    let run = app.dungeon.shooter.as_ref().unwrap().clone();
    let journal =
        angel_delve::rules::host_commands::Journal::restore(commands, &session, run.raid_id)
            .unwrap();
    (run, journal)
}
#[test]
fn running_model_survives_metadata_admission_failure_and_returns_once() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-speculative-retry-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(root.clone());
    let run = crate::drive::together_shooter::Run::new(0, 9, Some("Guest"));
    app.dungeon.shooter.set(Some(run.clone()));
    let club = crate::drive::together_grants::Scripted::new(
        &["wish retry-grace | Retry Grace\nsay retry grace\ntier damage 10 :: Original\n"],
        Duration::from_millis(100),
    );
    app.dungeon.forge.grants.oracle = Some(club.clone());
    app.teach_spoken(vec![(1, "retry grace".into(), 1, Default::default())]);
    let job = app.dungeon.forge.learning[0].job;
    assert!(job.is_some());
    let runners = app.dungeon.forge.grants.runners.clone().unwrap();
    let mut event = angel_delve::rules::speculative::Event::new(&run);
    event.lamp(&run, 1, run.seat_epoch(1), "retained metadata", true);
    let pending = app.dungeon.shooter.capture_speculative(event).unwrap();
    app.dungeon.forge.speculative = Some(pending.clone());
    let (run, journal) = restore_point(&app, &root);
    let session = app.dungeon.shooter.wish_session();
    app.dungeon.shooter.set(None);
    assert!(!app.flush_speculative());
    app.teach_asked();
    app.teach_next();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.dungeon.forge.learning[0].returned {
        app.take_grants();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let answer = app.dungeon.forge.learning[0].answer.clone();
    assert!(answer.is_some());
    app.watch_learning();
    assert_eq!(app.dungeon.forge.learning[0].answer, answer);
    assert_eq!(app.dungeon.forge.speculative, Some(pending));
    app.dungeon.shooter.restore_session(run, Some(journal));
    assert_eq!(app.dungeon.shooter.wish_session(), session);
    assert!(app.flush_speculative());
    app.teach_next();
    assert_eq!(app.dungeon.forge.learning[0].job, job);
    assert_eq!(app.dungeon.forge.learning[0].answer, answer);
    assert!(std::sync::Arc::ptr_eq(
        &runners,
        app.dungeon.forge.grants.runners.as_ref().unwrap()
    ));
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    assert_eq!(
        app.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .lamp
            .wishing
            .iter()
            .filter(|p| p.words == "retained metadata")
            .count(),
        1
    );
    drop(app);
    drop(runners);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn in_flight_replaced_session_publishes_original_library_without_game_adoption() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-detached-model-{}", std::process::id()));
    let original = root.join("original");
    let replacement = root.join("replacement");
    std::fs::create_dir_all(&original).unwrap();
    std::fs::create_dir_all(&replacement).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(original.clone());
    app.dungeon
        .shooter
        .set(Some(crate::drive::together_shooter::Run::new(
            0,
            9,
            Some("Guest"),
        )));
    let club = crate::drive::together_grants::Scripted::new(
        &[
            "wish detached-grace | Detached Grace\nsay detached grace\ntier damage 10 :: Original answer\n",
        ],
        Duration::from_millis(200),
    );
    app.dungeon.forge.grants.oracle = Some(club.clone());
    app.teach_spoken(vec![(1, "detached grace".into(), 1, Default::default())]);
    let job = app.dungeon.forge.learning[0].job;
    let runners = app.dungeon.forge.grants.runners.clone().unwrap();
    assert!(job.is_some());
    assert!(!app.dungeon.forge.learning[0].returned);
    let session = app.dungeon.shooter.wish_session();
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(replacement.clone());
    let mut fresh = crate::drive::together_shooter::Run::new(0, 9, Some("Guest"));
    fresh.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    fresh.cues.push("already issued".into());
    app.dungeon.shooter.set(Some(fresh));
    assert_ne!(app.dungeon.shooter.wish_session(), session);
    app.teach_next();
    assert!(app.dungeon.forge.learning[0].detached);
    assert_eq!(app.dungeon.forge.learning[0].job, job);
    let source = original.join(".angel/dungeon/phrasebook.txt");
    let owner = angel_live::host::workspace_store::try_private_lock(&source).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.dungeon.forge.learning[0].returned {
        app.watch_learning();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let answer = app.dungeon.forge.learning[0].answer.clone().unwrap();
    app.watch_learning();
    assert_eq!(app.dungeon.forge.learning[0].answer.as_ref(), Some(&answer));
    assert_eq!(app.dungeon.forge.learning[0].job, job);
    assert!(
        !source.exists(),
        "locked original source must retain its answer for retry"
    );
    drop(owner);
    app.watch_learning();
    assert!(app.dungeon.forge.learning.is_empty());
    let published = crate::drive::together_shooter::phrasebook::read(
        &std::fs::read_to_string(&source).unwrap(),
    );
    assert_eq!(published, vec![answer]);
    assert!(!replacement.join(".angel/dungeon/phrasebook.txt").exists());
    let run = app.dungeon.shooter.as_ref().unwrap();
    assert!(run.lamp.wishing.is_empty());
    assert!(run.makers.is_empty());
    assert!(run.ladders.is_empty());
    assert!(run.book.get("wish-detached-grace-1").is_none());
    assert_eq!(run.players[&1].spell_cooldowns[0], 71);
    assert!(run.cues.iter().any(|cue| cue == "already issued"));
    assert_eq!(app.dungeon.forge.jobs, 1);
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &runners,
        app.dungeon.forge.grants.runners.as_ref().unwrap()
    ));
    drop(app);
    drop(runners);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_workspace_reset_preserves_running_wish_until_original_source_publication() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-wish-real-cd-{}", std::process::id()));
    let original = root.join("original");
    let replacement = root.join("replacement");
    let home = root.join("home");
    for path in [&original, &replacement, &home] {
        std::fs::create_dir_all(path).unwrap();
    }
    let _home = crate::tests::TestEnvGuard::set("HOME", home.to_str().unwrap());
    let _mcp = crate::tests::TestEnvGuard::set(
        "ANGEL_MCP_CONFIG",
        root.join("absent-mcp.json").to_str().unwrap(),
    );
    let _workspace = crate::tests::TestEnvGuard::unset("ANGEL_WORKSPACE");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(original.clone());
    app.dungeon
        .shooter
        .set(Some(crate::drive::together_shooter::Run::new(
            0,
            9,
            Some("Guest"),
        )));
    let club = crate::drive::together_grants::Scripted::new(
        &[
            "wish reset-grace | Reset Grace\nsay reset grace\ntier damage 10 :: Original workspace answer\n",
        ],
        Duration::from_secs(1),
    );
    app.dungeon.forge.grants.oracle = Some(club.clone());
    app.teach_spoken(vec![(1, "reset grace".into(), 1, Default::default())]);
    let job = app.dungeon.forge.learning[0].job;
    let runners = app.dungeon.forge.grants.runners.clone().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while club.inside.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.dungeon.forge.learning[0].returned);
    app.dungeon.forge.draft = Some(ReforgeDraft::new(1, true));
    app.dungeon.forge.cards_seen = Some(Vec::new());
    let changed = app.change_workspace(replacement.to_str());
    assert!(changed.starts_with("working directory →"), "{changed}");
    assert_eq!(app.tools.current_workspace(), replacement.as_path());
    assert!(app.dungeon.shooter.is_none());
    assert!(app.dungeon.forge.learning[0].detached);
    assert_eq!(app.dungeon.forge.learning[0].job, job);
    assert_eq!(app.dungeon.forge.jobs, 1);
    assert!(app.dungeon.forge.draft.is_none());
    assert!(app.dungeon.forge.cards_seen.is_none());
    assert!(app.dungeon.forge.spoken.is_empty());
    assert!(app.dungeon.forge.wish_session.is_none());
    assert!(std::sync::Arc::ptr_eq(
        &runners,
        app.dungeon.forge.grants.runners.as_ref().unwrap()
    ));
    // Exercise the actual frame pump even though /cd restored no scene.
    while !app.dungeon.forge.learning.is_empty() {
        app.advance_dungeon_guest();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let text = std::fs::read_to_string(original.join(".angel/dungeon/phrasebook.txt")).unwrap();
    let source = crate::drive::together_shooter::phrasebook::read(&text);
    assert_eq!(source.len(), 1);
    assert_eq!(source[0].id, "reset-grace");
    assert_eq!(source[0].tiers[0].1, "Original workspace answer");
    assert!(!replacement.join(".angel/dungeon/phrasebook.txt").exists());
    assert!(app.dungeon.shooter.is_none());
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    assert_eq!(app.dungeon.forge.jobs, 1);
    drop(app);
    drop(runners);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn detached_source_conflict_retains_exact_answer_without_overwriting_original_library() {
    let _guard = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!("angel-detached-conflict-{}", std::process::id()));
    let source = root.join(".angel/dungeon/phrasebook.txt");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let answer = crate::drive::together_shooter::phrasebook::read(
        "wish detached-kept | Detached Kept\nsay detached kept\ntier damage 10 :: Captured\n",
    )
    .pop()
    .unwrap();
    let mut lesson = Lesson::new(1, "detached kept".into());
    lesson.detached = true;
    lesson.returned = true;
    lesson.taught = true;
    lesson.job = Some(77);
    lesson.authored = true;
    lesson.workspace = Some(root.clone());
    lesson.answer = Some(answer.clone());
    app.dungeon.forge.learning.push(lesson);
    for conflict in [
        "wish detached-kept | Operator Edit\nsay operator\ntier damage 99 :: Edited\n",
        "wish detached-kept | Partial operator edit\n",
    ] {
        std::fs::write(&source, conflict).unwrap();
        app.drain_detached_lessons();
        assert_eq!(std::fs::read_to_string(&source).unwrap(), conflict);
        assert_eq!(app.dungeon.forge.learning[0].answer.as_ref(), Some(&answer));
        assert_eq!(app.dungeon.forge.learning[0].job, Some(77));
    }
    std::fs::remove_file(&source).unwrap();
    app.drain_detached_lessons();
    assert!(app.dungeon.forge.learning.is_empty());
    assert_eq!(
        crate::drive::together_shooter::phrasebook::read(
            &std::fs::read_to_string(&source).unwrap()
        ),
        vec![answer]
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn replacement_journal_with_same_raid_and_seats_retires_all_old_metadata() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let mut run = crate::drive::together_shooter::Run::new(0, 9, Some("Guest"));
    run.players
        .get_mut(&1)
        .unwrap()
        .deck
        .push("wish-captured-1".into());
    app.dungeon.shooter.set(Some(run.clone()));
    assert!(app.sync_wish_session());
    let original_session = app.dungeon.shooter.wish_session();
    let mut book = crate::drive::together_shooter::phrasebook::read(
        "wish captured | Captured\nsay captured\ntier damage 10 :: Original\n",
    );
    book[0].learned = true;
    let mut event = angel_delve::rules::speculative::Event::ladders(&run, &book);
    event.lamp(&run, 1, run.seat_epoch(1), "old words", true);
    event.maker(&run, 1, run.seat_epoch(1), "captured", "Old maker");
    let pending = app.dungeon.shooter.capture_speculative(event).unwrap();
    app.dungeon.forge.speculative = Some(pending.clone());
    app.dungeon.forge.ladders = Some(pending.clone());
    let mut lesson = Lesson::new(1, "old words".into());
    lesson.job = Some(77);
    lesson.taught = true;
    lesson.answer = book.pop();
    lesson.metadata = Some(pending.clone());
    app.dungeon.forge.learning.push(lesson);
    app.dungeon.shooter.set(Some(run.clone()));
    assert_ne!(app.dungeon.shooter.wish_session(), original_session);
    // Test the lock-level delivery guard independently of App coordination.
    app.dungeon.shooter.deliver_speculative(&pending).unwrap();
    app.teach_next();
    let current = app.dungeon.shooter.as_ref().unwrap();
    assert_eq!(current.raid_id, run.raid_id);
    assert_eq!(current.seat_epochs, run.seat_epochs);
    assert!(current.lamp.wishing.is_empty());
    assert!(current.makers.is_empty());
    assert!(current.ladders.is_empty());
    assert!(app.dungeon.forge.speculative.is_none());
    assert!(app.dungeon.forge.ladders.is_none());
    assert_eq!(app.dungeon.forge.learning.len(), 1);
    assert!(app.dungeon.forge.learning[0].detached);
    assert_eq!(app.dungeon.forge.learning[0].job, Some(77));
    assert!(app.dungeon.forge.learning[0].metadata.is_none());
    assert_eq!(app.dungeon.forge.jobs, 0);
}
