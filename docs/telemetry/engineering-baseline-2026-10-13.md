# Engineering baseline — 2026-10-13 (in progress)

Frozen identity, measured costs, and the next-change protocol for the
coding/RL loop. Game/world work is fenced off and untouched by this effort.

## Frozen identity (verified)

| Item | Value |
|---|---|
| Branch / HEAD | `glm_map_fixes` / `1c82660503f212ca001c2547135f3c45bd761b6e` |
| Source digest (angel-source-v2) | `9dd74be57582e6230657c3a69ac507e7011a2082c33191bcd6462779ca367d5f` (two routes agree: `--commit HEAD`, clean worktree) |
| Dirty files at freeze | 10, all game/world (`together_shooter`, `world_viz/overworld/arena`, chariot tests/docs); zero engineering |
| Shipped `cockpit/target/release/angel` | **unbound** (`cockpit_source_sha256: "unbound"`, exe sha256 `7ab88629…`) — bench runner correctly refuses it |
| Baseline binary | native `--locked --release --no-default-features` in clean worktree `/tmp/angelx-baseline-1c826605`, built in 9m24s; independently verified `--build-info` source `9dd74be5…`, exe sha256 `9bfb845f…`, sandbox sha256 `b74b98f4…`; preserved under `artifacts/speed-loop/baseline-2026-10-13/bin/` (resources sha remains `unbound`; runner hashes copied resources independently) |
| Bench gate | config-only pinned cell `cell-config-pinned` exits 0: 12 tasks, catalog/grading/selection hashed, evaluator snapshot verified |
| Pilot r1 | **invalid** — all 12 attempts infra-failed pre-agent (`Errno 30` creating `~/.cache/verifiers/...` under the agent sandbox) while the runner still exited 0; preserved unedited at `cell-pilot-luna-medium`, never to be scored. Rerun in operator environment as `cell-pilot-luna-medium-r2` |

Manifest: `artifacts/speed-loop/baseline-2026-10-13/freeze-manifest.txt`.

## Cost structure (12 historical `angel.usage.v2` envelopes, mixed cohorts)

Measured with `/tmp/angelx_prefix_audit.py` over
`angelx-ds41flash-nothink.jsonl` + `angelx-main-full.jsonl`. Per attempt
(median): wall 19.6 s (p90 28.0), model 18.2 s, tool 0.3 s, startup 1.1 s,
9 model calls, 137k cumulative input tokens, 2.1k output, cache-hit 87%
(p90 98%). Model time is 93% of wall; harness tool time 1.4%. Startup is
4.7% of wall, and inside startup `pre_request_ms` (1062 ms in the sampled
envelope) dwarfs identity prewarm (2 ms), registry (72 ms), and workspace
recon (11 ms) combined. Caveat: rust30 traces use an older trace schema
(no `runtime.envelope.usage`); schema paths must be verified per cohort,
never assumed.

## Benchmark position (from README + full-136 audit)

Polyglot-136 is saturated (134–136/136 for every serious model) and stays
as regression smoke only. Angel's edge is token efficiency: 135/136 in
6.9M input tokens vs OpenCode 67.7M at 132/136 on DeepSeek V4.1 Flash.
No across-the-board wall win is established (full-136 audit). Steering
requires headroom: Terminal-Bench 2 contract and promotion protocol in
`artifacts/speed-loop/baseline-2026-10-13/hard-cohort-contract.md`
(assets already local: `hub-terminal-bench-2`, verifiers 0.3.1).

## Strength ladder

Spec only: `artifacts/speed-loop/baseline-2026-10-13/strength-ladder-spec.md`.
Append-only verified-attempt rows, Wilson-LCB ranking with cost tie-break,
UCB1 seat picker reusing Sloptomizer's bandit, user-specified routes always
win. Bootstrap prior is the existing static
`docs/telemetry/model-calibration.toml` (2026-09-09).

## Pilot r2 (verified baseline)

