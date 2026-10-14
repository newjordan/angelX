use super::*;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn joined_angelx_ballot_taps_reach_the_host_in_order_and_close_locally() {
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host.dungeon_command(Some("host --1"));
    let link = link_of(&message);
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(&format!("join {link}")));
    let spin = |host: &mut App, friend: &mut App, ready: fn(&App, &App) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while !ready(host, friend) {
            host.advance_shooter_at(std::time::Instant::now());
            friend.advance_joined();
            assert!(
                std::time::Instant::now() < deadline,
                "ballot never reached the host"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    spin(&mut host, &mut friend, |host, friend| {
        run(host).players.contains_key(&2)
            && friend
                .joined_knight()
                .is_some_and(|k| k["ballot_open"].is_boolean())
    });
    press(&mut friend, KeyCode::Char('v'));
    press(&mut friend, KeyCode::Char('5'));
    press(&mut friend, KeyCode::Down);
    press(&mut friend, KeyCode::Left);
    press(&mut friend, KeyCode::Char('f'));
    assert!(friend.dungeon.joined_ballot_open);
    assert!(!friend.dungeon.local_inputs(std::time::Instant::now())[&1].fire);
    spin(&mut host, &mut friend, |host, _| {
        let run = run(host);
        run.playtest.seats.get(&2).is_some_and(|s| s.selected == 1)
            && run
                .playtest
                .records
                .get(&run.players[&2].name)
                .is_some_and(|r| r.contains_key("hud"))
    });
    let hosted_run = run(&host);
    let votes = &hosted_run.playtest.records[&hosted_run.players[&2].name];
    assert_eq!(
        votes["controls"],
        crate::drive::together_shooter::votes::Rating::Stars(5)
    );
    assert_eq!(
        votes["hud"],
        crate::drive::together_shooter::votes::Rating::Stars(1)
    );
    assert!(hosted_run.playtest.open(2) && !hosted_run.playtest.open(1));
    press(&mut friend, KeyCode::Esc);
    assert!(friend.dungeon.expanded && !friend.dungeon.joined_ballot_open);
    spin(&mut host, &mut friend, |host, _| {
        !run(host).playtest.open(2)
    });
}

#[test]
fn dungeon_ballot_keys_isolate_couch_seats_and_save_votes() {
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = game();
    fixture.bind(&mut app);
    paint(&mut app, 120, 40);
    // The host votes under their own name, not a placeholder.
    let host_name = run(&app).players[&1].name.clone();
    let muted = app.dungeon.audio.voices.muted;
    press(&mut app, KeyCode::Char('v'));
    assert!(run(&app).playtest.open(1));
    assert!(!run(&app).playtest.open(2));
    assert_eq!(app.dungeon.audio.voices.muted, muted);
    let host = (run(&app).players[&1].x, run(&app).players[&1].y);
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Char('i'));
    assert!(app.dungeon.local_inputs(std::time::Instant::now())[&2].move_y < 0);
    advance(&mut app, 2);
    assert_eq!(host, (run(&app).players[&1].x, run(&app).players[&1].y));
    press(&mut app, KeyCode::Char('4'));
    assert_eq!(
        app.realm().votes[&host_name]["controls"],
        crate::drive::together_shooter::votes::Rating::Stars(4)
    );
    let realm_path = app
        .world
        .rewards_path()
        .unwrap()
        .with_extension("realm.json");
    let saved: crate::drive::together_realm::Realm =
        serde_json::from_slice(&std::fs::read(realm_path).unwrap()).unwrap();
    assert_eq!(
        saved.votes[&host_name]["controls"],
        crate::drive::together_shooter::votes::Rating::Stars(4)
    );
    assert!(
        app.dungeon_command(Some("votes"))
            .contains("New controls: 4.0/5 (1)")
    );
    let export = fixture.home.join(".angelX/playtest/votes-0.2.2.jsonl");
    let lines: Vec<serde_json::Value> = std::fs::read_to_string(export)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["stars"], 4);
    press(&mut app, KeyCode::Char('v'));
    assert!(!run(&app).playtest.open(1));
    press(&mut app, KeyCode::Char('b'));
    assert!(run(&app).playtest.open(2));
    assert_eq!(run(&app).playtest.local_seat(), 2);
    press(&mut app, KeyCode::Char('9'));
    assert_eq!(
        app.realm().votes["Test Guest"]["controls"],
        crate::drive::together_shooter::votes::Rating::Stars(5)
    );
    press(&mut app, KeyCode::Char('v'));
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Char('5'));
    assert_eq!(
        app.realm().votes["Test Guest"]["controls"],
        crate::drive::together_shooter::votes::Rating::Stars(1)
    );
    assert_eq!(
        app.realm().votes[&host_name]["controls"],
        crate::drive::together_shooter::votes::Rating::Stars(4)
    );
    press(&mut app, KeyCode::Esc);
    assert!(!run(&app).playtest.open(2));
    assert!(run(&app).playtest.open(1));
    press(&mut app, KeyCode::Esc);
    assert!(!run(&app).playtest.open(1));
    assert!(app.dungeon.expanded);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.dungeon.audio.voices.muted, muted, "K still walks P2");
    app.on_key(event::KeyEvent::new(
        KeyCode::Char('K'),
        KeyModifiers::SHIFT,
    ));
    assert_ne!(app.dungeon.audio.voices.muted, muted);
    app.dungeon_command(Some("off"));
    app.dungeon_command(Some("start"));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(
        app.dungeon.audio.voices.muted, muted,
        "K toggles voices solo"
    );
}

/// The hosted delve's one link, as the host's message gives it.
fn link_of(message: &str) -> String {
    message
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("http://"))
        .unwrap_or_else(|| panic!("no link in: {message}"))
        .to_string()
}

/// A delve begun, and the party down the Winding Stair onto floor one.
fn game() -> App {
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
    assert!(
        run(&app).at_home_now(),
        "every delve begins in the Undercroft"
    );
    app.dungeon.shooter.edit().unwrap().descend_for_test();
    app
}

struct RealmSaveFixture {
    root: std::path::PathBuf,
    home: std::path::PathBuf,
    workspace: std::path::PathBuf,
}

impl RealmSaveFixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "angel-realm-save-error-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = root.join("home");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(home.join(".angelX"), b"not a directory").unwrap();
        Self {
            root,
            home,
            workspace,
        }
    }

    fn bind(&self, app: &mut App) {
        app.tools = Arc::new(crate::agent::harness::ToolRegistry::with_team(
            self.workspace.clone(),
            Vec::new(),
        ));
        app.world = crate::stage::world_viz::World::for_workspace(&self.workspace);
        app.dungeon.realm = None;
    }

    fn allow_saves(&self) {
        std::fs::remove_file(self.home.join(".angelX")).unwrap();
        std::fs::create_dir_all(self.home.join(".angelX/world-rewards")).unwrap();
    }
}

