# angel-cockpit

Angel's **terminal-first** cockpit: a Rust/ratatui agent harness that drives a
MOA system of model agents through a tool-using loop, with a verifiable-reward RL
substrate for software-dev tasks.

> **Direction:** angelX is a terminal-native application. The ordinary terminal
> is the complete production surface: model and thinking controls, portraits,
> miniviz, image viewing, shell, and transcripts must all work without an
> external renderer or terminal fork.

## Hot-swappable executable systems

The full Delve `Run::step` implementation runs in its own supervised executable.
The cockpit owns the committed game state, seats, sockets, terminal, sessions and
realm effects. Replacing the rules executable transfers complete live state and
switches dispatch between requests; the cockpit and existing game stay alive.

After updating the cockpit once, keep this running while editing Rust rules:

```sh
npm run dev:systems
```

The source launcher builds and publishes the initial worker automatically.
The watcher builds only `angel-delve-system` and its rules dependencies, then
atomically publishes `cockpit/systems/delve/engine`. It does not build the
cockpit, renderer, browser page, providers or V8. A failed build leaves the
running implementation available. `/reload status` shows the active content
identity, worker PID, successful handoffs, rejected candidates and current run.

A candidate first prepares on a background thread. At a request boundary it
restores a copied, versioned checkpoint, validates it, and activates. Only then
does dispatch switch and the old process retire. Failed preparation, state
conversion or activation keeps the existing implementation. A timed-out or
crashed active request retains the host's committed state and pauses rules;
publishing corrected code recovers from that state. The request is not replayed.

Live checkpoints include pending purchases, achievements, sound/notice queues,
input latches, private counters and minigame histories. Ordinary save files and
public wire payloads keep their existing serialization. Unsupported live-state
schemas are rejected explicitly. Host phrasebook and settings accompany rules
requests. Runtime resources remain host-owned, so workers cannot accidentally
restart a terminal or a multiplayer seat during handoff.

`ANGEL_DELVE_SYSTEM` selects a published executable; `ANGEL_HOT_SYSTEMS=0`
selects the embedded implementation. New prebuilt packages ship the worker;
older packages retain their embedded engine. The reusable supervisor and framed
protocol live in `crates/live/src/system.rs`; the Delve executable and its stable
boundary live in `crates/delve-system`.

Run the process lifecycle suite with `npm run test:live`. Test the two separately
compiled native implementations and complete state parity with:

```sh
cargo test --manifest-path crates/Cargo.toml -p angel-delve-system --profile fast --features swap-test
```

The native terminal suite also has an explicit
`ordinary_cockpit_real_pty_native_system_swap_preserves_the_host_and_run` case;
set `ANGEL_SYSTEM_TEST_WORKER` and `ANGEL_SYSTEM_TEST_REPLACEMENT` to those two
built binaries and run that case with `--ignored`.

The terminal renderer and provider/tool execution remain compiled into the host.
The JavaScript boundaries below cover composition and agent selection policy;
they have a separate, smaller contract from executable system handoff.

## Live application modules

Build this native host once, then leave it running while editing `cockpit/live/`.
The live boundary covers these production behaviors:

| Edit | What changes in the running host |
|---|---|
| `ui/index.js` | Responsive pane widths, idle/reasoning heights, side-panel order and visibility, idle/busy composer text. Existing chat, focus and hit-test geometry survive. |
| `game/index.js` | The authoritative Delve's movement, roll/attack/shot speed, enemy pace/damage/health/count, hit-stop, camera and adventure rules exposed by `/dungeon tune`. The run, knights and joined seats stay alive. |
| `tool-router/index.js`, `rank.js` | Actual per-turn tool shortlisting and ranking. The shipped BM25 algorithm matches the native fallback. A generation is pinned for each decision; edits apply at the next tool-bubble turn, including a cached-prefix seat. |
| `effort/effort.bend`, `index.js` | The reasoning effort chosen for each model request, authored in Bend (kind `policy`). The shipped `choose` returns the native effort. A choice the seat does not accept keeps the native one, and `LAWS.bend` states that for every edit. A generation is pinned per turn; an edit applies from the next turn, and the request receipt carries `effort_leg` (`live:<digest>`). |
| `errors/errors.bend`, `index.js` | How a failed provider request is classed (permanent, transient, quota, context), whether the turn replays it and how long it waits first. Its laws, checked on every build, say a rejected certificate or provider veto never replays; the host bounds every answer by the native cap, attempt budget and 2 s wait, and a looser answer falls back to native. Edits apply at the next turn. |
| `realm/index.js` | Miniworld guardian size, labels and animation durations. |
| `agent/index.js` | Optional next-turn guidance; empty text preserves the normal prompt. |
| `peers/`, `world-help/` | Explicit report presentation. |
| `characters/` | Registered `/character` command, `live.characters` surface, per-player appearance choices and animated character frames. State and choices survive implementation replacement. |
| `abilities/` | Registered `/ability` command, `live.abilities` surface and executable per-player spell timelines. Existing casts lease original code/assets while new casts use replacement code. |

