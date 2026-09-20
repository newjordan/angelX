use super::*;

#[test]
fn parses_only_explicit_truthy_values() {
    assert!(!parse(None));
    for value in ["", "0", "false", "NO", "off", "disabled"] {
        assert!(!parse(Some(value)), "{value}");
    }
    for value in ["1", "true", "YES", "on", "yolo"] {
        assert!(parse(Some(value)), "{value}");
    }
}

#[test]
fn status_names_the_behavioral_boundary() {
    let _guard = crate::tests::env_lock();
    let _yolo = crate::tests::TestEnvGuard::set("ANGEL_YOLO", "1");
    let _smart = crate::tests::TestEnvGuard::unset("ANGEL_YOLO_SMART");
    let status = status_text();
    assert!(status.contains("approvals"));
    assert!(status.contains("sandboxing"));
    assert!(status.contains("turn hop/deadline/progress/completion/verification gates"));
    assert!(status.contains("remain"));
}

#[test]
fn smart_and_full_are_mutually_exclusive_when_set() {
    let _guard = crate::tests::env_lock();
    let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _smart = crate::tests::TestEnvGuard::unset("ANGEL_YOLO_SMART");
    assert_eq!(profile(), Profile::Guarded);

    set_smart(true);
    assert!(smart_enabled());
    assert!(!enabled());
    assert_eq!(profile(), Profile::Smart);
    assert!(workspace_power());
    assert!(code_effects_allowed());

    set(true);
    assert!(enabled());
    assert!(!smart_enabled());
    assert_eq!(profile(), Profile::Full);

    set_smart(true);
    assert_eq!(profile(), Profile::Smart);
    assert!(!enabled());

    clear_all();
    assert_eq!(profile(), Profile::Guarded);
}

#[test]
fn smart_auto_approves_only_workspace_scopes() {
    let _guard = crate::tests::env_lock();
    let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "1");
    assert!(smart_auto_approves(
        &crate::approval::ApprovalScope::ActionBatch("action-capsule:deadbeef".into())
    ));
    assert!(smart_auto_approves(
        &crate::approval::ApprovalScope::SelfTest
    ));
    assert!(!smart_auto_approves(
        &crate::approval::ApprovalScope::PhoneModel("sota".into())
    ));
    assert!(!smart_auto_approves(
        &crate::approval::ApprovalScope::RemoteHost("spark".into())
    ));
    assert!(!smart_auto_approves(&crate::approval::ApprovalScope::Peer(
        "reviewer".into()
    )));
}

#[test]
fn smart_status_names_kept_guards() {
    let _guard = crate::tests::env_lock();
    let _full = crate::tests::TestEnvGuard::unset("ANGEL_YOLO");
    let _smart = crate::tests::TestEnvGuard::set("ANGEL_YOLO_SMART", "1");
    let status = status_text();
    assert!(status.contains("SMART"));
    assert!(status.contains("sandbox"));
    assert!(status.contains("timeouts"));
    assert!(status.contains("hooks"));
    assert!(status.contains("verify"));
}
