# Stable mechanisms and replaceable systems

This is the working inventory for the AngelX modularity refactor in private
PR #29. "Stable" means infrequent mechanism changes with explicit compatibility;
it does not freeze bugs in place. Feature behavior must stay outside these
contracts, even when that behavior is important to the application.

## Stable foundational systems

A Rust crate is a build/dependency boundary, not automatically a separate
application system. Expose a small set of coherent foundations:

| System | Stable responsibility | Current implementation pieces |
|---|---|---|
| **Glass** | Scene contract, rendering, window/canvas/terminal presentation, input and graphics resource handles | `angel-stage`, `angel-gpu`, native `angel-glass`, browser `angel-web-render`, terminal adapter `angel-glass-term` |
| **Link** | Protocol framing, clocks, replication, sessions, seats, sockets and reconnect continuity | `angel-net`, `angel-serve`, transport portions of `angel-web` |
| **Kernel / module host** | Scheduling, module/service registry, state commits, resource ownership, effect dispatch and generation handoff | Currently spread through cockpit and `angel-live`; extract the shared mechanism |

These are stable mechanism contracts. Authored scenes, character presentation,
combat rules, agent decisions and application workflows stay outside them.

### Glass is one subsystem

`stage` describes a scene; `gpu` implements the shared renderer; `glass` hosts it
in a native window; `web-render` hosts that same renderer in a browser canvas.
The latter imports `angel_gpu::Core`, so these are mostly useful layers rather
than four duplicate engines. Their current names/descriptions create noise when
presented as independent foundations.

Publish one Glass contract and organize its internal implementation under Glass.
Keep separate internal crates only where they avoid rebuilds, isolate native vs
browser dependencies, or enforce a useful ownership boundary. Modules should not
have to know which backend draws a scene. Do not merge everything into one heavy
native crate, pull native-window dependencies into module APIs, or multiply
public systems just because a Cargo package exists.

There is also real boundary leakage: current `crates/glass/src` includes authored
`wizard`, `squire`, `shore`, `monolith`, `showcase`, sprite and transition content.
Extract that presentation into reloadable modules/packs before treating the
whole implementation as stable. Keep generic interpolation/composition in Glass;
keep particular characters, animations, scenes and artistic choices in packs.

### Current crate inventory within those systems

| Crate | Intended stable portion | Content/policy to keep outside |
|---|---|---|
| `angel-net` | Wire formats, replication, clocks and client contracts | Game/agent decisions |
| `angel-stage` | Glass scene/layer/mesh/material contracts | Authored scenes, themes and animation graphs |
| `angel-gpu` | Glass pipelines and execution of scene primitives | Feature-specific drawing policy |
| `angel-glass` | Native Glass window/input/composition machinery | Authored showcase/character/scenery modules already mixed into this crate |
| `angel-web-render` | Browser Glass backend using the shared GPU core | Game/client-specific views |
| `angel-glass-term` | Glass terminal/PTY adapter | Shell/cockpit workflows |
| `angel-serve` | Link connections, sessions, seats and delivery | Simulation/work-board behavior |
| `angel-web` | Browser host ABI and generic input/socket plumbing | Particular game clients and feature UI |

A feature edit should not modify stable mechanism source or force the native
host to relink. This inventory is not a blanket instruction to freeze every line
of the existing mixed crates.

## Existing crates to split or make replaceable

| Current crate | Replaceable systems to extract |
|---|---|
| `angelX-cockpit` (`cockpit`) | Surface composition/widgets, miniworld presentation, commands, agent strategy, tool definitions, provider adapters, goals/loops, memory policy and application workflows |
| `angel-delve` (`crates/delve`) | Rules/simulation, world generation, game rendering, content, activities, dialogue and gameplay policy; separate shared view/state contracts from implementations |
| `angel-delve-system` (`crates/delve-system`) | Current executable implementation of the rules system; keep its host boundary small and move toward module-owned state plus view deltas |
| `angel-realm-host` (`crates/realm-host`) | The realm's host side, cut out of the cockpit binary crate so game and cockpit edits stop rebuilding each other; its build script builds and embeds the Delve's browser page, and the host modules that need nothing of the cockpit move in here. The stage is in: the miniworld (`world_viz`), `hearth`, `village`, `knight_cast`, `identity`, the settlement ledger and the loop-summary types. The delve's host is in too: `together`, `together_sim` (the server and its service), the seats (`together_seat`, `together_join`), the realm and its save (`together_realm`, `chivalry`), the wish grants and painters, the live loadouts/abilities/appearances, the sounds, sheets and files, and the render worker (`game_pipeline`). The few answers only the cockpit has are lent through `hooks` (Lean/Comp posture, the published agent stage, repository identity, the village's HTTP) and `services` (the club in hand as the grants' `Oracle`, an owned child process for the painters, the window's paint door for a joined host); the cockpit re-exports each module at its old path. The rules' Bend seam (`together_rules`) and the V8-run tests of the live ability code stay in the cockpit. Arcade painters onto a ratatui frame (`world_viz::art`) and the world pane's drag state stay in the cockpit until the shooter view moves |
| `angel-table` (`crates/table`) | Round Table/work-board behavior and projections |
| `angel-muster` (`crates/muster`) | Crowd-game rules, bot/player behavior and game-specific rendering |
| `angel-web-delve` (`crates/web/delve`) | Replaceable Delve browser client; coordinate simulation/prediction contract versions with the authoritative rules module |
| `angel-web-muster` (`crates/web/muster`) | Replaceable crowd-game browser client |
| `angel-live` (`crates/live`) | Shared runtime-independent kernel/contracts, JS bundle adapter and process supervisor now have separate internal modules. Keep this one subsystem; extract packages only for measured build isolation |

`angel-bots`, Delve smoke/dev hosts, examples and benchmarks are development
tools. Keep them independent from production boot and use them to prove the
boundaries; they are not permanent application-core dependencies.

## Bend integration (operator direction, 2026-10-26)

Bend 2 is now an authored source backend behind the existing JS module host.
The real Fairy Stun Field's definition, state transitions and status policy
compile from `cockpit/live/ability-code/fairies.bend`; the ordinary adapter
emits existing game commands. Captured source/proofs compile on the module
worker before contract validation and atomic dependency activation. Source and
compiled-artifact receipts are versioned and bounded; compiler failures retain
working generations and are cached between edits. Packaged code runs without
the developer compiler. No crate, registry, watcher or public subsystem was
added. [Bend integration](bend-feasibility.md) documents setup, the contracts,
compiler pin, initial measurements and remaining language boundaries.

The native field check preserves old radius/stun behavior while starting a new
generation, verifies real near/far enemy control and failed proof retention,
and preserves multiplayer state/cooldowns. The larger Rust-core, character-pack
and agent/workflow refactor continues. The preserved wish/grant work was restored
after that Bend implementation commit; its completed ownership boundary and
remaining acceptance work are recorded below.

## Kernel packaging boundaries to establish

These are subsystem boundaries, not a proposal to create a crate for every row.
They are pieces of one Kernel/module-host subsystem; introduce separate Cargo
packages only when dependency isolation or build measurements justify them.

| Target boundary | Responsibility |
|---|---|
| Module API | Implemented manifests, event/result/effect types, state envelopes and prepared resource bindings; richer Glass scene references and typed/WIT bindings remain open |
| Kernel | Implemented dynamic command/service/surface/resource routes, dependency graph, state commits, generation fencing and prepared resource leases. Tool execution integration remains open |
| V8 backend | Implemented as `angel_live::v8` (cargo feature `v8`): warm pooled isolates with per-generation cached module setup, the 200 ms / 32 MiB bounds, and the policy generations shipped from `cockpit/live` (`angel_live::shipped`). The cockpit and the Delve rules worker both link it. Existing JS/CommonJS modules and checked Bend source builds; persistent module state and coherent dependency generations |
| Wasm backend | Rust/Wasm execution behind the same lifecycle; choose engine/ABI after an end-to-end performance and integration proof |
| Process backend | Native services, framed transport, deadlines and retirement; extract the working supervisor in `angel-live::system` |

The kernel depends on module contracts and abstract host ports. It must not
import Delve, Table, provider SDKs, feature widgets or a particular execution
engine. Feature modules request registered services instead of importing cockpit
`App` internals. The executable composes Kernel, Glass, Link and selected runtime
backends. Asset loading and audio playback are host resource services; character
art, sound banks and effect policy remain replaceable content.

## Systems that should change while AngelX runs

| System | Module-owned behavior | Host-owned mechanism/resources |
|---|---|---|
| Character packs | Character art, animation graphs, spells/abilities, passives, SFX/VFX and loadouts | Asset handles, playback/mixing, timing, rendering and effect execution |
| Surface layer | New panes/widgets, layout, themes, interactions, miniworld scenes and animations | Terminal/window, input delivery, scene primitives, GPU resources and presentation |
| Agent strategy | Planning, routing, memory selection, compaction policy, orchestration and tool choice | Active request/job identities, cancellation, model streams and effect execution |
| Provider/tool adapters | Protocol adapters, tool schemas/implementations and new registered capabilities | Network/process services, credentials, existing operator authority and effect receipts |
| Game/world | Rules, content, activities, world generation and presentation | Seats, sockets, clocks, input transport and committed host effects |
| Goal/workflow controllers | Goals, loops, handoffs, work-board behavior and background policies | Scheduling, persistence transactions and host-owned jobs |
| New services | New commands, surfaces, tools and service interfaces registered by manifest | Registry, resource handles, dependency admission and lifecycle |

Critical application behavior belongs in modules too. The stable layer protects
continuity of resources and work; it should not accumulate feature policy just
because that policy is important.

## Character packs and player options

Treat every playable character as a resolved, versioned pack/loadout rather than
an assortment of hardcoded branches. Players can select compatible options from
different packs; the resolved dependency closure has one inspectable identity.

| Pack surface | Hot-swappable options |
|---|---|
| Character art | Sprites, sheets, portraits, meshes, skins, equipment visuals and palette/material choices |
| Animation | Clips, frame timing, rigs, state graphs, blends, transitions, movement/cast/hit/death presentation |
| Spells and abilities | Definitions and executable behavior: targeting, cast/release, cooldown/cost policy, projectiles, summons, passives, status effects and upgrades |
| SFX | Sound banks, voices, ability-to-sound mappings, variation and levels |
| VFX | Effect graphs, particles, trails, impacts, lighting, screen shake and animation-linked effects |
| Player choices | Per-character loadouts, selected visuals/voices/effects, bindings and persistent preferences |

Keep position, health, resources, cooldowns and active ability-instance identity
coherent during a pack change. Existing casts, projectiles, voices and effects
retain their owning generation until completion; new activations use the new
resolved pack. Compatible animation replacements preserve normalized playback
phase. Incompatible rigs/state require an explicit remap/migration.

Load and validate a complete candidate pack off the frame/tick path, including
referenced assets and ability modules. A missing file or failed behavior module
retains the working pack. Resource handles keep textures/audio alive until their
last in-flight user retires. Shared games announce resolved pack and asset
identities so clients can display the same selection and required behavior.

