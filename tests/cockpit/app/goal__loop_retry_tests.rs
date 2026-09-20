use super::*;
use crate::drive::loop_ctl::{LoopState, LoopStatus};
use std::time::{Duration, Instant};

fn fixture(tag: &str, test: impl FnOnce(&Path)) {
    let _lock = crate::tests::env_lock();
    let root = std::env::temp_dir().join(format!(
        "angel-goal-retry-{tag}-{}-{}",
        std::process::id(),
        now_ms()
    ));
    std::fs::create_dir(&root).unwrap();
    let _goal_file = crate::tests::TestEnvGuard::set(
        "ANGEL_GOAL_FILE",
        root.join("store/goal.json").to_str().unwrap(),
    );
    let _loop_file = crate::tests::TestEnvGuard::set(
        "ANGEL_LOOP_FILE",
        root.join("loop.json").to_str().unwrap(),
    );
    let _mirror = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_STATE_DIR");
    test(&root);
    std::fs::remove_dir_all(root).unwrap();
}

fn app_with_goal() -> crate::App {
    let mut app = crate::seed_preview_app();
    let mut goal = Goal::new("exact operator objective");
    goal.rounds = 1;
    goal.max_rounds = Some(3);
    goal.blocked_reason = Some("retain this candidate blocker on failed checkpoint".into());
    goal.blocked_streak = 2;
    goal.updated_ms = 123;
    save_for(&mut goal, app.tools.current_workspace()).unwrap();
    app.goal = Some(goal);
    app.loop_ctl = LoopState {
        status: LoopStatus::Running,
        workspace: Some(app.tools.current_workspace().to_path_buf()),
        tokens_spent: 41,
        ..Default::default()
    };
    app
}

#[test]
fn failed_goal_checkpoint_retries_without_consuming_rounds_or_mutating_live_goal() {
    fixture("io", |root| {
        let mut app = app_with_goal();
        let before = serde_json::to_vec(app.goal.as_ref().unwrap()).unwrap();
        let committed = std::fs::read(root.join("store/goal.json")).unwrap();
        // Exact owned path blocker: ENOTDIR-like I/O failure, not filesystem filling.
        std::fs::rename(root.join("store"), root.join("retained-store")).unwrap();
        std::fs::write(root.join("store"), b"owned parent blocker").unwrap();
        for _ in 0..3 {
            app.loop_ctl.wake_at = None; // exercise the next timer expiration without sleeping
            let start = Instant::now();
            app.loop_arm();
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert!(app.loop_ctl.wake_at.unwrap() >= start + Duration::from_secs(60));
            assert!(app.thinking.is_none() && app.loop_pending.is_none());
            assert!(!app.loop_ctl.awaiting_turn);
            assert_eq!(app.loop_ctl.tokens_spent, 41);
            assert_eq!(app.loop_ctl.iteration, 0);
            assert_eq!(
                serde_json::to_vec(app.goal.as_ref().unwrap()).unwrap(),
                before
            );
            assert_eq!(
                std::fs::read(root.join("retained-store/goal.json")).unwrap(),
                committed
            );
            assert!(
                app.loop_ctl
                    .last_error
                    .as_deref()
                    .unwrap()
                    .contains("goal durability checkpoint failed")
            );
            app.loop_ctl = crate::drive::loop_ctl::load_for(app.tools.current_workspace()).unwrap();
            assert_eq!(app.loop_ctl.status, LoopStatus::Running);
            assert!(
                app.loop_ctl.wake_at.unwrap() > Instant::now() + Duration::from_secs(59),
                "restart preserves checkpoint backoff"
            );
            app.loop_arm();
            assert!(
                app.thinking.is_none(),
                "future wake cannot buy a model retry"
            );
            assert_eq!(
                serde_json::to_vec(app.goal.as_ref().unwrap()).unwrap(),
                before
            );
        }
        std::fs::remove_file(root.join("store")).unwrap();
        std::fs::rename(root.join("retained-store"), root.join("store")).unwrap();
        // Exercise the successful reservation directly, without any model call.
        assert_eq!(app.loop_goal_gate(), Ok(None));
        let goal = app.goal.as_ref().unwrap();
        assert_eq!(
            goal.rounds, 2,
            "one successful reservation after three failed attempts"
        );
        assert_eq!(goal.text, "exact operator objective");
        assert_eq!(goal.max_rounds, Some(3));
        assert_eq!(goal.blocked_reason, None);
        assert_eq!(goal.blocked_streak, 0);
        assert_eq!(load_for(app.tools.current_workspace()).unwrap().rounds, 2);
        assert!(
            !std::fs::read_dir(root.join("store")).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp"))
        );
    });
}

#[test]
fn explicit_goal_gates_and_round_cap_remain_paused_even_when_storage_is_broken() {
    fixture("gates", |root| {
        for (status, spent) in [
            (GoalStatus::Paused, false),
            (GoalStatus::Blocked, false),
            (GoalStatus::Done, false),
            (GoalStatus::Active, true),
        ] {
            let mut app = app_with_goal();
            let goal = app.goal.as_mut().unwrap();
            goal.status = status;
            if spent {
                goal.rounds = 3;
            }
            let before = serde_json::to_vec(goal).unwrap();
            std::fs::rename(root.join("store"), root.join("retained-store")).unwrap();
            std::fs::write(root.join("store"), b"owned parent blocker").unwrap();
            app.loop_arm();
            assert_eq!(app.loop_ctl.status, LoopStatus::Paused);
            assert!(app.loop_ctl.wake_at.is_none());
            assert!(app.thinking.is_none());
            assert_eq!(app.loop_ctl.tokens_spent, 41);
            assert_eq!(
                serde_json::to_vec(app.goal.as_ref().unwrap()).unwrap(),
                before
            );
            assert!(
                !app.messages
                    .last()
                    .unwrap()
                    .text
                    .contains("checkpoint failed"),
                "explicit gate precedes storage"
            );
            std::fs::remove_file(root.join("store")).unwrap();
            std::fs::rename(root.join("retained-store"), root.join("store")).unwrap();
        }
    });
}
