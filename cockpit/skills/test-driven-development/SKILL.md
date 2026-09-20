---
name: test-driven-development
description: Write a failing test first, make it pass, then refactor — the red/green/refactor loop, using angel's run_tests/check tools.
---

# Test-driven development

Drive each change through a test before writing the implementation. The loop:

1. **Red** — write the smallest test that captures the next behavior, and run it.
   Confirm it *fails for the right reason* (assertion, not a compile error you
   didn't intend). Use `run_tests` (scoped to the new test if the runner allows).
2. **Green** — write the least code that makes it pass. No extra abstraction yet.
   Re-run `run_tests`; it should pass and nothing else should break.
3. **Refactor** — with the test green as a safety net, clean up names, dedupe,
   simplify. Re-run after each refactor.

Rules of thumb:
- One behavior per test; name it for the behavior, not the function.
- A test that can't fail tests nothing — see it red before you see it green.
- Test the contract (inputs → outputs / errors), not the implementation detail.
- Keep pure logic separate from I/O so it's testable without the world (this is
  why angel splits e.g. parsers from their env/fs wrappers).
- Before declaring done: `run_tests` clean, then `check` (compiles) and `lint`.

When a bug is reported, first write a test that reproduces it (red), *then* fix —
the test becomes the regression guard.