A pack must be able to add a new character or ability through runtime
registration. Requiring another Rust enum variant, `App` branch or embedded
asset rebuild means that surface has not been extracted yet. Existing spell/card
hotloading is a useful entry point; it does not cover this entire contract.

## Handoff and performance contract

1. Build/capture one immutable implementation and its dependency closure.
2. Prepare it without admitting live requests or executing host effects.
3. Reach the declared boundary; retain in-flight work with its owning generation.
4. Export versioned state, migrate a copy, validate it and stage resource handoff.
5. Commit the dependency group, dispatch table, state ownership and resource
   leases together. Reject late completions from the wrong request/generation.
6. Retire old resources and record the new implementation identity. Pre-commit
   failures keep the old system; post-effect recovery uses reconciled state.

Module working state stays with the module. Send events, commands, batches and
changed views on normal paths; reserve full snapshots for reload/checkpoint
boundaries. Rendering consumes cached views; typing and model stream paint must
not wait for visual work. Keep bounded queues and measure dropped/coalesced work.

The current Delve process bridge transfers the rules Run per request (excluding
host-owned appearance bytes) to support
the existing direct-mutation API. It proves executable/state handoff, but this is
an intermediate compatibility path. Replace `RunMut` writes with explicit
commands before keeping the authoritative Run in its worker and publishing view
deltas. Measure four-seat play and UI responsiveness during that extraction.

## Current incremental acceptance

The shared kernel and API 2 JS backend now admit new stateful commands, cached
text surfaces, services and prepared resources while the host runs. Code/assets
form one immutable generation. Migration/resource failure retains the complete
old generation; dependency contracts, stale commits, service generation leases
and resource retirement have targeted tests. The ordinary app routes these
commands and surfaces without feature-specific branches.

Namespaced resource exports now support separate providers with the same pack
name. Declared imports carry exact executable/native resource leases, and source,
asset or resource-state changes rebind transitive consumers atomically while
preserving their state. An open-pane terminal test and JS/native instance tests
cover independent provider updates, rejection and old-generation retirement.
The shipped resolver now composes independent ability, art, sound and effect
catalogues. Selected slots execute their original provider and retain the resolved
closure per cast. Media/art changes preserve the computational instance and
install art/slots together. Missing selected options reject the whole closure.
Affected active service jobs still defer rebinding.

The character appearance adapter supports player options, authored pixels or
captured image frames, animation timing and compatible phase handoff. Gameplay
state survives an appearance swap. The executable ability adapter now accepts
registered per-player slot choices and runs authored timelines through a bounded
physics port. Computational instances stage/validate whole batches before mana,
cooldown and game commits. Pending/running casts retain code and asset leases;
new casts use replacements, and failed batches quarantine without partial effects
or automatic replay. Native projectile data retains its cast/source identity.
Ability SFX now support captured WAV banks and per-player choices. Active casts
retain their cue map and bytes; native children, queued Link events, HTTP readers
and browser voices retain playback resources independently. Invalid media cannot
partially commit a cast. Ability VFX now use prepared graph banks, shared
native/browser painting and per-effect source/resource leases that outlive
casts. Immutable graph identities and view revisions avoid retransmitting or
rebuilding sprite data on ordinary animation ticks. Full live schema 3 carries
the scene; rules RPC excludes it and the host retains ownership.
The authored Fairy Stun Field combines a stationary pulse timeline, a reusable
area-stun port and six animated sprite instances. The effect pack imports its
radius/lifetime from the ability pack, so changed behavior and visuals activate
together while old fields keep their captured versions. Indexed artwork prepares
off the draw path. The underlying status primitive and indexed-frame format need
a native foundation update; field-specific behavior and artwork remain authored.
Real terminal tests with native and embedded rules change the radius from 3.5 to
7 while the first field continues, then cast the new field in a second slot. The
first field controls one enemy and the wider field controls both, with the same
host PID. Observed automatic activation was 409–446 ms in these local runs.
Broader character-event sound overrides, trails/lighting/shake,
rich animation graphs, richer physics, rich Glass
scenes and efficient game view deltas are still outstanding. Ability execution
currently batches one V8 invocation per active generation on the simulation
thread; execution scheduling needs further efficiency work. Do not count these
incremental adapters as the entire character
pack acceptance or mark the PR ready on this evidence.

The rules bridge also avoids redundant JSON-tree copies and reuses the typed
Run produced by validation. Its current four-seat/eight-frame fixture reports
72,724 transmitted rules bytes versus 706,172 full-state bytes, with 4.7 ms
median / 5.6 ms p95 in the latest local run. The separate four-player executable
ability fixture applies 720 game actions, 720 sound events and 720 one-tick VFX
in 180 ticks at 2.4 ms median / 5.3 ms p95 adapter time in the latest shared-machine
run; that measurement excludes
rules IPC, physics, drawing and actual playback. The 64-effect particle painter
measures 0.077 ms median / 0.085 ms p95 by itself. These remain local fixtures.
The rules path remains a compatibility bridge; game-owned
working state, explicit host commands and changed-view publication are still
required before that boundary is complete.

Host-edit extraction now routes offer choices, activity exits, voting, travel,
party seating and equipment changes through `rules::host_commands`. Identified
batches prepare a candidate world and retain success or failure receipts until
individually acknowledged. Session/raid fences reject stale requests; count and
byte capacity apply backpressure without evicting unresolved results. Startup
game configuration and captured avatars commit under one lock, while appearance
remains host-owned. This is still an intermediate adapter: the journal currently
lives beside the host's Run. The Home transaction port below extends it;
22 remaining direct App writers, worker-owned state and view deltas still need
conversion. Twelve command tests
and 146 selected native checks cover the current port, including private restore,
uncertain persistence and cooperative writer exclusion. Twelve strict terminal
runs pass on the base branch. A temporary integration with Muse's 0.2.6 release
also passes 167 native checks, the same 12 terminal runs and 55 packaging checks;
the read-only operator lock checker passes. That trial merge was aborted.

Realm bank transfers now retain one identified claim while new spoils accumulate.
A private version-3 checkpoint captures Run and journal under the same core lock,
then releases it before file IO. Unclaimed later triumphs also persist beside a
retained claim (ordinary Run saves omit that transient field). Genuine version-1
saves still restore with a new session identity, and version-2 saves retain theirs. Settlement forces that source checkpoint before writing balances
and their receipt together in the realm JSON, then acknowledges the original claim.
A post-rename error keeps its identity: retry confirms the disk receipt and fsyncs
before acknowledging, without paying again. Cooperative realm writers use a private
cross-process lock and compare the loaded file hash before replacing it; stale
copies and conflicting payloads fail closed. Unrelated unsaved edits are retained
if confirming an uncertain transaction would overwrite them. Receipt cursors retain
up to 4,096 session identities and never evict an old one to admit a new payment;
source-session retirement/receipt compaction is still an explicit future boundary.
This transaction currently covers banked spoils, cleared floors and triumphs;
the Home port below extends the transaction boundary to purchases, peddler
deliveries and achievements.
A real-terminal case holds a 19-gold claim while treasury storage is unavailable,
replaces the native rules executable on the same PID, verifies the saved claim's
identity and payload, then recovers storage and confirms one payment and durable
acknowledgement. Closing the game afterward does not replay the reward.

The Home transaction port now evaluates purchase/progression policy through
`delve.home` version 1 on the active Delve executable. `rules::home_settlement`
defines its explicit captured-facts/economy-context request and result. Native App
code coordinates an identified prepare, game delivery, durable source receipt,
refund/finalization and acknowledgement. Finite stall stock is reserved, paid
cards retain captured definitions, and undeliverable goods receive identified
refunds. Bank and Home transactions share the existing private lock/stamp/atomic
writer, with separate cursor streams. No crate or public subsystem was added.

Private session saves retain a typed Home outbox beside Run and journal;
genuine older checkpoints still migrate (the brew port below extends this to
version 4). Pending quest facts stay visible to gameplay,
and completion consumes only their original prefixes/counter amounts. New facts
survive, and candidate workers cannot erase pending intentions. The live rules
codec remains schema 3. An explicit committed realm context also detects derived
work such as a foal growing after another raid, without continuously generating
empty claims. This is still a host-side command adapter, not worker-owned state.

Validation before the native policy extraction: 30 focused command/Home/market checks pass, including realm
context changes, preservation of newer local edits, repeated goblin escapes and
private outbox rejection. The full Delve run passes 2,811 checks with 300 ignored;
170 selected native checks pass with 11 ignored, including private restore of an
already delivered purchase without replay, newer facts and uncertain payment/refund
writes. Thirteen strict terminal runs pass (ten native-worker scenarios plus three
embedded ability/visual variants). The new case keeps a purchase, quest rewards
and XP visible while storage is blocked, swaps the native worker on the same PID,
then confirms one delivery and durable acknowledgement after recovery. All 55
selected packaging/install checks and runtime-helper closure (7,617 entries) pass.
A temporary integration with Muse's 0.2.6 release (`efae388e8`) also passes
170 native checks, the same 13 terminal runs, 55 packaging checks and helper
closure (7,619 entries); protected-file hashes pass read-only. Worker sources
and their lockfile are identical to the tested base. The trial merge was aborted
before publication; its release changes are not part of this commit.

The native supervisor negotiates required computation contract versions during
prepare, before a candidate can retire the working process. Pure computations
run only on the active generation; candidate admission still needs the existing
validated checkpoint boundary. Home policy sends bounded facts and context,
with no Run, and cannot change bank-owned reclaimed-floor/raid counters. The
host validates results and retains the existing lock/hash/atomic transaction
writer. Destination receipt lookup precedes the evaluator, so even recovery
from an uncertain write uses the exact saved result without a live worker.
Only the explicit embedded-engine path evaluates the linked policy in process.

Seven native-process lifecycle checks pass, including missing/wrong contract
rejection, in-flight computation handoff, output rejection and retirement. Five
state checks and three real Delve worker replacement checks pass. The Home
fixture changes its reward in a separately linked executable while the original
result, multiplayer seats and issued cue survive. Local automatic activation
measures 307 ms; its 5,489-byte request and 2,962-byte result take 371 µs median /
505 µs p95 across 60 calls. These measurements exclude compilation and durable
file IO. Rust edits still require worker compilation; this pass's worker/test
build took 77 seconds with two build jobs.

Current extraction validation passes: 2,812 Delve tests (300 ignored), 171 selected
native host checks (11 ignored), 39 Kernel/loader/resource unit checks (one
measurement ignored), seven process lifecycle checks and the five state / three
Delve process-swap checks above. Thirteen strict terminal runs pass: ten native
worker scenarios and three embedded ability/VFX/fairy variants. The Home case
holds a reserved purchase, XP and quest facts while storage is blocked, replaces
the native executable on the same host PID, then receives the new policy's
27-gold outcome instead of the original 20-gold outcome after recovery. Closing
does not repay it. All 55 selected packaging/install checks and runtime-helper
closure (7,617 entries) pass. The earlier 0.2.6 trial above predates this policy
extraction; it has not been repeated for this pass. Protected files are unchanged.