Luna-direct, `gpt-6-luna`, effort medium, one attempt/task, 600 s cap,
concurrency 1. Canonical summary
(`cell-pilot-luna-medium-r2/summary.json`) and an independent exact-path
extractor agree on calls/tokens; wall agrees within envelope-vs-task
accounting (330.7 vs 331.8 s).

- Solved: **12/12**, all trace rewards = 1.0, cohort complete.
- Task wall: sum 331.8 s, median 24.1 s, nearest-rank p90 42.4 s, max 60.4 s.
- Model/tool/startup: 296.5 s / 32.6 s / 1.23 s. Startup median 0.106 s.
- Calls: 62 model calls (median 4.5/task), 65 tool calls.
- Usage (12/12 complete): input 432,698; cached 238,592 (55.1%); uncached
  194,106; output 11,332; reasoning 587.
- Per solve: 36,058 input, 16,176 uncached, 944 output tokens.
- Evaluator wall including verification/run overhead: 369.6 s.
- Tail: `rust-acronym` 60.4 s includes one provider call with 34.956 s to
  first token. First-token time totals 182.5 s, 61.6% of model time, so a
  single pilot is a baseline snapshot, not a stable latency claim.

## Prefix audit (r2 wire usage, no prompt bodies)

First model call per task: median input **4,256 tokens** (range
3,752-6,402), cached input **0 on all 12**. Later calls are 63.3% cached.
The bounded-task prefix already fits the essential schema profile; no
first-request stub probe is needed for this route because provider usage is
complete on every call. The uncached first calls contribute 55,845 tokens,
28.8% of all uncached input. Cross-process cache reuse therefore matters,
but first-call size is already lean.

Terminal narration calls (last call per task, zero tool calls, 172-386 reply
chars) cost 41.3 s (13.9% of model time), 98,239 input (31,167 uncached),
887 output. This replaces startup as the measured target: startup was
falsified as a candidate on this route (0.106 s median, not the historical
mixed-cohort 1.1 s, whose `pre_request` span likely covered route-specific
work absent here).

## Next measured change candidate (falsifiable)

**Green-terminal for bounded tasks.** Angel already synthesizes a terminal
accepted answer when an explicit task acceptance command passes
(`turn/mod.rs`, `proof.passed` -> `ACCEPTED`, no further model call), and it
already tracks `green_verify_achieved` for model-run verifications. Candidate:
behind a bounded-task flag, when the final tool batch is a successful
verification call, no mutation follows it, and no error/advisory is
outstanding, return the same deterministic accepted answer instead of paying
one narration call. Predicted effect from r2: -12 model calls (-19%), up to
-41 s model time and -31k uncached tokens per 12-task cell, with identical
workspaces. Falsifier: interleaved A/B + B/A on the same pinned pilot plus a
negative set (partial test commands, red-then-green, mutation-after-green,
acceptance-command tasks); reject on any solve loss, any premature stop on
the negative set, or median wall saving < 1.5 s/task. Interactive turns are
out of scope; a user-facing answer still needs the model.

## Test triage at frozen HEAD (verified)

- Canonical gate `check-cockpit-fast.sh` stops **before tests** at clean
  HEAD: `check-active-connections.py` reports two Rust files without an
  anchor — `cockpit/src/drive/together_shooter/bowcountry.rs` and
  `cockpit/src/drive/together_shooter/pastimes/longbow.rs` (committed game
  files; fenced from this effort, recorded not fixed). Log:
  `test-triage-full.log`.
- Underlying suite, clean worktree, debug `--no-default-features`,
  `--test-threads=8`, helper pinned: **6279 passed; 0 failed; 231 ignored**
  in 540.87 s (`test-triage-cargo.log`). The handoff's 49
  agent/harness/labyrinth/sandbox failures do **not reproduce** at frozen
  HEAD under this gate; that run was on a dirty tree (~70 uncommitted files)
  with different feature/runtime conditions, and no log was retained, so the
  49 should be treated as an unverified dirty-tree observation, not a HEAD
  defect list. Re-derive before citing.