impl Drop for RealmSaveFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn with_blocked_realm_save() -> (RealmSaveFixture, crate::tests::TestEnvGuard) {
    let fixture = RealmSaveFixture::new();
    let home = crate::tests::TestEnvGuard::set("HOME", fixture.home.to_str().unwrap());
    (fixture, home)
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
    let now = app.dungeon.shooter.clock().unwrap_or_else(std::time::Instant::now);
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
            "Floor 1/6",
            "P1",
            "P2",
            "100/100",
            "F4 game",
            "Esc composer",
            "arrows aim",
            "E interact",
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
    // Knights start in their smallclothes; arm both so a held aim shoots.
    for id in [1, 2] {
        let hero = app.dungeon.shooter.edit().unwrap().players.get_mut(&id).unwrap();
        hero.weapon = crate::drive::together_shooter::Weapon::Bow;
    }
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
    let now = app.dungeon.shooter.clock().unwrap();
    app.advance_shooter_at(now + std::time::Duration::from_secs(30));
    assert_eq!(
        run(&app).tick,
        3 + 1,
        "after a stall the server's beat starts over: no racing to catch up"
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
    assert_eq!(
        run(&app).raid_id,
        raid,
        "R is the ultimate while the delve runs"
    );
    assert!(
        app.dungeon.notice.contains("charging"),
        "{}",
        app.dungeon.notice
    );
    app.dungeon.shooter.edit().unwrap().phase = Phase::Wiped;
    press(&mut app, KeyCode::Char('r'));
    assert!(run(&app).raid_id > raid);
    assert!(run(&app).at_home_now(), "home to the Undercroft");
    assert_eq!(run(&app).phase, Phase::Exploring);
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

/// A friend's seat on the hosted delve, by its link over a real socket: a
/// client as a friend's angelX or browser is, seated once welcomed.
fn friend_seat(app: &mut App) -> crate::drive::together_seat::Seat {
    let server = app.dungeon.shooter.server().unwrap();
    let link = angel_serve::link::link(&format!("http://{}", server.addr()), &server.key());
    let seat = crate::drive::together_seat::Seat::connect(&link, "Friend").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while seat.knight().is_none() {
        assert!(std::time::Instant::now() < deadline, "the friend was never seated");
        seat.tick(std::time::Instant::now());
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    seat
}

/// Step the host until `ready`, the friend's seat sending its keys as it goes.
fn host_until(app: &mut App, seat: &crate::drive::together_seat::Seat, what: &str, ready: impl Fn(&App) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    while !ready(app) {
        assert!(std::time::Instant::now() < deadline, "{what}");
        seat.tick(std::time::Instant::now());
        let now = app
            .dungeon
            .shooter
            .clock()
            .map_or_else(std::time::Instant::now, |last| last + std::time::Duration::from_nanos(33_333_333));
        app.advance_shooter_at(now);
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
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
    // The friend opens the link: their seat plays knight 2.
    let friend = friend_seat(&mut app);
    assert_eq!(friend.knight(), Some(2));
    host_until(&mut app, &friend, "the friend never said who they are", |app| run(app).players[&2].name == "Friend");

    friend.set_held(angel_net::input::button::RIGHT);
    let (x, host_x) = (run(&app).players[&2].x, run(&app).players[&1].x);
    host_until(&mut app, &friend, "the friend's key never moved knight 2", |app| run(app).players[&2].x > x + 1.0);
    assert_eq!(run(&app).players[&1].x, host_x, "and only knight 2");

    // The friend's own client draws their own screen.
    let picture = friend.frame(std::time::Instant::now()).expect("the friend's frame");
    assert_eq!((picture.w, picture.h), crate::stage::world_viz::overworld::arena::art_v2::size());

    // Esc stones the host, but the friend keeps playing.
    let host_position = (run(&app).players[&1].x, run(&app).players[&1].y);
    app.collapse_dungeon();
    let tick = run(&app).tick;
    let x = run(&app).players[&2].x;
    host_until(&mut app, &friend, "the delve stood still while the friend played", |app| {
        run(app).tick > tick + 5 && run(app).players[&2].x > x
    });
    assert!(run(&app).players[&1].stone);
    friend.set_held(0);

    press(&mut app, KeyCode::F(6));
    press(&mut app, KeyCode::Enter); // the Delve's menu: continue
    paint(&mut app, 80, 24);
    advance(&mut app, 0);
    assert!(!run(&app).players[&1].stone);
    assert_eq!(
        (run(&app).players[&1].x, run(&app).players[&1].y),
        host_position
    );

    // The friend stays for the next delve, as the same knight.
    let raid = run(&app).raid_id;
    app.dungeon.shooter.edit().unwrap().phase = crate::drive::together_shooter::Phase::Won;
    press(&mut app, KeyCode::Char('r'));
    paint(&mut app, 80, 24);
    advance(&mut app, 2);
    assert!(run(&app).raid_id > raid);
    assert_eq!(
        run(&app).players[&2].name,
        "Friend",
        "the friend stays for the next delve"
    );
    assert_eq!(friend.knight(), Some(2), "on the same seat");
    assert!(app.thinking.is_none() && app.pending_turn.is_none());
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
    let invitation = link_of(&message);
    assert!(invitation.ends_with(&app.dungeon.shooter.server().unwrap().key()));
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

    let address = app.dungeon.shooter.server().unwrap().addr();
    let kicked = app.dungeon_command(Some("kick"));
    assert!(
        kicked.contains("Bob") && kicked.contains("no longer works"),
        "{kicked}"
    );
    assert!(app.dungeon.hosting.is_none());
    assert_eq!(run(&app).raid_id, raid);
    assert!(!run(&app).players.contains_key(&2));
    assert!(
        std::net::TcpStream::connect(address).is_err(),
        "the old link is dead"
    );

    // Past the entrance, hosting starts a fresh delve with the guest.
    let shooter = app.dungeon.shooter.edit().unwrap();
    shooter.descend_for_test();
    shooter.enter_for_test(1);
    assert!(!shooter.at_entrance());
    app.dungeon_command(Some("host Cleo 127.0.0.1:0"));
    assert!(run(&app).raid_id > raid);
    assert!(run(&app).at_entrance());
    assert_eq!(run(&app).players[&2].name, "Cleo");

    let address = app.dungeon.shooter.server().unwrap().addr();
    assert!(app.dungeon_command(Some("off")).contains("revoked"));
    assert!(app.dungeon.hosting.is_none() && app.dungeon.shooter.is_none());
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
        let shooter = app.dungeon.shooter.edit().unwrap();
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
            let run = app.dungeon.shooter.edit().unwrap();
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
            let run = app.dungeon.shooter.edit().unwrap();
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
            let run = app.dungeon.shooter.edit().unwrap();
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
            let run = app.dungeon.shooter.edit().unwrap();
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
            let shooter = app.dungeon.shooter.edit().unwrap();
            shooter
                .players
                .get_mut(&1)
                .unwrap()
                .carried
                .add(Spoil::Ore, 3);
            app.dungeon.cards_open = true;
            app.dungeon.cards_page = 1;
        }
        let _ = app.dungeon.shooter.edit().unwrap();
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
    let line = format!("/dungeon join {}", link_of(&message));
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.dungeon_command(Some(line.trim_start_matches("/dungeon ")));
    for _ in 0..200 {
        host.advance_dungeon_guest();
        friend.advance_joined();
        let joined = friend.dungeon.joined.as_ref().unwrap();
        if joined.with_latest(|_| ()).is_some()
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
        .edit()
        .unwrap()
        .players
        .get_mut(&2)
        .unwrap()
        .hand = vec!["potion".into(), "thunder-scroll".into()];
    press(&mut friend, KeyCode::Tab);
    for _ in 0..200 {
        host.advance_dungeon_guest();
        friend.advance_joined();
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
        app.dungeon.hosting.is_none(),
        "solo play opens no door"
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
    assert!(app.dungeon.hosting.is_none());
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
    let run = app.dungeon.shooter.edit().unwrap();
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
fn failed_realm_save_keeps_run_rewards_and_blocks_restart_or_close_until_retry() {
    use crate::drive::together_realm::{Realm, Spoil};
    use crate::drive::together_shooter::Phase;
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    let mut app = game();
    fixture.bind(&mut app);
    let raid = run(&app).raid_id;
    {
        let run = app.dungeon.shooter.edit().unwrap();
        let mut haul = crate::drive::together_realm::Spoils::default();
        haul.add(Spoil::Gold, 19);
        run.bank.push(crate::drive::together_realm::Haul {
            hero: 1,
            spoils: haul,
            why: "fixture haul".into(),
        });
        run.reclaimed.push(run.dungeon.pack);
        run.triumph = Some(crate::drive::together_shooter::Triumph::Dragon);
        run.phase = Phase::Wiped;
    }

    assert!(!app.settle_realm());
    assert_eq!(app.realm().treasury.get(Spoil::Gold), 0);
    assert_eq!(app.realm().reclaimed.values().sum::<u32>(), 0);
    assert_eq!(app.realm().raids_won, 0);
    assert_eq!(run(&app).bank.len(), 1);
    assert_eq!(run(&app).reclaimed.len(), 1);
    assert_eq!(
        run(&app).triumph,
        Some(crate::drive::together_shooter::Triumph::Dragon)
    );

    press(&mut app, KeyCode::Char('r'));
    assert_eq!(
        run(&app).raid_id,
        raid,
        "a failed bank cannot be discarded by restart"
    );
    let close = app.dungeon_command(Some("off"));
    assert!(close.contains("Could not save the realm"), "{close}");
    assert_eq!(
        run(&app).raid_id,
        raid,
        "a failed bank cannot be discarded by close"
    );

    fixture.allow_saves();
    assert!(app.settle_realm());
    assert_eq!(app.realm().treasury.get(Spoil::Gold), 19);
    assert_eq!(app.realm().reclaimed.values().sum::<u32>(), 1);
    assert_eq!(app.realm().raids_won, 1);
    assert!(run(&app).bank.is_empty());
    assert!(run(&app).reclaimed.is_empty());
    assert!(run(&app).triumph.is_none());
    let persisted = Realm::beside(app.world.rewards_path());
    assert_eq!(persisted.treasury.get(Spoil::Gold), 19);
    assert_eq!(persisted.raids_won, 1);
}

#[test]
fn failed_home_realm_save_keeps_orders_marks_and_feats_for_retry() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    let mut app = game();
    fixture.bind(&mut app);
    app.realm().treasury.add(Spoil::Gold, 200);
    app.realm().treasury.add(Spoil::Ore, 4);
    app.realm().treasury.add(Spoil::Wax, 2);
    app.realm().home.countryside.built = 2;
    app.realm().home.countryside.relics = 2;
    let surface_before = app.realm().home.countryside.clone();
    let records = crate::drive::together_shooter::pastimes::Records {
        best: [345, 0, 0, 0],
        wins: [1, 0, 0, 0],
        plays: [2, 0, 0, 0],
        hobbyhorse: Default::default(),
        sled: Default::default(),
        pot: Default::default(),
        cart: Default::default(),
        surf: Default::default(),
        volley: Default::default(),
        halfpipe: Default::default(),
        chariot: Default::default(),
        trencher: Default::default(),
        duckshoot: Default::default(),
        longbow: Default::default(),
        cheese: crate::drive::together_shooter::pastimes::Score {
            best: 400,
            wins: 1,
            plays: 2,
        },
    };
    let pending = app.dungeon.shooter.edit().unwrap();
    pending.home.countryside.games = records.clone();
    for i in 0..6 {
        pending.marks.insert(format!("country:visit:{i}"), 1);
    }
    for i in 0..3 {
        pending.marks.insert(format!("country:relic:{i}"), 1);
        pending.marks.insert(format!("country:lamp:{i}"), 1);
    }
    pending.marks.insert("country:build:bakery".into(), 1);
    app.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::Forge,
    });
    app.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("trophy:cinderjaw".into(), 1);
    app.dungeon.shooter.edit().unwrap().feats.push("roadkill");
    let pending_feats = run(&app).feats.clone();

    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury.get(Spoil::Gold), 200);
    assert_eq!(app.realm().treasury.get(Spoil::Ore), 4);
    assert_eq!(app.realm().treasury.get(Spoil::Wax), 2);
    assert_eq!(
        app.realm().home.countryside,
        surface_before,
        "all surface changes roll back on save failure"
    );
    assert_eq!(
        run(&app).home.countryside.games,
        records,
        "pending game records remain retryable"
    );
    assert_eq!(run(&app).marks.get("country:build:bakery"), Some(&1));
    assert_eq!(run(&app).marks.get("country:lamp:2"), Some(&1));
    assert_eq!(app.realm().home.level(Station::Forge), 0);
    assert!(!app.realm().home.trophies.contains_key("cinderjaw"));
    assert!(!app.realm().home.feats.contains("roadkill"));
    assert_eq!(
        run(&app).orders,
        vec![Order {
            knight: 1,
            station: Station::Forge
        }]
    );
    assert_eq!(run(&app).marks.get("trophy:cinderjaw"), Some(&1));
    assert_eq!(run(&app).feats, pending_feats);

    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury.get(Spoil::Gold), 295);
    assert_eq!(app.realm().treasury.get(Spoil::Ore), 4);
    assert_eq!(app.realm().treasury.get(Spoil::Wax), 4);
    assert!(app.realm().home.countryside.has(0));
    assert_eq!(app.realm().home.countryside.lamps, 7);
    assert!(app.realm().home.countryside.lamps_paid);
    assert!(app.realm().home.countryside.ramble_paid && app.realm().home.countryside.relics_paid);
    assert_eq!(app.realm().home.countryside.games, records);
    let surface = app.realm().home.countryside.clone();
    assert_eq!(run(&app).home.countryside, surface);
    assert_eq!(app.realm().home.level(Station::Forge), 1);
    assert_eq!(app.realm().home.trophies.get("cinderjaw"), Some(&1));
    assert!(app.realm().home.feats.contains("roadkill"));
    assert!(run(&app).orders.is_empty());
    assert!(run(&app).marks.is_empty());
    assert!(run(&app).feats.is_empty());
    let saved = app.realm().treasury.clone();
    let surface = app.realm().home.countryside.clone();
    assert!(app.settle_home());
    assert_eq!(
        app.realm().treasury,
        saved,
        "retry/empty settle never pays twice"
    );
    assert_eq!(app.realm().home.countryside, surface);
    // The western survey uses the very same rollback/retry transaction. Its
    // reward and new game record must survive a disk failure, without doubling.
    let before = app.realm().treasury.clone();
    for i in 0..3 {
        app.dungeon
            .shooter
            .edit()
            .unwrap()
            .marks
            .insert(format!("country:survey:{i}"), 1);
    }
    app.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("country:west:0".into(), 1);
    app.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("country:west:1".into(), 1);
    std::fs::remove_dir_all(fixture.home.join(".angelX")).unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    assert_eq!(app.realm().home.countryside.survey, 0);
    assert_eq!(run(&app).marks.get("country:survey:2"), Some(&1));
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.realm().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 45
    );
    assert_eq!(
        app.realm().treasury.get(Spoil::Ore),
        before.get(Spoil::Ore) + 2
    );
    assert_eq!(app.realm().home.countryside.west_visited, 3);
    assert_eq!(app.realm().home.countryside.survey, 7);
    assert!(app.realm().home.countryside.survey_paid);
    let paid = app.realm().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury, paid);
    eastern_save_retry(&mut app, &fixture);
    northern_save_retry(&mut app, &fixture);
    coastal_save_retry(&mut app, &fixture);
    woodland_save_retry(&mut app, &fixture);
    wetland_save_retry(&mut app, &fixture);
    aerie_save_retry(&mut app, &fixture);
    mummers_save_retry(&mut app, &fixture);
    hippodrome_save_retry(&mut app, &fixture);
    windcountry_save_retry(&mut app, &fixture);
}

