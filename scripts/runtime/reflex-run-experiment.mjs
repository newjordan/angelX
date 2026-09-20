import { runBenchmark } from './benchmark-runner.mjs'
import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// reflex-run-experiment — the interventional runner (Reflex M3).
//
// Closes the loop config-causal (M2) opened: take a config-experiment proposal
// ("pin knob X=V"), run it as a Control Bench A/B against a baseline on a frozen
// agentic suite, and fold the signed verdict back into the causal graph via
// updateBeliefs. This is the INTERVENTIONAL channel — unlike M2's observational
// ledger mining (which only prioritizes), a bench A/B can CONCLUDE a hypothesis.
//
// The do-operator is subjects.combo_env: the intervention's knob map becomes the
// child `angel` process env, so the two arms differ ONLY by the knobs under test.
// The variance guard sizes the metric_delta threshold to 2× the observed
// accuracy_std across seeds, so seed noise can't manufacture a verdict.
//
// Pure core (buildSubjectSpec / extractAB / concludeFromReport) is separated from
// the bench spawn + graph persistence in the CLI, so the conclude-with-correct-
// sign logic is unit-tested against a mock report with no Python, no fleet.

import { updateBeliefs } from './causal-loop.mjs'
import { contractFromControlExperiment, verifyContract } from './experiment-contracts.mjs'

const BASELINE_SUBJECT = 'reflex-baseline'
const TREATMENT_SUBJECT = 'reflex-treatment'

/**
 * The --subject-spec payload for a control-bench contract: two combo subjects
 * differing only by the intervention knobs, pinned to one frozen suite.
 */
export function buildSubjectSpec(contract) {
  const r = contract.runner || {}
  const iv = contract.intervention || {}
  return {
    suite: r.suite,
    seeds: r.seeds ?? 2,
    quick: r.quick ?? true,
    ...(r.ids ? { ids: r.ids } : {}),
    subjects: [
      { name: BASELINE_SUBJECT, env: iv.baselineKnobs || {} },
      { name: TREATMENT_SUBJECT, env: iv.knobs || {} },
    ],
  }
}

/**
 * Pull the baseline/treatment quality metric + its per-seed std out of a Control
 * Bench report (`~/.angel0/benchmarks/benchmark-<request-id>.json`, which carries the full
 * per-subject `metrics` list). Metric name defaults to the contract's measured
 * axis (accuracy_pct).
 */
export function extractAB(report, measured = 'accuracy_pct') {
  const bySub = new Map((report?.metrics || []).map((m) => [m.subject, m]))
  const read = (name) => {
    const m = bySub.get(name)
    if (!m) return null
    return {
      [measured]: typeof m[measured] === 'number' ? m[measured] : NaN,
      accuracy_std:
        m.accuracy_std != null && Number.isFinite(Number(m.accuracy_std))
          ? Number(m.accuracy_std)
          : null,
      n: m.n,
    }
  }
  return { baseline: read(BASELINE_SUBJECT), treatment: read(TREATMENT_SUBJECT) }
}

/**
 * The metric_delta threshold: 2× the larger per-seed accuracy_std (the plan's
 * variance guard), floored at `floor` (default 1.0 pt). The floor is load-bearing:
 * with `--seeds 1` there is NO variance estimate, so an unfloored guard
 * of 0 would count a null result (delta=0) as SUPPORTS. Unknown variance now
 * returns null and cannot support a conclusion. A measured zero from repeated
 * seeds still uses the floor; noisy repeated cohorts use the larger 2×std.
 */
export function varianceGuard(ab, floor = 1.0) {
  const bs = ab?.baseline?.accuracy_std
  const ts = ab?.treatment?.accuracy_std
  if (bs == null || ts == null) return null
  return Math.max(2 * Math.max(bs, ts), floor)
}

/**
 * The testable core: given a graph, a control-bench contract, and a parsed bench
 * report, size the variance guard, run the metric_delta verifier, and fold the
 * verdict into the graph (signed SUPPORTS/CONTRADICTS edge; concludes on a
 * decisive verdict at confidence ≥ 0.6). Mutates `graph` in place; the caller
 * persists. No I/O, no spawn.
 *
 * @returns {{ab, verification, beliefs, contract, skipped?:string}}
 */
export function concludeFromReport(graph, contract, report, opts = {}) {
  const measured = (contract.verifier?.treatmentPath || 'treatment.accuracy_pct').split('.').pop()
  const ab = extractAB(report, measured)
  if (
    !ab.baseline ||
    !ab.treatment ||
    varianceGuard(ab) == null ||
    !Number.isFinite(ab.baseline[measured]) ||
    !Number.isFinite(ab.treatment[measured])
  ) {
    return {
      ab,
      verification: null,
      beliefs: null,
      contract,
      skipped: 'missing baseline/treatment metric or variance in report',
    }
  }

  const minDelta = varianceGuard(ab)
  const guarded = {
    ...contract,
    verifier: { ...contract.verifier, minDelta },
  }
  const data = {
    baseline: { [measured]: ab.baseline[measured] },
    treatment: { [measured]: ab.treatment[measured] },
  }
  const verification = verifyContract(guarded, { data })

  const now = opts.now || new Date().toISOString()
  const beliefs = updateBeliefs(graph, {
    hypothesisId: contract.hypothesisId,
    verdict: verification.verdict,
    confidence: verification.confidence,
    evidence:
      `Control Bench ${guarded.runner.suite}: ${TREATMENT_SUBJECT} ${measured}=${ab.treatment[measured]} ` +
      `vs ${BASELINE_SUBJECT} ${ab.baseline[measured]} (Δ=${(ab.treatment[measured] - ab.baseline[measured]).toFixed(2)}, ` +
      `guard ≥ ${minDelta.toFixed(2)}).`,
    experimentNodeId: `exp_${contract.id}`,
    now,
    bud: opts.bud ?? true,
  })
  return { ab, verification, beliefs, contract: guarded }
}