For example, set `game`'s `knobs` to `{ hero_speed: 20, fire_rate: 150 }` while
playing. The whole rule set is checked against the engine's names, ranges and
steps before activation; one unknown rule rejects the complete edit. These are
an overlay on the workspace's saved tuning, not writes to its tuning file.
Removing a name restores its saved/native setting. `/dungeon tune` identifies
live overrides; ordinary tuning beneath an override remains available.

UI and game tuning modules generate validated native policy on reload. Executable
ability instances run one bounded JS call per active generation per simulation
tick, batching its casts. Tool routing runs on the agent worker, once
per eligible turn; it ranks the admitted catalogue and cannot register tools or
execute them. Native authorization, model streams and in-flight work retain their
owners. API 2 modules can register commands, cached text surfaces, services and prepared
resources through the shared kernel. Glass scene primitives, new physics
primitives and provider/tool implementations still have native dependencies;
their extraction remains listed in the modularity inventory.

Changes activate automatically after a stable capture and 120 ms debounce;
the watcher polls every 100 ms. Idle polls inspect metadata and reuse captured
bundles, so large unchanged asset packs are not reread or rehashed.
`/reload` validates immediately; `/reload status` shows active generations,
source digests and rejected edits. Routine reloads stay out of the transcript.
`/live <module-id> [JSON input]` invokes a module directly, so a new report can
be added without another native command implementation.

Each directory contains `module.json` and JavaScript:

```json
{"api_version":1,"id":"example","kind":"text","entry":"index.js","probe":{}}
```

```js
const helper = require('./helper.js')
module.exports = { apiVersion: 1, render(input) { return helper.describe(input) } }
```

For a stateful system use API 2:

```json
{"api_version":2,"id":"counter","kind":"system","entry":"index.js","state_schema":1,"exports":[{"kind":"command","name":"count","contract":"command/1"},{"kind":"surface","name":"counter","contract":"surface/1"}]}
```

```js
module.exports = {
  apiVersion: 2,
  initialize: () => ({ count: 0 }),
  reduce: (state, event) => ({ state: { count: state.count + 1 }, output: 'counted' }),
  view: state => ({ counter: { title: 'Counter', lines: [String(state.count)] } })
}
```

Install the directory while AngelX runs. `/count` dispatches the registered
command; `/open live.counter` opens its cached Glass surface. Editing its code
preserves the state. Add `migrate(previousCheckpoint, {schema})` when changing
state shape; a schema change without migration rejects the candidate. Candidate
initialization/migration, views and resource preparation run on copied state on
the loader worker, before an atomic dependency-group commit. Rendering reads
cached views. API 1 projections retain their smaller contract below.

`resources(state)` publishes named `resource` exports. Native resource adapters
prepare decoded data before commit and expose it through reference-counted
bindings. Declare local binary files in manifest `assets`; `asset('idle.png')`
returns their content identity. Source and asset edits form one captured
generation; missing files or failed decoding retain the working generation.
Each asset is at most 2 MiB, with 16 MiB/64 assets per module. Consumers lease
immutable bytes/prepared data until they retire. API 2 state and resource data
are each limited to 512 KiB, views/output to 64 KiB each, with a 1,152 KiB total
serialized evaluator envelope.

For character appearance, `/character violet 1`, `/character gold 2` and
`/character off 1` select options for a player. `/open live.characters` shows
the choices. Edit `characters/looks.js` or add an option to its catalogue; new
options do not require a Rust variant. Frames may contain 1,024 RGBA pixels or
a declared 32×32 image asset. Up to eight frames and a tick interval define the
animation. Compatible replacements preserve normalized phase and do not reset
health, position or cooldowns. Appearances remain in full saves and replication, while the rules RPC excludes
those bytes and the host reattaches its prepared appearance after validation.
The remaining Run still crosses that bridge; module-owned state/view deltas are
still open.
Art, executable abilities, sounds and effects are independent module catalogues.
The `abilities` module resolves player choices into one `character-loadout/1`
resource per player; `/loadout` inspects choices. Art and slots install together
under the simulation lock. Invalid selected options reject the complete closure.

For executable spells, `/ability spark 1 1` or `/ability bloom 2 3` binds an
authored ability to a player's slot; `/ability off [player] [slot]` restores the
ordinary spell. `/open live.abilities` shows the current catalogue. Edit or add
behavior in `ability-code/spells.js` while playing. The native port supports actual
projectile, heal and collision-aware move commands. Mana/cooldowns remain paid
across replacements. Pending and running casts retain their original executable
generation; new casts use the replacement. `/ability cancel [player]` explicitly
abandons pending/running module casts, including quarantined runtime failures.
`/reload status` shows active/pending casts, generations, accepted starts,
applied actions and faults. A rejected candidate retains the working generation.

