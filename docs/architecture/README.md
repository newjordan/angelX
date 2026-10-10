# Architecture and repository map

[Documentation index](../README.md) · [Operations schematic](operations.md) ·
[Lean host and hotloading proposal](lean-host.md) ·
[Connected loop settlement](connected-settlement.md) ·
[Stables and knights tournament](stables-tournament.md)

## Status and scope

The **current-state** map was traced against `97d64bc7` (`rig/0.1.94`). It is a
map of this checkout, not a claim to cover unpublished work in another worktree.
The operator's direction is a leaner system with more JavaScript and hotloading.
The **proposed** boundaries and folders below support that direction; they are
not an announcement that a general JavaScript module loader already exists.
Reconcile them with the active implementation before moving source.

The first migration step is navigability, not a mass rename. This directory owns
developer architecture; user guides remain at their existing paths. Runtime
entry-point paths and the native host remain stable. Incremental JS extraction
places dossier behavior and probe rules under `lib/dossier/`, workflow behavior
under `lib/habits/`, shared scoring under `lib/research/`, and authored-write rules
under `lib/evidence/`. See the
[implemented seam](lean-host.md#first-extraction-dossier-core) for its limits.

The subsequent [connected settlement](connected-settlement.md) slice links saved
loop work, the player hall, passive surveys and playable Delve admission. Real
local reports can become source-linked exhibits; this does not change the
proposed JS hotloading boundary or make the game a second loop controller.

These are checkout-only developer pages, not additions to the public release
allowlist. The [documentation index](../README.md) includes historical/internal
material; publishing it needs a separate review of its dependency closure.

## Read by question

| Question | Start here |
|---|---|
| What happens from launch to response? | [Operations: launch and turns](operations.md#launch-and-agent-turns) |
| How do teams and longer jobs run? | [Operations: orchestration](operations.md#orchestration-and-long-running-work) |
| What owns state, knowledge and the display? | [Operations: state and presentation](operations.md#state-knowledge-and-presentation) |
| How does loop research become a place I can visit? | [Connected settlement and exhibits](connected-settlement.md) |
| How do stables and local knight practice connect both worlds? | [Stables and knights tournament](stables-tournament.md) |
| What actually reloads today? | [Reload inventory](lean-host.md#what-exists-today) |
| What should stay native, and what can become JS? | [Proposed ownership](lean-host.md#proposed-ownership-boundary) |
| How should a live replacement behave? | [Proposed reload protocol](lean-host.md#proposed-reload-protocol) |
| How do I prove a migration preserved behavior? | [Acceptance matrix](lean-host.md#acceptance-matrix) |
| Where are the checks and shipping rules? | [Development and release](operations.md#development-and-release) |

## Current repository layout

| Path | Responsibility; placement rule |
|---|---|
| [`bin/`](../../bin/) and [`install.sh`](../../install.sh) | Public launch/install entry points; not application policy |
| [`cockpit/src/`](../../cockpit/src/) | Native host **and currently much of the application**; do not mistake all existing Rust for an irreducible kernel |
| [`cockpit/modules/`](../../cockpit/modules/) | TOML presentation-module manifests; not a directory of JS plugins |
| [`cockpit/assets/`](../../cockpit/assets/) | Art, audio and other shipped presentation resources |
| [`cockpit/skills/`](../../cockpit/skills/) and [`cockpit/personas/`](../../cockpit/personas/) | Agent playbooks and seat descriptions |
| [`scripts/runtime/`](../../scripts/runtime/) | Shipped JS/Python/shell worker entry points and support code |
| [`lib/`](../../lib/) | Reusable JS: dossier/probe and habit cores, evidence rules, causal graph/scoring and process leases; workers import libraries, not the reverse |
| [`scripts/check/`](../../scripts/check/) | Development checks and structural gates |
| [`scripts/bench/`](../../scripts/bench/) | Benchmark execution and analysis; not live application modules |
| [`scripts/release/`](../../scripts/release/) | Packaging, source evidence and installation verification |
| [`scripts/release-report/`](../../scripts/release-report/) and [`scripts/readme-charts/`](../../scripts/readme-charts/) | Reporting and documentation figures |
| [`tests/`](../../tests/) | Rust subsystem tests, Node script tests, Python validation |
| [`docs/`](../) | User guides, developer maps, release evidence and historical telemetry |
| [`release/`](../../release/) | Release metadata and configuration |
| [`vendor/`](../../vendor/) and [`third-party/`](../../third-party/) | Vendored code and attribution material; preserve provenance |

Generated output, local credentials, session state and experiment artifacts are
not module source. Do not promote an untracked local directory into the public
layout simply because it appears in a developer checkout.

### Native code: where to look today

These are navigation buckets, not a mechanically enforced dependency graph.
See the [native overview](../../cockpit/README.md#layout) for placement guidance.

| Bucket | Present responsibilities |
|---|---|
| [`app/`](../../cockpit/src/app/) | Application state, bootstrap, slash commands, turn/display coordination |
| [`agent/`](../../cockpit/src/agent/) | Provider routes, model calls, tool registry, execution, delegation, editing |
| [`drive/`](../../cockpit/src/drive/) | Goals, loops, campaigns, research controllers, co-op/game execution |
| [`knowledge/`](../../cockpit/src/knowledge/) | Sessions, ledgers, recall, Atlas, dossier and evidence |
| [`platform/`](../../cockpit/src/platform/) | Processes, workspace identity, resource lookup, policy, surface lifecycle |
| [`ui/`](../../cockpit/src/ui/) | Input, terminal rendering, panes, media, inspection |
| [`stage/`](../../cockpit/src/stage/) | World visualization, identity and ceremonies |

Rust tests intentionally live outside the crate's source tree, often connected
by `#[path]`. Their subsystem folders do not mirror these buckets. Use the
[operation-to-test map](operations.md#test-entry-points), rather than guessing a
test's location or renaming tests to make the trees look alike.

## Proposed destination: modules by responsibility

**Proposal, not existing paths or a required rename.** The active hotloader's
conventions should settle exact names. Prefer a small native host plus cohesive
JS features, not an ever-growing `scripts/` directory that becomes a second
monolith.

```text
cockpit/                   native host; retained location during migration
modules/
  <feature>/
    module.json            identity, host contract and state-schema versions
    index.mjs              lifecycle entry point; no import-time side effects
    ...                    feature implementation, named by responsibility
    README.md              inputs, outputs, state owner, activation/reload policy
lib/
  contracts/               shared boundary schemas and fixtures
  ...                      genuinely shared JS libraries, not feature internals
tests/
  modules/<feature>/       pure behavior and module lifecycle tests
  contracts/               host/module compatibility and fault-injection tests
  cockpit/                 retained native tests
  scripts/                 launcher, packaging and worker tests
scripts/
  runtime/                 compatibility entry points during migration
  check/                   development gates
  bench/                   experiments and measurement
  release/                 bundle/install verification
docs/
  architecture/            current system map and explicit design proposals
```

`modules/` above is intentionally distinct from the existing
`cockpit/modules/` presentation manifests. Do not give a new directory two
meanings, create placeholder module trees, or rename the Rust crate before a
real extraction needs it.

### Migration rules

1. **Pick a behavior, not a large file.** Record its input/output contract,
   durable-state owner, native calls and existing test coverage.
2. **Extract pure logic before lifecycle.** A deterministic transform is easier
   to compare than a controller holding a provider stream or a live game.
3. **Keep one writer and one implementation.** Temporary launch shims may forward
   to the new module; they should not carry a second copy of its behavior.
4. **Move with the consumers.** Update imports, Rust resource paths, test wiring,
   package inventory and installation checks in the same migration. The
   [resource resolver](../../cockpit/src/platform/runtime_paths.rs) and
   [release inventory](../../scripts/release/release-evidence.mjs) name shipped
   helpers explicitly. A development checkout finding a file is not proof that
   an installed bundle contains it.
5. **Preserve public entry points.** Existing CLI commands and persisted schemas
   need a compatibility path or an explicit migration, not a silent rename.
6. **Measure the benefit.** Record native rebuilds avoided, edit-to-activation
   latency, memory and behavior parity; moving lines from Rust to JS alone is
   not evidence of a leaner system.
7. **Update the map in the extraction patch.** Mark a proposal as implemented
   only when its source and acceptance evidence exist.

## Maintaining these pages

Use relative source/test links, stable symbols in prose, and small workflow
subsections. Avoid copied source listings, hand-maintained line counts and
machine-specific worktree paths. Keep historical receipts in telemetry and link
them as evidence, not as the current specification. Do not silently turn this
proposal into a claim about what another agent has already built.
