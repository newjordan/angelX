use super::*;
use ratatui::{Terminal, backend::TestBackend};

fn game() -> App {
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    app
}

fn paint(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, app))
        .unwrap();
    if width == 0 {
        return String::new();
    }
    terminal
        .backend()
        .buffer()
        .content
        .chunks(width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(event::KeyEvent::new(code, KeyModifiers::NONE));
}

fn run(app: &App) -> &crate::drive::together_shooter::Run {
    app.dungeon.shooter.as_ref().unwrap()
}

fn advance(app: &mut App, ticks: u32) {
    let now = app.dungeon.clock.unwrap_or_else(std::time::Instant::now);
    app.advance_shooter_at(now);
    for tick in 1..=ticks {
        app.advance_shooter_at(now + std::time::Duration::from_nanos(33_333_333) * tick);
    }
}

#[test]
fn dungeon_starts_a_realtime_arena_with_visible_controls_without_a_model_turn() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    assert_eq!(run(&app).players[&2].name, "Test Guest");
    assert!(app.dungeon.expanded);
    for (width, height) in [(80, 24), (120, 40), (160, 48)] {
        let text = paint(&mut app, width, height);
        for label in [
            "DUNGEON DELVE",
            "Floor 1/3",
            "P1",
            "P2",
            "HP 100",
            "F6: game",
            "Esc: composer",
            "arrows aim/fire",
        ] {
            assert!(text.contains(label), "{label} at {width}×{height}:\n{text}");
        }
        assert!(app.dungeon.controls_visible && app.needs_fast_tick());
        assert!(!app.world_pane_visible && app.world_buttons.is_empty());
    }
    assert!(!input::parse("/dungeon start").unwrap().needs_idle());
    app.dungeon_command(Some("off"));
    app.dungeon_command(Some("start"));
    assert_eq!(run(&app).players.len(), 1, "no guest name starts solo");
}

#[test]
fn dungeon_held_movement_and_aim_do_not_wait_for_another_player_or_touch_composer() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    app.dungeon.key_releases = true;
    let messages = app.messages.len();
    let (x1, x2) = (run(&app).players[&1].x, run(&app).players[&2].x);
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Up);
    advance(&mut app, 12);
    assert_eq!(run(&app).tick, 12);
    assert!((run(&app).players[&1].x - x1 - 5.2).abs() < 0.001);
    assert!((run(&app).players[&2].x - x2 - 5.2).abs() < 0.001);
    assert!(run(&app).projectiles.iter().any(|p| !p.hostile));
    assert_eq!(app.messages.len(), messages);
    assert!(app.input.is_empty() && app.thinking.is_none() && app.pending_turn.is_none());
}

#[test]
fn dungeon_routes_repeats_and_releases_without_releasing_into_a_modal() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    let input = crate::ui::input::TerminalInput::spawn(|wait| {
        std::thread::sleep(wait);
        Ok(None)
    })
    .unwrap();
    let key = |kind| {
        event::Event::Key(event::KeyEvent::new_with_kind(
            KeyCode::Char('d'),
            KeyModifiers::NONE,
            kind,
        ))
    };
    crate::apply_terminal_input(&mut app, &input, Some(key(event::KeyEventKind::Repeat))).unwrap();
    advance(&mut app, 1);
    let x = run(&app).players[&1].x;
    crate::apply_terminal_input(&mut app, &input, Some(key(event::KeyEventKind::Release))).unwrap();
    assert!(app.dungeon.key_releases);
    advance(&mut app, 6);
    assert_eq!(run(&app).players[&1].x, x);
    let (reply, receiver) = mpsc::channel();
    app.pending_approval = Some(PendingApproval {
        prompt: "fixture".into(),
        scope_label: None,
        reply,
    });
    crate::apply_terminal_input(
        &mut app,
        &input,
        Some(event::Event::Key(event::KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            event::KeyEventKind::Release,
        ))),
    )
    .unwrap();
    assert!(receiver.try_recv().is_err() && app.pending_approval.is_some());
}

#[test]
fn dungeon_alternate_shift_keycodes_dash_and_release_the_same_held_direction() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    let x = run(&app).players[&1].x;
    // Crossterm uses the alternate uppercase glyph and removes Shift.
    press(&mut app, KeyCode::Char('D'));
    advance(&mut app, 1);
    assert!(run(&app).players[&1].x - x > 0.6, "Shift+D still rolls");
    assert!(run(&app).players[&1].dash_cooldown > 0);
    app.dungeon_key(event::KeyEvent::new_with_kind(
        KeyCode::Char('D'),
        KeyModifiers::NONE,
        event::KeyEventKind::Release,
    ));
    assert!(app.dungeon.held.is_empty());
    assert!(app.dungeon_keyboard_active());
    app.collapse_dungeon();
    assert!(!app.dungeon_keyboard_active());
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('!'));
    assert_eq!(app.input, "A!");
}

#[test]
fn dungeon_legacy_key_leases_expire_and_long_stalls_have_bounded_catchup() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    press(&mut app, KeyCode::Char('d'));
    advance(&mut app, 3);
    let x = run(&app).players[&1].x;
    let now = app.dungeon.clock.unwrap();
    app.advance_shooter_at(now + std::time::Duration::from_secs(30));
    assert_eq!(
        run(&app).tick,
        7,
        "stalls run at most four simulation steps"
    );
    assert_eq!(
        run(&app).players[&1].x,
        x,
        "legacy inputs expire instead of sticking"
    );
}

#[test]
fn dungeon_escape_paste_focus_loss_and_modals_pause_and_clear_input() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    app.dungeon.key_releases = true;
    press(&mut app, KeyCode::Char('d'));
    advance(&mut app, 3);
    let tick = run(&app).tick;
    press(&mut app, KeyCode::Esc);
    advance(&mut app, 30);
    assert_eq!(run(&app).tick, tick);
    assert!(app.dungeon.held.is_empty());
    press(&mut app, KeyCode::Char('f'));
    assert_eq!(app.input, "f");
    press(&mut app, KeyCode::F(6));
    assert!(app.dungeon.expanded);
    assert!(app.input.is_empty());
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.input, "f");
    app.input.clear();
    app.cursor = 0;
    press(&mut app, KeyCode::F(6));
    press(&mut app, KeyCode::Enter); // the Delve's menu: continue
    paint(&mut app, 80, 24);
    let x = run(&app).players[&1].x;
    advance(&mut app, 2);
    assert_eq!(run(&app).players[&1].x, x);
    press(&mut app, KeyCode::Char('d'));
    app.set_terminal_focused(false);
    let tick = run(&app).tick;
    advance(&mut app, 8);
    assert_eq!(run(&app).tick, tick);
    assert!(app.dungeon.held.is_empty());
    app.set_terminal_focused(true);
    let (reply, receiver) = mpsc::channel();
    app.pending_approval = Some(PendingApproval {
        prompt: "fixture approval".into(),
        scope_label: None,
        reply,
    });
    assert!(paint(&mut app, 80, 24).contains("fixture approval"));
    advance(&mut app, 8);
    assert_eq!(run(&app).tick, tick);
    press(&mut app, KeyCode::Enter);
    assert!(receiver.try_recv().is_ok());
    paint(&mut app, 80, 24);
    app.on_paste("draft text");
    assert!(!app.dungeon.expanded);
    assert_eq!(app.input, "draft text");
    assert!(app.dungeon.held.is_empty());
}

#[test]
fn dungeon_resize_pauses_hidden_controls_but_preserves_escape() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    let input = crate::ui::input::TerminalInput::spawn(|wait| {
        std::thread::sleep(wait);
        Ok(None)
    })
    .unwrap();
    press(&mut app, KeyCode::Char('d'));
    crate::apply_terminal_input(&mut app, &input, Some(event::Event::Resize(40, 12))).unwrap();
    assert!(app.dungeon.held.is_empty());
    for (width, height) in [(40, 12), (80, 10), (0, 0)] {
        paint(&mut app, width, height);
        press(&mut app, KeyCode::Char('d'));
        advance(&mut app, 30);
        assert_eq!(run(&app).tick, 0);
        assert!(app.dungeon.held.is_empty());
    }
    press(&mut app, KeyCode::Esc);
    assert!(!app.dungeon.expanded);
}

#[test]
fn dungeon_restart_requires_finished_run_and_retains_players_with_new_generation() {
    use crate::drive::together_shooter::Phase;
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    let raid = run(&app).raid_id;
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(run(&app).raid_id, raid);
    assert!(app.dungeon.notice.contains("still active"));
    app.dungeon.shooter.as_mut().unwrap().phase = Phase::Wiped;
    press(&mut app, KeyCode::Char('r'));
    assert!(run(&app).raid_id > raid);
    assert_eq!(run(&app).phase, Phase::Fighting);
    assert_eq!(run(&app).players[&2].name, "Test Guest");
    assert_eq!(run(&app).players[&1].hp, 100);
}

