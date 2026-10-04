use super::*;
use crate::drive::together_shooter::{Input, Run};
use std::collections::BTreeMap;

const EMBER: &str =
    "name Ember Quiver\nkind hold\nrarity rare\ntext Hot.\ndamage 25\nart\n..6..\n.565.\n..7..\n";

#[test]
fn spell_cards_require_cooldown_and_accept_cast_shapes() {
    for shape in ["bolt", "ring", "meteor"] {
        let raw = format!("name Spark\nkind spell\ncooldown 3\ncast {shape}\nheal 20\n");
        assert!(
            check("spark", &raw).is_ok(),
            "{shape}: {:?}",
            check("spark", &raw)
        );
    }
    assert!(check("x", "name X\nkind spell\nheal 20\n").is_err());
    assert!(check("x", "name X\nkind play\ncooldown 3\nheal 20\n").is_err());
    assert!(check("x", "name X\nkind spell\ncooldown nope\nheal 20\n").is_err());
    assert!(check("x", "name X\nkind spell\ncooldown 3\ncast dragon\n").is_err());
    assert!(check("x", "name X\nkind spell\ncooldown 3\ngold 100\n").is_err());
}

#[test]
fn every_builtin_card_passes_its_own_checker_and_has_art() {
    let book = Book::builtin();
    assert_eq!(book.cards.len(), BUILTIN.len());
    for card in &book.cards {
        assert!(!card.art.is_empty(), "{} draws its own art", card.id);
        assert!(card.art.len() <= ART && card.art.iter().all(|r| r.chars().count() <= ART));
        assert!(!card.text.is_empty(), "{} has flavour text", card.id);
    }
    assert!(
        book.cards.windows(2).all(|w| w[0].id < w[1].id),
        "sorted, so rolls replay"
    );
    assert!(book.cards.iter().any(|c| c.drop > 0) && book.cards.iter().any(|c| c.chest > 0));
}

#[test]
fn a_friend_card_is_checked_like_runes() {
    let checked = check("ember-quiver", EMBER).unwrap();
    assert_eq!(checked.card.id, "ember-quiver");
    assert_eq!(checked.card.kind, Kind::Hold);
    assert_eq!(checked.card.effects, [Effect::Damage(25)]);
    assert_eq!(
        (checked.card.drop, checked.card.chest),
        (3, 4),
        "rarity weights"
    );

    let loud = check("x", "name X\nkind hold\ndamage 900\n").unwrap();
    assert_eq!(loud.card.effects, [Effect::Damage(50)]);
    assert!(loud.notes.iter().any(|n| n.contains("clamped")));

    let errors = |raw: &str| check("x", raw).unwrap_err().errors.join(" | ");
    assert!(errors("name X\nkind hold\nexplode 5\n").contains("unknown word `explode`"));
    assert!(errors("name X\nkind hold\nheal 5\n").contains("heal works on take or play"));
    assert!(errors("name X\nkind take\ndamage 5\n").contains("damage works on hold"));
    assert!(errors("name X\nkind hold\n").contains("at least one effect"));
    assert!(errors("kind hold\ndamage 5\n").contains("needs a `name`"));
    assert!(errors("name X\nkind arm\n").contains("needs `arm"));
    assert!(
        errors("name X\nkind hold\ndamage 5\nrate 5\nspeed 5\npierce 1\n").contains("at most 3")
    );
    assert!(
        errors("name X\nkind hold\ndamage 5\nart\n................!\n").contains("unknown word")
    );
    assert!(errors("name X\nkind hold\ndamage 5\nart\n.................\n").contains("16 by 16"));
}

#[test]
fn arm_cards_take_plain_weapons_or_forge_runes() {
    let sword = check(
        "blade",
        "name Blade\nkind arm\nmelee damage=34 reach=2 arc=120 every=18\nlook steel blade\n",
    )
    .unwrap();
    assert!(matches!(sword.card.arm, Some(Arm::Forged(ref w)) if w.melee.is_some()));
    let bow = check("bow", "name Bow\nkind arm\narm crossbow\n").unwrap();
    assert_eq!(bow.card.arm, Some(Arm::Plain(Weapon::Crossbow)));
    let greedy = check(
        "g",
        "name G\nkind arm\nbolt damage=999 speed=20 every=1 range=48\n",
    );
    // The Forge's tier-I ceiling holds on cards too: capped or rejected.
    if let Ok(checked) = greedy {
        let Some(Arm::Forged(weapon)) = checked.card.arm else {
            panic!("a rune card forges")
        };
        assert!(weapon.dps() <= 70.5, "{}", weapon.dps());
    }
}

