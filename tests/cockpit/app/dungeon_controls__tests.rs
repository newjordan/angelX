use super::*;
use ratatui::{Terminal, backend::TestBackend};

/// A host command, asked again while a dropped host's browser generations
/// drain. Admission is nonblocking by design (`serve::http::preparation`, two
/// generations at once), and since the Delve's bundle grew to about a thousand
/// files and a 44 MB wasm (2026-11-07) a cancelled preparation outlives the
/// test that started it by seconds, so the next test is told "busy".
macro_rules! host_when_admitted {
    ($app:expr, $args:expr) => {{
        let started = std::time::Instant::now();
        loop {
            let said = $app.dungeon_command(Some($args));
            if !said.contains("browser generations busy")
                || started.elapsed() > std::time::Duration::from_secs(180)
            {
                break said;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    }};
}

#[test]
fn joined_angelx_ballot_taps_reach_the_host_in_order_and_close_locally() {
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host_when_admitted!(host, "host --1");
    let link = link_of(&message);
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(&format!("join {link}")));
    let spin = |stage: &str, host: &mut App, friend: &mut App, ready: fn(&App, &App) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while !ready(host, friend) {
            host.advance_shooter_at(std::time::Instant::now());
            friend.advance_joined();
            assert!(
                std::time::Instant::now() < deadline,
                "ballot never reached the host ({stage})"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    spin("the friend's seat and ballot", &mut host, &mut friend, |host, friend| {
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
    spin("the friend's votes", &mut host, &mut friend, |host, _| {
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
        crate::delve::together_shooter::votes::Rating::Stars(5)
    );
    assert_eq!(
        votes["hud"],
        crate::delve::together_shooter::votes::Rating::Stars(1)
    );
    assert!(hosted_run.playtest.open(2) && !hosted_run.playtest.open(1));
    press(&mut friend, KeyCode::Esc);
    assert!(friend.dungeon.expanded && !friend.dungeon.joined_ballot_open);
    spin("the ballot's close", &mut host, &mut friend, |host, _| {
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
    let muted = app.realm.dungeon.audio.voices.muted;
    press(&mut app, KeyCode::Char('v'));
    assert!(run(&app).playtest.open(1));
    assert!(!run(&app).playtest.open(2));
    assert_eq!(app.realm.dungeon.audio.voices.muted, muted);
    let host = (run(&app).players[&1].x, run(&app).players[&1].y);
    press(&mut app, KeyCode::Char('f'));
    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Char('i'));
    assert!(app.realm.dungeon.local_inputs(std::time::Instant::now())[&2].move_y < 0);
    advance(&mut app, 2);
    assert_eq!(host, (run(&app).players[&1].x, run(&app).players[&1].y));
    press(&mut app, KeyCode::Char('4'));
    assert_eq!(
        app.economy().votes[&host_name]["controls"],
        crate::delve::together_shooter::votes::Rating::Stars(4)
    );
    let realm_path = app
        .world
        .rewards_path()
        .unwrap()
        .with_extension("realm.json");
    let saved: crate::delve::together_realm::Realm =
        serde_json::from_slice(&std::fs::read(realm_path).unwrap()).unwrap();
    assert_eq!(
        saved.votes[&host_name]["controls"],
        crate::delve::together_shooter::votes::Rating::Stars(4)
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
        app.economy().votes["Test Guest"]["controls"],
        crate::delve::together_shooter::votes::Rating::Stars(5)
    );
    press(&mut app, KeyCode::Char('v'));
    press(&mut app, KeyCode::Char('l'));
    press(&mut app, KeyCode::Char('5'));
    assert_eq!(
        app.economy().votes["Test Guest"]["controls"],
        crate::delve::together_shooter::votes::Rating::Stars(1)
    );
    assert_eq!(
        app.economy().votes[&host_name]["controls"],
        crate::delve::together_shooter::votes::Rating::Stars(4)
    );
    press(&mut app, KeyCode::Esc);
    assert!(!run(&app).playtest.open(2));
    assert!(run(&app).playtest.open(1));
    press(&mut app, KeyCode::Esc);
    assert!(!run(&app).playtest.open(1));
    assert!(app.realm.dungeon.expanded);
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(app.realm.dungeon.audio.voices.muted, muted, "K still walks P2");
    app.on_key(event::KeyEvent::new(
        KeyCode::Char('K'),
        KeyModifiers::SHIFT,
    ));
    assert_ne!(app.realm.dungeon.audio.voices.muted, muted);
    app.dungeon_command(Some("off"));
    app.dungeon_command(Some("start"));
    press(&mut app, KeyCode::Char('k'));
    assert_eq!(
        app.realm.dungeon.audio.voices.muted, muted,
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
    app.realm.dungeon.shooter.edit().unwrap().descend_for_test();
    app
}

fn pending_home(app: &App) -> angel_delve::rules::home_settlement::HomeClaim {
    app.realm.dungeon.shooter.pending_home().unwrap().expect("retained home claim")
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
        app.realm.world = crate::delve::world_viz::World::for_workspace(&self.workspace);
        app.realm.dungeon.realm = None;
        app.realm.dungeon.save_path = Some(self.workspace.join(".angel/dungeon/session.json"));
    }

    fn block_saves(&self) {
        // Simulate an inaccessible store without deleting the authoritative
        // balances/receipts that the next compare-and-swap must observe.
        std::fs::rename(self.home.join(".angelX"), self.home.join(".angelX-save-backup")).unwrap();
        std::fs::write(self.home.join(".angelX"), b"not a directory").unwrap();
    }
    fn allow_saves(&self) {
        std::fs::remove_file(self.home.join(".angelX")).unwrap();
        let backup = self.home.join(".angelX-save-backup");
        if backup.exists() {
            std::fs::rename(backup, self.home.join(".angelX")).unwrap();
        } else {
            std::fs::create_dir_all(self.home.join(".angelX/world-rewards")).unwrap();
        }
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

/// Wishes knight `id` holds, plain and super.
fn held(app: &App, id: u32) -> u8 {
    let hero = &run(app).players[&id];
    hero.wishes + hero.great_wishes
}

fn run(app: &App) -> &crate::delve::together_shooter::Run {
    app.realm.dungeon.shooter.as_ref().unwrap()
}

fn advance(app: &mut App, ticks: u32) {
    let now = app.realm.dungeon.shooter.clock().unwrap_or_else(std::time::Instant::now);
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
    assert!(app.realm.dungeon.expanded);
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
        assert!(app.realm.dungeon.controls_visible && app.needs_fast_tick());
        assert!(!app.world_pane_visible && app.world_buttons.is_empty());
    }
    assert!(!input::parse("/dungeon start").unwrap().needs_idle());
    app.dungeon_command(Some("off"));
    app.dungeon_command(Some("start"));
    assert_eq!(run(&app).players.len(), 1, "no guest name starts solo");
}

#[test]
fn dungeon_p_and_end_bring_each_couch_knights_next_card_up_once_a_press() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    app.realm.dungeon.key_releases = true;
    for id in [1, 2] {
        let hero = app.realm.dungeon.shooter.edit().unwrap().players.get_mut(&id).unwrap();
        hero.hand = vec!["potion".into(), "holy-water".into(), "thunder-scroll".into()];
    }
    // Held, each key turns its own knight's hand once.
    press(&mut app, KeyCode::Char('p'));
    press(&mut app, KeyCode::End);
    advance(&mut app, 6);
    for id in [1, 2] {
        assert_eq!(run(&app).players[&id].hand, ["holy-water", "thunder-scroll", "potion"], "knight {id}");
    }
}

#[test]
fn dungeon_held_movement_and_aim_do_not_wait_for_another_player_or_touch_composer() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    app.realm.dungeon.key_releases = true;
    let messages = app.messages.len();
    // Knights start in their smallclothes; arm both so a held aim shoots.
    for id in [1, 2] {
        let hero = app.realm.dungeon.shooter.edit().unwrap().players.get_mut(&id).unwrap();
        hero.weapon = crate::delve::together_shooter::Weapon::Bow;
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
fn dungeon_n_held_lays_the_floor_over_the_first_knights_screen_and_moves_no_one() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 80, 24);
    app.realm.dungeon.key_releases = true;
    advance(&mut app, 3);
    let seat = app.dungeon.shooter.seat(1).expect("the first knight's seat");
    let expected_tick = run(&app).tick;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while seat.run().is_none_or(|view| view.tick < expected_tick) && std::time::Instant::now() < until {
        seat.tick(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(seat.run().is_some_and(|view| view.tick >= expected_tick), "seat received the latest run");
    let now = std::time::Instant::now();
    let shot = |app: &App| {
        let seat = app.realm.dungeon.shooter.seat(1).expect("the first knight's seat");
        seat.frame(now).expect("a picture").px
    };
    let plain = shot(&app);
    assert!(shot(&app) == plain, "the same moment, the same picture");
    let (x, tick) = (run(&app).players[&1].x, run(&app).tick);
    // Held, the whole floor is on the seat's own screen...
    press(&mut app, KeyCode::Char('n'));
    assert!(app.realm.dungeon.held.contains_key(&KeyCode::Char('n')));
    let up = shot(&app);
    let changed = up.iter().zip(plain.iter()).filter(|(a, b)| a != b).count();
    assert!(changed > 20_000, "the map covers the view ({changed} pixels changed)");
    // ...and nobody walks, aims or fires for it.
    advance(&mut app, 12);
    assert_eq!(run(&app).players[&1].x, x);
    assert!(run(&app).tick > tick && run(&app).projectiles.iter().all(|p| p.hostile));
    // Let go, and the map is gone from it.
    app.on_key(event::KeyEvent::new_with_kind(
        KeyCode::Char('n'),
        KeyModifiers::NONE,
        event::KeyEventKind::Release,
    ));
    assert!(!app.realm.dungeon.held.contains_key(&KeyCode::Char('n')));
    let gone = shot(&app).iter().zip(up.iter()).filter(|(a, b)| a != b).count();
    assert!(gone > 20_000, "the map is off the view ({gone} pixels changed)");
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
    assert!(app.realm.dungeon.key_releases);
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
    assert!(app.realm.dungeon.held.is_empty());
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
    let seat = app.dungeon.shooter.seat(1).expect("the first knight's seat").clone();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while seat.run().is_none() && std::time::Instant::now() < until {
        advance(&mut app, 1);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(seat.run().is_some(), "seat joined before the key press");
    press(&mut app, KeyCode::Char('d'));
    advance(&mut app, 3);
    let x = run(&app).players[&1].x;
    let now = app.realm.dungeon.shooter.clock().unwrap();
    let tick = run(&app).tick;
    app.advance_shooter_at(now + std::time::Duration::from_secs(30));
    assert_eq!(
        run(&app).tick,
        tick + 1,
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
    app.realm.dungeon.key_releases = true;
    press(&mut app, KeyCode::Char('d'));
    advance(&mut app, 3);
    let tick = run(&app).tick;
    press(&mut app, KeyCode::Esc);
    advance(&mut app, 30);
    assert_eq!(run(&app).tick, tick);
    assert!(app.realm.dungeon.held.is_empty());
    press(&mut app, KeyCode::Char('f'));
    assert_eq!(app.input, "f");
    press(&mut app, KeyCode::F(6));
    assert!(app.realm.dungeon.expanded);
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
    assert!(app.realm.dungeon.held.is_empty());
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
    assert!(!app.realm.dungeon.expanded);
    assert_eq!(app.input, "draft text");
    assert!(app.realm.dungeon.held.is_empty());
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
    assert!(app.realm.dungeon.held.is_empty());
    for (width, height) in [(40, 12), (80, 10), (0, 0)] {
        paint(&mut app, width, height);
        press(&mut app, KeyCode::Char('d'));
        advance(&mut app, 30);
        assert_eq!(run(&app).tick, 0);
        assert!(app.realm.dungeon.held.is_empty());
    }
    press(&mut app, KeyCode::Esc);
    assert!(!app.realm.dungeon.expanded);
}

#[test]
fn dungeon_restart_requires_finished_run_and_retains_players_with_new_generation() {
    use crate::delve::together_shooter::Phase;
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
        app.realm.dungeon.notice.contains("charging"),
        "{}",
        app.realm.dungeon.notice
    );
    app.realm.dungeon.shooter.edit().unwrap().phase = Phase::Wiped;
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
fn friend_seat(app: &mut App) -> crate::delve::together_seat::Seat {
    let server = app.realm.dungeon.shooter.server().unwrap();
    let link = angel_serve::link::link(&format!("http://{}", server.addr()), &server.key());
    let seat = crate::delve::together_seat::Seat::connect(&link, "Friend").unwrap();
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
fn host_until(app: &mut App, seat: &crate::delve::together_seat::Seat, what: &str, ready: impl Fn(&App) -> bool) {
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
    let message = host_when_admitted!(app, "host Test Guest 127.0.0.1:0");
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
    assert_eq!((picture.w, picture.h), crate::delve::world_viz::overworld::arena::seat_size());

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
    app.realm.dungeon.shooter.edit().unwrap().phase = crate::delve::together_shooter::Phase::Won;
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

    let message = host_when_admitted!(app, "host Bob");
    assert_eq!(
        run(&app).raid_id,
        raid,
        "the entrance-room delve gains player 2"
    );
    assert_eq!(run(&app).players[&2].name, "Bob");
    let invitation = link_of(&message);
    assert!(invitation.ends_with(&app.realm.dungeon.shooter.server().unwrap().key()));
    assert!(app.dungeon_command(Some("invite")).contains(&invitation));
    assert!(
        host_when_admitted!(app, "host Someone Else")
            .contains(&invitation)
    );
    assert!(
        app.dungeon_command(Some("status"))
            .contains("friends invited")
    );
    assert!(app.dungeon_command(Some("help")).contains("/dungeon_host"));

    let address = app.realm.dungeon.shooter.server().unwrap().addr();
    let kicked = app.dungeon_command(Some("kick"));
    assert!(
        kicked.contains("Bob") && kicked.contains("no longer works"),
        "{kicked}"
    );
    assert!(app.realm.dungeon.hosting.is_none());
    assert_eq!(run(&app).raid_id, raid);
    assert!(!run(&app).players.contains_key(&2));
    assert!(
        std::net::TcpStream::connect(address).is_err(),
        "the old link is dead"
    );

    // Past the entrance, hosting starts a fresh delve with the guest.
    let shooter = app.realm.dungeon.shooter.edit().unwrap();
    shooter.descend_for_test();
    shooter.enter_for_test(1);
    assert!(!shooter.at_entrance());
    host_when_admitted!(app, "host Cleo 127.0.0.1:0");
    assert!(run(&app).raid_id > raid);
    assert!(run(&app).at_entrance());
    assert_eq!(run(&app).players[&2].name, "Cleo");

    let address = app.realm.dungeon.shooter.server().unwrap().addr();
    assert!(app.dungeon_command(Some("off")).contains("revoked"));
    assert!(app.realm.dungeon.hosting.is_none() && app.realm.dungeon.shooter.is_none());
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
        let shooter = app.realm.dungeon.shooter.edit().unwrap();
        let fight = shooter
            .dungeon
            .rooms
            .iter()
            .position(|r| r.kind == crate::delve::together_shooter::RoomKind::Fight)
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
            let run = app.realm.dungeon.shooter.edit().unwrap();
            run.descend_for_test();
            app.realm.dungeon.forge.draft = Some(crate::delve::ReforgeDraft {
                player: 1,
                part: crate::delve::together_shooter::knights::Part::Defense,
                words: "a trip wall shield my buddy can shoot through, 3x wide, costs mana".into(),
                upgrade_page: 0,
                confirm: (name == "delve_wish_confirm").then_some(true),
                pick: None,
                known: None,
                sanctuary: true,
                hub_row: 0,
            });
        }
        if name == "delve_sanctuary1" {
            // The first floor's guardian has just fallen.
            let run = app.realm.dungeon.shooter.edit().unwrap();
            let stairs = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == crate::delve::together_shooter::RoomKind::Stairs)
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
            let run = app.realm.dungeon.shooter.edit().unwrap();
            let hall = run
                .dungeon
                .rooms
                .iter()
                .position(|r| r.kind == crate::delve::together_shooter::RoomKind::Ledge)
                .unwrap();
            run.enter_for_test(hall);
            for _ in 0..40 {
                run.step(&std::collections::BTreeMap::new());
            }
        }
        if name == "delve_wish_scroll" || name == "delve_wish_seal" {
            let run = app.realm.dungeon.shooter.edit().unwrap();
            run.calm_for_test();
            run.clear_for_test();
            let mut draft = crate::delve::ReforgeDraft::new(1, false);
            draft.pick = Some("volley".into());
            if name == "delve_wish_seal" {
                draft.known = draft.pick.clone();
                draft.confirm = Some(true);
            }
            app.realm.dungeon.forge.draft = Some(draft);
        }
        if name == "delve_intro" {
            app.open_intro();
            app.realm.dungeon.intro = Some(crate::delve::delve_intro_viz::Intro {
                knight: 3,
                ..app.realm.dungeon.intro.take().unwrap()
            });
        }
        if name == "delve_realm" {
            use crate::delve::together_realm::{Spoil, Spoils, check};
            let realm = app.economy();
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
            let shooter = app.realm.dungeon.shooter.edit().unwrap();
            shooter
                .players
                .get_mut(&1)
                .unwrap()
                .carried
                .add(Spoil::Ore, 3);
            app.realm.dungeon.cards_open = true;
            app.realm.dungeon.cards_page = 1;
        }
        let _ = app.realm.dungeon.shooter.edit().unwrap();
        if name == "delve_cards" || name == "delve_book" {
            app.realm.dungeon.cards_open = true;
            app.realm.dungeon.cards_sel = 2;
            app.realm.dungeon.cards_page = usize::from(name == "delve_book");
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
    let message = host_when_admitted!(host, "host --2");
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
        app.realm.dungeon.expanded && app.realm.dungeon.intro.is_some(),
        "the way in is the Delve's menu"
    );
    assert!(app.input.is_empty());
    let menu = paint(&mut app, 120, 40);
    assert!(menu.contains("CONTINUE THE DELVE"), "{menu}");
    press(&mut app, KeyCode::Enter);
    assert!(paint(&mut app, 120, 40).contains("DUNGEON DELVE"));
    assert!(
        app.realm.dungeon.hosting.is_none(),
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
    assert!(!app.realm.dungeon.controls_visible);
    assert!(
        app.realm.world.delve_called && app.realm.world.delve_lit,
        "the knight waits at the lit gate"
    );
    let rect = app
        .panes
        .rect_of(crate::ui::mouse::PaneId::Artifacts)
        .expect("the mini-viz pane");
    // Only the gate drawn on the map is the way in, not the whole pane
    // (here the map looks elsewhere, the gate out of its view): a click on
    // the mini-viz leaves the draft alone, and F4 is the way back in.
    let (column, row) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert!(!app.delve_map_hit(column, row), "the gate is out of the map's view here");
    app.on_mouse(event::MouseEvent {
        kind: event::MouseEventKind::Down(event::MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
    assert!(
        app.realm.dungeon.intro.is_none(),
        "a click on the mini-viz away from the gate does not open the Delve"
    );
    assert_eq!(app.input, "Make the sword swing heavier");
    press(&mut app, KeyCode::F(4));
    assert!(
        app.realm.dungeon.intro.is_some(),
        "F4 opens the Delve's menu again"
    );
    press(&mut app, KeyCode::Enter);
    assert!(app.realm.dungeon.expanded && app.input.is_empty());
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
    assert!(!app.realm.dungeon.expanded);
    assert!(app.realm.dungeon.shooter.is_none());
    assert!(app.realm.dungeon.hosting.is_none());
    assert_eq!(app.input, "Keep this unfinished thought");
    assert_eq!(app.cursor, 7);
}

#[test]
fn dungeon_tab_opens_the_card_screen_which_pauses_and_esc_closes_it() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    paint(&mut app, 160, 48);
    press(&mut app, KeyCode::Tab);
    assert!(app.realm.dungeon.cards_open);
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
    assert!(!app.realm.dungeon.cards_open);
    assert!(
        app.realm.dungeon.expanded,
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
    let run = app.realm.dungeon.shooter.edit().unwrap();
    run.players
        .get_mut(&1)
        .unwrap()
        .carried
        .add(crate::delve::together_realm::Spoil::Bone, 6);
    run.players
        .get_mut(&1)
        .unwrap()
        .carried
        .add(crate::delve::together_realm::Spoil::Wax, 2);
    run.retreat();
    app.settle_realm();
    let host = app.host_name();
    assert!(
        app.realm.dungeon.notice.contains(&format!(
            "{host} banked 3 bone · 1 wax (the party turned back)"
        )),
        "{}",
        app.realm.dungeon.notice
    );
    assert_eq!(app.economy().treasury.label(), "3 bone · 1 wax");
    assert!(
        app.realm.dungeon.shooter.as_ref().unwrap().bank.is_empty(),
        "banked once"
    );
    app.settle_realm();
    assert_eq!(app.economy().treasury.label(), "3 bone · 1 wax");

    assert!(
        app.dungeon_command(Some("wish a candle shrine"))
            .contains("wished for")
    );
    let id = app
        .economy()
        .wishes
        .iter()
        .find(|w| w.words == "a candle shrine")
        .unwrap()
        .id
        .clone();
    let (mut draft, _) = crate::delve::together_realm::check(
        &id,
        "name Candle Shrine\nprice bone 3, wax 1\nart\n.5.\nhHh\n",
    )
    .unwrap();
    draft.id = id.clone();
    app.economy().draft(draft).unwrap();
    let listed = app.dungeon_command(Some("wishes"));
    assert!(
        listed.contains("Candle Shrine") && listed.contains("ready"),
        "{listed}"
    );
    assert!(
        app.dungeon_command(Some(&format!("grant {id}")))
            .contains("rises")
    );
    assert!(app.economy().treasury.is_empty());
    assert_eq!(app.economy().built().len(), 1);
}

#[test]
fn failed_realm_save_keeps_run_rewards_and_blocks_restart_or_close_until_retry() {
    use crate::delve::together_realm::{Realm, Spoil};
    use crate::delve::together_shooter::Phase;
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    let mut app = game();
    fixture.bind(&mut app);
    let raid = run(&app).raid_id;
    {
        let run = app.realm.dungeon.shooter.edit().unwrap();
        let mut haul = crate::delve::together_realm::Spoils::default();
        haul.add(Spoil::Gold, 19);
        run.bank.push(crate::delve::together_realm::Haul {
            hero: 1,
            spoils: haul,
            why: "fixture haul".into(),
        });
        run.reclaimed.push(run.dungeon.pack);
        run.triumph = Some(crate::delve::together_shooter::Triumph::Dragon);
        run.phase = Phase::Wiped;
    }

    assert!(!app.settle_realm());
    assert_eq!(app.economy().treasury.get(Spoil::Gold), 0);
    assert_eq!(app.economy().reclaimed.values().sum::<u32>(), 0);
    assert_eq!(app.economy().raids_won, 0);
    assert!(
        run(&app).bank.is_empty() && run(&app).reclaimed.is_empty() && run(&app).triumph.is_none()
    );
    let retained = app.realm.dungeon.shooter.claim_realm().unwrap().unwrap();
    assert_eq!(retained.transfer.hauls.len(), 1);
    assert_eq!(retained.transfer.reclaimed.len(), 1);
    assert_eq!(
        retained.transfer.triumph,
        Some(crate::delve::together_shooter::Triumph::Dragon)
    );
    let checkpoint: serde_json::Value =
        serde_json::from_slice(&std::fs::read(app.realm.dungeon.save_path.as_ref().unwrap()).unwrap())
            .unwrap();
    assert_eq!(checkpoint["version"], 7);
    assert_eq!(checkpoint["commands"]["session"], retained.session);

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
    assert_eq!(app.economy().treasury.get(Spoil::Gold), 19);
    assert_eq!(app.economy().reclaimed.values().sum::<u32>(), 1);
    assert_eq!(app.economy().raids_won, 1);
    assert!(run(&app).bank.is_empty());
    assert!(run(&app).reclaimed.is_empty());
    assert!(run(&app).triumph.is_none());
    let persisted = Realm::beside(app.realm.world.rewards_path());
    assert_eq!(persisted.treasury.get(Spoil::Gold), 19);
    assert_eq!(persisted.raids_won, 1);
}
#[test]
fn failed_home_realm_save_keeps_orders_marks_and_feats_for_retry() {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    let mut app = game();
    fixture.bind(&mut app);
    app.economy().treasury.add(Spoil::Gold, 200);
    app.economy().treasury.add(Spoil::Ore, 4);
    app.economy().treasury.add(Spoil::Wax, 2);
    app.economy().home.countryside.built = 2;
    app.economy().home.countryside.relics = 2;
    let surface_before = app.economy().home.countryside.clone();
    let records = crate::delve::together_shooter::pastimes::Records {
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
        hop: Default::default(),
        cinder: crate::delve::together_shooter::pastimes::Score {best:37,wins:1,plays:1},
        latch: Default::default(),
        mossbell: Default::default(),
        rafterwake: Default::default(),
        pennon: Default::default(),
        roundtable: Default::default(),
        brasswake: crate::delve::together_shooter::pastimes::Score { best: 345, wins: 1, plays: 2 },
        powder: Default::default(),
        gander: Default::default(),
        siege: Default::default(),
        peal: Default::default(),
        cheese: crate::delve::together_shooter::pastimes::Score {
            best: 400,
            wins: 1,
            plays: 2,
        },
    };
    let pending = app.realm.dungeon.shooter.edit().unwrap();
    pending.home.countryside.games = records.clone();
    for i in 0..6 {
        pending.marks.insert(format!("country:visit:{i}"), 1);
    }
    for i in 0..3 {
        pending.marks.insert(format!("country:relic:{i}"), 1);
        pending.marks.insert(format!("country:lamp:{i}"), 1);
    }
    pending.marks.insert("country:build:bakery".into(), 1);
    app.realm.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::Forge,
    });
    app.realm.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("trophy:cinderjaw".into(), 1);
    app.realm.dungeon.shooter.edit().unwrap().feats.push("roadkill");
    let pending_feats = run(&app).feats.clone();

    assert!(!app.settle_home());
    assert_eq!(app.economy().treasury.get(Spoil::Gold), 200);
    assert_eq!(app.economy().treasury.get(Spoil::Ore), 4);
    assert_eq!(app.economy().treasury.get(Spoil::Wax), 2);
    assert_eq!(
        app.economy().home.countryside,
        surface_before,
        "all surface changes roll back on save failure"
    );
    assert_eq!(
        run(&app).home.countryside.games,
        records,
        "pending game records remain retryable"
    );
    assert_eq!(pending_home(&app).transfer.marks.get("country:build:bakery"), Some(&1));
    assert_eq!(pending_home(&app).transfer.marks.get("country:lamp:2"), Some(&1));
    assert_eq!(app.economy().home.level(Station::Forge), 0);
    assert!(!app.economy().home.trophies.contains_key("cinderjaw"));
    assert!(!app.economy().home.feats.contains("roadkill"));
    assert_eq!(
        pending_home(&app).transfer.orders,
        vec![Order {
            knight: 1,
            station: Station::Forge
        }]
    );
    assert_eq!(pending_home(&app).transfer.marks.get("trophy:cinderjaw"), Some(&1));
    assert_eq!(pending_home(&app).transfer.noticed, pending_feats);
    assert_eq!(run(&app).orders, pending_home(&app).transfer.orders);
    assert_eq!(run(&app).marks, pending_home(&app).transfer.marks);
    assert_eq!(run(&app).feats, pending_feats, "quests and purchases remain visible while saves fail");

    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(app.economy().treasury.get(Spoil::Gold), 295);
    assert_eq!(app.economy().treasury.get(Spoil::Ore), 4);
    assert_eq!(app.economy().treasury.get(Spoil::Wax), 4);
    assert!(app.economy().home.countryside.has(0));
    assert_eq!(app.economy().home.countryside.lamps, 7);
    assert!(app.economy().home.countryside.lamps_paid);
    assert!(app.economy().home.countryside.ramble_paid && app.economy().home.countryside.relics_paid);
    assert_eq!(app.economy().home.countryside.games, records);
    let surface = app.economy().home.countryside.clone();
    assert_eq!(run(&app).home.countryside, surface);
    assert_eq!(app.economy().home.level(Station::Forge), 1);
    assert_eq!(app.economy().home.trophies.get("cinderjaw"), Some(&1));
    assert!(app.economy().home.feats.contains("roadkill"));
    assert!(run(&app).orders.is_empty());
    assert!(run(&app).marks.is_empty());
    assert!(run(&app).feats.is_empty());
    let saved = app.economy().treasury.clone();
    let surface = app.economy().home.countryside.clone();
    assert!(app.settle_home());
    assert_eq!(
        app.economy().treasury,
        saved,
        "retry/empty settle never pays twice"
    );
    assert_eq!(app.economy().home.countryside, surface);
    // The western survey uses the very same rollback/retry transaction. Its
    // reward and new game record must survive a disk failure, without doubling.
    let before = app.economy().treasury.clone();
    for i in 0..3 {
        app.realm.dungeon
            .shooter
            .edit()
            .unwrap()
            .marks
            .insert(format!("country:survey:{i}"), 1);
    }
    app.realm.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("country:west:0".into(), 1);
    app.realm.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("country:west:1".into(), 1);
    fixture.block_saves();
    assert!(!app.settle_home());
    assert_eq!(app.economy().treasury, before);
    assert_eq!(app.economy().home.countryside.survey, 0);
    assert_eq!(pending_home(&app).transfer.marks.get("country:survey:2"), Some(&1));
    fixture.allow_saves();
    assert!(app.settle_home());
    assert_eq!(
        app.economy().treasury.get(Spoil::Gold),
        before.get(Spoil::Gold) + 45
    );
    assert_eq!(
        app.economy().treasury.get(Spoil::Ore),
        before.get(Spoil::Ore) + 2
    );
    assert_eq!(app.economy().home.countryside.west_visited, 3);
    assert_eq!(app.economy().home.countryside.survey, 7);
    assert!(app.economy().home.countryside.survey_paid);
    let paid = app.economy().treasury.clone();
    assert!(app.settle_home());
    assert_eq!(app.economy().treasury, paid);
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
    app.realm.dungeon.drafting = Some(("no-such-wish-zz".into(), std::time::Instant::now(), false));
    app.watch_drafting();
    assert!(
        app.realm.dungeon.drafting.is_some(),
        "waits for the turn to be seen"
    );
    app.realm.dungeon.drafting = Some(("no-such-wish-zz".into(), std::time::Instant::now(), true));
    app.watch_drafting();
    assert!(app.realm.dungeon.drafting.is_none());
    assert!(
        app.realm.dungeon.notice.contains("without")
            && app.realm.dungeon.notice.contains("no-such-wish-zz.wish"),
        "{}",
        app.realm.dungeon.notice
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
        std::env::var_os("ANGEL_TESTER_DIR").map_or_else(|| std::env::temp_dir().join("delve-tester"), Into::into),
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
    app.realm.dungeon.key_releases = true;
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
        if let Some(run) = app.realm.dungeon.shooter.as_ref().cloned() {
            *shot += 1;
            let img = crate::delve::world_viz::overworld::arena::frame(&run, 768, 448);
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
            let realm = app.economy().clone();
            facts = serde_json::json!({
                "frame_png": path,
                "arena_units": [crate::delve::together_shooter::WIDTH, crate::delve::together_shooter::HEIGHT],
                "phase": run.phase, "floor": run.floor(), "pack": run.dungeon.pack.name(),
                "room_kind": format!("{:?}", room.kind), "doors_barred": run.barred(),
                "doorways_nesw": room.doors, "tick": run.tick, "score": run.score,
                "stairs_here": (0..crate::delve::together_shooter::ROWS as i32).any(|r| (0..crate::delve::together_shooter::COLS as i32).any(|c| room.tile(c, r) == crate::delve::together_shooter::Tile::Stairs)),
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
                "card_screen_open": app.realm.dungeon.cards_open, "game_expanded": app.realm.dungeon.expanded,
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
                        let realm = app.economy();
                        let which = which.trim();
                        which
                            .parse::<usize>()
                            .ok()
                            .and_then(|n| realm.wishes.get(n.wrapping_sub(1)))
                            .or_else(|| realm.wishes.iter().find(|w| w.id == which))
                            .is_some_and(|w| {
                                w.status == crate::delve::together_realm::Status::Asked
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
            if app.realm.dungeon.shooter.is_some() && !app.realm.dungeon.notice.is_empty() {
                said.push(format!("notice: {}", app.realm.dungeon.notice));
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
    assert!(app.realm.world.delve_called && app.realm.dungeon.shooter.is_none());
    for _ in 0..4000 {
        if app.realm.world.at_delve_gate() {
            break;
        }
        app.realm.world.tick_overworld();
    }
    assert!(app.realm.world.at_delve_gate(), "he walks there");
    app.watch_delve_gate();
    assert!(
        app.messages
            .last()
            .is_some_and(|m| m.text.contains("Delve's gate"))
    );
    press(&mut app, KeyCode::F(4));
    assert!(app.realm.dungeon.intro.is_some() && app.dungeon_view_active());
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
    let run = app.realm.dungeon.shooter.as_ref().expect("the delve begins");
    assert_eq!(run.players[&1].knight.as_deref(), Some("lynette"));
    assert_eq!(
        run.dungeon.pack,
        crate::delve::together_shooter::Pack::Cavern
    );
    assert!(app.realm.dungeon.intro.is_none() && app.realm.dungeon.expanded);
    assert!(
        app.realm.world.delve_lit,
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
    let message = host_when_admitted!(app, "host --3");
    let link = link_of(&message);
    assert_eq!(message.matches("http://").count(), 1, "one link for everyone: {message}");
    assert_eq!(run(&app).players.len(), 1, "knights join as friends arrive");
    assert!(app.dungeon_command(Some("invite")).contains(&link));

    // Three friends open the one link: each takes a seat of their own.
    let friends: Vec<_> = ["Matt", "Cleo", "Bram"]
        .into_iter()
        .map(|name| crate::delve::together_seat::Seat::connect(&link, name).unwrap())
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
    let watcher = crate::delve::together_seat::Seat::connect(&link, "Late").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while watcher.status() == angel_serve::client::LinkStatus::Connecting {
        assert!(std::time::Instant::now() < deadline);
        watcher.tick(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(watcher.knight(), None);
}

#[test]
fn dungeon_host_for_five_seats_five_friends_and_a_sixth_only_watches() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    // What `/dungeon_host --5` runs: the host's seat and five friends'.
    let message = host_when_admitted!(app, "host --5");
    let link = link_of(&message);
    assert!(message.contains("Delve hosted for 5 friends"), "{message}");
    assert_eq!(app.realm.dungeon.hosting.as_ref().map(|h| h.friends), Some(5));
    assert_eq!(message.matches("http://").count(), 1, "one link for everyone: {message}");

    let names = ["Matt", "Cleo", "Bram", "Enid", "Ywain"];
    let friends: Vec<_> = names
        .into_iter()
        .map(|name| crate::delve::together_seat::Seat::connect(&link, name).unwrap())
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while run(&app).players.len() < 6 {
        assert!(std::time::Instant::now() < deadline, "the party never gathered: {}", run(&app).players.len());
        for friend in &friends {
            friend.tick(std::time::Instant::now());
        }
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let mut knights: Vec<u32> = friends.iter().filter_map(|f| f.knight()).collect();
    knights.sort();
    assert_eq!(knights, vec![2, 3, 4, 5, 6]);
    for (friend, name) in friends.iter().zip(names) {
        let knight = friend.knight().unwrap();
        assert_eq!(run(&app).players[&knight].name, name);
        assert!(run(&app).players[&knight].knight.is_some(), "a knight of the company");
    }
    // Every friend a different knight of the company.
    let mut company: Vec<_> = run(&app).players.iter().filter(|(id, _)| **id != 1).filter_map(|(_, h)| h.knight.clone()).collect();
    company.sort();
    company.dedup();
    assert_eq!(company.len(), 5);
    // Past the six a seventh only watches.
    let watcher = crate::delve::together_seat::Seat::connect(&link, "Late").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while watcher.status() == angel_serve::client::LinkStatus::Connecting {
        assert!(std::time::Instant::now() < deadline);
        watcher.tick(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(watcher.knight(), None);
    // And a friend's knight past the fifth may be kicked as any.
    assert!(app.dungeon_command(Some("kick 6")).contains("left the delve"));
    assert!(app.dungeon_command(Some("kick 7")).contains("Kick a friend's knight"));
}

#[test]
fn dungeon_host_with_no_knight_seats_six_friends_the_last_in_as_knight_one() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    // What a dedicated machine's `/dungeon_host --watch --6` runs: every
    // seat a friend's.
    let message = host_when_admitted!(app, "host --watch --6");
    let link = link_of(&message);
    assert!(message.contains("Delve hosted for 6 friends"), "{message}");
    assert_eq!(app.realm.dungeon.hosting.as_ref().map(|h| (h.friends, h.watch)), Some((6, true)));
    let host = run(&app).players[&1].name.clone();

    // Five friends take the other seats first; the host's place waits.
    let names = ["Matt", "Cleo", "Bram", "Enid", "Ywain", "Gawain"];
    let mut friends: Vec<_> = names[..5]
        .iter()
        .map(|name| crate::delve::together_seat::Seat::connect(&link, name).unwrap())
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while run(&app).players.len() < 6 || friends.iter().any(|f| f.knight().is_none()) {
        assert!(std::time::Instant::now() < deadline, "the five never gathered: {}", run(&app).players.len());
        for friend in &friends {
            friend.tick(std::time::Instant::now());
        }
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let mut knights: Vec<u32> = friends.iter().filter_map(|f| f.knight()).collect();
    knights.sort();
    assert_eq!(knights, vec![2, 3, 4, 5, 6]);
    assert_eq!(run(&app).players[&1].name, host, "the host's place still its own");

    // The sixth takes the host's place: knight 1, in their own name.
    friends.push(crate::delve::together_seat::Seat::connect(&link, names[5]).unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while friends[5].knight().is_none()
        || run(&app).players[&1].name != names[5]
        || run(&app).players[&1].absent
        || run(&app).party_len() < 6
    {
        assert!(std::time::Instant::now() < deadline, "the sixth never took the host's place");
        for friend in &friends {
            friend.tick(std::time::Instant::now());
        }
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(friends[5].knight(), Some(1));
    for (friend, name) in friends.iter().zip(names) {
        assert_eq!(run(&app).players[&friend.knight().unwrap()].name, name);
    }
    assert_eq!(run(&app).players.len(), 6, "no seventh knight");
    assert!(app.dungeon_friends_seated());

    // Past the six a seventh only watches.
    let watcher = crate::delve::together_seat::Seat::connect(&link, "Late").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while watcher.status() == angel_serve::client::LinkStatus::Connecting {
        assert!(std::time::Instant::now() < deadline);
        watcher.tick(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(watcher.knight(), None);

    // Knight 1 may be kicked as any friend's: the place stays, empty.
    assert!(app.dungeon_command(Some("kick 1")).contains("left the delve"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !run(&app).players[&1].absent {
        assert!(std::time::Instant::now() < deadline, "the host's place never emptied");
        for friend in &friends[..5] {
            friend.tick(std::time::Instant::now());
        }
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(run(&app).party_len(), 5);
}

#[test]
fn dungeon_host_takes_up_to_five_friends_and_no_more() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    for (words, friends) in [
        ("host --5", 5usize),
        ("host --9", 5),
        ("host --6", 5),
        ("host --4 --watch", 4),
        ("host --6 --watch", 6),
        ("host --watch --9", 6),
        ("host -3", 3),
        ("host --0", 1),
        ("host", 1),
    ] {
        let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
        let message = host_when_admitted!(app, words);
        assert!(message.contains("Delve hosted"), "{words}: {message}");
        assert_eq!(app.realm.dungeon.hosting.as_ref().map(|h| h.friends), Some(friends), "{words}");
        assert_eq!(app.realm.dungeon.hosting.as_ref().map(|h| h.watch), Some(words.contains("--watch")), "{words}");
    }
    // The most a host may seat, and the host's own seat, are the party; a
    // host who plays no knight seats the whole party.
    assert_eq!(1 + super::MAX_FRIENDS as u32, crate::delve::together_shooter::MAX_PLAYERS);
    assert_eq!(super::MAX_WATCHED_FRIENDS as u32, crate::delve::together_shooter::MAX_PLAYERS);
}

/// Tick a friend's seat (and the host) until the server has answered it.
fn answered(app: &mut App, seat: &crate::delve::together_seat::Seat) -> angel_serve::client::LinkStatus {
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
    use crate::delve::together_seat::Seat;
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let link = link_of(&host_when_admitted!(app, "host --3"));

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
    // The welcome reaches Cleo before the sim applies Joined, so her client
    // is Playing a step before the host's run has her. Wait for that step
    // before kicking, or the party check races the join.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !run(&app).players.contains_key(&3) {
        assert!(std::time::Instant::now() < deadline, "Cleo never joined the host's run");
        cleo.tick(std::time::Instant::now());
        app.advance_dungeon_guest();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(run(&app).players[&3].name, "Cleo");

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
    assert!(run(&app).players.contains_key(&3), "kicking Matt dropped Cleo");
    assert_eq!(run(&app).players[&3].name, "Cleo");
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
    let message = host_when_admitted!(host, "host --1");
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

    // A friend's known wish goes to the host and lands on their knight, who
    // holds one (the Undercroft gives none; a floor gives two). Its homing
    // waits on a creature wish (`combos`); the card is theirs at once.
    host.dungeon.shooter.edit().unwrap().give_wishes(1, 0);
    friend
        .dungeon
        .joined
        .as_ref()
        .unwrap()
        .send(crate::delve::together_gear::BOON, br#"{"wish":"seeker"}"#);
    // The host's run opens the scroll on the friend's view; their 1 picks
    // through their own seat's spell key.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while run(&host).players[&2].offer.is_none() {
        assert!(std::time::Instant::now() < deadline, "the friend's scroll never opened");
        host.advance_shooter_at(std::time::Instant::now());
        friend.advance_joined();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !run(&host).players[&2].deck.iter().any(|c| c == "wish-seeker-1") {
        if friend.wish_offer_open(2) {
            press(&mut friend, KeyCode::Char('1'));
        }
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
    host_when_admitted!(app, "host --3");
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
    let message = host_when_admitted!(app, "host --2");
    assert!(message.contains("menu is open"), "{message}");
    // Friends can join at once: the delve is already open behind the menu.
    let raid = run(&app).raid_id;
    let intro = app.realm.dungeon.intro.as_ref().expect("the Delve's menu");
    assert_eq!(
        intro.field(),
        crate::delve::delve_intro_viz::Field::Continue
    );
    assert_eq!((intro.party, intro.hosting), (2, Some(2)));
    assert!(paint(&mut app, 160, 48).contains("2 seats"));
    press(&mut app, KeyCode::Enter);
    assert!(app.realm.dungeon.intro.is_none() && app.dungeon_view_active());
    assert_eq!(run(&app).raid_id, raid, "continue keeps the open delve");
    // Hosting again shows the same link and the menu again.
    let again = host_when_admitted!(app, "host --2");
    assert!(again.contains(&link_of(&message)), "{again}");
    assert!(app.realm.dungeon.intro.is_some());
}

#[test]
fn a_new_delve_while_hosting_keeps_the_seated_friends() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    host_when_admitted!(app, "host --3");
    let shooter = app.realm.dungeon.shooter.edit().unwrap();
    shooter.join_seat(3, "Matt");
    let lynette = crate::delve::together_shooter::knights::knight("lynette").unwrap();
    shooter.outfit(3, lynette);
    let raid = run(&app).raid_id;
    // From the Delve's menu, with friends along: a fresh delve.
    app.open_intro();
    app.realm.dungeon.intro.as_mut().unwrap().party = 2;
    app.realm.dungeon
        .intro
        .as_mut()
        .unwrap()
        .go(crate::delve::delve_intro_viz::Field::Begin);
    press(&mut app, KeyCode::Enter);
    assert!(run(&app).raid_id > raid, "a new delve, not the old map");
    assert_eq!(run(&app).players[&3].name, "Matt");
    assert_eq!(run(&app).players[&3].knight.as_deref(), Some("lynette"));
    assert!(app.realm.dungeon.hosting.is_some(), "the same link");
}

#[test]
fn the_scroll_offers_three_ways_to_have_a_wish_and_esc_rolls_it_up() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    let wishes = held(&app, 1);
    app.open_reforge();
    assert!(app.realm.dungeon.forge.draft.is_some(), "the scroll opens");
    assert!(paint(&mut app, 160, 48).contains("THE WISH SCROLL"));
    press(&mut app, KeyCode::Enter);
    assert!(app.realm.dungeon.forge.draft.is_some(), "an empty wish is not offered");
    for c in "three bolts at once".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    assert!(app.realm.dungeon.forge.draft.is_none(), "the run's scroll takes it from here");
    let offer = run(&app).players[&1].offer.clone().expect("the genie's choices");
    assert_eq!(offer.choices.len(), 3, "{offer:?}");
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("THE WISH SCROLL") && screen.contains("The genie's deal") && screen.contains("1 2 3 choose"),
        "{screen}"
    );
    assert_eq!(held(&app, 1), wishes, "nothing is spent by asking");
    // T again does not open a second scroll over it.
    app.open_reforge();
    assert!(app.realm.dungeon.forge.draft.is_none());
    // Esc rolls it up, through the seat's menu key: nothing spent.
    press(&mut app, KeyCode::Esc);
    advance(&mut app, 2);
    assert!(run(&app).players[&1].offer.is_none(), "rolled up");
    assert!(app.realm.dungeon.activity_menu.is_none(), "and no menu over the game");
    assert_eq!(held(&app, 1), wishes);
}

#[test]
fn a_won_room_grants_a_known_wish_at_once_without_leaving_the_game() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    let shooter = app.realm.dungeon.shooter.edit().unwrap();
    shooter.calm_for_test();
    shooter.clear_for_test();
    let wishes = held(&app, 1);
    assert_eq!(wishes, 3, "a delve starts with three wishes held");
    app.open_reforge();
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("Triple Volley I"),
        "known wishes are listed"
    );
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let screen = paint(&mut app, 160, 48);
    assert!(screen.contains("As you said"), "{screen}");
    press(&mut app, KeyCode::Char('1'));
    assert!(app.realm.dungeon.forge.draft.is_none());
    assert!(run(&app).players[&1].offer.is_none(), "picked");
    assert!(app.dungeon_view_active(), "the game never left the screen");
    assert!(
        app.realm.dungeon.notice.starts_with("Granted"),
        "{}",
        app.realm.dungeon.notice
    );
    let hero = &run(&app).players[&1];
    assert!(
        hero.deck.iter().any(|c| c.starts_with("wish-")),
        "{:?}",
        hero.deck
    );
    assert_eq!(held(&app, 1), wishes - 1, "the wish spends one held");
    // Typed words find a known wish too, with the wishes still held.
    app.open_reforge();
    for c in "bounce my shots".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('1'));
    // Granted; the bounce itself waits on a machine wish (`combos`).
    let deck = &run(&app).players[&1].deck;
    assert!(deck.iter().any(|c| c == "wish-ricochet-1"), "{deck:?}");
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
    app.realm.dungeon.save_path = Some(workspace.join(".angel/dungeon/session.json"));
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    let shooter = app.realm.dungeon.shooter.edit().unwrap();
    shooter.descend_for_test();
    shooter.calm_for_test();
    shooter.clear_for_test();
    // The book as the delve ships it: no other test's Starlight in it.
    crate::delve::together_files::refresh_phrasebook(&workspace);
    // The host's model, as a grant runner asks it: a moment, then the entry.
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_millis(150));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    let wishes = held(&app, 1);
    press(&mut app, KeyCode::Char('t'));
    for c in "make my arrows glow like stars".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Enter);
    let screen = paint(&mut app, 160, 48);
    assert!(
        screen.contains("once learned"),
        "unknown words are offered too: {screen}"
    );
    press(&mut app, KeyCode::Char('1'));
    app.teach_asked();
    assert!(app.dungeon_view_active(), "the game stays on screen");
    assert_eq!(app.realm.dungeon.forge.learning.len(), 1);
    assert!(
        app.realm.dungeon.notice.contains("keep playing"),
        "{}",
        app.realm.dungeon.notice
    );
    assert_eq!(held(&app, 1), wishes - 1, "a wish is spent on it");
    assert_eq!(run(&app).lamp.pending(1).count(), 1, "the lamp smokes while it is written");
    assert!(app.thinking.is_none() && app.pending_turn.is_none(), "no turn of the host's is taken");
    // A runner writes the wish into the workspace's phrasebook...
    let start = std::time::Instant::now();
    while run(&app).players[&1].deck.iter().all(|c| c != "wish-starlight-1")
        && start.elapsed() < std::time::Duration::from_secs(10)
    {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.watch_learning();
    }
    let book = workspace.join(".angel/dungeon/phrasebook.txt");
    assert!(std::fs::read_to_string(&book).unwrap().contains("wish starlight |"));
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    // ...and the book reloads with it: granted, mid-game.
    assert!(
        app.realm.dungeon.notice.contains("Starlight Arrows"),
        "{}",
        app.realm.dungeon.notice
    );
    let deck = &run(&app).players[&1].deck;
    assert!(deck.iter().any(|c| c == "wish-starlight-1"), "{deck:?}");
    assert!(app.realm.dungeon.forge.learning.is_empty());
    assert!(run(&app).lamp.wishing.is_empty());
    assert!(run(&app).lamp.landed.last().is_some_and(|l| l.knight == 1), "the lamp flares for it");
    let _ = std::fs::remove_dir_all(&workspace);
}

const STARLIGHT: &str = "wish starlight | Starlight Arrows\nsay make my arrows glow like stars | starlight arrows | glowing arrows\ntier homing 1; damage 10 :: Arrows glow and seek.\ntier homing 2; damage 15 :: Arrows burn bright and seek.\ntier homing 2; damage 20 :: Arrows are stars.\n";

/// Look again until every lesson is over, or ten seconds pass.
fn until_granted(app: &mut App) {
    let start = std::time::Instant::now();
    while !app.realm.dungeon.forge.learning.is_empty() && start.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.watch_learning();
    }
}

/// Knight 1 speaks `words` on the scroll: its choices open.
fn speak(app: &mut App, words: &str) {
    app.open_reforge();
    for c in words.chars() {
        press(app, KeyCode::Char(c));
    }
    press(app, KeyCode::Enter);
}

/// The workspace removed, and the book read again without what it learned.
fn forget(workspace: &std::path::Path) {
    let _ = std::fs::remove_dir_all(workspace);
    crate::delve::together_files::refresh_phrasebook(workspace);
}

/// Look again until `done`, or ten seconds pass.
fn look_until(app: &mut App, done: impl Fn(&App) -> bool) {
    let start = std::time::Instant::now();
    while !done(app) && start.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.realm.dungeon.forge.looked = None;
        app.watch_learning();
    }
}

#[test]
fn spoken_words_go_to_the_model_before_the_pick_and_a_late_pick_is_granted_at_once() {
    use angel_realm_host::control::Pick;
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("spoken");
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_millis(100));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    // A super wish: as you said is two steps.
    app.realm.dungeon.shooter.edit().unwrap().players.get_mut(&1).unwrap().great_wishes = 1;
    let wishes = held(&app, 1);
    speak(&mut app, "make my arrows glow like stars");
    assert!(run(&app).players[&1].offer.is_some(), "the scroll is open");
    // Sent before any pick: one lesson, not chosen, its job out.
    let lesson = &app.realm.dungeon.forge.learning[..];
    assert_eq!(lesson.len(), 1, "{}", app.realm.dungeon.notice);
    assert_eq!(lesson[0].pick, Pick::Open);
    assert!(lesson[0].job.is_some(), "its job is out before the pick");
    assert!(app.realm.dungeon.notice.contains("starts writing"), "{}", app.realm.dungeon.notice);
    assert_eq!(held(&app, 1), wishes, "nothing is spent by speaking");
    assert_eq!(run(&app).lamp.pending(1).count(), 1, "the lamp smokes at once");
    // Written while the knight chooses: kept, granted to no one yet.
    look_until(&mut app, |app| app.realm.dungeon.forge.learning.iter().all(|l| l.learned_id.is_some()));
    assert_eq!(app.realm.dungeon.forge.learning.len(), 1, "a landed entry waits for the pick");
    assert!(run(&app).players[&1].deck.iter().all(|c| !c.starts_with("wish-starlight")));
    assert_eq!(held(&app, 1), wishes);
    // The pick grants it on the next look, with its steps, and no second job.
    press(&mut app, KeyCode::Char('1'));
    app.watch_learning();
    let deck = &run(&app).players[&1].deck;
    assert!(deck.iter().any(|c| c == "wish-starlight-2"), "two steps: {deck:?}");
    assert!(app.realm.dungeon.forge.learning.is_empty());
    assert!(app.realm.dungeon.notice.contains("Starlight Arrows"), "{}", app.realm.dungeon.notice);
    assert_eq!(held(&app, 1), wishes - 1, "one wish spent");
    assert_eq!(club.asked.lock().unwrap().len(), 1, "the model was asked once");
    assert!(run(&app).lamp.wishing.is_empty());
    forget(&workspace);
}

const BRIGHT_PINK: &str = "wish bright-pink | Bright Pink\nsay make my ghouls bright pink | pink ghouls | pink dead\ntier max_hp 10 :: Your dead turn bright pink, and 10 more health.\ndraw worn at=body\n.7777.\n7cccc7\n7c99c7\n7cccc7\n.7777.\n";

#[test]
fn a_summoners_words_for_his_dead_are_written_for_them_and_granted_to_them() {
    use angel_realm_host::control::Pick;
    use crate::delve::together_shooter::dead_wishes::Target;
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("dead-wish");
    let club = crate::delve::together_grants::Scripted::new(&[BRIGHT_PINK], std::time::Duration::from_millis(150));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    assert!(app.realm.dungeon.shooter.edit().unwrap().take_up(1, "gravecaller"));
    let max_hp = run(&app).players[&1].max_hp;
    speak(&mut app, "make my ghouls bright pink");
    let target = |app: &App| run(app).players[&1].offer.as_ref().and_then(|o| o.target);
    assert_eq!(target(&app), Some(Target::Dead), "his words name his dead");
    let screen = paint(&mut app, 160, 48);
    assert!(screen.contains("For: you  your dead  both  [4]"), "the scroll says whom it is for: {screen}");
    assert_eq!(app.realm.dungeon.forge.learning.len(), 1);
    assert_eq!(app.realm.dungeon.forge.learning[0].target, Target::Dead);
    assert!(app.realm.dungeon.forge.learning[0].brief.contains("never a summon spell"));
    // Both is still his dead's lesson; turned to himself, his words are
    // written again for him, and turned back the first is taken up again.
    press(&mut app, KeyCode::Char('4'));
    app.teach_asked();
    assert_eq!((target(&app), app.realm.dungeon.forge.learning.len()), (Some(Target::Both), 1));
    press(&mut app, KeyCode::Char('4'));
    app.teach_asked();
    assert_eq!(target(&app), Some(Target::You));
    let lessons = &app.realm.dungeon.forge.learning;
    assert_eq!(lessons.len(), 2);
    assert_eq!((lessons[0].pick, lessons[1].pick, lessons[1].target), (Pick::Learn, Pick::Open, Target::You));
    assert!(!lessons[1].brief.contains("Gravecaller"), "his own is the plain brief");
    press(&mut app, KeyCode::Char('4'));
    app.teach_asked();
    assert_eq!(target(&app), Some(Target::Dead));
    assert_eq!((app.realm.dungeon.forge.learning[0].pick, app.realm.dungeon.forge.learning[1].pick), (Pick::Open, Pick::Learn));
    // Picked, it lands on his dead and not on him.
    press(&mut app, KeyCode::Char('1'));
    app.teach_asked();
    look_until(&mut app, |app| run(app).players[&1].deck.iter().any(|c| c.starts_with("wish-bright-pink")));
    let hero = &run(&app).players[&1];
    assert!(hero.dead_wishes.iter().any(|(id, t)| id.starts_with("bright-pink") && *t == Target::Dead), "{:?}", hero.dead_wishes);
    // Its health (as its grade gives it) is theirs, none of it his.
    assert!(hero.dead_gifts.hp > 0);
    assert_eq!(hero.max_hp, max_hp);
    forget(&workspace);
}

#[test]
fn a_pick_before_the_spoken_words_are_written_is_granted_when_they_land() {
    use angel_realm_host::control::Pick;
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("picked-early");
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_millis(400));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    let wishes = held(&app, 1);
    speak(&mut app, "make my arrows glow like stars");
    assert_eq!(app.realm.dungeon.forge.learning.len(), 1);
    // The genie's deal: the wish as said and a purse, none of it to his
    // freedom.
    press(&mut app, KeyCode::Char('3'));
    app.teach_asked();
    let lesson = &app.realm.dungeon.forge.learning[..];
    assert_eq!(lesson.len(), 1, "the pick takes up the lesson already writing");
    assert_eq!((lesson[0].pick, lesson[0].fills, lesson[0].steps), (Pick::Chosen, false, 1));
    assert!(app.realm.dungeon.notice.contains("already writing"), "{}", app.realm.dungeon.notice);
    assert_eq!(held(&app, 1), wishes - 1);
    assert_eq!(run(&app).lamp.pending(1).count(), 1, "one smoke, not two");
    until_granted(&mut app);
    let deck = &run(&app).players[&1].deck;
    assert!(deck.iter().any(|c| c == "wish-starlight-1"), "{deck:?}");
    assert_eq!(run(&app).genie.freedom, 0, "the deal fills no freedom");
    assert_eq!(held(&app, 1), wishes - 1, "nothing given back, nothing spent twice");
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    forget(&workspace);
}

#[test]
fn a_rolled_up_scroll_learns_its_words_for_next_time_and_spends_nothing() {
    use angel_realm_host::control::Pick;
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("rolled-up");
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_millis(200));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    let wishes = held(&app, 1);
    speak(&mut app, "make my arrows glow like stars");
    app.realm.dungeon.shooter.as_mut().unwrap().drop_offer(1);
    app.teach_asked();
    assert_eq!(app.realm.dungeon.forge.learning[0].pick, Pick::Learn);
    assert_eq!(run(&app).lamp.pending(1).count(), 0, "the smoke goes with the scroll");
    until_granted(&mut app);
    let book = workspace.join(".angel/dungeon/phrasebook.txt");
    assert!(std::fs::read_to_string(&book).unwrap().contains("wish starlight |"), "learned");
    assert!(run(&app).players[&1].deck.iter().all(|c| !c.starts_with("wish-starlight")), "granted to no one");
    assert_eq!(held(&app, 1), wishes, "no wish spent, none given back");
    assert!(app.realm.dungeon.notice.contains("learned"), "{}", app.realm.dungeon.notice);
    // Next time the words are a known wish: granted at once from the book.
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
    speak(&mut app, "make my arrows glow like stars");
    assert!(app.realm.dungeon.forge.learning.is_empty(), "nothing to write");
    press(&mut app, KeyCode::Char('1'));
    assert!(run(&app).players[&1].deck.iter().any(|c| c == "wish-starlight-1"));
    assert_eq!(held(&app, 1), wishes - 1);
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    forget(&workspace);
}

#[test]
fn the_genies_whim_grants_his_wish_and_the_spoken_words_are_only_learned() {
    use crate::delve::together_shooter::genie::{Choice, Take};
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("whim");
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_millis(200));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    let wishes = held(&app, 1);
    let deck = run(&app).players[&1].deck.clone();
    let other = crate::delve::dungeon_reforge::known_wishes(&deck, None, run(&app).dungeon.depth)[0].0.clone();
    speak(&mut app, "make my arrows glow like stars");
    // The genie's whim in the middle: a wish he picks instead.
    {
        let mut run = app.realm.dungeon.shooter.edit().unwrap();
        let mut offer = run.players[&1].offer.clone().unwrap();
        offer.choices[1] = Choice {
            take: Take::Whim,
            wish: Some(other.clone()),
            steps: 1,
            head: "The genie's whim".into(),
            gives: String::new(),
            cost: String::new(),
            tip: None,
        };
        // The fixture authors a different offered choice; capture its source
        // just as an admitted policy result does before publishing the UI.
        offer.binding = Some(crate::delve::together_shooter::scrolls::Binding::capture(
            &run,
            1,
            &offer,
            &crate::delve::together_shooter::phrasebook::book(),
            None,
        ).unwrap());
        run.players.get_mut(&1).unwrap().offer = Some(offer);
    }
    press(&mut app, KeyCode::Char('2'));
    let card = crate::delve::together_shooter::phrasebook::get(&other).unwrap().card_id(0);
    assert!(run(&app).players[&1].deck.contains(&card), "his wish is granted at once");
    assert_eq!(held(&app, 1), wishes - 1);
    until_granted(&mut app);
    let book = workspace.join(".angel/dungeon/phrasebook.txt");
    assert!(std::fs::read_to_string(&book).unwrap().contains("wish starlight |"), "learned");
    assert!(run(&app).players[&1].deck.iter().all(|c| !c.starts_with("wish-starlight")));
    assert_eq!(held(&app, 1), wishes - 1, "one wish, for the whim");
    forget(&workspace);
}

#[test]
fn opening_and_rolling_up_scrolls_cannot_flood_the_runners() {
    use angel_realm_host::control::{MOST_IN_FLIGHT, Pick};
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("flood");
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_secs(3));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    let wishes = held(&app, 1);
    let said = ["zorbly quaxing flimflam", "wobbletrance of mirkwhistle", "plinkered dazzlewump", "frumious bandersnatch kin"];
    for words in &said[..MOST_IN_FLIGHT] {
        speak(&mut app, words);
        app.realm.dungeon.shooter.as_mut().unwrap().drop_offer(1);
        app.teach_asked();
    }
    assert_eq!(app.realm.dungeon.forge.learning.len(), MOST_IN_FLIGHT);
    // A fourth is not sent while the others write...
    speak(&mut app, said[MOST_IN_FLIGHT]);
    assert!(run(&app).players[&1].offer.is_some());
    assert_eq!(app.realm.dungeon.forge.learning.len(), MOST_IN_FLIGHT, "not sent");
    // ...and spent, it is given back, as a chosen wish past the bound is.
    press(&mut app, KeyCode::Char('1'));
    app.teach_asked();
    assert_eq!(app.realm.dungeon.forge.learning.len(), MOST_IN_FLIGHT);
    assert_eq!(held(&app, 1), wishes, "spent and given back once");
    // The first words spoken again take up their lesson, not a new one.
    speak(&mut app, said[0]);
    let open: Vec<_> = app.realm.dungeon.forge.learning.iter().filter(|l| l.pick == Pick::Open).collect();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].words, said[0]);
    assert_eq!(app.realm.dungeon.forge.learning.len(), MOST_IN_FLIGHT);
    forget(&workspace);
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
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(run(&app).book.get("ember-bolt").is_none());
    std::fs::write(
        workspace.join(".angel/dungeon/cards/ember-bolt.card"),
        "name Ember Bolt\nkind hold\ntext Hotter shots.\ndamage 10\n",
    )
    .unwrap();
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(run(&app).book.get("ember-bolt").is_some(), "hot-loaded");
    assert!(
        app.realm.dungeon.notice.contains("Ember Bolt"),
        "{}",
        app.realm.dungeon.notice
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
    let shooter = app.realm.dungeon.shooter.edit().unwrap();
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
        app.realm.dungeon.notice.starts_with("Granted"),
        "{}",
        app.realm.dungeon.notice
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
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
    let raw = "name Cha-Ching Meteor\nkind spell\nrarity relic\nby Money Knight\ncooldown 2\ncast meteor\nnova 25\n";
    let path = dir.join("cha-ching-meteor.card");
    std::fs::write(&path, raw).unwrap();
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(run(&app).book.get("cha-ching-meteor").is_some());
    assert!(app.realm.dungeon.notice.contains("The book takes new cards"));
    assert_eq!(run(&app).players[&1].spells, [None, None, None, None]);
    let now = std::time::Instant::now();
    app.realm.dungeon.shooter.rescan_spells();
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
    app.realm.dungeon
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
        app.realm.dungeon.forge.looked = None;
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
    app.realm.dungeon.key_releases = true;
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(
        app.realm.dungeon
            .local_inputs(std::time::Instant::now())
            .get(&1)
            .unwrap()
            .cast,
        1
    );
    for (key, slot) in [('6', 2), ('\\', 4)] {
        app.realm.dungeon.held.clear();
        press(&mut app, KeyCode::Char(key));
        assert_eq!(
            app.realm.dungeon
                .local_inputs(std::time::Instant::now())
                .get(&2)
                .unwrap()
                .cast,
            slot
        );
    }
    app.realm.dungeon
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
    assert!(!app.realm.dungeon.key_releases, "a terminal with no key releases");
    let ms = std::time::Duration::from_millis;

    // One tap: the knight walks about its own width, not two.
    press(&mut app, KeyCode::Char('d'));
    let at = app.realm.dungeon.held[&KeyCode::Char('d')].at;
    assert_eq!(app.realm.dungeon.local_inputs(at + ms(60))[&1].move_x, 1);
    assert_eq!(
        app.realm.dungeon.local_inputs(at + ms(120))[&1].move_x,
        0,
        "a tap is over well inside the old 180 ms lease"
    );

    // The terminal's own key repeat: the knight walks until it stops.
    app.realm.dungeon.held.clear();
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('d'));
    let at = app.realm.dungeon.held[&KeyCode::Char('d')].at;
    assert_eq!(app.realm.dungeon.local_inputs(at + ms(150))[&1].move_x, 1);
    assert_eq!(app.realm.dungeon.local_inputs(at + ms(200))[&1].move_x, 0);

    // Actions keep the whole lease, so a tap always reaches a friend's host.
    app.realm.dungeon.held.clear();
    press(&mut app, KeyCode::Char('f'));
    let at = app.realm.dungeon.held[&KeyCode::Char('f')].at;
    assert!(app.realm.dungeon.local_inputs(at + ms(150))[&1].fire);
}

#[test]
fn playing_in_a_friends_delve_never_clears_the_whole_screen_each_tick() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host_when_admitted!(host, "host --1");
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
    let inputs = app.realm.dungeon.local_inputs(std::time::Instant::now());
    assert!(inputs[&1].take);
    assert!(!inputs[&2].swing, "E is the first knight's take only");
    app.realm.dungeon.held.clear();
    press(&mut app, KeyCode::Char('['));
    let inputs = app.realm.dungeon.local_inputs(std::time::Instant::now());
    assert!(inputs[&2].swing && !inputs[&1].take);
}

#[test]
fn a_friends_angelx_takes_weapons_and_casts_spells_like_the_hosts_first_knight() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host_when_admitted!(host, "host --1");
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
            (|i: &crate::delve::together_shooter::Input| i.interact && !i.fire && !i.bomb)
                as fn(&crate::delve::together_shooter::Input) -> bool,
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
    let shooter = app.realm.dungeon.shooter.edit().unwrap();
    shooter.enemies.clear();
    shooter.phase = crate::delve::together_shooter::Phase::Exploring;
    paint(&mut app, 120, 40);
    app.realm.dungeon.key_releases = true;
    press(&mut app, KeyCode::Char('g'));
    advance(&mut app, 3);
    let hero = &run(&app).players[&1];
    assert!(hero.vigil && hero.stone);
    assert!(paint(&mut app, 120, 40).contains("VIGIL"));
    // The key comes up and the knight stays warded.
    app.realm.dungeon.held.clear();
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
    let message = host_when_admitted!(host, "host --1");
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
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::merchant;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.economy().treasury.add(Spoil::Gold, 100);
    app.settle_home();
    let host = app.realm.dungeon.shooter.edit().unwrap();
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
    let tile = crate::delve::together_shooter::TILE_UNITS;
    (hero.x, hero.y) = ((c as f32 + 0.5) * tile, (r as f32 + 0.5) * tile);
    hero.bombs = 0;
    for _ in 0..merchant::STAND_TICKS {
        host.step(&std::collections::BTreeMap::new());
    }
    app.settle_home();
    assert_eq!(run(&app).players[&1].bombs, 1, "{:?}", run(&app).found);
    assert_eq!(app.economy().treasury.get(Spoil::Gold), 100 - price);
    assert_eq!(run(&app).treasury.get(Spoil::Gold), 100 - price);
    assert!(run(&app).peddled.is_empty());
}

#[test]
fn the_undercroft_spends_the_realms_treasury_and_remembers_what_it_built() {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    assert!(run(&app).at_home_now());
    {
        let realm = app.economy();
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
    app.realm.dungeon.shooter.edit().unwrap().orders.push(order);
    app.settle_home();
    assert_eq!(app.economy().home.level(Station::Forge), 1);
    assert_eq!(app.economy().treasury.get(Spoil::Gold), 50);
    assert_eq!(app.economy().treasury.get(Spoil::Ore), 0);
    assert_eq!(run(&app).home.level(Station::Forge), 1);
    assert_eq!(
        run(&app).players[&1].bonus.damage,
        8,
        "the edge is dealt at once"
    );
    assert!(
        app.realm.dungeon.notice.contains("Tobbin's Edge I"),
        "{}",
        app.realm.dungeon.notice
    );
    // Too poor for the next rung: nothing is paid, and the ledger says why.
    app.realm.dungeon.shooter.edit().unwrap().orders.push(order);
    app.settle_home();
    assert_eq!(app.economy().home.level(Station::Forge), 1);
    assert_eq!(app.economy().treasury.get(Spoil::Gold), 50);
    assert!(
        app.realm.dungeon.notice.contains("need"),
        "{}",
        app.realm.dungeon.notice
    );
    // A new delve begins in the cellar as built.
    app.realm.dungeon.shooter.edit().unwrap().phase = crate::delve::together_shooter::Phase::Wiped;
    app.dungeon_view_active();
    app.start_shooter(None);
    assert_eq!(run(&app).home.level(Station::Forge), 1);
    assert_eq!(run(&app).players[&1].bonus.damage, 8);
    // The deepest floor reached is remembered for the ladders that need it.
    app.realm.dungeon.shooter.edit().unwrap().descend_for_test();
    app.realm.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.economy().home.deepest, 2);
}

/// The Undercroft's whole screen for review, beside `dungeon_write_delve_screens`.
#[test]
#[ignore]
fn dungeon_write_home_screen() {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::home::Station;
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
        let realm = app.economy();
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
    let run = app.realm.dungeon.shooter.edit().unwrap();
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
    let run = app.realm.dungeon.shooter.edit().unwrap();
    run.enter_for_test(1);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 17.0);
    let fire = std::collections::BTreeMap::from([(
        1,
        crate::delve::together_shooter::Input {
            fire: true,
            ..Default::default()
        },
    )]);
    for _ in 0..30 {
        run.step(&fire);
    }
    for _ in 0..crate::delve::together_shooter::fortune::SPIN_TICKS {
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
    use crate::delve::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = app
        .economy()
        .treasury
        .get(crate::delve::together_realm::Spoil::Gold);
    app.realm.dungeon.shooter.edit().unwrap().feats.push("roadkill");
    app.settle_home();
    assert!(app.economy().home.feats.contains("roadkill"));
    assert_eq!(app.economy().home.boxes.len(), 1);
    assert!(
        app.realm.dungeon.notice.contains("Roadkill"),
        "{}",
        app.realm.dungeon.notice
    );
    assert!(run(&app).banner_now().is_some(), "the banner is up");
    // Noticed again: nothing more.
    app.realm.dungeon.shooter.edit().unwrap().feats.push("roadkill");
    app.settle_home();
    assert_eq!(app.economy().home.boxes.len(), 1);
    // Opened at the coffer: into the treasury, and the run shows it.
    app.realm.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::Coffer,
    });
    app.settle_home();
    assert!(app.economy().home.boxes.is_empty());
    assert_eq!(app.economy().home.opened, 1);
    assert!(
        app.economy()
            .treasury
            .get(crate::delve::together_realm::Spoil::Gold)
            > gold
    );
    assert!(run(&app).unboxed.is_some());
    assert!(
        app.realm.dungeon.notice.contains("Silver Box held"),
        "{}",
        app.realm.dungeon.notice
    );
}

#[test]
fn the_realm_no_longer_publishes_viewer_records() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    let home = serde_json::to_value(&app.economy().home).unwrap();
    assert!(home.get("best_show").is_none());
}

#[test]
fn wren_pays_a_finished_bounty_and_pins_the_next() {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::bounties::{self, Pinned, slay_mark};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    assert_eq!(
        app.economy().home.bounties.len(),
        bounties::PINNED,
        "pinned on arrival"
    );
    app.economy().home.bounties[0] = Pinned {
        id: "bones".into(),
        have: 24,
    };
    let gold = app.economy().treasury.get(Spoil::Gold);
    let run_now = app.realm.dungeon.shooter.edit().unwrap();
    run_now.marks.insert(
        slay_mark(crate::delve::together_shooter::EnemyKind::Skeleton),
        1,
    );
    app.settle_home();
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold + 120);
    assert!(
        app.realm.dungeon.notice.contains("Bone Collector"),
        "{}",
        app.realm.dungeon.notice
    );
    assert_eq!(app.economy().home.bounties.len(), bounties::PINNED);
    assert!(app.economy().home.bounties.iter().all(|p| p.id != "bones"));
    assert_eq!(
        run(&app).treasury.get(Spoil::Gold),
        gold + 120,
        "the run shows it"
    );
}

#[test]
fn bounty_progress_is_saved_before_source_acknowledgement_during_a_fight() {
    use crate::delve::together_shooter::bounties::{Pinned, slay_mark};
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    fixture.bind(&mut app);
    assert!(app.settle_home(), "{}", app.realm.dungeon.notice);
    app.economy().home.bounties[0] = Pinned {
        id: "bones".into(),
        have: 0,
    };
    let run_now = app.realm.dungeon.shooter.edit().unwrap();
    run_now.phase = crate::delve::together_shooter::Phase::Fighting;
    run_now.marks.insert(
        slay_mark(crate::delve::together_shooter::EnemyKind::Skeleton),
        2,
    );
    assert!(app.settle_home(), "{}", app.realm.dungeon.notice);
    assert_eq!(app.economy().home.bounties[0].have, 2);
    assert!(
        !app.realm.dungeon.bounties_unsaved,
        "source facts cannot be acknowledged before progress is durable"
    );
    let disk = crate::delve::together_realm::Realm::beside(app.realm.world.rewards_path());
    assert_eq!(disk.home.bounties[0].have, 2);
    assert!(run(&app).marks.is_empty());
    assert!(app.realm.dungeon.shooter.pending_home().unwrap().is_none());
    app.realm.dungeon.shooter.edit().unwrap().phase = crate::delve::together_shooter::Phase::Exploring;
    app.settle_home();
    assert!(
        !app.realm.dungeon.bounties_unsaved,
        "saved once the fight is over"
    );
}

#[test]
fn the_realm_keeps_its_trophies_and_a_full_hall_is_a_feat() {
    use crate::delve::together_shooter::trophies::PLINTHS;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    app.economy().home.trophies.clear();
    let run_now = app.realm.dungeon.shooter.edit().unwrap();
    run_now.marks.insert("trophy:cinderjaw".into(), 1);
    app.settle_home();
    assert_eq!(app.economy().home.trophies.get("cinderjaw"), Some(&1));
    assert!(
        app.realm.dungeon.notice.contains("Cinderjaw"),
        "{}",
        app.realm.dungeon.notice
    );
    assert_eq!(run(&app).home.trophies.get("cinderjaw"), Some(&1));
    for plinth in &PLINTHS {
        let run_now = app.realm.dungeon.shooter.edit().unwrap();
        run_now.marks.insert(format!("trophy:{}", plinth.id), 1);
        app.settle_home();
    }
    assert_eq!(app.economy().home.trophies.get("cinderjaw"), Some(&2));
    assert!(app.economy().home.feats.contains("full_hall"));
}

#[test]
fn experience_reaches_a_level_and_sir_ector_teaches_its_lesson() {
    use crate::delve::together_shooter::home::{Order, Station};
    use crate::delve::together_shooter::talents::{LESSONS, LEVELS, knight_key};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    let key = knight_key(&run(&app).players[&1]);
    app.economy().home.knights.remove(&key);
    let run_now = app.realm.dungeon.shooter.edit().unwrap();
    run_now.marks.insert("xp".into(), LEVELS[0]);
    app.settle_home();
    assert_eq!(app.economy().home.prowess(&key).level(), 2);
    assert!(
        app.realm.dungeon.notice.contains("Sir Ector"),
        "{}",
        app.realm.dungeon.notice
    );
    app.realm.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::LessonB,
    });
    app.settle_home();
    assert_eq!(app.economy().home.prowess(&key).learned, [1]);
    assert!(app.economy().home.feats.contains("first_lesson"));
    assert!(
        app.realm.dungeon.notice.contains(LESSONS[0][1].name),
        "{}",
        app.realm.dungeon.notice
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
    use crate::delve::together_shooter::rescues::RESIDENTS;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    app.economy().home.residents.clear();
    app.realm.dungeon
        .shooter
        .edit()
        .unwrap()
        .marks
        .insert("rescue:mabel".into(), 1);
    app.settle_home();
    assert!(app.economy().home.residents.contains("mabel"));
    assert!(
        app.realm.dungeon.notice.contains("Mabel"),
        "{}",
        app.realm.dungeon.notice
    );
    assert!(run(&app).players[&1].deck.iter().any(|c| c == "home-stew"));
    for resident in &RESIDENTS {
        app.realm.dungeon
            .shooter
            .edit()
            .unwrap()
            .marks
            .insert(format!("rescue:{}", resident.id), 1);
        app.settle_home();
    }
    assert!(app.economy().home.feats.contains("full_house"));
}

#[test]
fn the_herald_counts_every_kill_into_the_bestiary() {
    use crate::delve::together_shooter::EnemyKind;
    use crate::delve::together_shooter::bounties::slay_mark;
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    app.settle_home();
    app.economy().home.bestiary.clear();
    let marks = &mut app.realm.dungeon.shooter.edit().unwrap().marks;
    marks.insert(slay_mark(EnemyKind::Skeleton), 3);
    marks.insert(slay_mark(EnemyKind::PitTyrant), 1);
    app.settle_home();
    let bestiary = &app.economy().home.bestiary;
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
        let realm = app.economy();
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
    let pages = crate::delve::shooter_viz::card_pages(app.realm.dungeon.shooter.as_ref().unwrap());
    app.realm.dungeon.cards_open = true;
    app.realm.dungeon.cards_page = pages - 1;
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
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = {
        let realm = app.economy();
        realm.treasury.add(Spoil::Gold, 200);
        realm.home.residents.insert("maud".into());
        realm.home.round = None;
        realm.treasury.get(Spoil::Gold)
    };
    let pour = Order {
        knight: 1,
        station: Station::TapB,
    };
    app.realm.dungeon.shooter.edit().unwrap().orders.push(pour);
    app.settle_home();
    assert_eq!(app.economy().home.round.as_deref(), Some("stout"));
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold - 80);
    assert_eq!(
        run(&app).home.round.as_deref(),
        Some("stout"),
        "the run sees it"
    );
    assert!(
        app.realm.dungeon.notice.contains("Dragon's Breath"),
        "{}",
        app.realm.dungeon.notice
    );
    // One round at a time.
    app.realm.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::TapA,
    });
    app.settle_home();
    assert_eq!(app.economy().home.round.as_deref(), Some("stout"));
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold - 80);
    assert!(
        app.realm.dungeon.notice.contains("waiting already"),
        "{}",
        app.realm.dungeon.notice
    );
    // Drunk at the top of the stair: the tab is clear.
    app.realm.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.economy().home.round, None);
    assert_eq!(run(&app).round.as_deref(), Some("stout"));
}

#[test]
fn dinadan_sings_for_gold_one_song_at_a_time() {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = {
        let realm = app.economy();
        realm.treasury.add(Spoil::Gold, 200);
        realm.home.song = None;
        realm.treasury.get(Spoil::Gold)
    };
    app.realm.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::SongC,
    });
    app.settle_home();
    assert_eq!(app.economy().home.song.as_deref(), Some("red"));
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold - 90);
    app.realm.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::SongA,
    });
    app.settle_home();
    assert_eq!(
        app.economy().home.song.as_deref(),
        Some("red"),
        "one at a time"
    );
    app.realm.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.economy().home.song, None, "sung, the tab is clear");
}

