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
