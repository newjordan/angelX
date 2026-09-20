# angel-cockpit

Angel's **terminal-first** cockpit: a Rust/ratatui agent harness that drives a
MOA system of model agents through a tool-using loop, with a verifiable-reward RL
substrate for software-dev tasks.

> **Direction:** angel0 is a terminal-native application. The ordinary terminal
> is the complete production surface: model and thinking controls, portraits,
> miniviz, image viewing, shell, and transcripts must all work without an
> external renderer or terminal fork.

## Model routes

The default club list uses ChatGPT/Grok OAuth and reachable local models.
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

## What's here

- `src/main.rs` — the ratatui TUI: chat with the in-hand agent, `Tab` to switch
  agents, `^G` shell pane, `^T` chart, and `/show` images or local MP4 reels in
  the Braille Scryglass. The agent turn runs on a
  worker thread; **Esc/^C soft-interrupt** a running turn. One turn has a
  64-tool-hop runaway guard by default; `ANGEL_MAX_HOPS=0` explicitly makes an
  individual turn unbounded, while later settled turns and campaigns remain
  separate.
- `src/session.rs` — **session log + resume.** Published snapshots atomically
  replace `~/.angel0/sessions/<id>.json`; writes are queued off the UI thread, so
  abrupt process death can still lose the newest enqueue-to-write window and a
  surfaced write failure leaves only the prior good snapshot. `/sessions` lists
  saved sessions (newest first); `/resume [id]` reloads one (no id = latest) and
  continues writing to it.
- `src/club/` — the internal `Club` trait, OpenAI-compatible HTTP/model
  implementations, the always-available `PracticeClub`, and the `Bag` route
  selector. `ANGEL_DRIVER` is an explicit preference; when it is unset, configured
  cloud routes and then reachable fleet models are preferred before the practice
  floor.
- `src/swarm/` and `src/formations.rs` — explicit local/SOTA
  Mixture-of-Agents and roster machinery: parallel personas with optional
  research / reflect / judge / samples / verify / cite / hedge stages
  (`ANGEL_MOA_*`). MoA is an engaged formation or selected swarm route, not the
  default meaning of an unset `ANGEL_DRIVER`.
- `src/deli.rs` — the `deli` driver (`ANGEL_DRIVER=deli`): a single-session run of
  the Deli_AutoResearch loop, thinking **deep over time** — bounded fresh-context
  iterations that accumulate findings, with stall detection, forced structural
  pivots, and direction diversity (`ANGEL_DELI_ROUNDS/_PIVOT/_STALL_STOP/`
  `_MIN_FINDINGS/_STATE_DIR`).
- `src/harness/` — the tool-loop (`run_turn`) + `ToolRegistry` + the tools.
  On startup it auto-injects the workspace's **AGENTS.md** (git-root→workspace,
  the Codex convention) into the system preamble so repo conventions ride along
  (`ANGEL_PROJECT_DOC=0` disables).
- `src/code_mode.rs` — the `code_mode` tool's engine (**on by default**;
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
- `src/reinforce.rs` — the self-reinforce loop + reward types.
- `src/{sandbox,pty,viewer,chart}.rs` — landlock sandbox, full-access PTY pane,
  portrait image protocol support (`ANGEL_IMAGE_PROTOCOL=kitty|sixel|iterm2`
  when explicitly selected), and bounded/off-thread terminal-native caches.
- `src/scryglass.rs` — the persistent Braille stage for the agent-driven
  live first-person world, brief Arrival establishing plates, eight-second still
  reveals, and silent local MP4 playback. F4 focuses it; drag or arrows look,
  the wheel or `+`/`-` changes the lens, right-click or `0` returns to
  agent-follow, and `m` toggles the map. `/learn [topic]` opens the local-first
  Scriptorium tutor and selectable Catalog; optional reference enrichment never
  blocks its objective, exercise, checkpoint, or editable Ask Tutor draft.
  Delivered visuals queue behind the active reveal; manual `/show` and `/open N`
  reveals pin until dismissed.
- `src/world_viz/life.rs` — the living realm: a slow weather drift, golden-hour
  and moon-blue grading, drifting cloud shadows, shoreline foam, starlight on
  the night sea, meadow wind gusts, and a prosperity ladder that physically
  grows the town (dock → windmill → market → tall keep, with construction
  sites previewing the next build). Arrival plates can be **forged-3D**
  (Hunyuan3D meshes baked to noir plates by `scripts/world-forge/` — see
  `assets/world-forge/README.md`), falling back to the woodcut set.
- `src/observatory.rs` — the native read-only campaign/report gallery inside
  the ordinary Artifacts pane. `/observatory` loads the report house;
  `/observatory campaign <id>` and `/observatory open <report-id>` retain exact
  command access. With an empty composer, arrows traverse the windowed campaign
  rail and report ledger and Enter opens the selected report through the normal
  media path.
- `src/ui_inspect.rs` — the ordinary cockpit's visual acceptance broker.
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
| Processes                  | `proc_run` (launch a daemon — inference server, dev server, long build — detached, log to `~/.angel0/proc`), `proc_status` (state + log tail), `proc_stop` (SIGTERM→SIGKILL the whole tree)                                                                                                                     |
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

¹ **`shell` sandbox posture (honest):** the Landlock ruleset confines _writes_ to
the active workspace plus explicitly enumerated scratch/runtime/cache,
per-user-install, worktree-Git, process-log, and accelerator-device roots needed
by local tools. It does **not** grant all of `$HOME` or the launch cwd. It leaves
the **whole filesystem readable**, the **network on** for the ordinary write
posture, and the child **inherits the parent environment** — including
`ANGEL_BRAIN_KEY` unless the separate secret-stripping control is armed. It
guards against accidental damage, **not** an adversarial model that wants to
exfiltrate or read secrets. It is on by default; `ANGEL_SANDBOX=0` is an explicit
unconfined override. The exact writable-root contract is in
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
  `go test ./...`, `swift test` — resolves the largest of several nested crates (angel0's `cockpit/`), and
  takes `{dir}` / `{crate}` to point at a subtree (`sidecar/forge` runs its python suite).
- `reinforce.rs` reward impls: `TestReward`, `LintReward`, `CompositeReward`
  (+ `code_health()` = 80% tests + 20% lint).
- `run_coding_eval(club, registry, task, verify)` drives a task through
  `run_turn` then scores it with a verifiable reward — the end-to-end loop.
- Set `ANGEL_TRAJECTORY_LOG=1` to persist each rollout as **reward-labeled
  JSONL** under `~/.angel0/trajectories` (training data).

## Run

```sh
ANGEL_BRAIN_KEY=<turbo key> cargo run        # authenticate fleet routes that require it
cargo run                                    # practice floor, upgraded when a live route is reachable
../bin/angel0 --yolo                         # unrestricted operator profile
```

`bin/angel0` builds and serves the cockpit and bundled world assets from the
checkout that contains the launcher, so switching branches changes the next
normal launch. Run `angel0 update` after moving or replacing the checkout to
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

- `/graph list` shows bundled and `~/.angel0/graphs` TOML specs; `/graph run
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
`ANGEL_SESSION_DIR` (default `~/.angel0/sessions`). **Full reference:
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
