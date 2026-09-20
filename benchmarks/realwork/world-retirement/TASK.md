# Real task: retire the outdoor sprite world

Remove angel0's older outdoor PNG/sprite/top-down game-world renderer and its
exclusive code, assets, configuration, and active documentation. This is actual
product cleanup, not a demonstration patch. Keep the ordinary terminal cockpit,
the true-3D Dotmax pixel world, and ALL room/location plate graphics working.
The operator explicitly wants Dotmax outdoors and the room plates preserved.

Requirements:

1. Retire the selectable top-down outdoor renderer completely. Merely changing
   its default, hiding its menu option, or leaving the old renderer in another
   directory does not complete the task. Ordinary world startup and return from
   a room must show Dotmax. Remove or explicitly redirect obsolete top-down
   selections to the retained view; do not silently resurrect the old style.
2. Remove the exclusive outdoor tile/sprite compositor, building PNG kit, backed
   overview map, and unneeded outdoor atlases/source PNGs. Trace references first.
   Move shared navigation/geometry into an appropriately named retained module
   when Dotmax needs it. Preserve real functionality instead of stubbing calls.
3. Keep room plates and their rendering/entry/exit paths. In particular preserve
   every file under `cockpit/assets/realm/ambient/` and
   `cockpit/assets/realm/v2/interiors/`, all `scriptorium-baseline*` files, and
   shared palettes. Preserve generic image/report viewing, agent portraits,
   model/thinking controls, and world navigation. PNG files are not globally
   forbidden; unrelated image features are outside the removal scope.
4. Update affected tests and current documentation for the retained behavior.
   Retired renderer-specific tests can be removed with their implementation.
   Do not delete unrelated tests or weaken validation to manufacture a pass.
   Add a focused regression for Dotmax outdoors and room-plate entry/exit.
5. Run the quarantine boundary check, Rust formatting, and a no-video compilation
   check including tests. Run focused retained-world tests when feasible. State
   exactly which checks passed/failed and what remains unfinished.

Start with `AGENTS.md` and `.mex/ROUTER.md`. Historical memory describing the
top-down style is superseded by this task. Neither subtree under `off-limits/`
may be read, searched, listed, copied, changed, or used. The task does not grant
access there. The benchmark checkout deliberately excludes that directory.

The must-remove paths and protected file hashes in sibling `acceptance.json`
are part of the public task contract. They are minimum gates, not an exhaustive
substitute for finding the old renderer's exclusive leftovers. Do not modify
the benchmark, scope instructions, boundary checker, compiler pin, or dependency
lockfiles. No new dependencies are required.

Work only in the supplied checkout. Do not inspect another competitor's work,
the controller's original checkout, or benchmark evidence directories. Use only
your assigned model, without delegates, external agents, or network research.
The local tools and warmed Cargo dependency cache are available. Do not build a
release binary, install/deploy changes, alter system configuration, or push.
The controller preserves your diff/commit and handles review and publication.

Suggested validation (time it honestly):

```sh
bash scripts/check-legacy-terminal-boundary.sh
cargo fmt --manifest-path cockpit/Cargo.toml --check
cargo check --locked --manifest-path cockpit/Cargo.toml --no-default-features --tests
```

Budget: 15 minutes, at most 64 model requests, low reasoning, 8,192 output
tokens per request. Spend that budget implementing and checking the cleanup.
Finish with a concise account of changes, verification, and remaining issues.