#[test]
fn beaumains_takes_a_wage_once_and_goes_down_with_the_party() {
    use crate::delve::together_realm::Spoil;
    use crate::delve::together_shooter::hireling::WAGE;
    use crate::delve::together_shooter::home::{Order, Station};
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.input = "/dungeon start".into();
    app.submit();
    let gold = {
        let realm = app.economy();
        realm.treasury.add(Spoil::Gold, 200);
        realm.home.hire = None;
        realm.treasury.get(Spoil::Gold)
    };
    let hire = Order {
        knight: 1,
        station: Station::Hire,
    };
    app.realm.dungeon.shooter.edit().unwrap().orders.push(hire);
    app.settle_home();
    assert_eq!(app.economy().home.hire.as_deref(), Some("beaumains"));
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold - WAGE);
    assert!(
        app.realm.dungeon.notice.contains("Beaumains"),
        "{}",
        app.realm.dungeon.notice
    );
    // Hired once is hired: a second wage is refused.
    app.realm.dungeon.shooter.edit().unwrap().orders.push(Order {
        knight: 1,
        station: Station::Hire,
    });
    app.settle_home();
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold - WAGE);
    // Down the stair he goes, and the realm's tab is clear.
    app.realm.dungeon.shooter.edit().unwrap().descend_for_test();
    app.settle_home();
    assert_eq!(app.economy().home.hire, None);
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
    app.realm.dungeon.key_releases = true;
    for (key, seat, check) in [
        (
            'e',
            1,
            (|i: &crate::delve::together_shooter::Input| i.interact && !i.fire && !i.bomb)
                as fn(&crate::delve::together_shooter::Input) -> bool,
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
        app.realm.dungeon.held.clear();
        press(&mut app, KeyCode::Char(key));
        assert!(
            check(&app.realm.dungeon.local_inputs(std::time::Instant::now())[&seat]),
            "{key}"
        );
    }
    press(&mut app, KeyCode::Char('`'));
    assert!(app.realm.dungeon.held.is_empty());
    let run = app.realm.dungeon.shooter.edit().unwrap();
    run.players.get_mut(&1).unwrap().spells = std::array::from_fn(|_| Some("cabbage-storm".into()));
    app.realm.dungeon.cards_open = true;
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
    app.realm.dungeon.save_path = Some(workspace.join(".angel/dungeon/session.json"));
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    let shooter = app.realm.dungeon.shooter.edit().unwrap();
    shooter.descend_for_test();
    shooter.calm_for_test();
    shooter.clear_for_test();
    // The book as the delve ships it: no other test's workspace in it.
    crate::delve::together_files::refresh_phrasebook(&workspace);
    (app, workspace)
}

