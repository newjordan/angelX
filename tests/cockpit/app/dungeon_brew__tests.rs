use super::*;

#[test]
fn a_brew_brief_shows_both_spells_whole_and_every_name_taken() {
    let book = cards::Book::builtin();
    let (storm, hedge) = (book.get("cabbage-storm").unwrap(), book.get("hedge-maze").unwrap());
    let imposture = book.get("the-imposture").unwrap().clone();
    let taken = cauldron::taken(&book);
    let text = brew_brief(
        "Jordan",
        [(storm, None), (hedge, Some("It was granted from the wish “Hedge Maze”"))],
        std::slice::from_ref(&imposture),
        &taken,
        "Kitchen Garden",
        std::path::Path::new(".angel/dungeon/brewed/brewed-000001-f2-k1.card"),
        std::path::Path::new(".angel/dungeon/sheets"),
    );
    // Both cards whole: names, flavour, effects, art.
    for card in [storm, hedge] {
        assert!(text.contains(&card.file()), "{} is shown whole", card.id);
        assert!(text.contains(card.text.trim()));
        assert!(text.contains(&card.art[0]));
    }
    assert!(text.contains("It was granted from the wish “Hedge Maze”"));
    // What the knight holds besides, and the stand-in.
    assert!(text.contains(&imposture.name) && text.contains("Kitchen Garden"));
    // Every name taken: the book's spells, the pot's recipes.
    assert!(text.contains("Never reuse one of these names"));
    for name in ["Cabbage Storm", "Hedge Maze", "Hot Cabbages", "Steam Burst", "Twin Moons"] {
        assert!(text.contains(&format!("| {name} |")), "{name} is taken");
    }
    // A cabbage went in, so cabbages may come out.
    assert!(text.contains("A cabbage went in"));
    // The vocabulary, the budget, the inks, the card format.
    assert!(text.contains(SPELL_LINES) && text.contains(SUMMONS) && text.contains(INKS));
    assert!(text.contains(&cauldron::POWER_BUDGET.to_string()));
    assert!(text.contains(&cards::rules()));
    assert!(text.contains("brewed-000001-f2-k1.card"));
    // No cabbage in, none out.
    let ward = book
        .cards
        .iter()
        .find(|c| c.kind == cards::Kind::Spell && !c.effects.iter().any(|e| matches!(e, cards::Effect::Cabbages(_))))
        .unwrap();
    let text = brew_brief(
        "Jordan",
        [(hedge, None), (ward, None)],
        &[],
        &taken,
        "Herb Garden",
        std::path::Path::new("brewed.card"),
        std::path::Path::new("sheets"),
    );
    assert!(text.contains("No cabbage went in"));
}

/// A delve on floor two with knight 1 holding the Kitchen Garden stand-in
/// of a Cabbage Storm thrown in with a Hedge Maze, and the run's ask.
fn brewing(workspace: &std::path::Path) -> (App, cauldron::Brewing) {
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .expect("preview registry should be uniquely owned")
        .set_workspace(workspace.to_path_buf());
    app.realm.dungeon.save_path = Some(workspace.join(".angel/dungeon/session.json"));
    app.input = "/dungeon start".into();
    app.submit();
    let run = app.realm.dungeon.shooter.edit().unwrap();
    run.descend_for_test();
    let garden = cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden);
    let stand_in = cauldron::brewed_card(garden, cauldron::Tier::Common);
    let ask = cauldron::Brewing {
        knight: 1,
        raid: run.raid_id,
        depth: run.dungeon.depth,
        cards: ["cabbage-storm".into(), "hedge-maze".into()],
        placeholder: stand_in.id.clone(),
    };
    run.players.get_mut(&1).unwrap().spells[0] = Some(stand_in.id.clone());
    run.book.insert(stand_in);
    run.brewing.push(ask.clone());
    (app, ask)
}

