use super::*;

fn run() -> Run {
    Run::new(4, 2, Some("Matt"))
}
fn worn_card(run: &Run) -> Card {
    let mut card = run.mirror_paint_card(1, "a red hat").unwrap();
    card.looks.worn = Some(looks::Sprite {
        frames: vec![vec!["7".into()]],
        ..Default::default()
    });
    card
}
fn gameplay(run: &Run) -> serde_json::Value {
    let mut value = serde_json::to_value(&run.players[&1]).unwrap();
    let h = value.as_object_mut().unwrap();
    for key in ["offer", "mirror_wish"] {
        h.remove(key);
    }
    value
}

#[test]
fn cosmetic_wish_is_free_and_never_grants_stats_gear_or_genie_freedom() {
    let mut run = run();
    let before = gameplay(&run);
    let genie = run.genie.clone();
    run.begin_mirror_wish(1).unwrap();
    let wish = phrasebook::get("volley").unwrap();
    run.offer_wish(1, "volley", Some(&wish)).unwrap();
    assert_eq!(run.players[&1].offer.as_ref().unwrap().choices.len(), 1);
    assert!(run.choose(1, 1).is_err());
    run.choose(1, 0).unwrap();
    assert_eq!(gameplay(&run), before);
    assert_eq!(run.genie, genie);
    assert!(run.asked[0].cosmetic);
    assert!(!run.asked[0].fills);
    assert!(!run.asked[0].great);
}

#[test]
fn painter_output_copies_only_worn_and_survives_realm_wardrobe_roundtrip() {
    let mut run = run();
    let before = gameplay(&run);
    let mut card = worn_card(&run);
    card.effects = vec![cards::Effect::Damage(900), cards::Effect::MaxHp(900)];
    card.kind = cards::Kind::Arm;
    card.arm = Some(cards::Arm::Plain(Weapon::Bow));
    run.wear_mirror_card(1, "a red hat", &card).unwrap();
    assert_eq!(gameplay(&run), before);
    assert_eq!(
        run.players[&1].worn(&run.book).last().copied(),
        card.looks.worn.as_ref()
    );
    let saved = serde_json::to_string(&run.home.wardrobe).unwrap();
    let mut next = self::run();
    next.home.wardrobe = serde_json::from_str(&saved).unwrap();
    next.restore_gear(1);
    assert_eq!(
        next.players[&1].worn(&next.book).last().copied(),
        card.looks.worn.as_ref()
    );
    let kept = next.book.get(&key(&next.players[&1].name)).unwrap();
    assert!(kept.effects.is_empty());
    assert!(kept.arm.is_none());
    assert_eq!((kept.drop, kept.chest), (0, 0));
    assert!(next.players[&1].deck.iter().all(|id| id != &kept.id));
}

#[test]
fn late_paint_follows_named_knight_not_reused_seat_and_old_saves_default() {
    let mut run = run();
    let mut card = worn_card(&run);
    run.wear_mirror_card(1, "hat", &card).unwrap();
    let name = run.players[&1].name.clone();
    run.rename_host("Someone else");
    assert!(run.players[&1].worn(&run.book).is_empty());
    card.looks.worn.as_mut().unwrap().frames[0][0] = "8".into();
    assert!(run.update_mirror_paint(&card));
    assert!(run.players[&1].worn(&run.book).is_empty());
    run.rename_host(&name);
    assert_eq!(
        run.players[&1].worn(&run.book).last().copied(),
        card.looks.worn.as_ref()
    );
    let wardrobe: gear::Wardrobe = serde_json::from_str("{}").unwrap();
    assert!(wardrobe.mirror_looks.is_empty());
    let mut old = serde_json::to_value(&run.players[&1]).unwrap();
    old.as_object_mut().unwrap().remove("mirror_wish");
    let hero: Hero = serde_json::from_value(old).unwrap();
    assert!(!hero.mirror_wish);
}

#[test]
fn cancel_exits_cosmetic_mode_and_free_words_cannot_free_the_genie() {
    let mut run = run();
    let before = run.genie.clone();
    run.begin_mirror_wish(1).unwrap();
    run.offer_wish(1, genie::FREE_WORDS, None).unwrap();
    run.choose(1, 0).unwrap();
    assert_eq!(run.genie, before);
    run.begin_mirror_wish(1).unwrap();
    run.offer_wish(1, "hat", None).unwrap();
    assert!(run.drop_offer(1));
    assert!(!run.players[&1].mirror_wish);
    assert!(
        run.offer_wish(1, "volley", phrasebook::get("volley").as_ref())
            .is_err()
    );
}

