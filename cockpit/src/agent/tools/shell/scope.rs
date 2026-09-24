//! The per-call shell capability: the shell's own sandbox policy.
//!
//! A model could once narrow a call to `read_only` or to a `write_paths` list.
//! Those scopes also cut the process tree off the GPU device nodes and scratch
//! space, and the tool told the model never to lift them, so a probe marked
//! read-only saw no GPU and the model went hunting for a CUDA it already had
//! (apollo, 2026-09-24). Every call now runs with the shell's full policy; such
//! arguments, if a model still sends them, are ignored.
use super::*;

pub(super) struct ExecutionScope {
    pub policy: SandboxPolicy,
}

impl ShellTool {
    pub(super) fn scope(&self, _args: &Value) -> Result<ExecutionScope, String> {
        Ok(ExecutionScope {
            policy: self.policy.clone(),
        })
    }
}