Ability sounds are module resources too: `/ability sound crystal|warm|quiet
[player]` selects a bank for future casts. Already-running casts retain their
original cue map, volume and captured bytes. Replace a declared WAV or edit its
mapping in `ability-sounds/sounds.js`; invalid audio rejects the complete candidate. The current
port supports complete PCM16 WAV files, mono/stereo, 8–48 kHz and at most ten
seconds. Native helpers, browser sources and remote native clients retain their
own playback data while generations retire. `sounds=` in `/reload status` counts
committed sound events; ordinary audio throttling can coalesce playback.

Ability VFX are authored in `ability-effects/effects.js`: `/ability effect
crystal|ember|quiet [player]` selects a graph bank. Ring, disc, particle and
captured sprite nodes have timed position, scale, colour and opacity keys.
Sprite frames share the character decoder. The same bounded painter runs in
native and browser Glass; painting performs no JS, image decode or file access.
Effects retain their original graph, assets and source generation after a cast
finishes and across code, image or player-choice changes. New casts take the new
selection. `/ability cancel [player]` also retires that player's active VFX.

Graph/instance publication changes only at admission, expiry or cancellation;
unchanged views reuse their serialized scene. Graph payloads travel once while
instances refer to them by content identity and birth tick. Rules-only RPC omits
this presentation data, which the host retains. Full live checkpoints use schema
3 with explicit schema-1/2 migration; durable saves omit transient effects.
`vfx=`, `vfx-active=` and `vfx-generations=` in `/reload status` inspect ownership.
Each cast retains its resolved selections and executes through its selected code
provider. Sound/art/effect edits rebind player loadouts without restarting that
code instance. Existing casts keep their old media; future casts use replacements.
Adding another declared provider or composing slots from different providers
requires authored module changes, with no native spell or App branch.
Character-event hooks, trails/lighting/shake and richer animation/physics ports
remain open.

Services declare `service` exports and consumers declare exact module/contract
imports. A reducer can return `effects: [{service, method, arguments}]`; the
kernel pins both generations, dispatches each call once and returns a
`completion` event to the issuing module. The worker drives these calls off the
UI thread. A failed completion is retained for diagnosis; `/reload cancel <id>`
explicitly abandons it without replaying the service. Host tool/model/network
execution is still owned by the existing host and is not exposed by this API.

The id matches the directory name. `text` returns a string; `ui`, `game`, `realm`,
`agent` and `tool-router` use the schemas shown in their shipped examples. The
probe must exercise the module with representative input. Relative CommonJS
dependencies within the directory are captured as one immutable generation;
Node builtins, npm packages, ES-module imports and background callbacks are
outside this first host contract. Each call uses a fresh isolate in the existing
V8 engine, with a 200 ms deadline and a 32 MiB heap budget. JSON input is bounded
at 512 KiB and output at 64 KiB. Modules transform JSON;
native code continues to own tool execution, processes, streams and persistence.

File capture and validation run on one background thread, with no JavaScript or
filesystem polling in the draw loop. Unchanged generations do not re-evaluate.
Bad syntax, missing dependencies and invalid results retain the working code.
Removing files retains the previous generation and reports the missing source;
it does not silently unload a live feature. Sessions and work are never restarted.

`ANGEL_LIVE_DIR` selects an operator-owned module root instead of
`<resource-root>/cockpit/live`; `ANGEL_HOTLOAD=0` disables the loader. Runtime
modules ship with the resource bundle and do not need the producing checkout.
The source root is selected by the launcher, not by the current project.

An agent file tool (`write_file`, `str_replace`, `multi_edit`, `apply_patch`)
that writes inside the module root returns the loader's verdict with its own
result: the `/reload status` line of each module it touched plus the time the
verdict took, which is the new generation, or the kept generation with the
compiler, proof or contract error. A verdict slower than two seconds rides the
next tool result, or the next turn's context. The session's workspace context
names the root as `live_dir`.

The loader is the independent `angel-live` crate. Run its small suite with
`npm run test:live`; the wrapper pins the repository toolchain and uses a
separate small build cache. Node is used as the test evaluator. The cockpit's
`agent::hotload::tests` also exercises the production embedded-V8 adapter. The
`interactive_terminal_smoke` target's
`ordinary_cockpit_real_pty_live_module_replacement_and_bad_edit_smoke` test
checks automatic replacement and a rejected edit inside the running terminal
host, including a composer repaint. `ordinary_cockpit_real_pty_live_game_rules_smoke`
checks a running Delve's rules, atomic rejection and override removal. Set
`ANGEL_PTY_SMOKE_STRICT=1` to require an available PTY. Native surface, simulation
and context/tool-selection suites cover layout/copy geometry, actual movement,
schema preservation and ranking parity.
Changing native Rust rendering or execution still needs a native build.

