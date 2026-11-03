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
    let wide = vec!["5".repeat(17)];
    assert!(
        Sprite::from_strip(Slot::Shot, &wide)
            .unwrap_err()
            .contains("at most 16x16")
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
    // Its full silhouette fits the shield arm. An oversized source is
    // sampled to the part, so a single source accent need not survive.
    let [x, y, w, h] = ink_bounds(&worn.frames).unwrap();
    assert!(w <= 8 && h <= 12, "{w}x{h}");
    assert!((x + w / 2 - 6).abs() <= 1 && (y + h / 2 - 13).abs() <= 1);
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
        dressed.w > body.w && dressed.h >= body.h,
        "room for the fitted shield beside the body"
    );
    let pad = (dressed.w - body.w) / 2;
    assert_eq!(dressed.get(pad + 10, 1), body.get(10, 1), "head remains visible");
    let pixels = vec![[80, 120, 160, 255]; 32 * 32];
    let avatar = knights::avatar_figure(hero, &pixels, false);
    let dressed_avatar = knights::dress(&avatar, &[&sprite], 0);
    assert_eq!(dressed_avatar.get(pad + 10, 1), Some([80, 120, 160]));
    assert_ne!(
        dressed_avatar.get(pad + 2, 9),
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


const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn motion_words_are_a_short_list() {
    for m in Move::ALL {
        assert_eq!(Move::parse(m.word()), Some(m));
    }
    assert_eq!(Move::parse("Bob"), Some(Move::Bob));
    assert_eq!(Move::parse("twirl"), None);
    assert_eq!(
        Move::parse_list("stretch,bob").unwrap(),
        [Move::Stretch, Move::Bob]
    );
    assert!(Move::parse_list("bob,twirl").unwrap_err().contains("move `twirl`: use stretch"));
    assert!(Move::parse_list("bob,bob").unwrap_err().contains("twice"));
    assert!(Move::parse_list("").is_err());
    assert!(paint_name(SHA));
    assert!(!paint_name(&SHA.to_uppercase()) && !paint_name(&SHA[1..]) && !paint_name("../x"));
}

#[test]
fn a_painted_moving_sprite_reads_back_from_its_block() {
    let raw = format!(
        "name Comet\nkind hold\ndamage 5\ndraw shot move=stretch,bob paint={SHA}\n.5.|5.5\n555|555\n\ndraw worn at=head paint={SHA}\nH....H\n.HiiH.\n"
    );
    let card = check("comet", &raw).unwrap().card;
    let shot = card.looks.shot.as_ref().unwrap();
    assert_eq!(shot.paint.as_deref(), Some(SHA));
    assert_eq!(shot.moves, [Move::Stretch, Move::Bob]);
    assert_eq!(shot.frames.len(), 2);
    let block = shot.to_block(Slot::Shot);
    assert_eq!(
        block,
        format!("draw shot paint={SHA} move=stretch,bob\n.5.|5.5\n555|555\n")
    );
    let worn = card.looks.worn.as_ref().unwrap();
    assert_eq!(worn.size(), (24, 24), "put on the body as before");
    assert_eq!(worn.paint.as_deref(), Some(SHA));
    // Written back and read again: the same sprites.
    let again = check(
        "comet",
        &format!(
            "name Comet\nkind hold\ndamage 5\n{block}\n{}",
            worn.to_block(Slot::Worn)
        ),
    )
    .unwrap()
    .card;
    assert_eq!(again.looks, card.looks);
    // A sprite in inks alone writes the header it always did, and saves
    // without the new fields.
    let plain = check("p", "name P\nkind hold\ndamage 5\ndraw shot\n.5.\n")
        .unwrap()
        .card;
    let sprite = plain.looks.shot.as_ref().unwrap();
    assert_eq!(sprite.to_block(Slot::Shot), "draw shot\n.5.\n");
    let json = serde_json::to_string(sprite).unwrap();
    assert_eq!(json, r#"{"frames":[[".5."]]}"#);
    assert_eq!(serde_json::from_str::<Sprite>(&json).unwrap(), *sprite);
}

#[test]
fn a_bad_paint_or_move_is_a_card_error() {
    let bad = |header: &str| errors(&format!("name X\nkind hold\ndamage 5\ndraw {header}\n.5.\n"));
    assert!(bad("shot paint=abc").contains("64 hex digits"));
    assert!(bad("shot move=twirl").contains("move `twirl`: use stretch, bob"));
    assert!(bad(&format!("shot paint={SHA} paint={SHA}")).contains("`paint=` appears twice"));
    assert!(bad("shot move=bob move=spin").contains("`move=` appears twice"));
    assert!(bad("shot glow=1").contains("may add `paint=<sha256>` (and `pixel`) and `move=<words>`"));
    assert!(bad("shot at=left").contains("at=<place>"));
}

#[test]
fn a_painted_card_survives_the_save_and_the_wire() {
    let mut run = Run::new(3, 1, None);
    let card = check(
        "comet",
        &format!("name Comet\nkind hold\ndamage 5\ndraw shot paint={SHA} move=spin\n.5.\n555\n"),
    )
    .unwrap()
    .card;
    run.book.insert(card.clone());
    let json = serde_json::to_string(&run).unwrap();
    let saved: Run = serde_json::from_str(&json).unwrap();
    assert_eq!(saved.book.get("comet"), Some(&card));
    let tree = crate::wire::publish(&run).unwrap();
    let sent = crate::wire::read(&tree).unwrap();
    let shot = sent.book.get("comet").unwrap().looks.shot.clone().unwrap();
    assert_eq!(shot.paint.as_deref(), Some(SHA));
    assert_eq!(shot.moves, [Move::Spin]);
}

#[test]
fn worn_rows_are_repaired_to_their_part_before_a_wish_is_accepted() {
    for at in ["head", "face", "body", "back", "left", "right", "feet", "aura"] {
        let rows = vec!["H".repeat(24); 24].join("\n");
        let entry = format!("wish fitted | Fitted\nsay fitted\ntier armor 1 :: Fitted.\ndraw worn at={at}\n{rows}\n");
        let (wishes, errors) = crate::rules::phrasebook::read_checked(&entry);
        assert!(errors.is_empty(), "{at}: {errors:?}");
        let sprite = wishes[0].card(0).unwrap().looks.worn.unwrap();
        let [_, _, w, h] = ink_bounds(&sprite.frames).unwrap();
        let size = Body::KNIGHT.part(at).unwrap().size;
        assert!(w <= size.0 && h <= size.1, "{at}: {w}x{h} exceeds {size:?}");
        assert!(sprite.frames[0].iter().any(|r| r.contains('H')));
        let reread = check("fitted", &format!("name Fitted\nkind hold\narmor 1\n{}", sprite.to_block(Slot::Worn))).unwrap().card;
        assert_eq!(sprite, reread.looks.worn.unwrap(), "fit survives save and reload");
    }
    assert!(errors("name X\nkind hold\narmor 1\ndraw worn at=head\nH\n".replace("H\n", &format!("{}\n", "H".repeat(25))).as_str()).contains("at most 24x24"));
}

#[test]
fn worn_parts_distinguish_a_face_a_cape_and_a_ground_aura() {
    assert_eq!(place("mask"), Some("face"));
    assert_eq!(place("cape"), Some("back"));
    assert_eq!(place("hands"), Some("right"));
    assert!(Body::KNIGHT.part("back").unwrap().under);
    assert!(Body::KNIGHT.part("aura").unwrap().under);
    assert!(!Body::KNIGHT.part("face").unwrap().under);
}

#[test]
fn a_sprite_may_colour_inks_of_its_own() {
    // The player's own purple: any hue, named on the draw line. `%` and `+`
    // are the realm's violet and pink dyes, so the own ink here is `~`.
    let raw = "name Purple Drank\nkind hold\narmor 1\ndraw orbit pal=~7a2bd1,&e0b0ff\n.~.|.&.\n~&~|&~&\n.~.|.&.\n";
    let card = check("drank", raw).unwrap().card;
    let orbit = card.looks.orbit.as_ref().expect("orbit");
    assert_eq!(orbit.pal, vec![('~', [0x7a, 0x2b, 0xd1]), ('&', [0xe0, 0xb0, 0xff])]);
    assert_eq!(orbit.rgb('~'), Some([0x7a, 0x2b, 0xd1]));
    assert_eq!(orbit.rgb('k'), crate::render::overworld::ink::ink('k'), "realm inks stay the realm's");
    let rows: Vec<&str> = orbit.frame(0).iter().map(String::as_str).collect();
    let im = crate::render::overworld::ink::Img::from_rows_pal(&rows, &orbit.pal);
    assert_eq!(im.get(1, 0), Some([0x7a, 0x2b, 0xd1]), "drawn in its own colour");
    assert_eq!(crate::render::overworld::ink::Img::from_rows(&rows).get(1, 0), None, "never a realm ink");
    // Marked as the realm inks nearest it, so its drawing still grades.
    let realm = realm_rows(&orbit.frames[0], &orbit.pal);
    assert!(realm.concat().chars().all(|c| c == '.' || crate::rules::script::INKS.contains(c)));
    // Back to the block that makes it, and as data.
    assert!(orbit.to_block(Slot::Orbit).starts_with("draw orbit pal=~7a2bd1,&e0b0ff\n"));
    let back: cards::Card = serde_json::from_str(&serde_json::to_string(&card).unwrap()).unwrap();
    assert_eq!(back, card);
    // An own ink with no colour, a colour that is not one, a realm ink.
    assert!(errors("name X\nkind hold\narmor 1\ndraw orbit\n.~.\n~~~\n").contains("gives it no colour"));
    assert!(errors("name X\nkind hold\narmor 1\ndraw orbit pal=~7a2bz1\n.~.\n").contains("six hex digits"));
    assert!(errors("name X\nkind hold\narmor 1\ndraw orbit pal=a7a2bd1\n.a.\n").contains("own ink is one of"));
}

#[test]
fn overlapping_palette_inks_fall_back_to_realm_or_use_the_override() {
    let raw = "name Dye\nkind hold\narmor 1\ndraw orbit\n.%+\n";
    let card = check("dye", raw).unwrap().card;
    let orbit = card.looks.orbit.unwrap();
    assert_eq!(orbit.unnamed_ink(), None);
    assert_eq!(orbit.rgb('%'), crate::ink::ink('%'));
    assert_eq!(orbit.rgb('+'), crate::ink::ink('+'));

    let raw = "name Dye\nkind hold\narmor 1\ndraw orbit pal=%123456\n.%+\n";
    let orbit = check("dye", raw).unwrap().card.looks.orbit.unwrap();
    assert_eq!(orbit.rgb('%'), Some([0x12, 0x34, 0x56]));
    assert_eq!(orbit.rgb('+'), crate::ink::ink('+'));
    assert!(errors("name X\nkind hold\narmor 1\ndraw orbit\n.&.\n").contains("gives it no colour"));
}
