//! The host's own wishes with the delve on its own threads, as a hosted
//! game runs it (the user, 2026-11-04: "my wishes arent goign through" while
//! a friend's landed). The older wish tests step the delve by hand; these
//! drive the same keys against the threaded server and look on frame by
//! frame, as the cockpit does.

use super::*;
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

const STARLIGHT: &str = "wish starlight | Starlight Arrows\nsay make my arrows glow like stars | starlight arrows | glowing arrows\ntier homing 1; damage 10 :: Arrows glow and seek.\ntier homing 2; damage 15 :: Arrows burn bright and seek.\ntier homing 2; damage 20 :: Arrows are stars.\n";

/// A distinct learned entry for stress runs: `id` granted for `said`.
fn entry(id: &str, name: &str, said: &str) -> String {
    format!(
        "wish {id} | {name}\nsay {said} | {name}\ntier damage 5 :: {name}, a little.\ntier damage 10 :: {name}, more.\ntier damage 15 :: {name}, most.\n"
    )
}

fn paint(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| crate::ui::draw::ui(frame, app)).unwrap();
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

fn typed(app: &mut App, words: &str) {
    for c in words.chars() {
        press(app, KeyCode::Char(c));
    }
}

fn run(app: &App) -> &crate::delve::together_shooter::Run {
    app.realm.dungeon.shooter.as_ref().unwrap()
}

fn held(app: &App, id: u32) -> u8 {
    let hero = &run(app).players[&id];
    hero.wishes + hero.great_wishes
}

fn has_card(app: &App, id: u32, prefix: &str) -> bool {
    run(app).players.get(&id).is_some_and(|h| h.deck.iter().any(|c| c.starts_with(prefix)))
}

/// One cockpit frame: the server steps itself; the frame latches, claims,
/// teaches and looks for grants, and draws.
fn frame(app: &mut App) {
    app.advance_shooter_at(Instant::now());
    let _ = paint(app, 160, 48);
}

/// Frames at about 60 a second until `done`, or `limit` passes.
fn frames_until(app: &mut App, limit: Duration, done: impl Fn(&App) -> bool) -> bool {
    let start = Instant::now();
    loop {
        frame(app);
        if done(app) {
            return true;
        }
        if start.elapsed() > limit {
            return false;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        frame(app);
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// What the host's wishing looks like now, for a failed assertion.
fn state(app: &App, id: u32) -> String {
    let hero = &run(app).players[&id];
    format!(
        "notice={:?} held={} stone={} offer_open={} asked={} learning={} draft={} lamp_pending={} threaded={}",
        app.realm.dungeon.notice,
        hero.wishes + hero.great_wishes,
        hero.stone,
        app.wish_offer_open(id),
        run(app).asked.len(),
        app.realm.dungeon.forge.learning.len(),
        app.realm.dungeon.forge.draft.is_some(),
        run(app).lamp.pending(id).count(),
        app.realm.dungeon.shooter.threaded(),
    )
}

struct Fixture {
    app: App,
    workspace: std::path::PathBuf,
    _home: crate::tests::TestEnvGuard,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.workspace);
        crate::delve::together_files::refresh_phrasebook(&self.workspace);
    }
}

fn temp(label: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wish-stress-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A hosted delve with the server on its own threads, the party down to
/// floor one and the room won, so knight 1 holds a wish.
fn hosted(label: &str, threaded: bool, oracle: std::sync::Arc<dyn crate::agent::club::Club>) -> Fixture {
    let root = temp(label);
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let home_guard = crate::tests::TestEnvGuard::set("HOME", home.to_str().unwrap());
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools)
        .expect("preview registry should be uniquely owned")
        .set_workspace(workspace.clone());
    app.realm.dungeon.save_path = Some(workspace.join(".angel/dungeon/session.json"));
    app.realm.dungeon.shooter.prefer_thread(threaded);
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    app.terminal_focused = true;
    let _ = paint(&mut app, 160, 48);
    assert_eq!(app.realm.dungeon.shooter.threaded(), threaded, "{}", app.realm.dungeon.notice);
    {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        run.descend_for_test();
        run.calm_for_test();
        run.clear_for_test();
    }
    crate::delve::together_files::refresh_phrasebook(&workspace);
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(oracle));
    frames(&mut app, 4);
    Fixture { app, workspace, _home: home_guard }
}

