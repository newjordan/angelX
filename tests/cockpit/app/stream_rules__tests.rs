use super::*;

#[test]
fn parses_valid_rules_and_skips_bad_ones() {
    let json = r#"[
            {"pattern": "(?i)as an ai", "reminder": "answer directly"},
            {"pattern": "TODO", "reminder": ""},
            {"pattern": "(unclosed", "reminder": "bad regex is skipped"},
            {"reminder": "no pattern"},
            {"pattern": "stub", "reminder": "no stubs"}
        ]"#;
    let rules = parse_rules(json);
    // Only the two well-formed rules survive; empty-reminder and bad-regex
    // and missing-pattern entries are dropped.
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].reminder, "answer directly");
    assert_eq!(rules[1].reminder, "no stubs");
}

#[test]
fn first_new_match_respects_fired_set() {
    let rules = StreamRules {
        rules: parse_rules(
            r#"[{"pattern":"foo","reminder":"r1"},{"pattern":"bar","reminder":"r2"}]"#,
        ),
    };
    let mut fired = HashSet::new();
    // Matches rule 0 first.
    let (idx, rule) = rules.first_new_match("xx foo yy", &fired).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(rule.reminder, "r1");
    // After firing 0, the same text no longer trips it.
    fired.insert(0);
    assert!(rules.first_new_match("xx foo yy", &fired).is_none());
    // A different pattern still trips.
    let (idx, _) = rules.first_new_match("foo and bar", &fired).unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn empty_or_garbage_source_yields_no_rules() {
    assert!(parse_rules("").is_empty());
    assert!(parse_rules("not json").is_empty());
    assert!(parse_rules("{}").is_empty());
    assert!(parse_rules("[]").is_empty());
}
