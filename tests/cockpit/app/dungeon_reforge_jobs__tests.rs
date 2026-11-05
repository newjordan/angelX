use super::*;
fn fixture(label: &str) -> (App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "angel-reforge-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(dir.clone());
    app.realm.dungeon.save_path = Some(dir.join(".angel/dungeon/session.json"));
    let mut run = crate::drive::together_shooter::Run::new(0, 9, Some("Guest"));
    run.descend_for_test();
    app.realm.dungeon.shooter.set(Some(run));
    (app, dir)
}
fn end_model(app: &mut App) {
    app.thinking = None;
    app.pending_turn = None;
    app.realm.dungeon.forge.forging.as_mut().unwrap().seen = true;
}
fn write_answer(app: &App) -> String {
    let work = app.realm.dungeon.forge.forging.as_ref().unwrap();
    let mut card = app
        .dungeon
        .shooter
        .as_ref()
        .unwrap()
        .book
        .cards
        .iter()
        .find(|c| c.kind == cards::Kind::Guard)
        .unwrap()
        .clone();
    card.name = "Silver Shield".into();
    std::fs::write(
        app.reforged_dir().join(format!("{}.card", work.id)),
        card.definition_file(),
    )
    .unwrap();
    work.id.clone()
}
#[test]
fn checkpoint_failure_retains_exact_reforge_answer_and_drains_without_a_new_model_turn() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("retained");
    app.request_reforge(1, Part::Defense, "a silver shield".into());
    assert!(app.realm.dungeon.forge.forging.as_ref().unwrap().launched);
    let id = write_answer(&app);
    end_model(&mut app);
    // Fail durable delivery after successful policy/equipment commit.
    let saved_path = app.realm.dungeon.save_path.take().unwrap();
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"not a directory").unwrap();
    app.realm.dungeon.save_path = Some(blocked.join("session.json"));
    app.watch_reforge();
    let held = app.realm.dungeon.forge.forging.as_ref().unwrap();
    assert!(held.completed.as_ref().unwrap().result.is_ok());
    assert!(
        app.realm.dungeon.notice.contains("retained"),
        "{}",
        app.realm.dungeon.notice
    );
    assert_eq!(
        app.realm.dungeon.shooter.as_ref().unwrap().players[&1]
            .guard_card
            .as_deref(),
        Some(id.as_str())
    );
    let sounds = app.realm.dungeon.shooter.as_ref().unwrap().sounds.clone();
    std::fs::write(
        app.reforged_dir().join(format!("{id}.card")),
        b"changed after the answer",
    )
    .unwrap();
    app.realm.dungeon.save_path = Some(saved_path);
    app.watch_reforge();
    assert!(app.realm.dungeon.forge.forging.is_none());
    assert!(
        app.realm
            .dungeon
            .shooter
            .pending_reforges()
            .unwrap()
            .is_empty()
    );
    assert_eq!(app.realm.dungeon.shooter.as_ref().unwrap().sounds, sounds);
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(
        app.realm.dungeon.notice.contains("Silver Shield"),
        "{}",
        app.realm.dungeon.notice
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn uncheckpointed_claim_waits_and_recovery_starts_the_original_model_request() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("admission");
    let original = app.realm.dungeon.save_path.take().unwrap();
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"blocked").unwrap();
    app.realm.dungeon.save_path = Some(blocked.join("session.json"));
    app.request_reforge(1, Part::Defense, "a silver shield".into());
    assert!(app.realm.dungeon.forge.forging.is_none());
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    let claims = app.realm.dungeon.shooter.pending_reforges().unwrap();
    assert_eq!(claims.len(), 1);
    app.realm.dungeon.save_path = Some(original);
    app.watch_reforge();
    assert_eq!(
        app.realm
            .dungeon
            .forge
            .forging
            .as_ref()
            .unwrap()
            .claim
            .as_ref(),
        Some(&claims[0])
    );
    assert!(app.realm.dungeon.forge.forging.as_ref().unwrap().persisted);
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn a_new_session_detaches_the_old_reforge_without_granting_its_file() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("session");
    app.request_reforge(1, Part::Defense, "shield".into());
    let id = write_answer(&app);
    end_model(&mut app);
    let mut replacement = crate::drive::together_shooter::Run::new(0, 9, None);
    replacement.descend_for_test();
    app.realm.dungeon.shooter.set(Some(replacement));
    app.watch_reforge();
    assert!(app.realm.dungeon.forge.forging.is_none());
    assert!(
        app.realm
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .book
            .get(&id)
            .is_none()
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_rejected_card_releases_the_reservation_without_another_model_turn() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("reject");
    app.request_reforge(1, Part::Defense, "shield".into());
    let id = app.realm.dungeon.forge.forging.as_ref().unwrap().id.clone();
    std::fs::write(
        app.reforged_dir().join(format!("{id}.card")),
        "name Bow\nkind arm\narm bow\n",
    )
    .unwrap();
    end_model(&mut app);
    app.watch_reforge();
    assert!(app.realm.dungeon.forge.forging.is_none());
    assert!(
        app.realm
            .dungeon
            .shooter
            .pending_reforges()
            .unwrap()
            .is_empty()
    );
    assert!(app.realm.dungeon.shooter.as_ref().unwrap().can_reforge(1));
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_restored_terminal_reforge_retires_without_rereading_the_file_or_starting_a_model() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("terminal");
    app.request_reforge(1, Part::Defense, "shield".into());
    let id = write_answer(&app);
    let claim = app
        .dungeon
        .forge
        .forging
        .as_ref()
        .unwrap()
        .claim
        .clone()
        .unwrap();
    let raw = app
        .read_forged(&app.reforged_dir().join(format!("{id}.card")))
        .unwrap();
    let input = cards::check(&id, &raw).unwrap().card;
    let done = app
        .dungeon
        .shooter
        .answer_reforge(&claim, input)
        .unwrap()
        .unwrap();
    assert!(done.result.is_ok());
    app.realm.dungeon.checkpoint(Instant::now(), true).unwrap();
    std::fs::write(
        app.reforged_dir().join(format!("{id}.card")),
        b"edited since commit",
    )
    .unwrap();
    drop(app);
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(dir.clone());
    app.realm.dungeon.restore(&dir).unwrap();
    app.watch_reforge();
    app.watch_reforge();
    assert!(app.realm.dungeon.forge.forging.is_none());
    assert!(
        app.realm
            .dungeon
            .shooter
            .pending_reforges()
            .unwrap()
            .is_empty()
    );
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(
        app.realm.dungeon.notice.contains("Silver Shield"),
        "{}",
        app.realm.dungeon.notice
    );
    assert!(
        app.realm
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .sounds
            .is_empty()
    );
    let published = std::fs::read_to_string(app.reforged_dir().join(format!("{id}.card"))).unwrap();
    assert_eq!(
        cards::check(&id, &published).unwrap().card,
        done.accepted.unwrap()
    );

    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_accepted_definition_publication_retains_the_terminal_receipt_until_recovery() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("source");
    app.request_reforge(1, Part::Defense, "shield".into());
    let id = write_answer(&app);
    let path = app.reforged_dir().join(format!("{id}.card"));
    let input = cards::check(&id, &app.read_forged(&path).unwrap())
        .unwrap()
        .card;
    end_model(&mut app);
    app.realm.dungeon.forge.forging.as_mut().unwrap().answer = Some(input);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    app.watch_reforge();
    let held = app.realm.dungeon.forge.forging.as_ref().unwrap();
    assert!(held.completed.as_ref().unwrap().result.is_ok());
    assert!(!held.acknowledged);
    assert_eq!(
        app.realm.dungeon.shooter.pending_reforges().unwrap().len(),
        1
    );
    let sounds = app.realm.dungeon.shooter.as_ref().unwrap().sounds.clone();
    std::fs::remove_dir(&path).unwrap();
    app.watch_reforge();
    assert!(app.realm.dungeon.forge.forging.is_none());
    assert!(
        app.realm
            .dungeon
            .shooter
            .pending_reforges()
            .unwrap()
            .is_empty()
    );
    assert_eq!(app.realm.dungeon.shooter.as_ref().unwrap().sounds, sounds);
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(cards::check(&id, &std::fs::read_to_string(&path).unwrap()).is_ok());
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
