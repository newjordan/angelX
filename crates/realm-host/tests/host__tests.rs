//! The seam between the cockpit and the game: the game's controls run on a
//! context that holds only what the contract lends (`angel_live::realm`).

use super::*;
use angel_live::realm::{Facts, HostCx, Move, Oracle, Panel, RealmHost};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// A cockpit that has nothing to ask a model.
struct NoOracle;

impl Oracle for NoOracle {
    fn respond(&self, _prompt: &str, _at_least: Duration) -> Result<String, String> {
        Err("no model in a test".into())
    }

    fn label(&self) -> &str {
        "none"
    }
}

/// The cockpit as a test holds it: a composer, a transcript, the moves asked.
struct TestCx {
    facts: Facts,
    workspace: PathBuf,
    composer: (String, usize),
    said: Vec<String>,
    moves: Vec<Move>,
    redraws: usize,
}

impl TestCx {
    fn new(label: &str) -> Self {
        let workspace = std::env::temp_dir().join(format!("host-seam-{label}-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&workspace);
        Self {
            facts: Facts { composer_empty: true, terminal_focused: true, ..Facts::default() },
            workspace,
            composer: (String::new(), 0),
            said: Vec::new(),
            moves: Vec::new(),
            redraws: 0,
        }
    }

    fn type_in(&mut self, text: &str) {
        self.composer = (text.to_string(), text.len());
        self.facts.composer_empty = text.is_empty();
    }
}

impl HostCx for TestCx {
    fn facts(&self) -> Facts {
        self.facts
    }

    fn workspace(&self) -> PathBuf {
        self.workspace.clone()
    }

    fn oracle(&self) -> Arc<dyn Oracle> {
        Arc::new(NoOracle)
    }

    fn say(&mut self, text: String) {
        self.said.push(text);
    }

    fn redraw(&mut self) {
        self.redraws += 1;
    }

    fn take_composer(&mut self) -> (String, usize) {
        self.facts.composer_empty = true;
        std::mem::take(&mut self.composer)
    }

    fn set_composer(&mut self, text: String, cursor: usize) {
        self.facts.composer_empty = text.is_empty();
        self.composer = (text, cursor);
    }

    fn make(&mut self, ask: Move) {
        self.moves.push(ask);
    }
}

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn host() -> GameHost {
    GameHost::new(world_viz::World::new(7))
}

#[test]
fn a_host_with_no_game_open_leaves_every_key_to_the_cockpit() {
    let mut host = host();
    let mut cx = TestCx::new("keys");
    assert!(!RealmHost::view_active(&host, &cx.facts));
    assert!(!RealmHost::keyboard_active(&host, &cx.facts));
    assert!(!RealmHost::wants_fast_tick(&host, &cx.facts));
    for code in [KeyCode::Char('w'), KeyCode::Esc, KeyCode::F(2), KeyCode::Enter] {
        assert!(!RealmHost::key(&mut host, &mut cx, press(code)), "{code:?}");
        assert!(!RealmHost::walk_key(&mut host, &mut cx, press(code)), "{code:?}");
    }
    assert!(cx.moves.is_empty());
    assert!(cx.said.is_empty());
}

#[test]
fn the_game_view_keeps_the_composers_draft_and_gives_it_back() {
    let mut host = host();
    let mut cx = TestCx::new("draft");
    cx.type_in("fix the build");
    host.session(&mut cx).expand_dungeon();
    // The draft left the composer for the game's keeping, the stage came up,
    // and the cockpit was asked to move there.
    assert_eq!(cx.composer, (String::new(), 0));
    assert_eq!(
        cx.moves,
        vec![Move::DismissIntro, Move::ShowWorld, Move::Focus("artifacts")]
    );
    assert!(host.dungeon.expanded);
    cx.moves.clear();

    RealmHost::collapse(&mut host, &mut cx);
    assert_eq!(cx.composer, ("fix the build".to_string(), 13));
    assert_eq!(cx.moves, vec![Move::Focus("core")]);
    assert!(!host.dungeon.expanded);
}

#[test]
fn slash_hands_the_keyboard_back_with_a_command_started() {
    let mut host = host();
    let mut cx = TestCx::new("slash");
    host.session(&mut cx).expand_dungeon();
    // Something is open in the game view for the key to find: the intro.
    host.dungeon.intro = Some(crate::delve_intro::Intro::default());
    cx.facts.composer_empty = true;
    let taken = RealmHost::key(&mut host, &mut cx, press(KeyCode::Char('x')));
    // The intro takes its own keys; an unrelated letter is not the cockpit's.
    assert!(taken);
    assert!(cx.composer.0.is_empty());
}

#[test]
fn a_door_in_the_map_asks_the_cockpit_for_its_panel() {
    use crate::world_viz::overworld::Opens;
    let mut host = host();
    let mut cx = TestCx::new("door");
    // No visitor at a door: nothing is asked.
    RealmHost::go_in(&mut host, &mut cx);
    assert!(cx.moves.is_empty());
    let _ = Opens::Library;
    let _ = Panel::Library;
}

#[test]
fn the_holdings_hear_a_turn_through_the_contract() {
    let mut host = host();
    RealmHost::note_call(&mut host, "t1", "shell", "command=git commit -m \"x\"");
    RealmHost::note_result(&mut host, "t1", true);
    RealmHost::note_turn(&mut host, 10, 20, 0);
    RealmHost::note_loop(&mut host, 2);
    assert_eq!(host.dungeon.ops_loop, 2);
}

#[test]
fn browser_capacity_refusal_preserves_hosted_runs_and_retry_after_retirement() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Instant;
    let _guard = crate::tests::browser_capacity();
    if crate::together_web::BUNDLE.is_none() { return; }
    let mut first = host(); let mut first_cx = TestCx::new("browser-capacity-one");
    first.dungeon.shooter.prefer_thread(false);
    let started = Instant::now();
    let outcome = first.session(&mut first_cx).dungeon_command(Some("host 127.0.0.1:0"));
    let first_activation = started.elapsed();
    assert!(first.dungeon.shooter.hosted(), "{outcome}");
    let identity = first.dungeon.shooter.invitation().unwrap();
    let raid = first.dungeon.shooter.as_ref().unwrap().raid_id;
    let mut second = host(); let mut second_cx = TestCx::new("browser-capacity-two");
    second.dungeon.shooter.prefer_thread(false);
    let outcome = second.session(&mut second_cx).dungeon_command(Some("host 127.0.0.1:0"));
    assert!(second.dungeon.shooter.hosted(), "{outcome}");
    let mut refused = host(); let mut refused_cx = TestCx::new("browser-capacity-three");
    refused.dungeon.shooter.prefer_thread(false);
    let started = Instant::now();
    let outcome = refused.session(&mut refused_cx).dungeon_command(Some("host 127.0.0.1:0"));
    assert!(outcome.contains("browser generations busy"), "{outcome}");
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(!refused.dungeon.shooter.hosted());
    assert_eq!(first.dungeon.shooter.invitation().unwrap(), identity);
    assert_eq!(first.dungeon.shooter.as_ref().unwrap().raid_id, raid);
    let mut guest = host(); let mut guest_cx = TestCx::new("browser-capacity-guest");
    let link = format!("join http://{}/#{}", identity.0, identity.1);
    let outcome = guest.session(&mut guest_cx).dungeon_command(Some(&link));
    assert!(outcome.starts_with("Joined the delve"), "{outcome}");
    let until_joined = Instant::now() + Duration::from_secs(10);
    loop {
        first.session(&mut first_cx).advance_shooter_at(Instant::now());
        guest.session(&mut guest_cx).advance_joined();
        if guest.dungeon.joined.as_ref().unwrap().seat.knight().is_some() { break; }
        assert!(Instant::now() < until_joined, "native guest never welcomed while browser generation was preparing");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut request = TcpStream::connect(first.dungeon.shooter.server().unwrap().addr()).unwrap();
    request.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    request.write_all(b"GET / HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
    let started = Instant::now(); let mut page = Vec::new(); request.read_to_end(&mut page).unwrap();
    let cold_http = started.elapsed();
    assert!(page.starts_with(b"HTTP/1.1 200"));
    assert!(first.dungeon.shooter.browser_failure().is_none());
    let save = first_cx.workspace.join(".angel/dungeon/session.json");
    first.dungeon.shooter.save_checkpoint(&save, raid).unwrap();
    let mut restore_attempt = host();
    restore_attempt.dungeon.shooter.prefer_thread(false);
    restore_attempt.dungeon.shooter.set(Some(crate::together_shooter::Run::new(99, 987, None)));
    let error = restore_attempt.dungeon.restore(&first_cx.workspace).unwrap_err();
    assert!(error.contains("browser generations busy"), "{error}");
    assert_eq!(restore_attempt.dungeon.shooter.as_ref().unwrap().raid_id, 987);
    assert!(!restore_attempt.dungeon.shooter.hosted());
    assert_eq!(restore_attempt.dungeon.shooter.seat(1).unwrap().knight(), Some(1));
    // Retirement must include the worker, not merely dropping its UI handle.
    drop(second);
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let outcome = refused.session(&mut refused_cx).dungeon_command(Some("host 127.0.0.1:0"));
        if refused.dungeon.shooter.hosted() { break; }
        assert!(outcome.contains("browser generations busy"), "{outcome}");
        assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(first.dungeon.shooter.invitation().unwrap(), identity);
    assert_eq!(first.dungeon.shooter.as_ref().unwrap().raid_id, raid);
    eprintln!("actual shipped host activation {first_activation:?}; first complete HTTP {cold_http:?}; third admission refused without changing running host/native guest");
    drop(guest); drop(refused); drop(first);
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        match restore_attempt.dungeon.restore(&first_cx.workspace) {
            Ok(()) => break,
            Err(error) if error.contains("browser generations busy") => {
                assert!(Instant::now() < until); std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("restore retry failed: {error}"),
        }
    }
    assert_eq!(restore_attempt.dungeon.shooter.as_ref().unwrap().raid_id, raid);
    assert_eq!(restore_attempt.dungeon.shooter.invitation().unwrap(), identity);
}

#[test]
fn failed_browser_notices_and_retries_without_replacing_native_session() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Instant;
    let _guard = crate::tests::browser_capacity();
    if crate::together_web::BUNDLE.is_none() { return; }
    let mut game = host(); let mut cx = TestCx::new("browser-failure-retry");
    game.dungeon.shooter.prefer_thread(false);
    let music = cx.workspace.join("cockpit/assets/dungeon/music");
    std::fs::create_dir_all(&music).unwrap();
    let oversized = music.join("oversized.mp3");
    std::fs::File::create(&oversized).unwrap().set_len(angel_serve::http::preparation::MAX_FILE_BYTES as u64 + 1).unwrap();
    let _resource = crate::tests::TestEnvGuard::set("ANGEL_RESOURCE_DIR", cx.workspace.to_str().unwrap());
    // A preceding lifecycle test can still own cancelled compression work.
    // Admission stays nonblocking and charged until that real work retires.
    let until_retired = Instant::now() + Duration::from_secs(20);
    loop {
        let outcome = game.session(&mut cx).dungeon_command(Some("host 127.0.0.1:0"));
        if game.dungeon.shooter.hosted() { break; }
        assert!(outcome.contains("browser generations busy"), "{outcome}");
        assert!(Instant::now() < until_retired); std::thread::sleep(Duration::from_millis(20));
    }
    let identity = game.dungeon.shooter.invitation().unwrap();
    let raid = game.dungeon.shooter.as_ref().unwrap().raid_id;
    let request = |game: &GameHost| {
        let mut socket = TcpStream::connect(game.dungeon.shooter.server().unwrap().addr()).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        socket.write_all(b"GET / HTTP/1.1\r\nConnection: close\r\n\r\n").unwrap();
        let mut page = Vec::new(); socket.read_to_end(&mut page).unwrap(); page
    };
    assert!(request(&game).starts_with(b"HTTP/1.1 503"));
    game.session(&mut cx).advance_shooter_at(Instant::now());
    assert!(game.dungeon.notice.contains("/dungeon host retries"), "{}", game.dungeon.notice);
    std::fs::remove_file(oversized).unwrap();
    let outcome = game.session(&mut cx).dungeon_command(Some("host"));
    assert!(outcome.contains("retry started"), "{outcome}");
    assert!(request(&game).starts_with(b"HTTP/1.1 200"));
    assert_eq!(game.dungeon.shooter.invitation().unwrap(), identity);
    assert_eq!(game.dungeon.shooter.as_ref().unwrap().raid_id, raid);
    let save = cx.workspace.join(".angel/dungeon/session.json");
    game.dungeon.shooter.save_checkpoint(&save, raid).unwrap();
    let oversized = music.join("oversized.mp3");
    std::fs::File::create(&oversized).unwrap().set_len(angel_serve::http::preparation::MAX_FILE_BYTES as u64 + 1).unwrap();
    drop(game);
    let mut restored = host();
    restored.dungeon.shooter.prefer_thread(false);
    let started = Instant::now();
    restored.dungeon.restore(&cx.workspace).unwrap();
    let restore_time = started.elapsed();
    assert_eq!(restored.dungeon.shooter.invitation().unwrap(), identity);
    assert_eq!(restored.dungeon.shooter.as_ref().unwrap().raid_id, raid);
    assert!(request(&restored).starts_with(b"HTTP/1.1 503"));
    restored.session(&mut cx).advance_shooter_at(Instant::now());
    assert!(restored.dungeon.notice.contains("/dungeon host retries"));
    std::fs::remove_file(oversized).unwrap();
    let outcome = restored.session(&mut cx).dungeon_command(Some("host"));
    assert!(outcome.contains("retry started"), "{outcome}");
    assert!(request(&restored).starts_with(b"HTTP/1.1 200"));
    assert_eq!(restored.dungeon.shooter.invitation().unwrap(), identity);
    assert_eq!(restored.dungeon.shooter.as_ref().unwrap().raid_id, raid);
    eprintln!("actual hosted checkpoint restore activation {restore_time:?}; same raid and link, browser generated off frame");
}

/// A delve hosted on loopback by one game, and a second game joined to it
/// from the composer: (host, guest, guest's cockpit). The join key is a
/// secret, so no assertion here prints the link or what the game said about it.
fn friends_delve(label: &str) -> (GameHost, GameHost, TestCx) {
    let mut hosted = host();
    hosted.dungeon.shooter.set(Some(crate::together_shooter::Run::new(7, 1, None)));
    hosted
        .dungeon
        .shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 2,
            key: None,
            bundle: None,
        })
        .unwrap();
    let (addr, key) = hosted.dungeon.shooter.invitation().unwrap();
    let mut guest = host();
    let mut cx = TestCx::new(label);
    let said = guest
        .session(&mut cx)
        .dungeon_command(Some(&format!("join http://{addr}/#{key}")));
    assert!(said.starts_with("Joined the delve"), "the guest did not join");
    assert!(guest.dungeon.joined.is_some());
    (hosted, guest, cx)
}

