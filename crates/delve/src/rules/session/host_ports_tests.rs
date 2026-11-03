use super::*;
use crate::rules::{
    home_settlement::{HomeContext, HomePolicyRequest},
    host_commands::Journal,
    runtime_state,
};
fn context() -> Context {
    Context {
        settings: Default::default(),
        phrasebook: (phrasebook::TEXT.into(), Vec::new()),
        wishes: Arc::new(Vec::new()),
    }
}
fn owner() -> Session {
    let mut owner = Session::new(
        Run::new(3, 9, Some("Friend")),
        Journal::new("host-test".into(), 9),
        context(),
    )
    .unwrap();
    owner.baseline().unwrap();
    owner
}
fn input(owner: &Session, event: Event) -> Envelope {
    Envelope {
        api: API,
        session: owner.journal.session().into(),
        raid: owner.run.raid_id,
        seq: owner.event_seq + 1,
        expected_view_revision: owner.view_revision,
        event,
    }
}
fn send(owner: &mut Session, event: Event) -> Frame {
    let ask = input(owner, event);
    owner.apply(ask, &mut DefaultPolicies).unwrap()
}
fn query(owner: &Session, query: Query) -> QueryReply {
    owner
        .query(QueryEnvelope {
            api: API,
            session: owner.journal.session().into(),
            raid: owner.run.raid_id,
            expected_view_revision: Some(owner.view_revision),
            query,
        })
        .unwrap()
        .reply
}
fn begin(owner: &Session) -> frame::Begin {
    frame::Begin {
        network_tick: 10,
        expected_tick: owner.run.tick,
        seats: frame::seats(&owner.run),
        controls: frame::Controls {
            keyboard: true,
            ..Default::default()
        },
        tune: owner.run.tune,
        input_ages: vec![],
        remove: vec![],
        away: vec![],
        arrival: Default::default(),
        presence: BTreeMap::new(),
        inputs: BTreeMap::new(),
        active_casts: vec![],
    }
}