## Model routes

The default club list uses ChatGPT/Grok OAuth and reachable local models.
Every model you serve yourself is a mode of one expandable `local` box:
`ANGEL_LOCAL_URL` is the first, `ANGEL_LOCAL_URLS` (`url` or `name=url`,
comma-separated) adds more, and opt-in discovery folds found surfaces in too.
Saved API keys do not enable additional providers. Muse and the separate
OpenAI API-key seat are removed, and no free-model catalog is auto-added.
An API-key subscription worker must explicitly enable its provider with
`ANGEL_API_CLUBS` (for example `glm`); this does not enable other providers.
Credentials are retained on disk, but disabled API keys are stripped from the
launcher environment, including with `ANGEL_NO_BUILD=1`.

`/loop` follows the selected concrete model unless `ANGEL_LOOP_LOCAL_CLUB`
explicitly overrides it. Account/billing/auth failures pause without retrying
the outer loop or escalating to a swarm. Select a working authorized route
and `/loop resume` to continue with the existing evidence.

Output limits follow the configured provider seat, even when its model label
or endpoint is customized. For example, an `openrouter` seat uses
`ANGEL_OPENROUTER_MAX_TOKENS`; a `glm` seat uses `ANGEL_GLM_MAX_TOKENS`.
The seat-specific value overrides `ANGEL_CLUB_MAX_TOKENS`. These are fixed
per-request ceilings: truncation retries and learned defaults cannot raise
them. An explicitly configured extraction limit is also fixed when applied.
Automatically inferred defaults may still grow within the bounded retry
policy. With no applicable limit or inferred default, the provider chooses
its output budget. A per-request limit is separate from the task's hop and
deadline bounds; it is not a total token or spending limit.

## Layout

`src/` is seven directories, ordered so dependencies point downward. `app`
reaches into everything; `platform` reaches into almost nothing.

| | |
|---|---|
| `app/` | the shell: central `App` state, command dispatch, startup wiring |
| `ui/` | rendering and input — layout, widgets, panes, live visualizations |
| `stage/` | the miniworld and its ceremonies |
| `drive/` | unattended controllers that run the engine over many turns |
| `agent/` | the model runtime: harness, tools, providers, sandbox, edits |
| `knowledge/` | what the cockpit records and recalls |
| `platform/` | process, paths, workspace identity, operator-owned policy |

A new module belongs in the lowest layer that can hold it: if it draws, `ui/`;
if it only computes or persists, `knowledge/` or `platform/`; if it runs the
model, `agent/`; if it runs unattended across turns, `drive/`.

## What's here

- `src/main.rs` — the ratatui TUI: chat with the in-hand agent, `Tab` to switch
  agents, `^G` shell pane, `^T` chart, and `/show` images or local MP4 reels in
  the Braille Scryglass. The agent turn runs on a
  worker thread; **Esc/^C soft-interrupt** a running turn. One turn has a
  64-tool-hop runaway guard by default; `ANGEL_MAX_HOPS=0` explicitly makes an
  individual turn unbounded, while later settled turns and campaigns remain
  separate.
- `src/knowledge/session.rs` — **session log + resume.** Published snapshots atomically
  replace `~/.angelX/sessions/<id>.json`; writes are queued off the UI thread, so
  abrupt process death can still lose the newest enqueue-to-write window and a
  surfaced write failure leaves only the prior good snapshot. `/sessions` lists
  saved sessions (newest first); `/resume [id]` reloads one (no id = latest) and
  continues writing to it.
- `src/agent/club/` — the internal `Club` trait, OpenAI-compatible HTTP/model
  implementations, the always-available `PracticeClub`, and the `Bag` route
  selector. `ANGEL_DRIVER` is an explicit preference; when it is unset, configured
  cloud routes and then reachable fleet models are preferred before the practice
  floor.
- `src/agent/swarm/` and `src/agent/formations.rs` — explicit local/SOTA
  Mixture-of-Agents and roster machinery: parallel personas with optional
  research / reflect / judge / samples / verify / cite / hedge stages
  (`ANGEL_MOA_*`). MoA is an engaged formation or selected swarm route, not the
  default meaning of an unset `ANGEL_DRIVER`.
- `src/drive/deli.rs` — the `deli` driver (`ANGEL_DRIVER=deli`): a single-session run of
  the Deli_AutoResearch loop, thinking **deep over time** — bounded fresh-context
  iterations that accumulate findings, with stall detection, forced structural
  pivots, and direction diversity (`ANGEL_DELI_ROUNDS/_PIVOT/_STALL_STOP/`
  `_MIN_FINDINGS/_STATE_DIR`).