#[test]
fn dungeon_legacy_forge_commands_do_not_discard_the_independent_arena() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.collapse_dungeon();
    let raid = run(&app).raid_id;
    for command in ["/together status", "/together off"] {
        app.input = command.into();
        app.cursor = app.input.len();
        app.submit();
        assert_eq!(run(&app).raid_id, raid);
    }
}

/// One request to the hosted guest listener, as an invited friend's angelX makes it.
fn guest_request(app: &App, method: &str, path: &str, body: &str) -> (u16, Vec<u8>) {
    use std::io::{Read, Write};
    let invitation = app.dungeon.guest.as_ref().unwrap().invitation_url();
    let (base, token) = invitation.split_once('#').unwrap();
    let address = base.strip_prefix("http://").unwrap().trim_end_matches('/');
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let status = String::from_utf8_lossy(&response[..split])
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    (status, response[split + 4..].to_vec())
}

fn guest_state(app: &App) -> serde_json::Value {
    let (status, body) = guest_request(app, "GET", "/state", "");
    assert_eq!(status, 200);
    serde_json::from_slice(&body).unwrap()
}

fn guest_walk(app: &App, sequence: u64, move_x: i8) -> u16 {
    let input = crate::drive::together_shooter::Input {
        move_x,
        ..Default::default()
    };
    let body =
        serde_json::json!({"sequence": sequence, "raid_id": run(app).raid_id, "input": input});
    guest_request(app, "POST", "/shooter/input", &body.to_string()).0
}

/// A delve hosted on an ephemeral loopback port, its surface painted and live.
fn hosted() -> (App, String) {
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = app.dungeon_command(Some("host Test Guest 127.0.0.1:0"));
    press(&mut app, KeyCode::Enter); // the Delve's menu: continue
    paint(&mut app, 80, 24);
    advance(&mut app, 0);
    (app, message)
}

#[test]
fn dungeon_host_lets_a_friend_play_while_host_is_stone() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let (mut app, message) = hosted();
    assert!(
        message.contains("http://127.0.0.1:") && message.contains('#'),
        "{message}"
    );
    assert!(message.contains("only on this machine"), "{message}");
    assert_eq!(run(&app).players[&2].name, "Test Guest");
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(paint(&mut app, 120, 40).contains("Friends play from their own angelX"));
    let state = guest_state(&app);
    assert_eq!(
        (state["playable"].clone(), state["paused"].clone()),
        (true.into(), false.into())
    );
    assert_eq!(state["players"][1]["name"], "Test Guest");

    assert_eq!(guest_walk(&app, 1, 1), 202);
    let (x, host_x) = (run(&app).players[&2].x, run(&app).players[&1].x);
    advance(&mut app, 3);
    assert!(run(&app).players[&2].x > x, "guest input moves player 2");
    assert_eq!(run(&app).players[&1].x, host_x, "and only player 2");

    // The host's own rendering reaches a friend as a PNG.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while guest_state(&app)["frame_seq"].as_u64().unwrap() == 0 {
        assert!(std::time::Instant::now() < deadline, "no frame was painted");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let (status, png) = guest_request(&app, "GET", "/frame.png?seq=1", "");
    assert_eq!(status, 200);
    let picture = image::load_from_memory(&png).unwrap();
    assert_eq!(
        (picture.width(), picture.height()),
        (
            crate::drive::together_guest::FRAME_W,
            crate::drive::together_guest::FRAME_H
        )
    );

    // Esc stones the host, but the guest keeps playing.
    app.collapse_dungeon();
    let tick = run(&app).tick;
    advance(&mut app, 5);
    assert!(run(&app).tick > tick);
    assert!(run(&app).players[&1].stone);
    let state = guest_state(&app);
    assert_eq!(
        (state["paused"].clone(), state["playable"].clone()),
        (false.into(), true.into())
    );
    assert_eq!(guest_walk(&app, 2, 1), 202);
    assert_eq!(guest_walk(&app, 3, 0), 202);

    press(&mut app, KeyCode::F(6));
    press(&mut app, KeyCode::Enter); // the Delve's menu: continue
    paint(&mut app, 80, 24);
    advance(&mut app, 0);
    assert_eq!(guest_state(&app)["paused"], false);
    assert!(!run(&app).players[&1].stone);
    assert_eq!(
        (run(&app).players[&1].x, run(&app).players[&1].y),
        (run(&app).players[&2].x, run(&app).players[&2].y)
    );
    assert_eq!(guest_walk(&app, 4, -1), 202);

    // Controls queued for a finished delve never enter its restart.
    let raid = run(&app).raid_id;
    app.dungeon.shooter.as_mut().unwrap().phase = crate::drive::together_shooter::Phase::Won;
    press(&mut app, KeyCode::Char('r'));
    paint(&mut app, 80, 24);
    advance(&mut app, 2);
    assert!(app.dungeon.remote.is_empty());
    assert!(run(&app).raid_id > raid);
    assert_eq!(
        run(&app).players[&2].name,
        "Test Guest",
        "the guest stays for the next delve"
    );
    let stale = serde_json::json!({"sequence": 5, "raid_id": raid, "input": crate::drive::together_shooter::Input { move_x: 1, ..Default::default() }});
    assert_eq!(
        guest_request(&app, "POST", "/shooter/input", &stale.to_string()).0,
        409
    );
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
}

#[test]
fn dungeon_guest_held_input_expires_without_a_renewal() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let (mut app, _) = hosted();
    assert_eq!(guest_walk(&app, 1, 1), 202);
    advance(&mut app, 3);
    let x = run(&app).players[&2].x;
    // A dropped connection: no renewal for a full second.
    let now = app.dungeon.clock.unwrap();
    app.advance_shooter_at(now + std::time::Duration::from_secs(1));
    assert_eq!(
        run(&app).players[&2].x,
        x,
        "the knight stops once the lease lapses"
    );
    advance(&mut app, 6);
    assert_eq!(run(&app).players[&2].x, x);

    // A renewal walks again.
    app.clear_dungeon_controls();
    advance(&mut app, 0);
    assert_eq!(guest_walk(&app, 2, 1), 202);
    advance(&mut app, 3);
    assert!(run(&app).players[&2].x > x);
}

#[test]
fn dungeon_host_joins_an_open_entrance_room_or_starts_fresh_and_kick_revokes() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    assert!(app.dungeon_command(Some("invite")).contains("No guest"));
    assert!(app.dungeon_command(Some("kick")).contains("No guest"));
    app.dungeon_command(Some("start"));
    let raid = run(&app).raid_id;
    assert_eq!(run(&app).players.len(), 1);

    let message = app.dungeon_command(Some("host Bob"));
    assert_eq!(
        run(&app).raid_id,
        raid,
        "the entrance-room delve gains player 2"
    );
    assert_eq!(run(&app).players[&2].name, "Bob");
    let invitation = app.dungeon.guest.as_ref().unwrap().invitation_url();
    assert!(message.contains(&invitation), "{message}");
    assert!(app.dungeon_command(Some("invite")).contains(&invitation));
    assert!(
        app.dungeon_command(Some("host Someone Else"))
            .contains(&invitation)
    );
    assert!(
        app.dungeon_command(Some("status"))
            .contains("friends invited")
    );
    assert!(app.dungeon_command(Some("help")).contains("/dungeon_host"));

    let address = app.dungeon.guest.as_ref().unwrap().address();
    let kicked = app.dungeon_command(Some("kick"));
    assert!(
        kicked.contains("Bob") && kicked.contains("no longer works"),
        "{kicked}"
    );
    assert!(app.dungeon.guest.is_none() && app.dungeon.remote.is_empty());
    assert_eq!(run(&app).raid_id, raid);
    assert!(!run(&app).players.contains_key(&2));
    assert!(
        std::net::TcpStream::connect(address).is_err(),
        "the old link is dead"
    );

    // Past the entrance, hosting starts a fresh delve with the guest.
    let shooter = app.dungeon.shooter.as_mut().unwrap();
    shooter.enter_for_test(1);
    assert!(!shooter.at_entrance());
    app.dungeon_command(Some("host Cleo 127.0.0.1:0"));
    assert!(run(&app).raid_id > raid);
    assert!(run(&app).at_entrance());
    assert_eq!(run(&app).players[&2].name, "Cleo");

    let address = app.dungeon.guest.as_ref().unwrap().address();
    assert!(app.dungeon_command(Some("off")).contains("revoked"));
    assert!(app.dungeon.guest.is_none() && app.dungeon.shooter.is_none());
    assert!(std::net::TcpStream::connect(address).is_err());
}

