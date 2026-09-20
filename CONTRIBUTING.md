# Contributing to Angel

- Use Linux x86_64 and the Rust toolchain pinned in `rust-toolchain.toml`.
- Start with `ANGEL_VIDEO=0 bin/angel0`; this includes portraits and images.
- Use Node.js for release-tool tests. The Rust trace-validator tests also need
  Python 3 with `jsonschema` available in the selected Python environment.
- Keep fixes focused and preserve reproducible failure evidence.

```sh
# Replace the filter with the affected module or test.
bash scripts/check-cockpit-fast.sh <test-filter> --test-threads=1

# Broader no-video coverage and public release-tool contracts.
bash scripts/check-cockpit-fast.sh --test-threads=1
npm run test:release
cargo fmt --manifest-path cockpit/Cargo.toml --check
bash scripts/check-legacy-terminal-boundary.sh
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
