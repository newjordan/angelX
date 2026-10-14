use super::*;
use crate::drive::together_shooter::RoomKind;
use angel_net::input::button;

fn fight() -> Run {
    let mut run = Run::new(3, 9, Some("Two"));
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight)
        .unwrap();
    run.enter_for_test(fight);
    run.calm_for_test();
    run
}

fn playing() -> Controls {
    Controls {
        active: true,
        keyboard: true,
        couch: true,
        ..Default::default()
    }
}

/// One terminal frame stepped by hand: the host's seat samples its keys,
/// then the server steps what is due.
fn frame(sim: &mut Sim, now: Instant) -> u32 {
    if let Some(seat) = sim.seat(1) {
        seat.tick(now);
    }
    sim.pump(now)
}

#[test]
fn the_server_steps_thirty_times_a_second_on_its_own_beat_and_nothing_while_paused() {
    let mut sim = Sim::from(Some(fight()));
    sim.set_controls(playing());
    let start = Instant::now();
    let tick = sim.as_ref().unwrap().tick;
    assert_eq!(sim.pump(start), 0, "the beat begins");
    assert_eq!(sim.pump(start + STEP * 3), 3);
    // Part of a step waits for the next pump.
    assert_eq!(sim.pump(start + STEP * 4 + STEP / 2), 1);
    assert_eq!(sim.pump(start + STEP * 5), 1);
    sim.latch();
    assert_eq!(sim.as_ref().unwrap().tick, tick + 5);
    // Paused, the beat goes on and the run stands.
    sim.set_controls(Controls::default());
    sim.pump(start + STEP * 8);
    sim.latch();
    assert_eq!(sim.as_ref().unwrap().tick, tick + 5);
}

#[test]
fn a_stall_starts_the_beat_over_rather_than_racing_to_catch_up() {
    let mut sim = Sim::from(Some(fight()));
    sim.set_controls(playing());
    let start = Instant::now();
    sim.pump(start);
    assert_eq!(sim.pump(start + Duration::from_secs(2)), 1);
    assert_eq!(sim.telemetry().ticks, 1);
    assert_eq!(sim.clock(), Some(start + Duration::from_secs(2)));
}

#[test]
fn the_hosts_own_knight_walks_on_the_keys_its_seat_samples_and_stops_when_let_go() {
    let mut sim = Sim::from(Some(fight()));
    sim.set_controls(playing());
    let seat = sim.seat(1).expect("the host's own seat").clone();
    assert_eq!(seat.knight(), Some(1), "the host's seat plays knight 1");
    let x = sim.as_ref().unwrap().players[&1].x;
    let start = Instant::now();
    seat.set_held(button::RIGHT);
    frame(&mut sim, start);
    for n in 1..=6 {
        frame(&mut sim, start + STEP * n);
    }
    sim.latch();
    let walked = sim.as_ref().unwrap().players[&1].x;
    assert!(
        walked > x,
        "the knight walks on its held key: {x} -> {walked}"
    );
    seat.set_held(0);
    frame(&mut sim, start + STEP * 7);
    frame(&mut sim, start + STEP * 8);
    sim.latch();
    let stopped = sim.as_ref().unwrap().players[&1].x;
    frame(&mut sim, start + STEP * 9);
    frame(&mut sim, start + STEP * 10);
    sim.latch();
    assert_eq!(sim.as_ref().unwrap().players[&1].x, stopped);
    // A tap shorter than a frame (held, then let go, before the frame) is
    // still a step.
    seat.set_held(button::LEFT);
    seat.set_held(0);
    frame(&mut sim, start + STEP * 11);
    frame(&mut sim, start + STEP * 12);
    sim.latch();
    assert!(
        sim.as_ref().unwrap().players[&1].x < stopped,
        "the tap walked"
    );
}

#[test]
fn an_edit_is_seen_by_the_next_read_without_a_copy_until_then() {
    let mut sim = Sim::from(Some(fight()));
    let first = Arc::clone(sim.snapshot().unwrap());
    if let Some(mut run) = sim.as_mut() {
        run.players.get_mut(&1).unwrap().x += 5.0;
    }
    let edited = Arc::clone(sim.snapshot().unwrap());
    assert!(!Arc::ptr_eq(&first, &edited));
    assert_eq!(edited.run.players[&1].x, first.run.players[&1].x + 5.0);
    // A read without an edit keeps the latched snapshot: no copy.
    assert!(Arc::ptr_eq(&edited, sim.snapshot().unwrap()));
}