/// Offline screen for review: `ANGEL_DELVE_SCREEN=<dir> cargo test ... -- --ignored`
/// writes each cell's symbol and colours as JSON lines.
#[test]
#[ignore]
fn dungeon_write_delve_screens() {
    let Some(dir) = std::env::var_os("ANGEL_DELVE_SCREEN") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = crate::tests::env_lock();
    let hex = |c: ratatui::style::Color| match c {
        ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => String::new(),
    };
    let _backdrop = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in-process");
    for (name, width, height) in [
        ("delve_160x48", 160, 48),
        ("delve_80x24", 80, 24),
        ("delve_miniviz", 160, 48),
        ("delve_cards", 160, 48),
        ("delve_book", 160, 48),
        ("delve_realm", 160, 48),
        ("delve_intro", 160, 48),
        ("delve_sanctuary", 160, 48),
        ("delve_sanctuary1", 160, 48),
        ("delve_ledge", 160, 48),
        ("delve_wish_confirm", 160, 48),
        ("delve_wish_scroll", 160, 48),
        ("delve_wish_seal", 160, 48),
    ] {
        let mut app = game();
        let shooter = app.dungeon.shooter.as_mut().unwrap();
        let fight = shooter
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == crate::drive::together_shooter::RoomKind::Fight)
            .unwrap();
        shooter.enter_for_test(fight);
        for _ in 0..100 {
            for hero in shooter.players.values_mut() {
                hero.hp = hero.max_hp;
            }
            shooter.step(&std::collections::BTreeMap::new());
        }
        if name.starts_with("delve_cards") || name == "delve_book" || name == "delve_160x48" {
            let hero = shooter.players.get_mut(&1).unwrap();
            hero.hand = vec!["potion".into(), "thunder-scroll".into()];
            hero.deck = vec![
                "ember-quiver".into(),
                "ember-quiver".into(),
                "mail".into(),
                "twin-string".into(),
            ];
            hero.arm = Some("crossbow".into());
        }
        if name == "delve_sanctuary" || name == "delve_wish_confirm" {
            let run = app.dungeon.shooter.as_mut().unwrap();
            run.descend_for_test();
            app.dungeon.forge.draft = Some(crate::app::control::ReforgeDraft {
                player: 1,
                part: crate::drive::together_shooter::knights::Part::Defense,
                words: "a trip wall shield my buddy can shoot through, 3x wide, costs mana".into(),
                confirm: (name == "delve_wish_confirm").then_some(true),
                pick: None,
                known: None,
                sanctuary: true,
            });
        }
        if name == "delve_sanctuary1" {
            // The first floor's guardian has just fallen.
            let run = app.dungeon.shooter.as_mut().unwrap();
            let stairs = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == crate::drive::together_shooter::RoomKind::Stairs)
                .unwrap();
            run.enter_for_test(stairs);
            run.calm_for_test();
            for enemy in &mut run.enemies {
                enemy.hp = 0;
            }
            for _ in 0..20 {
                run.step(&std::collections::BTreeMap::new());
            }
        }
        if name == "delve_ledge" {
            let run = app.dungeon.shooter.as_mut().unwrap();
            let hall = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == crate::drive::together_shooter::RoomKind::Ledge)
                .unwrap();
            run.enter_for_test(hall);
            for _ in 0..40 {
                run.step(&std::collections::BTreeMap::new());
            }
        }
        if name == "delve_wish_scroll" || name == "delve_wish_seal" {
            let run = app.dungeon.shooter.as_mut().unwrap();
            run.calm_for_test();
            run.clear_for_test();
            let mut draft = crate::app::control::ReforgeDraft::new(1, false);
            draft.pick = Some("volley".into());
            if name == "delve_wish_seal" {
                draft.known = draft.pick.clone();
                draft.confirm = Some(true);
            }
            app.dungeon.forge.draft = Some(draft);
        }
        if name == "delve_intro" {
            app.open_intro();
            app.dungeon.intro = Some(crate::ui::viz::delve_intro_viz::Intro {
                knight: 3,
                ..app.dungeon.intro.take().unwrap()
            });
        }
        if name == "delve_realm" {
            use crate::drive::together_realm::{Spoil, Spoils, check};
            let realm = app.realm();
            let mut haul = Spoils::default();
            haul.add(Spoil::Bone, 9);
            haul.add(Spoil::Wax, 3);
            haul.add(Spoil::Gold, 420);
            haul.add(Spoil::Bond, 2);
            realm.bank("Jordan", &haul);
            let shrine = "name Wayside Shrine\nwords a little shrine for knights who fell\nby Sam\nnear chapel\nprice bone 8, wax 3\nart\n....hHHh....\n...hiHHih...\n...hi55ih...\n..hHi66iHh..\n..hiJJJJih..\n..hJJ77JJh..\n..hJ7887Jh..\n.GGGGGGGGGG.\n.GjjjjjjjjG.\n";
            let (draft, _) = check("shrine", shrine).unwrap();
            realm.draft(draft).unwrap();
            realm
                .ask("Sam", "a tavern with a dragon skull over the door")
                .unwrap();
            let hall =
                "name Hall of Dragonslayers\nprice scale 1, bond 6, gold 900\nart\nGGGG\nhHHh\n";
            let (draft, _) = check("hall", hall).unwrap();
            realm.draft(draft).unwrap();
            let shooter = app.dungeon.shooter.as_mut().unwrap();
            shooter
                .players
                .get_mut(&1)
                .unwrap()
                .carried
                .add(Spoil::Ore, 3);
            app.dungeon.cards_open = true;
            app.dungeon.cards_page = 1;
        }
        let _ = app.dungeon.shooter.as_mut().unwrap();
        if name == "delve_cards" || name == "delve_book" {
            app.dungeon.cards_open = true;
            app.dungeon.cards_sel = 2;
            app.dungeon.cards_page = usize::from(name == "delve_book");
        }
        if name == "delve_miniviz" {
            app.collapse_dungeon();
            app.input = "Make the sword swing heavier".into();
            app.cursor = app.input.len();
        }
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw::ui(frame, &mut app))
            .unwrap();
        let out: Vec<String> = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| serde_json::json!([cell.symbol(), hex(cell.fg), hex(cell.bg)]).to_string())
            .collect();
        std::fs::write(
            dir.join(format!("{name}.jsonl")),
            format!("{width} {height}\n{}", out.join("\n")),
        )
        .unwrap();
    }
    // A friend's angelX, joined to a host's delve over the loopback.
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = game();
    let message = host.dungeon_command(Some("host --2"));
    let snap = |app: &mut App, name: &str| {
        let (width, height) = (160, 48);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| crate::ui::draw::ui(frame, app))
            .unwrap();
        let out: Vec<String> = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| serde_json::json!([cell.symbol(), hex(cell.fg), hex(cell.bg)]).to_string())
            .collect();
        std::fs::write(
            dir.join(format!("{name}.jsonl")),
            format!("{width} {height}\n{}", out.join("\n")),
        )
        .unwrap();
    };
    snap(&mut host, "delve_hosted_menu");
    let line = message
        .lines()
        .find_map(|l| {
            l.split_once("friend 1:")
                .map(|(_, rest)| rest.trim().to_string())
        })
        .unwrap();
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.dungeon_command(Some(line.trim_start_matches("/dungeon ")));
    for _ in 0..200 {
        host.advance_dungeon_guest();
        let joined = friend.dungeon.joined.as_ref().unwrap();
        if (joined.frame().is_some() || joined.with_view(|_, _| ()).is_some())
            && joined
                .state()
                .is_some_and(|s| s["players"].as_array().is_some_and(|p| p.len() == 2))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    friend.dungeon.notice = "Matt joins the party as knight 2".into();
    let (width, height) = (160, 48);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut friend))
        .unwrap();
    let out: Vec<String> = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| serde_json::json!([cell.symbol(), hex(cell.fg), hex(cell.bg)]).to_string())
        .collect();
    std::fs::write(
        dir.join("delve_joined.jsonl"),
        format!("{width} {height}\n{}", out.join("\n")),
    )
    .unwrap();
    // The friend's own cards, from the host's book.
    host.dungeon
        .shooter
        .as_mut()
        .unwrap()
        .players
        .get_mut(&2)
        .unwrap()
        .hand = vec!["potion".into(), "thunder-scroll".into()];
    press(&mut friend, KeyCode::Tab);
    for _ in 0..200 {
        host.advance_dungeon_guest();
        let joined = friend.dungeon.joined.as_ref().unwrap();
        if joined.book().is_some()
            && joined.state().is_some_and(|s| {
                s["players"][1]["hand_ids"]
                    .as_array()
                    .is_some_and(|h| h.len() == 2)
            })
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    snap(&mut friend, "delve_joined_cards");
}

