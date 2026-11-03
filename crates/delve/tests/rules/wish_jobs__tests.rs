use super::*;
use crate::rules::{host_commands::Journal, runtime_state, dead_wishes::Target};
fn fixture() -> (Run, Journal, Asked) {
    let mut run = Run::new(0, 9, Some("Guest"));
    let ask = Asked {
        cosmetic: false,
        cosmetic_name: String::new(),
        fairy: false,
        star: false,
        knight: 1,
        words: "golden grace".into(),
        steps: 1,
        great: false,
        fills: true,
        making: None,
        target: Target::You,
    };
    run.asked.push(ask.clone());
    (run, Journal::new("wish-session".into(), 9), ask)
}
fn answer() -> Wish {
    super::super::phrasebook::read(
        "wish captured-grace | Captured Grace\nsay golden grace\ntier damage 10 :: Grace\n",
    )
    .remove(0)
}
#[test]
fn exact_claim_definition_completion_and_ack_survive_handoff() {
    let (mut run, mut journal, ask) = fixture();
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    let claim = journal
        .claim_wish(&mut run, ask, "Original maker".into())
        .unwrap();
    assert!(run.asked.is_empty());
    assert!(journal.acknowledge(9, claim.request).is_err());
    let checkpoint = runtime_state::capture(&run).unwrap();
    let mut next = runtime_state::restore(checkpoint).unwrap();
    let mut journal = Journal::restore(journal.capture().unwrap(), &claim.session, 9).unwrap();
    journal.validate_wish_handoff(&run, &next).unwrap();
    let input = answer();
    let mut accepted = input.clone();
    accepted.name = "Replacement Grace".into();
    let done = journal
        .complete_wish(
            &mut next,
            &claim,
            Some(input.clone()),
            Some(accepted.clone()),
            true,
            None,
        )
        .unwrap();
    assert!(matches!(&done.outcome, Outcome::Granted(name) if name == "Replacement Grace I"));
    assert_eq!(next.players[&1].spell_cooldowns[0], 71);
    assert_eq!(next.players.len(), 2);
    assert!(next.players[&1].wish_jobs.is_empty());
    assert_eq!(
        next.book.get(&accepted.card_id(0)).unwrap().by,
        "Original maker"
    );
    let before = runtime_state::capture(&next).unwrap().into_value();
    assert_eq!(
        journal
            .complete_wish(
                &mut next,
                &claim,
                Some(input.clone()),
                Some(accepted.clone()),
                true,
                None
            )
            .unwrap(),
        done
    );
    assert_eq!(runtime_state::capture(&next).unwrap().into_value(), before);
    accepted.name = "Late edit".into();
    assert!(
        journal
            .complete_wish(&mut next, &claim, Some(input), Some(accepted), true, None)
            .is_err()
    );
    journal.acknowledge_wish(&claim, &done).unwrap();
    assert!(journal.pending_wishes().unwrap().is_empty());
    assert!(
        journal
            .complete_wish(&mut next, &claim, None, None, false, Some("again".into()))
            .is_err()
    );
}
#[test]
fn identical_words_have_distinct_refunds_and_reused_seats_get_neither_effects_nor_refund() {
    let (mut run, mut journal, ask) = fixture();
    run.asked.push(ask.clone());
    let first = journal
        .claim_wish(&mut run, ask.clone(), String::new())
        .unwrap();
    let second = journal.claim_wish(&mut run, ask, String::new()).unwrap();
    assert_ne!(first.work.binding, second.work.binding);
    let wishes = run.players[&1].wishes;
    let done = journal
        .complete_wish(
            &mut run,
            &second,
            None,
            None,
            false,
            Some("model failed".into()),
        )
        .unwrap();
    assert!(matches!(done.outcome, Outcome::Refunded(_)));
    assert_eq!(run.players[&1].wishes, wishes + 1);
    journal.acknowledge_wish(&second, &done).unwrap();
    assert_eq!(journal.pending_wishes().unwrap(), vec![first.clone()]);
    let fresh = Run::new(0, 9, None).players.remove(&1).unwrap();
    run.players.insert(1, fresh);
    let before = runtime_state::capture(&run).unwrap().into_value();
    let done = journal
        .complete_wish(&mut run, &first, Some(answer()), Some(answer()), true, None)
        .unwrap();
    assert!(matches!(done.outcome, Outcome::Stale(_)));
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
}
#[test]
fn reservations_and_old_envelopes_cannot_forge_or_drop_wish_ownership() {
    let (mut run, mut journal, ask) = fixture();
    let claim = journal.claim_wish(&mut run, ask, String::new()).unwrap();
    let mut bad = run.clone();
    bad.players.get_mut(&1).unwrap().wish_jobs.clear();
    assert!(journal.validate_wish_handoff(&run, &bad).is_err());
    bad.players
        .get_mut(&1)
        .unwrap()
        .wish_jobs
        .push("unowned".into());
    assert!(journal.validate_wish_world(&bad).is_err());
    let mut saved = journal.capture().unwrap();
    saved["schema"] = 4.into();
    assert!(Journal::restore(saved, &claim.session, 9).is_err());
    let mut checkpoint = runtime_state::capture(&run).unwrap();
    checkpoint.schema = 4;
    assert!(runtime_state::restore(checkpoint).is_err());
    // A genuine previous generation with no reservations remains readable.
    let clean = Run::new(0, 9, None);
    let mut old = runtime_state::capture(&clean).unwrap();
    old.schema = 4;
    let fields = old.run.as_object_mut().unwrap();
    for name in ["squire_company_reserved", "squire_company_claimed", "squire_pending_delves"] {
        fields.remove(name);
    }
    runtime_state::restore(old).unwrap();
}
#[test]
fn all_resolved_definitions_roundtrip_without_sheet_io_and_invalid_policy_is_rejected() {
    for wish in super::super::phrasebook::read(super::super::phrasebook::TEXT) {
        let (book, errors) = super::super::phrasebook::read_checked(&wish.definition_file());
        assert!(errors.is_empty(), "{}: {errors:?}", wish.id);
        assert_eq!(book, vec![wish]);
    }
    let (mut run, mut journal, ask) = fixture();
    let claim = journal.claim_wish(&mut run, ask, String::new()).unwrap();
    let request = PolicyRequest {
        work: claim.work.clone(),
        wish: answer(),
    };
    request.validate().unwrap();
    let mut invalid = answer();
    invalid.tiers[0].0 = vec!["unbounded-magic 999999".into()];
    assert!(request.validate_result(&Ok(invalid.clone())).is_err());
    let before = runtime_state::capture(&run).unwrap().into_value();
    assert!(
        journal
            .complete_wish(&mut run, &claim, Some(answer()), Some(invalid), true, None)
            .is_err()
    );
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
}
#[test]
fn logical_rejection_keeps_exact_input_and_refunds_once_after_restore() {
    let (mut run, mut journal, ask) = fixture();
    let claim = journal.claim_wish(&mut run, ask, String::new()).unwrap();
    let input = answer();
    let done = journal
        .complete_wish(
            &mut run,
            &claim,
            Some(input.clone()),
            None,
            true,
            Some("replacement refused".into()),
        )
        .unwrap();
    let wishes = run.players[&1].wishes;
    let mut journal = Journal::restore(journal.capture().unwrap(), &claim.session, 9).unwrap();
    assert_eq!(
        journal
            .complete_wish(
                &mut run,
                &claim,
                Some(input),
                None,
                true,
                Some("replacement refused".into())
            )
            .unwrap(),
        done
    );
    assert_eq!(run.players[&1].wishes, wishes);
}

