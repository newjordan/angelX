import { runGraphCli } from './worker-lock.mjs'
import { workerPaths } from './worker-paths.mjs'
// reflex-reconfigure — write concluded config wins back as a launch overlay.
//
// The output half of Reflex (M4). Reads the causal graph, selects config
// hypotheses the interventional channel has actually CONCLUDED (belief ≥ 0.85 and
// backed by ≥1 Control Bench SUPPORTS edge — an observational-only hypothesis, or
// one confirmed by a single run below threshold, never qualifies), and
// regenerates `<repo>/.angel.auto.env`: a whole-file overlay of winning knob
// values with the evidence attached in comments.
//
// Promotion rules (docs/WORKERS.md):
//   • Never writes `.angel.env` (that holds live API keys) and never emits a var
//     whose name matches KEY|TOKEN|SECRET|AUTH|PASSWORD. Only KNOB_CATALOG knobs.
//   • Tier A (pure routing knobs) auto-writes; Tier B is emitted commented-out
//     plus a proposals note — a human opts in by copying it into `.angel.env`.
//   • The overlay is sourced immediately BEFORE `.angel.env` (bin/angel0), so a
//     hand pin ALWAYS wins; to delegate a knob to reflex, delete the hand pin.
//   • Every regeneration archives the previous overlay (rollback = copy back or
//     just delete `.angel.auto.env`; `ANGEL_REFLEX=0` disables the whole loop).
//   • Env is read once at launch everywhere — a change takes effect next start.
//     No hot-reload (there is none in the cockpit; do not build one).
//
// Pure core (selectWinners / renderOverlay / renderProposals) is separated from
// the file I/O in the CLI so the selection + rendering is unit-tested with a
// hand-built graph — no disk, no clock.

import { beliefProbability } from './causal-loop.mjs'
import { NODE_TYPE, EDGE_TYPE } from '../../lib/research/CausalGraph.js'
import { KNOB_CATALOG, CONFIG_PROJECT } from './config-causal.mjs'

// Conclusion threshold: reflex only writes a knob once its belief clears this.
export const CONCLUDE_THRESHOLD = 0.85

// Tier A — pure routing knobs reflex may AUTO-write (reversible by deleting the
// overlay). Everything else in the catalog is Tier B (propose-only).
export const TIER_A_KNOBS = new Set([
  'ANGEL_SOTA_MOA_PROPOSE_CLUB',
  'ANGEL_SOTA_MOA_JUDGE_CLUB',
  'ANGEL_SOTA_MOA_VERIFY_CLUB',
  'ANGEL_SOTA_MOA_AGG_CLUB',
  'ANGEL_MOA_JUDGE_FANOUT',
  'ANGEL_SOTA_MOA_JUDGE',
  'ANGEL_SOTA_MOA_VERIFY',
])

// Belt-and-suspenders secrets denylist (mirrors experience.rs). A catalog knob
// never matches this, but the overlay writer refuses to emit one regardless.
const SECRET_RE = /KEY|TOKEN|SECRET|AUTH|PASSWORD/i
export const isSecretName = (name) => SECRET_RE.test(String(name || ''))

const CATALOG_NAMES = new Set(KNOB_CATALOG.map((k) => k.name))

/**
 * The concluded, interventional-backed config wins — one per knob.
 *
 * A hypothesis qualifies iff: it is a reflex config hypothesis on a catalog knob,
 * it has ≥1 incoming SUPPORTS edge (only the interventional channel adds edges —
 * observational mining never does), its belief ≥ threshold, and it did not
 * conclude negative. Per knob, the highest-belief winner is kept.
 *
 * @param {CausalGraph} graph
 * @param {{threshold?:number}} [opts]
 * @returns {Array<{knobName,value,belief,tier,hypothesisId,metric,benchEvidence,ledgerEvidence}>}
 */