include!("dungeon_eastern_retry.rs");
include!("dungeon_northern_retry.rs");

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
        if let Some(run) = app.dungeon.shooter.as_ref().cloned() {
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
fn dungeon_host_for_three_gives_one_link_and_each_friend_their_own_knight() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    // What `/dungeon_host --3` runs.
    let message = app.dungeon_command(Some("host --3"));
    let link = link_of(&message);
    assert_eq!(message.matches("http://").count(), 1, "one link for everyone: {message}");
    assert_eq!(run(&app).players.len(), 1, "knights join as friends arrive");
    assert!(app.dungeon_command(Some("invite")).contains(&link));

    // Three friends open the one link: each takes a seat of their own.
    let friends: Vec<_> = ["Matt", "Cleo", "Bram"]
        .into_iter()
        .map(|name| crate::drive::together_seat::Seat::connect(&link, name).unwrap())
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    while run(&app).players.len() < 4 {
        assert!(std::time::Instant::now() < deadline, "the party never gathered: {}", run(&app).players.len());
        for friend in &friends {
            friend.tick(std::time::Instant::now());
        }
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let mut knights: Vec<u32> = friends.iter().filter_map(|f| f.knight()).collect();
    knights.sort();
    assert_eq!(knights, vec![2, 3, 4]);
    for (friend, name) in friends.iter().zip(["Matt", "Cleo", "Bram"]) {
        let knight = friend.knight().unwrap();
        assert_eq!(run(&app).players[&knight].name, name);
        assert!(run(&app).players[&knight].knight.is_some(), "a knight of the company");
    }
    // Past the seats a fourth only watches.
    let watcher = crate::drive::together_seat::Seat::connect(&link, "Late").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while watcher.status() == angel_serve::client::LinkStatus::Connecting {
        assert!(std::time::Instant::now() < deadline);
        watcher.tick(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(watcher.knight(), None);
}

/// Tick a friend's seat (and the host) until the server has answered it.
fn answered(app: &mut App, seat: &crate::drive::together_seat::Seat) -> angel_serve::client::LinkStatus {
    use angel_serve::client::LinkStatus;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        seat.tick(std::time::Instant::now());
        app.advance_dungeon_guest();
        let status = seat.status();
        let seated = status == LinkStatus::Playing && seat.knight().is_some();
        if seated || matches!(status, LinkStatus::Rejected(..) | LinkStatus::Closed) {
            return status;
        }
        assert!(std::time::Instant::now() < deadline, "no answer: {status:?}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn the_host_locks_the_door_makes_a_new_link_and_kicks_one_knight() {
    use angel_net::proto::Reject;
    use angel_serve::client::LinkStatus;
    use crate::drive::together_seat::Seat;
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let link = link_of(&app.dungeon_command(Some("host --3")));

    // Locked, the link lets nobody new in; unlocked, it does.
    assert!(app.dungeon_command(Some("lock")).contains("closed to newcomers"));
    let late = Seat::connect(&link, "Late").unwrap();
    assert!(matches!(answered(&mut app, &late), LinkStatus::Rejected(Reject::Locked, _)));
    assert!(app.dungeon_command(Some("unlock")).contains("newcomers in again"));
    let matt = Seat::connect(&link, "Matt").unwrap();
    assert_eq!(answered(&mut app, &matt), LinkStatus::Playing);
    assert_eq!(matt.knight(), Some(2));

    // A new link: the old one lets nobody new in, the new one does, and a
    // friend already seated keeps their knight.
    let renewed = app.dungeon_command(Some("invite new"));
    let fresh = link_of(&renewed);
    assert_ne!(fresh, link, "{renewed}");
    let old = Seat::connect(&link, "Old").unwrap();
    assert!(matches!(answered(&mut app, &old), LinkStatus::Rejected(Reject::Key, _)));
    let cleo = Seat::connect(&fresh, "Cleo").unwrap();
    assert_eq!(answered(&mut app, &cleo), LinkStatus::Playing);
    assert_eq!(cleo.knight(), Some(3));
    assert_eq!(matt.status(), LinkStatus::Playing);

    // One knight kicked: Matt's leaves, his seat is told so, and Cleo plays on.
    let said = app.dungeon_command(Some("kick 2"));
    assert!(said.contains("Matt left the delve"), "{said}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while matt.status() == LinkStatus::Playing || run(&app).players.contains_key(&2) {
        assert!(std::time::Instant::now() < deadline, "knight 2 never left: {:?}", matt.status());
        matt.tick(std::time::Instant::now());
        cleo.tick(std::time::Instant::now());
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(matches!(answered(&mut app, &matt), LinkStatus::Rejected(Reject::Kicked, _)), "{:?}", matt.status());
    assert!(run(&app).players.contains_key(&3));
    assert_eq!(cleo.status(), LinkStatus::Playing);
    assert!(app.dungeon_command(Some("kick 2")).contains("Nobody plays knight 2"));
    assert!(app.dungeon_command(Some("kick 1")).contains("Kick a friend's knight"));
}

#[test]
fn a_friends_angelx_joins_by_pasting_its_line_and_sees_the_hosts_game() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host.dungeon_command(Some("host --1"));
    let line = format!("/dungeon join {}", link_of(&message));
    assert!(line.starts_with("/dungeon join http://"), "{line}");

    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    let joined = friend.dungeon_command(Some(line.trim_start_matches("/dungeon ")));
    assert!(joined.starts_with("Joined the delve"), "{joined}");
    assert!(friend.dungeon_view_active());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut seen = false;
    while std::time::Instant::now() < deadline {
        host.advance_dungeon_guest();
        friend.advance_joined();
        let party = friend
            .dungeon
            .joined
            .as_ref()
            .and_then(|j| j.state())
            .and_then(|s| s["players"].as_array().map(Vec::len));
        let joined = friend.dungeon.joined.as_ref().unwrap();
        let shown = joined.with_latest(|_| ()).is_some();
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
        friend.advance_joined();
        let tick = friend
            .dungeon
            .joined
            .as_ref()
            .unwrap()
            .with_latest(|run| run.tick);
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
    host.dungeon.shooter.edit().unwrap().window = Some(Default::default());
    friend
        .dungeon
        .joined
        .as_ref()
        .unwrap()
        .send(crate::drive::together_gear::BOON, br#"{"wish":"seeker"}"#);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while run(&host).players[&2].bonus.homing == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the friend's wish never landed"
        );
        host.advance_shooter_at(std::time::Instant::now());
        friend.advance_joined();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }

    // On Kitty the friend's frame reaches the terminal as a PNG. (The
    // client builds the kingdom's atlas on its first frame, as a friend's
    // terminal has long done by the time it is looked at.)
    friend.dungeon.joined.as_ref().unwrap().seat.frame(std::time::Instant::now());
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
        friend.advance_joined();
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
    friend.terminal_focused = true;
    friend.dungeon.key_releases = true;
    press(&mut friend, KeyCode::Char('a'));
    press(&mut friend, KeyCode::Char('w'));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        host.advance_shooter_at(std::time::Instant::now());
        friend.advance_joined();
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
    // Hosting again shows the same link and the menu again.
    let again = app.dungeon_command(Some("host --2"));
    assert!(again.contains(&link_of(&message)), "{again}");
    assert!(app.dungeon.intro.is_some());
}

#[test]
fn a_new_delve_while_hosting_keeps_the_seated_friends() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.dungeon_command(Some("host --3"));
    let shooter = app.dungeon.shooter.edit().unwrap();
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
    assert!(app.dungeon.hosting.is_some(), "the same link");
}

#[test]
fn the_scroll_reads_a_wish_back_and_seals_it_only_when_asked() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.terminal_focused = true;
    app.dungeon.shooter.edit().unwrap().descend_for_test();
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
    let shooter = app.dungeon.shooter.edit().unwrap();
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
    app.dungeon.shooter.edit().unwrap().window = Some(Default::default());
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
    let shooter = app.dungeon.shooter.edit().unwrap();
    shooter.descend_for_test();
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
    let shooter = app.dungeon.shooter.edit().unwrap();
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
    assert_eq!(run(&app).players[&1].spells, [None, None, None, None]);
    let now = std::time::Instant::now();
    app.dungeon.shooter.rescan_spells();
    app.advance_shooter_at(now);
    assert_eq!(
        run(&app).players[&1].spells,
        [Some("cha-ching-meteor".into()), None, None, None],
        "book-only loading must not suppress delivery into the first empty slot"
    );
    assert_eq!(run(&app).players[&2].spells, [None, None, None, None]);
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
        .edit()
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
        [Some("cha-ching-meteor".into()), None, None, None]
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
    // The spells held sit on their keys, the empty slots out of the way;
    // the second knight's are on 5, 6, 7 and backslash.
    let text = paint(&mut app, 80, 24);
    for label in ["[1]", "Spark"] {
        assert!(text.contains(label), "{label}:\n{text}");
    }
    let text = paint(&mut app, 160, 48);
    assert!(text.contains("[1]  Spark"), "{text}");
    assert!(
        text.contains("spells: none yet"),
        "the second knight's:\n{text}"
    );
    assert!(!text.contains("]  empty"), "no empty spell rows:\n{text}");
    app.dungeon.key_releases = true;
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(
        app.dungeon
            .local_inputs(std::time::Instant::now())
            .get(&1)
            .unwrap()
            .cast,
        1
    );
    for (key, slot) in [('6', 2), ('\\', 4)] {
        app.dungeon.held.clear();
        press(&mut app, KeyCode::Char(key));
        assert_eq!(
            app.dungeon
                .local_inputs(std::time::Instant::now())
                .get(&2)
                .unwrap()
                .cast,
            slot
        );
    }
    app.dungeon
        .shooter
        .edit()
        .unwrap()
        .players
        .get_mut(&1)
        .unwrap()
        .spell_cooldowns[0] = 60;
    let text = paint(&mut app, 80, 24);
    assert!(
        text.contains("[1]  Spark") && text.contains(" 2s"),
        "{text}"
    );
    assert!(!app.input.contains('1'));
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
    let line = format!("/dungeon join {}", link_of(&message));
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
fn on_a_shared_keyboard_the_first_knights_take_never_swings_the_seconds_sword() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 120, 40);
    press(&mut app, KeyCode::Char('e'));
    let inputs = app.dungeon.local_inputs(std::time::Instant::now());
    assert!(inputs[&1].take);
    assert!(!inputs[&2].swing, "E is the first knight's take only");
    app.dungeon.held.clear();
    press(&mut app, KeyCode::Char('['));
    let inputs = app.dungeon.local_inputs(std::time::Instant::now());
    assert!(inputs[&2].swing && !inputs[&1].take);
}

#[test]
fn a_friends_angelx_takes_weapons_and_casts_spells_like_the_hosts_first_knight() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host.dungeon_command(Some("host --1"));
    let line = format!("/dungeon join {}", link_of(&message));
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(line.trim_start_matches("/dungeon ")));
    assert!(friend.dungeon_view_active());
    press(&mut friend, KeyCode::Char('e'));
    let now = std::time::Instant::now();
    assert!(
        friend.dungeon.local_inputs(now)[&1].take,
        "E takes a weapon"
    );
    for (key, slot) in [('1', 1), ('2', 2), ('3', 3), ('4', 4)] {
        friend.dungeon.held.clear();
        press(&mut friend, KeyCode::Char(key));
        let now = std::time::Instant::now();
        assert_eq!(friend.dungeon.local_inputs(now)[&1].cast, slot, "{key}");
    }
    // E meets the world, X throws a bomb, Z drinks: the host's own keys.
    for (key, check) in [
        (
            'e',
            (|i: &crate::drive::together_shooter::Input| i.interact && !i.fire && !i.bomb)
                as fn(&crate::drive::together_shooter::Input) -> bool,
        ),
        ('x', |i| i.bomb && !i.interact),
        ('z', |i| i.play == 1 && i.cast == 0),
    ] {
        friend.dungeon.held.clear();
        press(&mut friend, KeyCode::Char(key));
        let now = std::time::Instant::now();
        assert!(check(&friend.dungeon.local_inputs(now)[&1]), "{key}");
    }
}