/// angelX's brewing turn ends, having written `card` (or nothing).
fn turn_ends(app: &mut App, ask: &cauldron::Brewing, card: Option<&str>) {
    let result_id = app.realm.dungeon.forge.kettles.iter().find(|k| k.claim.work.ask.knight == ask.knight).unwrap().claim.work.result_id.clone();
    let path = app.brewed_dir().join(format!("{result_id}.card"));
    if let Some(card) = card {
        std::fs::write(&path, card).unwrap();
    }
    app.thinking = None;
    app.pending_turn = None;
    for kettle in &mut app.realm.dungeon.forge.kettles {
        kettle.seen = kettle.started.is_some();
    }
    app.watch_brewing();
}

fn workspace(what: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "delve-brew-{what}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const ORCHARD: &str = "name Frostbriar Orchard\nkind spell\nrarity rare\ntext Thorns, a crop and a cold wind.\ndrop 0\nchest 0\ncooldown 20\ncabbages 3\nhedge 1\nward 1\nart\n..fDDf..\n.fDEEDf.\nfDE44EDf\nfDE44EDf\n.fDEEDf.\n..fDDf..\n";

#[test]
fn a_brewed_answer_takes_the_stand_ins_place_while_the_game_runs() {
    let _guard = crate::tests::env_lock();
    let dir = workspace("take");
    let (mut app, ask) = brewing(&dir);
    app.thinking = None;
    app.pending_turn = None;
    app.watch_brewing();
    assert!(app.realm.dungeon.shooter.as_ref().unwrap().brewing.is_empty(), "the cockpit took the ask");
    assert_eq!(app.realm.dungeon.forge.kettles.len(), 1);
    assert!(app.realm.dungeon.forge.kettles[0].started.is_some(), "angelX is brewing");
    let result_id = app.realm.dungeon.forge.kettles[0].claim.work.result_id.clone();
    turn_ends(&mut app, &ask, Some(ORCHARD));
    // The first draft goes back for its review turn, a second look by the
    // model before the game installs it (`Kettle::reviewed`).
    assert!(app.realm.dungeon.forge.kettles[0].reviewed && app.realm.dungeon.forge.kettles[0].answer.is_none());
    turn_ends(&mut app, &ask, Some(ORCHARD));
    let run = app.realm.dungeon.shooter.as_ref().unwrap();
    assert_eq!(run.players[&1].spells[0].as_deref(), Some(result_id.as_str()), "{}", app.realm.dungeon.notice);
    assert_eq!(run.book.get(&result_id).unwrap().name, "Frostbriar Orchard");
    assert!(app.realm.dungeon.notice.contains("Frostbriar Orchard"), "{}", app.realm.dungeon.notice);
    assert!(app.realm.dungeon.forge.kettles.is_empty() && !app.realm.dungeon.forge.stirring);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rejected_brew_is_mended_once_and_then_leaves_the_stand_in() {
    let _guard = crate::tests::env_lock();
    let dir = workspace("mend");
    let (mut app, ask) = brewing(&dir);
    app.thinking = None;
    app.pending_turn = None;
    app.watch_brewing();
    // Over the power budget: turned away, and sent back to be mended.
    let binding = app.realm.dungeon.forge.kettles[0].claim.work.placeholder.id.clone();
    let result_id = app.realm.dungeon.forge.kettles[0].claim.work.result_id.clone();
    let greedy = ORCHARD.replace("cooldown 20", "cooldown 2");
    turn_ends(&mut app, &ask, Some(&greedy));
    // The first draft goes back for its review turn, a second look by the
    // model before the game installs it (`Kettle::reviewed`).
    turn_ends(&mut app, &ask, Some(&greedy));
    assert!(app.realm.dungeon.notice.contains("mending"), "{}", app.realm.dungeon.notice);
    let kettle = &app.realm.dungeon.forge.kettles[0];
    assert!(kettle.mending.as_ref().unwrap().iter().any(|e| e.contains("power")));
    assert!(kettle.started.is_some(), "the mending turn has begun");
    // Still turned away: the stand-in stays.
    turn_ends(&mut app, &ask, Some(&greedy));
    let run = app.realm.dungeon.shooter.as_ref().unwrap();
    assert_eq!(run.players[&1].spells[0].as_deref(), Some(binding.as_str()));
    assert!(run.book.get(&result_id).is_none());
    assert!(app.realm.dungeon.notice.contains("keeps Kitchen Garden"), "{}", app.realm.dungeon.notice);
    assert!(app.realm.dungeon.forge.kettles.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_uncertain_brew_checkpoint_retries_the_same_answer_without_a_second_grant_or_model_turn() {
    let _guard = crate::tests::env_lock();
    let dir = workspace("uncertain");
    let (mut app, ask) = brewing(&dir);
    app.thinking = None;
    app.pending_turn = None;
    app.watch_brewing();
    let claim = app.realm.dungeon.forge.kettles[0].claim.clone();
    let path = app.realm.dungeon.save_path.clone().unwrap();
    turn_ends(&mut app, &ask, Some(ORCHARD));
    // The first draft goes back for its review turn, a second look by the
    // model before the game installs it (`Kettle::reviewed`).
    crate::platform::workspace_store::storage_atomic_tests::fail_after_publish(path.clone());
    turn_ends(&mut app, &ask, Some(ORCHARD));
    assert!(
        app.realm.dungeon.forge.kettles[0]
            .completed
            .as_ref()
            .unwrap()
            .result
            .is_ok()
    );
    let before = app.realm.dungeon.shooter.as_ref().unwrap().sounds.clone();
    assert_eq!(
        before
            .iter()
            .filter(|sound| **sound == "wish_grant")
            .count(),
        1
    );
    let answer_path = app
        .brewed_dir()
        .join(format!("{}.card", claim.work.result_id));
    std::fs::write(
        &answer_path,
        ORCHARD.replace("Frostbriar Orchard", "Changed Orchard"),
    )
    .unwrap();
    app.watch_brewing();
    assert!(app.realm.dungeon.forge.kettles.is_empty());
    assert_eq!(app.realm.dungeon.shooter.as_ref().unwrap().sounds, before);
    assert_eq!(
        app.realm.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .book
            .get(&claim.work.result_id)
            .unwrap()
            .name,
        "Frostbriar Orchard"
    );
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(app.realm.dungeon.shooter.pending_brews().unwrap().is_empty());
    let raw = std::fs::read_to_string(&answer_path).unwrap();
    let published = cards::check(&claim.work.result_id, &raw).unwrap().card;
    assert_eq!(
        Some(&published),
        app.realm.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .book
            .get(&claim.work.result_id)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_full_receipt_queue_still_drains_existing_brews_before_admitting_new_work() {
    let _guard = crate::tests::env_lock();
    let dir = workspace("backpressure");
    let (mut app, ask) = brewing(&dir);
    app.thinking = None;
    app.pending_turn = None;
    app.watch_brewing();
    let first = app.realm.dungeon.forge.kettles[0].claim.clone();
    let mut filler = ask.clone();
    filler.knight = 2;
    app.realm.dungeon.shooter.as_mut().unwrap().join_seat(2, "Guest");
    for _ in 0..63 {
        {
            let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
            run.players.get_mut(&2).unwrap().spells[0] = Some(filler.placeholder.clone());
            run.brewing.push(filler.clone());
        }
        app.realm.dungeon.shooter.claim_brew().unwrap().unwrap();
    }
    {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        run.players.get_mut(&2).unwrap().spells[0] = Some(filler.placeholder.clone());
        run.brewing.push(filler);
    }
    assert_eq!(app.realm.dungeon.shooter.pending_brews().unwrap().len(), 64);
    turn_ends(&mut app, &ask, Some(ORCHARD));
    // The first draft goes back for its review turn, a second look by the
    // model before the game installs it (`Kettle::reviewed`).
    turn_ends(&mut app, &ask, Some(ORCHARD));
    let pending = app.realm.dungeon.shooter.pending_brews().unwrap();
    assert_eq!(pending.len(), 63);
    assert!(pending.iter().all(|claim| claim.request != first.request));
    assert_eq!(app.realm.dungeon.shooter.as_ref().unwrap().brewing.len(), 1);
    assert_eq!(
        app.realm.dungeon
            .shooter
            .as_ref()
            .unwrap()
            .book
            .get(&first.work.result_id)
            .unwrap()
            .name,
        "Frostbriar Orchard"
    );
    let _ = std::fs::remove_dir_all(dir);
}
