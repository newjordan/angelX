# Blaze installer PR opportunity: publish an executable generation, not four files

Date: 2026-10-28. Upstream inspected and executed:
`indexable-inc/blaze` at `b383b0bcf5da33816809b00a37e1c36a0e761efb`.
Status: **reproduced local reduction; proposed upstream work, not an upstream fix
or submitted PR**.

## Evidence in the implementing source

Pinned `install.sh`:

- Lines 11–12 explicitly describe repeat installation as installing the newest
  release over the old one. This means "repeat latest updates" is intentional;
  a reuse improvement must preserve that behavior unless a version is pinned.
- Lines 136–142 accept `BLAZE_ARCHIVE` and use a local archive.
- Lines 212–224 implement `put()` as `cp`, `chmod`, `mv` for each executable in a
  loop. Rename protects an individual running executable, but not consistency
  of the four programs together.
- Lines 239–248 invoke toolchain setup unless `--no-toolchain` is selected.

These observations are from the script itself, not generated fixtures or docs.

## Isolated actual-script reproduction

```sh
python3 scripts/perf/repro-blaze-install-publication.py \
  reports/performance/blaze-setup-iteration13/upstream-install-publication.json
```

The probe constructs a tiny local tar archive with four **inert shell scripts**
named `blaze`, `cargo-blaze`, `ra-jit`, `ra-jit-rustc`. It runs the unmodified
pinned upstream script with `BLAZE_ARCHIVE`, a temporary `CARGO_HOME` and
`--no-toolchain`. Tripwire curl/wget programs reject any attempted download.
The real checkout-local or global compiler files are never replaced or executed.
Only the isolated release-publication path is under test; this does not measure
Blaze compilation or daemon behavior.

Results:

1. First install and byte-identical repeat both exit 0.
2. All four repeated bytes match, but each executable's inode/mtime identity
   changes. Repeat pinned installation has avoidable metadata/publication churn.
3. Reset the temporary installed scripts to print `old-generation`. Inject a
   failure only in `mv` of `ra-jit-rustc`, delegating the other moves to real `mv`.
4. Installer exits **93**. Its installed directory contains:

| Program | Installed contents after failure |
|---|---|
| `blaze` | new-generation |
| `cargo-blaze` | new-generation |
| `ra-jit` | new-generation |
| `ra-jit-rustc` | **old-generation** |

5. Network tripwire file is absent. No release download, Rust install, real
   compiler replacement or live provider call occurred. Temporary files are
   removed. Source, probe and archive hashes and per-step logs are retained.

This is deterministic fault injection in the actual upstream shell script, not
an observed production incident or evidence of a runtime ABI failure. It proves
that a failed install can leave a mixed set of executable contents; whether any
particular released pair is compatible was not tested.

## Proposed PR scope and acceptance

**Primary PR: transactional release-generation installation.** Stage and validate
all required images; serialize competing installers; publish/select a complete
versioned generation only after all copies and checks succeed. Keep running
processes on the previous generation. Preserve upstream platform and old-release
compatibility, including cargo-blaze generation for releases predating the
rename. Account for sibling-executable lookup before choosing selectors or a
launcher layout; a single file-level atomic rename is not an all-images commit.

Candidate acceptance cases:

- Inject failure at copy/chmod/rename of **each** image: old complete generation
  remains selected, no mixed executable set.
- Two cooperating installs of same/different versions: one complete winner and
  no shared fixed `.blaze-new` temporary-name collision.
- Already-running old frontend/daemon survives installation; new invocations
  consistently use one complete generation.
- Pinned byte-identical repeat does not rewrite installed images. Latest/update
  mode still performs its intended release-resolution checks.
- Local archive path remains usable offline, and toolchain setup behavior remains
  explicit. Preserve checksum behavior; a local receipt is not a signature.
- Test Linux and macOS behavior, interrupted install/cleanup and older release
  compatibility before claiming a portable upstream fix.

**Secondary feature:** explicit verified pinned/offline reuse, with clear
identity receipts and an option distinct from "install latest". The angelX local
installer now demonstrates that workflow at roughly 0.18 s warm repeat setup;
this is not a transferable upstream timing claim. The current upstream archive
path already avoids downloads and already offers `--no-toolchain`; those are not
missing features. The new opportunity is complete-generation publication plus
verified no-op semantics for a pinned, already-correct install.

## Retained sources and receipts

- `scripts/perf/repro-blaze-install-publication.py`
- `reports/performance/blaze-setup-iteration13/upstream-install-publication.json`
  and three adjacent log files
- `cockpit/target/tools/blaze-src/install.sh` (generated local pinned checkout;
  identity recorded in the JSON)
- [Local verified setup implementation and limits](blaze-setup-reuse-20261028.md)

No upstream checkout modification, patch application, PR publication or release
claim is made. This finding is separate from previously recorded Blaze
restore/replay, callback, embedding, feature and sandbox reductions.
