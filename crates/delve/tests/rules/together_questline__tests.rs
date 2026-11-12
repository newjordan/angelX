//! The questlines' tests: the engine, the files, the lock, the grant and
//! the sheet's lore lines.

use super::*;
use crate::rules::questline::{book, Questline};

/// Every shipped file loads clean, with line-numbered errors when not.
#[test]
fn every_shipped_file_loads_clean() {
    let b = book();
    assert!(b.notes.is_empty(), "notes: {:?}", b.notes);
    // Re-pinned 2026-11-06: Bomrekedan's seven questlines (`bom-*.txt`,
    // `power bomrekedan:`) ship beside the seats; they teach no power, so
    // the seats are the lines that do (was `!is_kingdom()`, 22 of 23).
    let seats = b.lines.iter().filter(|q| !q.is_kingdom() && !q.is_bomrekedan()).count();
    assert_eq!(seats, 22, "the bible names twenty-two seats");
    assert_eq!(b.lines.iter().filter(|q| q.is_bomrekedan()).count(), 7, "the fort's seven lines");
    // Beside them, the kingdom's own line: the Heartwright's.
    assert_eq!(b.lines.len(), 30);
    for q in &b.lines {
        assert!(!q.teacher.is_empty(), "{}: no teacher", q.power);
        assert!(!q.art.is_empty(), "{}: no figure", q.power);
        assert!(!q.trial.name.is_empty(), "{}: no trial", q.power);
    }
}

/// A bad file answers with line numbers.
#[test]
fn a_bad_file_answers_with_line_numbers() {
    let raw = "teacher Nobody\nroom nowhere\nstep A | desc | Not-A-Counter 0 | x\n";
    let notes = Questline::parse("test", raw).unwrap_err();
    assert!(notes.iter().any(|n| n.starts_with("test:3:")), "{notes:?}");
}

/// A tier 3 wish is granted with no questline walked: no teacher holds the
/// third tier back any more (2026-10-22).
#[test]
fn a_tier_three_wish_is_granted_with_no_questline_walked() {
    let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
    let book_ = phrasebook::book();
    let wish = book_.iter().find(|w| w.id == "fork").unwrap();
    for tier in 1..=3 {
        let said = r.grant(1, wish).unwrap_or_else(|why| panic!("tier {tier} is free: {why}"));
        assert!(said.contains("Fork"), "{said}");
    }
}

/// Every wish questline names its wish by the phrasebook's own id, so its
/// teacher opens and its tier 3 names them: Ricochet, Seeker Fletching,
/// Fleet Foot, Iron Skin and Blood Drinker once named theirs bounce,
/// homing, speed, armor and vamp, and were refused with no teacher named.
#[test]
fn every_wish_questline_names_a_wish_and_its_teacher_opens() {
    let wishes = phrasebook::book();
    for q in book().lines.iter().filter(|q| q.power.starts_with("wish:")) {
        let id = &q.power["wish:".len()..];
        assert!(wishes.iter().any(|w| w.id == id), "{}: no such wish", q.power);
    }
    for id in ["ricochet", "seeker", "fleet", "iron", "leech"] {
        let wish = wishes.iter().find(|w| w.id == id).unwrap();
        let q = book().get(&format!("wish:{id}")).unwrap_or_else(|| panic!("{id} has no teacher"));
        let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
        for t in 0..2 {
            r.players.get_mut(&1).unwrap().deck.push(wish.card_id(t));
        }
        // The teacher opens to a knight holding the second level...
        assert!(r.quest_gate(1, q), "{id}: the teacher opens");
        // ...and the third is the knight's without them.
        r.grant(1, wish).unwrap_or_else(|why| panic!("{id}: the third tier is free: {why}"));
    }
}

