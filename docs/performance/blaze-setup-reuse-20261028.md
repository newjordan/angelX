# Blaze repeat setup: verified reuse instead of reprovisioning

Date: 2026-10-28. Scope: checkout-local **developer setup**, not application
startup, cold boot, a shipping release, or a full-application Blaze qualification.

## Installed behavior

```sh
python3 scripts/runtime/install-blaze.py           # install once; verify/reuse thereafter
python3 scripts/runtime/install-blaze.py --offline # never fetch or provision
npm run test:setup                               # bounded installer regressions
```

The project pins Blaze commit
`b383b0bcf5da33816809b00a37e1c36a0e761efb` and
`nightly-2026-10-27`. The former installer unconditionally invoked
`rustup toolchain install` and rebuilt Blaze even when its complete pinned
installation was already available. With Rustup's distribution and update URLs
pointed at closed **loopback** port 9, that baseline failed at channel sync,
although this machine already had the four executables, matching Rust nightly,
rust-src and wasm target. No external download was needed or permitted by the
control. This is an availability/repeated-work defect, not a measured online
baseline setup duration.

The updated `scripts/runtime/install-blaze.py`:

- Validates a bounded receipt, exact repository/revision/toolchain, precisely the
  four expected executable names, executable regular-file types and every image's
  SHA-256. Symlinks, missing files, malformed receipts and corrupted bytes fail
  before selection or a rebuild. The receipt detects local accidental corruption;
  it is **not** a signed release or a defense against a malicious local writer.
- Reuses a valid generation instead of fetching, installing or compiling.
  An already-correct selector is not rewritten. If the source checkout exists,
  its pin, toolchain and clean status are still checked, including untracked
  files even under `status.showUntrackedFiles=no`. A removed source checkout can
  be tolerated when an installed pinned generation is valid.
- Locally probes Rust with `RUSTUP_AUTO_INSTALL=0`, validates registered rust-src,
  host std and wasm std component inventories, then checks source/wasm core,
  alloc and std sentinels. These are installed-file presence checks, **not**
  cryptographic attestation of the Rust sysroot. Normal mode provisions only
  missing/incomplete components; offline mode fails with an actionable message.
- Uses absolute pinned Cargo/rustc, an explicit host target/output directory,
  empty wrapper overrides and empty encoded rustflags for a necessary initial
  compiler build. This overrides the host application's Cargo config/environment
  wrappers, target-directory redirects and target-specific rustflags. Offline
  compiler builds also pass `--offline`; dependencies must already be cached.
- Serializes cooperating installers with a checkout-local directory lock. A
  new generation is staged completely before rename; selection changes only
  after validation. Existing corrupt/live generations are never repaired in
  place. Failure leaves the previous selector intact. A killed installer can
  leave the lock behind: confirm no installer is running before manually
  removing `cockpit/target/tools/.blaze-install.lock`. The lock does not protect
  against arbitrary writers ignoring the installer protocol.

Global Cargo config and stable Rust are not changed. The application launcher
and its freshness decisions are untouched. Blaze is still an explicit,
experimental backend; normal `auto` uses qualified Cargo.

## Final-source measurements

Probe:

```sh
python3 scripts/perf/measure-blaze-setup.py \
  reports/performance/blaze-setup-iteration13/final-measurements.json
```

Five interleaved rounds, normal/offline ordering reversed on alternate rounds.
All runs used the real installed compiler images and sysroot, with download
servers directed to loopback. A guard records every installer child command and
rejects any `fetch`, `install`, `build`, `update` or `clone` request before running
it. It still executes the actual Git/Rust probes. Durations include Python,
audit overhead and log writing, and are warm shared-workstation observations.

| Repeat setup | Samples | Median | Min–max |
|---|---:|---:|---:|
| Normal mode, complete local install | 5 | **178.63 ms** | 166.53–362.82 ms |
| Explicit offline mode | 5 | **184.43 ms** | 166.19–266.60 ms |

All ten returned verified reuse. Hash, inode, size and nanosecond mtime of all
four images, their receipt and the selector were unchanged. No application
restart, compiler replacement or source modification was required. There is no
online baseline ratio or claimed startup speedup.

### Real Cargo configuration control

The same probe creates an isolated stdlib-only fixture. Deliberately inherited
Cargo config/environment specify two wrappers that exit 91, two wrong target
directories, an invalid host rustflag and a wasm default target. Without the
installer's settings, actual Cargo exits 101 and the wrapper tripwire fires.

Using **production `build_settings()`**, actual pinned Cargo builds the host
binary from original, semantically edited and restored source, and all three
executions print the expected distinct output. No wrapper is invoked, neither
wrong target directory exists, and the original/restored source and executable
hashes match. This validates Cargo precedence and output lookup, not a full
Blaze compiler rebuild or application-level edit speedup.

## Regression evidence and limits

`npm run test:setup` passes **19 tests** using the repository's existing Python
unittest discovery convention. It is also wired into `test:hygiene`, so normal
`npm test` reaches it. The broader `npm test` suite was not rerun for this
installer-only change.

Cases include valid reuse, corruption/wrong receipt/extra images/missing or
non-executable images, symlink rejection, offline missing components/source,
selector recovery, mocked initial build and failure preservation, wrapper and
target overrides, missing alloc and host files, installer lock contention/error
cleanup, source pin/toolchain mismatch and a **real Git repository** configured
to hide untracked files. The original installer failed the initial 16-test
regression set; final set and actual local setup controls pass. The fixture
checks are not substitutes for a clean online first-install measurement.

Independent review prompted component-inventory checks, configuration overrides,
Git untracked policy and cooperating-installer serialization. No paid provider
or Muse calls were made. The independent review receipt and response are recorded
in the iteration report.

Evidence directory: `reports/performance/blaze-setup-iteration13/`.
Important files: `baseline.py`, `baseline-offline.log`, `red-tests.log`,
`final-gate-tests.log`, `final-measurements.json`, adjacent per-step logs,
`source-identities.json`. Earlier intermediate measurement files remain available;
use `final-measurements.json` for the values above.

## Newly reproduced Blaze PR opportunity

See [the isolated upstream publication reduction](blaze-install-publication-20261028.md).
It executes the pinned upstream installer against an inert local archive, not
the real installed compiler. It demonstrates both repeat metadata churn and a
mixed executable generation after an injected final-rename failure. The local
installer change above is not an upstream patch or release; no PR was submitted.

## Remaining performance boundary

One warm launcher xtrace measured 254.25 ms total, with a 204.98 ms gap around
Cargo's freshness check and 16.72 ms running `--version`. That diagnostic is not
an interleaved benchmark. Avoid replacing the source check with a timestamp or
stale executable shortcut merely to win a warm-launch number. The next productive
startup experiment is a Cargo phase/freshness cost attribution with matched
source identity, followed by a measured candidate that preserves current-source
behavior. Full-application Blaze remains blocked by the previously documented
correctness/sandbox findings.
