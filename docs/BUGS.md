# Bugs

One entry per defect, newest first. Each entry carries the command, the observed
result, the evidence that rules out the obvious suspects, and what would settle it.

---

## BUG-0001 — the full `cargo test` run cannot reach a summary (killed by the agent shell's wall-clock budget)

**Status:** diagnosed · observed 2026-09-21 on `main` (`19bea5d`), 32-core box, 94 GB RAM.

**Impact:** the repository's own gate — `cargo test`, the command the self-model
tells an agent to keep green — cannot be run in one shot from an agent turn. It
never prints a summary. A change can therefore be verified only family by family,
and a suite that cannot be run to a verdict is exactly the "does not hang"
failure the benchmark objective is about. Wall-clock, not correctness: every
family measured on its own is green.

### The foreground kill

```
cd cockpit && timeout 3000 cargo test 2>&1 | tail -25
```

```
bash: line 1: 139780 Killed                  timeout 3000 cargo test 2>&1
     139781 Done                    | tail -25
```

Exit **137** (`128 + SIGKILL`) — on `timeout` itself and on `cargo`, so this is
neither a test panic nor `timeout(1)` firing (that would be `124`/SIGTERM). No
`test result:` line was ever written. Before the kill, libtest had reported 25
tests *"running for over 60 seconds"*: `agent::tools::jev` (3),
`agent::tools::nav` (8), `agent::tools::plan` (1), `agent::tools::proc` (13).

### The cause: the agent shell's foreground budget, not the suite

- `sleep 240` in the foreground: **`Terminated`** — the shell (and the pipeline
  with it) is signalled out from under the command.
- `sleep 120` in the foreground, immediately before: ran to completion.
- So the wrapper terminates a foreground command somewhere in **(120 s, 240 s]**,
  and it signals the whole process group: a suite that needs longer can never
  print a verdict, whatever it does.
- Same suite, detached with `setsid`, survives the turn and runs on:

  ```
  cd cockpit && setsid nohup bash -c 'timeout 2400 cargo test --bin angel \
    -- --test-threads=8; echo "CARGO_EXIT=$?"' \
    > /tmp/angel_full_test_8.log 2>&1 < /dev/null &
  ```

  (a plain `nohup … &` is *not* enough — that job died with the tool's process
  group and left a 0-byte log). The detached run compiled the crate as **0.1.3**
  and was still working through the suite with **no failures** when this entry
  was written; the log carries `CARGO_EXIT=` and a closing timestamp.

### Ruled out

- **Memory.** At kill time: 94 166 MB total, 12 495 MB free, 70 377 MB
  available, 168 GB swap free; no readable cgroup memory limit
  (`/sys/fs/cgroup/memory.max` absent). This is not the OOM killer.
- **The slow families.** `cargo test --bin angel agent::tools::proc` → **24
  passed, 0 failed, in 4.66 s**. The "over 60 seconds" notices are wall-clock
  starvation under libtest's default parallelism (`nproc` = 32 workers over
  ~4 874 tests), not hangs. Nothing in those families is individually stuck.
- **The change under test.** `cargo test --bin angel startup_intro` → 17
  passed, 0 failed, 2 ignored, 5.9 s on the current HEAD.

### The stall under load is contention, not a deadlock

The detached 8-worker run printed **204 ok in its first 24 s**, then went quiet
with the log's mtime frozen, and `ps` named the reason: an unrelated benchmark was
already in flight on the same box —

```
/usr/bin/timeout -k 15 600 /path/to/angel_tests/angelX-bench/polyglot-20260921/pin/angel \
  --task-json --workspace ws/doubly-linked-list --task-id task-43b562c9b8b1cd6ab966 ...
```

— so the suite's worker slots were starved by a live campaign rather than wedged.
Any measurement of this suite has to state what else the box was doing; a quiet
box and a benching box are different instruments.

**Trap, for any harness health check:** `pgrep -f 'cargo test --bin angel'`
reports RUNNING even when nothing is running, because the probe's own command
line matches the pattern. A first pass of this investigation was misled by it —
that is where the "it is still running" reading came from. Use
`ps -eo pid,cmd | rg '[c]argo test'`.

### What would close this

**Development update, 2026-09-30:** a current working-tree no-video snapshot
completed **5,066 passed / 0 failed / 85 ignored** in 608.22 seconds with
`--test-threads=1`, using a private test image and matching CLI/helper siblings.
The ordinary `scripts/check/check-cockpit-fast.sh` now prepares that layout so a
concurrent build cannot unlink the executable that tests re-exec. This establishes
a complete development verdict; it does not qualify the original foreground
wrapper, default video support, or an optimized release. Long foreground gates
still need an execution surface that can remain active for the full run.