Brewing now has an identified request/completion port in the existing game
journal. A claim captures both input cards and binds this particular stand-in
and result to its session, raid and command identity. A late answer cannot
replace a later brew of the same recipe. App retains its model turn, while
`delve.brew` version 1 evaluates authored validation policy in the active native
worker using explicit facts, the answer and taken-name/effect summaries. The
host checks the card contract and game ownership before once-only delivery;
slot cooldowns, seats and already-issued effects survive. No crate, effect
ledger or separate registry was added.

Private checkpoint version 4 adds the unclaimed brew outbox; versions 1/2/3 still
migrate. Journal schema 3 retains captured work and terminal answers alongside
existing Home/bank receipts, with shared count/byte backpressure and individual
acknowledgement. Model work starts after its claim is durable. Admission backpressure must still
allow existing completions to drain, including when all 64 receipts are held. Worker or storage
failure retains the same answer without starting another model turn; world and
terminal receipt reach disk before acknowledgement and resource retirement.
Completed answers restore without invoking newer policy or rereading a changed
file. Only logical card rejection requests the existing single mend. Provider
job/stream restoration across a host restart remains a separate boundary.

Unchanged broker frames use the existing command sequence as a discovery cursor;
captured cards copy only when work is admitted. Pending-binding validation borrows
journal data on the tick path. Full Run serialization is not added to the brew
computation port. The existing full-Run rules tick bridge and host-owned journal
remain intermediate adapters. Brewing removes two more direct App writers,
leaving 22 at that stage. Prompt construction and model scheduling remain feature code in the
native coordinator; they are not part of stable Kernel machinery.

Six new lifecycle checks cover captured definitions, stale-session/player
rejection, independent bindings, cached delivery without recharge/replayed effects,
invalid handoffs, cancellation before and after admission, and capacity. Character
renaming retains its unique brew binding. All 2,818 Delve checks pass (300 ignored).
Five state checks and four executable replacement checks pass. The brew payload
is 4,882 bytes in and 327 bytes out, with no Run. Across 60 calls, the replacement
fixture measures 146 µs median / 185 µs p95; automatic activation measures
307 ms. Those figures exclude Rust compilation and durable IO; the worker/test rebuild took
85 seconds with two jobs; the later fixture-only rebuild takes 4.8 seconds. The
fixture replaces the authored power/name policy: the original implementation
rejects the two-second answer, then the replacement accepts that same answer
while retaining the earlier committed result. Native validation passes: 174
selected host checks (12 marked ignored), plus the explicitly run native Sim
brew replacement case. The latter preserves the seat pointer, two players,
remaining cooldown and cached grant while the new policy handles the next
answer. Broker checks include an uncertain checkpoint without resubmission and
a full 64-receipt queue still draining existing work. Thirteen strict terminal
runs pass (ten native-worker cases and three embedded ability/VFX/fairy variants).
All 55 selected packaging/install checks and runtime-helper closure (7,621
entries) pass. One input-lease check failed while a fixture build was running;
it passed alone and in the final selection without compilation overlap, with
its implementation unchanged. The earlier 0.2.6 trial predates these extractions.
Captured job copies retire with their receipts; spell definitions remain in the existing bounded book
so issued effects can still refer to them.

Host-authored kit reforges now use identified request/completion ports in the
same journal. `delve.reforge` version 1 computes policy in the active executable
from a captured request and answer, without sending Run. Captured work contains
the player's words, original definition, actual held kit, target part, floor and
room. The hero's scoped reservation survives implementation handoff; a new seat
occupant does not inherit it. A late answer cannot overwrite changed equipment,
a new floor or a new session. Failed/cancelled work releases only its own
reservation. Successful equipment changes retain spell cooldowns and existing
book definitions for issued effects.

The complete definition export includes guard behavior and Forge runes, which
the older spell-brief excerpt deliberately omitted. Replacement output must
round-trip through the actual checker, including these executable properties.
Journal schema 4 retains exact answers and terminal results with the existing
count/byte limits and per-ID acknowledgements. Live state schema 4 and private
checkpoint version 5 carry the reservation; genuine earlier versions migrate,
while older envelopes cannot conceal the new job data.

The native coordinator keeps its active model turn, queues only host handles,
and discovers captured work through the existing command cursor. Admission is
checkpointed before a model starts. Worker/IO failure retains the exact answer;
world and terminal receipt reach disk before ACK, followed by the retirement
checkpoint. Both brew and reforge delivery durably publish the accepted complete
definition before ACK, so a later painter reload cannot restore the model
draft’s earlier behavior. Publication failure retains the terminal receipt for
retry without evaluating newer policy or running another model turn. Restored terminal receipts skip newer policy, file reads and model
submission. Request-scoped filenames prevent same-word draft collisions. One
more direct App writer is removed, leaving 21. No crate or separate receipt
ledger was introduced. The joined-client file-send protocol remains a
compatibility path; this pass does not claim durable guest delivery or provider
stream restoration after a host-process restart.

All 2,824 Delve checks pass (300 marked ignored), including six new lifecycle
checks for exact handoff/completion, stale seats and changed kits/floors, bad
candidates, shared receipt backpressure, full-book rollback and complete
weapon/guard definitions. Five state checks and five native executable swap
checks pass. The reforge computation carries 1,009 bytes in and 402 bytes out,
without Run. Across 60 calls it measures 40 µs median / 49 µs p95, with automatic
activation 321 ms after executable publication. These figures exclude Rust
compilation, durable IO and the full App frame. The final worker/test rebuild
took 38.6 seconds after its dependency library had already been compiled.

Native validation passes 181 selected checks (13 marked ignored), including six
new coordinator checks and private reservation/terminal restore. Both explicitly
run native Sim replacement cases pass; the reforge case activates in 311 ms
while preserving the seat pointer, two players, cooldown 71, the old grant and
the pending reservation. The next answer comes from the new implementation.
The coordinator checks cover failed admission and delivery checkpoints, stale
sessions, logical rejection, restoration without file/model replay and failed
accepted-source publication followed by exact retry. Timing-sensitive host
checks ran without compilation overlap. All thirteen strict terminal runs pass:
ten native-worker scenarios and three embedded ability/VFX/fairy variants. The
retained bank-claim harness now checks private checkpoint version 5. The regular
App/terminal rebuild took 2m46s; its assertion-only harness rebuild took 2.44s.
All 55 selected packaging/install checks and staged runtime-helper closure
(7,627 entries) pass. Protected files remain unchanged. The earlier 0.2.6
integration trial predates this extraction and was not repeated; the documented
release allowlist gap remains separate from these selected gates.

The wish request/completion pipeline below follows this extraction, including
captured definitions, delivery/refund identities, scroll context and native
`Pool`/`Lesson.job` handles. Private playtest/book edits and remaining direct
writers follow that ownership work. Moving the journal and working Run into the existing
worker then enables changed-view publication through the existing wire
publisher/client. These policy/command ports do not complete game state
ownership or remove the compatibility bridge.

Idle module polling now reuses captured bundles after bounded metadata checks.
The 16 MiB asset-pack fixture reads zero payload bytes per idle poll and measures
40 µs median / 50 µs p95, versus 38.6 ms for forced full recapture. Changed
captures still require matching contents and stable metadata, and manual reload
always recaptures. In the strict terminal checks, an open stateful surface
repaints in 0.43 seconds and an executable ability replacement appears in 0.45
seconds while the original cast and host remain alive.

## Bend vertical extraction evidence (2026-10-26)

The production Fairy Stun Field now compiles from Bend through an optional
versioned source-build declaration in the existing JS backend. Its definition,
placement, pulse schedule, room/expiry checks and stun policy moved out of the
JS adapter. Independent art/sound/effect providers and the existing dependency
closure still own media composition. The module worker captures source/proofs,
checks them, emits bounded JS and admits a complete candidate. Old casts retain
their implementation/state/media; new casts receive the replacement. No new
crate, registry, watcher, public system or per-ability Rust branch was added.

Packaged source/output receipts bypass the optional compiler. Modified inputs
must compile and pass checks; stale artifacts and bad proofs keep the working
generation. Failed builds cache their captured metadata so idle polling does
not repeat compilation. Rust owns a private staging root, bounds builder IO,
kills the whole compiler process group before reaping, joins pipe owners and
removes private source captures even after forced termination. The default
compiler is pinned to Bend 2.0.39/`221a350c2f7f7134884740018e6e90001fb6d230`.
Bend runtime/Base attribution and its Apache-2.0 license accompany the emitted
artifact. These are ordinary-checker results; no `--verdict` result is claimed.

Validation: 46 ordinary Kernel/loader checks plus two distinct explicitly run
Bend checks (48 total), and seven process lifecycle checks pass. Logs:
`/tmp/angelx-bend-loader-cleanup-final.log` and
`/tmp/angelx-bend-loader-cleanup-real-final.log`. Tests cover exact receipts,
missing/invalid proof graphs, offline artifact operation, automatic source edits,
failed-candidate caching, IO limits and process/staging cleanup. The loader
fixture measures automatic activation at 593 ms after a Bend source write.

All 183 selected native checks and two explicit Bend cases pass (185 total):
`/tmp/angelx-bend-native-cleanup-selected-final.log` and
`/tmp/angelx-bend-native-cleanup-real-final.log`. They preserve multiplayer state
and cooldowns, reject a broken proof without replacing resources, keep a
3.5-radius/12-tick field, and run a 7-radius/24-tick replacement beside it against
real near/far enemies. VFX consume matching captured definitions. Room changes
retire the groups. Four concurrent fields through 120 complete rules/ability
ticks measure 1,651 µs median / 1,982 µs p95 (max 2,243 µs), with 44 area actions,
excluding rendering and compilation. Explicit capture/check/compile/activation
measures 221 ms. These are local shared-machine measurements.

All thirteen strict terminal runs pass: ten native-worker scenarios in
`/tmp/angelx-bend-native-pty-final.log` (32.77 s) and three embedded variants in
`/tmp/angelx-bend-embedded-pty-final.log` (9.16 s). Actual Bend edits activate in
628 ms native / 603 ms embedded with the same PID, an old field still alive,
one versus two controlled enemies, and cancellation/cleanup verified. The
normal App had a cached-object linker failure; rebuilding without incremental
reuse succeeded in 7m48s (`/tmp/angelx-bend-app-pty-build3.log`). No caches were
cleared. The final native unit rebuild took 52.38 s after the staging-ownership
fix; the broader preceding build took 3m58s. A two-provider test fixture was
updated to copy its complete Bend source/artifact bundle.

All 61 selected packaging/Bend/browser-media checks and staged runtime-helper
closure (7,636 entries) pass (`/tmp/angelx-bend-packaging-publish-final.log`,
`/tmp/angelx-bend-helper-closure-final.log`). Fresh-cache default compiler setup
also passes and returns Bend 2.0.39; its isolated test cache was removed afterward. Protected files are
unchanged; there was no reseal, public push, or new 0.2.6 integration trial. The
existing full documented-release allowlist gap remains outside these selected
gates. The preserved wish/grant extraction was restored after this coherent
Bend slice was published; the subsequent work is recorded below.

## Retained wish/grant extraction (2026-10-26)

