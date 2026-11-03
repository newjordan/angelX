use super::*;
use crate::rules::session::{DefaultPolicies, Envelope, Event, Reply, Session, checkpoint};
use crate::rules::{
    Phase,
    host_commands::{Command, Journal, Request},
    runtime_state, votes,
};

fn context() -> Context {
    Context {
        settings: Default::default(),
        phrasebook: (crate::rules::phrasebook::TEXT.into(), Vec::new()),
        wishes: Arc::new(Vec::new()),
    }
}
fn make_owner(run: Run) -> Session {
    let mut owner = Session::new(run, Journal::new("frame-test".into(), 9), context()).unwrap();
    owner.baseline().unwrap();
    owner
}
fn facts(run: &Run, active: bool) -> Begin {
    Begin {
        network_tick: 17,
        expected_tick: run.tick,
        seats: seats(run),
        controls: Controls {
            active,
            keyboard: true,
            ..Default::default()
        },
        tune: run.tune,
        input_ages: Vec::new(),
        remove: Vec::new(),
        away: Vec::new(),
        arrival: ArrivalState::default(),
        presence: BTreeMap::new(),
        inputs: BTreeMap::new(),
        active_casts: Vec::new(),
    }
}
fn envelope(owner: &Session, event: Event) -> Envelope {
    Envelope {
        api: super::super::API,
        session: "frame-test".into(),
        raid: 9,
        seq: owner.event_seq + 1,
        expected_view_revision: owner.view_revision,
        event,
    }
}
fn send(owner: &mut Session, event: Event) -> crate::rules::session::Frame {
    let ask = envelope(owner, event);
    owner.apply(ask, &mut DefaultPolicies).unwrap()
}
fn finish_input(frame: &Fence) -> Finish {
    Finish {
        begin_seq: frame.begin_seq,
        network_tick: frame.network_tick,
        expected_tick: frame.tick,
    }
}
fn begin_frame(owner: &mut Session, input: Begin) -> Fence {
    match send(owner, Event::BeginFrame(input)).reply {
        Reply::FrameBegun(begun) => begun.frame,
        reply => panic!("{reply:?}"),
    }
}
fn finish_frame(owner: &mut Session, fence: &Fence) -> Outputs {
    match send(owner, Event::FinishFrame(finish_input(fence))).reply {
        Reply::FrameFinished(output) => output,
        reply => panic!("{reply:?}"),
    }
}
fn private(run: &Run) -> serde_json::Value {
    runtime_state::capture(run).unwrap().into_value()
}

// Frozen legacy DelveServer control prelude, retained independently here so
// the new phase split is checked against the behavior being extracted.
fn legacy_controls(run: &mut Run, controls: Controls) {
    if let Some(seat) = run.playtest.seats.get_mut(&2)
        && seat.couch != controls.couch
    {
        seat.couch = controls.couch;
    }
    if run.wishing != controls.wishing {
        run.wishing = controls.wishing;
    }
    let continuous = run.at_home_now() || run.room().kind == RoomKind::Kingdom;
    let companion = run.players.get(&2).filter(|h| h.hp > 0).map(|h| (h.x, h.y));
    let outside = run.exits.outside.contains_key(&1);
    if let Some(host) = run.players.get_mut(&1).filter(|_| !controls.friend_one) {
        let stone =
            (controls.hosted && !controls.keyboard) || host.vigil || host.away || controls.watching;
        if host.stone
            && !continuous
            && !outside
            && !stone
            && let Some((x, y)) = companion
        {
            host.x = x;
            host.y = y;
        }
        if host.stone != stone {
            host.stone = stone;
        }
    }
}
fn legacy_note(state: &mut ArrivalState, run: &Run, tick: u32) {
    let room = RoomKey {
        raid: run.raid_id,
        depth: run.dungeon.depth,
        room: run.at,
    };
    if state.room.is_some_and(|old| old != room) {
        state.arrival = Some(Arrival {
            tick,
            until: tick.wrapping_add(3 * crate::rules::HZ),
        });
    }
    state.room = Some(room);
}
fn legacy_hold(
    state: &mut ArrivalState,
    run: &Run,
    tick: u32,
    presence: &BTreeMap<u32, Presence>,
) -> Hold {
    if !run.active() {
        state.arrival = None;
        return Hold::Open;
    }
    let living = run.players.values().any(|hero| hero.hp > 0);
    if living && !run.players.values().any(|hero| hero.hp > 0 && !hero.stone) {
        return Hold::Empty;
    }
    if let Some(arrival) = state.arrival {
        let waiting = presence.iter().any(|(&id, here)| {
            here.playing
                && here.applied < arrival.tick
                && run.players.get(&id).is_some_and(|hero| !hero.away)
        });
        if waiting && tick < arrival.until {
            return Hold::Arriving;
        }
        state.arrival = None;
    }
    Hold::Open
}

