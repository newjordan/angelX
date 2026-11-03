use super::*;
use crate::rules::{runtime_state, phrasebook, cards};
fn game(guest: Option<&str>) -> Run {
    let mut run = Run::new(0, 9, guest);
    run.descend_for_test();
    for hero in run.players.values_mut() {
        hero.wishes = 3;
        hero.great_wishes = 0;
    }
    run
}
fn wish(name: &str, power: u32) -> Wish {
    phrasebook::read(&format!("wish captured-choice | {name}\nsay retained choice\ntier damage {power} :: Captured power\n")).remove(0)
}
#[test]
fn an_open_offer_keeps_its_resolved_definition_through_source_change_and_handoff() {
    let mut run = game(None);
    let old = wish("Original Choice", 10);
    run.offer_wish(1, &old.name, Some(&old)).unwrap();
    let live = runtime_state::capture(&run).unwrap();
    let mut next = runtime_state::restore(live).unwrap();
    // The explicit source need never have been installed globally. Changing
    // or removing the global entry cannot change this pending choice.
    let result = next.choose(1, 0).unwrap();
    assert!(result.contains("Original Choice"));
    assert_eq!(
        next.book.get(&old.card_id(0)).unwrap().effects,
        vec![cards::Effect::Damage(10)]
    );
    assert!(next.players[&1].offer.is_none());
}
#[test]
fn retained_sources_are_private_on_first_and_incremental_wire_but_durable() {
    let mut run = game(None);
    let source = wish("Private Choice", 10);
    run.offer_wish(1, &source.name, Some(&source)).unwrap();
    let private = serde_json::to_value(&run).unwrap();
    assert!(private["players"]["1"]["offer"]["binding"].is_object());
    let restored: Run = serde_json::from_value(private).unwrap();
    assert_eq!(
        restored.players[&1].offer.as_ref().unwrap().binding,
        run.players[&1].offer.as_ref().unwrap().binding
    );
    let mut publisher = crate::wire::Publisher::default();
    let first = publisher.publish(&run).unwrap();
    let mut changed = source.clone();
    changed.name = "Changed Private Choice".into();
    changed.tiers[0].0 = vec!["damage 20".into()];
    run.offer_wish(1, &changed.name, Some(&changed)).unwrap();
    for tree in [
        first,
        crate::wire::publish(&run).unwrap(),
        publisher.publish(&run).unwrap(),
    ] {
        let json = serde_json::to_value(&crate::wire::read(&tree).unwrap()).unwrap();
        assert!(json["players"]["1"]["offer"].get("binding").is_none());
        assert!(json["players"]["1"]["offer"]["choices"].is_array());
    }
    assert!(run.players[&1].offer.as_ref().unwrap().binding.is_some());
}
#[test]
fn legacy_scrolls_require_reopening_and_cannot_hide_new_sources_in_old_envelopes() {
    let mut run = game(None);
    let source = wish("Legacy Choice", 10);
    run.offer_wish(1, &source.name, Some(&source)).unwrap();
    let mut capture = runtime_state::capture(&run).unwrap();
    capture.schema = 5;
    let fields = capture.run.as_object_mut().unwrap();
    for name in ["squire_company_reserved", "squire_company_claimed", "squire_pending_delves"] {
        fields.remove(name);
    }
    assert!(runtime_state::restore(capture.clone()).is_err());
    capture.run["players"]["1"]["offer"]
        .as_object_mut()
        .unwrap()
        .remove("binding");
    let mut legacy = runtime_state::restore(capture).unwrap();
    let wishes = legacy.players[&1].wishes;
    assert!(legacy.choose(1, 0).unwrap_err().contains("reopen"));
    assert_eq!(legacy.players[&1].wishes, wishes);
    assert!(legacy.players[&1].offer.is_some());
}
#[test]
fn source_conflicts_and_changed_tiers_do_not_charge_or_heal_the_chooser() {
    let mut run = game(Some("Guest"));
    let old = wish("Old Choice", 10);
    run.offer_wish(1, &old.name, Some(&old)).unwrap();
    let mut changed = old.clone();
    changed.tiers[0].0 = vec!["damage 20".into()];
    run.grant(2, &changed).unwrap();
    run.players.get_mut(&1).unwrap().hp = 13;
    let before = serde_json::to_value(&run.players[&1]).unwrap();
    assert!(run.choose(1, 0).is_err());
    assert_eq!(serde_json::to_value(&run.players[&1]).unwrap(), before);
    assert_eq!(
        run.book.get(&old.card_id(0)).unwrap().effects,
        vec![cards::Effect::Damage(20)]
    );
}
#[test]
fn bolder_keeps_the_displayed_blood_price_when_maximum_health_changes() {
    let mut run = game(None);
    run.players.get_mut(&1).unwrap().max_hp = 90;
    run.players.get_mut(&1).unwrap().hp = 90;
    let mut selected = None;
    for seed in 0..16 {
        run.seed = seed;
        run.offer_wish(1, "new unlearned idea", None).unwrap();
        selected = run.players[&1]
            .offer
            .as_ref()
            .unwrap()
            .choices
            .iter()
            .position(|c| c.take == crate::rules::genie::Take::Bolder);
        if selected.is_some() {
            break;
        }
    }
    let selected = selected.expect("a seed with a bolder choice");
    run.players.get_mut(&1).unwrap().max_hp = 180;
    run.choose(1, selected).unwrap();
    assert_eq!(run.players[&1].hp, 60);
}

