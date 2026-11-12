# Stable mechanisms and replaceable systems

This is the working inventory for the AngelX modularity refactor in private
draft PR #37, continuing the merged PR #29. "Stable" means infrequent mechanism changes with explicit compatibility;
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

Player selections currently live in the live module's in-memory checkpoint:
the module worker constructs a fresh registry on startup, and its checkpoint
handoff covers replacement within that worker. A process restart has no durable
selection restore path here. Seat numbers are session-local and the network's
seat token is a session resume credential, so neither is by itself a durable
player identity. Persistent per-player choices still need durable identity,
storage and validated restore, including unavailable selected packs; reload
tests alone do not prove restart persistence.

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

### November 2 recovery and 0.2.7 reconciliation

Private PR #29 was merged into `dev` on November 1 at `1c6a4ada516e`. Its
published commit does not contain the later native-owner integration. The
original integration and chat worktrees disappeared. The v23 native-owner
source was recovered at `~/.codex/angelx-activation-work/recovered-v23`; its
tracked diff and four untracked source hashes matched the original v23 test
run before documentation recovery. Suite v23 passed 104 selected lifecycle and
crash-boundary checks on that source, with matching before/after fingerprints.
The prior full `angel-delve` library suite passed 2,993 tests (301 ignored).
These results apply to that recovered source, not to current `dev`.

Current private `dev` is `79bf9383f`; it includes 0.2.7 and has since moved
Delve host code into `crates/realm-host`. The recovered source patch does not
apply directly. A separate private follow-up checkout at
`~/.codex/angelx-activation-work/private-followup` is based on that `dev`
commit. The primary checkout contains extensive existing user changes and was
not modified. Reconcile ownership and behavioral changes into the current
architecture before creating a follow-up draft PR; preserve all operator locks.
The remaining scene-incarnation capability, runtime cost, receipt compaction,
full acceptance and actual production cutover remain unverified.

Read-only reconciliation found that current `crates/realm-host/src/together_sim.rs`
still keeps `Run` and `Journal` in native Core and sends a full Run through
`step_system`; it does not depend on `angel-delve-system`. The owned-session
foundation remains available in `crates/live`, `crates/delve-system`, and
`crates/delve`, but native worker ownership has not yet been activated. The
recovered stable-sync marker/outbox and its receipt recovery are also absent
from current `dev`; the old cockpit control paths are now realm-host controls,
with model-turn accounting still crossing the cockpit/realm-host boundary.
Neither the recovered v23 code nor current `dev` includes the staged atomic
scene-incarnation fence. Port these as coherent, separately tested boundaries;
do not replace newer squire, Kingdom Loop, ability, or checkpoint changes with
older files wholesale.

The 0.2.7 reconciliation found the existing character-pack path already binds
separate executable ability, art/animation, SFX and VFX resources; that evidence
is not proof of native world-authority replacement. The showcase squire remains
an authored sprite/pose implementation inside Glass. A separate candidate
extraction is being prepared to swap its prepared art and clip while preserving
the open showcase's position, phase and resource ownership. This is a Glass
subsystem boundary, not a proposed new public system or crate.

The native worker cutover still depends on transactional ability/source plans:
current realm-host mutates `Run` in the native prelude and after-step paths,
while the recovered implementation freezes source/frame/actor ownership across
uncertain receipts. Switching only the frame driver or adding an unused
Authority adapter would not be an operational extraction. Detailed current-dev
port constraints are preserved at
`~/.codex/angelx-activation-work/reconciliation-0.2.7/RECONCILIATION.md`.

Follow-up work is staged outside this checkout: the realm-host authority port
has a frame/source-plan draft but still lacks caller-ticket and connection
reconciliation; the stable marker/outbox port awaits compilation and independent
crash/exit review. A showcase squire clip draft is being reviewed for actual
removal of authored Glass policy and failure-atomic resource selection. None of
these drafts is yet evidence of a current-dev cutover.

The first 0.2.7 Glass baseline build failed because the machine selected
incompatible Rust compilers for artifacts and build scripts. A fresh target now
pins both Cargo and rustc to the repository's `1.99.0` toolchain; the build is
running in `~/.codex/angelx-activation-work/private-followup-target-rust1990`.

The first pinned 0.2.7 Glass baseline build succeeded with the repository's
Rust 1.99.0 toolchain; this was a compile-only baseline, not a hot-swap test
(`~/.codex/angelx-activation-work/test-logs/glass-baseline-027-pinned.log`).
The current-dev native authority candidate was applied to a separate test
checkout and `angel-realm-host` test type-check is running. Its source has not
yet been integrated into the follow-up checkout or behavior-tested. A separate
stable/outbox draft and a showcase clip draft remain under review: the former
has unresolved completion/forced-exit and receipt-compaction boundaries, and the
latter still duplicates the authored fallback in Glass.

The first 0.2.7 authority review candidate was applied only to the separate
`native-v9-checkout`; its `angel-realm-host --tests` type-check is running with
pinned Rust 1.99.0. The independently ported stable/outbox work has important
differences from v9: worker-completion crash recovery, origin-workspace binding,
exact payload receipts and forced-exit behavior still need reconciliation. A
showcase clip draft removed its duplicate Glass pixel templates, but its first
card-path replacement was transparent; that visible regression must be fixed
before the draft can be integrated.

The first native test type-check exposed ten current-dev API mismatches after a
test-only fixture feature was enabled. An isolated v11 correction addresses
those compiler diagnostics and is being checked; compilation and lifecycle
behavior are not yet verified. The independent stable/outbox delta applies to
that candidate but has not been integrated or tested with it. Its bounded receipt
ledger deliberately fails closed at 16,384 entries until a safe compaction
protocol exists; provider completion and the first durable local write also
remain separate operations.

The revised showcase draft routes card pixels through the live clip and removes
the old Glass pixel fallback. Review found a remaining startup/disabled-hotload
blank interval, card animation still authored in Glass, and a missing release
bundle entry. These must be resolved and tested before integration. Its first
five focused Glass tests are building in a separate checkout; they do not by
themselves establish the end-to-end Registry-to-Glass activation path.

