//! The questlines' tests: the engine, the files, the lock, the grant and
//! the sheet's lore lines.

use super::*;
use crate::rules::questline::{book, Questline};

/// Every shipped file loads clean, with line-numbered errors when not.
#[test]
fn every_shipped_file_loads_clean() {
    let b = book();
    assert!(b.notes.is_empty(), "notes: {:?}", b.notes);
    assert_eq!(b.lines.len(), 22, "the bible names twenty-two seats");
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

/// A knight is turned away before the questline and granted after it.
#[test]
fn a_tier_three_wish_is_locked_then_granted() {
    let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
    let book_ = phrasebook::book();
    let wish = book_.iter().find(|w| w.id == "fork").unwrap();
    let q = book().get("wish:fork").unwrap().clone();
    let hero = r.players.get(&1).unwrap();
    let key = crate::rules::home::Home::school_key(hero);
    // Tier 1 and tier 2 stay as the school set them: Adept and Master,
    // the school's own locks, untouched by the teachers.
    r.home.school.knights.insert(
        key.clone(),
        crate::rules::school::Standing {
            rank: crate::rules::school::Rank::Master,
            ..Default::default()
        },
    );
    for _ in 0..2 {
        r.grant(1, wish).expect("tiers one and two are free");
    }
    // Tier 3 answers in one line naming the teacher and where they live.
    let refused = r.grant(1, wish).unwrap_err();
    assert!(
        refused.contains("Stilt") && refused.contains("millpond"),
        "the refusal names the teacher and the place: {refused}"
    );
    // ...and after the questline is walked to its end, it grants: every
    // counter at its need and the trial passed.
    let mut walk = crate::rules::questline::Walk::default();
    for s in &q.steps {
        walk.counters.insert(s.counter.clone(), s.need);
        walk.done.insert(walk.done.len() as u32 + 1);
    }
    walk.counters.insert(q.trial.counter.clone(), q.trial.need);
    walk.trial_done = true;
    r.home
        .questlines
        .knights
        .entry(key)
        .or_default()
        .insert("wish:fork".to_string(), walk);
    let said = r.grant(1, wish).expect("the questline is walked");
    assert!(said.contains("Fork"), "{said}");
}

/// A skill's level III is locked, then taught.
#[test]
fn a_skills_level_three_is_locked_then_taught() {
    let mut r = crate::rules::Run::new(3, 1, Some("Friend"));
    assert_eq!(r.skill_level(1, skills::GAS), 1);
    // The school's Adept rank buys level II.
    let hero = r.players.get(&1).unwrap();
    let key = crate::rules::home::Home::school_key(hero);
    r.home.school.knights.insert(
        key.clone(),
        crate::rules::school::Standing {
            rank: crate::rules::school::Rank::Adept,
            ..Default::default()
        },
    );
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
        let key = crate::rules::home::Home::school_key(hero);
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
    let key = crate::rules::home::Home::school_key(hero);
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
