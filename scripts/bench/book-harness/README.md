# book-harness bench

The tools behind `docs/telemetry/book-encoder-20260929.md` and
`docs/telemetry/harness-speed-20260929.md`. Outputs land in `out/` (ignored).

The harness runs the `angel-sandbox` **beside its own binary**, so keep each
build's `angel` and `angel-sandbox` together in one directory and pass both.

The stub runners (`run_one.sh`, `run_ts.sh`, `run_phase.sh`) bind an ephemeral
loopback port and wait for their own server's ready receipt. They return the
task process's exit status and remove their private temporary workspace and
home on exit; logs remain in `out/`. Set `TMPDIR` to choose the temporary parent.
The task runs in its own process group, so cancelling the runner also stops its
descendants, including children that ignore SIGTERM. Timestamped runners finish
their stderr formatter before returning. The reported task wall time includes
the common Python session launcher and excludes teardown.

`decide_capture.sh <bin-dir> <label> <scenario>` uses the same private task and
stub ownership for a scripted decision capture, returning the task's exit
status. `decide_turns.sh` runs a multi-turn scenario in its own unique tmux
session and retains TUI stderr. It cleans up the session, stub and jobs still
inside its private workspace on completion or cancellation; jobs that leave
that workspace are outside the TUI cleanup scope. If no request becomes idle
within its polling window, the driver returns 124 rather than reporting a
completed capture.

`task_run.sh` uses the same task-group and formatter ownership and an ephemeral
proxy listener. It returns the task exit status, retains separate verification
logs, and reports `verification_rc` alongside `solved`. Its fixture unittest
check has a 30-second deadline (`VERIFY_TIMEOUT_SECS` overrides it); cancellation
also stops that check and its descendants.

E3 copies files through `copy_fixture.py`: Git metadata is excluded at every
level, filesystem links become private copies, and the fixture gets a fresh
local baseline with no remote. Linked worktrees and submodules therefore do
not retain the source checkout's Git state. Directory cycles and special files
fail setup before a model session starts. Originally tracked files remain in
the private baseline even if they match ignore rules; ignored dependencies
stay untracked. E3 owns one temporary directory and a unique tmux session,
and cleans them up on startup failure or cancellation.

The E1, E3 and E5 campaign drivers create their output directory, wait for
each planned child run, and return the first failed child status after the
campaign finishes. E3 also reports each round's status in its driver output.
PID-only interruption or termination stops active route chains and sessions,
then waits for their cleanup. Campaign cancellation uses Linux pidfds to keep
signals bound to the recorded child process even after a numeric PID is reused.

| tool | what it does |
|---|---|
| `stub_model.py` | Scripted OpenAI-compatible model: answers instantly with a fixed coding script, so two builds see identical model behaviour and only the harness differs. `STUB_CYCLES=N` = a 3N+3-turn session; `STUB_MODE=reader` decodes every stamp once. |
| `run_one.sh <angel> <helper> <label> plain\|reader` | One headless task against the stub (one-bug fixture). |
| `run_ts.sh` | Same, with timestamped task events on stderr (for `phases.py`, `harness_time.py`). |
| `run_phase.sh` | Same, with `ANGEL_TURN_PHASE_TRACE=1` phase breadcrumbs. |
| `proxy_log.py` | Pass-through logger for a **real** OpenAI-compatible endpoint: forwards unchanged, logs each request in the stub's format plus provider usage (Chat Completions or the Responses API); GET probes go to `<log>.get`. |
| `task_run.sh <bin-dir> <label> [onebug\|twobug]` | One headless task against a real model through `proxy_log.py`; reports solved/turns/wall. `ROUTE` picks the seat (`seat.sh`); arm with `ANGEL_BOOK_INTRO=0\|1`. |
| `seat.sh` | `seat_env <port>`: the harness env for `ROUTE` = `local` (needs `PROXY_UPSTREAM`, `ANGEL_LOCAL_MODEL`), `glm-flash`, `deepseek-flash` or `muse` (the native seats, keys from the environment), with the proxy in front of the seat's URL variable. |
| `e1.sh`, `e5.sh`, `e1_table.py` | E1 (intro / bare / pre-book) and E5 (base / head) chains per route, and their per-model median/range table. |
| `e2_count.py <route> <label>...` | E2: each stub session's final request sent to the provider with a 16-token cap; prints the prompt tokens it reports. |
| `e3.sh <rounds> <route>...`, `e3_session.sh <bin-dir> <label>` | E3: live `/loop` sessions in the TUI, driven through tmux, on a copy of the Yukon heesch checkout (`HEESCH`, no remote, no Yukon login) with `e3/verify_heesch.py` as the verifier. Each round runs both arms of every route at once. |
| `route_action.py [--each] out/*.jsonl` | E3a: for each ⡪ route in the newest model-facing messages, whether the model's next call matches the route's action (`d2467_research.rs`). Pinned by `test_route_action.py`. |
| `run_summary.py out/*.jsonl` | Turns, ledger reads, tool calls, provider tokens, wall per run. |
| `gaps.py` | Between-request wall gaps by observed action, including response latency, tool execution and request building. Accepts JSONL paths; `--audit-json` retains unknown actions and clocks. |
| `phases.py`, `harness_time.py` | Phase split by session quarter and measured harness vs tool time using response-completion and tool-event clocks. Run from this directory with labels. |
| `stracesum.py` | Writes/opens/execs per path from an `strace -f` log. |
| `tokens/analyze.mjs`, `tokens/curve.mjs` | Local text-token estimates under four bundled tokenizers. Estimated fresh input uses an exact unchanged prefix and reports context/schema changes (`npm install` in `tokens/`). |
| `relay.py`, `relay_run.sh`, `answer.py`, `pending.py` | File relay: a request waits for a hand-written or agent-written reply (the Sonnet runs). `relay_run.sh <angel> <helper> <fresh-run-dir> [directive]` preserves the task's exit status; `pending.py` reports completed and cancelled runs as `DONE <rc>`. |
| `vignettes/` | The H3 single-turn vignettes (route vs bare). |

Examples:

The relay runner requires a new output directory so previous requests and replies stay intact. It uses a private workspace, home and temporary directory, an ephemeral loopback port, and the shared runner cleanup. The manual task keeps its interaction budget; its final unittest verification defaults to 30 seconds (`VERIFY_TIMEOUT_SECS` overrides it). Output receipts survive cleanup. Requests and replies written by the relay tools become visible after the complete JSON has been written; external reply writers should also publish with a rename.

```sh
# harness-only speed, three runs per build (release builds in b/ and h/)
for i in 1 2 3; do STUB_CYCLES=70 MAX_HOPS=260 ./run_ts.sh h/angel h/angel-sandbox head-$i plain; done
python3 harness_time.py --assume-instant-stub head-1 head-2 head-3
python3 phases.py --assume-instant-stub head-2

# a real model, both arms of the introduction
PROXY_UPSTREAM=https://host/v1 PROXY_KEY=$KEY ANGEL_LOCAL_MODEL=glm-x ANGEL_BOOK_INTRO=1 ./task_run.sh h intro-1 twobug
python3 run_summary.py out/intro-*.jsonl
```

The explicit stub assumption estimates response handling from request arrival for old stub logs that lack a response completion clock. Real proxy logs use their recorded response completion; missing or inconsistent clocks remain visible as partial coverage. Response latency includes proxy and transport work.
