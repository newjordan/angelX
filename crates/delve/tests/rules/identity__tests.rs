use super::*;

#[test]
fn the_scrolls_own_answers_all_check_and_make_a_piece() {
    let errors = phrasebook::read_checked(phrasebook::TEXT).1;
    assert!(errors.is_empty(), "{errors:?}");
    let book = phrasebook::read(phrasebook::TEXT);
    let piece = |id: &str| piece_of_wish(book.iter().find(|w| w.id == id).unwrap_or_else(|| panic!("{id}")));
    assert_eq!(piece("plain-frame"), Some(Piece::Part("body")));
    assert_eq!(piece("great-helm"), Some(Piece::Part("head")));
    assert_eq!(piece("iron"), Some(Piece::Part("shoulders")));
    assert_eq!(piece("breastplate"), Some(Piece::Part("body")));
    assert_eq!(piece("buckler"), Some(Piece::Part("left")));
    assert_eq!(piece("gauntlet"), Some(Piece::Part("right")));
    assert_eq!(piece("boots"), Some(Piece::Part("feet")));
    for id in ["yew-bow", "war-crossbow", "hand-cannon", "bright-blade"] {
        assert_eq!(piece(id), Some(Piece::Weapon), "{id}");
    }
    for id in ["pilgrims-arrow", "ladys-veil", "trebuchet", "bladewind", "stillhour", "axe-storm"] {
        assert_eq!(piece(id), Some(Piece::Ultimate), "{id}");
    }
    assert_eq!(piece("firebolt"), Some(Piece::Magic));
    assert_eq!(piece("volley"), None);
    for ask in Ask::ROOMS {
        let id = neutral(ask);
        assert!(book.iter().any(|w| w.id == id), "{id}");
    }
}

/// A delve begun as the game begins it with the identity start: the host
/// and `friends` more knights, walked into the first room of the first
/// floor.
pub(crate) fn shapes(friends: u32) -> Run {
    let mut run = Run::at_home(7, 1, None, home::Home::default(), Default::default());
    run.naked_start = true;
    run.identity = true;
    run.rename_host("Ada");
    for id in 2..=friends + 1 {
        run.join_seat(id, ["Bors", "Cai", "Dag", "Ena", "Fen"][id as usize - 2]);
    }
    run.descend_for_test();
    run
}

fn wish(run: &mut Run, id: u32, wish: &str) -> Result<String, String> {
    let w = phrasebook::get(wish).unwrap_or_else(|| panic!("{wish}"));
    run.wish(id, &w)
}

/// Walk the party into room `index` of the first floor.
fn walk_in(run: &mut Run, index: usize) {
    run.calm_for_test();
    run.enemies.clear();
    run.enter(index, Some(2));
}

#[test]
fn a_knight_begins_as_a_bare_shape_with_the_ball() {
    let run = shapes(3);
    let shapes: Vec<Option<Shape>> = run.players.values().map(|h| h.shape).collect();
    assert_eq!(shapes, [Some(Shape::Square), Some(Shape::Triangle), Some(Shape::Circle), Some(Shape::Diamond)]);
    for h in run.players.values() {
        assert_eq!(h.arm.as_deref(), Some(BALL), "{}", h.name);
        assert!(h.forged.as_ref().is_some_and(|w| w.bolt.is_some()), "the ball is thrown");
        assert!(h.spells.iter().all(Option::is_none));
        assert!(h.hand.is_empty());
        assert_eq!(h.bombs, super::super::bombs::at(1).restock, "first descent stocks the bomb belt");
        assert_eq!(h.guard, cards::Guard::Roll);
        assert_eq!(h.max_hp, SHAPE_HP);
        assert_eq!(h.owned, gear::Owned::default());
        assert!(h.ult.is_none() && h.unmade(&run.book));
        assert_eq!(h.wishes, 1, "the first room's wish, not the start's three");
    }
    assert!(run.dungeon.rooms.iter().flat_map(|r| &r.items).all(|i| !["bow", "crossbow", "handgonne"].contains(&i.card.as_str())));
}

