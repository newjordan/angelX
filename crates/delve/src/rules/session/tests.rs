use super::*;
use crate::rules::{
    host_commands::{Command, Journal, Request},
    runtime_state,
};

fn context(text: &str) -> Context {
    Context {
        settings: Default::default(),
        phrasebook: (text.into(), vec![format!("source {text}")]),
        wishes: Arc::new(Vec::new()),
    }
}
fn owner() -> Session {
    Session::new(
        Run::new(3, 9, Some("friend")),
        Journal::new("session-test".into(), 9),
        context(""),
    )
    .unwrap()
}
fn envelope(owner: &Session, event: Event) -> Envelope {
    Envelope {
        api: API,
        session: owner.journal.session().into(),
        raid: owner.run.raid_id,
        seq: owner.event_seq + 1,
        expected_view_revision: owner.view_revision,
        event,
    }
}
fn tick() -> Event {
    Event::Tick {
        inputs: BTreeMap::new(),
        step: true,
    }
}
fn send(owner: &mut Session, event: Event) -> Frame {
    let input = envelope(owner, event);
    owner.apply(input, &mut DefaultPolicies).unwrap()
}

fn query(owner: &Session, query: Query) -> Result<QueryReply, String> {
    owner
        .query(QueryEnvelope {
            api: API,
            session: owner.journal.session().into(),
            raid: owner.run.raid_id,
            expected_view_revision: Some(owner.view_revision),
            query,
        })
        .map(|frame| frame.reply)
}

#[test]
fn session_step_parity_with_four_seats_and_exact_journal_commands() {
    let mut run = Run::new(17, 9, Some("second"));
    run.join_seat(3, "third");
    run.join_seat(4, "fourth");
    let mut expected = run.clone();
    let mut journal = Journal::new("session-test".into(), 9);
    let context = context("");
    let mut owner =
        Session::new(run, Journal::new("session-test".into(), 9), context.clone()).unwrap();
    owner.baseline().unwrap();
    for i in 0..180 {
        let inputs = (1..=4)
            .map(|seat| {
                (
                    seat,
                    Input {
                        move_x: if i % 2 == 0 { 1 } else { -1 },
                        aim_x: 1,
                        fire: i % 3 == 0,
                        ..Default::default()
                    },
                )
            })
            .collect();
        {
            let _settings = crate::settings::scoped(context.settings);
            let _book = phrasebook::scoped(owner.parsed.clone());
            expected.step(&inputs);
        }
        send(&mut owner, Event::Tick { inputs, step: true });
        if i % 30 == 0 {
            let request = Request {
                session: "session-test".into(),
                raid: 9,
                id: journal.next_id().unwrap(),
                commands: vec![Command::DropOffer { player: 1 }],
            };
            let expected_receipt = journal.apply(&mut expected, request.clone()).unwrap();
            assert_eq!(
                send(&mut owner, Event::Commands(request)).reply,
                Reply::Commands(expected_receipt)
            );
        }
    }
    assert_eq!(
        runtime_state::capture(&expected).unwrap().into_value(),
        runtime_state::capture(&owner.run).unwrap().into_value()
    );
    assert_eq!(journal.capture().unwrap(), owner.journal.capture().unwrap());
}

#[test]
fn session_exact_last_exchange_replay_precedes_stale_revision_checks() {
    let mut owner = owner();
    let baseline = owner.baseline().unwrap();
    let ask = envelope(&owner, tick());
    let first = owner.apply(ask.clone(), &mut DefaultPolicies).unwrap();
    assert_eq!(first.view.base_revision, baseline.revision);
    let before = owner.checkpoint().unwrap().encode().unwrap();
    assert_eq!(
        owner.apply(ask.clone(), &mut DefaultPolicies).unwrap(),
        first
    );
    let mut altered = ask.clone();
    altered.event = Event::Tick {
        inputs: BTreeMap::new(),
        step: false,
    };
    assert!(owner.apply(altered, &mut DefaultPolicies).is_err());
    assert_eq!(owner.checkpoint().unwrap().encode().unwrap(), before);
    send(&mut owner, tick());
    assert!(
        owner.apply(ask, &mut DefaultPolicies).is_err(),
        "only the last exchange may replay"
    );
    assert!(!owner.is_poisoned());
}

