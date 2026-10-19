use super::*;

#[test]
fn shield_wall_wishes_are_checked_guard_cards_not_passive_armor() {
    let wishes = read(
        "wish double-wide-shield | Double Wide Shield\nkind guard\nsay a double wide shield | double shield | shield wall\ntier guard wall width=2 drain=10 empower=0 :: Hold Space for a two-wide shield wall.\ntier guard wall width=3 drain=15 empower=0 :: Hold Space for a three-wide shield wall.\ntier guard wall width=5 drain=25 empower=0 :: Hold Space for a five-wide shield wall.\n",
    );
    assert_eq!(wishes.len(), 1);
    for (tier, width) in [2, 3, 5].into_iter().enumerate() {
        let card = wishes[0].card(tier).expect("a valid shield wall tier");
        assert_eq!(card.kind, cards::Kind::Guard);
        assert!(matches!(card.guard, Some(cards::Guard::Wall(wall)) if wall.width == width));
        assert!(card.effects.is_empty(), "not an armor substitute");
    }
}

#[test]
fn every_wish_in_the_phrasebook_is_a_card_the_checker_takes_at_every_tier() {
    assert!(book().len() >= 12, "{}", book().len());
    for wish in book().iter() {
        assert!(!wish.says.is_empty(), "{} has phrases", wish.id);
        for tier in 0..wish.tiers.len() {
            let card = wish
                .card(tier)
                .unwrap_or_else(|e| panic!("{} tier {tier}: {e}", wish.id));
            assert_eq!(card.kind, cards::Kind::Hold);
            assert!(
                !card.effects.is_empty(),
                "{} tier {tier} does something",
                wish.id
            );
        }
    }
}

#[test]
fn players_words_find_the_wish_they_mean() {
    let id = |said: &str| find(said).map(|w| w.id);
    assert_eq!(id("triple wide fire my shots"), Some("volley".to_string()));
    assert_eq!(
        id("make my shots bounce off the walls"),
        Some("ricochet".to_string())
    );
    assert_eq!(id("I want homing shots please"), Some("seeker".to_string()));
    assert_eq!(
        id("fire faster"),
        Some("quick".to_string()),
        "the fuller phrase wins over `faster`"
    );
    assert_eq!(id("move faster"), Some("fleet".to_string()));
    assert_eq!(id("heal my friend when I hit"), Some("mend".to_string()));
    assert_eq!(id("chain lightning"), Some("chain".to_string()));
    assert_eq!(id("make my shots explosive"), Some("powder".to_string()));
    assert_eq!(id("summon a dragon to ride"), None);
}

#[test]
fn a_wish_climbs_its_ladder_and_stops_at_the_top() {
    let volley = get("volley").unwrap();
    let mut deck: Vec<String> = Vec::new();
    assert_eq!(volley.next_tier(&deck), Some(0));
    assert!(volley.card_text(0).contains("volley 8"));
    deck.push(volley.card_id(0));
    assert_eq!(volley.next_tier(&deck), Some(1));
    deck = vec![volley.card_id(2)];
    assert_eq!(volley.next_tier(&deck), None);
}

#[test]
fn wish_ids_fit_the_card_id_at_every_tier_without_losing_the_ladder_marker() {
    let id = "a".repeat(MAX_ID);
    let tiers = "tier damage 5 :: A gift.\n".repeat(MAX_TIERS);
    let (book, errors) = read_checked(&format!("wish {id} | Gift\nsay gift\n{tiers}"));
    assert!(errors.is_empty(), "{errors:?}");
    for tier in 0..MAX_TIERS {
        let wish = &book[0];
        assert_eq!(wish.card(tier).unwrap().id, wish.card_id(tier));
        assert_eq!(
            wish.next_tier(&[wish.card_id(tier)]),
            (tier + 1 < MAX_TIERS).then_some(tier + 1)
        );
    }
    let (book, errors) = read_checked(&format!(
        "wish {id}a | Too Long\nsay gift\ntier armor 1 :: Gift.\n"
    ));
    assert!(book.is_empty() && errors.join(" ").contains("24 characters"));
}

#[test]
fn drawn_wishes_carry_their_own_art() {
    let volley = get("volley").unwrap().card(0).unwrap();
    let rows = volley.art_rows();
    assert!(
        rows.len() >= 10,
        "the volley icon, not the kind's glyph: {rows:?}"
    );
}

#[test]
fn an_entry_may_carry_its_own_card_art() {
    let book = read(
        "wish glow | Glow\nsay make it glow\ntier homing 1 :: It glows.\nart\n..5..\n.545.\n..5..\n\nwish next | Next\nsay next\ntier rate 5 :: Next.\n",
    );
    assert_eq!(book.len(), 2);
    assert_eq!(book[0].art, vec!["..5..", ".545.", "..5.."]);
    assert!(book[1].art.is_empty());
    let card = book[0].card(0).unwrap();
    assert_eq!(card.art_rows().len(), 3, "the entry's art, not a glyph");
}

