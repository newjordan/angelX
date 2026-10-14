use super::*;
use crate::rules::Run;
use crate::rules::cards::{self, check};

fn errors(raw: &str) -> String {
    check("x", raw).unwrap_err().errors.join(" | ")
}

#[test]
fn a_strip_splits_into_frames_and_each_shows_in_turn() {
    let rows: Vec<String> = [".5.|5.5", "555|.5."]
        .iter()
        .map(|r| r.to_string())
        .collect();
    let sprite = Sprite::from_strip(Slot::Orbit, &rows).unwrap();
    assert_eq!(sprite.frames.len(), 2);
    assert_eq!(sprite.frames[0], vec![".5.", "555"]);
    assert_eq!(sprite.frame(0), &sprite.frames[0][..]);
    assert_eq!(sprite.frame(TICKS_PER_FRAME), &sprite.frames[1][..]);
    assert_eq!(sprite.frame(TICKS_PER_FRAME * 2), &sprite.frames[0][..]);
    // And back to the block that makes it.
    assert_eq!(
        sprite.to_block(Slot::Orbit),
        "draw orbit\n.5.|5.5\n555|.5.\n"
    );
}

#[test]
fn a_strip_is_bounded() {
    let five = vec!["5|5|5|5|5".to_string()];
    assert!(
        Sprite::from_strip(Slot::Summon, &five)
            .unwrap_err()
            .contains("at most 4")
    );
    let ragged = vec!["5|5".to_string(), "5".to_string()];
    assert!(
        Sprite::from_strip(Slot::Summon, &ragged)
            .unwrap_err()
            .contains("put `|`")
    );
    let wide = vec!["5".repeat(13)];
    assert!(
        Sprite::from_strip(Slot::Shot, &wide)
            .unwrap_err()
            .contains("at most 12x12")
    );
    let blank = vec!["...|.5.".to_string()];
    assert!(
        Sprite::from_strip(Slot::Shot, &blank)
            .unwrap_err()
            .contains("frame 1 has no inked pixel")
    );
}

#[test]
fn a_card_carries_its_looks_slot_by_slot() {
    let raw = format!(
        "name Spiked Pavise\nkind hold\narmor 1\ndraw worn at=left\n{PAVISE}\n\ndraw shot\n.55.|.66.\n5665|6556\n.55.|.66.\n\ndraw orbit\n.H.\nHiH\n.H.\n\ndraw aura\n..2222..\n.2....2.\n..2222..\n"
    );
    let card = check("pavise", &raw).unwrap().card;
    let worn = card.looks.worn.as_ref().expect("worn");
    assert_eq!(worn.size(), (24, 24), "put on the 24x24 worn canvas");
    // The shield arm: its boss lands left of the knight's middle.
    let boss = worn.frames[0].iter().position(|r| r.contains('6')).unwrap();
    let col = worn.frames[0][boss].find('6').unwrap();
    assert!(col < 12 && (10..16).contains(&boss), "{col},{boss}");
    assert_eq!(card.looks.shot.as_ref().unwrap().frames.len(), 2);
    assert!(card.looks.orbit.is_some() && card.looks.aura.is_some());
    assert_eq!(
        card.effects,
        [cards::Effect::Armor(1)],
        "looks change nothing else"
    );
    // Saved and sent as data, and read back the same.
    let json = serde_json::to_string(&card).unwrap();
    let back: cards::Card = serde_json::from_str(&json).unwrap();
    assert_eq!(back, card);
    // A card without looks saves as it always did.
    let plain = check("p", "name P\nkind hold\ndamage 5\n").unwrap().card;
    assert!(!serde_json::to_string(&plain).unwrap().contains("looks"));
}

