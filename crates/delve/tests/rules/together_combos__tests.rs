use super::*;
use crate::rules::cards::check;

fn card(id: &str, text: &str) -> Card {
    check(id, text).unwrap_or_else(|e| panic!("{id}: {:?}", e.errors)).card
}

#[test]
fn a_wish_is_of_the_families_its_words_and_effects_name() {
    let fire = card("wish-fire-farts-1", "name Fire Farts I\nkind hold\nburn 1\nfart 1\n");
    let f = families(&fire);
    assert!(f.contains(&Family::Fire) && f.contains(&Family::Venom), "{f:?}");
    let frogs = card("wish-frogs-1", "name Exploding Frogs I\nkind spell\nsummon 2 swarm\ncooldown 8\n");
    assert!(families(&frogs).contains(&Family::Creature));
    let tux = card("wish-tuxedo-1", "name Tuxedo I\nkind hold\narmor 1\n");
    assert!(families(&tux).contains(&Family::Body));
}

#[test]
fn every_recipe_needs_two_families_and_names_a_held_word() {
    for r in RECIPES {
        assert_ne!(r.needs[0], r.needs[1], "{}", r.name);
        assert!(crate::rules::cards::cap(r.effect) > 0, "{} is a held word", r.effect);
    }
}

#[test]
fn a_recipe_is_made_only_when_both_families_are_held() {
    let mut held = BTreeSet::new();
    held.insert(Family::Projectile);
    assert!(!unlocked("homing", &held));
    assert!(unlocked("damage", &held), "an ordinary word needs no recipe");
    held.insert(Family::Creature);
    assert!(unlocked("homing", &held));
    assert!(made(&held).iter().any(|r| r.effect == "homing"));
    assert!(!unlocked("chain", &held), "chain lightning still wants a storm");
}

#[test]
fn found_cards_lend_no_families() {
    let fork = card("fork", "name Fork\nkind hold\nfork 1\n");
    assert!(held_families([&fork]).is_empty());
}

#[test]
fn the_list_reads_as_plain_words() {
    let held: BTreeSet<Family> = [Family::Projectile, Family::Creature].into_iter().collect();
    assert_eq!(list(&held), "Creature, Projectile");
}