Spent wishes now move into the existing bounded game command journal through
`delve.wish/1`. Captured definitions, maker, target, identity context and
step/gauge choices accompany the request. A terminal receipt owns the exact
accepted definition and grant/plain/refund/stale outcome. World changes and
receipt commit together; durable checkpoint and accepted source publication
precede acknowledgement. Reload/retry uses that terminal result rather than
consulting a changed phrasebook or repeating policy/model effects.

Native `Pool` runners and `Lesson.job` handles remain native resources. Choosing
an already running speculative lesson promotes the same job. A session change
detaches its coordinator without stopping its model call. Seat incarnation
counters additionally prevent same-name departure/rejoin from adopting the
previous occupant's speculative work. Departing seats discard unclaimed words;
already captured claims become stale without granting or refunding a newcomer.
There are at most six seat counters and the journal retains its existing count
and byte bounds, per-ID acknowledgement and admission backpressure.

Live state schema 5, journal schema 5 and private save version 6 migrate genuine
previous envelopes. Current state cannot be relabeled as an older schema to
hide wish reservations or seat ownership. Definition export retains resolved
art and executable tiers. Source publication uses the existing private atomic
writer and an advisory owner lock, preserves other entries, and deduplicates
simultaneously returned names and IDs before publication. A policy cannot replace
a different card definition under an ID another knight already holds; the
request refunds before any grant. Shared held definitions retain their original
maker. A changed definition needs a new wish ID. Per-character generation pins
for the broader mutable card catalogue still require extraction; this port must
not silently rewrite an older grant while that work remains open.

All 2,835 game checks pass (300 explicit review/benchmark cases remain ignored),
including eleven wish ownership cases. The latest full run took 82.42 seconds;
its final rebuild took 1m54s (`/tmp/angelx-wish-core-full8.log`). Exact whole-hero
rollback prevents a refused target change from healing a wounded summoner.
Exact tier identity, using the existing wish-ID parser, also prevents upgrading
one gift from removing a separately named variant or its cooldown.

Seven new native coordinator cases cover admission and delivery checkpoint
failure, terminal restore without model/effect replay, speculative promotion,
seat reuse, simultaneous names, private source lock contention, and a conflict
that preserves both source bytes and the actual held card. All 190 selected
ordinary native checks and ten additional grant-runner cases pass; five explicit
native swap/Bend cases also pass (205 distinct native checks total). Logs are
`/tmp/angelx-wish-native-focused6.log`, `/tmp/angelx-wish-native-selected6.log` and
`/tmp/angelx-wish-native-swaps1.log`; the native unit rebuild took 7m45s.

The real wish swap keeps a native model call running, preserves the same pool
and seat objects, rejects an invalid executable without replacing the working
worker, retains the old actual card and exact terminal receipt, then grants a
fresh pending definition through the new policy. A logical rejection refunds
once. Publication of an already built implementation to observed activation
measures 310 ms. Twelve policy RPC plus atomic-delivery calls measure 185 µs
median / 229 µs p95 and maximum. These local shared-machine figures exclude
Rust compilation, model calls, rendering and durable IO. Five worker state
checks and five native executable replacement checks pass after a 1m18s worker
rebuild (`/tmp/angelx-wish-system2.log`).

The explicit Bend checks also pass with the new state codec: field compilation
and activation measure 282 ms; four fields across 120 complete rules/ability
ticks measure 2,230 µs median / 3,941 µs p95 (44 area actions, no rendering or
compilation). This sample does not establish a causal speed change from the
preceding shared-machine measurements. The 61 selected packaging/Bend/browser
media checks and final staged helper closure (7,642 entries) pass. All thirteen
strict terminal scenarios pass: ten native-worker cases in 32.86 seconds and
three embedded cases in 9.60 seconds. Actual Bend source edits activate in
605 ms native / 628 ms embedded, with the same PID, old/new fields and real
near/far enemies. Logs are `/tmp/angelx-wish-native-pty-final.log` and
`/tmp/angelx-wish-embedded-pty-final.log`. The regular App and fresh terminal
harness rebuilt in 6m47s with incremental reuse disabled, preserving existing
caches (`/tmp/angelx-wish-app-pty-build1.log`). No new 0.2.6 integration trial or
full documented-release gate is claimed. Protected files remain unchanged.

The world and journal still reside in host `Core`, and ticks still cross the
full Run compatibility bridge. These retained work ports make later worker
ownership possible; they do not establish local worker state or view deltas.
The fresh audit finds 13 remaining production App mutable-Run sites, down from
21 at the reforge boundary (`/tmp/angelx-wish-remaining-app-run-writers.txt`).
These include speculative lamp/scroll/book operations, private playtest state
and initial outfit setup. Continue those extractions before
removing `RunMut` or claiming the broader ownership boundary is complete.

## Painter catalogue extraction (2026-10-26)

Painter completion now enters `delve.card-paint/1` on the existing replaceable
rules worker. Native files, sheet resolution, painter children and completion
notifications retain their existing host owners. The policy receives only the
captured current/source cards affected by one completion (at most eight, within
one MiB), and returns a bounded subset of those IDs. It receives no Run or
checkpoint. No new package, registry, watcher or effect ledger was added.
The worker's accumulating `serve_with_*` forwarding functions were consolidated
into one named `Policies` composition point.

The default admission policy declines a source whose gameplay definition has
changed. Maker metadata and visual progress are independent. A replacement
policy may admit that source's visuals, but the commit port structurally carries
only art/looks plus the exact expected catalogue entry. It cannot replace names,
cooldowns, effects, weapon definitions or makers. Every entry in a batch is
checked before any write; a newer definition or paint makes the entire old batch
stale. Legacy worn attachment metadata also defines identity parts: a visual
update that changes that classification or creates an unearned body piece is
rejected, including when a replacement policy requests it. Identity migration
needs an explicit future contract. Existing command receipts provide replay, byte/count bounds and atomic
world/result commit. Native admission failures retain the completion through the
existing painter channel; retrying does not restart its model or process.

All 2,840 game checks pass (300 explicit review/benchmark cases ignored),
including five painter contract cases: exact capture and visual-only changes,
replay/handoff, atomic stale-batch rejection, bounded input/result admission and
retained identity classification. The full run took 87.17 seconds; the latest
focused rebuild took 4m43s (`/tmp/angelx-paint-core-full6.log`,
`/tmp/angelx-paint-core5.log`). All five worker state and five executable
replacement checks pass after the final worker rebuild in 35.43 seconds
(`/tmp/angelx-paint-system2.log`).

All 214 selected ordinary native checks and six explicitly run swap/Bend
checks pass (220 distinct checks): `/tmp/angelx-paint-native-selected3.log`
(77.07 seconds) and `/tmp/angelx-paint-native-swaps3.log` (6.13 seconds).
The selection includes thirteen real painter process/media cases and the new
App retry case. The actual replacement test keeps the same Sim/seat objects,
admits a previously declined source through a new policy, checks the exact held
card and non-book world state, rejects a bad executable, and verifies replaced
and current worker PIDs are reaped after retirement. An older same-name wish test
now waits for observable lesson completion instead of the model's counter,
which drops before the answer reaches its runner channel.

Publication of the already built paint implementation to activation measures
329 ms. Twelve captured-card policy RPC plus atomic journal commits measure
323 µs median / 866 µs p95 and maximum, excluding compilation, model calls,
rendering and durable IO. The preceding measurement on the same production
source was 323 ms / 156 µs median / 388 µs p95. These are local shared-machine
samples, not a causal speed comparison. The latest complete native unit rebuild
took 8m31s; the final test-only synchronization rebuild took 1m20s
(`/tmp/angelx-paint-native-build2.log`, `/tmp/angelx-paint-native-build3.log`).
The explicit Bend checks also pass: 705 ms capture/check/compile/activation,
and four fields through 120 complete rules/ability ticks at 4,916 µs median /
6,767 µs p95 (44 area actions, no rendering/compilation in the tick sample).

The staged runtime helper closure passes with 7,646 entries. No regular App or
terminal harness rebuild is claimed for this painter slice; the preceding
thirteen strict terminal and 61 selected packaging/Bend/media checks remain the
preceding pass's evidence. Current native App/Sim and real painter tests exercise
the changed path. Protected files remain unchanged; there was no reseal, public
push, or new 0.2.6 integration/release trial.

This removes the two production App book writes in `watch_painters`, leaving
11 direct mutable-Run sites (`/tmp/angelx-paint-remaining-app-run-writers.txt`).
Scroll offer/speculative lamp/ladder metadata, initial outfit, playtest and
explicit card reload still need extraction. The service's spell-file loader is
another direct catalogue owner outside this App-site count. Legacy projectiles
still resolve card visuals by ID, so visual generation pins for those shots
remain open; the existing executable character-pack casts retain their separate
captured generation/media leases. This pass preserves gameplay effects, seats,
cooldowns and held definitions; it does not establish legacy visual pinning or
worker-local Run/journal ownership. Continue the remaining command ports before
removing the full-Run tick compatibility bridge and publishing view deltas.

## Scroll policy and retained choice sources (2026-10-26)

Scroll opening now uses `delve.scroll/1` through the existing rules worker and
`Policies` composition point. Local words, remote learned words and known wish
IDs enter the existing command journal. The policy receives one knight's bounded
metadata (currency, identity, spell headers, deck IDs, genie state and choice
summaries), not a Run, avatar, executable tiers or sprites. Native capture keeps
one immutable phrasebook generation. The same default policy implementation
serves embedded and native execution; there is no second copy of its decisions.
This remains a Rust executable policy backend; the separate Bend ability backend
is unchanged. No crate, watcher, registry or independent receipt ledger was added.

An open scroll privately retains the exact resolved known/whim definitions and
built-in spell it offered, the next tier, raid and seat incarnation, displayed
blood price and currency kind. A source or policy replacement can change a new
scroll without changing the meaning of an open choice. Shared held IDs cannot
be overwritten with different executable definitions; equivalent definitions
retain their canonical maker and current visual progress. Refused grants restore
the complete hero, preserving wounded health and target state. Changed tiers,
spell slots/definitions and stale owners require reopening without payment.

Live Run schema 6 and private save version 7 retain these sources. Genuine older
offers receive an explicit legacy marker: their UI remains dismissible, but
choosing requires reopening because the original source cannot be recovered.
Relabeled older envelopes containing new source fields are rejected. Public
multiplayer serialization omits the binding before first/delta publication;
worker policy requests carry only the previous UI. The legacy fixture save writer
rejects new bound sources instead of labeling them as a version-1 checkpoint.
Journal replay skips policy execution, and a rejected multi-command batch commits
neither a partial offer nor its sound.

All 2,851 game checks pass (300 explicit review/benchmark cases ignored),
including eleven new scroll cases: retained sources and handoff, real second
public publication after a source edit, legacy reopening, shared-ID conflicts,
frozen price/currency, changed tier/seat/spell, unavailable freedom, bounded
metadata/result admission and exact journal replay/batch rollback. The full run
took 90.06 seconds after a 5m22s rebuild (`/tmp/angelx-scroll-core-full5.log`).
All five worker state and five executable replacement checks pass after a 2m43s
worker rebuild (`/tmp/angelx-scroll-system1.log`).

