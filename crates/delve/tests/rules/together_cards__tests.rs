use super::*;
use crate::rules::{Input, Run};
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

    // Past its range, and past what a deck may hold (damage 40).
    let loud = check("x", "name X\nkind hold\ndamage 900\n").unwrap();
    assert_eq!(loud.card.effects, [Effect::Damage(40)]);
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
    assert_eq!(Bonus::of(five).damage, 40, "five quivers stop at the cap");
    // Mail is worn in whole pieces: copies of one shirt make one piece
    // (1 + a half + a quarter), a second kind of mail the second.
    let mail = book.get("mail").unwrap();
    assert_eq!(Bonus::of([mail, mail, mail, mail]).armor, 1);
    let plate = check("plate", "name Plate\nkind hold\narmor 1\n").unwrap().card;
    assert_eq!(Bonus::of([mail, &plate]).armor, 2);
    assert_eq!(Bonus::of([mail, &plate, mail, &plate]).armor, 2, "the cap");
}

#[test]
fn copies_of_one_card_give_less_each() {
    let book = Book::builtin();
    let quiver = book.get("ember-quiver").unwrap();
    // 25, then half of it, then a quarter, then nothing.
    assert_eq!(Bonus::of([quiver]).damage, 25);
    assert_eq!(Bonus::of([quiver, quiver]).damage, 38);
    assert_eq!(Bonus::of([quiver, quiver, quiver]).damage, 40, "44, capped");
    let spurs = book.get("swift-spurs").unwrap();
    let three = Bonus::of([spurs, spurs, spurs]).speed;
    let four = Bonus::of([spurs, spurs, spurs, spurs]).speed;
    assert_eq!(three, four, "a fourth copy adds nothing");
    // Different cards still add: each its own first copy.
    let draw = book.get("quick-draw").unwrap();
    assert_eq!(Bonus::of([quiver, draw]).damage, 25);
    assert_eq!(Bonus::of([quiver, draw]).rate, 20);
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
    let bow = crate::rules::Weapon::Bow.arms().damage;
    assert!(shots.iter().all(|p| p.damage == bow * 125 / 100));
}

#[test]
fn old_checkpoints_with_item_kinds_still_load() {
    let old = r#"{"kind":{"arms":"crossbow"},"x":1.0,"y":2.0,"held_off":null}"#;
    let item: crate::rules::Item = serde_json::from_str(old).unwrap();
    assert_eq!(item.card, "crossbow");
    let old = r#"{"kind":"potion","x":1.0,"y":2.0,"held_off":null}"#;
    let item: crate::rules::Item = serde_json::from_str(old).unwrap();
    assert_eq!(item.card, "potion");
}

#[test]
fn a_three_slot_save_pads_the_fourth_spell() {
    let mut run = Run::new(9, 1, None);
    let hero = run.players.get_mut(&1).unwrap();
    hero.spells[0] = Some("cabbage-storm".into());
    hero.spell_cooldowns[0] = 45;
    let shorten = |mut value: serde_json::Value| {
        for hero in value["players"].as_object_mut().unwrap().values_mut() {
            hero["spells"].as_array_mut().unwrap().truncate(3);
            hero["spell_cooldowns"].as_array_mut().unwrap().truncate(3);
        }
        value
    };
    let old = shorten(serde_json::to_value(&run).unwrap());
    let loaded: Run = serde_json::from_value(old).unwrap();
    assert!(loaded.valid_snapshot());
    assert_eq!(
        loaded.players[&1].spells,
        [Some("cabbage-storm".into()), None, None, None]
    );
    assert_eq!(loaded.players[&1].spell_cooldowns, [45, 0, 0, 0]);
    assert_eq!(run.wire_copy().players[&1].spells, loaded.players[&1].spells);
}

#[test]
fn interact_is_optional_on_the_wire_and_only_sent_when_pressed() {
    let old =
        r#"{"move_x":1,"move_y":0,"aim_x":0,"aim_y":0,"fire":true,"dash":false,"bomb":false}"#;
    let input: Input = serde_json::from_str(old).unwrap();
    assert!(input.fire && !input.interact);
    assert!(
        serde_json::to_value(Input::default())
            .unwrap()
            .get("interact")
            .is_none()
    );
    let pressed = Input {
        interact: true,
        ..Default::default()
    };
    assert_eq!(serde_json::to_value(pressed).unwrap()["interact"], true);
    assert_eq!(
        serde_json::from_value::<Input>(serde_json::to_value(pressed).unwrap()).unwrap(),
        pressed
    );
}

