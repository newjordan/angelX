# Source release evidence

angel0 produces a deterministic, checksummed source archive of the ordinary
terminal cockpit and its headless `angel --task-json` runtime:

```bash
npm run release:evidence -- --out .angel/release/<receipt-dir>
```

`--out` is optional and defaults to `.angel/release/`; a repository-local output
must stay under `.angel/` (ignored), and an absolute path outside the checkout
is also accepted. The command writes an untracked release set:

- `angel0-source-<version>-<commit>.tar` — normalized GNU tar source archive;
- `*.tar.sha256` — archive checksum sidecar;
- `*.manifest.json` — source, dependency, toolchain, file, and artifact
  evidence (`schema: angel0-source-release-evidence/v1`,
  `scope: ordinary-terminal-cockpit-source`);
- `*.manifest.json.sha256` — manifest-file checksum sidecar.

The archive is a source distribution, not a relocatable prebuilt binary. After
extraction, `bin/angel0` builds `cockpit/target/release/angel` and
`angel-sandbox` from the included cockpit source and the two vendored path
dependencies, `vendor/dotmax` and the patched `vendor/ureq`. Local credentials
and `.angel.env` are never included.

## What the archive contains

The inventory is an explicit allowlist in [release-evidence.mjs](../scripts/release-evidence.mjs):

- the terminal and headless runtime, launcher and vendored Rust dependencies;
- embedded skills, personas, fixtures, telemetry tables and Sloptomizer modules;
- runtime artwork, graph presets and the optional portal renderer;
- repository-memory, Habitsmith, Conductor, Still, machine-queue and prompt-compression helpers;
- current usage and developer guides, licenses, notices and verification tools.

Provider parser fixtures live under `cockpit/fixtures/usage/`.
Benchmark task bundles, private experiments, credentials, session histories and
operator records are excluded. Tests check the compile-time include inventory, JavaScript helper import/dispatch
closure, and runtime asset packaging. [Worker contracts](WORKERS.md) document
one-tick execution, state paths, limits and evaluator configuration.

## What the gate proves

The gate fails closed unless every release input is a committed regular file
from the allowlist. It rejects dirty or untracked inputs inside that scope,
skip-worktree and assume-unchanged index flags, symbolic links, path
traversal, the `off-limits/` namespace, non-registry Cargo sources, missing
Cargo checksums or npm sha512 integrity, missing dependency license evidence,
and unavailable supported-target Cargo packages. It inventories the inputs
again after archiving so a concurrent source change cannot silently produce
promotable evidence.

Public-content hygiene runs over every text entry: production source and
documentation may not carry a per-user absolute home path, a CGNAT literal,
a live tailnet DNS name, a machine-specific hardware hostname, or a host mount
coordinate. CGNAT values are tolerated only in test and fixture paths so
private-network behavior stays testable. Operator-specific values such as the
fleet `overwatch` binary path and the Spark peer's tailnet/Hydra host labels
are therefore runtime inputs (`ANGEL_OVERWATCH_CMD`, `$HOME`, and
`ANGEL_SPARK_HOST`), never source literals. Operators supply them through the
gitignored repo-local `.angel.env` that `bin/angel0` sources (see
`cockpit/docs/ENV.md`).

The manifest binds:

- Git commit and tree identity, and a `release_inputs_dirty: false` claim;
- every archive path, Git mode, byte length, and SHA-256, plus a canonical
  entries digest and a semantic manifest digest;
- the tar byte length and SHA-256;
- the cockpit source identity (`cockpit_source_sha256`) that the built binary
  must report back through `angel --build-info --json`;
- `package-lock.json` v3 identity and sha512 integrity for every registry
  package;
- `Cargo.lock` identity and checksums for every locked registry package, and
  target-filtered, locked, offline Cargo metadata for
  `x86_64-unknown-linux-gnu`;
- license metadata or a hashed license file for every resolved dependency,
  with the product root recorded as MIT and the root `LICENSE` included in the
  checksummed source inventory; third-party licenses and notices remain intact;