fn wish(app: &mut App, words: &str) {
    app.open_reforge();
    for c in words.chars() {
        press(app, KeyCode::Char(c));
    }
    press(app, KeyCode::Enter);
    // The genie's first choice: as you said.
    press(app, KeyCode::Char('1'));
    app.teach_asked();
}

#[test]
fn a_creative_armor_wish_is_learned_as_a_new_look_not_granted_as_stock_iron() {
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("pavise");
    // The model writes an entry in its own words: found by being its own.
    let entry = format!(
        "wish spiked-pavise | Spiked Pavise\nsay tower shield | pavise\ntier armor 1 :: A pavise.\ntier armor 2 :: Heavier.\ntier armor 2; max_hp 15 :: Whole.\ndraw worn at=left\n{}\n",
        crate::delve::together_shooter::looks::PAVISE
    );
    let club = crate::delve::together_grants::Scripted::new(&[entry.as_str()], std::time::Duration::from_millis(100));
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    wish(&mut app, "a bigger spiked shield");
    assert_eq!(app.realm.dungeon.forge.learning.len(), 1, "{}", app.realm.dungeon.notice);
    assert!(run(&app).players[&1].deck.iter().all(|c| !c.starts_with("wish-iron")));
    // The lesson's brief offers the worn look, and says gear is never an orbit.
    let brief = app.realm.dungeon.forge.learning[0].brief.clone();
    assert!(brief.contains("Teach the angelX Delve's phrasebook"), "{brief}");
    assert!(brief.contains("draw worn"), "{brief}");
    assert!(brief.contains("Never use orbit"), "{brief}");
    assert!(brief.contains("max_hp"), "{brief}");
    assert!(brief.contains("Reply with ONE entry"), "{brief}");
    assert!(!brief.contains("sheet summon"), "a runner has no image tool: {brief}");
    until_granted(&mut app);
    assert!(app.realm.dungeon.notice.contains("Spiked Pavise"), "{}", app.realm.dungeon.notice);
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
    // Summons on a held card: turned away. Mended: a spell.
    let club = crate::delve::together_grants::Scripted::new(
        &[
            "wish golden-devils | Golden Devils\nsay tiny golden devils that explode\ntier summon 2 swarm :: Devils.\n",
            "wish golden-devils | Golden Devils\nkind spell\nsay tiny golden devils that explode\ntier summon 2 swarm; cooldown 8 :: Devils.\ndraw summon\n.5.|5.5\n555|555\n",
        ],
        std::time::Duration::ZERO,
    );
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    wish(&mut app, "tiny golden devils that explode");
    assert_eq!(app.realm.dungeon.forge.learning.len(), 1, "{}", app.realm.dungeon.notice);
    until_granted(&mut app);
    let asked = club.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 2, "asked once, mended once");
    assert!(asked[1].contains("spell or play"), "{}", asked[1]);
    assert!(app.realm.dungeon.notice.contains("Golden Devils"), "{}", app.realm.dungeon.notice);
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
    use crate::delve::together_realm::{Realm, Spoil};
    use crate::delve::together_shooter::{Run, college, home::Home};
    let _lock = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = game();
    fixture.bind(&mut app);
    let before = app.economy().treasury.clone();
    app.economy().save().unwrap();
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
    app.realm.dungeon.shooter.set(Some(r));
    std::fs::rename(
        fixture.home.join(".angelX"),
        fixture.home.join(".angelX-college-backup"),
    )
    .unwrap();
    std::fs::write(fixture.home.join(".angelX"), b"not a directory").unwrap();
    assert!(!app.settle_home());
    assert_eq!(app.economy().treasury, before);
    assert!(!app.economy().home.college.done.contains("sigils"));
    let path = fixture.workspace.join(".angel/dungeon/session.json");
    app.realm.dungeon.save_path = Some(path.clone());
    app.realm.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let mut encoded: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(encoded["country_intents"]["college:turn-in:sigils"], 1);
    let mut restored = game();
    fixture.bind(&mut restored);
    restored.realm.dungeon.shooter.set(None);
    assert!(restored.realm.dungeon.restore(&fixture.workspace).is_err(), "an unreadable realm must not erase its company");
    std::fs::remove_file(fixture.home.join(".angelX")).unwrap();
    std::fs::rename(
        fixture.home.join(".angelX-college-backup"),
        fixture.home.join(".angelX"),
    )
    .unwrap();
    restored.realm.dungeon.restore(&fixture.workspace).unwrap();
    encoded["country_intents"]["college:accept:made-up"] = 1.into();
    std::fs::write(&path, serde_json::to_vec(&encoded).unwrap()).unwrap();
    assert!(restored.realm.dungeon.restore(&fixture.workspace).is_err());
    std::fs::write(&path, &bytes).unwrap();
    restored.realm.dungeon.realm = None;
    assert!(restored.settle_home());
    let paid = restored.economy().treasury.clone();
    assert_eq!(paid.get(Spoil::Gold), before.get(Spoil::Gold) + 75);
    assert!(restored.economy().home.college.done.contains("sigils"));
    assert!(restored.economy().home.college.valid());
    assert!(restored.settle_home());
    assert_eq!(restored.economy().treasury, paid);
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
    assert_eq!(restored.economy().treasury, paid);
    assert!(college::intent_key("college:sample:711:1:0:0"));
    assert!(!college::intent_key("college:map:711:1:0:0"));
}