#[test]
fn departure_discards_unclaimed_words_and_fences_same_name_rejoins() {
    let (mut run, mut journal, mut ask) = fixture();
    run.asked.clear();
    ask.knight = 2;
    run.asked.push(ask.clone());
    let epoch = run.seat_epoch(2);
    run.leave(2);
    assert!(run.asked.is_empty());
    run.join_seat(2, "Guest");
    assert_ne!(run.seat_epoch(2), epoch);
    assert!(
        journal
            .claim_wish(&mut run, ask.clone(), String::new())
            .is_err()
    );
    // Old saves may already have an orphan queued; it cannot block a valid seat.
    run.leave(2);
    run.asked.push(ask.clone());
    let stale = journal.claim_wish(&mut run, ask, String::new()).unwrap();
    assert!(run.lamp.wishing.iter().all(|pending| pending.knight != 2));
    let before = runtime_state::capture(&run).unwrap().into_value();
    assert!(matches!(
        journal
            .complete_wish(&mut run, &stale, None, None, false, Some("left".into()))
            .unwrap()
            .outcome,
        Outcome::Stale(_)
    ));
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
}

#[test]
fn receipt_capacity_does_not_consume_queued_wish_and_seat_transfer_cannot_claim_old_work() {
    let (mut run, mut journal, ask) = fixture();
    let mut claims = Vec::new();
    for _ in 0..64 {
        if run.asked.is_empty() {
            run.asked.push(ask.clone());
        }
        claims.push(
            journal
                .claim_wish(&mut run, ask.clone(), String::new())
                .unwrap(),
        );
    }
    run.asked.push(ask.clone());
    let before = runtime_state::capture(&run).unwrap().into_value();
    assert!(
        journal
            .claim_wish(&mut run, ask.clone(), String::new())
            .is_err()
    );
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
    let done = journal
        .complete_wish(
            &mut run,
            &claims[0],
            None,
            None,
            false,
            Some("cancel".into()),
        )
        .unwrap();
    journal.acknowledge_wish(&claims[0], &done).unwrap();
    journal.claim_wish(&mut run, ask, String::new()).unwrap();
    run.seat_first("New owner");
    let before = runtime_state::capture(&run).unwrap().into_value();
    assert!(matches!(
        journal
            .complete_wish(
                &mut run,
                &claims[1],
                Some(answer()),
                Some(answer()),
                true,
                None
            )
            .unwrap()
            .outcome,
        Outcome::Stale(_)
    ));
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
}