#[test]
fn a_friends_delve_is_never_open_beside_a_game_of_ones_own() {
    let _guard = crate::tests::env_lock();
    let (_hosted, mut guest, mut cx) = friends_delve("venue-commands");
    assert_eq!(guest.dungeon.venue(), crate::control::Venue::Joined);
    for verb in ["start", "host 127.0.0.1:0"] {
        guest.session(&mut cx).dungeon_command(Some(verb));
        assert!(
            guest.dungeon.joined.is_some()
                && guest.dungeon.shooter.is_none()
                && guest.dungeon.hosting.is_none(),
            "`/dungeon {verb}` opened a game beside the friend's delve"
        );
    }
    // The Delve's menu is the other way to a new game: the gate on the map
    // opens it, and its Begin starts one.
    guest.session(&mut cx).open_intro();
    for _ in 0..2 {
        guest.session(&mut cx).dungeon_key(press(KeyCode::Enter));
    }
    assert!(guest.dungeon.shooter.is_none(), "the menu began a delve beside the friend's");
    assert!(guest.dungeon.intro.is_none(), "the menu opened over the friend's delve");
    assert!(guest.dungeon.joined.is_some() && guest.dungeon.expanded);
}

#[test]
fn joining_a_friends_delve_closes_the_delves_menu() {
    let _guard = crate::tests::env_lock();
    let mut hosted = host();
    hosted.dungeon.shooter.set(Some(crate::together_shooter::Run::new(7, 1, None)));
    hosted
        .dungeon
        .shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 2,
            key: None,
            bundle: None,
        })
        .unwrap();
    let (addr, key) = hosted.dungeon.shooter.invitation().unwrap();
    let mut guest = host();
    let mut cx = TestCx::new("venue-menu");
    // The menu is open, then something other than its own Esc puts the game
    // away: the menu's choices stay.
    guest.session(&mut cx).open_intro();
    guest.session(&mut cx).collapse_dungeon();
    assert!(guest.dungeon.intro.is_some());
    let said = guest
        .session(&mut cx)
        .dungeon_command(Some(&format!("join http://{addr}/#{key}")));
    assert!(said.starts_with("Joined the delve"), "the guest did not join");
    assert!(guest.dungeon.intro.is_none(), "keys still go to a menu that can only begin another game");
    for _ in 0..2 {
        guest.session(&mut cx).dungeon_key(press(KeyCode::Enter));
    }
    assert!(guest.dungeon.shooter.is_none());
}


