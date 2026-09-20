use super::*;

#[test]
fn solo_blocks_sota_even_when_consult_env_on() {
    let _g = crate::tests::env_lock();
    set_solo_mode(false);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("ANGEL_ALLOW_SOTA_CONSULT", "1") };
    assert!(allow_sota_consult());
    assert!(deny_sota_if_blocked("openai").is_ok());
    set_solo_mode(true);
    assert!(!allow_sota_consult());
    assert!(deny_sota_if_blocked("openai").is_err());
    assert!(deny_sota_if_blocked("codex").is_err());
    assert!(
        deny_sota_if_blocked("gpt-5.3-codex-spark").is_err()
            || deny_sota_if_blocked("openai").is_err()
    );
    // Local-ish labels that aren't sota stay open.
    assert!(deny_sota_if_blocked("practice").is_ok());
    set_solo_mode(false);
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var("ANGEL_ALLOW_SOTA_CONSULT") };
}