/// Knight 1 says `words` on the terminal's scroll and picks the genie's first way.
fn host_wishes(app: &mut App, words: &str) {
    let keyboard_before = app.dungeon_keyboard_active();
    app.open_reforge();
    frames(app, 2);
    let after_t = {
        let r = run(app);
        let h = &r.players[&1];
        format!(
            "after T: draft={} input={:?} keyboard_before={keyboard_before} keyboard_now={} view={} hosting={} notice={:?} plate={} ballot_open={} activity_menu={:?} hp={} absent={} room={:?} can_reforge={} wish_line_opens={} intro={} expanded={} controls_visible={} cards_open={}",
            app.realm.dungeon.forge.draft.is_some(),
            app.input,
            app.dungeon_keyboard_active(),
            app.dungeon_view_active(),
            app.realm.dungeon.hosting.is_some(),
            app.realm.dungeon.notice,
            r.floor_plate(Some(1)).is_some(),
            r.playtest.open(1),
            app.realm.dungeon.activity_menu,
            h.hp,
            h.absent,
            r.room().kind,
            r.can_reforge(1),
            r.wish_line_opens(1),
            app.realm.dungeon.intro.is_some(),
            app.realm.dungeon.expanded,
            app.realm.dungeon.controls_visible,
            app.realm.dungeon.cards_open,
        )
    };
    typed(app, words);
    press(app, KeyCode::Enter);
    let opened = frames_until(app, Duration::from_secs(3), |a| a.wish_offer_open(1));
    assert!(opened, "the scroll's choices never opened for {words:?}: {} | {after_t} | input now {:?}", state(app, 1), app.input);
    press(app, KeyCode::Char('1'));
}

fn spoken_wish_is_granted(threaded: bool) {
    let _guard = crate::tests::env_lock();
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], Duration::from_millis(300));
    let mut fx = hosted(if threaded { "spoken-threaded" } else { "spoken-stepped" }, threaded, club.clone());
    let app = &mut fx.app;
    let before = held(app, 1);
    assert!(before > 0, "knight 1 holds a wish: {}", state(app, 1));
    host_wishes(app, "make my arrows glow like stars");
    let granted = frames_until(app, Duration::from_secs(20), |a| has_card(a, 1, "wish-starlight"));
    assert!(granted, "knight 1's spoken wish was never granted: {} asked_model={}", state(app, 1), club.asked.lock().unwrap().len());
    assert_eq!(held(app, 1), before - 1, "one wish spent: {}", state(app, 1));
}

#[test]
fn stress_a_stepped_hosts_spoken_wish_is_granted() {
    spoken_wish_is_granted(false);
}

#[test]
fn stress_a_threaded_hosts_spoken_wish_is_granted() {
    spoken_wish_is_granted(true);
}

#[test]
fn stress_a_threaded_hosts_lamp_wish_is_granted() {
    let _guard = crate::tests::env_lock();
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], Duration::from_millis(300));
    let mut fx = hosted("lamp-threaded", true, club.clone());
    let app = &mut fx.app;
    let before = held(app, 1);
    app.open_lamp_wish();
    frames(app, 2);
    assert!(app.realm.dungeon.forge.draft.is_some(), "the lamp opens the wish line: {}", state(app, 1));
    typed(app, "make my arrows glow like stars");
    press(app, KeyCode::Enter);
    let opened = frames_until(app, Duration::from_secs(3), |a| a.wish_offer_open(1));
    assert!(opened, "the scroll's choices never opened: {}", state(app, 1));
    press(app, KeyCode::Char('1'));
    let granted = frames_until(app, Duration::from_secs(20), |a| has_card(a, 1, "wish-starlight"));
    assert!(granted, "the lamp's wish was never granted: {}", state(app, 1));
    assert_eq!(held(app, 1), before - 1);
}

