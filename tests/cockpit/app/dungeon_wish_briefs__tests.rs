use super::*;

#[test]
fn lesson_examples_are_checked_data_with_worn_gear_and_active_summons() {
    let text = learn_brief(
        "Jordan",
        "bigger spiked shield",
        std::path::Path::new(".angel/dungeon/phrasebook.txt"),
        std::path::Path::new(".angel/dungeon/sheets"),
    );
    assert!(text.contains("max_hp 5-40"));
    assert!(text.contains("Never use orbit"));
    // The player's words, literally: bigger is `size`, fire is `burn`, and
    // never a plain damage bump; every new word is offered.
    assert!(text.contains("Never answer with a plain damage bump"));
    assert!(text.contains("Jordan's own wish"));
    for word in ["size 1-3", "knock 1-3", "burn 1-3", "chill 1-3", "poison 1-3", "boomerang 1-1", "velocity 10-60"] {
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
    let (rows, errors) = crate::drive::together_files::phrasebook_with_sheets(examples, &dir);
    assert!(errors.is_empty(), "{errors:?}");
    let (wishes, errors) = crate::drive::together_shooter::phrasebook::read_checked(&rows);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(wishes.len(), 3);
    assert!(wishes[0].card(0).unwrap().looks.worn.is_some());
    assert!(wishes[1].card(0).unwrap().looks.summon.is_some());
    let arrows = wishes[2].card(0).unwrap();
    assert!(arrows.looks.shot.is_some(), "bigger arrows are drawn");
    assert_eq!(arrows.effects, vec![crate::drive::together_shooter::cards::Effect::Size(1)]);
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
    let sheets = crate::drive::together_sheets::dir(ws);
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
    let mut realm = crate::drive::together_realm::Realm::default();
    let id = realm.ask("Jordan", "a golden devil shrine").unwrap();
    let path = ws.join(format!(".angel/realm/wishes/{id}.wish"));
    std::fs::write(
        dir.join("realm.txt"),
        crate::drive::together_realm::brief(&realm.wishes[0], &path, &realm.treasury),
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
}
