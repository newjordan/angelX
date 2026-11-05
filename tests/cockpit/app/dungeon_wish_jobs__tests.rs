use super::*;
use angel_delve::rules::{genie::Asked, phrasebook, wish_jobs};

const ANSWER: &str =
    "wish captured-grace | Captured Grace\nsay golden grace\ntier damage 10 :: Grace\n";
fn fixture(label: &str) -> (App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "angel-wish-{label}-{}-{}",
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
    crate::drive::together_files::refresh_phrasebook(&dir);
    app.realm.dungeon.forge.wish_session = app.realm.dungeon.shooter.wish_session();
    (app, dir)
}
fn queue(app: &mut App, player: u32, words: &str) -> wish_jobs::Claim {
    app.realm.dungeon.shooter.edit().unwrap().asked.push(Asked {
        cosmetic: false,
        cosmetic_name: String::new(),
        fairy: false,
        star: false,
        knight: player,
        words: words.into(),
        steps: 1,
        great: false,
        fills: true,
        making: None,
        target: Default::default(),
    });
    app.realm
        .dungeon
        .shooter
        .new_wish_claims(&mut None, "Original maker")
        .unwrap()
        .into_iter()
        .last()
        .unwrap()
}
fn returned(claim: wish_jobs::Claim) -> Lesson {
    Lesson {
        claim: Some(claim.clone()),
        pick: Pick::Chosen,
        taught: true,
        returned: true,
        answer: Some(phrasebook::read(ANSWER).remove(0)),
        authored: true,
        ..Lesson::new(claim.work.ask.knight, claim.work.ask.words.clone())
    }
}
fn poll(app: &mut App) {
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
}

#[test]
fn committed_wish_survives_checkpoint_failure_source_edit_and_restore_without_regrant() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("retained");
    let claim = queue(&mut app, 1, "golden grace");
    let mut lesson = returned(claim.clone());
    lesson.persisted = true;
    let path = app.realm.dungeon.save_path.take().unwrap();
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"blocked").unwrap();
    app.realm.dungeon.save_path = Some(blocked.join("session.json"));
    assert!(!app.deliver_wish_lesson(&mut lesson));
    let done = lesson.completed.clone().unwrap();
    assert!(matches!(&done.outcome, Outcome::Granted(name) if name == "Captured Grace I"));
    assert!(
        app.realm
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .sounds
            .contains(&angel_delve::rules::overclass::land_sound(0)),
        "tier I lands"
    );
    let card = app
        .dungeon
        .shooter
        .as_ref()
        .unwrap()
        .book
        .get("wish-captured-grace-1")
        .cloned();
    assert!(card.is_some());
    // Save terminal world+receipt, then simulate owner restart before source IO/ACK.
    app.realm.dungeon.save_path = Some(path);
    app.realm.dungeon.checkpoint(Instant::now(), true).unwrap();
    drop(app);
    let (mut app, unused) = fixture("restore-owner");
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(dir.clone());
    app.realm.dungeon.restore(&dir).unwrap();
    app.realm.dungeon.forge.wish_session = app.realm.dungeon.shooter.wish_session();
    app.teach_asked();
    assert_eq!(
        app.realm.dungeon.forge.jobs, 0,
        "terminal result never relaunches model"
    );
    let source = dir.join(".angel/dungeon/phrasebook.txt");
    std::fs::write(
        &source,
        "wish other-entry | Other Entry\nsay other entry\ntier damage 4 :: Other\n",
    )
    .unwrap();
    let sounds = app.realm.dungeon.shooter.as_ref().unwrap().sounds.clone();
    assert!(
        !sounds
            .iter()
            .any(|s| angel_delve::rules::overclass::LAND_SOUNDS.contains(s))
    );
    poll(&mut app);
    assert!(app.realm.dungeon.forge.learning.is_empty());
    assert!(
        app.realm
            .dungeon
            .shooter
            .new_wish_claims(&mut None, "")
            .unwrap()
            .is_empty()
    );
    assert_eq!(app.realm.dungeon.shooter.as_ref().unwrap().sounds, sounds);
    assert_eq!(
        app.realm
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .book
            .get("wish-captured-grace-1")
            .cloned(),
        card
    );
    let text = std::fs::read_to_string(source).unwrap();
    assert!(text.contains("Other Entry") && text.contains("Captured Grace"));
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(unused).unwrap();
}