#[test]
fn dungeon_controls_activity_leave_preempts_overlays_and_only_releases_its_seat() {
    let _guard = crate::tests::env_lock();
    let mut app = game();
    let mut r = crate::delve::together_shooter::Run::at_home(
        41,
        1,
        Some("Friend"),
        Default::default(),
        Default::default(),
    );
    r.walk_kingdom(None);
    let geo = crate::delve::world_viz::overworld::kingdom::geography(&r.kingdom.wishes);
    r.enter_kingdom_place(
        geo.sites
            .iter()
            .position(|s| s.room == crate::delve::together_shooter::RoomKind::Hippodrome)
            .unwrap(),
    );
    r.floor_plates.clear();
    r.board_chariot(1);
    app.realm.dungeon.shooter.set(Some(r));
    paint(&mut app, 120, 40);
    assert!(paint(&mut app, 120, 40).contains("Esc: leave"));
    press(&mut app, KeyCode::Char('o'));
    assert!(app.realm.dungeon.local_inputs(std::time::Instant::now())[&2].dash);
    press(&mut app, KeyCode::Char('u'));
    assert!(app.realm.dungeon.local_inputs(std::time::Instant::now())[&2].bomb);
    assert!(run(&app).exits.outside.is_empty());
    app.clear_dungeon_controls();
    // Bring the manual server up to the time used by its rendered clients.
    app.advance_shooter_at(std::time::Instant::now());
    advance(&mut app, 60);
    assert!(app.realm.dungeon.shooter.seat(1).and_then(|s| s.run()).is_some_and(|r| r.can_hold_leave(1)), "the seat received the new race before holding Menu");
    app.realm.dungeon.cards_open = true;
    app.realm.dungeon.exhibit_text = Some("Overlay".into());
    app.realm.dungeon.key_releases = true;
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.realm.dungeon.activity_menu, None);
    assert!(app.realm.dungeon.local_inputs(std::time::Instant::now())[&1].leave_held);
    assert!(!app.realm.dungeon.cards_open && app.realm.dungeon.exhibit_text.is_none());
    let ticks = run(&app).chariot.as_ref().unwrap().t;
    advance(&mut app, 3);
    assert!(run(&app).exits.outside.is_empty(), "a short hold does not leave");
    advance(&mut app, 18);
    assert!(run(&app).exits.outside.contains_key(&1), "hold={:?} keys={} keyboard={} tick={}", run(&app).exits.holding, app.realm.dungeon.held.len(), app.dungeon_keyboard_active(), run(&app).tick);
    assert!(run(&app).chariot.is_some());
    assert_eq!(run(&app).chariot.as_ref().unwrap().driver, 0);
    assert_eq!(run(&app).chariot.as_ref().unwrap().racer_of(2), Some(1));
    assert!(run(&app).chariot.as_ref().unwrap().t >= ticks);
    app.realm.dungeon.held.remove(&KeyCode::Esc);
    press(&mut app, KeyCode::Delete);
    advance(&mut app, 18);
    assert_eq!(
        run(&app).room().kind,
        crate::delve::together_shooter::RoomKind::Kingdom
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
    let message = host_when_admitted!(host, "host --1");
    host.expand_dungeon();
    let link = format!("/dungeon join {}", link_of(&message));
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(link.trim_start_matches("/dungeon ")));
    let spin = |host: &mut App, friend: &mut App, stage: &str, ready: fn(&App, &App) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while !ready(host, friend) {
            paint(host, 120, 40);
            paint(friend, 120, 40);
            host.advance_shooter_at(std::time::Instant::now());
            friend.advance_joined();
            assert!(
                std::time::Instant::now() < deadline,
                "{stage}: exit did not reach host; host room {:?}, hold {}, outside {:?}; friend hold {}, menu {:?}, keys {:?}",
                run(host).room().kind,
                run(host).leave_progress(2),
                run(host).exits.outside.keys().collect::<Vec<_>>(),
                friend.joined_knight().is_some_and(|k| k["can_hold_leave"].as_bool() == Some(true)),
                friend.dungeon.activity_menu,
                friend.dungeon.held.keys().collect::<Vec<_>>(),
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    spin(&mut host, &mut friend, "joining", |h, f| {
        run(h).players.contains_key(&2) && f.joined_knight().is_some()
    });
    let r = host.dungeon.shooter.edit().unwrap();
    r.walk_kingdom(None);
    let geo = crate::delve::world_viz::overworld::kingdom::geography(&r.kingdom.wishes);
    r.enter_kingdom_place(
        geo.sites
            .iter()
            .position(|s| s.room == crate::delve::together_shooter::RoomKind::Lists)
            .unwrap(),
    );
    r.floor_plates.clear();
    r.mount_up(2);
    spin(&mut host, &mut friend, "joust snapshot", |_, f| {
        // The tavern can already be left. Wait for the actual mounted
        // activity to arrive before pressing its hold-to-leave key.
        f.can_leave_dungeon_activity(2)
            && f.dungeon.joined.as_ref().is_some_and(|joined| {
                joined.with_latest(|r| {
                    r.room().kind == crate::delve::together_shooter::RoomKind::Lists
                        && r.joust.as_ref().is_some_and(|j| j.knight == 2)
                        && r.can_hold_leave(2)
                }) == Some(true)
            })
    });
    friend.dungeon.cards_open = true;
    friend.dungeon.key_releases = true;
    press(&mut friend, KeyCode::Esc);
    assert_eq!(friend.dungeon.activity_menu, None);
    spin(&mut host, &mut friend, "leaving", |h, f| {
        run(h).exits.outside.contains_key(&2) && !f.dungeon.activity_leave_pending
    });
    assert_eq!(run(&host).room().kind, crate::delve::together_shooter::RoomKind::Lists);
    if let Some(joust) = &run(&host).joust { assert_eq!(joust.knight, 1); }
    assert_eq!(
        run(&host).view_for_seat(2).unwrap().room().kind,
        crate::delve::together_shooter::RoomKind::Kingdom
    );
    friend.dungeon.held.remove(&KeyCode::Esc);
    host.dungeon_command(Some("off"));
}

#[test]
fn angelx_work_reaches_the_holdings_and_the_court_settles_at_the_save() {
    use crate::delve::together_realm::{Realm, Spoil};
    use crate::delve::together_shooter::{Pack, Run, holdings as hd};
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    fixture.bind(&mut app);
    let gold = app.economy().treasury.get(Spoil::Gold);
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
    let h = app.economy().home.holdings.clone();
    assert_eq!((h.stores, h.retainers, h.patrols, h.unrest), (2, 1, 3, 1));
    assert_eq!(h.feed.get("BUILDS"), Some(&1));
    assert_eq!(h.feed.get("WALLS"), None, "a failed test run mends nothing");
    assert!(app.realm.dungeon.ops.is_empty() && app.realm.dungeon.ops_calls.is_empty());
    // Saved with the realm, and never a coin of gold.
    let again = Realm::beside(app.realm.world.rewards_path());
    assert_eq!(again.home.holdings, h);
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold);
    // A slow turn stirs unrest too.
    app.note_ops_turn(0, 0, hd::SLOW_TURN_SECS);
    app.note_ops_end(true);
    assert_eq!(app.economy().home.holdings.unrest, 2);
    // The Court's marks settle at the save, from the realm's purse.
    app.economy()
        .home
        .holdings
        .cleared
        .insert(Pack::Crypt.name().to_string());
    app.economy().treasury.add(Spoil::Gold, 7000);
    let (home, purse) = (app.economy().home.clone(), app.economy().treasury.clone());
    app.realm.dungeon.shooter.set(Some(Run::at_home(3, 1, None, home, purse)));
    app.realm.dungeon
        .shooter
        .as_mut()
        .unwrap()
        .marks
        .insert(hd::buy_mark(hd::Deed::Floor(Pack::Crypt), false), 1);
    assert!(app.settle_home());
    assert_eq!(app.economy().treasury.get(Spoil::Gold), gold + 1000);
    let key = hd::Deed::Floor(Pack::Crypt).key();
    assert!(app.economy().home.holdings.holding(&key).is_some());
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
    app.realm.dungeon.shooter.pump(now);
    app.realm.dungeon.shooter.pump(now + crate::delve::together_sim::STEP * 2);
    assert!(app.dungeon_command(Some("tune")).contains("hero_speed = 20 (code: 13)"));
    assert!(app.dungeon_command(Some("help")).contains("/dungeon tune [name value]"));
}