#[test]
fn the_first_floor_is_three_rooms_a_wish_each() {
    let mut run = shapes(1);
    assert_eq!(run.dungeon.depth, 1);
    let kinds: Vec<RoomKind> = run.dungeon.rooms.iter().map(|r| r.kind).collect();
    assert_eq!(kinds, [RoomKind::Start, RoomKind::Fight, RoomKind::Stairs]);
    assert!(run.notices.iter().any(|n| n.contains("What are you?")), "{:?}", run.notices);
    walk_in(&mut run, 1);
    assert!(run.players.values().all(|h| h.wishes == 2));
    // Walking back and in again gives nothing more.
    walk_in(&mut run, 0);
    walk_in(&mut run, 1);
    assert!(run.players.values().all(|h| h.wishes == 2));
    walk_in(&mut run, 2);
    assert!(run.players.values().all(|h| h.wishes == 3));
    assert!(run.cues.iter().any(|c| c == "identity_room:ultimate"));
    // The second floor is the ordinary game.
    run.descend_for_test();
    assert!(run.dungeon.rooms.len() > 3);
    assert!(run.players.values().all(|h| h.wishes == 3 + overclass::FLOOR_WISHES));
}

#[test]
fn the_questions_are_answered_in_order_and_nothing_else_is_granted() {
    let mut run = shapes(0);
    run.players.get_mut(&1).unwrap().wishes = 3;
    let refused = wish(&mut run, 1, "volley").unwrap_err();
    assert!(refused.contains("What are you?"), "{refused}");
    assert!(wish(&mut run, 1, "yew-bow").unwrap_err().contains("What are you?"), "the body first");
    assert!(wish(&mut run, 1, "firebolt").is_err(), "spells come later");
    assert_eq!(run.players[&1].wishes, 3, "nothing spent");
    wish(&mut run, 1, "great-helm").unwrap();
    assert!(run.players[&1].owned.helmet, "a head guards as a helmet does");
    assert!(wish(&mut run, 1, "boots").unwrap_err().contains("What do you fight with?"));
    wish(&mut run, 1, "yew-bow").unwrap();
    assert_eq!(run.players[&1].weapon, Weapon::Bow);
    assert!(run.players[&1].forged.is_none(), "the ball is put down");
    assert!(run.players[&1].unmade(&run.book));
    wish(&mut run, 1, "trebuchet").unwrap();
    let h = &run.players[&1];
    assert_eq!(h.ult, Some(ults::Ult::Trebuchet));
    assert!(!h.unmade(&run.book) && h.made);
    assert!(run.cues.iter().any(|c| c == "made"));
    assert!(run.notices.iter().any(|n| n.contains("is whole")), "{:?}", run.notices);
    // Whole: spells, power and more parts are ordinary wishes.
    run.players.get_mut(&1).unwrap().wishes = 3;
    wish(&mut run, 1, "firebolt").unwrap();
    wish(&mut run, 1, "volley").unwrap();
    wish(&mut run, 1, "boots").unwrap();
    assert_eq!(run.players[&1].making(&run.book).unwrap().parts, ["head", "feet"]);
}

#[test]
fn an_ultimate_charges_only_once_it_is_wished() {
    let mut run = shapes(0);
    walk_in(&mut run, 1);
    run.spawn_at_for_test(EnemyKind::Skeleton, 10.0, 10.0);
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    assert_eq!(run.players[&1].ult_charge, 0);
    run.players.get_mut(&1).unwrap().wishes = 3;
    for w in ["breastplate", "hand-cannon", "stillhour"] {
        wish(&mut run, 1, w).unwrap();
    }
    run.calm_for_test();
    run.spawn_at_for_test(EnemyKind::Skeleton, 10.0, 10.0);
    for _ in 0..60 {
        run.step(&BTreeMap::new());
    }
    assert!(run.players[&1].ult_charge > 0);
}

#[test]
fn a_super_wish_waits_until_the_knight_is_whole() {
    let mut run = shapes(0);
    run.players.get_mut(&1).unwrap().wishes = 0;
    run.players.get_mut(&1).unwrap().great_wishes = 1;
    let w = phrasebook::get("great-helm").unwrap();
    let why = run.offer_wish(1, &w.name, Some(&w)).unwrap_err();
    assert!(why.contains("super wish waits"), "{why}");
    run.players.get_mut(&1).unwrap().wishes = 1;
    run.offer_wish(1, &w.name, Some(&w)).unwrap();
    let offer = run.players[&1].offer.clone().unwrap();
    assert_eq!(offer.choices[0].steps, 1);
    assert_eq!(offer.ask.as_deref(), Some("What are you?"));
    run.choose(1, 0).unwrap();
    assert_eq!((run.players[&1].wishes, run.players[&1].great_wishes), (0, 1));
}

