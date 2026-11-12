use super::*;
use crate::agent::club::Club;
use crate::app::control::realm::club_oracle as oracle;
use std::sync::Arc;
use std::time::Duration;
use angel_realm_host::together_grants::{GRANT_CALL, GRANT_RETRY};
use crate::agent::club::{read_timeout_on_this_thread as bound, with_read_timeout_at_least as within};

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
        assert_eq!(std::thread::spawn(move || bound(s(60))).join().unwrap(), s(60));
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
    assert_eq!(oracle.respond("a wish", Duration::from_secs(10)), Ok("60".to_string()));
    // And the floor does not outlive the call.
    assert_eq!(bound(Duration::from_secs(60)), Duration::from_secs(60));
}

/// A club with an effort to choose, as the OpenAI route is: it says which
/// effort a realm ask came at, or that it was asked plainly.
struct Efforted {
    levels: Vec<String>,
}

impl Club for Efforted {
    fn respond(&self, _prompt: &str) -> Result<String, String> {
        Ok("plain".into())
    }

    fn label(&self) -> &str {
        "efforted"
    }

    fn reasoning_effort(&self) -> Option<String> {
        Some("xhigh".into())
    }

    fn reasoning_levels(&self) -> &[String] {
        &self.levels
    }

    fn chat_with_effort(
        &self,
        _messages: &[crate::agent::club::ChatMsg],
        _tools: &[crate::agent::club::ToolDef],
        effort: Option<&str>,
    ) -> Result<crate::agent::club::ClubReply, String> {
        // The grant's floor still holds inside the effort's call.
        Ok(crate::agent::club::ClubReply::Text(format!(
            "{} {}",
            effort.unwrap_or("none"),
            bound(Duration::from_secs(60)).as_secs()
        )))
    }
}

#[test]
fn a_realm_ask_reasons_at_medium_not_at_the_cockpits_effort() {
    let _guard = crate::tests::env_lock();
    let levels = ["low", "medium", "high", "xhigh"].map(String::from).to_vec();
    let oracle = oracle(Arc::new(Efforted { levels }));
    {
        let _unset = crate::tests::TestEnvGuard::unset("ANGEL_WISH_EFFORT");
        assert_eq!(oracle.respond("a wish", GRANT_CALL), Ok("medium 90".to_string()));
    }
    {
        let _low = crate::tests::TestEnvGuard::set("ANGEL_WISH_EFFORT", "Low");
        assert_eq!(oracle.respond("a wish", GRANT_CALL), Ok("low 90".to_string()));
    }
    {
        // The club's own effort when the operator asks for it, and when the
        // route has no such level.
        let _inherit = crate::tests::TestEnvGuard::set("ANGEL_WISH_EFFORT", "inherit");
        assert_eq!(oracle.respond("a wish", GRANT_CALL), Ok("plain".to_string()));
        let _odd = crate::tests::TestEnvGuard::set("ANGEL_WISH_EFFORT", "ludicrous");
        assert_eq!(oracle.respond("a wish", GRANT_CALL), Ok("plain".to_string()));
    }
    // A club with no effort to choose is asked as it always was.
    let _unset = crate::tests::TestEnvGuard::unset("ANGEL_WISH_EFFORT");
    assert_eq!(crate::app::control::realm::club_oracle(Arc::new(Bound)).respond("a wish", GRANT_CALL), Ok("90".to_string()));
}
