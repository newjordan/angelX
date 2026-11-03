#![cfg(all(feature = "swap-test", unix))]

use angel_delve::rules::{
    Run, cards, cauldron,
    host_commands::Journal,
    session::{self, Context, Event, Query, QueryReply, checkpoint, jobs},
};
use angel_delve_system::client::Client;
use angel_live::system::owned::{Limits, OwnedSystem};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    artifact: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "delve-owned-process-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self {
            artifact: root.join("engine"),
            root,
        }
    }
    fn publish(&self, source: &str) {
        let pending = self.artifact.with_extension("pending");
        fs::copy(source, &pending).unwrap();
        fs::rename(pending, &self.artifact).unwrap();
    }
    fn publish_broken_candidate(&self) {
        let pending = self.artifact.with_extension("pending");
        fs::write(&pending, "#!/bin/sh\nexit 19\n").unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(pending, &self.artifact).unwrap();
    }
    fn host(&self) -> OwnedSystem {
        OwnedSystem::watch(
            "delve",
            self.artifact.clone(),
            Duration::from_secs(2),
            BTreeMap::from([(session::CONTRACT.to_owned(), session::API)]),
            Limits::default(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn initial_with_pending_brew() -> session::checkpoint::SessionCheckpoint {
    let mut run = Run::new(77, 123, Some("Guest"));
    run.join_seat(3, "Remote Squire");
    run.join_seat(4, "Remote Knight");
    for index in 0..300 {
        let id = format!("lease-probe-{index:03}");
        let source = format!(
            "name Lease Probe {index}\nkind spell\ncooldown 4\ncast bolt\ntext A harmless measured fixture spell.\n"
        );
        let checked = cards::check(&id, &source)
            .unwrap_or_else(|error| panic!("fixture card {id} rejected: {error:?}"));
        assert!(run.book.insert(checked.card), "fixture book capacity");
    }
    let placeholder = cauldron::brewed_card(
        cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
        cauldron::Tier::Common,
    );
    run.players.get_mut(&1).unwrap().spells[0] = Some(placeholder.id.clone());
    run.brewing.push(cauldron::Brewing {
        knight: 1,
        raid: run.raid_id,
        depth: run.dungeon.depth,
        cards: ["cabbage-storm".into(), "hedge-maze".into()],
        placeholder: placeholder.id.clone(),
    });
    run.book.insert(placeholder);
    let session_id = "owned-delvecase";
    let mut journal = Journal::new(session_id.into(), run.raid_id);
    journal.claim_brew(&mut run).unwrap().unwrap();
    let context = Context::default();
    checkpoint::capture(&run, &journal, &context, 0, 0).unwrap()
}

fn start(host: &mut OwnedSystem, initial: &session::checkpoint::SessionCheckpoint) -> Client {
    let mut client = Client::new(initial).unwrap();
    let value = serde_json::to_value(initial).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match host.initialize(value.clone(), &mut client) {
            Ok(frame) => {
                assert_eq!(client.view().revision(), 1);
                assert_eq!(frame["revision"].as_u64(), Some(1));
                return client;
            }
            Err(error)
                if error.contains("preparing its first implementation")
                    && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("Delve worker initialization failed: {error}"),
        }
    }
}

fn send_tick(host: &mut OwnedSystem, client: &mut Client) -> Value {
    let event = client
        .envelope(Event::Tick {
            inputs: BTreeMap::new(),
            step: true,
        })
        .unwrap();
    let frame = host
        .event(serde_json::to_value(event).unwrap(), client)
        .unwrap();
    assert_eq!(client.take_reply(), Some(session::Reply::Done));
    frame
}

fn pending_brews(
    host: &mut OwnedSystem,
    client: &Client,
) -> Vec<angel_delve::rules::brew_jobs::Claim> {
    let query = client.query(Query::Job(jobs::Query::PendingBrews));
    let mut reply = None;
    host.query(serde_json::to_value(&query).unwrap(), |value| {
        reply = Some(client.read_query(&query, value)?);
        Ok(())
    })
    .unwrap();
    match reply.unwrap() {
        QueryReply::Job(jobs::Reply::Brews(claims)) => claims,
        other => panic!("unexpected pending-brew query reply: {other:?}"),
    }
}

fn wait_for_handoff(host: &mut OwnedSystem, client: &mut Client, target_swaps: u64) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match host.refresh(client) {
            Ok(true) if host.status().swaps >= target_swaps => return,
            Ok(_) | Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(15)),
            result => panic!(
                "owned Delve candidate did not activate: {result:?}; {:?}",
                host.status()
            ),
        }
    }
}