#[test]
fn the_genies_whim_for_an_unmade_knight_answers_the_question() {
    for seed in 0..12 {
        let mut run = shapes(0);
        run.seed = seed;
        let w = phrasebook::get("great-helm").unwrap();
        run.offer_wish(1, &w.name, Some(&w)).unwrap();
        for c in &run.players[&1].offer.as_ref().unwrap().choices {
            if let Some(id) = &c.wish {
                let piece = piece_of_wish(&phrasebook::get(id).unwrap()).unwrap();
                assert!(matches!(piece, Piece::Part(_)), "seed {seed}: {id}");
            }
        }
    }
}

#[test]
fn words_spent_by_an_unmade_knight_carry_the_question_to_the_learner() {
    let mut run = shapes(0);
    run.offer_wish(1, "an ogre with a stone hide", None).unwrap();
    run.choose(1, 0).unwrap();
    let asked = run.asked.pop().unwrap();
    let making = asked.making.unwrap();
    assert_eq!(making.asks().first(), Some(&Ask::Body));
    assert!(making.brief().contains("What are you?"), "{}", making.brief());
}

#[test]
fn with_no_one_to_learn_the_words_the_scroll_grants_its_plain_answer() {
    let mut run = shapes(0);
    run.players.get_mut(&1).unwrap().wishes = 3;
    for (n, want) in ["plain frame", "yew bow", "pilgrim's arrow"].into_iter().enumerate() {
        let granted = run.grant_neutral(1).unwrap();
        assert!(granted.to_lowercase().contains(want), "{n}: {granted}");
    }
    assert!(!run.players[&1].unmade(&run.book));
    assert!(run.grant_neutral(1).is_err());
}

#[test]
fn a_learned_ultimate_names_one_the_game_has() {
    let book = phrasebook::read(
        "wish dragon-roar | Dragon's Roar\nsay a dragon's roar\ntier ult grand_chord :: Dragon's roar: the room is pulled to you, stunned, because dragons command.\n",
    );
    assert_eq!(book.len(), 1);
    let card = book[0].card(0).unwrap();
    assert_eq!(card.ult, Some(ults::Ult::GrandChord));
    assert_eq!(piece_of(&card), Some(Piece::Ultimate));
    let bad = phrasebook::read_checked("wish x | X\nsay x\ntier ult meteor_swarm :: no.\n");
    assert!(bad.0.is_empty() && !bad.1.is_empty());
    assert!(cards::check("x", "name X\nkind arm\narm bow\nult stillhour\n").is_err(), "an ultimate is held");
}

#[test]
fn identity_off_keeps_the_kit_start() {
    let mut run = Run::at_home(7, 1, None, home::Home::default(), Default::default());
    run.naked_start = true;
    run.rename_host("Ada");
    run.join_seat(2, "Bors");
    run.outfit(2, knights::knight("gareth").unwrap());
    assert!(run.players.values().all(|h| h.shape.is_none()));
    run.descend_for_test();
    assert!(run.dungeon.rooms.len() > 3);
    assert!(run.players.values().all(|h| h.wishes == overclass::START_WISHES));
    assert!(run.dungeon.rooms[0].items.iter().any(|i| ["bow", "crossbow", "handgonne"].contains(&i.card.as_str())), "the cache waits");
    assert!(wish(&mut run, 1, "volley").is_ok());
    assert!(!on(), "experimental: off unless ANGEL_DELVE_IDENTITY=1");
}

#[test]
fn the_intros_knight_keeps_its_name_and_colours_but_not_its_kit() {
    let mut run = shapes(1);
    run.outfit(2, knights::knight("gareth").unwrap());
    let h = &run.players[&2];
    assert_eq!(h.knight.as_deref(), Some("gareth"));
    assert_eq!(h.shape, Some(Shape::Triangle));
    assert_eq!(h.arm.as_deref(), Some(BALL));
    assert_eq!(h.bombs, 0);
    assert_eq!(h.max_hp, SHAPE_HP);
}

#[test]
fn a_class_taken_up_by_a_shape_is_a_calling_not_a_kit() {
    let mut run = shapes(0);
    let mage = classes::CLASSES.iter().find(|c| c.name == "Mage").unwrap();
    run.become_class(1, mage);
    let h = &run.players[&1];
    assert_eq!(h.class.as_deref(), Some("Mage"));
    assert_eq!(h.arm.as_deref(), Some(BALL));
    assert!(h.spells.iter().all(Option::is_none));
}