#[test]
fn session_stale_foreign_and_oversized_envelopes_do_not_mutate_or_poison() {
    let mut owner = owner();
    owner.baseline().unwrap();
    let original = owner.checkpoint().unwrap().encode().unwrap();
    for fault in 0..5 {
        let mut ask = envelope(&owner, tick());
        match fault {
            0 => ask.api += 1,
            1 => ask.session = "other".into(),
            2 => ask.raid += 1,
            3 => ask.seq += 1,
            _ => ask.expected_view_revision += 1,
        }
        assert!(owner.apply(ask, &mut DefaultPolicies).is_err());
    }
    let mut big = context("");
    big.phrasebook.0 = "x".repeat(MAX_EVENT_BYTES + 1);
    let ask = envelope(&owner, Event::ContextChanged(big));
    assert!(owner.apply(ask, &mut DefaultPolicies).is_err());
    assert_eq!(owner.checkpoint().unwrap().encode().unwrap(), original);
    assert!(!owner.is_poisoned());
}

#[test]
fn session_receipt_pressure_is_an_accepted_refusal_then_ticks_continue() {
    let mut owner = owner();
    owner.baseline().unwrap();
    let mut refused = false;
    for request in 1..=100 {
        let frame = send(
            &mut owner,
            Event::Commands(Request {
                session: "session-test".into(),
                raid: 9,
                id: request,
                commands: vec![Command::DropOffer { player: 1 }],
            }),
        );
        if matches!(frame.reply, Reply::Refused(_)) {
            refused = true;
            break;
        }
    }
    assert!(refused, "bounded Journal must apply backpressure");
    assert!(!owner.is_poisoned());
    let before = owner.run.tick;
    send(&mut owner, tick());
    assert_eq!(owner.run.tick, before + 1);
}

#[test]
fn session_postmutation_invalid_state_or_frame_failure_poison_all_ports() {
    struct Invalid;
    impl Policies for Invalid {
        fn step(&mut self, run: &mut Run, _: &BTreeMap<u32, Input>) {
            run.tick = MAX_TICK + 1;
        }
    }
    let mut owner = owner();
    owner.baseline().unwrap();
    let ask = envelope(&owner, tick());
    assert!(owner.apply(ask, &mut Invalid).is_err());
    assert!(owner.is_poisoned());
    assert!(query(&owner, Query::Cursor).is_err());
    assert!(owner.checkpoint().is_err());
    let ask = envelope(&owner, tick());
    assert!(owner.apply(ask, &mut DefaultPolicies).is_err());

    let mut owner = super::tests::owner();
    owner.view_limits = view::Limits::new(view::MAX_BASELINE_BYTES, 1).unwrap();
    owner.baseline().unwrap();
    let ask = envelope(&owner, tick());
    assert!(owner.apply(ask, &mut DefaultPolicies).is_err());
    assert!(
        owner.is_poisoned(),
        "mutated world cannot survive an unpublishable reply"
    );
}

#[test]
fn session_context_scopes_alternate_and_nest_without_source_reinstall_on_tick() {
    struct Inspect {
        source: String,
        errors: Vec<String>,
        parsed: Option<Arc<Vec<phrasebook::Wish>>>,
    }
    impl Policies for Inspect {
        fn step(&mut self, _: &mut Run, _: &BTreeMap<u32, Input>) {
            assert_eq!(
                phrasebook::snapshot(),
                (self.source.clone(), self.errors.clone())
            );
            let book = phrasebook::book();
            if let Some(previous) = &self.parsed {
                assert!(Arc::ptr_eq(previous, &book));
            }
            self.parsed = Some(book);
        }
    }
    let mut a = owner();
    let mut b = owner();
    a.baseline().unwrap();
    b.baseline().unwrap();
    let source_a = context("# A");
    let source_b = context("# B");
    send(&mut a, Event::ContextChanged(source_a.clone()));
    send(&mut b, Event::ContextChanged(source_b.clone()));
    let mut pa = Inspect {
        source: source_a.phrasebook.0,
        errors: source_a.phrasebook.1,
        parsed: None,
    };
    let mut pb = Inspect {
        source: source_b.phrasebook.0,
        errors: source_b.phrasebook.1,
        parsed: None,
    };
    for _ in 0..4 {
        let ask = envelope(&a, tick());
        a.apply(ask, &mut pa).unwrap();
        let ask = envelope(&b, tick());
        b.apply(ask, &mut pb).unwrap();
    }
    struct Nested<'a> {
        inner: &'a mut Session,
        expect: String,
    }
    impl Policies for Nested<'_> {
        fn step(&mut self, _: &mut Run, _: &BTreeMap<u32, Input>) {
            assert_eq!(phrasebook::snapshot().0, self.expect);
            send(self.inner, tick());
            assert_eq!(phrasebook::snapshot().0, self.expect);
        }
    }
    let ask = envelope(&a, tick());
    a.apply(
        ask,
        &mut Nested {
            inner: &mut b,
            expect: "# A".into(),
        },
    )
    .unwrap();
}

