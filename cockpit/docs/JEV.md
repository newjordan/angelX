# Jev decisions and benchmark arithmetic

Optional model tool. Not a public product feature. `jev_decide` registers only
when the operator sets `TYPESAFE_API_KEY`; `ANGEL_JEV=0` disables it. There is
no bundled credential and no extra runtime.

The ordinary cockpit exposes two deferred tools through `tool_search`:

- `jev_decide` asks TypeSafe's Jev narrow technical questions about supplied
  evidence. It returns probability percentages, choice distributions and
  confidence, or ordered rubric scores. These are **model estimates**.
- `benchmark_compare` calculates percentages from paired, nonnegative baseline
  and candidate samples on one named dataset. It reports mean, median, nearest
  rank p95, sample standard deviation, signed improvement, ratio-of-means speedup,
  and paired wins/ties/losses. It runs locally without credentials.

Neither tool creates verification, reward, submission, or promotion evidence.
For example, latency falling from 10 ms to 5 ms is a 50% reduction and a 2×
speedup. A Jev probability of 90% is neither of those measurements.

## Configuration

Keep `TYPESAFE_API_KEY` in a private environment file with owner-only permissions, or the ignored `.angel.env`.

The interactive launcher loads `.angel.env`, plus the shared host credentials when `ANGEL_HOST_ENV=1`.
Headless launches inherit the caller's environment or use an explicit
`ANGEL_RUNNER_ENV_FILE`, such as `~/.config/host_env/system.env` for the Jev CLI;
they do not import all interactive secrets. A research runner's `--env-file`
must also provide its selected generation credential.

`ANGEL_JEV=0` disables Jev. With a key present the tool is registered without
making a request. `ANGEL_JEV_MODEL` defaults to `jev-latest`;
`ANGEL_JEV_TIMEOUT_MS` defaults to 5000 (250–30000); `ANGEL_JEV_MAX_CALLS` defaults
to 64 attempts per registry (1–256). Failed calls consume the budget. There are
no automatic retries. Results are cached in memory for five minutes, up to 32
entries. Cache hits report zero new token usage and retain original usage under
`cached_provider_usage`. No evidence or key is stored by the tool on disk.

Requests go only to the [official TypeSafe endpoint](https://docs.typesafe.ai/api),
using a bearer header, without redirects or a caller-controlled destination.
Only explicitly supplied state/questions are sent; the tool does not crawl the
repository. Known credentials are rejected in request bodies. Requests are
limited to 32 KiB, state to 24 KiB, responses to 64 KiB, and batches to eight
questions with at most sixteen options each. Errors do not echo response bodies.

## Runner and command-line use

For scripts, use the same native implementation without a generation turn:

```sh
ANGEL_RUNNER_ENV_FILE="$PWD/.angel.env" bin/angelX --jev-json <<'JSON'
{
  "state": "Synthetic example: correctness passed on three public diagnostic cases; no full-corpus result yet.",
  "questions": [
    {"id":"next","type":"choice","instructions":"Which diagnostic should come next?","options":["Run the full public corpus","Repeat the same three cases"]},
    {"id":"complete","type":"noul","instructions":"Does this evidence establish a full-corpus improvement?"},
    {"id":"coverage","type":"score","instructions":"How complete is the validation?","options":["No tests","Diagnostic subset only","Full public corpus","Independent held-out evaluation"]}
  ]
}
JSON
```

Question ids only correlate results; Jev does not see them. Put the actual
question in `instructions`. A `noul` has no options or separate confidence.
A `score` uses ordered levels indexed from zero; its normalized rubric position
is **not** a probability. All response types, ranges, labels and distributions
are validated before reaching the calling model.

## Where to use it

1. **Code audits:** first collect compiler, connection-audit and test evidence.
   Ask Jev to prioritize a small set of suspected defects or choose the next
   diagnostic. Trace actual callers before deleting a suspected dead path.
2. **Competition experiments:** use `benchmark_compare` on matched samples from
   one dataset. Ask Jev separate questions about regression risk, strength of
   the supplied evidence, or which hypothesis to test next. Preserve the exact
   candidate and independent evaluator receipt.
3. **Failure diagnosis:** batch narrow classifications of an explicit error
   excerpt and proposed diagnoses. Include an insufficient-evidence option.
4. **Review prioritization:** score independently defined concerns using concrete
   rubric levels. Escalate uncertainty to code inspection or tests; do not make
   Jev a hidden correctness gate, automatic model router, or per-hop tax.

Use `npm run check:connections` to check literal module targets and unanchored
Rust files in active roots. This is also part of `check-cockpit-fast.sh`. The
static check does not prove runtime reachability or behavior; ordinary cockpit
and runner tests remain required.
