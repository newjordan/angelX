//! The Claude seat profile: the first-party Anthropic seat is never sent the
//! two pages that order a line written before every tool call; every other
//! seat hears them as before.
use super::{OMITTED, omitted, omitting};
use crate::agent::club::{ChatMsg, ChatRole, HttpClub, ProviderContract, ToolCall, ToolDef};
use crate::agent::harness::book::{
    Route, continuity, d12467_sloptomizer as live, introduction, ledger, m_method, w_workflow,
};
use crate::tests::TestEnvGuard;
use serde_json::{Value, json};

fn page(route: Route, index: usize) -> &'static str {
    crate::agent::harness::book::primary(route.primary)
        .unwrap()
        .subs
        .iter()
        .find(|sub| sub.route == route)
        .unwrap()
        .pages[index]
}

fn left_out() -> Vec<&'static str> {
    OMITTED
        .iter()
        .map(|&(route, index)| page(route, index))
        .collect()
}

fn read_file() -> ToolDef {
    ToolDef {
        name: "read_file".into(),
        description: "read".into(),
        params: json!({"type": "object"}),
    }
}

fn call(id: &str, name: &str, args: Value) -> ChatMsg {
    ChatMsg::assistant_calls_with_reasoning(
        vec![ToolCall {
            id: id.into(),
            name: name.into(),
            args,
        }],
        None,
    )
}

/// The system prompt's types, then a compaction's handoff that introduces
/// `⠺⠙` again (its short pages inline) and `⠍⠑` (its signal).
fn types_and_handoff() -> Vec<ChatMsg> {
    let handoff = format!("{}{}\n⠺⠙⠍⠑", live::ATTENTION, live::HANDOFF.cells());
    assert!(continuity::is_handoff(&handoff));
    vec![
        ChatMsg::system("⠽⠃⠍⠁⠽⠑\n⠍⠓\n`/work/demo`"),
        ChatMsg::user("Fix the failing test."),
        ChatMsg::harness(handoff),
    ]
}

/// The pages' own stamps on a tool result's tail and on advice turns, then
/// the route on a turn of its own.
fn stamps_and_advice() -> Vec<ChatMsg> {
    vec![
        ChatMsg::system("You fix code."),
        ChatMsg::user("Fix the failing test."),
        call("a", "run_tests", json!({})),
        ChatMsg::tool("a", "tests: 0 passed, 1 failed\n⠍⠑⠙"),
        ChatMsg::harness("⠺⠙⠃"),
        ChatMsg::harness("⠺⠙"),
    ]
}

/// Ledger reads of a type, a route and a page.
fn ledger_reads() -> Vec<ChatMsg> {
    let workspace = std::path::Path::new("/nonexistent/claude-seat-profile");
    let read = |cells: &str| ledger::read(workspace, cells).unwrap();
    let ledger_call = |id: &str, cells: &str| {
        call(
            id,
            "read_file",
            json!({"path": format!("ledger://{cells}")}),
        )
    };
    vec![
        ChatMsg::system("⠽⠙"),
        ChatMsg::user("Fix the failing test."),
        ledger_call("a", "⠽⠑⠺⠙"),
        ChatMsg::tool("a", read("⠽⠑⠺⠙")),
        ledger_call("b", "⠺⠙⠃"),
        ChatMsg::tool("b", read("⠺⠙⠃")),
        ledger_call("c", "⠍⠑⠙"),
        ChatMsg::tool("c", read("⠍⠑⠙")),
    ]
}

fn every_path() -> [Vec<ChatMsg>; 3] {
    [types_and_handoff(), stamps_and_advice(), ledger_reads()]
}