fn legacy_begin(run: &mut Run, input: &Begin) -> (Vec<Signal>, ArrivalState) {
    for age in &input.input_ages {
        run.latch_input_age.insert(age.seat.player, age.ticks);
    }
    legacy_controls(run, input.controls);
    if run.tune != input.tune {
        run.tune = input.tune;
    }
    run.set_absent(1, input.controls.watching && !input.controls.friend_one);
    let mut notices = Vec::new();
    for removed in &input.remove {
        let name = run.players[&removed.player].name.clone();
        run.leave(removed.player);
        notices.push(Signal::Notice {
            hear: Hear::All,
            text: format!("{name} left the party"),
        });
    }
    for away in &input.away {
        run.set_away(away.seat.player, away.away);
    }
    let mut arrival = input.arrival.clone();
    legacy_note(&mut arrival, run, input.network_tick);
    run.hold(legacy_hold(
        &mut arrival,
        run,
        input.network_tick,
        &input.presence,
    ));
    let inputs = input.inputs.iter().map(|(&id, v)| (id, v.input)).collect();
    if input.controls.active {
        for (&id, v) in &input.inputs {
            if let Some(command) = v.input.vote {
                run.vote_command(id, command);
            }
        }
        run.step(&inputs);
    } else {
        for (&id, v) in &input.inputs {
            if v.input.valid() && v.input.leave_activity {
                run.leave_activity(id);
            }
        }
    }
    if input.controls.active {
        legacy_note(&mut arrival, run, input.network_tick);
    }
    (notices, arrival)
}
fn legacy_finish(
    run: &mut Run,
    input: &Begin,
    context: &Context,
    fence: Fence,
    prelude: Vec<Signal>,
) -> Outputs {
    for (&id, v) in &input.inputs {
        if v.input.valid() && v.input.again {
            run.ask_again(id);
        }
    }
    let again_due = run.again_due();
    let cues = std::mem::take(&mut run.cues);
    let mut postlude = Vec::new();
    for name in std::mem::take(&mut run.sounds) {
        postlude.push(Signal::Sound {
            hear: Hear::Party,
            name: name.into(),
        });
    }
    for (player, name) in std::mem::take(&mut run.seat_sounds) {
        postlude.push(Signal::Sound {
            hear: Hear::Knight(player),
            name: name.into(),
        });
    }
    for text in std::mem::take(&mut run.notices) {
        postlude.push(Signal::Notice {
            hear: Hear::All,
            text,
        });
    }
    for (player, text) in std::mem::take(&mut run.seat_notices) {
        postlude.push(Signal::Notice {
            hear: Hear::Knight(player),
            text,
        });
    }
    let feel = std::mem::take(&mut run.feel.lines);
    if run.kingdom.wishes != context.wishes {
        run.kingdom.wishes = context.wishes.clone();
        run.dungeon.world_revision = run.dungeon.world_revision.wrapping_add(1);
        run.raise_kingdom();
    }
    let mended = (!drawable(run)).then(|| run.mend().into_iter().map(str::to_owned).collect());
    Outputs {
        frame: fence,
        prelude,
        postlude,
        cues,
        feel,
        again_due,
        mended,
    }
}

