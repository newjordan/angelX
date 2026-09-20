use super::*;
#[test]
fn receipt_dispatch_short_reason() {
    let preview = ActionPreview {
        tool: "shell".into(),
        scope: String::new(),
        command_like: true,
    };
    for (error, reason) in [
        (
            "sandbox: mount not confined; private body",
            "sandbox: mount not confined",
        ),
        ("spawn failed: ENOENT\nprivate body", "helper: ENOENT"),
        ("timed out; private body", "timeout"),
    ] {
        let receipt = preview.receipt(&format!("tool error: {error}"), 12);
        assert_eq!(
            receipt,
            format!("action receipt · shell dispatch error · 12 ms · {reason}")
        );
    }
}
