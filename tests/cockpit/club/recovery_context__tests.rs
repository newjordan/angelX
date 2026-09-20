use super::*;
#[test]
fn recovery_context_is_durable_but_not_http_provider_wire_or_model_instruction() {
    let _lock = crate::tests::env_lock();
    let mut message = ChatMsg::user("diagnostic");
    message.recovery_context.push(owned_recovery_context_ref());
    let saved = serde_json::to_vec(&message).unwrap();
    let restored: ChatMsg = serde_json::from_slice(&saved).unwrap();
    assert_eq!(restored.recovery_context, message.recovery_context);
    assert_eq!(&*restored.content, "diagnostic");
    let club = HttpClub::new("owned", "http://127.0.0.1:1/v1", "owned-model", None);
    let body = club
        .build_body(&[restored], &[], false)
        .unwrap()
        .to_string();
    assert!(!body.contains("owned-import"));
    assert!(!body.contains("recovery_context"));
    assert!(body.contains("diagnostic"));
}