#[test]
fn frame_split_matches_legacy_controls_presence_hold_and_complete_output_order() {
    for active in [false, true] {
        for hold in [Hold::Open, Hold::Empty, Hold::Arriving] {
            for watching in [false, true] {
                let mut run = Run::new(17, 9, Some("Friend"));
                run.descend_for_test();
                run.players.get_mut(&1).unwrap().stone = true;
                run.cues.push("chorus".into());
                run.sounds.push("hit");
                run.seat_sounds.push((1, "coin"));
                run.notices.push("all notice".into());
                run.seat_notices.push((1, "private notice".into()));
                run.feel.lines.push("ledger".into());
                let mut legacy = run.clone();
                let mut owner = make_owner(run);
                let mut input = facts(&owner.run, active);
                match hold {
                    Hold::Open => {}
                    Hold::Empty => {
                        input.controls.hosted = true;
                        input.controls.keyboard = false;
                    }
                    Hold::Arriving => {
                        input.arrival = ArrivalState {
                            room: Some(RoomKey {
                                raid: 9,
                                depth: owner.run.dungeon.depth,
                                room: owner.run.at,
                            }),
                            arrival: Some(Arrival {
                                tick: input.network_tick,
                                until: input.network_tick.wrapping_add(3 * crate::rules::HZ),
                            }),
                        };
                        input.presence.insert(
                            1,
                            Presence {
                                playing: true,
                                applied: input.network_tick - 1,
                            },
                        );
                    }
                }
                input.controls.watching = watching;
                input.controls.wishing = true;
                input.controls.couch = true;
                input.input_ages.push(InputAge {
                    seat: Seat {
                        player: 1,
                        epoch: owner.run.seat_epoch(1),
                    },
                    ticks: 27,
                });
                input.remove.push(Seat {
                    player: 2,
                    epoch: owner.run.seat_epoch(2),
                });
                input.away.push(Away {
                    seat: Seat {
                        player: 1,
                        epoch: owner.run.seat_epoch(1),
                    },
                    away: watching,
                });
                input.inputs.insert(
                    1,
                    SeatInput {
                        epoch: owner.run.seat_epoch(1),
                        input: Input {
                            aim_x: 1,
                            fire: active,
                            ..Default::default()
                        },
                    },
                );
                let (early, arrival) = legacy_begin(&mut legacy, &input);
                let fence = begin_frame(&mut owner, input.clone());
                assert_eq!(private(&owner.run), private(&legacy));
                assert_eq!(owner.pending_frame.as_ref().unwrap().arrival, arrival);
                // Native ability effects belong here. A cue/sound stands in for a
                // resolved ability result, not a model/native callback or second queue.
                owner.run.sounds.push("boom");
                legacy.sounds.push("boom");
                let expected =
                    legacy_finish(&mut legacy, &input, &owner.context, fence.clone(), early);
                let actual = finish_frame(&mut owner, &fence);
                assert_eq!(actual, expected);
                assert_eq!(private(&owner.run), private(&legacy));
                assert_eq!(
                    actual.prelude,
                    vec![Signal::Notice {
                        hear: Hear::All,
                        text: "Friend left the party".into()
                    }]
                );
                assert!(matches!(
                    &actual.postlude[0],
                    Signal::Sound {
                        hear: Hear::Party,
                        ..
                    }
                ));
                assert!(owner.pending_frame.is_none());
            }
        }
    }
}

#[test]
fn frame_active_votes_precede_step_and_paused_leave_is_taken_without_voting_or_step() {
    let mut run = Run::new(3, 9, None);
    let mut journal = Journal::new("frame-test".into(), 9);
    journal
        .prepare_votes(&mut run, Default::default(), Default::default(), false)
        .unwrap();
    let mut owner = Session::new(run, journal, context()).unwrap();
    owner.baseline().unwrap();
    let mut input = facts(&owner.run, true);
    input.inputs.insert(
        1,
        SeatInput {
            epoch: owner.run.seat_epoch(1),
            input: Input {
                vote: Some(votes::Command {
                    serial: 1,
                    action: votes::Action::Toggle,
                }),
                ..Default::default()
            },
        },
    );
    struct VoteFirst;
    impl Policies for VoteFirst {
        fn step(&mut self, run: &mut Run, _: &BTreeMap<u32, Input>) {
            assert!(run.playtest.open(1));
        }
    }
    let ask = envelope(&owner, Event::BeginFrame(input));
    assert!(matches!(
        owner.apply(ask, &mut VoteFirst).unwrap().reply,
        Reply::FrameBegun(_)
    ));
    let fence = owner.pending_frame.as_ref().unwrap().frame.clone();
    finish_frame(&mut owner, &fence);

    let mut run = Run::at_home(41, 9, None, Default::default(), Default::default());
    run.walk_kingdom(Some(crate::rules::bannerpeak::ENTRY));
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(
        1,
        Input {
            interact: true,
            ..Default::default()
        },
    )]));
    assert!(run.can_leave_activity(1));
    let mut owner = make_owner(run);
    let before = owner.run.tick;
    let mut input = facts(&owner.run, false);
    input.inputs.insert(
        1,
        SeatInput {
            epoch: owner.run.seat_epoch(1),
            input: Input {
                leave_activity: true,
                vote: Some(votes::Command {
                    serial: 5,
                    action: votes::Action::Toggle,
                }),
                ..Default::default()
            },
        },
    );
    let fence = begin_frame(&mut owner, input);
    assert_eq!(owner.run.tick, before);
    assert!(owner.run.bannerpeak.is_none());
    assert!(!owner.run.playtest.open(1));
    finish_frame(&mut owner, &fence);
}

