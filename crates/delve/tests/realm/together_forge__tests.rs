use super::*;

fn ok(text: &str) -> Checked {
    check(text, START_LOC).unwrap_or_else(|rejected| panic!("{:?}", rejected.errors))
}

fn errors(text: &str) -> Vec<String> {
    check(text, START_LOC).unwrap_err().errors
}

#[test]
fn forge_starter_bow_matches_the_delves_slow_heavy_shot() {
    let wand = Weapon::starter();
    assert_eq!(wand.kind(), "ranged");
    assert_eq!(wand.loc, 1);
    let bolt = wand.bolt.unwrap();
    assert_eq!((bolt.damage, bolt.speed, bolt.every), (32, 12.0, 15));
    assert_eq!(wand.dps(), 64.0);
}

#[test]
fn forge_reads_the_three_documented_examples() {
    let fan = ok(
        "name Ember Fan\nlook ember star\nbolt damage=10 speed=12 every=15 range=30\nspread shots=3 arc=30",
    );
    assert_eq!(fan.weapon.kind(), "ranged");
    assert_eq!(fan.weapon.shots(), 3);
    assert!(fan.notes.is_empty(), "{:?}", fan.notes);
    let halberd =
        ok("name Glass Halberd\nlook ice blade\nmelee damage=34 reach=2.0 arc=140 every=16");
    assert_eq!(halberd.weapon.kind(), "melee");
    assert!(halberd.notes.is_empty(), "{:?}", halberd.notes);
    let choir = ok(
        "name Venom Choir\nlook venom wave\nbolt damage=8 speed=12 every=20 range=24\nmelee damage=24 reach=1.8 arc=180 every=20",
    );
    assert_eq!(choir.weapon.kind(), "hybrid");
    assert_eq!(choir.weapon.loc, 2);
    assert!(choir.notes.is_empty(), "{:?}", choir.notes);
}

#[test]
fn forge_takes_a_chat_answer_with_prose_fences_and_loose_spacing() {
    let answer = "Here is your weapon!\n\n```rune\n# a frosty one\nname Frost Lance\nlook ice shard\nbolt damage = 10 speed= 30 every =6\n```\nEnjoy!";
    let checked = ok(answer);
    assert_eq!(checked.weapon.name, "Frost Lance");
    let bolt = checked.weapon.bolt.unwrap();
    assert_eq!((bolt.damage, bolt.speed, bolt.every), (10, 20.0, 15));
    assert!(
        checked
            .notes
            .iter()
            .any(|note| note.contains("no range given"))
    );
}

#[test]
fn forge_clamps_out_of_range_values_and_says_so() {
    let checked = ok("bolt damage=99 speed=5 every=1 range=500");
    let bolt = checked.weapon.bolt.unwrap();
    assert!((4..=100).contains(&bolt.damage));
    assert_eq!(bolt.speed, 8.0);
    assert_eq!(bolt.range, 48.0);
    assert!(checked.notes.iter().any(|n| n.contains("lowered to fit")));
    assert!(checked.notes.iter().any(|n| n.contains("speed 5")));
}

#[test]
fn forge_lowers_damage_to_fit_the_caps_and_every_result_fits() {
    let checked = ok(
        "bolt damage=24 speed=30 every=3\nspread shots=3 arc=40\nmelee damage=60 reach=2 arc=360 every=8",
    );
    let weapon = &checked.weapon;
    assert!(weapon.dps() <= 70.0, "{}", weapon.dps());
    assert!(weapon.burst() <= 100, "{}", weapon.burst());
    assert!(
        checked
            .notes
            .iter()
            .any(|n| n.contains("lowered to fit the limits"))
    );
    for damage in [4, 12, 24] {
        for every in [3, 10, 30] {
            for shots in [2, 3] {
                let text = format!(
                    "bolt damage={damage} every={every}\nspread shots={shots} arc=20\nmelee damage=60 every=8"
                );
                let weapon = ok(&text).weapon;
                assert!(weapon.dps() <= 70.0 && weapon.burst() <= 100, "{text}");
            }
        }
    }
}

#[test]
fn forge_rejects_with_reasons_a_model_can_fix() {
    let reasons = errors("spread shots=3 arc=20");
    assert!(reasons.iter().any(|e| e.contains("needs a `bolt` line")));
    let reasons = errors("bolt damage=9\nparry\nfireball big");
    assert!(
        reasons
            .iter()
            .any(|e| e.contains("line 2") && e.contains("tier II"))
    );
    assert!(
        reasons
            .iter()
            .any(|e| e.contains("line 3") && e.contains("not a rune"))
    );
    let reasons = errors("bolt damage=9 colour=red");
    assert!(reasons.iter().any(|e| e.contains("no `colour`")));
    let reasons = errors("bolt damage=9\nbolt damage=10");
    assert!(reasons.iter().any(|e| e.contains("only one `bolt`")));
    let reasons = errors("look purple cube\nbolt damage=9");
    assert!(reasons.iter().any(|e| e.contains("look takes")));
    let fix = check("melee", START_LOC).map(|_| ()).err();
    assert!(fix.is_none(), "a bare melee line uses defaults");
}

#[test]
fn forge_counts_loc_and_rejects_spending_more_than_held() {
    let free = ok("# comment\n\nname A\nlook gold orb\nbolt damage=9");
    assert_eq!(free.weapon.loc, 1);
    let reasons = check("bolt damage=9\nmelee damage=20", 1)
        .unwrap_err()
        .errors;
    assert!(
        reasons
            .iter()
            .any(|e| e.contains("use 2 LOC but you hold 1"))
    );
}

#[test]
fn forge_bounds_size_lines_and_line_length() {
    assert!(check(&"#".repeat(MAX_BYTES + 1), START_LOC).is_err());
    let long = format!("name {}\nbolt damage=9", "x".repeat(90));
    assert!(errors(&long).iter().any(|e| e.contains("longer than")));
    let many = "# c\n".repeat(70) + "bolt damage=9";
    assert!(errors(&many).iter().any(|e| e.contains("over 64 lines")));
}

#[test]
fn forge_canonical_runes_round_trip_to_the_same_weapon() {
    let first =
        ok("name  Venom Choir\nlook venom wave\nbolt damage=30 every=4\nspread shots=3 arc=50")
            .weapon;
    let again = ok(&first.runes).weapon;
    assert_eq!(first, again);
}

#[test]
fn forge_rules_name_the_knights_loc_and_current_weapon() {
    let rules = rules(START_LOC, &Weapon::starter().runes);
    assert!(rules.contains("I hold 6 LOC"));
    assert!(rules.contains("bolt damage=32 speed=12 every=15 range=48"));
    assert!(rules.contains("at or under 70"));
    let fix = Rejected {
        errors: vec!["line 2: nope".into()],
    }
    .fix_it();
    assert!(fix.contains("- line 2: nope") && fix.contains("only the corrected rune file"));
}
