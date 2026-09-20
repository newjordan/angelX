// experiment-contracts — durable contracts for first-class discovery cycles.
//
// A contract turns "try this hypothesis" into a replayable artifact: what is
// being tested, how Angel should run it, how the result must be verified, and
// what verdict was ingested. The control bridge owns execution; this module
// stays local, deterministic, and testable.

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  writeFileSync,
} from './private-store-fs.mjs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
const STORE_DIR = process.env.ANGEL_EXPERIMENT_DIR || join(ROOT, '.angel-experiments')
const CONTRACT_DIR = join(STORE_DIR, 'contracts')

const VALID_VERDICTS = new Set(['supports', 'contradicts', 'inconclusive'])

function nowIso() {
  return new Date().toISOString()
}

function safeId(s) {
  const id = String(s || 'contract')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80)
  return id || 'contract'
}

function contractPath(id) {
  return join(CONTRACT_DIR, `${safeId(id)}.json`)
}

function ensureStore() {
  mkdirSync(CONTRACT_DIR, { recursive: true })
}

function clampConfidence(n, fallback = 0.7) {
  const x = Number(n)
  if (!Number.isFinite(x)) return fallback
  return Math.min(1, Math.max(0.01, x))
}

function explicitVerdictFromText(text) {
  const marker = String(text || '').match(
    /^\s*VERDICT\s*[:=-]\s*(SUPPORTS|CONTRADICTS|INCONCLUSIVE)\s*$/im,
  )
  if (!marker) return null
  const confidenceMatch = String(text || '').match(
    /^\s*CONFIDENCE\s*[:=-]\s*(0?\.\d+|1(?:\.0+)?|0|1)\s*$/im,
  )
  const raw = marker[1].toLowerCase()
  return {
    verdict:
      raw === 'supports' ? 'supports' : raw === 'contradicts' ? 'contradicts' : 'inconclusive',
    confidence: confidenceMatch ? clampConfidence(confidenceMatch[1]) : 0.7,
  }
}

function textFromArtifacts(artifacts = {}) {
  const parts = []
  if (artifacts.text) parts.push(String(artifacts.text))
  if (artifacts.report) parts.push(String(artifacts.report))
  const record = artifacts.record
  if (Array.isArray(record?.log)) parts.push(record.log.map((l) => String(l?.msg || '')).join('\n'))
  if (record) parts.push(JSON.stringify(record))
  if (artifacts.data) parts.push(JSON.stringify(artifacts.data))
  return parts.join('\n')
}

function readPath(obj, path) {
  if (!path) return undefined
  return String(path)
    .split('.')
    .filter(Boolean)
    .reduce((acc, key) => {
      if (acc == null) return undefined
      if (Array.isArray(acc) && /^\d+$/.test(key)) return acc[Number(key)]
      return acc[key]
    }, obj)
}

function metricDeltaVerdict(contract, artifacts) {
  const verifier = contract.verifier || {}
  const source = artifacts?.data ?? artifacts?.record ?? artifacts ?? {}
  const baseline = Number(readPath(source, verifier.baselinePath))
  const treatment = Number(readPath(source, verifier.treatmentPath))
  if (!Number.isFinite(baseline) || !Number.isFinite(treatment)) {
    return {
      verdict: 'inconclusive',
      confidence: 0.35,
      evidence: `Metric verifier could not read numeric baseline/treatment paths for ${contract.id}.`,
      metrics: { baseline: null, treatment: null, delta: null },
    }
  }

  const delta = treatment - baseline
  const minDelta = Number.isFinite(Number(verifier.minDelta)) ? Number(verifier.minDelta) : 0
  const direction = verifier.direction || contract.metric?.direction || 'higher'
  const supports =
    direction === 'lower' || direction === 'smaller' || direction === 'down'
      ? delta <= -Math.abs(minDelta)
      : delta >= Math.abs(minDelta)
  const contradicts =
    direction === 'lower' || direction === 'smaller' || direction === 'down'
      ? delta > Math.abs(minDelta)
      : delta < -Math.abs(minDelta)
  const verdict = supports ? 'supports' : contradicts ? 'contradicts' : 'inconclusive'
  const confidence = verdict === 'inconclusive' ? 0.45 : 0.8
  return {
    verdict,
    confidence,
    evidence:
      `Metric verifier compared ${verifier.treatmentPath} (${treatment}) against ` +
      `${verifier.baselinePath} (${baseline}); delta=${delta}.`,
    metrics: { baseline, treatment, delta },
  }
}

