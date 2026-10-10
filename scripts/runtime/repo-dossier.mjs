// repo-dossier — worker/CLI adapter. Pure mining and graph projections live in
// lib/dossier/core.mjs; this layer owns paths, store caps, redaction and file I/O.
// Preserve the historical import surface while consumers migrate to the core.
import { runGraphCli } from './worker-lock.mjs'
import { storeCap } from './store-caps.mjs'
import { join } from 'node:path'
import { homedir } from 'node:os'
import { existsSync, realpathSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import {
  assertRepoKey,
  mineRepoFacts,
  ingestRepoFacts,
  cutSample,
  dossierFacts,
  compileDossier,
  proposeDossierProbe,
} from '../../lib/dossier/core.mjs'

export * from '../../lib/dossier/core.mjs'

export function dossierPaths(argv = [], env = process.env, userHomeDir = homedir()) {
  const flag = (name) => {
    const index = argv.indexOf(name)
    return index >= 0 ? argv[index + 1] : undefined
  }
  const configured = (value) => (typeof value === 'string' && value.trim() ? value : undefined)
  const stateDir = join(userHomeDir, '.angelX')
  const outDir = flag('--out') || configured(env.ANGEL_DOSSIER_DIR) || join(stateDir, 'dossier')
  return {
    graphPath: flag('--graph') || configured(env.ANGEL_CAUSAL_GRAPH) || join(outDir, 'graph.json'),
    ledgerPath:
      flag('--ledger') ||
      configured(env.ANGEL_EXPERIENCE_LOG) ||
      join(stateDir, 'experience', 'ledger.jsonl'),
    outDir,
    cutDir: flag('--cut') || configured(env.ANGEL_CUT_DIR) || join(stateDir, 'cut'),
    onlyRepo: flag('--repo'),
  }
}

// Named alongside the Rust stores in docs/telemetry/store-caps.toml.
export const DOSSIER_STORE_BYTES = storeCap('dossier_artifact', 'max_bytes')
export const DOSSIER_STORE_FACTS = storeCap('dossier_artifact', 'max_entries')

// Applied at both publication sites, after the canonical redaction pass.
export function boundedDossierText(artifact, redact = (text) => text) {
  const facts = [...artifact.facts]
    .sort((a, b) => String(b.lastVerifiedAt || '').localeCompare(String(a.lastVerifiedAt || '')))
    .slice(0, DOSSIER_STORE_FACTS)
  const encode = (count) =>
    redact(JSON.stringify({ ...artifact, facts: facts.slice(0, count) }, null, 2))
  let low = 0
  let high = facts.length
  let best = encode(0)
  if (Buffer.byteLength(best) > DOSSIER_STORE_BYTES)
    throw new Error('Dossier metadata exceeds store cap')
  while (low <= high) {
    const mid = Math.floor((low + high) / 2)
    const text = encode(mid)
    if (Buffer.byteLength(text) <= DOSSIER_STORE_BYTES) {
      best = text
      low = mid + 1
    } else high = mid - 1
  }
  return best
}

// ─── CLI ─────────────────────────────────────────────────────────────────────
// I/O lives here only. `--mine` folds ledger events into the shared graph;
// `--compile` writes one artifact per repo key (or `--repo <key>`); `--rank`
// prints the probe queue. Invoke one writer per graph file at a time.

async function cli(argv) {
  const { readFileSync, writeFileSync, mkdirSync, existsSync, redactText } =
    await import('./private-store-fs.mjs')
  const { graphPath, ledgerPath, outDir, cutDir, onlyRepo } = dossierPaths(argv)
  if (onlyRepo !== undefined) assertRepoKey(onlyRepo)
  const refresh = argv.includes('--refresh')

  const { collectManifest } = await import('./cut-evidence.mjs')
  const CausalGraph = (await import('../../lib/research/CausalGraph.js')).default
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

  if (argv.includes('--mine') || refresh) {
    const graph = loadGraph()
    const cutRows = collectManifest(cutDir)
    const mined = mineRepoFacts(loadLedger(), cutRows)
    let nodes = 0
    let updated = 0
    for (const [key, rec] of Object.entries(mined)) {
      if (onlyRepo && key !== onlyRepo) continue
      assertRepoKey(key)
      const { added } = ingestRepoFacts(graph, key, rec)
      nodes += added.nodes
      updated += added.updated
    }
    writeFileSync(graphPath, JSON.stringify(graph.serialize(), null, 2))
    // The two sources are reported separately on purpose. The ledger's count is
    // recurrence; the Cut's is VERDICTS. Conflating them is how this subsystem
    // spent its whole life believing it had evidence it did not have.
    const verdicts = cutRows.filter((r) => cutSample(r)).length
    const independent = cutRows.filter((r) => cutSample(r)?.independent).length
    console.log(
      `dossier mine → ${graphPath}\n` +
        `  ${Object.keys(mined).length} repo(s) in ${ledgerPath}\n` +
        `  ${cutRows.length} cut row(s) in ${cutDir} → ` +
        `${verdicts} verdict(s), ${independent} independent\n` +
        `  ${nodes} new + ${updated} refreshed fact(s)`,
    )
    if (!refresh) return 0
  }

  if (argv.includes('--compile') || refresh) {
    const graph = loadGraph()
    const mined = mineRepoFacts(loadLedger(), collectManifest(cutDir))
    const keys = onlyRepo ? [onlyRepo] : [...new Set(dossierFacts(graph).map((n) => n.repoKey))]
    mkdirSync(outDir, { recursive: true })
    for (const key of keys) {
      assertRepoKey(key)
      const artifact = compileDossier(graph, key, { thread: mined[key]?.thread ?? null })
      const path = join(outDir, `${key}.json`)
      writeFileSync(path, boundedDossierText(artifact, redactText))
      console.log(`dossier compile → ${path} (${artifact.facts.length} fact(s))`)
    }
    if (keys.length === 0) console.log('no dossier facts in the graph — run --mine first')
    return 0
  }

  if (argv.includes('--rank')) {
    const graph = loadGraph()
    const { ranking } = proposeDossierProbe(graph, onlyRepo, { limit: 12 })
    console.log(`dossier probe queue (top ${ranking.length}):\n`)
    ranking.forEach((r, i) =>
      console.log(
        `${String(i + 1).padStart(2)}. ${r.label}\n    priority ${r.priority.toFixed(3)} · ${r.rationale}`,
      ),
    )
    if (ranking.length === 0) console.log('  (no dossier facts)')
    return 0
  }

  console.log(
    'usage: node scripts/runtime/repo-dossier.mjs --refresh|--mine|--compile|--rank [--repo <key>] [--graph <path>] [--ledger <path>] [--cut <dir>] [--out <dir>]',
  )
  return 2
}

const isCli =
  process.argv[1] &&
  existsSync(process.argv[1]) &&
  import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href
if (isCli) {
  runGraphCli(cli, process.argv.slice(2)).then((code) => process.exit(code ?? 0))
}
