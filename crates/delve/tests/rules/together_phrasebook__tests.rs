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
            // The identity start's weapons and magic are arms and spells
            // (`identity`); every other stock wish is held.
            assert_eq!(card.kind.word(), wish.kind, "{}", wish.id);
            assert!(
                !card.effects.is_empty() || card.arm.is_some() || card.ult.is_some(),
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
    assert_eq!(volley.next_tier(&deck), Some(3), "the third climbs on to the fourth");
    deck = vec![volley.card_id(4)];
    assert_eq!(volley.next_tier(&deck), None);
}

#[test]
fn wish_ids_fit_the_card_id_at_every_tier_without_losing_the_ladder_marker() {
    let id = "a".repeat(MAX_ID);
    // Each tier gives more than the one below, as every ladder must.
    let tiers: String = (0..MAX_TIERS)
        .map(|t| format!("tier damage {} :: A gift.\n", 5 + 4 * t))
        .collect();
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
    let both = vec![learned.card_id(0), iron.card_id(4)];
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
        "draw worn\n.5.\n*bad\n",
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
    // Every tier of every stock wish gives more than the one below it, and
    // none promises past what wishes may give.
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
            // A weapon's ladder climbs in its runes and a spell's in its
            // recharge and what it casts (`identity`'s Bright Blade, Firebolt).
            let bonus = (
                cards::Bonus::of([&kit, &card]),
                hp,
                format!(
                    "{:?} {:?} {} {:?}",
                    card.arm, card.effects, card.cooldown, card.ult
                ),
            );
            assert!(
                last.as_ref() != Some(&bonus),
                "{} tier {} adds nothing",
                wish.id,
                tier + 1
            );
            for effect in &card.effects {
                let (word, v) = effect.parts();
                assert!(
                    cards::wish_most(word).is_none_or(|most| v <= most),
                    "{} tier {} asks {word} {v}, past what wishes give",
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
    assert!(
        damage(&crude) < damage(&fine),
        "{} {}",
        damage(&crude),
        damage(&fine)
    );
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
    assert_eq!(
        graded_line("cooldown 8", artistry::Grade::Crude),
        "cooldown 14"
    );
    assert_eq!(
        graded_line("summon 3 swarm", artistry::Grade::Crude),
        "summon 2 swarm"
    );
    assert_eq!(
        graded_line("cooldown 10", artistry::Grade::Legendary),
        "cooldown 9"
    );
    assert_eq!(
        graded_line("guard shield empower 15", artistry::Grade::Crude),
        "guard shield empower 15"
    );
}

#[test]
fn every_wish_of_the_last_game_checks_at_every_grade_it_earns() {
    let text = include_str!("fixtures/last_game_phrasebook.txt");
    let (book, errors) = read_checked(&format!("{LEARNED}\n{text}"));
    // The one the live game's checker turned away then (a guard's `empower`
    // on its own line) is still turned away; the rest that are, are turned
    // away for a ladder that does not climb past what wishes give.
    assert!(
        errors.iter().all(|e| e.contains("blocking-bullets")
            || e.contains("more than wishes ever give")
            || e.contains("gives nothing more than tier")),
        "{errors:?}"
    );
    assert!(book.len() >= 10, "{}", book.len());
    for wish in &book {
        assert!(wish.grade().is_some());
    }
}

#[test]
fn a_wish_not_safe_for_work_is_turned_away_and_none_ships() {
    let rude = "wish rude-bullets | Rude Bullets\nsay butt bullets | shit bullets\ntier homing 1 :: They seek.\ntier homing 2 :: Harder.\n";
    let (book, errors) = read_checked(rude);
    // A public build has no private table: every wish stays safe for work.
    #[cfg(not(private_table))]
    assert!(
        book.is_empty()
            && errors
                .iter()
                .any(|e| e.contains("`shit` is not for this game")),
        "{errors:?}"
    );
    // Dev's private table grants a rude ask as asked (`private_table`,
    // 028003d68); only its slurs are turned away. The slur is the one word
    // of the table's own rot13 list, so no source file here spells it.
    #[cfg(private_table)]
    {
        assert_eq!(book.len(), 1, "{errors:?}");
        let slur: String = super::super::private_table::NOT_AT_THE_TABLE[0]
            .chars()
            .map(|c| match c {
                'a'..='m' => (c as u8 + 13) as char,
                'n'..='z' => (c as u8 - 13) as char,
                c => c,
            })
            .collect();
        let (book, errors) = read_checked(&rude.replace("shit bullets", &format!("{slur} bullets")));
        assert!(
            book.is_empty()
                && errors
                    .iter()
                    .any(|e| e.contains(&format!("`{slur}` is not for this game"))),
            "{errors:?}"
        );
    }
    // Toilet humour is the game's own.
    let (book, errors) = read_checked(&rude.replace(" | shit bullets", " | fart bullets"));
    assert_eq!(book.len(), 1, "{errors:?}");
    // Nothing the game ships, nor the play-test kept for its tests, says one.
    for (name, text) in [
        ("the delve's phrasebook", TEXT),
        (
            "the last game's",
            include_str!("fixtures/last_game_phrasebook.txt"),
        ),
    ] {
        for line in text.lines() {
            assert_eq!(
                super::super::cards::not_for_work(line),
                None,
                "{name}: {line}"
            );
        }
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
        println!(
            "GRADE,{},{score},{grade},art {:?}",
            wish.id,
            artistry::marks(&wish.art)
        );
    }
}

#[test]
fn every_stock_wish_climbs_its_ladder_by_the_checker() {
    let (book, errors) = read_checked(TEXT);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(book.len(), read(TEXT).len());
}

#[test]
fn a_tier_no_better_than_the_one_below_is_turned_away_by_name() {
    // The last game's Mirror Copies: tiers 2 and 3 call two turrets, and
    // the game raises both cooldowns to 20s, so "they answer you sooner"
    // gave nothing.
    let (book, errors) = read_checked(&format!(
        "{LEARNED}\nwish mirror-copies | Mirror Copies\nkind spell\nsay mirror clones\n\
tier summon 1 turret; cooldown 14 :: A mirror copy of you stands and looses your bolts.\n\
tier summon 2 turret; cooldown 16 :: Two mirror copies, both loosing your bolts.\n\
tier summon 2 turret; cooldown 10 :: Two mirror copies, and they answer you sooner.\n"
    ));
    assert!(book.is_empty());
    let errors = errors.join(" | ");
    assert!(
        errors.contains("wish mirror-copies tier 3: gives nothing more than tier 2")
            && errors.contains("cooldown raised to 20s"),
        "{errors}"
    );
    // A shorter cooldown that the game keeps is more.
    let (book, errors) = read_checked(
        "wish mirrors | Mirrors\nkind spell\nsay mirrors\ntier summon 2 turret; cooldown 30 :: Two.\ntier summon 2 turret; cooldown 20 :: Sooner.\n",
    );
    assert_eq!(book.len(), 1, "{errors:?}");
}

#[test]
fn a_tier_past_what_wishes_give_is_turned_away() {
    // Twin Fletch III promised three arrows; wishes give one more at most.
    let (book, errors) = read_checked(
        "wish twin-fletch | Twin Fletch\nsay twin fletch\ntier shots 1 :: Two arrows.\ntier shots 1; velocity 20 :: Faster.\ntier shots 2; velocity 30 :: Three arrows.\n",
    );
    assert!(book.is_empty());
    let errors = errors.join(" | ");
    assert!(
        errors.contains("wish twin-fletch tier 3: shots 2") && errors.contains("shots 1 at most"),
        "{errors}"
    );
    // A chain repeated is fine while the tier adds something else.
    let (book, errors) = read_checked(
        "wish bolts | Bolts\nsay bolts\ntier chain 1 :: Sparks.\ntier chain 1; velocity 20 :: Faster.\n",
    );
    assert_eq!(book.len(), 1, "{errors:?}");
}

#[test]
fn upgrade_words_find_the_wish_the_knight_holds() {
    let book = read(&format!(
        "{TEXT}\n{LEARNED}\nwish pasta-armor | Pasta Armor\nsay pasta armor | noodle armor | lasagne plate\n\
tier armor 1 :: Lasagne.\ntier armor 2 :: Thicker.\ntier armor 2; max_hp 15 :: Full.\n\n\
wish large-meatball-spell | Large Meatball\nkind spell\nsay large meatball spell | big meatball\n\
tier cast meteor; cooldown 9 :: A meatball.\ntier cast meteor; cooldown 7; nova 25 :: A larger one.\n"
    ));
    let pasta = book.iter().find(|w| w.id == "pasta-armor").unwrap();
    let meatball = book
        .iter()
        .find(|w| w.id == "large-meatball-spell")
        .unwrap();
    let id = |said: &str, deck: &[String]| find_in(&book, said, deck).map(|w| w.id);
    let held = [pasta.card_id(0), meatball.card_id(0)];
    assert_eq!(
        id("upgrade my pasta armor", &held),
        Some("pasta-armor".into())
    );
    assert_eq!(
        id("make my pasta armor stronger", &held),
        Some("pasta-armor".into())
    );
    assert_eq!(
        id("increase meatball size", &held),
        Some("large-meatball-spell".into())
    );
    // Holding none of it, the words are as before: a new thing to learn.
    assert_eq!(id("upgrade my pasta armor", &[]), None);
    assert_eq!(id("increase meatball size", &[]), None);
    // Upgrade words for nothing held fall through to the stock wishes.
    assert_eq!(id("stronger shots", &held), Some("heavy".into()));
    assert_eq!(id("more shots", &held), Some("twin".into()));
}

/// A wish said by its own name, or asked up a tier by name with the tier
/// said back, is the wish the book has: it was learned again three times
/// over as three different "Permanent Mirrors" (2026-10-23).
#[test]
fn a_wish_said_by_its_name_or_upgraded_by_name_is_the_one_held() {
    let book = read(&format!(
        "{TEXT}\n{LEARNED}\nwish permanent-mirrors | Permanent Mirrors\nsay 3 permanent mirror copies of myself that repeat my spells and attacks | three mirror clones | mirror selves that copy me\n\
tier armor 1 :: One mirror.\ntier armor 2 :: Two mirrors.\ntier armor 2; max_hp 15 :: Three mirrors.\n"
    ));
    let mirror = book.iter().find(|w| w.id == "permanent-mirrors").unwrap();
    let mirrors = |said: &str, deck: &[String]| find_in(&book, said, deck).map(|w| w.id);
    // Its name alone finds it, held or not.
    assert_eq!(
        mirrors("Permanent Mirrors", &[]).as_deref(),
        Some("permanent-mirrors")
    );
    let held = vec![mirror.card_id(0)];
    assert_eq!(
        mirrors("permanent mirrors", &held).as_deref(),
        Some("permanent-mirrors")
    );
    // An upgrade by name, with the tier said back, is that wish too.
    for said in [
        "Upgrade my Permanent Mirrors 1",
        "upgrade permanent mirrors II",
        "more permanent mirrors",
    ] {
        assert_eq!(
            mirrors(said, &held).as_deref(),
            Some("permanent-mirrors"),
            "{said}"
        );
    }
    // A new thing that only shares a word is not it.
    assert_ne!(
        mirrors("mirror armor that reflects fire", &held).as_deref(),
        Some("permanent-mirrors")
    );
}

/// Every stock ladder of more than one tier runs to the fifth (the
/// operator, 2026-10-28: "we want to expand to level 5 on the items"), its
/// fourth and fifth each a real climb: more of a word, or a word more,
/// within what wishes give (`the_stock_ladders_climb_within_their_caps`).
#[test]
fn the_stock_ladders_run_to_the_fifth_tier() {
    let book = read(TEXT);
    let long: Vec<&Wish> = book.iter().filter(|w| w.tiers.len() > 1).collect();
    assert!(long.len() >= 27, "{}", long.len());
    for wish in &long {
        assert_eq!(wish.tiers.len(), MAX_TIERS, "{} climbs to V", wish.id);
        assert_eq!(wish.derived, 0, "{}: the delve's own are written whole", wish.id);
        for tier in 0..MAX_TIERS {
            let card = wish.card(tier).unwrap();
            assert!(card.name.ends_with(["I", "II", "III", "IV", "V"][tier]), "{}", card.name);
            // The new tiers' lines are whole on the card, never cut.
            let says = &wish.tiers[tier].1;
            assert!(tier < 3 || says.chars().count() <= cards::MAX_TEXT, "{} tier {}: {says}", wish.id, tier + 1);
        }
    }
    // One-tier wishes (the ultimates, the arms, the plain frame) stay so.
    assert!(book.iter().filter(|w| w.tiers.len() == 1).all(|w| w.kind == "arm" || w.card(0).unwrap().ult.is_some() || w.id == "plain-frame"));
}

/// A long ladder's cards: the first common, those between rare, the fourth
/// and fifth relic (the checker's top rarity: the fifth's own mark is on its
/// picture). A ladder of three keeps its third relic.
#[test]
fn a_long_ladders_high_tiers_are_relics() {
    let ricochet = get("ricochet").unwrap();
    let rarities: Vec<cards::Rarity> = (0..5).map(|t| ricochet.card(t).unwrap().rarity).collect();
    use cards::Rarity::{Common, Rare, Relic};
    assert_eq!(rarities, vec![Common, Rare, Rare, Relic, Relic]);
    let three = read("wish three | Three\nsay three\nkind guard\ntier guard roll :: Roll.\ntier guard shield :: Shield.\ntier guard spin :: Spin.\n");
    let rarities: Vec<cards::Rarity> = (0..3).map(|t| three[0].card(t).unwrap().rarity).collect();
    assert_eq!(rarities, vec![Common, Rare, Relic]);
}

/// A learned wish of the last game, written to three tiers.
fn learned_three(entry: &str) -> (Vec<Wish>, Vec<String>) {
    read_checked(&format!("{LEARNED}\n{entry}"))
}

const BIGGER_ARROWS: &str = "wish bigger-arrows | Bigger Arrows\nsay bigger arrows | big arrows\n\
tier size 1 :: Your arrows are bigger.\ntier size 2 :: Bigger again.\ntier size 2; knock 1 :: Huge arrows that shove.\n\
draw shot\n.2.......jW.\n22ooooooojWW\n.2.......jW.\n";

/// A learned ladder written to three tiers climbs on to the fifth by rule:
/// a line raised a step where it can be, a small extra word where nothing
/// can, each tier checked and more than the one below, never past what
/// wishes give. The same words climb the same way every time they are
/// read, and its source is never written past what it said.
#[test]
fn a_learned_three_tier_ladder_climbs_on_to_the_fifth_by_rule() {
    let (book, errors) = learned_three(BIGGER_ARROWS);
    assert!(errors.is_empty(), "{errors:?}");
    let arrows = &book[0];
    assert_eq!((arrows.tiers.len(), arrows.derived), (5, 2));
    assert_eq!(arrows.tiers[3].0, vec!["size 2", "knock 2"], "the shove raised a step");
    assert_eq!(arrows.tiers[4].0, vec!["size 2", "knock 2", "damage 8"], "then a small extra");
    assert!(arrows.tiers[3].1.starts_with("Mighty: ") && arrows.tiers[4].1.starts_with("The full gift: "));
    for tier in 0..5 {
        let card = arrows.card(tier).unwrap();
        for effect in &card.effects {
            let (word, v) = effect.parts();
            assert!(cards::wish_most(word).is_none_or(|most| v <= most), "{word} {v}");
        }
    }
    assert!(arrows.ladder_fault().is_none());
    // Pure: read again, or from its own source, it is the same ladder.
    assert_eq!(learned_three(BIGGER_ARROWS).0, book);
    let source = arrows.definition_file();
    assert_eq!(source.matches("\ntier ").count(), 3, "only what was written:\n{source}");
    assert_eq!(read(&source), book);
    assert_eq!(arrows.written().tiers.len(), 3);
    assert!(super::super::wish_jobs::valid_wish(arrows));
    // Captured before ladders climbed (an old save's open scroll), the
    // ladder as written is still its own source's.
    assert!(super::super::wish_jobs::valid_wish(&arrows.written()));
    // The delve's own, and a learned ladder written whole, are as written.
    assert!(read(TEXT).iter().all(|w| w.derived == 0));
    let whole = format!("{LEARNED}\n{}", get("ricochet").unwrap().definition_file());
    assert_eq!(read(&whole)[0].derived, 0);
}

/// Each kind climbs in its own numbers: a spell's recharge sooner, a
/// summons one more (its recharge raised to match), a wall wider, a
/// weapon's runes harder, a held word to what wishes give and then a word
/// beside it. What cannot climb at all (a shield, a plain bow, a ladder of
/// one or two) stays as written.
#[test]
fn every_kind_of_learned_ladder_climbs_in_its_own_numbers() {
    let climbed = |entry: &str| {
        let (book, errors) = learned_three(entry);
        assert!(errors.is_empty(), "{errors:?}");
        book[0].tiers.iter().map(|(lines, _)| lines.join("; ")).collect::<Vec<_>>()
    };
    let bolt = climbed("wish star-bolt | Star Bolt\nkind spell\nsay star bolt\ntier cast bolt; cooldown 9 :: Bolt.\ntier cast bolt; cooldown 7 :: Sooner.\ntier cast bolt; cooldown 6 :: Sooner still.\n");
    assert_eq!(bolt[3..], ["cast bolt; cooldown 5", "cast bolt; cooldown 4"]);
    let devils = climbed("wish golden-devils | Golden Devils\nkind spell\nsay golden devils\ntier summon 2 swarm; cooldown 8 :: Two.\ntier summon 3 swarm; cooldown 12 :: Three.\ntier summon 4 swarm; cooldown 16 :: Four.\ndraw summon\n.5.|5.5\n555|555\n");
    assert_eq!(devils[3..], ["summon 5 swarm; cooldown 20", "summon 6 swarm; cooldown 24"]);
    let wall = climbed("wish great-wall | Great Wall\nkind guard\nsay great wall\ntier guard wall width=2 drain=10 empower=0 :: Two.\ntier guard wall width=3 drain=15 empower=0 :: Three.\ntier guard wall width=5 drain=25 empower=0 :: Five.\n");
    assert_eq!(wall[3..], ["guard wall width=6 drain=25 empower=0", "guard wall width=7 drain=25 empower=0"]);
    let club = climbed("wish big-club | Big Club\nkind arm\nsay big club\ntier melee damage=20 reach=2 arc=90 every=20 :: A club.\ntier melee damage=26 reach=2 arc=90 every=20 :: Heavier.\ntier melee damage=32 reach=2 arc=90 every=20 :: Heaviest.\n");
    assert_eq!(club[3..], ["melee damage=36 reach=2 arc=90 every=20", "melee damage=41 reach=2 arc=90 every=20"]);
    let speed = climbed("wish zoom | Zoom\nsay zoom zoom\ntier speed 5 :: Zoom.\ntier speed 8 :: Zoomier.\ntier speed 12 :: Zoomiest.\n");
    assert_eq!(speed[3..], ["speed 16", "speed 20"]);
    // A shield has no number to raise: its ladder stays at three.
    let (book, _) = learned_three("wish turtle | Turtle\nkind guard\nsay turtle\ntier guard roll :: Roll.\ntier guard shield :: Shield.\ntier guard spin :: Spin.\n");
    assert_eq!((book[0].tiers.len(), book[0].derived), (3, 0));
    // A ladder of two, or one, is as it was written.
    let (book, _) = learned_three("wish two-step | Two Step\nsay two step\ntier rate 5 :: One.\ntier rate 10 :: Two.\n");
    assert_eq!(book[0].tiers.len(), 2);
}

/// A learned ladder the painter dressed past its written top (`draw worn
/// paint=… 5`, painted for the climbed fifth) still reads: its looks are
/// counted against the ladder it climbs to.
#[test]
fn a_look_painted_for_a_climbed_tier_reads_with_its_ladder() {
    let entry = format!("{BIGGER_ARROWS}\ndraw shot 5\n.5.......jW.\n55ooooooojWW\n.5.......jW.\n");
    let (book, errors) = learned_three(&entry);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(book[0].tiers.len(), 5);
    assert_eq!(read(&book[0].definition_file()), book, "and writes back as it was read");
}

/// A learned entry as the genie's model writes it on its own: its `vfx`
/// lines said for the whole wish, outside any tier. It reads, and every
/// tier that names no recipe of its own carries them (2026-11-05: such
/// answers were turned away as "unknown line `vfx`").
#[test]
fn a_wish_wide_vfx_line_is_every_tiers_recipe() {
    let text = "wish rotten-fish-stench | Rotten Fish Stench\n\
        say rotten fish stench | fish stench\n\
        vfx aura flies poop count=4\n\
        tier damage 5 :: A whiff.\n\
        tier damage 10 :: A reek.\n\
        tier damage 15; vfx hit ring storm back :: The whole harbour.\n";
    let (book, errors) = read_checked(text);
    assert!(errors.is_empty(), "{errors:?}");
    let wish = &book[0];
    assert_eq!(wish.card(0).unwrap().looks.vfx.layers.len(), 1, "the wish's recipe on tier one");
    assert_eq!(wish.card(1).unwrap().looks.vfx.layers.len(), 1, "and on tier two");
    let top = wish.card(2).unwrap();
    assert_eq!(top.looks.vfx.layers.len(), 1, "a tier with its own recipe keeps only its own");
    assert!(top.looks.vfx.layers[0].line().contains("ring"), "{:?}", top.looks.vfx.layers[0].line());
}

/// A `vfx` line right after an `art` block ends the block; it is not a row.
#[test]
fn a_vfx_line_ends_an_art_block() {
    let text = "wish glow | Glow\nsay glow\nart\n..YY..\n.YYYY.\nvfx aura flies poop count=4\ntier damage 5 :: Glows.\n";
    let (book, errors) = read_checked(text);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(book[0].art.len(), 2, "{:?}", book[0].art);
}

/// "Ice waves", "lava burst", "frost nova": a wish for an area spell is a spell of area casts, never a
/// bolt, and its ladder climbs by size and a sooner recharge like any other spell's.
#[test]
fn an_area_spell_wish_climbs_by_size_and_a_sooner_recharge() {
    use crate::rules::cards::{Area, Effect, Element, Form};
    let (book, errors) = learned_three(
        "wish ice-waves | Ice Waves\nkind spell\nsay ice waves | a wave of ice | frost wave\n\
tier cast wave frost; cooldown 14 :: A cold wave.\ntier cast wave frost 2; cooldown 12 :: A freezing wave.\n\
tier cast wave frost 3; cooldown 10 :: A great freezing wave.\n",
    );
    assert!(errors.is_empty(), "{errors:?}");
    let sizes: Vec<u8> = (0..book[0].tiers.len())
        .map(|tier| match book[0].card(tier).unwrap().effects[..] {
            [Effect::Area(Area { form: Form::Wave, element: Element::Frost, size })] => size,
            ref other => panic!("tier {tier} is not a frost wave: {other:?}"),
        })
        .collect();
    assert_eq!(&sizes[..3], [1, 2, 3], "the written tiers climb in size");
    assert_eq!(sizes.len(), 5, "and the ladder is climbed to five");
    let waits: Vec<u32> = (0..5).map(|tier| book[0].card(tier).unwrap().cooldown).collect();
    assert!(waits.windows(2).all(|w| w[1] <= w[0]) && waits[4] < waits[0], "ever sooner: {waits:?}");
    // The same ladder that stays at one size must still climb, by recharge.
    let (book, errors) = learned_three(
        "wish lava-burst | Lava Burst\nkind spell\nsay lava burst | an eruption\n\
tier cast burst fire; cooldown 14 :: A hot burst.\ntier cast burst fire; cooldown 11 :: Sooner.\ntier cast burst fire; cooldown 8 :: Sooner still.\n",
    );
    assert!(errors.is_empty(), "{errors:?}");
    let waits: Vec<u32> = (0..5).map(|tier| book[0].card(tier).unwrap().cooldown).collect();
    // (A plain picture grades the numbers, so the wait is longer than written; the climb is the same.)
    assert!(waits.windows(2).all(|w| w[1] <= w[0]) && waits[4] < waits[2], "{waits:?}");
}
