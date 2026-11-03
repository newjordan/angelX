use super::*;

/// A calm fight room on floor one with knight one taken up as the Mage,
/// facing east, monsters placed at `foes`.
fn mage(foes: &[(EnemyKind, f32, f32)]) -> Run {
    let mut run = Run::new(21, 1, None);
    let fight = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.open_room_for_test(fight);
    run.enter_for_test(fight);
    run.calm_for_test();
    run.enemies.clear();
    let class = classes::CLASSES.iter().find(|c| c.name == "Mage").unwrap();
    run.become_class(1, class);
    for &(kind, x, y) in foes {
        run.spawn_at_for_test(kind, x, y);
    }
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (12.0, 14.0);
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    hero.invulnerable = 0;
    run.phase = Phase::Fighting;
    run
}

/// Key `cast` pressed for a tick, then let go for one.
fn tap(run: &mut Run, cast: u8) {
    run.step(&BTreeMap::from([(1, Input { cast, ..Input::default() })]));
    run.step(&BTreeMap::new());
}

/// Draw the three essences on keys `keys`, then weave on 4.
fn weave(run: &mut Run, keys: [u8; 3]) {
    for k in keys {
        tap(run, k);
    }
    tap(run, 4);
}

#[test]
fn every_mix_of_three_makes_its_own_spell_in_any_order() {
    use Essence::*;
    let mut seen = std::collections::BTreeSet::new();
    for a in Essence::ALL {
        for b in Essence::ALL {
            for c in Essence::ALL {
                let spell = spell_of(&[a, b, c]).expect("every mix weaves");
                assert_eq!(spell_of(&[c, a, b]), Some(spell));
                seen.insert(spell);
            }
        }
    }
    assert_eq!(seen.len(), SPELLS.len(), "ten mixes, ten spells");
    assert_eq!(SPELLS[spell_of(&[Ember, Ember, Ember]).unwrap()].name, "Sunfall");
    assert_eq!(SPELLS[spell_of(&[Frost, Storm, Ember]).unwrap()].name, "Cataclysm");
    assert_eq!(spell_of(&[Ember, Frost]), None, "two essences weave nothing");
}

#[test]
fn a_press_draws_one_essence_and_the_ring_keeps_the_last_three() {
    let mut run = mage(&[]);
    for _ in 0..5 {
        run.step(&BTreeMap::from([(1, Input { cast: 1, ..Input::default() })]));
    }
    assert_eq!(run.players[&1].weave.orbs, vec![Essence::Ember], "held, it draws once");
    run.step(&BTreeMap::new());
    tap(&mut run, 2);
    tap(&mut run, 3);
    tap(&mut run, 2);
    assert_eq!(run.players[&1].weave.orbs, vec![Essence::Frost, Essence::Storm, Essence::Frost]);
}

#[test]
fn hoarfrost_strikes_and_chills_all_round_and_waits_its_cooldown() {
    let mut run = mage(&[(EnemyKind::Demon, 15.0, 14.0), (EnemyKind::Demon, 9.0, 14.0), (EnemyKind::Demon, 30.0, 14.0)]);
    let hp: Vec<u32> = run.enemies.iter().map(|e| e.hp).collect();
    weave(&mut run, [2, 2, 2]);
    let w = &run.players[&1].weave;
    assert_eq!(w.last.map(usize::from), spell_of(&[Essence::Frost; 3]));
    assert!(w.cooldowns[spell_of(&[Essence::Frost; 3]).unwrap()] > 0);
    assert!(run.enemies[0].hp < hp[0] && run.enemies[1].hp < hp[1], "both sides struck");
    assert!(run.enemies[0].chilled > 0 && run.enemies[1].chilled > 0, "and chilled");
    assert_eq!(run.enemies[2].hp, hp[2], "the far one is out of reach");
    let after = run.enemies[0].hp;
    tap(&mut run, 4);
    assert_eq!(run.enemies[0].hp, after, "a spell on its cooldown is not cast again");
}

#[test]
fn thunderhead_strikes_at_most_five_near_where_the_mage_aims() {
    let foes: Vec<_> = (0..7).map(|i| (EnemyKind::Demon, 19.0 + (i % 3) as f32, 12.0 + (i / 3) as f32 * 1.5)).collect();
    let mut run = mage(&foes);
    let hp: Vec<u32> = run.enemies.iter().map(|e| e.hp).collect();
    weave(&mut run, [3, 3, 3]);
    let struck = run.enemies.iter().zip(&hp).filter(|(e, h)| e.hp < **h).count();
    assert_eq!(struck, 5);
}

#[test]
fn sunfall_marks_its_strike_and_leaves_fire() {
    let mut run = mage(&[]);
    weave(&mut run, [1, 1, 1]);
    assert!(!run.heroes.pillars.is_empty(), "the star is falling");
    assert!(run.heroes.zones.iter().any(|z| z.kind == heroes::ZoneKind::Fire));
}

#[test]
fn the_mage_holds_a_staff_and_no_card_spells() {
    let run = mage(&[]);
    let h = &run.players[&1];
    assert_eq!(h.class.as_deref(), Some("Mage"));
    assert_eq!(h.arm.as_deref(), Some("spark-staff"), "a staff, not a bow");
    assert!(h.spells.iter().all(Option::is_none), "its magic is woven, not slotted");
}

#[test]
fn a_knight_who_is_not_the_mage_does_not_weave() {
    let mut run = mage(&[]);
    let archer = classes::CLASSES.iter().find(|c| c.name == "Archer").unwrap();
    run.become_class(1, archer);
    tap(&mut run, 1);
    tap(&mut run, 2);
    assert!(run.players[&1].weave.orbs.is_empty());
}

#[test]
fn a_mage_has_two_slots_and_one_more_every_second_floor_up_to_ten() {
    assert_eq!(slots(1), 2);
    assert_eq!(slots(2), 2);
    assert_eq!(slots(3), 3);
    assert_eq!(slots(6), 4);
    assert_eq!(slots(7), 5);
    assert_eq!(slots(17), MAX_SLOTS);
    assert_eq!(slots(40), MAX_SLOTS, "ten at most");
}

#[test]
fn a_new_spell_fills_a_free_slot_and_a_full_weave_refuses_the_unknown() {
    let mut run = mage(&[]);
    assert_eq!(slots(run.dungeon.depth), 2);
    weave(&mut run, [1, 1, 1]);
    weave(&mut run, [2, 2, 2]);
    let known = run.players[&1].weave.known.clone();
    assert_eq!(known.len(), 2, "two spells learned into the two slots");
    // A third mix: no slot for it, nothing cast, nothing learned.
    weave(&mut run, [3, 3, 3]);
    let w = &run.players[&1].weave;
    assert_eq!(w.known, known);
    let storm = spell_of(&[Essence::Storm; 3]).unwrap();
    assert_eq!(w.cooldowns[storm], 0, "the unknown spell was not cast");
    assert!(run.found.as_ref().is_some_and(|f| f.2.contains("NO SLOT")), "{:?}", run.found);
    // A known one still weaves, once ready.
    let ember = spell_of(&[Essence::Ember; 3]).unwrap();
    run.players.get_mut(&1).unwrap().weave.cooldowns[ember] = 0;
    weave(&mut run, [1, 1, 1]);
    assert!(run.players[&1].weave.cooldowns[ember] > 0);
}
