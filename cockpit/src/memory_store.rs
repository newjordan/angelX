//! Long-form memory backend — the "memory palace".
//!
//! Compaction deposits distilled, *structured* notes here as **drawers**; recall
//! pulls the relevant ones back into a later turn's context. The store is backed
//! by an external [MemPalace](https://github.com/mempalace/mempalace) MCP server,
//! spoken over the same stdio JSON-RPC client as any other MCP server (see
//! [`crate::mcp`]).
//!
//! Memory is treated as **infrastructure, not an agent tool**: host code
//! (compaction, recall) calls it directly, so it owns a dedicated client rather
//! than routing through the agent-facing tool registry — that also avoids a
//! second MemPalace process contending on the same on-disk store.
//!
//! Everything is best-effort and opt-in. With `ANGEL_MEMPALACE_CMD` unset the
//! store is a [`NullStore`] no-op and the cockpit behaves exactly as before; a
//! spawn/handshake failure degrades to the same no-op rather than breaking a turn.

use crate::mcp::{McpClient, ServerSpec};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One unit of stored memory — maps directly onto a MemPalace `add_drawer` call.
/// `wing` groups by project/agent, `room` by topic/section-kind (Decisions,
/// Files, Facts, …), `content` is the note text, `source` tags provenance (the
/// session id) so a drawer can be traced back to where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drawer {
    pub wing: String,
    pub room: String,
    pub content: String,
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryHealth {
    Disabled,
    Healthy,
    Degraded,
}

/// The palace interface host code depends on. A trait so compaction/recall are
/// testable against an in-memory double and degrade to a no-op when unconfigured.
pub trait MemoryStore: Send + Sync {
    /// File one drawer. Returns the server's confirmation text. Best-effort: the
    /// caller treats an `Err` as "not stored", preserves inline continuity, and
    /// reports the bounded batch outcome without blocking the primary turn.
    fn deposit(&self, drawer: &Drawer) -> Result<String, String>;

    /// Retrieve up to `limit` recall blocks relevant to `query`, optionally scoped
    /// to one `wing`, most-relevant first. Each element is one drawer's text,
    /// ready to inject.
    fn search(&self, query: &str, limit: usize, wing: Option<&str>) -> Result<Vec<String>, String>;

    /// Cheap palace overview for session wake-up (drawer counts + wing/room map).
    fn status(&self) -> Result<String, String>;

    /// Whether a real backend is wired (false for [`NullStore`]) — lets callers
    /// skip recall plumbing entirely when there's nothing behind it.
    fn is_live(&self) -> bool {
        true
    }

    fn health(&self) -> MemoryHealth {
        if self.is_live() {
            MemoryHealth::Healthy
        } else {
            MemoryHealth::Disabled
        }
    }
}

/// No backend configured: every operation is a silent no-op. Keeps callers
/// branch-free — they always hold a store, it just does nothing.
pub struct NullStore;

impl MemoryStore for NullStore {
    fn deposit(&self, _drawer: &Drawer) -> Result<String, String> {
        Ok(String::new())
    }
    fn search(
        &self,
        _query: &str,
        _limit: usize,
        _wing: Option<&str>,
    ) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn status(&self) -> Result<String, String> {
        Ok(String::new())
    }
    fn is_live(&self) -> bool {
        false
    }
    fn health(&self) -> MemoryHealth {
        MemoryHealth::Disabled
    }
}

/// MemPalace-backed store: deposits/searches via its MCP tools over stdio.
pub struct McpMemoryStore {
    client: Mutex<Arc<McpClient>>,
    reconnect: Option<(ServerSpec, Duration)>,
    health: Mutex<MemoryHealth>,
}

impl McpMemoryStore {
    // MemPalace tool names, from `mempalace/mcp_server.py`. Host-side we call the
    // raw remote names (not the cockpit-mangled `<server>__<tool>` form, which is
    // only for tools surfaced to the agent).
    const ADD: &'static str = "mempalace_add_drawer";
    const SEARCH: &'static str = "mempalace_search";
    const STATUS: &'static str = "mempalace_status";