/// Step the host until `ready`, every friend's seat sending its keys.
fn host_all_until(app: &mut App, seats: &[&crate::delve::together_seat::Seat], what: &str, ready: impl Fn(&App) -> bool) {
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
    // Three seats, every one a friend's: two come, and the host's place
    // (the last a newcomer takes) stays empty.
    host_when_admitted!(app, "host --watch --3");
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
        let r = app.realm.dungeon.shooter.edit().unwrap();
        for hero in r.players.values_mut() {
            hero.hp = 0;
        }
        r.phase = crate::delve::together_shooter::Phase::Wiped;
    }
    let raid = run(&app).raid_id;
    // Confirm only after both guests have received the verdict they are answering.
    host_all_until(&mut app, &[&a, &b], "the friends never saw the verdict", |_| {
        [&a, &b].iter().all(|seat| seat.run().is_some_and(|r| r.over()))
    });
    // One asks: the party waits for the other.
    a.set_held(button::OK);
    host_all_until(&mut app, &[&a, &b], "the first ask never reached the host", |app| run(app).again.is_some());
    a.set_held(0);
    assert_eq!(run(&app).raid_id, raid, "nobody yanked from the verdict");
    // The other asks: a new run in the Sozzled Griffin, where every delve
    // begins, both alive, the same link.
    b.set_held(button::OK);
    host_all_until(&mut app, &[&a, &b], "the party never went back", |app| run(app).raid_id > raid);
    b.set_held(0);
    let r = run(&app);
    assert_eq!(r.room().kind, crate::delve::together_shooter::RoomKind::Alehouse);
    assert!(r.active());
    for k in [ka, kb] {
        assert!(r.players[&k].hp > 0, "knight {k} back on its feet");
    }
    assert!(app.realm.dungeon.hosting.is_some(), "the same link");
}