- `src/agent/harness/` — the tool-loop (`run_turn`) + `ToolRegistry` + the tools.
  On startup it auto-injects the workspace's **AGENTS.md** (git-root→workspace,
  the Codex convention) into the system preamble so repo conventions ride along
  (`ANGEL_PROJECT_DOC=0` disables).
- `src/agent/code_mode.rs` — the `code_mode` tool's engine (**on by default**;
  `ANGEL_CODE_MODE=0` disables): a **synchronous** V8 runtime (the cockpit is
  tokio-free) that runs a model-authored JS program with every cockpit tool bound
  as a host function, so one call can loop/branch/filter over tools in a single
  turn. `batch([{tool,args},…])` runs footprint-safe segments **concurrently**
  around strict effect/conflict barriers (the mass-testing/mass-build primitive
  — `std::thread::scope`, per-item error capture, cap via
  `ANGEL_CODE_MODE_BATCH_CONCURRENCY`). The only capabilities a script has are
  those tools (no net/fs of its own), each keeping its existing
  sandboxing; a watchdog terminates runaway JS (`ANGEL_CODE_MODE_TIMEOUT_MS`
  default 300s/`0`=unbounded, `ANGEL_CODE_MODE_HEAP_MB`). Strip-mined from Codex's
  `code-mode`, rebuilt blocking.
- `src/drive/reinforce.rs` — the self-reinforce loop + reward types.
- `src/agent/sandbox.rs`, `src/platform/pty.rs`, `src/ui/{viewer,chart}.rs` — landlock sandbox, full-access PTY pane,
  portrait image protocol support (`ANGEL_IMAGE_PROTOCOL=kitty|sixel|iterm2`
  when explicitly selected), and bounded/off-thread terminal-native caches.
- `src/ui/scryglass.rs` — the persistent Braille stage for the agent-driven
  live first-person world, brief Arrival establishing plates, eight-second still
  reveals, and silent local MP4 playback. F4 focuses it; drag or arrows look,
  the wheel or `+`/`-` changes the lens, right-click or `0` returns to
  agent-follow, and `m` toggles the map. A click on the 2D map (or `p`, or
  `/world walk`) puts your own knight on it: arrows or WASD walk, Enter goes
  in at a door, Esc leaves (`src/app/control/overworld_walk.rs`,
  `src/stage/world_viz/overworld/visitor.rs`). `/learn [topic]` opens the local-first
  Scriptorium tutor and selectable Catalog; optional reference enrichment never
  blocks its objective, exercise, checkpoint, or editable Ask Tutor draft.
  Delivered visuals queue behind the active reveal; manual `/show` and `/open N`
  reveals pin until dismissed.
- `src/stage/world_viz/life.rs` — the living realm's clock: a slow weather drift
  folded with session health, and the renown ladder that grows the town (dock →
  windmill → market → tall keep), which the overworld draws on its own sites.
- `src/app/observatory.rs` — the native read-only campaign/report gallery inside
  the ordinary Artifacts pane. `/observatory` loads the report house;
  `/observatory campaign <id>` and `/observatory open <report-id>` retain exact
  command access. With an empty composer, arrows traverse the windowed campaign
  rail and report ledger and Enter opens the selected report through the normal
  media path.
- `src/ui/ui_inspect.rs` — the ordinary cockpit's visual acceptance broker.
  `ui_verify` applies one typed, display-only control or Observatory campaign
  operation through the production input path before draw; `ui_inspect`
  performs a read-only draw or pages a cached snapshot. Both return normalized
  exact Ratatui cells and matching semantic state from one immutable completed
  frame.

## Agent toolset

File tools are workspace-scoped and descriptor-confined. On Linux the direct
read/write/edit path is anchored beneath the workspace with `openat2` where
available (and a no-follow component fallback otherwise), so validation and use
cannot be split by a symlink swap. `safe_path` still rejects absolute and lexical
`..` escapes for pathname-oriented operations.

The workspace defaults to the **directory you launched `angel` in** (like Claude
Code), overridable by `$ANGEL_WORKSPACE`. Change it mid-session with `/cd <path>`
(alias `/workspace`; no arg prints the current root) — that rebuilds the tool
registry scoped to the new directory. `/sandbox` always shows the active root.

`read_file` is paged rather than a whole-file context dump: it returns 200
complete source lines by default (400 maximum, 20 KiB maximum). When a page has
more content it prints the one-based `offset` for the next contiguous page; pass
that offset back with an optional smaller `limit` when inspecting a large file.