    fn reconnectable(client: Arc<McpClient>, spec: ServerSpec, timeout: Duration) -> Self {
        Self {
            client: Mutex::new(client),
            reconnect: Some((spec, timeout)),
            health: Mutex::new(MemoryHealth::Healthy),
        }
    }

    fn mark_degraded(&self) {
        if let Ok(mut health) = self.health.lock() {
            *health = MemoryHealth::Degraded;
        }
    }

    fn reconnect_once(&self) -> Result<(), String> {
        let Some((spec, timeout)) = &self.reconnect else {
            return Err("memory transport cannot be reconnected".to_string());
        };
        let client = McpClient::connect(spec, *timeout)?;
        *self
            .client
            .lock()
            .map_err(|_| "memory client lock poisoned".to_string())? = client;
        if let Ok(mut health) = self.health.lock() {
            *health = MemoryHealth::Healthy;
        }
        Ok(())
    }

    fn ensure_connected(&self) -> Result<(), String> {
        if self.health() == MemoryHealth::Degraded {
            self.reconnect_once()?;
        }
        Ok(())
    }

    fn call(&self, tool: &str, args: &serde_json::Value) -> Result<String, String> {
        self.client
            .lock()
            .map_err(|_| "memory client lock poisoned".to_string())?
            .call_tool(tool, args)
    }

    fn call_replayable(&self, tool: &str, args: &serde_json::Value) -> Result<String, String> {
        self.ensure_connected()?;
        match self.call(tool, args) {
            Ok(result) => Ok(result),
            Err(first) => {
                self.mark_degraded();
                self.reconnect_once()
                    .map_err(|reconnect| format!("{first}; reconnect failed: {reconnect}"))?;
                self.call(tool, args).inspect_err(|_| self.mark_degraded())
            }
        }
    }
}

impl MemoryStore for McpMemoryStore {
    fn deposit(&self, drawer: &Drawer) -> Result<String, String> {
        self.ensure_connected()?;
        let result = self.call(
            Self::ADD,
            &json!({
                "wing": drawer.wing,
                "room": drawer.room,
                "content": drawer.content,
                "source_file": drawer.source,
                "added_by": "angel-compaction",
            }),
        );
        let text = match result {
            Ok(text) => text,
            Err(error) => {
                self.mark_degraded();
                return Err(format!(
                    "deposit unconfirmed after transport failure; not replayed: {error}"
                ));
            }
        };
        interpret_write(&text)
    }

    fn search(&self, query: &str, limit: usize, wing: Option<&str>) -> Result<Vec<String>, String> {
        let mut args = json!({ "query": query, "limit": limit });
        if let Some(w) = wing {
            args["wing"] = json!(w);
        }
        let text = self.call_replayable(Self::SEARCH, &args)?;
        Ok(parse_search_results(&text))
    }

    fn status(&self) -> Result<String, String> {
        self.call_replayable(Self::STATUS, &json!({}))
    }

    fn health(&self) -> MemoryHealth {
        self.health
            .lock()
            .map(|health| *health)
            .unwrap_or(MemoryHealth::Degraded)
    }
}

/// Interpret a `mempalace_add_drawer` reply. MemPalace signals soft failures
/// *inside* an otherwise-successful JSON-RPC result (`{"success": false, "error":
/// …}`, or `{"error": "No palace …"}` on an uninitialized palace) rather than via
/// the MCP `isError` flag — so a raw `Ok` would count a no-op write as "stored".
/// Treat an explicit failure as `Err`; `{"success": true}` (including the
/// `already_exists` dedupe case) and any non-JSON reply stay `Ok`.
fn interpret_write(text: &str) -> Result<String, String> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        if v.get("success").and_then(|s| s.as_bool()) == Some(false) {
            let e = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("deposit rejected");
            return Err(e.to_string());
        }
        if v.get("success").is_none()
            && let Some(e) = v.get("error").and_then(|e| e.as_str())
        {
            return Err(e.to_string());
        }
    }
    Ok(text.to_string())
}

