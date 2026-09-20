# Working on the cockpit source

The `self_map` tool reads the active cockpit crate and returns its module map.
Pass `module: "tools/nav"` for a module outline. In an interactive self-work
session, `write: true` also writes `SELF.md` in that crate. Headless diagnostics
are read-only and require an explicit source pin.

`ANGEL_SELF_SRC` selects the cockpit crate directory. Without a pin, the source
locator checks the build-time crate path and nearby checkout. Source context is
injected only for work on the cockpit; `ANGEL_SELF_MODEL=0` disables injection.

The `/self` workflow uses an isolated worktree. Its integration gate requires
both the build and tests to pass. Inspect the resulting diff and evidence before
integrating a change.

Implementation: [self_map](../cockpit/src/tools/self_model.rs),
[self-work controller](../cockpit/src/self_loop.rs).