#[test]
fn controls_changed_is_immediate_while_paused_and_identical_is_noop_or_phase_refused() {
    let mut owner = owner();
    let before = owner.run.tick;
    let change = frame::ControlChange {
        expected_tick: before,
        seats: frame::seats(&owner.run),
        controls: frame::Controls {
            hosted: true,
            wishing: true,
            couch: true,
            ..Default::default()
        },
    };
    assert_eq!(
        send(&mut owner, Event::ControlsChanged(change.clone())).reply,
        Reply::Changed(true)
    );
    assert!(owner.run.players[&1].stone && owner.run.wishing);
    assert_eq!(owner.run.tick, before);
    let state = runtime_state::capture(&owner.run).unwrap().into_value();
    assert_eq!(
        send(&mut owner, Event::ControlsChanged(change.clone())).reply,
        Reply::Changed(false)
    );
    assert_eq!(
        runtime_state::capture(&owner.run).unwrap().into_value(),
        state
    );
    let mut stale = change.clone();
    stale.expected_tick += 1;
    assert!(matches!(
        send(&mut owner, Event::ControlsChanged(stale)).reply,
        Reply::Refused(_)
    ));
    let facts = begin(&owner);
    send(&mut owner, Event::BeginFrame(facts));
    assert!(matches!(
        send(&mut owner, Event::ControlsChanged(change)).reply,
        Reply::Refused(_)
    ));
}
#[test]
fn campaign_captured_source_cannot_overwrite_replacement_or_session() {
    let mut owner = owner();
    let zero = owner.run.campaign;
    let mut value = serde_json::to_value(zero).unwrap();
    value["counts"][0] = 1.into();
    let first = serde_json::from_value(value.clone()).unwrap();
    value["counts"][0] = 2.into();
    let second = serde_json::from_value(value).unwrap();
    let captured = input(
        &owner,
        Event::CampaignChanged {
            observed: zero,
            campaign: first,
        },
    );
    assert_eq!(
        owner
            .apply(captured.clone(), &mut DefaultPolicies)
            .unwrap()
            .reply,
        Reply::Changed(true)
    );
    assert!(matches!(
        send(
            &mut owner,
            Event::CampaignChanged {
                observed: zero,
                campaign: second
            }
        )
        .reply,
        Reply::Refused(_)
    ));
    assert_eq!(owner.run.campaign, first);
    assert_eq!(
        send(
            &mut owner,
            Event::CampaignChanged {
                observed: first,
                campaign: first
            }
        )
        .reply,
        Reply::Changed(false)
    );
    let mut replacement = Session::new(
        owner.run.clone(),
        Journal::new("new-session".into(), 9),
        context(),
    )
    .unwrap();
    replacement.baseline().unwrap();
    let before = replacement.checkpoint().unwrap().encode().unwrap();
    assert!(replacement.apply(captured, &mut DefaultPolicies).is_err());
    assert_eq!(replacement.checkpoint().unwrap().encode().unwrap(), before);
}
fn agrees(journal: &Journal, run: &Run, context: Option<&HomeContext>) {
    let expected = journal.has_home_work(run, context).unwrap();
    let mut clone =
        Journal::restore(journal.capture().unwrap(), journal.session(), run.raid_id).unwrap();
    let mut run = run.clone();
    let actual = match context {
        Some(context) => clone.claim_home_with_context(&mut run, context),
        None => clone.claim_home(&mut run),
    }
    .unwrap()
    .is_some();
    assert_eq!(expected, actual);
}
#[test]
fn home_ready_matches_claim_predicate_pending_completion_ack_and_context_only_changes() {
    let mut owner = owner();
    assert_eq!(
        query(&owner, Query::HomeReady { context: None }),
        QueryReply::HomeReady(true)
    );
    agrees(&owner.journal, &owner.run, None);
    let claim = owner.journal.claim_home(&mut owner.run).unwrap().unwrap();
    assert_eq!(
        query(&owner, Query::HomeReady { context: None }),
        QueryReply::HomeReady(true)
    );
    let request = HomePolicyRequest {
        context: HomeContext {
            home: owner.run.home.clone(),
            treasury: owner.run.treasury.clone(),
            reclaimed: Default::default(),
            raids_won: 0,
        },
        transfer: claim.transfer.clone(),
        host: "Host".into(),
        unsaved: false,
    };
    let result = request.evaluate().unwrap();
    let committed = result.context();
    owner
        .journal
        .complete_home(&mut owner.run, &claim, result)
        .unwrap();
    let completed = owner.journal.home_completion(&claim).unwrap().unwrap();
    assert_eq!(
        query(
            &owner,
            Query::HomeReady {
                context: Some(committed.clone())
            }
        ),
        QueryReply::HomeReady(true)
    );
    agrees(&owner.journal, &owner.run, Some(&committed));
    owner.journal.acknowledge_home(&claim, &completed).unwrap();
    assert_eq!(
        query(&owner, Query::HomeReady { context: None }),
        QueryReply::HomeReady(false)
    );
    agrees(&owner.journal, &owner.run, None);
    agrees(&owner.journal, &owner.run, Some(&committed));
    let mut changed = committed;
    changed.raids_won += 1;
    assert_eq!(
        query(
            &owner,
            Query::HomeReady {
                context: Some(changed.clone())
            }
        ),
        QueryReply::HomeReady(true)
    );
    agrees(&owner.journal, &owner.run, Some(&changed));
    assert_eq!(
        query(&owner, Query::HomeReady { context: None }),
        QueryReply::HomeReady(false)
    );
}
#[test]
fn bounded_queued_wish_prefix_and_votes_ready_queries_preserve_exact_sources() {
    let mut run = Run::new(3, 9, None);
    let asked = crate::rules::genie::Asked {
        cosmetic: false,
        cosmetic_name: String::new(),
        fairy: false,
        star: false,
        knight: 1,
        words: "retained source".into(),
        steps: 1,
        great: false,
        fills: false,
        making: None,
        target: Default::default(),
    };
    run.asked = vec![
        asked.clone(),
        crate::rules::genie::Asked {
            cosmetic: false,
            cosmetic_name: String::new(),
            fairy: false,
            words: "second source".into(),
            ..asked.clone()
        },
    ];
    let mut owner = Session::new(run, Journal::new("host-test".into(), 9), context()).unwrap();
    owner.baseline().unwrap();
    let before = owner.checkpoint().unwrap().encode().unwrap();
    assert_eq!(
        query(&owner, Query::QueuedWishes { limit: 1 }),
        QueryReply::QueuedWishes(vec![asked])
    );
    assert_eq!(
        query(&owner, Query::VotesReady),
        QueryReply::VotesReady(false)
    );
    assert_eq!(owner.checkpoint().unwrap().encode().unwrap(), before);
    for limit in [0, 65] {
        assert!(
            owner
                .query(QueryEnvelope {
                    api: API,
                    session: "host-test".into(),
                    raid: 9,
                    expected_view_revision: Some(owner.view_revision),
                    query: Query::QueuedWishes { limit }
                })
                .is_err()
        );
    }
    send(
        &mut owner,
        Event::Job(jobs::Event::PrepareVotes {
            records: Default::default(),
            seen: Default::default(),
            couch: false,
        }),
    );
    assert_eq!(
        query(&owner, Query::VotesReady),
        QueryReply::VotesReady(true)
    );
    let facts = begin(&owner);
    let begun = match send(&mut owner, Event::BeginFrame(facts)).reply {
        Reply::FrameBegun(begun) => begun,
        reply => panic!("{reply:?}"),
    };
    assert_eq!(
        query(&owner, Query::FramePhase),
        QueryReply::FramePhase(Some(frame::PhaseInfo {
            begun: begun.clone(),
            abilities_applied: false
        }))
    );
    let restored = Session::restore(owner.checkpoint().unwrap()).unwrap();
    assert_eq!(
        query(&restored, Query::FramePhase),
        query(&owner, Query::FramePhase)
    );
    send(
        &mut owner,
        Event::FinishFrame(frame::Finish {
            begin_seq: begun.frame.begin_seq,
            network_tick: begun.frame.network_tick,
            expected_tick: begun.frame.tick,
        }),
    );
    assert_eq!(
        query(&owner, Query::FramePhase),
        QueryReply::FramePhase(None)
    );
}
#[test]
fn host_queries_keep_existing_envelope_bounds_and_poison_fences() {
    let mut owner = owner();
    let before = owner.checkpoint().unwrap().encode().unwrap();
    let mut context = HomeContext {
        home: Default::default(),
        treasury: Default::default(),
        reclaimed: Default::default(),
        raids_won: 0,
    };
    context.reclaimed.insert("x".repeat(MAX_EVENT_BYTES + 1), 1);
    assert!(
        owner
            .query(QueryEnvelope {
                api: API,
                session: "host-test".into(),
                raid: 9,
                expected_view_revision: Some(owner.view_revision),
                query: Query::HomeReady {
                    context: Some(context)
                }
            })
            .is_err()
    );
    assert_eq!(owner.checkpoint().unwrap().encode().unwrap(), before);
    owner.poisoned = true;
    assert!(
        owner
            .query(QueryEnvelope {
                api: API,
                session: "host-test".into(),
                raid: 9,
                expected_view_revision: Some(owner.view_revision),
                query: Query::FramePhase
            })
            .is_err()
    );
}