/// Host and guest enter the cave over the loopback socket. The host digs.
/// The guest's decoded run has that hole, and a guest who joins after the dig has it too.
#[test]
fn a_guest_decodes_the_cave_the_host_dug() {
    use std::time::Instant;
    let _guard = crate::tests::env_lock();
    let mut run = crate::together_shooter::Run::new(11, 2, Some("Briar"));
    // descend_for_test leaves floor 2. The mouth is stamped only on floor 1,
    // so a hold there never enters. A real session stamps the mouth on arrival.
    run.calm_for_test();
    if let Some(start) = run.dungeon.rooms.iter().position(|r| r.kind == crate::together_shooter::RoomKind::Start) {
        run.at = start;
        run.stamp_cave_mouth();
    }
    let mouth = crate::together_shooter::caves::FLOOR_MOUTH.at;
    let x = mouth.0 as f32 * crate::together_shooter::TILE_UNITS;
    let y = mouth.1 as f32 * crate::together_shooter::TILE_UNITS;
    for hero in run.players.values_mut() {
        hero.x = x;
        hero.y = y;
    }
    let mut hosted = host();
    hosted.dungeon.shooter.prefer_thread(false);
    hosted.dungeon.shooter.set(Some(run));
    hosted
        .dungeon
        .shooter
        .host(crate::together_sim::Door {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats: 4,
            key: None,
            bundle: None,
        })
        .unwrap();
    // host() serves a new live run. Controls set before that are discarded,
    // and the new server stays inactive, so the mouth hold never ticks.
    let playing = crate::together_sim::Controls {
        active: true,
        keyboard: true,
        hosted: true,
        watching: false,
        ..Default::default()
    };
    hosted.dungeon.shooter.set_controls(playing);
    let (addr, key) = hosted.dungeon.shooter.invitation().unwrap();
    let mut guest = host();
    let mut cx = TestCx::new("cave-wire");
    let said = guest
        .session(&mut cx)
        .dungeon_command(Some(&format!("join http://{addr}/#{key}")));
    assert!(said.starts_with("Joined the delve"), "the guest did not join");

    let hold = {
        let mut input = crate::together_shooter::Input::default();
        input.interact = true;
        crate::together_seat::bits(&input)
    };
    let mut snapshot = 0u64;
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut stepped = 0u32;
    while Instant::now() < deadline {
        if let Some(seat) = hosted.dungeon.shooter.seat(1) {
            seat.set_held(hold);
            seat.tick(Instant::now());
        }
        hosted.dungeon.shooter.pump(Instant::now());
        stepped += 1;
        // as_ref caches the first snapshot. The cockpit latches each frame.
        hosted.dungeon.shooter.latch();
        if let Some(joined) = guest.dungeon.joined.as_mut() {
            joined.seat.set_held(hold);
            joined.seat.tick(Instant::now());
            joined.refresh(std::time::Instant::now());
            if snapshot == 0 && joined.run().is_some() {
                snapshot = joined.seat.socket_bytes().0;
            }
        }
        std::thread::sleep(Duration::from_millis(30));
        let host_in = hosted.dungeon.shooter.as_ref().is_some_and(|r| r.caves.inside(1));
        let guest_in = guest
            .dungeon
            .joined
            .as_ref()
            .and_then(|j| j.run())
            .is_some_and(|r| r.caves.inside(1));
        if host_in && guest_in {
            break;
        }
    }
    let host_run = hosted.dungeon.shooter.as_ref().expect("host run");
    if !host_run.caves.inside(1) {
        let hero = host_run.players.get(&1);
        eprintln!(
            "enter miss steps={stepped} depth={} stone={:?} pos={:?}",
            host_run.dungeon.depth,
            hero.map(|h| h.stone),
            hero.map(|h| (h.x, h.y)),
        );
    }
    assert!(host_run.caves.inside(1), "host knight never entered");
    let decoded = guest.dungeon.joined.as_ref().and_then(|j| j.run()).expect("guest run");
    assert!(decoded.caves.inside(1), "guest decoded run has no cave seat");
    assert_eq!(decoded.caves.seed, host_run.caves.seed);

    let aim = host_run.caves.soft_aim().expect("a soft face");
    if let Some(run) = hosted.dungeon.shooter.edit() {
        if let Some(seat) = run.caves.seats.get_mut(&1) {
            seat.x = aim.0;
            seat.y = aim.1;
        }
    }
    let mut dig = crate::together_shooter::Input::default();
    dig.fire = true;
    dig.aim_x = aim.2;
    dig.aim_y = aim.3;
    let bits = crate::together_seat::bits(&dig);
    let before = hosted.dungeon.shooter.seat(1).map(|s| s.socket_bytes().0).unwrap_or(0);
    let mut guest_saw_bursts = false;
    let dig_deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < dig_deadline {
        if let Some(seat) = hosted.dungeon.shooter.seat(1) {
            seat.set_held(bits);
            seat.tick(Instant::now());
        }
        hosted.dungeon.shooter.pump(Instant::now());
        hosted.dungeon.shooter.latch();
        if let Some(joined) = guest.dungeon.joined.as_mut() {
            joined.seat.tick(Instant::now());
            joined.refresh(std::time::Instant::now());
        }
        std::thread::sleep(Duration::from_millis(30));
        let host_fp = hosted.dungeon.shooter.as_ref().map(|r| r.caves.fingerprint());
        let guest_fp = guest
            .dungeon
            .joined
            .as_ref()
            .and_then(|j| j.run())
            .map(|r| r.caves.fingerprint());
        let dug = hosted
            .dungeon
            .shooter
            .as_ref()
            .is_some_and(|r| !r.caves.changes.is_empty());
        // The rock breaking a guest draws: the host's bursts, over the socket.
        guest_saw_bursts |= guest
            .dungeon
            .joined
            .as_ref()
            .and_then(|j| j.run())
            .is_some_and(|r| !r.caves.bursts.is_empty());
        if dug && host_fp.is_some() && host_fp == guest_fp {
            break;
        }
    }
    let host_run = hosted.dungeon.shooter.as_ref().expect("host run");
    assert!(!host_run.caves.changes.is_empty(), "host dig did not change a cell");
    if let Some(joined) = guest.dungeon.joined.as_mut() {
        joined.refresh(std::time::Instant::now());
    }
    let decoded = guest.dungeon.joined.as_ref().and_then(|j| j.run()).expect("guest run");
    if decoded.caves.fingerprint() != host_run.caves.fingerprint() {
        let bytes = guest.dungeon.joined.as_ref().map(|j| j.seat.socket_bytes());
        eprintln!(
            "fingerprint host {} guest {} host_changes {} guest_changes {} host_tick {} guest_tick {} bytes {:?}",
            host_run.caves.fingerprint(),
            decoded.caves.fingerprint(),
            host_run.caves.changes.len(),
            decoded.caves.changes.len(),
            host_run.tick,
            decoded.tick,
            bytes,
        );
    }
    assert_eq!(decoded.caves.fingerprint(), host_run.caves.fingerprint(), "guest fingerprint");
    assert!(!decoded.caves.changes.is_empty(), "guest missed the hole");
    assert!(guest_saw_bursts, "the guest never had the rock breaking to draw");

    let mut late = host();
    let mut late_cx = TestCx::new("cave-late");
    let said = late
        .session(&mut late_cx)
        .dungeon_command(Some(&format!("join http://{addr}/#{key}")));
    assert!(said.starts_with("Joined the delve"), "late guest did not join");
    let want = host_run.caves.fingerprint();
    let late_deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < late_deadline {
        hosted.dungeon.shooter.pump(Instant::now());
        hosted.dungeon.shooter.latch();
        if let Some(joined) = late.dungeon.joined.as_mut() {
            joined.seat.tick(Instant::now());
            joined.refresh(std::time::Instant::now());
            if joined.run().is_some_and(|r| r.caves.fingerprint() == want) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let late_run = late.dungeon.joined.as_ref().and_then(|j| j.run()).expect("late run");
    assert_eq!(late_run.caves.fingerprint(), want, "late joiner missed the hole");

    let after = hosted.dungeon.shooter.seat(1).map(|s| s.socket_bytes().0).unwrap_or(0);
    let guest_seat = guest.dungeon.joined.as_ref().expect("guest seat");
    let mut last = guest_seat.seat.socket_bytes().0;
    if snapshot == 0 {
        snapshot = last;
    }
    for _ in 0..40 {
        hosted.dungeon.shooter.pump(Instant::now());
        hosted.dungeon.shooter.latch();
        if let Some(joined) = guest.dungeon.joined.as_mut() {
            joined.seat.tick(Instant::now());
            joined.refresh(std::time::Instant::now());
            last = joined.seat.socket_bytes().0;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    let mut still = Vec::new();
    for _ in 0..60 {
        hosted.dungeon.shooter.pump(Instant::now());
        hosted.dungeon.shooter.latch();
        if let Some(joined) = guest.dungeon.joined.as_mut() {
            joined.seat.tick(Instant::now());
            joined.refresh(std::time::Instant::now());
            let inn = joined.seat.socket_bytes().0;
            still.push(inn.saturating_sub(last));
            last = inn;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    let mut walk_in = crate::together_shooter::Input::default();
    walk_in.move_x = 1;
    let walk_bits = crate::together_seat::bits(&walk_in);
    let mut walking = Vec::new();
    for _ in 0..60 {
        if let Some(seat) = hosted.dungeon.shooter.seat(1) {
            seat.set_held(walk_bits);
            seat.tick(Instant::now());
        }
        hosted.dungeon.shooter.pump(Instant::now());
        hosted.dungeon.shooter.latch();
        if let Some(joined) = guest.dungeon.joined.as_mut() {
            joined.seat.set_held(walk_bits);
            joined.seat.tick(Instant::now());
            joined.refresh(std::time::Instant::now());
            let inn = joined.seat.socket_bytes().0;
            walking.push(inn.saturating_sub(last));
            last = inn;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    let mean = |xs: &[u64]| if xs.is_empty() { 0 } else { xs.iter().sum::<u64>() / xs.len() as u64 };
    let max = |xs: &[u64]| xs.iter().copied().max().unwrap_or(0);
    let dig_burst = after.saturating_sub(before);
    let line = format!(
        "join_snapshot {snapshot} still_mean {} still_max {} walk_mean {} walk_max {} dig_burst {dig_burst}",
        mean(&still),
        max(&still),
        mean(&walking),
        max(&walking)
    );
    eprintln!("{line}");
    if let Ok(path) = std::env::var("FX1_GUEST_BYTES") {
        let _ = std::fs::write(path, line);
    }
}