The isolated native v11 candidate now passes `cargo check --tests` for
`angel-realm-host` with unchanged source fingerprints. This checks types only;
the first selected lifecycle tests are building. Independent review found two
behavioral blockers: recovery can stall with no game open, and some local
commands receive the wrong ticket scope and are refused. It also flagged unused
squire-payment paths for investigation. The stable/outbox delta incorrectly
credits the first seat for an unknown route and may write to a different
checkpoint path from the host's drain. These are being corrected before the
candidate is integrated or published.

The first showcase test build failed on a borrowed-identity lifetime error; no
Glass test ran. A newer scratch candidate also addresses startup bootstrapping,
card animation and release packaging, but has not yet been compiled or validated.

Six selected tests passed on the isolated v11 source with matching before/after
fingerprints, including a real worker swap with an open frame and lost finish,
seat-incarnation fencing, source uncertainty and durable outbox replay. These
results predate the combined v12 and stable/outbox corrections; those newer
changes require their own tests. A candidate Glass revision is now under test.

The first stable/outbox delta also touched the operator-locked
`cockpit/src/drive/loop_ctl.rs` in the isolated test checkout. That file was
immediately restored to the exact private-dev version and the lock check passes;
no protected change belongs in the integration. Forced-exit retention is being
redesigned around an unlocked boundary before the delta can be accepted.

The candidate Glass build passed six focused tests covering open-clip swaps,
phase retention, absent art, lease cleanup, card pixels and card timing, with
unchanged tested source fingerprints. These are Glass-unit results, not proof of
the whole Registry-to-Glass path. An independent review then found that a stale
startup clip could reappear after removal followed by a bad edit; a candidate fix
and regression test are now included in the cockpit build under test. The
original six tests precede that cockpit-only fix.

The revised native candidate adds no-game recovery and correct local command
ticket routing; its targeted tests are running. A separate stable/outbox revision
uses an unlocked caller seam, restores the old `Thinking::spawn` signature, and
adds a loop admission check and retry coverage. It has not yet been compiled or
combined with the v12 test source. No new draft PR has been published.

Private `dev` advanced to `ac10f81cec88` during the follow-up. The showcase
candidate applied cleanly on a new isolated checkout of that base; protected
files match that base and the lock check passes. The earlier source at
`79bf9383f` passed six Glass tests and five cockpit bootstrap/registry tests;
the latter include the fix for removal followed by a failed edit. Tests are
being repeated on the newer base before publication. Release evidence tests
also pass (31 tests), and the runtime inventory lists the module and its pack.

The isolated native v12 source passed ten selected tests, including the four
new no-game recovery and command-routing regressions, with unchanged source
fingerprints. These results do not cover the newer stable/outbox delta or the
advanced `dev` base. The durable receipt ledger still needs safe compaction and
there is no provider/local-storage atomic transaction.

On `ac10f81cec88`, the candidate again passed six Glass-unit tests and five
cockpit startup/registry tests on matching source fingerprints. Private `dev`
then advanced to `78754756fa24` with an identical tree, so the follow-up checkout
was fast-forwarded without changing those tested bytes. Independent review found
two gaps before publication: standalone Glass consumers need their authored pack
installed at their entry points, and remote Glass clients need live clip updates
and removals. Both are being addressed; the present Glass candidate is not yet
a complete extraction. The combined native/outbox candidate is under test in a
separate checkout, and its long-lived receipt-cap limitation remains open.

The combined native/outbox build exposed a visibility error, and the cockpit
test build also exposed a fixture compiled in dependency mode without its test
support. Minimal isolated fixes are under test; the combined candidate has not
yet passed these checks. The receipt cap is intentional until safe retirement
exists: a producer may retry an older event after a later receipt, so simply
evicting old IDs can double-credit a seat after a crash. A regression was
prepared for that sequence.

Private `dev` advanced to `40d55f033` with only documentation changes; the
follow-up fast-forwarded without changing the previously tested Glass source.
Standalone entry-point loading and versioned remote clip replacement/removal
are staged in separate patches and have not yet been integrated or tested.

The combined native authority and outbox source passes fourteen selected
realm-host lifecycle and recovery tests on matching source fingerprints. Its
cockpit production binary also passes `cargo check`; the full cockpit test build
currently has 411 diagnostics, mainly old test fixtures borrowing the new
owned `RunMut` API. Fixture migration is in progress. The bounded receipt
ledger remains fail-closed: a later event can be applied before an earlier
producer retries, so evicting older random IDs without a shared retirement
lease can double-credit after restart.

The combined Glass candidate passes twelve focused Glass tests and five cockpit
startup/registry tests with unchanged source fingerprints. Those include remote
replacement, removal, late-viewer catch-up, old viewer compatibility and
per-field decoding bounds. The standalone terminal build is being checked.
Remote review found that the existing large frame envelope is allocated before
the new per-field bounds are checked; this needs attention before publication.
A new viewer cannot connect to an older v1 host, while a v1 viewer can connect
to a v2 host without receiving clip extensions. A single end-to-end
Registry-to-Glass visibility measurement remains outstanding.

After adding standalone entry-point loading and remote update/removal, the
current Glass candidate passed sixteen targeted Glass tests and five cockpit
startup/registry tests with unchanged source fingerprints. The standalone
`angel-glass-term --all-targets` check passed, as did 31 release-evidence tests
before the final remote-only changes. The remote reader now chooses a clip
frame limit from the decoded tag before accumulating the body; separate tests
cover raw and compressed oversized clips, large video, late subscribers and
unchanged frames. V1 viewers can attach to a v2 host without clip extensions;
a v2 viewer cannot attach to a v1 host. Generic callers of `wire::read` must
now supply the `FrameMessage` bound.

On this machine, one warm headless synthetic pack trial measured 51 changes
after three warmups: file write plus forced Registry refresh and snapshot was
p50 212 µs / p95 783 µs; the App-to-Glass host setter was p50 1 µs / p95 3 µs.
Combined p50/p95 were 214/784 µs. This measures a synthetic pack and a direct
refresh into a no-window Host; it excludes file watching, scheduling, renderer,
GPU, first visible pixels and network. It is not a full edit-to-visible result
or a general runtime benchmark. The measurement source and method are recorded
in `docs/performance/showcase-squire-activation-latency.md`.

Native cockpit fixture migration reduced the combined build's 411 compiler
diagnostics to six; a further correction is under test. Passing the isolated
realm-host cases and a production type-check does not yet establish the whole
native cutover on current dev. The receipt-cap and provider/local-write limits
remain open.

