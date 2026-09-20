# Sloptomizer research options

`loop_research` lets an agent select ideas, run isolated experiments, and learn
from verifier results during an active `/loop`. It uses Sloptomizer's Pareto
selector, UCB bandit, and MicroLearner retrieval, token model, and small MLP.

- `suggest` ranks supplied and previously tested ideas using `pareto`, `bandit`,
  `memory`, or a combination. It makes no model requests.
- `run` tries an `idea` on the loop's current model and effort in an isolated
  source copy. `compare: true` runs a baseline and candidate from the same snapshot.
- A paired run stops before starting the candidate if the baseline has no
  completed verifier evidence. The failed baseline receipt remains available.
- `status` and `results` return progress, patches, verifier receipts, measurements,
  and timings. The main loop can continue working while an experiment runs.
- Completed verifier results update selection statistics and MicroLearner state.
  Later suggestions use that saved feedback, including after a process restart.
- `stop` cancels the experiment. Pausing or ending its parent loop cancels it too.
  Cancelled or unverified attempts retain their artifacts without a learning reward.

Learning is scoped to the workspace, objective, verifier, and model route.
`use_memory: false` runs without historical snippets; `verify: null` runs without
assigning a reward. Candidate patches remain available for the agent to review,
apply, and verify. Learning errors are reported alongside the experiment result.

Python 3 is required; the runtime and its standard-library-only modules are
embedded in the cockpit binary. `ANGEL_RESEARCH_PYTHON` selects the interpreter.

## Provenance and the duplicated copy

Ten of these modules are copied byte-for-byte from the original Sloptomizer
source; [UPSTREAM.json](UPSTREAM.json) records the sha256 of each one and the
commit they came from. Where a development checkout also carries the full import
archive, those same ten files exist there under
`experimental/sloptomizer/upstream/`, so the same bytes appear twice. That is
deliberate: this directory is compiled into the binary by `include_bytes!`
(`src/rl_ctl/research_bridge.rs`) and must build without the archive, while the
archive stays the untouched import. The pair is guarded, not free:

```sh
npm run check:research-embed   # embed matches the receipt; archive copy has not forked
npm run check:duplicates       # every other duplicate group in the tree is named
```

`check:research-embed` fails when an embedded module stops matching its recorded
hash, when the archive copy diverges from the embedded one, or when
`source-receipt.json` no longer describes the archived bytes. Edit an embedded
module only together with the receipt and the archive copy.

[Source provenance](UPSTREAM.json) ·
[Integration tests](../../../tests/cockpit/app/rl_ctl__campaign_tests__research_tests.rs)
