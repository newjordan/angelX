use super::*;
use crate::rules::{
    host_commands::{Command, Journal, Request},
    runtime_state,
};
fn world() -> (Run, Journal) {
    let mut run = Run::new(0, 9, Some("Guest"));
    run.descend_for_test();
    assert!(run.can_reforge(1));
    (run, Journal::new("reforge-session".into(), 9))
}
fn claim(run: &mut Run, journal: &mut Journal, player: u32) -> Claim {
    journal
        .claim_reforge(run, player, Part::Defense, "a silver shield".into())
        .unwrap()
}
fn answer(run: &Run, claim: &Claim) -> cards::Card {
    let mut card = run
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
}
#[test]
fn captured_kit_and_reservation_survive_handoff_and_exact_completion_never_replays() {
    let (mut run, mut journal) = world();
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    let claim = claim(&mut run, &mut journal, 1);
    let input = answer(&run, &claim);
    assert!(!run.can_reforge(1));
    assert!(journal.acknowledge(9, claim.request).is_err());
    let checkpoint = runtime_state::capture(&run).unwrap();
    let mut restored = runtime_state::restore(checkpoint.clone()).unwrap();
    let mut saved = journal.capture().unwrap();
    let mut journal = Journal::restore(saved.clone(), &claim.session, 9).unwrap();
    journal.validate_reforge_handoff(&run, &restored).unwrap();
    // Captured brief data is independent of later book/name edits.
    let original = claim.work.original.clone();
    if let Some(mut edited) = original.clone() {
        edited.name = "Later definition".into();
        restored.book.insert(edited);
    }
    restored.players.get_mut(&1).unwrap().name = "Renamed knight".into();
    assert_eq!(claim.work.original, original);
    let accepted = PolicyRequest {
        work: claim.work.clone(),
        card: input.clone(),
    }
    .evaluate()
    .unwrap()
    .unwrap();
    let done = journal
        .complete_reforge(&mut restored, &claim, input.clone(), accepted.clone())
        .unwrap();
    assert!(done.result.is_ok());
    assert_eq!(restored.players[&1].spell_cooldowns[0], 71);
    assert_eq!(restored.players.len(), 2);
    assert!(restored.players[&1].reforge_job.is_none());
    let before = runtime_state::capture(&restored).unwrap().into_value();
    assert_eq!(
        journal
            .complete_reforge(&mut restored, &claim, input.clone(), accepted.clone())
            .unwrap(),
        done
    );
    assert_eq!(
        runtime_state::capture(&restored).unwrap().into_value(),
        before
    );
    let mut changed = accepted;
    changed.name = "Changed policy".into();
    assert!(
        journal
            .complete_reforge(&mut restored, &claim, input, changed)
            .is_err()
    );
    journal.acknowledge_reforge(&claim, &done).unwrap();
    assert!(journal.pending_reforges().unwrap().is_empty());
    saved["schema"] = 3.into();
    assert!(Journal::restore(saved, &claim.session, 9).is_err());
    assert!(
        runtime_state::restore(runtime_state::Checkpoint {
            schema: 3,
            run: checkpoint.run
        })
        .is_err()
    );
}
#[test]
fn changed_kit_floor_and_reused_seat_cannot_receive_the_old_answer() {
    for scenario in 0..3 {
        let (mut run, mut journal) = world();
        let claim = claim(&mut run, &mut journal, 1);
        let card = answer(&run, &claim);
        match scenario {
            0 => run.players.get_mut(&1).unwrap().guard = cards::Guard::Shield,
            1 => run.dungeon.depth += 1,
            _ => {
                let fresh = Run::new(0, 9, None).players.remove(&1).unwrap();
                run.players.insert(1, fresh);
            }
        }
        // Some builtin guard choices are already Shield; always force a new kit.
        if scenario == 0 {
            run.players.get_mut(&1).unwrap().guard_card = None;
        }
        let before = run.sounds.clone();
        let done = journal
            .complete_reforge(&mut run, &claim, card.clone(), card)
            .unwrap();
        assert!(done.result.is_err(), "scenario {scenario}");
        assert_eq!(run.sounds, before);
        assert!(run.book.get(&claim.work.result_id).is_none());
        assert!(run.players[&1].reforge_job.is_none());
        assert_eq!(
            journal
                .cancel_reforge(&mut run, &claim, "later".into())
                .unwrap(),
            done
        );
    }
}
#[test]
fn invalid_candidate_and_unknown_reservation_are_rejected_without_consuming_work() {
    let (mut run, mut journal) = world();
    let claim = claim(&mut run, &mut journal, 1);
    let mut bad = run.clone();
    bad.players.get_mut(&1).unwrap().reforge_job = None;
    assert!(journal.validate_reforge_handoff(&run, &bad).is_err());
    bad.players.get_mut(&1).unwrap().reforge_job = Some("unknown".into());
    assert!(journal.validate_reforge_world(&bad).is_err());
    let card = answer(&run, &claim);
    let mut wrong = card.clone();
    wrong.id = "other-job".into();
    assert!(
        journal
            .complete_reforge(&mut run, &claim, card.clone(), wrong)
            .is_err()
    );
    let mut stale = claim.clone();
    stale.session = "new-session".into();
    assert!(
        journal
            .complete_reforge(&mut run, &stale, card.clone(), card)
            .is_err()
    );
    assert!(journal.reforge_completion(&claim).unwrap().is_none());
    let done = journal
        .cancel_reforge(&mut run, &claim, "cancelled".into())
        .unwrap();
    assert!(run.can_reforge(1));
    let next = journal
        .claim_reforge(&mut run, 1, Part::Defense, "next shield".into())
        .unwrap();
    assert_ne!(next.work.result_id, claim.work.result_id);
    journal.acknowledge_reforge(&claim, &done).unwrap();
    assert_eq!(
        run.players[&1].reforge_job.as_deref(),
        Some(next.work.result_id.as_str())
    );
}
#[test]
fn shared_receipt_backpressure_does_not_reserve_a_knight_until_admitted() {
    let (mut run, mut journal) = world();
    let first = claim(&mut run, &mut journal, 1);
    for _ in 1..64 {
        journal
            .apply(
                &mut run,
                Request {
                    session: "reforge-session".into(),
                    raid: 9,
                    id: journal.next_id().unwrap(),
                    commands: vec![Command::DropOffer { player: 2 }],
                },
            )
            .unwrap();
    }
    assert!(
        journal
            .claim_reforge(&mut run, 2, Part::Offense, "new bow".into())
            .unwrap_err()
            .contains("capacity")
    );
    assert!(run.players[&2].reforge_job.is_none());
    let done = journal
        .cancel_reforge(&mut run, &first, "cancelled".into())
        .unwrap();
    journal.acknowledge_reforge(&first, &done).unwrap();
    journal
        .claim_reforge(&mut run, 2, Part::Offense, "new bow".into())
        .unwrap();
}

