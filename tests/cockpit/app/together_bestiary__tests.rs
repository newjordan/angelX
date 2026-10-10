use super::*;

/// Whether a kind belongs in the Bestiary; every kind is named, so a new
/// monster has to be thought about here.
fn in_the_book(kind: EnemyKind) -> bool {
    match kind {
        // Guardians have the Trophy Hall; quintains never fall.
        EnemyKind::Boss | EnemyKind::Dummy => false,
        EnemyKind::Bat
        | EnemyKind::Skeleton
        | EnemyKind::Wraith
        | EnemyKind::Imp
        | EnemyKind::Demon
        | EnemyKind::Dragon
        | EnemyKind::Mimic
        | EnemyKind::Sapper
        | EnemyKind::Necromancer
        | EnemyKind::Warboar
        | EnemyKind::Slime
        | EnemyKind::Goblin
        | EnemyKind::Hob
        | EnemyKind::Shaman
        | EnemyKind::Ward
        | EnemyKind::Flesher
        | EnemyKind::Silkmother
        | EnemyKind::Spiderling
        | EnemyKind::PitTyrant
        | EnemyKind::Hexer
        | EnemyKind::Lich
        | EnemyKind::Hollow => true,
    }
}

#[test]
fn every_monster_that_falls_has_its_page() {
    assert!(ENTRIES.iter().all(|e| in_the_book(e.kind)));
    let mut kinds: Vec<String> = ENTRIES.iter().map(|e| key(e.kind)).collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds.len(), ENTRIES.len(), "one page each");
    assert_eq!(ENTRIES.len(), 22, "and every kind that falls");
}

#[test]
fn a_page_is_kept_under_its_kill_mark() {
    for entry in &ENTRIES {
        assert_eq!(
            bounties::slay_mark(entry.kind).strip_prefix("slay:"),
            Some(key(entry.kind).as_str())
        );
    }
}
