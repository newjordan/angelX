# Optional loop research

- `loop_research` is available throughout an active loop.
- `suggest`: select `pareto`, `bandit`, `memory`, or any combination. Supply candidate ideas if useful. No model requests.
- `run`: try a chosen `idea` in an isolated copy, on the loop's current model and effort. Work continues asynchronously.
- `compare: true`: explicitly add a baseline attempt against the same frozen source. Two attempts; a real measured delta when both complete verification.
- `use_memory: false`: omit historical snippets from a run. `verify: null`: explore without assigning a learning reward.
- `status`, `results`, `stop`: inspect the work, collect its patch/receipts, or leave research while the main loop continues.
- Completed physical verifier outcomes feed the original algorithms. Slow-model and MLP scores, fast-context snippets and bandit advice reach the next suggestion.
- Learning persists per workspace, objective, verifier and route. Records retain source hashes; advice can cover earlier source revisions. Only paired experiments establish a within-source delta.
- Receipts include advice, source snapshot, baseline, candidate, learning and total worker timings. Learning failure has an explicit `completed_with_learning_error` status.
- A stopped attempt retains artifacts without becoming a negative training example. Pausing, replacing or ending the loop also cancels its research worker.
- No automatic patch application or policy installation. Native `rl_campaign` remains the audited policy path. Submit a verified winner when ready.
- Requires Python 3 standard library only. The binary embeds the runtime; no development checkout or external service is needed. `ANGEL_RESEARCH_PYTHON` selects an interpreter.
- `UPSTREAM.json` fingerprints the ten unchanged original modules. Minimal package initializers and `runner.py` are the integration adapter.

These options qualify execution and feedback continuity. They do not establish a
live-model performance gain or provider-weight training. The broader original
Sloptomizer application remains in the private experimental tree.