/// A skill's level III is locked, then taught.
#[test]
fn a_skills_level_three_is_locked_then_taught() {
    let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
    // Every knight starts at level II.
    let hero = r.players.get(&1).unwrap();
    let key = crate::rules::home::Home::knight_key(hero);
    assert_eq!(r.skill_level(1, skills::GAS), 2);
    assert_eq!(r.gas_life(1), skills::GAS_LIFE_L[1]);
    // Level III comes from the questline alone: Widow Peascod's cloud-work.
    let q = book().get("skill:gas").unwrap().clone();
    let mut walk = crate::rules::questline::Walk::default();
    for s in &q.steps {
        walk.counters.insert(s.counter.clone(), s.need);
    }
    walk.counters.insert(q.trial.counter.clone(), q.trial.need);
    walk.trial_done = true;
    r.home
        .questlines
        .knights
        .entry(key.clone())
        .or_default()
        .insert("skill:gas".to_string(), walk);
    assert_eq!(r.skill_level(1, skills::GAS), 3);
    assert_eq!(r.gas_life(1), skills::GAS_LIFE_L[2]);
    assert_eq!(r.gas_radius(1), skills::GAS_RADIUS_L[2]);
}

/// One full questline (Awakened Fork) walked step by step, by cue and by
/// counted deed.
#[test]
fn awakened_fork_walked_step_by_step() {
    let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
    // The gate: the teacher opens to a holder of the second rank.
    let said = r.speak(1, "wish:fork").unwrap();
    assert!(said.contains("patience") || said.contains("becomes two"), "{said}");
    // Hold fork 2: the deck carries the wish's second card.
    let book_ = phrasebook::book();
    let wish = book_.iter().find(|w| w.id == "fork").unwrap();
    let hero = r.players.get_mut(&1).unwrap();
    hero.deck.push(wish.card_id(1));
    r.quest_step_done(1, "wish:fork");
    // Step by step: the one-leg stand (a cue), then the counted deeds.
    let q = book().get("wish:fork").unwrap().clone();
    for s in &q.steps {
        let line = r.speak(1, "wish:fork").unwrap();
        assert!(line.contains(&s.name) || line.contains("—"), "{}", line);
        if s.cue {
            // A cued step serves on the press.
            let again = r.speak(1, "wish:fork").unwrap();
            assert!(!again.is_empty());
        } else {
            r.note_quest(1, &s.counter, s.need);
            r.flush_questlines();
        }
    }
    // The trial: one fork-pure hall.
    r.note_quest(1, &q.trial.counter, 1);
    r.flush_questlines();
    let walk = {
        let hero = r.players.get(&1).unwrap();
        let key = crate::rules::home::Home::knight_key(hero);
        r.home.questlines.walk(&key, "wish:fork")
    };
    assert!(walk.finished(), "the trial is passed");
    // The closing line, and the keepsake once.
    let close = r.speak(1, "wish:fork").unwrap();
    assert!(close.contains("Heron"), "{close}");
    let again = r.speak(1, "wish:fork").unwrap();
    assert!(!again.contains("press"), "{again}");
}

/// Progress survives a save and a load.
#[test]
fn progress_survives_save_and_load() {
    let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
    r.note_quest(1, "one_leg_stands", 1);
    r.flush_questlines();
    let saved = serde_json::to_string(&r.home).unwrap();
    let loaded: crate::rules::home::Home = serde_json::from_str(&saved).unwrap();
    let hero = r.players.get(&1).unwrap();
    let key = crate::rules::home::Home::knight_key(hero);
    assert_eq!(loaded.questlines.walk(&key, "wish:fork").count("one_leg_stands"), 1);
}

/// An old save loads with no progress and loses nothing.
#[test]
fn an_old_save_loads() {
    let old = r#"{"wardrobe":{},"knights":{}}"#;
    let home: crate::rules::home::Home = serde_json::from_str(old).unwrap();
    assert!(home.questlines.knights.is_empty());
}

/// The sheet shows a teacher and a step.
#[test]
fn the_sheet_shows_a_teacher_and_a_step() {
    let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
    let book_ = phrasebook::book();
    let wish = book_.iter().find(|w| w.id == "fork").unwrap();
    let hero = r.players.get_mut(&1).unwrap();
    hero.deck.push(wish.card_id(1));
    let sheet = crate::client::sheet(&r, 1);
    assert!(sheet.contains("Stilt"), "the sheet names the teacher");
    assert!(sheet.contains("millpond"), "the sheet says where they live");
    r.note_quest(1, "one_leg_stands", 1);
    r.flush_questlines();
    let sheet = crate::client::sheet(&r, 1);
    assert!(sheet.contains("1/1") || sheet.contains("step"), "{sheet}");
}