#[test]
fn live_delvecase_handoff_preserves_current_checkpoint_context_seat_and_journal_then_changes_future_rules()
 {
    let fixture = Fixture::new();
    fixture.publish(env!("CARGO_BIN_EXE_angel-delve-system"));
    let initial = initial_with_pending_brew();
    let mut host = fixture.host();
    let mut client = start(&mut host, &initial);
    assert_eq!(pending_brews(&mut host, &client).len(), 1);

    // The handoff source is an active checkpoint after a real accepted event,
    // rather than the original startup image.
    send_tick(&mut host, &mut client);
    let before = host.checkpoint_now(&mut client).unwrap();
    let before_checkpoint: session::checkpoint::SessionCheckpoint =
        serde_json::from_value(before.clone()).unwrap();
    assert_eq!(before_checkpoint.event_seq, 1);
    assert_eq!(before_checkpoint.context, initial.context);
    let before_restored = checkpoint::restore(before_checkpoint.clone()).unwrap();
    assert_eq!(before_restored.run.players[&3].name, "Remote Squire");
    assert_eq!(before_restored.run.players[&4].name, "Remote Knight");
    assert_eq!(
        before_restored.run.book.cards.len(),
        // Authored cards, recipe stand-in, and its per-claim retained binding.
        Run::new(0, 0, None).book.cards.len() + 302
    );
    assert_eq!(before_restored.journal.pending_brews().unwrap().len(), 1);
    let before_cursor = (client.event_seq(), client.view().revision());
    assert_eq!(before_cursor, (1, before_checkpoint.view_revision));

    let old_pid = host.status().worker_pid.unwrap();
    let activation_started = Instant::now();
    fixture.publish(env!("CARGO_BIN_EXE_angel-delve-system-fixture"));
    wait_for_handoff(&mut host, &mut client, 2);
    let activation_ms = activation_started.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "Delve owned activation (artifact copy/poll/capture/prepare/checkpoint/view handoff; excludes Rust compile): activation_ms={activation_ms:.3}"
    );
    assert_eq!(host.status().swaps, 2);
    let active_pid = host.status().worker_pid.unwrap();
    assert_ne!(active_pid, old_pid);
    #[cfg(target_os = "linux")]
    assert!(
        !Path::new(&format!("/proc/{old_pid}")).exists(),
        "handoff should reap the retired worker"
    );
    assert_eq!(
        (client.event_seq(), client.view().revision()),
        before_cursor
    );
    let after_handoff = host.checkpoint_now(&mut client).unwrap();
    assert_eq!(
        after_handoff, before,
        "replacement must preserve full current checkpoint"
    );
    assert_eq!(pending_brews(&mut host, &client).len(), 1);

    let before_book_len = before_restored.run.book.cards.len();
    let mut expected = before_restored.run;
    expected.step(&BTreeMap::new());
    expected.score += 7;
    send_tick(&mut host, &mut client);
    let after = host.checkpoint_now(&mut client).unwrap();
    let after: session::checkpoint::SessionCheckpoint = serde_json::from_value(after).unwrap();
    let restored = checkpoint::restore(after).unwrap();
    assert_eq!(restored.run.score, expected.score);
    assert_eq!(restored.run.tick, expected.tick);
    assert_eq!(restored.run.players[&3].name, "Remote Squire");
    assert_eq!(restored.run.players[&4].name, "Remote Knight");
    assert_eq!(restored.run.book.cards.len(), before_book_len);
    assert_eq!(restored.journal.pending_brews().unwrap().len(), 1);
    assert_eq!(restored.context, initial.context);
    assert_eq!(client.event_seq(), 2);
    assert_eq!(client.view().run().score, expected.score);

    let mut tick_micros = Vec::with_capacity(24);
    let mut frame_sizes = Vec::with_capacity(24);
    for _ in 0..24 {
        let started = Instant::now();
        let frame = send_tick(&mut host, &mut client);
        tick_micros.push(started.elapsed().as_micros());
        frame_sizes.push(serde_json::to_vec(&frame).unwrap().len());
    }
    let boundary_checkpoint = host.checkpoint_now(&mut client).unwrap();
    let full_checkpoint_bytes = serde_json::to_vec(&boundary_checkpoint).unwrap().len();
    tick_micros.sort_unstable();
    frame_sizes.sort_unstable();
    let p50_us = tick_micros[12];
    let p95_us = tick_micros[22];
    let max_us = tick_micros[23];
    let median_json_frame_bytes = frame_sizes[12];
    assert!(
        median_json_frame_bytes * 4 < full_checkpoint_bytes,
        "delta frame should be materially smaller than full checkpoint: median frame={median_json_frame_bytes}, checkpoint={full_checkpoint_bytes}"
    );
    eprintln!(
        "Delve owned raw Tick (RPC + Session validation + delta + typed Client; excludes render/native ability/model/durable IO): samples=24 p50_us={p50_us} p95_us={p95_us} max_us={max_us} median_json_frame_bytes={median_json_frame_bytes} full_session_checkpoint_bytes={full_checkpoint_bytes}; added_cards=300 valid spell cards, no art"
    );
    let final_checkpoint: session::checkpoint::SessionCheckpoint =
        serde_json::from_value(boundary_checkpoint).unwrap();
    assert_eq!(final_checkpoint.event_seq, 26);
    assert_eq!(
        checkpoint::restore(final_checkpoint)
            .unwrap()
            .run
            .book
            .cards
            .len(),
        before_book_len
    );

    let active_image = fs::read_link(format!("/proc/{active_pid}/exe")).ok();
    drop(host);
    #[cfg(target_os = "linux")]
    assert!(
        !Path::new(&format!("/proc/{active_pid}")).exists(),
        "last host drop should reap the active worker"
    );
    if let Some(image) = active_image {
        assert!(
            !image.exists(),
            "last host drop should release the active pinned executable image"
        );
    }
}

