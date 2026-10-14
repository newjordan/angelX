# Making Angel measurable: a frozen baseline, a harder benchmark, and three harness fixes chased to ground

*Research log, 2026-10-13. Test driver: Watermelkong (Muse Wonder-Full 1.0)
running the engineering loop. Model under measurement for the steering
cells: Grok 4.7 low (Angel grok-api route). Everything below is k=1 per
cell unless stated; hashes are the reproducibility pins.*

## TL;DR (postable)

We froze Angel (our coding-agent harness) at a source-pinned baseline,
found the usual benchmark (Polyglot-136) saturated, and stood up
Terminal-Bench 2 on two home rigs. Angel scored 13/20 on a frozen
steering cohort. On the 7 tasks it failed under two different models, the
reference Grok harness solved 3 — so we built a "sentinel" (budget/artifact
overwatch) and re-ran. V1 flopped (1/7, off-target). The fix that mattered
was boring: we taught the benchmark adapter to *keep the agent's result
envelope*, and the first retained log showed the real bug — 339 unbounded
retries against a permanent TLS `UnknownIssuer` error in cert-less task
containers, zero tool calls, whole budget burned in hop 1. Angel's HTTP
client trusted only the OS cert store; it now unions the bundled Mozilla
roots. Final fail-both score on one binary: **0/7 → 5/7** (the reference
harness scored 3/7), after two more iterations the retained logs forced:
persistent services that outlive the agent process, and an exemption from
Angel's own shutdown cleanup that was killing them anyway.
Benchmarks didn't just score the harness — the observability work they
forced found a bug no score could name.

## Method

- Frozen identity: branch `glm_map_fixes` at `1c826605`, source digest
  (angel-source-v2) `9dd74be5…`, verified by two independent routes
  (`--commit HEAD` and a clean worktree). The previously shipped binary
  was **unbound** (dirty build inputs) and correctly refused by the bench
  runner — a baseline is a binary you can name, or it is nothing.
- Benchmark ladder: Polyglot-136 kept as regression smoke (saturated at
  134–136/136 for every serious model; Angel's edge there is cost:
  135/136 in 6.9M input tokens vs 67.7M for a peer harness at 132/136).
  Steering moved to Terminal-Bench 2: a frozen selection of 20 tasks +
  `fix-git` smoke + 5 never-tune holdouts, run sharded across two local
  rigs (Apollo, Turbo) with a glibc-floor binary (cargo-zigbuild
  `gnu.2.31`, max `GLIBC_2.30`) so the same bytes run in bookworm task
  images. Every trial's agent env pins binary sha256 and source sha256,
  fail-closed, verified again inside the task box.
- One attempt per task per cell. No cherry-picking: infra-failed cells
  are preserved and never scored (an early pilot cell exited 0 with
  12/12 infra failures; it is kept, labelled invalid).

## Baseline numbers

- 12-task pilot (Luna medium): 12/12, 331.8 s task wall (median 24.1 s,
  p90 42.4 s), 62 model calls, 432,698 input tokens (55.1% cached),
  11,332 output. First-call prefix median 4,256 tokens at 0% cached;
  terminal-narration calls cost 41.3 s — the green-terminal follow-up.
- Terminal-Bench 2 steering-20: Kimi K3 medium **11/20**, Grok 4.7 low
  **13/20** with zero regressions (Grok alone passed `cancel-async-tasks`
  and `pypi-server`; `fix-git` smoke 49 s). Grok 4.7 low adopted as the
  steering standard.
- Test triage at frozen HEAD: 6,279 passed / 0 failed / 231 ignored; a
  handoff claim of "49 pre-existing failures" did not reproduce and is
  treated as an unverified dirty-tree observation.

## The control that indicted the harness

Seven tasks failed under *both* models. We ran the stock Grok harness
(GrokBuild, grok-4.7 low) on exactly those seven:

| Result | Tasks |
|---|---|
| GrokBuild solves, Angel fails (3) | `regex-log`, `git-multibranch`, `nginx-request-logging` |
| Both fail identically (4) | `configure-git-webserver`, `kv-store-grpc`, `polyglot-rust-c`, `query-optimize` |

Angel's two 900 s timeouts (`regex-log`, `polyglot-rust-c`) produced *no
output file at all*. That became the sentinel hypothesis: give the loop a
clock and artifact discipline.

