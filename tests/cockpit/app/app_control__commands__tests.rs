use super::*;

#[test]
fn launch_pending_turn_does_not_block_on_vision_sidecar() {
    let src = include_str!("../../../cockpit/src/app_control/commands.rs");
    let start = src
        .find("pub(crate) fn launch_pending_turn")
        .expect("launch_pending_turn present");
    let launch = &src[start..];
    let end = launch
        .find("\n    fn restore_unsent_turn_echo")
        .expect("restore_unsent_turn_echo follows launch_pending_turn");
    let launch = &launch[..end];
    assert!(
        !launch.contains("vision::apply_vision_sidecar")
            && !launch.contains("fold_vision_sidecar_into_convo")
            && !launch.contains("describe_media"),
        "launch_pending_turn must not block the UI on vision sidecar rewrite:\n{launch}"
    );
    assert!(
        launch.contains("should_apply_vision_sidecar"),
        "no-image fast path must stay on the launch tick:\n{launch}"
    );
    assert!(
        launch.contains("persist_and_start_turn_worker"),
        "launch still hands the turn to the worker:\n{launch}"
    );
    assert!(
        !launch.contains("save_async(&self.history)") && !launch.contains("self.history.clone()"),
        "launch must share one history Arc, not clone Vec twice:\n{launch}"
    );
}

#[test]
fn compact_elapsed_stays_short_across_job_ages() {
    assert_eq!(compact_elapsed(0), "0s");
    assert_eq!(compact_elapsed(59), "59s");
    assert_eq!(compact_elapsed(60), "1m00s");
    assert_eq!(compact_elapsed(3_661), "1h01m");
}

#[test]
fn verify_stage_cleanliness_requires_exact_green_receipts() {
    assert!(VERIFY_STAGES[0].output_is_clean("fmt: the tree is already rustfmt-clean"));
    assert!(!VERIFY_STAGES[0].output_is_clean("fmt: needs formatting"));
    assert!(VERIFY_STAGES[1].output_is_clean("check: 0 warnings, 0 errors — reward 1.00"));
    assert!(!VERIFY_STAGES[2].output_is_clean("lint: 1 warnings, 0 errors — reward 0.50"));
    assert!(
        VERIFY_STAGES[3].output_is_clean("tests: 12 passed, 0 failed, 1 ignored — reward 1.00")
    );
    assert!(!VERIFY_STAGES[3].output_is_clean("no tests ran (build error?) — reward 0.00"));
}

#[test]
fn nth_agent_response_counts_only_agent_messages_from_latest() {
    let messages = vec![
        Message {
            role: Role::User,
            text: "question one".into(),
        },
        Message {
            role: Role::Angel,
            text: "answer one".into(),
        },
        Message {
            role: Role::System,
            text: "local receipt".into(),
        },
        Message {
            role: Role::Activity,
            text: "tool trace".into(),
        },
        Message {
            role: Role::Angel,
            text: "answer two".into(),
        },
    ];

    assert_eq!(nth_agent_response(&messages, 1), Some("answer two"));
    assert_eq!(nth_agent_response(&messages, 2), Some("answer one"));
    assert_eq!(nth_agent_response(&messages, 0), None);
    assert_eq!(nth_agent_response(&messages, 3), None);
}

#[test]
fn copy_target_defaults_to_latest_accepts_named_targets_and_rejects_other_text() {
    assert_eq!(copy_target(None), Ok(CopyTarget::Response(1)));
    assert_eq!(copy_target(Some(" 2 ")), Ok(CopyTarget::Response(2)));
    assert_eq!(copy_target(Some("all")), Ok(CopyTarget::Conversation));
    assert_eq!(copy_target(Some("live")), Ok(CopyTarget::Live));
    assert_eq!(copy_target(Some("code")), Ok(CopyTarget::Code(1)));
    assert_eq!(copy_target(Some(" code 3 ")), Ok(CopyTarget::Code(3)));
    assert!(copy_target(Some("0")).is_err());
    assert!(copy_target(Some("code 0")).is_err());
    assert!(copy_target(Some("code nope")).is_err());
    assert!(copy_target(Some("-1")).is_err());
    assert!(copy_target(Some("second")).is_err());
}