- the pinned Rust toolchain and the actual Node, Git, Cargo, rustc, clippy,
  rustfmt, and GNU tar versions used;
- the supply-chain policy: the pinned `rusty_v8` prebuilt static library, the
  digest-pinned clean-builder image, the digest-pinned OSV-Scanner image, the
  three packaged lockfiles with their hashes and package counts, and the
  time-bounded advisory exception policy.

Regular-file metadata is normalized from Git rather than copied from the host
checkout: non-executable files are `0644`, executables are `0755`, uid/gid and
mtime are zero, and only regular USTAR entries are permitted.

## Verification commands

Each verifier prints its own usage string when called without arguments:

```text
verify-release-evidence.mjs MANIFEST --v8-archive PATH [--receipt PATH]
verify-release-container.mjs MANIFEST --v8-archive PATH [--receipt PATH]
verify-release-advisories.mjs MANIFEST [--receipt PATH] [--raw PATH]
verify-release-install.mjs MANIFEST [--container-receipt PATH] [--binary PATH] [--receipt PATH]
```

They are exposed as `npm run release:verify`, `release:verify:container`,
`release:verify:advisories`, and `release:verify:install`. Every receipt is
written beside the manifest with a `.sha256` sidecar, and every receipt states
its narrower negative claims explicitly.

### Fresh-extraction offline build (`release:verify`)

```bash
npm run release:verify -- \
  .angel/release/<receipt-dir>/angel0-source-<version>-<commit>.manifest.json \
  --v8-archive /path/to/librusty_v8.a
```

The verifier parses USTAR headers itself before writing anything: header
checksums, path normalization, regular-file type, exact `0644`/`0755` modes,
zero uid/gid/mtime, zero padding and trailer bytes, and every manifest size and
SHA-256, then the archive's cockpit source identity against the manifest. It
writes a fresh temporary tree using exclusive creates and builds it with
`cargo build --bin angel --no-default-features --release --locked --offline`
inside Bubblewrap (`/usr/bin/bwrap`). The build receives only the extracted
source, read-only host Cargo executable/registry/git caches (not Cargo
credentials), the read-only Rust toolchain, and the checksum-pinned `rusty_v8`
static library at `--v8-archive`; its network namespace is unshared and
ambient host environment variables are absent. The resulting binary must emit
`angel-build-info/v1` with the packaged cockpit source identity and the
required runner capabilities and the manifest-bound resource identity, then pass
the networkless runner-contract smoke.
The checksummed `*.verification.json` receipt records the build log digest,
binary identity, isolation, reused host inputs, and the negative clean-host
claim.

The `v8` crate's build script downloads its prebuilt archive even under
`--offline`, so `release/supply-chain-policy.json` pins the crate version,
upstream URL, decompressed byte count, and SHA-256 of that library. The
verifier fails rather than enabling network when the supplied archive is
absent or different. A matching copy is normally present in a developer
checkout as `cockpit/target/release/gn_out/obj/librusty_v8.a`.

### Clean-container build (`release:verify:container`)

```bash
docker pull docker.io/library/rust@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1
docker build --pull=false --network=bridge \
  --file release/Dockerfile.clean-builder \
  --tag angel0-clean-builder:rust-1.95.0-bookworm-v1 \
  release/

npm run release:verify:container -- \
  .angel/release/<receipt-dir>/angel0-source-<version>-<commit>.manifest.json \
  --v8-archive /path/to/librusty_v8.a
```

Requires a Docker daemon the invoking user can reach. The policy pins the
expected image ID, the official Rust base digest, the Dockerfile SHA-256, the
Rust/platform identity, and all nine direct native package versions. The
verifier refuses to pull or rebuild implicitly and accepts the local candidate
only when every recorded fact matches. The recorded image ID comes from the
previous build of this byte-identical Dockerfile on the prior evidence host; it
has not yet been rebuilt on an angel0 Docker host, and the Dockerfile names
Debian packages without version constraints, so rebuilding is not by itself a
guarantee of reproducing that ID. When it differs, re-pin the policy from the
inspected image rather than weakening the check.

