//! Long-form memory backend — the "memory palace".
//!
//! Compaction deposits distilled, *structured* notes here as **drawers**; recall
//! pulls the relevant ones back into a later turn's context. The store is backed
//! by an external [MemPalace](https://github.com/mempalace/mempalace) MCP server,
//! spoken over the same stdio JSON-RPC client as any other MCP server (see
//! [`crate::agent::mcp`]).
//!
//! Memory is treated as **infrastructure, not an agent tool**: host code
//! (compaction, recall) calls it directly, so it owns a dedicated client rather
//! than routing through the agent-facing tool registry — that also avoids a
//! second MemPalace process contending on the same on-disk store.
//!
//! Everything is best-effort and opt-in. With `ANGEL_MEMPALACE_CMD` unset the
//! store is a [`NullStore`] no-op and the cockpit behaves exactly as before; a
//! spawn/handshake failure degrades to the same no-op rather than breaking a turn.

use crate::agent::mcp::{McpClient, ServerSpec};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drawer {
    pub wing: String,
    pub room: String,
    pub content: String,
    pub source: String,
}

pub use crate::telemetry::MemoryHealth;

pub trait MemoryStore: Send + Sync {
    fn deposit(&self, drawer: &Drawer) -> Result<String, String>;

    fn search(&self, query: &str, limit: usize, wing: Option<&str>) -> Result<Vec<String>, String>;

    fn status(&self) -> Result<String, String>;

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

pub struct McpMemoryStore {
    client: Mutex<Arc<McpClient>>,
    reconnect: Option<(ServerSpec, Duration)>,
    health: Mutex<MemoryHealth>,
}

impl McpMemoryStore {
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
        return Vec::new();
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
#[path = "../../../../tests/cockpit/app/memory_store__tests.rs"]
mod tests;