/// Parse a `mempalace_search` reply into one recall block per hit. MemPalace
/// returns `{"results": [{"wing","room","text",…}], …}` JSON-dumped into a single
/// text block, so we extract each hit's verbatim `text` (prefixed with its
/// `[wing/room]`) rather than injecting the whole JSON. An error dict / no-results
/// reply yields an empty list; a non-JSON reply falls back to the raw text so a
/// response is never silently dropped.
fn parse_search_results(text: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        let t = text.trim();
        return if t.is_empty() {
            Vec::new()
        } else {
            vec![t.to_string()]
        };
    };
    let Some(results) = v.get("results").and_then(|r| r.as_array()) else {
        return Vec::new(); // error dict ("No palace found") / unexpected shape
    };
    results
        .iter()
        .filter_map(|hit| {
            let body = hit.get("text").and_then(|t| t.as_str())?.trim();
            if body.is_empty() {
                return None;
            }
            let wing = hit.get("wing").and_then(|w| w.as_str()).unwrap_or("");
            let room = hit.get("room").and_then(|r| r.as_str()).unwrap_or("");
            Some(if wing.is_empty() && room.is_empty() {
                body.to_string()
            } else {
                format!("[{wing}/{room}] {body}")
            })
        })
        .collect()
}

/// Connect to the configured MemPalace server, or return a [`NullStore`].
///
/// Configured via `ANGEL_MEMPALACE_CMD` — a command line that starts the MemPalace
/// MCP server over stdio, e.g.
/// `mempalace-mcp --palace /srv/angel-palace --backend chroma`, or, when
/// the cockpit runs off the box that owns the store,
/// `ssh model-host mempalace-mcp --palace /srv/angel-palace` (the data stays on
/// the explicitly configured host and JSON-RPC tunnels over the SSH pipe).
/// Unset → no-op store. Spawn/handshake failure → no-op store (logged by the
/// caller), never a hard error.
pub fn connect_from_env() -> Arc<dyn MemoryStore> {
    let Some(cmdline) = std::env::var("ANGEL_MEMPALACE_CMD")
        .ok()
        .filter(|s| !s.trim().is_empty())
    else {
        return Arc::new(NullStore);
    };
    let parts = shell_split(&cmdline);
    let Some((command, args)) = parts.split_first() else {
        return Arc::new(NullStore);
    };
    let spec = ServerSpec {
        name: "mempalace".to_string(),
        command: command.clone(),
        args: args.to_vec(),
        env: Vec::new(),
    };
    let timeout = Duration::from_secs(
        std::env::var("ANGEL_MEMPALACE_TIMEOUT")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(30),
    );
    // Bound startup: the connect runs at launch on the main thread, before the TUI
    // draws, and `initialize` is otherwise only bounded by the (long) per-call
    // timeout — a wedged server or a stalling `ssh` would hang the cockpit. Run it
    // on a side thread and give up after a short handshake deadline, degrading to
    // the no-op store. A late client is dropped (its child reaped by McpClient's
    // Drop).
    let connect_deadline = Duration::from_secs(
        std::env::var("ANGEL_MEMPALACE_CONNECT_TIMEOUT")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(8),
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let connect_spec = spec.clone();
    std::thread::spawn(move || {
        let _ = tx.send(McpClient::connect(&connect_spec, timeout));
    });
    match rx.recv_timeout(connect_deadline) {
        Ok(Ok(client)) => Arc::new(McpMemoryStore::reconnectable(client, spec, timeout)),
        _ => Arc::new(NullStore),
    }
}

/// Minimal quote-aware split of a command line on unquoted whitespace, honoring
/// single and double quotes (so a `--palace "/srv/with space/palace"` survives).
/// Not a full shell — just enough for the one config string we parse, avoiding a
/// dependency.
fn shell_split(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else {
                    cur.push(c);
                }
            }
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    in_token = true;
                }
                c if c.is_whitespace() => {
                    if in_token {
                        out.push(std::mem::take(&mut cur));
                        in_token = false;
                    }
                }
                c => {
                    cur.push(c);
                    in_token = true;
                }
            },
        }
    }
    if in_token {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
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
}
