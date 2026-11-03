# Bend modules above the Rust core

AngelX now loads Bend-authored behavior through the existing V8 module backend.
The first production module is the Fairy Stun Field. Its definition, placement,
pulse schedule, room check, expiry, radius and stun duration live in
`cockpit/live/ability-code/fairies.bend`. A small CommonJS adapter translates
its typed result to the existing ability command port. Spark and Bloom remain
JS modules. Glass, Link and Kernel remain the public foundations; no new crate,
registry or game-specific Rust branch was added.

## Use it

Install the pinned developer compiler once, using existing Node, Bun and Git:

```sh
node scripts/live/bend.mjs setup
```

The tool builds Bend 2.0.39 from clean upstream commit
`221a350c2f7f7134884740018e6e90001fb6d230` into the user's cache under
`angel/bend2/<commit>`. It does not change the global toolchain. A clean local
checkout of that commit can be passed as the final setup argument. For an
existing compiler, set `ANGEL_BEND_COMPILER` to its absolute binary path; the
same variable can name the pinned source `bend2/main.ts` with Bun available.
`BUN` selects that developer runtime if needed.

After the host containing this loader update is built, run AngelX normally and
edit `cockpit/live/ability-code/fairies.bend`. An overridden `ANGEL_LIVE_DIR`
selects a different module directory. The existing 100 ms module scanner finds
the change, captures source, checks the proof, emits JS, validates the module's
contracts and atomically replaces the dependency closure. The host keeps
running. `/reload status` reports the active generation or rejection;
`/reload` explicitly retries an unchanged failed build, including after compiler
installation. There is no second watcher to start.

`/ability fairies [player] [slot]` selects the skill. For example, changing
`Params{3.5, 150, 12, 12}` changes radius, lifetime, stun ticks and pulse spacing.
The game and independent VFX pack consume the same accepted definition. Existing
casts keep their captured implementation, parameters and media; future casts
use the new generation. Player choices, seats, mana and cooldowns are owned by
the existing host contracts and survive a module replacement.

Packaged generations contain resolved JS plus a source/output receipt and run
without Node, Bun or Bend installed. Changed Bend source requires the developer
compiler; a missing compiler keeps the working generation and reports the setup
command. To prepare an updated artifact for distribution:

```sh
node scripts/live/bend.mjs build cockpit/live/ability-code
node --test tests/scripts/bend-modules.test.mjs
```

## Source and state contracts

An existing API-1 or API-2 JS module can declare this optional source build:

```json
"bend": {
  "entry": "fairies.bend",
  "proof": "PROOF.bend",
  "output": "fairies.compiled.js"
}
```

The ordinary adapter imports the emitted CommonJS module with `require` and
uses its pure typed definitions. Its existing manifest still declares the
commands, surfaces, services, resources, dependencies and state schema. Bend is
an authoring/compiler backend behind those contracts, so adding another Bend
module does not require another native application dispatch branch.

The builder receives an immutable capture of local Bend sources. `Base` and
captured relative Bend imports are supported; proof imports must reach both
`LAWS.bend` and the entry. Hub fetches, arbitrary foreign files and reads of the
changing source directory are outside this pure-module build contract. The
normal filesystem capture, source/asset byte limits and metadata checks apply.
Build protocol/receipt schema 1 records the compiler version/pin and complete
source/output hashes. The loader rejects stale or incompatible artifacts.

Compilation occurs on the existing module worker. It never runs during cast
execution or painting. The builder has bounded output and a ten-second outer
deadline, retires its process group before reaping, and joins its pipe owners.
Rust owns a private staging root and removes captured source even when the
builder is forcibly terminated.
Failed source captures are cached until an input changes or an explicit retry;
unchanged polls perform metadata checks without reading/compiling payloads.
In-memory candidates never publish partially compiled files. Distribution
publishes output and receipt separately; an intervening mismatched pair is
rejected or rebuilt from the same captured source before activation.

The field's state uses the current computational-instance contract. An old cast
retains its original source and state while another generation starts beside it.
Only validated action batches commit actor state, physical effects, audio and
VFX. Cancellation, room changes, expiry and resource retirement use the existing
owners. The full-Run native worker bridge remains a separate open boundary.