#[test]
fn a_failed_chosen_job_refunds_instead_of_adopting_a_matching_book_entry() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("cancel");
    let claim = queue(&mut app, 1, "a firebolt");
    let before = app.realm.dungeon.shooter.as_ref().unwrap().players[&1].wishes;
    let mut lesson = returned(claim);
    lesson.answer = None;
    lesson.authored = false;
    lesson.failed = Some("too many in flight".into());
    app.realm.dungeon.forge.learning.push(lesson);
    poll(&mut app);
    assert!(app.realm.dungeon.forge.learning.is_empty());
    let run = app.realm.dungeon.shooter.as_ref().unwrap();
    assert_eq!(run.players[&1].wishes, before + 1);
    assert!(
        !run.players[&1]
            .deck
            .iter()
            .any(|id| id.starts_with("wish-firebolt"))
    );
    assert!(app.realm.dungeon.notice.contains("too many in flight"));
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn speculative_job_is_promoted_once_but_never_adopted_by_a_reused_seat() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("promotion");
    let club = crate::drive::together_grants::Scripted::new(&[ANSWER], Duration::from_millis(200));
    app.realm.dungeon.forge.grants.oracle =
        Some(crate::drive::realm_services::oracle(club.clone()));
    app.teach_spoken(vec![(2, "golden grace".into(), 1, Default::default())]);
    let job = app.realm.dungeon.forge.learning[0].job;
    let runners = app.realm.dungeon.forge.grants.runners.clone().unwrap();
    let claim = queue(&mut app, 2, "golden grace");
    app.teach_claim(claim);
    assert_eq!(app.realm.dungeon.forge.learning.len(), 1);
    assert_eq!(app.realm.dungeon.forge.learning[0].job, job);
    assert!(std::sync::Arc::ptr_eq(
        &runners,
        app.realm.dungeon.forge.grants.runners.as_ref().unwrap()
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.realm.dungeon.forge.learning.is_empty() {
        poll(&mut app);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    app.teach_spoken(vec![(
        2,
        "different golden grace".into(),
        2,
        Default::default(),
    )]);
    let old_job = app.realm.dungeon.forge.learning[0].job;
    {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        run.leave(2);
        run.join_seat(2, "Guest");
    }
    let claim = queue(&mut app, 2, "different golden grace");
    app.teach_claim(claim);
    let chosen = app
        .dungeon
        .forge
        .learning
        .iter()
        .find(|l| l.pick == Pick::Chosen)
        .unwrap();
    assert_ne!(
        chosen.job, old_job,
        "new occupant cannot adopt prior model request"
    );
    assert!(std::sync::Arc::ptr_eq(
        &runners,
        app.realm.dungeon.forge.grants.runners.as_ref().unwrap()
    ));
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn simultaneous_same_name_results_share_one_captured_definition_and_id() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("same-name");
    let club =
        crate::drive::together_grants::Scripted::new(&[ANSWER, ANSWER], Duration::from_millis(20));
    app.realm.dungeon.forge.grants.oracle =
        Some(crate::drive::realm_services::oracle(club.clone()));
    for player in [1, 2] {
        app.realm
            .dungeon
            .forge
            .learning
            .push(Lesson::new(player, "golden grace".into()));
    }
    app.teach_next();
    let deadline = Instant::now() + Duration::from_secs(5);
    // The model counter drops before its runner queues the completed answer.
    // Wait for both observable lessons, including separate drain batches.
    while app
        .realm
        .dungeon
        .forge
        .learning
        .iter()
        .any(|lesson| !lesson.returned)
    {
        app.take_grants();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let lessons = &app.realm.dungeon.forge.learning;
    assert_eq!(lessons[0].answer, lessons[1].answer);
    assert!(lessons.iter().all(|l| l.authored && l.returned));
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn admission_checkpoint_failure_delays_model_launch_and_retries_original_claim_once() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("admission");
    let club = crate::drive::together_grants::Scripted::new(&[ANSWER], Duration::ZERO);
    app.realm.dungeon.forge.grants.oracle =
        Some(crate::drive::realm_services::oracle(club.clone()));
    let claim = queue(&mut app, 1, "golden grace");
    app.teach_claim(claim.clone());
    let path = app.realm.dungeon.save_path.take().unwrap();
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"blocked").unwrap();
    app.realm.dungeon.save_path = Some(blocked.join("session.json"));
    app.teach_next();
    assert_eq!(app.realm.dungeon.forge.jobs, 0);
    assert!(app.realm.dungeon.forge.grants.runners.is_none());
    assert!(club.asked.lock().unwrap().is_empty());
    assert_eq!(
        app.realm.dungeon.forge.learning[0].claim.as_ref(),
        Some(&claim)
    );
    app.realm.dungeon.save_path = Some(path);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.realm.dungeon.forge.learning.is_empty() {
        poll(&mut app);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.realm.dungeon.forge.jobs, 1);
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn source_lock_contention_retains_terminal_delivery_until_source_and_ack_are_durable() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("source-lock");
    let claim = queue(&mut app, 1, "golden grace");
    let mut lesson = returned(claim.clone());
    let source = dir.join(".angel/dungeon/phrasebook.txt");
    let owner = crate::platform::workspace_store::try_private_lock(&source).unwrap();
    assert!(!app.deliver_wish_lesson(&mut lesson));
    assert!(lesson.completed.is_some() && !lesson.acknowledged);
    assert!(
        app.realm
            .dungeon
            .shooter
            .wish_completion(&claim)
            .unwrap()
            .is_some()
    );
    let sounds = app.realm.dungeon.shooter.as_ref().unwrap().sounds.clone();
    drop(owner);
    assert!(app.deliver_wish_lesson(&mut lesson));
    assert!(lesson.acknowledged);
    assert_eq!(app.realm.dungeon.shooter.as_ref().unwrap().sounds, sounds);
    assert_eq!(
        std::fs::read_to_string(source)
            .unwrap()
            .matches("wish captured-grace |")
            .count(),
        1
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_conflicting_policy_answer_refunds_without_overwriting_the_accepted_source_or_card() {
    let _guard = crate::tests::env_lock();
    let (mut app, dir) = fixture("identity-conflict");
    let first = queue(&mut app, 1, "golden grace");
    let mut lesson = returned(first);
    assert!(app.deliver_wish_lesson(&mut lesson));
    let source = dir.join(".angel/dungeon/phrasebook.txt");
    let bytes = std::fs::read(&source).unwrap();
    let card = app
        .dungeon
        .shooter
        .as_ref()
        .unwrap()
        .book
        .get("wish-captured-grace-1")
        .unwrap()
        .clone();
    let second = queue(&mut app, 2, "golden grace");
    let mut lesson = returned(second);
    lesson.answer.as_mut().unwrap().name = "Different Grace".into();
    let before = app.realm.dungeon.shooter.as_ref().unwrap().players[&2].wishes;
    assert!(app.deliver_wish_lesson(&mut lesson));
    assert!(matches!(
        lesson.completed.as_ref().unwrap().outcome,
        Outcome::Conflict(_)
    ));
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    assert_eq!(
        app.realm
            .dungeon
            .shooter
            .as_ref()
            .unwrap()
            .book
            .get(&card.id),
        Some(&card)
    );
    assert_eq!(
        app.realm.dungeon.shooter.as_ref().unwrap().players[&2].wishes,
        before + 1
    );
    drop(app);
    std::fs::remove_dir_all(dir).unwrap();
}
