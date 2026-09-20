use super::*;

#[test]
fn parses_block_note_and_clear() {
    assert_eq!(
        parse_verdict("BLOCK: the loop never terminates")
            .blocker
            .as_deref(),
        Some("the loop never terminates")
    );
    assert_eq!(
        parse_verdict("note: consider the empty case")
            .note
            .as_deref(),
        Some("consider the empty case")
    );
    assert!(parse_verdict("CLEAR").is_clear());
    assert!(parse_verdict("looks good to me").is_clear());
    // Empty message after the marker is treated as clear, not a blank block.
    assert!(parse_verdict("BLOCK:").is_clear());
    assert!(parse_verdict("NOTE:").is_clear());
    assert!(parse_verdict("NOTE:\nBLOCK:").is_clear());
    // Mixed case and empty BLOCK: fall through to a later NOTE.
    assert_eq!(
        parse_verdict("BlOcK:\n  NoTe: recovered").note.as_deref(),
        Some("recovered")
    );
}

#[test]
fn block_takes_precedence_and_ignores_trailing_prose() {
    let reply = "BLOCK: missing error handling\nsome extra chatter";
    let v = parse_verdict(reply);
    assert_eq!(v.blocker.as_deref(), Some("missing error handling"));
    assert!(v.note.is_none());
}

#[test]
fn annotate_marks_blocker_and_note_distinctly() {
    let block = annotate(&parse_verdict("BLOCK: x")).unwrap();
    assert_eq!(block, "\n\n> ⚠ advisor (blocker): x");
    let mixed = annotate(&parse_verdict("bLoCk: x")).unwrap();
    assert_eq!(mixed, block);
    let note = annotate(&parse_verdict("NOTE: y")).unwrap();
    assert_eq!(note, "\n\n> 💡 advisor: y");
    assert!(annotate(&parse_verdict("CLEAR")).is_none());
    assert!(annotate(&parse_verdict("BLOCK:")).is_none());
    let hop = annotate_hop(&parse_verdict("NOTE: thrash")).unwrap();
    assert_eq!(hop, "[advisor hop · note] thrash");
    assert_eq!(
        annotate_hop(&parse_verdict("BLOCK: stop")),
        Some("[advisor hop · blocker] stop".into())
    );
    // Repeated annotation detection stays on the exact answer bytes.
    let twice = format!("done{block}{block}");
    assert!(already_annotated(&twice));
    assert!(already_annotated(&format!("done{note}")));
}

#[test]
fn review_prompt_embeds_task_and_answer() {
    let p = review_prompt("do the thing", "here is the thing");
    assert!(p.contains("do the thing"));
    assert!(p.contains("here is the thing"));
    let h = hop_review_prompt("task", "ran tests · red");
    assert!(h.contains("TOOL HOP") && h.contains("ran tests"));
}

#[test]
fn mode_from_env_tokens() {
    let _lock = crate::tests::env_lock();
    let _g = crate::tests::TestEnvGuard::unset("ANGEL_ADVISOR");
    assert_eq!(AdvisorMode::from_env(), AdvisorMode::Off);
    let _g = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "1");
    assert_eq!(AdvisorMode::from_env(), AdvisorMode::Final);
    assert!(final_enabled() && !hops_enabled());
    let _g = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "hops");
    assert_eq!(AdvisorMode::from_env(), AdvisorMode::Hops);
    assert!(final_enabled() && hops_enabled());
    let _g = crate::tests::TestEnvGuard::set("ANGEL_ADVISOR", "off");
    assert_eq!(AdvisorMode::from_env(), AdvisorMode::Off);
}

#[test]
fn already_annotated_detects_prior_gate() {
    assert!(!already_annotated("plain answer"));
    assert!(already_annotated("done\n\n> ⚠ advisor (blocker): x"));
    assert!(already_annotated("done\n\n> 💡 advisor: y"));
}
