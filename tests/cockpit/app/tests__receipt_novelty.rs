//! Actual-App desired-contract controls. No provider or submission is invoked;
//! real correlated tool events simulate successful execution receipts.
use super::*;
use crate::agent::harness::{ExecutionOutcome, ToolEventId, ToolOutcome, VerificationOutcome};
use crate::drive::loop_ctl::{LoopState, LoopStatus};
use crate::ui::toolstrip::{ToolStrip, ToolStripSnapshot};

fn receipt(command: &str, result: &str) -> ToolStripSnapshot {
    let mut strip = ToolStrip::default();
    let id = ToolEventId("owned-simulated-execution".into());
    strip.call_event(id.clone(), "shell", command);
    strip.result_event(
        &id,
        "shell",
        result,
        ToolOutcome {
            execution: ExecutionOutcome::Succeeded,
            verification: VerificationOutcome::NotApplicable,
        },
    );
    strip.snapshot()
}

fn submit(id: usize) -> ToolStripSnapshot {
    receipt(
        "yukon submit candidate.c --note-file submission.md",
        &format!("submission {id} accepted"),
    )
}

fn with_loop(check: impl FnOnce(&mut App, &std::path::Path)) {
    let _lock = env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-receipt-novelty-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let workspace = root.join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&workspace)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(
        workspace.join("candidate.c"),
        "int kernel(void) { return 1; }\n",
    )
    .unwrap();
    std::fs::write(workspace.join("submission.md"), "fixed baseline\n").unwrap();
    // Durable loop/goal stores live outside the candidate workspace so saving
    // the actual App cannot manufacture a workspace-change signal.
    let _file = TestEnvGuard::set("ANGEL_LOOP_FILE", root.join("loop.json").to_str().unwrap());
    let _goal = TestEnvGuard::set("ANGEL_GOAL_FILE", root.join("goal.json").to_str().unwrap());
    let _mirror = TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    let mut app = crate::seed_preview_app();
    app.terminal_focused = false;
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        task: "improve measured kernel performance using a discriminating local check before official evaluation".into(),
        workspace: Some(workspace.clone()),
        podrace: true,
        stall_stop: 20,
        pivot: 20,
        ..Default::default()
    };
    check(&mut app, &workspace);
}

fn harvest(app: &mut App, tools: ToolStripSnapshot) {
    app.loop_harvest_with_tools("DIRECTION: evaluate the candidate".into(), tools);
}

#[test]
fn unchanged_candidate_new_submission_id_is_activity_without_stagnation_reset() {
    with_loop(|app, workspace| {
        let bytes = std::fs::read(workspace.join("candidate.c")).unwrap();
        harvest(app, submit(829));
        app.loop_ctl.stale_count = 2;
        let tier = app.loop_ctl.tier;
        harvest(app, submit(830));
        assert_eq!(std::fs::read(workspace.join("candidate.c")).unwrap(), bytes);
        assert!(!app.loop_ctl.log.last().unwrap().workspace_changed);
        assert_eq!(
            app.loop_ctl.submissions, 2,
            "both actual receipts remain accounted"
        );
        assert_eq!(
            app.loop_ctl.status,
            LoopStatus::Running,
            "useful work continues; no tool dispatch is exercised here"
        );
        assert_eq!(app.loop_ctl.tier, tier, "no automatic model escalation");
        let prompt = app
            .loop_iteration_convo()
            .pop()
            .unwrap()
            .content
            .to_string();
        assert!(
            prompt.contains("Repeated competitive submissions of unchanged candidates are banned"),
            "next action must preserve the repeat ban and request a comparison: {prompt}"
        );
        assert_eq!(
            app.loop_ctl.stale_count, 3,
            "a new server ID does not establish productive novelty"
        );
    });
}

#[test]
fn same_receipt_replay_neither_double_counts_nor_resets_stagnation() {
    with_loop(|app, _| {
        harvest(app, submit(829));
        app.loop_ctl.stale_count = 2;
        harvest(app, submit(829));
        assert_eq!(app.loop_ctl.submissions, 1);
        assert_eq!(app.loop_ctl.stale_count, 3);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    });
}

#[test]
fn changed_candidate_submission_is_activity_without_proven_improvement() {
    with_loop(|app, workspace| {
        harvest(app, submit(829));
        app.loop_ctl.stale_count = 2;
        std::fs::write(
            workspace.join("candidate.c"),
            "int kernel(void) { return 2; }\n",
        )
        .unwrap();
        harvest(app, submit(830));
        assert!(app.loop_ctl.log.last().unwrap().workspace_changed);
        assert_eq!(app.loop_ctl.submissions, 2);
        assert_eq!(
            app.loop_ctl.stale_count, 3,
            "changed source and accepted submission are not proof of objective improvement"
        );
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    });
}

