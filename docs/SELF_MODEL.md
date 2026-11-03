# Working on the cockpit source

The `self_map` tool reads the active cockpit crate and returns its module map.
Pass `module: "tools/nav"` for a module outline. In an interactive self-work
session, `write: true` also writes `SELF.md` in that crate. Headless diagnostics
are read-only and require an explicit source pin.

`ANGEL_SELF_SRC` selects the cockpit crate directory. Without a pin, the source
locator checks the build-time crate path and nearby checkout. Source context is
injected only for work on the cockpit; `ANGEL_SELF_MODEL=0` disables injection.

The source-map projection lives in `scripts/runtime/self-map.mjs` and
`lib/self-map/report.mjs`. Node.js is required for a map or outline; changes to
either file apply on the next request and invalidate the cached self-work context.
Each request uses a private snapshot of both files. Complete saves to both files
before making a request: without a shared publication manifest, a stable pause
between the two saves cannot be detected. The cockpit discovers and reads
the source files, keeps the safety guidance and gate, and sends one bounded
batch to the projection. It reports a missing or failing module explicitly.
Source files are limited to 2 MiB each and 12 MiB in total, with at most 1,024
modules per map. The serialized request is capped at 16 MiB; the worker has a
10-second deadline and a 1 MiB output cap; each module file is capped at 64 KiB.
Unreadable or non-UTF-8 modules have an empty description and outline in the
map. An oversized input or a failed projection makes the map unavailable; the
self-work guidance still appears alongside the error.

The `/self` workflow uses an isolated worktree. Its integration gate requires
both the build and tests to pass. Inspect the resulting diff and evidence before
integrating a change.

For the benchmark-driven messaging and routing campaign, use the
[speed-loop handoff](https://github.com/newjordan/angelX/blob/main/docs/SPEED_LOOP.md). It includes the starting command, current
evidence, experiment sequence and acceptance criteria.

Implementation: [self_map](../cockpit/src/agent/tools/self_model.rs),
[self-work controller](../cockpit/src/drive/self_loop.rs).