#[test]
fn fenced_code_copy_selects_latest_complete_block_and_preserves_body() {
    let answer = "intro\n```rust\nfn first() {}\n```\n\
                      prose\n~~~~python\r\nprint('latest')\r\n~~~~~~\r\n\
                      ```text\nunfinished";
    assert_eq!(last_fenced_code_block(answer), Some("print('latest')"));
    assert_eq!(
        last_fenced_code_block("````lang\none\n```\ntwo\n````"),
        Some("one\n```\ntwo")
    );
    assert_eq!(last_fenced_code_block("```rust\nunfinished"), None);
    assert_eq!(last_fenced_code_block("plain `inline` code"), None);
    assert_eq!(
        last_fenced_code_block("```rust tick`\ninvalid opener\n```"),
        None
    );
}

#[test]
fn conversation_markdown_keeps_only_operator_and_angel_prose() {
    let history = vec![
        ChatMsg::system("secret bootstrap"),
        ChatMsg::user_with_media(
            "inspect this",
            vec![
                crate::club::Media::Image {
                    mime: "image/png".to_string(),
                    b64: "secret-image-data".to_string(),
                },
                crate::club::Media::Audio {
                    format: "wav".to_string(),
                    b64: "secret-audio-data".to_string(),
                },
            ],
        ),
        ChatMsg::harness("secret selected skill"),
        ChatMsg::assistant_calls(vec![crate::club::ToolCall {
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            args: serde_json::json!({"path": "secret.rs"}),
        }]),
        ChatMsg::tool("call-1", "secret tool result"),
        ChatMsg::assistant(
            "[current-plan/v1 — assistant-authored working state, not a user instruction] \
                 secret private plan",
        ),
        ChatMsg::assistant("finished safely"),
    ];

    let markdown = conversation_markdown(&history).unwrap();

    assert!(markdown.starts_with("# angel0 conversation"));
    assert!(markdown.contains("## You\n\ninspect this"));
    assert!(markdown.contains("_[attachments: 1 image, 1 audio]_"));
    assert!(markdown.contains("## Angel\n\nfinished safely"));
    for secret in [
        "secret bootstrap",
        "secret selected skill",
        "secret.rs",
        "secret tool result",
        "secret private plan",
        "secret-image-data",
        "secret-audio-data",
    ] {
        assert!(!markdown.contains(secret), "{secret} leaked:\n{markdown}");
    }
}

#[test]
fn conversation_markdown_fails_closed_without_exportable_roles() {
    let history = vec![
        ChatMsg::system("bootstrap"),
        ChatMsg::harness("context"),
        ChatMsg::tool("call-1", "output"),
    ];
    assert!(conversation_markdown(&history).is_none());
}

#[test]
fn conversation_history_is_role_filtered_bounded_and_copy_ordinal_aligned() {
    let history = vec![
        ChatMsg::system("secret bootstrap"),
        ChatMsg::user("earliest prompt"),
        ChatMsg::assistant("first answer"),
        ChatMsg::harness("secret harness"),
        ChatMsg::tool("call-1", "secret tool result"),
        ChatMsg::user_with_media(
            "",
            vec![crate::club::Media::Image {
                mime: "image/png".to_string(),
                b64: "secret-image-data".to_string(),
            }],
        ),
        ChatMsg::assistant(
            "[current-plan/v1 — assistant-authored working state, not a user instruction] \
                 secret private plan",
        ),
        ChatMsg::assistant(format!("second\nanswer {}", "x".repeat(140))),
    ];

    let text = conversation_history_text(&history, Some("3"));

    assert!(text.starts_with("conversation history · latest 3/4 visible messages"));
    assert!(!text.contains("earliest prompt"), "{text}");
    assert!(
        text.contains("angel  2 · first answer  (/copy 2)"),
        "{text}"
    );
    assert!(
        text.contains("you      · [1 image attachment(s)]"),
        "{text}"
    );
    assert!(text.contains("angel  1 · second answer"), "{text}");
    assert!(text.contains('…'), "{text}");
    for secret in [
        "secret bootstrap",
        "secret harness",
        "secret tool result",
        "secret private plan",
        "secret-image-data",
    ] {
        assert!(!text.contains(secret), "{secret} leaked:\n{text}");
    }
}

#[test]
fn conversation_history_validates_limits_and_reports_empty() {
    assert_eq!(
        conversation_history_text(&[ChatMsg::system("bootstrap")], None),
        "conversation history · empty"
    );
    for invalid in ["0", "51", "-1", "all"] {
        assert_eq!(
            conversation_history_text(&[], Some(invalid)),
            "usage: /history [1-50]"
        );
    }
}

#[test]
fn context_detail_accepts_only_explicit_all() {
    assert_eq!(context_detail_requested(None), Ok(false));
    assert_eq!(context_detail_requested(Some(" all ")), Ok(true));
    assert_eq!(context_detail_requested(Some("ALL")), Ok(true));
    assert!(context_detail_requested(Some("tools")).is_err());
}