## Sentinel v1 — built, measured, falsified

Deterministic, model-free overwatch for deadline-carrying turns: Drift at
50% of budget before the first edit, Deliver at 85% with nothing landed,
quiet by default (`observe` = operator notices only; `intervene` adds one
first-write advisory). Plus its face in the game world — city-watch
guards patrolling Keep ↔ castle while a turn runs. Rerun on the fail-both
7 (clean candidate `c184ac86…`): **1/7**. The single flip
(`query-optimize`, 283 s) happened before the sentinel's first threshold,
so it cannot be credited to it; the three targeted tasks stayed 0. The
secondary effect was real — zero external timeouts — but a clock and one
advisory do not land artifacts. Killing your own hypothesis cleanly is
also a result.

## Sentinel v2 + the envelope unlock

V2 kept the quiet core and added the delivery mechanics: named-artifact
drift that unrelated edits cannot disarm, a bounded final-answer gate
(missing deliverable or failed server-liveness check withholds the
answer, twice at most), and — most importantly — the adapter now
downloads Angel's result envelope and event stderr *before container
teardown* and backfills Harbor token accounting (smoke proof: envelope
retained, 154,293 input / 140,032 cached / 2,022 output tokens now
visible where Harbor previously reported nulls).

The first retained long-task log ended the mystery: `regex-log` and
`polyglot-rust-c` burned their budgets in **hop 1** — 339/343 provider
retries, `tls connection init failed: UnknownIssuer`, zero tool calls.
Angel's vendored ureq was built with `native-certs`, whose root store
uses *only* OS certificates and never the bundled Mozilla roots that the
same build already linked. Minimal task images ship no ca-certificates,
so every HTTPS call failed permanently, and unbounded retries ate the
clock the sentinel was built to watch. Fix: union both stores
(`vendor/ureq/src/rtls.rs`) — OS/private CAs keep working, public
endpoints verify everywhere.

Measured results (k=1, Grok 4.7 low, per-task jobs with named artifacts
and liveness commands from the public task instructions):

| Task | Baseline | V1 clock | V3 +TLS | V5 final (one binary) |
|---|---|---|---|---|
| regex-log | 0 (900 s kill) | 0 | **1** | **1** (107 s) |
| git-multibranch | 0 | 0 | **1** (gate ×2) | **1** |
| nginx-request-logging | 0 | 0 | 0 (gate ×1) | **1** (8/8) |
| polyglot-rust-c | 0 (900 s kill) | 0 | 0 (file-set) | **1** (exact-dir gate) |
| configure-git-webserver | 0 | 0 | 0 | 0 (0/3 attempts; 404 deploy) |
| kv-store-grpc | 0 (5/7) | 0 (5/7) | 0 (5/7) | **1** (7/7) |
| query-optimize | 0 (4/6) | **1** | **1** (6/6) | 0 (5/6 runtime) |
| **Total** | **0/7** | **1/7** | **3/7** | **5/7** |