export function contractFromExperiment(experiment, opts = {}) {
  if (!experiment?.hypothesisId) throw new Error('contractFromExperiment: hypothesisId is required')
  const createdAt = opts.now || nowIso()
  const suffix = safeId(createdAt.replace(/\.\d+z$/i, 'z'))
  const id = opts.id || `contract-${safeId(experiment.hypothesisId)}-${suffix}`
  const profiles =
    Array.isArray(opts.profiles) && opts.profiles.length ? opts.profiles : ['gepa', 'slop']
  const verifier = opts.verifier || {
    type: 'explicit_verdict',
    verdictMarker: 'VERDICT',
    confidenceMarker: 'CONFIDENCE',
    fallback: 'inconclusive',
  }
  const prompt = [
    opts.prompt || experiment.prompt,
    '',
    'EXPERIMENT CONTRACT:',
    `Contract id: ${id}`,
    `Hypothesis id: ${experiment.hypothesisId}`,
    `Prediction: ${experiment.prediction || '(none supplied)'}`,
    'End the report with standalone machine-readable lines:',
    'VERDICT: SUPPORTS|CONTRADICTS|INCONCLUSIVE',
    'CONFIDENCE: 0.00-1.00',
    'Use INCONCLUSIVE if the run did not directly test the prediction.',
  ]
    .filter(Boolean)
    .join('\n')

  return {
    version: 1,
    id,
    status: 'proposed',
    createdAt,
    updatedAt: createdAt,
    hypothesisId: experiment.hypothesisId,
    label: experiment.label || experiment.hypothesisId,
    question: experiment.question || '',
    prediction: experiment.prediction || '',
    metric: experiment.metric || null,
    score: experiment.score ?? null,
    rationale: experiment.rationale || '',
    runner: {
      type: 'autoresearch',
      target: {
        kind: 'hypothesis',
        title: experiment.target_title || experiment.label || experiment.hypothesisId,
        hypothesisId: experiment.hypothesisId,
        contractId: id,
      },
      profiles,
      budgetUsd: opts.budgetUsd ?? 25,
      maxLoops: opts.maxLoops ?? 1,
      report: opts.report ?? 'thorough',
      prompt,
    },
    verifier,
    artifacts: [],
    verification: null,
    ingestion: null,
  }
}

/**
 * A control-bench contract: the interventional half of Reflex (M3). Turns a
 * config-experiment proposal (config-causal.mjs) into a replayable A/B —
 * baseline knobs vs treatment knobs on a frozen Control Bench suite, verified by
 * `metric_delta` on the bench's quality metric (accuracy_pct, pass@1). The
 * `intervention` is the do-operator: its knob maps become the child `angel`
 * process env via subjects.combo_env. `minDelta` is a placeholder — the runner
 * sizes it to 2× the observed accuracy_std (the variance guard) before verifying.
 *
 * @param {{hypothesisId, knob?, value?, knobs?, metric?, label?, prediction?, question?, score?, rationale?}} experiment
 * @param {{now?, id?, suite?, seeds?, quick?, ids?, baselineKnobs?, minDelta?, measuredMetric?, prompt?}} [opts]
 */
export function contractFromControlExperiment(experiment, opts = {}) {
  if (!experiment?.hypothesisId)
    throw new Error('contractFromControlExperiment: hypothesisId is required')
  const createdAt = opts.now || nowIso()
  const suffix = safeId(createdAt.replace(/\.\d+z$/i, 'z'))
  const id = opts.id || `contract-${safeId(experiment.hypothesisId)}-${suffix}`

  const knobs =
    experiment.knobs ||
    (experiment.knob ? { [experiment.knob]: String(experiment.value ?? '') } : {})
  const baselineKnobs = opts.baselineKnobs ?? experiment.baselineKnobs ?? null
  const suite = opts.suite || experiment.suite || 'control-code-v2'
  // The bench grades one quality axis (accuracy_pct / pass@1, higher-is-better),
  // so that is what metric_delta compares — regardless of which ledger-native
  // metric first minted the hypothesis. The runner concludes the accuracy_pct
  // hypothesis for this knob/value.
  const measured = opts.measuredMetric || 'accuracy_pct'

  return {
    version: 1,
    id,
    status: 'proposed',
    createdAt,
    updatedAt: createdAt,
    hypothesisId: experiment.hypothesisId,
    label: experiment.label || experiment.hypothesisId,
    question: experiment.question || '',
    prediction: experiment.prediction || '',
    metric: experiment.metric || { name: measured, direction: 'higher' },
    score: experiment.score ?? null,
    rationale: experiment.rationale || '',
    // The do-operator: knob maps layered onto the child angel process env.
    intervention: { knobs, baselineKnobs },
    runner: {
      type: 'control-bench',
      suite,
      seeds: opts.seeds ?? 2,
      quick: opts.quick ?? true,
      ids: opts.ids ?? null,
    },
    verifier: {
      type: 'metric_delta',
      baselinePath: `baseline.${measured}`,
      treatmentPath: `treatment.${measured}`,
      direction: 'higher', // accuracy_pct: higher is better
      minDelta: opts.minDelta ?? 0, // runner overrides with 2× accuracy_std
    },
    artifacts: [],
    verification: null,
    ingestion: null,
  }
}