#[test]
fn grant_failure_restores_maker_and_target_then_refunds_exactly_once() {
    let (mut run, mut journal, ask) = fixture();
    let wish = super::super::phrasebook::read("wish captured-bolt | Captured Bolt\nkind spell\nsay captured bolt\ntier cast bolt; cooldown 6 :: Bolt\n").remove(0);
    let hero = run.players.get_mut(&1).unwrap();
    hero.spells = std::array::from_fn(|i| Some(format!("occupied-{i}")));
    run.makers
        .insert(wish.id.clone(), (1, "Prior maker".into()));
    let claim = journal
        .claim_wish(&mut run, ask, "New maker".into())
        .unwrap();
    let wishes = run.players[&1].wishes;
    let done = journal
        .complete_wish(&mut run, &claim, Some(wish.clone()), Some(wish), true, None)
        .unwrap();
    assert!(
        matches!(&done.outcome, Outcome::Refunded(why) if why.contains("all four spell slots"))
    );
    assert_eq!(
        run.makers
            .get("captured-bolt")
            .map(|(_, name)| name.as_str()),
        Some("Prior maker")
    );
    assert_eq!(run.players[&1].wishes, wishes + 1);
}

#[test]
fn a_full_book_refusal_does_not_heal_a_summoner_when_target_rolls_back() {
    let (mut run, mut journal, mut ask) = fixture();
    assert!(run.take_up(1, super::super::minions::GRAVECALLER));
    let wish = super::super::phrasebook::read("wish captured-health | Captured Health\nsay captured health\ntier max_hp 10 :: Health\ntier max_hp 20 :: More health\n").remove(0);
    run.grant(1, &wish).unwrap();
    run.players.get_mut(&1).unwrap().hp = 1;
    let mut n = 0;
    while run.book.cards.len() < super::super::cards::MAX_BOOK {
        let mut card = wish.card(0).unwrap();
        card.id = format!("filler-{n}");
        n += 1;
        assert!(run.book.insert(card));
    }
    run.asked.clear();
    ask.target = Target::Dead;
    run.asked.push(ask.clone());
    let claim = journal.claim_wish(&mut run, ask, "Maker".into()).unwrap();
    let hero = run.players[&1].clone();
    let done = journal
        .complete_wish(&mut run, &claim, Some(wish.clone()), Some(wish), true, None)
        .unwrap();
    assert!(matches!(&done.outcome, Outcome::Refunded(reason) if reason.contains("book is full")));
    let after = &run.players[&1];
    assert_eq!(after.hp, hero.hp);
    assert_eq!(after.max_hp, hero.max_hp);
    assert_eq!(after.dead_wishes, hero.dead_wishes);
    assert_eq!(after.deck, hero.deck);
}