- An agent-facing way to hand a gate to a detached runner and collect its
  verdict (the `setsid` recipe above works today, but nothing in the harness
  teaches it, so every long gate looks like a hang).
- Or a bounded documented gate: `--test-threads` capped, and the network
  families (`jev`, `nav`) given per-test budgets so a loopback or DNS timeout
  cannot hold a worker slot for minutes. A suite that fits the budget beats a
  suite that is aspirational.
- Re-measure once: `time cargo test --bin angel -- --test-threads=32` detached,
  to record the suite's true wall clock rather than the 8-thread partial.

## BUG-0002 — the new hang guards are inert under `ANGEL_YOLO=1`

Status: fixed in the working tree on 2026-09-21, not committed. The mixed dirty
package (grok, website, intro) is still the other seat's; do not push it as-is.

The package that sat uncommitted on top of `36adc72` changes `tool_hard_timeout()`
(default 900 s) and `tool_idle_floor()` (default 120 s) in
`cockpit/src/agent/harness/exec.rs`, rejects `sleep` polling in
`cockpit/src/agent/tools/shell.rs`, and exports `ANGEL_TOOL_IDLE_FLOOR_SECS=120` /
`ANGEL_TOOL_HARD_TIMEOUT=600` from `bin/angelX`. Both new defaults sit behind a bypass:

```rust
pub(crate) fn tool_hard_timeout() -> Option<Duration> {
    if crate::platform::yolo::enabled() {
        return None;
    }
    ...
```

`yolo::enabled()` is `profile() == Profile::Full`, and this box runs with `ANGEL_YOLO=1`.
So the guard does nothing in the posture the operator actually uses — which is where the
hang was experienced. Yolo governs *approvals*; a runaway process group is containment,
not consent.

Four of the package's failures are that bypass, measured on one unchanged tree:

| test | `ANGEL_YOLO=1` | `ANGEL_YOLO` unset |
|---|---|---|
| `exec::timeout_diag_tests::cancellable_timeout_path_also_captures_diagnostics` | FAILED, 30.06 s | ok, 0.31 s |
| `exec::timeout_diag_tests::sigterm_grace_lets_a_polite_child_exit_before_sigkill` | FAILED | ok, 0.31 s |
| `exec::activity::tests::run_turn_child_activity_is_owned_recent_and_released` | FAILED (`owned_child_active`) | ok, 0.00 s |
| `exec::delegate_activity::tests::delegate_heartbeat_does_not_erase_silence_and_panic_cleans_up` | FAILED (`worker panic`) | ok, 0.07 s |

The 30 s red is the tell: the test waits out its own deadline for a timeout the bypass
removed, then asserts `capture.timed_out` false. Nothing in these tests pins the posture
they assume, so they inherit ambient env — green in CI, red on the operator's box.

The fifth is a real regression, red with the bypass either way:

- `agent::tools::shell::tests::shell_tool_honors_registry_cancel_authority` — FAILED,
  3.61 s. The new synchronous-sleep guard rejects the command before the registry's
  queued cancel can be honored: *"shell command rejected: sleeping for more than 10s
  inside a synchronous tool call freezes the terminal UI."* A cancel already queued
  should outrank the guard.

Fix, in the working tree, not committed:

- `tool_hard_timeout()` and `tool_idle_floor()` no longer return `None` under
  yolo. Operator `0` still opts out. `tool_timeout()` is unchanged: yolo still
  removes the operator-workload timer. Non-fixed caller deadlines in
  `output_timed_inner` stay stripped under yolo; that separate contract is
  `yolo_preserves_background_descendants_after_the_shell_exits`. The two 30 s
  diagnostics now pin `ANGEL_YOLO` off instead of inheriting the box.
- `call_with_cancel` skips the excessive-sleep redirect when a registry cancel
  token is present, so `sleep 30 & wait` can start and then be cancelled.
  `call()` with no cancel token still rejects `sleep` over 10 s.

Verified 2026-09-21, `cargo test --bin angel -- --exact <path>`, both
`ANGEL_YOLO=1` and `env -u ANGEL_YOLO`, each 1 passed / 0 failed:
`yolo_does_not_disable_containment_ceilings`, both 30 s diagnostics,
`run_turn_child_activity_is_owned_recent_and_released`,
`delegate_heartbeat_does_not_erase_silence_and_panic_cleans_up`,
`shell_tool_honors_registry_cancel_authority`,
`interactive_shell_rejects_excessive_sleep`. Full suite not run (BUG-0001 plus
the live polyglot campaign).

Not verified here: the full suite. BUG-0001's wall-clock cap plus a live benchmark
campaign on the same box prevent a complete run; every result above comes from the
exact-test form, which is what makes each pairing meaningful.

