//! Dispatch-harness tests use the exact handler passed to System::serve.
//! They do not assert supervisor/process replacement behavior.
use super::*;
use angel_delve::rules::{
    Run, cards, cauldron,
    host_commands::{Command, Journal, Request},
};
use session::{
    Context, Envelope, Event as OwnedEvent, Frame, Query, QueryEnvelope, Reply, Session,
    checkpoint::SessionCheckpoint, jobs, view,
};

type TestWorker = Worker<fn(&mut Run, &BTreeMap<u32, Input>)>;
fn step(run: &mut Run, inputs: &BTreeMap<u32, Input>) {
    run.step(inputs);
}
fn worker() -> TestWorker {
    Worker::new(
        step as fn(&mut Run, &BTreeMap<u32, Input>),
        Policies::default(),
    )
}
fn seed(run: Run) -> SessionCheckpoint {
    Session::new(
        run,
        Journal::new("owned-worker".into(), 9),
        Context::default(),
    )
    .unwrap()
    .checkpoint()
    .unwrap()
}
fn initial() -> SessionCheckpoint {
    seed(Run::new(17, 9, Some("second")))
}
fn stage(worker: &mut TestWorker, checkpoint: &SessionCheckpoint) -> view::Frame {
    let canonical = worker
        .handle("owned.restore", serde_json::to_value(checkpoint).unwrap())
        .unwrap();
    assert_eq!(canonical, serde_json::to_value(checkpoint).unwrap());
    let baseline =
        serde_json::from_value(worker.handle("owned.baseline", Value::Null).unwrap()).unwrap();
    worker.handle("owned.activate", Value::Null).unwrap();
    baseline
}
fn envelope(seq: u64, revision: u64, event: OwnedEvent) -> Envelope {
    Envelope {
        api: session::API,
        session: "owned-worker".into(),
        raid: 9,
        seq,
        expected_view_revision: revision,
        event,
    }
}
fn exchange(
    worker: &mut TestWorker,
    seq: &mut u64,
    revision: &mut u64,
    event: OwnedEvent,
) -> Frame {
    let input = envelope(*seq + 1, *revision, event);
    let body = serde_json::to_value(input).unwrap();
    assert!(
        body.get("state").is_none()
            && body.get("checkpoint").is_none()
            && body.get("run").is_none()
    );
    let frame: Frame = serde_json::from_value(worker.handle("owned.event", body).unwrap()).unwrap();
    *seq = frame.seq;
    *revision = frame.view.revision;
    frame
}
fn checkpoint(worker: &mut TestWorker) -> Value {
    worker.handle("owned.checkpoint", Value::Null).unwrap()
}
fn query(worker: &mut TestWorker, query: Query, revision: u64) -> session::QueryFrame {
    serde_json::from_value(
        worker
            .handle(
                "owned.query",
                serde_json::to_value(QueryEnvelope {
                    api: session::API,
                    session: "owned-worker".into(),
                    raid: 9,
                    expected_view_revision: Some(revision),
                    query,
                })
                .unwrap(),
            )
            .unwrap(),
    )
    .unwrap()
}
fn tick() -> OwnedEvent {
    OwnedEvent::Tick {
        inputs: BTreeMap::new(),
        step: true,
    }
}

#[test]
fn retained_owner_runs_180_events_and_journal_without_state_on_event_path() {
    let seed = initial();
    let mut worker = worker();
    let base = stage(&mut worker, &seed);
    let mut expected = Session::restore(seed).unwrap();
    assert_eq!(expected.baseline().unwrap(), base);
    let (mut seq, mut revision) = (0, base.revision);
    for i in 0..180 {
        let event = if i % 30 == 0 {
            OwnedEvent::Commands(Request {
                session: "owned-worker".into(),
                raid: 9,
                id: i / 30 + 1,
                commands: vec![Command::DropOffer { player: 1 }],
            })
        } else {
            tick()
        };
        let input = envelope(seq + 1, revision, event.clone());
        let expected_frame = expected
            .apply(input, &mut session::DefaultPolicies)
            .unwrap();
        let actual = exchange(&mut worker, &mut seq, &mut revision, event);
        assert_eq!(actual, expected_frame);
    }
    assert_eq!(
        checkpoint(&mut worker),
        serde_json::to_value(expected.checkpoint().unwrap()).unwrap()
    );
    assert!(matches!(
        query(&mut worker, Query::Cursor, revision).reply,
        session::QueryReply::Cursor {
            next_request: 7,
            event_seq: 180,
            ..
        }
    ));
}