Context protection is aggregate, not merely per-tool: after normal compaction,
if a fresh protected tool tail would still exceed the active model window, the
harness trims its oldest result(s) to a clear re-run marker before the next
request. This adds no command, tool, or model hop. Recalled long-term-memory
notes are likewise kept as complete ranked blocks inside a bounded budget.
Compaction also carries bounded machine state for recently read/modified paths
and typed verifier outcomes. Verification evidence is never inferred from prose:
denied actions do not count, unfamiliar reports remain inconclusive, and any
later successful mutation invalidates older outcomes.
The latest successful body for each of eight distinct skills and the current
todo snapshot bypass ordinary age/size pruning so active working rules reach the
summarizer intact. Invoked skill names survive as a bounded machine ledger; the
aggregate context-fit pass may still trim any payload to honor the model window.
When compaction would consume the latest user request, the harness also retains
one bounded copy in `User` role after the summary. This is rolling and
suffix-aware: repeated passes replace rather than duplicate it, and a newer
surviving user direction suppresses a stale background anchor. Provider-overflow
recovery uses the same contract.
Current todo step/status state is deterministic across compaction too: every
successful paired `todo` result ends in canonical state, and up to 32 prioritized
steps cross in a budget-bound `Assistant` message. The `System` note contains
only its SHA-256 proof, so raw plan text is not promoted in authority; tampered,
unpaired, failed, denied, or forged snapshots are ignored.
Todo replacement validates atomically before changing state, and its single-copy
canonical output is capped at 64 items × 500 characters to bound resend cost.

| Category                   | Tools                                                                                                                                                                                                                                                                                                          |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Edit                       | `read_file`, `write_file`, `str_replace`, `multi_edit` (atomic batch), `apply_patch` (unified diff)                                                                                                                                                                                                            |
| Navigate                   | `grep` (ripgrep + fallback), `find_files` (glob), `file_search` (fuzzy filename), `outline` (symbol map), `list_dir`                                                                                                                                                                                           |
| LSP (auto; `ANGEL_LSP=0/1` overrides) | analyzer-grade intelligence from one warm language server: local `/diagnostics <file>`, `/symbols <file>`, `/symbol <query>`, and `/definition\|references\|hover <file> <symbol>` run off the UI thread; model tools expose the same diagnostics, navigation, outline, and project-symbol capabilities                                                                                                               |
| Git (read-only)            | `git_diff` (uncommitted changes), `git_status` (branch + staged/modified/untracked), `git_log` (recent commits, path-scopable)                                                                                                                                                                                 |
| Web                        | `web_search` (local SearXNG; `ANGEL_WEB_SEARCH=0` disables, `ANGEL_SEARXNG_URL` overrides), `web_fetch` (GET → readable text), `http_request` (full-verb HTTP with headers/body for JSON APIs)                                                                                                                 |
| Build/verify               | `shell` (landlock¹), `cargo`, `run_tests`, `lint` (clippy), `check` (cargo check), `fmt`. Local `/build [cargo args]`, `/run [cargo args]`, `/check [cargo args]`, `/test [cargo args]`, and `/lint [cargo args]` reuse those tools off the UI thread; `/verify [cargo args]` runs non-mutating fmt-check → check → lint → tests and says `clean` only when every stage proves it. `/fmt` checks only; exact `/fmt write` formats. Each owns the named flight slot and is physically cancellable. |
| Processes                  | `proc_run` (launch a daemon — inference server, dev server, long build — detached, log to `~/.angelX/proc`), `proc_status` (state + log tail), `proc_stop` (SIGTERM→SIGKILL the whole tree)                                                                                                                     |
| GPU/Fleet (read-only)      | `gpu_stat` (util/VRAM/temp/power/processes; nvidia-smi → rocm-smi → xpu-smi), `fleet_status` (tailscale peers), `vast_instances` (rented pods; recon only — never destroys)                                                                                                                                    |
| LLM endpoints              | `llm_probe` (is it up + what does it serve), `llm_bench` (measured TTFT + decode tok/s, warmup excluded)                                                                                                                                                                                                       |
| Technical decisions | `jev_decide` (deferred, configured TypeSafe key; advisory probabilities) and `benchmark_compare` (deferred, local paired-sample percentages and speedups). See [Jev workflows](docs/JEV.md). |
| Plan                       | `todo`, `handoff` (agent-authored resume brief; the latest note is carried verbatim across context compaction in Assistant role)                                                                                                                                                                               |
| Orchestrate                | `delegate` / `integrate` (specialist clubs in isolated git worktrees)                                                                                                                                                                                                                                          |
| Code-mode                  | `code_mode` (**on by default**; `ANGEL_CODE_MODE=0` disables): run a JS program in an embedded V8 isolate that calls the tools above as host functions — loops/branching/filtering in **one** turn instead of many round-trips. `batch([{tool,args},…])` fans footprint-safe segments out **concurrently** around strict ordered barriers |