Subsequent focused validation passed sixteen Glass tests after the bounded
frame-reader and late-subscriber fix, five cockpit startup/registry tests, and
the standalone terminal all-targets check. The release-evidence suite passes
31 tests after adding the compile-time pack and timing note to its inventory.
Both lockfiles gained only an `angel-glass` → `serde_json` edge; the existing
package identities and counts are unchanged, and the release policy's recorded
lockfile hashes were updated. This is not a fresh advisory scan.

The separate native candidate now also passes cockpit test compilation and ten
selected cockpit receipt/context/exit tests; these results do not mean that the
candidate is included in the Glass extraction. Next: review and integrate the
native cutover with its production state-preservation and longer-running
receipt lifecycle checks. Also measure true edit-to-visible timing, including
the watcher, UI scheduling and rendering, and compare steady-state frame cost.

### Shipped policy capture on the live-module draft

The shipped policy inventory now captures the same JSON dependencies and
bounded generation inputs as the live loader. This keeps an embedded policy's
generation digest consistent with the directory that can replace it. Capture
uses two matching reads and embeds the validated bytes, and watches selected
files and directories for rebuilds. Manifest selection must remain stable
across capture. Four V8 capture-contract tests and all 132 Glass library tests
passed on the merged draft source, including digest parity and rejection
boundaries.

On the merged draft at `c778edce0`, a fresh cockpit build and five focused
startup, invalid-clip and removal/rollback tests passed, followed by the
standalone `angel-glass-term --all-targets` check. The source fingerprint was
unchanged throughout. These checks cover the published Glass extraction; they
do not validate the separate native candidate.

The native world-authority and receipt work remains a separate candidate. Its
real-worker swap and Realm-host tests passed against an earlier dev base, but
current dev's off-frame settlement and control changes still need semantic
reconciliation and fresh lifecycle, performance and cockpit verification. It
is not included in this draft change.

In the separate brew discovery prototype, review found that two stale observers
could both take one recovered result and contend for its sole checkpoint owner.
An isolated correction passed 12 admission tests and 17 brew tests, with one
native swap test ignored. Its workers still hold the shared simulation core
during native IPC and lack a proven bounded shutdown/drain path; neither that
correction nor the broader native candidate is included here.
A separate held-browser continuity test exposed another native cutover gap:
replacing the old core replaces its live sound-asset registry, while an
already-issued voice can still own its bytes. The new server then cannot find
that sound in its registry. Preserve or reconcile those registries before
cutover and verify the issued URL remains usable through retirement. An
isolated correction shares the retained registry before candidate preparation;
focused tests preserve a prior sound through startup and browser readiness,
admit six new seats and reclaim candidate-only media after a failed preparation.
It remains outside this draft until the broader native cutover is reconciled.

### Browser generation preparation

Hosting and hosted-session restore now admit a retained browser generation
before changing the run. Capture, hashing and compression run on a bounded
worker; the server and native/WebSocket seats can start while page requests
wait for that generation. Captured bytes and their content URLs stay together,
and an in-flight response retains its bytes through server shutdown. Only a
failed generation that never published assets can be retried on the same
server, preserving its run, listener, join key and native seats. A successful
generation is not replaced by this path.

The process admits at most two retained or preparing generations, with bounded
file count and size. Sixteen cold HTTP requests per generation may wait for at
most 30 seconds; additional classified requests receive 503, leaving room in
the connection-admission pool while preparation is pending. Raw or idle
connections can still fill that existing pool. A failed worker releases its
capacity after it finishes, while a cancelled worker retains capacity until
its filesystem work actually exits. Source stamps detect ordinary concurrent
changes before the captured generation is published; immutable version
directories or an atomic source manifest are still needed for a
filesystem-wide atomic source update.
A blocked filesystem call cannot currently be forcibly cancelled.

URL and ETag construction now share the same content digest; gzip policy and
the resulting URL, validator and response bytes are unchanged. On the composed
source, 58 Serve unit tests, 25 real socket tests, seven browser-capture tests,
seven native host tests and 13 Sim tests passed; five existing Sim tests were
ignored. Tests include concurrent cold requests, native joins, failed-generation
retry, capacity refusal, asset parity with the previous capture, and an in-flight
body surviving shutdown. The implementation was independently reviewed and
operator-locked files and their existing seals were unchanged.

In shared-machine development-profile samples, capture of 391 shipped files
(189,379,456 bytes) took 54–68 ms. Hash/compression of that input took 9.985 s
before digest reuse and 6.068 s afterward; these are phase samples, not a
production benchmark. One subsequent native host activation took 1.896 s,
the first complete HTTP response 3.923 s, and hosted restore 39 ms. A warm
100-HEAD sample took 3.700 ms total. Full native activation, browser-first-view
latency, live replacement of successful generations and sustained runtime cost
remain open; moving preparation off-frame does not itself make it faster.

### Precomputed embedded browser assets

The build now captures immutable embedded files and prepares their existing
content digests and gzip representations once. At runtime, reuse requires the
same name, MIME type and exact bytes; changed or beside-the-binary assets still
take the normal capture and preparation path. The generated Rust references
are published atomically, and old linked executables retain their embedded
bytes if a later build fails. The generated index is written earlier, so this
is not a transaction over every build output. Nor does it make concurrent
source-tree edits an atomic filesystem snapshot or live-replace a successful
browser generation.

On the composed source, eight browser capture/parity tests, 62 Serve unit
tests, 25 real socket tests and 197 release-tooling tests passed. The tests cover
the full 391-file response set, URL/ETag/plain/gzip parity and changed or missing
beside music. The release tests use fixtures; they do not prove an actual
shipping package. A measured preloaded 189 MB server workload prepared the
first page in 353–359 ms, compared with 1.80–1.88 s using normal preparation;
this excludes initial resource reads and is not a launch measurement. A portable
proxy binary grew by 13,869,152 bytes, and build-time preparation took 1.47–1.70 s
in the optimized sample. Actual release-binary size, platform portability,
and end-to-end launch remain to be qualified. A separate clone-local Cargo
experiment built both WebGPU and WebGL renderers and exercised shell asset
edit, addition, removal and restoration. The generated source returned to its
original hash after restoration, and 12 decoded plain/gzip response bodies
matched the source and legacy preparation. The experiment also caught and
avoided stale absolute paths inherited from a copied Cargo target; it primed
the build in its own source tree before measuring.