/// Three new wishes back to back, each written slowly, the knight playing
/// on: all three land and three wishes are spent.
#[test]
fn stress_a_threaded_host_with_three_slow_wishes_in_flight_gets_all_three() {
    let _guard = crate::tests::env_lock();
    let words = ["a cloak of falling leaves", "boots that spark on stone", "a hat that hums at night"];
    let ids = ["leafcloak", "sparkboots", "humhat"];
    let entries: Vec<String> = words
        .iter()
        .zip(ids)
        .map(|(w, id)| entry(id, &format!("The {id}"), w))
        .collect();
    let refs: Vec<&str> = entries.iter().map(String::as_str).collect();
    let club = crate::delve::together_grants::Scripted::new(&refs, Duration::from_secs(2));
    let mut fx = hosted("three-threaded", true, club.clone());
    let app = &mut fx.app;
    {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        run.players.get_mut(&1).unwrap().wishes = 5;
    }
    frames(app, 3);
    let before = held(app, 1);
    for w in words {
        host_wishes(app, w);
        frames(app, 10);
    }
    let all = frames_until(app, Duration::from_secs(40), |a| ids.iter().all(|id| has_card(a, 1, &format!("wish-{id}"))));
    let landed: Vec<_> = ids.iter().filter(|id| has_card(app, 1, &format!("wish-{id}"))).collect();
    assert!(all, "only {landed:?} of {ids:?} landed: {} model_calls={}", state(app, 1), club.asked.lock().unwrap().len());
    assert_eq!(held(app, 1), before - 3, "three wishes spent: {}", state(app, 1));
}

/// The host's terminal loses focus while a friend is seated: the host's
/// knight stands as stone. A wish already chosen still lands.
#[test]
fn stress_a_threaded_hosts_wish_lands_while_its_terminal_is_unfocused() {
    let _guard = crate::tests::env_lock();
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], Duration::from_secs(2));
    let mut fx = hosted("unfocused-threaded", true, club.clone());
    let app = &mut fx.app;
    let before = held(app, 1);
    host_wishes(app, "make my arrows glow like stars");
    frames(app, 3);
    app.terminal_focused = false;
    let granted = frames_until(app, Duration::from_secs(20), |a| has_card(a, 1, "wish-starlight"));
    assert!(granted, "the chosen wish never landed with the terminal unfocused: {}", state(app, 1));
    assert_eq!(held(app, 1), before - 1);
}

/// A friend joins over the link and both knights wish at once.
#[test]
fn stress_a_threaded_host_and_a_joined_friend_both_get_their_wishes() {
    let _guard = crate::tests::env_lock();
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let friend_entry = entry("emberboots", "Ember Boots", "boots of glowing embers");
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT, friend_entry.as_str()], Duration::from_secs(1));
    let mut fx = hosted("friend-threaded", true, club.clone());
    let message = fx.app.dungeon_command(Some("host --1"));
    let link = message
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("http://"))
        .unwrap_or_else(|| panic!("no link in: {message}"))
        .to_string();
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(&format!("join {link}")));
    let start = Instant::now();
    while !fx.app.realm.dungeon.shooter.as_ref().is_some_and(|r| r.players.contains_key(&2)) {
        frame(&mut fx.app);
        friend.advance_joined();
        assert!(start.elapsed() < Duration::from_secs(10), "the friend never took a seat: {}", fx.app.realm.dungeon.notice);
        std::thread::sleep(Duration::from_millis(10));
    }
    // `host` opens the Delve's menu over the game; the host goes back in.
    fx.app.realm.dungeon.intro = None;
    {
        let mut run = fx.app.realm.dungeon.shooter.as_mut().unwrap();
        for id in [1, 2] {
            run.players.get_mut(&id).unwrap().wishes = 2;
        }
    }
    for _ in 0..20 {
        frame(&mut fx.app);
        friend.advance_joined();
        std::thread::sleep(Duration::from_millis(16));
    }
    let host_before = held(&fx.app, 1);
    // The host wishes first; the friend while the host's is being written.
    host_wishes(&mut fx.app, "make my arrows glow like stars");
    press(&mut friend, KeyCode::Char('t'));
    friend.advance_joined();
    typed(&mut friend, "boots of glowing embers");
    press(&mut friend, KeyCode::Enter);
    let mut picked = false;
    let start = Instant::now();
    let both = loop {
        frame(&mut fx.app);
        friend.advance_joined();
        let _ = paint(&mut friend, 160, 48);
        if !picked && friend.wish_offer_open(friend.joined_actor()) {
            press(&mut friend, KeyCode::Char('1'));
            picked = true;
        }
        if has_card(&fx.app, 1, "wish-starlight") && has_card(&fx.app, 2, "wish-emberboots") {
            break true;
        }
        if start.elapsed() > Duration::from_secs(30) {
            break false;
        }
        std::thread::sleep(Duration::from_millis(16));
    };
    assert!(
        both,
        "host granted={} friend granted={} friend_picked={picked} host: {} friend notice={:?} model_calls={}",
        has_card(&fx.app, 1, "wish-starlight"),
        has_card(&fx.app, 2, "wish-emberboots"),
        state(&fx.app, 1),
        friend.realm.dungeon.notice,
        club.asked.lock().unwrap().len(),
    );
    assert_eq!(held(&fx.app, 1), host_before - 1, "{}", state(&fx.app, 1));
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 * p) as usize).min(v.len() - 1)]
}