#[test]
fn g_keeps_vigil_through_the_composer_and_g_again_wakes_the_knight() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    // A vigil is kept between fights: the opening room stands empty.
    let shooter = app.dungeon.shooter.edit().unwrap();
    shooter.enemies.clear();
    shooter.phase = crate::drive::together_shooter::Phase::Exploring;
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
    let line = format!("/dungeon join {}", link_of(&message));
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

#[test]
fn the_surface_peddler_is_paid_from_the_realms_treasury() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::merchant;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.realm().treasury.add(Spoil::Gold, 100);
    app.settle_home();
    let host = app.dungeon.shooter.edit().unwrap();
    let camp = host.merchant_camp().expect("surface peddler");
    let offer = camp
        .offers
        .iter()
        .position(|o| o.goods == merchant::Goods::Bomb)
        .unwrap();
    let price = camp.offers[offer].gold;
    host.enter_for_test(camp.room);
    let (c, r) = merchant::PLATES[offer];
    let hero = host.players.get_mut(&1).unwrap();
    let tile = crate::drive::together_shooter::TILE_UNITS;
    (hero.x, hero.y) = ((c as f32 + 0.5) * tile, (r as f32 + 0.5) * tile);
    hero.bombs = 0;
    for _ in 0..merchant::STAND_TICKS {
        host.step(&std::collections::BTreeMap::new());
    }
    app.settle_home();
    assert_eq!(run(&app).players[&1].bombs, 1, "{:?}", run(&app).found);
    assert_eq!(app.realm().treasury.get(Spoil::Gold), 100 - price);
    assert_eq!(run(&app).treasury.get(Spoil::Gold), 100 - price);
    assert!(run(&app).peddled.is_empty());
}