Model-emitted mixed tool batches are partitioned into maximal consecutive,
pairwise-safe footprint segments. Safe reads and disjoint writes run in
parallel, while conflicts and effectful tools remain strict in-order barriers;
tool results are always committed in original call-ID order. The same scheduler
governs `code_mode`'s JavaScript `batch()`, whose malformed schedules fail
closed to one serial call per segment.

Both `shell` and generic `cargo` expose the same canonical command verdict to
the model and experience ledger. A trustworthy nonzero exit is a tool error;
timeouts or unavailable status are explicitly inconclusive; only exit zero gets
a pass receipt. `cargo test` still needs observed test results to count as green.

¹ **`shell` sandbox posture:** every seat's shell runs with the same full
developer posture. Writes reach the active workspace plus the scratch/runtime/
cache, per-user-install, worktree-Git, process-log and accelerator-device roots
local tools need, so the GPU is always reachable. The **whole filesystem is
readable**, the **network is on**, and the child **inherits the parent
environment**. It is an accident guard between projects (it does not grant all
of `$HOME` or the launch cwd), not a cage for the model: there is no read-only
or reduced posture, and none should be added. It is on by default;
`ANGEL_SANDBOX=0` runs unconfined. The exact writable-root contract is in
[`docs/ENV.md`](docs/ENV.md).

**Platform: Linux-only in practice.** Landlock is implemented only on Linux; on
macOS `sandbox::apply` returns `Err`, and because it runs in the child's
`pre_exec` the spawn fails closed — so `shell`/`cargo`/`run_tests`/`lint`/`check`
all error out there. Native Windows won't even compile (an unconditional
`use std::os::unix::process::CommandExt` in `harness.rs`); "Windows" support means
WSL only.

## RL / reinforcement (RLVR)

Verifiable rewards from ground truth, not just an LLM judge:

