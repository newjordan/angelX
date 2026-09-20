---
name: systematic-debugging
description: Find the root cause by forming and testing hypotheses, not by guessing — reproduce, isolate, bisect, fix, regression-guard.
---

# Systematic debugging

Resist the urge to change code until you understand the failure. Work the loop:

1. **Reproduce reliably.** Get a deterministic repro (a command, a `run_tests`
   case). If it's flaky, that *is* the first bug — chase the nondeterminism
   (shared state, ordering, time, randomness) before anything else.
2. **Read the actual error.** The message + the first failing frame usually name
   the cause. Don't skim it. `grep` the exact text to its source.
3. **Form one hypothesis** about the cause, and a cheap way to confirm/refute it
   (a log line, a unit test, a `code_mode` probe). Test the *opposite* if you're
   stuck — confirmation bias hides the cause.
4. **Isolate / bisect.** Narrow the surface: shrink the input, comment out halves,
   `git_diff` the last working state. Binary-search the change or the data that
   triggers it.
5. **Fix the cause, not the symptom.** A `?:` guard that hides a `None` is not a
   fix. Once found, write a test that fails on the bug (it's your regression
   guard), then make it pass.
6. **Verify the blast radius.** `run_tests` + `check` + `lint`; confirm you didn't
   fix one case and break a neighbor.

Smell: if you've made 3+ changes and still don't understand *why* the bug
happens, stop editing — you're guessing. Go back to step 3 with a real probe.
