use super::*;
use crate::drive::together_shooter::cards::Guard;

#[test]
fn every_knight_of_the_company_carries_real_cards() {
    let book = crate::drive::together_shooter::Book::builtin();
    for k in COMPANY {
        assert!(
            book.get(k.arm).is_some_and(|c| c.arm.is_some()),
            "{} arm {}",
            k.id,
            k.arm
        );
        assert!(
            book.get(k.guard).is_some_and(|c| c.guard.is_some()),
            "{} guard {}",
            k.id,
            k.guard
        );
        assert!(k.hand.iter().all(|c| book.get(c).is_some()));
    }
    let ids: std::collections::BTreeSet<_> = COMPANY.iter().map(|k| k.id).collect();
    assert_eq!(ids.len(), COMPANY.len());
}

#[test]
fn outfitting_dresses_the_knight_and_begin_in_picks_the_first_delve() {
    let mut run = Run::new(3, 1, None);
    let lynette = knight("lynette").unwrap();
    run.outfit(1, lynette);
    let hero = &run.players[&1];
    assert_eq!(hero.guard, Guard::Shield);
    assert_eq!(hero.arm.as_deref(), Some("crossbow"));
    assert_eq!(hero.colours(), lynette.colours);
    let other = if run.dungeon.pack == Pack::Crypt {
        Pack::Cavern
    } else {
        Pack::Crypt
    };
    run.begin_in(other);
    assert_eq!(run.dungeon.pack, other);
    assert_eq!(run.cues.first().map(String::as_str), Some("run_start"));
    let galahad = knight("galahad").unwrap();
    run.outfit(1, galahad);
    assert_eq!(run.players[&1].max_hp, 130);
    assert!(
        run.players[&1].forged.is_some(),
        "the blade is a forged melee arm"
    );
}

#[test]
fn a_kit_is_worn_by_a_knight_of_the_serving_house() {
    use crate::stage::houses;
    let percival = knight("percival").unwrap();
    // Nothing serving (the stub route): the Keep's own household.
    houses::note_serving(&houses::Serving::default());
    assert_eq!(
        percival.styled(house_for_seat(1)),
        "Sir Percival, the Seeker"
    );
    let mut run = Run::new(3, 2, Some("Friend"));
    run.outfit(1, percival);
    assert_eq!(run.players[&1].house, None);
    assert_eq!(
        run.players[&1].knight_name().as_deref(),
        Some("Sir Percival")
    );

    // DeepSeek serves, Kimi sits beside it in a formation.
    let (deepseek, kimi) = (houses::by_key("deepseek"), houses::by_key("kimi"));
    houses::note_serving(&houses::Serving {
        lead: deepseek,
        seated: kimi.into_iter().collect(),
        turn: false,
    });
    let ds = houses::get(deepseek.unwrap());
    let first = &ds.knights[percival.slot()];
    assert_eq!(
        percival.styled(house_for_seat(1)),
        format!("{} of {}, the Seeker", first.name, ds.castle)
    );
    run.outfit(1, percival);
    let lynette = knight("lynette").unwrap();
    run.outfit(2, lynette);
    assert_eq!(run.players[&1].house.as_deref(), Some("deepseek"));
    assert_eq!(run.players[&1].knight_name(), Some(first.name.clone()));
    let km = houses::get(kimi.unwrap());
    assert_eq!(run.players[&2].house.as_deref(), Some("kimi"));
    assert_eq!(
        run.players[&2].knight_name(),
        Some(km.knights[lynette.slot()].name.clone())
    );
    // The kit is unchanged: same arms, same ultimate, same saved id.
    assert_eq!(run.players[&2].knight.as_deref(), Some("lynette"));
    assert_eq!(run.players[&2].colours(), lynette.colours);

    // A save from before the houses has no `house`: it loads, and the
    // Keep's household names the knight.
    let mut saved = serde_json::to_value(&run.players[&1]).unwrap();
    saved.as_object_mut().unwrap().remove("house");
    let old: crate::drive::together_shooter::Hero = serde_json::from_value(saved).unwrap();
    assert_eq!(old.house, None);
    assert_eq!(old.knight_name().as_deref(), Some("Sir Percival"));
    houses::note_serving(&houses::Serving::default());
}