#[test]
fn an_open_plain_scroll_does_not_consume_a_later_super_wish() {
    let mut run = game(None);
    let source = wish("Plain Choice", 10);
    run.offer_wish(1, &source.name, Some(&source)).unwrap();
    run.players.get_mut(&1).unwrap().great_wishes = 1;
    run.choose(1, 0).unwrap();
    assert_eq!(
        (run.players[&1].wishes, run.players[&1].great_wishes),
        (2, 1)
    );
    run.offer_wish(1, "unlearned super choice", None).unwrap();
    assert!(run.players[&1].offer.as_ref().unwrap().great);
    run.players.get_mut(&1).unwrap().great_wishes = 0;
    let before = serde_json::to_value(&run.players[&1]).unwrap();
    assert!(run.choose(1, 0).unwrap_err().contains("no longer held"));
    assert_eq!(serde_json::to_value(&run.players[&1]).unwrap(), before);
}

#[test]
fn changed_tier_and_reused_seat_reject_without_spending_the_new_owners_wish() {
    let mut run = game(None);
    let mut source = wish("Tier Choice", 10);
    source
        .tiers
        .push((vec!["damage 20".into()], "Second".into()));
    run.offer_wish(1, &source.name, Some(&source)).unwrap();
    run.grant(1, &source).unwrap();
    let before = serde_json::to_value(&run.players[&1]).unwrap();
    assert!(run.choose(1, 0).unwrap_err().contains("climbed"));
    assert_eq!(serde_json::to_value(&run.players[&1]).unwrap(), before);
    run.offer_wish(1, &source.name, Some(&source)).unwrap();
    *run.seat_epochs.entry(1).or_default() += 1;
    let before = serde_json::to_value(&run.players[&1]).unwrap();
    assert!(run.choose(1, 0).unwrap_err().contains("seat ownership"));
    assert_eq!(serde_json::to_value(&run.players[&1]).unwrap(), before);
}

#[test]
fn built_in_spell_changes_retain_the_open_scroll_and_cooldown_on_refusal() {
    let mut run = game(None);
    let card = cards::check(
        "retained-spell",
        "name Retained Spell\nkind spell\ncooldown 5\nnova 10\n",
    )
    .unwrap()
    .card;
    run.book.insert(card.clone());
    run.players.get_mut(&1).unwrap().spells[0] = Some(card.id.clone());
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    run.offer_wish(1, "upgrade Retained Spell", None).unwrap();
    run.players
        .get_mut(&1)
        .unwrap()
        .spell_tiers
        .insert(card.id.clone(), 1);
    let before = serde_json::to_value(&run.players[&1]).unwrap();
    assert!(run.choose(1, 0).unwrap_err().contains("tier or slot"));
    assert_eq!(serde_json::to_value(&run.players[&1]).unwrap(), before);
    run.players.get_mut(&1).unwrap().spell_tiers.clear();
    let mut changed = card.clone();
    changed.effects = vec![cards::Effect::Nova(20)];
    run.book.insert(changed);
    assert!(run.choose(1, 0).unwrap_err().contains("spell changed"));
    assert_eq!(run.players[&1].spell_cooldowns[0], 71);
    assert_eq!(run.players[&1].wishes, 3);
    // Pure presentation advances do not invalidate a retained executable spell.
    let mut painted = card;
    painted.by = "New painter".into();
    run.book.insert(painted);
    run.choose(1, 0).unwrap();
    assert_eq!(run.players[&1].spell_cooldowns[0], 71);
    assert_eq!(run.players[&1].wishes, 2);
}

