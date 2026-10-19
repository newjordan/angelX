# Angel harness benchmark handoff — 2026-10-13

## Read this first

This is the restart point for the Angel coding/RL-loop engineering program
after the 2026-10-13 benchmark session. It is safe to clear the prior session
after this file is committed to private `dev`.

Authoritative long-form records:

- `docs/telemetry/research-angel-harness-2026-10-13.md` — postable research
  write-up, including method, negative results, limitations, and an X thread.
- `docs/telemetry/engineering-baseline-2026-10-13.md` — running engineering log.
- Local-only, git-ignored evidence under
  `artifacts/speed-loop/baseline-2026-10-13/`, especially
  `tb2-selection.json`, preserved candidate binaries, and the Harbor adapter
  in `tb2-project/`. A fresh clone will not contain those artifacts.

Standing scope: technical harness work only — correctness, reliability,
streaming/performance, usability, testing, tooling, and docs that match code.
Do not tune games for fun, author game/visual content, or make game-feel
decisions. The city watch is the one operator-requested world surface in this
program. When technical work touches a game system, fix the technical fault
and report design numbers without retuning them.

Standing model instruction from the session: do not rerun Kimi for this
steering cohort. Grok 4.7 at low effort is the steering standard unless the
operator changes it.

## Terminal result

On the adversarial seven-task Terminal-Bench 2 subset that Angel originally
failed under both Kimi K3 medium and Grok 4.7 low:

| Cell | Result |
|---|---:|
| Frozen Angel baseline | 0/7 |
| GrokBuild control, Grok 4.7 low | 3/7 |
| Angel sentinel v1 (clock/advisories) | 1/7 |
| Angel sentinel v2 (gate; pre-TLS fix) | 1/7 nominal; two tasks TLS-invalid |
| Angel sentinel v3 (+ TLS root union) | 3/7 |
| Angel sentinel v5 final stack | **5/7 on one binary** |

The targeted trio — `regex-log`, `git-multibranch`, and
`nginx-request-logging` — is 3/3 on v5, tying GrokBuild. Angel additionally
passes `polyglot-rust-c` and `kv-store-grpc`, which GrokBuild failed. This
supports an adversarial-subset claim only. It does **not** establish overall
superiority to the Grok harness because no GrokBuild steering-20 control was
run.

Strict v5 first attempts were:

- Pass: `regex-log`, `git-multibranch`, `nginx-request-logging`,
  `polyglot-rust-c`, `kv-store-grpc`.
- Fail: `configure-git-webserver`, `query-optimize`.

All TB2 candidate cells are k=1 per task unless an explicit extra attempt is
named below. Extra attempts on `configure-git-webserver` must not be folded
into the strict cell score.

## Private Git state at handoff

Private remote: `origin` → `newjordan/angelX-private`.

Before this handoff commit, `origin/dev` was:

- `89c2eb44` — dev base used for scoped publication.
- `d864d0c5` — sentinel delivery gate, TLS root union, city watch, tests,
  environment docs, engineering baseline, and initial research log.
- `81a48040` — persistent services, shutdown-cleanup exemption, exact-dir
  gate, v4/v5 tests and measurement updates.

This handoff is intended to be the next scoped commit after `81a48040`.
Verify it with `git ls-remote origin refs/heads/dev` after publication rather
than trusting any hash self-reference in this file.

The main workspace remains `.` on `glm_map_fixes` at
`1c826605`, with a deliberately dirty tree. As of this handoff, all 15 scoped
sentinel/TLS/watch/research files in that workspace were byte-identical to
the publish worktree content already on private `dev`. Do not broad-add,
stash, reset, clean, or otherwise normalize the main tree: it also contains
operator game work and other changes outside this program.

Local branch `dev` was still at `89c2eb44`, behind `origin/dev`; publication
was done from a detached publish worktree at `/tmp/angelx-dev-publish`.
Do not assume `/tmp` worktrees survive reboot. The candidate build worktree
was `/tmp/angelx-sentinel-1c826605`, with detached bench-only commits:

- `3662081e` — sentinel v1 candidate.
- `a3716a9d` — sentinel v2 candidate.
- `ec62b5c9` — TLS union candidate (v3).
- `faa41948` — persistent services + owned/exact gate candidate (v4).
- `835e7858` — shutdown spare candidate (v5).

Those candidate commits explain reproducible benchmark builds; private `dev`
contains the reviewed product implementation and documents, not bench-only
commit layering.

## Frozen and candidate identities

Frozen baseline:

- Commit: `1c826605`
- Source digest: `9dd74be57582e6230657c3a69ac507e7011a2082c33191bcd6462779ca367d5f`
- Glibc-floor executable: `12eb2cea54623615d5262a4b533ab1505a1d374569b324284623026eba65355f`
- Helper: `1cc27c231a9e5743d0b4510dd0562a32d821873d344403c8045b6ac854304b27`

Final v5 candidate:

- Source digest: `7c316b4cd43017ba5a648a4a11b526d1a09d361bb6dc071411ce8d19c2015941`
- Executable: `d937deea179504f9d57013d55e2d94ac145e72d40f66ba4ff4ba2bf024fed053`
- Helper: `6b94b830753338d6300b6fcca6e9fd21fac927c50920ec669f280086a539d2fc`
- Maximum glibc: `GLIBC_2.30`
- Local preserved pair:
  `artifacts/speed-loop/baseline-2026-10-13/bin-sentinel5/`

Intermediate candidates and full per-task evidence remain recorded in the
local `tb2-selection.json`. Never overwrite a baseline or prior candidate
directory; stage candidates beside it.

## What was actually fixed

1. **TLS trust roots.** Vendored ureq with `native-certs` used only the OS
   certificate store. Cert-less task images made every public HTTPS provider
   call fail with `UnknownIssuer`. `vendor/ureq/src/rtls.rs` now unions the
   bundled Mozilla roots with native/private roots.
2. **Benchmark observability.** The local Harbor adapter downloads Angel's
   result envelope and event stderr before container teardown and backfills
   token accounting. This adapter is under git-ignored `artifacts/`, not in
   the Git commit.
3. **Sentinel delivery gate.** Deadline-carrying turns can name artifacts,
   exact-directory contracts, and a liveness command. Intervention is opt-in
   and bounded to two final-answer withholds.
4. **Persistent post-exit services.** `proc_run persistent=true` is available
   only when `ANGEL_PROC_PERSISTENT_SERVICES=1`. Registered persistent
   sessions are spared by process-owner shutdown cleanup. This intentionally
   escapes normal sandbox-helper containment and is for throwaway task
   environments with a declared post-exit endpoint, not ordinary turns.
5. **City watch.** While a turn runs, two guards of the serving house patrol
   between the Keep and that house's castle. This is presentation only and
   was not part of benchmark candidate binaries.

Do not credit the sentinel clock alone for the final score. V1 was measured
and falsified on its targeted tasks. The gains came from TLS correction,
observability, artifact/directory enforcement, and process-survival fixes.

## Last two reds

### 1. `configure-git-webserver` — last shared hard failure

Status: reward 0 in every Angel cell and in the GrokBuild control. V5 made
three attempts total; only the first counts in the strict 5/7 cell.

What is now known:

- The original process-survival explanation is no longer sufficient. In the
  first v5 attempt, the verifier reached an HTTP server after Angel exited
  and received 404. The remaining break is therefore in the deploy chain,
  not simply "server died at exit."
- The second attempt failed earlier, around the verifier's SSH/expect path.
- The third attempt used dual SSH+HTTP liveness. The sentinel spent both
  allowed withholds, after which the final answer stood and the verifier
  still received 404 for the expected deployed page.
- Retained v5 evidence shows the model repeatedly rebuilding SSH, the bare
  repository, the post-receive hook, and web service. A late full repository
  reinitialization makes ownership and hook execution prime suspects, but
  the retained event rendering does not preserve every shell script body or
  hook output needed to prove the exact broken link.
- Do not keep rerunning identical attempts and call chance a harness fix.
  Pass@N may be reported separately if the operator chooses, but it is not
  evidence of a code correction.

Recommended next experiment, if the operator wants 7/7 rather than stopping
at a proven 5/7 subset win:

1. Run **one instrumented diagnostic**, sequentially and at concurrency 1.
2. Before teardown, retain operator-named non-secret diagnostics derived
   from the public task contract: the hook file, relevant directory and
   ownership listing, effective SSH configuration summary, document-root
   listing, and service logs. Do not download keys, tokens, or verifier
   internals.
3. Require an end-to-end self-proof as the SSH user before accepting the
   model's answer: clone or push through the public SSH path, trigger the
   hook, and curl the expected deployed page. Record which single link fails:
   authentication, push authorization, hook execution, checkout ownership,
   document root, or HTTP service.
4. Change product code only if that failed link implicates a general Angel
   mechanism. If it is model system-administration execution, record it as a
   model/task-quality residual and stop. Do not encode a task-specific fix in
   the harness or read hidden verifier material to manufacture a pass.

Decision needed from operator: accept this as a shared hard red outside the
proven harness-bug set, or authorize the one diagnostic above.

### 2. `query-optimize` — borderline, not a clean bug

Status by Angel candidate cell:

| Cell | Result |
|---|---:|
| Frozen Kimi/Grok baseline | 0, 4/6 |
| GrokBuild control | 0, identical 4/6 |
| Sentinel v1 | 1, 6/6 |
| Sentinel v2 | 0, 5/6 runtime only |
| Sentinel v3 | 1, 6/6 |
| Sentinel v5 strict cell | 0, 5/6 runtime only |

What is known:

- Correctness, single-query, and small-result checks pass in the recent
  failures. The unstable check is runtime versus golden.
- Sentinel was dormant in both passing candidate runs, so neither pass is
  evidence for sentinel effectiveness.
- This is presently a stochastic model/output-performance borderline. Rig or
  verifier timing variance has not been isolated, and no harness mechanism
  has been implicated by retained evidence.

Recommended next experiment:

1. Run query only on the exact v5 binary at k=3 or k=5, sequentially on one
   rig, and report it explicitly as pass-rate evidence — never fold a later
   pass into the strict k=1 cell.
2. Retain the candidate SQL through an operator-named diagnostic download so
   runtime differences can be tied to output shape rather than vibes.
3. Only if the same output shape repeatedly lands 5/6 should an
   operator-supplied local performance acceptance be considered. It should
   compare candidate behavior with the public task's own original path and
   must not inspect hidden verifier material.
4. If passes and failures interleave with materially similar output, classify
   it as measurement/performance variance and stop spending harness effort.

Decision needed from operator: spend roughly three sequential query attempts
(its verifier is about nine minutes each) to estimate pass rate, or leave it
classified as borderline and move to the full-cohort comparison.

## What remains beyond the last two

- **Overall Grok comparison is unproven.** No GrokBuild steering-20 cell
  exists, and final v5 has not been run over the full steering-20 cohort.
  Before saying Angel is ahead overall, run the final Angel binary over
  steering-20 and, if comparative claims matter, GrokBuild over the same
  cohort under the same attempt rules.
- **Permanent transport failures still retry too long.** The TLS cause found
  here is fixed, but a different permanent certificate/auth/transport error
  can still consume the available deadline through retries. Classify
  permanent provider errors separately from transient retryables.
- **Green-terminal A/B remains open.** The pilot identified terminal
  narration cost; it was superseded by reliability work. Use the pinned pilot
  and compare reward, wall, calls, and tokens at equal attempt rules.
- **Treebeard HTTP diff is separate.** The main workspace still contains an
  uncommitted prior-session change around `cockpit/src/agent/club/http.rs`
  (with an associated modified club test file). It was not in the benchmark
  candidate builds and was not published in the sentinel commits. Review it
  as its own change; do not assume it is either safe or included.
- **Undernetwork proposal is separate.** `docs/architecture/undernetwork-proposal.md`
  remains an untracked prior-session proposal in the main workspace. It was
  not validated or published by this benchmark program.

## Rig and benchmark safety rules

The operator was explicit: do not brick the rigs.

- Start every rig interaction read-only: uptime/load, free disk, container
  count, whether Harbor is already running, and the exact target paths.
- Never reboot, prune, broadly kill, clean, or overwrite a baseline or prior
  candidate. Stage new binaries and job outputs beside old ones.
- Use detached, logged jobs and low concurrency: sequential per-task jobs or
  at most two concurrent trials per rig were the final safe pattern.
- Check remaining route validity before launching. Never print, paste, or
  commit credentials or local access material.
- Turbo rebooted once externally during the program and was reverified before
  reuse. Its root disk was 85% used with about 134 GB free at last check;
  always recheck rather than carrying that number forward.
- Apollo had other users and nontrivial background load near the end. Recheck
  load before scheduling anything long.
- Access material left in benchmark work directories on the rigs still needs
  an operator decision. Do not read it into chat, docs, or commits; remove it
  only when the operator says to.

## Clear-out checklist

- [x] Product fixes and final research are on private `dev` through
      `81a48040`.
- [x] Machine-readable local evidence is preserved under
      `artifacts/speed-loop/baseline-2026-10-13/`.
- [x] Candidate binaries are preserved beside the frozen baseline.
- [x] No scoped product file remains unpublished: the main-workspace copies
      were byte-identical to private `dev` at handoff reconciliation.
- [x] This handoff is the scoped successor to `81a48040`; verify the live
      `origin/dev` hash after publication.
- [ ] Operator chooses whether to run the configure diagnostic.
- [ ] Operator chooses whether to spend a query-only pass-rate cell.
- [ ] Operator decides whether local benchmark access material stays or goes.

After the checked handoff push is verified, this session can be cleared. A
future session should start from this file, the research log, and `git log`
on private `dev`, then inspect live status before believing any path or rig
state named here.
