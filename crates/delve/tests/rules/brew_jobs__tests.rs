use super::*;
use crate::rules::{home, host_commands::Journal, runtime_state};

const ORCHARD: &str = "name Frostbriar Orchard\nkind spell\nrarity rare\ntext Thorns, a crop and a cold wind.\ndrop 0\nchest 0\ncooldown 20\ncabbages 3\nhedge 1\nward 1\nart\n..fDDf..\n.fDEEDf.\nfDE44EDf\nfDE44EDf\n.fDEEDf.\n..fDDf..\n";
fn queued(run: &mut Run) {
    let placeholder = cauldron::brewed_card(
        cauldron::recipe(cauldron::Essence::Garden, cauldron::Essence::Garden),
        cauldron::Tier::Common,
    );
    let ask = cauldron::Brewing {
        knight: 1,
        raid: run.raid_id,
        depth: run.dungeon.depth,
        cards: ["cabbage-storm".into(), "hedge-maze".into()],
        placeholder: placeholder.id.clone(),
    };
    let hero = run.players.get_mut(&1).unwrap();
    hero.spells[0] = Some(placeholder.id.clone());
    hero.spell_cooldowns[0] = 71;
    hero.bar[0] = Some(cards::BarSlot::Spell(placeholder.id.clone()));
    run.book.insert(placeholder);
    run.brewing.push(ask);
}
fn fixture() -> (Run, Journal, Claim, cards::Card) {
    let mut run = Run::at_home(
        3,
        9,
        Some("Guest"),
        home::Home::default(),
        Default::default(),
    );
    run.descend_for_test();
    queued(&mut run);
    let mut journal = Journal::new("brew-session".into(), 9);
    let claim = journal.claim_brew(&mut run).unwrap().unwrap();
    let card = cards::check(&claim.work.result_id, ORCHARD).unwrap().card;
    (run, journal, claim, card)
}
fn accepted(run: &Run, claim: &Claim, card: &cards::Card) -> cards::Card {
    PolicyRequest {
        work: claim.work.clone(),
        card: card.clone(),
        taken: cauldron::taken(&run.book),
    }
    .evaluate()
    .unwrap()
    .unwrap()
}
#[test]
fn captured_inputs_and_unique_binding_survive_a_new_floor_brew_without_cross_delivery() {
    let (mut run, mut journal, first, card) = fixture();
    let old_input = first.work.inputs[0].clone();
    let mut changed = old_input.clone();
    changed.name = "Changed input".into();
    run.book.insert(changed);
    assert_eq!(first.work.inputs[0], old_input);
    queued(&mut run);
    let second = journal.claim_brew(&mut run).unwrap().unwrap();
    assert_ne!(first.work.placeholder.id, second.work.placeholder.id);
    assert_ne!(first.work.result_id, second.work.result_id);
    let answer = accepted(&run, &first, &card);
    let before = run.sounds.clone();
    let done = journal
        .complete_brew(&mut run, &first, card, answer)
        .unwrap();
    assert!(done.result.is_err());
    assert_eq!(run.sounds, before);
    assert_eq!(
        run.players[&1].spells[0].as_ref(),
        Some(&second.work.placeholder.id)
    );
    assert_eq!(journal.pending_brews().unwrap().len(), 2);
}
#[test]
fn terminal_delivery_and_its_exact_answer_restore_without_recharge_or_replayed_effects() {
    let (mut run, mut journal, claim, card) = fixture();
    run.players.get_mut(&1).unwrap().name = "Renamed knight".into();
    let answer = accepted(&run, &claim, &card);
    let done = journal
        .complete_brew(&mut run, &claim, card.clone(), answer.clone())
        .unwrap();
    assert!(done.result.is_ok());
    assert_eq!(
        run.players[&1].spells[0].as_ref(),
        Some(&claim.work.result_id)
    );
    assert_eq!(run.players[&1].spell_cooldowns[0], 71);
    assert_eq!(run.players.len(), 2);
    let world = runtime_state::capture(&run).unwrap().into_value();
    let saved = journal.capture().unwrap();
    let mut restored = Journal::restore(saved.clone(), &claim.session, 9).unwrap();
    assert_eq!(
        restored
            .complete_brew(&mut run, &claim, card.clone(), answer.clone())
            .unwrap(),
        done
    );
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), world);
    let mut changed = answer;
    changed.name = "New policy".into();
    assert!(
        restored
            .complete_brew(&mut run, &claim, card, changed)
            .is_err()
    );
    let mut old = saved;
    old["schema"] = 2.into();
    assert!(Journal::restore(old, &claim.session, 9).is_err());
    restored.acknowledge_brew(&claim, &done).unwrap();
    assert!(restored.pending_brews().unwrap().is_empty());
}
#[test]
fn stale_sessions_changed_players_and_invalid_policy_output_cannot_grant() {
    let (mut run, mut journal, claim, card) = fixture();
    let answer = accepted(&run, &claim, &card);
    let mut stale = claim.clone();
    stale.session = "another-session".into();
    assert!(
        journal
            .complete_brew(&mut run, &stale, card.clone(), answer.clone())
            .is_err()
    );
    let request = PolicyRequest {
        work: claim.work.clone(),
        card: card.clone(),
        taken: Vec::new(),
    };
    let mut bad = answer.clone();
    bad.id = "someone-elses-spell".into();
    assert!(request.validate_result(&Ok(bad)).is_err());
    run.players.remove(&1);
    assert!(
        journal
            .complete_brew(&mut run, &claim, card, answer)
            .unwrap()
            .result
            .is_err()
    );
    assert!(run.book.get(&claim.work.result_id).is_none());
}
#[test]
fn pending_bindings_reject_bad_handoffs_and_cancellation_is_identified() {
    let (mut run, mut journal, claim, _) = fixture();
    assert!(journal.acknowledge(9, claim.request).is_err());
    let mut bad = run.clone();
    bad.book
        .cards
        .retain(|card| card.id != claim.work.placeholder.id);
    assert!(journal.validate_brew_world(&bad).is_err());
    let done = journal
        .cancel_brew(&mut run, &claim, "expired".into())
        .unwrap();
    assert_eq!(
        journal
            .cancel_brew(&mut run, &claim, "new policy".into())
            .unwrap(),
        done
    );
    assert_eq!(
        run.players[&1].spells[0].as_ref(),
        Some(&claim.work.placeholder.id)
    );
    journal.acknowledge_brew(&claim, &done).unwrap();
}
#[test]
fn queue_and_receipt_backpressure_preserve_work_until_an_individual_acknowledgement() {
    let (mut run, mut journal, first, _) = fixture();
    for _ in 1..64 {
        queued(&mut run);
        journal.claim_brew(&mut run).unwrap().unwrap();
    }
    queued(&mut run);
    let queued = run.brewing.clone();
    assert!(
        journal
            .claim_brew(&mut run)
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(run.brewing, queued);
    assert_eq!(journal.pending_brews().unwrap().len(), 64);
    let done = journal
        .cancel_brew(&mut run, &first, "no answer".into())
        .unwrap();
    journal.acknowledge_brew(&first, &done).unwrap();
    journal.claim_brew(&mut run).unwrap().unwrap();
    assert!(run.brewing.is_empty());
    assert_eq!(journal.pending_brews().unwrap().len(), 64);
}

#[test]
fn changing_a_spell_before_model_admission_retires_its_exact_queue_fact_without_blocking_new_work()
{
    let mut run = Run::new(3, 9, None);
    let mut journal = Journal::new("brew-early-cancel".into(), 9);
    queued(&mut run);
    run.players.get_mut(&1).unwrap().spells[0] = None;
    assert!(journal.claim_brew(&mut run).unwrap().is_none());
    assert!(run.brewing.is_empty());
    assert!(journal.pending_brews().unwrap().is_empty());
    queued(&mut run);
    assert!(journal.claim_brew(&mut run).unwrap().is_some());
}
