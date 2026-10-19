# L01 — clickable Markdown transcript links

Operator-requested UI bugfix; no performance or leadership claim.

## Change

Preserve Markdown destinations independently of labels, derive visible hit regions
with the same ratatui wrapping as the transcript, and activate on mouse release.
Workspace-confined local artifacts open in Stage; HTTP(S) uses the existing opener.
Drags remain selection/copy gestures. Frozen selection-frame hit regions survive
a redraw between press and release. Small streamed Markdown replies are covered.
The preceding Glass Object Dock changes remain in the shared working tree.

## Evidence

Registration, amendment, original failures, all candidate logs, final source
snapshot, exact dirty-tree patch, hashes, and toolchain identities are retained in
`artifacts/harness-research/transcript-links/L01/`.

- `red.log`: test fixture compile error; not behavioral failure evidence.
- `red2.log`: primary artifact-opening regression failed before implementation.
- Candidate runs: 1, 5, 6, then 7 passing targeted tests.
- Final command: `cd cockpit && cargo test --features glass --no-default-features --bin angel transcript_link -- --nocapture`.
- Final result: **7 passed, 0 failed**, 5940 filtered out. Four existing compiler warnings remain.
- Coverage: exact local artifact/PNG destinations, streaming, redraw between press
  and release, duplicate labels, Unicode/wrapped/scroll-clipped hit cells, reference
  links, angle-bracket autolinks, drag selection, stale release, unsafe/missing targets.

## Limits and remaining deployment

One extra targeted invocation beyond the original five-attempt budget was recorded
in a timestamped amendment before the final run. Final source identity is retained;
historical per-attempt snapshots were not captured, so no repeatable A/B promotion
or independent holdout/audit claim is made. HTTP(S) launch was not exercised live.
No broader suite, fresh GPU acceptance, production rebuild, or restart was performed.
The running cockpit must be rebuilt/relaunched to use these source changes.
