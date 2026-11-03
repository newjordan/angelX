//! Prompt-cache prefix stability across hops, per wire and dialect.
//!
//! Every hop re-renders the whole conversation. A provider's prefix cache only
//! hits when that re-render is byte-identical up to the previously cached
//! boundary, so each route is grown through a reasoned tool turn and a tool
//! result and every request's serialized messages must prefix the next one's.

use super::deepseek_messages::{Flavor, to_messages_body_for};
use super::*;
use serde_json::{Value, json};

/// Knobs that shape outbound messages (splices, effort, dialect, cache
/// markers). Tests scrub them while holding the crate env lock so a hop pair
/// cannot be built under two different configurations.
const PREFIX_ENV: &[&str] = &[
    "ANGEL_REASONING_EFFORT",
    "ANGEL_DEEPSEEK_REASONING_EFFORT",
    "ANGEL_ANTHROPIC_EFFORT",
    "ANGEL_PREFIX_OPENAI_REASONING_EFFORT",
    "ANGEL_PREFIX_GROK_REASONING_EFFORT",
    "ANGEL_PREFIX_DEEPSEEK_REASONING_EFFORT",
    "ANGEL_PREFIX_DEEPSEEK_FORWARDER_REASONING_EFFORT",
    "ANGEL_PREFIX_QWEN_REASONING_EFFORT",
    "ANGEL_PREFIX_GLM_REASONING_EFFORT",
    "ANGEL_ANTHROPIC_REASONING_EFFORT",
    "ANGEL_OPENROUTER_REASONING_EFFORT",
    "ANGEL_PREFIX_OPENAI_REASONING_DIALECT",
    "ANGEL_PREFIX_GROK_REASONING_DIALECT",
    "ANGEL_PREFIX_DEEPSEEK_REASONING_DIALECT",
    "ANGEL_PREFIX_DEEPSEEK_FORWARDER_REASONING_DIALECT",
    "ANGEL_PREFIX_QWEN_REASONING_DIALECT",
    "ANGEL_PREFIX_GLM_REASONING_DIALECT",
    "ANGEL_ANTHROPIC_REASONING_DIALECT",
    "ANGEL_OPENROUTER_REASONING_DIALECT",
    "ANGEL_SOTA_CAVEMAN",
    "ANGEL_ECON",
    "ANGEL_BOOK_INTRO",
    "ANGEL_OPT_DUP",
    "ANGEL_OPENROUTER_ANTHROPIC_CACHE",
    "ANGEL_ANTHROPIC_CACHE_PREFIX_FLOOR",
    "ANGEL_ANTHROPIC_CACHE_TTL",
];

struct PrefixEnv {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl PrefixEnv {
    fn scrub(keys: &[&'static str]) -> Self {
        let mut previous = Vec::with_capacity(keys.len());
        for &key in keys {
            previous.push((key, std::env::var_os(key)));
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(key) };
        }
        resync_prefix_caches();
        Self { previous }
    }
}

impl Drop for PrefixEnv {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                // TODO: Audit that the environment access only happens in single-threaded code.
                Some(value) => unsafe { std::env::set_var(key, value) },
                // TODO: Audit that the environment access only happens in single-threaded code.
                None => unsafe { std::env::remove_var(key) },
            }
        }
        resync_prefix_caches();
    }
}

fn resync_prefix_caches() {
    resync_reasoning_effort_env_from_env();
    resync_prompt_cache_from_env();
    resync_openrouter_anthropic_cache_from_env();
}

fn system_prompt() -> String {
    "You are a careful coding agent. Keep the committed history byte-stable across hops. "
        .repeat(80)
}

fn tools() -> Vec<ToolDef> {
    vec![ToolDef {
        name: "read_file".into(),
        description: "Read a file".into(),
        params: json!({
            "type": "object",
            "properties": {"path": {"type": "string"}},
            "required": ["path"]
        }),
    }]
}

fn read_call(id: &str, path: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: "read_file".into(),
        args: json!({"path": path}),
    }
}

