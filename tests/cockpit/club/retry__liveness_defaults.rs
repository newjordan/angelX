use super::*;

#[test]
fn silent_provider_defaults_are_bounded_to_two_minutes() {
    let _lock = crate::tests::env_lock();
    let _timeout = crate::tests::TestEnvGuard::unset("ANGEL_HTTP_TIMEOUT");
    let _retries = crate::tests::TestEnvGuard::unset("ANGEL_HTTP_RETRIES");
    let _rate_retries = crate::tests::TestEnvGuard::unset("ANGEL_HTTP_RATELIMIT_RETRIES");

    let policy = HttpPolicy::from_env();
    assert_eq!(
        policy.read_timeout,
        Duration::from_secs(DEFAULT_HTTP_READ_TIMEOUT_SECS)
    );
    assert_eq!(policy.retries, DEFAULT_HTTP_RETRIES);
    assert_eq!(policy.rate_limit_retries, DEFAULT_HTTP_RATELIMIT_RETRIES);
    assert_eq!(
        policy.read_timeout * (policy.retries + 1),
        Duration::from_secs(120),
        "a silent provider must not multiply into a many-minute foreground hang"
    );
}
