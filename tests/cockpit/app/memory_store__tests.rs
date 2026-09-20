use super::*;

#[test]
fn null_store_is_a_silent_noop() {
    let s = NullStore;
    assert!(!s.is_live());
    assert_eq!(
        s.deposit(&Drawer {
            wing: "w".into(),
            room: "r".into(),
            content: "c".into(),
            source: "s".into(),
        }),
        Ok(String::new())
    );
    assert_eq!(s.search("anything", 5, None), Ok(Vec::new()));
    assert_eq!(s.status(), Ok(String::new()));
}

#[test]
fn connect_from_env_without_config_is_a_noop_store() {
    // No ANGEL_MEMPALACE_CMD in the test environment → never spawns a process.
    // (Set-and-restore would race other tests; absence is the default path.)
    if std::env::var_os("ANGEL_MEMPALACE_CMD").is_none() {
        assert!(!connect_from_env().is_live());
    }
}

#[test]
fn interpret_write_distinguishes_soft_failures() {
    // success:true (incl. dedupe) and plain confirmations stay Ok…
    assert!(interpret_write(r#"{"success": true, "id": "abc"}"#).is_ok());
    assert!(interpret_write(r#"{"success": true, "reason": "already_exists"}"#).is_ok());
    assert!(interpret_write("filed").is_ok()); // non-JSON confirmation
    // …but an in-result failure becomes Err so it isn't counted as stored.
    assert_eq!(
        interpret_write(r#"{"success": false, "error": "bad wing"}"#),
        Err("bad wing".to_string())
    );
    assert_eq!(
        interpret_write(r#"{"error": "No palace found", "hint": "run init"}"#),
        Err("No palace found".to_string())
    );
}

#[test]
fn parse_search_results_extracts_one_block_per_hit() {
    let json = r#"{"results": [
            {"wing": "angel0", "room": "Facts", "text": "gemma4 listens on :8000"},
            {"wing": "angel0", "room": "Files", "text": "  compaction.rs  "},
            {"wing": "angel0", "room": "Empty", "text": "   "}
        ], "total_before_filter": 3}"#;
    let blocks = parse_search_results(json);
    assert_eq!(blocks.len(), 2, "the blank-text hit is dropped");
    assert_eq!(blocks[0], "[angel0/Facts] gemma4 listens on :8000");
    assert_eq!(blocks[1], "[angel0/Files] compaction.rs");
}

#[test]
fn parse_search_results_handles_error_and_nonjson() {
    // An error dict (no `results`) → nothing to recall, never injected as memory.
    assert!(parse_search_results(r#"{"error": "No palace found"}"#).is_empty());
    assert!(parse_search_results(r#"{"results": []}"#).is_empty());
    // Non-JSON → fall back to the raw text rather than silently dropping it.
    assert_eq!(
        parse_search_results("plain text"),
        vec!["plain text".to_string()]
    );
    assert!(parse_search_results("   ").is_empty());
}

#[test]
fn shell_split_handles_plain_and_quoted_args() {
    assert_eq!(
        shell_split("mempalace-mcp --palace /srv/p --backend chroma"),
        vec!["mempalace-mcp", "--palace", "/srv/p", "--backend", "chroma"]
    );
    assert_eq!(
        shell_split(r#"cmd --palace "/srv/with space/palace" --x"#),
        vec!["cmd", "--palace", "/srv/with space/palace", "--x"]
    );
    assert_eq!(
        shell_split("ssh model-host mempalace-mcp --palace '/data/p'"),
        vec!["ssh", "model-host", "mempalace-mcp", "--palace", "/data/p"]
    );
    assert!(shell_split("   ").is_empty());
    // An unterminated quote absorbs the rest into the final token (graceful, no
    // panic) — fine for a single config string.
    assert_eq!(
        shell_split(r#"cmd --palace "/srv/p"#),
        vec!["cmd", "--palace", "/srv/p"]
    );
}

#[test]
fn transport_degrades_reconnects_replays_search_but_not_deposit() {
    let dir = std::env::temp_dir().join(format!("angel-memory-crash-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ledger = dir.join("calls.log");
    let script = r#"
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"initialize"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"protocolVersion":"x"}}\n' "$id" ;;
    *'"mempalace_add_drawer"'*) printf 'deposit\n' >> "$STATE_FILE"; exit 0 ;;
    *'"mempalace_search"'*)
      printf 'search\n' >> "$STATE_FILE"
      printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"{\\"results\\":[{\\"wing\\":\\"w\\",\\"room\\":\\"r\\",\\"text\\":\\"recalled\\"}]}"}]}}\n' "$id" ;;
    *'"mempalace_status"'*) printf '{"jsonrpc":"2.0","id":%s,"result":{"content":[{"type":"text","text":"healthy"}]}}\n' "$id" ;;
  esac
done
"#;
    let spec = ServerSpec {
        name: "memory-crash-mock".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        env: vec![("STATE_FILE".into(), ledger.to_string_lossy().into_owned())],
    };
    let timeout = Duration::from_secs(2);
    let client = McpClient::connect(&spec, timeout).expect("connect memory mock");
    let store = McpMemoryStore::reconnectable(client, spec, timeout);
    let deposit = store.deposit(&Drawer {
        wing: "w".into(),
        room: "r".into(),
        content: "once".into(),
        source: "test".into(),
    });
    assert!(
        deposit
            .expect_err("closed transport is ambiguous")
            .contains("deposit unconfirmed")
    );
    assert_eq!(store.health(), MemoryHealth::Degraded);

    let recalled = store.search("query", 3, Some("w")).expect("safe replay");
    assert_eq!(recalled, vec!["[w/r] recalled"]);
    assert_eq!(store.health(), MemoryHealth::Healthy);
    let calls = std::fs::read_to_string(&ledger).unwrap();
    assert_eq!(calls.lines().filter(|line| *line == "deposit").count(), 1);
    assert_eq!(calls.lines().filter(|line| *line == "search").count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