#[test]
fn the_undercroft_spends_the_realms_treasury_and_remembers_what_it_built() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    assert!(run(&app).at_home_now());
    {
        let realm = app.realm();
        realm.treasury.add(Spoil::Gold, 200);
        realm.treasury.add(Spoil::Ore, 4);
    }
    app.settle_home();
    assert_eq!(
        run(&app).treasury.get(Spoil::Gold),
        200,
        "the run sees the treasury"
    );
    let order = Order {
        knight: 1,
        station: Station::Forge,
    };
    app.dungeon.shooter.edit().unwrap().orders.push(order);
    app.settle_home();
    assert_eq!(app.realm().home.level(Station::Forge), 1);
    assert_eq!(app.realm().treasury.get(Spoil::Gold), 50);
    assert_eq!(app.realm().treasury.get(Spoil::Ore), 0);
    assert_eq!(run(&app).home.level(Station::Forge), 1);
    assert_eq!(
        run(&app).players[&1].bonus.damage,
        8,
        "the edge is dealt at once"
    );
    assert!(
        app.dungeon.notice.contains("Tobbin's Edge I"),
        "{}",
        app.dungeon.notice
    );
    // Too poor for the next rung: nothing is paid, and the ledger says why.
    app.dungeon.shooter.edit().unwrap().orders.push(order);
    app.settle_home();
    assert_eq!(app.realm().home.level(Station::Forge), 1);
    assert_eq!(app.realm().treasury.get(Spoil::Gold), 50);
    assert!(
        app.dungeon.notice.contains("need"),
        "{}",
        app.dungeon.notice
    );
    // A new delve begins in the cellar as built.
    app.dungeon.shooter.edit().unwrap().phase = crate::drive::together_shooter::Phase::Wiped;
    app.dungeon_view_active();
    app.start_shooter(None);
    assert_eq!(run(&app).home.level(Station::Forge), 1);
    assert_eq!(run(&app).players[&1].bonus.damage, 8);
    // The deepest floor reached is remembered for the ladders that need it.
    app.dungeon.shooter.edit().unwrap().descend_for_test();
    app.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.realm().home.deepest, 2);
}

/// The Undercroft's whole screen for review, beside `dungeon_write_delve_screens`.
#[test]
#[ignore]
fn dungeon_write_home_screen() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::home::Station;
    let Some(dir) = std::env::var_os("ANGEL_DELVE_SCREEN") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = crate::tests::env_lock();
    let _backdrop = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in-process");
    let hex = |c: ratatui::style::Color| match c {
        ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => String::new(),
    };
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    {
        let realm = app.realm();
        for (s, n) in [
            (Spoil::Gold, 2200),
            (Spoil::Bone, 44),
            (Spoil::Wax, 30),
            (Spoil::Ore, 35),
            (Spoil::Gem, 12),
            (Spoil::Ember, 28),
            (Spoil::Scale, 1),
            (Spoil::Bond, 30),
        ] {
            realm.treasury.add(s, n);
        }
        realm.home.levels.insert(Station::Forge, 1);
        realm.home.levels.insert(Station::Rack, 1);
        realm.home.deepest = 3;
    }
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    app.terminal_focused = true;
    let run = app.dungeon.shooter.edit().unwrap();
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (8.0, 7.0);
    hero.buying = 12;
    for _ in 0..8 {
        run.step(&std::collections::BTreeMap::new());
    }
    let (width, height) = (160u16, 48u16);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = format!("{width} {height}\n");
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            out.push_str(
                &serde_json::to_string(&(cell.symbol(), hex(cell.fg), hex(cell.bg))).unwrap(),
            );
            out.push('\n');
        }
    }
    std::fs::write(dir.join("delve_home.jsonl"), out).unwrap();
    // Fortune's hall, the wheel just stopped.
    let run = app.dungeon.shooter.edit().unwrap();
    run.enter_for_test(1);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 17.0);
    let fire = std::collections::BTreeMap::from([(
        1,
        crate::drive::together_shooter::Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..30 {
        run.step(&fire);
    }
    for _ in 0..crate::drive::together_shooter::fortune::SPIN_TICKS {
        run.step(&std::collections::BTreeMap::new());
    }
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = format!("{width} {height}\n");
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            out.push_str(
                &serde_json::to_string(&(cell.symbol(), hex(cell.fg), hex(cell.bg))).unwrap(),
            );
            out.push('\n');
        }
    }
    std::fs::write(dir.join("delve_fortune.jsonl"), out).unwrap();
}

#[test]
fn achievements_are_kept_once_and_their_boxes_open_into_the_treasury() {
    use crate::drive::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = app
        .realm()
        .treasury
        .get(crate::drive::together_realm::Spoil::Gold);
    app.dungeon.shooter.edit().unwrap().feats.push("roadkill");
    app.settle_home();
    assert!(app.realm().home.feats.contains("roadkill"));
    assert_eq!(app.realm().home.boxes.len(), 1);
    assert!(
        app.dungeon.notice.contains("Roadkill"),
        "{}",
        app.dungeon.notice
    );
    assert!(run(&app).banner_now().is_some(), "the banner is up");
    // Noticed again: nothing more.
    app.dungeon.shooter.edit().unwrap().feats.push("roadkill");
    app.settle_home();
    assert_eq!(app.realm().home.boxes.len(), 1);
    // Opened at the coffer: into the treasury, and the run shows it.
    app.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::Coffer,
    });
    app.settle_home();
    assert!(app.realm().home.boxes.is_empty());
    assert_eq!(app.realm().home.opened, 1);
    assert!(
        app.realm()
            .treasury
            .get(crate::drive::together_realm::Spoil::Gold)
            > gold
    );
    assert!(run(&app).unboxed.is_some());
    assert!(
        app.dungeon.notice.contains("Silver Box held"),
        "{}",
        app.dungeon.notice
    );
}

#[test]
fn the_realm_keeps_its_best_show() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let best = app.realm().home.best_show;
    // A small rise waits; a tenth of a million more, or the show's end,
    // is kept.
    app.dungeon.shooter.edit().unwrap().audience = best + 40;
    app.settle_home();
    assert_eq!(app.realm().home.best_show, best);
    app.dungeon.shooter.edit().unwrap().audience = best + 140;
    app.settle_home();
    assert_eq!(app.realm().home.best_show, best + 140);
    let run_now = app.dungeon.shooter.edit().unwrap();
    run_now.audience = best + 150;
    run_now.phase = crate::drive::together_shooter::Phase::Wiped;
    app.settle_home();
    assert_eq!(app.realm().home.best_show, best + 150);
    assert_eq!(
        run(&app).home.best_show,
        best + 150,
        "the run's home shows it"
    );
}

#[test]
fn wren_pays_a_finished_bounty_and_pins_the_next() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::bounties::{self, Pinned, slay_mark};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    assert_eq!(
        app.realm().home.bounties.len(),
        bounties::PINNED,
        "pinned on arrival"
    );
    app.realm().home.bounties[0] = Pinned {
        id: "bones".into(),
        have: 24,
    };
    let gold = app.realm().treasury.get(Spoil::Gold);
    let run_now = app.dungeon.shooter.edit().unwrap();
    run_now.marks.insert(
        slay_mark(crate::drive::together_shooter::EnemyKind::Skeleton),
        1,
    );
    app.settle_home();
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold + 120);
    assert!(
        app.dungeon.notice.contains("Bone Collector"),
        "{}",
        app.dungeon.notice
    );
    assert_eq!(app.realm().home.bounties.len(), bounties::PINNED);
    assert!(app.realm().home.bounties.iter().all(|p| p.id != "bones"));
    assert_eq!(
        run(&app).treasury.get(Spoil::Gold),
        gold + 120,
        "the run shows it"
    );
}

#[test]
fn bounty_progress_is_saved_between_fights() {
    use crate::drive::together_shooter::bounties::{Pinned, slay_mark};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    app.realm().home.bounties[0] = Pinned {
        id: "bones".into(),
        have: 0,
    };
    let run_now = app.dungeon.shooter.edit().unwrap();
    run_now.phase = crate::drive::together_shooter::Phase::Fighting;
    run_now.marks.insert(
        slay_mark(crate::drive::together_shooter::EnemyKind::Skeleton),
        2,
    );
    app.settle_home();
    assert_eq!(app.realm().home.bounties[0].have, 2);
    assert!(app.dungeon.bounties_unsaved, "not saved mid-fight");
    app.dungeon.shooter.edit().unwrap().phase = crate::drive::together_shooter::Phase::Exploring;
    app.settle_home();
    assert!(
        !app.dungeon.bounties_unsaved,
        "saved once the fight is over"
    );
}

#[test]
fn the_realm_keeps_its_trophies_and_a_full_hall_is_a_feat() {
    use crate::drive::together_shooter::trophies::PLINTHS;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    app.realm().home.trophies.clear();
    let run_now = app.dungeon.shooter.edit().unwrap();
    run_now.marks.insert("trophy:cinderjaw".into(), 1);
    app.settle_home();
    assert_eq!(app.realm().home.trophies.get("cinderjaw"), Some(&1));
    assert!(
        app.dungeon.notice.contains("Cinderjaw"),
        "{}",
        app.dungeon.notice
    );
    assert_eq!(run(&app).home.trophies.get("cinderjaw"), Some(&1));
    for plinth in &PLINTHS {
        let run_now = app.dungeon.shooter.edit().unwrap();
        run_now.marks.insert(format!("trophy:{}", plinth.id), 1);
        app.settle_home();
    }
    assert_eq!(app.realm().home.trophies.get("cinderjaw"), Some(&2));
    assert!(app.realm().home.feats.contains("full_hall"));
}

