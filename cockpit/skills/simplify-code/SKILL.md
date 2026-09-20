---
name: simplify-code
description: Reduce complexity without changing behavior — delete, dedupe, flatten, name well — with tests green before and after.
---

# Simplify code

Simplification is behavior-preserving by definition: have a green `run_tests`
before you start and after every step, so "simpler" never means "different".

Moves, roughly in order of payoff:
- **Delete.** Dead code, unused params, speculative abstraction, commented-out
  blocks. The simplest code is the code that isn't there.
- **Dedupe.** Fold copy-pasted logic into one well-named function. But don't
  over-DRY: two things that look alike yet change for different reasons should
  stay apart.
- **Flatten control flow.** Early-return guard clauses over nested `if`; handle
  the error/edge case first and let the happy path fall through unindented.
- **Name for intent.** A precise name removes the need for a comment. Rename
  until the call site reads like a sentence.
- **Shrink scope.** Narrow visibility, move locals next to first use, make data
  immutable where you can.
- **Right-size types.** Replace stringly-typed flags and bare tuples with enums /
  small structs when it removes a class of mistake.

Keep the diff focused: simplification and feature changes in separate commits.
Match the surrounding style — simpler should also mean *consistent*. Verify with
`run_tests` + `lint`; the win is fewer lines, fewer branches, clearer names — not
cleverness.