#[test]
fn a_changed_policy_cannot_overwrite_another_players_already_held_definition() {
    let (mut run, mut journal, mut ask) = fixture();
    let first = journal
        .claim_wish(&mut run, ask.clone(), "Original maker".into())
        .unwrap();
    let wish = answer();
    journal
        .complete_wish(
            &mut run,
            &first,
            Some(wish.clone()),
            Some(wish.clone()),
            true,
            None,
        )
        .unwrap();
    let original = run.book.get(&wish.card_id(0)).unwrap().clone();
    ask.knight = 2;
    run.asked.push(ask.clone());
    let second = journal
        .claim_wish(&mut run, ask, "New maker".into())
        .unwrap();
    let mut changed = wish.clone();
    changed.name = "Worker Blessing".into();
    let done = journal
        .complete_wish(&mut run, &second, Some(wish), Some(changed), true, None)
        .unwrap();
    assert!(
        matches!(&done.outcome, Outcome::Conflict(reason) if reason.contains("already holds a different version"))
    );
    assert_eq!(run.book.get(&original.id), Some(&original));
    assert_eq!(
        run.makers.get("captured-grace"),
        Some(&(1, "Original maker".into()))
    );
    assert!(!run.players[&2].deck.contains(&original.id));
    // A second knight can still take the identical canonical gift; its first
    // maker and existing card remain unchanged.
    run.asked.push(second.work.ask.clone());
    let third = journal
        .claim_wish(&mut run, second.work.ask.clone(), "Second maker".into())
        .unwrap();
    let done = journal
        .complete_wish(&mut run, &third, Some(answer()), Some(answer()), true, None)
        .unwrap();
    assert!(matches!(done.outcome, Outcome::Granted(_)));
    assert_eq!(run.book.get(&original.id), Some(&original));
    assert!(run.players[&2].deck.contains(&original.id));
}

#[test]
fn upgrading_a_base_wish_keeps_a_separate_hyphenated_variant_and_its_cooldown() {
    let (mut run, mut journal, ask) = fixture();
    run.asked.clear();
    let base = super::super::phrasebook::read("wish base-bolt | Base Bolt\nkind spell\nsay base bolt\ntier cast bolt; cooldown 8 :: Bolt\ntier cast bolt; cooldown 6 :: Better bolt\n").remove(0);
    let mut variant = base.clone();
    variant.id = "base-bolt-2".into();
    variant.name = "Variant Bolt".into();
    run.grant(1, &base).unwrap();
    run.grant(1, &variant).unwrap();
    let variant_card = variant.card_id(0);
    let slot = run.players[&1]
        .spells
        .iter()
        .position(|id| id.as_deref() == Some(variant_card.as_str()))
        .unwrap();
    run.players.get_mut(&1).unwrap().spell_cooldowns[slot] = 23;
    run.asked.push(ask.clone());
    let claim = journal.claim_wish(&mut run, ask, "Maker".into()).unwrap();
    let done = journal
        .complete_wish(
            &mut run,
            &claim,
            Some(base.clone()),
            Some(base.clone()),
            true,
            None,
        )
        .unwrap();
    assert!(matches!(done.outcome, Outcome::Granted(_)));
    let hero = &run.players[&1];
    assert!(hero.deck.contains(&base.card_id(1)));
    assert!(
        hero.deck.contains(&variant_card),
        "a shared prefix is not shared ownership"
    );
    assert_eq!(hero.spell_cooldowns[slot], 23);
    assert_eq!(hero.spells[slot].as_deref(), Some(variant_card.as_str()));
}
