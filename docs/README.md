# Documentation index

Start with the [project README](../README.md) for installation and the product
overview. This index separates current guides, developer architecture, and
historical evidence without moving established documentation paths.

**Checkout-only developer index.** This index and `architecture/` are not yet in
the public release allowlist. They link internal history as well as shipped
guides. Keep public entry-point docs unchanged until publishing this map and
its link dependencies is an intentional release change.

## Architecture and development

- **[Architecture and repository map](architecture/README.md)** — current folder
  responsibilities, placement rules, and a proposed module-oriented destination.
- **[Software operations schematic](architecture/operations.md)** — launch, turns,
  tools, orchestration, state, workers, rendering and release; source/test links.
- **[Connected loop settlement](architecture/connected-settlement.md)** — player
  hall expansion, shared playable/passive maps, crafting and real research exhibits;
  commands, worker deposition contract and current limits.
- **[Lean host and hotloading proposal](architecture/lean-host.md)** — existing
  reload mechanisms versus proposed JS module replacement, ownership, failure
  handling and acceptance criteria. This is a proposal, not a shipped loader.
- [Contributing](../CONTRIBUTING.md), [native cockpit overview](../cockpit/README.md),
  [scripts](../scripts/README.md), [tests](../tests/README.md).
- [Working on the cockpit source](SELF_MODEL.md),
  [environment reference](../cockpit/docs/ENV.md),
  [checked launch intent](checked-launch-intent.md).

## Operating angelX

| Need | Guide |
|---|---|
| Connect models | [Models](MODELS.md) |
| Find an interactive command | [Commands](COMMANDS.md) |
| Understand feature evidence | [Features](FEATURES.md) |
| Understand memory and recall | [Memory](MEMORY.md) |
| Run local workers and inspect their artifacts | [Workers](WORKERS.md) |
| Plug in a competition | [Cartridges](CARTRIDGES.md) and [example cartridge](examples/cartridges/lean-kernel/) |
| Understand the competition evaluator interface | [Runner contract](../cockpit/docs/COMPETITION_RUNNER.md) |
| Review execution safeguards | [Harness circuit breakers](../cockpit/docs/HARNESS_CIRCUIT_BREAKERS.md) |
| Distinguish model-estimated decisions from benchmark arithmetic | [Jev](../cockpit/docs/JEV.md) |
| Explore live charts | [Graph garden](GRAPH_GARDEN.md) |
| Understand the world presentation | [World adventure](world-adventure.md), [School of Magic](world-school.md) |
| Play or extend the co-op dungeon | [The Delve](DELVE.md), [Delve lore](DELVE-LORE.md) |
| Map research questions, evidence and refutations | [Research labyrinth](LABYRINTH.md) |

## Shipping and attribution

- [Release evidence and verification](release-evidence.md).
- [0.2.0 release notes](RELEASE_0.2.0.md) — what 0.2.0 adds, with commands.
- [0.1.9 release line](RELEASE_0.1.9.md) — version-specific, not a rolling specification.
- [Security](../SECURITY.md), [third-party notices](../THIRD_PARTY_NOTICES.md),
  [image provenance](images/README.md).

## Backlog and historical evidence

- [Bugs](BUGS.md) and [wishlist](WISHLIST.md): recorded issues and ideas; inspect
  current source before treating a historical entry as an unresolved defect.
- [Speed-loop handoff](SPEED_LOOP.md): campaign context and evidence.
- [Telemetry](telemetry/): dated experiments, measurements and handoffs. Their
  reported results apply to their recorded source/settings, not every checkout.
- [Stables and knights tournament](architecture/stables-tournament.md): playable
  owner-local practice, actual 3D routes, controls and privacy/render limits.
- [Source-layer reorganization handoff](handoff-refactor-src-layers.md): explicitly
  superseded; useful history, not the current migration plan.

Keep new architecture pages under `architecture/`, worked examples under
`examples/`, figures/provenance under `images/`, and dated research receipts under
`telemetry/`. Link from here instead of creating another competing root overview.