#[test]
fn a_freedom_offer_that_becomes_unavailable_keeps_the_agreement_and_currency() {
    let mut run = game(None);
    run.players.get_mut(&1).unwrap().great_wishes = 1;
    run.genie.freedom = crate::rules::genie::FREEDOM;
    run.offer_wish(1, crate::rules::genie::FREE_WORDS, None)
        .unwrap();
    run.genie.freedom = 0;
    let before = serde_json::to_value(&run.players[&1]).unwrap();
    assert!(run.choose(1, 0).is_err());
    assert_eq!(serde_json::to_value(&run.players[&1]).unwrap(), before);
}

#[test]
fn policy_metadata_contains_no_executable_source_and_rejects_uncaptured_results() {
    let run = game(None);
    let source = wish("Metadata Choice", 10);
    let capture = Capture::resolved(
        &run,
        1,
        &source.name,
        Some(&source),
        None,
        phrasebook::book(),
    )
    .unwrap();
    let json = serde_json::to_string(&capture.request).unwrap();
    assert!(!json.contains("damage 10"));
    assert!(!json.contains("\"binding\""));
    assert!(!json.contains("\"run\""));
    assert!(!json.contains("\"avatar\""));
    let good = capture.request.evaluate().unwrap();
    let mut bad = good.clone();
    bad.as_mut().unwrap().offer.choices[0].wish = Some("not-captured".into());
    assert!(capture.request.validate_result(&bad).is_err());
    let mut bad = good.clone();
    bad.as_mut().unwrap().offer.choices[0].steps = 0;
    assert!(capture.request.validate_result(&bad).is_err());
    let mut bad = good.clone();
    bad.as_mut().unwrap().offer.binding = Some(Binding::legacy(9, 1, 0));
    assert!(capture.request.validate_result(&bad).is_err());
    let prepared = capture.prepare(&run, good).unwrap();
    let mut changed = run.clone();
    changed.players.get_mut(&1).unwrap().wishes -= 1;
    let before = runtime_state::capture(&changed).unwrap();
    assert!(prepared.deliver(&mut changed).is_err());
    assert_eq!(runtime_state::capture(&changed).unwrap().run, before.run);
}

#[test]
fn the_journal_replays_scroll_receipts_without_rerunning_policy_and_rolls_back_a_batch() {
    use crate::rules::host_commands::{Command, Journal, Request as CommandRequest};
    let mut run = game(None);
    let mut journal = Journal::new("scroll-session".into(), run.raid_id);
    let scroll = Command::Scroll {
        player: 1,
        intent: Intent::Local {
            words: "fresh policy idea".into(),
            known: None,
        },
    };
    let request = CommandRequest {
        session: "scroll-session".into(),
        raid: run.raid_id,
        id: 1,
        commands: vec![scroll.clone()],
    };
    let mut calls = 0;
    let done = journal
        .apply_with_scroll(&mut run, request.clone(), |run, player, intent| {
            calls += 1;
            let capture = Capture::new(run, player, intent)?;
            let result = capture.request.evaluate()?;
            capture.prepare(run, result)
        })
        .unwrap();
    assert!(done.result.is_ok());
    let before = runtime_state::capture(&run).unwrap();
    assert_eq!(
        journal
            .apply_with_scroll(&mut run, request, |_, _, _| panic!("replayed policy"))
            .unwrap(),
        done
    );
    assert_eq!(calls, 1);
    assert_eq!(runtime_state::capture(&run).unwrap().run, before.run);
    let request = CommandRequest {
        session: "scroll-session".into(),
        raid: run.raid_id,
        id: 2,
        commands: vec![
            scroll,
            Command::ChooseOffer {
                player: 2,
                index: 0,
            },
        ],
    };
    let rejected = journal.apply(&mut run, request.clone()).unwrap();
    assert!(rejected.result.is_err());
    assert_eq!(
        runtime_state::capture(&run).unwrap().run,
        before.run,
        "no partial offer or sound publication"
    );
    assert_eq!(
        journal
            .apply_with_scroll(&mut run, request, |_, _, _| panic!("replayed rejection"))
            .unwrap(),
        rejected
    );
}
