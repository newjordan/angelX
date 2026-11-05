use super::*;
use crate::agent::club::{
    read_timeout_on_this_thread as bound, with_read_timeout_at_least as within,
};
use angel_realm_host::together_grants::{GRANT_CALL, GRANT_RETRY};

#[test]
fn a_grants_floor_raises_only_a_shorter_bound_and_only_on_its_own_thread() {
    let s = Duration::from_secs;
    assert_eq!(within(GRANT_CALL, || bound(s(60))), GRANT_CALL);
    // The operator's longer bound, or none at all, stands.
    assert_eq!(within(GRANT_CALL, || bound(s(600))), s(600));
    assert_eq!(within(GRANT_CALL, || bound(Duration::ZERO)), Duration::ZERO);
    // Nested, then restored; another thread never sees it.
    within(GRANT_CALL, || {
        assert_eq!(within(GRANT_RETRY, || bound(s(60))), GRANT_RETRY);
        assert_eq!(bound(s(60)), GRANT_CALL);
        assert_eq!(
            std::thread::spawn(move || bound(s(60))).join().unwrap(),
            s(60)
        );
    });
    assert_eq!(bound(s(60)), s(60));
}

/// A club that says the whole-call bound a 60 s policy reads as on its thread.
struct Bound;

impl Club for Bound {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok(bound(Duration::from_secs(60)).as_secs().to_string())
    }

    fn label(&self) -> &str {
        "bound"
    }
}

#[test]
fn the_club_in_hand_answers_the_realm_under_the_floor_it_was_asked() {
    let oracle = oracle(Arc::new(Bound));
    assert_eq!(oracle.label(), "bound");
    assert_eq!(oracle.respond("a wish", GRANT_CALL), Ok("90".to_string()));
    assert_eq!(oracle.respond("a wish", GRANT_RETRY), Ok("120".to_string()));
    // Under the club's own 60 s bound the floor does not shorten anything.
    assert_eq!(
        oracle.respond("a wish", Duration::from_secs(10)),
        Ok("60".to_string())
    );
    // And the floor does not outlive the call.
    assert_eq!(bound(Duration::from_secs(60)), Duration::from_secs(60));
}