(V2, pre-TLS-fix, scored 1/7 nominal with two TLS-invalid tasks; V4,
persistent services without the shutdown exemption, scored 1/4 on the
remaining tasks. The strict V5 cell counts each task's first V5 attempt.)

Three readings, in order of confidence. (1) The TLS union is proven
causally: `regex-log` went from 0 tool calls in 867 s of certificate
retries to a solve in 114 s, and `polyglot-rust-c` started doing real
work (269 s, 9 calls) and now fails the same strict file-set check the
reference harness failed. (2) The gate has one direct save: on
`git-multibranch` (V3) it withheld the final answer twice — retained
stderr shows *"server liveness check failed (curl exit 7)… answer
withheld"* — after which the model fixed the deployment and the verifier
passed; without the gate, either answer ends the turn as a zero, exactly
as at baseline. (3) The remaining server tasks (`nginx…`, `kv-store`)
expose the next bug class honestly: servers can answer Angel-side yet be
dead when the post-exit verifier connects — survival across the agent
process/exec session, not liveness at answer time. `query-optimize`
passed in V1 and V3 with the sentinel dormant both times; we do not
credit the sentinel for it. On the targeted harness-gap trio the final
stack stands at 3/3, tying the reference harness, and it additionally
passes `polyglot-rust-c` and `kv-store-grpc`, which the reference harness
failed — 5/7 vs 3/7 on this adversarial subset. (4) Server survival took
two more fixes the logs dictated: an operator-opt-in persistent-service
launch (`proc_run persistent=true`, own session, untracked), after V4
proved Angel's shutdown subreaper cleanup still killed those daemons at
exit; V5 registers persistent sessions (starttime-pinned) as exempt from
that cleanup. With that, `nginx…` (8/8) and `kv-store` (7/7) passed with
their servers answering the post-exit verifier, and an exact-directory
gate ("this directory must contain exactly {main.rs}") saved
`polyglot-rust-c`. The last red, `configure-git-webserver` (0/3 attempts,
failed by every harness in every cell), now reaches a live server and
fails on the git→web deploy chain itself — task execution, with no
further harness mechanism implicated by the retained evidence.

## Limitations (read before quoting)

- k=1 everywhere on TB2: single-attempt flips are signals, not proofs.
- Angel reaches grok-4.7 via its grok-api club (OAuth bearer); GrokBuild
  via the grok CLI OAuth session — same named model, different routes.
- The sentinel gate is opt-in (`intervene`) and bounded (2 blocks);
  interactive turns without a deadline never arm it.
- Provider retries are still unbounded by default; bounding permanent
  transport errors is separate follow-up work.
- Persistent services are an operator opt-in that deliberately escapes
  sandbox-helper containment; they are meant for throwaway task
  environments with a declared post-exit endpoint, not ordinary turns.
- `configure-git-webserver` remains red for every harness in every cell;
  its remaining failure is deploy-chain execution, not a mechanism above.

## Reproducibility

Frozen baseline source `9dd74be5…`, floor exe `12eb2cea…`; sentinel v1
candidate `c184ac86…` / exe `518aa1dc…`; sentinel v2 candidate
`05a14dd2…` / exe `9834ee51…`; sentinel v3 (+ TLS union) candidate
`29e1ab6d…` / exe `4d22fb0a…`; sentinel v5 final candidate
`7c316b4d…` / exe `d937deea…`; full per-task evidence in
`artifacts/speed-loop/baseline-2026-10-13/tb2-selection.json` and the
engineering baseline log (`docs/telemetry/engineering-baseline-2026-10-13.md`).
Rigs were preflighted read-only, jobs ran detached at low concurrency,
and no baseline artifact was overwritten by any candidate.

## X thread draft (paste-ready)

1/ We froze our coding-agent harness (Angel) at a source-pinned baseline
and moved steering to Terminal-Bench 2 after Polyglot saturated. TB2
steering-20: 13/20 (Grok 4.7 low). Then we interrogated the 7 tasks it
failed under *two* models. A thread on what benchmarks forced us to fix 🧵

2/ Control first: the reference Grok harness solved 3 of those 7. So part
of the gap was ours. Two tasks timed out at 900s having created *no
output file at all*. We built a "sentinel": budget-fraction overwatch for
deadline task runs. V1 result: 1/7 — and the one flip happened before it
could act. Hypothesis dead, cleanly.

3/ So V2 added observability before more mechanics: the adapter now keeps
the agent's full result envelope + event log before container teardown
(and token accounting stopped being nulls). First retained log of a
"timeout" task: hop 1 burned 867s on 339 provider retries —
`UnknownIssuer`. Zero tool calls. The model was never reached.

4/ Root cause: our HTTP client (vendored ureq + `native-certs`) trusted
*only* the OS cert store. Minimal task containers ship no ca-certificates,
so every HTTPS call failed permanently — in exactly the two "timeout"
images. Fix: union OS roots with the bundled Mozilla roots the build
already linked. regex-log: 867s/0 calls → solved in 114s.

5/ Then delivery mechanics: a gate that withholds "done" while named
artifacts are missing, an exact-directory contract, liveness checks, and
persistent services exempt from our own shutdown cleanup (which was
killing detached daemons at exit — found the same way, in the logs).
Final fail-both on one binary: 0/7 → 5/7, targeted trio 3/3, plus two
tasks the reference harness failed. One deploy-chain task stays red for
everyone.

6/ Lessons: freeze bytes you can name; keep the envelope or you're
debugging a black box; a benchmark "timeout" can be a cert store;
advisories don't land artifacts — gates do; and publish the negative
results with the positives. Full log + hashes in the repo
(docs/telemetry/research-angel-harness-2026-10-13.md). k=1 caveats apply.