All 215 selected ordinary native checks and seven explicitly run swap/Bend
checks pass (222 distinct): `/tmp/angelx-scroll-native-selected5.log` (68.41s)
and `/tmp/angelx-scroll-native-swaps5.log` (6.59s). Private-save tests retain an
open definition through compact checkpoint/restore, reject a falsely relabeled
older envelope and refuse a genuine legacy choice without charge. Two older
save-version assertions were updated; the existing forced-whim fixture now
captures its replacement source before publishing that authored choice.

The actual scroll test edits the phrasebook and replaces the executable while
an offer and native model call are live. The old offer grants its old resolved
definition; a new offer uses the new source and replacement price (15 health
instead of 30). The test explicitly retires the older held ID before allowing
its canonical definition to change; this does not establish legacy card
generation coexistence. The same runner/seat objects, two players, cooldowns
and issued cue survive, the model returns exactly once, an invalid executable
leaves the working policy usable, and both retired/current worker PIDs and the
test's artifact directory are cleaned up.

Publication of the already built scroll implementation to activation measures
299 ms. Twelve metadata RPC plus atomic journal commits measure 1,191 µs median /
1,389 µs p95 and maximum. These shared-machine samples exclude Rust compilation,
model work, rendering and durable IO; no own compilation overlaps the runtime
sample. The full native unit synchronization build took 5m59s; the final test
synchronization rebuild took 3m08s (`/tmp/angelx-scroll-native-build2.log`,
`/tmp/angelx-scroll-native-build4.log`). The explicit Bend checks also pass:
212 ms capture/check/compile/activation, and four fields across 120 complete
rules/ability ticks at 1,508 µs median / 1,802 µs p95 (44 actions; no rendering
or compilation in the tick sample). These figures do not establish a causal
speed change from earlier shared-machine samples.

The staged runtime helper closure passes with 7,650 entries. The regular App
and terminal harness were not rebuilt for this slice; the preceding strict PTY
and selected packaging/Bend/media results remain scoped to their recorded heads.
Protected files remain unchanged; there was no reseal or public push.

Private `dev` advanced to `be287282d9f564e66a0c265450604d7644ba67e9`
during this pass. A read-only merge audit found three conflicts already present
against the preceding refactor head: `dungeon_realm.rs`, the supply-chain policy
and its release-evidence test. The incoming country/holdings update must move
inside the retained realm transaction; taking the old App mutation would undo
that extraction. Reconcile this private base, preserve its owner-approved lock
manifest/bytes and rerun integration checks before the next vertical extraction.
This pass's results are the unmerged 0.2.5 source, not a new 0.2.6 trial.

The production App audit now finds ten mutable-Run sites, with inline test
fixtures excluded (`/tmp/angelx-scroll-remaining-app-run-writers.txt`). Next:
speculative lamp sent/dropped/retirement and ladder/maker metadata; then explicit
catalogue/service loaders, initial outfit, playtest and appearance installation.
The host still owns the Run/journal and ticks still serialize the full Run,
including retained open sources. Worker-local state and view deltas, legacy card
visual generations, joined delivery, broader agent/provider/tool/workflow and
remaining authored Glass content remain open. This extraction does not complete
those boundaries or the full modularity refactor.

## Private 0.2.6 base reconciliation

The scroll extraction is published as `dcf4d03e` in private draft PR #29.
Its 0.2.5 results remain scoped above. This integration includes
private `dev` at `be287282d9f564e66a0c265450604d7644ba67e9`.
Three conflicts are resolved: the App keeps its retained realm-claim flow,
the incoming country/holdings update is applied inside the durable settlement
transaction before rent day, and the merged lockfile hashes/counts are recorded
in the existing supply-chain policy (Cargo 520/378, npm 79).

A new regression exercises the country larder reward through an uncertain
publish and repeated claim confirmation, so a recovered receipt cannot issue
another sack or rent day. Before the parallel extractions below, the merged
0.2.6 game suite passed 2,890 checks (300 ignored), and 78 selected packaging,
install, provider, media and Bend checks passed. The corrected public document
inventory and generic fixture paths pass the actual required/public/documented
source/runtime-helper closure and 28 release-evidence checks. Logs:
`/tmp/angelx-dev-integration-core1.log`,
`/tmp/angelx-dev-integration-packaging1.log`,
`/tmp/angelx-dev-integration-closure3.log`, and
`/tmp/angelx-dev-integration-release3.log`.
The incoming operator lock manifest and protected bytes are preserved exactly;
the read-only lock check passes without resealing. The durable claim also now
publishes canonical ranch wins, so uncertain replay and a fresh realm load
observe the same home state.

## Retained host command extractions (validated integration)

`AGENT_COORDINATION.md` is the shared ownership document. Sol 6.1 low agents
implemented speculative metadata and catalogue ports; Luna low implemented
release closure and vote exports. Root integrated the registrations, settlement
port, reset ownership and tests in one build queue. Their next session/view,
context and artifact-lease deliveries remain isolated and uncompiled. Claude
lanes remain open for authored Glass content and agent/provider/tool execution;
claims belong in the canonical document.

Speculative lamp, maker and ladder metadata now enters the existing atomic
Journal, with captured source, raid, seat and native session ownership. An
unchanged ladder does not submit an empty transaction. Replacing a session
detaches old Lessons without cancelling their native jobs: returned answers
remain attached to their original job, library and workspace, and publication
retries through the existing atomic source writer without adopting old work
into the replacement game. The actual `/cd`, `/dungeon off` and `/together off`
reset paths keep those jobs, and the ordinary frame pump drains detached answers
even without a game scene. Conflicting operator source retains the answer.

Catalogue ports capture immutable card/boss inputs before leaving their native
owner, then fence admission against observed definitions and delivery seats.
Held definitions include floor items, wardrobe and queued brew inputs; pending
brew/reforge result IDs remain reserved by their existing Journal work. A native
queue retains exact unadmitted inputs through receipt pressure. Admitted refusal
can recover from changed source, including oversized input correction; stale
owners cannot import an old targeted delivery into a new seat. Duplicate card
IDs and boss targets reject atomically. Cooldown-only spell edits retain the
canonical maker/visuals, current recharge, issued effects and existing floor
items; subsequent casts use the new recharge. Recipe inputs/stand-ins and open
scroll sources stay frozen. These are command ports, not independent executable
policy replacements or legacy visual generation coexistence.

Playtest exports use a retained Journal claim before external IO. Journal
schema 6 adds initialization state; genuine older private facts are recovered
without overwriting them with stale realm values. Claim capture transfers the
old change queue atomically, leaving subsequent events separate even when their
values repeat after restart. Stable export IDs, the existing durable atomic
writer and acknowledgement of the original receipt handle uncertain completion
without another admission slot. Full private checkpoints precede IO and follow
retirement; an unavailable checkpoint path refuses export.

Saved settlement admission uses bounded version-1 public geometry through the
same Journal. Private artifact metadata stays native-owned. Invalid maps,
markers, stale new-run state and wrong-site navigation reject atomically; the
App's hall command reports failed admission rather than claiming navigation.

Published `37e41b1544c7` combined-source evidence:

| Scope | Current result | Evidence |
|---|---|---|
| Full Delve core | 2,921 pass; 300 explicitly ignored review/benchmark cases; 12.67 s after 5m46 build | `/tmp/angelx-parallel-core7.log` |
| Worker state and executable replacement | Five state + five actual replacement checks pass; 1.45 s + 1.75 s after 1m38 build | `/tmp/angelx-parallel-system4.log` |
| Native coordinator, jobs, controls, realm, appearance and harness selection | 849 pass; 28 explicitly ignored; 177.64 s after 9m34 native build | `/tmp/angelx-parallel-native-tests2.log`, `/tmp/angelx-parallel-native-build2.log` |
| Explicit native replacement/Bend checks | Seven pass in 6.47 s, including five policy replacements, real field behavior and four-seat runtime | `/tmp/angelx-parallel-native-swaps2.log` |
| Source/runtime release closure | 7,664 staged entries pass required/public/documented/helper closure and Node lock inspection | `/tmp/angelx-parallel-release-closure-final.log` |
| Selected 0.2.6 packaging and release-evidence checks | 78 packaging/install/provider/media/Bend + 28 release-evidence tests pass before these ports; packaging sources unchanged | `/tmp/angelx-dev-integration-packaging1.log`, `/tmp/angelx-dev-integration-release3.log` |

The fresh native swaps preserve runner/seat identity, active model work, retained
answers/receipts, cooldowns and issued effects; their lifecycle assertions also
check rejected candidates and retired worker PIDs/artifacts. Built-publication to
activation measures 297 ms Brew, 320 ms Reforge, 309 ms Wish, 308 ms Paint and
304 ms Scroll. Twelve Wish RPC+atomic deliveries measure 202 µs median / 232 µs
p95; Paint 219 / 453 µs; Scroll 1,192 / 1,546 µs. Actual Bend capture, proof,
compile and activation measures 211 ms. Four captured fields across four seats
run 120 complete rules/ability ticks at 1,564 µs median / 1,791 µs p95 (44 area
actions). These are local shared-machine samples with other agents on the host;
our builds ran sequentially and ended before the measurements. The policy
samples exclude compilation/model/rendering/durable IO; the field runtime
excludes compilation/rendering. No whole-application frame-time claim is made.

The integration runner initially found a wrong test cwd, an invalid large-card
fixture, a held-spell recharge regression and a canonical realm replay mismatch.
Those are corrected; peer review also caught and fixed metadata edits redelivering
a full-slot floor spell. The new core source-membership fixture was corrected to
check the captured wish, rather than assume an open scroll has only one source.
All 2,580 source fingerprints remained identical through the final native gates.

At published `37e41b1544c7`, Run and Journal remain host-owned and the last
production App mutable-Run site is appearance installation. The next extraction
below removes that App writer and establishes the owned-session mechanism. The
native server authority switch is still a separate, unfinished boundary.

## Owned-session foundation — published private draft

This pass establishes a working exclusive owner and native transaction seams;
it does not yet switch the cockpit's native coordinator to that owner. No new
crate, registry, watcher, cast ledger or public subsystem was introduced.

| Boundary | Implemented ownership and contract |
|---|---|
| Kernel process owner | `kernel.owned-state:1` wraps the existing System. The worker owns mutable state; the supervisor keeps a checkpoint, bounded accepted input/semantic-witness tail, exact unaccepted input and an immutable executable lease. New policy handoff uses the current active checkpoint; crash recovery replays only with the pinned original image. |
| Delve owner | `delve.session:1` keeps Run, the existing Journal, scoped immutable settings/phrasebook context, exact sequence/revision and a retained open native frame. Checkpoint schema 1 contains runtime schema 7 and Journal schema 6. Native resources, models, file IO, audio and VFX leases stay native. |
| Link read view | API-1 bounded baseline/delta frames publish immutable public state, without private allocator, queues, claims or native appearance. Dictionary/tree/budget changes stage atomically. Shared Book revisions and dictionary/subtree leases avoid rebuilding unchanged data. The typed receiver still materializes a full public Run; its runtime cost remains open. |
| Native ability transaction | All participating provider actor transitions, exact request ownership, aggregate game actions, audio admission and VFX stage before world admission. Exclusive actor guards commit only after known ACK. Uncertain frame/source callbacks retain their exact prepared plan, resource leases and intents; an already accepted frame is reconciled through its phase bit without reapplying world effects. |
| Prepared character installation | App hotload delegates one resource selection to Sim; native slot/art installation takes one core lock and reapplies unchanged selections after Run/server replacement. Production `RunMut` is removed; the direct guard is fixture-only. This remains the compatibility native owner until the worker switch. |

