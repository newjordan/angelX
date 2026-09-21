# angelX Harness & Benchmark Circuit Breakers Cheat Sheet

This guide defines the default mode and required circuit breakers for unattended evaluations, benchmark harnesses (e.g., Prime Verifiers, SWE-bench, polyglot suites), and autonomous headless runs.

---

## 1. Quick Reference: The Evaluation Baseline

When running angelX in headless or benchmark mode, **never run with unbounded circuit breakers**. Interactive developer sessions default to unbounded (`0`) so the agent never gives up on a human user, but benchmarks with greedy sampling (`temperature: 0.0`) **require** finite circuit breakers.

### Standard Benchmark Preset (Copy-Paste)

```bash
# === Circuit Breakers (Anti-Spin & Loop Prevention) ===
export ANGEL_SPIN_LIMIT=4                     # Stop after 4 identical tool batches (stops loops in ~5s)
export ANGEL_UNPRODUCTIVE_STREAK_STOP=12      # Stop after 12 non-mutating/non-progress tool calls
export ANGEL_TOOL_CYCLE_REPEATS=4             # Detect short alternating cycles (A -> B -> A -> B)
export ANGEL_TASK_TREEBEARD=1                 # Enable Treebeard context compression (NEVER set to 0)

# === Execution Horizons ===
export ANGEL_MAX_HOPS=30                      # Cap total turn count (benchmarks solve in 4-8 hops)
export ANGEL_TOOL_TIMEOUT=120                 # Max seconds per tool process (cargo/npm/pytest)
export ANGEL_TOOL_IDLE_SECS=20                # Kill silent/hung processes emitting 0 bytes

# === Task Discipline & Verification ===
export ANGEL_MUTATION_THRASH_NUDGE=3          # Warn on rapid file thrash
export ANGEL_MUTATION_THRASH_STOP=6           # Stop on persistent file thrash
export ANGEL_POST_GREEN_TOOL_BATCHES=1        # Stop tool calling once verifier is green
export ANGEL_NO_EDIT_ANSWER_GUARD=1           # Block answer emission without workspace edits
export ANGEL_TASK_CODING_DISCIPLINE=1         # Enforce clean coding practices
export ANGEL_TASK_RECON=repo                  # Free pre-turn symbol and repository map
export ANGEL_TASK_RECON_MAX_CALLS=24          # Bounded pre-flight exploration calls
```

---

## 2. Parameter Reference & Danger Matrix

| Parameter | Interactive CLI Default | Harness Default | Why It Matters | Fatal Value (Do NOT Use) |
| :--- | :---: | :---: | :--- | :--- |
| `ANGEL_SPIN_LIMIT` | `0` (off) | **`4`** | At greedy `temp: 0.0`, an unmutated failure produces identical prompts, creating an infinite sampling loop. `4` breaks the loop in seconds. | `0` (runs until killed by wall clock) |
| `ANGEL_UNPRODUCTIVE_STREAK_STOP` | `0` (off) | **`12`** | Prevents endless passive reading/listing commands when progress is stalled. | `0` (burns full token budget) |
| `--max-hops` / `ANGEL_MAX_HOPS` | `0` (unbounded) | **`30`** | Hard ceiling on turns. Real tasks pass in 4–8 turns; 30 gives ample leeway without runaway cost. | `1000` (allows 10-minute spin traps) |
| `ANGEL_TASK_TREEBEARD` | `1` (on) | **`1`** | Compresses historical tool outputs. Disabling it bloats context and reinforces repetition attractors. | `0` (causes context bloat and loops) |
| `ANGEL_TOOL_IDLE_SECS` | `0` (off) | **`20`** | Kills commands (like hung test suites or interactive prompts) that stop emitting output. | `0` (waits for full `TOOL_TIMEOUT`) |
| `ANGEL_TOOL_TIMEOUT` | `120` | **`120`** | Hard wall time per tool invocation (e.g. `cmake --build`). | `0` (unbounded tool hangs) |
| `ANGEL_POST_GREEN_TOOL_BATCHES` | `0` | **`1`** | Immediately finishes the episode once the verifier passes instead of continuing to probe. | `0` (continues burning tokens after solve) |

---

## 3. Integration Patterns

### Pattern A: Prime Verifiers / Evaluator Harness (`eval`)

Pass the circuit breakers via `--env.agent.harness.*`:

```bash
uv run eval angel-action-v1 @ configs/deepseek-flash.toml \
  --env.taskset.tasks-path tasks-polyglot-v1.json \
  --env.agent.harness.id angel-action-v1 \
  --env.agent.harness.angel-bin /path/to/angel \
  --env.agent.harness.max-hops 30 \
  --env.agent.harness.env.ANGEL_SPIN_LIMIT 4 \
  --env.agent.harness.env.ANGEL_UNPRODUCTIVE_STREAK_STOP 12 \
  --env.agent.harness.env.ANGEL_TOOL_IDLE_SECS 20 \
  -n 136 -c 1
```

### Pattern B: Direct Headless Task Execution (`angel --task-json`)

When invoking the `angel` binary directly for automated benchmark rollouts:

```bash
ANGEL_SPIN_LIMIT=4 \
ANGEL_UNPRODUCTIVE_STREAK_STOP=12 \
ANGEL_TOOL_IDLE_SECS=20 \
angel --task-json \
  --workspace "$WORKSPACE" \
  --task-id "$TASK_ID" \
  --run-id "$RUN_ID" \
  --max-hops 30 \
  --deadline-secs 600 \
  --reasoning-effort low \
  --tool-profile essential \
  < "$PROMPT_FILE" > "$RESULT_JSON"
```

### Pattern C: Python Environment Wrapper (`harness.py`)

In any custom Python harness adapter, bake the defaults directly into the launch environment:

```python
env = {
    "ANGEL_SPIN_LIMIT": "4",
    "ANGEL_UNPRODUCTIVE_STREAK_STOP": "12",
    "ANGEL_TOOL_IDLE_SECS": "20",
    "ANGEL_TASK_TREEBEARD": "1",
    **config.resolved_env,  # Allows explicit operator override if needed
}
```

---

## 4. Troubleshooting Diagnostic

If an agent run is exceeding expected durations:

1. **Check for Spin Traps**:
   If the log shows identical tool calls repeated $>4$ times:
   `ANGEL_SPIN_LIMIT` is unset or `0`. Set `ANGEL_SPIN_LIMIT=4`.
2. **Check Context Compression**:
   If the prompt token count grows $>80\text{k}$ tokens on turn 5:
   `ANGEL_TASK_TREEBEARD` was disabled (`0`). Set `ANGEL_TASK_TREEBEARD=1`.
3. **Check Stderr Leakage**:
   Ensure `strip_launcher_stderr` is active so internal sandbox telemetry (`sandbox-hardlinks: {json}`) does not pollute the model's observation window.