/// How evenly a joined friend's seat hears the host's ticks, and how long
/// the host's frames and the server's steps take, for `WISH_STRESS_SECS`
/// (20). Appends one JSON line to `WISH_STRESS_OUT` labelled
/// `WISH_STRESS_LABEL`. Run under whatever load the machine is under.
#[test]
#[ignore]
fn stress_seat_cadence() {
    let _guard = crate::tests::env_lock();
    let secs: u64 = std::env::var("WISH_STRESS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(20);
    let label = std::env::var("WISH_STRESS_LABEL").unwrap_or_else(|_| "run".into());
    let timing = temp("cadence-timing").join("ticks.log");
    let _timing = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_FRAME_TIMING", timing.to_str().unwrap());
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], Duration::from_millis(100));
    let mut fx = hosted("cadence", true, club);
    let message = fx.app.dungeon_command(Some("host --1"));
    let link = message
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("http://"))
        .unwrap_or_else(|| panic!("no link in: {message}"))
        .to_string();
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(&format!("join {link}")));
    let start = Instant::now();
    while !fx.app.realm.dungeon.shooter.as_ref().is_some_and(|r| r.players.contains_key(&2)) {
        fx.app.advance_shooter_at(Instant::now());
        friend.advance_joined();
        assert!(start.elapsed() < Duration::from_secs(10), "the friend never took a seat");
        std::thread::sleep(Duration::from_millis(5));
    }
    fx.app.realm.dungeon.intro = None;
    let tick_of = |f: &App| f.realm.dungeon.joined.as_ref().and_then(|j| j.with_latest(|r| r.tick));
    let mut gaps = Vec::new();
    let mut host_frames = Vec::new();
    let mut last: Option<(u64, Instant)> = None;
    let mut ticks_seen = 0u64;
    let mut next_frame = Instant::now();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        let now = Instant::now();
        if now >= next_frame {
            let t = Instant::now();
            fx.app.advance_shooter_at(t);
            host_frames.push(t.elapsed().as_secs_f64() * 1000.0);
            next_frame = now + Duration::from_millis(16);
        }
        friend.advance_joined();
        if let Some(tick) = tick_of(&friend) {
            let seen = Instant::now();
            match last {
                Some((was, at)) if tick != was => {
                    gaps.push(seen.duration_since(at).as_secs_f64() * 1000.0);
                    ticks_seen += tick.saturating_sub(was);
                    last = Some((tick, seen));
                }
                None => last = Some((tick, seen)),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let elapsed = start.elapsed().as_secs_f64();
    let late = gaps.iter().filter(|g| **g > 66.0).count();
    let (mut step_p50, mut step_max, mut lines) = (Vec::new(), 0u64, 0);
    for line in std::fs::read_to_string(&timing).unwrap_or_default().lines() {
        let field = |name: &str| -> Option<u64> {
            let at = line.find(name)? + name.len();
            line[at..].split_whitespace().next()?.parse().ok()
        };
        if line.starts_with("sim ") {
            lines += 1;
            if let Some(p) = field("step_us p50=") {
                step_p50.push(p as f64);
            }
            step_max = step_max.max(field("max=").unwrap_or(0));
        }
    }
    let report = serde_json::json!({
        "label": label,
        "secs": elapsed,
        "updates": gaps.len(),
        "ticks_per_sec": ticks_seen as f64 / elapsed,
        "gap_ms": {"p50": pct(&mut gaps.clone(), 0.5), "p95": pct(&mut gaps.clone(), 0.95), "p99": pct(&mut gaps.clone(), 0.99), "max": pct(&mut gaps.clone(), 1.0)},
        "gaps_over_66ms": late,
        "host_frame_ms": {"p50": pct(&mut host_frames.clone(), 0.5), "p99": pct(&mut host_frames.clone(), 0.99), "max": pct(&mut host_frames.clone(), 1.0)},
        "server_step_us": {"p50_of_windows": pct(&mut step_p50, 0.5), "max": step_max, "windows": lines},
    });
    println!("CADENCE {report}");
    if let Ok(out) = std::env::var("WISH_STRESS_OUT") {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(out).unwrap();
        writeln!(f, "{report}").unwrap();
    }
    assert!(gaps.len() > 10, "the friend heard almost nothing: {report}");
}

/// Knight 1 as the user's knight stood on 2026-11-04 (from the save): five
/// plain wishes, a super wish, a star's wish and a freedom wish held. A
/// new wish said on T's scroll still lands, and one wish is spent.
#[test]
fn stress_a_threaded_host_holding_star_super_and_freedom_wishes_gets_a_new_wish() {
    let _guard = crate::tests::env_lock();
    let club = crate::delve::together_grants::Scripted::new(&[STARLIGHT], Duration::from_millis(500));
    let mut fx = hosted("jordans-knight", true, club.clone());
    let app = &mut fx.app;
    {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        let hero = run.players.get_mut(&1).unwrap();
        hero.wishes = 5;
        hero.great_wishes = 1;
        hero.star_wish = true;
        hero.freedom_wishes = 1;
    }
    frames(app, 4);
    let hero = &run(app).players[&1];
    let before = (hero.wishes, hero.great_wishes, hero.star_wish, hero.freedom_wishes);
    host_wishes(app, "make my arrows glow like stars");
    let granted = frames_until(app, Duration::from_secs(20), |a| has_card(a, 1, "wish-starlight"));
    let hero = &run(app).players[&1];
    let after = (hero.wishes, hero.great_wishes, hero.star_wish, hero.freedom_wishes);
    assert!(granted, "the wish never landed: {} held before={before:?} after={after:?}", state(app, 1));
    assert_ne!(before, after, "something was spent on it: {}", state(app, 1));
}

/// The seat cadence on a heavy delve: the night's save and phrasebook
/// (`WISH_STRESS_SAVE`, `WISH_STRESS_BOOK`), the party down a floor and
/// `WISH_STRESS_ENEMIES` (24) skeletons in the room, with the rules worker
/// when `ANGEL_DELVE_SYSTEM` names one. Same report as `stress_seat_cadence`.
#[test]
#[ignore]
fn stress_seat_cadence_heavy() {
    let _guard = crate::tests::env_lock();
    let secs: u64 = std::env::var("WISH_STRESS_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(20);
    let enemies: usize = std::env::var("WISH_STRESS_ENEMIES").ok().and_then(|s| s.parse().ok()).unwrap_or(24);
    let label = std::env::var("WISH_STRESS_LABEL").unwrap_or_else(|_| "heavy".into());
    let root = temp("cadence-heavy");
    let home = root.join("home");
    let workspace = root.join("workspace");
    let dungeon = workspace.join(".angel/dungeon");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&dungeon).unwrap();
    if let Ok(save) = std::env::var("WISH_STRESS_SAVE") {
        std::fs::copy(save, dungeon.join("session.json")).unwrap();
    }
    if let Ok(book) = std::env::var("WISH_STRESS_BOOK") {
        std::fs::copy(book, dungeon.join("phrasebook.txt")).unwrap();
    }
    let _home = crate::tests::TestEnvGuard::set("HOME", home.to_str().unwrap());
    let timing = root.join("ticks.log");
    let _timing = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_FRAME_TIMING", timing.to_str().unwrap());
    let _public = crate::tests::TestEnvGuard::unset("ANGEL_DUNGEON_PUBLIC_URL");
    let _listen = crate::tests::TestEnvGuard::set("ANGEL_DUNGEON_LISTEN", "127.0.0.1:0");
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools).unwrap().set_workspace(workspace.clone());
    app.realm.dungeon.shooter.prefer_thread(true);
    crate::delve::together_files::refresh_phrasebook(&workspace);
    let restored = app.realm.dungeon.restore(&workspace);
    if app.realm.dungeon.shooter.is_none() {
        app.input = "/dungeon start Test Guest".into();
        app.submit();
    }
    app.terminal_focused = true;
    let _ = paint(&mut app, 160, 48);
    assert!(app.realm.dungeon.shooter.threaded(), "restored={restored:?} {}", app.realm.dungeon.notice);
    let message = app.dungeon_command(Some("host --1"));
    let link = message
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("http://"))
        .unwrap_or_else(|| panic!("no link in: {message}"))
        .to_string();
    let mut friend = App::preview(crate::ui::viewer::Viewer::static_preview());
    friend.terminal_focused = true;
    friend.dungeon_command(Some(&format!("join {link}")));
    let start = Instant::now();
    while !app.realm.dungeon.shooter.as_ref().is_some_and(|r| r.players.contains_key(&2)) {
        app.advance_shooter_at(Instant::now());
        friend.advance_joined();
        assert!(start.elapsed() < Duration::from_secs(20), "the friend never took a seat");
        std::thread::sleep(Duration::from_millis(5));
    }
    app.realm.dungeon.intro = None;
    let (book_cards, run_bytes) = {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        if run.dungeon.depth == 0 {
            run.descend_for_test();
        }
        for i in 0..enemies {
            run.spawn_at_for_test(
                angel_delve::rules::EnemyKind::Skeleton,
                6.0 + (i % 8) as f32 * 2.5,
                5.0 + (i / 8) as f32 * 2.5,
            );
        }
        (run.book.cards.len(), serde_json::to_vec(&*run).map(|v| v.len()).unwrap_or(0))
    };
    let tick_of = |f: &App| f.realm.dungeon.joined.as_ref().and_then(|j| j.with_latest(|r| r.tick));
    let (mut gaps, mut host_frames) = (Vec::new(), Vec::new());
    let mut last: Option<(u64, Instant)> = None;
    let mut ticks_seen = 0u64;
    let mut next_frame = Instant::now();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        let now = Instant::now();
        if now >= next_frame {
            let t = Instant::now();
            app.advance_shooter_at(t);
            host_frames.push(t.elapsed().as_secs_f64() * 1000.0);
            next_frame = now + Duration::from_millis(16);
        }
        friend.advance_joined();
        if let Some(tick) = tick_of(&friend) {
            let seen = Instant::now();
            match last {
                Some((was, at)) if tick != was => {
                    gaps.push(seen.duration_since(at).as_secs_f64() * 1000.0);
                    ticks_seen += tick.saturating_sub(was);
                    last = Some((tick, seen));
                }
                None => last = Some((tick, seen)),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let elapsed = start.elapsed().as_secs_f64();
    let late = gaps.iter().filter(|g| **g > 66.0).count();
    let (mut step_p50, mut pub_p50, mut step_max, mut windows) = (Vec::new(), Vec::new(), 0u64, 0);
    for line in std::fs::read_to_string(&timing).unwrap_or_default().lines() {
        if !line.starts_with("sim ") {
            continue;
        }
        windows += 1;
        let nums: Vec<u64> = line
            .split_whitespace()
            .filter_map(|w| w.split_once('=').and_then(|(_, v)| v.parse().ok()))
            .collect();
        // sim n=.. step_us p50=.. max=.. publish_us p50=.. max=..
        if nums.len() >= 5 {
            step_p50.push(nums[1] as f64);
            step_max = step_max.max(nums[2]);
            pub_p50.push(nums[3] as f64);
        }
    }
    let alive = app.realm.dungeon.shooter.as_ref().map_or(0, |r| r.enemies.len());
    let report = serde_json::json!({
        "label": label,
        "rules_worker": std::env::var("ANGEL_DELVE_SYSTEM").is_ok(),
        "book_cards": book_cards,
        "run_json_bytes": run_bytes,
        "enemies_left": alive,
        "secs": elapsed,
        "updates": gaps.len(),
        "ticks_per_sec": ticks_seen as f64 / elapsed,
        "gap_ms": {"p50": pct(&mut gaps.clone(), 0.5), "p95": pct(&mut gaps.clone(), 0.95), "p99": pct(&mut gaps.clone(), 0.99), "max": pct(&mut gaps.clone(), 1.0)},
        "gaps_over_66ms": late,
        "host_frame_ms": {"p50": pct(&mut host_frames.clone(), 0.5), "p99": pct(&mut host_frames.clone(), 0.99), "max": pct(&mut host_frames.clone(), 1.0)},
        "server_step_us": {"p50": pct(&mut step_p50, 0.5), "max": step_max, "windows": windows},
        "publish_us_p50": pct(&mut pub_p50, 0.5),
    });
    println!("CADENCE {report}");
    if let Ok(out) = std::env::var("WISH_STRESS_OUT") {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(out).unwrap();
        writeln!(f, "{report}").unwrap();
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(gaps.len() > 10, "the friend heard almost nothing: {report}");
}

/// The model in hand, every reply kept: how long it took, what it was asked
/// (a mend's brief carries the checker's errors) and what came back.
struct Recording {
    inner: std::sync::Arc<dyn crate::agent::club::Club>,
    calls: std::sync::Mutex<Vec<(f64, String, Result<String, String>)>>,
}

impl crate::agent::club::Club for Recording {
    fn respond(&self, prompt: &str) -> Result<String, String> {
        let t = Instant::now();
        let reply = self.inner.respond(prompt);
        self.calls.lock().unwrap().push((t.elapsed().as_secs_f64(), prompt.to_string(), reply.clone()));
        reply
    }
    fn label(&self) -> &str {
        self.inner.label()
    }
    fn reasoning_effort(&self) -> Option<String> {
        self.inner.reasoning_effort()
    }
    fn reasoning_levels(&self) -> &[String] {
        self.inner.reasoning_levels()
    }
    fn checked_launch_effort(&self, requested: &str) -> Result<String, String> {
        self.inner.checked_launch_effort(requested)
    }
    fn chat_with_effort(
        &self,
        messages: &[crate::agent::club::ChatMsg],
        tools: &[crate::agent::club::ToolDef],
        effort: Option<&str>,
    ) -> Result<crate::agent::club::ClubReply, String> {
        let t = Instant::now();
        let reply = self.inner.chat_with_effort(messages, tools, effort);
        let prompt = messages.last().map(|m| m.content.to_string()).unwrap_or_default();
        let text = match &reply {
            Ok(crate::agent::club::ClubReply::Text(text)) => Ok(text.clone()),
            Ok(_) => Err("tool calls".to_string()),
            Err(e) => Err(e.clone()),
        };
        self.calls.lock().unwrap().push((t.elapsed().as_secs_f64(), format!("[effort {effort:?}]\n{prompt}"), text));
        reply
    }
}

const NIGHT_WISHES: [&str; 6] = [
    "my bullets to fork",
    "freeze and slow",
    "rotten fish stench",
    "bullets hit twice",
    "razorblade gauntlets that bleed enemies",
    "a cape that farts fire",
];

/// The host's real model writing `WISH_STRESS_WORDS` (';'-separated, default
/// six of the night's kinds of wishes) all at once, one knight each, as a
/// party wishes: per wish, how long until usable, the model's first answer
/// and its mend, what was granted or why it was handed back, and the
/// checker's errors on every first answer. `ANGEL_DRIVER` and its model and
/// effort pick the model. Appends a JSON line to `WISH_STRESS_OUT`.
#[test]
#[ignore]
fn stress_real_model_wishes() {
    let _guard = crate::tests::env_lock();
    let words: Vec<String> = std::env::var("WISH_STRESS_WORDS")
        .map(|w| w.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_else(|_| NIGHT_WISHES.iter().map(|s| s.to_string()).collect());
    let label = std::env::var("WISH_STRESS_LABEL").unwrap_or_else(|_| "real".into());
    let real_home = std::env::var("HOME").unwrap();
    let root = temp("real-model");
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    // The model's sign-in and settings, nothing else of the real home.
    std::os::unix::fs::symlink(format!("{real_home}/.codex"), home.join(".codex")).unwrap();
    let _home = crate::tests::TestEnvGuard::set("HOME", home.to_str().unwrap());
    let club = std::sync::Arc::new(Recording {
        inner: crate::agent::club::Bag::standard().in_hand(),
        calls: std::sync::Mutex::new(Vec::new()),
    });
    let mut app = App::preview(crate::ui::viewer::Viewer::static_preview());
    std::sync::Arc::get_mut(&mut app.tools).unwrap().set_workspace(workspace.clone());
    app.realm.dungeon.save_path = Some(workspace.join(".angel/dungeon/session.json"));
    app.input = "/dungeon start Test Guest".into();
    app.submit();
    app.terminal_focused = true;
    {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        run.descend_for_test();
        run.calm_for_test();
        run.clear_for_test();
    }
    crate::delve::together_files::refresh_phrasebook(&workspace);
    app.realm.dungeon.forge.grants.oracle = Some(crate::app::control::realm::club_oracle(club.clone()));
    let n = words.len();
    let mut asked = Vec::new();
    {
        let mut run = app.realm.dungeon.shooter.as_mut().unwrap();
        for k in 1..=n as u32 {
            if !run.players.contains_key(&k) {
                run.join_seat(k, &format!("Knight {k}"));
            }
            run.players.get_mut(&k).unwrap().wishes = 3;
            run.spend_wish(k);
            asked.push(crate::delve::together_shooter::genie::Asked {
                cosmetic: false,
                cosmetic_name: String::new(),
                fairy: false,
                star: false,
                knight: k,
                words: words[k as usize - 1].clone(),
                steps: 1,
                great: false,
                fills: true,
                making: None,
                target: Default::default(),
            });
        }
    }
    let decks: Vec<usize> = (1..=n as u32).map(|k| run(&app).players[&k].deck.len()).collect();
    let start = Instant::now();
    app.realm.dungeon.shooter.as_mut().unwrap().asked.extend(asked);
    app.teach_asked();
    let mut landed: Vec<Option<f64>> = vec![None; n];
    while !app.realm.dungeon.forge.learning.is_empty() && start.elapsed() < Duration::from_secs(420) {
        app.realm.dungeon.forge.looked = None;
        app.watch_learning();
        for k in 1..=n {
            if landed[k - 1].is_none() && run(&app).players[&(k as u32)].deck.len() > decks[k - 1] {
                landed[k - 1] = Some(start.elapsed().as_secs_f64());
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let calls = club.calls.lock().unwrap().clone();
    let turned_away: Vec<serde_json::Value> = calls
        .iter()
        .filter_map(|(secs, prompt, _)| {
            let (_, tail) = prompt.split_once("THE GAME'S CHECKER TURNED IT AWAY:\n")?;
            let errors: Vec<&str> = tail.lines().take_while(|l| l.starts_with("- ")).collect();
            let said = words.iter().find(|w| prompt.contains(w.as_str())).cloned().unwrap_or_default();
            Some(serde_json::json!({"words": said, "mend_call_s": secs, "errors": errors}))
        })
        .collect();
    let wishes: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            let back = app.realm.dungeon.forge.landings.iter().find(|l| l.player as usize == i + 1);
            let timing = back.and_then(|l| l.timing);
            serde_json::json!({
                "words": words[i],
                "usable_s": landed[i],
                "model_s": timing.map(|t| t.model.as_secs_f32()),
                "mend_s": timing.and_then(|t| t.mend.map(|m| m.as_secs_f32())),
                "granted": back.and_then(|l| l.granted.clone()),
                "failed": back.and_then(|l| l.failed.clone()),
            })
        })
        .collect();
    let granted = wishes.iter().filter(|w| !w["granted"].is_null()).count();
    let report = serde_json::json!({
        "label": label,
        "route": club.inner.label(),
        "effort": std::env::var("ANGEL_OPENAI_REASONING_EFFORT").ok(),
        "wishes": n,
        "granted": granted,
        "mended": wishes.iter().filter(|w| !w["mend_s"].is_null()).count(),
        "calls": calls.len(),
        "call_s": calls.iter().map(|c| (c.0 * 10.0).round() / 10.0).collect::<Vec<_>>(),
        "last_usable_s": landed.iter().flatten().cloned().fold(0.0, f64::max),
        "per_wish": wishes,
        "turned_away": turned_away,
        "notice": app.realm.dungeon.notice,
    });
    println!("REAL {report}");
    if let Ok(out) = std::env::var("WISH_STRESS_OUT") {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(out).unwrap();
        writeln!(f, "{report}").unwrap();
    }
    let _ = std::fs::remove_dir_all(&root);
}