#[test]
fn a_shapes_save_loads_and_an_old_save_has_no_shape() {
    let mut run = shapes(1);
    wish(&mut run, 1, "great-helm").unwrap();
    let text = serde_json::to_string(&run).unwrap();
    let back: Run = serde_json::from_str(&text).unwrap();
    assert!(back.valid_snapshot());
    assert_eq!(back.players[&1].shape, Some(Shape::Square));
    assert_eq!(back.players[&1].making(&back.book).unwrap().parts, ["head"]);
    let old = text.replace(r#""shape":"square","#, "").replace(r#""shape":"triangle","#, "");
    let old: Run = serde_json::from_str(&old).unwrap();
    assert!(old.valid_snapshot());
    assert!(old.players[&1].shape.is_none());
}

#[test]
fn a_worn_piece_remembers_its_place_through_the_card_text() {
    let w = phrasebook::get("buckler").unwrap();
    let card = w.card(0).unwrap();
    assert_eq!(card.looks.worn.as_ref().unwrap().at.as_deref(), Some("left"));
    let again = cards::check("again", &card.file()).unwrap().card;
    assert_eq!(again.looks.worn, card.looks.worn);
    let ult = phrasebook::get("phantasm").unwrap().card(0).unwrap();
    assert_eq!(cards::check("again", &ult.file()).unwrap().card.ult, ult.ult);
}

#[test]
fn the_streamlined_floor_is_one_bare_room_with_all_three_wishes() {
    set_simple(true);
    let mut run = shapes(2);
    assert!(run.simple_creation());
    assert_eq!(run.dungeon.rooms.len(), 1, "one room");
    assert!(run.boss_gates.is_none());
    for _ in 0..60 {
        run.step(&Default::default());
    }
    assert!(run.enemies.is_empty(), "no foes");
    assert!(run.frogs.wild.is_empty() && run.works.perils.is_empty() && run.traps.is_empty());
    assert!(run.pots_of(0).is_empty(), "no props");
    let room = run.room();
    assert!(room.roster.is_empty() && room.items.is_empty() && room.chest.is_none());
    let tiles: Vec<Tile> = (0..room.rows as i32).flat_map(|r| (0..room.cols as i32).map(move |c| (c, r))).map(|(c, r)| room.tile(c, r)).collect();
    assert!(tiles.iter().all(|t| matches!(t, Tile::Floor | Tile::Wall | Tile::Door | Tile::Stairs)), "no blocks or hazards");
    assert!((0..room.rows as i32).any(|r| (0..room.cols as i32).any(|c| room.tile(c, r) == Tile::Stairs)), "the way down");
    let ids: Vec<u32> = run.players.keys().copied().collect();
    assert!(run.players.values().all(|h| h.wishes as usize == ROOMS), "three wishes each");
    // The stairs wait for the knights to be made, then open.
    assert!(run.creation_holds_stairs());
    for &id in &ids {
        for _ in 0..ROOMS {
            run.grant_neutral(id).unwrap();
        }
    }
    assert!(run.players.values().all(|h| h.made));
    assert!(!run.creation_holds_stairs());
    set_simple(false);
}

#[test]
fn the_streamlined_room_is_never_dared_to_fell_a_guardian_it_does_not_hold() {
    use crate::rules::dares::DareKind;
    set_simple(true);
    for seed in 0..60 {
        let mut run = Run::at_home(seed, 1, None, home::Home::default(), Default::default());
        run.naked_start = true;
        run.identity = true;
        run.rename_host("Ada");
        run.join_seat(2, "Bors");
        run.descend_for_test();
        assert!(run.simple_creation());
        assert!(!run.floor_has_guardian(), "seed {seed}: the room holds no guardian");
        let kind = run.dare.as_ref().map(|d| d.kind);
        assert_ne!(kind, Some(DareKind::MakeItQuick), "seed {seed}: no guardian to fell");
    }
    set_simple(false);
    // A floor that does hold its guardian may still be dared to fell it.
    let mut quick = false;
    for seed in 0..60 {
        let mut run = Run::new(seed, 1, None);
        run.descend_for_test();
        assert!(run.floor_has_guardian(), "seed {seed}: floor one keeps its guardian");
        quick |= run.dare.as_ref().is_some_and(|d| d.kind == DareKind::MakeItQuick);
    }
    assert!(quick, "Make It Quick still comes up where a guardian waits");
}