#[test]
fn dungeon_harness_strip_is_visible_without_taking_game_input() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    assert!(paint(&mut app, 80, 24).contains("Harness · ready · Esc to code"));
    let (sender, _receiver) = std::sync::mpsc::channel();
    app.pending_approval = Some(PendingApproval {
        prompt: "test".into(),
        scope_label: None,
        reply: sender,
    });
    assert!(app.dungeon_harness_status().contains("needs you"));
}

#[test]
fn dungeon_native_miniviz_expand_and_escape_preserve_composer_draft() {
    let _guard = crate::tests::env_lock();
    let _backdrop = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in-process");
    let mut app = game();
    paint(&mut app, 120, 40);
    press(&mut app, KeyCode::Esc);
    app.input = "Make the sword swing heavier".into();
    app.cursor = 9;
    let raid = run(&app).raid_id;
    press(&mut app, KeyCode::F(4));
    assert!(
        app.dungeon.expanded && app.dungeon.intro.is_some(),
        "the way in is the Delve's menu"
    );
    assert!(app.input.is_empty());
    let menu = paint(&mut app, 120, 40);
    assert!(menu.contains("CONTINUE THE DELVE"), "{menu}");
    press(&mut app, KeyCode::Enter);
    assert!(paint(&mut app, 120, 40).contains("DUNGEON DELVE"));
    assert!(
        app.dungeon.guest.is_none(),
        "native play needs no browser server"
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.input, "Make the sword swing heavier");
    assert_eq!(app.cursor, 9);
    assert_eq!(run(&app).raid_id, raid);
    let tick = run(&app).tick;
    advance(&mut app, 10);
    assert_eq!(run(&app).tick, tick, "solo pauses while coding");
    let text = paint(&mut app, 160, 48);
    assert!(text.contains("Make the sword swing heavier"), "{text}");
    assert!(!app.dungeon.controls_visible);
    assert!(
        app.world.delve_called && app.world.delve_lit,
        "the knight waits at the lit gate"
    );
    let rect = app
        .panes
        .rect_of(crate::ui::mouse::PaneId::Artifacts)
        .expect("the mini-viz pane");
    app.on_mouse(event::MouseEvent {
        kind: event::MouseEventKind::Down(event::MouseButton::Left),
        column: rect.x + rect.width / 2,
        row: rect.y + rect.height / 2,
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        app.dungeon.intro.is_some(),
        "a click on the mini-viz opens the Delve's menu"
    );
    press(&mut app, KeyCode::Enter);
    assert!(app.dungeon.expanded && app.input.is_empty());
    assert_eq!(run(&app).raid_id, raid);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.input, "Make the sword swing heavier");
    assert_eq!(app.cursor, 9);
}

#[test]
fn dungeon_close_restores_saved_composer_draft() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    press(&mut app, KeyCode::Esc);
    app.input = "Keep this unfinished thought".into();
    app.cursor = 7;
    press(&mut app, KeyCode::F(4));
    assert!(app.input.is_empty());
    app.dungeon_command(Some("off"));
    assert!(!app.dungeon.expanded);
    assert!(app.dungeon.shooter.is_none());
    assert!(app.dungeon.guest.is_none());
    assert_eq!(app.input, "Keep this unfinished thought");
    assert_eq!(app.cursor, 7);
}

#[test]
fn dungeon_tab_opens_the_card_screen_which_pauses_and_esc_closes_it() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 160, 48);
    press(&mut app, KeyCode::Tab);
    assert!(app.dungeon.cards_open);
    assert!(
        !app.dungeon_keyboard_active(),
        "solo play pauses on the card screen"
    );
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("YOUR CARDS") && screen.contains("Bow"),
        "{screen}"
    );
    press(&mut app, KeyCode::Down);
    assert!(paint(&mut app, 160, 48).contains("THE REALM WE BUILD"));
    press(&mut app, KeyCode::Down);
    assert!(paint(&mut app, 160, 48).contains("THE BOOK"));
    press(&mut app, KeyCode::Esc);
    assert!(!app.dungeon.cards_open);
    assert!(
        app.dungeon.expanded,
        "Esc on the card screen returns to the fight, not the composer"
    );
    assert!(
        app.dungeon_command(Some("cards"))
            .contains("kind   take | hold | play | arm")
    );
}

#[test]
fn dungeon_banked_spoils_reach_the_treasury_and_pay_for_a_wish() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    let run = app.dungeon.shooter.as_mut().unwrap();
    run.players
        .get_mut(&1)
        .unwrap()
        .carried
        .add(crate::drive::together_realm::Spoil::Bone, 6);
    run.players
        .get_mut(&1)
        .unwrap()
        .carried
        .add(crate::drive::together_realm::Spoil::Wax, 2);
    run.retreat();
    app.settle_realm();
    let host = app.host_name();
    assert!(
        app.dungeon.notice.contains(&format!(
            "{host} banked 3 bone · 1 wax (the party turned back)"
        )),
        "{}",
        app.dungeon.notice
    );
    assert_eq!(app.realm().treasury.label(), "3 bone · 1 wax");
    assert!(
        app.dungeon.shooter.as_ref().unwrap().bank.is_empty(),
        "banked once"
    );
    app.settle_realm();
    assert_eq!(app.realm().treasury.label(), "3 bone · 1 wax");

    assert!(
        app.dungeon_command(Some("wish a candle shrine"))
            .contains("wished for")
    );
    let id = app
        .realm()
        .wishes
        .iter()
        .find(|w| w.words == "a candle shrine")
        .unwrap()
        .id
        .clone();
    let (mut draft, _) = crate::drive::together_realm::check(
        &id,
        "name Candle Shrine\nprice bone 3, wax 1\nart\n.5.\nhHh\n",
    )
    .unwrap();
    draft.id = id.clone();
    app.realm().draft(draft).unwrap();
    let listed = app.dungeon_command(Some("wishes"));
    assert!(
        listed.contains("Candle Shrine") && listed.contains("ready"),
        "{listed}"
    );
    assert!(
        app.dungeon_command(Some(&format!("grant {id}")))
            .contains("rises")
    );
    assert!(app.realm().treasury.is_empty());
    assert_eq!(app.realm().built().len(), 1);
}

#[test]
fn dungeon_drafting_ends_with_the_turn_and_says_when_no_draft_came() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.dungeon.drafting = Some(("no-such-wish-zz".into(), std::time::Instant::now(), false));
    app.watch_drafting();
    assert!(
        app.dungeon.drafting.is_some(),
        "waits for the turn to be seen"
    );
    app.dungeon.drafting = Some(("no-such-wish-zz".into(), std::time::Instant::now(), true));
    app.watch_drafting();
    assert!(app.dungeon.drafting.is_none());
    assert!(
        app.dungeon.notice.contains("without")
            && app.dungeon.notice.contains("no-such-wish-zz.wish"),
        "{}",
        app.dungeon.notice
    );
}

