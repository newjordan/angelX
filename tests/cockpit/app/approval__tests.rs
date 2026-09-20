use super::*;
use std::thread;

/// Spawn a fake UI that answers each incoming request from a scripted list of
/// decisions (in order), recording how many requests it actually received.
fn fake_ui(b: &Broker, script: Vec<Decision>) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let rx = b.set_ui();
    let count = Arc::new(AtomicUsize::new(0));
    let c = Arc::clone(&count);
    thread::spawn(move || {
        let mut it = script.into_iter();
        while let Ok(req) = rx.recv() {
            c.fetch_add(1, Ordering::SeqCst);
            let d = it.next().unwrap_or(Decision::Deny);
            let _ = req.reply.send(d);
        }
    });
    count
}

#[test]
fn adversarial_approval_reply_cannot_cross_turn_reset() {
    let _lock = crate::tests::env_lock();
    for decision in [Decision::Approve, Decision::ApproveAll] {
        let b = Broker::default();
        let ui = b.set_ui();
        let scope = ApprovalScope::ActionBatch("exact-batch".into());
        thread::scope(|threads| {
            let worker = threads.spawn(|| b.ask(scope.clone(), "old turn"));
            let request = ui.recv().unwrap();
            b.reset();
            request.reply.send(decision).unwrap();
            assert_eq!(worker.join().unwrap(), Decision::Deny);
        });
        assert!(b.state.lock().unwrap().grants.is_empty());
    }
}

#[test]
fn headless_denies() {
    let b = Broker::default();
    assert_eq!(
        b.ask(ApprovalScope::PhoneModel("sota".into()), "call SOTA?"),
        Decision::Deny
    );
}

#[test]
fn scope_labels_are_typed_single_line_and_bounded() {
    assert_eq!(ApprovalScope::SelfTest.label(), "self-test");
    assert_eq!(
        ApprovalScope::RemoteHost(" spark\n\u{202e}evil\t host ".into()).label(),
        "remote host · spark evil host"
    );
    assert_eq!(
        ApprovalScope::Peer("\u{00ad}\u{200b}\u{fe0f}\n".into()).label(),
        "peer · unspecified"
    );
    assert_eq!(
        ApprovalScope::Peer("a\u{0007}b".into()).label(),
        "peer · ab"
    );
    let label = ApprovalScope::PhoneModel("x".repeat(200)).label();
    assert!(label.starts_with("phone model · "));
    assert_eq!(
        label.trim_start_matches("phone model · ").chars().count(),
        SCOPE_FRAGMENT_MAX_CHARS
    );
    assert!(!label.contains('\n'));
    let boundary = ApprovalScope::ActionBatch(format!("{} next", "x".repeat(95))).label();
    assert!(!boundary.ends_with(' '));
    assert_eq!(
        boundary
            .trim_start_matches("action batch · ")
            .chars()
            .count(),
        95
    );
}

#[test]
fn yolo_global_ask_approves_without_a_ui() {
    let _guard = crate::tests::env_lock();
    let previous = std::env::var_os("ANGEL_YOLO");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_YOLO", "1") };
    assert_eq!(
        ask(ApprovalScope::RemoteHost("spark".into()), "run it"),
        Decision::Approve
    );
    match previous {
        // TODO: Audit that the environment access only happens in single-threaded code.
        Some(value) => unsafe { std::env::set_var("ANGEL_YOLO", value) },
        // TODO: Audit that the environment access only happens in single-threaded code.
        None => unsafe { std::env::remove_var("ANGEL_YOLO") },
    }
}

#[test]
fn approve_and_deny_pass_through() {
    let b = Broker::default();
    let _c = fake_ui(&b, vec![Decision::Approve, Decision::Deny]);
    let scope = ApprovalScope::PhoneModel("reasoner".into());
    assert_eq!(b.ask(scope.clone(), "1"), Decision::Approve);
    assert_eq!(b.ask(scope, "2"), Decision::Deny);
}

#[test]
fn approve_all_coalesces_same_scope_without_reprompting() {
    let b = Broker::default();
    // The fake UI would only answer ONE request (then Deny). If coalescing
    // works, the 2nd/3rd asks never reach the UI.
    let count = fake_ui(&b, vec![Decision::ApproveAll]);
    let scope = ApprovalScope::PhoneModel("math".into());
    assert_eq!(b.ask(scope.clone(), "agent 1"), Decision::Approve);
    assert_eq!(b.ask(scope.clone(), "agent 2"), Decision::Approve);
    assert_eq!(b.ask(scope, "agent 3"), Decision::Approve);
    // give the UI thread a beat to have logged anything it received
    thread::sleep(Duration::from_millis(20));
    assert_eq!(
        count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "only the first ask prompts"
    );
}

#[test]
fn approve_all_is_exact_to_host_model_peer_or_batch() {
    let b = Broker::default();
    let count = fake_ui(
        &b,
        vec![Decision::ApproveAll, Decision::Deny, Decision::Deny],
    );
    let first = ApprovalScope::PhoneModel("model-a".into());
    assert_eq!(b.ask(first.clone(), "p"), Decision::Approve);
    assert_eq!(b.ask(first, "p2"), Decision::Approve);
    assert_eq!(
        b.ask(ApprovalScope::PhoneModel("model-b".into()), "p3"),
        Decision::Deny
    );
    assert_eq!(
        b.ask(ApprovalScope::RemoteHost("model-a".into()), "r"),
        Decision::Deny
    );
    thread::sleep(Duration::from_millis(20));
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 3);
}

#[test]
fn reset_clears_approve_all() {
    let b = Broker::default();
    let count = fake_ui(&b, vec![Decision::ApproveAll, Decision::ApproveAll]);
    let scope = ApprovalScope::ActionBatch("sha256:a".into());
    assert_eq!(b.ask(scope.clone(), "a"), Decision::Approve);
    assert_eq!(b.ask(scope.clone(), "b"), Decision::Approve); // cached
    b.reset();
    assert_eq!(b.ask(scope, "c"), Decision::Approve); // prompts again after reset
    thread::sleep(Duration::from_millis(20));
    assert_eq!(
        count.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "reset re-armed the prompt"
    );
}

#[test]
fn stale_grant_expires_inside_the_same_turn() {
    let b = Broker::default();
    let scope = ApprovalScope::Peer("reviewer".into());
    let count = fake_ui(&b, vec![Decision::ApproveAll, Decision::Deny]);
    assert_eq!(b.ask(scope.clone(), "first"), Decision::Approve);
    b.state
        .lock()
        .unwrap()
        .grants
        .get_mut(&scope)
        .unwrap()
        .granted_at = std::time::Instant::now() - REPLY_TIMEOUT;
    assert_eq!(b.ask(scope, "expired"), Decision::Deny);
    thread::sleep(Duration::from_millis(20));
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
}