### Six-seat character selections

The rules admit six players, but character loadout registration and its
appearance, executable and VFX validators previously stopped at four. The
shipped command now registers and accepts all six seats while keeping four
ability slots per player. Its executable probe gives a selected sixth-seat
ability the same player context and rules validation as the earlier seats.

On the composed source, all three appearance tests passed, including checkpoint
restore and invalid-player rejection. The character ability suite passed 21
tests, with four existing visual/Bend-specific tests ignored. A V8 registry
test selects art,
abilities, sound and effects for seats five and six, casts both, validates the
issued effect scene, and replaces the executable provider while retaining the
casts, sounds, visuals, health, mana and cooldowns. A shared-machine debug
sample on the composed source measured the two-cast rules/ability tick at
2.11 ms and forced provider refresh plus source activation at 60.77 ms; these
are not production latency or end-to-end rendered edit-to-visible measurements.
Character selections still need durable player identities and restart
persistence as noted above.

### Ability ownership across seat replacement

An ability request can remain queued while a player leaves and the seat is
reused. The executable host now checks the request's captured raid and seat
epoch before admission and retains those fields with pending and running
casts. It checks ownership before and after each rules step, including an
already-processed tick. When replacement advances the seat epoch, the new
occupant cannot execute the previous occupant's work. In the tested immediate
guest-seat replacement, previously issued projectiles, voices and visual
entries remain. Ordinary provider replacement continues to retain active casts
and their original dependencies.

The original V8 regression admitted a queued request and emitted an effect for
the replacement; the corrected implementation passed the complete ability
suite: 25 passed and four optional tests ignored. The tests cover queued and
running replacement, same-tick retirement, a request from another raid,
preservation of issued effects, six seats and existing provider swaps. The
protected-file check passed. Three alternating debug-profile samples of 180
four-player ticks measured median ability-batch times of 575–615 µs before and
582–636 µs after; concurrent work and the small sample make this an indicative
measurement rather than a precise overhead estimate.

Raid and seat epoch are not a unique identity across independent sessions. In
hostless/watch mode a remote occupant can hold seat one; that seat's removal
does not use the guest-seat retirement path, so its transport-owned identity
and replacement fencing remain unproven. A real watch-server regression in the
isolated profile candidate confirms the case: a matching reserved name joins
seat one, a same-token reconnect keeps its lease, and a fresh-token replacement
with the same name incorrectly receives that same lease. The final assertion
fails; the candidate and its prospective reservation fix are not in this PR.
Profile selections still need an authoritative session-incarnation lease;
already-issued follow visuals also need ownership-aware detachment to avoid
following a replacement, and visuals currently retire when their player is
absent. The durable store, session lease and native owner cutover remain
isolated candidates, not part of this change.

### Independent character animation

The character selector can now choose authored looping clips independently of
art. A versioned animation pack lowers frame ordering and timing exactly into
the existing bounded wire representation, and replacement preserves normalized
playback phase. Glass's arena view samples the current animated frame. Optional
imports keep the character selector and unrelated abilities available when the
motion provider is absent: the requested clip remains in selector state, the
UI reports the pending choice and default art plays until the provider returns.
A present incompatible provider, invalid clip or removed selected option is
rejected without replacing the last working generation. Existing required
imports remain required; older loaders reject the new manifest field.

On the composed source, 101 live-module tests and 28 ability tests passed,
with four ignored in each suite; both Delve appearance tests also passed from
the integration checkout. Tests cover late provider registration,
contract and cycle checks, preservation of old leases, missing-pack fallback,
live replacement and a real Run snapshot restore. The cold selector restore
uses captured state loaded by a test fixture; it does not test a production
durable-profile load path. A debug forced-refresh sample measured 26 ms to
install and 82 ms through publication of guest pixels, with a 201,239-byte
snapshot. This excludes the edit itself and is not an automatic-watcher,
GPU-presentation or release benchmark. Clips currently lower to at most eight
frames; animation graphs, quick-rig and durable profile composition remain
open.

The first integration omitted the animated loadout contract from the live
Sim's source filter. The Sim now admits already-validated `Loadout` bindings
instead of maintaining a second contract-name list. A production-boundary
regression installs art, an executable slot and an animation through
`Sim::set_live_characters`, then edits and reloads the animation while
checking that the slot remains installed. It also waits for the real local Link
seat to enter play and the authoritative hero to return, then sends a spell
input and observes an ability start through the live Sim. The fixture removes
enemies to keep transport timing from ending the fight. The focused test and
the full active ability suite passed (29 passed, four ignored). An initial
local projection alone was not a readiness signal: earlier attempts sent the
input while the seat was reconnecting and are not evidence of cast behavior.


### Character animation graphs

A `character-animation-graph-pack/1` supplies bounded authored states and
ordered transitions over authoritative gameplay facts. Players select a graph
independently of their art and ability providers; the versioned
`character-graph-loadout/1` resolves both resources before activation. Missing
optional graph packs retain the requested choice and use default art until the
pack returns. Invalid replacements preserve the last working generation. Graph
states use the existing animation wire format, and phase is preserved or
restarted according to the authored edge. Graph and standalone clip selections
are mutually exclusive. No additional public subsystem or crate is introduced.

The cursor belongs to a player, raid and seat epoch and retires on explicit
avatar reassignment, including matching source fingerprints. An explicit newer
same-owner selection can reclaim that avatar; ordinary source reload and graph
provider disappearance/return cannot. Retiring graph presentation does not
discard an ability cancellation. Animation state and phase survive a
same-token socket resume and two listener rehosts. Walking
is detected by before/after authoritative position samples, rather than the
footstep phase, which can wrap to zero during continuous movement. Samples are
fenced across owner, tick, room, depth and topology changes. Same-room jumps
under 2.5 units remain indistinguishable from physical movement; fresh Runtime
binding still lacks an authoritative session-incarnation token. A replacement
seat cannot claim an old seat's graph until that context exists; graph refusal
leaves the existing unleased ability/art behavior unchanged, including the
legacy isolation issue below. Graph profile persistence, skeletal/blended
graphs, one-shot completion and native-owner handoff are not yet implemented.