#[test]
fn a_look_shows_only_where_its_kind_keeps_it() {
    assert!(
        errors("name X\nkind take\nheal 5\ndraw worn\n.5.\n").contains("does not show on a take")
    );
    assert!(
        errors("name X\nkind spell\ncooldown 5\ncast bolt\ndraw orbit\n.5.\n")
            .contains("orbit` belongs on a hold")
    );
    assert!(errors("name X\nkind hold\ndamage 5\ndraw summon\n.5.\n").contains("needs a `summon`"));
    assert!(errors("name X\nkind hold\ndamage 5\ndraw cape\n.5.\n").contains("draw `cape`"));
    assert!(
        errors("name X\nkind hold\ndamage 5\ndraw worn\n.5.\ndraw worn\n.5.\n").contains("twice")
    );
    assert!(errors("name X\nkind hold\ndamage 5\ndraw shot at=left\n.5.\n").contains("at=<place>"));
    assert!(errors("name X\nkind hold\ndamage 5\ndraw worn at=tail\n.5.\n").contains("at=`tail`"));
    // A picture is cut when the card loads from a workspace; sent alone,
    // a card draws its rows.
    assert!(errors("name X\nkind hold\ndamage 5\nsheet worn shield.png\n").contains("sheets"));
    let icon_after_sprite = check(
        "x",
        "name X\nkind hold\ndamage 5\ndraw shot\n.5.\nart\n.H.\n",
    )
    .unwrap()
    .card;
    assert_eq!(icon_after_sprite.art, [".H."]);
    assert_eq!(icon_after_sprite.looks.shot.unwrap().frames[0], [".5."]);
    // The ash sprite's summons word is still an effect, not a look.
    assert!(check("ash", "name Ash\nkind play\nsprite 1\n").is_ok());
}

#[test]
fn shots_of_a_held_or_spell_card_may_take_a_forged_colour_and_shape() {
    let card = check("x", "name Venom\nkind hold\ndamage 5\nlook venom star\n")
        .unwrap()
        .card;
    assert_eq!(card.looks.style, Some(("venom".into(), "star".into())));
    assert_eq!(
        Projectile::look_of(&card),
        Some(("venom".into(), "star".into()))
    );
    assert!(
        errors("name X\nkind hold\ndamage 5\nlook plaid star\n").contains("look takes a colour")
    );
    // On a weapon it is still a rune.
    let arm = check(
        "x",
        "name Bow\nkind arm\nlook gold arrow\nbolt damage=32 speed=12 every=15 range=48\n",
    )
    .unwrap()
    .card;
    assert!(arm.looks.style.is_none());
}

#[test]
fn a_knight_wears_what_their_cards_draw_newest_on_top() {
    let mut run = Run::new(3, 1, None);
    let shield = check(
        "pavise",
        &format!("name Pavise\nkind hold\narmor 1\ndraw worn at=left\n{PAVISE}\n"),
    )
    .unwrap()
    .card;
    let helm = check(
        "helm",
        "name Horned Helm\nkind hold\nmax_hp 5\ndraw worn at=head\nH....H\n.HiiH.\n",
    )
    .unwrap()
    .card;
    let fire = check(
        "fire",
        "name Fire\nkind hold\ndamage 5\ndraw shot\n.7.\n767\n.7.\n",
    )
    .unwrap()
    .card;
    for card in [shield, helm, fire] {
        run.book.insert(card.clone());
        run.players.get_mut(&1).unwrap().deck.push(card.id);
    }
    let hero = &run.players[&1];
    let worn = hero.worn(&run.book);
    assert_eq!(worn.len(), 2);
    assert!(
        worn[1].frames[0].iter().any(|r| r.contains("HiiH")),
        "the helm, newest, on top"
    );
    assert_eq!(
        hero.shot_look(&run.book),
        Some(("card".into(), "fire".into()))
    );
    assert!(hero.newest(&run.book, Slot::Orbit).is_none());
    // A spell wish's tier marker in the deck does not recolour the bow.
    let spell = check(
        "spell",
        "name Spell\nkind spell\ncooldown 4\ncast bolt\ndraw shot\n.5.\n555\n",
    )
    .unwrap()
    .card;
    run.book.insert(spell);
    run.players.get_mut(&1).unwrap().deck.push("spell".into());
    assert_eq!(
        run.players[&1].shot_look(&run.book),
        Some(("card".into(), "fire".into()))
    );
    // Duplicate shirts are one visual layer, too.
    run.players
        .get_mut(&1)
        .unwrap()
        .deck
        .extend(["pavise".into(), "pavise".into()]);
    assert_eq!(run.players[&1].worn(&run.book).len(), 2);
}