/// `ANGEL_WISH_SCROLL_SHOTS=DIR`: the terminal's cells version of the wish
/// scroll, written as text (`scroll-cells.txt`) and as HTML with its
/// colours (`scroll-cells.html`).
#[test]
#[ignore]
fn write_wish_scroll_cells() {
    let Some(dir) = std::env::var_os("ANGEL_WISH_SCROLL_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let _guard = crate::tests::env_lock();
    let mut app = game();
    app.terminal_focused = true;
    paint(&mut app, 160, 48);
    let mut pages = Vec::new();
    for moment in ["typing", "choices", "upgrades", "max", "no-wishes"] {
        if moment == "typing" {
            app.open_reforge();
            press(&mut app, KeyCode::Down);
        } else if moment == "choices" {
            press(&mut app, KeyCode::Enter);
        } else {
            app.realm.dungeon.forge.draft = None;
            let run = app.realm.dungeon.shooter.edit().unwrap();
            run.drop_offer(1);
            if moment == "upgrades" {
                for id in ["firebolt", "volley"] {
                    run.grant(1, &crate::delve::together_shooter::phrasebook::get(id).unwrap()).unwrap();
                }
                let iron = crate::delve::together_shooter::phrasebook::get("iron").unwrap();
                // Deep enough down for a ladder's fifth (`overclass::top_tier`).
                run.dungeon.depth = angel_delve::rules::FLOORS;
                for _ in &iron.tiers { run.grant(1, &iron).unwrap(); }
            }
            if moment == "no-wishes" {
                let hero = run.players.get_mut(&1).unwrap();
                (hero.wishes, hero.great_wishes) = (0, 0);
            }
            app.open_reforge();
            if moment == "max" { press(&mut app, KeyCode::PageDown); }
        }
        let mut terminal = Terminal::new(TestBackend::new(160, 48)).unwrap();
        terminal.draw(|frame| crate::ui::draw::ui(frame, &mut app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rgb = |c: ratatui::style::Color| match c {
            ratatui::style::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            _ => "#000000".into(),
        };
        let mut html = String::from("<pre style=\"font:14px/1 monospace;background:#000\">");
        let mut text = String::new();
        for y in 0..48u16 {
            for x in 0..160u16 {
                let cell = &buf[(x, y)];
                html.push_str(&format!(
                    "<span style=\"color:{};background:{}\">{}</span>",
                    rgb(cell.fg),
                    rgb(cell.bg),
                    cell.symbol().replace('<', "&lt;")
                ));
                text.push_str(cell.symbol());
            }
            html.push('\n');
            text.push('\n');
        }
        html.push_str("</pre>");
        pages.push(html);
        std::fs::write(dir.join(format!("scroll-cells-{moment}.txt")), text).unwrap();
    }
    std::fs::write(dir.join("scroll-cells.html"), pages.join("<hr>")).unwrap();
}

/// The words five knights wish at once in the timing bench.
const TIMED_WISHES: [&str; 5] = [
    "my arrows turn into fireballs",
    "a frog that fights beside me",
    "a shield made of ice",
    "lightning that jumps between enemies",
    "huge exploding cannonballs",
];

/// Wish timing, from the wish sealed to its skill in the knight's deck,
/// with one wish and with five at once: `ANGEL_WISH_TIMING=<dir> cargo test
/// --bin angel wish_grant_timing -- --ignored --nocapture`, the host's
/// model with `ANGEL_WISH_TIMING_REAL=1` (and its `ANGEL_DRIVER`), else a
/// stub that answers in `ANGEL_WISH_TIMING_STUB_MS` (default 20 s, about
/// one real call). Writes `timing.json`, and the briefs the old turn-at-a-
/// time lesson sent (`before-N.txt`, for `angel --task`) beside it.
#[test]
#[ignore]
fn wish_grant_timing() {
    let Some(dir) = std::env::var_os("ANGEL_WISH_TIMING").map(std::path::PathBuf::from) else {
        return;
    };
    let _guard = crate::tests::env_lock();
    std::fs::create_dir_all(&dir).unwrap();
    let before_ws = dir.join("before-ws");
    for (n, words) in TIMED_WISHES.iter().enumerate() {
        let brief = learn_brief_for_bench(words, &before_ws);
        std::fs::write(dir.join(format!("before-{}.txt", n + 1)), brief).unwrap();
    }
    let real = std::env::var("ANGEL_WISH_TIMING_REAL").is_ok_and(|v| v == "1");
    let stub_ms: u64 = std::env::var("ANGEL_WISH_TIMING_STUB_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let club: std::sync::Arc<dyn crate::agent::club::Club> = if real {
        crate::agent::club::Bag::standard().in_hand()
    } else {
        crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_millis(stub_ms))
    };
    let mut report = Vec::new();
    let counts: Vec<usize> = std::env::var("ANGEL_WISH_TIMING_COUNTS")
        .ok()
        .map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or(vec![1, 5]);
    for count in counts {
        let (mut app, workspace) = wishing_app(&format!("timing-{count}"));
        app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
        let mut asked = Vec::new();
        {
            let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
            for k in 1..=count as u32 {
                if !run.players.contains_key(&k) {
                    run.join_seat(k, &format!("Knight {k}"));
                }
                let hero = run.players.get_mut(&k).unwrap();
                hero.wishes = 3;
                run.spend_wish(k);
                asked.push(crate::delve::together_shooter::genie::Asked {
                    cosmetic: false,
                    cosmetic_name: String::new(),
                    fairy: false,
                    star: false,
                    knight: k,
                    words: TIMED_WISHES[k as usize - 1].to_string(),
                    steps: 1,
                    great: false,
                    fills: true,
                    making: None,
                    target: Default::default(),
                });
            }
        }
        let decks: Vec<usize> = (1..=count as u32).map(|k| run(&app).players[&k].deck.len()).collect();
        let start = std::time::Instant::now();
        app.realm.dungeon.shooter.edit().unwrap().asked.extend(asked);
        app.teach_asked();
        let sent = start.elapsed();
        let mut landed: Vec<Option<f32>> = vec![None; count];
        while !app.realm.dungeon.forge.learning.is_empty() && start.elapsed() < std::time::Duration::from_secs(400) {
            app.realm.dungeon.forge.looked = None;
            app.watch_learning();
            for k in 1..=count {
                if landed[k - 1].is_none() && run(&app).players[&(k as u32)].deck.len() > decks[k - 1] {
                    landed[k - 1] = Some(start.elapsed().as_secs_f32());
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let wishes: Vec<serde_json::Value> = (0..count)
            .map(|i| {
                let back = app.realm.dungeon.forge.landings.iter().find(|l| l.player as usize == i + 1);
                let timing = back.and_then(|l| l.timing);
                serde_json::json!({
                    "words": TIMED_WISHES[i],
                    "usable_s": landed[i],
                    "back_s": back.map(|l| l.took.as_secs_f32()),
                    "queued_s": timing.map(|t| t.queued.as_secs_f32()),
                    "model_s": timing.map(|t| t.model.as_secs_f32()),
                    "mend_s": timing.and_then(|t| t.mend.map(|m| m.as_secs_f32())),
                    "granted": back.and_then(|l| l.granted.clone()),
                    "failed": back.and_then(|l| l.failed.clone()),
                })
            })
            .collect();
        let line = serde_json::json!({
            "wishes_at_once": count,
            "route": club.label(),
            "all_sent_ms": sent.as_secs_f64() * 1000.0,
            "last_usable_s": landed.iter().flatten().cloned().fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v)))),
            "wishes": wishes,
            "notice": app.realm.dungeon.notice,
        });
        println!("{line}");
        report.push(line);
        let _ = std::fs::remove_dir_all(&workspace);
    }
    std::fs::write(dir.join("timing.json"), serde_json::to_string_pretty(&report).unwrap()).unwrap();
}

/// Wish timing with the knight taking a while to choose: from the words
/// spoken (the scroll opens) to the wish in their deck, the pick made after
/// each of `ANGEL_WISH_CHOOSE_MS` (default 5000,15000,30000), the stub
/// answering in `ANGEL_WISH_TIMING_STUB_MS` (default 20 s): `ANGEL_WISH_TIMING=<dir>
/// cargo test --bin angel wish_choose_timing -- --ignored --nocapture`.
/// Writes `choose-timing.json`.
#[test]
#[ignore]
fn wish_choose_timing() {
    let Some(dir) = std::env::var_os("ANGEL_WISH_TIMING").map(std::path::PathBuf::from) else {
        return;
    };
    let _guard = crate::tests::env_lock();
    std::fs::create_dir_all(&dir).unwrap();
    let stub_ms: u64 = std::env::var("ANGEL_WISH_TIMING_STUB_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let chooses: Vec<u64> = std::env::var("ANGEL_WISH_CHOOSE_MS")
        .ok()
        .map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect())
        .unwrap_or(vec![5_000, 15_000, 30_000]);
    let mut report = Vec::new();
    for choose_ms in chooses {
        let (mut app, workspace) = wishing_app(&format!("choose-{choose_ms}"));
        let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], std::time::Duration::from_millis(stub_ms));
        app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
        let deck = run(&app).players[&1].deck.len();
        let start = std::time::Instant::now();
        app.offer_from_scroll(1, "make my arrows glow like stars", None);
        assert!(run(&app).players[&1].offer.is_some(), "{}", app.realm.dungeon.notice);
        let mut picked = None;
        let mut granted = None;
        while granted.is_none() && start.elapsed() < std::time::Duration::from_secs(400) {
            if picked.is_none() && start.elapsed() >= std::time::Duration::from_millis(choose_ms) {
                app.realm.dungeon.shooter.as_mut().unwrap().choose(1, 0).unwrap();
                picked = Some(start.elapsed().as_secs_f32());
            }
            app.realm.dungeon.forge.looked = None;
            app.watch_learning();
            if run(&app).players[&1].deck.len() > deck {
                granted = Some(start.elapsed().as_secs_f32());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let line = serde_json::json!({
            "choose_s": choose_ms as f32 / 1000.0,
            "runner_s": stub_ms as f32 / 1000.0,
            "picked_s": picked,
            "granted_s": granted,
            "asked_model": club.asked.lock().unwrap().len(),
            "notice": app.realm.dungeon.notice,
        });
        println!("{line}");
        report.push(line);
        let _ = std::fs::remove_dir_all(&workspace);
    }
    std::fs::write(dir.join("choose-timing.json"), serde_json::to_string_pretty(&report).unwrap()).unwrap();
}

/// The old lesson's brief for `words`, its entry written into the
/// phrasebook of workspace `ws` (`angel --task` reads it as a turn).
fn learn_brief_for_bench(words: &str, ws: &std::path::Path) -> String {
    angel_realm_host::control::bench_learn_brief("Jordan", words, ws)
}

#[test]
fn a_spent_wish_on_unknown_words_is_still_taught_after_a_restart() {
    let _guard = crate::tests::env_lock();
    use angel_realm_host::control::Pick;
    let (mut app, workspace) = wishing_app("wish-restart");
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(crate::delve::together_grants::Scripted::new(
        &["Sorry, no."],
        std::time::Duration::from_secs(1),
    )));
    {
        let run = app.realm.dungeon.shooter.edit().unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        hero.wishes = 1;
        hero.great_wishes = 0;
        run.offer_wish(1, "a sword of pure starlight", None)
            .unwrap();
        run.choose(1, 0).unwrap();
    }
    app.teach_asked();
    assert_eq!(run(&app).players[&1].wishes, 0);
    assert!(
        run(&app).asked.is_empty(),
        "journal now owns the spent words"
    );
    assert_eq!(run(&app).lamp.wishing.len(), 1);
    let claim = app
        .dungeon
        .forge
        .learning
        .iter()
        .find(|l| l.pick == Pick::Chosen)
        .unwrap()
        .claim
        .clone()
        .unwrap();
    app.realm.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    let text = std::fs::read_to_string(app.realm.dungeon.save_path.as_ref().unwrap()).unwrap();
    assert!(text.contains("a sword of pure starlight"));
    app.realm.dungeon.forge.learning.clear();
    app.realm.dungeon.forge.wish_cursor = None;
    app.realm.dungeon.restore(&workspace).unwrap();
    app.teach_asked();
    let chosen: Vec<_> = app
        .dungeon
        .forge
        .learning
        .iter()
        .filter(|l| l.pick == Pick::Chosen)
        .collect();
    assert_eq!(chosen.len(), 1);
    assert_eq!(chosen[0].claim.as_ref(), Some(&claim));
    assert_eq!(run(&app).players[&1].wishes, 0);
    assert_eq!(run(&app).lamp.wishing.len(), 1, "one smoke, not two");
    let lesson = &mut app.realm.dungeon.forge.learning[0];
    lesson.asked = std::time::Instant::now() - std::time::Duration::from_secs(400);
    lesson.failed = Some("no answer in time".into());
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
    assert_eq!(run(&app).players[&1].wishes, 1);
    assert!(run(&app).asked.is_empty() && run(&app).lamp.wishing.is_empty());
    assert!(
        app.realm.dungeon
            .shooter
            .new_wish_claims(&mut None, "")
            .unwrap()
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(workspace);
}


/// A wish already spent, then learned, that the grant cannot land (every
/// spell slot taken) is given back, and the knight who wished hears why
/// on their own screen.
#[test]
fn a_learned_wish_that_cannot_be_granted_tells_the_knight_why() {
    use angel_realm_host::control::{Lesson, Pick};
    use crate::delve::together_shooter::phrasebook;
    let (mut app, workspace) = wishing_app("refund-why");
    let book_path = workspace.join(".angel/dungeon/phrasebook.txt");
    std::fs::create_dir_all(book_path.parent().unwrap()).unwrap();
    std::fs::write(
        &book_path,
        "wish firebolt | Firebolt\nkind spell\nsay a firebolt\ntier cast bolt; cooldown 6 :: A firebolt from your hand.\n",
    )
    .unwrap();
    crate::delve::together_files::refresh_phrasebook(&workspace);
    {
        let run = app.realm.dungeon.shooter.edit().unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        hero.spells = std::array::from_fn(|i| Some(format!("pad-{i}")));
        hero.deck.retain(|id| !id.starts_with("wish-firebolt"));
        hero.wishes = 0;
    }
    let deck = app.realm.dungeon.wish_deck(1);
    let book = phrasebook::book();
    let fire = book.iter().find(|w| w.id == "firebolt");
    assert!(
        fire.is_some(),
        "firebolt missing; spells {:?}",
        book.iter()
            .filter(|w| w.kind == "spell")
            .map(|w| w.id.as_str())
            .collect::<Vec<_>>()
    );
    assert!(fire.unwrap().next_tier(&deck).is_some(), "deck {deck:?}");
    app.realm.dungeon.forge.looked = None;
    let ask = crate::delve::together_shooter::genie::Asked {
        cosmetic: false,
        cosmetic_name: String::new(),
        fairy: false,
        star: false,
        knight: 1,
        words: "a firebolt".into(),
        steps: 1,
        great: false,
        fills: true,
        making: None,
        target: Default::default(),
    };
    app.realm.dungeon.shooter.edit().unwrap().asked.push(ask);
    let claim = app
        .dungeon
        .shooter
        .new_wish_claims(&mut None, "Test")
        .unwrap()
        .remove(0);
    app.realm.dungeon.forge.wish_session = app.realm.dungeon.shooter.wish_session();
    app.realm.dungeon.forge.learning.push(Lesson {
        claim: Some(claim),
        taught: true,
        pick: Pick::Chosen,
        returned: true,
        learned_id: Some("firebolt".into()),
        ..Lesson::new(1, "a firebolt".into())
    });
    let before = held(&app, 1);
    app.watch_learning();
    assert_eq!(held(&app, 1), before + 1, "the spent wish came back");
    // The seat's other notices (the join rite's among them) are not the wish's.
    let notices: Vec<String> = run(&app)
        .seat_notices
        .iter()
        .filter(|(id, _)| *id == 1)
        .map(|(_, text)| text.clone())
        .collect();
    let line = notices
        .iter()
        .find(|text| text.contains("given back"))
        .cloned()
        .unwrap_or_default();
    assert!(line.contains("given back"), "{notices:?}");
    assert!(line.contains("all four spell slots are full"), "{line}");
    assert!(
        app.realm.dungeon.notice.contains("given back"),
        "{}",
        app.realm.dungeon.notice
    );
    forget(&workspace);
}

/// A wish still being written is never granted a friend's wish that lands
/// in the book first, whatever else is in flight: "throwing dice that
/// release a random turret" was given a friend's Fart Cloud (2026-10-23).
#[test]
fn a_wish_in_flight_is_not_granted_a_friends_new_entry() {
    use angel_realm_host::control::{Lesson, Pick};
    let (mut app, workspace) = wishing_app("no-crossed-wish");
    crate::delve::together_files::refresh_phrasebook(&workspace);
    let before: Vec<String> = crate::delve::together_shooter::phrasebook::book()
        .iter()
        .map(|w| w.id.clone())
        .collect();
    {
        let run = app.realm.dungeon.shooter.edit().unwrap();
        run.players.get_mut(&1).unwrap().wishes = 0;
    }
    // Knight 1's dice wish is out with its runner, nothing back yet.
    let words = "throwing dice that release a random turret based on what number lands";
    app.realm.dungeon
        .shooter
        .edit()
        .unwrap()
        .asked
        .push(crate::delve::together_shooter::genie::Asked {
            cosmetic: false,
            cosmetic_name: String::new(),
            fairy: false,
            star: false,
            knight: 1,
            words: words.into(),
            steps: 1,
            great: false,
            fills: true,
            making: None,
            target: Default::default(),
        });
    let claim = app
        .dungeon
        .shooter
        .new_wish_claims(&mut None, "Test")
        .unwrap()
        .remove(0);
    app.realm.dungeon.forge.wish_session = app.realm.dungeon.shooter.wish_session();
    app.realm.dungeon.forge.learning.push(Lesson {
        claim: Some(claim),
        taught: true,
        pick: Pick::Chosen,
        before: Some(before),
        job: Some(7),
        ..Lesson::new(1, words.into())
    });
    // A friend's fart cloud lands in the book first.
    let book_path = workspace.join(".angel/dungeon/phrasebook.txt");
    std::fs::create_dir_all(book_path.parent().unwrap()).unwrap();
    std::fs::write(
        &book_path,
        "wish fart-cloud | Fart Cloud\nkind hold\nsay a fart cloud | fart cloud\ntier fart 1 :: Your hits leave a small fart cloud.\n",
    )
    .unwrap();
    crate::delve::together_files::refresh_phrasebook(&workspace);
    app.realm.dungeon.forge.looked = None;
    app.watch_learning();
    assert!(
        !run(&app).players[&1]
            .deck
            .iter()
            .any(|c| c.starts_with("wish-fart-cloud")),
        "the dice wish was granted the friend's fart cloud: {:?}",
        run(&app).players[&1].deck
    );
    assert!(
        app.realm.dungeon
            .forge
            .learning
            .iter()
            .any(|l| l.player == 1 && l.job == Some(7)),
        "the dice wish still waits for its own runner"
    );
    forget(&workspace);
}

#[test]
fn dungeon_wish_upgrades_show_spells_first_and_pick_exactly_the_next_tier() {
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("upgrade-held");
    let spell = crate::delve::together_shooter::phrasebook::get("firebolt").unwrap();
    let hold = crate::delve::together_shooter::phrasebook::get("volley").unwrap();
    {
        let run = app.realm.dungeon.shooter.edit().unwrap();
        run.grant(1, &hold).unwrap();
        run.grant(1, &spell).unwrap();
    }
    let wishes = held(&app, 1);
    app.open_reforge();
    for (w, h) in [(160, 48), (80, 24)] {
        let screen = paint(&mut app, w, h);
        assert!(screen.contains("Upgrade what you hold"), "{screen}");
        let spell_at = screen.find("1 Firebolt  I → II").expect(&screen);
        let hold_at = screen.find("2 Triple Volley  I → II").expect(&screen);
        assert!(spell_at < hold_at, "spells first");
        assert!(screen.contains(&spell.tiers[1].1), "{screen}");
        assert!(screen.contains(&hold.tiers[1].1), "{screen}");
    }
    press(&mut app, KeyCode::Char('1'));
    assert!(app.realm.dungeon.forge.draft.is_none());
    assert_eq!(held(&app, 1), wishes, "the usual offer comes before spending");
    assert_eq!(run(&app).players[&1].offer.as_ref().unwrap().choices[0].gives, "Firebolt II");
    press(&mut app, KeyCode::Char('1'));
    let hero = &run(&app).players[&1];
    assert!(hero.deck.contains(&spell.card_id(1)) && !hero.deck.contains(&spell.card_id(0)));
    assert!(hero.deck.contains(&hold.card_id(0)) && !hero.deck.contains(&hold.card_id(1)));
    assert_eq!(held(&app, 1), wishes - 1);
    // Typing still uses the same path, including a digit inside the line.
    speak(&mut app, "upgrade Triple Volley");
    assert_eq!(run(&app).players[&1].offer.as_ref().unwrap().choices[0].gives, "Triple Volley II");
    press(&mut app, KeyCode::Esc);
    app.open_reforge();
    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.realm.dungeon.forge.draft.as_ref().unwrap().words, "a1");
    forget(&workspace);
}