## Hard steering cohort (stood up to dry-run; execution blocked)

Selection frozen in `tb2-selection.json`: smoke `fix-git`, 20 steering tasks
(stratified build/code/sysadmin/data/git), 5 never-tune holdouts, from the
official 89-task TB2 registry. The dedicated env in `tb2-project/`
(uv-managed `.venv` + lock) repairs the hub lock root cause — harbor's
unbounded `supabase>=2.28.2` plus `prerelease=allow` pulled supabase
3.0.0a1, whose postgrest dropped `base_request_builder`; pinned
`supabase<3` resolves supabase/postgrest 2.32.0 and the taskset loads
**89 tasks, frozen selection 26/26**. The Angel Harbor adapter
(`tb2-project/angel_harbor_agent.py`) uploads the pinned binary pair with
fail-closed sha256 + in-box `--build-info` source checks and passes its
fake-environment test; `harbor run --print-config` resolves the fix-git job.
The glibc-floor pair is now built and verified from the same frozen source
(cargo-zigbuild `gnu.2.31`, max `GLIBC_2.30`, exe `12eb2cea…`, helper
`1cc27c23…`, in `bin-floor/`), and a Verifiers-side `AngelTb2Harness`
(snapshotted `angel_action_v1` + Harbor-task adaptation) resolves in a Prime
dry-run with the floor pins. All three execution routes were then attempted
and are terminally blocked in this agent session: Docker (`harbor run` exit
1, daemon unreachable in-sandbox; unsandboxed approval disabled), Prime
(`~/.prime` api_key empty — login never completed; eval hung and was
terminated), and subprocess (refused by design for image tasks, EnvError in
traces). Resolved the same day via the local fleet instead of Prime (evaluators are
MIT/open-source; only Prime's hosted runtime is paid, and it is not used):
Apollo (x86_64, Docker 29.1.3) ran the `fix-git` smoke with the floor
binary — **reward 1.0, 1/1 trials, 2m10s job wall** (job
`smoke-jobs/2026-10-13__10-52-18` on Apollo; Harbor token/cost fields are
null, so TB token accounting must come from Angel envelopes). Turbo
(x86_64, 32 cores, 67 GB) is staged identically with in-container hash and
`--build-info` verification. The aarch64 rigs and arm64 Mac are excluded:
TB images are amd64 (fix-git: arch amd64, glibc 2.36) and the floor binary
is x86_64. Steering-20 then ran sharded 10/10 across Apollo (`-n 2`) and Turbo
(`-n 4`), one attempt per task, two model cells. **Kimi K3 medium: 11/20**
(Apollo 5/10 in 23m43s, Turbo 6/10 in 31m25s). **Grok 4.7 low: 13/20**
(Apollo 7/10 in 24m41s, Turbo 6/10 in 15m29s; fix-git smoke 49s vs Kimi
2m10s). Paired flips are one-directional: Grok alone passes
`cancel-async-tasks` and `pypi-server`; Kimi alone passes nothing. Seven
tasks fail under both models (`configure-git-webserver`, `kv-store-grpc`,
`git-multibranch`, `nginx-request-logging`, `polyglot-rust-c`,
`query-optimize`, `regex-log`) — that fail-both set, three of which surface
as agent timeouts against the adapter's 900s cap, is the harness fault line
to inspect before further model tuning. Grok 4.7 low is adopted as the
steering standard. Full per-task evidence is in the selection file; Harbor
does not capture Angel token usage, so these cells report reward and wall
only — the adapter should download the Angel envelope before container
teardown in a follow-up.

## GrokBuild control on the fail-both set (2026-10-13, harvested post-crash)

At the operator's request ("test the grok harness on the same suite"), the
stock Harbor `GrokBuild` agent (xAI grok CLI, `-m grok-4.7`,
`reasoning_effort=low`, k=1) ran the 7 tasks both Angel models failed. A
thin subclass (`tb2-project/grokbuild_oauth_agent.py`) seeds the host OAuth
`auth.json` into the task box because the grok CLI rejects an OAuth access
token as `XAI_API_KEY` ("Not signed in" — first smoke job
`grokbuild-control-jobs/2026-10-13__12-26-54`, errored, not scored). A local
host crash/reboot interrupted the session, but both rig shards completed
server-side and were harvested read-only afterward; no rerun was needed and
no new load was placed on the rigs. Jobs: Apollo
`grokbuild-control-jobs/2026-10-13__12-29-23` (kv-store smoke, 11m01s) and
`__12-40-55` (2 tasks, 3m10s), Turbo `__12-44-21` (4 tasks, 10m51s), zero
errored trials in the scored jobs.

| Task | Angel (Grok 4.7 low) | GrokBuild (grok-4.7 low) |
|---|---|---|
| `regex-log` | 0 — 900s timeout, `/app/regex.txt` never created | **1** |
| `git-multibranch` | 0 — finished, deploy verify fails | **1** |
| `nginx-request-logging` | 0 — 3/8, nginx not running at verify | **1** — 8/8 |
| `configure-git-webserver` | 0 — 0/1, hello page missing | 0 — identical single failure |
| `kv-store-grpc` | 0 — 5/7, server not running | 0 — 5/7, identical 2 failures |
| `polyglot-rust-c` | 0 — 900s timeout, nothing created | 0 — finished; polyglot built but extra `main.rs` fails exact file-set check |
| `query-optimize` | 0 — 4/6, correct but db-modified + slower than golden | 0 — 4/6, identical failures |

Reading: **GrokBuild 3/7 vs Angel 0/7** on Angel's worst subset, so part of
the gap is harness, not model or task. The sharpest evidence is the two
Angel 900s timeouts that produced no output file at all (`regex-log`,
`polyglot-rust-c`) where GrokBuild finished both (one solved, one near
miss): Angel's long-task loop lacks time-budget/progress discipline —
land an output artifact early, then improve it. `nginx-request-logging`
adds a server-lifecycle gap (work left not running for the verifier),
which GrokBuild avoided there but not on `kv-store-grpc`. The remaining
three (`configure-git-webserver`, `kv-store-grpc`, `query-optimize`) fail
with byte-identical verifier signatures under both harnesses and are not
Angel-specific. Caveats: k=1 per cell (single-attempt noise is possible),
and the two cells reach the same named model by different routes (Angel
grok-api club vs grok CLI OAuth). Retained Angel trial artifacts are
nearly empty (34-byte `trial.log`, empty `agent/`), so *what* the Angel
loop did for 900s is unobservable until the adapter downloads the Angel
envelope before container teardown — that follow-up is now the critical
path for timeout diagnosis. Rig notes from the same harvest: Apollo
healthy (31d uptime, load 0.2; pre-existing unrelated containers left
untouched); Turbo healthy but root disk at 85% (134G free) — no cleanup
performed.

## Sentinel v1 (built 2026-10-13, uncommitted)

Operator-approved response to the control result, built quiet-first per the
operator's direction (no noisy interventions; judgment stays with local
overwatch lanes): a deterministic, model-free sentinel for deadline-carrying
turns (`cockpit/src/agent/harness/sentinel.rs`) rendering latched verdicts
from budget fraction × mutation × green × named artifacts — Drift at 50%
before the first edit, Deliver at 85% with nothing landed. `ANGEL_SENTINEL`
defaults to `observe` (operator notices only) when a deadline exists and
off otherwise; `intervene` may raise the existing first-write advisory once.
The Harbor adapter passes `ANGEL_TURN_DEADLINE_SECS=840` (inside Harbor's
900 s kill) only for candidate cells (`ANGELX_SENTINEL`), so pinned baseline
cells are unchanged — notably, Angel previously ran TB2 with hops but *no*
in-box clock at all. The game-world face is the city watch: two guards of
the serving house patrol Keep gate ↔ castle gate while a turn runs
(`overworld/live.rs`, `March.watch` kept apart from `March.riders` so the
retinue contracts hold). Receipts: sentinel 10/10, overworld 127/127,
advisories 13/13, adapter fake-env incl. sentinel env, clippy clean in
touched files, anchors unchanged (2 pre-existing fenced game files).