BeginFrame captures exact seat incarnations, controls, ages, departures, presence,
arrival facts, inputs and trusted retained native casts. AbilityBatch is the only
mutation permitted before FinishFrame. Finish returns ordered departure notices,
party/private sounds, notices, chorus cues and feel output once. Pending phases,
paid casts, cooldowns and already-issued effects survive checkpoint/handoff;
new seats cannot inherit old permissions. Held executable input coalesces the
same outstanding exact source until explicit ACK/cancel, keeping its original
request tick. Genuine schema-6 unresolved ability outboxes refuse migration
rather than invent an incarnation or silently discard work.

Exact HomeReady, VotesReady, BrewReady, queued-wish, ability and frame-phase
queries replace private-state guesses. Immediate controls/campaign events and
owner-executed authored paint policy use existing game/Journal contracts. Client
validation fences reply type, receipt/claim identity and complete accepted public
state before commit. Shared identity helpers prevent independent client encoders.

Fresh core validation passes 2,990 tests (300 explicitly ignored) in 11.20 s
(`/tmp/angelx-owned-core6.log`), including real JSON envelopes for every ability
event and the pending paid-cast checkpoint. Net passes
69 tests (`/tmp/angelx-owned-net5.log`), including both compatibility and exact
nested ordered-delta cases. Kernel passes 68 unit, seven real process
and two compile-fail guard checks (`/tmp/angelx-owned-live4.log`,
`/tmp/angelx-owned-live-process4.log`, `/tmp/angelx-owned-live-doc4.log`).
The first full core run found a real exact-order delta bug: omitted struct/map
entries reappeared at the tail after patch application. Owned views explicitly choose exact-order diffing through the shared recursive
implementation, using existing Set patches when order cannot be represented.
The ordinary replication diff retains its existing compact representation; the
full core gate confirms legacy byte budgets and golden streams remain exact.
The real process gate also caught internally tagged ability events buffering
numeric seat-map keys as strings. Adjacent tagging now preserves typed decoding;
the schema was introduced in this pass. Two drain fixtures also retained startup cues; the fixtures now
isolate the injected output and assert one sound. A same-frame renderer mismatch
occurred once and did not reproduce in focused, parallel-render or five full
old-binary diagnostic runs; its cause remains unresolved and was not hidden by
changing the assertion. The owned worker gate passes 30 checks: 15 dispatch/Client, four actual owned
process handoff/recovery cases, six state and five executable replacement tests
(`/tmp/angelx-owned-system6.log`). This includes handoff during an open frame,
accepted ability batch replay after killing the replacement worker, retained phase
and exact single projectile/cue/sound delivery, plus retired PID/image cleanup.
All 3,173 source fingerprints stayed unchanged through the foundation gate.
After that gate, the Client was hardened to require the raid inside captured brew
work to match both job-claim and command-receipt scope. A new regression sends
valid-looking foreign-raid work through each path; all 16 worker library tests
pass (`/tmp/angelx-owned-brew-fence1.log`). The subsequent full worker gate
passes 31 checks: 16 library, four owned-process, six state and five replacement
(`/tmp/angelx-owned-system7.log`). Only the worker Client changed since the
native build source baseline; it is not a cockpit dependency. Ability outboxes
intentionally continue to expose stale retained requests for explicit retirement;
rejecting them at the Client query would strand checkpoint state already accepted
by the owner. Native production checking passed and the native test binary built
in 31m35s. Its first behavior run had 856 passes, 28 ignored and three failures
(`/tmp/angelx-owned-native-tests4.log`): two fixtures expected the authored name
instead of the `slot1` alias, and one system fixture omitted its required instance
constructor. The revised assertions check both alias and executable binding; the
fixture now implements the system instance contract. A fresh test binary built
in 18m41s and all three corrected tests passed
(`/tmp/angelx-owned-native-build6.log`, `/tmp/angelx-owned-native-focused6.log`);
source fingerprints were unchanged during that recheck. The seven explicit native
replacement/Bend checks passed in 8.26s (`/tmp/angelx-owned-native-swaps4.log`)
on the preceding source-equivalent production code. The clean-source release
gate passes at private commit `66642fade11e` (`/tmp/angelx-owned-release-final2.log`);
its source archive and manifest are in `~/.codex/angelx-owned-release-final`.
A fresh-host build was not repeated.

Four-seat/300-added-spell actual worker measurements: artifact publication to
activation 331.163 ms; 24 raw Tick exchanges measure 4,308 µs median / 5,282 µs
p95 / 6,586 µs maximum. Median JSON Frame size is 494 bytes against a 165,890-byte
full Session checkpoint. Tick samples include RPC, owner validation, delta and
typed Client; they exclude render, native ability work, models and durable IO.
Activation excludes Rust compilation. These are shared-workstation samples while
other agents use the machine, not complete native frame-time acceptance. Typed
receiver/domain-cache work and the actual native three-phase frame remain open.

Release path closure covers 7,678 selected files, including all 14 new untracked
source files, and all 129 release tests pass
(`/tmp/angelx-owned-release-closure1.log`). The committed source-archive gate
passes for `66642fade11e`; the preceding published archive is recorded above.

### Read-view reuse — verified follow-up

When a validated delta changes only owner status or private work, Receiver now
retains the previously validated `Arc<Run>`. It still validates the staged wire
tree and the new status before commit; any changed public subtree, reordered
field, or baseline decodes and validates a fresh Run. Existing views remain
immutable. The full Delve suite passes 2,992 tests (301 ignored), and all 31
worker checks pass including real process handoff/recovery
(`/tmp/angelx-view-core1.log`, `/tmp/angelx-view-worker1.log`). Native production
checking also passes (`/tmp/angelx-view-native-check1.log`, 57.09 s); the full
native behavior suite was not repeated for this internal view change.

In paired fast-profile receiver-only runs with four seats, 300 added cards,
64 warmups and 512 samples, status-only acceptance changed from 915.060 µs
median / 956.217 µs p95 to 11.020 / 14.828 µs. Game-changing deltas still
decode a Run: baseline 959.414 / 989.169 µs, candidate 984.369 / 1,192.370 µs.
These are shared-machine samples, not end-to-end frames, and exclude publisher,
RPC and rendering; the game-changing p95 difference is not attributed to the
change without controlled repeat measurements (`/tmp/angelx-view-baseline1.log`,
`/tmp/angelx-view-candidate1.log`). This optimization does not remove full
materialization for normal gameplay edits or complete the native owner switch.

Next extraction: replace native Core's retained Run/Journal and full-Run tick RPC
with Embedded(Session) or Worker(OwnedSystem + read-only Client), selected only
at session initialization. Feed native appearances/Scene as presentation overlays;
route every service job/intention through the same owner, and resolve retained
frame/source transactions before accepting later work. Keep the last accepted
snapshot readable independently of the mutating owner lock: the current
`Shared::fresh` core lock would otherwise make Glass wait through a slow worker
RPC. Queue/admit intentions without forcing ordinary drawing to wait on that
exchange, and prove delayed-worker UI/read behavior. Persist only at explicit
quiescent checkpoints, preserving the existing durable source/receipt ordering.
Never recover an uncertain worker request by silently switching to embedded code.
The first combined native cutover draft now passes production `cargo check` in
`~/.codex/worktrees/native-owner-integration/angelX` after four
type/borrow fixes (`/tmp/angelx-native-integration-check2.log`). Ticket, home
and three worker fixture drafts have since been composed, and the combined
production check also passes (`/tmp/angelx-native-integration-check4.log`).
The first test check found 33 obsolete fixture/lifetime errors; after partial
migration the second found 15 (`/tmp/angelx-native-integration-testcheck2.log`):
14 were in real-worker paint/scroll fixtures and one ticket test clone was
fixed. The lifecycle draft migrates those fixtures and retains the original
worker seed when startup is unavailable; test check 3 then exposed a Cargo
feature-unification error in the fixture-only command reply validator
(`/tmp/angelx-native-integration-testcheck3.log`). The isolated integration
now explicitly enables that validator with the test fixture feature, while the
fixture worker still requires capability negotiation. Test check 4 passes
(`/tmp/angelx-native-integration-testcheck4.log`); all four separate feature
exclusion, policy and worker-capability tests passed, with logs
`/tmp/angelx-fixture-{exclusion,inclusion,worker-default,worker-opt-in}.log`.
The first attempted featureless
Delve test configuration could not compile existing tests that require the
renderer; the retry retains the default renderer while still excluding
fixture-control. Runtime behavior and the full native worker tests remain
unverified for this draft.
Startup continuation, other one-shot callers and durable home lineage remain open.
The caller audit also found immediate navigation after startup, host/kick
teardown before owner receipts, direct guest invitations, and several gear,
vote and settlement actions whose uncertain results lack retained callers.
Offer Drop/Turn, startup/install continuations and a bounded durable home lineage
draft are now composed in the isolated integration checkout. Combined test
compilation found one error (`/tmp/angelx-native-integration-testcheck5.log`), and
none of these additions has passed runtime tests. Review found two startup
defects: waiting for installation can prevent the old owners from being pumped,
and a repeated start can replace the sole caller-held installation ticket. Fix
and test both before cutover. The lineage draft fences uncertain accepted replies
across a crash rather than claiming to recover them; durable authority receipts
remain necessary. Do not enable the owner cutover while these flows can discard
or overtake accepted commands.