#[test]
fn experience_reaches_a_level_and_sir_ector_teaches_its_lesson() {
    use crate::drive::together_shooter::home::{Order, Station};
    use crate::drive::together_shooter::talents::{LESSONS, LEVELS, knight_key};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    let key = knight_key(&run(&app).players[&1]);
    app.realm().home.knights.remove(&key);
    let run_now = app.dungeon.shooter.edit().unwrap();
    run_now.marks.insert("xp".into(), LEVELS[0]);
    app.settle_home();
    assert_eq!(app.realm().home.prowess(&key).level(), 2);
    assert!(
        app.dungeon.notice.contains("Sir Ector"),
        "{}",
        app.dungeon.notice
    );
    app.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::LessonB,
    });
    app.settle_home();
    assert_eq!(app.realm().home.prowess(&key).learned, [1]);
    assert!(app.realm().home.feats.contains("first_lesson"));
    assert!(
        app.dungeon.notice.contains(LESSONS[0][1].name),
        "{}",
        app.dungeon.notice
    );
    assert!(
        run(&app).players[&1]
            .deck
            .iter()
            .any(|c| c == "home-talent-1")
    );
}

#[test]
fn a_rescue_comes_home_to_stay_and_all_four_are_a_full_house() {
    use crate::drive::together_shooter::rescues::RESIDENTS;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    app.realm().home.residents.clear();
    app.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("rescue:mabel".into(), 1);
    app.settle_home();
    assert!(app.realm().home.residents.contains("mabel"));
    assert!(
        app.dungeon.notice.contains("Mabel"),
        "{}",
        app.dungeon.notice
    );
    assert!(run(&app).players[&1].deck.iter().any(|c| c == "home-stew"));
    for resident in &RESIDENTS {
        app.dungeon
            .shooter
            .edit()
            .unwrap()
            .marks
            .insert(format!("rescue:{}", resident.id), 1);
        app.settle_home();
    }
    assert!(app.realm().home.feats.contains("full_house"));
}

#[test]
fn the_herald_counts_every_kill_into_the_bestiary() {
    use crate::drive::together_shooter::EnemyKind;
    use crate::drive::together_shooter::bounties::slay_mark;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    app.realm().home.bestiary.clear();
    let marks = &mut app.dungeon.shooter.edit().unwrap().marks;
    marks.insert(slay_mark(EnemyKind::Skeleton), 3);
    marks.insert(slay_mark(EnemyKind::PitTyrant), 1);
    app.settle_home();
    let bestiary = &app.realm().home.bestiary;
    assert_eq!(bestiary.get("Skeleton"), Some(&3));
    assert_eq!(bestiary.get("PitTyrant"), Some(&1));
}

/// The Herald's Bestiary page of the card screen, for review.
#[test]
#[ignore]
fn dungeon_write_bestiary_screen() {
    let Some(dir) = std::env::var_os("ANGEL_DELVE_SCREEN") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = crate::tests::env_lock();
    let _backdrop = crate::tests::TestEnvGuard::set("ANGEL_BACKDROP", "in-process");
    let hex = |c: ratatui::style::Color| match c {
        ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => String::new(),
    };
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    {
        let realm = app.realm();
        for (kind, n) in [
            ("Bat", 412),
            ("Skeleton", 377),
            ("Slime", 158),
            ("Wraith", 96),
            ("Imp", 141),
            ("Sapper", 33),
            ("Necromancer", 12),
            ("Mimic", 3),
            ("Demon", 22),
            ("Dragon", 2),
            ("Goblin", 4),
        ] {
            realm.home.bestiary.insert(kind.into(), n);
        }
    }
    app.input = "/dungeon start".into();
    app.submit();
    app.terminal_focused = true;
    app.settle_home();
    let pages = crate::ui::viz::shooter_viz::card_pages(app.dungeon.shooter.as_ref().unwrap());
    app.dungeon.cards_open = true;
    app.dungeon.cards_page = pages - 1;
    let (width, height) = (160u16, 48u16);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| crate::ui::draw::ui(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut out = format!("{width} {height}\n");
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            out.push_str(
                &serde_json::to_string(&(cell.symbol(), hex(cell.fg), hex(cell.bg))).unwrap(),
            );
            out.push('\n');
        }
    }
    std::fs::write(dir.join("delve_bestiary.jsonl"), out).unwrap();
}

#[test]
fn maud_pours_a_round_from_the_treasury_one_at_a_time() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = {
        let realm = app.realm();
        realm.treasury.add(Spoil::Gold, 200);
        realm.home.residents.insert("maud".into());
        realm.home.round = None;
        realm.treasury.get(Spoil::Gold)
    };
    let pour = Order {
        knight: 1,
        station: Station::TapB,
    };
    app.dungeon.shooter.edit().unwrap().orders.push(pour);
    app.settle_home();
    assert_eq!(app.realm().home.round.as_deref(), Some("stout"));
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold - 80);
    assert_eq!(
        run(&app).home.round.as_deref(),
        Some("stout"),
        "the run sees it"
    );
    assert!(
        app.dungeon.notice.contains("Dragon's Breath"),
        "{}",
        app.dungeon.notice
    );
    // One round at a time.
    app.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::TapA,
    });
    app.settle_home();
    assert_eq!(app.realm().home.round.as_deref(), Some("stout"));
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold - 80);
    assert!(
        app.dungeon.notice.contains("waiting already"),
        "{}",
        app.dungeon.notice
    );
    // Drunk at the top of the stair: the tab is clear.
    app.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.realm().home.round, None);
    assert_eq!(run(&app).round.as_deref(), Some("stout"));
}

#[test]
fn dinadan_sings_for_gold_one_song_at_a_time() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = {
        let realm = app.realm();
        realm.treasury.add(Spoil::Gold, 200);
        realm.home.song = None;
        realm.treasury.get(Spoil::Gold)
    };
    app.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::SongC,
    });
    app.settle_home();
    assert_eq!(app.realm().home.song.as_deref(), Some("red"));
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold - 90);
    app.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::SongA,
    });
    app.settle_home();
    assert_eq!(
        app.realm().home.song.as_deref(),
        Some("red"),
        "one at a time"
    );
    app.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.realm().home.song, None, "sung, the tab is clear");
}

#[test]
fn beaumains_takes_a_wage_once_and_goes_down_with_the_party() {
    use crate::drive::together_realm::Spoil;
    use crate::drive::together_shooter::hireling::WAGE;
    use crate::drive::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = {
        let realm = app.realm();
        realm.treasury.add(Spoil::Gold, 200);
        realm.home.hire = None;
        realm.treasury.get(Spoil::Gold)
    };
    let hire = Order {
        knight: 1,
        station: Station::Hire,
    };
    app.dungeon.shooter.edit().unwrap().orders.push(hire);
    app.settle_home();
    assert_eq!(app.realm().home.hire.as_deref(), Some("beaumains"));
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold - WAGE);
    assert!(
        app.dungeon.notice.contains("Beaumains"),
        "{}",
        app.dungeon.notice
    );
    // Hired once is hired: a second wage is refused.
    app.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::Hire,
    });
    app.settle_home();
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold - WAGE);
    // Down the stair he goes, and the realm's tab is clear.
    app.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.realm().home.hire, None);
    assert!(run(&app).hireling.is_some());
}

include!("dungeon_coastal_retry.rs");

include!("dungeon_woodland_retry.rs");

include!("dungeon_wetland_retry.rs");

include!("dungeon_aerie_retry.rs");

include!("dungeon_mummers_retry.rs");

include!("dungeon_hippodrome_retry.rs");

include!("dungeon_windcountry_retry.rs");

#[test]
fn the_new_card_and_interact_keys_leave_couch_skills_alone() {
    let _lock = crate::tests::env_lock();
    let mut app = game();
    let _ = paint(&mut app, 160, 48);
    app.dungeon.key_releases = true;
    for (key, seat, check) in [
        (
            'e',
            1,
            (|i: &crate::drive::together_shooter::Input| i.interact && !i.fire && !i.bomb)
                as fn(&crate::drive::together_shooter::Input) -> bool,
        ),
        ('f', 1, |i| i.fire && !i.interact),
        ('x', 1, |i| i.bomb),
        ('4', 1, |i| i.cast == 4 && i.play == 0),
        ('z', 1, |i| i.play == 1 && i.cast == 0),
        ('h', 2, |i| i.interact && i.take && !i.fire),
        (']', 2, |i| i.play == 1 && i.skill == 0),
        ('\\', 2, |i| i.cast == 4 && i.play == 0),
        ('-', 2, |i| i.skill == 4 && i.play == 0),
    ] {
        app.dungeon.held.clear();
        press(&mut app, KeyCode::Char(key));
        assert!(
            check(&app.dungeon.local_inputs(std::time::Instant::now())[&seat]),
            "{key}"
        );
    }
    press(&mut app, KeyCode::Char('`'));
    assert!(app.dungeon.held.is_empty());
    let run = app.dungeon.shooter.edit().unwrap();
    run.players.get_mut(&1).unwrap().spells = std::array::from_fn(|_| Some("cabbage-storm".into()));
    app.dungeon.cards_open = true;
    let text = paint(&mut app, 160, 48);
    assert!(
        text.contains("4 Cabbage Storm"),
        "fourth spell renders in the card screen: {text}"
    );
}

