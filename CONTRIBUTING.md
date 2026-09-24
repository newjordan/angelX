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
- A no-video test pass does not qualify video support or an optimized release.
  Follow the [release guide](docs/release-evidence.md) for those checks.
- Keep credentials, personal configuration, raw sessions and machine-specific
  paths out of patches. See [SECURITY.md](SECURITY.md) for security reports.
- Benchmark claims need pinned source, model and effort, explicit run settings,
  retained failures and an independent evaluator. A completed answer is not
  itself proof of success.
