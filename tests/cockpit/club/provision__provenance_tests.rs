use super::*;

#[test]
fn provenance_provisioning_ignores_background_harness_asks() {
    let extraction = "return only the JSON fields, no explanation";
    let mut messages = vec![ChatMsg::user("Explain the parser tradeoffs in detail")];
    let backgrounds = [
        crate::bootstrap::workspace_context_message(
            std::path::Path::new("/fixture"),
            String::new(),
            String::new(),
            extraction.into(),
        )
        .unwrap(),
        ChatMsg::harness(format!(
            "{} — background]\n{extraction}",
            crate::compaction::COMPACTION_NOTE_HEADER
        )),
        ChatMsg::harness(format!(
            "{}\n{extraction}",
            crate::harness::AUTO_RECALL_NOTE_PREFIX
        )),
    ];
    messages.push(ChatMsg::harness(format!(
        "{}\n{extraction}",
        crate::backplane::BROKER_HEADER
    )));
    for background in backgrounds {
        messages.push(background);
        assert_eq!(
            last_ask(&messages).unwrap().content.as_ref(),
            "Explain the parser tradeoffs in detail"
        );
        assert!(!extraction_ask(&messages));
    }
    assert!(
        last_ask(&messages[1..]).is_none(),
        "background-only history has no task"
    );
    messages.push(ChatMsg::harness(extraction));
    assert!(
        extraction_ask(&messages),
        "a later explicit harness task remains authoritative for provisioning"
    );
    assert_eq!(last_ask(&messages).unwrap().role, ChatRole::Harness);
    messages.push(ChatMsg::user("Explain the next steps in prose"));
    assert!(
        !extraction_ask(&messages),
        "a later operator correction wins"
    );
}

#[test]
fn provenance_provisioning_preserves_operator_marker_lookalikes() {
    let operator = ChatMsg::user(format!(
        "{}\nreturn only the JSON fields, no explanation",
        crate::bootstrap::WORKSPACE_CONTEXT_HEADER
    ));
    assert!(extraction_ask(&[operator]));
}