#[test]
fn a_creative_wish_goes_to_be_learned_not_to_the_nearest_stock_card() {
    let book = read(TEXT);
    let id = |said: &str| find_in(&book, said, &[]).map(|w| w.id);
    // Words no stock phrase covers make a new thing.
    assert_eq!(id("golden spiky armor that explodes"), None);
    assert_eq!(id("spiky armor that spins around me"), None);
    assert_eq!(id("a bigger spiked shield"), None);
    assert_eq!(id("bigger spiked shield"), None);
    assert_eq!(id("armor that explodes"), None);
    assert_eq!(id("tiny golden devils that explode"), None);
    // Stock wishes still land at once, filler and all.
    assert_eq!(id("armor"), Some("iron".into()));
    assert_eq!(id("give me some armor please"), Some("iron".into()));
    assert_eq!(id("spin around me"), Some("morningstar".into()));
    assert_eq!(
        id("make my shots bounce off the walls"),
        Some("ricochet".into())
    );
}

#[test]
fn a_tie_goes_to_the_learned_entry_and_a_full_ladder_is_passed_over() {
    let text = format!(
        "{TEXT}\n{LEARNED}\nwish iron-pauldrons | Iron Pauldrons\nsay armor | pauldrons\ntier armor 1 :: Pauldrons.\n"
    );
    let book = read(&text);
    let learned = book.iter().find(|w| w.id == "iron-pauldrons").unwrap();
    assert!(learned.learned);
    assert!(!book.iter().find(|w| w.id == "iron").unwrap().learned);
    assert_eq!(
        find_in(&book, "armor", &[]).map(|w| w.id),
        Some("iron-pauldrons".into()),
        "the same cover: the learned entry"
    );
    // Holding the learned one in full, the stock one is found.
    let full = vec![learned.card_id(0)];
    assert_eq!(
        find_in(&book, "armor", &full).map(|w| w.id),
        Some("iron".into())
    );
    // Holding both in full, nothing is.
    let iron = book.iter().find(|w| w.id == "iron").unwrap();
    let both = vec![learned.card_id(0), iron.card_id(2)];
    assert_eq!(find_in(&book, "armor", &both), None);
}

#[test]
fn a_turned_away_entry_says_why() {
    let (book, errors) = read_checked(
        "wish loud | Loud\nsay loud\ntier explode 5 :: Boom.\n\nwish big | Big Art\nsay big art\ntier damage 5 :: Big.\nart\n.................\n\nwish fine | Fine\nsay fine\ntier damage 5 :: Fine.\n",
    );
    assert_eq!(book.len(), 1);
    assert_eq!(book[0].id, "fine");
    let errors = errors.join(" | ");
    assert!(
        errors.contains("wish loud tier 1") && errors.contains("explode"),
        "{errors}"
    );
    assert!(
        errors.contains("wish big tier 1") && errors.contains("16 by 16"),
        "{errors}"
    );
}

#[test]
fn bad_sprite_rows_and_unresolved_sheets_reject_the_whole_wish() {
    for tail in [
        "draw worn\n.5.\n?bad\n",
        "sheet worn missing.png\n",
        "draw worn 0\n.5.\n",
        "draw worn 4\n.5.\n",
    ] {
        let (book, errors) = read_checked(&format!(
            "wish shield | Shield\nsay shield\ntier armor 1 :: Shield.\n{tail}"
        ));
        assert!(
            book.is_empty(),
            "a broken look must not grant an invisible wish"
        );
        assert!(!errors.is_empty());
    }
}

#[test]
fn every_stock_wish_draws_its_own_art_from_the_game_itself() {
    // Built in: no file on disk is read for it.
    for wish in read(TEXT) {
        assert!(art_for(&wish.id).is_some(), "{}", wish.id);
    }
}

#[test]
fn a_wish_dresses_its_cards_and_a_tier_may_grow_the_look() {
    let iron = read(TEXT).into_iter().find(|w| w.id == "iron").unwrap();
    let first = iron.card(0).unwrap();
    let worn = first
        .looks
        .worn
        .as_ref()
        .expect("Iron Skin shows on the knight");
    assert!(!worn.frames[0].concat().contains('W'), "plain pauldrons");
    let last = iron.card(2).unwrap();
    let spiked = last.looks.worn.as_ref().unwrap();
    assert!(spiked.frames[0].concat().contains('W'), "spiked at the top");
    // A learned devil swarm: a spell that summons, in its own look.
    let book = read(
        "wish golden-devils | Golden Devils\nkind spell\nsay tiny golden devils that explode | devil swarm\ntier summon 2 swarm; cooldown 8 :: Two devils.\ntier summon 4 swarm; cooldown 16 :: Four devils.\ndraw summon\n.5.|5.5\n555|555\n",
    );
    assert_eq!(book.len(), 1, "every tier checks");
    let card = book[0].card(1).unwrap();
    assert_eq!(card.imps(), Some((4, super::super::imps::Behaviour::Swarm)));
    assert_eq!(card.looks.summon.as_ref().unwrap().frames.len(), 2);
    assert_eq!(
        find_in(&book, "tiny golden devils that explode", &[]).map(|w| w.id),
        Some("golden-devils".into())
    );
}

