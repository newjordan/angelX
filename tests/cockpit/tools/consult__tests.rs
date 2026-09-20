use super::*;

struct MockClub {
    label: String,
    response: String,
    available: bool,
}

impl MockClub {
    fn new(label: &str, response: &str) -> Self {
        Self {
            label: label.to_string(),
            response: response.to_string(),
            available: true,
        }
    }

    fn down(label: &str) -> Self {
        Self {
            label: label.to_string(),
            response: "should not run".to_string(),
            available: false,
        }
    }
}

impl Club for MockClub {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        if !self.available {
            return Err(format!("{} model not available", self.label));
        }
        Ok(self.response.clone())
    }
    fn label(&self) -> &str {
        &self.label
    }
    fn is_available(&self) -> bool {
        self.available
    }
    fn chat_streaming(
        &self,
        _messages: &[ChatMsg],
        _tools: &[ToolDef],
        _cancel: &std::sync::atomic::AtomicBool,
        _on_delta: &mut dyn FnMut(crate::agent::club::StreamDelta),
    ) -> Result<ClubReply, String> {
        if !self.available {
            return Err(format!("{} model not available", self.label));
        }
        Ok(ClubReply::Text(self.response.clone()))
    }
}

#[test]
fn consult_model_calls_resolved_club() {
    let mock_luna: Arc<dyn Club> = Arc::new(MockClub::new("luna", "Luna response for code help"));
    let fallback: Arc<dyn Club> = Arc::new(MockClub::new("self", "Fallback self response"));

    let tool = ConsultModelTool::new(vec![mock_luna], Some(fallback));
    let res = tool.call(&serde_json::json!({
        "club": "luna",
        "prompt": "How do I optimize Tokio channel receiver loops?"
    }));
    assert!(res.is_ok());
    let text = res.unwrap();
    assert!(text.contains("luna"));
    assert!(text.contains("Luna response for code help"));
}

#[test]
fn consult_unknown_club_fails_closed() {
    let openai: Arc<dyn Club> = Arc::new(MockClub::new("openai", "should not run"));
    let fallback: Arc<dyn Club> = Arc::new(MockClub::new("deepseek", "self"));
    let tool = ConsultModelTool::new(vec![openai], Some(fallback));
    let err = tool
        .call(&serde_json::json!({
            "club": "not-a-real-club",
            "prompt": "hi"
        }))
        .unwrap_err();
    assert!(err.contains("unknown club"), "{err}");
    assert!(!err.contains("should not run"));
}

#[test]
fn consult_openai_blocked_by_explicit_user_setting() {
    let _g = crate::tests::env_lock();
    crate::agent::tools::solo::set_solo_mode(false);
    let _policy = crate::tests::TestEnvGuard::set("ANGEL_ALLOW_SOTA_CONSULT", "0");
    let openai: Arc<dyn Club> = Arc::new(MockClub::new("openai", "codex"));
    let self_c: Arc<dyn Club> = Arc::new(MockClub::new("deepseek", "self"));
    let tool = ConsultModelTool::new(vec![openai], Some(self_c));
    let err = tool
        .call(&serde_json::json!({
            "club": "openai",
            "prompt": "think hard for me"
        }))
        .unwrap_err();
    assert!(
        err.contains("blocked") || err.contains("solo") || err.contains("ALLOW_SOTA"),
        "{err}"
    );
}

#[test]
fn consult_configured_remote_model_works_without_extra_permission_flag() {
    let _g = crate::tests::env_lock();
    crate::agent::tools::solo::set_solo_mode(false);
    let _policy = crate::tests::TestEnvGuard::unset("ANGEL_ALLOW_SOTA_CONSULT");
    let remote: Arc<dyn Club> = Arc::new(MockClub::new("openai-api", "fixture review"));
    let tool = ConsultModelTool::new(vec![remote], None);
    let result = tool
        .call(&serde_json::json!({
            "club": "openai-api", "prompt": "review this fixture"
        }))
        .unwrap();
    assert!(result.contains("fixture review"), "{result}");
    assert!(tool.def().description.contains("Configured remote models"));
}

#[test]
fn code_review_defaults_to_self() {
    let openai: Arc<dyn Club> = Arc::new(MockClub::new("openai", "should not run"));
    let fallback: Arc<dyn Club> =
        Arc::new(MockClub::new("deepseek", "Verdict: Clean. Self review."));

    let tool = CodeReviewTool::new(vec![openai], Some(fallback));
    let res = tool.call(&serde_json::json!({
        "target": "fn main() { println!(\"hello\"); }",
        "focus": "correctness"
    }));
    assert!(res.is_ok());
    let text = res.unwrap();
    assert!(text.contains("deepseek"), "{text}");
    assert!(text.contains("Self review"));
    assert!(!text.contains("should not run"));
}