#[test]
fn changing_only_submission_note_does_not_mint_candidate_progress() {
    with_loop(|app, workspace| {
        harvest(app, submit(829));
        app.loop_ctl.stale_count = 2;
        let bytes = std::fs::read(workspace.join("candidate.c")).unwrap();
        std::fs::write(
            workspace.join("submission.md"),
            "redraw unchanged frontier\n",
        )
        .unwrap();
        harvest(app, submit(830));
        assert_eq!(std::fs::read(workspace.join("candidate.c")).unwrap(), bytes);
        assert!(
            app.loop_ctl.log.last().unwrap().workspace_changed,
            "fixture catches the broader workspace-hash shortcut"
        );
        assert_eq!(app.loop_ctl.submissions, 2);
        assert_eq!(app.loop_ctl.stale_count, 3);
    });
}

#[test]
fn failed_turn_with_new_unchanged_submission_receipt_keeps_stagnation() {
    with_loop(|app, _| {
        harvest(app, submit(829));
        app.loop_ctl.stale_count = 2;
        app.loop_harvest_error_with_tools(
            "owned synthetic end-of-turn failure".into(),
            submit(830),
        );
        assert_eq!(app.loop_ctl.submissions, 2);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(app.loop_ctl.stale_count, 3);
    });
}

#[test]
fn failed_turn_status_poll_is_activity_without_candidate_progress() {
    with_loop(|app, _| {
        harvest(app, submit(829));
        app.loop_ctl.stale_count = 2;
        let tools = receipt("yukon status 829", "submission 829 still queued");
        assert!(tools.verified_outcome_actions.is_empty());
        app.loop_harvest_error_with_tools("owned synthetic end-of-turn failure".into(), tools);
        assert_eq!(app.loop_ctl.submissions, 1);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(app.loop_ctl.stale_count, 3);
    });
}

#[test]
fn failed_turn_successful_measurement_keeps_evidence_without_improvement_credit() {
    with_loop(|app, _| {
        harvest(app, submit(829));
        app.loop_ctl.stale_count = 2;
        app.loop_harvest_error_with_tools(
            "owned synthetic end-of-turn failure".into(),
            receipt("./benchmark.sh candidate.c", "score=1.25"),
        );
        assert_eq!(app.loop_ctl.measured_candidates, 1);
        assert_eq!(app.loop_ctl.stale_count, 3);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    });
}

#[test]
fn ordinary_loop_retains_non_competition_outcome_accounting() {
    with_loop(|app, _| {
        harvest(app, submit(829));
        app.loop_ctl.podrace = false;
        app.loop_ctl.stale_count = 2;
        app.loop_harvest_error_with_tools(
            "owned synthetic end-of-turn failure".into(),
            receipt("yukon status 829", "submission 829 still queued"),
        );
        assert_eq!(app.loop_ctl.stale_count, 0);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
    });
}

#[test]
fn competitive_prompt_preserves_repeat_ban_before_first_receipt() {
    with_loop(|app, _| {
        let prompt = app
            .loop_iteration_convo()
            .pop()
            .unwrap()
            .content
            .to_string();
        assert!(prompt.contains("Never resubmit unchanged code"), "{prompt}");
        assert!(
            prompt.contains("Run only required checks"),
            "required validation remains explicit: {prompt}"
        );
        assert!(
            prompt.contains("distinct validated submission"),
            "legitimate distinct submissions stay prompt: {prompt}"
        );
    });
}

#[test]
fn periodic_soft_pivot_preserves_objective_staleness_and_continuation() {
    with_loop(|app, _| {
        app.loop_ctl.stall_stop = 3;
        app.loop_ctl.pivot = 3;
        let tier = app.loop_ctl.tier;
        // These are simulated observed receipts, not authorization to repeat
        // competitive submissions. Neither receipt can erase the prior state.
        app.loop_ctl.stale_count = 2;
        harvest(app, submit(829));
        assert_eq!(app.loop_ctl.stale_count, 3);
        assert_eq!(app.loop_ctl.status, LoopStatus::Running);
        assert_eq!(app.loop_ctl.tier, tier);
        assert!(app.loop_ctl.wake_at.is_some());
        assert!(
            app.loop_ctl
                .last_setback
                .as_deref()
                .unwrap()
                .starts_with("no comparable objective improvement recorded")
        );
        harvest(app, submit(830));
        assert_eq!(app.loop_ctl.stale_count, 4);
        assert!(
            app.loop_ctl
                .last_setback
                .as_deref()
                .unwrap()
                .starts_with("measurement/submission execution recorded"),
            "pivot must remain periodic, not fire on every later turn"
        );
    });
}