#[test]
fn real_worker_death_replays_accepted_prefix_without_republishing_reply_or_effects() {
    let fixture = Fixture::new();
    fixture.publish(env!("CARGO_BIN_EXE_angel-delve-system"));
    let initial = checkpoint::capture(
        &Run::new(17, 9, Some("Guest")),
        &Journal::new("owned-recovery".into(), 9),
        &Context::default(),
        0,
        0,
    )
    .unwrap();
    let mut host = fixture.host();
    let mut client = start(&mut host, &initial);
    send_tick(&mut host, &mut client);
    assert_eq!(client.event_seq(), 1);
    assert!(
        client.take_reply().is_none(),
        "accepted reply is consumed once"
    );

    let old_pid = host.status().worker_pid.unwrap();
    let old_image = fs::read_link(format!("/proc/{old_pid}/exe")).ok();
    let killed = Command::new("kill")
        .args(["-KILL", &old_pid.to_string()])
        .status()
        .expect("kill utility is available");
    assert!(killed.success());
    let query = client.query(Query::Cursor);
    assert!(
        host.query(serde_json::to_value(&query).unwrap(), |value| {
            client.read_query(&query, value).map(|_| ())
        })
        .is_err()
    );
    assert!(host.status().worker_pid.is_none());
    host.recover(&mut client).unwrap();
    let new_pid = host.status().worker_pid.unwrap();
    assert_ne!(new_pid, old_pid);
    assert_eq!(client.event_seq(), 1);
    assert!(
        client.take_reply().is_none(),
        "replay must not dispatch an old reply"
    );

    let cursor = client.query(Query::Cursor);
    let mut response = None;
    let raw = host
        .query(serde_json::to_value(&cursor).unwrap(), |value| {
            response = Some(client.read_query(&cursor, value)?);
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        response.unwrap(),
        QueryReply::Cursor { event_seq: 1, .. }
    ));
    let _ = raw;
    send_tick(&mut host, &mut client);
    assert_eq!(client.event_seq(), 2);

    let new_image = fs::read_link(format!("/proc/{new_pid}/exe")).ok();
    drop(host);
    assert!(!Path::new(&format!("/proc/{new_pid}")).exists());
    if let Some(image) = new_image.or(old_image) {
        assert!(
            !image.exists(),
            "last worker drop should release pinned executable image"
        );
    }
}

