use super::*;
use crate::together_shooter::RoomKind;
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

#[test]
#[ignore = "requires the two native system binaries built with swap-test"]
fn native_brew_policy_swap_keeps_pending_jobs_seats_cooldowns_and_cached_deliveries() {
    use angel_delve::rules::{brew_jobs, cauldron, cards};
    let root = std::env::temp_dir().join(format!(
        "native-brew-port-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_WORKER").unwrap(),
        &artifact,
    )
    .unwrap();
    let mut sim = Sim::from(Some(fight()));
    let seat = sim.seat(1).unwrap() as *const Seat;
    {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core).engine = Some(
            angel_live::system::System::watch_with_contracts(
                "delve",
                artifact.clone(),
                Duration::from_secs(2),
                BTreeMap::from([(brew_jobs::CONTRACT.into(), brew_jobs::API)]),
            )
            .unwrap(),
        );
    }
    let mut claims = Vec::new();
    for player in [1, 2] {
        {
            let mut run = sim.as_mut().unwrap();
            let placeholder = cauldron::brewed_card(
                cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
                cauldron::Tier::Common,
            );
            run.players.get_mut(&player).unwrap().spells[0] = Some(placeholder.id.clone());
            run.players.get_mut(&player).unwrap().spell_cooldowns[0] = 71;
            let ask = cauldron::Brewing {
                knight: player,
                raid: run.raid_id,
                depth: run.dungeon.depth,
                cards: ["cabbage-storm".into(), "hedge-maze".into()],
                placeholder: placeholder.id.clone(),
            };
            run.book.insert(placeholder);
            run.brewing.push(ask);
        }
        claims.push(sim.claim_brew().unwrap().unwrap());
    }
    let activate = |sim: &mut Sim, count| {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let live = sim.live.as_ref().unwrap();
            let mut core = lock(&live.shared().core);
            step_system(&mut core, &BTreeMap::new(), false);
            if core.engine.as_ref().unwrap().status().swaps == count {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", core.engine_error);
            drop(core);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    activate(&mut sim, 1);
    let raw = "name Frostbriar Orchard\nkind spell\nrarity rare\ntext Thorns and a cold wind.\ndrop 0\nchest 0\ncooldown 20\ncabbages 3\nhedge 1\nward 1\nart\n..fDDf..\n.fDEEDf.\nfDE44EDf\nfDE44EDf\n.fDEEDf.\n..fDDf..\n";
    let inputs: Vec<_> = claims
        .iter()
        .enumerate()
        .map(|(index, claim)| {
            let answer = if index == 1 {
                raw.replace("Frostbriar Orchard", "Moonberry Orchard").replace("hedge 1", "heal 5")
            } else { raw.to_owned() };
            cards::check(&claim.work.result_id, &answer).unwrap().card
        })
        .collect();
    let done = sim
        .answer_brew(&claims[0], inputs[0].clone())
        .unwrap()
        .unwrap();
    assert_eq!(done.result.as_deref(), Ok("Frostbriar Orchard"));
    let staged = artifact.with_extension("pending");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT").unwrap(),
        &staged,
    )
    .unwrap();
    let edited = Instant::now();
    std::fs::rename(staged, &artifact).unwrap();
    activate(&mut sim, 2);
    eprintln!(
        "native Sim brew activation={:?}; host_pid={}",
        edited.elapsed(),
        std::process::id()
    );
    assert_eq!(
        sim.answer_brew(&claims[0], inputs[0].clone())
            .unwrap()
            .unwrap(),
        done
    );
    assert_eq!(
        sim.answer_brew(&claims[1], inputs[1].clone())
            .unwrap()
            .unwrap()
            .result
            .as_deref(),
        Ok("Worker Orchard")
    );
    assert_eq!(seat, sim.seat(1).unwrap() as *const Seat);
    let mut cursor = None;
    assert_eq!(sim.new_brew_claims(&mut cursor).unwrap().len(), 2);
    assert!(sim.new_brew_claims(&mut cursor).unwrap().is_empty());
    {
        let live = sim.live.as_ref().unwrap();
        let core = lock(&live.shared().core);
        assert_eq!(core.run.players.len(), 2);
        assert_eq!(core.run.players[&2].spell_cooldowns[0], 71);
        assert_eq!(
            core.run
                .sounds
                .iter()
                .filter(|s| **s == "wish_grant")
                .count(),
            2
        );
    }
    drop(sim);
    std::fs::remove_dir_all(root).unwrap();
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
fn live_rules_change_actual_movement_without_replacing_the_run_or_its_seat() {
    let mut sim = Sim::from(Some(fight()));
    sim.set_controls(playing());
    let seat = sim.seat(1).unwrap().clone();
    let seat_address = sim.seat(1).unwrap() as *const Seat;
    let raid = sim.as_ref().unwrap().raid_id;
    let policy = |speed| {
        Arc::new(GamePolicy {
            knobs: BTreeMap::from([("hero_speed".into(), speed)]),
        })
    };
    sim.set_live_rules(policy(4.0));
    seat.set_held(button::RIGHT);
    let start = Instant::now();
    let x = sim.as_ref().unwrap().players[&1].x;
    frame(&mut sim, start);
    for n in 1..=3 {
        frame(&mut sim, start + STEP * n);
    }
    sim.latch();
    let slow_x = sim.as_ref().unwrap().players[&1].x;
    sim.set_live_rules(policy(30.0));
    for n in 4..=6 {
        frame(&mut sim, start + STEP * n);
    }
    sim.latch();
    let run = sim.as_ref().unwrap();
    assert_eq!(run.raid_id, raid);
    assert_eq!(seat_address, sim.seat(1).unwrap() as *const Seat);
    assert_eq!(run.tune.hero_speed, 30.0);
    assert!(slow_x > x);
    assert!(run.players[&1].x - slow_x > (slow_x - x) * 2.0);
    sim.set_live_rules(Arc::default());
    frame(&mut sim, start + STEP * 7);
    sim.latch();
    assert_eq!(
        sim.as_ref().unwrap().tune.hero_speed,
        Tune::default().hero_speed
    );
}

#[test]
fn game_overlay_tracks_base_changes_and_removal_without_rewriting_saved_tuning() {
    let mut overlay = LiveTune::default();
    let mut base = Tune::default();
    base.hero_speed = 10.0;
    overlay.replace(Arc::new(GamePolicy {
        knobs: BTreeMap::from([("hero_speed".into(), 20.0), ("fire_rate".into(), 200.0)]),
    }));
    assert_eq!(overlay.effective(base).hero_speed, 20.0);
    assert_eq!(overlay.effective(base).cooldown(10), 5);
    base.hero_speed = 12.0;
    base.foe_hit = 150;
    assert_eq!(overlay.effective(base).foe_hit, 150);
    overlay.replace(Arc::default());
    assert_eq!(overlay.effective(base), base);
    assert_eq!(base.hero_speed, 12.0);
    assert!(
        validate_live_rules(&GamePolicy {
            knobs: BTreeMap::from([("hero_speed".into(), 999.0)])
        })
        .is_err()
    );
    assert!(
        validate_live_rules(&GamePolicy {
            knobs: BTreeMap::from([("not_a_rule".into(), 1.0)])
        })
        .is_err()
    );
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
    let seat = sim.seat(1).unwrap().clone();
    // Let the client's server-clock sample window turn over after startup.
    // An initial stall can otherwise make fresh sounds look stale for 90 ticks.
    let until = Instant::now() + Duration::from_secs(15);
    let mut ticks = 0;
    while (seat.run().is_none() || ticks < 100) && Instant::now() < until {
        ticks += frame(&mut sim, Instant::now());
        std::thread::sleep(Duration::from_millis(2));
        seat.tick(Instant::now());
    }
    assert!(seat.run().is_some(), "the seat must subscribe before a sound is emitted");
    assert!(ticks >= 100, "the server-clock sample window must warm up");
    if let Some(mut run) = sim.as_mut() {
        run.sounds.push("gold");
        run.cues.push("npc".into());
    }
    let mut lines = Vec::new();
    let until = Instant::now() + Duration::from_secs(5);
    while !lines.iter().any(|l: &String| l == "sfx gold") && Instant::now() < until {
        frame(&mut sim, Instant::now());
        std::thread::sleep(Duration::from_millis(2));
        seat.tick(Instant::now());
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
        frame(&mut sim, Instant::now());
        std::thread::sleep(Duration::from_millis(2));
        seat.tick(Instant::now());
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
    let run = fight();
    // Client graphics admission is outside this server-clock measurement.
    angel_delve::render::overworld::arena::slot_art::warm_all();
    angel_delve::render::overworld::arena::prepare_entry(&run, Some(1));
    let mut sim = Sim::spawn(run);
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
    assert!(run.players[&1].x > x, "host x {x} -> {}, ticks {ticks}; client {:?}",
        run.players[&1].x, seat.run().map(|r| (r.tick, r.players[&1].x)));
    // Edits from the terminal go in between ticks.
    if let Some(mut run) = sim.as_mut() {
        run.players.get_mut(&2).unwrap().hp = 1;
    }
    assert_eq!(sim.as_ref().unwrap().players[&2].hp, 1);
    // The checkpoint came from a published tick, written by the service
    // thread: this thread never wrote it.
    let until = Instant::now() + Duration::from_secs(5);
    let checkpoint_advanced = || {
        std::fs::read(&save)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|saved| saved["run"]["tick"].as_u64())
            .is_some_and(|saved_tick| saved_tick > tick)
    };
    while !checkpoint_advanced() && Instant::now() < until {
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

/// The service reads the campaign's board off the sim thread and the run
/// takes its counts, the realm's heart questline with them; a board that
/// goes missing takes them back to zeros, and nothing in the run can raise
/// them on its own.
#[test]
fn the_service_brings_the_campaigns_board_into_the_run() {
    let dir = std::env::temp_dir().join(format!("angel-vec-sim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let board = dir.join("board.json");
    std::fs::write(
        &board,
        r#"{"schema":"vec.board/1","quest":{"vec_data_staged":1,"vec_heart_proxy_runs":3}}"#,
    )
    .unwrap();
    let mut sim = Sim::from(Some(fight()));
    sim.set_controls(Controls {
        campaign: Some(board.clone()),
        ..playing()
    });
    let start = Instant::now();
    frame(&mut sim, start);
    frame(&mut sim, start + Duration::from_millis(40));
    sim.latch();
    let run = sim.as_ref().unwrap();
    assert_eq!(run.campaign.count("vec_data_staged"), 1);
    assert_eq!(run.campaign.count("vec_heart_proxy_runs"), 3);
    // The board goes: within a second the run reads zeros.
    std::fs::remove_file(&board).unwrap();
    frame(&mut sim, start + Duration::from_millis(1100));
    frame(&mut sim, start + Duration::from_millis(1140));
    sim.latch();
    assert!(sim.as_ref().unwrap().campaign.is_empty());
    // With no board named, the run stays at zeros.
    sim.set_controls(playing());
    frame(&mut sim, start + Duration::from_millis(2200));
    sim.latch();
    assert!(sim.as_ref().unwrap().campaign.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn private_checkpoint_keeps_the_world_and_unacknowledged_claim_in_one_session() {
    use angel_delve::rules::host_commands::{Command, Journal};
    let dir = std::env::temp_dir().join(format!(
        "angel-sim-claim-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = dir.join("session.json");
    let mut sim = Sim::spawn(fight());
    let mut gold = angel_delve::realm::Spoils::default();
    gold.add(angel_delve::realm::Spoil::Gold, 19);
    sim.as_mut().unwrap().bank.push(angel_delve::realm::Haul {
        hero: 1,
        spoils: gold.clone(),
        why: "pending checkpoint".into(),
    });
    let claim = sim.claim_realm().unwrap().unwrap();
    sim.command(Command::LeaveActivity { player: 1 }).unwrap();
    sim.as_mut().unwrap().bank.push(angel_delve::realm::Haul {
        hero: 1,
        spoils: gold,
        why: "later checkpoint".into(),
    });
    sim.save_checkpoint(&path, 11).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["version"], 7);
    assert_eq!(saved["raid_serial"], 11);
    let run: Run = serde_json::from_value(saved["run"].clone()).unwrap();
    assert_eq!(run.bank.len(), 1);
    let commands =
        Journal::restore(saved["commands"].clone(), &claim.session, run.raid_id).unwrap();
    let mut restored = Sim::default();
    restored.restore_session(run.clone(), Some(commands));
    assert_eq!(restored.claim_realm().unwrap(), Some(claim.clone()));
    restored.acknowledge_realm(&claim).unwrap();
    let next = restored.claim_realm().unwrap().unwrap();
    assert_eq!(next.request, claim.request + 2);
    assert_eq!(next.transfer.hauls[0].why, "later checkpoint");
    restored.set(Some(run));
    let new_session = restored.claim_realm().unwrap().unwrap();
    assert_ne!(new_session.session, claim.session);
    assert_eq!(new_session.request, 1);
    drop(restored);
    drop(sim);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "requires the two native system binaries built with swap-test"]
fn native_reforge_policy_swap_keeps_reservations_seats_cooldowns_and_cached_deliveries() {
    use angel_delve::rules::{reforge_jobs, cards, knights::Part};
    let root = std::env::temp_dir().join(format!(
        "native-reforge-port-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_WORKER").unwrap(),
        &artifact,
    )
    .unwrap();
    let mut run = Run::new(0, 9, Some("Guest"));
    run.descend_for_test();
    let mut sim = Sim::from(Some(run));
    let seat = sim.seat(1).unwrap() as *const Seat;
    {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core).engine = Some(
            angel_live::system::System::watch_with_contracts(
                "delve",
                artifact.clone(),
                Duration::from_secs(2),
                BTreeMap::from([(reforge_jobs::CONTRACT.into(), reforge_jobs::API)]),
            )
            .unwrap(),
        );
    }
    let mut claims = Vec::new();
    for player in [1, 2] {
        sim.as_mut()
            .unwrap()
            .players
            .get_mut(&player)
            .unwrap()
            .spell_cooldowns[0] = 71;
        claims.push(
            sim.claim_reforge(player, Part::Defense, "a silver shield".into())
                .unwrap(),
        );
    }
    let activate = |sim: &mut Sim, count| {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let live = sim.live.as_ref().unwrap();
            let mut core = lock(&live.shared().core);
            step_system(&mut core, &BTreeMap::new(), false);
            if core.engine.as_ref().unwrap().status().swaps == count {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", core.engine_error);
            drop(core);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    activate(&mut sim, 1);
    let inputs: Vec<_> = claims
        .iter()
        .map(|claim| {
            let mut card = sim
                .as_ref()
                .unwrap()
                .book
                .cards
                .iter()
                .find(|c| c.kind == cards::Kind::Guard)
                .unwrap()
                .clone();
            card.id = claim.work.result_id.clone();
            card.name = "Silver Shield".into();
            cards::check(&claim.work.result_id, &card.definition_file())
                .unwrap()
                .card
        })
        .collect();
    let done = sim
        .answer_reforge(&claims[0], inputs[0].clone())
        .unwrap()
        .unwrap();
    assert_eq!(done.result.as_deref(), Ok("Silver Shield"));
    let staged = artifact.with_extension("pending");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT").unwrap(),
        &staged,
    )
    .unwrap();
    let edited = Instant::now();
    std::fs::rename(staged, &artifact).unwrap();
    activate(&mut sim, 2);
    eprintln!(
        "native Sim reforge activation={:?}; host_pid={}",
        edited.elapsed(),
        std::process::id()
    );
    assert_eq!(
        sim.answer_reforge(&claims[0], inputs[0].clone())
            .unwrap()
            .unwrap(),
        done
    );
    assert_eq!(
        sim.answer_reforge(&claims[1], inputs[1].clone())
            .unwrap()
            .unwrap()
            .result
            .as_deref(),
        Ok("Worker Aegis")
    );
    assert_eq!(seat, sim.seat(1).unwrap() as *const Seat);
    let mut cursor = None;
    assert_eq!(sim.new_reforge_claims(&mut cursor).unwrap().len(), 2);
    assert!(sim.new_reforge_claims(&mut cursor).unwrap().is_empty());
    {
        let live = sim.live.as_ref().unwrap();
        let core = lock(&live.shared().core);
        assert_eq!(core.run.players.len(), 2);
        assert_eq!(core.run.players[&2].spell_cooldowns[0], 71);
        assert_eq!(
            core.run
                .sounds
                .iter()
                .filter(|s| **s == "card_pickup")
                .count(),
            2
        );
    }
    drop(sim);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires the two native system binaries built with swap-test"]
fn native_wish_policy_swap_keeps_active_model_jobs_and_exact_terminal_answers() {
    use angel_delve::rules::{genie::Asked, phrasebook, wish_jobs};
    use crate::together_grants::{Pool, Job, Scripted};
    let root = std::env::temp_dir().join(format!(
        "native-wish-port-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_WORKER").unwrap(),
        &artifact,
    )
    .unwrap();
    let mut run = Run::new(0, 9, Some("Guest"));
    run.descend_for_test();
    for hero in run.players.values_mut() {
        hero.spell_cooldowns[0] = 71;
    }
    let mut sim = Sim::from(Some(run));
    let seat = sim.seat(1).unwrap() as *const Seat;
    {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core).engine = Some(
            angel_live::system::System::watch_with_contracts(
                "delve",
                artifact.clone(),
                Duration::from_secs(2),
                BTreeMap::from([(wish_jobs::CONTRACT.into(), wish_jobs::API)]),
            )
            .unwrap(),
        );
    }
    let activate = |sim: &mut Sim, count| {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let live = sim.live.as_ref().unwrap();
            let mut core = lock(&live.shared().core);
            step_system(&mut core, &BTreeMap::new(), false);
            if core.engine.as_ref().unwrap().status().swaps == count {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", core.engine_error);
            drop(core);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    activate(&mut sim, 1);
    for player in [1, 2] {
        sim.as_mut().unwrap().asked.push(Asked { cosmetic: false, cosmetic_name: String::new(), fairy: false, star: false,
            knight: player,
            words: "golden grace".into(),
            steps: 1,
            great: false,
            fills: true,
            making: None,
            target: Default::default(),
        });
    }
    let claims = sim.new_wish_claims(&mut None, "Original maker").unwrap();
    let source =
        "wish captured-grace | Captured Grace\nsay golden grace\ntier damage 10 :: Grace\n";
    let input = phrasebook::read(source).remove(0);
    let done = sim
        .answer_wish(&claims[0], Some(input.clone()), true, None)
        .unwrap();
    assert!(
        matches!(&done.outcome, wish_jobs::Outcome::Granted(name) if name == "Captured Grace I")
    );
    let original_card = sim.as_ref().unwrap().book.get("wish-captured-grace-1").unwrap().clone();
    // Real native-owned model call is already in flight during executable replacement.
    let pending_source = source.replace("wish captured-grace |", "wish pending-grace |");
    let club = Scripted::new(&[pending_source.as_str()], Duration::from_secs(2));
    let mut pool = Pool::default();
    let runners = pool.ready(&(club.clone() as std::sync::Arc<dyn crate::services::Oracle>));
    runners.send(Job {
        id: 42,
        oracle: club.clone(),
        brief: "golden grace".into(),
        sheets: root.join("sheets"),
        paint: root.join("paint"),
    });
    let deadline = Instant::now() + Duration::from_secs(4);
    while club.inside.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let staged = artifact.with_extension("pending");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT").unwrap(),
        &staged,
    )
    .unwrap();
    let edited = Instant::now();
    std::fs::rename(staged, &artifact).unwrap();
    activate(&mut sim, 2);
    eprintln!(
        "native wish implementation publication->activation={:?}; host_pid={}",
        edited.elapsed(),
        std::process::id()
    );
    assert!(std::sync::Arc::ptr_eq(
        &runners,
        pool.runners.as_ref().unwrap()
    ));
    assert!(
        club.inside.load(std::sync::atomic::Ordering::SeqCst) > 0,
        "call must still be active at swap"
    );
    let active_pid = {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core)
            .engine
            .as_ref()
            .unwrap()
            .status()
            .worker_pid
    };
    let invalid = artifact.with_extension("invalid");
    std::fs::write(&invalid, b"invalid native candidate").unwrap();
    std::fs::rename(invalid, &artifact).unwrap();
    loop {
        let live = sim.live.as_ref().unwrap();
        let mut core = lock(&live.shared().core);
        step_system(&mut core, &BTreeMap::new(), false);
        let status = core.engine.as_ref().unwrap().status();
        if status.rejected > 0 {
            assert_eq!(status.swaps, 2);
            assert_eq!(status.worker_pid, active_pid);
            break;
        }
        assert!(Instant::now() < deadline, "bad candidate was not examined");
        drop(core);
        std::thread::sleep(Duration::from_millis(10));
    }
    let model = loop {
        let mut done = runners.poll();
        if !done.is_empty() {
            break done.remove(0);
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(model.id, 42);
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    let (_, definition) = model.result.unwrap();
    let changed = sim
        .answer_wish(&claims[1], Some(definition), true, None)
        .unwrap();
    assert!(
        matches!(&changed.outcome, wish_jobs::Outcome::Granted(name) if name.ends_with("Worker Blessing I")), "{changed:?}"
    );
    assert_eq!(changed.accepted.as_ref().unwrap().name, "Worker Blessing");
    assert_eq!(sim.as_ref().unwrap().book.get("wish-captured-grace-1"), Some(&original_card));
    let sounds = sim.as_ref().unwrap().sounds.clone();
    assert_eq!(
        sim.answer_wish(&claims[0], Some(input), true, None)
            .unwrap(),
        done
    );
    assert_eq!(
        sim.as_ref().unwrap().sounds,
        sounds,
        "terminal replay cannot issue effects"
    );
    assert_eq!(seat, sim.seat(1).unwrap() as *const Seat);
    assert_eq!(sim.as_ref().unwrap().players.len(), 2);
    assert_eq!(sim.as_ref().unwrap().players[&2].spell_cooldowns[0], 71);
    let checkpoint = root.join("session.json");
    sim.save_checkpoint(&checkpoint, 9).unwrap();
    sim.acknowledge_wish(&claims[0], &done).unwrap();
    sim.acknowledge_wish(&claims[1], &changed).unwrap();
    sim.as_mut().unwrap().asked.push(Asked { cosmetic: false, cosmetic_name: String::new(), fairy: false, star: false,
        knight: 1,
        words: "reject wish policy".into(),
        steps: 1,
        great: false,
        fills: true,
        making: None,
        target: Default::default(),
    });
    let rejected = sim.new_wish_claims(&mut None, "").unwrap().remove(0);
    let input = phrasebook::read(source).remove(0);
    let before = sim.as_ref().unwrap().players[&1].wishes;
    let declined = sim
        .answer_wish(&rejected, Some(input.clone()), true, None)
        .unwrap();
    assert!(
        matches!(&declined.outcome, wish_jobs::Outcome::Refunded(reason) if reason.contains("replacement declined"))
    );
    assert_eq!(declined.input, Some(input.clone()));
    assert!(declined.accepted.is_none());
    assert_eq!(
        sim.answer_wish(&rejected, Some(input), true, None).unwrap(),
        declined
    );
    assert_eq!(sim.as_ref().unwrap().players[&1].wishes, before + 1);
    sim.acknowledge_wish(&rejected, &declined).unwrap();
    let mut micros = Vec::new();
    for n in 0..12 {
        sim.as_mut().unwrap().asked.push(Asked { cosmetic: false, cosmetic_name: String::new(), fairy: false, star: false,
            knight: 1,
            words: format!("bench grace {n}"),
            steps: 1,
            great: false,
            fills: true,
            making: None,
            target: Default::default(),
        });
        let claim = sim.new_wish_claims(&mut None, "").unwrap().remove(0);
        let mut input = phrasebook::read(source).remove(0);
        input.id = format!("bench-grace-{n}");
        let started = Instant::now();
        let done = sim.answer_wish(&claim, Some(input), true, None).unwrap();
        micros.push(started.elapsed().as_micros());
        sim.acknowledge_wish(&claim, &done).unwrap();
    }
    micros.sort();
    eprintln!(
        "wish policy RPC+atomic delivery n={} p50={}us p95={}us max={}us (no model/render/build)",
        micros.len(),
        micros[6],
        micros[11],
        micros[11]
    );
    drop(sim);
    drop(pool);
    drop(runners);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires the two native system binaries built with swap-test"]
fn native_paint_policy_swap_preserves_cards_seats_and_cooldowns() {
    use angel_delve::rules::{card_paint, cards, runtime_state};
    let root = std::env::temp_dir().join(format!(
        "native-paint-port-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_WORKER").unwrap(),
        &artifact,
    )
    .unwrap();
    let mut run = fight();
    let mut original = cards::check(
        "paint-probe",
        "name Paint Probe\nkind spell\ncooldown 5\nnova 10\ndraw shot\n00\n00\n",
    )
    .unwrap()
    .card;
    original.by = "Original maker".into();
    run.book.insert(original.clone());
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    run.cues.push("issued effect".into());
    let mut sim = Sim::from(Some(run));
    let seat = sim.seat(1).unwrap() as *const Seat;
    {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core).engine = Some(
            angel_live::system::System::watch_with_contracts(
                "delve",
                artifact.clone(),
                Duration::from_secs(2),
                BTreeMap::from([(card_paint::CONTRACT.into(), card_paint::API)]),
            )
            .unwrap(),
        );
    }
    let activate = |sim: &mut Sim, count| {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let live = sim.live.as_ref().unwrap();
            let mut core = lock(&live.shared().core);
            step_system(&mut core, &BTreeMap::new(), false);
            if core.engine.as_ref().unwrap().status().swaps == count {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", core.engine_error);
            drop(core);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let mut authored = original.clone();
    authored.by.clear();
    authored.name = "Changed source".into();
    authored.effects = vec![cards::Effect::Nova(99)];
    authored.cooldown = 1;
    authored.looks.shot.as_mut().unwrap().frames = vec![vec!["11".into(), "11".into()]];
    // Preparing the worker cannot admit or lose the native completion.
    assert!(sim.paint_cards(vec![authored.clone()]).is_err());
    activate(&mut sim, 1);
    assert!(
        !sim.paint_cards(vec![authored.clone()]).unwrap(),
        "original rules must decline changed gameplay source"
    );
    let before = runtime_state::capture(sim.as_ref().unwrap()).unwrap().run;
    let old_pid = {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core)
            .engine
            .as_ref()
            .unwrap()
            .status()
            .worker_pid
            .unwrap()
    };
    let pending = artifact.with_extension("pending");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT").unwrap(),
        &pending,
    )
    .unwrap();
    let started = Instant::now();
    std::fs::rename(&pending, &artifact).unwrap();
    activate(&mut sim, 2);
    let activated = started.elapsed();
    assert!(
        sim.paint_cards(vec![authored.clone()]).unwrap(),
        "replacement policy was not used"
    );
    assert_eq!(sim.seat(1).unwrap() as *const Seat, seat);
    let painted = sim
        .as_ref()
        .unwrap()
        .book
        .get("paint-probe")
        .unwrap()
        .clone();
    let mut projected = painted.clone();
    projected.looks = original.looks.clone();
    assert_eq!(projected, original, "paint rewrote gameplay or maker");
    assert_eq!(painted.looks, authored.looks);
    let mut after = runtime_state::capture(sim.as_ref().unwrap()).unwrap().run;
    after["book"] = before["book"].clone();
    assert_eq!(after, before);
    // Malformed replacement retains the working policy and process.
    std::fs::write(&pending, b"bad executable").unwrap();
    std::fs::rename(&pending, &artifact).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let live = sim.live.as_ref().unwrap();
        let mut core = lock(&live.shared().core);
        step_system(&mut core, &BTreeMap::new(), false);
        if core.engine.as_ref().unwrap().status().rejected > 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        drop(core);
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut samples = Vec::new();
    for n in 0..12 {
        authored.looks.shot.as_mut().unwrap().frames[0] =
            vec![if n % 2 == 0 { "22".into() } else { "11".into() }; 2];
        let started = Instant::now();
        assert!(sim.paint_cards(vec![authored.clone()]).unwrap());
        samples.push(started.elapsed().as_micros());
    }
    samples.sort();
    eprintln!(
        "paint policy publication->activation={activated:?}; 12 captured-card RPC+atomic commits median={}us p95={}us; no model/render/build/durable IO; host_pid={}",
        samples[6],
        samples[11],
        std::process::id()
    );
    let new_pid = {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core)
            .engine
            .as_ref()
            .unwrap()
            .status()
            .worker_pid
            .unwrap()
    };
    assert_ne!(old_pid, new_pid);
    drop(sim);
    #[cfg(target_os = "linux")]
    for pid in [old_pid, new_pid] {
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "worker leaked after retirement"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires the two native system binaries built with swap-test"]
fn native_scroll_policy_swap_keeps_open_sources_and_active_model_work() {
    use angel_delve::rules::{
        cards,
        host_commands::{Command, Reply},
        phrasebook, scrolls,
    };
    use crate::together_grants::{Job, Pool, Scripted};
    struct RestoreBook((String, Vec<String>));
    impl Drop for RestoreBook {
        fn drop(&mut self) {
            phrasebook::install(self.0.0.clone(), self.0.1.clone());
        }
    }
    let restore = RestoreBook(phrasebook::snapshot());
    let source = "wish policy-probe | policy probe\nsay policy probe\ntier damage 5 :: First\ntier damage 10 :: Second\ntier damage 15 :: Third\n";
    phrasebook::install(format!("{}\n{source}", restore.0.0), Vec::new()).unwrap();
    let root = std::env::temp_dir().join(format!(
        "native-scroll-port-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_WORKER").unwrap(),
        &artifact,
    )
    .unwrap();
    // Pick a seed through the public constructor, retaining that exact world.
    let wish = phrasebook::get("policy-probe").unwrap();
    let mut run = (0..32)
        .find_map(|seed| {
            let mut run = Run::new(seed, 9, Some("Guest"));
            run.descend_for_test();
            for hero in run.players.values_mut() {
                hero.wishes = 3;
                hero.great_wishes = 0;
                hero.hp = 90;
                hero.max_hp = 90;
                hero.spell_cooldowns[0] = 71;
            }
            run.cues.push("issued effect".into());
            run.offer_wish(2, &wish.name, Some(&wish)).unwrap();
            run.players[&2]
                .offer
                .as_ref()
                .unwrap()
                .choices
                .iter()
                .any(|c| c.take == angel_delve::rules::genie::Take::Bolder)
                .then_some(run)
        })
        .expect("a seed offering Bolder");
    run.drop_offer(2);
    let mut sim = Sim::from(Some(run));
    let seat = sim.seat(1).unwrap() as *const Seat;
    {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core).engine = Some(
            angel_live::system::System::watch_with_contracts(
                "delve",
                artifact.clone(),
                Duration::from_secs(2),
                BTreeMap::from([(scrolls::CONTRACT.into(), scrolls::API)]),
            )
            .unwrap(),
        );
    }
    let activate = |sim: &mut Sim, swaps| {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let live = sim.live.as_ref().unwrap();
            let mut core = lock(&live.shared().core);
            step_system(&mut core, &BTreeMap::new(), false);
            if core.engine.as_ref().unwrap().status().swaps == swaps {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", core.engine_error);
            drop(core);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    activate(&mut sim, 1);
    sim.command(Command::KnownWish {
        player: 1,
        wish: "policy-probe".into(),
    })
    .unwrap();
    let offered = sim.as_ref().unwrap().players[&1].offer.clone().unwrap();
    assert_ne!(offered.line, "Replacement scroll policy");
    let old_pid = {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core)
            .engine
            .as_ref()
            .unwrap()
            .status()
            .worker_pid
            .unwrap()
    };
    let model_source = "wish model-probe | Model Probe\nsay model probe\ntier damage 5 :: Result\n";
    let club = Scripted::new(&[model_source], Duration::from_secs(2));
    let mut pool = Pool::default();
    let runners = pool.ready(&(club.clone() as std::sync::Arc<dyn crate::services::Oracle>));
    runners.send(Job {
        id: 42,
        oracle: club.clone(),
        brief: "model probe".into(),
        sheets: root.join("sheets"),
        paint: root.join("paint"),
    });
    let deadline = Instant::now() + Duration::from_secs(8);
    while club.inside.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    // Edit source and replace an actual executable with an open scroll and job.
    phrasebook::install(
        format!(
            "{}\n{}",
            restore.0.0,
            source
                .replace("damage 10", "damage 12")
                .replace("damage 5", "damage 8")
        ),
        Vec::new(),
    )
    .unwrap();
    let pending = artifact.with_extension("pending");
    std::fs::copy(
        std::env::var("ANGEL_SYSTEM_TEST_REPLACEMENT").unwrap(),
        &pending,
    )
    .unwrap();
    let started = Instant::now();
    std::fs::rename(&pending, &artifact).unwrap();
    activate(&mut sim, 2);
    let activated = started.elapsed();
    assert_eq!(
        sim.as_ref().unwrap().players[&1].offer.as_ref(),
        Some(&offered)
    );
    assert!(std::sync::Arc::ptr_eq(
        &runners,
        pool.runners.as_ref().unwrap()
    ));
    assert!(club.inside.load(std::sync::atomic::Ordering::SeqCst) > 0);
    assert!(matches!(
        sim.command(Command::ChooseOffer {
            player: 1,
            index: 0
        })
        .unwrap(),
        Reply::Chosen(_)
    ));
    assert_eq!(
        sim.as_ref()
            .unwrap()
            .book
            .get("wish-policy-probe-1")
            .unwrap()
            .effects,
        vec![cards::Effect::Damage(5)]
    );
    // Open a fresh offer against the new definition and replacement price.
    // Retire the old held card first: shared held IDs cannot be overwritten.
    sim.as_mut()
        .unwrap()
        .players
        .get_mut(&1)
        .unwrap()
        .deck
        .retain(|id| id != "wish-policy-probe-1");
    sim.command(Command::KnownWish {
        player: 2,
        wish: "policy-probe".into(),
    })
    .unwrap();
    let fresh = sim.as_ref().unwrap().players[&2].offer.clone().unwrap();
    assert_eq!(fresh.line, "Replacement scroll policy");
    let bolder = fresh
        .choices
        .iter()
        .position(|c| c.take == angel_delve::rules::genie::Take::Bolder)
        .expect("chosen seed offers Bolder");
    assert_eq!(fresh.choices[bolder].cost, "costs 15 health");
    sim.command(Command::ChooseOffer {
        player: 2,
        index: bolder as u8,
    })
    .unwrap();
    assert_eq!(sim.as_ref().unwrap().players[&2].hp, 75);
    assert_eq!(
        sim.as_ref()
            .unwrap()
            .book
            .get("wish-policy-probe-2")
            .unwrap()
            .effects,
        vec![cards::Effect::Damage(12)]
    );
    assert_eq!(sim.seat(1).unwrap() as *const Seat, seat);
    assert_eq!(sim.as_ref().unwrap().players.len(), 2);
    assert_eq!(sim.as_ref().unwrap().players[&2].spell_cooldowns[0], 71);
    assert!(sim.as_ref().unwrap().cues.contains(&"issued effect".into()));
    // Invalid candidates keep the active implementation usable.
    std::fs::write(&pending, b"bad executable").unwrap();
    std::fs::rename(&pending, &artifact).unwrap();
    loop {
        let live = sim.live.as_ref().unwrap();
        let mut core = lock(&live.shared().core);
        step_system(&mut core, &BTreeMap::new(), false);
        if core.engine.as_ref().unwrap().status().rejected > 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        drop(core);
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut samples = Vec::new();
    for _ in 0..12 {
        let started = Instant::now();
        sim.command(Command::Scroll {
            player: 1,
            intent: scrolls::Intent::Local {
                words: "policy probe unseen".into(),
                known: None,
            },
        })
        .unwrap();
        samples.push(started.elapsed().as_micros());
    }
    samples.sort();
    let model = loop {
        let mut results = runners.poll();
        if !results.is_empty() {
            break results.remove(0);
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(model.id, 42);
    assert_eq!(club.asked.lock().unwrap().len(), 1);
    assert_eq!(model.result.unwrap().1.name, "Model Probe");
    eprintln!(
        "scroll policy built-publication->activation={activated:?}; 12 metadata RPC+atomic journal commits median={}us p95={}us; excludes compile/model/render/durable IO; host_pid={}",
        samples[6],
        samples[11],
        std::process::id()
    );
    let new_pid = {
        let live = sim.live.as_ref().unwrap();
        lock(&live.shared().core)
            .engine
            .as_ref()
            .unwrap()
            .status()
            .worker_pid
            .unwrap()
    };
    assert_ne!(old_pid, new_pid);
    drop(sim);
    drop(pool);
    drop(runners);
    #[cfg(target_os = "linux")]
    for pid in [old_pid, new_pid] {
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "worker leaked"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