/// A lockstep host for a playtesting agent: the real cockpit App behind a
/// tiny local HTTP API, where game time only moves when the agent acts.
/// `ANGEL_TESTER_PORT=8790 ANGEL_TESTER_DIR=<dir> cargo test ... dungeon_tester_host -- --ignored`
/// - `GET /look` — the screen as text, the room as a PNG path, and facts.
/// - `POST /act` — `{"press":["tab"], "keys":[{"key":"d"}], "ms":600, "command":"/dungeon wishes"}`.
/// - `POST /quit`.
#[test]
#[ignore]
fn dungeon_tester_host() {
    use std::io::{BufRead, BufReader, Read, Write};
    let Ok(port) = std::env::var("ANGEL_TESTER_PORT") else {
        return;
    };
    let dir = std::path::PathBuf::from(
        std::env::var("ANGEL_TESTER_DIR").unwrap_or_else(|_| "/work/tmp/delve-tester".into()),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = crate::tests::env_lock();
    let _backdrop = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in-process");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let two = std::env::var("ANGEL_TESTER_PLAYERS").is_ok_and(|v| v == "2");
    app.input = if two {
        "/dungeon start Squire".into()
    } else {
        "/dungeon start".into()
    };
    app.submit();
    app.dungeon.key_releases = true;
    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{port}")).unwrap();
    let mut shot = 0u32;

    let key = |name: &str| -> Option<(KeyCode, KeyModifiers)> {
        let (shift, name) = match name.strip_prefix("shift+") {
            Some(rest) => (true, rest),
            None => (false, name),
        };
        let code = match name.to_ascii_lowercase().as_str() {
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "space" => KeyCode::Char(' '),
            "tab" => KeyCode::Tab,
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "f4" => KeyCode::F(4),
            one if one.chars().count() == 1 => KeyCode::Char(one.chars().next().unwrap()),
            _ => return None,
        };
        Some((
            code,
            if shift {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            },
        ))
    };

    let look = |app: &mut App, shot: &mut u32, said: &str| -> serde_json::Value {
        let screen = paint(app, 160, 48);
        let mut facts = serde_json::json!({});
        if let Some(run) = app.dungeon.shooter.clone() {
            *shot += 1;
            let img = crate::stage::world_viz::overworld::arena::frame(&run, 768, 448);
            let path = dir.join(format!("frame-{shot:04}.png"));
            image::save_buffer(
                &path,
                &img.rgb_bytes(),
                img.w as u32,
                img.h as u32,
                image::ColorType::Rgb8,
            )
            .unwrap();
            let room = run.room();
            let realm = app.realm().clone();
            facts = serde_json::json!({
                "frame_png": path,
                "arena_units": [crate::drive::together_shooter::WIDTH, crate::drive::together_shooter::HEIGHT],
                "phase": run.phase, "floor": run.floor(), "pack": run.dungeon.pack.name(),
                "room_kind": format!("{:?}", room.kind), "doors_barred": run.barred(),
                "doorways_nesw": room.doors, "tick": run.tick, "score": run.score,
                "stairs_here": (0..crate::drive::together_shooter::ROWS as i32).any(|r| (0..crate::drive::together_shooter::COLS as i32).any(|c| room.tile(c, r) == crate::drive::together_shooter::Tile::Stairs)),
                "chest": room.chest.map(|c| serde_json::json!({"x": c.x, "y": c.y, "open": c.open})),
                "heroes": run.players.iter().map(|(id, h)| serde_json::json!({
                    "id": id, "name": h.name, "x": h.x, "y": h.y, "hp": h.hp, "max_hp": h.max_hp,
                    "hand": h.hand, "deck": h.deck, "arm": h.arm, "carrying": h.carried.label(), "stone": h.stone,
                })).collect::<Vec<_>>(),
                "monsters": run.enemies.iter().map(|e| serde_json::json!({"kind": e.kind, "x": e.x, "y": e.y, "hp": e.hp})).collect::<Vec<_>>(),
                "cards_on_floor": room.items.iter().map(|i| serde_json::json!({
                    "card": run.book.get(&i.card).map_or(i.card.clone(), |c| format!("{} ({})", c.name, c.kind.word())),
                    "x": i.x, "y": i.y,
                })).collect::<Vec<_>>(),
                "treasury": realm.treasury.label(),
                "wishes": realm.wishes.iter().enumerate().map(|(n, w)| serde_json::json!({
                    "n": n + 1, "name": if w.name.is_empty() { &w.words } else { &w.name }, "status": w.status,
                    "price": w.price.label(), "need": realm.treasury.shortfall(&w.price),
                })).collect::<Vec<_>>(),
                "card_screen_open": app.dungeon.cards_open, "game_expanded": app.dungeon.expanded,
            });
        }
        serde_json::json!({ "screen": screen, "facts": facts, "said": said })
    };

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut first = String::new();
        if reader.read_line(&mut first).is_err() {
            continue;
        }
        let mut length = 0usize;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
                break;
            }
            if let Some(v) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                length = v.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; length.min(1 << 20)];
        let _ = reader.read_exact(&mut body);
        let action: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
        let mut said = Vec::new();
        let quit = first.starts_with("POST /quit");
        if first.starts_with("POST /act") {
            for name in action["press"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
            {
                match key(name) {
                    Some((code, mods)) => {
                        app.on_key(event::KeyEvent::new(code, mods));
                        let mut up = event::KeyEvent::new(code, mods);
                        up.kind = event::KeyEventKind::Release;
                        app.on_key(up);
                    }
                    None => said.push(format!("unknown key `{name}`")),
                }
            }
            if let Some(command) = action["command"].as_str().filter(|c| !c.trim().is_empty()) {
                let command = command.trim();
                let grant_asked = command
                    .strip_prefix("/dungeon grant ")
                    .is_some_and(|which| {
                        let realm = app.realm();
                        let which = which.trim();
                        which
                            .parse::<usize>()
                            .ok()
                            .and_then(|n| realm.wishes.get(n.wrapping_sub(1)))
                            .or_else(|| realm.wishes.iter().find(|w| w.id == which))
                            .is_some_and(|w| {
                                w.status == crate::drive::together_realm::Status::Asked
                            })
                    });
                if !command.starts_with("/dungeon") {
                    said.push("the tester may only run /dungeon commands".into());
                } else if grant_asked {
                    said.push(
                        "drafting a wish needs the host's live angelX; skipped in this test".into(),
                    );
                } else {
                    let before = app.messages.len();
                    app.input = command.into();
                    app.cursor = app.input.len();
                    app.submit();
                    said.extend(
                        app.messages[before.min(app.messages.len())..]
                            .iter()
                            .map(|m| m.text.to_string()),
                    );
                }
            }
            let held: Vec<(KeyCode, KeyModifiers)> = action["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| k["key"].as_str().and_then(key))
                .collect();
            let ms = action["ms"]
                .as_u64()
                .unwrap_or(if held.is_empty() { 0 } else { 500 })
                .min(3000);
            paint(&mut app, 160, 48);
            for &(code, mods) in &held {
                app.on_key(event::KeyEvent::new(code, mods));
            }
            advance(&mut app, (ms / 33) as u32);
            for &(code, mods) in &held {
                let mut up = event::KeyEvent::new(code, mods);
                up.kind = event::KeyEventKind::Release;
                app.on_key(up);
            }
            if app.dungeon.shooter.is_some() && !app.dungeon.notice.is_empty() {
                said.push(format!("notice: {}", app.dungeon.notice));
            }
        }
        let reply = if quit {
            serde_json::json!({"bye": true})
        } else {
            look(&mut app, &mut shot, &said.join("\n"))
        };
        let bytes = serde_json::to_vec(&reply).unwrap();
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        let _ = stream.write_all(&bytes);
        if quit {
            break;
        }
    }
}

#[test]
fn dungeon_calls_the_knight_to_the_gate_then_the_intro_begins_the_delve() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    assert!(app.dungeon_command(None).contains("sets out for the Delve"));
    assert!(app.world.delve_called && app.dungeon.shooter.is_none());
    for _ in 0..4000 {
        if app.world.at_delve_gate() {
            break;
        }
        app.world.tick_overworld();
    }
    assert!(app.world.at_delve_gate(), "he walks there");
    app.watch_delve_gate();
    assert!(
        app.messages
            .last()
            .is_some_and(|m| m.text.contains("Delve's gate"))
    );
    press(&mut app, KeyCode::F(4));
    assert!(app.dungeon.intro.is_some() && app.dungeon_view_active());
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("THE OLD DELVE") && screen.contains("Sir Percival"),
        "{screen}"
    );
    press(&mut app, KeyCode::Right);
    assert!(paint(&mut app, 160, 48).contains("Dame Lynette"));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    let run = app.dungeon.shooter.as_ref().expect("the delve begins");
    assert_eq!(run.players[&1].knight.as_deref(), Some("lynette"));
    assert_eq!(
        run.dungeon.pack,
        crate::drive::together_shooter::Pack::Cavern
    );
    assert!(app.dungeon.intro.is_none() && app.dungeon.expanded);
    assert!(
        app.world.delve_lit,
        "the gate burns while they are down there"
    );
}

#[test]
fn dungeon_host_for_three_gives_three_links_and_each_seat_its_own_knight() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    // What `/dungeon_host --3` runs.
    let message = app.dungeon_command(Some("host --3"));
    let links: Vec<&str> = message
        .lines()
        .filter_map(|l| l.split_once("/dungeon join ").map(|(_, link)| link.trim()))
        .collect();
    assert_eq!(links.len(), 3, "{message}");
    let tokens: std::collections::BTreeSet<&str> = links
        .iter()
        .map(|l| l.rsplit_once('#').unwrap().1)
        .collect();
    assert_eq!(tokens.len(), 3, "each friend has their own code");
    assert_eq!(run(&app).players.len(), 1, "knights join as friends arrive");
    assert_eq!(
        app.dungeon_command(Some("invite"))
            .matches("/dungeon join ")
            .count(),
        3
    );

    // Each link speaks for its own seat: friend 3's hello seats knight 4.
    let (base, token) = crate::drive::together_join::parse_link(links[2]).unwrap();
    let reply = ureq::post(&format!("{base}/hello"))
        .set("Authorization", &format!("Bearer {token}"))
        .send_string("Matt")
        .unwrap()
        .into_string()
        .unwrap();
    assert!(reply.contains("knight 4"), "{reply}");
    for _ in 0..50 {
        app.advance_dungeon_guest();
        if run(&app).players.contains_key(&4) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(run(&app).players[&4].name, "Matt");
    assert!(!run(&app).players.contains_key(&2));
    assert!(
        run(&app).players[&4].knight.is_some(),
        "a knight of the company"
    );
}