Measured the same day as a candidate cell on all 7 fail-both tasks (clean
candidate source `c184ac86…` = HEAD + sentinel only, floor exe `518aa1dc…`
in `bin-sentinel/`, detached bench commit `3662081e`; Grok 4.7 low,
`ANGELX_SENTINEL=intervene`, k=1, Apollo/Turbo at `-n 2`; `fix-git` smoke
1.0 in 53 s first). Result: **1/7 solved** — `query-optimize` flipped to a
full 6/6 pass (it had failed identically at 4/6 in every prior cell,
including GrokBuild), while the three targeted harness-gap tasks
(`regex-log`, `git-multibranch`, `nginx-request-logging`) and
`polyglot-rust-c` stayed 0 with baseline-identical verifier signatures;
`regex-log` still never created `/app/regex.txt` and `polyglot-rust-c`
still never created `/app/polyglot`. The secondary aim did land: zero
exceptions (no external `AgentTimeoutError`), with the two former 900 s
timeouts ending as ~876 s agent executions. Verdict: **the sentinel as
built does not fix the targeted failures** — knowing the clock and raising
one first-write advisory is not enough when the loop never lands an
artifact; the single flip is off-target and, at k=1, indistinguishable
from variance. The timings sharpen that reading: five tasks ended in
63–386 s, before the sentinel's first 50%-of-840 s intervention point, so
the sentinel was dormant for them (including the `query-optimize` flip at
283 s); only the two former timeouts crossed its thresholds. What the loop
did after any advisory remains unobservable
(retained trial artifacts still carry no Angel envelope), so envelope
download stays the critical path, followed by stronger delivery mechanics
(named-artifact enforcement and server-liveness handoff), not more nudges.

