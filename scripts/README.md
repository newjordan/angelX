# scripts/

Host-side belt around the cockpit. Not the TUI.

For the full checkout, start with the [documentation index](../docs/README.md)
and [operations schematic](../docs/architecture/operations.md). The
[lean-host proposal](../docs/architecture/lean-host.md) distinguishes existing
runtime workers from proposed hot-reloadable JavaScript modules.

| Directory | What |
| --- | --- |
| `release/` | Public archive + verify gates |
| `check/` | Quality / boundary / connection gates |
| `runtime/` | Host workers/helpers: shipped dossier, Habitsmith, pxpipe and store helpers; explicitly marked checkout-only tools below |
| `bench/` | Benchmark runners, model/harness experiments, and result analysis |
| `release-report/` | Release-report rendering helpers |
| `readme-charts/` | Documentation chart production |

Shared runtime libraries live in [`lib/`](../lib/); script tests live in
[`tests/scripts/`](../tests/scripts/). Keep new runtime entry points in
`runtime/`, checks in `check/`, and measurement tooling in `bench/`, rather than
adding unrelated root-level scripts. Moving a shipped helper also requires
updating its consumers and the [release inventory](release/release-evidence.mjs).

The dossier worker demonstrates the boundary: [`lib/dossier/core.mjs`](../lib/dossier/core.mjs)
owns mining and projections; [`runtime/repo-dossier.mjs`](runtime/repo-dossier.mjs)
owns CLI paths, store caps, redaction and publication. Its old exports remain
compatible. Shared scoring and authored-write rules live under `lib/research/`
and `lib/evidence/`; pure libraries do not import worker entry points.

Habitsmith follows the same split: [`lib/habits/core.mjs`](../lib/habits/core.mjs)
owns workflow rules and draft rendering, while
[`runtime/habitsmith.mjs`](runtime/habitsmith.mjs) writes proposed drafts and
persists budgets. Shared [`probe rules`](../lib/dossier/probes.mjs) no longer
require importing the scheduled dossier worker.

The **checkout-only** [`settlement-exhibits` publisher](runtime/settlement-exhibits.mjs)
follows that split: [`lib/settlement/exhibits.mjs`](../lib/settlement/exhibits.mjs)
validates manifest data, while the CLI handles bounded source reads, credential
checks and private no-overwrite publication. Native Rust still owns saved-loop
admission and the world; publishing is not verification. See the
[settlement workflow](../docs/architecture/connected-settlement.md#optional-checkout-publisher-linux).
This helper requires Linux and Python 3 and is not a shipped runtime dependency.

Conductor / Still / Cut / Reflex experiment CLIs, machine-queue, and research cohorts are operator-private. They are not this tree.