#[test]
fn invalid_replacement_candidate_is_rejected_without_retiring_the_active_delve_owner() {
    let fixture = Fixture::new();
    fixture.publish(env!("CARGO_BIN_EXE_angel-delve-system"));
    let initial = checkpoint::capture(
        &Run::new(41, 12, Some("Guest")),
        &Journal::new("owned-candidate".into(), 12),
        &Context::default(),
        0,
        0,
    )
    .unwrap();
    let mut host = fixture.host();
    let mut client = start(&mut host, &initial);
    let active_pid = host.status().worker_pid.unwrap();
    let rejected_before = host.status().rejected;
    fixture.publish_broken_candidate();
    let deadline = Instant::now() + Duration::from_secs(10);
    while host.status().rejected == rejected_before && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(15));
    }
    assert!(
        host.status().rejected > rejected_before,
        "broken candidate was not rejected"
    );
    assert_eq!(host.status().worker_pid, Some(active_pid));
    let frame = send_tick(&mut host, &mut client);
    assert_eq!(frame["seq"].as_u64(), Some(1));
    assert_eq!(host.status().worker_pid, Some(active_pid));
}

#[test]
fn real_worker_open_frame_handoff_then_accepted_batch_death_recovers_exact_finish_once() {
    use angel_delve::rules::{
        Shot, abilities as port,
        session::{abilities, frame},
    };

    let fixture = Fixture::new();
    fixture.publish(env!("CARGO_BIN_EXE_angel-delve-system"));
    let mut run = Run::new(31, 9, None);
    let definition = port::Slot {
        module: "process-probe".into(),
        generation: 1,
        ability: "bolt".into(),
        name: "Process Probe".into(),
        mana: 6.0,
        cooldown: 90,
        icon: String::new(),
    };
    let hero = run.players.get_mut(&1).unwrap();
    hero.mana = 20.0;
    hero.module_spells[0] = Some(definition.clone());
    hero.spell_cooldowns[0] = 0;
    let request = port::Request {
        raid: run.raid_id,
        epoch: run.seat_epoch(1),
        definition: definition.clone(),
        player: 1,
        slot: 0,
        module: definition.module.clone(),
        generation: definition.generation,
        ability: definition.ability.clone(),
        tick: run.tick,
    };
    run.module_casts.push(request.clone());
    run.cues.push("owned-frame-cue".into());
    let initial = checkpoint::capture(
        &run,
        &Journal::new("open-frame-process".into(), 9),
        &Context::default(),
        0,
        0,
    )
    .unwrap();
    let mut host = fixture.host();
    let mut client = start(&mut host, &initial);
    let begin = frame::Begin {
        network_tick: 11,
        expected_tick: run.tick,
        seats: frame::seats(&run),
        controls: frame::Controls {
            active: true,
            keyboard: true,
            ..Default::default()
        },
        tune: run.tune,
        input_ages: vec![],
        remove: vec![],
        away: vec![],
        arrival: Default::default(),
        presence: BTreeMap::new(),
        inputs: BTreeMap::new(),
        active_casts: vec![],
    };
    let envelope = client.envelope(Event::BeginFrame(begin)).unwrap();
    host.event(serde_json::to_value(envelope).unwrap(), &mut client)
        .unwrap();
    let begun = match client.take_reply().unwrap() {
        session::Reply::FrameBegun(begun) => begun,
        reply => panic!("{reply:?}"),
    };
    let open = host.checkpoint_now(&mut client).unwrap();
    let open_checkpoint: checkpoint::SessionCheckpoint =
        serde_json::from_value(open.clone()).unwrap();
    assert!(
        !open_checkpoint
            .pending_frame
            .as_ref()
            .unwrap()
            .abilities_applied
    );
    let old_pid = host.status().worker_pid.unwrap();
    fixture.publish(env!("CARGO_BIN_EXE_angel-delve-system-fixture"));
    wait_for_handoff(&mut host, &mut client, 2);
    assert_ne!(host.status().worker_pid, Some(old_pid));
    assert_eq!(host.checkpoint_now(&mut client).unwrap(), open);
    assert!(
        client.take_reply().is_none(),
        "handoff cannot redeliver Begin"
    );

    let cast = abilities::ActiveCast {
        session: "open-frame-process".into(),
        request: request.clone(),
        origin: port::Origin {
            module: request.module.clone(),
            generation: request.generation,
            cast: "one-bolt".into(),
        },
        started: true,
    };
    let batch = abilities::Batch {
        fence: abilities::OwnerFence::capture(client.view().run(), client.view().session()),
        frame: begun.frame.clone(),
        starts: vec![abilities::Start {
            cast: cast.clone(),
            default_sound: true,
        }],
        actions: vec![abilities::Action {
            cast,
            command: port::Command::Projectile {
                offset: [0.0, 0.0],
                velocity: [10.0, 0.0],
                damage: 5,
                ttl: 60,
                pierce: 0,
                kind: Shot::Bolt,
            },
        }],
        acknowledge: vec![request],
    };
    let batch_envelope = client
        .envelope(Event::Ability(abilities::Event::Batch(batch)))
        .unwrap();
    // Independent semantic witness starts at the real open-frame checkpoint.
    // Neither Batch nor Finish calls the alternate step policy again.
    let mut expected = session::Session::restore(open_checkpoint).unwrap();
    let expected_batch = expected
        .replay(batch_envelope.clone(), &mut session::DefaultPolicies)
        .unwrap();
    host.event(serde_json::to_value(batch_envelope).unwrap(), &mut client)
        .unwrap();
    assert_eq!(client.take_reply(), Some(expected_batch.reply));
    let accepted_view = client.view().clone();
    let accepted_seq = client.event_seq();
    // Do NOT checkpoint here: accepted Batch must remain in OwnedSystem's
    // replay tail after the open-frame checkpoint/handoff above.
    let dead_pid = host.status().worker_pid.unwrap();
    assert!(
        Command::new("kill")
            .args(["-KILL", &dead_pid.to_string()])
            .status()
            .unwrap()
            .success()
    );
    let phase_query = client.query(Query::FramePhase);
    assert!(
        host.query(serde_json::to_value(&phase_query).unwrap(), |value| {
            client.read_query(&phase_query, value).map(|_| ())
        })
        .is_err()
    );
    host.recover(&mut client).unwrap();
    assert_eq!(client.event_seq(), accepted_seq);
    assert_eq!(client.view().revision(), accepted_view.revision());
    assert_eq!(
        serde_json::to_value(client.view().run()).unwrap(),
        serde_json::to_value(accepted_view.run()).unwrap()
    );
    assert!(
        client.take_reply().is_none(),
        "replay must not redeliver native-start ACK"
    );
    let phase_query = client.query(Query::FramePhase);
    let mut phase = None;
    host.query(serde_json::to_value(&phase_query).unwrap(), |value| {
        phase = Some(client.read_query(&phase_query, value)?);
        Ok(())
    })
    .unwrap();
    match phase.unwrap() {
        QueryReply::FramePhase(Some(phase)) => {
            assert_eq!(phase.begun, begun);
            assert!(
                phase.abilities_applied,
                "native frozen plan reconciles known ACK, never resends Batch"
            );
        }
        reply => panic!("{reply:?}"),
    }
    let finish = client
        .envelope(Event::FinishFrame(frame::Finish {
            begin_seq: begun.frame.begin_seq,
            network_tick: begun.frame.network_tick,
            expected_tick: begun.frame.tick,
        }))
        .unwrap();
    let expected_finish = expected
        .replay(finish.clone(), &mut session::DefaultPolicies)
        .unwrap();
    host.event(serde_json::to_value(finish).unwrap(), &mut client)
        .unwrap();
    match &expected_finish.reply {
        session::Reply::FrameFinished(output) => {
            assert_eq!(
                output
                    .cues
                    .iter()
                    .filter(|cue| cue.as_str() == "owned-frame-cue")
                    .count(),
                1
            );
            assert_eq!(output.postlude.iter().filter(|signal| matches!(signal, frame::Signal::Sound { name, .. } if name == "spell_cast")).count(), 1);
        }
        reply => panic!("{reply:?}"),
    }
    assert_eq!(client.take_reply(), Some(expected_finish.reply));
    assert!(
        client.take_reply().is_none(),
        "Finish outputs delivered once"
    );
    let actual = host.checkpoint_now(&mut client).unwrap();
    assert_eq!(
        actual,
        serde_json::to_value(expected.checkpoint().unwrap()).unwrap()
    );
    let restored = checkpoint::restore(serde_json::from_value(actual).unwrap()).unwrap();
    assert_eq!(
        restored.run.projectiles.len(),
        1,
        "accepted projectile is not duplicated by recovery"
    );
    assert!(
        restored.run.module_casts.is_empty(),
        "exact request ACK survives replay"
    );
}