## Sentinel v2/v3 cells and the TLS root cause (2026-10-13, measured)

V2 added envelope retention (adapter downloads `angel-result.json` +
stderr pre-teardown; Harbor token accounting backfilled), named-artifact
drift/gate and a liveness handoff (`ANGEL_SENTINEL_ARTIFACTS`,
`ANGEL_SENTINEL_LIVENESS_CMD`, gate capped at 2 withholds). V2 scored
1/7 nominal (`git-multibranch`), but the first retained envelopes proved
`regex-log`/`polyglot-rust-c` infra-invalid for every pre-fix Angel
binary: hop 1 burned ~867 s on 339/343 unbounded retries against TLS
`UnknownIssuer`, zero tool calls — vendored ureq with `native-certs`
trusts only the OS store, and those task images ship no ca-certificates.
V3 unions webpki + native roots (`vendor/ureq/src/rtls.rs`, candidate
`29e1ab6d…` / exe `4d22fb0a…`) and scored **3/7** on the full fail-both
set: `regex-log` 1 (114 s, TLS-fix proof), `git-multibranch` 1 (gate
withheld twice on dead :8443, then pass — direct gate-causal evidence),
`query-optimize` 1 (sentinel dormant; not credited). Targeted trio
2/3 vs baseline 0/3 (GrokBuild 3/3). Open: server survival across agent
exit (`nginx…` gate ×1 then dead at verifier; `kv-store` 5/7 identical),
polyglot exact file-set, unbounded retries on permanent transport
errors. Postable write-up: `research-angel-harness-2026-10-13.md`.

## Sentinel v4/v5: persistent services and the final fail-both cell (measured)