#[test]
fn squire_claim_replay_keeps_paid_company_and_deferred_descent() {
    use crate::rules::{home, world, RoomKind, TILE_UNITS, squire_company::company::RECRUIT_PLATES};
    use crate::realm::{Spoil, Spoils};
    let mut treasury = Spoils::default();
    treasury.add(Spoil::Gold, 10_000);
    let mut run = Run::at_home(61, 9, None, home::Home::default(), treasury.clone());
    run.players.get_mut(&1).unwrap().name = "Ada".into();
    let yard = world::room_of(&run.dungeon, RoomKind::SquireBarracks).unwrap();
    run.arrive(yard, (12.0, 10.0));
    let (x, y, w, h) = RECRUIT_PLATES[0];
    let hero = run.players.get_mut(&1).unwrap();
    hero.x = (x as f32 + w as f32 / 2.0) * TILE_UNITS;
    hero.y = (y as f32 + h as f32 / 2.0) * TILE_UNITS;
    run.tick_squire_company(&BTreeMap::new());
    run.tick_squire_company(&BTreeMap::from([(1, Input { interact: true, ..Default::default() })]));
    let purchase = *run.peddled.first().expect("yard purchase");
    let price = run.peddle_price(purchase).unwrap();
    let mut journal = Journal::new("squire-replay".into(), run.raid_id);
    let claim = journal.claim_home(&mut run).unwrap().unwrap();
    assert!(run.squire_company_claimed);
    assert!(!run.squire_company_reserved);
    run.squire_pending_delves = 1;
    let outbox = crate::rules::home_settlement::HomeOutbox::capture(&run).unwrap();
    let mut durable: Run = serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
    outbox.install(&mut durable).unwrap();
    assert!(durable.squire_company_claimed);
    assert_eq!(durable.squire_pending_delves, 1);
    let state = runtime_state::capture(&run).unwrap();
    let mut restored = runtime_state::restore(state).unwrap();
    let mut restored_journal = Journal::restore(journal.capture().unwrap(), journal.session(), run.raid_id).unwrap();
    restored_journal.validate_home_world(&restored).unwrap();
    assert_eq!(restored_journal.claim_home(&mut restored).unwrap(), Some(claim.clone()));
    let request = HomePolicyRequest { context: HomeContext { home: claim.transfer.view.clone(), treasury, reclaimed: Default::default(), raids_won: 0 }, transfer: claim.transfer.clone(), host: "Host".into(), unsaved: false };
    let result = request.evaluate().unwrap();
    assert_eq!(result.treasury.get(Spoil::Gold), 10_000 - price);
    assert_eq!(result.home.squire_company.rosters["Ada"].squires.len(), 1);
    restored_journal.complete_home(&mut restored, &claim, result.clone()).unwrap();
    let done = restored_journal.home_completion(&claim).unwrap().unwrap();
    assert_eq!(restored.home.squire_company.rosters["Ada"].squires.len(), 1);
    assert_eq!(restored.home.squire_company.delves, result.home.squire_company.delves + 1);
    assert!(!restored.squire_company_reserved);
    assert!(!restored.squire_company_claimed);
    restored_journal.sync_home(&mut restored, result.home.clone(), result.treasury.clone()).unwrap();
    assert_eq!(restored.home.squire_company.delves, result.home.squire_company.delves + 1);
    restored_journal.acknowledge_home(&claim, &done).unwrap();
    assert!(restored_journal.has_home_work(&restored, Some(&result.context())).unwrap());
}

#[test]
fn squire_first_sync_never_overwrites_newer_durable_company() {
    use crate::rules::home;
    let mut run = Run::at_home(61, 9, None, home::Home::default(), Default::default());
    let mut journal = Journal::new("old-company".into(), run.raid_id);
    let mut durable = run.home.clone();
    durable.squire_company.board = 2;
    journal.sync_home(&mut run, durable.clone(), Default::default()).unwrap();
    assert_eq!(run.home.squire_company.board, 2);
    let claim = journal.claim_home(&mut run).unwrap().unwrap();
    assert_eq!(claim.transfer.view.squire_company, durable.squire_company);
}