/// The conversation every route grows through: a reasoned tool call, its
/// result, a reasoned final answer, a new ask, and a second reasoned tool call
/// whose result is the newest message. Each hop is a request the agent loop
/// actually sends.
fn growing_hops(reasoning: &[&str; 3]) -> Vec<Vec<ChatMsg>> {
    let hop1 = vec![
        ChatMsg::system(system_prompt()),
        ChatMsg::user("inspect src/main.rs"),
    ];
    let mut hop2 = hop1.clone();
    hop2.push(ChatMsg::assistant_calls_with_reasoning(
        vec![read_call("call_1", "src/main.rs")],
        Some(reasoning[0].to_string()),
    ));
    hop2.push(ChatMsg::tool("call_1", "fn main() {}"));
    let mut hop3 = hop2.clone();
    hop3.push(ChatMsg::assistant_with_reasoning(
        "The entry point is trivial.",
        Some(reasoning[1].to_string()),
    ));
    hop3.push(ChatMsg::user("now read the tests"));
    let mut hop4 = hop3.clone();
    hop4.push(ChatMsg::assistant_calls_with_reasoning(
        vec![read_call("call_2", "tests/cli.rs")],
        Some(reasoning[2].to_string()),
    ));
    hop4.push(ChatMsg::tool("call_2", "tests: 3 passed"));
    vec![hop1, hop2, hop3, hop4]
}

const PLAIN_REASONING: [&str; 3] = [
    "read the entry point before editing",
    "the entry point is trivial",
    "read the tests before claiming a pass",
];

const SIGNED_REASONING: [&str; 3] = [
    r#"[{"type":"thinking","thinking":"read the entry point before editing","signature":"sig-call-1"}]"#,
    r#"[{"type":"thinking","thinking":"the entry point is trivial","signature":"sig-answer"}]"#,
    r#"[{"type":"thinking","thinking":"read the tests before claiming a pass","signature":"sig-call-2"}]"#,
];

fn chat_hops(club: &HttpClub, reasoning: &[&str; 3]) -> Vec<Value> {
    let tools = tools();
    growing_hops(reasoning)
        .iter()
        .map(|history| {
            club.build_body_with_effort(history, &tools, false, Some("high"))
                .expect("chat body")
                .get("messages")
                .cloned()
                .expect("chat body carries messages")
        })
        .collect()
}

fn messages_hops(club: &HttpClub, flavor: Flavor, reasoning: &[&str; 3]) -> Vec<Value> {
    let tools = tools();
    growing_hops(reasoning)
        .iter()
        .map(|history| {
            let chat = club
                .build_body_with_effort(history, &tools, false, Some("high"))
                .expect("chat body");
            to_messages_body_for(&chat, flavor)
                .expect("messages body")
                .get("messages")
                .cloned()
                .expect("messages body carries messages")
        })
        .collect()
}

/// Request N's serialized messages must be a literal prefix of request N+1's,
/// and the next byte must continue the array with one more message: a string
/// prefix without the comma would also hold if an element were truncated or
/// rewritten at the boundary.
fn assert_serialized_prefix(route: &str, previous: &Value, next: &Value) {
    let previous_items = previous
        .as_array()
        .unwrap_or_else(|| panic!("{route}: previous messages must be a JSON array: {previous}"));
    let next_items = next
        .as_array()
        .unwrap_or_else(|| panic!("{route}: next messages must be a JSON array: {next}"));
    assert!(
        !previous_items.is_empty(),
        "{route}: a growing conversation always carries messages"
    );
    assert!(
        next_items.len() > previous_items.len(),
        "{route}: request N+1 must append a message ({} -> {})",
        previous_items.len(),
        next_items.len()
    );
    let previous = serde_json::to_string(previous).expect("serialize previous messages");
    let next = serde_json::to_string(next).expect("serialize next messages");
    let stable = previous
        .strip_suffix(']')
        .unwrap_or_else(|| panic!("{route}: messages must serialize as a JSON array: {previous}"));
    assert!(
        next.starts_with(stable),
        "{route}: request N's messages are not a byte-identical prefix of request N+1's\n\
         previous: {previous}\nnext: {next}"
    );
    assert_eq!(
        next.as_bytes().get(stable.len()),
        Some(&b','),
        "{route}: request N's messages are only a string prefix, not a whole-array prefix\n\
         previous: {previous}\nnext: {next}"
    );
}

fn assert_hops_are_prefixes(route: &str, hops: &[Value]) {
    assert!(hops.len() > 1, "{route}: need at least two requests");
    for wave in hops.windows(2) {
        assert_serialized_prefix(route, &wave[0], &wave[1]);
    }
}

/// A prefix check alone passes when a route silently drops the newest message:
/// every new turn must actually appear in the request that carries it.
fn assert_turn_present(route: &str, hop: &Value, needle: &str) {
    assert!(
        hop.to_string().contains(needle),
        "{route}: `{needle}` is missing from a request that should carry it: {hop}"
    );
}

