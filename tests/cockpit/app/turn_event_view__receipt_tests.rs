use super::*;
#[test]
fn receipt_keys_range_and_latest_reason() {
    let mut row = notice_text("action receipt · shell dispatch error · 2694 ms · helper: ENOENT");
    for ms in [43, 1503] {
        row = receipt_gauge_text(
            &row,
            &format!(
                "action receipt · shell dispatch error · {ms} ms · sandbox: mount not confined"
            ),
        )
        .unwrap();
    }
    assert_eq!(
        row,
        ".. action receipt · shell dispatch error ×3 · last 1503 ms · 43–2694 ms · sandbox: mount not confined"
    );
    for class in [
        "applied",
        "ran",
        "dispatch error",
        "denied",
        "timeout",
        "verifier failed",
    ] {
        let note = format!("action receipt · shell {class} · 4 ms");
        let parsed = action_receipt(&note).unwrap();
        assert_eq!(
            parsed.key,
            format!(
                "receipt:shell:{}",
                if class == "verifier failed" {
                    "verifier"
                } else {
                    class
                }
            )
        );
    }
    assert!(receipt_gauge_text(&row, "action receipt · shell ran · 2 ms").is_none());
    assert!(action_receipt("action receipt · shell ran · bad ms").is_none());
    println!("receipt key classes=6; latest=1503 ms min=43 ms max=2694 ms; latest reason retained");
}