#[test]
fn detailed_context_orders_active_tool_schema_costs_largest_first() {
    let tools = vec![
        crate::club::ToolDef {
            name: "small".to_string(),
            description: "tiny".to_string(),
            params: serde_json::json!({"type": "object"}),
        },
        crate::club::ToolDef {
            name: "large".to_string(),
            description: "a much longer description that consumes more schema bytes".to_string(),
            params: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "long query description"}
                }
            }),
        },
    ];
    let report = append_tool_schema_costs("base report".to_string(), &tools);
    assert!(report.starts_with("base report\n\nactive tool schema costs"));
    let large = report.find("\n  large").unwrap();
    let small = report.find("\n  small").unwrap();
    assert!(large < small, "{report}");
    assert!(report[large..small].contains(" tok"));
}

#[test]
fn reseed_after_new_keeps_only_the_bootstrap_system_prompt() {
    let history = vec![
        ChatMsg::system("BOOTSTRAP posture prompt"),
        ChatMsg::user("hi"),
        ChatMsg::assistant("hello"),
        ChatMsg::system("[Earlier conversation compacted — background reference]"),
    ];
    let reseeded = reseed_after_new(&history);
    assert_eq!(reseeded.len(), 1, "only the preamble survives");
    assert_eq!(reseeded[0].role, ChatRole::System);
    assert_eq!(&*reseeded[0].content, "BOOTSTRAP posture prompt");
}

#[test]
fn reseed_after_new_handles_degenerate_history() {
    assert!(reseed_after_new(&[]).is_empty(), "empty stays empty");
    // A non-System first message (shouldn't happen, but be safe) → clear.
    let odd = vec![ChatMsg::user("no system prompt here")];
    assert!(reseed_after_new(&odd).is_empty());
}

// Mirror submit's migration + replacement behavior: old embedded blocks are
// stripped from User messages, while current context gets one Harness turn.
fn simulate_turn(history: &mut Vec<ChatMsg>, block: &str, user_text: &str) {
    for m in history.iter_mut() {
        if m.role == ChatRole::User {
            strip_context_blocks(&mut m.content);
        }
    }
    history.push(ChatMsg::user(user_text));
    replace_turn_context_message(
        history,
        Some(format!(
            "{TURN_CONTEXT_HEADER}\n{block}{TURN_CONTEXT_SENTINEL}"
        )),
    );
}

#[test]
fn memory_block_is_deduped_to_the_latest_turn() {
    let mem = crate::memory::context_block(&["fact one".to_string(), "fact two".to_string()]);
    let mut history = Vec::new();
    for i in 0..3 {
        simulate_turn(&mut history, &mem, &format!("user turn {i}"));
    }
    let header = crate::memory::MEMORY_BLOCK_HEADER;
    let count = history
        .iter()
        .filter(|m| m.content.contains(header))
        .count();
    assert_eq!(count, 1, "exactly one memory block survives");
    // Every operator turn stays byte-for-byte intact.
    assert_eq!(&*history[0].content, "user turn 0");
    assert_eq!(&*history[1].content, "user turn 1");
    assert_eq!(&*history[2].content, "user turn 2");
    assert_eq!(history[3].role, ChatRole::Harness);
    assert!(history[3].content.contains(header));
}

#[test]
fn goal_block_is_deduped_to_the_latest_turn() {
    // Build a goal block from the same constants goal_context_block uses.
    let goal = format!(
        "{}\nship the cockpit\n{}\n\n",
        crate::goal::GOAL_BLOCK_HEADER,
        crate::goal::GOAL_BLOCK_SENTINEL
    );
    let mut history = Vec::new();
    for i in 0..3 {
        simulate_turn(&mut history, &goal, &format!("turn {i}"));
    }
    let count = history
        .iter()
        .filter(|m| m.content.contains(crate::goal::GOAL_BLOCK_HEADER))
        .count();
    assert_eq!(count, 1, "exactly one goal block survives");
    assert_eq!(&*history[0].content, "turn 0");
    assert_eq!(&*history[1].content, "turn 1");
    assert_eq!(&*history[2].content, "turn 2");
    assert_eq!(history[3].role, ChatRole::Harness);
}