Each run starts with an empty Cargo home: a bridge-networked container runs
`cargo fetch --locked` only, a `--network none` container runs the offline
release build with the read-only pinned V8 library, and a third read-only
container probes the binary. The gate publishes that executable beside the
manifest as `*.cockpit-linux-x86_64` with a checksum sidecar.

### Isolated prefix install and recovery (`release:verify:install`)

```bash
npm run release:verify:install -- \
  .angel/release/<receipt-dir>/angel0-source-<version>-<commit>.manifest.json
```

Requires the clean-container receipt and published executable from the
previous step and the same Docker daemon. It installs into a fresh temporary
prefix, launches the binary in a networkless read-only container that mounts
only that prefix and home. The prefix includes the manifest-verified source and
artwork in a versioned resource bundle; the installed binary must resolve its
helpers there without the build checkout. The verifier replaces the package through a rollback-journaled
transaction, injects one interrupted replacement, and proves the checksummed
prior binary and operator-state sentinel are restored. It is a same-artifact
replacement and isolated recovery proof, not cross-version migration,
package-manager integration, or a physically clean host.

### Dependency advisories (`release:verify:advisories`)

```bash
docker pull ghcr.io/google/osv-scanner@sha256:64e86bec6df2466feea5137fc7c78fb3b7c21ec077f014d7130f64810e50676b

npm run release:verify:advisories -- \
  .angel/release/<receipt-dir>/angel0-source-<version>-<commit>.manifest.json
```

Requires Docker and outbound access to OSV.dev. The verifier freshly extracts
the release, copies only the exact `cockpit/Cargo.lock`,
`cockpit/portal-renderer/Cargo.lock`, and `package-lock.json` into the scanner
mount, validates OSV-Scanner `2.3.8` at commit
`408fcd6f8707999a29e7ba45e15809764cf24f67`, and compares OSV's returned package
counts with the manifest's per-lockfile counts. Python is explicitly
`not-applicable` for this scope. The point-in-time results are written to
checksummed `*.advisory-osv.json` and `*.advisory-verification.json` receipts;
the gate fails on missing results, an unknown finding, or a stale or expired
exception. The policy currently carries two accepted unmaintained transitive
records, `bincode 1.3.3` (RUSTSEC-2025-0141) and `paste 1.0.15`
(RUSTSEC-2024-0436), expiring 2026-10-18. An exception is evidence of accepted
residual risk, not evidence that an advisory is absent, and without a retained
advisory receipt this list is the accepted exception scope, not proof of the
current OSV result.

## Supported evidence host

Linux x86_64 with Node 20.19+, 22.13+, or 24+, Rust 1.95.0 with the pinned
`clippy` and `rustfmt` components (`rust-toolchain.toml`), Git, GNU tar, and
Bubblewrap for `release:verify`. Cargo dependency inspection is offline; a
missing cached dependency fails the gate instead of fetching. The archive can
be re-checked with:

```bash
cd .angel/release/<receipt-dir>
sha256sum -c angel0-source-*.tar.sha256
sha256sum -c angel0-source-*.manifest.json.sha256
```

## Claims this evidence does not make

The claim remains intentionally narrower: local release provenance,
source-package consumption, and an optional digest-pinned clean-container
build and container-scoped install lifecycle. A separate optional advisory
command can add a checksummed point-in-time OSV receipt over every packaged
lockfile; the source manifest and its exception policy are not themselves
evidence that this scan was run. It is not yet a physically separate
clean-host install, an advisory-clean graph, or an externally anchored
vulnerability attestation.

Fresh extraction proves the package builds without consulting the source
checkout, but it reuses the host's Cargo cache, Rust toolchain, and pinned V8
library. The clean-container gate removes those host inputs but still shares
the host kernel, Docker daemon, locally built image, and pinned V8 library.
Neither proves binary reproducibility across systems or a signed attestation.
The manifest and checksums prove integrity, not publisher authenticity; obtain
both through a trusted distribution or signing channel.
