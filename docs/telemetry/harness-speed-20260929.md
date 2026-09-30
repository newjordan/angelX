# Harness speed — lab notebook (2026-09-29)

Branch `claude/dreamy-carson-7d17h8`. Operator's brief: make the harness faster
and more efficient; remove no system, connect systems where they should be
connected, and add nothing that blocks or governs work.

## Goal

Cut the harness's own time per hop (the time between the model's reply and
the next request) without changing what any system records or decides.

## Metrics

| id | metric | better |
|---|---|---|
| S1 | wall time of a scripted session (stub model, release build) | lower |
| S2 | median harness time per hop, by tool | lower |
| S3 | growth of S2 across the session (first vs last quarter) | flatter |
| S4 | processes spawned per hop (`strace`) | fewer |
| S5 | behaviour: the same tests pass, the same records are written | must hold |

## Method

The scripted stub model from the book benchmarks (`bench/stub_model.py`,
`bench/run_one.sh`) answers instantly, so the gap between two requests is the
harness's own time: running the tool plus everything around it. The long
session: 70 red cycles (read, red test run, swap one wrong operator for
another), then the fix, a green run and the answer, 213 requests. Release build,
three runs each; `strace -f` for process counts, `gdb` breakpoints for callers.

## Baseline (`3295966`, release)

213 requests in 21.5 s (21.6, 22.1, 21.9). Per hop, median:

| tool | first quarter | last quarter | median |
|---|---|---|---|
| `read_file` | 7.8 ms | 22.6 ms | 16 ms |
| `str_replace` | 50.7 ms | 63.8 ms | 60 ms |
| `run_tests` | 212 ms | 225 ms | 223 ms |

Reported by the harness itself: tool time 19.4 s, of which 4.9 s is overhead
outside the tools' own work; 1.3 s residual between hops; 0.7 s model I/O.

`strace` (debug, 20 cycles, 63 requests): 534 `git` processes, 508 of them
workspace fingerprints (`status`, `ls-files --stage`, `ls-files`,
`ls-files --others`, run in parallel). By hop: `read_file` 1 fingerprint,
`str_replace` 2, `run_tests` 3. Every sandboxed command (the post-edit syntax
check, the test run) also runs `bwrap … /usr/bin/true` first.

## Log

### 1. Fingerprints only where a fact needs one (`d5988ad`)

A `gdb` breakpoint on `workspace_fingerprint` named the callers: after every
successful call the turn loop fingerprinted the workspace for a green-run record
it keeps only for verifier calls, and a test run fingerprinted the same
unchanged tree up to three times (red run, verifier attempt, untested edit).
One lazily taken fingerprint per tool result now serves all of them.

`strace`: fingerprints per hop `read_file` 1 → 0, `str_replace` 2 → 1,
`run_tests` 3 → 1 (24 → 8 git processes per cycle). Harness time (the session
minus tool execution) 7.25–7.63 s → 6.46–6.58 s over three runs each.

### 2. The Python verifier recompiled the standard library every run (`75927ff`)

A traced test run: 250 ms in `python3.11`, of which the test itself is
nothing. The verifier pointed `pycache_prefix` at an absent directory so a stale
or planted project `.pyc` can never stand in for its source; that also threw
away the standard library's install-time bytecode, so unittest and its imports
were compiled from source on every run. Direct timing: 193 ms per run; with a
standard-library-only cache 44 ms.