#[test]
fn session_parallel_source_contexts_remain_thread_local() {
    let threads: Vec<_> = ["# thread A", "# thread B"]
        .into_iter()
        .map(|source| {
            std::thread::spawn(move || {
                let mut owner = owner();
                owner.baseline().unwrap();
                let context = context(source);
                send(&mut owner, Event::ContextChanged(context.clone()));
                struct Check(String);
                impl Policies for Check {
                    fn step(&mut self, _: &mut Run, _: &BTreeMap<u32, Input>) {
                        assert_eq!(phrasebook::snapshot().0, self.0);
                    }
                }
                for _ in 0..8 {
                    let ask = envelope(&owner, tick());
                    owner.apply(ask, &mut Check(source.into())).unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}

#[test]
fn session_tick_wire_has_no_checkpoint_or_source_and_context_equal_is_noop() {
    let mut owner = owner();
    owner.baseline().unwrap();
    let cached = owner.parsed.clone();
    let context = owner.context.clone();
    assert_eq!(
        send(&mut owner, Event::ContextChanged(context)).reply,
        Reply::Changed(false)
    );
    assert!(Arc::ptr_eq(&cached, &owner.parsed));
    let ask = envelope(&owner, tick());
    let value = serde_json::to_value(&ask).unwrap();
    let object = value["event"]["value"].as_object().unwrap();
    assert_eq!(object.len(), 2);
    assert!(object.contains_key("inputs") && object.contains_key("step"));
    let frame = owner.apply(ask, &mut DefaultPolicies).unwrap();
    let mut receiver =
        view::Receiver::new("session-test".into(), 9, view::Limits::default()).unwrap();
    assert!(
        receiver.delta(&frame.view).is_err(),
        "delta is not a full baseline/checkpoint"
    );
}

#[test]
fn session_pending_brew_restore_exact_completion_retry_and_ack_use_existing_journal() {
    let mut run = Run::new(0, 9, None);
    run.descend_for_test();
    let placeholder = crate::rules::cauldron::brewed_card(
        crate::rules::cauldron::recipe(
            crate::rules::cauldron::Essence::Garden,
            crate::rules::cauldron::Essence::Garden,
        ),
        crate::rules::cauldron::Tier::Common,
    );
    run.players.get_mut(&1).unwrap().spells[0] = Some(placeholder.id.clone());
    run.brewing.push(crate::rules::cauldron::Brewing {
        knight: 1,
        raid: 9,
        depth: run.dungeon.depth,
        cards: ["cabbage-storm".into(), "hedge-maze".into()],
        placeholder: placeholder.id.clone(),
    });
    run.book.insert(placeholder);
    let mut owner = Session::new(run, Journal::new("session-test".into(), 9), context("")).unwrap();
    owner.baseline().unwrap();
    assert_eq!(
        query(&owner, Query::BrewReady).unwrap(),
        QueryReply::BrewReady(true)
    );
    let claim = match send(&mut owner, Event::Job(jobs::Event::ClaimBrew)).reply {
        Reply::Job(jobs::Reply::Brew(Some(claim))) => claim,
        reply => panic!("{reply:?}"),
    };
    assert_eq!(
        query(&owner, Query::BrewReady).unwrap(),
        QueryReply::BrewReady(false)
    );
    let checkpoint = owner.checkpoint().unwrap();
    let mut owner = Session::restore(checkpoint).unwrap();
    owner.baseline().unwrap();
    assert_eq!(
        query(&owner, Query::Job(jobs::Query::PendingBrews)).unwrap(),
        QueryReply::Job(jobs::Reply::Brews(vec![claim.clone()]))
    );
    struct Brew(usize);
    impl Policies for Brew {
        fn brew(
            &mut self,
            request: &crate::rules::brew_jobs::PolicyRequest,
        ) -> Result<crate::rules::brew_jobs::PolicyResult, String> {
            self.0 += 1;
            Ok(Ok(crate::rules::cards::check(
                &request.work.result_id,
                "name Retained Brew\nkind spell\ncooldown 4\ncast bolt\n",
            )
            .unwrap()
            .card))
        }
    }
    let input = crate::rules::cards::check(
        "native-answer",
        "name Retained Brew\nkind spell\ncooldown 4\ncast bolt\n",
    )
    .unwrap()
    .card;
    let event = Event::Job(jobs::Event::CompleteBrew {
        claim: claim.clone(),
        input,
    });
    let mut policy = Brew(0);
    let ask = envelope(&owner, event.clone());
    let frame = owner.apply(ask, &mut policy).unwrap();
    let completed = match frame.reply {
        Reply::Job(jobs::Reply::BrewCompleted(Some(done))) => done,
        reply => panic!("{reply:?}"),
    };
    assert!(completed.result.is_ok());
    let before = runtime_state::capture(&owner.run).unwrap().into_value();
    let ask = envelope(&owner, event);
    assert_eq!(
        owner.apply(ask, &mut policy).unwrap().reply,
        Reply::Job(jobs::Reply::BrewCompleted(Some(completed.clone())))
    );
    assert_eq!(
        policy.0, 1,
        "retained completion never invokes policy again"
    );
    assert_eq!(
        runtime_state::capture(&owner.run).unwrap().into_value(),
        before
    );
    send(
        &mut owner,
        Event::Job(jobs::Event::AcknowledgeBrew { claim, completed }),
    );
    assert_eq!(
        query(&owner, Query::Job(jobs::Query::PendingBrews)).unwrap(),
        QueryReply::Job(jobs::Reply::Brews(Vec::new()))
    );
}

#[test]
fn session_query_envelope_fences_revision_and_ownership_without_changing_state() {
    let mut owner = owner();
    owner.baseline().unwrap();
    let before = owner.checkpoint().unwrap().encode().unwrap();
    let valid = QueryEnvelope {
        api: API,
        session: "session-test".into(),
        raid: 9,
        expected_view_revision: Some(owner.view_revision),
        query: Query::Cursor,
    };
    for fault in 0..4 {
        let mut query = valid.clone();
        match fault {
            0 => query.api += 1,
            1 => query.session = "other".into(),
            2 => query.raid += 1,
            _ => query.expected_view_revision = Some(owner.view_revision + 1),
        }
        assert!(owner.query(query).is_err());
    }
    assert!(owner.query(valid).is_ok());
    assert_eq!(owner.checkpoint().unwrap().encode().unwrap(), before);
}

#[test]
fn session_caught_policy_unwind_restores_outer_context_and_poison_owner() {
    struct Panic;
    impl Policies for Panic {
        fn step(&mut self, run: &mut Run, _: &BTreeMap<u32, Input>) {
            run.score += 1;
            panic!("after partial mutation");
        }
    }
    let outer = phrasebook::prepare_scoped("# outer".into(), vec!["outer diagnostic".into()]);
    let _scope = phrasebook::scoped(outer);
    let _settings = crate::settings::scoped(crate::settings::Settings {
        feel_ledger: true,
        open_floors: true,
    });
    let mut owner = owner();
    owner.baseline().unwrap();
    let ask = envelope(&owner, tick());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        owner.apply(ask, &mut Panic)
    }));
    assert!(result.is_err());
    assert!(owner.is_poisoned());
    assert_eq!(
        phrasebook::snapshot(),
        ("# outer".into(), vec!["outer diagnostic".into()])
    );
    assert_eq!(
        crate::settings::get(),
        crate::settings::Settings {
            feel_ledger: true,
            open_floors: true
        }
    );
    assert!(query(&owner, Query::Cursor).is_err());
}

#[test]
fn session_context_settings_only_change_reuses_parsed_source_cache() {
    let mut owner = owner();
    owner.baseline().unwrap();
    let parsed = owner.parsed.clone();
    let mut changed = owner.context.clone();
    changed.settings.feel_ledger = !changed.settings.feel_ledger;
    assert_eq!(
        send(&mut owner, Event::ContextChanged(changed)).reply,
        Reply::Changed(true)
    );
    assert!(Arc::ptr_eq(&parsed, &owner.parsed));
}

#[test]
fn session_rejects_native_overlay_ownership_and_preflight_counter_overflow() {
    let mut run = Run::new(3, 9, None);
    run.module_visuals.revision = 1;
    assert!(Session::new(run, Journal::new("session-test".into(), 9), context("")).is_err());
    let mut owner = owner();
    owner.baseline().unwrap();
    owner.event_seq = u64::MAX;
    let ask = Envelope {
        api: API,
        session: "session-test".into(),
        raid: 9,
        seq: u64::MAX,
        expected_view_revision: owner.view_revision,
        event: tick(),
    };
    let tick_before = owner.run.tick;
    assert!(owner.apply(ask, &mut DefaultPolicies).is_err());
    assert!(!owner.is_poisoned());
    assert_eq!(owner.run.tick, tick_before);
}

#[test]
fn session_staged_replay_accepted_tail_preserves_private_checkpoint_and_carried_revision() {
    let mut run = Run::new(0, 9, None);
    run.descend_for_test();
    let placeholder = crate::rules::cauldron::brewed_card(
        crate::rules::cauldron::recipe(
            crate::rules::cauldron::Essence::Garden,
            crate::rules::cauldron::Essence::Garden,
        ),
        crate::rules::cauldron::Tier::Common,
    );
    run.players.get_mut(&1).unwrap().spells[0] = Some(placeholder.id.clone());
    run.brewing.push(crate::rules::cauldron::Brewing {
        knight: 1,
        raid: 9,
        depth: run.dungeon.depth,
        cards: ["cabbage-storm".into(), "hedge-maze".into()],
        placeholder: placeholder.id.clone(),
    });
    run.book.insert(placeholder);
    let mut uninterrupted =
        Session::new(run, Journal::new("session-test".into(), 9), context("")).unwrap();
    uninterrupted.baseline().unwrap();
    let claim = match send(&mut uninterrupted, Event::Job(jobs::Event::ClaimBrew)).reply {
        Reply::Job(jobs::Reply::Brew(Some(claim))) => claim,
        reply => panic!("{reply:?}"),
    };
    // Supervisor seeds recovery only after bootstrap baseline. The exact old
    // executable + source context is pinned while this accepted tail replays.
    let seed = uninterrupted.checkpoint().unwrap();
    assert!(seed.view_revision > 0);
    let input = crate::rules::cards::check(
        "native-answer",
        "name Replayed Brew\nkind spell\ncooldown 4\ncast bolt\n",
    )
    .unwrap()
    .card;
    let complete = Event::Job(jobs::Event::CompleteBrew {
        claim: claim.clone(),
        input,
    });
    struct SameImage {
        policy_calls: usize,
    }
    impl Policies for SameImage {
        fn brew(
            &mut self,
            request: &crate::rules::brew_jobs::PolicyRequest,
        ) -> Result<crate::rules::brew_jobs::PolicyResult, String> {
            self.policy_calls += 1;
            Ok(Ok(crate::rules::cards::check(
                &request.work.result_id,
                "name Replayed Brew\nkind spell\ncooldown 4\ncast bolt\n",
            )
            .unwrap()
            .card))
        }
    }
    let mut original_policy = SameImage { policy_calls: 0 };
    let mut tail = Vec::new();
    let mut completed = None;
    for index in 0..160 {
        let event = match index {
            0 | 1 => complete.clone(),
            40 => {
                let mut changed = context("# replay source context");
                changed.settings.feel_ledger = true;
                Event::ContextChanged(changed)
            }
            120 => Event::Job(jobs::Event::AcknowledgeBrew {
                claim: claim.clone(),
                completed: completed.clone().unwrap(),
            }),
            _ => tick(),
        };
        let ask = envelope(&uninterrupted, event);
        let frame = uninterrupted
            .apply(ask.clone(), &mut original_policy)
            .unwrap();
        if let Reply::Job(jobs::Reply::BrewCompleted(Some(done))) = &frame.reply {
            completed = Some(done.clone());
        }
        tail.push((
            ask,
            ReplayFrame {
                api: frame.api,
                session: frame.session,
                raid: frame.raid,
                seq: frame.seq,
                view_revision: frame.view.revision,
                reply: frame.reply,
            },
        ));
    }
    assert_eq!(original_policy.policy_calls, 1);
    let accepted = uninterrupted.checkpoint().unwrap();
    let mut recovered = Session::restore(seed).unwrap();
    let mut recovered_policy = SameImage { policy_calls: 0 };
    for (input, witness) in &tail {
        assert_eq!(
            &recovered
                .replay(input.clone(), &mut recovered_policy)
                .unwrap(),
            witness
        );
        assert!(
            recovered.last.is_none(),
            "semantic replay must not invent old dictionary frames"
        );
        assert!(!recovered.baseline_ready);
    }
    assert_eq!(
        recovered_policy.policy_calls, 1,
        "replay invokes pure policy only, retained completion retry bypasses it"
    );
    let recovered_before_baseline = recovered.checkpoint().unwrap();
    assert_eq!(
        accepted.encode().unwrap(),
        recovered_before_baseline.encode().unwrap()
    );
    let baseline = recovered.baseline().unwrap();
    assert_eq!(baseline.revision, accepted.view_revision);
    assert_eq!(baseline.base_revision, 0);
    assert_eq!(
        recovered.checkpoint().unwrap().encode().unwrap(),
        recovered_before_baseline.encode().unwrap(),
        "resync baseline preserves exact carried checkpoint"
    );
    let mut receiver =
        view::Receiver::new("session-test".into(), 9, view::Limits::default()).unwrap();
    assert_eq!(
        receiver.baseline(&baseline).unwrap().revision(),
        accepted.view_revision
    );
    // New-owner activation does not reconstruct the last live exchange cache.
    let last = tail.last().unwrap().0.clone();
    assert!(
        recovered
            .apply(last.clone(), &mut recovered_policy)
            .is_err()
    );
    assert!(recovered.replay(last, &mut recovered_policy).is_err());
    assert!(
        !recovered.is_poisoned(),
        "active-owner replay rejection precedes mutation"
    );
    assert!(recovered.baseline().is_err());
}

#[test]
fn session_staged_replay_bad_envelopes_or_state_poison_and_do_not_invent_duplicate_cache() {
    let mut active = owner();
    active.baseline().unwrap();
    let seed = active.checkpoint().unwrap();
    let ask = envelope(&active, tick());
    let expected = active.apply(ask.clone(), &mut DefaultPolicies).unwrap();
    let mut staged = Session::restore(seed.clone()).unwrap();
    let replayed = staged.replay(ask.clone(), &mut DefaultPolicies).unwrap();
    assert_eq!(replayed.reply, expected.reply);
    assert_eq!(replayed.view_revision, expected.view.revision);
    assert!(staged.last.is_none());
    assert!(
        staged.replay(ask.clone(), &mut DefaultPolicies).is_err(),
        "duplicate staged inputs are not silently applied or cached"
    );
    assert!(staged.is_poisoned());
    for fault in 0..3 {
        let mut staged = Session::restore(seed.clone()).unwrap();
        let mut invalid = ask.clone();
        match fault {
            0 => invalid.seq += 1,
            1 => invalid.expected_view_revision += 1,
            _ => {
                invalid.event = Event::Tick {
                    inputs: BTreeMap::from([(0, Input::default())]),
                    step: true,
                }
            }
        }
        assert!(staged.replay(invalid, &mut DefaultPolicies).is_err());
        assert!(staged.is_poisoned());
        assert!(staged.checkpoint().is_err());
        assert!(query(&staged, Query::Cursor).is_err());
    }
    struct Invalid;
    impl Policies for Invalid {
        fn step(&mut self, run: &mut Run, _: &BTreeMap<u32, Input>) {
            run.tick = MAX_TICK + 1;
        }
    }
    let mut staged = Session::restore(seed).unwrap();
    assert!(staged.replay(ask, &mut Invalid).is_err());
    assert!(staged.is_poisoned());
}

#[test]
fn session_staged_replay_logical_refusal_is_a_bounded_accepted_semantic_reply() {
    let mut original = owner();
    original.baseline().unwrap();
    let seed = original.checkpoint().unwrap();
    let ask = envelope(
        &original,
        Event::Job(jobs::Event::Acknowledge {
            raid: 9,
            request: 999,
        }),
    );
    let accepted = original.apply(ask.clone(), &mut DefaultPolicies).unwrap();
    assert!(matches!(&accepted.reply, Reply::Refused(_)));
    let mut recovered = Session::restore(seed).unwrap();
    let replayed = recovered.replay(ask, &mut DefaultPolicies).unwrap();
    assert_eq!(replayed.reply, accepted.reply);
    assert!(!recovered.is_poisoned());
    assert_eq!(
        recovered.checkpoint().unwrap().encode().unwrap(),
        original.checkpoint().unwrap().encode().unwrap()
    );
}

fn paint_card() -> super::super::cards::Card {
    super::super::cards::check(
        "paint-capture",
        "name Paint Capture\nkind spell\ncooldown 5\nnova 10\nart\n00\n00\ndraw shot\n00\n00\n",
    )
    .unwrap()
    .card
}

#[test]
fn authored_paint_default_skips_changed_definition_and_preserves_book() {
    let mut owner = owner();
    let original = paint_card();
    owner.run.book.insert(original.clone());
    owner.baseline().unwrap();
    let mut authored = original.clone();
    authored.cooldown += 1;
    let event = Event::AuthoredCards {
        cards: vec![authored],
    };
    assert_eq!(send(&mut owner, event).reply, Reply::Changed(false));
    assert_eq!(owner.run.book.get(&original.id), Some(&original));
}

#[test]
fn authored_paint_uses_override_retains_definition_and_exact_retry() {
    struct Paint(usize);
    impl Policies for Paint {
        fn paint(&mut self, request: &card_paint::Request) -> Result<card_paint::Decision, String> {
            self.0 += 1;
            Ok(card_paint::Decision {
                accepted: request
                    .candidates
                    .iter()
                    .map(|c| c.current.id.clone())
                    .collect(),
            })
        }
    }
    let mut owner = owner();
    let original = paint_card();
    owner.run.book.insert(original.clone());
    owner.baseline().unwrap();
    let mut authored = original.clone();
    authored.cooldown += 1;
    authored.art = vec!["11".into(), "11".into()];
    authored.looks.shot.as_mut().unwrap().frames = vec![authored.art.clone()];
    let mut expected = original.clone();
    expected.art = authored.art.clone();
    expected.looks = authored.looks.clone();
    let input = envelope(
        &owner,
        Event::AuthoredCards {
            cards: vec![authored],
        },
    );
    let mut policy = Paint(0);
    let replay_seed = owner.checkpoint().unwrap();
    let result = owner.apply(input.clone(), &mut policy).unwrap();
    assert_eq!(result.reply, Reply::Changed(true));
    assert_eq!(owner.run.book.get(&original.id), Some(&expected));
    let checkpoint = owner.checkpoint().unwrap();
    assert_eq!(owner.apply(input.clone(), &mut policy).unwrap(), result);
    assert_eq!(policy.0, 1);
    assert_eq!(
        owner.checkpoint().unwrap().encode().unwrap(),
        checkpoint.encode().unwrap()
    );
    let mut restored = Session::restore(replay_seed).unwrap();
    assert_eq!(
        restored.replay(input, &mut policy).unwrap().reply,
        result.reply
    );
    assert_eq!(
        restored.checkpoint().unwrap().encode().unwrap(),
        checkpoint.encode().unwrap()
    );
    assert_eq!(policy.0, 2);
}

#[test]
fn authored_paint_invalid_policy_output_refuses_without_catalogue_or_receipt_mutation() {
    struct Invalid;
    impl Policies for Invalid {
        fn paint(&mut self, _: &card_paint::Request) -> Result<card_paint::Decision, String> {
            Ok(card_paint::Decision {
                accepted: vec!["uncaptured".into()],
            })
        }
    }
    let mut owner = owner();
    let original = paint_card();
    owner.run.book.insert(original.clone());
    owner.baseline().unwrap();
    let journal = owner.journal.capture().unwrap();
    let input = envelope(
        &owner,
        Event::AuthoredCards {
            cards: vec![original.clone()],
        },
    );
    assert!(matches!(
        owner.apply(input, &mut Invalid).unwrap().reply,
        Reply::Refused(_)
    ));
    assert_eq!(owner.run.book.get(&original.id), Some(&original));
    assert_eq!(owner.journal.capture().unwrap(), journal);
}
