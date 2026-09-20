use super::*;

#[test]
fn research_compose_disclosure_drops_background_and_keeps_supported_answers() {
    for answer in [
        "**No evidence exists to answer this question, so I cite nothing.**\n\nThe docs (pax-token, pax-upgrade) contain no such value.",
        "The corpus contains no evidence answering this question. Background: http://127.0.0.1:12345/doc/kestrel-m2",
        "Evidence is missing. [Background](https://example.test/doc/1)",
        "Evidence is insufficient; the answer is 123 [1].",
    ] {
        assert_eq!(finalize(answer), DISCLOSURE);
        assert_eq!(
            draft(&[ChatMsg::user("research"), ChatMsg::assistant(answer)]).as_deref(),
            Some(DISCLOSURE)
        );
    }
    for answer in [
        "The GlazeR R-series fires at **1185 °C**. Source: http://127.0.0.1:12345/doc/glaze-r",
        "30 requests per minute (http://127.0.0.1:12345/doc/sundial-api)",
        "42 [source](https://example.test/doc/1). Evidence is missing for a second question.",
        "Evidence is missing for the second part; the first is 42 [1].",
        "The passage says ‘Evidence is missing.’ [1]",
    ] {
        assert_eq!(finalize(answer), answer);
    }
}

#[test]
fn research_origin_envelope_schema_and_preamble() {
    let _guard = crate::tests::env_lock();
    let _env =
        crate::tests::TestEnvGuard::set("ANGEL_SEARXNG_URL", "http://127.0.0.1:34251/search");
    let history = [ChatMsg::user("Research question: use the corpus.")];
    let configured = origin(&history);
    assert_eq!(configured.as_deref(), Some("http://127.0.0.1:34251/search"));
    let mut defs = vec![
        crate::agent::tools::web::WebSearchTool.def(),
        crate::agent::tools::web::WebFetchTool.def(),
    ];
    describe_surface(&mut defs, configured.as_deref());
    let envelope = serde_json::json!({"preamble":preamble(configured.as_deref()), "tools":defs.iter().map(|d| serde_json::json!({"name":d.name,"description":d.description,"parameters":d.params})).collect::<Vec<_>>()});
    assert!(
        envelope["preamble"]
            .as_str()
            .unwrap()
            .contains(configured.as_deref().unwrap())
    );
    for def in envelope["tools"].as_array().unwrap() {
        assert!(
            def["description"]
                .as_str()
                .unwrap()
                .contains(configured.as_deref().unwrap())
        );
    }
    assert!(preamble(configured.as_deref()).contains("do not probe the host with shell"));
    drop(_env);
    let _env = crate::tests::TestEnvGuard::set("ANGEL_SEARXNG_URL", "");
    assert_eq!(origin(&history), None);
    assert!(!preamble(None).contains("http"));
    assert!(
        !crate::agent::tools::web::WebSearchTool
            .def()
            .description
            .contains("127.0.0.1")
    );
    let declared = [ChatMsg::user(
        "Research question: use sources.\nCorpus origin: http://127.0.0.1:12345",
    )];
    assert_eq!(origin(&declared).as_deref(), Some("http://127.0.0.1:12345"));
    assert!(
        crate::agent::tools::web::research_origin("https://user:secret@example.test").is_none()
    );
    assert_eq!(
        crate::agent::tools::web::research_search_endpoint("http://127.0.0.1:12345"),
        "http://127.0.0.1:12345/search"
    );
    assert_eq!(
        crate::agent::tools::web::research_search_endpoint("https://example.test/custom/search"),
        "https://example.test/custom/search"
    );
}

#[test]
fn research_repeated_search_resets_on_fetch_and_user_turn() {
    let search = || {
        ChatMsg::assistant_calls(vec![ToolCall {
            id: "s".into(),
            name: "web_search".into(),
            args: serde_json::json!({"query":"clock"}),
        }])
    };
    let mut history = vec![ChatMsg::user("research-answer"), search(), search()];
    assert!(repeated_search(&history));
    history.push(ChatMsg::assistant_calls(vec![ToolCall {
        id: "f".into(),
        name: "web_fetch".into(),
        args: serde_json::json!({"url":"https://example.test/doc/1"}),
    }]));
    history.push(search());
    assert!(!repeated_search(&history));
    history.push(search());
    assert!(repeated_search(&history));
    history.push(ChatMsg::user("new task"));
    assert!(!repeated_search(&history));
}

