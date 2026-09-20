use super::queue_proc_completion_for_loop;
use crate::loop_ctl::{LoopState, LoopStatus};
use std::path::Path;

#[test]
fn completion_wakes_only_authorized_matching_running_loop() {
    let workspace = Path::new("/solver/project-a");
    let mut state = LoopState {
        workspace: Some(workspace.to_path_buf()),
        status: LoopStatus::Running,
        max_iters: 4,
        iteration: 4, // wakeup must not replenish an exhausted budget
        token_budget: 100,
        tokens_spent: 100,
        ..Default::default()
    };
    assert!(!queue_proc_completion_for_loop(
        &mut state,
        Path::new("/solver/project-b"),
        "foreign".into()
    ));
    assert!(state.pending_proc_completions.is_empty());
    assert!(queue_proc_completion_for_loop(
        &mut state,
        workspace,
        "proof exited 2; not accepted".into()
    ));
    assert!(state.wake_at.is_some());
    assert_eq!(
        (
            state.max_iters,
            state.iteration,
            state.token_budget,
            state.tokens_spent
        ),
        (4, 4, 100, 100)
    );
    for status in [
        LoopStatus::Idle,
        LoopStatus::Paused,
        LoopStatus::Stopped,
        LoopStatus::Done,
        LoopStatus::Failed,
    ] {
        state.status = status;
        state.wake_at = None;
        assert!(!queue_proc_completion_for_loop(
            &mut state,
            workspace,
            "completion".into()
        ));
        assert!(state.wake_at.is_none());
    }
}

#[test]
fn completion_context_is_bounded_and_does_not_collide_with_active_turn() {
    let workspace = Path::new("/solver/project");
    let mut state = LoopState {
        workspace: Some(workspace.into()),
        status: LoopStatus::Running,
        awaiting_turn: true,
        ..Default::default()
    };
    for n in 0..8 {
        assert!(queue_proc_completion_for_loop(
            &mut state,
            workspace,
            format!("job {n} exited")
        ));
    }
    assert!(!queue_proc_completion_for_loop(
        &mut state,
        workspace,
        "overflow".into()
    ));
    assert_eq!(state.pending_proc_completions.len(), 8);
    assert!(state.wake_at.is_none());
}