#[test]
fn a_friends_angelx_joins_by_pasting_its_line_and_sees_the_hosts_game() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host.dungeon_command(Some("host --1"));
    let line = message
        .lines()
        .find_map(|l| {
            l.split_once("friend 1:")
                .map(|(_, rest)| rest.trim().to_string())
        })
        .expect("an invitation line");
    assert!(line.starts_with("/dungeon join http://"), "{line}");

    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    let joined = friend.dungeon_command(Some(line.trim_start_matches("/dungeon ")));
    assert!(joined.starts_with("Joined the delve"), "{joined}");
    assert!(friend.dungeon_view_active());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut seen = false;
    while std::time::Instant::now() < deadline {
        host.advance_dungeon_guest();
        let party = friend
            .dungeon
            .joined
            .as_ref()
            .and_then(|j| j.state())
            .and_then(|s| s["players"].as_array().map(Vec::len));
        let joined = friend.dungeon.joined.as_ref().unwrap();
        let shown = joined.frame().is_some() || joined.with_view(|_, _| ()).is_some();
        if party == Some(2) && shown {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(seen, "the friend sees a party of two and the room");
    assert!(run(&host).players.contains_key(&2));
    assert_eq!(friend.joined_actor(), 2);

    // The friend's angelX holds its own mirror of the host's delve.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mirrored = loop {
        host.advance_shooter_at(std::time::Instant::now());
        let tick = friend
            .dungeon
            .joined
            .as_ref()
            .unwrap()
            .with_view(|run, _| run.tick);
        if tick.is_some_and(|t| t > 0 && run(&host).tick.saturating_sub(t) <= 3) {
            break true;
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    };
    assert!(mirrored, "the mirror keeps up with the host's ticks");

    // A friend's known wish goes to the host and lands on their knight.
    host.dungeon.shooter.as_mut().unwrap().window = Some(Default::default());
    friend.dungeon.joined.as_ref().unwrap().post(
        "/boon",
        "application/json",
        br#"{"wish":"seeker"}"#.to_vec(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while run(&host).players[&2].bonus.homing == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the friend's wish never landed"
        );
        host.advance_shooter_at(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    // On Kitty the friend's frame reaches the terminal as a PNG.
    let mut kitty = App::preview(crate::ui::viewer::Viewer::kitty_for_test());
    kitty.dungeon.joined = friend.dungeon.joined.take();
    kitty.expand_dungeon();
    let mut terminal = Terminal::new(TestBackend::new(160, 48)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut kitty))
        .unwrap();
    let written: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(
        written.contains("a=T,U=1,f=100"),
        "the frame's upload is written with the draw"
    );
    friend.dungeon.joined = kitty.dungeon.joined.take();

    // Tab opens the friend's own cards, looked up in the host's book.
    press(&mut friend, KeyCode::Tab);
    assert!(friend.dungeon.cards_open);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while friend.dungeon.joined.as_ref().unwrap().book().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the host's book never came"
        );
        host.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let screen = paint(&mut friend, 160, 48);
    assert!(screen.contains("YOUR CARDS"), "{screen}");
    assert!(friend.joined_card_pages() > 1, "and the book's pages");
    press(&mut friend, KeyCode::Down);
    assert!(paint(&mut friend, 160, 48).contains("THE BOOK"));
    press(&mut friend, KeyCode::Tab);
    assert!(!friend.dungeon.cards_open);

    // The friend holds a key: their knight walks in the host's game.
    let start = (run(&host).players[&2].x, run(&host).players[&2].y);
    friend
        .dungeon
        .joined
        .as_ref()
        .unwrap()
        .set_input(crate::drive::together_shooter::Input {
            move_x: -1,
            move_y: -1,
            ..Default::default()
        });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        host.advance_shooter_at(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let end = (run(&host).players[&2].x, run(&host).players[&2].y);
    assert!(
        (end.0 - start.0).abs() + (end.1 - start.1).abs() > 2.0,
        "the friend's knight moved: {start:?} -> {end:?}"
    );

    assert!(friend.dungeon_command(Some("leave")).contains("left"));
    assert!(friend.dungeon.joined.is_none() && !friend.dungeon_view_active());
}

#[test]
fn hosting_alone_pauses_on_esc_and_never_wipes_a_party_of_statues() {
    let _guard = crate::tests::env_lock();
    let _backdrop = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in-process");
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.terminal_focused = true;
    app.dungeon_command(Some("host --3"));
    press(&mut app, KeyCode::Enter); // the Delve's menu: continue
    paint(&mut app, 160, 48);
    advance(&mut app, 5);
    assert!(run(&app).tick > 0, "the host plays");
    // Away before any friend arrives (sending the invitations): a pause.
    press(&mut app, KeyCode::Esc);
    let tick = run(&app).tick;
    advance(&mut app, 30);
    assert!(run(&app).active(), "no wipe: {:?}", run(&app).phase);
    assert_eq!(run(&app).tick, tick, "alone, the delve waits");
    assert!(!run(&app).players[&1].stone);
    app.terminal_focused = false;
    advance(&mut app, 30);
    assert!(run(&app).active());

    // A party of statues is not a fallen party.
    let mut statues = run(&app).clone();
    for hero in statues.players.values_mut() {
        hero.stone = true;
    }
    statues.step(&std::collections::BTreeMap::new());
    assert!(statues.active(), "{:?}", statues.phase);
}

#[test]
fn dungeon_host_opens_the_delves_menu_over_the_entrance_room() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.terminal_focused = true;
    let message = app.dungeon_command(Some("host --2"));
    assert!(message.contains("menu is open"), "{message}");
    // Friends can join at once: the delve is already open behind the menu.
    let raid = run(&app).raid_id;
    let intro = app.dungeon.intro.as_ref().expect("the Delve's menu");
    assert_eq!(
        intro.field(),
        crate::ui::viz::delve_intro_viz::Field::Continue
    );
    assert_eq!((intro.party, intro.hosting), (2, Some(2)));
    assert!(paint(&mut app, 160, 48).contains("2 seats"));
    press(&mut app, KeyCode::Enter);
    assert!(app.dungeon.intro.is_none() && app.dungeon_view_active());
    assert_eq!(run(&app).raid_id, raid, "continue keeps the open delve");
    // Hosting again shows the same lines and the menu again.
    let again = app.dungeon_command(Some("host --2"));
    assert_eq!(again.matches("/dungeon join ").count(), 2, "{again}");
    assert!(app.dungeon.intro.is_some());
}

#[test]
fn a_new_delve_while_hosting_keeps_the_seated_friends() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.dungeon_command(Some("host --3"));
    let shooter = app.dungeon.shooter.as_mut().unwrap();
    shooter.join_seat(3, "Matt");
    let lynette = crate::drive::together_shooter::knights::knight("lynette").unwrap();
    shooter.outfit(3, lynette);
    let raid = run(&app).raid_id;
    // From the Delve's menu, with friends along: a fresh delve.
    app.open_intro();
    app.dungeon.intro.as_mut().unwrap().party = 2;
    app.dungeon
        .intro
        .as_mut()
        .unwrap()
        .go(crate::ui::viz::delve_intro_viz::Field::Begin);
    press(&mut app, KeyCode::Enter);
    assert!(run(&app).raid_id > raid, "a new delve, not the old map");
    assert_eq!(run(&app).players[&3].name, "Matt");
    assert_eq!(run(&app).players[&3].knight.as_deref(), Some("lynette"));
    assert!(app.dungeon.guest.is_some(), "the same invitations");
}

#[test]
fn the_scroll_reads_a_wish_back_and_seals_it_only_when_asked() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.terminal_focused = true;
    app.dungeon.shooter.as_mut().unwrap().descend_for_test();
    paint(&mut app, 160, 48);
    press(&mut app, KeyCode::Char('t'));
    assert!(app.dungeon.forge.draft.is_some(), "the scroll opens");
    assert!(paint(&mut app, 160, 48).contains("THE SCROLL OF ONE WISH"));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.dungeon.forge.draft.as_ref().unwrap().confirm,
        None,
        "an empty wish is not read back"
    );
    for c in "three bolts at once".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.dungeon.forge.draft.as_ref().unwrap().confirm,
        Some(true)
    );
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("ONE WISH") && screen.contains("SEAL THE WISH"),
        "{screen}"
    );
    // Rewrite goes back to the words, kept.
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Enter);
    let draft = app.dungeon.forge.draft.as_ref().unwrap();
    assert_eq!(
        (draft.confirm, draft.words.as_str()),
        (None, "three bolts at once")
    );
    // N backs out too; Y seals.
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.dungeon.forge.draft.as_ref().unwrap().confirm, None);
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('y'));
    assert!(app.dungeon.forge.draft.is_none());
    assert!(
        app.dungeon.notice.contains("sealed") || app.dungeon.notice.contains("reforging"),
        "the wish goes to angelX: {}",
        app.dungeon.notice
    );
}