#[test]
fn dungeon_wish_upgrades_show_max_and_open_greyed_when_no_wishes_remain() {
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("upgrade-empty");
    let spell = crate::delve::together_shooter::phrasebook::get("firebolt").unwrap();
    {
        let run = app.realm.dungeon.shooter.edit().unwrap();
        // Deep enough down for a ladder's fifth (`overclass::top_tier`).
        run.dungeon.depth = angel_delve::rules::FLOORS;
        for _ in &spell.tiers { run.grant(1, &spell).unwrap(); }
        run.grant(1, &crate::delve::together_shooter::phrasebook::get("volley").unwrap()).unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        (hero.wishes, hero.great_wishes) = (0, 0);
    }
    app.open_reforge();
    let screen = paint(&mut app, 160, 48);
    assert!(screen.contains("Firebolt  V  MAX"), "{screen}");
    assert!(screen.contains("No wishes held: each floor brings two"), "{screen}");
    assert!(app.realm.dungeon.wish_upgrades(1).0.iter().all(|u| !u.available));
    press(&mut app, KeyCode::Char('1'));
    assert!(run(&app).players[&1].offer.is_none());
    assert_eq!(app.realm.dungeon.forge.draft.as_ref().unwrap().words, "1");
    forget(&workspace);
}

#[test]
fn dungeon_wish_upgrade_pages_reach_every_held_ladder() {
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("upgrade-pages");
    let book = crate::delve::together_shooter::phrasebook::book();
    for wish in book.iter().filter(|w| !w.learned && w.kind == "hold").take(12) {
        app.realm.dungeon.shooter.edit().unwrap().grant(1, wish).unwrap();
    }
    let last = app.realm.dungeon.wish_upgrades(1).0.last().unwrap().clone();
    app.open_reforge();
    for _ in 0..5 { press(&mut app, KeyCode::PageDown); }
    let screen = paint(&mut app, 160, 48);
    assert!(screen.contains(&last.name), "the twelfth held wish is reachable: {screen}");
    assert!(screen.contains("PgUp/PgDn: more held · 6/6"), "{screen}");
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(run(&app).players[&1].offer.as_ref().unwrap().choices[0].gives, format!("{} II", last.name));
    forget(&workspace);
}