fn assert_grown_turns_present(route: &str, hops: &[Value]) {
    assert_eq!(
        hops.len(),
        4,
        "{route}: the fixture grows through four hops"
    );
    assert_turn_present(route, &hops[0], "inspect src/main.rs");
    assert_turn_present(route, &hops[1], "call_1");
    assert_turn_present(route, &hops[1], "fn main() {}");
    assert_turn_present(route, &hops[2], "The entry point is trivial.");
    assert_turn_present(route, &hops[2], "now read the tests");
    assert_turn_present(route, &hops[3], "call_2");
    assert_turn_present(route, &hops[3], "tests: 3 passed");
}

fn assert_route_prefixes(route: &str, hops: &[Value]) {
    assert_grown_turns_present(route, hops);
    assert_hops_are_prefixes(route, hops);
}

/// A replaying route must keep the reasoning it already sent in the prefix;
/// dropping it would re-prefill the provider's cached tokens even though every
/// remaining message stayed in order.
fn assert_reasoning_replays(route: &str, hops: &[Value], reasoning: &[&str; 3]) {
    let latest = hops.last().expect("a hop").to_string();
    for thought in reasoning {
        assert!(
            latest.contains(thought),
            "{route}: the newest request dropped earlier reasoning `{thought}`"
        );
    }
}

/// The cache-control markers move by design; strip them to check the
/// token-bearing content prefix that the provider actually caches.
fn without_cache_control(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| key.as_str() != "cache_control")
                .map(|(key, value)| (key.clone(), without_cache_control(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_cache_control).collect()),
        other => other.clone(),
    }
}

fn assert_content_hops_are_prefixes(route: &str, hops: &[Value]) {
    let stripped = hops.iter().map(without_cache_control).collect::<Vec<_>>();
    assert_hops_are_prefixes(route, &stripped);
}

fn assert_content_route_prefixes(route: &str, hops: &[Value]) {
    assert_grown_turns_present(route, hops);
    assert_content_hops_are_prefixes(route, hops);
}

/// A route selection is part of the fixture: if URL/model inference stops
/// resolving to the dialect under test, the test must fail rather than quietly
/// exercise the generic OpenAI spelling.
fn assert_dialect(club: &HttpClub, expected: ReasoningDialect, route: &str) {
    assert_eq!(
        club.reasoning_dialect(),
        expected,
        "{route} must resolve its own reasoning dialect"
    );
}

fn openai_club() -> HttpClub {
    HttpClub::new(
        "prefix-openai",
        "https://api.openai.com/v1",
        "gpt-6.1-sol",
        None,
    )
}

fn deepseek_club() -> HttpClub {
    HttpClub::new(
        "prefix-deepseek",
        "https://api.deepseek.com/v1",
        "deepseek-v4-pro",
        None,
    )
}

fn anthropic_club() -> HttpClub {
    HttpClub::new(
        "prefix-anthropic",
        "https://api.anthropic.com",
        "claude-sonnet-5-5",
        Some("test-key".into()),
    )
    .with_env_namespace("ANTHROPIC")
    .with_provider_contract(ProviderContract::Anthropic)
    .with_anthropic_messages(None)
}

fn openrouter_claude_club() -> HttpClub {
    HttpClub::new(
        "anthropic/claude-sonnet-4.6",
        "https://openrouter.ai/api/v1",
        "anthropic/claude-sonnet-4.6",
        Some("test-key".into()),
    )
}

/// The OpenRouter Claude route is selected by the vendor-prefixed model: its
/// body must carry the Anthropic cache breakpoints, not the generic OpenAI
/// messages array.
fn assert_openrouter_claude_route(club: &HttpClub, hops: &[Value]) {
    assert_dialect(club, ReasoningDialect::OpenAiEffort, "openrouter claude");
    assert!(
        hops.iter()
            .all(|hop| hop.to_string().contains("cache_control")),
        "the OpenRouter Claude route must keep its Anthropic cache breakpoints: {hops:?}"
    );
}

#[test]
fn openai_chat_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let hops = chat_hops(&openai_club(), &PLAIN_REASONING);
    assert_route_prefixes("openai chat", &hops);
}