#[test]
fn existing_card_painter_accepts_ink_only_and_roundtrips_paint_assets() {
    let run = run();
    let mut card = worn_card(&run);
    let sprite = card.looks.worn.as_mut().unwrap();
    sprite.paint = Some("a".repeat(64));
    sprite.pixel = true;
    let sprite = sprite.clone();
    let checked = cards::check(&card.id, &card.file()).unwrap().card;
    assert!(checked.effects.is_empty());
    assert_eq!(checked.looks.worn.as_ref().unwrap().paint, sprite.paint);
    let serialized = serde_json::to_string(&Look {
        words: "hat".into(),
        worn: sprite.clone(),
    })
    .unwrap();
    let restored: Look = serde_json::from_str(&serialized).unwrap();
    assert_eq!(restored.worn, sprite);
    assert!(cards::check("empty-held", "name Empty\nkind hold\nrarity common\n").is_err());
}

#[test]
fn first_paint_can_land_after_its_owner_leaves_and_newlines_are_not_card_directives() {
    let mut run = run();
    let name = run.players[&1].name.clone();
    run.begin_mirror_wish(1).unwrap();
    run.offer_wish(1, "hat\ndamage 900", None).unwrap();
    run.choose(1, 0).unwrap();
    let mut card = run.mirror_paint_card(1, "hat\ndamage 900").unwrap();
    assert!(card.file().contains("text hat damage 900\n"));
    assert!(!card.file().contains("\ndamage 900"));
    card.looks.worn = worn_card(&run).looks.worn;
    run.rename_host("a different knight");
    assert!(run.update_mirror_paint(&card));
    run.rename_host(&name);
    assert_eq!(
        run.players[&1].worn(&run.book).last().copied(),
        card.looks.worn.as_ref()
    );
}

#[test]
fn cancelling_before_words_and_choosing_with_wishes_held_never_spends() {
    let mut run = run();
    run.players.get_mut(&1).unwrap().wishes = 3;
    run.players.get_mut(&1).unwrap().great_wishes = 2;
    let before = gameplay(&run);
    run.begin_mirror_wish(1).unwrap();
    let inputs = BTreeMap::from([(
        1,
        Input {
            activity_menu: true,
            ..Default::default()
        },
    )]);
    run.offer_inputs(&inputs).unwrap();
    assert!(!run.players[&1].mirror_wish);
    assert_eq!(gameplay(&run), before);
    run.begin_mirror_wish(1).unwrap();
    run.offer_wish(1, "upgrade my bow", None).unwrap();
    run.choose(1, 0).unwrap();
    assert_eq!(gameplay(&run), before);
    assert!(run.asked.iter().all(|a| a.cosmetic));
}

#[test]
fn a_guest_snapshot_and_a_new_seat_restore_the_saved_worn_asset() {
    let mut run = run();
    let mut card = worn_card(&run);
    card.looks.worn.as_mut().unwrap().paint = Some("b".repeat(64));
    run.wear_mirror_card(1, "hat", &card).unwrap();
    let name = run.players[&1].name.clone();
    run.join_seat(2, &name);
    assert_eq!(
        run.players[&2].worn(&run.book).last().copied(),
        card.looks.worn.as_ref()
    );
    let guest: Run = serde_json::from_slice(&serde_json::to_vec(&run).unwrap()).unwrap();
    assert_eq!(
        guest.players[&2].worn(&guest.book).last().copied(),
        card.looks.worn.as_ref()
    );
    run.begin_mirror_wish(1).unwrap();
    run.rename_host("not the same knight");
    assert!(!run.players[&1].mirror_wish);
}

#[test]
fn queued_painter_keeps_its_owner_when_the_seat_is_reused_before_dispatch() {
    let mut run = run();
    let name = run.players[&1].name.clone();
    run.begin_mirror_wish(1).unwrap();
    run.offer_wish(1, "silver hat", None).unwrap();
    run.choose(1, 0).unwrap();
    let asked = run.asked[0].clone();
    run.rename_host("somebody else");
    assert_eq!(asked.cosmetic_name, name);
    let mut card = run
        .mirror_paint_card_for(&asked.cosmetic_name, &asked.words)
        .unwrap();
    card.looks.worn = worn_card(&run).looks.worn;
    assert!(run.update_mirror_paint(&card));
    assert!(run.players[&1].worn(&run.book).is_empty());
    run.rename_host(&name);
    assert_eq!(
        run.players[&1].worn(&run.book).last().copied(),
        card.looks.worn.as_ref()
    );
}