#[test]
fn failed_stage_and_early_activation_preserve_active_owner_and_legacy_cannot_mutate_it() {
    let mut worker = worker();
    let seed = initial();
    let base = stage(&mut worker, &seed);
    let before = checkpoint(&mut worker);
    assert!(
        worker
            .handle("owned.restore", json!({"schema":999}))
            .is_err()
    );
    assert_eq!(checkpoint(&mut worker), before);
    assert!(worker.handle("owned.activate", Value::Null).is_err());
    worker
        .handle("owned.restore", serde_json::to_value(&seed).unwrap())
        .unwrap();
    assert!(worker.handle("owned.activate", Value::Null).is_err());
    assert_eq!(checkpoint(&mut worker), before);
    let mut invalid = envelope(1, 999, tick());
    invalid.session = "wrong".into();
    assert!(
        worker
            .handle("owned.replay", serde_json::to_value(invalid).unwrap())
            .is_err()
    );
    assert!(worker.handle("owned.baseline", Value::Null).is_err());
    assert!(worker.handle("owned.activate", Value::Null).is_err());
    assert!(
        worker
            .handle("apply", json!({"state":seed.run,"event":{}}))
            .is_err()
    );
    assert!(worker.handle("activate", Value::Null).is_err());
    assert_eq!(checkpoint(&mut worker), before);
    assert_eq!(
        query(&mut worker, Query::Cursor, base.revision).event_seq,
        0
    );
    worker.handle("dispose", Value::Null).unwrap();
    assert!(worker.handle("owned.checkpoint", Value::Null).is_err());
    assert!(worker.handle("owned.activate", Value::Null).is_err());
}