#[test]
fn a_full_card_book_rejects_delivery_without_equipping_a_missing_definition() {
    let (mut run, mut journal) = world();
    let claim = claim(&mut run, &mut journal, 1);
    let card = answer(&run, &claim);
    let mut index = 0;
    while run.book.cards.len() < cards::MAX_BOOK {
        let mut filler = card.clone();
        filler.id = format!("filler-{index}");
        index += 1;
        assert!(run.book.insert(filler));
    }
    let original = run.players[&1].guard_card.clone();
    let sounds = run.sounds.clone();
    let done = journal
        .complete_reforge(&mut run, &claim, card.clone(), card)
        .unwrap();
    assert!(done.result.unwrap_err().contains("full"));
    assert_eq!(run.players[&1].guard_card, original);
    assert_eq!(run.sounds, sounds);
    assert!(run.players[&1].reforge_job.is_none());
}

#[test]
fn complete_definition_round_trips_guards_plain_arms_and_forged_runes() {
    for raw in [
        "name Shield\nkind guard\nguard shield\nwhere crypt\n",
        "name Wall\nkind guard\nguard wall width=4 drain=30 empower=20\n",
        "name Bow\nkind arm\narm bow\n",
        "name Silver Bolt\nkind arm\nlook steel arrow\nbolt damage=10 speed=12 every=20 range=30\n",
    ] {
        let card = cards::check("definition", raw).unwrap().card;
        assert_eq!(
            cards::check(&card.id, &card.definition_file())
                .unwrap()
                .card,
            card
        );
    }
    let (mut run, mut journal) = world();
    let claim = journal
        .claim_reforge(&mut run, 1, Part::Offense, "silver arrows".into())
        .unwrap();
    let input = cards::check(
        &claim.work.result_id,
        "name Silver Bolt\nkind arm\nlook steel arrow\nbolt damage=10 speed=12 every=20 range=30\n",
    )
    .unwrap()
    .card;
    let accepted = PolicyRequest {
        work: claim.work.clone(),
        card: input.clone(),
    }
    .evaluate()
    .unwrap()
    .unwrap();
    let done = journal
        .complete_reforge(&mut run, &claim, input, accepted)
        .unwrap();
    assert!(done.result.is_ok());
    assert_eq!(
        run.players[&1].arm.as_deref(),
        Some(claim.work.result_id.as_str())
    );
    assert!(run.players[&1].forged.is_some());
}
