use super::*;
use crate::rules::{
    host_commands::{Command, Journal, Reply, Request as CommandRequest},
    runtime_state,
};
fn card() -> Card {
    cards::check(
        "paint-capture",
        "name Paint Capture\nkind spell\ncooldown 5\nnova 10\nart\n00\n00\ndraw shot\n00\n00\n",
    )
    .unwrap()
    .card
}
fn request() -> Request {
    let mut current = card();
    current.by = "Original maker".into();
    let mut authored = current.clone();
    authored.by.clear();
    authored.art = vec!["11".into(), "11".into()];
    authored.looks.shot.as_mut().unwrap().frames = vec![authored.art.clone()];
    Request {
        candidates: vec![Candidate { current, authored }],
    }
}
fn game() -> (Run, Journal) {
    let mut run = Run::new(0, 9, Some("Guest"));
    run.book.insert(request().candidates.remove(0).current);
    run.players.get_mut(&1).unwrap().spell_cooldowns[0] = 71;
    run.sounds.push("hit");
    run.cues.push("already issued effect".into());
    (run, Journal::new("paint-session".into(), 9))
}
#[test]
fn painter_changes_only_captured_visuals_preserving_game_state_and_maker() {
    let (mut run, mut journal) = game();
    let request = request();
    let updates = request.updates(&request.evaluate().unwrap()).unwrap();
    let before = runtime_state::capture(&run).unwrap().run;
    let receipt = journal
        .apply(
            &mut run,
            CommandRequest {
                session: "paint-session".into(),
                raid: 9,
                id: 1,
                commands: vec![Command::PaintCards {
                    updates: updates.clone(),
                }],
            },
        )
        .unwrap();
    assert_eq!(receipt.result, Ok(vec![Reply::Changed(true)]));
    let painted = run.book.get("paint-capture").unwrap();
    assert_eq!(painted.by, "Original maker");
    assert_eq!(painted.art, request.candidates[0].authored.art);
    assert_eq!(painted.cooldown, 5);
    let mut after = runtime_state::capture(&run).unwrap().run;
    after["book"] = before["book"].clone();
    assert_eq!(after, before, "paint changed non-catalogue state");
    let mut projected = painted.clone();
    projected.art = updates[0].expected.art.clone();
    projected.looks = updates[0].expected.looks.clone();
    assert_eq!(projected, updates[0].expected);
}
#[test]
fn replay_handoff_and_late_capture_are_atomic() {
    let (mut run, mut journal) = game();
    let request = request();
    let updates = request.updates(&request.evaluate().unwrap()).unwrap();
    let command = CommandRequest {
        session: "paint-session".into(),
        raid: 9,
        id: 1,
        commands: vec![Command::PaintCards {
            updates: updates.clone(),
        }],
    };
    let receipt = journal.apply(&mut run, command.clone()).unwrap();
    let before = runtime_state::capture(&run).unwrap().into_value();
    let mut journal = Journal::restore(journal.capture().unwrap(), "paint-session", 9).unwrap();
    assert_eq!(journal.apply(&mut run, command).unwrap(), receipt);
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
    // One stale entry rejects the whole batch, before even its valid first entry.
    let mut stale = updates[0].clone();
    stale.expected.id = "unknown-paint".into();
    let current = run.book.get("paint-capture").unwrap().clone();
    let valid = Update {
        expected: current,
        art: updates[0].expected.art.clone(),
        looks: updates[0].expected.looks.clone(),
    };
    let failed = journal
        .apply(
            &mut run,
            CommandRequest {
                session: "paint-session".into(),
                raid: 9,
                id: 2,
                commands: vec![Command::PaintCards {
                    updates: vec![valid, stale],
                }],
            },
        )
        .unwrap();
    assert!(failed.result.is_err());
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
    journal.acknowledge(9, 1).unwrap();
    journal.acknowledge(9, 2).unwrap();
}
#[test]
fn changed_source_policy_can_accept_art_without_replacing_behavior() {
    let (mut run, _) = game();
    let mut request = request();
    request.candidates[0].authored.effects = vec![cards::Effect::Nova(99)];
    request.candidates[0].authored.cooldown = 1;
    request.candidates[0].authored.name = "Different definition".into();
    assert!(request.evaluate().unwrap().accepted.is_empty());
    // The replaceable policy may permit visual reuse under a changed source.
    let decision = Decision {
        accepted: vec!["paint-capture".into()],
    };
    deliver(&mut run, &request.updates(&decision).unwrap()).unwrap();
    let card = run.book.get("paint-capture").unwrap();
    assert_eq!(card.effects, vec![cards::Effect::Nova(10)]);
    assert_eq!(card.cooldown, 5);
    assert_eq!(card.name, "Paint Capture");
    assert_eq!(card.by, "Original maker");
    assert_eq!(card.art, request.candidates[0].authored.art);
}
#[test]
fn malformed_duplicates_unrequested_results_and_oversize_are_rejected() {
    let mut request = request();
    assert!(request.validate().is_ok());
    assert!(
        request
            .validate_result(&Decision {
                accepted: vec!["other".into()]
            })
            .is_err()
    );
    assert!(
        request
            .validate_result(&Decision {
                accepted: vec!["paint-capture".into(); 2]
            })
            .is_err()
    );
    let mut malformed = request.clone();
    malformed.candidates[0]
        .authored
        .looks
        .shot
        .as_mut()
        .unwrap()
        .frames = vec![vec!["*".into()]];
    assert!(malformed.validate().is_err());
    request.candidates.push(request.candidates[0].clone());
    assert!(request.validate().is_err());
    request.candidates = vec![request.candidates[0].clone(); MAX_CANDIDATES + 1];
    assert!(request.validate().is_err());
}

#[test]
fn paint_cannot_change_identity_parts_or_create_an_unearned_body_piece() {
    let raw = "name Copper Crown\nkind hold\ndamage 10\ndraw worn at=head\n00\n00\n";
    let current = cards::check("copper-crown", raw).unwrap().card;
    let authored = cards::check("copper-crown", &raw.replace("at=head", "at=feet"))
        .unwrap()
        .card;
    let request = Request {
        candidates: vec![Candidate {
            current: current.clone(),
            authored,
        }],
    };
    request.validate().unwrap();
    assert!(request.evaluate().unwrap().accepted.is_empty());
    assert!(
        request
            .validate_result(&Decision {
                accepted: vec![current.id.clone()]
            })
            .is_err()
    );
    let candidate = &request.candidates[0];
    let bad = Update {
        expected: current.clone(),
        art: candidate.authored.art.clone(),
        looks: candidate.authored.looks.clone(),
    };
    assert!(!valid_updates(&[bad.clone()]));
    let mut run = Run::new(0, 9, None);
    run.book.insert(current);
    let before = runtime_state::capture(&run).unwrap().into_value();
    assert!(deliver(&mut run, &[bad]).is_err());
    assert_eq!(runtime_state::capture(&run).unwrap().into_value(), before);
    let current = cards::check("copper-crown", "name Copper Crown\nkind hold\ndamage 10\n")
        .unwrap()
        .card;
    let authored = cards::check("copper-crown", raw).unwrap().card;
    let request = Request {
        candidates: vec![Candidate { current, authored }],
    };
    request.validate().unwrap();
    assert!(request.evaluate().unwrap().accepted.is_empty());
}
