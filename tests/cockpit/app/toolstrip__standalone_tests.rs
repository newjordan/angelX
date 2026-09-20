#[cfg(test)]
#[test]
fn delegate_status_reports_real_activity_and_heartbeat_only_silence() {
    let mut delegate = crate::agent::harness::DelegateSnapshot {
        label: "test".into(),
        phase: "thinking".into(),
        elapsed_secs: 900,
        event_age_secs: Some(0),
        progress_age_secs: Some(1),
        calls: 12,
        delegates: 1,
    };
    let row = delegate_status_row(&delegate, 110);
    assert!(row.left.contains("delegate · thinking"));
    assert!(row.right.contains("#12 · update 1s ago"));
    assert!(!row.verifier);
    delegate.progress_age_secs = Some(180);
    let row = delegate_status_row(&delegate, 150);
    assert!(row.left.contains("quiet 3m00s · connected"), "{}", row.left);
    assert_eq!(delegate_quiet_secs(&delegate), 180);
    for width in 0..151 {
        let row = delegate_status_row(&delegate, width);
        assert!(
            UnicodeWidthStr::width(row.left.as_str())
                + row.padding
                + UnicodeWidthStr::width(row.right.as_str())
                <= width
        );
    }
}

#[cfg(test)]
#[test]
fn worker_status_distinguishes_cpu_silence_and_small_panes() {
    let mut child = crate::agent::harness::ChildSnapshot {
        program: "rustc".into(),
        elapsed_secs: 540,
        output_age_secs: None,
        cpu_age_secs: Some(0),
        setting_up: false,
        workers: 1,
    };
    let row = worker_status_row(&child, 100);
    assert!(row.left.contains("compiling (rustc) · CPU active"));
    assert!(row.right.contains("output none"));
    assert!(!row.verifier);
    assert_eq!(child_quiet_secs(&child), 0);
    child.cpu_age_secs = Some(40);
    assert!(worker_status_row(&child, 100).left.contains("quiet 40s"));
    assert_eq!(child_quiet_secs(&child), 40);
    child.output_age_secs = Some(0);
    assert!(
        worker_status_row(&child, 100)
            .left
            .contains("output received")
    );
    for width in 0..101 {
        let row = worker_status_row(&child, width);
        assert!(
            UnicodeWidthStr::width(row.left.as_str())
                + row.padding
                + UnicodeWidthStr::width(row.right.as_str())
                <= width
        );
    }
}
