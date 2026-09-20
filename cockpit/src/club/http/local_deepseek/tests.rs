use super::*;
use crate::club::{ChatMsg, Club, HttpClub, Metadata, ToolCall};

const ALIAS: &str = "deepseek-v4-flash-dspark";
const LOCAL: &str = "http://127.0.0.1:18888/v1";

fn configured() -> HttpClub {
    let mut club = HttpClub::new("owned-local-profile", LOCAL, ALIAS, None);
    club.local_reasoning_profile =
        ReasoningProfile::parse(Some("deepseek-v4-vllm"), LOCAL, Some(ALIAS));
    club.inject_metadata_for_tests(Metadata {
        context_window: 1_048_576,
        supports_cache: true,
        supports_reasoning: None,
        supports_tools: true,
    });
    club
}

#[test]
fn local_deepseek_profile_exposes_verified_rungs_and_preserves_wire_model() {
    let club = configured();
    assert_eq!(
        club.reasoning_levels()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        LEVELS,
    );
    for level in LEVELS {
        assert_eq!(club.set_reasoning_effort(level).as_deref(), Some(level));
        let body = club
            .build_body(&[ChatMsg::user("bounded check")], &[], true)
            .unwrap();
        assert_eq!(body["model"], ALIAS);
        assert_eq!(body["reasoning_effort"], level);
        assert!(body.get("thinking").is_none());
        assert!(body.get("chat_template_kwargs").is_none());
    }
    assert_eq!(club.set_reasoning_effort("invented"), None);
}

#[test]
fn local_deepseek_profile_does_not_enable_official_private_reasoning_replay() {
    let club = configured();
    let calls = vec![ToolCall {
        id: "owned-call".into(),
        name: "local_probe_sum".into(),
        args: serde_json::json!({"a": 7, "b": 11}),
    }];
    let messages = [
        ChatMsg::user("call the fixture"),
        ChatMsg::assistant_calls_with_reasoning(calls, Some("owned-private-reasoning".into())),
        ChatMsg::tool("owned-call", "18"),
    ];
    let body = club
        .build_body_with_effort(&messages, &[], true, Some("max"))
        .unwrap();
    assert_eq!(body["model"], ALIAS);
    assert_eq!(body["messages"][1]["content"], "");
    assert_eq!(body["messages"][2]["tool_call_id"], "owned-call");
    assert!(!body.to_string().contains("owned-private-reasoning"));
    assert!(body["messages"][1].get("reasoning_content").is_none());
}

#[test]
fn local_deepseek_profile_rejects_public_unpinned_unknown_and_changed_model() {
    for profile in [
        ReasoningProfile::parse(
            Some("deepseek-v4-vllm"),
            "https://api.deepseek.com/v1",
            Some(ALIAS),
        ),
        ReasoningProfile::parse(Some("deepseek-v4-vllm"), LOCAL, None),
        ReasoningProfile::parse(Some("future-profile"), LOCAL, Some(ALIAS)),
    ] {
        let mut club = configured();
        club.local_reasoning_profile = profile;
        assert!(club.reasoning_levels().is_empty());
        assert!(
            club.build_body(&[ChatMsg::user("check")], &[], true)
                .is_err()
        );
    }
    let club = configured();
    *club.model.lock().unwrap() = Some("replacement-model".into());
    assert!(club.reasoning_levels().is_empty());
    assert!(
        club.build_body(&[ChatMsg::user("check")], &[], true)
            .unwrap_err()
            .contains("different served model")
    );
}

#[test]
fn local_deepseek_profile_leaves_unconfigured_aliases_and_rejections_truthful() {
    let plain = HttpClub::new("owned-unconfigured-alias", LOCAL, ALIAS, None);
    assert_eq!(plain.set_reasoning_effort("max"), None);
    let club = configured();
    *club.reasoning_rejected.lock().unwrap() =
        Some("owned backend rejected reasoning_effort".into());
    assert!(club.reasoning_levels().is_empty());
    assert_eq!(club.set_reasoning_effort("max"), None);
    let body = club
        .build_body_with_effort(&[ChatMsg::user("check")], &[], true, Some("max"))
        .unwrap();
    assert!(body.get("reasoning_effort").is_none());
    assert!(
        club.effort_gate_usage()
            .last
            .unwrap()
            .contains("backend rejected")
    );
    assert!(
        ReasoningProfile::parse(Some("deepseek-v4-vllm"), LOCAL, Some(ALIAS))
            .error(ALIAS, super::super::ReasoningDialect::QwenEnableThinking)
            .unwrap()
            .contains("openai effort")
    );
}