export function selectWinners(graph, opts = {}) {
  const threshold = opts.threshold ?? CONCLUDE_THRESHOLD

  const supportsByDst = new Map()
  for (const e of graph.edges()) {
    if (e.type !== EDGE_TYPE.SUPPORTS) continue
    const arr = supportsByDst.get(e.dst) || []
    arr.push(e)
    supportsByDst.set(e.dst, arr)
  }

  const byKnob = new Map()
  for (const node of graph.nodesOfType(NODE_TYPE.HYPOTHESIS)) {
    if (node.projectId !== CONFIG_PROJECT.id) continue
    if (!node.knobName || !CATALOG_NAMES.has(node.knobName)) continue
    if (isSecretName(node.knobName)) continue // never, even if catalogued
    if (node.outcome === 'negative') continue // contradicted → don't write it
    const supports = supportsByDst.get(node.id)
    if (!supports || supports.length === 0) continue // interventional-backed only
    const belief = beliefProbability(graph, node.id)
    if (belief < threshold) continue

    const cur = byKnob.get(node.knobName)
    if (!cur || belief > cur.belief) byKnob.set(node.knobName, { node, belief, supports })
  }

  return [...byKnob.values()]
    .map(({ node, belief, supports }) => ({
      knobName: node.knobName,
      value: node.knobValue,
      belief,
      tier: TIER_A_KNOBS.has(node.knobName) ? 'A' : 'B',
      hypothesisId: node.id,
      metric: node.metric?.name || null,
      // The most recent SUPPORTS edge's label carries the metric_delta evidence.
      benchEvidence:
        supports
          .map((e) => e.label)
          .filter(Boolean)
          .pop() || null,
      ledgerEvidence: node.observed || null,
    }))
    .sort((a, b) => (a.knobName < b.knobName ? -1 : a.knobName > b.knobName ? 1 : 0))
}

function evidenceComment(w) {
  const parts = [`p=${w.belief.toFixed(2)}`]
  if (w.benchEvidence) parts.push(w.benchEvidence)
  if (
    w.ledgerEvidence &&
    Number.isFinite(w.ledgerEvidence.gradient) &&
    w.ledgerEvidence.baseline !== null
  ) {
    const g = w.ledgerEvidence.gradient
    parts.push(
      `ledger ${w.metric} ${g >= 0 ? '+' : ''}${g.toFixed(3)} over ${w.ledgerEvidence.samples} turn(s)`,
    )
  }
  return `# ${parts.join(' · ')}`
}

/**
 * Render the whole `.angel.auto.env` file from the selected winners. Tier A knobs
 * are live `NAME=value` lines; Tier B are commented `# NAME=value` proposals.
 * A secret-named or non-catalog winner is refused (belt-and-suspenders). Pure.
 */
export function renderOverlay(winners, opts = {}) {
  const ts = opts.now || new Date().toISOString()
  const safe = winners.filter((w) => CATALOG_NAMES.has(w.knobName) && !isSecretName(w.knobName))
  const tierA = safe.filter((w) => w.tier === 'A')
  const tierB = safe.filter((w) => w.tier === 'B')

  const out = [
    `# GENERATED by reflex ${ts} — do not hand-edit. Hand pins in .angel.env win.`,
    `# Sourced immediately BEFORE .angel.env (bin/angel0); delete a pin there to`,
    `# delegate that knob to reflex. Rollback: delete this file, or ANGEL_REFLEX=0.`,
    `# Every value below concluded at belief ≥ ${CONCLUDE_THRESHOLD} with a Control Bench SUPPORTS edge.`,
    '',
  ]
  if (!tierA.length) out.push('# (no Tier-A routing knobs concluded yet)', '')
  for (const w of tierA) {
    out.push(evidenceComment(w), `${w.knobName}=${w.value}`, '')
  }
  if (tierB.length) {
    out.push(
      '# ── Tier B (propose-only) — reflex will not auto-apply these. To adopt one,',
      '#    copy it (uncommented) into .angel.env yourself. Also in ~/.angel0/reflex/proposals.md.',
      '',
    )
    for (const w of tierB) {
      out.push(evidenceComment(w), `# ${w.knobName}=${w.value}`, '')
    }
  }
  return out.join('\n').replace(/\n+$/, '\n')
}

