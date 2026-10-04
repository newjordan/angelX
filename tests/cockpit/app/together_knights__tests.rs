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