#[test]
fn research_off_surface_discovery_is_not_document_work() {
    for command in [
        "cat /proc/net/tcp",
        "ss -tlnp",
        "ps aux | grep corpus",
        "curl http://127.0.0.1:8888/search",
    ] {
        assert!(off_surface(
            "shell",
            &serde_json::json!({"command":command})
        ));
    }
    assert!(!off_surface(
        "shell",
        &serde_json::json!({"command":"python3 analyze.py"})
    ));
    assert!(!off_surface(
        "web_fetch",
        &serde_json::json!({"url":"http://127.0.0.1:12345/doc/1"})
    ));
    assert!(!crate::agent::club::final_response_requested(&[
        ChatMsg::user(COMPOSE)
    ]));
    assert!(crate::agent::club::final_response_requested(&[
        ChatMsg::harness(COMPOSE)
    ]));
}

#[test]
fn research_task_mode_uses_skill_selection_and_acceptance_contract() {
    let _guard = crate::tests::env_lock();
    let old = std::env::var_os("ANGEL_TASK_ACCEPT_CMD");
    unsafe {
        std::env::remove_var("ANGEL_TASK_ACCEPT_CMD");
    }
    let research = vec![ChatMsg::user(
        "Research question q04: answer using sources and citations.",
    )];
    assert!(selected(&research));
    assert!(selected(&[ChatMsg::user(
        "Use research-answer for this question."
    )]));
    assert!(!selected(&[ChatMsg::user(
        "Fix the research dashboard and run the tests."
    )]));
    assert!(!selected(&[ChatMsg::user(
        "Remove an unused renderer and preserve shared navigation. \
             Add regression tests and run local checks. \
             Use the existing source; do not perform network research."
    )]));
    assert!(!selected(&[ChatMsg::user(
        "Fix the research answer API and add regression tests."
    )]));

    assert!(!selected(&[ChatMsg::user(
        "Read the fixture URL using web_fetch and report what it returns."
    )]));
    unsafe {
        std::env::set_var("ANGEL_TASK_ACCEPT_CMD", "cargo test");
    }
    assert!(!selected(&research));
    unsafe {
        match old {
            Some(v) => std::env::set_var("ANGEL_TASK_ACCEPT_CMD", v),
            None => std::env::remove_var("ANGEL_TASK_ACCEPT_CMD"),
        }
    }
}

#[test]
fn research_stopped_draft_is_current_verbatim_and_not_raw_markup() {
    let answer = "42 [source](https://example.test/a)";
    let mut history = vec![ChatMsg::user("research-answer"), ChatMsg::assistant(answer)];
    assert_eq!(draft(&history).as_deref(), Some(answer));
    history.push(ChatMsg::assistant(""));
    history.push(ChatMsg::assistant(
        r#"{"authority_profile": {"profile": "full"}}"#,
    ));
    assert_eq!(draft(&history).as_deref(), Some(answer));
    history.push(ChatMsg::user("new request"));
    assert_eq!(draft(&history), None);
    history.push(ChatMsg::assistant(""));
    assert_eq!(draft(&history), None);
}

#[test]
fn research_sources_use_urls_not_queries_snippets_or_fragments() {
    let a = sources(
        "web_search",
        &serde_json::json!({"query":"a"}),
        "1. A\n https://example.test/a#first\n snippet",
    );
    let b = sources(
        "web_search",
        &serde_json::json!({"query":"b"}),
        "1. Different\n https://example.test/a#second\n other snippet",
    );
    assert_eq!(a, b);
    assert_eq!(a.len(), 1);
    assert!(sources("web_search", &Value::Null, "no results").is_empty());
    assert!(
        sources(
            "web_fetch",
            &serde_json::json!({"url":"https://example.test/a"}),
            ""
        )
        .is_empty()
    );
    assert_eq!(
        sources(
            "web_fetch",
            &serde_json::json!({"url":"https://example.test/a"}),
            "body"
        ),
        a
    );
}