export function saveContract(contract) {
  if (!contract?.id) throw new Error('saveContract: contract.id is required')
  ensureStore()
  const updated = { ...contract, updatedAt: nowIso() }
  const file = contractPath(updated.id)
  const tmp = `${file}.tmp`
  writeFileSync(tmp, JSON.stringify(updated, null, 2))
  renameSync(tmp, file)
  return updated
}

export function readContract(id) {
  const file = contractPath(id)
  if (!existsSync(file)) return null
  try {
    return JSON.parse(readFileSync(file, 'utf8'))
  } catch {
    return null
  }
}

export function listContracts() {
  if (!existsSync(CONTRACT_DIR)) return []
  return readdirSync(CONTRACT_DIR)
    .filter((f) => f.endsWith('.json'))
    .map((f) => {
      try {
        return JSON.parse(readFileSync(join(CONTRACT_DIR, f), 'utf8'))
      } catch {
        return null
      }
    })
    .filter(Boolean)
    .sort(
      (a, b) =>
        new Date(b.updatedAt || b.createdAt || 0) - new Date(a.updatedAt || a.createdAt || 0),
    )
}

export function verifyContract(contract, artifacts = {}) {
  if (!contract?.id) throw new Error('verifyContract: contract.id is required')
  const verifier = contract.verifier || { type: 'explicit_verdict' }
  let out
  if (verifier.type === 'metric_delta') {
    out = metricDeltaVerdict(contract, artifacts)
  } else {
    const parsed = explicitVerdictFromText(textFromArtifacts(artifacts))
    if (parsed) {
      out = {
        ...parsed,
        evidence: `Contract ${contract.id} found explicit ${parsed.verdict.toUpperCase()} verdict.`,
      }
    } else if (artifacts?.record?.ok === false || artifacts?.record?.status === 'failed') {
      out = {
        verdict: 'inconclusive',
        confidence: 0.35,
        evidence: `Contract ${contract.id} run failed before producing a verdict.`,
      }
    } else {
      out = {
        verdict: verifier.fallback === 'contradicts' ? 'contradicts' : 'inconclusive',
        confidence: artifacts?.record?.ok ? 0.55 : 0.35,
        evidence: `Contract ${contract.id} did not produce a standalone verdict marker.`,
      }
    }
  }

  const verdict = VALID_VERDICTS.has(out.verdict) ? out.verdict : 'inconclusive'
  return {
    ok: true,
    contractId: contract.id,
    hypothesisId: contract.hypothesisId,
    verifier: verifier.type || 'explicit_verdict',
    verifiedAt: nowIso(),
    verdict,
    confidence: clampConfidence(out.confidence, verdict === 'inconclusive' ? 0.45 : 0.7),
    evidence: out.evidence || '',
    metrics: out.metrics || null,
  }
}

export function attachContractOutcome(contract, outcome) {
  if (!contract?.id) throw new Error('attachContractOutcome: contract.id is required')
  const artifacts = Array.isArray(contract.artifacts) ? contract.artifacts : []
  const next = {
    ...contract,
    status: outcome?.status || (outcome?.verification ? 'verified' : contract.status || 'proposed'),
    artifacts: outcome?.artifact ? [...artifacts, outcome.artifact] : artifacts,
    verification: outcome?.verification || contract.verification || null,
    ingestion: outcome?.ingestion || contract.ingestion || null,
  }
  return saveContract(next)
}

export const _internal = { explicitVerdictFromText, readPath, safeId }