Seen live, 2026-09-21 18:32: a `/yolo on` cockpit launched at 17:10 (binary built
16:50, before the fix landed in the tree at 17:17) ran
`python3 -m http.server 8765 …` through the shell tool in the foreground. The
server never wrote a byte, sat in S, and the turn waited on it for 20+ minutes:
yolo had stripped `ANGEL_TOOL_TIMEOUT`, and the bypass above had stripped the idle
floor, so nothing was left to fire. The wait loop was iterating the whole time —
it was choosing not to kill. `yolo_does_not_disable_containment_ceilings` only
checks the accessors, so
`exec::timeout_diag_tests::yolo_idle_floor_reaps_a_silent_foreground_server` now
drives the real sandboxed path under `ANGEL_YOLO=1` with the cancel token the
shell tool always passes; it fails in 30 s with the bypass reintroduced and passes
in ~2 s without it. Any cockpit launched before 17:17 still runs the unguarded
binary until it is restarted (the launcher rebuilds from HEAD).

## BUG-0003..0007 — the five overwatch harness problems (triaged 2026-09-26; reviewed 2026-09-30)

Source: `/work/artifacts/overwatch-handoff-20260925.md` § "angelX harness problems seen today (fix after
the comp)". Triage landed in that file; seams repeated here so the repo carries them. Separately verified:
the five *polyglot harness-root* fixes (`angel_tests/HANDOFF-harness-roots-20260923.md`, branch
`dev-harness-roots`) are all in `dev` already (`b205b31` is an ancestor of `dev`). The statuses below
describe the ordinary cockpit; working-tree fixes still need to be committed and released.

- **BUG-0003 `ANGEL_DRIVER=k3` silently falls back — fixed in the working tree.** `k3`/`kimi-k3`
  aliases already resolve to `kimi`. Startup now reports an explicit preference that is unresolved,
  unavailable, or not selected, along with the actual startup route. Availability is checked for
  the requested slot, rather than another available slot in its box. The diagnostic does not call
  a provider or change selection. Coverage: `k3_driver_aliases_resolve_to_the_kimi_seat` and the
  `driver_startup_notice` tests in [bag__standalone_tests.rs](../tests/cockpit/club/bag__standalone_tests.rs).
  The full no-video test run on 2026-09-30 passed these regressions.
- **BUG-0004 a steer can't break a running `sleep`.** Steers deliver only at hop boundaries
  (`turn/mod.rs:2290` `steer_drain`); the shell sleep guard only rejects *excessive* sleep up front
  (shell.rs:1186). A queued steer waits out an in-hop sleep while background+sleep-poll seats accrue loop
  actions. Fix: wake in-flight sleeps from the steer queue (reuse the registry cancel channel), count
  cumulative per-turn sleep in the guard, and score background+sleep-poll hops as idle, not actions.
  **Still open.** The 2026-09-30 working-tree guard recognizes literal sleeps inside quotes, wrappers,
  evaluation and shell payloads, and sums duration operands for one sleep invocation. It does not
  wake a running tool from the steer queue, evaluate variable durations, or accumulate sleeps across
  separate calls or loop iterations.
- **BUG-0005 a mid-turn steer halted the rig B loop — fixed in `dev`.** A steer can now
  intentionally release an in-flight provider request when no tool or background work is running.
  The loop recognizes that interruption, preserves the guidance, and continues with the next
  iteration. Explicit interruption with no queued steer still pauses. Cancelled stream events are
  discarded before watchdog refresh, so a buffered token cannot erase the steer marker.
  Coverage: `a_steer_releasing_the_request_does_not_pause_the_loop` and
  `an_interrupt_without_a_queued_steer_still_pauses_the_loop` in
  [cancel_tests.rs](../tests/cockpit/loop_ctl/cancel_tests.rs), passed in the full no-video run
  on 2026-09-30. The new working-tree regression
  `a_buffered_provider_token_after_a_steer_does_not_pause_the_loop` also passed in the
  34-test cancellation family.
- **BUG-0006 plan restatement loop (Sol, five identical turns) — fixed in `dev`.** Consecutive identical
  normalized final replies accrue staleness even when workspace changes or novel receipts would
  otherwise credit progress. The existing stall policy handles the accumulated count. Coverage:
  `a_repeated_reply_does_not_reset_staleness_even_with_churn` in
  [evidence__tests.rs](../tests/cockpit/loop_ctl/evidence__tests.rs); passed in the full no-video run
  on 2026-09-30. A changed reply with fresh findings still resets staleness.
- **BUG-0007 workshop START ignores Enter — fixed in `dev`.** Enter starts from settings rows as
  promised by the workshop help. Coverage: `loop_workshop_enter_starts_even_when_focus_sits_on_a_row`
  in [app/tests.rs](../tests/cockpit/app/tests.rs); passed in the full no-video run on 2026-09-30.