The frozen composed-source ability suite passed 44 tests with four ignored;
policy checks passed 41 assertions. Coverage includes real movement across
footstep wrap, pauses and repeated observers, seat changes, rehosting, socket
resume, graph provider replacement, guest pixels and the connected-seat cast
regression, explicit reselection, overflow, fallback and cancellation. The six
new ownership tests also passed in the PR checkout with all 11 tested source
hashes matching the frozen candidate. A debug sample measured 233 ms for 100,000 paired runtime hooks (one graph cursor,
two heroes) and 114 ms from forced refresh through guest
pixels. These figures exclude simulation physics, the automatic watcher,
network and GPU presentation; they are not a full-frame or release benchmark.
The earlier wrap regression failed before the corrected implementation.

### Native integration gates

The tracked `cockpit/src/drive/together_abilities/tests.rs` is not the test
module wired by `cockpit/src/drive/mod.rs`; the active path is under
`tests/cockpit/drive/together_abilities__tests.rs`. The dormant file contains
distinct native ownership, acknowledgement and retry cases, so its assertions
need reconciliation with the native candidate before it can be removed. It is
not evidence of executed coverage.

Current legacy character selections are unleased resources indexed by numeric
seat. Retiring a prior seat's casts does not retire those sources, so a fresh
occupant can receive the prior selection when the Runtime installs them again.
Two real-token regressions confirmed this for both the first watch seat and a
regular seat after a fresh-token replacement and provider reload; same-token
resume occurred before each replacement. The isolated durable-profile candidate fences leased profiles but intentionally
retains legacy unleased semantics; it does not establish full seat isolation.
A migration needs an explicit current-reservation selection or claim, while
same-token resume and already-issued effects retain their existing owners.

The separate native-owner candidate passed its original sound regression and a
real process replacement retaining six seats, a Link receipt, journal and
previously issued HTTP sound bytes. Its broader cockpit execution exposed an
issued-visual loss on seat replacement and a controls-test stack overflow. A
refused-frame cue-drain test also failed with additional startup cues; their
ownership and delivery need investigation. These candidates are not in this
PR, and these focused passes do not prove their full lifecycle or cleanup.

On the later isolated native-owner candidate, seven real-process replacement
scenarios passed, including six seats and unchanged previously issued HTTP
sound bytes; six further process transport tests passed with worker-death,
open-frame retry and single cue/sound delivery. These tests used separately
stripped, hash-verified debug workers, not production release artifacts. The
full controls suite still has a clustered browser/host failure: the candidate
passed 81, failed five and ignored eight; a clean current-production baseline
also reproduced that cluster (74 passed, 10 failed, eight ignored). The native
candidate's original controls stack overflow is fixed in isolation. Native
open-Begin ownership, first Runtime/resource transfer, reconciled lifecycle,
profile legacy-seat migration and release packaging remain integration gates.
The isolated 300-card transport fixture measured raw Tick p50 20.239 ms and
p95 22.206 ms over 24 samples; the median JSON delta frame was 397 bytes and
the checkpoint was 302,332 bytes. Worker activation took 3.402 s excluding
Rust compilation. Rendering, native ability work, model work and durable IO
were excluded. This is a fixture diagnostic, not a production frame-budget
result, and warrants investigation before claiming the native path meets the
desired runtime or activation costs. The measured v17 candidate clones a full Run for its native ability path, and `DelveServer::step` calls `Shared::publish`
every step, whose `Core::snapshot` clones the accepted Run to apply native
presentation overlays. Publication also does field traversal/fingerprinting
and conditional typed Run decoding. Small wire deltas alone do not establish
a lightweight local-state fast path; both per-frame clones must be eliminated
or otherwise demonstrated to be off that path before integrating the cutover.

In a separate animation-profile/reservation candidate, a full host run passed
562, failed three and ignored 49: a restored HTTP response was empty, then two
restores reported browser generations busy. The same three cases passed in a
fresh process; that is not a full-suite pass. A clean current-PR host
baseline passed 553 with 49 ignored and no failures. A repeat of the candidate's
same exact host executable from the package directory then passed 565 with 49
ignored; the original failures remain evidence of nondeterminism rather than
a demonstrated cleanup fix. Existing real-generation coverage checks bounded
refusal, preservation of active hosts and a successful retry after an owner
retires. This candidate is not integrated.

A separate debug attribution run of that transport fixture observed zero
checkpoint callbacks during 24 ordinary ticks and a 397-byte median delta.
Raw Tick p50/p95 were 11.139/15.571 ms; typed client event staging was
4.486/6.163 ms, and local Session.apply was 6.206/6.973 ms. A small Cursor RPC
with typed validation was 0.095/0.694 ms. These are separate busy-host samples,
not a controlled speed comparison or an isolation of Publisher alone; rules,
invariants, field projection and diff are included in Session.apply.

A separate 64-sample debug observation-acquisition benchmark on that fixture
measured the old Run clone/drop at p50 446 µs, p95 926 µs, and a mean of
7,668 allocations requesting 1,090,023 bytes. Retaining the same immutable
HostView Arc measured below the timer's 1 µs resolution with zero allocations.
Only acquisition/drop was measured; this does not establish a frame speedup.
The Arc approach is in an unintegrated candidate, and the presentation
publication still clones the Run every step.

Next, compose a reservation-bound selection intent with graph and profile
ownership. A graph presentation refusal must not silently discard unrelated
ability cancellation, and explicit same-owner reselection must be distinguishable
from an unchanged reload after an independent avatar takeover. A fresh seat
needs authoritative current-reservation context before it can claim a prior
numeric seat; graph-off or provider disappearance must not erase that fence.
Coordinate this with native open-Begin retirement and preserve the original
Resource, visual, sound and pending-work owners through listener/worker handoff.

Independent review reproduced a stale profile-cleanup failure using a real
Kernel-bound resource and Runtime: replacing the Run with a different
session/seat owner carrying an intentionally matching avatar caused
`set_source_incarnation` to erase that avatar. The old cleanup compared only
`source_id`; it must also fence the actual session, reservation and seat epoch,
and preserve already-issued resource owners. Reproduction and failed output
are retained under `<local>/profile-independent-review/`; the profile/graph
composition has not yet fixed or integrated this case.