/** The `~/.angel0/reflex/proposals.md` body for the Tier-B winners. Pure. */
export function renderProposals(winners, opts = {}) {
  const ts = opts.now || new Date().toISOString()
  const tierB = winners.filter((w) => w.tier === 'B' && CATALOG_NAMES.has(w.knobName))
  const out = [`# Reflex Tier-B proposals — updated ${ts}`, '']
  if (!tierB.length) {
    out.push('_No Tier-B knobs have concluded yet._')
    return out.join('\n') + '\n'
  }
  out.push('These concluded with interventional evidence but are propose-only (not')
  out.push('routing knobs). Adopt one by adding it to `.angel.env`:', '')
  for (const w of tierB) {
    out.push(`- \`${w.knobName}=${w.value}\` — ${evidenceComment(w).replace(/^# /, '')}`)
  }
  return out.join('\n') + '\n'
}

// ─── CLI ─────────────────────────────────────────────────────────────────────
// --dry-run prints the overlay + proposals; --apply writes them (archiving the
// previous overlay first). The graph has no locking — the reflex tick (M5) takes
// a lockfile; this is a manual one-shot.

async function cli(argv) {
  const { readFileSync, writeFileSync, existsSync, mkdirSync, copyFileSync } =
    await import('./private-store-fs.mjs')
  const { fileURLToPath } = await import('node:url')
  const { dirname, join } = await import('node:path')
  const os = await import('node:os')

  const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..')
  const flag = (n, d) => {
    const i = argv.indexOf(n)
    return i >= 0 ? argv[i + 1] : d
  }
  const apply = argv.includes('--apply')
  const graphPath = flag('--graph', workerPaths().graph)
  const overlayPath = flag('--overlay', join(ROOT, '.angel.auto.env'))
  const threshold = Number(flag('--threshold', String(CONCLUDE_THRESHOLD)))
  const reflexDir = workerPaths().reflex

  const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
  const graph = existsSync(graphPath)
    ? CausalGraph.deserialize(JSON.parse(readFileSync(graphPath, 'utf8')))
    : new CausalGraph()

  const winners = selectWinners(graph, { threshold })
  const overlay = renderOverlay(winners)
  const proposals = renderProposals(winners)
  const tierA = winners.filter((w) => w.tier === 'A')
  const tierB = winners.filter((w) => w.tier === 'B')

  if (!apply) {
    console.log(
      `reflex reconfigure (dry-run) · ${winners.length} concluded win(s) · threshold ${threshold}`,
    )
    console.log(
      `  Tier A (would auto-write): ${tierA.map((w) => `${w.knobName}=${w.value}`).join(', ') || '(none)'}`,
    )
    console.log(
      `  Tier B (propose-only):     ${tierB.map((w) => `${w.knobName}=${w.value}`).join(', ') || '(none)'}`,
    )
    console.log(`\n─── ${overlayPath} would become ───\n${overlay}`)
    return 0
  }

  mkdirSync(reflexDir, { recursive: true })
  mkdirSync(join(reflexDir, 'overlays'), { recursive: true })
  // Archive the previous overlay before overwriting (rollback path).
  if (existsSync(overlayPath)) {
    const stamp = new Date()
      .toISOString()
      .replace(/[^0-9]/g, '')
      .slice(0, 14)
    copyFileSync(overlayPath, join(reflexDir, 'overlays', `${stamp}.env`))
  }
  // Hard rule: refuse to touch .angel.env.
  if (/\.angel\.env$/.test(overlayPath)) {
    console.error('refusing to write .angel.env — reflex only writes .angel.auto.env')
    return 1
  }
  writeFileSync(overlayPath, overlay)
  writeFileSync(join(reflexDir, 'proposals.md'), proposals)
  console.log(
    `reflex reconfigure applied → ${overlayPath}\n` +
      `  Tier A written: ${tierA.map((w) => w.knobName).join(', ') || '(none)'}\n` +
      `  Tier B proposals: ${join(reflexDir, 'proposals.md')} (${tierB.length})\n` +
      `  takes effect on next \`angel0\` start (env is read once at launch).`,
  )
  return 0
}

const isCli =
  process.argv[1] && (await import('node:url')).fileURLToPath(import.meta.url) === process.argv[1]
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