#[test]
fn four_spells_cast_and_quaff_works_through_the_hand() {
    let mut run = Run::new(9, 1, None);
    run.players.get_mut(&1).unwrap().hp = 20;
    for n in 1..=4 {
        let card = check(
            &format!("spell-{n}"),
            "name Mending\nkind spell\ncooldown 3\nheal 10\n",
        )
        .unwrap()
        .card;
        assert!(run.add_card(card, 1));
    }
    assert!(run.players[&1].spells.iter().all(Option::is_some));
    for n in 1..=4 {
        run.step(&BTreeMap::from([(
            1,
            Input {
                cast: n,
                ..Default::default()
            },
        )]));
        assert!(run.players[&1].spell_cooldowns[usize::from(n - 1)] > 0);
    }
    run.players.get_mut(&1).unwrap().hand = vec!["potion".into(), "holy-water".into()];
    run.step(&BTreeMap::from([(
        1,
        Input {
            play: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(run.players[&1].hand, ["holy-water"]);
    for _ in 0..crate::rules::PLAY_REARM {
        run.step(&BTreeMap::new());
    }
    run.step(&BTreeMap::from([(
        1,
        Input {
            play: 1,
            ..Default::default()
        },
    )]));
    assert!(run.players[&1].hand.is_empty());
}

#[test]
fn ordinary_three_floor_delves_find_two_or_three_distinct_spells() {
    let mut learned = 0;
    for seed in 0..32 {
        let mut run = Run::new(seed, 1, None);
        let mut found = std::collections::BTreeSet::new();
        for depth in 1..=3 {
            assert_eq!(run.floor(), depth);
            let rooms: Vec<_> = run
                .dungeon
                .rooms
                .iter()
                .enumerate()
                .filter(|(_, room)| {
                    matches!(
                        room.kind,
                        crate::rules::RoomKind::Fight
                            | crate::rules::RoomKind::Treasure
                    )
                })
                .map(|(at, _)| at)
                .collect();
            let guardian = run.boss_gates.as_ref().map(|g| g.guardian);
            for at in rooms.into_iter().chain(guardian) {
                run.enter(at, Some(2));
                for _ in 0..600 {
                    run.players.get_mut(&1).unwrap().invulnerable = 100_000;
                    for enemy in &mut run.enemies {
                        enemy.hp = 0;
                    }
                    run.step(&BTreeMap::new());
                    if run.phase == crate::rules::Phase::Exploring {
                        break;
                    }
                }
                for item in &run.room().items {
                    if run
                        .book
                        .get(&item.card)
                        .is_some_and(|c| c.kind == Kind::Spell)
                    {
                        found.insert(item.card.clone());
                    }
                }
                found.extend(run.players[&1].spells.iter().flatten().cloned());
            }
            if depth < 3 {
                run.descend_for_test();
            }
        }
        learned += found.len();
    }
    eprintln!(
        "32 three-floor delves found {learned} distinct spells ({:.2} per delve)",
        learned as f32 / 32.0
    );
    assert!(
        (64..=96).contains(&learned),
        "{learned} spells in 32 delves"
    );
}

#[test]
fn a_held_card_promises_no_more_than_a_deck_may_hold() {
    let held = |raw: &str| check("x", raw).unwrap();
    for (line, effect) in [
        ("chain 3", Effect::Chain(1)),
        ("orbit 3", Effect::Orbit(2)),
        ("burst 50", Effect::Burst(25)),
        ("volley 1", Effect::Volley(5)),
        ("shots 2", Effect::Shots(1)),
    ] {
        let checked = held(&format!("name X\nkind hold\n{line}\n"));
        assert_eq!(checked.card.effects, [effect], "{line}");
        assert!(!checked.notes.is_empty(), "{line} says it was clamped");
    }
    // A play or spell card's own numbers are not held, and keep their range.
    let nova = held("name X\nkind spell\ncooldown 30\nnova 120\n");
    assert_eq!(nova.card.effects, [Effect::Nova(120)]);
    // What a learned wish may use, with the ranges it will keep.
    let effects = hold_effects();
    assert!(effects.contains("max_hp 5-40"), "{effects}");
    assert!(effects.contains("chain 1-1"), "{effects}");
    assert!(effects.contains("volley 5-8"), "{effects}");
    assert!(effects.contains("armor 1-2: mail"), "{effects}");
    assert!(!effects.contains("3 damage per hit"), "{effects}");
}
