# book-harness bench

The tools behind `docs/telemetry/book-encoder-20260929.md` and
`docs/telemetry/harness-speed-20260929.md`. Outputs land in `out/` (ignored).

The harness runs the `angel-sandbox` **beside its own binary**, so keep each
build's `angel` and `angel-sandbox` together in one directory and pass both.

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
| `gaps.py`, `phases.py`, `harness_time.py` | Per-hop harness time by tool; phase split by session quarter; total harness vs tool time. Run from this directory with labels. |
| `stracesum.py` | Writes/opens/execs per path from an `strace -f` log. |
| `tokens/analyze.mjs`, `tokens/curve.mjs` | Wire tokens under DeepSeek V4, GPT-4o, Qwen3, Llama 3 tokenizers (`npm install` in `tokens/`). |
| `relay.py`, `relay_run.sh`, `answer.py`, `pending.py` | File relay: a request waits for a hand-written or agent-written reply (the Sonnet runs). |
| `vignettes/` | The H3 single-turn vignettes (route vs bare). |

Examples:

```sh
# harness-only speed, three runs per build (release builds in b/ and h/)
for i in 1 2 3; do STUB_CYCLES=70 MAX_HOPS=260 ./run_ts.sh h/angel h/angel-sandbox head-$i plain; done
python3 harness_time.py head-1 head-2 head-3; python3 phases.py head-2

# a real model, both arms of the introduction
PROXY_UPSTREAM=https://host/v1 PROXY_KEY=$KEY ANGEL_LOCAL_MODEL=glm-x ANGEL_BOOK_INTRO=1 ./task_run.sh h intro-1 twobug
python3 run_summary.py out/intro-*.jsonl
```