- `run_tests`/`lint`/`check` return structured counts **and** a `[0,1]` reward. `run_tests` picks the runner
  from the workspace's own files — `cargo test`, `npm test` / `node --test`, `pytest` / `unittest discover`,
  `go test ./...`, `swift test` — resolves the largest of several nested crates (angelX's `cockpit/`), and
  takes `{dir}` / `{crate}` to point at a subtree (`sidecar/forge` runs its python suite).
- `reinforce.rs` reward impls: `TestReward`, `LintReward`, `CompositeReward`
  (+ `code_health()` = 80% tests + 20% lint).
- `run_coding_eval(club, registry, task, verify)` drives a task through
  `run_turn` then scores it with a verifiable reward — the end-to-end loop.
- Set `ANGEL_TRAJECTORY_LOG=1` to persist each rollout as **reward-labeled
  JSONL** under `~/.angelX/trajectories` (training data).

## Run

```sh
ANGEL_LOCAL_URL=http://127.0.0.1:8080/v1 cargo run   # your own model server as the `local` box
cargo run                                            # practice floor, upgraded when a live route is reachable
../bin/angelX --yolo                                 # unrestricted operator profile
```

`bin/angelX` builds and serves the cockpit and bundled world assets from the
checkout that contains the launcher, so switching branches changes the next
normal launch. Run `angelX update` after moving or replacing the checkout to
refresh the CLI and desktop shortcuts; leave `ANGEL_NO_BUILD` unset while
reviewing branch changes.

`--yolo` sets the single `ANGEL_YOLO=1` profile. It bypasses ordinary action
approvals, Landlock (including reviewer postures), subprocess/HTTP/formation and
shared tool timeouts, sealed child-environment scrubbing,
background-descendant cleanup, PreToolUse denials, and
effect/private-network restrictions. Turn hop/deadline, progress, completion,
verification, and multi-turn loop budgets remain active unless their own
`ANGEL_*` control disables them. Toggle it live with `/yolo on|off|status`;
Esc/^C and `/stop` remain explicit operator controls. Use `proc_run` when a
long-lived process should also have a durable handle and log; ordinary
`command &` descendants are left alive.

The default build enables local MP4 decoding through the vendored dotmax/FFmpeg
path. `cargo run --no-default-features` keeps the world and still-image
Scryglass but reports reels as unavailable, which is the supported build for
hosts without FFmpeg development libraries.

### Brain Route controls

- `F9` or a `MODEL` button opens the concrete route deck; `F10` or `THINK`
  opens the selected model's backend-supported reasoning levels. `F9 MODEL`
  and `F10 THINK` lead the global command rail and remain clickable when a
  narrow terminal collapses the entire agent pane.
- Press `/` inside either deck to filter exact agent/model slugs or effort
  names. Arrow keys move, `Tab` crosses MODEL/THINK, and `Enter` confirms.
- Model rows lead with the actual model and identify its connection alongside
  it. Aliases of the same backend appear once; separate connections serving
  the same model remain distinct. Ready models are shown first. `a` or **All**
  reveals offline entries; searching also includes them, without enabling them.
- `d` or **Details** shows operational history and user ratings. The ordinary
  picker keeps model choice and supported thinking controls in front.
- `/think <filter>` (also `/thinking` or `/effort`) opens the selected model's
  prefiltered effort deck and still waits for Enter before changing anything.
- `/model <filter>` opens an already-filtered deck. The form
  `/model <exact-model>@<effort>` prepares both choices, but still waits for
  explicit confirmation. `/model auto` clears the remembered explicit route.
- Model rows keep one-click route selection and expose a separate
  `[think: level ▾]` target for inspect-without-activating. Within the picker,
  `[` and `]` preview the highlighted model's thinking levels; Enter applies
  the model and level together, and Esc cancels. Operational (`★`) and explicit-user
  quality (`◆`) picks remain advisory until confirmed; both reserve at least
  20% of a model's known context window. Live `ctx ~N%` row badges mark tight
  and full candidates before switching; a known ≥95%-full route requires a
  second Enter/click, while `Esc` leaves it untouched for `/compact`.
- The same `★`/`◆` evidence channels rank backend-supported THINK levels;
  `o` or `q` focuses the recommended effort but never applies it without Enter.
- Moving the MODEL cursor previews that agent's portrait. Deck previews are
  reversible: the route and backend effort remain unchanged until Enter/click,
  and Esc restores the committed identity.
- Agent portraits use the selected inline pixel protocol when the terminal is
  recognized or calibrated at startup, with cached colored half blocks only as the compatibility fallback. Their
  active/idle states are prefetched so a turn does not blank the portrait, and
  Codex carries distinct neutral/active art.

### Formation roster controls

- Click `FORMATION` or enter `/moa` to open the formation board. Preset aliases
  such as `/moa recon`, `/moa gpu`, and `/moa math` draft a roster for review; they do not
  spend tokens or start a turn on their own.
- Choose the formation, move into its execution graph with `Tab`, select an
  active or reserve seat, and assign one concrete model. The board keeps exact
  repeated-seat order and reports readiness, estimated token budget, cost, and
  local/remote composition before engagement.
- `Engage next` consumes the roster after one successful MoA turn. `Engage
  session` keeps it for later turns. Reopening the board while an MoA run is in
  flight stages the replacement roster without changing the running lineup.
- `/moa <message>` sends only through an already engaged next-turn or session
  roster; otherwise the message remains in the composer and the board opens.

### Agent-graph role pools

- `/graph list` shows bundled and `~/.angelX/graphs` TOML specs; `/graph run
  <name> <task>` launches one and opens its live Round Table stage.
- Nodes with the same `pool` are one reusable logical specialist role. Their
  persona, club, tools, effort, and trainable intent must match; prompts and
  dependencies may differ. `pool_max_inflight` bounds simultaneous task leases
  for that role and defaults to one, composing with the graph-wide seat cap.
- Fan-in sends a digest-labeled typed result envelope for every dependency and
  shares `ANGEL_GRAPH_DEP_CAP` as one total body budget. This keeps large task
  waves auditable without funneling an independently capped full answer from
  every worker into the synthesizer. See bundled `research-pool` for the
  evidence/counterevidence/constraints → synthesis → reviewer pattern.

Common env knobs: `ANGEL_DRIVER`, `ANGEL_<LABEL>_URL`/`_MODEL`, `ANGEL_MAX_HOPS`
(0 = explicitly unbounded; headless/sub-agent default 64), `ANGEL_IMAGE_PROTOCOL`
(`kitty`, `sixel`, `iterm2`, or `halfblocks`), `ANGEL_TRAJECTORY_LOG`,
`ANGEL_SESSION_DIR` (default `~/.angelX/sessions`). **Full reference:
[`docs/ENV.md`](docs/ENV.md)** — every `ANGEL_*` knob, grouped, with defaults (a
test keeps it complete).

### Local Rust quality gate

From the repository root, run `npm run check:cockpit-quality` after preparing the
locked Cargo dependencies. This offline gate checks quarantine independence,
Rust formatting, and all-target strict Clippy with the existing `dead-code` and
`clippy::type-complexity` exceptions. It exits on the first failure and uses one
Cargo job unless `CARGO_BUILD_JOBS` is set. Run the ordinary Cargo/PTY tests too
when behavior changes; a clean lint gate does not establish behavioral coverage.

Typed `run_tests`, `check`, and `lint` now accept `runtime: auto|rust|node|python`.
Auto selection requires one language at the workspace root; choose an explicit
runtime in a mixed project. Node's builtin test runner and Python's stdlib
unittest runner produce attributed test receipts. Native `check` is syntax-only.
Custom test scripts/frameworks and native lint configurations without a supported
adapter return an explicit inconclusive result. The `cargo` tool remains Rust-only.
