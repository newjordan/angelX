---
name: self-modify
description: Safely change angel0-cockpit's OWN source — understand it with self_map, edit in an isolated git worktree, and integrate only when build+test are green.
---

# Modify yourself safely

You are `angel0-cockpit`. This playbook is for changing **your own code**. The one
rule: **never let a change reach the live tree unless it builds AND the tests pass.**
You must never brick yourself.

## 1. Understand before you touch

- Call `self_map` for the full map of your source (modules, purposes, key types,
  build/test/run). Call `self_map({"module":"<name>"})` for one module's outline.
- The injected "Self-model" block in your system prompt already tells you the crate
  root and build/test commands. Drill in with `outline`, `grep`, `file_search`, and
  (if `ANGEL_LSP=1`) `lsp_definition`/`lsp_references` before editing.
- Read `docs/SELF_MODEL.md` for the architecture and the safety design.

## 2. Make the change in isolation

- Prefer `delegate("<club>", "<task>")` — it runs on an **isolated git worktree**
  with sandboxed `shell`/`cargo`, so the live tree is never at risk. Risky or broad
  edits MUST go this route.
- Keep edits small and reversible. Everything is git: a bad change is a branch drop
  away.

## 3. Gate every change on build + test

This is non-negotiable. A change is acceptable only if **both** hold:

- `cargo build` (or `cargo check`) succeeds — a non-compiling tree is an instant
  reject.
- `cargo test` is green — at least one test ran and none failed.

Run them via the `cargo` / `run_tests` tools. Do not ignore a failure without
current, reproducible evidence that it predates the change.
Use development builds during edits. For final native qualification, use
`scripts/check-cockpit-fast.sh --release <relevant existing filters>` from a
committed candidate; it binds identity before compilation and verifies the
resulting executable. A known failed verifier requires a diagnosis, not repeated
whole-cohort rebuilds of unchanged source. Return a coherent tested candidate
with unresolved acceptance stated explicitly; do not keep polishing after the
requested gates pass.
The same logic is encoded in `tools::self_model::run_self_gate` /
`evaluate_gate` — the integrate/commit condition.

## 4. Integrate only when green

- If the gate is green, `integrate("<branch>")` to merge the worktree back.
- If it's red, do **not** integrate. Fix it on the branch and re-run the gate, or
  report the blocker with the preserved branch and logs. Keep incomplete work
  available when interrupted; discarding work is an explicit separate action.

## Don'ts

- Don't edit `src/main.rs` or `src/app_control.rs` casually — they're large, hot UI
  files; prefer new modules.
- Don't commit/push unless explicitly asked.
- Don't claim it works because it compiled — "it built" ≠ "tests pass". State
  exactly what the gate reported.