#[test]
fn frame_again_and_drains_commit_once_with_exact_finish_replay() {
    let mut run = Run::new(3, 9, None);
    run.phase = Phase::Wiped;
    run.cues.clear();
    run.sounds.clear();
    run.seat_sounds.clear();
    run.cues.push("once".into());
    run.sounds.push("hit");
    let mut owner = make_owner(run);
    let mut input = facts(&owner.run, false);
    input.inputs.insert(
        1,
        SeatInput {
            epoch: owner.run.seat_epoch(1),
            input: Input {
                again: true,
                ..Default::default()
            },
        },
    );
    let fence = begin_frame(&mut owner, input);
    assert!(owner.run.again.is_none());
    let ask = envelope(&owner, Event::FinishFrame(finish_input(&fence)));
    let frame = owner.apply(ask.clone(), &mut DefaultPolicies).unwrap();
    let output = match &frame.reply {
        Reply::FrameFinished(output) => output,
        _ => panic!(),
    };
    assert!(output.again_due);
    assert_eq!(output.cues, vec!["once"]);
    assert_eq!(
        output
            .postlude
            .iter()
            .filter(|signal| matches!(signal, Signal::Sound { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        vec![Signal::Sound {
            hear: Hear::Party,
            name: "hit".into()
        }]
    );
    assert!(owner.run.cues.is_empty() && owner.run.sounds.is_empty());
    let after = owner.checkpoint().unwrap().encode().unwrap();
    assert_eq!(owner.apply(ask, &mut DefaultPolicies).unwrap(), frame);
    assert_eq!(owner.checkpoint().unwrap().encode().unwrap(), after);
    assert!(matches!(
        send(&mut owner, Event::FinishFrame(finish_input(&fence))).reply,
        Reply::Refused(_)
    ));
    assert_eq!(
        owner.run.again.as_ref().unwrap().left,
        crate::rules::again::AGAIN_WAIT - 1
    );
}

#[test]
fn frame_pending_phase_blocks_unrelated_writes_and_fences_seat_reincarnations() {
    let mut owner = make_owner(Run::new(3, 9, Some("Friend")));
    let input = facts(&owner.run, false);
    let fence = begin_frame(&mut owner, input.clone());
    let before = private(&owner.run);
    for event in [
        Event::BeginFrame(input),
        Event::Tick {
            inputs: BTreeMap::new(),
            step: true,
        },
        Event::ContextChanged(context()),
        Event::Commands(Request {
            session: "frame-test".into(),
            raid: 9,
            id: 1,
            commands: vec![Command::DropOffer { player: 1 }],
        }),
        Event::Job(crate::rules::session::jobs::Event::ClaimBrew),
    ] {
        assert!(matches!(send(&mut owner, event).reply, Reply::Refused(_)));
        assert_eq!(private(&owner.run), before);
    }
    let mut wrong = finish_input(&fence);
    wrong.network_tick += 1;
    assert!(matches!(
        send(&mut owner, Event::FinishFrame(wrong)).reply,
        Reply::Refused(_)
    ));
    assert!(owner.pending_frame.is_some());
    finish_frame(&mut owner, &fence);
    let stale = facts(&owner.run, false);
    owner.run.leave(2);
    owner.run.join_seat(2, "Replacement");
    assert!(matches!(
        send(&mut owner, Event::BeginFrame(stale)).reply,
        Reply::Refused(_)
    ));
    assert!(owner.pending_frame.is_none());
}

#[test]
fn frame_pending_checkpoint_requires_exact_marker_and_replays_finish_semantically() {
    let mut owner = make_owner(Run::new(3, 9, Some("Friend")));
    owner.run.cues.clear();
    owner.run.cues.push("retained cue".into());
    let mut begin = facts(&owner.run, false);
    begin.remove.push(Seat {
        player: 2,
        epoch: owner.run.seat_epoch(2),
    });
    let fence = begin_frame(&mut owner, begin);
    let saved = owner.checkpoint().unwrap();
    assert_eq!(saved.pending_frame.as_ref().unwrap().frame, fence);
    let mut missing = serde_json::to_value(&saved).unwrap();
    missing.as_object_mut().unwrap().remove("pending_frame");
    assert!(checkpoint::decode(&serde_json::to_vec(&missing).unwrap()).is_err());
    let mut changed = saved.clone();
    changed
        .pending_frame
        .as_mut()
        .unwrap()
        .frame
        .seats
        .insert(1, 999);
    assert!(Session::restore(changed).is_err());
    let mut changed = saved.clone();
    changed.pending_frame = None;
    assert!(checkpoint::validate_handoff(&saved, &changed).is_err());
    let finish = envelope(&owner, Event::FinishFrame(finish_input(&fence)));
    let accepted = owner.apply(finish.clone(), &mut DefaultPolicies).unwrap();
    let mut recovered = Session::restore(saved).unwrap();
    let replay = recovered.replay(finish, &mut DefaultPolicies).unwrap();
    assert_eq!(replay.reply, accepted.reply);
    assert!(
        matches!(&replay.reply,Reply::FrameFinished(output) if output.prelude.len()==1&&output.cues==vec!["retained cue"])
    );
    assert_eq!(
        recovered.checkpoint().unwrap().encode().unwrap(),
        owner.checkpoint().unwrap().encode().unwrap()
    );
    let before = recovered.checkpoint().unwrap().encode().unwrap();
    recovered.baseline().unwrap();
    assert_eq!(recovered.checkpoint().unwrap().encode().unwrap(), before);
}

#[test]
fn frame_hold_actual_tick_and_unchanged_wishes_keep_exact_issued_state() {
    let mut owner = make_owner(Run::new(3, 9, None));
    let mut input = facts(&owner.run, true);
    input.arrival = ArrivalState {
        room: Some(RoomKey {
            raid: 9,
            depth: owner.run.dungeon.depth,
            room: owner.run.at,
        }),
        arrival: Some(Arrival {
            tick: 17,
            until: 17 + 3 * crate::rules::HZ,
        }),
    };
    input.presence.insert(
        1,
        Presence {
            playing: true,
            applied: 16,
        },
    );
    struct Frozen;
    impl Policies for Frozen {
        fn step(&mut self, _: &mut Run, _: &BTreeMap<u32, Input>) {}
    }
    let ask = envelope(&owner, Event::BeginFrame(input));
    let before = owner.run.tick;
    let fence = match owner.apply(ask, &mut Frozen).unwrap().reply {
        Reply::FrameBegun(begun) => begun.frame,
        _ => panic!(),
    };
    assert_eq!(fence.tick, before);
    assert_eq!(owner.run.held, Hold::Arriving);
    let revision = owner.run.dungeon.world_revision;
    finish_frame(&mut owner, &fence);
    assert_eq!(owner.run.dungeon.world_revision, revision);
}

#[test]
fn frame_native_proofs_keep_original_slot_but_fence_session_and_retire_removed_gameplay() {
    use crate::rules::{abilities as port, session::abilities::ActiveCast};
    let mut run = Run::new(3, 9, Some("Friend"));
    let original = port::Slot {
        module: "module".into(),
        generation: 1,
        ability: "spark".into(),
        name: "Spark".into(),
        mana: 2.0,
        cooldown: 90,
        icon: String::new(),
    };
    let mut replacement = original.clone();
    replacement.generation = 2;
    run.players.get_mut(&2).unwrap().module_spells[0] = Some(replacement);
    let proof = ActiveCast {
        session: "frame-test".into(),
        request: port::Request {
            raid: 9,
            epoch: run.seat_epoch(2),
            definition: original.clone(),
            player: 2,
            slot: 0,
            module: original.module.clone(),
            generation: 1,
            ability: original.ability.clone(),
            tick: run.tick,
        },
        origin: port::Origin {
            module: original.module,
            generation: 1,
            cast: "old-cast".into(),
        },
        started: true,
    };
    let mut owner = make_owner(run);
    let mut input = facts(&owner.run, false);
    input.active_casts.push(proof.clone());
    let mut wrong = input.clone();
    wrong.active_casts[0].session = "replaced-session".into();
    let before = private(&owner.run);
    assert!(matches!(
        send(&mut owner, Event::BeginFrame(wrong)).reply,
        Reply::Refused(_)
    ));
    assert_eq!(private(&owner.run), before);
    let fence = begin_frame(&mut owner, input.clone());
    assert_eq!(
        owner.pending_frame.as_ref().unwrap().active_casts,
        vec![proof.clone()]
    );
    finish_frame(&mut owner, &fence);
    input.expected_tick = owner.run.tick;
    input.remove.push(Seat {
        player: 2,
        epoch: owner.run.seat_epoch(2),
    });
    let fence = begin_frame(&mut owner, input);
    assert!(
        owner
            .pending_frame
            .as_ref()
            .unwrap()
            .active_casts
            .is_empty(),
        "removed seat has no gameplay permission; native resource lifetime is external"
    );
    finish_frame(&mut owner, &fence);
}

#[test]
fn frame_standing_wish_context_is_applied_after_drains_and_only_raises_on_change() {
    let run = Run::new(3, 9, None);
    let mut owner = make_owner(run);
    let mut changed = context();
    changed.wishes = Arc::new(vec![crate::realm::Wish {
        id: "shrine".into(),
        name: "Wayside Shrine".into(),
        words: String::new(),
        by: "Friend".into(),
        status: crate::realm::Status::Built,
        price: Default::default(),
        art: vec![
            "..GG..".into(),
            ".GHHG.".into(),
            ".GHHG.".into(),
            "GGGGGG".into(),
        ],
        near: "chapel".into(),
        paid: Vec::new(),
        raised: 1,
        lot: None,
    }]);
    send(&mut owner, Event::ContextChanged(changed.clone()));
    let before = owner.run.dungeon.world_revision;
    let input = facts(&owner.run, false);
    let fence = begin_frame(&mut owner, input);
    assert!(
        owner.run.kingdom.wishes.is_empty(),
        "postlude owns standing-wish adoption"
    );
    finish_frame(&mut owner, &fence);
    assert_eq!(owner.run.kingdom.wishes, changed.wishes);
    assert_ne!(owner.run.dungeon.world_revision, before);
    let stable = owner.run.dungeon.world_revision;
    let input = facts(&owner.run, false);
    let fence = begin_frame(&mut owner, input);
    finish_frame(&mut owner, &fence);
    assert_eq!(owner.run.dungeon.world_revision, stable);
    assert!(Arc::ptr_eq(
        &owner.run.kingdom.wishes,
        &owner.context.wishes
    ));
}

#[test]
fn frame_paid_cast_proof_survives_owner_death_during_step() {
    use crate::rules::{abilities as port, session::abilities::ActiveCast};
    let run = Run::new(3, 9, Some("Friend"));
    let definition = port::Slot {
        module: "module".into(),
        generation: 1,
        ability: "spark".into(),
        name: "Spark".into(),
        mana: 2.0,
        cooldown: 90,
        icon: String::new(),
    };
    let proof = ActiveCast {
        session: "frame-test".into(),
        request: port::Request {
            raid: 9,
            epoch: run.seat_epoch(2),
            definition: definition.clone(),
            player: 2,
            slot: 0,
            module: definition.module.clone(),
            generation: 1,
            ability: definition.ability.clone(),
            tick: run.tick,
        },
        origin: port::Origin {
            module: definition.module,
            generation: 1,
            cast: "paid-before-death".into(),
        },
        started: true,
    };
    let mut owner = make_owner(run);
    let mut input = facts(&owner.run, true);
    input.active_casts.push(proof.clone());
    struct Death;
    impl Policies for Death {
        fn step(&mut self, run: &mut Run, _: &BTreeMap<u32, Input>) {
            run.players.get_mut(&2).unwrap().hp = 0;
        }
    }
    let ask = envelope(&owner, Event::BeginFrame(input));
    let fence = match owner.apply(ask, &mut Death).unwrap().reply {
        Reply::FrameBegun(begun) => begun.frame,
        reply => panic!("{reply:?}"),
    };
    assert_eq!(
        owner.pending_frame.as_ref().unwrap().active_casts,
        vec![proof]
    );
    assert!(
        !owner.is_poisoned(),
        "death does not terminate already-paid native cast ownership"
    );
    assert!(owner.checkpoint().unwrap().pending_frame.is_some());
    finish_frame(&mut owner, &fence);
}

#[test]
fn frame_arrival_is_derived_after_removal_changes_room_and_tracks_poststep_room() {
    let mut run = Run::new(7, 9, Some("Friend"));
    run.descend_for_test();
    let from = run.at;
    let back = if from == 0 { 1 } else { 0 };
    assert!(back < run.dungeon.rooms.len());
    run.exits.outside.insert(
        1,
        crate::rules::exits::Entry {
            at: back,
            party: run
                .players
                .iter()
                .map(|(&id, hero)| (id, (hero.x, hero.y)))
                .collect(),
        },
    );
    let mut legacy = run.clone();
    let mut owner = make_owner(run);
    let mut input = facts(&owner.run, false);
    input.arrival.room = Some(RoomKey {
        raid: 9,
        depth: owner.run.dungeon.depth,
        room: from,
    });
    input.remove.push(Seat {
        player: 2,
        epoch: owner.run.seat_epoch(2),
    });
    input.presence.insert(
        1,
        Presence {
            playing: true,
            applied: 0,
        },
    );
    let (_, expected) = legacy_begin(&mut legacy, &input);
    let fence = begin_frame(&mut owner, input);
    assert_eq!(owner.run.at, back);
    assert_eq!(owner.pending_frame.as_ref().unwrap().arrival, expected);
    assert_eq!(
        expected.room,
        Some(RoomKey {
            raid: 9,
            depth: owner.run.dungeon.depth,
            room: back
        })
    );
    assert_eq!(
        expected.arrival,
        Some(Arrival {
            tick: 17,
            until: 17 + 3 * crate::rules::HZ
        })
    );
    finish_frame(&mut owner, &fence);
    let mut input = facts(&owner.run, true);
    input.arrival = expected;
    struct MoveRoom;
    impl Policies for MoveRoom {
        fn step(&mut self, run: &mut Run, _: &BTreeMap<u32, Input>) {
            let next = if run.at == 0 { 1 } else { 0 };
            run.enter_for_test(next);
        }
    }
    let ask = envelope(&owner, Event::BeginFrame(input));
    let begun = match owner.apply(ask, &mut MoveRoom).unwrap().reply {
        Reply::FrameBegun(begun) => begun,
        reply => panic!("{reply:?}"),
    };
    assert_eq!(begun.arrival.room.unwrap().room, owner.run.at);
    assert_eq!(
        begun.arrival.arrival,
        Some(Arrival {
            tick: 17,
            until: 17 + 3 * crate::rules::HZ
        })
    );
}
#[test]
fn frame_arrival_preserves_unsigned_wrap_deadline_comparison() {
    let mut owner = make_owner(Run::new(3, 9, None));
    let mut input = facts(&owner.run, false);
    input.network_tick = u32::MAX - 1;
    input.arrival = ArrivalState {
        room: Some(RoomKey {
            raid: 9,
            depth: owner.run.dungeon.depth,
            room: owner.run.at,
        }),
        arrival: Some(Arrival {
            tick: u32::MAX - 1,
            until: (u32::MAX - 1).wrapping_add(3 * crate::rules::HZ),
        }),
    };
    input.presence.insert(
        1,
        Presence {
            playing: true,
            applied: 0,
        },
    );
    let mut legacy = owner.run.clone();
    let (_, arrival) = legacy_begin(&mut legacy, &input);
    let fence = begin_frame(&mut owner, input);
    assert_eq!(owner.run.held, Hold::Open);
    assert_eq!(owner.pending_frame.as_ref().unwrap().arrival, arrival);
    assert!(arrival.arrival.is_none());
    finish_frame(&mut owner, &fence);
}