A separate 64-sample debug Publisher/Receiver attribution on the 610-card
fixture measured changing-tick Publisher p50/p95 3.522/4.775 ms with 10,359
mean allocation calls; Receiver p50/p95 4.478/6.080 ms with 17,901. Standalone
`Run::valid_snapshot` measured 2.794/4.273 ms and 9,612 calls. Even a
status-only Publisher still traversed/validated the Run, whereas the receiver
reused the typed Run for that case. These independent medians are not additive;
the measurements exclude RPC, rules stepping, rendering and checkpoint capture.
Incremental validation and typed view application remain necessary before
claiming the native ordinary-tick path meets the lightweight-state goal.

The unintegrated native reservation-handoff candidate has a separate confirmed
shutdown race. A real WebSocket paused after the old listener's stop check can
resume after its Sessions have transferred and admit a seat into the new
owner's shared reservations while its Joined event goes to the old listener.
The deterministic barrier regression fails in the isolated d985 candidate;
its test and log are retained under `<local>/profile-independent-review/` and
`<local>/profile-review-race-d985`. The current PR does not contain that Sessions
transfer. Fix and test both late and already-admitted connections, including
handoff and resource ownership, before bringing the candidate into the PR.

An isolated correction fences source admission under the reservation lock and
tracks watcher connection identities across handoff. Four real-connection
boundary tests cover late admission, an already-admitted seat, watcher
capacity and old cleanup, and a queued join with a retained stopped Manual;
the frozen candidate's full Serve suite passed 98 tests. This is not yet a
combined native-gameplay or profile test. Review also found that the first
candidate changed the public `Sessions::watcher_left` signature, so retain
source compatibility while qualifying keyed internal cleanup. Logs, source
hashes and the isolated patches are in `<local>/profile-independent-review/`.

The later isolated event-driven transport prototype preserves the public
watcher API and passed 106 Serve tests, three default-API tests, and Linux TLS
and all-features build checks. A real notifier-syscall failure and production
Sim recovery from a stopped handoff remain unproved; Windows and macOS builds
were not run. The first combined graph/profile ability run passed 41, failed
11 and ignored four. In particular, fresh selections made before a seat
exists must remain usable without granting a later replacement seat an old
selection. Their one-use provenance must originate at explicit native user
ingress: generic command dispatch also handles storage completions and module
restoration and cannot establish that provenance. No combined fix is shipped.
An explicit edit to one slot also cannot authorize unrelated restored art,
graph or slots from the same atomic loadout. The pending selection must carry
field-scoped authority, or start from a clean draft, and be consumed only by
the first proven matching reservation. Status, save, completion, reload and
edits for other seats must not create or extend that authority.
An isolated generic receipt implementation passed its default and V8 serial
suites, but independent adversarial tests exposed two blockers: a panicking
native selection factory terminates the module worker, and replacing a native
resource can run its destructor while the selection-authority mutex is held.
The latter permits deadlock or lock inversion. Preserve
`<local>/live-explicit-selection-evidence/adversarial-v1.log`; do not integrate
that version. A corrected isolated v2 passed the targeted panic and reentrant
destructor tests, 153 default tests and 173 V8 tests (six and eight ignored,
respectively), plus a V8 production-library build. Its source and receipts are
under `<local>/live-explicit-selection-evidence/`. The profile/graph composition
and actual user-ingress and reservation handoff remain unqualified.

On a separate native-overlay candidate, six of seven real worker replacement
tests passed; the seventh used a fixed two-second model delay. In a separate
controlled run, activation took approximately 3.2 seconds. Its blocking-model
test retained the job through replacement and returned its answer after
release, using frozen v19 workers with matching production source. That
isolated pass does not establish acceptable activation speed. A changing-frame
debug fixture with six animated avatars measured simulation-step p50/p95
16.186/29.925 ms and wire publication 5.305/10.677 ms; these separate
measurements include different work and are not additive. Repeat UI
observation was cached, while first materialization remained measurable.
These timings exclude production release and GPU performance.
The v20 isolated simulation suite subsequently passed 89 tests with eight
ignored, including the controlled model and actual retained Sound and Prepared
resource lifetime tests. Seven actual replacement scenarios then passed with
v20-built, hash-verified debug workers. The controlled model remained active
until release; activation was 1.182 seconds in that run, compared with 3.193
seconds in the earlier controlled sample under different load. These samples
are not a speed comparison. Integration with the current profile/transport
source and production release-worker qualification remain pending.

Built-in encounter templates are immutable compile-time inputs; repeated
validation now reuses a checked per-pack copy while each mutable Run catalogue
continues to own its boss values. A dynamically authored provider must bypass
or invalidate this built-in cache when that boundary becomes live. On a
matched debug 610-card/no-art fixture, three alternating baseline/cache runs
measured validation p50 2.660–2.688 ms versus 0.069–0.070 ms, Publisher p50
3.314–3.375 ms versus 0.558–0.596 ms, and Receiver p50 4.121–4.243 ms versus
1.139–1.247 ms. All six encoded traces were identical. These are separate
standalone observations excluding RPC, rules stepping and rendering, not a
whole-frame benchmark. Across all seven packs the cache retains about 63 KiB
of measured Rust heap/layout storage; cold lookups can be slower.
The formatted current-source tests passed 3 cache differential, 41 boss-gate,
64 session (one ignored) and 15 live-catalogue cases; the default-feature
WebAssembly type-check passed. A separate direct `bosses::read_into` regression
passed against both matched libraries for all packs, including two revisions,
another authored ID, other-pack preservation and malformed-reload rollback.
Results are under `<local>/tmp/native-owner-step2-evidence/`.

On the cache-qualified source, a separate 64-frame debug attribution found
that the full Receiver accepted a changing 610-card/no-art view in p50/p95
1.076/1.170 ms and requested a mean 1,302,221 bytes in 9,306 allocations.
Typed Run decoding plus display restoration, excluding final validation, took
0.826/0.972 ms. The nested Book decode accounted for 0.536/0.580 ms and
7,142 allocations; cloning an owned Book still needed those allocations,
while retaining an immutable `Arc<Book>` was below timer resolution and made
none. That Arc observation is a hypothetical acquisition measurement, not an
implemented optimization. The Book subtree's existing wire-document Arc was
unchanged for all 64 frames, and reconstructed Run values matched the actual
Receiver at the first and last frame. An ownership-aware typed reuse path is a
candidate next step only if it preserves public behavior, validation and
atomic rollback for malformed deltas. These component timings are not additive
and exclude the full frame and renderer; the standalone caller and prebuilt
libraries used different debug optimization levels. Source fingerprints,
commands and logs are in `typed-view-cost-v22-receipt.json` under
`<local>/tmp/native-owner-step2-evidence/`.