#[test]
fn the_runs_sounds_reach_the_seats_and_its_moments_reach_the_hosts_chorus() {
    let mut sim = Sim::from(Some(fight()));
    sim.set_controls(playing());
    let start = Instant::now();
    frame(&mut sim, start);
    if let Some(mut run) = sim.as_mut() {
        run.sounds.push("gold");
        run.cues.push("npc".into());
    }
    let seat = sim.seat(1).unwrap().clone();
    let mut lines = Vec::new();
    let until = Instant::now() + Duration::from_secs(5);
    let mut n = 1;
    while !lines.iter().any(|l: &String| l == "sfx gold") && Instant::now() < until {
        frame(&mut sim, start + STEP * n);
        n += 1;
        std::thread::sleep(Duration::from_millis(2));
        seat.tick(start + STEP * n);
        lines.extend(seat.take_lines());
    }
    assert!(lines.iter().any(|l| l == "sfx gold"), "{lines:?}");
    assert!(sim.drain_news().contains(&News::Cue("npc".into())));
    // The chorus's line goes to every seat.
    sim.say(angel_delve::wire::Event::Say {
        hear: angel_delve::wire::Hear::All,
        who: "blaise".into(),
        name: "Old Blaise".into(),
        words: "Mind the stair.".into(),
    });
    let until = Instant::now() + Duration::from_secs(5);
    let mut said = Vec::new();
    while said.is_empty() && Instant::now() < until {
        frame(&mut sim, start + STEP * n);
        n += 1;
        std::thread::sleep(Duration::from_millis(2));
        seat.tick(start + STEP * n);
        said.extend(
            seat.take_lines()
                .into_iter()
                .filter(|l| l.starts_with("say ")),
        );
    }
    assert_eq!(said, vec!["say Old Blaise: Mind the stair.".to_string()]);
}

#[test]
fn a_new_delve_takes_the_old_ones_place_on_the_same_server_with_the_same_seats() {
    let mut sim = Sim::from(Some(fight()));
    let addr = sim.server().unwrap().addr();
    let seat = sim.seat(1).unwrap().clone();
    let mut next = Run::new(5, 10, None);
    next.calm_for_test();
    sim.set(Some(next));
    assert_eq!(sim.as_ref().unwrap().raid_id, 10);
    assert_eq!(sim.server().unwrap().addr(), addr);
    assert_eq!(sim.seat(1).unwrap().knight(), seat.knight());
}

#[test]
fn hosting_moves_the_run_onto_a_door_friends_reach_and_unhosting_moves_it_back() {
    let mut sim = Sim::from(Some(fight()));
    let solo = sim.server().unwrap().addr();
    assert!(solo.ip().is_loopback() && !sim.hosted());
    let x = sim.as_ref().unwrap().players[&1].x;
    sim.host(Door {
        bind: "127.0.0.1:0".parse().unwrap(),
        seats: 2,
        key: Some("ROUNDTABLEKEY".into()),
        bundle: None,
    })
    .unwrap();
    assert!(sim.hosted());
    assert_ne!(sim.server().unwrap().addr(), solo);
    let (door, key) = sim.invitation().unwrap();
    assert_eq!(key, "ROUNDTABLEKEY");
    assert_eq!(door, sim.server().unwrap().addr().to_string());
    // The same run, and the host's own seat on the new server.
    assert_eq!(sim.as_ref().unwrap().players[&1].x, x);
    assert_eq!(sim.seat(1).unwrap().knight(), Some(1));
    // A door that cannot be opened leaves the delve where it was.
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let busy = Door {
        bind: taken.local_addr().unwrap(),
        seats: 2,
        key: None,
        bundle: None,
    };
    assert!(sim.host(busy).is_err());
    assert_eq!(sim.invitation().unwrap().1, "ROUNDTABLEKEY");
    sim.unhost().unwrap();
    assert!(!sim.hosted() && sim.invitation().is_none());
    assert!(sim.server().unwrap().addr().ip().is_loopback());
}