#[test]
fn a_won_room_grants_a_known_wish_at_once_without_leaving_the_game() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    let shooter = app.dungeon.shooter.as_mut().unwrap();
    shooter.calm_for_test();
    shooter.clear_for_test();
    assert!(run(&app).can_wish(1), "the room is won: a window opens");
    press(&mut app, KeyCode::Char('t'));
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("Triple Volley I"),
        "known wishes are listed"
    );
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("ONE WISH") && screen.contains("yours at once"),
        "{screen}"
    );
    press(&mut app, KeyCode::Char('y'));
    assert!(app.dungeon.forge.draft.is_none());
    assert!(app.dungeon_view_active(), "the game never left the screen");
    assert!(
        app.dungeon.notice.starts_with("Granted"),
        "{}",
        app.dungeon.notice
    );
    let hero = &run(&app).players[&1];
    assert!(
        hero.deck.iter().any(|c| c.starts_with("wish-")),
        "{:?}",
        hero.deck
    );
    assert!(!run(&app).can_wish(1), "one wish a window");
    // Typed words find a known wish too, in the next window.
    app.dungeon.shooter.as_mut().unwrap().window = Some(Default::default());
    press(&mut app, KeyCode::Char('t'));
    for c in "bounce my shots".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert_eq!(run(&app).players[&1].bonus.bounce, 1);
}