V4 added `proc_run persistent=true` (operator opt-in
`ANGEL_PROC_PERSISTENT_SERVICES`), an owned-job gate block, and
exact-directory contracts (`ANGEL_SENTINEL_EXACT_DIRS`); it scored 1/4
on the remaining tasks (`polyglot-rust-c` via the exact-dir gate) and
proved persistent daemons still died at Angel exit — the main-process
subreaper cleanup signals every descendant, adopted `setsid` sessions
included. V5 registers persistent sessions (starttime-pinned) as exempt
from that cleanup and scored a strict same-binary **5/7** on fail-both
(regex, git, nginx 8/8, polyglot, kv 7/7; configure 0/3 attempts at the
deploy chain, query 5/6 runtime borderline), vs baseline 0/7 and
GrokBuild 3/7; targeted trio 3/3. Candidate `7c316b4d…` / exe
`d937deea…` (`bin-sentinel5/`), cells `sentinel5-jobs` + `sentinel5b-jobs`
(+ `sentinel5c` configure attempt with dual SSH+HTTP liveness). All
harness bugs this program identified are fixed with receipts; remaining
reds are task-execution quality also failed by the GrokBuild control.

## Resource pins (pilot cell)

Resource files sha256 `8d3a2f33…`, runner `6b0ce5be…`, catalog `0faa3169…`,
selected rows `cbdf9e81…`, evaluator files `83ccb5b6…`; full values in
`cell-pilot-luna-medium-r2/manifest.json`.

## Completion audit

Done: frozen pins, config gate, pilot baseline, prefix/token audit,
strength-ladder spec, falsifiable candidate, test triage, TB env repair +
taskset load, Angel Harbor + Verifiers adapters, glibc-floor binary pair,
fix-git smoke, and steering-20 under two models (Kimi K3 medium 11/20,
Grok 4.7 low 13/20) on the local fleet. Not done and not claimed: any
harness behavior change, any peer-harness win, TB token accounting (Harbor
does not retain Angel usage). The GrokBuild control above has since run:
3/7 vs Angel 0/7 on the fail-both set. Next steps: teach the adapter to
download the Angel envelope before teardown (timeout diagnosis is blind
without it), then inspect/repair long-task loop discipline (output
artifact early, time-budget awareness) and server-lifecycle handoff to
the verifier, then the green-terminal A/B on the pinned pilot. Grok 4.7
low is the steering standard going forward.

## V5 medium follow-up on the two misses (2026-10-13 evening, measured)

Per the operator, after the harness handoff (`9e3456aa`): the preserved v5
binary (exe `d937deea…`, source `7c316b4c…`) reran only the two strict-v5
misses at **Grok 4.7 medium**, k=1, with the exact v5 per-task sentinel
configs recovered from the rig launch scripts (`configure-git-webserver`
on Apollo with `/git/server` artifact + :8080 liveness; `query-optimize`
on Turbo plain). Launched in parallel 2026-10-14T00:52:44Z as
`sentinel5-medium-jobs/2026-10-13__19-52-44` on each rig
(`run-sentinel5-medium.sh`, staged beside the v5 scripts); the route token
expired 01:21:24Z and both agent phases finished before it (00:57:00Z /
00:58:19Z) with zero TLS/auth markers and completed envelopes, so the
cell is valid. Envelopes confirm `reasoning_effort=medium`, model
`grok-4.7`; rewards corroborated by trial result, job mean, and
`reward.txt`; zero errored trials.

| Task (medium) | Reward | Agent | Hops | Input / gen | Gate | vs v5 low |
|---|---:|---:|---:|---:|---:|---|
| `configure-git-webserver` | 0 (0/1) | 251.6 s | 15 | 198,242 / 12,893 | 0 | 0 at 264.8 s, 30 hops, 442,150 / 11,584, gate 1 |
| `query-optimize` | **1 (6/6)** | 332.2 s | 9 | 81,640 / 6,405 | 0 | 0 (5/6 runtime) at 297.7 s, 9 hops, 72,414 / 3,274 |

