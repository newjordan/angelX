use super::*;

fn fresh(prompt: &str) -> Arc<[ChatMsg]> {
    Arc::from(vec![
        ChatMsg::system("fresh system"),
        ChatMsg::user(prompt.to_string()),
        ChatMsg::harness("learned policy"),
    ])
}

fn finished() -> Vec<ChatMsg> {
    vec![
        ChatMsg::system("first system"),
        ChatMsg::user("iteration one"),
        ChatMsg::assistant("read the brief, profiled, cut 5% [evidence: test: bench]"),
    ]
}

#[test]
fn a_podrace_iteration_continues_the_conversation_it_finished() {
    let _guard = crate::tests::env_lock();
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY");
    let _limit = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY_TOKENS");
    keep("loop-a", true, finished());

    let next = next_conversation(Some("loop-a"), fresh("iteration two state"));
    assert_eq!(next.len(), 5, "three kept messages, then the update and its harness block");
    assert_eq!(&*next[0].content, "first system", "the first system prompt still leads");
    assert_eq!(next[3].role, ChatRole::User);
    assert!(next[3].content.starts_with(CONTINUES), "{}", next[3].content);
    assert!(next[3].content.ends_with("iteration two state"));
    assert_eq!(&*next[4].content, "learned policy");
    assert!(
        !next.iter().any(|msg| &*msg.content == "fresh system"),
        "a second system prompt would break the shared prefix"
    );

    // The kept conversation is used once; the finished turn hands back the longer one.
    let again = next_conversation(Some("loop-a"), fresh("iteration three state"));
    assert_eq!(again.len(), 3, "nothing kept: a fresh conversation");
    clear();
}

#[test]
fn nothing_is_carried_when_it_should_start_fresh() {
    let _guard = crate::tests::env_lock();
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY");
    let _limit = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY_TOKENS");

    keep("loop-a", false, finished());
    assert_eq!(
        next_conversation(Some("loop-a"), fresh("x")).len(),
        3,
        "a loop that is not a podrace keeps its fresh contexts unless asked"
    );

    keep("loop-a", true, finished());
    assert_eq!(
        next_conversation(Some("loop-b"), fresh("x")).len(),
        3,
        "another loop never inherits a conversation"
    );

    keep("loop-a", true, finished());
    assert_eq!(
        next_conversation(None, fresh("x")).len(),
        3,
        "an ordinary turn is never a continuation"
    );
    clear();

    let _small = crate::tests::TestEnvGuard::set("ANGEL_LOOP_CARRY_TOKENS", "5");
    keep("loop-a", true, finished());
    assert_eq!(
        next_conversation(Some("loop-a"), fresh("x")).len(),
        3,
        "a conversation past the limit is not carried"
    );
    clear();
}

#[test]
fn the_switch_overrides_the_default_both_ways() {
    let _guard = crate::tests::env_lock();
    let _limit = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY_TOKENS");
    {
        let _on = crate::tests::TestEnvGuard::set("ANGEL_LOOP_CARRY", "1");
        keep("loop-a", false, finished());
        assert_eq!(next_conversation(Some("loop-a"), fresh("x")).len(), 5);
    }
    {
        let _off = crate::tests::TestEnvGuard::set("ANGEL_LOOP_CARRY", "0");
        keep("loop-a", true, finished());
        assert_eq!(next_conversation(Some("loop-a"), fresh("x")).len(), 3);
    }
    clear();
}

#[test]
fn a_declined_stop_is_not_continued_and_carry_never_reads_reply_text() {
    let _guard = crate::tests::env_lock();
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY");
    let _limit = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY_TOKENS");
    // The refusal fact travels in the stop reason: the caller clears instead
    // of keeping, so the same conversation is never sent to be declined again.
    assert!(!continues_after(
        crate::agent::harness::TurnStopReason::Declined
    ));

    // An ordinary answer that merely mentions a refusal is still carried.
    let mut working = finished();
    working.push(ChatMsg::assistant(
        "The reviewer declined the request to widen the cache, so I kept it at 64 and re-ran the benchmark: p99 is 180 ms, down from 240 ms, and the regression test passes. Next I will profile the allocator, since the flame graph now shows it at 31% of the hot path, and compare the pooled variant against the arena variant on the same trace before touching the eviction code again in the next iteration of this work.",
    ));
    keep("loop-a", true, working);
    assert_eq!(next_conversation(Some("loop-a"), fresh("x")).len(), 6);
    clear();
}

#[test]
fn only_a_stop_that_leaves_the_conversation_whole_is_continued() {
    use crate::agent::harness::TurnStopReason as Stop;
    for reason in [Stop::Answer, Stop::Deadline, Stop::IdleTimeout, Stop::Interrupt] {
        assert!(continues_after(reason), "{}", reason.as_str());
    }
    for reason in [
        Stop::MaxHops,
        Stop::ExecutionBlocked,
        Stop::CaptureFailure,
        Stop::CheckpointFailure,
        Stop::ProviderError,
        Stop::Declined,
        Stop::NeedsPro,
    ] {
        assert!(!continues_after(reason), "{}", reason.as_str());
    }
}

#[test]
fn a_turn_stopped_mid_work_is_continued_from_its_last_tool_result() {
    let _guard = crate::tests::env_lock();
    let _mode = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY");
    let _limit = crate::tests::TestEnvGuard::unset("ANGEL_LOOP_CARRY_TOKENS");
    // What a turn stopped by its time budget hands back: no closing answer.
    let mut stopped = finished();
    stopped.push(ChatMsg::user("iteration two state"));
    stopped.push(ChatMsg::harness("the profile run is half done"));
    keep("loop-a", true, stopped);

    let next = next_conversation(Some("loop-a"), fresh("iteration three state"));
    assert_eq!(next.len(), 7, "five kept messages, then the update and its harness block");
    assert!(next[5].content.starts_with(CONTINUES), "{}", next[5].content);
    assert!(next[5].content.ends_with("iteration three state"));
    clear();
}

