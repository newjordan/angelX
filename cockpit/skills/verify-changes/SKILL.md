---
name: verify-changes
description: After editing, prove it works before claiming done — analyzer diagnostics, then build, then tests, then lint — instead of asserting success.
---

# Verify changes

A change isn't done because it looks right. Prove it, cheapest signal first:

1. **Analyzer first (`lsp_diagnostics <file>`).** Right after editing a file, get
   ground-truth errors/warnings from the language server — it's faster than a full
   `cargo`/`check` and catches type errors, unresolved names, and wrong signatures
   immediately. Fix every error it reports before moving on. (Needs `ANGEL_LSP`; if
   it's not available, skip to the build.)
2. **Then build/check.** `check` (or `cargo` for Rust) for the cross-module view
   the per-file analyzer can't give — trait coherence, breakage in callers,
   now-unused symbols.
3. **Then tests.** `run_tests`, scoped to the affected tests when you can. A green
   build with red tests is not done.
4. **Then lint + format.** `lint` and `fmt` so the diff matches house style — a
   reviewer shouldn't have to spend a comment on whitespace.
5. **State the result honestly.** Report what you ran and its outcome. If a step
   failed, or you skipped one, say so — never claim "done and verified" on a check
   you didn't run.

Smell: writing "this should now work" without having run anything. The tools are
right there — run them. Re-reading your own edit is not verification (the
no-progress guard will nudge you); execute a check instead.