Assessment. (1) `configure-git-webserver` fails **identically** at medium:
the verifier reaches the web server and gets HTTP 404 for the expected
deployed page, the same terminal signature as v5 low attempt 1 — reached
with half the hops and input tokens. Effort is not the binding
constraint; the handoff's shared-hard-red classification stands, now
0-for-4 on the v5 binary (3 low + 1 medium) and 0 in every earlier Angel
cell and the GrokBuild control. The only remaining recommended spend is
the handoff's one instrumented diagnostic, not further plain attempts at
any effort. (2) `query-optimize` passes 6/6 at medium including the
runtime-vs-golden check that failed in the v5 low cell, but this is
**not** evidence that medium fixes it: it passed at low in v1 and v3 with
the sentinel dormant, and the medium trial sits inside that known
pass/fail interleave. Borderline/performance-variance classification
stands absent an operator-chosen k≥3 pass-rate cell. The strict v5 low
cell remains **5/7**; this follow-up is a separate 1/2 cell on the missed
pair. Across both efforts the v5 binary has passed 6 of the 7 fail-both
tasks at least once. Full record: `sentinel_v5_medium_followup_2026_10_13`
in `tb2-selection.json`; harvester `harvest_tb2.py` added to
`tb2-project/` and both rigs, validated against the recorded v5 jobs
before use.

## Query-optimize low k=3 pass-rate cell (2026-10-13 night, measured)

The operator authorized the handoff's query-only pass-rate cell: exact v5
binary, Grok 4.7 **low**, k=3 sequential on Turbo (-k 3 -n 1), v5 query
config. The route token had expired, so it was refreshed via the grok CLI
(expiry 2026-10-14T07:19Z) and restaged; `fix-git` smoke passed at low
before the cell. The adapter gained opt-in `ANGELX_RETAIN_FILES`
(sha `f07291e5…`, fake-env test extended and passing) so each attempt
retained `/app/sol.sql`. The first launch is **invalid and not scored**:
Turbo rebooted externally at 01:27Z, ~3 min into attempt 1 (container
exit 137, `OOMKilled=false`, no envelope); after post-reboot
reverification the cell relaunched as
`sentinel5-lowk3r2-jobs/2026-10-13__20-31-44` and the smoke passed again.

Result: **2/3 pass** (Harbor mean 0.667, 0 errored). Chronological:
attempt 1 reward 0 (5/6, runtime only, 383 s agent, 12 hops); attempt 2
reward 1 (6/6, 302 s, 11 hops); attempt 3 reward 1 (6/6, 303 s, 11 hops).
Sentinel dormant in all three. Cumulative Angel-at-low on this task is
now **4 passes / 8 attempts**; medium stays 1/1, and nothing revises the
strict 5/7 cell.

The retained SQL settles the open classification question: this is **not
timing noise**. All attempts emit the same algorithm, but in two regimes.
The failing formulation ranks with `ROW_NUMBER` over the full unfiltered
set before aggregating and runs at a deterministic 1.52 s median against
the golden query's 1.03 s (speedup 0.675; the earlier v5-low failure
measured 0.661, and within-run spread is ~0.5%). Passing formulations
aggregate and filter first, rank only qualifying words, and tie the
golden at ~1.02 s (speedup 0.999 in both passes here, 1.016 for the
medium pass). No measured attempt lands near the threshold. The task is
therefore a model output-shape lottery — roughly even odds at low
between a parity form and a ~1.5× slower form — with no harness
mechanism implicated. By the handoff's own decision tree, the only
harness-side lever would be a general local performance acceptance
(prove the optimized artifact matches the public original before
answering); that is an operator product decision. No further attempts on
this task are recommended. Full record:
`sentinel_v5_lowk3_query_2026_10_13` in `tb2-selection.json`.
