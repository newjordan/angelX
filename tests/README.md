# Tests

Project test sources live here. Rust unit tests remain private submodules behind
`#[cfg(test)]`; they are excluded from ordinary application builds.

- `cockpit/`: Rust unit tests grouped by subsystem, with executable-level checks
  in `cockpit/integration/`.
- `scripts/`: Node.js tests for workers, launchers and release packaging.
- `python/`: Python validation tests.

Run `npm test` for the packaged script suites and `npm run test:cockpit` for
the cockpit unit tests. `npm run test:cockpit-pty` exercises the terminal UI.
Cargo integration targets are declared in `cockpit/Cargo.toml`.

Subprocess fixture entry points are marked ignored and invoked explicitly by
their ordinary parent tests. The parent verifies that its filter selects a real
fixture; an empty subprocess run is not a passing check. Live model tests are
also ignored by default and require their documented opt-in configuration.