#[test]
fn the_stock_ladders_climb_within_their_caps() {
    // Every tier of every stock wish is worth at least the one below it,
    // and none promises past what a deck may hold.
    for wish in read(TEXT) {
        let mut last = None;
        for tier in 0..wish.tiers.len() {
            let card = wish.card(tier).unwrap();
            let hp: u32 = card
                .effects
                .iter()
                .map(|e| match *e {
                    cards::Effect::MaxHp(v) => v,
                    _ => 0,
                })
                .sum();
            // Its knight holds every family (`combos`): the super words count.
            let kit = cards::check("wish-kit-1", "name Kit\nkind hold\ntext frog bullet storm fire shadow corn gun mail fast\nspeed 5\n").unwrap().card;
            let bonus = (cards::Bonus::of([&kit, &card]), hp);
            if let Some(prev) = last
                && bonus == prev
            {
                // Only a tier past a word's wish cap may add nothing more.
                assert!(
                    card.effects.iter().any(|e| {
                        let (word, v) = e.parts();
                        v > cards::wish_cap(word)
                    }),
                    "{} tier {} adds nothing",
                    wish.id,
                    tier + 1
                );
            }
            last = Some(bonus);
        }
    }
}

/// A learned wish, drawn as `art`.
fn learned_wish(art: &str) -> Wish {
    let text = format!(
        "{LEARNED}\nwish heavy-hits | Heavy Hits\nsay heavy hits\ntier damage 20; velocity 30 :: Hit harder.\ntier damage 25 :: Harder.\nart\n{art}\n"
    );
    let book = read(&text);
    assert_eq!(book.len(), 1, "{:?}", read_checked(&text).1);
    book.into_iter().next().unwrap()
}

const FINE_ART: &str = "....jjjjjj....\n...jJhhhhJj...\n..jJhh44hhJj..\n..jJh4@@4hJj..\n..jJh4@6@4hJ..\n..jJh4@@4hJj..\n..jJhh44hhJj..\n...jJhhhhJj...\n....jjjjjj....";
const CRUDE_ART: &str = "77777777\n77777777\n77777777\n77777777";

#[test]
fn a_crude_drawing_grants_a_weaker_wish_than_a_fine_one() {
    let fine = learned_wish(FINE_ART);
    let crude = learned_wish(CRUDE_ART);
    let (fg, _) = fine.grade().unwrap();
    let (cg, _) = crude.grade().unwrap();
    assert!(fg > cg, "{fg} over {cg}");
    assert_eq!(cg, artistry::Grade::Crude);
    let damage = |w: &Wish| cards::Bonus::of([&w.card(0).unwrap()]).damage;
    let speed = |w: &Wish| cards::Bonus::of([&w.card(0).unwrap()]).velocity;
    assert!(damage(&crude) < damage(&fine), "{} {}", damage(&crude), damage(&fine));
    assert!(speed(&crude) < speed(&fine));
}

#[test]
fn the_delve_s_own_wishes_are_given_whole() {
    for wish in book().iter().filter(|w| !w.learned) {
        assert!(wish.grade().is_none(), "{}", wish.id);
    }
}

#[test]
fn a_crude_spell_waits_longer_and_calls_fewer() {
    assert_eq!(graded_line("cooldown 8", artistry::Grade::Crude), "cooldown 14");
    assert_eq!(graded_line("summon 3 swarm", artistry::Grade::Crude), "summon 2 swarm");
    assert_eq!(graded_line("cooldown 10", artistry::Grade::Legendary), "cooldown 9");
    assert_eq!(graded_line("guard shield empower 15", artistry::Grade::Crude), "guard shield empower 15");
}

#[test]
fn every_wish_of_the_last_game_checks_at_every_grade_it_earns() {
    let text = include_str!("fixtures/last_game_phrasebook.txt");
    let (book, errors) = read_checked(&format!("{LEARNED}\n{text}"));
    // The one the live game's checker turned away then (a guard's `empower`
    // on its own line) is still turned away.
    assert!(errors.iter().all(|e| e.contains("blocking-bullets")), "{errors:?}");
    assert!(book.len() >= 30, "{}", book.len());
    for wish in &book {
        assert!(wish.grade().is_some());
    }
}

/// The last game's wishes, graded: score, grade and each picture's marks.
/// `cargo test -p angel-delve --lib grade_census -- --ignored --nocapture`
#[test]
#[ignore]
fn grade_census() {
    let text = include_str!("fixtures/last_game_phrasebook.txt");
    for wish in read(&format!("{LEARNED}\n{text}")) {
        let (grade, score) = wish.grade().unwrap();
        println!("GRADE,{},{score},{grade},art {:?}", wish.id, artistry::marks(&wish.art));
    }
}