The combined native profile, selection and transport candidate passed an
isolated no-default cockpit compile check with all 10,313 source fingerprints
unchanged. This check does not run the new UI, clipboard, socket or
feature-enabled lifecycle tests and does not qualify the candidate for
integration. Static review identified two additional ingress boundaries: a
tool-generated UI key must not inherit the physical user's selection
authority through a custom keybinding, and a deferred clipboard submission
must retain the reservation and provider witnesses from the original submit
rather than recapturing them after the read finishes. Same-owner resealing
also needs to retain a matching bound lease across a transient disconnect
without accepting a replacement owner. The source receipt and compile log are
under `<local>/character-profile-graph-evidence/`; the candidate remains
separate until those regressions and the combined lifecycle gates pass.

The initial isolated Glass card-motion extraction passed 163 remote-feature
tests and failed three: a cycle-boundary comparison, remote packet size and a
legacy wave assertion. Its source fingerprints were unchanged during the run;
the failure log is `<local>/glass-card-evidence/v1-glass-remote-workspace.log`.
The replacement candidate must preserve reactions for existing version-one
art packs through an explicitly owned motion resource, cover provider removal
and replacement, and pass the old-viewer and actual-renderer tests before it
can be integrated. The initial candidate is not part of the PR.

The Registry does not currently provide an explicit module withdrawal. A
missing source is deliberately treated as a failed edit: the existing
`crates/live/src/tests.rs` removal regression requires the last good module to
remain active and report an error. Deleting a directory therefore does not
prove resource or route removal, and the Glass compatibility candidate's
deletion-based lifecycle assertions cannot pass as written. A withdrawal
contract needs to stop new admissions, reconcile dependent modules, and retain
issued jobs and leased resources until their owners release them. In
particular, Kernel completion and service invocation currently resolve through
the active module/route tables, so simply deleting those tables can strand
already-issued work. Design and qualification remain open; do not change the
failed-edit behavior to simulate withdrawal.

### Current private dev reconciliation (in progress)

Private dev advanced from `199e09c2cc9d` to `c51d804648f5` with a
cartridge and plug extraction. The isolated reconciliation retains the
`DriveWorld` character-resource binding and delegates prepared character
installation to the world while retaining Glass's independent squire clip
binding and application. It carries the animation pack and graph contracts
through the existing loadout binder. The two Rust lockfile records in the
release policy are calculated from the composed lockfiles. The protected seal
check passes and protected files match upstream. This is source reconciliation,
not a completed extraction: the merged source still needs feature-specific
build, lifecycle, behavior, cleanup and cost qualification before publication.

Glass card-motion v3h remains a separate candidate: its pre-reconciliation
Glass suite passed 170 tests (two manual tests ignored), a real GPU lifecycle
test passed, release fixtures passed 32 tests, and the old viewer accepted an
actual new packet. Motion is approximate, and explicit provider withdrawal
remains open. Native v2 passed the actual trusted-versus-synthetic F3 test and
31 clipboard tests, but a reservation-admission gap requires the unqualified
v3 fix. The two-file typed Book candidate also requires qualification on the
composed revision; prior component tests do not establish end-to-end cost.

On the first composed source, the no-default cockpit/bin/test check passed
(`cargo check --locked --no-default-features --bin angel --tests`; 10,364
source files matched before/after), 97 `angel-live` library tests passed with
four ignored, and 31 release-evidence tests passed. Review then found the
console-only Glass squire label missing and standalone cartridge tests lacking
the dependency testing feature. Their focused fixes are applied separately;
they require a fresh check and do not inherit the first source fingerprint.

The revised Glass-without-Delve cockpit check passed. The first default-feature
check found 23 stale test references to the pre-cartridge `drive` paths; these
are now redirected to the Delve test boundary and require a repeat check.

After redirecting the test paths, the default cockpit/bin/test check passed;
the standalone cartridge check and a boss-guest test passed in an isolated
checkout with an offline-generated standalone lock. On the composed source,
14 hotload tests passed (one manual timing ignored), 44 character lifecycle
tests passed (four ignored), 161 remote-enabled Glass library tests passed,
and 31 release-evidence tests passed. The character tests include six-seat
reload, live fairy fields, executable and animation swaps, and retained issued
sound and visual effects; they do not qualify the separate native v3 candidate.
All 10,364 source fingerprints matched across these checks.

With compilation idle, the ignored headless activation fixture measured 51
samples: file write through forced Registry refresh and App-to-Glass setter
p50/p95 1.459/1.948 ms; the setter alone was 0.006/0.010 ms. This excludes the
watcher, renderer and GPU, and its current assertion does not read back the
installed host identity. A separate real Registry-to-remote-viewer identity
regression passed for two valid edits and retention after a malformed edit. A four-seat Bend fairy-field fixture
measured 120 complete rules/ability ticks p50/p95 0.704/1.054 ms (maximum
1.240 ms), excluding rendering and compilation. Both are debug local samples,
not a comparison with a matching baseline or a full frame budget.

The Glass-without-Delve console fallback also passed its real runtime regression.
The identity and console tests ran against the same 10,364-file source
fingerprint with no changes during either run. Their receipts and the composed
check, lifecycle and timing logs are under `<local>/angelx-pr37-dev-c51-*`.
Explicit Registry withdrawal, the retained-publication prototype, the native
reservation fix, old-viewer requalification on this source and full
watcher-to-GPU latency remain open.

A post-publication plug audit found the merged cockpit had gained two
Delve-gated core tests and direct game-type names. The duplicate character
assertion remains covered by the existing cartridge-boundary hotload test; the
console fallback regression now lives with the console implementation while
still exercising the production showcase bridge. Both tests passed on an
unchanged 10,364-file source fingerprint. The plug check reports five core
gates (its limit) and zero stray game names; protected seals still pass.


## Implemented: Glass card motion with Registry V10

The implementation in private draft PR #37 was qualified in isolated source
`<local>/glass-card-motion-registry-v10c-47a7990d4`, based on
`47a7990d4c1d5d655f44aed0273aa4b30e4e715f`. It composes the authored
card-motion extraction, V9 dependency/withdrawal foundation and V10
empty-ready-scan shortcut. Its tested contents were published at
`c4eab07fb23999e2cba909019a55032c2c12f220`; compatibility with later private
dev changes still requires verification. The cartridge-first binder,
installed-identity regression, console-only fallback, plug boundary and
protected locks/seals remain intact.

