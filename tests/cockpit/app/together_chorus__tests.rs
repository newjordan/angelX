use super::*;

#[test]
fn every_scripted_line_is_well_formed_and_most_are_voiced() {
    let lines = script();
    assert!(lines.len() >= 60);
    let dir = voices_dir();
    // Lines may wait for a recording (they show as subtitles meanwhile).
    let mut recorded = 0;
    for line in &lines {
        assert!(!line.words.is_empty() && !line.cue.is_empty(), "{line:?}");
        recorded += usize::from(recording(dir.as_deref(), line).is_some());
        assert_ne!(
            name(&line.who),
            line.who.as_str(),
            "{} is a named character",
            line.who
        );
    }
    assert!(recorded >= 80, "most lines are voiced ({recorded})");
    let mut ids: Vec<_> = lines.iter().map(|l| &l.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), lines.len(), "line ids are unique");
}

#[test]
fn big_moments_are_a_duet_and_bosses_speak_for_themselves() {
    let mut chorus = Chorus {
        muted: true,
        ..Chorus::default()
    };
    let now = Instant::now();
    chorus.cue("boss_rise:cinderjaw", now);
    let first = chorus.tick(now).unwrap();
    assert_eq!(first.line.who, "herald");
    assert!(chorus.tick(now).is_none(), "one line at a time");
    let second = chorus.tick(first.until).unwrap();
    assert_eq!(second.line.id, "cinderjaw-rise");
    assert_eq!(
        chorus.subtitle(first.until).map(|l| l.id.as_str()),
        Some("cinderjaw-rise")
    );
}

#[test]
fn small_moments_wait_their_turn_and_big_ones_cut_in() {
    let mut chorus = Chorus {
        muted: true,
        ..Chorus::default()
    };
    let now = Instant::now();
    chorus.cue("room_clear", now);
    let said = chorus.tick(now).unwrap();
    chorus.cue("room_clear", now + Duration::from_secs(1));
    chorus.cue("bond", now);
    assert!(
        chorus.tick(said.until).is_none(),
        "cooldowns and a busy voice let small moments go"
    );
    chorus.cue("victory", now);
    let next = chorus.tick(now).unwrap();
    assert_eq!(next.line.who, "herald", "victory interrupts");
    assert_eq!(next.line.words, "Victory!");
}

#[test]
fn specific_lines_win_and_general_ones_cover_the_rest() {
    let mut chorus = Chorus {
        muted: true,
        ..Chorus::default()
    };
    let now = Instant::now();
    chorus.cue("card_arm:handgonne", now);
    assert_eq!(chorus.tick(now).unwrap().line.id, "tobbin-handgonne");
    let later = now + Duration::from_secs(60);
    chorus.cue("card_arm:some-friends-axe", later);
    assert!(
        chorus.tick(later).is_none(),
        "a pickup nobody can name stays quiet"
    );
    chorus.cue("card_guard:kite-shield", later);
    assert_eq!(chorus.tick(later).unwrap().line.id, "tobbin-shield");
}

#[test]
fn a_small_secret_speaks_its_own_line_alone_and_hints_once_each() {
    let mut chorus = Chorus {
        muted: true,
        ..Chorus::default()
    };
    let now = Instant::now();
    chorus.cue("secret_found:cup", now);
    let said = chorus.tick(now).unwrap();
    assert_eq!(said.line.id, "herald-found-cup", "not the vault's line");
    assert!(
        chorus.tick(said.until).is_none(),
        "no second voice for a small find"
    );
    for name in [
        "cup", "glass", "seal", "salt", "coal", "lantern", "reed", "wick", "sill",
    ] {
        for cue in [
            format!("secret_hint:{name}"),
            format!("secret_found:{name}"),
        ] {
            assert!(script().iter().any(|l| l.cue == cue), "{cue} has a line");
        }
    }
}