#[test]
fn grok_chat_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = HttpClub::new(
        "prefix-grok",
        "https://api.x.ai/v1",
        "grok-4.7",
        Some("test-key".into()),
    );
    assert_dialect(&club, ReasoningDialect::OpenAiEffort, "grok chat");
    let hops = chat_hops(&club, &PLAIN_REASONING);
    assert_route_prefixes("grok chat", &hops);
}

#[test]
fn deepseek_chat_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let hops = chat_hops(&deepseek_club(), &PLAIN_REASONING);
    assert_route_prefixes("deepseek chat", &hops);
    assert_reasoning_replays("deepseek chat", &hops, &PLAIN_REASONING);
}

#[test]
fn deepseek_forwarder_chat_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = HttpClub::new(
        "prefix-deepseek-forwarder",
        "https://gateway.example.com/v1",
        "deepseek-v4-pro",
        Some("test-key".into()),
    )
    .with_provider_contract(ProviderContract::DeepSeek);
    let hops = chat_hops(&club, &PLAIN_REASONING);
    assert_route_prefixes("deepseek forwarder chat", &hops);
    assert_reasoning_replays("deepseek forwarder chat", &hops, &PLAIN_REASONING);
}

#[test]
fn qwen_local_chat_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = HttpClub::new(
        "prefix-qwen",
        "http://127.0.0.1:18888/v1",
        "qwen3.6-27b",
        None,
    );
    let hops = chat_hops(&club, &PLAIN_REASONING);
    assert_dialect(
        &club,
        ReasoningDialect::QwenEnableThinking,
        "qwen local chat",
    );
    assert_route_prefixes("qwen local chat", &hops);
    assert_reasoning_replays("qwen local chat", &hops, &PLAIN_REASONING);
}

#[test]
fn glm_chat_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = HttpClub::new(
        "prefix-glm",
        "https://api.z.ai/api/coding/paas/v4",
        "glm-5.3",
        Some("test-key".into()),
    );
    let hops = chat_hops(&club, &PLAIN_REASONING);
    assert_dialect(&club, ReasoningDialect::GlmThinking, "glm chat");
    assert_route_prefixes("glm chat", &hops);
}

#[test]
fn deepseek_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = deepseek_club().with_deepseek_messages();
    let hops = messages_hops(&club, Flavor::DeepSeek, &PLAIN_REASONING);
    assert_route_prefixes("deepseek messages", &hops);
    assert_reasoning_replays("deepseek messages", &hops, &PLAIN_REASONING);
}

#[test]
#[ignore = "prefix break: finish_body moves the last-message cache_control breakpoint to each hop's newest last block"]
fn anthropic_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let hops = messages_hops(&anthropic_club(), Flavor::Anthropic, &SIGNED_REASONING);
    assert_route_prefixes("anthropic messages", &hops);
}

#[test]
fn anthropic_messages_content_stays_a_byte_prefix_without_cache_markers() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let hops = messages_hops(&anthropic_club(), Flavor::Anthropic, &SIGNED_REASONING);
    assert_content_route_prefixes("anthropic messages", &hops);
}

#[test]
#[ignore = "prefix break: the OpenRouter Claude tail cache_control breakpoint moves to each hop's newest last text message"]
fn openrouter_claude_messages_stay_a_byte_prefix_as_the_turn_grows() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = openrouter_claude_club();
    let hops = chat_hops(&club, &PLAIN_REASONING);
    assert_openrouter_claude_route(&club, &hops);
    assert_route_prefixes("openrouter claude chat", &hops);
}

#[test]
#[ignore = "prefix break: the moving tail marker rewrites each hop's newest text message into a content-part array, then it reverts to a plain string, so marker-free bytes move too"]
fn openrouter_claude_content_stays_a_byte_prefix_without_cache_markers() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = openrouter_claude_club();
    let hops = chat_hops(&club, &PLAIN_REASONING);
    assert_openrouter_claude_route(&club, &hops);
    assert_content_route_prefixes("openrouter claude chat", &hops);
}

/// The Claude vendor slug must keep selecting the OpenRouter Anthropic cache
/// path; otherwise the ignored strict tests would quietly exercise the generic
/// OpenAI body and report a false prefix break.
#[test]
fn openrouter_claude_route_keeps_its_anthropic_cache_wire() {
    let _lock = crate::tests::env_lock();
    let _env = PrefixEnv::scrub(PREFIX_ENV);
    let club = openrouter_claude_club();
    let hops = chat_hops(&club, &PLAIN_REASONING);
    assert_openrouter_claude_route(&club, &hops);
}
