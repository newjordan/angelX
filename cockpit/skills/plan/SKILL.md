---
name: plan
description: Turn a vague task into a small, ordered, verifiable plan before editing — scope, sequence, risks, and a checkpoint per step.
---

# Plan before building

For anything beyond a trivial edit, plan first — a wrong plan cheaply revised
beats a wrong implementation expensively unwound.

1. **Pin the goal and done-condition.** One sentence each: what success is, and
   how you'll *know* it (the test/command that will be green). If you can't state
   the done-condition, the task is still ambiguous — resolve that first.
2. **Survey before you commit.** `grep` / `file_search` / `outline` the relevant
   code so the plan fits what's actually there, not what you assume.
3. **Decompose into small, ordered steps** — each independently verifiable and,
   ideally, independently committable. Prefer a sequence where every step leaves
   the build green.
4. **Front-load risk.** Do the uncertain/load-bearing step early (a spike or
   probe) so a dead end surfaces before you've built on top of it.
5. **Note constraints & non-goals** — what must not change (public API, behavior,
   style), and what you're deliberately leaving out.
6. **Track it.** Use the `todo` tool for multi-step work; mark steps done as you
   verify them, and revise the plan when reality disagrees — don't push a stale
   plan forward.

Record the plan as the todo list (or the message), then execute step by step,
verifying each. Plans are cheap; rework is not.
