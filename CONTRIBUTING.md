# Contributing to Angel

## The dream

A place for magic folk, wizards, and knights of the round sphere to come
together, expand the microcosm together and cast the spell of light and truth
upon the macrocosms.

## Operable models, not confined ones

angelX exists to make models operable: long, deep research and competition
work at machine speed, with the whole rig at their disposal. Every seat and
worker gets the full developer posture: the GPU device nodes, the network,
scratch space, background jobs (`proc_run`) and every registered tool.

- Do not add read-only modes, per-call write scopes, confined worker shells,
  tool allowlists, or instructions that tell a model to ask for a scope change.
  Where one shows up in a failing run, remove it.
- Trimming tool schemas to save tokens is for short bounded edits only. A
  competition or an autonomous loop always sees the research tools: web
  search and fetch, papers, code hosts, symbol navigation and the GPU.
- The only boundaries kept are the ones that stop work corrupting itself: an
  experiment copy must not alias the live tree it came from, and one project's
  workspace is not a write root for its siblings.
- Anyone who wants a locked-down sandbox should use a different program.


## Setup

- Use Linux x86_64 and the Rust toolchain pinned in `rust-toolchain.toml`.
- Start with `ANGEL_VIDEO=0 bin/angelX`; this includes portraits and images.
- Source launches default to the incremental `fast` profile: cockpit and Delve
  use lighter optimization, dependencies stay optimized, and the native and
  nested browser builds skip cross-crate LTO. The first build populates a
  separate cache; keep using the same checkout, profile and features to reuse it.
- `ANGEL_NO_BUILD=1 bin/angelX` reopens the existing fast binary without building;
  source edits will not appear until the next build. To run the existing release
  binary, use `ANGEL_FAST=0 ANGEL_NO_BUILD=1 bin/angelX`.
- Use `ANGEL_FAST=0 bin/angelX` for a release build and `ANGEL_DEV=1 bin/angelX`
  for unoptimized debugging. Prebuilt installs use their shipped release binary.
- Fast/debug images are source-unbound by default: changing a live module's Git
  tree does not change baked Rust metadata and trigger a native rebuild. Use
  `ANGEL_BIND_SOURCE=1` when a development image needs committed source identity;
  release builds continue to bind clean source by default.
- For cockpit composition, game tuning, tool-routing policy, reports, guardian
  presentation and next-turn guidance, edit
  `cockpit/live/` with the host running. Use `/reload status` to inspect the active
  generation. The [live-module contract](cockpit/README.md#live-application-modules)
  explains the directory layout, dependencies and failure behavior.
- Test the isolated loader with `npm run test:live`; this does not compile the
  cockpit, graphics stack, V8 archive or embedded browser page.
- Use Node.js for release-tool tests. The Rust trace-validator tests also need
  Python 3 with `jsonschema` available in the selected Python environment.
- Keep fixes focused and preserve reproducible failure evidence.

```sh
# Replace the filter with the affected module or test.
bash scripts/check/check-cockpit-fast.sh <test-filter> --test-threads=1

# Broader no-video coverage and public release-tool contracts.
bash scripts/check/check-cockpit-fast.sh --test-threads=1
npm run test:release
cargo fmt --manifest-path cockpit/Cargo.toml --check
  bash scripts/check/check-legacy-terminal-boundary.sh
  npm run check:duplicates
  npm run check:research-embed
```

- The fast runner starts test processes with both YOLO authority flags off;
  individual fixtures may opt in explicitly. Do not time builds as model work.
- Development builds and tests use the same incremental `fast` profile as
  source launches, rather than creating another debug target tree. The gate
  omits video/window features, so Cargo may retain separate feature artifacts
  within that profile; release qualification remains an explicit `--release`.
- The development gate keeps a private copy of the test executable and its CLI
  and sandbox-helper siblings, so concurrent builds can replace shared target
  files while tests run. It removes its own temporary directory on exit;
  `TMPDIR` selects the parent and must have room for the copied test images.
- A no-video test pass does not qualify video support or an optimized release.
  Follow the [release guide](docs/release-evidence.md) for those checks.
- Keep credentials, personal configuration, raw sessions and machine-specific
  paths out of patches. See [SECURITY.md](SECURITY.md) for security reports.
- Benchmark claims need pinned source, model and effort, explicit run settings,
  retained failures and an independent evaluator. A completed answer is not
  itself proof of success.

## Pull requests

Fork the repository, branch off `main`, and open the pull request against `main`.

The public product website is [https://omni-space.tech](https://omni-space.tech).
Its source is `website/`. Preview it with `python3 -m http.server` from that
directory. Publishing it, and any later change to the older hostname that still
serves a previous deployment, is the manual cutover in
[docs/site-rollout.md](docs/site-rollout.md).

[https://angelx.dev](https://angelx.dev) keeps serving that previous deployment
until the cutover is approved. It is not the product website. The Vercel
project linked to this repository (`angelx`) has its root at the repository
root, and [`vercel.json`](vercel.json) skips that project's build
(`ignoreCommand` exits 0) so a merge does not replace the previous deployment.
Do not point that project at `website/` and do not remove the skip until that
cutover is explicitly approved.

- Keep each patch focused.
- Run the check scripts listed in [Setup](#setup) before opening the pull request.
- Expect review from [@newjordan](https://github.com/newjordan). Jordan reviews and merges; there are no other human maintainers yet.