#[test]
fn consult_down_local_falls_back_to_another_live_local() {
    let turbo: Arc<dyn Club> = Arc::new(MockClub::down("turbo"));
    let spark: Arc<dyn Club> = Arc::new(MockClub::new("spark", "spark is up"));
    let self_c: Arc<dyn Club> = Arc::new(MockClub::new("self", "self should wait"));
    let tool = ConsultModelTool::new(vec![turbo, spark], Some(self_c));
    let text = tool
        .call(&serde_json::json!({
            "club": "turbo",
            "prompt": "review this kernel"
        }))
        .unwrap();
    assert!(text.contains("spark"), "{text}");
    assert!(text.contains("spark is up"), "{text}");
    assert!(text.contains("turbo is not reachable"), "{text}");
    assert!(!text.contains("should not run"), "{text}");
}

#[test]
fn consult_down_local_falls_back_to_self_when_no_peer_is_up() {
    let turbo: Arc<dyn Club> = Arc::new(MockClub::down("turbo"));
    let self_c: Arc<dyn Club> = Arc::new(MockClub::new("deepseek", "do it in-hand"));
    let tool = ConsultModelTool::new(vec![turbo], Some(self_c));
    let text = tool
        .call(&serde_json::json!({
            "club": "turbo",
            "prompt": "second opinion"
        }))
        .unwrap();
    assert!(text.contains("do it in-hand"), "{text}");
    assert!(text.contains("turbo is not reachable"), "{text}");
    assert!(text.contains("self (deepseek)"), "{text}");
}

#[test]
fn consult_down_local_skips_instead_of_failing_when_nothing_is_up() {
    let turbo: Arc<dyn Club> = Arc::new(MockClub::down("turbo"));
    let tool = ConsultModelTool::new(vec![turbo], None);
    let text = tool
        .call(&serde_json::json!({
            "club": "turbo",
            "prompt": "second opinion"
        }))
        .unwrap();
    assert!(text.contains("consult_model skipped"), "{text}");
    assert!(text.contains("turbo is not reachable"), "{text}");
    assert!(text.contains("Do the work yourself"), "{text}");
    assert!(!text.contains("should not run"), "{text}");
}

#[test]
fn code_review_down_local_falls_back_to_self() {
    let turbo: Arc<dyn Club> = Arc::new(MockClub::down("turbo"));
    let fallback: Arc<dyn Club> = Arc::new(MockClub::new("deepseek", "Verdict: Clean."));
    let tool = CodeReviewTool::new(vec![turbo], Some(fallback));
    let text = tool
        .call(&serde_json::json!({
            "club": "turbo",
            "target": "fn main() {}",
            "focus": "correctness"
        }))
        .unwrap();
    assert!(text.contains("deepseek"), "{text}");
    assert!(text.contains("Verdict: Clean."), "{text}");
    assert!(text.contains("turbo is not reachable"), "{text}");
}

#[test]
fn leanstral_tool_sends_only_the_caller_snippet() {
    let leanstral: Arc<dyn Club> = Arc::new(MockClub::new("leanstral", "by decide"));
    let glm: Arc<dyn Club> = Arc::new(MockClub::new("glm", "should not run"));
    let tool = LeanstralTool::new(vec![leanstral, glm]);
    let text = tool
        .call(&serde_json::json!({
            "prompt": "lemma foo : 1 + 1 = 2 := by sorry"
        }))
        .unwrap();
    assert!(text.contains("[leanstral club=leanstral]"), "{text}");
    assert!(text.contains("by decide"), "{text}");
    assert!(!text.contains("should not run"), "{text}");
}

#[test]
fn leanstral_tool_skips_when_the_specialist_is_down() {
    let leanstral: Arc<dyn Club> = Arc::new(MockClub::down("leanstral"));
    let turbo: Arc<dyn Club> = Arc::new(MockClub::new("turbo", "should not run"));
    let tool = LeanstralTool::new(vec![leanstral, turbo]);
    let text = tool
        .call(&serde_json::json!({ "prompt": "lemma foo : True := trivial" }))
        .unwrap();
    assert!(text.contains("leanstral skipped"), "{text}");
    assert!(!text.contains("should not run"), "{text}");
}
