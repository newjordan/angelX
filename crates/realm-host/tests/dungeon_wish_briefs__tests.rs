use super::*;

#[test]
fn lesson_examples_are_checked_data_with_worn_gear_and_active_summons() {
    // A fresh phrasebook of our own: the operator's live one at the
    // repo's root already holds the words this brief is to offer.
    let empty = std::env::temp_dir().join(format!("wish-brief-phrase-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let book = empty.join("phrasebook.txt");
    std::fs::write(&book, "").unwrap();
    let sheets = empty.join("sheets");
    std::fs::create_dir_all(&sheets).unwrap();
    let text = learn_brief("Jordan", "bigger spiked shield", &book, &sheets);
    assert!(text.contains("max_hp 5-40"));
    assert!(text.contains("Never use orbit"));
    // The player's words, literally: bigger is `size`, fire is `burn`, and
    // never a plain damage bump; every new word is offered.
    assert!(text.contains("Never answer with a plain damage bump"));
    assert!(text.contains("Jordan's own wish"));
    // Each at what a wish may give at most (`cards::wish_most`), not the card cap.
    for word in [
        "size 1-2",
        "knock 1-2",
        "burn 1-2",
        "chill 1-2",
        "poison 1-2",
        "boomerang 1-1",
        "velocity 10-40",
    ] {
        assert!(text.contains(word), "brief offers {word}");
    }
    for word in [
        "worn",
        "shot",
        "orbit",
        "summon",
        "aura",
        "image tool",
        "1 to 4 frames",
    ] {
        assert!(text.contains(word), "brief offers {word}");
    }
    let examples = text
        .split("Examples:\n")
        .nth(1)
        .unwrap()
        .split("Held cards' lines")
        .next()
        .unwrap();
    let dir = std::env::temp_dir().join(format!("wish-briefs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("golden-devils-summon.png"),
        include_bytes!("../../../cockpit/fixtures/sheets/golden-devils.png"),
    )
    .unwrap();
    let (rows, errors) = crate::together_files::phrasebook_with_sheets(examples, &dir, &dir.join("paint"));
    assert!(errors.is_empty(), "{errors:?}");
    let (wishes, errors) = crate::together_shooter::phrasebook::read_checked(&rows);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(wishes.len(), 3);
    // Five tiers each, as the brief asks: the fourth and fifth a real climb.
    assert!(text.contains("five tiers: a modest first grant, stronger, strong, mighty, then the full gift"));
    assert!(wishes.iter().all(|w| w.tiers.len() == 5), "{:?}", wishes.iter().map(|w| w.tiers.len()).collect::<Vec<_>>());
    assert!(wishes[0].card(0).unwrap().looks.worn.is_some());
    assert!(wishes[1].card(0).unwrap().looks.summon.is_some());
    let arrows = wishes[2].card(0).unwrap();
    assert!(arrows.looks.shot.is_some(), "bigger arrows are drawn");
    assert_eq!(arrows.effects, vec![crate::together_shooter::cards::Effect::Size(1)]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn reforging_briefs_require_field_looks_and_keep_the_kit_kind() {
    for part in [Part::Offense, Part::Defense] {
        let text = brief(
            "Jordan",
            part,
            "bigger spiked shield",
            std::path::Path::new(".angel/dungeon/reforged/shield.card"),
            std::path::Path::new(".angel/dungeon/sheets"),
        );
        assert!(text.contains("draw worn"));
        assert!(text.contains("never use `orbit` as a stand-in"));
        assert!(text.contains("image tool") && text.contains("summon N swarm|guard|turret"));
        assert!(text.contains(if part == Part::Offense {
            "kind   arm"
        } else {
            "kind   guard"
        }));
    }
}

/// Exact briefs for review, without running a drafting tool.
#[test]
#[ignore]
fn write_wish_briefs() {
    let Some(dir) = std::env::var_os("ANGEL_ARENA_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let dir = dir.join("briefs");
    std::fs::create_dir_all(&dir).unwrap();
    let ws = std::path::Path::new("/workspace");
    let phrasebook = ws.join(".angel/dungeon/phrasebook.txt");
    let sheets = crate::together_sheets::dir(ws);
    std::fs::write(
        dir.join("learn.txt"),
        learn_brief(
            "Jordan",
            "tiny golden devils that explode",
            &phrasebook,
            &sheets,
        ),
    )
    .unwrap();
    for (file, part, words) in [
        ("reforge-offense", Part::Offense, "venom comets"),
        ("reforge-defense", Part::Defense, "a bigger spiked shield"),
    ] {
        let path = ws.join(format!(".angel/dungeon/reforged/{file}.card"));
        std::fs::write(
            dir.join(format!("{file}.txt")),
            brief("Jordan", part, words, &path, &sheets),
        )
        .unwrap();
    }
    let mut realm = crate::together_realm::Realm::default();
    let id = realm.ask("Jordan", "a golden devil shrine").unwrap();
    let path = ws.join(format!(".angel/realm/wishes/{id}.wish"));
    std::fs::write(
        dir.join("realm.txt"),
        crate::together_realm::brief(&realm.wishes[0], &path, &realm.treasury),
    )
    .unwrap();
    std::fs::write(
        dir.join("repair.txt"),
        mend_brief(
            "Jordan",
            "tiny golden devils that explode",
            &phrasebook,
            &["wish golden-devils: line 6: no sheet golden-devils-summon.png".into()],
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("grant.txt"),
        grant_brief_for(
            "Jordan",
            "tiny golden devils that explode",
            None,
            Target::You,
        ),
    )
    .unwrap();
}

#[test]
fn briefs_offer_the_motion_words() {
    let learn = learn_brief(
        "Jordan",
        "meatball bullets",
        std::path::Path::new(".angel/dungeon/phrasebook.txt"),
        std::path::Path::new(".angel/dungeon/sheets"),
    );
    let forge = brief(
        "Jordan",
        Part::Offense,
        "meatball bow",
        std::path::Path::new(".angel/dungeon/reforged/x.card"),
        std::path::Path::new(".angel/dungeon/sheets"),
    );
    for text in [learn, forge] {
        assert!(text.contains("move=<word>[,<word>]"));
        assert!(text.contains("stretch, bob, wobble, spin, jiggle, wiggle, pulse, shake"));
    }
}

#[test]
fn an_unmade_knights_lesson_asks_the_model_to_decide_what_the_wish_bestows() {
    use crate::together_shooter::identity::{Making, Shape};
    let lesson = |making: &Making, words: &str| {
        learn_brief_for(
            "Jordan",
            words,
            std::path::Path::new(".angel/dungeon/phrasebook.txt"),
            std::path::Path::new(".angel/dungeon/sheets"),
            Some(making),
        )
    };
    let body = Making {
        shape: Some(Shape::Triangle),
        ..Making::default()
    };
    let text = lesson(&body, "an ogre with a stone hide");
    assert!(text.contains("THIS WISH MAKES THE KNIGHT"));
    assert!(text.contains("a triangle wishing themselves into being"));
    assert!(
        text.contains("THE BODY") && text.contains("`max_hp`") && text.contains("Ogre's frame")
    );
    let weapon = Making {
        parts: vec!["body".into()],
        ..body.clone()
    };
    assert!(lesson(&weapon, "a club made of a tree").contains("THE WEAPON"));
    let ultimate = Making {
        weapon: true,
        ..weapon
    };
    let text = lesson(&ultimate, "a dragon's roar");
    assert!(text.contains("THE ULTIMATE") && text.contains("grand_chord"));
    // A made knight's lesson is the ordinary one.
    let whole = Making {
        made: true,
        ..ultimate
    };
    assert!(!lesson(&whole, "fire faster").contains("THIS WISH MAKES THE KNIGHT"));
}

#[test]
fn a_grant_runners_brief_asks_for_the_entry_as_its_reply_and_its_examples_check() {
    let text = grant_brief_for(
        "Jordan",
        "tiny golden devils that explode",
        None,
        Target::You,
    );
    assert!(text.contains("Reply with ONE entry and nothing else"));
    assert!(text.contains("Jordan's own wish"));
    assert!(!text.contains("image tool"), "a runner has no tools");
    assert!(!text.contains("Append ONE entry"));
    let examples = text
        .split("Examples:\n")
        .nth(1)
        .unwrap()
        .split("Held cards' lines")
        .next()
        .unwrap();
    assert!(!examples.contains("sheet "));
    let (wishes, errors) = crate::together_shooter::phrasebook::read_checked(examples);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(wishes.len(), 3);
    assert!(
        wishes[1].card(0).unwrap().looks.summon.is_some(),
        "the devils are drawn in rows"
    );
}

#[test]
fn a_summoners_wish_for_his_dead_tells_the_learner_so_and_nothing_else_changes() {
    let words = "make my ghouls bright pink";
    let own = grant_brief_for("Ada", words, None, Target::You);
    let dead = grant_brief_for("Ada", words, None, Target::Dead);
    let both = grant_brief_for("Ada", words, None, Target::Both);
    assert!(
        !own.contains("Gravecaller"),
        "a wish for himself is the brief it always was"
    );
    for brief in [&dead, &both] {
        assert!(
            brief.contains("never a summon spell") && brief.contains("raised minions"),
            "{brief}"
        );
        assert!(brief.contains("`draw worn at=body` swatch"));
    }
    assert!(dead.contains("for his raised dead, not for himself"));
    assert!(both.contains("for himself and his raised dead alike"));
    // The note is all that is added, right after the words.
    assert_eq!(dead.replace(&dead_note("Ada", Target::Dead), ""), own);
    assert!(dead.contains(&format!("“{words}”\n\nAda plays the Gravecaller")));
}


#[test]
fn creator_door_reaches_the_genie_brief() {
    for words in ["a fire bolt", "a tiny golden devil", "a tall smith", "a long spear"] {
        let door = creator_wish_brief("Ada", words, None, Target::You);
        let genie = grant_brief_for("Ada", words, None, Target::You);
        assert_eq!(door, genie, "{words}");
        assert!(door.contains(words));
    }
}
