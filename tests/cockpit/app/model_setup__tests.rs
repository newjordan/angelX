use super::connect_text;

#[test]
fn routes_provider_guidance_without_secret_values() {
    let grok = connect_text(Some("GROK"));
    assert!(grok.contains("grok login --oauth"));
    assert!(grok.contains("ANGEL_GROK_OAUTH_FILE"));
    assert!(!grok.contains("oauth-token"));
    assert!(!grok.contains("sk-"));

    let unknown = connect_text(Some("made-up"));
    assert!(unknown.contains("unknown provider"));
    assert!(unknown.contains("usage: /connect"));
    assert!(connect_text(None).contains("providers:"));
}