Test check 5 found one test-only `PathBuf` type error in the lineage fault
fixture; it has been corrected and check 6 passes
(`/tmp/angelx-native-integration-testcheck6.log`). Further review found
that temporary failure to acquire an initial ticket can drop startup or offer
input, settlement retry can lose a deferred start, and lineage storage capacity
double-counts its reserved answer space after admission. Independent fixes and
fault tests are being prepared; these are open correctness defects, not accepted
behavior. The earlier four feature tests only validate the fixture protocol,
not the composed native owner or these new workflows. Isolated patches for
startup liveness, caller admission retention and lineage reservation accounting
are now composed, and combined test check 7 passes
(`/tmp/angelx-native-integration-testcheck7.log`). Their regression tests and
the six real-worker lifecycle tests ran under `angelx-native-runtime-suite.service`
with identical source fingerprints before and after. Six lineage tests and the
focused startup/install/source tests passed; five of six real-worker scenarios
passed. Both offer ownership tests panicked and the real-worker scroll test
failed its offer-ticket fence. Those three failures are under investigation
(`/tmp/angelx-native-runtime-suite-run.log` and `/tmp/angelx-native-case-*.log`);
the cutover is not accepted. Draft asynchronous candidate preparation with
revision-fenced source/art updates, plus deferred-seat reconciliation, has since
been composed. Test check 8 found test-fixture compilation errors; these changes have no runtime
evidence yet and require fresh lifecycle and cleanup tests. Crash recovery of an
accepted native reply still requires a durable authority journal.
Test check 8 found two private-field access errors in the new App transport
fixture; its test-only seam and asynchronous polling are being corrected. The
scroll test now uses the real offer-ticket API in the isolated checkout, but its
worker run has not yet been repeated. The two offer panic backtraces reach
`Live::stop` during manual `Sim::set_session`, before offer processing; manual
startup temporarily cleared its only retained `Shared` before installing its
server owner, and invalid seed creation dropped a completely ownerless `Live`,
masking the original error. A fix is composed rather than switching the tests
to threaded mode; the underlying seed validation still needs investigation.
Test check 9 exposed an outdated numeric revision assertion;
it now checks generation identity, and test check 10 passes
(`/tmp/angelx-native-integration-testcheck10.log`).
Async preparation's audit adds candidate quarantine after a worker-thread panic
and unique source-generation identities; the selection and deferred-seat tests
are now composed but unrun. A further first-install race left tuning changes
behind during preparation; a candidate-metadata fix is composed and test check 11
passes (`/tmp/angelx-native-integration-testcheck11.log`). Review also found that
a failed native service-thread spawn could be reported as successful startup; a
fix and fault test are composed. Test check 12 passes
(`/tmp/angelx-native-integration-testcheck12.log`). Whole-owner async cancellation and
replacement still need process/watcher cleanup evidence. These remain draft
changes until exercised together. A fresh focused runtime suite, including the
async/source-generation and service-admission tests plus the previously failing
offer and scroll cases, is running as `angelx-native-runtime-suite-v2.service`;
it records source fingerprints before and after and bounds each test invocation.
That suite completed with identical fingerprints: 11 focused cases passed,
including the corrected real-worker scroll swap, async preparation, panic
quarantine and service-admission retry; five cases failed. Two character-source
tests found an empty registry fixture, two offer tests found no active session
after seed validation, and the deferred-seat test encountered a checkpoint
notice in its fixture. These failures are being diagnosed without relaxing the
assertions (`/tmp/angelx-native-runtime-suite-v2-run.log`). A real-worker test
for dropping an in-flight candidate and retiring its worker, watcher and image
is composed but not yet run; test check 13 passes
(`/tmp/angelx-native-integration-testcheck13.log`). The v2 runner did not repeat
the earlier brew/reforge/wish/paint or initialization cases on this source, so
their prior passes must not be reported as current-source results.
The character-source fixture used invalid export names; it now uses the actual
seat-specific resource names and checks typed bindings. Test check 14 passes
(`/tmp/angelx-native-integration-testcheck14.log`), but the corrected cases
still need runtime execution.
The offer fixtures also violated the existing scroll checkpoint contract: their
choices had zero steps and no binding. They now use a valid bound choice, and
the deferred-seat test has its own private checkpoint path. Test check 15 passes
(`/tmp/angelx-native-integration-testcheck15.log`). Runtime suite v3 is running
against a fixed source snapshot and includes the corrected cases, the earlier
real-worker swaps and the new worker/watcher/image teardown test
(`angelx-native-runtime-suite-v3.service`).
Independent adapter review found a production brew cancellation gap: after an
uncertain `CancelBrew` reply, the exact reason is not retained and the model job
can be started again before retry. A correction with lost-reply coverage is in
progress; the v3 snapshot does not include it.
Runtime suite v3 completed with 25 passing checks and two failing offer-owner
checks (`/tmp/angelx-native-runtime-suite-v3-run.log`), with identical source
fingerprints before and after. All six real-worker initialization and policy
swap scenarios passed on that snapshot, as did the Linux in-flight candidate
teardown test for worker PID, watcher and captured image. The two offer tests
now reach admission but reject their observed offer as changed; their cause is
under investigation. A retained brew-cancellation fix and a seat-fenced kick
continuation are composed in the isolated checkout and not yet runtime-tested;
test check 16 passes (`/tmp/angelx-native-integration-testcheck16.log`). Separate
audit found that RPC-thread startup failure
may leave worker descendants and that a cleanup-test panic guard could signal a
reused PID; corrections are in progress. A hard UI reset can still discard
brew/reforge coordination and is not solved by the narrow cancellation fix.
The current isolated draft now includes the group-cleanup and reset-ownership
changes; test check 17 passes (`/tmp/angelx-native-integration-testcheck17.log`).
Runtime suite v4 is running against a fixed fingerprint and includes the brew,
kick, painter and RPC-spawn regressions (`angelx-native-runtime-suite-v4.service`).
Review identified a shutdown deadlock when deferred join/action notices wait
behind a pending close while the close waits for that queue to drain. A further
correction is being prepared; v4 does not include it, so even a clean v4 run
will not qualify the cutover.
V4 completed on unchanged source with 28 passing checks and three failures:
offer admission while another owner is busy, the brew cancellation fixture, and
the kick lost-reply fixture. All real-worker swap/initialization/cleanup checks,
the painter reset check, and the RPC descendant cleanup check passed. Detailed
logs are `/tmp/angelx-native-runtime-suite-v4-run.log` and
`/tmp/angelx-native-v4-case-*.log`. The deferred-transport close fix is now
composed and passes test compilation (`/tmp/angelx-native-integration-testcheck18.log`),
but remains untested. Review also identified guest reforge queues stranded by
leaving and a checked shutdown path that can bypass pending authored paint; both
remain open.
The v4 offer, brew and kick test repairs and reset follow-ups are composed; test
check 19 passes (`/tmp/angelx-native-integration-testcheck19.log`). Runtime suite
v5 is building and will repeat the worker scenarios plus close, guest reforge,
paint shutdown and workspace-wish regressions on one source fingerprint
(`angelx-native-runtime-suite-v5.service`). To leave build headroom without
touching source or other agents' work, an idle, dated 11 GiB incremental compiler
cache from this task was removed; it is regenerable build output.
V5 completed with 34 passing checks and three failures on an unchanged source
fingerprint (`/tmp/angelx-native-runtime-suite-v5-run.log`): offline kick retry
and two workspace/native wish fixtures. The earlier offer, brew and kick-reply
cases passed. The failing cases are under investigation; preserving already
issued cues remains a required behavior, not an assertion to remove. Separate
drafts for retained painting failures and BeginDelve ownership are now composed
but untested, alongside their known gaps: immutable paint-output snapshots and
the multiplayer invitation path. Test check 20 passes
(`/tmp/angelx-native-integration-testcheck20.log`). The offline-kick fixture
also now completes the real client handshake before kicking; it still needs a
runtime rerun. Another idle dated
incremental compiler cache was removed to maintain disk headroom; source and
other agents' work remain intact.
The v5 offline-kick fixture now establishes real client seats before kicking.
Wish/workspace tests were updated to preserve their original-source and
exactly-once assertions while observing native cue output and checked close;
neither change has been rerun yet. A draft paint handoff now captures immutable
authored cards and fences superseded work; test check 21 and independent review
are in progress. This is an in-process boundary only: durable snapshot/receipt
recovery after application death and source-write uncertainty remain open.
Test check 21 caught a test-only borrow issue; it is corrected and test check 22
passes (`/tmp/angelx-native-integration-testcheck22.log`). Runtime suite v6 is
running on a fixed snapshot with the newer paint and BeginDelve tests. Review has
already found two defects outside that snapshot's acceptance: blocked background
readers are not included in the painter admission limit, and nonadjacent equal
paint batches can incorrectly collapse an A→B→A update. Fixes and focused tests
are being prepared; v6 success alone would not qualify the cutover.
V6 completed with 41 passing checks and two failures on an unchanged source
fingerprint (`/tmp/angelx-native-runtime-suite-v6-run.log`). The remaining
failures are the offline-kick fixture and the retained BeginDelve lost-reply
test. The previously failing wish/workspace checks now pass, including the
actual issued-cue delivery assertion. Ordered A→B→A paint admission, the total
painter-owner cap and a multiplayer host/invite continuation have since been
composed; their compilation and runtime behavior remain unverified. Exact paint
source publication after I/O failure and byte-aware native batching remain open.
Test check 23 found that the host-invite composition reintroduced a borrow error
in the offline-kick fixture; the test now reacquires the server after the pump.
Test check 24 passes (`/tmp/angelx-native-integration-testcheck24.log`); these
latest patches are still unverified at runtime. The v6 failing kick fixture now
uses the already-seated host and waits for the friend handshake; the BeginDelve
fixture now gives asynchronous initialization real time. Byte-aware paint
batching and a retained in-process source-write plan are composed, with test
check 25 and independent audit in progress. The write plan does not claim crash
recovery or atomic coordination with external writers that ignore its lock.
Test check 25 caught another fixture borrow across the new handshake loop; it
was corrected and check 26 passes (`/tmp/angelx-native-integration-testcheck26.log`).
Independent review also found that a
transient error before a paint source plan is captured can lose its input, and
that stopped painters can leave readers parked on retries or a full channel.
Both require retained input and cancellation-aware teardown before acceptance.
The intro host continuation now captures the selected friend capacity and guards
competing host commands; test check 27 passes
(`/tmp/angelx-native-integration-testcheck27.log`). Runtime suite v7 is running
on a fixed fingerprint, including the new paint capacity, source-plan, batch
boundary and multiplayer-intro checks (`angelx-native-runtime-suite-v7.service`).
Ordinary standalone host startup needs its own retained continuation and remains
open; v7 does not include the pending pre-capture and painter teardown fixes.
V7 completed with 53 passing checks and one failing multiplayer-intro host
check on an unchanged source fingerprint (`/tmp/angelx-native-runtime-suite-v7-run.log`).
The previously failing offline kick and BeginDelve tests passed. The failed
multiplayer host case is being diagnosed. Retained pre-capture I/O, bounded
published-output permits, painter teardown and standalone-host continuations
are now composed but have not passed compilation or runtime checks; test check
28 passes (`/tmp/angelx-native-integration-testcheck28.log`). The additional code
has not passed runtime checks and does not make an unresolved pre-crash native
receipt or painter plan recoverable after process death.
The v7 multiplayer fixture now isolates ambient host settings and waits for
real asynchronous progress. Paint source publication and handoff now share an
ordered, nonblocking boundary; test check 29 passes
(`/tmp/angelx-native-integration-testcheck29.log`). Runtime suite v8 is building
on a fixed fingerprint that also covers its untracked painter lifecycle tests;
it adds standalone hosting, source-I/O, teardown and ordering cases
(`angelx-native-runtime-suite-v8.service`). These changes remain unaccepted.
Audit found an unhosting race while a named host join owns a receipt; a guard
and test are now composed. A separate concern that later paint publication
deletes an earlier snapshot's image was disproved by tracing the content-addressed
paint store; a two-output retention test is composed to verify it. Another dated
idle incremental compiler cache was removed to maintain disk headroom.
V8 completed with 57 passing checks and five failures on an unchanged source
fingerprint (`/tmp/angelx-native-runtime-suite-v8-run.log`). The previously
failing multiplayer-intro check passed; standalone hosting and four new painter
source/teardown/ordering checks failed. Their causes are under investigation;
test check 30 passes for the newly composed host guard and asset test
(`/tmp/angelx-native-integration-testcheck30.log`). Do not
treat the earlier 53/54 result as validation of these newer changes.
The v8 paint failures were traced to invalid opaque test images and a raw-helper
error-semantics regression; the draft restores those semantics while testing
production lost-ack adoption. The standalone-host fixture now waits for actual
asynchronous progress and explicitly chooses host-knight mode. Test check 31
passes (`/tmp/angelx-native-integration-testcheck31.log`). Runtime suite v9
repeats the broader suite and adds host-guard, retained-asset and publication
handoff checks on a fixed source snapshot (`angelx-native-runtime-suite-v9.service`).
The final PR acceptance criteria and real edit-to-activation measurement remain
open even if these focused checks pass.
V9 completed with all 65 focused checks passing and matching source fingerprints
(`/tmp/angelx-native-runtime-suite-v9-run.log`). This includes all six real-worker
swap/initialization cases, in-flight worker cleanup, painter retention/ordering,
standalone and intro hosting, and the formerly failing cases. It does not prove
the broader refactor, crash recovery, or the unexercised direct caller boundaries.
A local-card retained-ticket draft and a real-worker activation observer were
subsequently composed; test check 32 passes
(`/tmp/angelx-native-integration-testcheck32.log`). The observer measures a manual
native driver and four seat projections, not end-to-end App input or rendering;
it has not yet been compiled or measured.
The local forge continuation is now composed alongside the local-card work.
Runtime suite v10 is rebuilding and retesting those additions; a dedicated
private-source native activation benchmark is warming in parallel. `/tmp`
reports free space but rejected the source copy with `Disk quota exceeded`, so
the benchmark uses a separate private directory under `~/.codex/`
and its own target, without changing the integration checkout. No timing result
exists yet, and recent direct settlement/visit callers remain open.
The v10 run compiled the native test binary, but `/tmp` exhausted its quota
during execution: later case logs are empty and its remaining reported failures
are infrastructure failures, not usable test evidence. Suite v11 is rerunning
the same fixed snapshot with logs and `TMPDIR` under the private `/home` benchmark
directory. The dedicated candidate warm build passed; actual activation timing
will run after validation so concurrent suite load does not distort it.
V11 completed with 66 passing checks and one failing local-forge case on an
unchanged source fingerprint (`~/.codex/angelx-activation-work/test-logs/suite-v11-run.log`).
The failure occurs because its fixture uses invalid weapon syntax; a repair is
being prepared. The separate four-seat manual native-driver activation trial is
now running against that same private source; the independent forge-fixture
failure does not change what this performance trial measures. It is not a full
App, socket or rendering latency benchmark.
The trial completed (`~/.codex/angelx-activation-work/trial-01/measurement.json`):
one warmed isolated semantic edit and build took 44.78 s from edit start to
changed behavior delivered to all four seat projections, including a 44.53 s
build; atomic executable publication to changed delivery took 222.14 ms. The
manual 30 Hz native frame/publish samples (n=1,917) had p50 2.20 ms, p95 3.35 ms,
p99 3.83 ms and max 51.17 ms on this shared machine. The real replacement worker
retained the brew owner and verified worker cleanup. This single trial is not an
App input, terminal rendering, socket, fresh-host build or production restart
measurement; no comparison or acceptance threshold is implied. The invalid forge
test fixture has since been corrected but has not been rerun.
Test check 33 passes for that correction
(`~/.codex/angelx-activation-work/test-logs/testcheck33.log`), and
suite v12 is rebuilding and rerunning the full focused set on the new snapshot.
The shared filesystem reached 3.8 GiB free while other builds were running;
dated, idle incremental compiler caches from this task were removed to restore
headroom. No source or other agent's work was removed.
V12 completed with 66 passing checks and one local-forge failure on matching
source fingerprints (`~/.codex/angelx-activation-work/test-logs/suite-v12-run.log`):
the fixed fixture reaches the retained receipt but its final notice assertion
fails. Audit found a real reply-mapping error: `EquipForged` succeeds with
`Done`, which the retained UI handler reported as unchanged. A correction is
composed but untested. A retained navigation draft covering
fresh-start Keep, College, Stables and Lists is composed; test check 34 is
passing (`~/.codex/angelx-activation-work/test-logs/testcheck34.log`).
The standalone settlement-entry path remains open. The shared
filesystem again fell to 3.2 GiB free; only further dated, idle incremental
compiler caches from this task were removed, restoring approximately 8.7 GiB.
Subsequent builds again reduced free space to 4.6 GiB; more dated caches from
this task were removed, restoring approximately 6.2 GiB. Navigation shutdown
ticket ownership, current-realm selection and retained settlement entry are now
also composed; test check 35 found a test compilation error. They have not yet
passed runtime tests.
Test check 35 found a test-only privacy error in the settlement fixture. A
test-only re-export fixes it, and test check 36 passes
(`~/.codex/angelx-activation-work/test-logs/testcheck36.log`).
Runtime suite v13 is running with the navigation and settlement lifecycle cases
added; it uses the private home scratch directory to avoid the `/tmp` quota.
V13 completed with 70 passing checks and three failures on matching source
fingerprints (`~/.codex/angelx-activation-work/test-logs/suite-v13-run.log`).
The prior forge reply test now passes. Fresh navigation passed, but the lost
navigation reply test and both settlement entry tests failed; their fixture and
ownership behavior was investigated in subsequent passes. Review also found
missing conflict and reset guards around the pending settlement owner; those
were addressed in the later focused suites. Shared
disk again fell below 4 GiB; only dated, idle incremental compiler caches owned
by this task were removed, restoring approximately 8 GiB without touching
source or other agents' work.
The v19 focused suite passed all 20 cases on matching source fingerprints,
including retained settlement entry, Scroll admission and lost replies, local
Vote, named-host Join, EndHosting lost-reply/service-failure recovery and
blocked Join ACK/disconnect cleanup
(`~/.codex/angelx-activation-work/test-logs/suite-v19-focused-run.log`).
The broader v21 suite passed all 93 selected checks on matching source
fingerprints, combining earlier worker swaps, lifecycle, image/painter, seat
and cleanup cases with the three stable-sync tests. Each cockpit filter ran
at least one test (`~/.codex/angelx-activation-work/test-logs/suite-v21-full-run.log`).
These are checks on the isolated integration worktree, not a
production cutover or full acceptance test. Four additional fixture-isolation
checks passed for the production codec, opt-in policy, and production/fixture
worker capabilities (`~/.codex/angelx-activation-work/test-logs/fixture-v20-run.log`).
The full `angel-delve` library suite passed 2,993 tests (301 ignored) on
matching integration source fingerprints
(`~/.codex/angelx-activation-work/test-logs/core-v21-tests.log`).
Scene/seat-fenced floor-plate and LeaveActivity patches remain staged. The
LeaveActivity draft fences room, activity and seat, but independent review
confirmed its floor-plate generation cannot distinguish an identically
recreated activity without a new plate. An activity-incarnation contract and
test are needed before composing that draft. Review
identified an admission race in the first draft and a checkpoint compatibility
risk for its persisted generation field. Nonzero generation loss is rejected
by owned checkpoint equality, but an older worker can activate at generation
zero and then fail to advance future generations. A versioned worker capability
must fence that downgrade. A `delve.scene-fence:1` worker-capability gate with
real replacement regressions is staged, to be validated together with the
atomic scene fence rather than activated alone. The composed draft checks
activity names twice per simulation tick; its allocation/runtime cost and
coverage of same-tick activity replacement need review before integration. SyncHome also needs
uncertain-receipt ownership: horse-trade mutations save
realm/treasury changes before the ephemeral SyncHome call, so an uncertain
response can strand the owner and a retry can repeat financial side effects.
A retained in-process transaction is now composed and test check 46 passed;
full runtime suite v21 passed the three new economic/lost-reply cases
alongside the earlier lifecycle checks. Review found that a crash after the
durable save but before native
acknowledgement still needs an outbox or restore-time reconciliation; a
versioned recovery design is staged at
`~/.codex/angelx-activation-work/agent-ticket/stable-sync-recovery-design.md`
and an implementation draft is staged at
`~/.codex/angelx-activation-work/agent-ticket/stable-sync-recovery.patch`.
Independent review found that deferred horse-service credits could be lost on
crash, and a failed checkpoint restore could hide the marker and allow writes.
A revised recovery draft adds a durable credit outbox, marker discovery and
reset fencing, and addresses prior audit findings on retry ordering, model-turn
gating, service-only outboxes and filesystem errors. It is composed only in
the integration worktree; test check 47 found four compile errors, fixed in
test check 48. Independent review then found stale restore-path precedence,
dropped retry-queue tails, a possible Realm lease leak after marker unlink and
a non-fail-closed creation check. Corrections and regressions are in progress.
The four fixes are composed and test check 49 passed. Full runtime suite
v22 is building the crash-boundary regressions with the previous cases; its
source fingerprint explicitly includes the new untracked recovery module.
Receipt growth still needs a safe compaction plan. The LeaveActivity owner must join the
stable-command gate if those drafts compose. Events
arriving concurrently within a tick remain conservatively ordered rather than
globally timestamped. Locked-file checks passed; this integration worktree has
not been published.
This is not a verified or published production cutover. Their review
identified required caller-owned operation tickets: equal command payloads are
not sufficient to identify which one-shot UI or network action owns a delayed
reply. Realm, home and vote workflows also need retained transaction phases,
and native model-job ACK retries must precede a blocked checkpoint only after
the original durable publication. Verify retained departure/input facts, seat
loadouts, avatar animation and avatar save overlays, as well as nonblocking
network publication, before replacing the current native owner.
Keep remaining character generations, agent/provider/tool/workflow and authored
Glass content extractions open. The current Client still links Delve's typed
Session/checkpoint validators and Run-shaped view; handoff deliberately requires
canonical state equality. That permits behavior replacement within this schema,
not arbitrary new world state or physics primitives without a native contract
change. Separating implementation policy from stable view/continuity contracts,
versioned state migration and extensible registered world services remains
required for the operator's broader self-modifying-system goal. Never hide this
coupling by silently dropping unknown state. No full-refactor completion is
claimed.

## Extraction order and acceptance

1. Preserve the tested full-rules executable handoff and its state codec.
2. Establish the coherent Glass/Link/Kernel contracts and audit actual duplicate
   code/content leakage. Extract the module registry and resource ownership.
3. Prove character packs can add/swap art, animations, ability behavior, SFX/VFX
   and loadout options through registered modules. Include a new surface and
   command without adding a Rust `App` branch or restarting the host.
4. Give Delve module-owned state, command-based edits and delta publication.
5. Extract agent strategy across model/tool request boundaries, preserving active
   work and effect receipts; then extract provider/tool adapters and workflows.
6. Move remaining application surfaces and game clients behind those contracts.

Each pass must leave a usable vertical slice with state/behavior parity, failed
candidate rejection, in-flight handoff coverage, resource-retirement evidence and
measured edit-to-activation/runtime cost. A manifest-only or tuning-only change
does not complete a system extraction. Keep PR #29 draft until the agreed module
acceptance checks pass; record the exact remaining boundaries between passes.