#[test]
fn plain_shots_and_each_spell_shape_take_the_card_look() {
    use crate::rules::{Input, RoomKind};
    use std::collections::BTreeMap;
    let mut run = Run::new(9, 1, None);
    let room = run
        .dungeon
        .rooms
        .iter()
        .position(|r| r.kind == RoomKind::Fight && !r.great())
        .unwrap();
    run.enter_for_test(room);
    run.calm_for_test();
    run.enemies.clear();
    let held = check(
        "comet",
        "name Comet\nkind hold\ndamage 5\ndraw shot\n.2.\n232\n",
    )
    .unwrap()
    .card;
    run.book.insert(held);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.x, hero.y) = (24.0, 14.0);
    hero.deck.push("comet".into());
    // Clearing an empty room sweeps its old shots; fire after that tick.
    run.step(&BTreeMap::new());
    run.step(&BTreeMap::from([(
        1,
        Input {
            fire: true,
            aim_x: 1,
            ..Default::default()
        },
    )]));
    assert!(
        run.projectiles
            .iter()
            .any(|p| p.sprite_card() == Some("comet"))
    );
    for (shape, count) in [("bolt", 1), ("ring", 12), ("meteor", 1)] {
        let own = check(
            shape,
            &format!(
                "name Spell\nkind spell\ncooldown 5\ncast {shape}\ndraw shot\n.5.|5.5\n555|555\n"
            ),
        )
        .unwrap()
        .card;
        run.book.insert(own);
        let hero = run.players.get_mut(&1).unwrap();
        hero.spells[0] = Some(shape.into());
        hero.spell_cooldowns[0] = 0;
        run.projectiles.clear();
        run.step(&BTreeMap::from([(
            1,
            Input {
                cast: 1,
                ..Default::default()
            },
        )]));
        assert_eq!(run.projectiles.len(), count, "{shape}");
        assert!(
            run.projectiles
                .iter()
                .all(|p| p.sprite_card() == Some(shape)),
            "own spell look beats held look"
        );
    }
    let spell = check("plain", "name Plain\nkind spell\ncooldown 5\ncast bolt\n")
        .unwrap()
        .card;
    run.book.insert(spell);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    hero.spells[0] = Some("plain".into());
    hero.spell_cooldowns[0] = 0;
    run.projectiles.clear();
    run.step(&BTreeMap::from([(
        1,
        Input {
            cast: 1,
            ..Default::default()
        },
    )]));
    assert_eq!(
        run.projectiles[0].sprite_card(),
        Some("comet"),
        "spell inherits held look"
    );
}

#[test]
fn worn_layers_keep_the_body_underneath_for_kit_and_avatar_knights() {
    use crate::render::overworld::arena::knights;
    let mut run = Run::new(9, 1, None);
    let hero = run.players.get_mut(&1).unwrap();
    (hero.aim_x, hero.aim_y) = (1.0, 0.0);
    let sprite = check(
        "pavise",
        &format!("name Pavise\nkind hold\narmor 1\ndraw worn at=left\n{PAVISE}\n"),
    )
    .unwrap()
    .card
    .looks
    .worn
    .unwrap();
    let body = knights::figure(hero, 1);
    let dressed = knights::dress(&body, &[&sprite], 0);
    assert!(
        dressed.w > body.w && dressed.h > body.h,
        "room for proud spikes"
    );
    assert_eq!(dressed.get(14, 5), body.get(10, 1), "head remains visible");
    let pixels = vec![[80, 120, 160, 255]; 32 * 32];
    let avatar = knights::avatar_figure(hero, &pixels, false);
    let dressed_avatar = knights::dress(&avatar, &[&sprite], 0);
    assert_eq!(dressed_avatar.get(14, 5), Some([80, 120, 160]));
    assert_ne!(
        dressed_avatar.get(5, 10),
        Some([80, 120, 160]),
        "shield covers avatar's arm"
    );
    hero.shielding = true;
    assert_ne!(
        knights::avatar_figure(hero, &pixels, false),
        avatar,
        "raised side shield is drawn"
    );
    assert_eq!(
        knights::avatar_figure(hero, &pixels, true),
        avatar,
        "a thrown shield stays off"
    );
    hero.gear = 2;
    assert_ne!(
        knights::figure(hero, 1),
        body,
        "base kit still follows health stage"
    );
}