## Ownership and language scope

| Owner | Responsibility |
|---|---|
| Rust Kernel | Lifecycle, dependency binding, deadlines/budgets, resource leases, job identities, state migration/commit, effect receipts and checkpoints |
| Rust Glass and Link machinery | Device/window/terminal/browser plumbing, rendering resources, audio playback, assets, connections, model/tool streams and live seats |
| Bend candidates | Executable abilities, animation transitions, game rules, economy/policy transforms, suitable agent planning/routing and scene descriptions |
| Authored resources | Art, animation data, sound, effect graphs and persistent loadouts accessed through generation-owned handles |

The next candidates need their own complete vertical extraction and measurement.
Bend has a small standard library, linked-list strings and no built-in JSON,
HTTP or TLS library. Text-heavy agent/provider adapters require particular care.
Bend 2 is a different language from the superseded Bend 1/HVM2. Its
[documented limitations](https://github.com/bendlang/bend/blob/221a350c2f7f7134884740018e6e90001fb6d230/README.md#limitations)
include one C compilation unit, no incremental native builds and a single-core
JS target. Native/GPU Bend execution is a later measured option.

`LAWS.bend` and `PROOF.bend` check inactive-field termination and zero-lifetime
expiry. The ordinary Bend checker accepts both and rejects a deliberately broken
termination branch. `--verdict` was not run, so there is no separately verified
kernel result claimed. Laws cover their actual statements. Foreign code,
backends, adapters, persistence and effects require independent validation;
F32 remains axiomatic. Rust validates host contracts regardless of proof status.

## Evidence and remaining acceptance

- 48 distinct Kernel/loader checks and seven process lifecycle checks pass.
  They cover automatic Bend replacement, cached proof failures, packaged offline
  operation, source/output receipts, byte/deadline bounds, descendant/pipe
  retirement and removal of private staging files after forced termination.
- 183 selected native checks pass, plus two explicitly run Bend field/measurement
  cases (185 total). The field case preserves two players and cooldowns, rejects
  a broken proof without changing resources, keeps the old 3.5-radius/12-tick
  behavior and starts a 7-radius/24-tick replacement. Real near/far enemies,
  captured VFX definitions and room retirement are checked.
- A four-seat fixture runs four captured Bend fields through 120 complete
  rules/ability ticks: 1,651 µs median / 1,982 µs p95 (2,243 µs maximum), with
  44 status-area actions. It excludes rendering and compilation.
- Automatic source-write-to-loader activation measures 593 ms in the final
  loader fixture. Explicit native capture/check/compile/activation measures
  221 ms. Real same-PID terminal replacement measures 628 ms with the native
  rules worker and 603 ms with embedded rules. Old fields continue, new casts
  control the farther enemy, and cancellation retires both actor/effect groups.
- All thirteen strict terminal scenarios pass (ten native-worker and three
  embedded ability/VFX/field variants). All 61 packaging/Bend/browser-media
  checks pass, including byte-for-byte regeneration of the shipped artifact.
  Runtime-helper closure includes 7,636 entries; fresh-cache default setup also
  builds and runs the pinned compiler successfully.

These are local shared-machine fixture measurements, not frame guarantees.
The normal App build encountered a cached-object linker failure and was rebuilt
successfully with incremental reuse disabled; existing caches were preserved.
The fresh App/terminal build took 7m48s, including affected workspace libraries.
This foundation update is built once; field edits thereafter use only the Bend
compiler and existing hot-load lifecycle. No Rust compiler runs in that path.
The recorded logs and runtime-helper closure result are in `modularity-map.md`.
The existing native/browser painter and wire contracts remain unchanged; Bend
runs in the authoritative host and emits their existing game/effect data.

The original isolated Node feasibility experiment remains under
`docs/architecture/examples/bend-fairy-field`. It established compiler feasibility
before the real module integration and does not substitute for game acceptance.
Bend conversion of additional systems, complete module-owned game state, broader
character events and agent/tool/provider workflow extraction remain open.