/// Every string a request body carries, one per line.
fn texts(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(texts).collect::<Vec<_>>().join("\n"),
        Value::Object(map) => map.values().map(texts).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn anthropic_seat() -> HttpClub {
    HttpClub::new(
        "claude-sonnet-5-5",
        "http://127.0.0.1:9",
        "claude-sonnet-5-5",
        Some("sk-ant-test".to_string()),
    )
    .with_provider_contract(ProviderContract::Anthropic)
    .with_anthropic_messages(None)
    .sota_tuned()
}

fn deepseek_seat() -> HttpClub {
    HttpClub::new(
        "deepseek",
        "https://api.deepseek.com/v1",
        "deepseek-chat",
        Some("sk-test".to_string()),
    )
    .sota_tuned()
}

#[test]
fn the_left_out_pages_are_the_two_caveman_lines() {
    assert_eq!(
        OMITTED,
        &[(m_method::REPAIR_BATCH, 3), (w_workflow::RESPONSE, 1)]
    );
    assert!(page(m_method::REPAIR_BATCH, 3).starts_with(
        "Before every tool call, the first and the fiftieth, you MUST write one caveman line"
    ));
    assert_eq!(
        page(w_workflow::RESPONSE, 1),
        "Precede every tool call with its caveman line, not a prose preamble."
    );
    for text in left_out() {
        assert!(!text.contains('\n'), "a page is one line of a reading");
    }
}

#[test]
fn only_inside_the_profile_is_a_page_left_out() {
    let text = page(w_workflow::RESPONSE, 1);
    assert!(!omitted(w_workflow::RESPONSE, text));
    assert!(omitting(|| omitted(w_workflow::RESPONSE, text)));
    assert!(!omitting(|| omitted(
        w_workflow::RESPONSE,
        page(w_workflow::RESPONSE, 0)
    )));
    assert!(
        !omitting(|| omitted(m_method::REPAIR_BATCH, text)),
        "by route"
    );
    assert!(!omitted(w_workflow::RESPONSE, text), "restored after");
}

/// The introductions of the types and of a handoff's re-introduction are the
/// book's own with exactly the two pages gone; outside the profile they carry
/// both.
#[test]
fn the_type_and_handoff_introductions_drop_exactly_the_two_pages() {
    let messages = types_and_handoff();
    let plain = introduction::introductions(&messages, None);
    let profiled = omitting(|| introduction::introductions(&messages, None));
    assert_eq!(plain.len(), 2, "{plain:?}");
    assert_eq!(profiled.len(), 2, "{profiled:?}");
    for ((at, plain), (profiled_at, profiled)) in plain.iter().zip(&profiled) {
        assert_eq!(at, profiled_at);
        let mut expected = plain.clone();
        for text in left_out() {
            expected = expected.replace(&format!(" {text}"), "");
        }
        assert_eq!(profiled, &expected);
        assert!(!profiled.contains("caveman"), "{profiled}");
    }
    for text in left_out() {
        assert!(plain[0].1.contains(text), "{}", plain[0].1);
    }
    assert!(plain[1].1.contains(left_out()[1]), "{}", plain[1].1);
    assert!(
        profiled[0].1.contains(
            "\nAct on clear requests immediately. Report new findings, blockers, or changes of plan briefly."
        ),
        "{}",
        profiled[0].1
    );
    assert!(
        profiled[0]
            .1
            .contains("In Treebeard, keep strategy short and park bulk under handles.\n"),
        "{}",
        profiled[0].1
    );
    assert!(
        profiled[1]
            .1
            .contains("⠺⠙ how to respond: Act on clear requests immediately. Report new findings"),
        "{}",
        profiled[1].1
    );
    assert!(
        profiled[1]
            .1
            .contains("⠍⠑ coding repair: batching, evidence and narration\n")
            || profiled[1]
                .1
                .ends_with("⠍⠑ coding repair: batching, evidence and narration"),
        "the route stays its signal: {}",
        profiled[1].1
    );
}

/// A page's own stamp is never introduced under the profile; its route, met
/// later, is introduced with the rest of its pages.
#[test]
fn a_left_out_page_stamp_is_never_introduced() {
    let messages = stamps_and_advice();
    let plain = introduction::introductions(&messages, None);
    let profiled = omitting(|| introduction::introductions(&messages, None));
    let at = |intros: &[(usize, String)]| intros.iter().map(|(at, _)| *at).collect::<Vec<_>>();
    assert_eq!(at(&plain), vec![3, 4, 5], "{plain:?}");
    assert!(plain[0].1.ends_with(&format!("⠍⠑⠙ {}", left_out()[0])));
    assert_eq!(plain[1].1, format!("⠺⠙⠃ {}", left_out()[1]));
    assert!(plain[2].1.contains(left_out()[1]));
    assert_eq!(at(&profiled), vec![5], "{profiled:?}");
    assert_eq!(
        profiled[0].1,
        format!(
            "{}\n⠺⠙ how to respond: {} {} {}",
            introduction::LEAD,
            page(w_workflow::RESPONSE, 0),
            page(w_workflow::RESPONSE, 2),
            page(w_workflow::RESPONSE, 3)
        )
    );
}

/// The seat's request on every path carries neither page and nothing of a
/// caveman line; the pages around them keep their words and their addresses.
/// The history is untouched.
#[test]
fn the_anthropic_seat_is_never_sent_either_page() {
    let _guard = crate::tests::env_lock();
    let _env = [
        TestEnvGuard::unset("ANGEL_SEAT_PROFILE"),
        TestEnvGuard::unset("ANGEL_BOOK_INTRO"),
        TestEnvGuard::set("ANGEL_SOTA_CAVEMAN", "1"),
    ];
    let mut all = String::new();
    for messages in every_path() {
        let before = messages
            .iter()
            .map(|m| m.content.to_string())
            .collect::<Vec<_>>();
        let body = anthropic_seat()
            .build_body(&messages, &[read_file()], false)
            .unwrap();
        let text = texts(&body["messages"]);
        for page in left_out() {
            assert!(!text.contains(page), "{text}");
        }
        assert!(!text.contains("caveman"), "{text}");
        assert_eq!(
            messages
                .iter()
                .map(|m| m.content.to_string())
                .collect::<Vec<_>>(),
            before
        );
        all.push_str(&text);
    }
    for kept in [
        "Act on clear requests immediately. Report new findings",
        "⠺⠙⠁  Act on clear requests immediately.",
        "⠺⠙⠉  Report new findings, blockers, or changes of plan briefly.",
        "⠍⠑⠉  In Treebeard, keep strategy short and park bulk under handles.",
        "Reuse fresh evidence.",
    ] {
        assert!(all.contains(kept), "{kept}: {all}");
    }
}

/// Every other seat, and the Anthropic seat with the profile switched off,
/// still hears both pages: in the introductions and in every ledger read,
/// whose tool results go out byte for byte as the ledger rendered them.
#[test]
fn every_other_seat_still_hears_both_pages() {
    let _guard = crate::tests::env_lock();
    let _env = [
        TestEnvGuard::unset("ANGEL_SEAT_PROFILE"),
        TestEnvGuard::unset("ANGEL_BOOK_INTRO"),
        TestEnvGuard::set("ANGEL_SOTA_CAVEMAN", "1"),
    ];
    let check = |club: HttpClub| {
        for messages in every_path() {
            let body = club.build_body(&messages, &[read_file()], false).unwrap();
            let text = texts(&body["messages"]);
            for page in left_out() {
                assert!(text.contains(page), "{text}");
            }
        }
        let messages = ledger_reads();
        let body = club.build_body(&messages, &[read_file()], false).unwrap();
        let wire = body["messages"].as_array().unwrap();
        let reads = messages.iter().filter(|m| m.role == ChatRole::Tool);
        let sent = wire.iter().filter(|w| w["role"] == "tool");
        assert_eq!(reads.clone().count(), 3);
        for (message, wire) in reads.zip(sent) {
            assert_eq!(wire["content"].as_str(), Some(&*message.content));
        }
    };
    check(deepseek_seat());
    let _off = TestEnvGuard::set("ANGEL_SEAT_PROFILE", "0");
    check(anthropic_seat());
}