// ─── CLI ─────────────────────────────────────────────────────────────────────
// Orchestrates the live run: build the contract, spawn the bench A/B, parse the
// newest report, conclude, persist the graph, and save the contract outcome.

async function cli(argv) {
  const { readFileSync, writeFileSync, existsSync, readdirSync, statSync, mkdtempSync } =
    await import('./private-store-fs.mjs')
  const { spawnSync } = await import('node:child_process')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
  const flag = (n, d) => {
    const i = argv.indexOf(n)
    return i >= 0 ? argv[i + 1] : d
  }
  const has = (n) => argv.includes(n)

  const graphPath = flag('--graph', workerPaths().graph)
  const ledgerPath = flag('--ledger', process.env.ANGEL_EXPERIENCE_LOG) || workerPaths().ledger
  const suite = flag('--suite', process.env.ANGEL_REFLEX_SUITE || 'reflex')
  const seeds = Number(flag('--seeds', '2'))
  const quick = !has('--full')
  const dryRun = has('--dry-run')

  const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
  const {
    proposeConfigExperiment,
    seedConfigHypotheses,
    ingestLedgerObservations,
    configHypothesisId,
    KNOB_CATALOG,
  } = await import('./config-causal.mjs')

  const loadGraph = () =>
    existsSync(graphPath)
      ? CausalGraph.deserialize(JSON.parse(readFileSync(graphPath, 'utf8')))
      : new CausalGraph()
  const loadLedger = () => {
    if (!existsSync(ledgerPath)) return []
    return readFileSync(ledgerPath, 'utf8')
      .split('\n')
      .filter((l) => l.trim())
      .map((l) => {
        try {
          return JSON.parse(l)
        } catch {
          return null
        }
      })
      .filter(Boolean)
  }

  const graph = loadGraph()
  if ((await import('./config-causal.mjs')).configHypotheses(graph).length === 0) {
    seedConfigHypotheses(graph)
    ingestLedgerObservations(graph, loadLedger())
  }

  // Target: explicit --knob/--value, else the top EIG proposal.
  let knob = flag('--knob')
  let value = flag('--value')
  if (!knob || !value) {
    const { proposal } = proposeConfigExperiment(graph, { limit: 1 })
    if (!proposal) {
      console.error('no config experiment to run (empty graph?)')
      return 1
    }
    knob = proposal.knob
    value = proposal.value
    console.log(`top proposal: ${proposal.label}  (priority ${proposal.priority.toFixed(2)})`)
  }
  const cat = KNOB_CATALOG.find((k) => k.name === knob)
  if (!cat) {
    console.error(`unknown knob ${knob}; catalog: ${KNOB_CATALOG.map((k) => k.name).join(', ')}`)
    return 1
  }
  // The bench measures accuracy_pct, so conclude that sibling hypothesis.
  const hypothesisId = configHypothesisId(cat.knob, value, 'accuracy_pct')
  const baselineVal = flag('--baseline')
  const baselineKnobs = baselineVal ? { [knob]: baselineVal } : null

  const contract = contractFromControlExperiment(
    { hypothesisId, knob, value, label: `${knob}=${value}` },
    { suite, seeds, quick, baselineKnobs },
  )
  const spec = buildSubjectSpec(contract)

  console.log(`\ncontract ${contract.id}`)
  console.log(
    `  A/B: ${TREATMENT_SUBJECT}={${knob}:${value}} vs ${BASELINE_SUBJECT}=${baselineVal ? `{${knob}:${baselineVal}}` : '(live baseline)'}`,
  )
  console.log(`  suite ${suite} · seeds ${seeds} · quick ${quick} · concludes ${hypothesisId}`)

  if (dryRun) {
    console.log('\n--dry-run: subject-spec that WOULD run:')
    console.log(JSON.stringify(spec, null, 2))
    return 0
  }

  const { report } = runBenchmark(spec)
  const { verification, ab, skipped } = concludeFromReport(graph, contract, report)
  if (skipped) {
    console.error(`could not conclude: ${skipped}`)
    return 1
  }

  writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
  console.log(
    `\nverdict: ${verification.verdict.toUpperCase()} (confidence ${verification.confidence})\n` +
      `  ${TREATMENT_SUBJECT} ${ab.treatment.accuracy_pct}% vs ${BASELINE_SUBJECT} ${ab.baseline.accuracy_pct}%\n` +
      `  ${verification.evidence}\n  graph updated → ${graphPath}`,
  )
  return 0
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
