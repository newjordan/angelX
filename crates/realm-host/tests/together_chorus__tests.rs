use super::*;

#[test]
fn every_line_is_someone_talking_to_the_party() {
    let lines = script();
    assert!(lines.len() >= 100);
    for line in &lines {
        assert!(!line.words.is_empty() && !line.cue.is_empty(), "{line:?}");
        assert_ne!(
            name(&line.who),
            line.who.as_str(),
            "{} is a named character",
            line.who
        );
        assert!(
            priority(&line.cue) > 2,
            "{} is dialogue, not commentary on the fight",
            line.cue
        );
    }
    for gone in ["herald", "shoggoth", "warden", "cinderjaw", "leviathan"] {
        assert!(
            lines.iter().all(|l| l.who != gone),
            "nobody narrates: {gone}"
        );
    }
    let mut ids: Vec<_> = lines.iter().map(|l| &l.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), lines.len(), "line ids are unique");
}

#[test]
fn a_greeting_shows_and_holds_a_moment() {
    let mut chorus = Chorus::default();
    let now = Instant::now();
    chorus.cue("npc:grubbins", now);
    let said = chorus.tick(now).unwrap();
    assert_eq!(said.line.who, "grubbins");
    assert!(chorus.tick(now).is_none(), "one line at a time");
    assert_eq!(
        chorus.subtitle(said.until).map(|l| l.id.as_str()),
        Some(said.line.id.as_str())
    );
}

#[test]
fn small_moments_wait_their_turn_and_big_ones_cut_in() {
    let mut chorus = Chorus::default();
    let now = Instant::now();
    chorus.cue("npc:kay", now);
    let said = chorus.tick(now).unwrap();
    chorus.cue("npc:kay", now + Duration::from_secs(1));
    chorus.cue("bar", now);
    assert!(
        chorus.tick(said.until).is_none(),
        "cooldowns and a busy subtitle let small moments go"
    );
    chorus.cue("wheel:turbo", now);
    let next = chorus.tick(now).unwrap();
    assert_eq!(next.line.who, "fortune", "the wheel cuts in");
}

#[test]
fn specific_lines_win_and_the_fight_is_left_to_the_knights() {
    let mut chorus = Chorus::default();
    let now = Instant::now();
    chorus.cue("built:forge", now);
    assert_eq!(chorus.tick(now).unwrap().line.cue, "built:forge");
    let later = now + Duration::from_secs(60);
    for fight in [
        "victory",
        "boss_rise:cinderjaw",
        "card_arm:handgonne",
        "room_clear",
    ] {
        chorus.cue(fight, later);
        assert!(
            chorus.tick(later).is_none(),
            "{fight} is the knights' to remark on"
        );
    }
}
