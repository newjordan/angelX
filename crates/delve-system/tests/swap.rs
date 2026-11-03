#![cfg(feature = "swap-test")]
use angel_delve::rules::{Input, Run, phrasebook, runtime_state};
use angel_live::system::System;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime},
};

fn state(run: &Run) -> Value {
    runtime_state::capture(run).unwrap().into_value()
}
fn validated(value: &Value, parsed: &std::cell::RefCell<Option<Run>>) -> Result<(), String> {
    let run = runtime_state::restore_value(value)?;
    parsed.replace(Some(run));
    Ok(())
}
fn event(inputs: &BTreeMap<u32, Input>) -> Value {
    json!({"inputs":inputs,"settings":angel_delve::settings::get(),"phrasebook":phrasebook::snapshot()})
}
fn publish(source: &str, dest: &Path) {
    let pending = dest.with_extension("pending");
    fs::copy(source, &pending).unwrap();
    fs::rename(pending, dest).unwrap();
}

#[test]
fn replaces_the_entire_native_rules_implementation_with_the_same_run_and_measured_tick_cost() {
    let root = std::env::temp_dir().join(format!(
        "delve-swap-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let artifact = root.join("engine");
    publish(env!("CARGO_BIN_EXE_angel-delve-system"), &artifact);
    let mut system = System::watch("delve", artifact.clone(), Duration::from_secs(2)).unwrap();
    let mut native = Run::new(470, 103, None);
    native.cues.push("pending host action".into());
    native.kingdom.door_latched.insert(1);
    let mut remote = native.clone();
    let input = BTreeMap::from([(
        1,
        Input {
            move_x: 1,
            fire: true,
            ..Default::default()
        },
    )]);
    let mut last_generation = None;
    let mut samples = Vec::new();
    for tick in 0..150 {
        if tick == 50 {
            publish(env!("CARGO_BIN_EXE_angel-delve-system-fixture"), &artifact);
        }
        let started = Instant::now();
        let before = state(&remote);
        let parsed = std::cell::RefCell::new(None);
        let (_next, _) = loop {
            match system.apply(before.clone(), event(&input), |value| {
                validated(value, &parsed)
            }) {
                Ok(reply) => break reply,
                Err(e)
                    if system.status().swaps == 0 && started.elapsed() < Duration::from_secs(5) =>
                {
                    let _ = e;
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("rules request failed: {e}"),
            }
        };
        native.step(&input);
        let status = system.status();
        if status.swaps == 2 {
            native.score += 7;
        }
        remote = parsed.into_inner().unwrap();
        samples.push(started.elapsed().as_micros());
        assert_eq!(state(&remote), state(&native), "tick {tick}");
        if tick == 49 {
            last_generation = status.active;
        }
        if tick >= 50 && status.swaps < 2 {
            thread::sleep(Duration::from_millis(12));
        }
    }
    assert_eq!(
        system.status().swaps,
        2,
        "second executable never activated"
    );
    assert_ne!(system.status().active, last_generation);
    assert_eq!(remote.raid_id, 103);
    samples.remove(0);
    samples.sort();
    eprintln!(
        "full Delve rules IPC: state={} bytes, p50={}us, p95={}us, max={}us, host_pid={}, generation_swaps={}",
        serde_json::to_vec(&state(&remote)).unwrap().len(),
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples.last().unwrap(),
        std::process::id(),
        system.status().swaps
    );
    assert!(
        samples[samples.len() * 95 / 100] < 33_333,
        "rules IPC exceeds the 30Hz tick budget"
    );
    drop(system);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn four_animated_characters_keep_appearance_out_of_rules_rpc_and_preserve_full_state() {
    let root = std::env::temp_dir().join(format!("delve-appearance-swap-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    publish(env!("CARGO_BIN_EXE_angel-delve-system"), &artifact);
    let mut system = System::watch("delve", artifact, Duration::from_secs(2)).unwrap();
    let mut expected = Run::new(33, 72, Some("Two"));
    expected.join_seat(3, "Three");
    expected.join_seat(4, "Four");
    for (id, hero) in &mut expected.players {
        hero.avatar = Some(angel_delve::realm::Avatar {
            source_id: format!("character-{id}"),
            animation: Some(angel_delve::realm::appearance::Animation {
                frames: vec![vec![[80, 120, 200, 255]; 1024]; 8],
                frame_ticks: 6,
                phase_offset: 0,
            }),
            pixels: vec![[80, 120, 200, 255]; 1024],
            sprite: vec![0; 256],
            portrait: vec![0; 512],
            colour: [80, 120, 200],
        });
    }
    let mut actual = expected.clone();
    let mut samples = Vec::new();
    let input = BTreeMap::new();
    let full_bytes = serde_json::to_vec(&runtime_state::capture(&actual).unwrap())
        .unwrap()
        .len();
    let rules_bytes = serde_json::to_vec(&runtime_state::capture_for_rules(&actual).unwrap())
        .unwrap()
        .len();
    assert!(
        full_bytes > rules_bytes + 512 * 1024,
        "appearance bytes still cross the rules boundary"
    );
    for _ in 0..60 {
        let started = Instant::now();
        let checkpoint = runtime_state::capture_for_rules(&actual)
            .unwrap()
            .into_value();
        let parsed = std::cell::RefCell::new(None);
        let (_next, _) = loop {
            match system.apply(checkpoint.clone(), event(&input), |value| {
                validated(value, &parsed)
            }) {
                Ok(next) => break next,
                Err(_)
                    if system.status().swaps == 0 && started.elapsed() < Duration::from_secs(5) =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("rules request: {error}"),
            }
        };
        let mut next = parsed.into_inner().unwrap();
        runtime_state::retain_appearance(&mut next, &actual);
        actual = next;
        expected.step(&input);
        samples.push(started.elapsed().as_micros());
        assert_eq!(state(&actual), state(&expected));
    }
    samples.remove(0);
    samples.sort();
    eprintln!(
        "four-seat animated character rules IPC: full={} bytes, rules={} bytes, p50={}us, p95={}us",
        full_bytes,
        rules_bytes,
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100]
    );
    drop(system);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn home_policy_replacement_changes_new_results_while_old_deliveries_keep_their_generation() {
    use angel_delve::realm::{Spoil, Spoils};
    use angel_delve::rules::{home, host_commands::Journal, home_settlement::*};
    let root = std::env::temp_dir().join(format!("delve-home-swap-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    publish(env!("CARGO_BIN_EXE_angel-delve-system"), &artifact);
    let mut system = System::watch_with_contracts(
        "delve",
        artifact.clone(),
        Duration::from_secs(2),
        BTreeMap::from([(POLICY_CONTRACT.into(), POLICY_API)]),
    )
    .unwrap();
    let mut run = Run::at_home(
        3,
        9,
        Some("Guest"),
        home::Home::default(),
        Spoils::default(),
    );
    run.marks.insert("native_policy_probe".into(), 1);
    run.marks.insert("xp".into(), 2);
    run.cues.push("already issued cue".into());
    let mut journal = Journal::new("native-home-swap".into(), 9);
    let claim = journal.claim_home(&mut run).unwrap().unwrap();
    let checkpoint = state(&run);
    let mut event = event(&BTreeMap::new());
    event["step"] = false.into();
    let activate = |system: &mut System, count| {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Ok((next, _)) = system.apply(checkpoint.clone(), event.clone(), |value| {
                let candidate = runtime_state::restore_value(value)?;
                journal.validate_home_world(&candidate)?;
                if *value != checkpoint {
                    return Err("pure handoff changed world".into());
                }
                Ok(())
            }) {
                assert_eq!(next, checkpoint);
                if system.status().swaps == count {
                    break;
                }
            }
            assert!(Instant::now() < deadline, "{:?}", system.status());
            thread::sleep(Duration::from_millis(10));
        }
    };
    activate(&mut system, 1);
    let first_pid = system.status().worker_pid.unwrap();
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 100);
    let request = HomePolicyRequest {
        context: HomeContext {
            home: run.home.clone(),
            treasury,
            reclaimed: BTreeMap::new(),
            raids_won: 0,
        },
        transfer: claim.transfer.clone(),
        host: "Host".into(),
        unsaved: false,
    };
    let evaluate = |system: &mut System| {
        let value = system
            .compute(
                POLICY_CONTRACT,
                POLICY_API,
                serde_json::to_value(&request).unwrap(),
                |value| {
                    let result: HomeResult =
                        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                    request.validate_result(&result)
                },
            )
            .unwrap();
        serde_json::from_value::<HomeResult>(value).unwrap()
    };
    let old_result = evaluate(&mut system);
    assert_eq!(old_result.treasury.get(Spoil::Gold), 100);
    let edited = Instant::now();
    publish(env!("CARGO_BIN_EXE_angel-delve-system-fixture"), &artifact);
    activate(&mut system, 2);
    let activated = edited.elapsed();
    assert_ne!(system.status().worker_pid, Some(first_pid));
    #[cfg(target_os = "linux")]
    assert!(!Path::new(&format!("/proc/{first_pid}")).exists());
    let mut samples = Vec::new();
    let mut new_result = old_result.clone();
    for _ in 0..60 {
        let start = Instant::now();
        new_result = evaluate(&mut system);
        samples.push(start.elapsed().as_micros());
    }
    assert_eq!(new_result.treasury.get(Spoil::Gold), 107);
    assert_eq!(
        state(&run),
        checkpoint,
        "computation touched the game or issued queues"
    );
    journal
        .complete_home(&mut run, &claim, old_result.clone())
        .unwrap();
    assert_eq!(run.treasury.get(Spoil::Gold), 100);
    assert_eq!(run.players.len(), 2);
    assert!(run.cues.iter().any(|cue| cue == "already issued cue"));
    assert_eq!(
        journal.home_completion(&claim).unwrap().unwrap().result,
        old_result
    );
    assert!(
        journal
            .complete_home(&mut run, &claim, new_result.clone())
            .is_err()
    );
    samples.sort();
    eprintln!(
        "Home policy IPC: request={} bytes result={} bytes (no Run); activation={:?}; p50={}us p95={}us; host_pid={}",
        serde_json::to_vec(&request).unwrap().len(),
        serde_json::to_vec(&new_result).unwrap().len(),
        activated,
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        std::process::id()
    );
    drop(system);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn brew_policy_swap_preserves_a_committed_answer_and_changes_a_pending_model_delivery() {
    use angel_delve::rules::{brew_jobs::*, cauldron, cards, host_commands::Journal};
    let root = std::env::temp_dir().join(format!("delve-brew-swap-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    publish(env!("CARGO_BIN_EXE_angel-delve-system"), &artifact);
    let mut system = System::watch_with_contracts(
        "delve",
        artifact.clone(),
        Duration::from_secs(2),
        BTreeMap::from([(CONTRACT.into(), API)]),
    )
    .unwrap();
    let mut run = Run::new(3, 9, Some("Guest"));
    let mut journal = Journal::new("brew-worker-session".into(), 9);
    let mut claims = Vec::new();
    for player in [1, 2] {
        let placeholder = cauldron::brewed_card(
            cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
            cauldron::Tier::Common,
        );
        run.players.get_mut(&player).unwrap().spells[0] = Some(placeholder.id.clone());
        run.players.get_mut(&player).unwrap().spell_cooldowns[0] = 71;
        run.brewing.push(cauldron::Brewing {
            knight: player,
            raid: run.raid_id,
            depth: run.dungeon.depth,
            cards: ["cabbage-storm".into(), "hedge-maze".into()],
            placeholder: placeholder.id.clone(),
        });
        run.book.insert(placeholder);
        claims.push(journal.claim_brew(&mut run).unwrap().unwrap());
    }
    let raw = "name Frostbriar Orchard\nkind spell\nrarity rare\ntext Thorns and a cold wind.\ndrop 0\nchest 0\ncooldown 20\ncabbages 3\nhedge 1\nward 1\nart\n..fDDf..\n.fDEEDf.\nfDE44EDf\nfDE44EDf\n.fDEEDf.\n..fDDf..\n";
    let mut requests: Vec<_> = claims
        .iter()
        .map(|claim| PolicyRequest {
            work: claim.work.clone(),
            card: cards::check(&claim.work.result_id, raw).unwrap().card,
            taken: cauldron::taken(&run.book),
        })
        .collect();
    let activate = |system: &mut System, run: &Run, journal: &Journal, count| {
        let checkpoint = state(run);
        let mut event = event(&BTreeMap::new());
        event["step"] = false.into();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Ok((next, _)) = system.apply(checkpoint.clone(), event.clone(), |value| {
                let candidate = runtime_state::restore_value(value)?;
                journal.validate_brew_world(&candidate)?;
                if *value != checkpoint {
                    return Err("handoff changed pending work".into());
                }
                Ok(())
            }) {
                assert_eq!(next, checkpoint);
                if system.status().swaps == count {
                    break;
                }
            }
            assert!(Instant::now() < deadline, "{:?}", system.status());
            thread::sleep(Duration::from_millis(10));
        }
    };
    let evaluate = |system: &mut System, request: &PolicyRequest| {
        let parsed = std::cell::RefCell::new(None);
        system
            .compute(
                CONTRACT,
                API,
                serde_json::to_value(request).unwrap(),
                |value| {
                    let result = <PolicyResult as serde::Deserialize>::deserialize(value)
                        .map_err(|e| e.to_string())?;
                    request.validate_result(&result)?;
                    parsed.replace(Some(result));
                    Ok(())
                },
            )
            .unwrap();
        parsed.into_inner().unwrap()
    };
    activate(&mut system, &run, &journal, 1);
    requests[1].card.cooldown = 2;
    assert!(
        evaluate(&mut system, &requests[1])
            .unwrap_err()
            .iter()
            .any(|error| error.contains("power"))
    );
    let first_pid = system.status().worker_pid.unwrap();
    let old = evaluate(&mut system, &requests[0]).unwrap();
    let done = journal
        .complete_brew(&mut run, &claims[0], requests[0].card.clone(), old.clone())
        .unwrap();
    let edited = Instant::now();
    publish(env!("CARGO_BIN_EXE_angel-delve-system-fixture"), &artifact);
    activate(&mut system, &run, &journal, 2);
    let activation = edited.elapsed();
    assert_ne!(system.status().worker_pid, Some(first_pid));
    #[cfg(target_os = "linux")]
    assert!(!Path::new(&format!("/proc/{first_pid}")).exists());
    assert_eq!(
        journal
            .complete_brew(&mut run, &claims[0], requests[0].card.clone(), old)
            .unwrap(),
        done
    );
    let mut samples = Vec::new();
    let mut next = None;
    for _ in 0..60 {
        let start = Instant::now();
        next = Some(evaluate(&mut system, &requests[1]).unwrap());
        samples.push(start.elapsed().as_micros());
    }
    let next = next.unwrap();
    assert_eq!(next.name, "Worker Orchard");
    let delivered = journal
        .complete_brew(&mut run, &claims[1], requests[1].card.clone(), next.clone())
        .unwrap();
    assert!(delivered.result.is_ok());
    assert_eq!(
        run.book.get(&claims[0].work.result_id).unwrap().name,
        "Frostbriar Orchard"
    );
    assert_eq!(run.players[&2].spell_cooldowns[0], 71);
    assert_eq!(run.sounds.iter().filter(|s| **s == "wish_grant").count(), 2);
    samples.sort();
    eprintln!(
        "Brew policy IPC: request={} bytes result={} bytes; activation={activation:?}; p50={}us p95={}us host_pid={}",
        serde_json::to_vec(&requests[1]).unwrap().len(),
        serde_json::to_vec(&next).unwrap().len(),
        samples[30],
        samples[57],
        std::process::id()
    );
    drop(system);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reforge_policy_swap_preserves_captured_kit_and_changes_only_uncommitted_answers() {
    use angel_delve::rules::{reforge_jobs::*, cards, knights::Part, host_commands::Journal};
    let root = std::env::temp_dir().join(format!("delve-reforge-swap-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let artifact = root.join("engine");
    publish(env!("CARGO_BIN_EXE_angel-delve-system"), &artifact);
    let mut system = System::watch_with_contracts(
        "delve",
        artifact.clone(),
        Duration::from_secs(2),
        BTreeMap::from([(CONTRACT.into(), API)]),
    )
    .unwrap();
    let mut run = Run::new(0, 9, Some("Guest"));
    run.dungeon.rooms[run.at].kind = angel_delve::rules::RoomKind::Sanctuary;
    let mut journal = Journal::new("reforge-worker-session".into(), 9);
    let mut claims = Vec::new();
    for player in [1, 2] {
        run.players.get_mut(&player).unwrap().spell_cooldowns[0] = 71;
        claims.push(
            journal
                .claim_reforge(&mut run, player, Part::Defense, "a silver shield".into())
                .unwrap(),
        );
    }
    let requests: Vec<_> = claims
        .iter()
        .map(|claim| {
            let mut card = run
                .book
                .cards
                .iter()
                .find(|c| c.kind == cards::Kind::Guard)
                .unwrap()
                .clone();
            card.id = claim.work.result_id.clone();
            card.name = "Silver Shield".into();
            PolicyRequest {
                work: claim.work.clone(),
                card: cards::check(&claim.work.result_id, &card.definition_file())
                    .unwrap()
                    .card,
            }
        })
        .collect();
    let activate = |system: &mut System, run: &Run, journal: &Journal, count| {
        let checkpoint = state(run);
        let mut event = event(&BTreeMap::new());
        event["step"] = false.into();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Ok((next, _)) = system.apply(checkpoint.clone(), event.clone(), |value| {
                let candidate = runtime_state::restore_value(value)?;
                journal.validate_reforge_handoff(run, &candidate)?;
                if *value != checkpoint {
                    return Err("handoff changed pending work".into());
                }
                Ok(())
            }) {
                assert_eq!(next, checkpoint);
                if system.status().swaps == count {
                    break;
                }
            }
            assert!(Instant::now() < deadline, "{:?}", system.status());
            thread::sleep(Duration::from_millis(10));
        }
    };
    let evaluate = |system: &mut System, request: &PolicyRequest| {
        let parsed = std::cell::RefCell::new(None);
        system
            .compute(
                CONTRACT,
                API,
                serde_json::to_value(request).unwrap(),
                |value| {
                    let result = <PolicyResult as serde::Deserialize>::deserialize(value)
                        .map_err(|e| e.to_string())?;
                    request.validate_result(&result)?;
                    parsed.replace(Some(result));
                    Ok(())
                },
            )
            .unwrap();
        parsed.into_inner().unwrap()
    };
    activate(&mut system, &run, &journal, 1);
    let first_pid = system.status().worker_pid.unwrap();
    let old = evaluate(&mut system, &requests[0]).unwrap();
    let done = journal
        .complete_reforge(&mut run, &claims[0], requests[0].card.clone(), old.clone())
        .unwrap();
    let edited = Instant::now();
    publish(env!("CARGO_BIN_EXE_angel-delve-system-fixture"), &artifact);
    activate(&mut system, &run, &journal, 2);
    let activation = edited.elapsed();
    assert_ne!(system.status().worker_pid, Some(first_pid));
    #[cfg(target_os = "linux")]
    assert!(!Path::new(&format!("/proc/{first_pid}")).exists());
    assert_eq!(
        journal
            .complete_reforge(&mut run, &claims[0], requests[0].card.clone(), old)
            .unwrap(),
        done
    );
    let mut samples = Vec::new();
    let mut next = None;
    for _ in 0..60 {
        let start = Instant::now();
        next = Some(evaluate(&mut system, &requests[1]).unwrap());
        samples.push(start.elapsed().as_micros());
    }
    let next = next.unwrap();
    assert_eq!(next.name, "Worker Aegis");
    let delivered = journal
        .complete_reforge(&mut run, &claims[1], requests[1].card.clone(), next.clone())
        .unwrap();
    assert!(delivered.result.is_ok());
    assert_eq!(
        run.book.get(&claims[0].work.result_id).unwrap().name,
        "Silver Shield"
    );
    assert_eq!(run.players[&2].spell_cooldowns[0], 71);
    assert_eq!(
        run.sounds.iter().filter(|s| **s == "card_pickup").count(),
        2
    );
    samples.sort();
    eprintln!(
        "Reforge policy IPC: request={} bytes result={} bytes; activation={activation:?}; p50={}us p95={}us host_pid={}",
        serde_json::to_vec(&requests[1]).unwrap().len(),
        serde_json::to_vec(&next).unwrap().len(),
        samples[30],
        samples[57],
        std::process::id()
    );
    drop(system);
    fs::remove_dir_all(root).unwrap();
}
