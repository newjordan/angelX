use super::*;

fn mirror_game() -> App {
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start Mirror Guest".into();
    app.submit();
    app.dungeon.key_releases = true;
    app
}

#[test]
fn couch_creator_routes_p1_arrows_and_p2_ijkl_without_combat() {
    let _guard = crate::tests::env_lock();
    let mut app = mirror_game();
    {
        let run = app.dungeon.shooter.edit().unwrap();
        run.players.get_mut(&1).unwrap().mirror.open = true;
        run.players.get_mut(&2).unwrap().mirror.open = true;
    }
    for code in [
        KeyCode::Down,
        KeyCode::Char('e'),
        KeyCode::Char('f'),
        KeyCode::Char('i'),
        KeyCode::Char('\\'),
        KeyCode::Enter,
    ] {
        app.dungeon.hold(code, false);
    }
    let inputs = app.dungeon.local_inputs(Instant::now());
    assert_eq!(
        inputs[&1],
        ShooterInput {
            move_y: 1,
            interact: true,
            ..Default::default()
        }
    );
    assert_eq!(
        inputs[&2],
        ShooterInput {
            move_y: -1,
            interact: true,
            ..Default::default()
        }
    );

    // If only P2's creator is up, the arrow keys navigate it, never aim P1.
    app.dungeon
        .shooter
        .edit()
        .unwrap()
        .players
        .get_mut(&1)
        .unwrap()
        .mirror
        .open = false;
    app.dungeon.held.clear();
    app.dungeon.hold(KeyCode::Left, false);
    app.dungeon.hold(KeyCode::Delete, false);
    let inputs = app.dungeon.local_inputs(Instant::now());
    assert_eq!((inputs[&1].aim_x, inputs[&1].aim_y), (0, 0));
    assert_eq!(
        inputs[&2],
        ShooterInput {
            move_x: -1,
            leave_held: true,
            ..Default::default()
        }
    );
}

#[test]
fn creator_keys_preempt_ballots_escape_and_activity_menus() {
    let _guard = crate::tests::env_lock();
    let mut app = mirror_game();
    app.dungeon
        .shooter
        .edit()
        .unwrap()
        .players
        .get_mut(&1)
        .unwrap()
        .mirror
        .open = true;
    app.dungeon.activity_menu = Some(1);
    let key = |code| event::KeyEvent::new(code, KeyModifiers::NONE);
    assert!(app.activity_key(key(KeyCode::Esc)));
    assert!(
        app.dungeon.expanded,
        "Esc closes the creator, never the terminal game"
    );
    assert!(app.dungeon.activity_menu.is_none());
    let input = app.dungeon.local_inputs(Instant::now())[&1];
    assert_eq!(
        input,
        ShooterInput {
            leave_held: true,
            ..Default::default()
        }
    );
    assert!(app.activity_key(key(KeyCode::Char('v'))));
    assert!(!app.dungeon.shooter.as_ref().unwrap().playtest.open(1));
}

#[test]
fn creator_name_uses_the_existing_text_draft_without_spending_a_wish() {
    let _guard = crate::tests::env_lock();
    let mut app = mirror_game();
    let wishes = {
        let run = app.dungeon.shooter.edit().unwrap();
        // The name row answers only a knight at the glass (`creator::near_mirror`).
        run.walk_alehouse();
        let hero = run.players.get_mut(&1).unwrap();
        (hero.x, hero.y) = (
            crate::drive::together_shooter::alehouse::MIRROR_STAND.0 * crate::drive::together_shooter::TILE_UNITS,
            crate::drive::together_shooter::alehouse::MIRROR_STAND.1 * crate::drive::together_shooter::TILE_UNITS,
        );
        hero.mirror.open = true;
        hero.mirror.row = 9;
        hero.wishes
    };
    let key = |code| event::KeyEvent::new(code, KeyModifiers::NONE);
    assert_eq!(app.mirror_key(key(KeyCode::Char('t'))), Some(true));
    assert_eq!(app.dungeon.forge.draft.as_ref().unwrap().player, 1);
    for c in "Nova".chars() {
        assert_eq!(app.mirror_key(key(KeyCode::Char(c))), Some(true));
    }
    assert_eq!(app.mirror_key(key(KeyCode::Enter)), Some(true));
    assert!(app.dungeon.forge.draft.is_none());
    let hero = &app.dungeon.shooter.as_ref().unwrap().players[&1];
    assert_eq!(hero.wishes, wishes);
    assert!(hero.offer.is_none(), "a name never opens the wish scroll");
    assert_eq!(app.dungeon.notice, "Mirror name saved.");
}