The bootstrap now installs a path hook: `sys.path` entries present before it
inserts anything (the interpreter's own library) keep their bytecode; every
other path loads source through a loader that never reads a `.pyc`. Prototype
against a planted, still-valid stale `.pyc`: plain Python passes (the threat),
the old verifier fails correctly in 180 ms, the hook fails correctly in 39 ms.
The existing stale-bytecode test passes unchanged.

Session: `run_tests` 219 → 64 ms per call; wall 22.8 s → 11.3 s.

### 3. One namespace probe per run, not per sandboxed command (`5ae2b6c`)

Each sandboxed command runs a fresh `angel-sandbox`, and each repeated the
Bubblewrap probe (`bwrap … /usr/bin/true`), polled on a 10 ms tick. The parent
already probes once; a clean verdict now travels in the helper's private
policy env (a stale "clean" can only make Bubblewrap fail closed). Probes per
run: one per sandboxed command → 1. Wall 11.1–11.6 s → 9.95–10.3 s.

Method note: the harness runs the `angel-sandbox` beside its own binary, not
`ANGEL_T_SANDBOX_HELPER`; my first measurement of this change ran the old
helper and showed nothing. Each build now runs from its own directory.

### 4. The timing receipt was deep-copied five times a hop (history-proportional)

`ANGEL_TURN_PHASE_TRACE=1` plus nine temporary marks bisected the growth to
`timing.finish_with_history`, run after every model reply and every tool
batch so a record written at any exit carries the hop. The receipt holds every
model and provider call sample (332 KB of JSON by hop 213), and each refresh
copied it about five times (the `json!` rebuild, the provider clone, the
retained copy, `to_value` into the ledger). The samples now sit behind an
`Arc`; the ledger keeps the typed receipt and serializes it only when a record
is written (a test pins the written row to the receipt's own JSON).

Debug build, first → last quarter: reply handling 6.6 → 29.3 ms becomes
1.3 → 9.1 ms; tool dispatch → results 22 → 43 ms becomes 9 → 19 ms. Whole debug
session 32.9 s → 17.2 s.

Release (`8937e07`): harness time 5.5–5.6 s → 3.9–4.1 s; `read_file`
post-tool time first → last quarter 2.2 → 11.2 ms becomes 1.2 → 5.4 ms; wall
9.95–10.3 s → 8.1–8.6 s. What still grows is the two sample vectors cloned into
each refresh and `background::observe` walking the tool results: about 4 ms
by hop 213.

### 5. The syntax check imported `pathlib` to read one file (`eb2d91d`)

The post-edit check and the verifier's syntax plan ran
`import pathlib, sys; compile(pathlib.Path(p).read_bytes(), …)`. The import is
13 ms of a 22 ms check; `open(p, 'rb').read()` reads the same bytes. Direct:
22.0 → 9.3 ms per edited file.

### Startup (examined, left)

123–133 ms to the first request in release, 85–95 ms of it the tool registry,
and 95 of the registry's 105 ms (debug) in `capture_verifier_runtimes`: it
SHA-256s every verifier executable before any tool is built, by design (the
pin must predate any model-owned dispatch). Node's 118 MB binary is the
critical path (~80 ms), even where no Node code exists; the cargo toolchain
(~85 MB over seven binaries) runs beside it. The digest could be taken on a
background thread during the first model request and joined before the first
dispatch; that moves when a security pin is established, for ~80 ms once per
task, so it is left for the operator.

Release (`eb2d91d`): `str_replace` post-tool time 32.4 → 19.9 ms; harness time
3.9–4.1 s → 3.12–3.14 s; wall 8.1–8.6 s → 7.43–7.49 s.

### Where the session stands

213-hop scripted session, release, three runs each (the tool's own time is the
test run and the edit; harness time is everything else):

| build | wall | harness | tools |
|---|---|---|---|
| baseline `3295966` | 21.5–22.1 s | 7.25–7.63 s | 15.0–16.3 s |
| + one fingerprint per result | 22.7–23.1 s | 6.46–6.58 s | 16.0–16.3 s |
| + source-only verifier imports | 11.1–11.6 s | 5.99–6.32 s | 4.90–5.10 s |
| + one namespace probe per run | 9.95–10.3 s | 5.47–5.63 s | 4.24–4.46 s |
| + shared timing samples | 8.1–8.6 s | 3.88–4.10 s | 4.00–4.32 s |
| + `open` for the syntax check | 7.43–7.49 s | 3.12–3.14 s | 4.11–4.16 s |

2.9× faster end to end; the harness's own time down 58%. The prompt stays
cache-stable: in 213 requests no earlier message and no tool schema changed
between one request and the next.

What is left per hop is mostly the tools themselves: a Python test run is
~40 ms, 25 ms of it `import unittest` (inherent to the interpreter), and the
edit's syntax check ~9 ms plus the sandbox. The history-proportional remainder
is 3–5 ms by hop 213 (the two sample vectors cloned into each refresh and the
walk over tool results in `background::observe`).

## Connections

An inventory (a search agent's 15 candidates, each checked here against the
code before acting) of systems built but not wired to their consumers.

Connected:

- **The continual harness reaches the next turn** (`2697f23`). An RL policy
  note or refinement was read into the start-up context and each /loop prompt
  as built, and nowhere else; an interactive session saw an entry `/rl`
  installed only after `/clear`, `/cd` or a restart. The per-turn knowledge
  broker now offers the block beside the dossier and the caddy card, and skips
  it when the conversation already carries it. Test fails without the change.
- **Interactive and /loop turns write the caddy** (`7354297`). The recipes and
  hazards card is read into every turn but only headless `--task` runs wrote to
  it — the gap the caddy exists for (a loop ran 46 iterations without its own
  verifier recipe). The turn worker now folds each finished turn in.
- **Escalations are visible** (`ade8267`). The loop's verifier-blocked
  escalations were persisted in `loop.json` and shown nowhere; `/loop status`
  and `progress.json` now carry them.
- **An unknown `ANGEL_COMP_PACKAGE` is named** (`40ab4ff`) at loop start
  instead of falling back silently.
- **Workspace identity probes run together** (`10f19ff`), taken at start-up and
  twice at shutdown (trajectory record, caddy record).

Left for the operator (each changes what a system decides, not only whether it
is heard):

- **`ANGEL_RL_REWARD` selects nothing.** `ENV.md` documents it as the RL scorer
  switch; every scorer is `TestReward` by design ("the default RLVR
  contract"), and the trajectory stamps the raw variable as `rl_reward`, so a
  row can say `lint` while `TestReward` scored it. Either honour the variable
  (changes every RL reward) or fix the docs and the stamp.
- **The Sloptomizer learns only from `loop_research`.** Loop acceptance
  verdicts and `rl_campaign` attempts carry physical verifier receipts but are
  never observed. They are unpaired and their "idea" is free text, so feeding
  them in would change what `suggest` ranks; it needs a design call (a flag on
  unpaired observations, what counts as the idea).
- **The loop's findings end with the loop**, and interactive/loop turns drop
  the harness's own tool ledger (the loop rebuilds it from the UI strip). Both
  are sound connections, larger than this pass.
- **Startup pins** (above): hash Node and the cargo toolchain in the
  background and join before the first dispatch, ~80 ms per task.

### 6. The pinned Rust toolchain is re-hashed in parallel (`74c4598`)

Before every cargo-based verifier call the seven pinned images (~85 MB) were
re-hashed one after another; each now runs on its own thread, the first
failure in toolchain order is the one reported, and the aggregate re-land pass
is unchanged. sha256 of the seven images: ~100 ms sequential, 51–76 ms
parallel (under load). The replacement tests (`pinned_cargo_rejects_replacement
_before_first_dispatch`, the replaced rustup proxy, the drifted controls) pass.

Concurrent workspace identity probes (`10f19ff`), ten interleaved short runs
each under load: shutdown 26 → 22.5 ms median, start-up identity 8 → 6 ms.

Full suite on `eb30821`: 4,937 passed, 85 ignored, 3 failed — the grok-auth
schema test and the two Bubblewrap tests this container cannot run (no user
namespace uid map), as on every run before.

## The three operator calls (approved: "Sloptimizer and RL looping should be the same thing")

### `ANGEL_RL_REWARD` stamps the scorer that ran (`03517d2`)

Every live evaluator (loop acceptance, coding eval, recovery eval, decision
replay) captures test-contract evidence; nothing produces lint or code-health
evidence. Honouring the switch as documented would have been harmful: a `lint`
scorer on test evidence fails closed (every loop verification "REJECTED
EVIDENCE"), and relabelling the evidence would let `parse_lint` credit a clean
lint on test output it never ran. So the switch stays inert and says so: rows
stamp `rl_reward: tests` (the live scorer) and keep a differing request as
`rl_reward_requested`; ENV.md describes what the variable does. Honouring it
for real needs an evaluator that runs the linter under the code-health
contract.

### The Sloptomizer and the RL loop are one learner (`53871b8`, `d6d4895`)

One learning scope per objective (task, verifier, route) now receives:

- `loop_research` runs (as before);
- **every verified /loop iteration**: the DIRECTION it named, the acceptance
  verdict, and the evidence's manifest and workspace digests, as an unpaired
  observation (a replayed unchanged-red receipt is not a new measurement);
- **every measured `rl_campaign` round**: the reflector's proposal, promoted or
  not, and the measured mean delta, as a paired observation.

`suggest` therefore ranks with the loop's and the campaign's real history and
stops reading cold after the first verified iteration. Observations run off the
UI thread under the store's file lock; `loop_research status` reports how many
loop verdicts were admitted and the last error. Tests: a loop verdict makes
`suggest` warm; the end-to-end campaign's round is observed; the loop records
each iteration's direction, repeats included.

### Executable pins off the start-up path (`16e48a7`)

Capture resolves, stats and opens each verifier executable as before and hashes
the open file on a background thread. Every tool dispatch first waits for all
pending digests, and every digest reader waits for its own, so no model-owned
action precedes the pin it is judged by. The open file's identity is checked
against the captured identity at open and after the read: a timestamped write,
an unlink or a rename-over while the pin is read fails closed (the rename test
exercises it). The window a coalesced same-length in-place write could slip
through is the read itself, as with the synchronous hash.

Release (`16e48a7`), ten interleaved short runs each: time to the first
request 123.5 → 75.5 ms median, registry construction 86.5 → 21 ms. The
scripted 7-hop task gains less (305.5 → 286.5 ms) because the stub answers at
once and the first dispatch waits out the remaining hash; behind a real model,
whose first reply takes far longer than the hash, the whole saving lands.

## Local replication (2026-09-29, atlas)

Host: AMD Ryzen 9 9950X3D (16C/32T), 91 GiB, Linux 7.2.5, a normal host with
user namespaces. `DEV` = `158b7a8` (`h/`) against the baseline `3295966`
(`b-base/`). Release builds, each run beside its own `angel-sandbox`.

### The 213-hop stub session, three runs per build, interleaved

| build | wall | harness | tools |
|---|---|---|---|
| baseline `3295966` | 13.49–13.70 s | 3.41–3.49 s | 9.90–10.16 s |
| `DEV` `158b7a8` | 3.46–3.50 s | 1.26–1.28 s | 2.11–2.12 s |

- **3.9× faster end to end** (the cloud measured 2.9×). **Harness time is
  down 63%** (cloud: 58%). The ratios hold and grow on this host.
- **Growth per hop** (`phases.py`, first quarter → last quarter):
  - head: `read_file` post-tool 0.8 → 2.6 ms, reply 0.5 → 1.9 ms;
  - baseline: `read_file` post-tool 2.1 → 5.3 ms, reply 1.1 → 4.6 ms.
- **Time to first request:** ten interleaved one-cycle runs per build, median
  `timing.startup_ms`: **46 ms (45–49) → 25 ms (23–25)**. The cloud measured
  123.5 → 75.5 ms.
- **E6, the verifier runtimes present:** Node 26.8.1 (a 150 MB binary, the
  largest pin), the cargo/rustup toolchain, and Python 3.14.
- **The deferred-pin replacement check** (the optional `strace` step) was not
  run; the unit test covers it.
- **Full suite on `DEV`, serial, release:** 5,019 passed, 0 failed, 89
  ignored. The three failures the cloud container saw (grok auth and the two
  Bubblewrap user-namespace tests) pass on this host.

### Behind a real model (E5)

E1's intro arm on the two-bug task, for `b-base/` and `h/`, n = 5 per build
per model, with build order shuffled within each round. The timing block of
each task receipt gives the split below: median, with the range in brackets,
in ms. "Harness %" counts tool overhead, residual, other time and
startup/shutdown as a share of wall time; the figure in brackets excludes
startup.

| model | build | wall | model | tool | tool overhead | residual | startup | harness % |
|---|---|---|---|---|---|---|---|---|
| GLM-5.3 flash | base | 22,948 (19,896–58,148) | 21,649 | 222 | 34 | 10 | 1,103 (1,091–1,376) | 5.0% (0.25%) |
| | head | 21,942 (20,736–25,079) | 20,589 | 53 | 14 | 9 | 1,077 (1,072–1,290) | 5.3% (0.15%) |
| DeepSeek flash | base | 6,491 (6,015–8,601) | 4,974 | 194 | 37 | 7 | 1,198 (1,176–1,484) | 19.6% (0.75%) |
| | head | 7,233 (6,025–7,526) | 5,933 | 81 | 11 | 9 | 1,184 (1,165–1,231) | 17.1% (0.40%) |
| Muse Spark 1.3 contributor | base | 29,450 (20,987–52,977) | 29,211 | 237 | 69 | 11 | 46 (45–57) | 0.5% (0.31%) |
| | head | 23,761 (21,402–35,275) | 23,639 | 110 | 25 | 15 | 23 (23–24) | 0.3% (0.18%) |

- **The residual per hop is far under 10% of model time on every model:**
  0.15–0.75% of wall, excluding startup. Tool time falls from 194–237 ms to
  53–110 ms (the verifier changes).
- **The startup gain lands fully only on Muse** (46 → 23 ms).
- **On the Chat Completions seats, startup is about 1.1–1.2 s on both builds.**
  That is 17–20% of a DeepSeek flash task and 5% of a GLM flash task. It is
  not the executable pins: a proxy log of the GET requests shows three serial
  metadata probes before the first request:
  - `/props` → 404, 456 ms;
  - `/v1/props` → 404, 378 ms;
  - `/v1/models` → 200, 393 ms.

  `/props` is llama.cpp's; a cloud provider can only 404 it. The stub never
  showed this cost, because it answers at once.

**Decision (what E5 was registered to decide).**

- **More per-hop speed work is not worth doing.** The harness's own time
  between hops is under 1% of a real session. X1, the 3–5 ms
  history-proportional remainder at hop 213, is deferred on this evidence.
- **The startup probes are the lever:** skipping the two `/props` round trips
  saves about 0.8 s per task on a cloud seat. That is X6 (`ff1b4f9` on
  `eng/handoff-20260929`), a new item: a private host with no provider
  contract probes as before.

### X6 measured: the `/props` probes skipped on a cloud seat (`ff1b4f9`)

DeepSeek flash, two-bug task, intro on, n = 5 per build, build order
shuffled within each round (`out/x6-*`). All ten runs solved.

| build | startup ms | GET probes | wall ms |
|---|---|---|---|
| `DEV` `158b7a8` | 1,169 (1,151–1,336) | 3 | 6,862 |
| `0e6857f` (X6) | 403 (399–429) | 1 | 5,884 |

- **Startup is down 766 ms and task wall down 14%.**
- **What remains is `/v1/models` (~400 ms), and it is not dead weight.**
  DeepSeek's catalog carries `context_window` (1,048,576), `max_output_tokens`
  and its effort levels. `catalog_capabilities` reads none of those keys, so
  the static map's 1,000,000 stands.
- **Left for the operator.** Either read the catalog's own keys (a
  connection that changes the window by 5%), or take the probe off the path
  to the first request. Both change what the seat decides, not only when it
  learns it.

### E5 for Grok (sparky)

grok-4.7 at `low`, base against head, n = 5 each, all solved:

| build | wall ms | model ms | tool ms | harness ms | startup ms |
|---|---|---|---|---|---|
| base `3295966` | 8,543 | 7,721 | 252 | 117 | 512 (494–543) |
| head `158b7a8` | 8,480 | 7,908 | 80 | 50 | 461 (433–473) |

- **Harness time behind a real model:** 0.6% of wall on head. Tool time drops
  about 3× (the verifier changes).
- **Startup:** about half a second on both builds. The `grok-api` seat behind
  the local logging proxy counts as a private host, so it still probes
  `/props`. On xAI's own host, X6 skips those probes.
- **The chart report** (every chart and table, regenerated from `out/`):
  https://claude.ai/artifact/S1RkLVpyrTvFgpYeB2YPws