#[test]
fn staged_semantic_replay_preserves_checkpoint_and_carried_baseline_revision() {
    let mut uninterrupted = worker();
    let base = stage(&mut uninterrupted, &initial());
    let seed = checkpoint(&mut uninterrupted);
    let (mut seq, mut revision) = (0, base.revision);
    let mut tail = Vec::new();
    for _ in 0..180 {
        let input = envelope(seq + 1, revision, tick());
        let frame = exchange(&mut uninterrupted, &mut seq, &mut revision, tick());
        tail.push((input, frame.reply));
    }
    let expected = checkpoint(&mut uninterrupted);
    let mut replacement = worker();
    replacement.handle("owned.restore", seed).unwrap();
    for (input, reply) in tail {
        let replay: session::ReplayFrame = serde_json::from_value(
            replacement
                .handle("owned.replay", serde_json::to_value(input).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(replay.reply, reply);
    }
    assert!(replacement.handle("owned.event", Value::Null).is_err());
    let baseline: view::Frame =
        serde_json::from_value(replacement.handle("owned.baseline", Value::Null).unwrap()).unwrap();
    assert_eq!(baseline.revision, revision);
    assert_eq!(baseline.base_revision, 0);
    let mut receiver =
        view::Receiver::new("owned-worker".into(), 9, view::Limits::default()).unwrap();
    receiver.baseline(&baseline).unwrap();
    replacement.handle("owned.activate", Value::Null).unwrap();
    assert_eq!(checkpoint(&mut replacement), expected);
}

fn accept_brew(request: &brew_jobs::PolicyRequest) -> Result<brew_jobs::PolicyResult, String> {
    Ok(Ok(cards::check(
        &request.work.result_id,
        "name Retained Brew\nkind spell\ncooldown 4\ncast bolt\n",
    )
    .unwrap()
    .card))
}
#[test]
fn retained_journal_brew_claim_completion_retry_and_ack() {
    let mut run = Run::new(0, 9, None);
    let placeholder = cauldron::brewed_card(
        cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
        cauldron::Tier::Common,
    );
    run.players.get_mut(&1).unwrap().spells[0] = Some(placeholder.id.clone());
    run.brewing.push(cauldron::Brewing {
        knight: 1,
        raid: 9,
        depth: run.dungeon.depth,
        cards: ["cabbage-storm".into(), "hedge-maze".into()],
        placeholder: placeholder.id.clone(),
    });
    run.book.insert(placeholder);
    let mut worker = worker();
    worker.policies.brew = accept_brew;
    let base = stage(&mut worker, &seed(run));
    let (mut seq, mut revision) = (0, base.revision);
    let claim = match exchange(
        &mut worker,
        &mut seq,
        &mut revision,
        OwnedEvent::Job(jobs::Event::ClaimBrew),
    )
    .reply
    {
        Reply::Job(jobs::Reply::Brew(Some(claim))) => claim,
        other => panic!("{other:?}"),
    };
    for _ in 0..180 {
        exchange(
            &mut worker,
            &mut seq,
            &mut revision,
            OwnedEvent::Tick {
                inputs: BTreeMap::new(),
                step: false,
            },
        );
    }
    assert_eq!(
        query(&mut worker, Query::Job(jobs::Query::PendingBrews), revision).reply,
        session::QueryReply::Job(jobs::Reply::Brews(vec![claim.clone()]))
    );
    let input = cards::check(
        "native-answer",
        "name Retained Brew\nkind spell\ncooldown 4\ncast bolt\n",
    )
    .unwrap()
    .card;
    let event = OwnedEvent::Job(jobs::Event::CompleteBrew {
        claim: claim.clone(),
        input,
    });
    let done = match exchange(&mut worker, &mut seq, &mut revision, event.clone()).reply {
        Reply::Job(jobs::Reply::BrewCompleted(Some(done))) => done,
        other => panic!("{other:?}"),
    };
    assert!(done.result.is_ok());
    assert_eq!(
        exchange(&mut worker, &mut seq, &mut revision, event).reply,
        Reply::Job(jobs::Reply::BrewCompleted(Some(done.clone())))
    );
    exchange(
        &mut worker,
        &mut seq,
        &mut revision,
        OwnedEvent::Job(jobs::Event::AcknowledgeBrew {
            claim,
            completed: done,
        }),
    );
    assert_eq!(
        query(&mut worker, Query::Job(jobs::Query::PendingBrews), revision).reply,
        session::QueryReply::Job(jobs::Reply::Brews(Vec::new()))
    );
}

fn panic_step(run: &mut Run, _: &BTreeMap<u32, Input>) {
    run.tick += 1;
    panic!("injected after mutation");
}
#[test]
fn poisoned_active_refuses_continuation_queries_and_checkpoints_until_replaced() {
    let mut worker = worker();
    let seed = initial();
    let base = stage(&mut worker, &seed);
    worker.step = panic_step;
    assert!(
        worker
            .handle(
                "owned.event",
                serde_json::to_value(envelope(1, base.revision, tick())).unwrap()
            )
            .is_err()
    );
    assert!(worker.handle("owned.checkpoint", Value::Null).is_err());
    let query = QueryEnvelope {
        api: session::API,
        session: "owned-worker".into(),
        raid: 9,
        expected_view_revision: None,
        query: Query::Cursor,
    };
    assert!(
        worker
            .handle("owned.query", serde_json::to_value(query).unwrap())
            .is_err()
    );
    assert!(
        worker
            .handle(
                "owned.event",
                serde_json::to_value(envelope(1, base.revision, tick())).unwrap()
            )
            .is_err()
    );
    worker.step = step;
    let base = stage(&mut worker, &seed);
    let (mut seq, mut revision) = (0, base.revision);
    exchange(&mut worker, &mut seq, &mut revision, tick());
    assert!(worker.handle("owned.checkpoint", Value::Null).is_ok());
}

#[test]
fn legacy_lifecycle_and_compute_contracts_remain_available_without_owner() {
    let mut worker = worker();
    let manifest = worker.handle("prepare", Value::Null).unwrap();
    assert_eq!(manifest["contracts"]["kernel.owned-state"], 1);
    assert_eq!(manifest["contracts"][session::CONTRACT], session::API);
    assert_eq!(manifest["contracts"][scrolls::CONTRACT], scrolls::API);
    let run = Run::new(17, 9, None);
    let captured = runtime_state::capture(&run).unwrap().into_value();
    worker.handle("restore", captured.clone()).unwrap();
    worker.handle("activate", Value::Null).unwrap();
    let result = worker.handle("apply", json!({"state":captured,"event":{
        "inputs":{},"settings":angel_delve::settings::Settings::default(),"phrasebook":["",[]],"step":false
    }})).unwrap();
    assert!(result.get("state").is_some());
    worker.handle("dispose", Value::Null).unwrap();
    assert!(worker.handle("apply", result).is_err());
}

fn captured() -> Value {
    runtime_state::capture(&Run::new(17, 9, None))
        .unwrap()
        .into_value()
}
fn event_body() -> Value {
    json!({
        "inputs": {},
        "settings": angel_delve::settings::Settings::default(),
        "phrasebook": ["", []],
        "step": false
    })
}
fn delta_apply(worker: &mut TestWorker) -> Result<Value, String> {
    worker.handle("apply", json!({"delta": [], "event": event_body()}))
}
fn whole_apply(worker: &mut TestWorker) -> Result<Value, String> {
    worker.handle("apply", json!({"state": captured(), "event": event_body()}))
}
/// A compute the worker refuses on contract grounds only if the lifecycle let it in.
fn compute(worker: &mut TestWorker) -> Result<Value, String> {
    worker.handle(
        "compute",
        json!({"contract": "no.such.contract", "version": 1, "input": null}),
    )
}
fn refused(result: Result<Value, String>, method: &str) -> bool {
    result.err().as_deref() == Some(&format!("invalid Delve system lifecycle request {method}"))
}

#[test]
fn legacy_lifecycle_allows_each_move_only_in_its_phase() {
    let mut worker = worker();
    // Fresh.
    assert!(refused(worker.handle("activate", Value::Null), "activate"));
    assert!(refused(whole_apply(&mut worker), "apply"));
    assert!(refused(compute(&mut worker), "compute"));
    worker.handle("dispose", Value::Null).unwrap();

    // Staged: a restored candidate is not yet the running implementation.
    worker.handle("restore", captured()).unwrap();
    assert!(refused(whole_apply(&mut worker), "apply"));
    assert!(refused(compute(&mut worker), "compute"));
    worker.handle("restore", captured()).unwrap();

    // Active.
    worker.handle("activate", Value::Null).unwrap();
    assert!(delta_apply(&mut worker).is_ok());
    assert_eq!(
        compute(&mut worker).unwrap_err(),
        "unsupported Delve computation contract"
    );
    // Activating again changes nothing, and neither does re-seeding: the worker
    // stays active and keeps its state.
    worker.handle("activate", Value::Null).unwrap();
    assert!(delta_apply(&mut worker).is_ok());
    worker.handle("restore", captured()).unwrap();
    assert!(delta_apply(&mut worker).is_ok());

    // Dispose goes back to Fresh from anywhere.
    worker.handle("dispose", Value::Null).unwrap();
    assert!(refused(delta_apply(&mut worker), "apply"));
    assert!(refused(worker.handle("activate", Value::Null), "activate"));
    worker.handle("restore", captured()).unwrap();
    worker.handle("activate", Value::Null).unwrap();
    assert!(delta_apply(&mut worker).is_ok());
    worker.handle("dispose", Value::Null).unwrap();
    worker.handle("dispose", Value::Null).unwrap();
}

#[test]
fn an_active_worker_whose_reseed_failed_is_cold_until_the_next_restore() {
    let mut worker = worker();
    worker.handle("restore", captured()).unwrap();
    worker.handle("activate", Value::Null).unwrap();
    assert!(delta_apply(&mut worker).is_ok());

    // The host re-seeds before a delta whenever it holds no state (its own
    // `System::step`): a body that does not restore leaves the worker active
    // but cold, and a delta is refused rather than applied to a base the
    // caller never sent.
    assert!(worker.handle("restore", json!({"not": "a state"})).is_err());
    assert_eq!(
        delta_apply(&mut worker).unwrap_err(),
        "missing resident state"
    );
    // Still the running implementation: whole states apply (and stay cold).
    assert!(whole_apply(&mut worker).is_ok());
    assert_eq!(
        delta_apply(&mut worker).unwrap_err(),
        "missing resident state"
    );
    assert_eq!(
        compute(&mut worker).unwrap_err(),
        "unsupported Delve computation contract"
    );
    // The next restore recovers it without a dispose or a second activate.
    worker.handle("restore", captured()).unwrap();
    assert!(delta_apply(&mut worker).is_ok());
}

#[test]
fn an_owned_session_takes_over_from_the_legacy_lifecycle_without_leaving_it() {
    let mut worker = worker();
    worker.handle("restore", captured()).unwrap();
    worker.handle("activate", Value::Null).unwrap();
    assert!(delta_apply(&mut worker).is_ok());

    // Staging an owned candidate takes the resident state and blocks the
    // legacy re-seed, but whole states still apply.
    let seed = initial();
    worker
        .handle("owned.restore", serde_json::to_value(&seed).unwrap())
        .unwrap();
    assert!(refused(worker.handle("restore", captured()), "restore"));
    assert!(refused(worker.handle("activate", Value::Null), "activate"));
    assert_eq!(
        delta_apply(&mut worker).unwrap_err(),
        "missing resident state"
    );
    assert!(whole_apply(&mut worker).is_ok());

    // Once the owner is active the legacy moves are refused, except compute.
    worker.handle("owned.baseline", Value::Null).unwrap();
    worker.handle("owned.activate", Value::Null).unwrap();
    assert!(refused(whole_apply(&mut worker), "apply"));
    assert!(refused(worker.handle("restore", captured()), "restore"));
    assert_eq!(
        compute(&mut worker).unwrap_err(),
        "unsupported Delve computation contract"
    );
    worker.handle("dispose", Value::Null).unwrap();
    assert!(refused(compute(&mut worker), "compute"));
}

#[test]
fn journal_pressure_is_an_accepted_refusal_and_ack_allows_retry() {
    let mut worker = worker();
    let base = stage(&mut worker, &initial());
    let (mut seq, mut revision) = (0, base.revision);
    let request = |id| {
        OwnedEvent::Commands(Request {
            session: "owned-worker".into(),
            raid: 9,
            id,
            commands: vec![Command::DropOffer { player: 1 }],
        })
    };
    for id in 1..=64 {
        assert!(matches!(
            exchange(&mut worker, &mut seq, &mut revision, request(id)).reply,
            Reply::Commands(_)
        ));
    }
    assert!(matches!(
        exchange(&mut worker, &mut seq, &mut revision, request(65)).reply,
        Reply::Refused(_)
    ));
    assert!(worker.handle("owned.checkpoint", Value::Null).is_ok());
    exchange(
        &mut worker,
        &mut seq,
        &mut revision,
        OwnedEvent::Job(jobs::Event::Acknowledge {
            raid: 9,
            request: 1,
        }),
    );
    assert!(matches!(
        exchange(&mut worker, &mut seq, &mut revision, request(65)).reply,
        Reply::Commands(_)
    ));
    assert!(matches!(
        query(&mut worker, Query::Cursor, revision).reply,
        session::QueryReply::Cursor {
            next_request: 66,
            ..
        }
    ));
}
