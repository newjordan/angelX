---
name: requesting-code-review
description: Prepare a change for review — tight diff, clear rationale, self-review first, tests + lint green — so review is fast and finds real issues.
---

# Requesting code review

A good review starts before you ask for one. Make the reviewer's job easy:

**Before requesting:**
- **Self-review the diff first** (`git_diff`). Read every hunk as if it were
  someone else's. You'll catch the obvious half before the reviewer does.
- **Green gates:** `run_tests`, `check`, `lint` all pass. Never send a red diff
  for review unless you're explicitly asking about the failure.
- **Tighten the diff.** One logical change per review. Pull unrelated cleanups
  into their own commit. Drop debug prints and stray TODOs.
- **New behavior has a test.** If it's not tested, say why in the description.

**The request itself** should answer, briefly:
- *What* changed and *why* (the problem, not just the patch).
- *How to verify* (the command you ran, the test that covers it).
- *What you're unsure about* — point reviewers at the risky hunk, the trade-off
  you made, the thing you'd want a second opinion on. Calibrated uncertainty
  gets better review than false confidence.
- *Scope boundaries* — what you deliberately left out.

When delegating a review to another agent (`delegate`), give it the diff, the
intent, and the specific dimensions to check (correctness, edge cases, security,
perf) rather than a vague "review this".
