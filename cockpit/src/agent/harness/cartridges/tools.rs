//! A tool cartridge's `[[tools]]`: each is a program in the cartridge's folder,
//! registered deferred, so it costs nothing until `tool_search` brings it in,
//! or, marked `core`, advertised with the core coding tools.
//! A call runs the program in the session's workspace with the call's
//! arguments as JSON on stdin; its stdout is the result, and a failing exit
//! is an error carrying the end of its stderr.

use super::config::ProgramTool;
use crate::agent::club::ToolDef;
use crate::agent::harness::{Tool, ToolRegistry};
use crate::agent::sandbox::process_owner::OwnedCommandExt;
use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_ERROR_BYTES: usize = 2 * 1024;
const POLL: Duration = Duration::from_millis(20);

pub(crate) struct CartridgeTool {
    spec: ProgramTool,
    folder: PathBuf,
    workspace: PathBuf,
}

impl CartridgeTool {
    pub(crate) fn new(spec: ProgramTool, folder: PathBuf, workspace: PathBuf) -> Self {
        Self {
            spec,
            folder,
            workspace,
        }
    }
}

impl Tool for CartridgeTool {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn def(&self) -> ToolDef {
        ToolDef {
            name: self.spec.name.clone(),
            description: self.spec.description.clone(),
            params: if self.spec.params.is_null() {
                serde_json::json!({"type": "object", "properties": {}})
            } else {
                self.spec.params.clone()
            },
        }
    }

    fn call(&self, args: &Value) -> Result<String, String> {
        self.call_with_cancel(args, None)
    }

    fn call_with_cancel(&self, args: &Value, cancel: Option<&AtomicBool>) -> Result<String, String> {
        let name = &self.spec.name;
        let program = self.folder.join(&self.spec.run);
        let mut child = std::process::Command::new(&program)
            .current_dir(&self.workspace)
            .env("ANGEL_CARTRIDGE_DIR", &self.folder)
            .env("ANGEL_WORKSPACE", &self.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn_owned()
            .map_err(|e| format!("{name}: start {}: {e}", program.display()))?;
        if let Some(mut stdin) = child.stdin.take() {
            // A program that never reads its arguments closes the pipe early.
            let _ = stdin.write_all(args.to_string().as_bytes());
        }
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let status = loop {
            if cancel.is_some_and(|signal| signal.load(Ordering::Acquire)) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name} cancelled"));
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(POLL),
                Err(e) => return Err(format!("{name}: {e}")),
            }
        };
        let stdout = stdout.join().unwrap_or_default();
        let stderr = stderr.join().unwrap_or_default();
        if !status.success() {
            let detail = tail(stderr.trim(), MAX_ERROR_BYTES);
            return Err(if detail.is_empty() {
                format!("{name} exited {status}")
            } else {
                format!("{name} exited {status}: {detail}")
            });
        }
        let mut output = stdout;
        if output.len() > MAX_OUTPUT_BYTES {
            let mut end = MAX_OUTPUT_BYTES;
            while !output.is_char_boundary(end) {
                end -= 1;
            }
            output.truncate(end);
            output.push_str(&format!("\n[{name}: output cut at {MAX_OUTPUT_BYTES} bytes]"));
        }
        Ok(output)
    }
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn tail(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// Whether a plugged-in cartridge marks `name` as a core tool.
pub(crate) fn is_core(name: &str) -> bool {
    super::all()
        .iter()
        .filter(|cartridge| cartridge.folder.is_some())
        .any(|cartridge| cartridge.config.tools.iter().any(|tool| tool.core && tool.name == name))
}

/// Every plugged-in config-only cartridge's tools: deferred, or core. A name already
/// taken keeps its first tool.
pub(crate) fn register(r: &mut ToolRegistry, workspace: &Path) {
    for cartridge in super::all() {
        let Some(folder) = &cartridge.folder else {
            continue;
        };
        for spec in &cartridge.config.tools {
            if r.has_tool(&spec.name) {
                eprintln!(
                    "cartridge: {}: tool `{}` is already taken",
                    cartridge.label(),
                    spec.name
                );
                continue;
            }
            let tool = Box::new(CartridgeTool::new(spec.clone(), folder.clone(), workspace.to_path_buf()));
            if spec.core {
                r.register(tool);
            } else {
                r.register_deferred(tool);
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/cockpit/harness/cartridges_tools__tests.rs"]
mod tests;