#[test]
fn the_server_steps_on_its_own_thread_and_the_checkpoint_is_written_off_it() {
    let dir = std::env::temp_dir().join(format!("angel-sim-thread-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let save = dir.join("session.json");
    let mut sim = Sim::spawn(fight());
    assert!(sim.threaded());
    let tick = sim.as_ref().unwrap().tick;
    let x = sim.as_ref().unwrap().players[&1].x;
    let seat = sim.seat(1).unwrap().clone();
    seat.set_held(button::RIGHT);
    let start = Instant::now();
    sim.set_controls(Controls {
        save: Some((save.clone(), 9)),
        ..playing()
    });
    // The terminal samples its seat's keys; the server keeps its own time.
    while start.elapsed() < Duration::from_millis(300) {
        seat.tick(Instant::now());
        std::thread::sleep(Duration::from_millis(16));
    }
    // A pump by the terminal does nothing: the thread owns the clock.
    assert_eq!(sim.pump(Instant::now()), 0);
    sim.latch();
    let elapsed = start.elapsed().as_secs_f32();
    let run = sim.as_ref().unwrap();
    let ticks = run.tick - tick;
    assert!(
        ticks as f32 >= elapsed * 30.0 * 0.5 && ticks as f32 <= elapsed * 30.0 + 2.0,
        "about 30 a second: {ticks} in {elapsed:.2} s"
    );
    assert!(run.players[&1].x > x);
    // Edits from the terminal go in between ticks.
    if let Some(mut run) = sim.as_mut() {
        run.players.get_mut(&2).unwrap().hp = 1;
    }
    assert_eq!(sim.as_ref().unwrap().players[&2].hp, 1);
    // The checkpoint came from a published tick, written by the service
    // thread: this thread never wrote it.
    let until = Instant::now() + Duration::from_secs(5);
    while !save.exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&save).unwrap()).unwrap();
    assert_eq!(saved["raid_serial"], 9);
    assert!(saved["run"]["tick"].as_u64().unwrap() > tick);
    drop(sim);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_knob_the_host_sets_reaches_the_run_survives_a_new_door_and_is_kept_in_the_tune_file() {
    let dir = std::env::temp_dir().join(format!("angel-delve-tune-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("tune.json");
    let mut sim = Sim::default();
    sim.prefer_thread(false);
    sim.set_tune_file(file.clone());
    sim.set(Some(fight()));
    sim.set_controls(playing());
    let names: Vec<_> = sim.knobs().iter().map(|k| k.name).collect();
    assert!(names.starts_with(&["snapshot_every", "hero_speed"]), "{names:?}");
    assert_eq!(sim.tune("hero_speed", 19.7), Ok(19.5), "on its step");
    assert!(sim.tune("nope", 1.0).is_err());
    let start = Instant::now();
    sim.pump(start);
    sim.pump(start + STEP * 2);
    sim.latch();
    assert_eq!(sim.as_ref().unwrap().tune.hero_speed, 19.5, "the run plays at it");
    // A new door keeps it, and the file has it for the next start.
    sim.host(Door { bind: "127.0.0.1:0".parse().unwrap(), seats: 2, key: None, bundle: None }).unwrap();
    let pace = sim.knobs().into_iter().find(|k| k.name == "hero_speed").unwrap();
    assert_eq!((pace.value, pace.default), (19.5, 13.0));
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !std::fs::read_to_string(&file).unwrap_or_default().contains("\"hero_speed\": 19.5") && Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(std::fs::read_to_string(&file).unwrap().contains("\"hero_speed\": 19.5"));
    // A new delve in this workspace starts from the file.
    let mut again = Sim::default();
    again.prefer_thread(false);
    again.set_tune_file(file.clone());
    again.set(Some(fight()));
    let pace = again.knobs().into_iter().find(|k| k.name == "hero_speed").unwrap();
    assert_eq!(pace.value, 19.5);
    let _ = std::fs::remove_dir_all(&dir);
}