#[test]
fn held_bonuses_stack_up_to_their_caps() {
    let book = Book::builtin();
    let quiver = book.get("ember-quiver").unwrap();
    let five = std::iter::repeat_n(quiver, 5);
    assert_eq!(Bonus::of(five).damage, 100, "five quivers stop at the cap");
    let mail = book.get("mail").unwrap();
    assert_eq!(Bonus::of([mail, mail, mail, mail]).armor, 3);
}

#[test]
fn a_new_card_enters_the_book_and_drops_at_the_knight() {
    let mut run = Run::new(3, 1, None);
    let card = check("ember", EMBER).unwrap().card;
    assert!(run.add_card(card, 1));
    assert!(run.book.get("ember").is_some());
    let item = run.room().items.last().unwrap().clone();
    assert_eq!(item.card, "ember");
    assert_eq!(
        item.held_off,
        Some(1),
        "it waits until the knight steps off"
    );
    let hero = run.players.get_mut(&1).unwrap();
    hero.y = item.y - 3.0;
    hero.x = item.x;
    run.step(&BTreeMap::new());
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (item.x, item.y);
    run.step(&BTreeMap::new());
    assert_eq!(run.players[&1].deck, ["ember"]);
    assert_eq!(run.players[&1].bonus.damage, 25);
}

#[test]
fn held_cards_change_the_shots() {
    let mut run = Run::new(9, 1, None);
    let hero = run.players.get_mut(&1).unwrap();
    hero.deck = vec!["twin-string".into(), "ember-quiver".into()];
    let book = run.book.clone();
    run.players.get_mut(&1).unwrap().rebonus(&book);
    run.step(&BTreeMap::from([(
        1,
        Input {
            aim_y: -1,
            fire: true,
            ..Default::default()
        },
    )]));
    let shots: Vec<_> = run.projectiles.iter().filter(|p| !p.hostile).collect();
    assert_eq!(shots.len(), 2, "twin string fans a second arrow");
    assert!(shots.iter().all(|p| p.damage == 32 * 125 / 100));
}

#[test]
fn old_checkpoints_with_item_kinds_still_load() {
    let old = r#"{"kind":{"arms":"crossbow"},"x":1.0,"y":2.0,"held_off":null}"#;
    let item: crate::drive::together_shooter::Item = serde_json::from_str(old).unwrap();
    assert_eq!(item.card, "crossbow");
    let old = r#"{"kind":"potion","x":1.0,"y":2.0,"held_off":null}"#;
    let item: crate::drive::together_shooter::Item = serde_json::from_str(old).unwrap();
    assert_eq!(item.card, "potion");
}

#[test]
fn spell_files_hot_load_once_into_first_empty_host_slot_with_flourish() {
    use super::super::Run;
    let dir = std::env::temp_dir().join(format!("angel-spell-cards-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut run = Run::new(19, 1, Some("friend"));
    let raw = "name Spark\nkind spell\ncooldown 3\ncast bolt\n";
    std::fs::write(dir.join("spark.card"), "name incomplete\nkind spell\n").unwrap();
    assert_eq!(run.hot_load_spells(&dir), 0);
    std::fs::write(dir.join("spark.card"), raw).unwrap();
    assert_eq!(run.hot_load_spells(&dir), 1);
    assert_eq!(run.players[&1].spells[0].as_deref(), Some("spark"));
    assert_eq!(run.players[&2].spells, [None, None, None]);
    assert_eq!(run.players[&1].hand.len(), 0);
    assert_eq!(run.blasts.len(), 1);
    assert!(run.sounds.contains(&"card_pickup"));
    assert!(run.found_line().unwrap().contains("Spark"));
    assert_eq!(
        run.hot_load_spells(&dir),
        0,
        "unchanged file must not flourish twice"
    );
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 25;
    std::fs::write(
        dir.join("spark.card"),
        raw.replace("cooldown 3", "cooldown 4"),
    )
    .unwrap();
    assert_eq!(run.hot_load_spells(&dir), 1);
    assert_eq!(
        run.players[&1].spell_cooldowns[0], 25,
        "editing cannot reset recharge"
    );
    assert_eq!(run.blasts.len(), 1);
    for id in ["a", "b", "c"] {
        std::fs::write(dir.join(format!("{id}.card")), raw).unwrap();
    }
    assert_eq!(run.hot_load_spells(&dir), 3);
    assert_eq!(run.players[&1].spells[1].as_deref(), Some("a"));
    assert_eq!(run.players[&1].spells[2].as_deref(), Some("b"));
    assert!(run.room().items.iter().any(|i| i.card == "c"));
    assert_eq!(run.hot_load_spells(&dir), 0);
    std::fs::remove_dir_all(&dir).unwrap();
}