#[test]
fn a_shooting_stars_wish_alone_opens_the_wish_line_and_spends_nothing_held() {
    let _guard = crate::tests::env_lock();
    let (mut app, workspace) = wishing_app("starfall");
    {
        let run = app.realm.dungeon.shooter.edit().unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        (hero.wishes, hero.great_wishes) = (0, 0);
    }
    // Nothing held: the lamp's line stays shut.
    app.open_lamp_wish();
    assert!(app.realm.dungeon.forge.draft.is_none(), "no wish, no line");
    app.realm.dungeon.shooter.edit().unwrap().star_falls_for_test();
    assert!(run(&app).players[&1].star_wish, "caught under the star");
    app.open_lamp_wish();
    assert!(app.realm.dungeon.forge.draft.is_some(), "a star's wish alone opens the wish line");
    assert_eq!(app.wish_upgrades(1).1, Some(crate::delve::together_shooter::genie::STAR_HELD));
    let screen = paint(&mut app, 160, 48);
    assert!(screen.contains("A star's wish: free"), "{screen}");
    // A known wish off the list, as you said: the star's.
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    let offer = run(&app).players[&1].offer.clone().expect("the star's scroll");
    assert!(offer.star && !offer.great, "{offer:?}");
    press(&mut app, KeyCode::Char('1'));
    let hero = &run(&app).players[&1];
    assert!(!hero.star_wish, "the star's wish is spent");
    assert_eq!((hero.wishes, hero.great_wishes), (0, 0), "and nothing of the knight's own");
    assert!(hero.deck.iter().any(|c| c.starts_with("wish-") && c.ends_with("-1")), "{:?}", hero.deck);
    forget(&workspace);
}

#[test]
fn joined_angelx_wish_upgrades_show_its_own_spells_and_reach_the_hosts_scroll() {
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut host = App::preview(crate::ui::viewer::Viewer::static_preview());
    let message = host_when_admitted!(host, "host --1");
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(&format!("join {}", link_of(&message))));
    let spin = |host: &mut App, friend: &mut App, ready: &dyn Fn(&App, &App) -> bool| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while !ready(host, friend) {
            // Guest traffic is serviced even when the local view is paused.
            host.advance_dungeon_guest();
            host.advance_shooter_at(std::time::Instant::now());
            friend.advance_joined();
            assert!(std::time::Instant::now() < deadline, "wish upgrade never reached the other seat: {} / {}", host.dungeon.notice, friend.dungeon.notice);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    spin(&mut host, &mut friend, &|h, f| run(h).players.contains_key(&2) && f.joined_knight().is_some());
    let spell = crate::delve::together_shooter::phrasebook::get("firebolt").unwrap();
    {
        let run = host.dungeon.shooter.edit().unwrap();
        run.grant(2, &spell).unwrap();
        run.grant(2, &crate::delve::together_shooter::phrasebook::get("volley").unwrap()).unwrap();
        run.players.get_mut(&2).unwrap().wishes = 2;
    }
    spin(&mut host, &mut friend, &|_, f| f.wish_upgrades(2).0.len() == 2);
    paint(&mut friend, 160, 48);
    press(&mut friend, KeyCode::Char('t'));
    let screen = paint(&mut friend, 160, 48);
    assert!(screen.contains("1 Firebolt  I → II") && screen.contains("2 Triple Volley  I → II"), "{screen}");
    assert!(screen.contains(&spell.tiers[1].1), "{screen}");
    press(&mut friend, KeyCode::Char('1'));
    spin(&mut host, &mut friend, &|h, f| run(h).players[&2].offer.is_some() && f.wish_offer_open(2));
    assert_eq!(run(&host).players[&2].offer.as_ref().unwrap().choices[0].gives, "Firebolt II");
    assert_eq!(run(&host).players[&2].wishes, 2);
    press(&mut friend, KeyCode::Char('1'));
    spin(&mut host, &mut friend, &|h, _| run(h).players[&2].deck.contains(&spell.card_id(1)));
    assert_eq!(run(&host).players[&2].wishes, 1);
    assert!(run(&host).players[&1].deck.iter().all(|c| !c.starts_with("wish-firebolt-")));
    host.dungeon.shooter.edit().unwrap().players.get_mut(&2).unwrap().wishes = 0;
    spin(&mut host, &mut friend, &|_, f| f.wish_upgrades(2).1 == Some("No wishes held: each floor brings two"));
    press(&mut friend, KeyCode::Char('t'));
    assert!(paint(&mut friend, 160, 48).contains("No wishes held: each floor brings two"));
    press(&mut friend, KeyCode::Char('1'));
    assert!(friend.dungeon.forge.draft.is_some(), "nothing can be picked");
}

#[test]
fn realm_claim_restores_after_uncertain_destination_write_without_repaying_or_losing_later_spoils()
{
    use crate::delve::together_realm::{Realm, Spoil, Spoils, Haul};
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = game();
    fixture.bind(&mut app);
    let mut gold = Spoils::default();
    gold.add(Spoil::Gold, 19);
    app.realm.dungeon.shooter.edit().unwrap().bank.push(Haul {
        hero: 1,
        spoils: gold,
        why: "first claim".into(),
    });
    app.realm.dungeon.shooter.edit().unwrap().triumph =
        Some(crate::delve::together_shooter::Triumph::Dragon);
    let realm_path = app
        .world
        .rewards_path()
        .unwrap()
        .with_extension("realm.json");
    crate::platform::workspace_store::storage_atomic_tests::fail_after_publish(realm_path);
    assert!(!app.settle_realm());
    assert!(
        app.realm.dungeon.notice.contains("after publish"),
        "{}",
        app.realm.dungeon.notice
    );
    let pending = app.realm.dungeon.shooter.claim_realm().unwrap().unwrap();
    assert_eq!(
        Realm::beside(app.realm.world.rewards_path())
            .treasury
            .get(Spoil::Gold),
        19
    );
    let mut gold = Spoils::default();
    gold.add(Spoil::Gold, 7);
    app.realm.dungeon.shooter.edit().unwrap().bank.push(Haul {
        hero: 1,
        spoils: gold,
        why: "earned while pending".into(),
    });
    app.realm.dungeon.shooter.edit().unwrap().triumph =
        Some(crate::delve::together_shooter::Triumph::Grail);
    // The ordinary private checkpoint captures both the retained identity and
    // later unclaimed rewards. No special shutdown repair/requeue is needed.
    app.realm.dungeon
        .checkpoint(std::time::Instant::now(), true)
        .unwrap();
    drop(app);
    let mut restored = game();
    fixture.bind(&mut restored);
    restored.realm.dungeon.shooter.set(None);
    restored.realm.dungeon.restore(&fixture.workspace).unwrap();
    assert_eq!(
        restored.realm.dungeon.shooter.claim_realm().unwrap(),
        Some(pending)
    );
    assert!(restored.settle_realm(), "{}", restored.realm.dungeon.notice);
    assert_eq!(restored.economy().treasury.get(Spoil::Gold), 26);
    assert_eq!(restored.economy().raids_won, 1);
    assert_eq!(restored.economy().grails, 1);
    assert!(restored.realm.dungeon.shooter.claim_realm().unwrap().is_none());
    assert!(restored.settle_realm());
    assert_eq!(
        Realm::beside(restored.world.rewards_path())
            .treasury
            .get(Spoil::Gold),
        26
    );
    let saved: serde_json::Value = serde_json::from_slice(
        &std::fs::read(restored.realm.dungeon.save_path.as_ref().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["commands"]["records"].as_array().unwrap().len(), 0);
}
#[test]
fn realm_claim_waits_for_its_source_checkpoint_and_close_drains_more_than_one_bounded_claim() {
    use crate::delve::together_realm::{Realm, Spoil, Spoils, Haul};
    let _guard = crate::tests::env_lock();
    let (fixture, _home) = with_blocked_realm_save();
    fixture.allow_saves();
    let mut app = game();
    fixture.bind(&mut app);
    let blocker = fixture.workspace.join("checkpoint-blocker");
    std::fs::write(&blocker, b"not a directory").unwrap();
    app.realm.dungeon.save_path = Some(blocker.join("session.json"));
    let mut gold = Spoils::default();
    gold.add(Spoil::Gold, 1);
    for _ in 0..65 {
        app.realm.dungeon.shooter.edit().unwrap().bank.push(Haul {
            hero: 1,
            spoils: gold.clone(),
            why: "bounded claim".into(),
        });
    }
    assert!(!app.settle_realm());
    assert_eq!(
        Realm::beside(app.realm.world.rewards_path())
            .treasury
            .get(Spoil::Gold),
        0
    );
    let claim = app.realm.dungeon.shooter.claim_realm().unwrap().unwrap();
    assert_eq!(claim.transfer.hauls.len(), 64);
    assert_eq!(run(&app).bank.len(), 1);
    app.realm.dungeon.save_path = Some(fixture.workspace.join(".angel/dungeon/session.json"));
    let closed = app.dungeon_command(Some("off"));
    assert!(closed.contains("Dungeon closed"), "{closed}");
    assert!(app.realm.dungeon.shooter.as_ref().is_none());
    assert_eq!(
        Realm::beside(app.realm.world.rewards_path())
            .treasury
            .get(Spoil::Gold),
        65
    );
}

#[test]
fn a_threaded_host_settles_off_its_frame_and_takes_the_same_realm_back() {
    let _guard = crate::tests::env_lock();
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    app.realm.dungeon.shooter.prefer_thread(true);
    app.input = "/dungeon start".into();
    app.submit();
    assert!(
        app.realm.dungeon.shooter.threaded(),
        "{}",
        app.realm.dungeon.notice
    );
    // Only this process's copy knows the asked wish: a realm read from disk
    // while the settler held it would not.
    let asked = app.economy().ask("Tester", "a lantern that hums").unwrap();
    assert!(
        app.settle_off_frame(),
        "a threaded host settles off its frame"
    );
    assert!(app.realm.dungeon.settling_off_frame());
    assert!(
        app.realm.dungeon.realm.is_none(),
        "the settler holds the realm"
    );
    assert!(
        app.realm
            .dungeon
            .realm_shown()
            .is_some_and(|realm| realm.wishes.iter().any(|w| w.id == asked)),
        "the screen shows the realm's copy meanwhile"
    );
    // A second cycle while one runs starts nothing new.
    assert!(app.settle_off_frame());
    // Asked for, the realm comes back from the settler, the same one.
    assert!(app.economy().wishes.iter().any(|w| w.id == asked));
    assert!(!app.realm.dungeon.settling_off_frame());
    assert!(app.realm.dungeon.realm_shown().is_some());
    // And the frame's own collection finds nothing left to take.
    app.realm.dungeon.collect_settled(false);
    assert!(app.economy().wishes.iter().any(|w| w.id == asked));
}