/// A Delve in a fresh workspace, the room won and a wish window open.
fn wishing_app(name: &str) -> (App, std::path::PathBuf) {
    let workspace = std::env::temp_dir().join(format!(
        "delve-{name}-{}-{}",
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
    let shooter = app.dungeon.shooter.edit().unwrap();
    shooter.descend_for_test();
    shooter.calm_for_test();
    shooter.clear_for_test();
    // The book as the delve ships it: no other test's workspace in it.
    crate::drive::together_files::refresh_phrasebook(&workspace);
    (app, workspace)
}

fn wish(app: &mut App, words: &str) {
    press(app, KeyCode::Char('t'));
    for c in words.chars() {
        press(app, KeyCode::Char(c));
    }
    press(app, KeyCode::Enter);
    press(app, KeyCode::Enter);
}

#[test]
fn a_creative_armor_wish_is_learned_as_a_new_look_not_granted_as_stock_iron() {
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("pavise");
    wish(&mut app, "a bigger spiked shield");
    assert_eq!(app.dungeon.forge.learning.len(), 1, "{}", app.dungeon.notice);
    assert!(run(&app).players[&1].deck.iter().all(|c| !c.starts_with("wish-iron")));
    // The lesson's brief offers the worn look, and says gear is never an orbit.
    let brief = app
        .messages
        .iter()
        .rev()
        .find(|m| m.text.contains("Teach the angelX Delve's phrasebook"))
        .map(|m| m.text.to_string())
        .expect("the lesson was sent");
    assert!(brief.contains("draw worn"), "{brief}");
    assert!(brief.contains("Never use orbit"), "{brief}");
    assert!(brief.contains("max_hp"), "{brief}");
    assert!(brief.contains(".angel/dungeon/sheets"), "{brief}");
    // angelX writes an entry in its own words: found by being new.
    let book = workspace.join(".angel/dungeon/phrasebook.txt");
    std::fs::create_dir_all(book.parent().unwrap()).unwrap();
    std::fs::write(
        &book,
        format!(
            "wish spiked-pavise | Spiked Pavise\nsay tower shield | pavise\ntier armor 1 :: A pavise.\ntier armor 2 :: Heavier.\ntier armor 2; max_hp 15 :: Whole.\ndraw worn at=left\n{}\n",
            crate::drive::together_shooter::looks::PAVISE
        ),
    )
    .unwrap();
    app.thinking = None;
    app.pending_turn = None;
    app.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(app.dungeon.notice.contains("Spiked Pavise"), "{}", app.dungeon.notice);
    let run = run(&app);
    let hero = &run.players[&1];
    assert_eq!(hero.armor, 1);
    assert_eq!(hero.worn(&run.book).len(), 1, "the knight wears it");
    assert_eq!(hero.bonus.orbit, 0, "no morningstar stands in for it");
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn an_entry_the_checker_turns_away_is_sent_back_once_to_be_mended() {
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("mend");
    wish(&mut app, "tiny golden devils that explode");
    assert_eq!(app.dungeon.forge.learning.len(), 1, "{}", app.dungeon.notice);
    // A sheet-painting turn can outlast the old three-minute cutoff.
    app.dungeon.forge.learning[0].asked =
        std::time::Instant::now() - std::time::Duration::from_secs(190);
    let book = workspace.join(".angel/dungeon/phrasebook.txt");
    std::fs::create_dir_all(book.parent().unwrap()).unwrap();
    // Summons on a held card: turned away.
    std::fs::write(
        &book,
        "wish golden-devils | Golden Devils\nsay tiny golden devils that explode\ntier summon 2 swarm :: Devils.\n",
    )
    .unwrap();
    app.thinking = None;
    app.pending_turn = None;
    app.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(app.dungeon.notice.contains("mending"), "{}", app.dungeon.notice);
    let lesson = &app.dungeon.forge.learning[0];
    assert!(
        lesson
            .mending
            .as_ref()
            .is_some_and(|e| e.join(" ").contains("spell or play")),
        "{:?}",
        lesson.mending
    );
    let brief = app
        .messages
        .iter()
        .rev()
        .find(|m| m.text.contains("was turned away"))
        .map(|m| m.text.to_string())
        .expect("the mending lesson was sent");
    assert!(brief.contains("spell or play"), "{brief}");
    // Mended: a spell, and granted.
    std::fs::write(
        &book,
        "wish golden-devils | Golden Devils\nkind spell\nsay tiny golden devils that explode\ntier summon 2 swarm; cooldown 8 :: Devils.\ndraw summon\n.5.|5.5\n555|555\n",
    )
    .unwrap();
    app.thinking = None;
    app.pending_turn = None;
    app.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(app.dungeon.notice.contains("Golden Devils"), "{}", app.dungeon.notice);
    assert!(
        run(&app).players[&1]
            .spells
            .iter()
            .flatten()
            .any(|s| s.starts_with("wish-golden-devils"))
    );
    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn college_reward_save_failure_checkpoint_and_retry_pays_only_once() {
    use crate::drive::together_realm::{Realm, Spoil};
    use crate::drive::together_shooter::{Run, college, home::Home};
    let _lock = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = game();
    fixture.bind(&mut app);
    let before = app.realm().treasury.clone();
    app.realm().save().unwrap();
    let mut r = Run::at_home(711, 1, None, Home::default(), before.clone());
    r.come_to_the_gate();
    for key in [
        "college:accept:sigils",
        "college:sigil:0",
        "college:sigil:1",
        "college:sigil:2",
        "college:bookshelf",
        "college:stars",
        "college:familiar",
        "college:turn-in:sigils",
    ] {
        r.marks.insert(key.into(), 1);
    }
    app.dungeon.shooter.set(Some(r));
    std::fs::rename(
        fixture.home.join(".angelX"),
        fixture.home.join(".angelX-college-backup"),
    )
    .unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.realm().treasury, before);
    assert!(!app.realm().home.college.done.contains("sigils"));
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.dungeon.save_path = Some(path.clone());
    app.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut encoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(encoded["country_intents"]["college:turn-in:sigils"], 1);
    let mut restored = game();
    fixture.bind(&mut restored);
    restored.dungeon.shooter.set(None);
    restored.dungeon.restore(&fixture.workspace).unwrap();
    encoded["country_intents"]["college:accept:made-up"] = 1.into();
    std::fs::write(&path, serde_json::to_vec(&encoded).unwrap()).unwrap();
    assert!(restored.dungeon.restore(&fixture.workspace).is_err());
    std::fs::write(&path, &bytes).unwrap();
    std::fs::remove_file(fixture.home.join(".angelX")).unwrap();
    std::fs::rename(
        fixture.home.join(".angelX-college-backup"),
        fixture.home.join(".angelX"),
    )
    .unwrap();
    restored.dungeon.realm = None;
    assert!(restored.settle_home());
    let paid = restored.realm().treasury.clone();
    assert_eq!(paid.get(Spoil::Gold), before.get(Spoil::Gold) + 75);
    assert!(restored.realm().home.college.done.contains("sigils"));
    assert!(restored.realm().home.college.valid());
    assert!(restored.settle_home());
    assert_eq!(restored.realm().treasury, paid);
    let loaded = Realm::beside(restored.world.rewards_path());
    assert_eq!(loaded.treasury, paid);
    assert!(loaded.home.college.done.contains("sigils"));
    restored
        .dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("college:turn-in:sigils".into(), 1);
    assert!(restored.settle_home());
    assert_eq!(restored.realm().treasury, paid);
    assert!(college::intent_key("college:sample:711:1:0:0"));
    assert!(!college::intent_key("college:map:711:1:0:0"));
}

#[test]
fn dungeon_controls_activity_leave_preempts_overlays_and_only_releases_its_seat() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    let mut r = crate::drive::together_shooter::Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    r.walk_kingdom(None);
    let geo = crate::stage::world_viz::overworld::kingdom::geography(&r.kingdom.wishes);
    r.enter_kingdom_place(
        geo.sites
            .iter()
            .position(|s| s.room == crate::drive::together_shooter::RoomKind::Hippodrome)
            .unwrap(),
    );
    r.floor_plates.clear();
    r.board_chariot(1);
    app.dungeon.shooter.set(Some(r));
    paint(&mut app, 120, 40);
    assert!(paint(&mut app, 120, 40).contains("LEAVE"));
    press(&mut app, KeyCode::Char('o'));
    assert!(app.dungeon.local_inputs(std::time::Instant::now())[&2].dash);
    press(&mut app, KeyCode::Char('u'));
    assert!(app.dungeon.local_inputs(std::time::Instant::now())[&2].bomb);
    assert!(run(&app).exits.outside.is_empty());
    app.clear_dungeon_controls();
    app.dungeon.cards_open = true;
    app.dungeon.exhibit_text = Some("Overlay".into());
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.dungeon.activity_menu, Some(1));
    assert!(!app.dungeon.cards_open && app.dungeon.exhibit_text.is_none());
    assert!(paint(&mut app, 120, 40).contains("[ LEAVE ]"));
    let ticks = run(&app).chariot.as_ref().unwrap().t;
    advance(&mut app, 3);
    // Only this seat is paused: a couch friend continues the race after departure.
    press(&mut app, KeyCode::Enter);
    assert!(run(&app).exits.outside.contains_key(&1));
    assert!(run(&app).chariot.is_some());
    assert_eq!(run(&app).chariot.as_ref().unwrap().driver, 2);
    assert!(run(&app).chariot.as_ref().unwrap().t >= ticks);
    assert!(app.dungeon.held.is_empty());
    press(&mut app, KeyCode::Delete);
    assert_eq!(
        run(&app).room().kind,
        crate::drive::together_shooter::RoomKind::Kingdom
    );
    assert!(run(&app).chariot.is_none());
}

#[test]
fn dungeon_controls_joined_leave_menu_reaches_host_without_ending_other_seats() {
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    host.terminal_focused = true;
    let message = host.dungeon_command(Some("host --1"));
    let link = format!("/dungeon join {}", link_of(&message));
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(link.trim_start_matches("/dungeon ")));
    let spin = |host: &mut App, friend: &mut App, ready: fn(&App, &App) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while !ready(host, friend) {
            paint(host, 120, 40);
            paint(friend, 120, 40);
            host.advance_shooter_at(std::time::Instant::now());
            friend.advance_joined();
            assert!(
                std::time::Instant::now() < deadline,
                "exit did not reach host"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    spin(&mut host, &mut friend, |h, f| {
        run(h).players.contains_key(&2) && f.joined_knight().is_some()
    });
    let r = host.dungeon.shooter.edit().unwrap();
    r.walk_kingdom(None);
    let geo = crate::stage::world_viz::overworld::kingdom::geography(&r.kingdom.wishes);
    r.enter_kingdom_place(
        geo.sites
            .iter()
            .position(|s| s.room == crate::drive::together_shooter::RoomKind::Lists)
            .unwrap(),
    );
    r.floor_plates.clear();
    r.mount_up(2);
    spin(&mut host, &mut friend, |_, f| {
        f.can_leave_dungeon_activity(2)
    });
    friend.dungeon.cards_open = true;
    press(&mut friend, KeyCode::Esc);
    assert_eq!(friend.dungeon.activity_menu, Some(2));
    press(&mut friend, KeyCode::Enter);
    spin(&mut host, &mut friend, |h, f| {
        run(h).exits.outside.contains_key(&2) && !f.dungeon.activity_leave_pending
    });
    assert!(run(&host).joust.is_some());
    assert_eq!(run(&host).joust.as_ref().unwrap().knight, 1);
    assert_eq!(
        run(&host).view_for_seat(2).unwrap().room().kind,
        crate::drive::together_shooter::RoomKind::Kingdom
    );
    assert!(friend.dungeon.held.is_empty());
    host.dungeon_command(Some("off"));
}

#[test]
fn angelx_work_reaches_the_holdings_and_the_court_settles_at_the_save() {
    use crate::drive::together_realm::{Realm, Spoil};
    use crate::drive::together_shooter::{Pack, Run, holdings as hd};
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    fixture.bind(&mut app);
    let gold = app.realm().treasury.get(Spoil::Gold);
    // A commit that lands, a test run that fails, a subagent sent out, a
    // turn's cache hits, three loop rounds and a failed turn, as the
    // harness reports them.
    app.note_ops_call("c1", "shell", "command=git commit -m x");
    app.note_ops_result("c1", true);
    app.note_ops_call("t1", "shell", "command=cargo test --bin angel");
    app.note_ops_result("t1", false);
    app.note_ops_call("s1", "spawn", "task=look");
    app.note_ops_turn(hd::CACHE_PER_SACK * 2, 0, 1);
    app.note_ops_loop(3);
    app.note_ops_end(false);
    let h = app.realm().home.holdings.clone();
    assert_eq!((h.stores, h.retainers, h.patrols, h.unrest), (2, 1, 3, 1));
    assert_eq!(h.feed.get("BUILDS"), Some(&1));
    assert_eq!(h.feed.get("WALLS"), None, "a failed test run mends nothing");
    assert!(app.dungeon.ops.is_empty() && app.dungeon.ops_calls.is_empty());
    // Saved with the realm, and never a coin of gold.
    let again = Realm::beside(app.world.rewards_path());
    assert_eq!(again.home.holdings, h);
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold);
    // A slow turn stirs unrest too.
    app.note_ops_turn(0, 0, hd::SLOW_TURN_SECS);
    app.note_ops_end(true);
    assert_eq!(app.realm().home.holdings.unrest, 2);
    // The Court's marks settle at the save, from the realm's purse.
    app.realm()
        .home
        .holdings
        .cleared
        .insert(Pack::Crypt.name().to_string());
    app.realm().treasury.add(Spoil::Gold, 7000);
    let (home, purse) = (app.realm().home.clone(), app.realm().treasury.clone());
    app.dungeon.shooter.set(Some(Run::at_home(3, 1, None, home, purse)));
    app.dungeon
        .shooter
        .as_mut()
        .unwrap()
        .marks
        .insert(hd::buy_mark(hd::Deed::Floor(Pack::Crypt), false), 1);
    assert!(app.settle_home());
    assert_eq!(app.realm().treasury.get(Spoil::Gold), gold + 1000);
    let key = hd::Deed::Floor(Pack::Crypt).key();
    assert!(app.realm().home.holdings.holding(&key).is_some());
    assert!(run(&app).home.holdings.holding(&key).is_some(), "the run sees it");
    assert_eq!(run(&app).treasury.get(Spoil::Gold), gold + 1000);
}

#[test]
fn dungeon_tune_lists_the_knobs_and_sets_one_for_everyone() {
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    assert!(app.dungeon_command(Some("tune")).contains("No delve is running"));
    app.dungeon_command(Some("start"));
    let list = app.dungeon_command(Some("tune"));
    assert!(list.contains("hero_speed = 13  Walk pace (units/s) [4..30]"), "{list}");
    assert!(list.contains("snapshot_every = 1"), "{list}");
    let set = app.dungeon_command(Some("tune hero_speed 20"));
    assert!(set.starts_with("hero_speed = 20 for everyone"), "{set}");
    assert!(app.dungeon_command(Some("tune hero_speed fast")).contains("is not a number"));
    assert!(app.dungeon_command(Some("tune nope 1")).contains("no knob nope"));
    // Taken between the next two ticks.
    let now = std::time::Instant::now();
    app.dungeon.shooter.pump(now);
    app.dungeon.shooter.pump(now + crate::drive::together_sim::STEP * 2);
    assert!(app.dungeon_command(Some("tune")).contains("hero_speed = 20 (code: 13)"));
    assert!(app.dungeon_command(Some("help")).contains("/dungeon tune [name value]"));
}

/// Step the host until `ready`, every friend's seat sending its keys.
fn host_all_until(app: &mut App, seats: &[&crate::drive::together_seat::Seat], what: &str, ready: impl Fn(&App) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !ready(app) {
        assert!(std::time::Instant::now() < deadline, "{what}");
        for seat in seats {
            seat.tick(std::time::Instant::now());
        }
        let now = app
            .dungeon
            .shooter
            .clock()
            .map_or_else(std::time::Instant::now, |last| last + std::time::Duration::from_nanos(33_333_333));
        app.advance_shooter_at(now);
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
}

#[test]
fn friends_take_a_fallen_party_back_to_the_kingdom_on_a_host_with_no_knight() {
    use angel_net::input::button;
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.dungeon_command(Some("host --watch --2"));
    press(&mut app, KeyCode::Enter);
    paint(&mut app, 80, 24);
    advance(&mut app, 0);
    let a = friend_seat(&mut app);
    let b = friend_seat(&mut app);
    let (ka, kb) = (a.knight().unwrap(), b.knight().unwrap());
    host_all_until(&mut app, &[&a, &b], "the friends never played", |app| {
        let r = run(app);
        r.players.get(&ka).is_some_and(|h| !h.away) && r.players.get(&kb).is_some_and(|h| !h.away) && r.players[&1].absent
    });
    assert_eq!(run(&app).party_len(), 2, "two people, a party of two");
    // The party falls.
    {
        let mut r = app.dungeon.shooter.edit().unwrap();
        for hero in r.players.values_mut() {
            hero.hp = 0;
        }
        r.phase = crate::drive::together_shooter::Phase::Wiped;
    }
    let raid = run(&app).raid_id;
    // One asks: the party waits for the other.
    a.set_held(button::OK);
    host_all_until(&mut app, &[&a, &b], "the first ask never reached the host", |app| run(app).again.is_some());
    a.set_held(0);
    assert_eq!(run(&app).raid_id, raid, "nobody yanked from the verdict");
    // The other asks: a new run in the kingdom, both alive, the same link.
    b.set_held(button::OK);
    host_all_until(&mut app, &[&a, &b], "the party never went back", |app| run(app).raid_id > raid);
    b.set_held(0);
    let r = run(&app);
    assert_eq!(r.room().kind, crate::drive::together_shooter::RoomKind::Kingdom);
    assert!(r.active());
    for k in [ka, kb] {
        assert!(r.players[&k].hp > 0, "knight {k} back on its feet");
    }
    assert!(app.dungeon.hosting.is_some(), "the same link");
}