#[test]
fn an_unknown_wish_is_learned_while_you_play_and_granted_when_the_book_reloads() {
    let _guard = crate::tests::env_lock();
    let workspace = std::env::temp_dir().join(format!(
        "delve-learn-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&workspace).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .expect("preview registry should be uniquely owned")
        .set_workspace(workspace.clone());
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    let shooter = app.dungeon.shooter.as_mut().unwrap();
    shooter.calm_for_test();
    shooter.clear_for_test();
    press(&mut app, KeyCode::Char('t'));
    for c in "make my arrows glow like stars".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("ONE WISH"),
        "unknown words are still read back"
    );
    press(&mut app, KeyCode::Enter);
    assert!(app.dungeon_view_active(), "the game stays on screen");
    assert_eq!(app.dungeon.forge.learning.len(), 1);
    assert!(
        app.dungeon.notice.contains("learning"),
        "{}",
        app.dungeon.notice
    );
    assert!(!run(&app).can_wish(1), "the window is spent on it");
    // angelX writes the wish into the workspace's phrasebook...
    let book = workspace.join(".angel/dungeon/phrasebook.txt");
    std::fs::create_dir_all(book.parent().unwrap()).unwrap();
    std::fs::write(
        &book,
        "wish starlight | Starlight Arrows\nsay make my arrows glow like stars | starlight arrows | glowing arrows\ntier homing 1; damage 10 :: Arrows glow and seek.\ntier homing 2; damage 20 :: Arrows burn bright and seek.\ntier homing 3; damage 30 :: Arrows are stars.\n",
    )
    .unwrap();
    app.thinking = None;
    app.pending_turn = None;
    app.dungeon.forge.looked = None;
    app.watch_learning();
    // ...and the book reloads with it: granted, mid-game.
    assert!(
        app.dungeon.notice.contains("Starlight Arrows"),
        "{}",
        app.dungeon.notice
    );
    assert_eq!(run(&app).players[&1].bonus.homing, 1);
    assert!(app.dungeon.forge.learning.is_empty());
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn a_card_written_while_the_game_runs_joins_the_book() {
    let _guard = crate::tests::env_lock();
    let workspace = std::env::temp_dir().join(format!(
        "delve-cards-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(workspace.join(".angel/dungeon/cards")).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .expect("preview registry should be uniquely owned")
        .set_workspace(workspace.clone());
    app.input = "/dungeon start".into();
    app.submit();
    app.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(run(&app).book.get("ember-bolt").is_none());
    std::fs::write(
        workspace.join(".angel/dungeon/cards/ember-bolt.card"),
        "name Ember Bolt\nkind hold\ntext Hotter shots.\ndamage 10\n",
    )
    .unwrap();
    app.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(run(&app).book.get("ember-bolt").is_some(), "hot-loaded");
    assert!(
        app.dungeon.notice.contains("Ember Bolt"),
        "{}",
        app.dungeon.notice
    );
    let _ = std::fs::remove_dir_all(&workspace);
}

/// Film plates: `ANGEL_DELVE_FILM=<dir> cargo test write_tui_film -- --ignored`.
/// The whole terminal, one game tick a line of `cells.jsonl` (each cell
/// `[symbol, fg, bg, bold]`, colours as ratatui writes them): `/dungeon`
/// typed at the prompt, the room won, the scroll opened, "triple wide fire
/// my shots" written, ONE WISH sealed, the grant. Kitty uploads in the
/// buffer are kept whole, so the room's own PNG rides along.
#[test]
#[ignore]
fn write_tui_film() {
    let Some(dir) = std::env::var_os("ANGEL_DELVE_FILM") else {
        return;
    };
    let _guard = crate::tests::env_lock();
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (w, h) = (160u16, 45u16);
    let mut out = String::new();
    let mut marks = String::new();
    let mut frame = 0usize;
    let mut shoot = |app: &mut App, out: &mut String| {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| crate::ui::draw::ui(f, app)).unwrap();
        let cells: Vec<serde_json::Value> = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| {
                serde_json::json!([
                    c.symbol(),
                    format!("{:?}", c.fg),
                    format!("{:?}", c.bg),
                    c.modifier.contains(ratatui::style::Modifier::BOLD)
                ])
            })
            .collect();
        out.push_str(&serde_json::to_string(&cells).unwrap());
        out.push('\n');
        frame += 1;
        frame
    };
    let mut app = App::preview(crate::ui::viewer::Viewer::kitty_for_test());
    app.terminal_focused = true;
    for _ in 0..20 {
        shoot(&mut app, &mut out);
    }
    for c in "/dungeon start Matt".chars() {
        press(&mut app, KeyCode::Char(c));
        for _ in 0..3 {
            shoot(&mut app, &mut out);
        }
    }
    for _ in 0..12 {
        shoot(&mut app, &mut out);
    }
    app.submit();
    marks.push_str(&format!("{} dungeon\n", shoot(&mut app, &mut out)));
    for _ in 0..90 {
        advance(&mut app, 1);
        shoot(&mut app, &mut out);
    }
    let shooter = app.dungeon.shooter.as_mut().unwrap();
    shooter.calm_for_test();
    shooter.clear_for_test();
    marks.push_str(&format!("{} won\n", shoot(&mut app, &mut out)));
    for _ in 0..45 {
        advance(&mut app, 1);
        shoot(&mut app, &mut out);
    }
    press(&mut app, KeyCode::Char('t'));
    marks.push_str(&format!("{} scroll\n", shoot(&mut app, &mut out)));
    for _ in 0..30 {
        advance(&mut app, 1);
        shoot(&mut app, &mut out);
    }
    for c in "triple wide fire my shots".chars() {
        press(&mut app, KeyCode::Char(c));
        for _ in 0..3 {
            advance(&mut app, 1);
            shoot(&mut app, &mut out);
        }
    }
    for _ in 0..20 {
        advance(&mut app, 1);
        shoot(&mut app, &mut out);
    }
    press(&mut app, KeyCode::Enter);
    marks.push_str(&format!("{} one_wish\n", shoot(&mut app, &mut out)));
    for _ in 0..60 {
        advance(&mut app, 1);
        shoot(&mut app, &mut out);
    }
    press(&mut app, KeyCode::Char('y'));
    marks.push_str(&format!("{} granted\n", shoot(&mut app, &mut out)));
    for _ in 0..90 {
        advance(&mut app, 1);
        shoot(&mut app, &mut out);
    }
    assert!(
        app.dungeon.notice.starts_with("Granted"),
        "{}",
        app.dungeon.notice
    );
    std::fs::write(dir.join("cells.jsonl"), out).unwrap();
    std::fs::write(dir.join("marks.txt"), marks).unwrap();
}

#[test]
fn dungeon_spell_loaded_by_card_watcher_reaches_first_empty_host_slot_once() {
    let _guard = crate::tests::env_lock();
    let workspace = std::env::temp_dir().join(format!(
        "angel-dungeon-spell-watchers-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&workspace);
    let dir = workspace.join(".angel/dungeon/cards");
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(workspace.clone());
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    press(&mut app, KeyCode::Esc);
    // Prime the older watcher's file stamp before writing the new spell.
    app.dungeon.forge.looked = None;
    app.watch_learning();
    let raw = "name Cha-Ching Meteor\nkind spell\nrarity relic\nby Money Knight\ncooldown 2\ncast meteor\nnova 25\n";
    let path = dir.join("cha-ching-meteor.card");
    std::fs::write(&path, raw).unwrap();
    app.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(run(&app).book.get("cha-ching-meteor").is_some());
    assert!(app.dungeon.notice.contains("The book takes new cards"));
    assert_eq!(run(&app).players[&1].spells, [None, None, None]);
    let now = std::time::Instant::now();
    app.dungeon.spell_scan = None;
    app.advance_shooter_at(now);
    assert_eq!(
        run(&app).players[&1].spells,
        [Some("cha-ching-meteor".into()), None, None],
        "book-only loading must not suppress delivery into the first empty slot"
    );
    assert_eq!(run(&app).players[&2].spells, [None, None, None]);
    assert!(run(&app).players[&1].hand.is_empty());
    assert!(
        run(&app)
            .found_line()
            .unwrap()
            .contains("learned Cha-Ching Meteor")
    );
    let blasts = run(&app).blasts.len();
    let items = run(&app).room().items.len();
    app.dungeon
        .shooter
        .as_mut()
        .unwrap()
        .players
        .get_mut(&1)
        .unwrap()
        .spell_cooldowns[0] = 45;
    // Both watchers may see an edit before the spell scan. It changes the
    // rules, but must neither reset recharge nor learn another copy.
    std::fs::write(&path, raw.replace("cooldown 2", "cooldown 3")).unwrap();
    for scan in 1..=2 {
        app.dungeon.forge.looked = None;
        app.watch_learning();
        app.advance_shooter_at(now + std::time::Duration::from_millis(500 * scan));
    }
    assert_eq!(run(&app).book.get("cha-ching-meteor").unwrap().cooldown, 3);
    assert_eq!(run(&app).players[&1].spell_cooldowns[0], 45);
    assert_eq!(
        run(&app).players[&1].spells,
        [Some("cha-ching-meteor".into()), None, None]
    );
    assert_eq!(run(&app).blasts.len(), blasts);
    assert_eq!(run(&app).room().items.len(), items);
    std::fs::remove_dir_all(&workspace).unwrap();
}

#[test]
fn dungeon_spells_hot_load_while_editing_and_have_free_keys_and_visible_hud_slots() {
    let _guard = crate::tests::env_lock();
    let workspace =
        std::env::temp_dir().join(format!("angel-dungeon-spells-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&workspace);
    let dir = workspace.join(".angel/dungeon/cards");
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .unwrap()
        .set_workspace(workspace.clone());
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    press(&mut app, KeyCode::Esc);
    std::fs::write(
        dir.join("spark.card"),
        "name Spark\nkind spell\ncooldown 3\nheal 20\n",
    )
    .unwrap();
    let now = std::time::Instant::now();
    let tick = run(&app).tick;
    app.advance_shooter_at(now);
    assert_eq!(
        run(&app).tick,
        tick,
        "paused solo editing does not advance simulation"
    );
    assert_eq!(run(&app).players[&1].spells[0].as_deref(), Some("spark"));
    press(&mut app, KeyCode::F(6));
    press(&mut app, KeyCode::Enter);
    let text = paint(&mut app, 80, 24);
    for label in [
        "[Z] Spark",
        "[B] empty",
        "[N] empty",
        "[5] empty",
        "[6] empty",
        "[7] empty",
    ] {
        assert!(text.contains(label), "{label}:\n{text}");
    }
    assert!(text.find("Hand:").unwrap() < text.find("[Z]").unwrap());
    app.dungeon.key_releases = true;
    press(&mut app, KeyCode::Char('z'));
    assert_eq!(app.dungeon.local_inputs(std::time::Instant::now()).get(&1).unwrap().cast, 1);
    press(&mut app, KeyCode::Char('6'));
    assert_eq!(app.dungeon.local_inputs(std::time::Instant::now()).get(&2).unwrap().cast, 2);
    app.dungeon
        .shooter
        .as_mut()
        .unwrap()
        .players
        .get_mut(&1)
        .unwrap()
        .spell_cooldowns[0] = 60;
    let text = paint(&mut app, 80, 24);
    assert!(text.contains("[Z] Spark · 2s"), "{text}");
    assert!(!app.input.contains('z'));
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn a_tap_without_key_releases_is_one_step_and_a_held_key_keeps_walking() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    assert!(!app.dungeon.key_releases, "a terminal with no key releases");
    let ms = std::time::Duration::from_millis;

    // One tap: the knight walks about its own width, not two.
    press(&mut app, KeyCode::Char('d'));
    let at = app.dungeon.held[&KeyCode::Char('d')].at;
    assert_eq!(app.dungeon.local_inputs(at + ms(60))[&1].move_x, 1);
    assert_eq!(
        app.dungeon.local_inputs(at + ms(120))[&1].move_x,
        0,
        "a tap is over well inside the old 180 ms lease"
    );

    // The terminal's own key repeat: the knight walks until it stops.
    app.dungeon.held.clear();
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('d'));
    let at = app.dungeon.held[&KeyCode::Char('d')].at;
    assert_eq!(app.dungeon.local_inputs(at + ms(150))[&1].move_x, 1);
    assert_eq!(app.dungeon.local_inputs(at + ms(200))[&1].move_x, 0);

    // Actions keep the whole lease, so a tap always reaches a friend's host.
    app.dungeon.held.clear();
    press(&mut app, KeyCode::Char('f'));
    let at = app.dungeon.held[&KeyCode::Char('f')].at;
    assert!(app.dungeon.local_inputs(at + ms(150))[&1].fire);
}

#[test]
fn playing_in_a_friends_delve_never_clears_the_whole_screen_each_tick() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host.dungeon_command(Some("host --1"));
    let line = message
        .lines()
        .find_map(|l| l.split_once("friend 1:").map(|(_, rest)| rest.trim()))
        .expect("an invitation line");
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(line.trim_start_matches("/dungeon ")));
    assert!(friend.dungeon_view_active());
    // Opening the view repaints once; playing in it must not.
    assert!(friend.take_redraw_request());
    for _ in 0..3 {
        friend.advance_joined();
        assert!(
            !friend.take_redraw_request(),
            "a full clear every tick flashes the terminal"
        );
    }
}

#[test]
fn g_keeps_vigil_through_the_composer_and_g_again_wakes_the_knight() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 120, 40);
    app.dungeon.key_releases = true;
    press(&mut app, KeyCode::Char('g'));
    advance(&mut app, 3);
    let hero = &run(&app).players[&1];
    assert!(hero.vigil && hero.stone);
    assert!(paint(&mut app, 120, 40).contains("VIGIL"));
    // The key comes up and the knight stays warded.
    app.dungeon.held.clear();
    advance(&mut app, 45);
    assert!(run(&app).players[&1].stone);
    let at = (run(&app).players[&1].x, run(&app).players[&1].y);
    press(&mut app, KeyCode::Char('g'));
    advance(&mut app, 3);
    let hero = &run(&app).players[&1];
    assert!(!hero.vigil && !hero.stone);
    assert_eq!((hero.x, hero.y), at, "woken where it stood");
}

#[test]
fn a_friends_g_reaches_the_host_and_their_knight_keeps_vigil_after_esc() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host.dungeon_command(Some("host --1"));
    let line = message
        .lines()
        .find_map(|l| l.split_once("friend 1:").map(|(_, rest)| rest.trim()))
        .expect("an invitation line");
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(line.trim_start_matches("/dungeon ")));
    let until = |host: &mut App, friend: &mut App, done: &dyn Fn(&App, &App) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline && !done(host, friend) {
            host.advance_shooter_at(std::time::Instant::now());
            friend.advance_joined();
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        done(host, friend)
    };
    // The host's HUD says it knows the vigil; only then does G go out.
    assert!(until(&mut host, &mut friend, &|_, friend| {
        friend
            .joined_knight()
            .is_some_and(|k| k["vigil"].as_bool() == Some(false))
    }));
    press(&mut friend, KeyCode::Char('g'));
    assert!(
        until(&mut host, &mut friend, &|host, _| run(host).players[&2]
            .vigil),
        "{}",
        friend.dungeon.notice
    );
    // Away to the composer: no keys go out, and the knight stays stone.
    press(&mut friend, KeyCode::Esc);
    assert!(until(&mut host, &mut friend, &|_, friend| {
        friend
            .joined_knight()
            .is_some_and(|k| k["vigil"].as_bool() == Some(true))
    }));
    for _ in 0..40 {
        host.advance_shooter_at(std::time::Instant::now());
        friend.advance_joined();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let knight = &run(&host).players[&2];
    assert!(knight.vigil && knight.stone);
}
