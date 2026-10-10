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

## Live relationships

Ordinary turns now send executed, typed verifier receipts to the same bundled
Sloptomizer through an asynchronous local worker. A public caveman line of
`observation → hypothesis → check → expected result` supplies optional context;
there is no extra bookkeeping tool call or required format. Bounded check and
diagnostic excerpts pass through the existing credential redactor before
retention; full tool output and private reasoning are not copied. Receipt and
check hashes retain identity, with the producing model, session, turn, and tool
call recorded on each live event.

This context is shared by workspace, objective, and configured verifier across
models. Fitness learning keeps its existing route-specific scope. Individual
tool checks, inconclusive outcomes, and hypothesis text never create fitness
rewards. Measured `/loop`, `loop_research`, and `rl_campaign` results contribute
to both paths, retaining source and receipt provenance. `loop_research suggest`
includes the shared relationships; `action: "context", task: "…"` recalls them
outside a loop too.

The live projection retains both positive and negative receipts, recognizes
changed verdicts, and notices identical evidence under the same hypothesis.
Changing diagnostics also retain a separate consecutive-verdict count. Repeated
failed verdicts emit sparse advice even when clocks or alternating failures make
every receipt different; the card does not claim the failures are identical or
that work made no progress. A changed hypothesis, expectation, model, or verdict
starts that count again, and the original receipts remain intact.
Advice arrives with `⚠⡫` book addresses and a small caveman relationship card.
English definitions appear at first use; models can decode the selected receipts
through `read_file ledger://⡫⠁`. A replacement model receives the run's used
signal legends again, including after compaction. Repetition notices become
sparser as the same evidence repeats. They never stop a turn, change a model,
force a pivot, deny a tool, or impose a research budget.

The used legend inventory persists as local session metadata, outside provider
messages. Automatic and operator-requested compaction preserve it; ordinary hops
do not add a handoff message, and unchanged evidence cards survive turn boundaries.

Completed background experiments announce their result inside the active turn.
The worker makes no provider requests and never holds up tool dispatch or turn
exit. Its memory can lag a running turn; queue overflow and storage failures
produce an advisory, while research continues. `ANGEL_SLOPTOMIZER_LIVE=0` disables
automatic live observation for ablation. Explicit experiment learning remains
available. Unit fixtures enable live persistence explicitly to avoid teaching
synthetic observations to an operator's store.

The [School of Magic](../../../docs/world-school.md) mirrors literal receipt,
check, contrast, and inconclusive counts in the world, with a study and an
underground archive. Its inhabitants do not initiate research.

These additions provide durable experimental context and reactive advice. They
do not yet establish a measured task-success or token-cost improvement; that
requires matched task-level trials with the live path enabled and disabled.

Python 3 is required; the runtime and its standard-library-only modules are
embedded in the cockpit binary. `ANGEL_RESEARCH_PYTHON` selects the interpreter.

## Provenance and the duplicated copy

Nine of these modules are copied byte-for-byte from the original Sloptomizer
source; `micro_llm/core.py` includes the F20 unigram fallback repair.
[UPSTREAM.json](UPSTREAM.json) records each bundled sha256, the source commit,
and the repaired module's original upstream sha256. Where a development checkout also carries the full import
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