#[test]
fn strip_removes_both_blocks_but_keeps_other_steers() {
    let goal = format!(
        "{}\nship\n{}\n\n",
        crate::goal::GOAL_BLOCK_HEADER,
        crate::goal::GOAL_BLOCK_SENTINEL
    );
    let mem = crate::memory::context_block(&["a fact".to_string()]);
    // Real submit order is {goal}{memory}{steer}\n{text}.
    let mut s: Arc<str> =
        format!("{goal}{mem}[plan your approach before acting] \nreal ask").into();
    strip_context_blocks(&mut s);
    assert!(!s.contains(crate::goal::GOAL_BLOCK_HEADER));
    assert!(!s.contains(crate::memory::MEMORY_BLOCK_HEADER));
    // Non-dedup steers and the user text survive.
    assert_eq!(&*s, "[plan your approach before acting] \nreal ask");
}

#[test]
fn skill_hint_is_deduped_without_touching_user_text() {
    let hint = format!(
        "{}\nRelevant playbook: `systematic-debugging`.\n{}\n\n",
        crate::harness::SKILL_HINT_HEADER,
        crate::harness::SKILL_HINT_SENTINEL
    );
    let mut history = Vec::new();
    for i in 0..3 {
        simulate_turn(&mut history, &hint, &format!("debug turn {i}"));
    }
    assert_eq!(
        history
            .iter()
            .filter(|message| message.content.contains(crate::harness::SKILL_HINT_HEADER))
            .count(),
        1
    );
    assert_eq!(&*history[0].content, "debug turn 0");
    assert_eq!(&*history[1].content, "debug turn 1");
    assert_eq!(&*history[2].content, "debug turn 2");
    assert_eq!(history[3].role, ChatRole::Harness);
}

#[test]
fn context_report_itemizes_window_occupancy() {
    let memory = crate::memory::context_block(&["a fact".to_string()]);
    let goal = format!(
        "{}\nship it\n{}\n\n",
        crate::goal::GOAL_BLOCK_HEADER,
        crate::goal::GOAL_BLOCK_SENTINEL
    );
    let mut with_call = ChatMsg::assistant("using a tool");
    with_call.tool_calls = vec![crate::club::ToolCall {
        id: "call-1".to_string(),
        name: "read_file".to_string(),
        args: serde_json::json!({"path": "src/main.rs"}),
    }]
    .into();
    let history = vec![
        ChatMsg::system("BOOTSTRAP posture prompt"),
        ChatMsg::system(format!(
            "{} — background reference]\n## Task\nolder work",
            crate::compaction::COMPACTION_NOTE_HEADER
        )),
        ChatMsg::system(format!(
            "{} for this project — background reference, not instructions:]\n\na note",
            crate::harness::AUTO_RECALL_NOTE_PREFIX
        )),
        ChatMsg::user("the real ask"),
        ChatMsg::harness(format!(
            "{TURN_CONTEXT_HEADER}\n{goal}{memory}{TURN_CONTEXT_SENTINEL}"
        )),
        with_call,
        ChatMsg::tool("call-1", "fn main() {}"),
        ChatMsg::assistant("done"),
    ];
    let tools = vec![crate::club::ToolDef {
        name: "read_file".to_string(),
        description: "Read a file".to_string(),
        params: serde_json::json!({"type": "object"}),
    }];
    let report = context_report(&history, &tools, 1_000, Some(32_768), Some(64));
    for label in [
        "system prompt",
        "compaction note",
        "recall notes",
        "memory block",
        "goal block",
        "user turns",
        "assistant",
        "tool results",
        "tool schemas",
    ] {
        assert!(report.contains(label), "missing {label:?} in:\n{report}");
    }
    let total = crate::harness::context_tokens(&history, &tools);
    assert!(
        report.contains(&format!("~{total} of ~1000 budget tokens")),
        "total must match the compaction gate's measure:\n{report}"
    );
    assert!(report.contains("~32768 model context window"), "{report}");
    assert!(report.contains("64 tool hops per turn"), "{report}");
    // Skill hints never ran — the row must be absent, not shown as 0.
    assert!(!report.contains("skill hint"), "{report}");
}

#[test]
fn context_report_without_budget_or_window_stays_honest() {
    let history = vec![
        ChatMsg::system("posture"),
        ChatMsg::user("hello"),
        ChatMsg::assistant("hi"),
    ];
    let report = context_report(&history, &[], 0, None, None);
    assert!(report.contains("no compaction budget set"), "{report}");
    assert!(!report.contains("model context window"), "{report}");
    assert!(
        report.contains("~0 tok (0 tools)"),
        "empty schema set reports zero:\n{report}"
    );
    assert!(
        report.contains("hard 20 provider calls; ANGEL_MAX_HOPS can stop sooner"),
        "{report}"
    );
    // Optional injection rows stay absent on a plain conversation.
    for label in [
        "compaction note",
        "recall notes",
        "memory block",
        "goal block",
    ] {
        assert!(!report.contains(label), "{label:?} leaked into:\n{report}");
    }
}