Art and authored card tracks have separate owners. `/2` art declares its motion
provider; `/1` art without a motion import keeps the unique ambient compatibility
default. Explicit optional `/1` motion intent survives an absent provider in a
private agent binding: unrelated ambient providers cannot silently replace it.
The existing api-version-2 import declaration carries that intent; no public
Glass/Kernel API or wire-schema extension was added for the tests.

V10 isolates unrelated changed closures while preserving atomic provider and
consumer replacement. A failed captured dependent candidate retains its admitted
composition and blocks its affected provider update. Missing source retains
accepted authored bytes with a visible error, but a valid provider update may
rebind those accepted bytes and advance the dependent/composite generation.
This is source retention, not identity pinning or unregistration. Already-issued
compositions and Resource imports remain on their old prepared dependency.

Explicit exact-generation withdrawal rejects a required provider without its
required dependents. Joint withdrawal removes the installed Host resource;
optional consumers instead retain art-only fallback. Process-local disable
intent prevents source scans or a newly enabled dependent from resurrecting a
removed provider. Invalid re-enable preserves suppression atomically; valid
re-enable publishes fresh identities. Actual remote removal/catch-up frames
synchronize lease assertions: old composites release after publisher frames and
issued references drop. Registry jobs can separately drain, retry or abort;
these visual-resource tests do not imply that issued jobs were cancelled.

Qualification on one frozen 10,371-file tracked-plus-untracked source fingerprint:

| Executed gate | Result |
| --- | --- |
| Glass-without-Delve App hotload, including installed identity and required/optional lifecycle | 13 passed, 2 manual ignored |
| Whole agent hotload subtree, including binding/version/bootstrap/source retention | 10 passed, 1 manual ignored |
| Console-only Glass fallback | 1 passed |
| Full remote-enabled Glass library suite | 170 passed, 2 manual ignored |
| Explicit real Vulkan renderer replacement/removal fixture | 1 passed; Immediate fallback when Mailbox unavailable |
| Frozen historical old-viewer binary decoding the newly generated candidate packet | 1 passed |
| Delve+Glass character/ability lifecycle, including casts, cooldowns, seats, animation and issued effects | 44 passed, 4 manual ignored |
| Entire Delve cartridge drive/settlement subtree | 42 passed, 1 manual ignored |
| App and agent hotload repeated from that same Delve+Glass binary | 13 and 10 passed; 2 and 1 manual ignored |
| Production release static tests and plug/legacy checks | 32 passed; five core gates, zero stray game names |

The packet gate enforces a complete 65,536-byte publication ceiling, including
identity and framing. Its constructed maximum art/track fixture is 61,907 bytes
(61,376-byte prepared JSON, 49,138-byte motion). Motion remains an approximation
of old renderer float formulas: sampled maximum logical offsets are Busy
0.331825 px, Oops 0.072449 px and Cheer 0.039604 px; boundary label/direction
changes are bounded to 1 ms in the regression, not bit-exact pose parity.

A real production inventory/archive was generated with a separate temporary
Git index; the ordinary index was unchanged. Archive SHA-256 is
`3b8769fae2c4e21d940071236ae79825cf39ed08aeb1fe24e13f4c227802652c`
(394,024,960 bytes). All six art/motion runtime files byte-match the source,
including the provider pack; both manifest and JavaScript imports are verified,
and extracted module JavaScript bootstraps the archived provider. This is a
source-archive/module-bootstrap check, not a packaged binary launch.

Evidence is under `<local>/glass-card-evidence/47-motion-v10c-*`: source-before and
source-after-runtime JSON receipts match exactly; binary/packet hashes, full
runtime logs, real-release inventory and archive receipt are preserved. Original
V9/V9b/V10 failures remain preserved: the overstrict missing-source identity
assertion, V8 numeric JSON normalization, and a cleanup assertion before an
outstanding publisher frame finished. Revised tests assert actual accepted-source
semantics and wait for an observed remote removal before requiring release.

Remaining boundaries are a full packaged desktop launch, real watcher-to-GPU or
photon latency, exact old float parity, persisted disable intent, and composition
with the separate native withdrawal/reservation candidate. No performance run
was performed in this lane. Later documentation-only changes have separate final
source receipts and do not replace the frozen runtime fingerprint.

### Follow-up: package the existing character animation providers

The source-release inventory now includes the already-shipped optional
`character-motion` and `character-graphs` module manifests and entry points.
The abilities manifest imports both providers, but the previous inventory
omitted them, so source archives could not offer those animation choices.
The release test builds and extracts an archive from the real provider files,
checks their declared contracts against the abilities manifest, verifies exact
file bytes and executes each provider. All 33 release-evidence tests pass;
the protected-file and plug-boundary checks also pass. This establishes source
archive coverage for these providers, not a packaged executable or GPU launch.
The separate native ownership, persistent-choice and dependency-handoff
composition remains under qualification and is not part of this change.

### Policy override lifecycle

Pure live policy overrides can be withdrawn or re-enabled as an atomic batch
through the existing control path. A `policy-control/1` receipt identifies the
resulting generations. Withdrawal leaves bounded code-free tombstones and
preserves already captured Programs; future decisions use their existing
shipped Bend or native fallback. Re-enable validates every requested policy
before publication. Mixed policy/system batches and policy declarations with
Kernel imports or exports are rejected rather than implying unsupported
dependency handoff. Disabled intent is process-local.

On the exact implementation source, two normal-bundle embedded-V8 consumer
tests passed for captured effort/error policy withdrawal and fallback. The full
live V8 suite passed 169 tests with five ignored both with and without refresh
shadow; five lifecycle tests were confirmed in each run. The release evidence suite
passed 33 tests; a source archive was inspected against the captured Bend files
and policy bootstrap probes. A local worker fixture observed a captured
JavaScript dependency edit activating in 321 ms; this excludes Bend compilation,
browser rendering and GPU presentation.

The control changes future offered policy generations. It does not establish a
turn-wide context for all policy consumers, inheritance by delegated work,
persistent disable choices or propagation to the separate rules worker. Those
boundaries require their own implementation and tests before claiming complete
live replacement.
