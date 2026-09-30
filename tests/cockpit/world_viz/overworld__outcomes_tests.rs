use super::*;
use crate::agent::harness::ToolEventId;
use crate::stage::world_viz::World;

fn outcome(execution: ExecutionOutcome, verification: VerificationOutcome) -> ToolOutcome {
    ToolOutcome {
        execution,
        verification,
    }
}

#[test]
fn receipts_distinguish_execution_verification_and_unresolved_work() {
    use ExecutionOutcome::*;
    use VerificationOutcome::{Failed as Red, Inconclusive, NotApplicable, Passed};
    for (execution, verification, seal) in [
        (Succeeded, NotApplicable, Seal::Completed),
        (Succeeded, Passed, Seal::Verified),
        (Succeeded, Red, Seal::Failed),
        (Succeeded, Inconclusive, Seal::Unresolved),
        (Failed, NotApplicable, Seal::Failed),
        (Panicked, NotApplicable, Seal::Failed),
        (NotStarted, NotApplicable, Seal::Unresolved),
        (Denied, NotApplicable, Seal::Unresolved),
        (Cancelled, NotApplicable, Seal::Unresolved),
        // A verifier label cannot turn an unexecuted call into a victory.
        (Cancelled, Passed, Seal::Unresolved),
    ] {
        assert_eq!(Seal::from_outcome(outcome(execution, verification)), seal);
    }
}

#[test]
fn a_receipt_needs_a_matching_call_and_duplicate_results_cannot_extend_it() {
    let mut world = World::new(7);
    let id = ToolEventId("lantern-trial".into());
    let green = outcome(ExecutionOutcome::Succeeded, VerificationOutcome::Passed);
    world.note_tool_result_event(&id, "shell", "passed", green);
    assert!(world.overworld_scene().outcomes.is_empty());
    world.note_tool_call_event(id.clone(), "shell", "cargo test");
    assert!(
        world.overworld_scene().outcomes.is_empty(),
        "dispatch is not success"
    );
    world.note_tool_result_event(&id, "shell", "passed", green);
    let echoes = world.overworld_scene().outcomes;
    assert_eq!(echoes.len(), 1);
    assert_eq!(
        (echoes[0].place, echoes[0].seal),
        (Place::Lists, Seal::Verified)
    );
    for _ in 0..LIFETIME - 1 {
        world.tick();
    }
    assert_eq!(world.overworld_scene().outcomes.len(), 1);
    world.note_tool_result_event(&id, "shell", "passed", green);
    world.tick();
    assert!(world.overworld_scene().outcomes.is_empty());
}

#[test]
fn a_new_errand_clears_its_old_lantern_and_uses_the_real_district() {
    let mut world = World::new(7);
    let old = ToolEventId("lantern-first".into());
    world.note_tool_call_event(old.clone(), "shell", "python analyze.py");
    world.note_tool_result_event(
        &old,
        "shell",
        "finished",
        outcome(
            ExecutionOutcome::Succeeded,
            VerificationOutcome::NotApplicable,
        ),
    );
    let echoes = world.overworld_scene().outcomes;
    assert_eq!(
        (echoes[0].place, echoes[0].seal),
        (Place::Village, Seal::Completed)
    );
    world.note_tool_call_event(
        ToolEventId("lantern-next".into()),
        "shell",
        "python analyze.py",
    );
    assert!(world.overworld_scene().outcomes.is_empty());
}

#[test]
fn result_storms_keep_only_one_lantern_per_place_and_latest_truth_wins() {
    let mut receipts = Outcomes::default();
    for tick in 0..5_000 {
        for place in Place::ALL {
            receipts.note(
                place,
                outcome(ExecutionOutcome::Succeeded, VerificationOutcome::Passed),
                tick,
            );
        }
    }
    receipts.note(
        Place::Smithy,
        outcome(ExecutionOutcome::Failed, VerificationOutcome::NotApplicable),
        5_000,
    );
    let shown = receipts.shown(5_000);
    assert_eq!(shown.len(), Place::ALL.len());
    assert_eq!(
        shown
            .iter()
            .find(|echo| echo.place == Place::Smithy)
            .unwrap()
            .seal,
        Seal::Failed
    );
    assert!(receipts.shown(5_000 + LIFETIME).is_empty());
}

#[test]
fn motion_preferences_keep_still_receipts_and_outcomes_have_distinct_shapes() {
    let early = Echo {
        place: Place::Smithy,
        seal: Seal::Verified,
        phase: 0,
    };
    let later = Echo { phase: 4, ..early };
    for motion in [MotionMode::Reduced, MotionMode::Off] {
        assert_eq!(
            lantern(early, motion).rgb_bytes(),
            lantern(later, motion).rgb_bytes()
        );
    }
    assert_ne!(
        lantern(early, MotionMode::Full).rgb_bytes(),
        lantern(later, MotionMode::Full).rgb_bytes()
    );
    let pictures: Vec<_> = [
        Seal::Completed,
        Seal::Verified,
        Seal::Failed,
        Seal::Unresolved,
    ]
    .into_iter()
    .map(|seal| lantern(Echo { seal, ..early }, MotionMode::Off).rgb_bytes())
    .collect();
    for i in 0..pictures.len() {
        for j in i + 1..pictures.len() {
            assert_ne!(pictures[i], pictures[j]);
        }
    }
    let mut scene = super::super::Scene::resting();
    scene.outcome_motion = MotionMode::Off;
    scene.outcomes = vec![later];
    super::super::pace(&mut scene, false);
    assert_eq!(
        scene.outcomes[0].phase, 0,
        "still receipts do not invalidate the cache each phase"
    );
}

#[test]
fn outcome_receipts_change_the_landmark_picture_and_stay_on_palette() {
    use super::super::ink::{BLACK, palette_index};
    use super::super::{Scene, View, frame_at};

    let scene = Scene::resting();
    let (tx, ty) = Place::Smithy.stand_world();
    let view = View::around((tx * 16) as f32, (ty * 16 - 24) as f32, 160, 112);
    let baseline = frame_at(&scene, view).rgb_bytes();
    for (name, seal) in [
        ("completed", Seal::Completed),
        ("verified", Seal::Verified),
        ("failed", Seal::Failed),
        ("unresolved", Seal::Unresolved),
    ] {
        let mut shown = scene.clone();
        shown.outcomes.push(Echo {
            place: Place::Smithy,
            seal,
            phase: 2,
        });
        assert_ne!(shown.key(), scene.key());
        let frame = frame_at(&shown, view);
        assert_ne!(
            frame.rgb_bytes(),
            baseline,
            "{name} must be visible at its landmark"
        );
        assert!(
            frame
                .pixels()
                .all(|pixel| pixel == BLACK || palette_index(pixel).is_some())
        );
        // Same opt-in review output as the existing deed-scene tests.
        if let Some(dir) = std::env::var_os("ANGEL_OVERWORLD_SHOTS") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            let mut ppm = format!("P6\n{} {}\n255\n", frame.w, frame.h).into_bytes();
            ppm.extend(frame.rgb_bytes());
            std::fs::write(dir.join(format!("outcome-{name}.ppm")), ppm).unwrap();
        }
    }
}