#[test]
fn strip_leaves_legacy_block_without_sentinel_untouched() {
    // A block written before sentinels existed has no [/memory]; leave it be.
    let mut s: Arc<str> = format!(
        "{}\n- old fact\n\nreal ask",
        crate::memory::MEMORY_BLOCK_HEADER
    )
    .into();
    let ptr = s.as_ref().as_ptr();
    let before = Arc::clone(&s);
    strip_context_blocks(&mut s);
    assert_eq!(&*s, &*before);
    assert_eq!(
        s.as_ref().as_ptr(),
        ptr,
        "unterminated legacy blocks must not allocate a replacement"
    );
}

#[test]
fn strip_is_noop_when_headers_are_absent() {
    let mut s: Arc<str> = Arc::from("plain operator text\nwith a second line");
    let ptr = s.as_ref().as_ptr();
    strip_context_blocks(&mut s);
    assert_eq!(&*s, "plain operator text\nwith a second line");
    assert_eq!(
        s.as_ref().as_ptr(),
        ptr,
        "clean user text must keep its existing allocation"
    );
}

#[test]
fn strip_delimited_block_borrows_when_header_is_absent_or_not_anchored() {
    let plain = "just a user question";
    let absent = strip_delimited_block(
        plain,
        crate::memory::MEMORY_BLOCK_HEADER,
        crate::memory::MEMORY_BLOCK_SENTINEL,
    );
    assert!(matches!(absent, Cow::Borrowed(_)));
    assert_eq!(absent.as_ptr(), plain.as_ptr());
    assert_eq!(&*absent, plain);

    let mid = format!(
        "please quote {} mid-sentence",
        crate::memory::MEMORY_BLOCK_HEADER
    );
    let unanchored = strip_delimited_block(
        &mid,
        crate::memory::MEMORY_BLOCK_HEADER,
        crate::memory::MEMORY_BLOCK_SENTINEL,
    );
    assert!(matches!(unanchored, Cow::Borrowed(_)));
    assert_eq!(unanchored.as_ptr(), mid.as_ptr());
    assert_eq!(&*unanchored, mid);
}

#[test]
fn flush_queued_steers_persists_with_save_async() {
    // Session has no UI-thread hook that distinguishes save vs save_async,
    // so pin the call site. Crash window is documented on the persist line.
    let src = include_str!("../../../cockpit/src/app_control/commands.rs");
    let start = src
        .find("pub(crate) fn flush_queued_steers")
        .expect("flush_queued_steers present");
    let body = &src[start..];
    let end = body
        .find("\n    pub(crate) fn system_msg")
        .expect("system_msg follows flush_queued_steers");
    let body = &body[..end];
    assert!(
        body.contains("persist_and_start_turn_worker"),
        "steer flush must enqueue persist like submit, not block the UI thread:\n{body}"
    );
    assert!(
        !body.contains("session.save(&self.history)"),
        "steer flush must not call blocking Session::save:\n{body}"
    );
}

#[test]
fn persist_and_start_turn_worker_shares_one_history_snapshot() {
    let src = include_str!("../../../cockpit/src/app_control/commands.rs");
    let start = src
        .find("fn persist_and_start_turn_worker")
        .expect("persist_and_start_turn_worker present");
    let body = &src[start..];
    let end = body
        .find("\n    fn start_turn_worker_from")
        .expect("start_turn_worker_from follows persist_and_start_turn_worker");
    let body = &body[..end];
    assert!(
        body.contains("Arc::from(self.history.as_slice())")
            && body.contains("save_history(Arc::clone(&snapshot))")
            && body.contains("start_turn_worker_from(snapshot)"),
        "persist and spawn must share one history Arc:\n{body}"
    );
    assert!(
        !body.contains("self.history.clone()") && !body.contains("history.to_vec()"),
        "UI persist+spawn must not clone history twice:\n{body}"
    );
}

#[test]
fn start_turn_worker_reuses_cached_in_hand_route_identity() {
    let src = include_str!("../../../cockpit/src/app_control/commands.rs");
    let start = src
        .find("fn start_turn_worker_from")
        .expect("start_turn_worker_from present");
    let body = &src[start..];
    let end = body
        .find("\n    fn queue_steer")
        .expect("queue_steer follows start_turn_worker_from");
    let body = &body[..end];
    assert!(
        body.contains("in_hand_route_identity()") && !body.contains("club.route_identity()"),
        "Enter spawn must clone the cached chrome identity:\n{body}"
    );
}
